//! Capability gating: what physics a case needs vs what the engine has.
//!
//! Every case declares (via its kind, description and propagation params)
//! the set of engine capabilities it requires. Cases whose requirements are
//! not yet implemented report `Skipped(requires: …)` instead of failing —
//! this is what makes "the harness runs and fails meaningfully before
//! propagation code exists" concrete (Pitfall 2: FORCE road cases are NOT a
//! Phase 1 numeric gate). Later plans/phases extend
//! [`implemented_capabilities`] and cases flip green with no harness rewrite.

use std::collections::BTreeSet;

use crate::cases::{CaseDefinition, CaseKind};

/// One unit of engine physics a case may require.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Capability {
    /// Free-field direct path: divergence + air absorption (plan 01-03).
    FreeField,
    /// Scene/path geometry primitives (plan 01-02).
    Geometry,
    /// Nord2000 road source model: Jonasson emission tables, sub-sources,
    /// pass-by integration (Phase 4).
    EmissionModel,
    /// Ground reflection / impedance effect (Phase 2).
    GroundEffect,
    /// Screen / barrier diffraction (Phase 2).
    Diffraction,
    /// Meteorological refraction: wind and temperature gradients (Phase 3).
    Refraction,
}

impl Capability {
    /// Stable kebab-case label used in `Skipped(requires: …)` reasons.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FreeField => "free-field",
            Self::Geometry => "geometry",
            Self::EmissionModel => "emission-model",
            Self::GroundEffect => "ground-effect",
            Self::Diffraction => "diffraction",
            Self::Refraction => "refraction",
        }
    }
}

/// The capability set a case needs before it can produce a comparable result.
///
/// FORCE straight-road cases require `EmissionModel + GroundEffect` at
/// minimum; screen groups additionally require `Diffraction` (derived from
/// the case description); wind / temperature-gradient variants additionally
/// require `Refraction` (nonzero `u` or `dt/dz`, or a wind mention in the
/// description).
#[must_use]
pub fn required_capabilities(case: &CaseDefinition) -> BTreeSet<Capability> {
    let mut required = BTreeSet::new();
    match case.kind {
        CaseKind::FreeField => {
            required.insert(Capability::FreeField);
        }
        CaseKind::Geometry => {
            required.insert(Capability::Geometry);
        }
        CaseKind::ForceStraightRoad
        | CaseKind::ForceCurvedRoad
        | CaseKind::ForceCityStreet
        | CaseKind::ForceYearlyAverage => {
            required.insert(Capability::EmissionModel);
            required.insert(Capability::GroundEffect);

            let description = case.description.to_lowercase();
            if description.contains("screen") {
                required.insert(Capability::Diffraction);
            }

            let nonzero = |v: Option<f64>| v.is_some_and(|x| x != 0.0);
            if nonzero(case.propagation.u_ms)
                || nonzero(case.propagation.dtdz)
                || description.contains("downwind")
                || description.contains("upwind")
            {
                required.insert(Capability::Refraction);
            }
        }
    }
    required
}

/// The capability set the engine currently implements.
///
/// Plan 01-02 turns on [`Capability::Geometry`] (azimuth + image-source
/// reflection through the scene model). Plan 01-03 adds
/// [`Capability::FreeField`]; Phases 2–4 extend further — cases flip green with
/// no harness rewrite.
#[must_use]
pub fn implemented_capabilities() -> BTreeSet<Capability> {
    BTreeSet::from([Capability::Geometry])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{PropagationParams, ReferenceVersion};

    fn base_case(kind: CaseKind, description: &str) -> CaseDefinition {
        CaseDefinition {
            id: "test::case".to_string(),
            name: "test".to_string(),
            kind,
            reference_version: ReferenceVersion::Force2009,
            description: description.to_string(),
            source_position: None,
            receiver_position: None,
            propagation: PropagationParams::default(),
            terrain_profile: Vec::new(),
            reference_spectrum: None,
            expected: None,
        }
    }

    #[test]
    fn free_field_case_requires_exactly_free_field() {
        let mut case = base_case(CaseKind::FreeField, "free-field 100 m");
        case.reference_version = ReferenceVersion::Analytic;
        let req = required_capabilities(&case);
        assert_eq!(req, BTreeSet::from([Capability::FreeField]));
    }

    #[test]
    fn straight_road_requires_at_least_emission_and_ground() {
        let case = base_case(
            CaseKind::ForceStraightRoad,
            "Flat terrain, d=100 m, hr=1.5 m, impedance A, homogeneous atm.",
        );
        let req = required_capabilities(&case);
        assert!(req.contains(&Capability::EmissionModel));
        assert!(req.contains(&Capability::GroundEffect));
        assert!(!req.contains(&Capability::Diffraction));
        assert!(!req.contains(&Capability::Refraction));
    }

    #[test]
    fn screen_cases_also_require_diffraction() {
        let case = base_case(
            CaseKind::ForceStraightRoad,
            "Flat terrain with thin screen, d=100 m, hr=1.5 m",
        );
        assert!(required_capabilities(&case).contains(&Capability::Diffraction));
    }

    #[test]
    fn wind_or_temperature_gradient_requires_refraction() {
        let mut windy = base_case(CaseKind::ForceStraightRoad, "Flat terrain, downwind");
        windy.propagation.u_ms = Some(5.0);
        assert!(required_capabilities(&windy).contains(&Capability::Refraction));

        let mut gradient = base_case(CaseKind::ForceStraightRoad, "Flat terrain");
        gradient.propagation.dtdz = Some(0.1);
        assert!(required_capabilities(&gradient).contains(&Capability::Refraction));

        let calm = base_case(
            CaseKind::ForceStraightRoad,
            "Flat terrain, homogeneous atm.",
        );
        assert!(!required_capabilities(&calm).contains(&Capability::Refraction));
    }

    #[test]
    fn geometry_is_implemented_in_plan_01_02() {
        let implemented = implemented_capabilities();
        assert!(
            implemented.contains(&Capability::Geometry),
            "plan 01-02 turns on the Geometry capability"
        );
        // FreeField is still plan 01-03; the FORCE physics is Phases 2-4.
        assert!(!implemented.contains(&Capability::FreeField));
        assert!(!implemented.contains(&Capability::EmissionModel));
    }
}
