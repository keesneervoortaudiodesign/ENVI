//! GEOX-01 cut-profile oracle test (09-01), mirroring `cog_window.rs`'s
//! committed-artifact pattern.
//!
//! Loads the committed real-DEM extract `tests/fixtures/rprofile/rprofile_dem.tif`
//! and the reference `rprofile.csv` (both generated offline by
//! `tools/rprofile_oracle/gen_rprofile_fixture.py`), builds a TIN from the DEM's
//! pixel-center samples, runs `envi_gis::profile::cut_profile`, and asserts the
//! extracted ground-z series matches the **independent raster-bilinear** reference
//! within the fixture's documented tolerance — the TIN-linear vs raster-bilinear
//! kernel delta, not bit-equality (09-RESEARCH assumption A3).
//!
//! The reference is produced by a bilinear sampler written from scratch in the
//! generator, never by the ENVI extractor, so this is not a self-referential
//! fixture (09-01 prohibition). **Python / GDAL / GRASS are NOT needed at test
//! time** — the `.tif` + `.csv` are committed data.

use envi_gis::cog::window::PixelWindow;
use envi_gis::cog::{MAX_DECODED_PX, decode_window};
use envi_gis::profile::cut_profile;

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/rprofile/");

/// The parsed oracle fixture: the DEM dimensions, the S→R endpoints + step, the
/// tolerance, and the expected `(x, z)` reference rows.
struct Fixture {
    width: usize,
    height: usize,
    s_xy: [f64; 2],
    r_xy: [f64; 2],
    step_m: f64,
    tol: f64,
    rows: Vec<[f64; 2]>,
}

/// Parse the CSV: `# key = value` comment lines carry the numeric metadata, then
/// an `x,z` header, then the reference data rows.
fn load_fixture() -> Fixture {
    let text = std::fs::read_to_string(format!("{DIR}rprofile.csv"))
        .unwrap_or_else(|e| panic!("rprofile.csv must exist: {e}"));

    let num = |key: &str| -> f64 {
        for line in text.lines() {
            let Some(rest) = line.strip_prefix("# ") else {
                continue;
            };
            let Some((k, v)) = rest.split_once('=') else {
                continue;
            };
            if k.trim() == key {
                return v
                    .trim()
                    .parse::<f64>()
                    .unwrap_or_else(|e| panic!("metadata {key} must be numeric: {e}"));
            }
        }
        panic!("metadata {key} not found in rprofile.csv");
    };

    let mut rows = Vec::new();
    let mut in_data = false;
    for line in text.lines() {
        if line.starts_with('#') {
            continue;
        }
        if line.trim() == "x,z" {
            in_data = true;
            continue;
        }
        if in_data && !line.trim().is_empty() {
            let (x, z) = line.split_once(',').expect("data row is `x,z`");
            rows.push([
                x.trim().parse::<f64>().expect("x is numeric"),
                z.trim().parse::<f64>().expect("z is numeric"),
            ]);
        }
    }
    assert!(!rows.is_empty(), "the reference CSV must carry sample rows");

    Fixture {
        width: num("width") as usize,
        height: num("height") as usize,
        s_xy: [num("s_x"), num("s_y")],
        r_xy: [num("r_x"), num("r_y")],
        step_m: num("step_m"),
        tol: num("tol"),
        rows,
    }
}

#[test]
fn cut_profile_matches_the_rprofile_oracle_within_tolerance() {
    let fx = load_fixture();

    // Decode the committed DEM extract through the real pure-Rust COG path.
    let bytes = std::fs::read(format!("{DIR}rprofile_dem.tif"))
        .unwrap_or_else(|e| panic!("rprofile_dem.tif must exist: {e}"));
    let raster = decode_window(
        &bytes,
        PixelWindow {
            col_off: 0,
            row_off: 0,
            width: fx.width as u32,
            height: fx.height as u32,
        },
        MAX_DECODED_PX,
    )
    .expect("committed DEM decodes");

    // Build the DGM TIN from the DEM's pixel-CENTER samples (the same seam
    // `envi-dgm` uses for imported terrain).
    let mut points: Vec<[f64; 3]> = Vec::with_capacity(fx.width * fx.height);
    for row in 0..fx.height {
        for col in 0..fx.width {
            let z = raster
                .get(col, row)
                .expect("the DEM extract has no nodata holes");
            let (mx, my) = raster
                .geo
                .pixel_to_map(col as f64 + 0.5, row as f64 + 0.5);
            points.push([mx, my, f64::from(z)]);
        }
    }
    let tin = envi_dgm::tin::build_tin(&points, &[]).expect("DEM samples build a TIN");

    // Run the ENVI extractor over the same S→R line the oracle walked.
    let profile = cut_profile(&tin, fx.s_xy, fx.r_xy, fx.step_m).expect("cut_profile succeeds");

    // The sampling geometry is shared, so the point count + x positions align 1:1
    // with the reference; only the interpolation kernel differs.
    assert_eq!(
        profile.len(),
        fx.rows.len(),
        "extracted point count must match the oracle walk"
    );

    let mut max_dz: f64 = 0.0;
    for (got, exp) in profile.iter().zip(&fx.rows) {
        assert!(
            (got[0] - exp[0]).abs() < 1e-6,
            "x position {} must align with the oracle {}",
            got[0],
            exp[0]
        );
        // Strictly-ascending, ground-z-only invariants ride along on the real data.
        let dz = (got[1] - exp[1]).abs();
        max_dz = max_dz.max(dz);
        assert!(
            dz <= fx.tol,
            "z {} vs oracle {} exceeds tol {} (Δ = {dz})",
            got[1],
            exp[1],
            fx.tol
        );
    }
    // The delta must be real (TIN ≠ raster-bilinear on the curved DEM) yet bounded
    // — a zero delta would mean the fixture is trivially planar.
    assert!(
        max_dz > 0.0,
        "the TIN vs raster-bilinear delta should be non-zero on a curved DEM"
    );
    assert!(profile[0][0] == 0.0, "x starts at the source ground point");
}
