//! End-to-end terrain-effect harness tests (plan 02-05).
//!
//! Three gates:
//! 1. **Oracle-pinned e2e** — the five committed `cases/terrain_*.toml` cases run
//!    through `run_case` (file → Scene → engine `terrain_effect` → two-channel
//!    readout → comparison) and report `Pass` against the 105-point scipy-oracle
//!    references at 0.1 dB.
//! 2. **Finiteness sweep** (ROADMAP success criterion 3 / T-02-14) — every FORCE
//!    straight-road geometry × 105 bands yields finite `ΔL_t`, `h_coh_factor` and
//!    `p_incoh`, or the typed `NonFlatTerrainNotImplemented` error (never NaN).
//! 3. **Two-channel turbulence floor** — the case-71 screen with turbulence sits
//!    strictly above the same screen with turbulence removed in the deep shadow.

use std::path::{Path, PathBuf};

use envi_engine::freq::FREQ_AXIS;
use envi_engine::propagation::PropagationError;
use envi_engine::propagation::coherence::CoherenceInputs;
use envi_engine::propagation::sound_speed_ms;
use envi_engine::propagation::terrain_effect::terrain_effect;
use envi_harness::cases::{CaseKind, discover};
use envi_harness::{Outcome, build_terrain_inputs, run_case};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

#[test]
fn five_terrain_cases_pass_against_the_oracle() {
    let root = repo_root();
    let d = discover(&root.join("refs"), &root.join("cases"));
    let mut seen = Vec::new();
    for discovered in &d.cases {
        let Ok(case) = &discovered.case else { continue };
        if case.kind != CaseKind::Terrain {
            continue;
        }
        seen.push(case.id.clone());
        match run_case(case) {
            Outcome::Pass => {}
            Outcome::Fail(report) => panic!(
                "{}: FAIL, max |dev| = {:.4} dB\n{}",
                case.id,
                report.max_abs_dev_db,
                report.render_table()
            ),
            other => panic!("{}: unexpected outcome {other:?}", case.id),
        }
    }
    // All five committed terrain cases must be discovered and pass.
    for expected in [
        "toml::terrain_flat_sigma200",
        "toml::terrain_mixed_case21",
        "toml::terrain_screen_thin_case71",
        "toml::terrain_screen_thick_case81",
        "toml::terrain_screens_double_case91",
    ] {
        assert!(
            seen.iter().any(|id| id == expected),
            "terrain case {expected} not discovered (seen {seen:?})"
        );
    }
    assert_eq!(seen.len(), 5, "expected exactly five terrain cases");
}

#[test]
fn finiteness_sweep_across_all_force_geometries_and_bands() {
    // ROADMAP success criterion 3: iterate every FORCE straight-road profile,
    // evaluate terrain_effect at all 105 points, and assert finite OR the typed
    // NonFlatTerrainNotImplemented error (non-flat profiles) — never NaN/Inf.
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
    let axis = &*FREQ_AXIS;

    let mut evaluated = 0usize;
    let mut nonflat = 0usize;
    for discovered in &d.cases {
        let Ok(case) = &discovered.case else { continue };
        if case.kind != CaseKind::ForceStraightRoad || case.terrain_profile.len() < 2 {
            continue;
        }

        // Build the profile + geometry via the harness scene builder, then drive
        // terrain_effect directly (a single placeholder sub-source, the case's
        // own turbulence parameters, homogeneous weather).
        let scene = match envi_harness::scene_build::build_scene(case) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let Some(profile) = scene.terrain.first() else {
            continue;
        };
        let Some(src) = scene
            .sources
            .first()
            .and_then(|s| s.sub_sources.first())
            .map(|ss| ss.position)
        else {
            continue;
        };
        let Some(rcv) = scene.receivers.first().map(|r| r.position) else {
            continue;
        };

        let t_air_c = case.propagation.t0_c.unwrap_or(15.0);
        let c0 = sound_speed_ms(t_air_c);
        let coh = CoherenceInputs {
            cv2: case.propagation.cv2.unwrap_or(0.0),
            ct2: case.propagation.ct2.unwrap_or(0.0),
            t_air_c,
            c0,
            roughness_r: 0.0,
            f_delta_nu: 1.0,
            d_m: (rcv[0] - src[0]).abs().max(1e-6),
        };

        match terrain_effect(profile, src, rcv, c0, &coh, axis) {
            Ok(te) => {
                assert_eq!(te.delta_l_db.len(), 105);
                for i in 0..105 {
                    assert!(
                        te.delta_l_db[i].is_finite()
                            && te.h_coh_factor[i].re.is_finite()
                            && te.h_coh_factor[i].im.is_finite()
                            && te.p_incoh[i].is_finite(),
                        "non-finite terrain effect for {} at {} Hz",
                        case.id,
                        axis.centres[i]
                    );
                }
                evaluated += 1;
            }
            // Non-flat terrain is a typed hard error — never NaN (Sub-model 3,
            // Phase 3). This is the documented, expected outcome for valley /
            // elevated-road / forest profiles.
            Err(PropagationError::NonFlatTerrainNotImplemented { .. }) => {
                nonflat += 1;
            }
            Err(e) => panic!("{}: unexpected terrain_effect error {e}", case.id),
        }
    }
    assert!(
        evaluated + nonflat >= 8,
        "expected to sweep the FORCE straight-road profiles (got {evaluated} flat + {nonflat} non-flat)"
    );
    eprintln!("finiteness sweep: {evaluated} flat-evaluated + {nonflat} non-flat (typed error)");
}

#[test]
fn turbulence_floors_the_screen_in_deep_shadow() {
    // The case-71 thin screen with turbulence yields strictly HIGHER levels in
    // deep shadow (Sub-model 7 scattered energy floors the screen) than with
    // turbulence removed — scattering only ever adds energy, never attenuation.
    let root = repo_root();
    let path = root.join("cases").join("terrain_screen_thin_case71.toml");
    let case = envi_harness::cases::toml::load_toml_case(&path).expect("case-71 must load");
    let (profile, src, rcv, coh) = build_terrain_inputs(&case).expect("inputs");
    let axis = &*FREQ_AXIS;

    let turbulent = terrain_effect(&profile, src, rcv, coh.c0, &coh, axis).unwrap();
    let calm_coh = CoherenceInputs {
        cv2: 0.0,
        ct2: 0.0,
        ..coh
    };
    let calm = terrain_effect(&profile, src, rcv, coh.c0, &calm_coh, axis).unwrap();

    // In the DEEPEST shadow band (where the calm screen attenuates most in the
    // 2–8 kHz range), turbulence scattering floors the screen — the turbulent
    // level sits well above the calm one. (Per-band the combined screen⇄ground
    // model is NOT monotone in turbulence: decoherence removes a constructive
    // ground lobe at some bands while Sub-model 7 floors the deep shadow — the
    // physically meaningful claim is the deep-shadow floor.)
    let shadow: Vec<usize> = axis
        .centres
        .iter()
        .enumerate()
        .filter(|&(_, &f)| (2000.0..=8000.0).contains(&f))
        .map(|(i, _)| i)
        .collect();
    let deepest = *shadow
        .iter()
        .min_by(|&&a, &&b| calm.delta_l_db[a].partial_cmp(&calm.delta_l_db[b]).unwrap())
        .unwrap();
    assert!(
        turbulent.delta_l_db[deepest] > calm.delta_l_db[deepest] + 1.0,
        "turbulence must floor the deepest shadow at {} Hz: turbulent {} vs calm {}",
        axis.centres[deepest],
        turbulent.delta_l_db[deepest],
        calm.delta_l_db[deepest]
    );
}
