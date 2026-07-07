//! # envi-harness
//!
//! Validation harness for the ENVI Nord2000 engine. **All I/O lives here**:
//! FORCE `.xls` parsing (calamine), synthetic TOML cases (serde), reference
//! comparison with FORCE tolerances, capability gating, and reporting.
//! The engine crate (`envi-engine`) never sees a file format.

pub mod capability;
pub mod cases;
pub mod compare;
pub mod scene_build;

use envi_engine::geometry::{PathGeometry, azimuth_deg, reflect_over_segment};

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
        cases::CaseKind::FreeField => {
            Outcome::Skipped("free-field dispatch lands in plan 01-03".to_string())
        }
        cases::CaseKind::Geometry => run_geometry_case(case),
        cases::CaseKind::ForceStraightRoad
        | cases::CaseKind::ForceCurvedRoad
        | cases::CaseKind::ForceCityStreet
        | cases::CaseKind::ForceYearlyAverage => {
            Outcome::Skipped("FORCE dispatch lands in Phases 2-4".to_string())
        }
    }
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
