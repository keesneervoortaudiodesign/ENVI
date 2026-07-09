//! Report-only emission validation: engine free-field `LE` from the CITED
//! Table A.1 coefficients vs the FORCE sheet's `LE − dL`, with **no fit**.
//!
//! # What this measures (and why it is report-only)
//!
//! With the road-emission coefficients now CITED (real Table A.1 from the
//! committed source-modelling report), the free-field pass-by spectrum can be
//! computed directly and compared to the FORCE sheet's own free-field spectrum
//! (`LE − dL`, EP 1335 p. 15) — a genuine numeric check of the coefficients
//! against real FORCE data, with no fitted offset.
//!
//! The outcome is a **documented gap, not a Pass**: Table A.1 is the report's
//! *intermediate* DK Nord 2005 set (§2.3.2: "a definite set … expected around
//! December 2006"). It reproduces the FORCE free-field spectrum *shape* but with a
//! systematic **~2.3 dBA over-prediction** on the flat cat-1 family (per-band up
//! to ~3.5 dB) — outside the Ch.6 1 dB tolerance. A numeric FORCE Pass needs the
//! definitive Dec-2006 coefficient set (or full reference-condition calibration).
//! Per the honest-green rule this is never forced to a Pass; this test asserts
//! only finiteness, coverage, and a generous gross-regression bound, and prints
//! the measured deltas. It Skips cleanly when the refs are absent.

use std::path::{Path, PathBuf};

use envi_engine::directivity::DirectivityBalloon;
use envi_engine::freq::{FREQ_AXIS, N_BANDS, N_THIRD_OCT};
use envi_harness::cases::{CaseDefinition, CaseKind, discover};
use envi_harness::compare::{l_ae, pick_third_octave};
use envi_harness::emission::passby::{free_field_passby_le, passby_points, speed_ms};
use envi_harness::emission::{RoadCategory, RoadSource, RoadSurface};

const SOURCE_LINE_X_M: f64 = 3.5; // lane centre 2.5 m + 1 m toward receiver
const FORCE_SPEED_KMH: f64 = 80.0; // nominal flat cat-1 family speed
/// Gross-regression guard only — NOT the Ch.6 tolerance. The measured systematic
/// offset is ~2.3 dBA (intermediate coefficients); anything beyond this bound is
/// a real bug, not the known coefficient-lineage gap.
const GROSS_BOUND_DBA: f64 = 8.0;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

fn is_flat(case: &CaseDefinition) -> bool {
    let rows = &case.terrain_profile;
    rows.len() >= 2 && rows.iter().all(|r| (r.z_m - rows[0].z_m).abs() < 1e-9)
}

/// Engine free-field `LE` (dBA overall) from cited coefficients for one flat case.
fn predicted_freefield_lae(case: &CaseDefinition) -> f64 {
    let axis = &*FREQ_AXIS;
    let hr = case.propagation.hr_m.expect("receiver height");
    let rows = &case.terrain_profile;
    let ground_z = rows[0].z_m;
    let last = rows[rows.len() - 1];
    let receiver = [last.x_m, 0.0, last.z_m + hr];
    let d_perp = last.x_m - SOURCE_LINE_X_M;

    let road = RoadSource {
        category: RoadCategory::Light,
        speed_kmh: FORCE_SPEED_KMH,
        surface: RoadSurface::Dac12,
        temperature_c: case.propagation.t0_c.unwrap_or(15.0),
    };
    let specs = road.expand([2.5, 0.0], [1.0, 0.0], ground_z);
    let v = speed_ms(FORCE_SPEED_KMH);
    let points = passby_points(d_perp, v).expect("valid pass-by geometry");

    let mut positions: Vec<[f64; 3]> = Vec::new();
    let mut balloons: Vec<&DirectivityBalloon> = Vec::new();
    let mut base_lw: Vec<[f64; N_BANDS]> = Vec::new();
    let mut tw: Vec<f64> = Vec::new();
    for p in &points {
        for s in &specs {
            let height = s.sub_source.position[2] - ground_z;
            positions.push([SOURCE_LINE_X_M, p.y_offset_m, ground_z + height]);
            balloons.push(&s.balloon);
            let mut lw = [0.0; N_BANDS];
            lw.copy_from_slice(s.sub_source.spectrum.as_slice());
            base_lw.push(lw);
            tw.push(p.time_weight_s);
        }
    }
    let predicted = free_field_passby_le(&positions, &balloons, &base_lw, &tw, receiver, axis)
        .expect("integrates");
    l_ae(&pick_third_octave(&predicted), axis)
}

#[test]
fn cited_coefficients_reproduce_force_freefield_shape_with_a_documented_offset() {
    let root = repo_root();
    if !root.join("refs").join("TestStraightRoad.xls").is_file() {
        eprintln!("SKIP: refs/TestStraightRoad.xls not fetched — run refs/fetch.sh");
        return;
    }
    let axis = &*FREQ_AXIS;
    let d = discover(&root.join("refs"), &root.join("cases"));

    let mut deltas: Vec<(String, f64)> = Vec::new();
    for discovered in &d.cases {
        let Ok(case) = &discovered.case else { continue };
        if case.kind != CaseKind::ForceStraightRoad
            || case.reference_spectrum.is_none()
            || case.propagation.hr_m.is_none()
            || !is_flat(case)
        {
            continue;
        }
        let reference = case.reference_spectrum.as_ref().unwrap();
        let target27: [f64; N_THIRD_OCT] =
            std::array::from_fn(|k| reference.bands[k].le_db - reference.bands[k].dl_db);
        let le_sheet = l_ae(&target27, axis);
        let le_pred = predicted_freefield_lae(case);
        let delta = le_pred - le_sheet;

        assert!(
            le_pred.is_finite() && le_sheet.is_finite(),
            "{}: non-finite LE (pred {le_pred}, sheet {le_sheet})",
            case.id
        );
        assert!(
            delta.abs() < GROSS_BOUND_DBA,
            "{}: free-field ΔLAE {delta:+.2} dBA exceeds the gross-regression bound \
             {GROSS_BOUND_DBA} — that is a bug, not the known coefficient-lineage gap",
            case.id
        );
        deltas.push((case.id.clone(), delta));
    }

    assert!(
        deltas.len() >= 8,
        "expected the flat cat-1 straight-road family, exercised {}",
        deltas.len()
    );

    let mean = deltas.iter().map(|(_, d)| d).sum::<f64>() / deltas.len() as f64;
    eprintln!(
        "emission_force_delta (report-only): {} flat cat-1 cases, mean free-field ΔLAE \
         {mean:+.2} dBA (CITED intermediate Table A.1 over-predicts FORCE; NOT a numeric Pass — \
         definitive Dec-2006 coefficients needed). Per-case:",
        deltas.len()
    );
    for (id, delta) in &deltas {
        eprintln!("  {id:18} ΔLAE {delta:+.2} dBA");
    }
}
