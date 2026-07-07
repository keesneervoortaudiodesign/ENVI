//! Case loading: FORCE `.xls` workbooks and synthetic TOML cases, all mapped
//! into one shared internal representation, [`CaseDefinition`].
//!
//! Two input sources, one representation (01-RESEARCH "Recommended harness
//! input schema"):
//! - [`xls`]: the FORCE/DELTA road-traffic workbooks (calamine, label-anchored)
//! - [`toml`]: synthetic cases we author ourselves (`cases/*.toml`)

use std::path::{Path, PathBuf};

use thiserror::Error;

pub mod toml;
pub mod xls;

/// What kind of scenario a case describes — drives capability requirements.
///
/// Extensible: curved-road / city-street / yearly-average layouts are loaded
/// for real in Phases 3–4; until then they surface as placeholder cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseKind {
    /// Synthetic free-field case with an analytic reference.
    FreeField,
    /// Synthetic hand-computed geometry check.
    Geometry,
    /// FORCE straight-road case (TestStraightRoad.xls worksheet).
    ForceStraightRoad,
    /// FORCE curved-road workbook placeholder (layout parsed in Phase 4).
    ForceCurvedRoad,
    /// FORCE city-street workbook placeholder (layout parsed in Phase 4).
    ForceCityStreet,
    /// FORCE yearly-average workbook placeholder (layout parsed in Phase 3/4).
    ForceYearlyAverage,
}

/// Typed load error for untrusted case input (ASVS V5 posture, T-01-01):
/// every malformed input yields one of these — never a panic.
#[derive(Debug, Error)]
pub enum CaseLoadError {
    /// Filesystem error reading a case file.
    #[error("I/O error reading {path}: {source}")]
    Io {
        /// Offending path.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
    /// TOML syntax / schema error.
    #[error("TOML parse error in {path}: {message}")]
    TomlParse {
        /// Offending file.
        path: PathBuf,
        /// Parser message.
        message: String,
    },
    /// Unrecognised `meta.kind` string.
    #[error("unknown case kind {got:?} in {path} (accepted kinds: \"free-field\", \"geometry\")")]
    UnknownKind {
        /// The rejected kind string.
        got: String,
        /// Offending file.
        path: PathBuf,
    },
    /// Unrecognised `meta.reference` string.
    #[error(
        "unknown reference version {got:?} in {path} \
         (accepted: \"analytic\", \"force-2009\", \"force-2010\")"
    )]
    UnknownReference {
        /// The rejected reference string.
        got: String,
        /// Offending file.
        path: PathBuf,
    },
    /// Workbook-level calamine failure (open / missing sheet / range).
    #[error("workbook error in {path}: {message}")]
    Workbook {
        /// Offending workbook.
        path: PathBuf,
        /// Underlying calamine message.
        message: String,
    },
    /// A label cell the parser anchors on was not found.
    #[error("sheet {sheet:?}: required label {label:?} not found")]
    MissingLabel {
        /// Worksheet name.
        sheet: String,
        /// The missing anchor label.
        label: String,
    },
    /// A required numeric value was absent, NaN or infinite.
    #[error("{context}: value for {what} is missing or not finite")]
    NonFinite {
        /// Sheet or file the value came from.
        context: String,
        /// What the value was supposed to be.
        what: String,
    },
    /// Terrain-profile X positions must be strictly ascending.
    #[error(
        "{context}: terrain profile X not strictly ascending at row {row} \
         (x = {x} after {prev_x})"
    )]
    NonAscendingProfile {
        /// Sheet or file.
        context: String,
        /// Offending row index (0-based within the profile).
        row: usize,
        /// Previous X value.
        prev_x: f64,
        /// Offending X value.
        x: f64,
    },
    /// Terrain-profile row cap exceeded (DoS guard).
    #[error("{context}: terrain profile has {count} rows (cap {cap})")]
    TooManyProfileRows {
        /// Sheet or file.
        context: String,
        /// Observed row count.
        count: usize,
        /// The enforced cap.
        cap: usize,
    },
    /// Workbook sheet cap exceeded (DoS guard).
    #[error("workbook {path} has {count} sheets (cap {cap})")]
    TooManySheets {
        /// Offending workbook.
        path: PathBuf,
        /// Observed sheet count.
        count: usize,
        /// The enforced cap.
        cap: usize,
    },
    /// The reference spectrum table must have exactly 27 rows.
    #[error("sheet {sheet:?}: reference spectrum has {got} rows (expected exactly 27)")]
    SpectrumRowCount {
        /// Worksheet name.
        sheet: String,
        /// Observed row count.
        got: usize,
    },
    /// Path-traversal guard (T-01-02): refusing to read outside the
    /// discovery roots.
    #[error("refusing to read {path}: outside the allowed root {root}")]
    OutsideRoot {
        /// The allowed root.
        root: PathBuf,
        /// The rejected path.
        path: PathBuf,
    },
    /// Domain validation failure (out-of-range parameter etc.).
    #[error("{context}: {message}")]
    Invalid {
        /// Sheet or file.
        context: String,
        /// What was wrong.
        message: String,
    },
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

/// One discovery result: either a loaded case or a per-case load failure.
///
/// A malformed sheet/file becomes a Failed-to-load outcome for that one case
/// instead of aborting the whole run (T-01-01).
#[derive(Debug)]
pub struct DiscoveredCase {
    /// Stable id, also used as the trial name.
    pub id: String,
    /// The loaded case, or why loading it failed.
    pub case: Result<CaseDefinition, CaseLoadError>,
}

/// Everything discovery found, plus human-readable notes (e.g. "reference
/// workbooks not fetched — run refs/fetch.sh").
#[derive(Debug, Default)]
pub struct Discovery {
    /// Deterministically ordered case list (TOML cases sorted by file name,
    /// then straight-road sheets in workbook order, then placeholders).
    pub cases: Vec<DiscoveredCase>,
    /// Non-fatal observations for the runner / CLI to surface.
    pub notes: Vec<String>,
}

/// Path-traversal guard (T-01-02): canonicalize `candidate` and require it
/// to live under the canonicalized `root` before it may be opened.
pub(crate) fn confine(root: &Path, candidate: &Path) -> Result<PathBuf, CaseLoadError> {
    let canon_root = root.canonicalize().map_err(|source| CaseLoadError::Io {
        path: root.to_path_buf(),
        source,
    })?;
    let canon = candidate.canonicalize().map_err(|source| CaseLoadError::Io {
        path: candidate.to_path_buf(),
        source,
    })?;
    if canon.starts_with(&canon_root) {
        Ok(canon)
    } else {
        Err(CaseLoadError::OutsideRoot {
            root: canon_root,
            path: canon,
        })
    }
}

/// Discover every runnable case under `cases_dir` (synthetic TOML, sorted)
/// and `refs_dir` (FORCE workbooks, if fetched).
///
/// - Missing `refs_dir` / workbooks degrade to a note, never an error: the
///   suite must stay green without the copyrighted reference files.
/// - Only paths inside the two given roots are ever opened (T-01-02).
/// - A malformed file becomes a per-case load failure, not an abort.
#[must_use]
pub fn discover(refs_dir: &Path, cases_dir: &Path) -> Discovery {
    let mut discovery = Discovery::default();

    // 1. Synthetic TOML cases — sorted file list for deterministic order.
    match std::fs::read_dir(cases_dir) {
        Ok(entries) => {
            let mut paths: Vec<PathBuf> = entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
                })
                .collect();
            paths.sort();
            for path in paths {
                let stem = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "unnamed".to_string());
                let case =
                    confine(cases_dir, &path).and_then(|safe| toml::load_toml_case(&safe));
                discovery.cases.push(DiscoveredCase {
                    id: format!("toml::{stem}"),
                    case,
                });
            }
        }
        Err(_) => discovery.notes.push(format!(
            "cases directory {} not found — no synthetic cases discovered",
            cases_dir.display()
        )),
    }

    // 2. FORCE straight-road workbook (fully parsed).
    let straight = refs_dir.join("TestStraightRoad.xls");
    if straight.is_file() {
        match confine(refs_dir, &straight).and_then(|safe| xls::load_straight_road(&safe)) {
            Ok(mut cases) => discovery.cases.append(&mut cases),
            Err(e) => discovery.cases.push(DiscoveredCase {
                id: "straight_road::workbook".to_string(),
                case: Err(e),
            }),
        }
    } else {
        discovery.notes.push(
            "reference workbooks not fetched — run refs/fetch.sh \
             (FORCE .xls cases skipped)"
                .to_string(),
        );
    }

    // 3. Remaining FORCE workbooks: counted but not parsed until Phases 3-4;
    //    each present workbook surfaces as one Skipped placeholder case.
    for (file, group, kind, label) in [
        ("TestCurvedRoad.xls", "curved_road", CaseKind::ForceCurvedRoad, "Curved Road"),
        ("TestCityStreet.xls", "city_street", CaseKind::ForceCityStreet, "City Street"),
        (
            "TestYearlyAverage.xls",
            "yearly_average",
            CaseKind::ForceYearlyAverage,
            "Yearly Average",
        ),
    ] {
        let path = refs_dir.join(file);
        if !path.is_file() {
            continue;
        }
        let id = format!("{group}::workbook");
        discovery.cases.push(DiscoveredCase {
            id: id.clone(),
            case: Ok(CaseDefinition {
                id,
                name: format!("FORCE {label} workbook (placeholder)"),
                kind,
                reference_version: ReferenceVersion::Force2009,
                description: format!("{file} present — layout parsing lands in Phases 3-4"),
                source_position: None,
                receiver_position: None,
                propagation: PropagationParams::default(),
                terrain_profile: Vec::new(),
                reference_spectrum: None,
                expected: None,
            }),
        });
    }

    discovery
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .to_path_buf()
    }

    #[test]
    fn discovery_finds_the_committed_toml_cases() {
        let root = repo_root();
        let d = discover(&root.join("refs"), &root.join("cases"));
        let toml_ids: Vec<&str> = d
            .cases
            .iter()
            .filter(|c| c.id.starts_with("toml::"))
            .map(|c| c.id.as_str())
            .collect();
        assert!(
            toml_ids.contains(&"toml::freefield_100m"),
            "expected the committed free-field fixture, got {toml_ids:?}"
        );
        // deterministic order: sorted by id within the toml group
        let mut sorted = toml_ids.clone();
        sorted.sort_unstable();
        assert_eq!(toml_ids, sorted, "TOML cases must be discovered in sorted order");
    }

    #[test]
    fn discovery_without_refs_degrades_to_a_note() {
        let root = repo_root();
        let missing = root.join("no-such-refs-dir");
        let d = discover(&missing, &root.join("cases"));
        assert!(
            d.cases.iter().all(|c| !c.id.starts_with("straight_road::")),
            "no xls cases may be emitted without the workbooks"
        );
        assert!(
            d.notes.iter().any(|n| n.contains("refs/fetch.sh")),
            "a note must point the user at refs/fetch.sh, got {:?}",
            d.notes
        );
    }

    #[test]
    fn confine_rejects_paths_outside_the_root() {
        let root = repo_root();
        let cases_root = root.join("cases");
        // A real file that exists but lives OUTSIDE cases/:
        let outside = root.join("Cargo.toml");
        let err = confine(&cases_root, &outside).unwrap_err();
        assert!(
            matches!(err, CaseLoadError::OutsideRoot { .. }),
            "expected OutsideRoot, got {err:?}"
        );
        // And a file inside the root passes:
        let inside = cases_root.join("freefield_100m.toml");
        assert!(confine(&cases_root, &inside).is_ok());
    }
}
