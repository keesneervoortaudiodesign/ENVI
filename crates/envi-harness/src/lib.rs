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

/// Result of running one case through the capability gate + engine + comparator.
#[derive(Debug)]
pub enum Outcome {
    /// Computed result within tolerance of the reference.
    Pass,
    /// Computed result outside tolerance — full per-band report attached.
    Fail(compare::ComparisonReport),
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
        cases::CaseKind::Geometry => {
            Outcome::Skipped("geometry dispatch lands in plan 01-02".to_string())
        }
        cases::CaseKind::ForceStraightRoad
        | cases::CaseKind::ForceCurvedRoad
        | cases::CaseKind::ForceCityStreet
        | cases::CaseKind::ForceYearlyAverage => {
            Outcome::Skipped("FORCE dispatch lands in Phases 2-4".to_string())
        }
    }
}
