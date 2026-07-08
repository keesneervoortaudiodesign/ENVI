//! # envi-harness
//!
//! Validation harness for the ENVI Nord2000 engine. **All I/O lives here**:
//! FORCE `.xls` parsing (calamine), synthetic TOML cases (serde), reference
//! comparison with FORCE tolerances, capability gating, and reporting.
//! The engine crate (`envi-engine`) never sees a file format.

pub mod capability;
pub mod cases;
pub mod compare;
pub mod emission;
pub mod scene_build;
pub mod weather;

use envi_engine::freq::FREQ_AXIS;
use envi_engine::geometry::{PathGeometry, azimuth_deg, reflect_over_segment};
use envi_engine::propagation::air_absorption::Atmosphere;
use envi_engine::propagation::coherence::CoherenceInputs;
use envi_engine::propagation::sound_speed_ms;
use envi_engine::propagation::terrain_effect::terrain_effect;
use envi_engine::propagation::{PropagationError, direct_path};
use envi_engine::scene::{GroundSegment, TerrainProfile};
use envi_engine::transfer::band_levels_db;

/// Result of running one case through the capability gate + engine + comparator.
#[derive(Debug)]
pub enum Outcome {
    /// Computed result within tolerance of the reference.
    Pass,
    /// Computed result outside tolerance — full per-band report attached
    /// (spectrum comparisons).
    Fail(compare::ComparisonReport),
    /// A non-spectrum failure (e.g. a geometry anchor mismatch or a scene-build
    /// error) with a message naming the offending quantity. Geometry checks
    /// have no 27-band report, so they surface here instead of [`Outcome::Fail`].
    FailDetail(String),
    /// Case cannot run yet; the string names why (e.g.
    /// `"requires: emission-model, ground-effect"`).
    Skipped(String),
}

/// Run one case: capability gate first, then dispatch by kind.
///
/// In plan 01-01 [`capability::implemented_capabilities`] is empty, so every
/// case gates to `Skipped(requires: …)` — the harness fails meaningfully
/// before propagation code exists. Plan 01-03 adds the free-field dispatch.
#[must_use]
pub fn run_case(case: &cases::CaseDefinition) -> Outcome {
    let required = capability::required_capabilities(case);
    let implemented = capability::implemented_capabilities();
    let missing: Vec<&'static str> = required
        .difference(&implemented)
        .map(|c| c.as_str())
        .collect();
    if !missing.is_empty() {
        return Outcome::Skipped(format!("requires: {}", missing.join(", ")));
    }

    // Dispatch by kind. Every arm is Skipped in this plan (implemented set is
    // empty, so the gate above always fires first); plans 01-02/01-03 and
    // Phases 2-4 replace these arms with real engine calls.
    match case.kind {
        cases::CaseKind::FreeField => run_freefield_case(case),
        cases::CaseKind::Geometry => run_geometry_case(case),
        cases::CaseKind::Terrain => run_terrain_case(case),
        // Phase 3 implements the refraction chain (CalcEqSSP + circular rays +
        // F_τ) and the weather routes; the engine + property/oracle tests cover
        // it in-crate. The synthetic refraction TOML cases carry no committed
        // numeric reference (`bands = "none"`) — the [ASSUMED] weather-route
        // constants are deliberately never pinned to a false numeric Pass
        // (D-03/D-04), so a reference-free refraction case fail-softs to Skipped.
        cases::CaseKind::Refraction => Outcome::Skipped(
            "no committed numeric reference (refraction validated in-crate by \
             property/oracle tests; [ASSUMED] weather constants not numerically pinned)"
                .to_string(),
        ),
        cases::CaseKind::ForceStraightRoad => run_force_straight_road_case(case),
        cases::CaseKind::ForceCurvedRoad
        | cases::CaseKind::ForceCityStreet
        | cases::CaseKind::ForceYearlyAverage => {
            Outcome::Skipped("curved / city / yearly FORCE layouts land in plan 04-04".to_string())
        }
    }
}

/// Run a FORCE straight-road case: the capability gate has already passed
/// (emission-model + ground-effect + refraction all implemented), so this is the
/// runtime dispatch. It mirrors [`run_terrain_case`] — a typed engine error maps
/// to `Skipped` mid-run (honest-green), a scene/build failure to `FailDetail`.
///
/// # Honest-green (D-03): the Jonasson emission coefficients are `[ASSUMED]`
///
/// The Nord2000 road source model is fully wired (04-02/04-03: sub-source split,
/// pass-by integration, directivity, the full propagation chain SM1/2/3 +
/// refraction), and the free-field `LE − dL` shape is anchored. But the absolute
/// rolling/propulsion sound-power coefficients (Jonasson SP 2006:12) could not be
/// obtained — they are `[ASSUMED]` fits (04-02). An OVERALL LAeq,24h / LAE /
/// LAmax numeric Pass therefore depends on unobtainable coefficients, so this
/// case stays `Skipped` with an honest, SHRUNKEN reason rather than a false Pass.
/// The propagation physics it exercises (terrain effect, refraction, the Ch.6
/// comparator + A-weighting conversions) is validated in-crate by the oracle /
/// property / anchor tests.
fn run_force_straight_road_case(case: &cases::CaseDefinition) -> Outcome {
    // The reference must be present (it always is for a loaded FORCE sheet).
    let Some(reference) = case.reference_spectrum.as_ref() else {
        return Outcome::FailDetail(
            "FORCE straight-road case is missing its 27-band reference spectrum".to_string(),
        );
    };
    if reference.bands.len() != envi_engine::freq::N_THIRD_OCT {
        return Outcome::FailDetail(format!(
            "FORCE reference has {} bands (expected {})",
            reference.bands.len(),
            envi_engine::freq::N_THIRD_OCT
        ));
    }

    // The road emission model is implemented but its coefficients are [ASSUMED]
    // (SP 2006:12 not obtained). A verified overall-level numeric Pass is not
    // honestly achievable — stay Skipped with the shrunken reason (never a false
    // Pass, D-03). This is the `provenance == Assumed` gate.
    if matches!(
        emission::coefficients::PROVENANCE,
        emission::Provenance::Assumed
    ) {
        return Outcome::Skipped(
            "requires: verified emission coefficients (Jonasson SP 2006:12); the road \
             emission model is wired but its rolling/propulsion coefficients are [ASSUMED], \
             so an overall LAeq,24h numeric Pass would be false. Propagation (SM1/2/3 + \
             refraction) + the Ch.6 comparator are validated in-crate."
                .to_string(),
        );
    }

    // Reached only once verified coefficients are in hand: build the road pass-by,
    // integrate LE / LAeq,24h / LAmax, and compare via the Ch.6 dip-shift rule.
    // (Left as the numeric-green entry point for the coefficient-verification plan.)
    Outcome::Skipped(
        "FORCE straight-road numeric comparison pending verified emission coefficients".to_string(),
    )
}

/// Run a synthetic geometry case end-to-end: file → Scene → engine geometry →
/// comparison against the hand-computed anchors within the case tolerance.
///
/// This is the vertical-slice payoff of plan 01-02 — the first capability to go
/// green. Each expected quantity present in `[expected.geometry]` is checked;
/// any mismatch returns [`Outcome::FailDetail`] naming the offending quantity.
fn run_geometry_case(case: &cases::CaseDefinition) -> Outcome {
    let scene = match scene_build::build_scene(case) {
        Ok(scene) => scene,
        Err(e) => return Outcome::FailDetail(format!("scene build failed: {e}")),
    };

    let Some(expected) = case.expected.as_ref() else {
        return Outcome::FailDetail("geometry case is missing its [expected] block".to_string());
    };
    let Some(geo) = expected.geometry.as_ref() else {
        return Outcome::FailDetail(
            "geometry case is missing its [expected.geometry] block".to_string(),
        );
    };
    let tol = geo.tolerance.unwrap_or(expected.tolerance_db);

    let Some(source) = scene
        .sources
        .first()
        .and_then(|s| s.sub_sources.first())
        .map(|ss| ss.position)
    else {
        return Outcome::FailDetail("scene has no source sub-source".to_string());
    };
    let Some(receiver) = scene.receivers.first().map(|r| r.position) else {
        return Outcome::FailDetail("scene has no receiver".to_string());
    };

    // Azimuth anchor (horizontal x/y plane).
    if let Some(want) = geo.azimuth_deg {
        let got = azimuth_deg([source[0], source[1]], [receiver[0], receiver[1]]);
        if (got - want).abs() > tol {
            return Outcome::FailDetail(format!("azimuth_deg: got {got}, want {want} (tol {tol})"));
        }
        // Exercise the direct-path primitive too (guards degenerate paths).
        if let Err(e) = PathGeometry::direct(source, receiver) {
            return Outcome::FailDetail(format!("direct path: {e}"));
        }
    }

    // Reflection anchors, authored in the x–z cut plane (y = 0).
    if geo.reflection_x.is_some() || geo.path_length_m.is_some() {
        let Some(seg) = geo.reflection_segment else {
            return Outcome::FailDetail(
                "reflection anchor set but no reflection_segment".to_string(),
            );
        };
        let s2 = [source[0], source[2]];
        let r2 = [receiver[0], receiver[2]];
        let Some(refl) = reflect_over_segment(s2, r2, seg[0], seg[1]) else {
            return Outcome::FailDetail("reflection is geometrically undefined".to_string());
        };
        if !refl.valid {
            return Outcome::FailDetail(format!(
                "reflection point x = {} lies outside the segment",
                refl.point_x
            ));
        }
        if let Some(want) = geo.reflection_x
            && (refl.point_x - want).abs() > tol
        {
            return Outcome::FailDetail(format!(
                "reflection_x: got {}, want {want} (tol {tol})",
                refl.point_x
            ));
        }
        if let Some(want) = geo.path_length_m {
            let got = refl.r1_m + refl.r2_m;
            if (got - want).abs() > tol {
                return Outcome::FailDetail(format!(
                    "path_length_m: got {got}, want {want} (tol {tol})"
                ));
            }
        }
    }

    Outcome::Pass
}

/// Run a synthetic free-field case end-to-end — the walking skeleton's payoff
/// (plan 01-03 Task 3): file → Scene → engine complex transfer → dB-domain
/// comparison → outcome.
///
/// Pipeline: [`scene_build::build_scene`] → the single sub-source + receiver →
/// [`PathGeometry::direct`] → [`direct_path`] (105 complex values) →
/// [`band_levels_db`] against the source spectrum → compared against
/// [`compare::analytic_freefield_reference`] (an independent dB-domain oracle)
/// at the case tolerance (1e-9 dB analytic identity, per 01-RESEARCH Open
/// Question 2 — deliberately stricter than the FORCE 1 dB). Comparison is in the
/// 105-point 1/12-octave space (the 27-band pick is for FORCE references).
fn run_freefield_case(case: &cases::CaseDefinition) -> Outcome {
    let scene = match scene_build::build_scene(case) {
        Ok(scene) => scene,
        Err(e) => return Outcome::FailDetail(format!("scene build failed: {e}")),
    };

    let Some(expected) = case.expected.as_ref() else {
        return Outcome::FailDetail("free-field case is missing its [expected] block".to_string());
    };
    let tol = expected.tolerance_db;

    // Atmosphere from the case propagation block (typed domain validation).
    let Some(t_air_c) = case.propagation.t0_c else {
        return Outcome::FailDetail(
            "free-field case is missing the air temperature t0".to_string(),
        );
    };
    let atmos = match Atmosphere::new(
        t_air_c,
        case.propagation.rh_percent,
        case.propagation.pressure_kpa,
    ) {
        Ok(a) => a,
        Err(e) => return Outcome::FailDetail(format!("invalid atmosphere: {e}")),
    };

    let Some(sub_source) = scene.sources.first().and_then(|s| s.sub_sources.first()) else {
        return Outcome::FailDetail("scene has no source sub-source".to_string());
    };
    let Some(receiver) = scene.receivers.first() else {
        return Outcome::FailDetail("scene has no receiver".to_string());
    };

    let path = match PathGeometry::direct(sub_source.position, receiver.position) {
        Ok(p) => p,
        Err(e) => return Outcome::FailDetail(format!("degenerate path: {e}")),
    };

    let axis = &*FREQ_AXIS;
    let transfer = match direct_path(&path, &atmos, axis) {
        Ok(h) => h,
        Err(e) => return Outcome::FailDetail(format!("direct path failed: {e}")),
    };

    // Engine complex path → receiver band levels; independent dB-domain oracle.
    let got = band_levels_db(&transfer, &sub_source.spectrum);
    let want = compare::analytic_freefield_reference(path.r_m, &atmos, &sub_source.spectrum, axis);

    let report = compare::compare_pointwise(&got, &want, tol, &axis.centres);
    if report.pass {
        Outcome::Pass
    } else {
        Outcome::Fail(report)
    }
}

/// Build an engine [`TerrainProfile`] and matching [`CoherenceInputs`] from a
/// terrain case's rows and atmosphere. Shared by [`run_terrain_case`] and the
/// finiteness-sweep test so both drive the exact same construction.
///
/// # Errors
///
/// Propagates [`PropagationError`]-style failures as a message string on a
/// malformed profile.
pub fn build_terrain_inputs(
    case: &cases::CaseDefinition,
) -> Result<(TerrainProfile, [f64; 3], [f64; 3], CoherenceInputs), String> {
    let rows = &case.terrain_profile;
    if rows.len() < 2 {
        return Err("terrain case needs at least two profile rows".to_string());
    }
    let points: Vec<[f64; 2]> = rows.iter().map(|r| [r.x_m, r.z_m]).collect();
    let segments: Vec<GroundSegment> = rows
        .windows(2)
        .map(|w| GroundSegment {
            flow_resistivity: w[0].flow_resistivity_kns_m4,
            roughness: w[0].roughness_m,
        })
        .collect();
    let profile = TerrainProfile::new(points, segments).map_err(|e| e.to_string())?;

    let src = case
        .source_position
        .ok_or_else(|| "terrain case missing source position".to_string())?;
    let rcv = case
        .receiver_position
        .ok_or_else(|| "terrain case missing receiver position".to_string())?;

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
    Ok((profile, src, rcv, coh))
}

/// Run a synthetic terrain-effect case: rows → `TerrainProfile` → engine
/// [`terrain_effect`] two-channel `ΔL_t` → comparison against the oracle-pinned
/// 105-point reference at the case tolerance (0.1 dB, the cross-implementation
/// gate of the 02-RESEARCH acceptance ladder).
fn run_terrain_case(case: &cases::CaseDefinition) -> Outcome {
    let Some(expected) = case.expected.as_ref() else {
        return Outcome::FailDetail("terrain case is missing its [expected] block".to_string());
    };
    let Some(reference) = expected.reference_bands.as_ref() else {
        return Outcome::FailDetail(
            "terrain case is missing its [expected] 105-point reference values".to_string(),
        );
    };

    let (profile, src, rcv, coh) = match build_terrain_inputs(case) {
        Ok(v) => v,
        Err(e) => return Outcome::FailDetail(format!("terrain inputs: {e}")),
    };

    let axis = &*FREQ_AXIS;
    let te = match terrain_effect(&profile, src, rcv, coh.c0, &coh, axis, None) {
        Ok(te) => te,
        Err(PropagationError::NonFlatTerrainNotImplemented { .. }) => {
            return Outcome::Skipped(
                "requires: non-flat-terrain (Sub-model 3, Phase 3)".to_string(),
            );
        }
        Err(e) => return Outcome::FailDetail(format!("terrain_effect failed: {e}")),
    };

    let report = compare::compare_pointwise(
        &te.delta_l_db,
        reference,
        expected.tolerance_db,
        &axis.centres,
    );
    if report.pass {
        Outcome::Pass
    } else {
        Outcome::Fail(report)
    }
}

#[cfg(test)]
mod tests {
    use envi_engine::freq::{FREQ_AXIS, N_BANDS};
    use envi_engine::geometry::PathGeometry;
    use envi_engine::propagation::air_absorption::Atmosphere;
    use envi_engine::propagation::direct_path;

    #[test]
    fn transfer_spectrum_is_105_complex_values_with_a_live_imaginary_part() {
        // The engine output wired by the free-field arm is genuinely complex:
        // 105 Complex<f64> values, at least one with a non-zero imaginary part
        // (the phase convention is exercised, not a real scalar shortcut).
        let atmos = Atmosphere::new(15.0, 70.0, 101.325).unwrap();
        let path = PathGeometry::direct([0.0, 0.0, 0.5], [100.0, 0.0, 1.5]).unwrap();
        let h = direct_path(&path, &atmos, &FREQ_AXIS).unwrap();
        assert_eq!(h.len(), N_BANDS);
        assert!(
            h.iter().any(|z| z.im.abs() > 1e-12),
            "the transfer spectrum must carry a live (non-zero) imaginary part at R = 100 m"
        );
    }
}
