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

use envi_engine::scene::{GroundSegment, TerrainProfile};

use super::{
    CaseDefinition, CaseKind, CaseLoadError, CityCoordinates, Contour, Coord3, CurvedCoordinates,
    DiscoveredCase, PropagationParams, ReferenceSpectrum, ReferenceVersion, SpectrumRow,
    TerrainRow,
};

/// DoS guard: maximum number of worksheets accepted per workbook.
pub const MAX_SHEETS: usize = 200;

/// DoS guard: maximum number of terrain-profile rows accepted per sheet.
pub const MAX_PROFILE_ROWS: usize = 10_000;

/// DoS guard: maximum number of coordinate rows scanned on a `Coordinates`
/// sheet (contour lines, screens, buildings, receivers).
pub const MAX_COORD_ROWS: usize = 20_000;

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
        source_spectrum: super::SourceSpectrum::default(),
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

// ---------------------------------------------------------------------------
// Curved / city / yearly loaders (plan 04-04) — extend the label-anchored idiom,
// never fork the parser.
// ---------------------------------------------------------------------------

/// The first numeric cell in `row` at a column strictly greater than `after_col`
/// (used to read a value laid out to the right of its label, whatever the exact
/// column — curved/city/yearly value columns differ).
fn first_num_in_row_after(range: &Range<Data>, row: u32, after_col: u32) -> Option<f64> {
    let width = range.width() as u32;
    ((after_col + 1)..width).find_map(|c| cell_num(range, row, c))
}

/// Read a `Coordinates`-sheet section: locate the `section` label in column A or
/// B, skip to its `X`/`Y`(/`Z`) header, then read numeric rows until a blank
/// (non-numeric X) row. `want_z` selects 3-column (X Y Z) vs 2-column (X Y).
///
/// Returns the rows plus the (0-based) column-D style label of the FIRST data
/// row (used to split contour lines by their `"Contour no. N"` annotations).
/// One raw coordinate row: `(x, y, z, annotation)` where the annotation is the
/// first string cell to the right of Z (e.g. `"Contour no. 1"`).
type CoordRow = (f64, f64, f64, Option<String>);

fn read_coord_section(
    range: &Range<Data>,
    sheet: &str,
    section_labels: &[&str],
    multi_block: bool,
) -> Result<Vec<CoordRow>, CaseLoadError> {
    let max_row = last_row(range).min(MAX_COORD_ROWS as u32);
    // Section headers are truncated/annotated ("Road centreline coordinates",
    // "Building #1", "Receivers …") — match by case-insensitive prefix in the
    // first three columns (city uses col B for its labels).
    let matches_label = |s: &str| {
        let lower = s.to_lowercase();
        section_labels
            .iter()
            .any(|l| lower.starts_with(&l.to_lowercase()))
    };
    let header = (0..=max_row)
        .find(|&r| (0..3u32).any(|c| cell_str(range, r, c).is_some_and(|s| matches_label(&s))))
        .ok_or_else(|| CaseLoadError::MissingSection {
            sheet: sheet.to_string(),
            section: section_labels[0].to_string(),
        })?;

    // The "X" column header may sit one row ABOVE the label (curved centreline,
    // city Building #1/#2) or ON the label row (city Building #3/#4, Receivers).
    let lo = header.saturating_sub(1);
    let xy_header = (lo..=(header + 4).min(max_row))
        .find(|&r| (0..3).any(|c| cell_str(range, r, c).is_some_and(|s| s == "X")))
        .unwrap_or(header);
    let x_col = (0..3)
        .find(|&c| cell_str(range, xy_header, c).is_some_and(|s| s == "X"))
        .unwrap_or(1);

    // Data starts at the first row after the X header, but never before the
    // section label row (city Building #1 has its data on the label row, which
    // sits below the X header).
    let data_start = (xy_header + 1).max(header);
    let mut rows = Vec::new();
    for r in data_start..=max_row {
        let Some(x) = cell_num(range, r, x_col) else {
            if rows.is_empty() {
                continue;
            }
            if multi_block {
                // Blank rows separate contour lines within one section — skip
                // them and keep reading to the end of the sheet (the terrain
                // section is last).
                continue;
            }
            break; // blank row terminates a single-block section
        };
        let y = cell_num(range, r, x_col + 1).unwrap_or(0.0);
        let z = cell_num(range, r, x_col + 2).unwrap_or(0.0);
        // Annotation column (e.g. "Contour no. 1", "Start of circular"): the
        // first string column to the right of Z.
        let note = (x_col + 3..range.width() as u32).find_map(|c| cell_str(range, r, c));
        for (what, v) in [("x", x), ("y", y), ("z", z)] {
            require_finite(
                v,
                &format!("sheet {sheet} section {}", section_labels[0]),
                what,
            )?;
        }
        rows.push((x, y, z, note));
        if rows.len() > MAX_COORD_ROWS {
            break;
        }
    }
    Ok(rows)
}

/// Read a labelled XYZ section into a `Vec<Coord3>` (annotations discarded),
/// tolerating an absent section (returns an empty vec — thin/thick screens are
/// optional).
fn optional_xyz(range: &Range<Data>, sheet: &str, labels: &[&str]) -> Vec<Coord3> {
    read_coord_section(range, sheet, labels, false).map_or_else(
        |_| Vec::new(),
        |rows| {
            rows.into_iter()
                .map(|(x, y, z, _)| Coord3 { x, y, z })
                .collect()
        },
    )
}

/// Parse the curved-road `Coordinates` sheet into typed geometry (EP 1335 Ch. 4):
/// road centre line, thin/thick screens + base, and the terrain contour lines
/// (split by their `"Contour no. N"` annotations, each an iso-elevation polyline).
///
/// # Errors
///
/// [`CaseLoadError::MissingSection`] if the road-centre-line or terrain sections
/// are absent; [`CaseLoadError::NonFinite`] on a non-finite coordinate cell.
pub fn parse_curved_coordinates(
    range: &Range<Data>,
    sheet: &str,
) -> Result<CurvedCoordinates, CaseLoadError> {
    let centreline = read_coord_section(
        range,
        sheet,
        &["Road centreline", "Road centre line"],
        false,
    )?
    .into_iter()
    .map(|(x, y, z, _)| Coord3 { x, y, z })
    .collect();

    let thin_screen = optional_xyz(range, sheet, &["Thin screen coordinates", "Thin screen"]);
    let thick_screen = optional_xyz(range, sheet, &["Thick screen coordinates", "Thick screen"]);
    let thick_screen_base = optional_xyz(range, sheet, &["Thick screen base"]);

    // Terrain contour lines: one section, internally split by "Contour no. N".
    let terrain = read_coord_section(range, sheet, &["Terrain coordinates", "Terrain"], true)?;
    let mut contours: Vec<Contour> = Vec::new();
    for (x, y, z, note) in terrain {
        let starts_new = note
            .as_deref()
            .is_some_and(|s| s.to_lowercase().contains("contour"));
        if starts_new || contours.is_empty() {
            contours.push(Contour {
                elevation_z: z,
                xy: Vec::new(),
            });
        }
        if let Some(c) = contours.last_mut() {
            c.xy.push([x, y]);
        }
    }

    Ok(CurvedCoordinates {
        centreline,
        thin_screen,
        thick_screen,
        thick_screen_base,
        contours,
    })
}

/// Parse the city-street `Coordinates` sheet into building footprints and
/// receiver positions (EP 1335 Ch. 5).
///
/// # Errors
///
/// [`CaseLoadError::MissingSection`] if the `Receivers` section is absent;
/// [`CaseLoadError::NonFinite`] on a non-finite coordinate cell.
pub fn parse_city_coordinates(
    range: &Range<Data>,
    sheet: &str,
) -> Result<CityCoordinates, CaseLoadError> {
    let mut buildings = Vec::new();
    for n in 1..=32 {
        let label = format!("Building #{n}");
        match read_coord_section(range, sheet, &[label.as_str()], false) {
            Ok(rows) => buildings.push(rows.into_iter().map(|(x, y, _z, _)| [x, y]).collect()),
            Err(_) => break, // no more numbered buildings
        }
    }
    let receivers = read_coord_section(range, sheet, &["Receivers", "Receiver"], false)?
        .into_iter()
        .map(|(x, y, _z, _)| [x, y])
        .collect();
    Ok(CityCoordinates {
        buildings,
        receivers,
    })
}

/// Build a per-source-point cut-plane [`TerrainProfile`] by interpolating the
/// terrain contour lines along the straight cut from `source_xy` to
/// `receiver_xy` (04-RESEARCH: Coordinates → cut-plane profile).
///
/// Each contour polyline is intersected with the cut line; every intersection
/// contributes a `(distance-along-cut, elevation)` sample. Samples are sorted by
/// distance (deduplicated), and the flat `sigma`/`roughness` are attached to
/// every segment. The profile `x` axis is the horizontal distance along the cut,
/// starting at the first contour crossing (the profile is NOT force-anchored to a
/// `x = 0` source-foot endpoint — source/receiver-foot endpoint handling is
/// deferred to when the curved numeric path is wired, since it needs an
/// extrapolated elevation rather than a contour intersection).
///
/// # Errors
///
/// [`CaseLoadError::Geometry`] if fewer than two distinct samples result (no
/// usable profile), or [`CaseLoadError::Geometry`] wrapping a
/// [`envi_engine::scene::SceneError`] if the assembled profile fails validation.
pub fn contour_profile(
    contours: &[Contour],
    source_xy: [f64; 2],
    receiver_xy: [f64; 2],
    sigma_kpa: f64,
    roughness_m: f64,
) -> Result<TerrainProfile, CaseLoadError> {
    let dir = [receiver_xy[0] - source_xy[0], receiver_xy[1] - source_xy[1]];
    let cut_len = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
    if !(cut_len.is_finite() && cut_len > 1e-9) {
        return Err(CaseLoadError::Geometry {
            context: "contour profile".to_string(),
            message: "degenerate source→receiver cut line".to_string(),
        });
    }
    let unit = [dir[0] / cut_len, dir[1] / cut_len];

    // Distance along the cut of a point projected onto the cut line.
    let mut samples: Vec<(f64, f64)> = Vec::new();
    for contour in contours {
        for seg in contour.xy.windows(2) {
            if let Some(t) = segment_cut_intersection(source_xy, unit, cut_len, seg[0], seg[1]) {
                samples.push((t, contour.elevation_z));
            }
        }
    }
    // Deduplicate near-equal distances (keep the first), sort ascending.
    samples.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut points: Vec<[f64; 2]> = Vec::new();
    for (t, z) in samples {
        if points.last().is_none_or(|p| (t - p[0]).abs() > 1e-6) {
            points.push([t, z]);
        }
    }
    if points.len() < 2 {
        return Err(CaseLoadError::Geometry {
            context: "contour profile".to_string(),
            message: format!(
                "only {} distinct contour samples along the cut",
                points.len()
            ),
        });
    }
    let segments: Vec<GroundSegment> = (0..points.len() - 1)
        .map(|_| GroundSegment {
            flow_resistivity: sigma_kpa,
            roughness: roughness_m,
        })
        .collect();
    TerrainProfile::new(points, segments).map_err(|e| CaseLoadError::Geometry {
        context: "contour profile".to_string(),
        message: e.to_string(),
    })
}

/// Intersect a terrain-contour segment `[a, b]` (in the XY plane) with the cut
/// line starting at `origin` with unit direction `unit`, of length `cut_len`.
/// Returns the along-cut distance `t ∈ [0, cut_len]` of the intersection, if the
/// segment crosses the cut line within both extents.
fn segment_cut_intersection(
    origin: [f64; 2],
    unit: [f64; 2],
    cut_len: f64,
    a: [f64; 2],
    b: [f64; 2],
) -> Option<f64> {
    // Cut point C(t) = origin + t·unit; segment point P(s) = a + s·(b−a).
    // Solve origin + t·unit = a + s·e for (t, s).
    let e = [b[0] - a[0], b[1] - a[1]];
    let det = unit[0] * (-e[1]) - unit[1] * (-e[0]);
    if det.abs() < 1e-12 {
        return None; // parallel
    }
    let rhs = [a[0] - origin[0], a[1] - origin[1]];
    let t = (rhs[0] * (-e[1]) - rhs[1] * (-e[0])) / det;
    let s = (unit[0] * rhs[1] - unit[1] * rhs[0]) / det;
    if (0.0..=1.0).contains(&s) && (0.0..=cut_len).contains(&t) {
        Some(t)
    } else {
        None
    }
}

/// Read a curved/city/yearly case sheet's overall level + 27-band reference
/// spectrum, label-anchored on `overall_labels` and `"Freq."` (the value column
/// varies by workbook — the first numeric cell to the right of the label is
/// taken). Populates a [`ReferenceSpectrum`]; `le`/`dl` are set to the band
/// value / `0.0` (curved/city/yearly sheets carry no free-field `LE`/`dL`).
fn parse_case_sheet_spectrum(
    range: &Range<Data>,
    sheet: &str,
    overall_labels: &[&str],
) -> Result<ReferenceSpectrum, CaseLoadError> {
    let context = format!("sheet {sheet}");
    // Overall level: label anywhere in cols A..G, value = first numeric to the
    // right.
    let overall_row = (0..=last_row(range))
        .find(|&r| {
            (0..(range.width() as u32).min(8)).any(|c| {
                cell_str(range, r, c).is_some_and(|s| overall_labels.iter().any(|l| s == *l))
            })
        })
        .ok_or_else(|| CaseLoadError::MissingLabel {
            sheet: sheet.to_string(),
            label: overall_labels[0].to_string(),
        })?;
    let overall_col = (0..(range.width() as u32).min(8))
        .find(|&c| {
            cell_str(range, overall_row, c).is_some_and(|s| overall_labels.iter().any(|l| s == *l))
        })
        .unwrap_or(0);
    let overall = first_num_in_row_after(range, overall_row, overall_col).ok_or_else(|| {
        CaseLoadError::NonFinite {
            context: context.clone(),
            what: overall_labels[0].to_string(),
        }
    })?;
    let overall = require_finite(overall, &context, overall_labels[0])?;

    // 27-band spectrum below the "Freq." label (may read "Freq. (Hz)").
    let is_freq = |s: &str| s.starts_with("Freq");
    let freq_row = (0..=last_row(range))
        .find(|&r| {
            (0..(range.width() as u32).min(8))
                .any(|c| cell_str(range, r, c).is_some_and(|s| is_freq(&s)))
        })
        .ok_or_else(|| CaseLoadError::MissingLabel {
            sheet: sheet.to_string(),
            label: "Freq.".to_string(),
        })?;
    let freq_col = (0..(range.width() as u32).min(8))
        .find(|&c| cell_str(range, freq_row, c).is_some_and(|s| is_freq(&s)))
        .unwrap_or(0);

    let mut bands = Vec::with_capacity(SPECTRUM_ROWS);
    for row in (freq_row + 1)..=last_row(range) {
        let Some(nominal) = cell_num(range, row, freq_col) else {
            continue; // units row / blanks
        };
        if bands.len() == SPECTRUM_ROWS {
            break;
        }
        let value = first_num_in_row_after(range, row, freq_col).ok_or_else(|| {
            CaseLoadError::NonFinite {
                context: context.clone(),
                what: format!("spectrum row {}", bands.len()),
            }
        })?;
        bands.push(SpectrumRow {
            nominal_hz: require_finite(nominal, &context, "Freq.")?,
            leq_24h_db: require_finite(value, &context, "band value")?,
            le_db: require_finite(value, &context, "band value")?,
            dl_db: 0.0,
        });
    }
    if bands.len() != SPECTRUM_ROWS {
        return Err(CaseLoadError::SpectrumRowCount {
            sheet: sheet.to_string(),
            got: bands.len(),
        });
    }
    // LAmax where present (curved sheets), else fall back to the overall level.
    let lamax = (0..=last_row(range))
        .find(|&r| (0..2).any(|c| cell_str(range, r, c).is_some_and(|s| s == "LAmax")))
        .and_then(|r| first_num_in_row_after(range, r, 0))
        .unwrap_or(overall);

    Ok(ReferenceSpectrum {
        bands,
        laeq_24h_db: overall,
        lae_db: overall,
        lamax_db: lamax,
    })
}

/// Load a curved/city/yearly workbook into one [`DiscoveredCase`] per numeric
/// case sheet (sheets named "1", "1L", … — the `Coordinates` / `Met. statistics`
/// sheets are skipped). Each case carries its parsed reference spectrum and the
/// given [`CaseKind`]; a malformed sheet becomes a per-case load error.
///
/// The road geometry (curved contour cut-planes / city façade reflections) is
/// re-derived by the `run_case` arm; these cases fail-soft to `Skipped` on the
/// `[ASSUMED]` emission-coefficient gate (honest-green), so the loader's job is
/// to surface every real case with its reference, replacing the old
/// one-placeholder-per-workbook.
///
/// # Errors
///
/// [`CaseLoadError::Workbook`] only when the file cannot be opened or the sheet
/// cap is exceeded.
fn load_force_workbook(
    path: &Path,
    kind: CaseKind,
    id_prefix: &str,
    overall_labels: &[&str],
) -> Result<Vec<DiscoveredCase>, CaseLoadError> {
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

    let mut cases = Vec::new();
    for sheet in &sheet_names {
        // Skip the geometry / statistics sheets — only numeric-prefixed case
        // sheets carry a result spectrum.
        if sheet == "Coordinates" || sheet == "Met. statistics" {
            continue;
        }
        let case = workbook
            .worksheet_range(sheet)
            .map_err(|e| CaseLoadError::Workbook {
                path: path.to_path_buf(),
                message: format!("sheet {sheet}: {e}"),
            })
            .and_then(|range| {
                let description = cell_str(&range, 2, 0)
                    .or_else(|| (1..=4).find_map(|r| cell_str(&range, r, 0)))
                    .unwrap_or_else(|| format!("{id_prefix} case {sheet}"));
                let spectrum = parse_case_sheet_spectrum(&range, sheet, overall_labels)?;
                Ok(CaseDefinition {
                    id: format!("{id_prefix}::{sheet}"),
                    name: format!("FORCE {id_prefix} case {sheet}"),
                    kind,
                    reference_version: ReferenceVersion::Force2009,
                    description,
                    source_position: None,
                    source_spectrum: super::SourceSpectrum::default(),
                    receiver_position: None,
                    propagation: PropagationParams::default(),
                    terrain_profile: Vec::new(),
                    reference_spectrum: Some(spectrum),
                    expected: None,
                })
            });
        cases.push(DiscoveredCase {
            id: format!("{id_prefix}::{sheet}"),
            case,
        });
    }
    Ok(cases)
}

/// Load `TestCurvedRoad.xls` — one case per numeric sheet (EP 1335 Ch. 4).
///
/// # Errors
///
/// As [`load_force_workbook`].
pub fn load_curved_road(path: &Path) -> Result<Vec<DiscoveredCase>, CaseLoadError> {
    load_force_workbook(
        path,
        CaseKind::ForceCurvedRoad,
        "curved_road",
        &["LAeq,24h", "LAeq"],
    )
}

/// Load `TestCityStreet.xls` — one case per numeric sheet (EP 1335 Ch. 5).
///
/// # Errors
///
/// As [`load_force_workbook`].
pub fn load_city_street(path: &Path) -> Result<Vec<DiscoveredCase>, CaseLoadError> {
    load_force_workbook(
        path,
        CaseKind::ForceCityStreet,
        "city_street",
        &["LAeq,24h", "LAeq"],
    )
}

/// Load `TestYearlyAverage.xls` — one case per numeric sheet (EP 1335 Ch. 3);
/// the overall level is `L_den` (Danish 12/3/9 hours, Pitfall 4).
///
/// # Errors
///
/// As [`load_force_workbook`].
pub fn load_yearly(path: &Path) -> Result<Vec<DiscoveredCase>, CaseLoadError> {
    load_force_workbook(
        path,
        CaseKind::ForceYearlyAverage,
        "yearly_average",
        &["Lden (dB)", "Lden", "L_den", "LAeq,24h"],
    )
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

    // ---- Curved/city/yearly (plan 04-04) ----

    fn workbook_path(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .join("refs")
            .join(name)
    }

    // The contour→cut-plane profile builder is pure geometry — testable without
    // refs. Two horizontal contour lines crossing a cut give a two-point
    // ascending profile at the crossing distances with the contour elevations.
    #[test]
    fn contour_profile_interpolates_a_cut_across_two_contours() {
        // Cut from (0,0) to (100,0) along +x. Two contours perpendicular to the
        // cut at x=20 (z=-0.5) and x=60 (z=-1.5).
        let contours = vec![
            Contour {
                elevation_z: -0.5,
                xy: vec![[20.0, -50.0], [20.0, 50.0]],
            },
            Contour {
                elevation_z: -1.5,
                xy: vec![[60.0, -50.0], [60.0, 50.0]],
            },
        ];
        let profile = contour_profile(&contours, [0.0, 0.0], [100.0, 0.0], 200.0, 0.0).unwrap();
        let pts = profile.points();
        assert_eq!(pts.len(), 2);
        assert_abs_diff_eq!(pts[0][0], 20.0, epsilon = 1e-9);
        assert_abs_diff_eq!(pts[0][1], -0.5, epsilon = 1e-9);
        assert_abs_diff_eq!(pts[1][0], 60.0, epsilon = 1e-9);
        assert_abs_diff_eq!(pts[1][1], -1.5, epsilon = 1e-9);
        // Strictly ascending X (validate_profile invariant).
        assert!(pts.windows(2).all(|w| w[0][0] < w[1][0]));
    }

    #[test]
    fn contour_profile_rejects_degenerate_and_undersampled_cuts() {
        let contours = vec![Contour {
            elevation_z: 0.0,
            xy: vec![[20.0, -50.0], [20.0, 50.0]],
        }];
        // Only one crossing ⇒ < 2 samples ⇒ Geometry error, never a panic.
        assert!(matches!(
            contour_profile(&contours, [0.0, 0.0], [100.0, 0.0], 200.0, 0.0),
            Err(CaseLoadError::Geometry { .. })
        ));
        // Zero-length cut is rejected.
        assert!(matches!(
            contour_profile(&contours, [0.0, 0.0], [0.0, 0.0], 200.0, 0.0),
            Err(CaseLoadError::Geometry { .. })
        ));
    }

    #[test]
    fn curved_coordinates_parse_when_refs_present() {
        let path = workbook_path("TestCurvedRoad.xls");
        if !path.is_file() {
            eprintln!("SKIP: {} not fetched — run refs/fetch.sh", path.display());
            return;
        }
        let mut wb = open_workbook::<Xls<std::io::BufReader<std::fs::File>>, _>(&path).unwrap();
        let range = wb.worksheet_range("Coordinates").unwrap();
        let coords = parse_curved_coordinates(&range, "Coordinates").unwrap();
        assert!(
            coords.centreline.len() >= 3,
            "road centre line must parse ({} pts)",
            coords.centreline.len()
        );
        assert!(
            coords.contours.len() >= 2,
            "terrain contour lines must parse ({} contours)",
            coords.contours.len()
        );
        // Every contour has at least two vertices and a finite elevation.
        for c in &coords.contours {
            assert!(c.xy.len() >= 2);
            assert!(c.elevation_z.is_finite());
        }
    }

    #[test]
    fn city_coordinates_parse_when_refs_present() {
        let path = workbook_path("TestCityStreet.xls");
        if !path.is_file() {
            eprintln!("SKIP: {} not fetched — run refs/fetch.sh", path.display());
            return;
        }
        let mut wb = open_workbook::<Xls<std::io::BufReader<std::fs::File>>, _>(&path).unwrap();
        let range = wb.worksheet_range("Coordinates").unwrap();
        let coords = parse_city_coordinates(&range, "Coordinates").unwrap();
        assert_eq!(coords.buildings.len(), 4, "four building footprints");
        assert!(coords.receivers.len() >= 4, "receiver positions must parse");
        // Building footprints are non-degenerate polygons.
        for b in &coords.buildings {
            assert!(b.len() >= 3, "a building footprint needs ≥ 3 vertices");
        }
    }

    #[test]
    fn curved_city_yearly_load_one_case_per_sheet_when_refs_present() {
        type Loader = fn(&Path) -> Result<Vec<DiscoveredCase>, CaseLoadError>;
        let workbooks: [(&str, Loader, usize); 3] = [
            ("TestCurvedRoad.xls", load_curved_road, 8),
            ("TestCityStreet.xls", load_city_street, 4),
            ("TestYearlyAverage.xls", load_yearly, 4),
        ];
        for (name, loader, min_cases) in workbooks {
            let path = workbook_path(name);
            if !path.is_file() {
                eprintln!("SKIP: {} not fetched — run refs/fetch.sh", path.display());
                continue;
            }
            let cases = loader(&path).expect("workbook must open");
            assert!(
                cases.len() >= min_cases,
                "{name}: expected ≥ {min_cases} cases, got {}",
                cases.len()
            );
            // Every case must carry a 27-band reference spectrum (never the old
            // placeholder) and load without error.
            for c in &cases {
                let def = c.case.as_ref().unwrap_or_else(|e| panic!("{}: {e}", c.id));
                let spec = def
                    .reference_spectrum
                    .as_ref()
                    .unwrap_or_else(|| panic!("{} has no reference spectrum", c.id));
                assert_eq!(spec.bands.len(), SPECTRUM_ROWS);
                assert!(spec.laeq_24h_db.is_finite());
            }
        }
    }
}
