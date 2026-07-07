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

use calamine::{Data, Range, Reader, Xls, open_workbook};

use super::{
    CaseDefinition, CaseKind, CaseLoadError, DiscoveredCase, PropagationParams, ReferenceSpectrum,
    ReferenceVersion, SpectrumRow, TerrainRow,
};

/// DoS guard: maximum number of worksheets accepted per workbook.
pub const MAX_SHEETS: usize = 200;

/// DoS guard: maximum number of terrain-profile rows accepted per sheet.
pub const MAX_PROFILE_ROWS: usize = 10_000;

/// Number of reference-spectrum rows every FORCE sheet must carry.
pub const SPECTRUM_ROWS: usize = 27;

/// Require a finite `f64`, with context for the error message.
pub(crate) fn require_finite(value: f64, context: &str, what: &str) -> Result<f64, CaseLoadError> {
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
pub(crate) fn validate_profile(rows: &[TerrainRow], context: &str) -> Result<(), CaseLoadError> {
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

/// Trimmed string content of a cell, if it is a string cell.
fn cell_str(range: &Range<Data>, row: u32, col: u32) -> Option<String> {
    match range.get_value((row, col)) {
        Some(Data::String(s)) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        }
        _ => None,
    }
}

/// Numeric content of a cell (Float or Int), if any. NaN screening happens
/// at the call sites via [`require_finite`].
fn cell_num(range: &Range<Data>, row: u32, col: u32) -> Option<f64> {
    match range.get_value((row, col)) {
        Some(Data::Float(v)) => Some(*v),
        Some(Data::Int(i)) => Some(*i as f64),
        _ => None,
    }
}

/// Last row index (absolute) of the used range.
fn last_row(range: &Range<Data>) -> u32 {
    range.end().map_or(0, |(r, _)| r)
}

/// Find the (absolute) row whose cell in `col` equals one of `labels`
/// (trimmed, exact match — prefix matching would confuse "LAE"/"LAeq,24h").
fn find_label_row(range: &Range<Data>, col: u32, labels: &[&str]) -> Option<u32> {
    let start = range.start().map_or(0, |(r, _)| r);
    (start..=last_row(range))
        .find(|&row| cell_str(range, row, col).is_some_and(|s| labels.iter().any(|l| s == *l)))
}

/// Read the value adjacent to a label: label-anchored as the PRIMARY key,
/// fixed row position as the fallback (with a warning to stderr) — this
/// survives the 2010-revision layout drift.
fn labelled_num(
    range: &Range<Data>,
    sheet: &str,
    label_col: u32,
    value_col: u32,
    labels: &[&str],
    fallback_row: u32,
) -> Result<Option<f64>, CaseLoadError> {
    let context = format!("sheet {sheet}");
    if let Some(row) = find_label_row(range, label_col, labels) {
        return match cell_num(range, row, value_col) {
            Some(v) => Ok(Some(require_finite(v, &context, labels[0])?)),
            None => Err(CaseLoadError::NonFinite {
                context,
                what: labels[0].to_string(),
            }),
        };
    }
    // Fallback: fixed row position from the verified 2009 layout.
    eprintln!(
        "warning: sheet {sheet}: label {:?} not found — falling back to fixed row {fallback_row}",
        labels[0]
    );
    match cell_num(range, fallback_row, value_col) {
        Some(v) => Ok(Some(require_finite(v, &context, labels[0])?)),
        None => Ok(None),
    }
}

/// Parse one straight-road worksheet (verified 2009 layout, label-anchored).
fn parse_sheet(range: &Range<Data>, sheet: &str) -> Result<CaseDefinition, CaseLoadError> {
    let context = format!("sheet {sheet}");

    // Description: row 2 col A in the verified layout; fall back to the
    // first non-title string in rows 1..=4.
    let description = cell_str(range, 2, 0)
        .or_else(|| (1..=4).find_map(|r| cell_str(range, r, 0)))
        .ok_or_else(|| CaseLoadError::Invalid {
            context: context.clone(),
            message: "no description string found in rows 1-4".to_string(),
        })?;

    let name = cell_str(range, 0, 0).unwrap_or_else(|| format!("Straight Road Case {sheet}"));

    // Propagation block: labels in col A (0), values in col B (1).
    // RH is not on the sheet — 70 % globally per the Env. Project 1335 text
    // (the PropagationParams default).
    let propagation = PropagationParams {
        hr_m: labelled_num(range, sheet, 0, 1, &["hr"], 11)?,
        t0_c: labelled_num(range, sheet, 0, 1, &["t0"], 12)?,
        z0_m: labelled_num(range, sheet, 0, 1, &["z0"], 13)?,
        zu_m: labelled_num(range, sheet, 0, 1, &["zu"], 14)?,
        u_ms: labelled_num(range, sheet, 0, 1, &["u"], 15)?,
        phi_deg: labelled_num(range, sheet, 0, 1, &["φ", "phi", "fi"], 16)?,
        su_ms: labelled_num(range, sheet, 0, 1, &["su"], 17)?,
        dtdz: labelled_num(range, sheet, 0, 1, &["dt/dz"], 18)?,
        sdtdz: labelled_num(range, sheet, 0, 1, &["sdt/dz"], 19)?,
        cv2: labelled_num(range, sheet, 0, 1, &["Cv2"], 20)?,
        ct2: labelled_num(range, sheet, 0, 1, &["Ct2"], 21)?,
        ..PropagationParams::default()
    };

    // Overall results: labels in col F (5), values in col G (6).
    // Exact label match matters: "LAeq,24h" also starts with "LAE".
    let laeq_24h_db =
        labelled_num(range, sheet, 5, 6, &["LAeq,24h", "LAeq"], 6)?.ok_or_else(|| {
            CaseLoadError::MissingLabel {
                sheet: sheet.to_string(),
                label: "LAeq,24h".to_string(),
            }
        })?;
    let lae_db = labelled_num(range, sheet, 5, 6, &["LAE"], 7)?.ok_or_else(|| {
        CaseLoadError::MissingLabel {
            sheet: sheet.to_string(),
            label: "LAE".to_string(),
        }
    })?;
    let lamax_db = labelled_num(range, sheet, 5, 6, &["LAmax"], 8)?.ok_or_else(|| {
        CaseLoadError::MissingLabel {
            sheet: sheet.to_string(),
            label: "LAmax".to_string(),
        }
    })?;

    // 27-band reference spectrum below the "Freq." label (col F..I), skipping
    // the units row ("Hz" is a string cell, not a number).
    let freq_row = find_label_row(range, 5, &["Freq.", "Freq"]).ok_or_else(|| {
        CaseLoadError::MissingLabel {
            sheet: sheet.to_string(),
            label: "Freq.".to_string(),
        }
    })?;
    let mut bands = Vec::with_capacity(SPECTRUM_ROWS);
    for row in (freq_row + 1)..=last_row(range) {
        let Some(nominal) = cell_num(range, row, 5) else {
            continue; // units row / trailing blanks
        };
        if bands.len() == SPECTRUM_ROWS {
            return Err(CaseLoadError::SpectrumRowCount {
                sheet: sheet.to_string(),
                got: bands.len() + 1,
            });
        }
        let band_ctx = format!("spectrum row {}", bands.len());
        let leq = cell_num(range, row, 6).ok_or_else(|| CaseLoadError::NonFinite {
            context: context.clone(),
            what: format!("{band_ctx}: Leq,24h"),
        })?;
        let le = cell_num(range, row, 7).ok_or_else(|| CaseLoadError::NonFinite {
            context: context.clone(),
            what: format!("{band_ctx}: LE"),
        })?;
        let dl = cell_num(range, row, 8).ok_or_else(|| CaseLoadError::NonFinite {
            context: context.clone(),
            what: format!("{band_ctx}: dL"),
        })?;
        bands.push(SpectrumRow {
            nominal_hz: require_finite(nominal, &context, &format!("{band_ctx}: Freq."))?,
            leq_24h_db: require_finite(leq, &context, &format!("{band_ctx}: Leq,24h"))?,
            le_db: require_finite(le, &context, &format!("{band_ctx}: LE"))?,
            dl_db: require_finite(dl, &context, &format!("{band_ctx}: dL"))?,
        });
    }
    if bands.len() != SPECTRUM_ROWS {
        return Err(CaseLoadError::SpectrumRowCount {
            sheet: sheet.to_string(),
            got: bands.len(),
        });
    }

    // Terrain profile: header row "X" | "Z" in cols A/B, units row below,
    // then data rows in cols A-D until a zero-padded (all-zero) row, a
    // non-numeric row, or the end of the used range.
    let header_row = (0..=last_row(range))
        .find(|&r| {
            cell_str(range, r, 0).is_some_and(|s| s == "X")
                && cell_str(range, r, 1).is_some_and(|s| s == "Z")
        })
        .ok_or_else(|| CaseLoadError::MissingLabel {
            sheet: sheet.to_string(),
            label: "X/Z terrain profile header".to_string(),
        })?;
    let mut terrain_profile: Vec<TerrainRow> = Vec::new();
    for row in (header_row + 1)..=last_row(range) {
        let Some(x) = cell_num(range, row, 0) else {
            if terrain_profile.is_empty() {
                continue; // units row directly under the header
            }
            break;
        };
        let z = cell_num(range, row, 1).unwrap_or(0.0);
        let sigma = cell_num(range, row, 2).unwrap_or(0.0);
        let rough = cell_num(range, row, 3).unwrap_or(0.0);
        if !terrain_profile.is_empty() && x == 0.0 && z == 0.0 && sigma == 0.0 && rough == 0.0 {
            break; // zero-padded terminator
        }
        terrain_profile.push(TerrainRow {
            x_m: x,
            z_m: z,
            flow_resistivity_kns_m4: sigma,
            roughness_m: rough,
        });
        if terrain_profile.len() > MAX_PROFILE_ROWS {
            break; // validate_profile reports the cap violation
        }
    }
    validate_profile(&terrain_profile, &context)?;

    Ok(CaseDefinition {
        id: format!("straight_road::{sheet}"),
        name,
        kind: CaseKind::ForceStraightRoad,
        reference_version: ReferenceVersion::Force2009,
        description,
        source_position: None, // derived from lane/height conventions in plan 01-02
        receiver_position: None,
        propagation,
        terrain_profile,
        reference_spectrum: Some(ReferenceSpectrum {
            bands,
            laeq_24h_db,
            lae_db,
            lamax_db,
        }),
        expected: None,
    })
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
    let mut workbook =
        open_workbook::<Xls<std::io::BufReader<std::fs::File>>, _>(path).map_err(|e| {
            CaseLoadError::Workbook {
                path: path.to_path_buf(),
                message: e.to_string(),
            }
        })?;
    let sheet_names = workbook.sheet_names();
    if sheet_names.len() > MAX_SHEETS {
        return Err(CaseLoadError::TooManySheets {
            path: path.to_path_buf(),
            count: sheet_names.len(),
            cap: MAX_SHEETS,
        });
    }

    let mut cases = Vec::with_capacity(sheet_names.len());
    for sheet in &sheet_names {
        let case = workbook
            .worksheet_range(sheet)
            .map_err(|e| CaseLoadError::Workbook {
                path: path.to_path_buf(),
                message: format!("sheet {sheet}: {e}"),
            })
            .and_then(|range| parse_sheet(&range, sheet));
        cases.push(DiscoveredCase {
            id: format!("straight_road::{sheet}"),
            case,
        });
    }
    Ok(cases)
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

        let spectrum = case1
            .reference_spectrum
            .as_ref()
            .expect("FORCE case has reference");
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
            assert_eq!(
                def.reference_version,
                super::super::ReferenceVersion::Force2009
            );
            assert_eq!(def.kind, super::super::CaseKind::ForceStraightRoad);
        }
    }

    #[test]
    fn nan_cell_values_are_rejected() {
        let rows = [row(3.25, 0.0, f64::NAN, 0.0)];
        let err = validate_profile(&rows, "test").unwrap_err();
        assert!(
            matches!(err, CaseLoadError::NonFinite { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn non_ascending_profile_x_is_rejected() {
        let rows = [row(3.25, 0.0, 12.5, 0.0), row(3.25, 0.0, 12.5, 0.0)];
        let err = validate_profile(&rows, "test").unwrap_err();
        assert!(
            matches!(err, CaseLoadError::NonAscendingProfile { .. }),
            "got {err:?}"
        );

        let rows = [row(5.0, 0.0, 12.5, 0.0), row(3.0, 0.0, 12.5, 0.0)];
        let err = validate_profile(&rows, "test").unwrap_err();
        assert!(
            matches!(err, CaseLoadError::NonAscendingProfile { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn profile_row_cap_is_enforced() {
        let rows: Vec<TerrainRow> = (0..=MAX_PROFILE_ROWS)
            .map(|i| row(i as f64, 0.0, 12.5, 0.0))
            .collect();
        let err = validate_profile(&rows, "test").unwrap_err();
        assert!(
            matches!(err, CaseLoadError::TooManyProfileRows { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn require_finite_rejects_nan_and_infinity() {
        assert!(require_finite(1.0, "t", "x").is_ok());
        assert!(require_finite(f64::NAN, "t", "x").is_err());
        assert!(require_finite(f64::INFINITY, "t", "x").is_err());
        assert!(require_finite(f64::NEG_INFINITY, "t", "x").is_err());
    }
}
