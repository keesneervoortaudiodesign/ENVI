//! FORCE `.xls` (legacy BIFF) workbook loader via calamine.
//!
//! Label-anchored parsing is the PRIMARY key (01-RESEARCH): the parser scans
//! column A for the propagation labels (`hr`, `t0`, `z0`, `zu`, `u`, `φ`,
//! `su`, `dt/dz`, `sdt/dz`, `Cv2`, `Ct2`) and column F for the result labels
//! (`LAeq,24h`, `LAE`, `LAmax`, `Freq.`), reading each value from the
//! adjacent column. Fixed row positions are only a fallback (with a warning)
//! — this survives the 2010-revision layout drift.
//!
//! The workbook is untrusted input (T-01-01): every cell read returns
//! `Result`, non-finite numbers are rejected where a finite value is
//! required, profile X must be strictly ascending, and hard caps bound the
//! sheet count and row counts.

use std::path::Path;

use super::{CaseLoadError, DiscoveredCase, TerrainRow};

/// DoS guard: maximum number of worksheets accepted per workbook.
pub const MAX_SHEETS: usize = 200;

/// DoS guard: maximum number of terrain-profile rows accepted per sheet.
pub const MAX_PROFILE_ROWS: usize = 10_000;

/// Number of reference-spectrum rows every FORCE sheet must carry.
pub const SPECTRUM_ROWS: usize = 27;

/// Require a finite `f64`, with context for the error message.
pub(crate) fn require_finite(
    value: f64,
    context: &str,
    what: &str,
) -> Result<f64, CaseLoadError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(CaseLoadError::NonFinite {
            context: context.to_string(),
            what: what.to_string(),
        })
    }
}

/// Validate a parsed terrain profile: finite values, strictly ascending X,
/// row cap enforced.
pub(crate) fn validate_profile(
    rows: &[TerrainRow],
    context: &str,
) -> Result<(), CaseLoadError> {
    if rows.len() > MAX_PROFILE_ROWS {
        return Err(CaseLoadError::TooManyProfileRows {
            context: context.to_string(),
            count: rows.len(),
            cap: MAX_PROFILE_ROWS,
        });
    }
    for (i, row) in rows.iter().enumerate() {
        for (what, v) in [
            ("profile X", row.x_m),
            ("profile Z", row.z_m),
            ("flow resistivity", row.flow_resistivity_kns_m4),
            ("roughness", row.roughness_m),
        ] {
            require_finite(v, context, &format!("{what} (row {i})"))?;
        }
        if i > 0 {
            let prev_x = rows[i - 1].x_m;
            if row.x_m <= prev_x {
                return Err(CaseLoadError::NonAscendingProfile {
                    context: context.to_string(),
                    row: i,
                    prev_x,
                    x: row.x_m,
                });
            }
        }
    }
    Ok(())
}

/// Load every straight-road case ("1" … "62") from `TestStraightRoad.xls`.
///
/// Returns one [`DiscoveredCase`] per worksheet; a malformed sheet becomes a
/// per-case load error rather than aborting the workbook. Every loaded case
/// is tagged [`super::ReferenceVersion::Force2009`].
///
/// # Errors
///
/// Returns a workbook-level error only when the file cannot be opened or the
/// sheet cap is exceeded.
pub fn load_straight_road(path: &Path) -> Result<Vec<DiscoveredCase>, CaseLoadError> {
    let _ = path;
    todo!("Task 2 GREEN: calamine label-anchored parse of all 62 sheets")
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;
    use std::path::PathBuf;

    fn straight_road_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .join("refs")
            .join("TestStraightRoad.xls")
    }

    fn row(x: f64, z: f64, sigma: f64, rough: f64) -> TerrainRow {
        TerrainRow {
            x_m: x,
            z_m: z,
            flow_resistivity_kns_m4: sigma,
            roughness_m: rough,
        }
    }

    /// Full-precision anchor from the authoritative .xls cells (Pitfall 1:
    /// NEVER the rounded report-appendix values). Auto-skips (green, with a
    /// note) when refs/ has not been fetched — libtest cannot dynamically
    /// mark unit tests ignored; the force runner surfaces the skip properly.
    #[test]
    fn straight_road_loads_62_cases_with_full_precision_anchor() {
        let path = straight_road_path();
        if !path.is_file() {
            eprintln!("SKIP: {} not fetched — run refs/fetch.sh", path.display());
            return;
        }
        let cases = load_straight_road(&path).expect("workbook must open");
        assert_eq!(cases.len(), 62, "TestStraightRoad.xls carries 62 cases");

        let case1 = cases[0].case.as_ref().expect("case 1 must load");
        assert_eq!(cases[0].id, "straight_road::1");
        assert!(
            case1.description.contains("Flat terrain"),
            "case 1 description: {:?}",
            case1.description
        );

        let spectrum = case1.reference_spectrum.as_ref().expect("FORCE case has reference");
        // Full-precision cell value, abs 1e-9 (the report appendix prints 39.4)
        assert_abs_diff_eq!(spectrum.laeq_24h_db, 39.39836757521, epsilon = 1e-9);
        assert_eq!(spectrum.bands.len(), SPECTRUM_ROWS);

        // Terrain semantics: first X is 3.25 m from the road centre line,
        // strictly ascending thereafter.
        let profile = &case1.terrain_profile;
        assert_abs_diff_eq!(profile[0].x_m, 3.25, epsilon = 1e-12);
        assert!(
            profile.windows(2).all(|w| w[0].x_m < w[1].x_m),
            "profile X must be strictly ascending: {profile:?}"
        );

        // Every xls case carries Force2009 provenance and all cases load.
        for c in &cases {
            let def = c.case.as_ref().unwrap_or_else(|e| panic!("{}: {e}", c.id));
            assert_eq!(def.reference_version, super::super::ReferenceVersion::Force2009);
            assert_eq!(def.kind, super::super::CaseKind::ForceStraightRoad);
        }
    }

    #[test]
    fn nan_cell_values_are_rejected() {
        let rows = [row(3.25, 0.0, f64::NAN, 0.0)];
        let err = validate_profile(&rows, "test").unwrap_err();
        assert!(matches!(err, CaseLoadError::NonFinite { .. }), "got {err:?}");
    }

    #[test]
    fn non_ascending_profile_x_is_rejected() {
        let rows = [row(3.25, 0.0, 12.5, 0.0), row(3.25, 0.0, 12.5, 0.0)];
        let err = validate_profile(&rows, "test").unwrap_err();
        assert!(matches!(err, CaseLoadError::NonAscendingProfile { .. }), "got {err:?}");

        let rows = [row(5.0, 0.0, 12.5, 0.0), row(3.0, 0.0, 12.5, 0.0)];
        let err = validate_profile(&rows, "test").unwrap_err();
        assert!(matches!(err, CaseLoadError::NonAscendingProfile { .. }), "got {err:?}");
    }

    #[test]
    fn profile_row_cap_is_enforced() {
        let rows: Vec<TerrainRow> = (0..=MAX_PROFILE_ROWS)
            .map(|i| row(i as f64, 0.0, 12.5, 0.0))
            .collect();
        let err = validate_profile(&rows, "test").unwrap_err();
        assert!(matches!(err, CaseLoadError::TooManyProfileRows { .. }), "got {err:?}");
    }

    #[test]
    fn require_finite_rejects_nan_and_infinity() {
        assert!(require_finite(1.0, "t", "x").is_ok());
        assert!(require_finite(f64::NAN, "t", "x").is_err());
        assert!(require_finite(f64::INFINITY, "t", "x").is_err());
        assert!(require_finite(f64::NEG_INFINITY, "t", "x").is_err());
    }
}
