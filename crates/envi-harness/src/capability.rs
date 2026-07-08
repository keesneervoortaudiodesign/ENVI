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
    /// Forest / vegetation scattering (Sub-model 10). The FORCE forest cases
    /// (121–124) need this; deferred to a later plan (Open-Q3) — so those cases
    /// keep an honest `Skipped(requires: forest-scattering)` reason rather than a
    /// false pass.
    ForestScattering,
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
            Self::ForestScattering => "forest-scattering",
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
        CaseKind::Terrain => {
            // Synthetic terrain-effect case: ground effect always; screen
            // diffraction when the profile encodes a screen (name/description).
            required.insert(Capability::GroundEffect);
            if case.description.to_lowercase().contains("screen") {
                required.insert(Capability::Diffraction);
            }
        }
        CaseKind::Refraction => {
            // `Capability::Refraction` is now implemented (see
            // `implemented_capabilities`), so the capability gate no longer fires
            // for these cases. They stay `Skipped` because `run_case` dispatch
            // returns Skipped("no committed numeric reference") — never a false
            // `Pass` (D-03). The required set is declared for completeness/
            // traceability, not to force the skip.
            required.insert(Capability::GroundEffect);
            required.insert(Capability::Refraction);
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
            if description.contains("forest") {
                required.insert(Capability::ForestScattering);
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
/// [`Capability::FreeField`]. Plan 02-05 (Phase 2 complete) adds
/// [`Capability::GroundEffect`] and [`Capability::Diffraction`]. Plan 03-03
/// (Phase 3 complete) adds [`Capability::Refraction`] — the weather routes +
/// CalcEqSSP + circular rays + F_τ coherence are wired, so the FORCE wind/
/// gradient cases' `Skipped(requires: …)` list SHRINKS: `refraction` drops out,
/// leaving **only** `emission-model` (Phase 4). No harness rewrite; no false
/// numeric Pass (D-03).
///
/// Plan 04-03 adds [`Capability::EmissionModel`] — the Nord2000 road source model
/// (Jonasson tables, sub-source split, pass-by integration, directivity) is wired
/// (04-02/04-03), so the FORCE road cases' `Skipped(requires: …)` list SHRINKS
/// again: `emission-model` drops out. `ForestScattering` (Sub-model 10) is NOT
/// implemented, so the forest cases (121–124) keep `forest-scattering` in their
/// missing set (honest gap, Open-Q3). No false numeric Pass: the road cases still
/// fail-soft to `Skipped` inside `run_case` because the Jonasson emission
/// coefficients are `[ASSUMED]` (SP 2006:12 not obtained) — the propagation chain
/// is validated in-crate, but an OVERALL LAeq,24h numeric Pass cannot be honestly
/// claimed without the verified coefficients (D-03).
#[must_use]
pub fn implemented_capabilities() -> BTreeSet<Capability> {
    BTreeSet::from([
        Capability::Geometry,
        Capability::FreeField,
        Capability::GroundEffect,
        Capability::Diffraction,
        Capability::Refraction,
        Capability::EmissionModel,
    ])
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
            source_spectrum: crate::cases::SourceSpectrum::default(),
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
        // FreeField goes green in plan 01-03; EmissionModel is flipped in 04-03.
        assert!(implemented.contains(&Capability::FreeField));
        assert!(implemented.contains(&Capability::EmissionModel));
    }

    #[test]
    fn plan_02_05_implements_ground_effect_and_diffraction() {
        let implemented = implemented_capabilities();
        assert!(implemented.contains(&Capability::GroundEffect));
        assert!(implemented.contains(&Capability::Diffraction));
    }

    #[test]
    fn plan_03_03_implements_refraction() {
        let implemented = implemented_capabilities();
        // Phase 3 closes: refraction is implemented (weather routes + F_tau).
        assert!(implemented.contains(&Capability::Refraction));
    }

    #[test]
    fn plan_04_03_flips_emission_model_and_shrinks_the_force_skip_reason() {
        // The requires-list SHRANK again for the in-scope straight-road FORCE
        // cases: after the 04-03 emission-model flip, a flat homogeneous / wind /
        // gradient road case has an EMPTY missing capability set — every physics
        // capability it needs is implemented. It may still fail-soft to Skipped
        // at RUN time on the [ASSUMED] emission coefficients (D-03), but the
        // CAPABILITY gate no longer fires.
        let implemented = implemented_capabilities();

        let mut downwind = base_case(CaseKind::ForceStraightRoad, "Flat terrain, downwind");
        downwind.propagation.u_ms = Some(5.0);
        let mut inversion = base_case(CaseKind::ForceStraightRoad, "Flat terrain");
        inversion.propagation.dtdz = Some(0.1);
        let flat = base_case(
            CaseKind::ForceStraightRoad,
            "Flat terrain, d=100 m, impedance A, homogeneous atm.",
        );

        for case in [downwind, inversion, flat] {
            let missing: BTreeSet<Capability> = required_capabilities(&case)
                .difference(&implemented)
                .copied()
                .collect();
            assert!(
                !missing.contains(&Capability::EmissionModel),
                "emission-model must be gone from the capability skip reason: {missing:?}"
            );
            assert!(
                missing.is_empty(),
                "in-scope straight-road FORCE case must have no missing capability: {missing:?}"
            );
        }
    }

    #[test]
    fn forest_cases_retain_forest_scattering_in_the_skip_reason() {
        // The forest FORCE cases (121–124) still need Sub-model 10 scattering,
        // which is NOT implemented — so they keep an honest
        // `Skipped(requires: forest-scattering)` reason (never a false pass).
        let implemented = implemented_capabilities();
        let forest = base_case(CaseKind::ForceStraightRoad, "Forest, hr=1.5, u=0");
        let missing: BTreeSet<Capability> = required_capabilities(&forest)
            .difference(&implemented)
            .copied()
            .collect();
        assert_eq!(
            missing,
            BTreeSet::from([Capability::ForestScattering]),
            "forest case must skip ONLY on forest-scattering"
        );
    }

    #[test]
    fn terrain_case_requires_ground_effect_and_screen_adds_diffraction() {
        let flat = base_case(CaseKind::Terrain, "flat ground sigma=200 (Sub-model 1)");
        let req = required_capabilities(&flat);
        assert!(req.contains(&Capability::GroundEffect));
        assert!(!req.contains(&Capability::Diffraction));
        // A terrain case is now fully implemented (no skip).
        assert!(req.difference(&implemented_capabilities()).next().is_none());

        let screen = base_case(CaseKind::Terrain, "thin screen (Sub-model 4)");
        assert!(required_capabilities(&screen).contains(&Capability::Diffraction));
    }
}
