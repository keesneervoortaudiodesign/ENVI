//! Case loading: FORCE `.xls` workbooks and synthetic TOML cases, all mapped
//! into one shared internal representation, [`CaseDefinition`].
//!
//! Two input sources, one representation (01-RESEARCH "Recommended harness
//! input schema"):
//! - [`xls`]: the FORCE/DELTA road-traffic workbooks (calamine, label-anchored)
//! - [`toml`]: synthetic cases we author ourselves (`cases/*.toml`)

use std::path::Path;

pub mod toml;
pub mod xls;

/// What kind of scenario a case describes — drives capability requirements.
///
/// Extensible: curved-road / city-street / yearly-average kinds arrive with
/// Phases 3–4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseKind {
    /// Synthetic free-field case with an analytic reference.
    FreeField,
    /// Synthetic hand-computed geometry check.
    Geometry,
    /// FORCE straight-road case (TestStraightRoad.xls worksheet).
    ForceStraightRoad,
}

/// Provenance of the reference values a case is compared against.
///
/// Pitfall 6 (01-RESEARCH): the 2010 revision changed some FORCE results;
/// provenance must travel with every case and every report line so that
/// swapping in the `*_20100610.xls` set later is a pure data change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceVersion {
    /// Analytic identity (synthetic cases).
    Analytic,
    /// Env. Project 1276 (2009) corrected test cases.
    Force2009,
    /// Env. Project 1335 (2010) revised test cases (`*_20100610.xls`).
    Force2010,
}

impl ReferenceVersion {
    /// Stable label for report lines.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Analytic => "analytic",
            Self::Force2009 => "force-2009",
            Self::Force2010 => "force-2010",
        }
    }
}

/// Propagation parameters from the FORCE per-worksheet propagation block
/// (or the TOML `[atmosphere]` table). All worksheet values are optional —
/// absence is data, not an error.
#[derive(Debug, Clone)]
pub struct PropagationParams {
    /// Receiver height above local terrain, m (`hr`).
    pub hr_m: Option<f64>,
    /// Air temperature at ground, °C (`t0`).
    pub t0_c: Option<f64>,
    /// Roughness length, m (`z0`).
    pub z0_m: Option<f64>,
    /// Anemometer height, m (`zu`).
    pub zu_m: Option<f64>,
    /// Wind speed at `zu`, m/s (`u`).
    pub u_ms: Option<f64>,
    /// Wind direction re north, degrees (`φ`).
    pub phi_deg: Option<f64>,
    /// Standard deviation of wind speed, m/s (`su`).
    pub su_ms: Option<f64>,
    /// Temperature gradient, °C/m (`dt/dz`).
    pub dtdz: Option<f64>,
    /// Standard deviation of the temperature gradient, °C/m (`sdt/dz`).
    pub sdtdz: Option<f64>,
    /// Wind turbulence strength, m^(4/3)/s² (`Cv2`).
    pub cv2: Option<f64>,
    /// Temperature turbulence strength, K/s² (`Ct2`).
    pub ct2: Option<f64>,
    /// Relative humidity, %. Not on the FORCE sheets — 70 % globally per the
    /// Env. Project 1335 report text.
    pub rh_percent: f64,
    /// Ambient pressure, kPa. Not on the FORCE sheets — 101.325 kPa assumed.
    pub pressure_kpa: f64,
}

impl Default for PropagationParams {
    fn default() -> Self {
        Self {
            hr_m: None,
            t0_c: None,
            z0_m: None,
            zu_m: None,
            u_ms: None,
            phi_deg: None,
            su_ms: None,
            dtdz: None,
            sdtdz: None,
            cv2: None,
            ct2: None,
            rh_percent: 70.0,
            pressure_kpa: 101.325,
        }
    }
}

/// One raw terrain-profile row from a FORCE worksheet (columns A–D).
///
/// `x` is the distance from the road centre line in the vertical cut plane
/// (NOT distance from the source — the source line sits at x = 2.5 m,
/// Pitfall 5). Semantic interpretation into a `Scene` is plan 01-02.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainRow {
    /// Distance from road centre line, m.
    pub x_m: f64,
    /// Terrain elevation, m.
    pub z_m: f64,
    /// Ground flow resistivity, kNs·m⁻⁴ (Nordtest σ).
    pub flow_resistivity_kns_m4: f64,
    /// Terrain roughness, m (class N = 0).
    pub roughness_m: f64,
}

/// One row of the 27-band FORCE reference spectrum table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpectrumRow {
    /// Nominal 1/3-octave label from the sheet's `Freq.` column (25 … 10000).
    /// Display/index-mapping only — never a computation frequency.
    pub nominal_hz: f64,
    /// A-weighted Leq,24h reference, dB.
    pub leq_24h_db: f64,
    /// Sound exposure level LE, dB.
    pub le_db: f64,
    /// Propagation effect dL = LE − free-field LE, dB (informational).
    pub dl_db: f64,
}

/// A full 27-band reference spectrum plus overall levels, at the .xls cells'
/// full float precision (Pitfall 1: never the rounded report-appendix values).
#[derive(Debug, Clone)]
pub struct ReferenceSpectrum {
    /// Exactly 27 rows, band index 0 (25 Hz) … 26 (10 kHz).
    pub bands: Vec<SpectrumRow>,
    /// Overall A-weighted LAeq,24h, dB.
    pub laeq_24h_db: f64,
    /// Overall A-weighted LAE, dB.
    pub lae_db: f64,
    /// Overall A-weighted LAmax, dB.
    pub lamax_db: f64,
}

/// Expected-result block for synthetic (TOML) cases.
#[derive(Debug, Clone)]
pub struct SyntheticExpected {
    /// Per-band comparison tolerance, dB (e.g. 1e-9 for analytic identities).
    pub tolerance_db: f64,
    /// What the expected values are, e.g. `"analytic:divergence+iso9613"`.
    pub bands: String,
}

/// The shared internal representation every loader emits and every consumer
/// (capability gate, engine dispatch, comparator, report) reads.
#[derive(Debug, Clone)]
pub struct CaseDefinition {
    /// Stable id used as the test/trial name, e.g. `"straight_road::1"` or
    /// `"toml::freefield_100m"`.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Scenario kind.
    pub kind: CaseKind,
    /// Reference-value provenance.
    pub reference_version: ReferenceVersion,
    /// Description string (FORCE row 2, e.g. "Flat terrain, d=100 m, …").
    pub description: String,
    /// Source position [x, y, z] in the local metric CRS, Z-up (synthetic
    /// cases; FORCE cases derive source geometry in plan 01-02).
    pub source_position: Option<[f64; 3]>,
    /// Receiver position [x, y, z] in the local metric CRS, Z-up.
    pub receiver_position: Option<[f64; 3]>,
    /// Propagation / atmosphere parameters.
    pub propagation: PropagationParams,
    /// Raw terrain-profile rows (FORCE cases; empty for free-field cases).
    pub terrain_profile: Vec<TerrainRow>,
    /// FORCE 27-band reference spectrum (None for synthetic cases).
    pub reference_spectrum: Option<ReferenceSpectrum>,
    /// Synthetic expected block (None for FORCE cases).
    pub expected: Option<SyntheticExpected>,
}

/// Discover every runnable case under `cases_dir` (synthetic TOML) and
/// `refs_dir` (FORCE workbooks, if fetched).
///
/// STUB (Task 1): returns an empty set so the end-to-end harness test fails
/// with "no cases discovered". Task 2 implements it.
#[must_use]
pub fn discover(_refs_dir: &Path, _cases_dir: &Path) -> Vec<CaseDefinition> {
    Vec::new()
}
