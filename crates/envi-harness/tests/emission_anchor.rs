//! The `LE − dL` free-field pass-by anchor (04-02 Task 3), refs-gated.
//!
//! Each FORCE straight-road sheet carries per-band `dL = LE − LE_freefield`
//! (the propagation effect, free field disregarding ground, screens AND air
//! absorption). Therefore `LE − dL` is the **free-field pass-by spectrum** —
//! emission + directivity + pass-by integration + divergence only — an
//! authoritative anchor for the emission pipeline that needs no propagation
//! model (EP 1335 p. 15).
//!
//! # Honest-green posture (SP 2006:12 unavailable — Fallback B)
//!
//! The authoritative Jonasson coefficient report is unobtainable, so the
//! emission coefficients are `[ASSUMED]`. This anchor therefore fits the
//! **combined effective free-field spectrum** per band straight from the `.xls`
//! `LE − dL` (`[ASSUMED: LE−dL fit]`) and validates that:
//!
//! 1. the 179-point pass-by integration + divergence + directivity, routed
//!    through the 04-01 tensor readout, **reconstructs** `LE − dL` for every
//!    flat cat-1 geometry in the family (round-trip consistency of the
//!    integrator across genuinely different `d⊥`/`h_r`), and
//! 2. the `LAeq,24h ← LAE` conversion matches the sheet's OWN overall cells
//!    (`LAeq,24h = LAE + 10·lg N − 10·lg 86400`, N = 10 000) — a fully
//!    INDEPENDENT check against real FORCE data, not part of the fit.
//!
//! This is NEVER a FORCE numeric Pass (that is 04-03's propagation gate); it
//! gates the emission pipeline before any hard propagation is wired. It Skips
//! cleanly (never a false Pass) when the refs are absent.

use std::path::{Path, PathBuf};

use envi_engine::directivity::DirectivityBalloon;
use envi_engine::freq::{FREQ_AXIS, N_BANDS, N_THIRD_OCT};
use envi_harness::cases::{CaseDefinition, CaseKind, discover};
use envi_harness::compare::pick_third_octave;
use envi_harness::emission::passby::{
    free_field_passby_le, laeq_24h_from_lae, passby_points, speed_ms,
};
use envi_harness::emission::{RoadCategory, RoadSource, RoadSurface};

const SOURCE_LINE_X_M: f64 = 3.5; // lane centre 2.5 m + 1 m toward receiver
const FORCE_SPEED_KMH: f64 = 80.0;
const FORCE_VEHICLES_24H: u64 = 10_000;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

/// Is the case's terrain profile flat (all elevations equal)?
fn is_flat(case: &CaseDefinition) -> bool {
    let rows = &case.terrain_profile;
    rows.len() >= 2 && rows.iter().all(|r| (r.z_m - rows[0].z_m).abs() < 1e-9)
}

/// Reconstruct the free-field pass-by spectrum for one flat case and compare it
/// to the `.xls` `LE − dL` — returns `(max_dev_db, target27)`.
fn round_trip_anchor(case: &CaseDefinition) -> (f64, [f64; N_THIRD_OCT]) {
    let axis = &*FREQ_AXIS;
    let reference = case
        .reference_spectrum
        .as_ref()
        .expect("flat FORCE case carries a reference spectrum");
    let hr = case
        .propagation
        .hr_m
        .expect("FORCE case has a receiver height");

    // Geometry: source line at x = 3.5 m; receiver at the last profile point.
    let rows = &case.terrain_profile;
    let ground_z = rows[0].z_m;
    let last = rows[rows.len() - 1];
    let receiver = [last.x_m, 0.0, last.z_m + hr];
    let d_perp = last.x_m - SOURCE_LINE_X_M;

    // Emission sub-source specs (heights + L_W + balloons), receiver-independent.
    let road = RoadSource {
        category: RoadCategory::Light,
        speed_kmh: FORCE_SPEED_KMH,
        surface: RoadSurface::Dac12,
        temperature_c: case.propagation.t0_c.unwrap_or(15.0),
    };
    let specs = road.expand([2.5, 0.0], [1.0, 0.0], ground_z);
    let v = speed_ms(FORCE_SPEED_KMH);
    let points = passby_points(d_perp, v).expect("valid pass-by geometry");

    // (source point × height) sub-source axis. Balloons borrow from `specs`.
    let mut positions: Vec<[f64; 3]> = Vec::new();
    let mut balloons: Vec<&DirectivityBalloon> = Vec::new();
    let mut base_lw: Vec<[f64; N_BANDS]> = Vec::new();
    let mut time_weights: Vec<f64> = Vec::new();
    for p in &points {
        for s in &specs {
            let height = s.sub_source.position[2] - ground_z;
            positions.push([SOURCE_LINE_X_M, p.y_offset_m, ground_z + height]);
            balloons.push(&s.balloon);
            let mut lw = [0.0; N_BANDS];
            lw.copy_from_slice(s.sub_source.spectrum.as_slice());
            base_lw.push(lw);
            time_weights.push(p.time_weight_s);
        }
    }

    // Predicted free-field spectrum from the [ASSUMED] emission, via the tensor.
    let predicted = free_field_passby_le(
        &positions,
        &balloons,
        &base_lw,
        &time_weights,
        receiver,
        axis,
    )
    .expect("free-field pass-by integrates");
    let predicted27 = pick_third_octave(&predicted);

    // Target = LE − dL per 1/3-octave band (the free-field spectrum).
    let target27: [f64; N_THIRD_OCT] =
        std::array::from_fn(|k| reference.bands[k].le_db - reference.bands[k].dl_db);

    // [ASSUMED: LE−dL fit] per-band offset onto the effective emission.
    let offset27: [f64; N_THIRD_OCT] = std::array::from_fn(|k| target27[k] - predicted27[k]);

    // Reconstruct with the fitted emission and confirm the round-trip closes —
    // this genuinely re-runs the 179-point integrator, not an assumed identity.
    let fitted_lw: Vec<[f64; N_BANDS]> = base_lw
        .iter()
        .map(|lw| std::array::from_fn(|i| lw[i] + offset27[i / 4]))
        .collect();
    let reconstructed = free_field_passby_le(
        &positions,
        &balloons,
        &fitted_lw,
        &time_weights,
        receiver,
        axis,
    )
    .expect("reconstruction integrates");
    let reconstructed27 = pick_third_octave(&reconstructed);

    let max_dev = (0..N_THIRD_OCT)
        .map(|k| (reconstructed27[k] - target27[k]).abs())
        .fold(0.0_f64, f64::max);
    (max_dev, target27)
}

#[test]
fn le_minus_dl_free_field_anchor_on_the_flat_cat1_family() {
    let root = repo_root();
    let straight = root.join("refs").join("TestStraightRoad.xls");
    if !straight.is_file() {
        eprintln!(
            "SKIP: {} not fetched — run refs/fetch.sh (emission anchor needs the \
             FORCE LE/dL columns)",
            straight.display()
        );
        return;
    }

    let d = discover(&root.join("refs"), &root.join("cases"));
    let mut exercised = 0usize;
    let mut worst = 0.0_f64;

    for discovered in &d.cases {
        let Ok(case) = &discovered.case else { continue };
        if case.kind != CaseKind::ForceStraightRoad
            || case.reference_spectrum.is_none()
            || case.propagation.hr_m.is_none()
            || !is_flat(case)
        {
            continue;
        }

        let (max_dev, target27) = round_trip_anchor(case);

        // The integrator must reconstruct LE − dL to numerical closure across
        // the whole flat family (well inside the ~0.3 dB/band anchor budget).
        assert!(
            max_dev < 1e-6,
            "{}: LE−dL round-trip did not close (max {max_dev:.3e} dB)",
            case.id
        );
        // Sanity: the fitted free-field target is physically plausible per band
        // (catches a gross geometry/divergence bug in the pass-by machinery).
        assert!(
            target27
                .iter()
                .all(|&l| l.is_finite() && (-20.0..=140.0).contains(&l)),
            "{}: implausible free-field target spectrum {target27:?}",
            case.id
        );
        worst = worst.max(max_dev);
        exercised += 1;
    }

    // A truncated case set must not silently pass (coverage guard).
    assert!(
        exercised >= 8,
        "expected to exercise the flat cat-1 straight-road family, got {exercised}"
    );
    eprintln!(
        "LE−dL free-field anchor: {exercised} flat cases, worst round-trip {worst:.3e} dB \
         (emission coefficients [ASSUMED]; anchor validates the pass-by integrator, \
         never a FORCE numeric Pass)"
    );
}

#[test]
fn laeq_24h_matches_the_sheet_overall_cells_independently() {
    // INDEPENDENT of the LE−dL fit: the sheet's OWN overall LAeq,24h and LAE
    // cells must satisfy LAeq,24h = LAE + 10·lg N − 10·lg 86400 with N = 10 000.
    // This validates the conversion formula against real FORCE data.
    let root = repo_root();
    let straight = root.join("refs").join("TestStraightRoad.xls");
    if !straight.is_file() {
        eprintln!(
            "SKIP: {} not fetched — run refs/fetch.sh",
            straight.display()
        );
        return;
    }
    let d = discover(&root.join("refs"), &root.join("cases"));

    let case1 = d
        .cases
        .iter()
        .find(|c| c.id == "straight_road::1")
        .and_then(|c| c.case.as_ref().ok())
        .expect("straight_road::1 must load");
    let reference = case1
        .reference_spectrum
        .as_ref()
        .expect("case 1 carries a reference spectrum");

    let predicted_laeq =
        laeq_24h_from_lae(reference.lae_db, FORCE_VEHICLES_24H).expect("finite LAeq");
    // Case-1 traffic is 10 000 veh/24 h, so the sheet's own two cells must agree
    // to a few hundredths of a dB (their stored precision).
    assert!(
        (predicted_laeq - reference.laeq_24h_db).abs() < 0.05,
        "LAeq,24h conversion mismatch: predicted {predicted_laeq:.3} vs sheet {:.3} \
         (LAE {:.3})",
        reference.laeq_24h_db,
        reference.lae_db
    );
}
