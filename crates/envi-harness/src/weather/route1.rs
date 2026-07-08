//! Route 1 — meteorological-class statistics → energy-weighted `L_den`
//! (MET-05, D-07).
//!
//! The yearly-average route: a probability distribution over meteorological
//! classes (M1–M25) as a function of the propagation direction, per period
//! (day / evening / night), is read from the FORCE `TestYearlyAverage.xls`
//! `Met. statistics` sheet. Each class maps to a log-lin profile `(A, B)`; the
//! engine is run per class and the receiver levels are combined **energy-
//! weighted** into a period level, and the three periods into `L_den` with the
//! standard `+0 / +5 / +10 dB` evening/night penalties.
//!
//! # I/O quarantine (D-15)
//!
//! All of Route 1 lives in **`envi-harness`**. The engine only ever sees a
//! concrete `(A, B, C, …)` profile; it knows nothing about class tables or the
//! `.xls`.
//!
//! # ⚠️ The class → `(A, B)` mapping is `[ASSUMED]` (RESEARCH Open Q1, D-04)
//!
//! AV 1106/07 takes `(A, B, C)` as **inputs** and does not define the
//! meteorological-class → profile conversion; the `Met. statistics` sheet
//! supplies only the **probabilities**, not the profiles. The
//! [`MetClass`] `(A, B)` values are therefore `[ASSUMED]` and are **never**
//! asserted against a FORCE numeric reference — the committed tests validate
//! only the **energy-weighting combination** (`10·lg(Σ p·10^{L/10})`), which is
//! a method-defined identity, not an assumed constant. The wind/gradient FORCE
//! cases stay `Skipped(requires: emission-model)` until Phase 4 (D-03); Route 1
//! never turns one into a numeric Pass.
//!
//! Provenance caveat: the probabilities are read from the git-ignored,
//! SHA-pinned `refs/TestYearlyAverage.xls`; when `refs/` is absent the loader
//! fails **typed** (`CaseLoadError`) and the caller fail-softs to `Skipped` —
//! never a false Pass, never a panic (T-03-03-01).

use std::path::Path;

use calamine::{Data, Range, Reader, Xls, open_workbook};

use crate::cases::CaseLoadError;

/// Number of meteorological classes in the FORCE `Met. statistics` sheet.
pub const N_MET_CLASSES: usize = 25;

/// DoS guard: maximum rows scanned in the `Met. statistics` sheet.
const MAX_STAT_ROWS: u32 = 4_000;

/// One meteorological class: an `[ASSUMED]` log-lin profile `(A, B)` and its
/// occurrence probability for a given direction/period.
///
/// `probability` is a fraction in `[0, 1]` (the sheet stores percentages; the
/// loader divides by 100). `(A, B)` are the `[ASSUMED]` refraction coefficients
/// — see the module docs: they are never numerically pinned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetClass {
    /// `[ASSUMED]` log coefficient `A`, m/s.
    pub a: f64,
    /// `[ASSUMED]` linear coefficient `B`, s⁻¹.
    pub b: f64,
    /// Occurrence probability (fraction in `[0, 1]`).
    pub probability: f64,
}

/// The three `L_den` sub-periods (07–19 day, 19–23 evening, 23–07 night — the
/// EU END / Directive 2002/49/EC boundaries).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    /// Day, 07:00–19:00 (12 h, no penalty).
    Day,
    /// Evening, 19:00–23:00 (4 h, +5 dB).
    Evening,
    /// Night, 23:00–07:00 (8 h, +10 dB).
    Night,
}

/// Which day/evening/night hour split to use when combining into `L_den`.
///
/// The evening/night **penalties** (+5 / +10 dB) are fixed by the END; only the
/// hour weights differ between national implementations (Pitfall 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HourScheme {
    /// EU END / Directive 2002/49/EC default: day 12 h, evening 4 h, night 8 h.
    #[default]
    EndDefault,
    /// Danish scheme (AV 1171/06 §2.2 Table 1): day 07–19 (12 h), evening 19–22
    /// (**3 h**), night 22–07 (**9 h**). The FORCE `TestYearlyAverage` workbook
    /// is Danish, so its `L_den` MUST use this split — NOT the EU 12/4/8 default
    /// (Pitfall 4).
    Danish,
}

impl Period {
    /// The three periods in the sheet's block order (day, evening, night).
    #[must_use]
    pub const fn all() -> [Period; 3] {
        [Period::Day, Period::Evening, Period::Night]
    }

    /// The `L_den` weighting (hours, penalty dB) for this period under the EU
    /// END default (12 h day +0, 4 h evening +5, 8 h night +10). Kept as the
    /// backward-compatible default; [`Self::weighting_scheme`] parameterizes it.
    #[must_use]
    pub const fn weighting(self) -> (f64, f64) {
        self.weighting_scheme(HourScheme::EndDefault)
    }

    /// The `L_den` weighting (hours, penalty dB) for this period under `scheme`.
    /// Only the hour weights vary between schemes; the +5 / +10 dB penalties are
    /// END-fixed (Pitfall 4).
    #[must_use]
    pub const fn weighting_scheme(self, scheme: HourScheme) -> (f64, f64) {
        match (scheme, self) {
            (HourScheme::EndDefault, Period::Day) => (12.0, 0.0),
            (HourScheme::EndDefault, Period::Evening) => (4.0, 5.0),
            (HourScheme::EndDefault, Period::Night) => (8.0, 10.0),
            (HourScheme::Danish, Period::Day) => (12.0, 0.0),
            (HourScheme::Danish, Period::Evening) => (3.0, 5.0),
            (HourScheme::Danish, Period::Night) => (9.0, 10.0),
        }
    }
}

/// Energy-weighted combination of per-class receiver levels into a single
/// period level: `L = 10·lg(Σ pᵢ·10^{Lᵢ/10})` (MET-05, success criterion 4).
///
/// The probabilities are **normalized** to sum to 1 first (the sheet columns
/// are percentages that sum to ≈100 %, up to interpolation), so the result is a
/// genuine probability-weighted energy average independent of the raw scale.
///
/// # Errors
///
/// - [`CaseLoadError::NonFinite`] if any level or probability is non-finite, or
///   if the probabilities do not sum to a positive finite total (so the
///   normalization is well-defined) — a malformed table never yields a silent
///   wrong level (T-03-03-01).
/// - [`CaseLoadError::Invalid`] if `levels` and `probabilities` differ in length
///   or are empty.
pub fn energy_weighted_level(levels: &[f64], probabilities: &[f64]) -> Result<f64, CaseLoadError> {
    if levels.is_empty() || levels.len() != probabilities.len() {
        return Err(CaseLoadError::Invalid {
            context: "weather route 1".to_string(),
            message: format!(
                "levels ({}) and probabilities ({}) must be non-empty and equal length",
                levels.len(),
                probabilities.len()
            ),
        });
    }
    let mut total_p = 0.0;
    for (i, &p) in probabilities.iter().enumerate() {
        if !p.is_finite() || p < 0.0 {
            return Err(CaseLoadError::NonFinite {
                context: "weather route 1".to_string(),
                what: format!("probability[{i}]"),
            });
        }
        total_p += p;
    }
    if !(total_p.is_finite() && total_p > 0.0) {
        return Err(CaseLoadError::NonFinite {
            context: "weather route 1".to_string(),
            what: "sum of probabilities (must be finite and > 0)".to_string(),
        });
    }
    let mut energy = 0.0;
    for (i, (&l, &p)) in levels.iter().zip(probabilities).enumerate() {
        if !l.is_finite() {
            return Err(CaseLoadError::NonFinite {
                context: "weather route 1".to_string(),
                what: format!("level[{i}]"),
            });
        }
        energy += (p / total_p) * 10f64.powf(l / 10.0);
    }
    Ok(10.0 * energy.log10())
}

/// Energy-weight the per-class receiver `levels` by the [`MetClass`]
/// occurrence probabilities into a single period level (MET-05). A thin wrapper
/// over [`energy_weighted_level`] that pairs each level with its class's
/// probability — the point where the `[ASSUMED]` class `(A, B)` (which produced
/// the level) and the sheet-read probability meet.
///
/// # Errors
///
/// As [`energy_weighted_level`] (length mismatch, non-finite, zero-sum).
pub fn energy_weighted_over_classes(
    levels: &[f64],
    classes: &[MetClass],
) -> Result<f64, CaseLoadError> {
    let probs: Vec<f64> = classes.iter().map(|c| c.probability).collect();
    energy_weighted_level(levels, &probs)
}

/// Combine three period levels into `L_den` under the EU END default hour split
/// (12/4/8): `L_den = 10·lg[(12·10^{Lday/10} + 4·10^{(Leve+5)/10} +
/// 8·10^{(Lnight+10)/10}) / 24]`.
///
/// # Errors
///
/// [`CaseLoadError::NonFinite`] if any period level is non-finite.
pub fn l_den(day: f64, evening: f64, night: f64) -> Result<f64, CaseLoadError> {
    l_den_scheme(day, evening, night, HourScheme::EndDefault)
}

/// Combine three period levels into `L_den` under a chosen [`HourScheme`].
///
/// FORCE `TestYearlyAverage` is Danish ⇒ [`HourScheme::Danish`] (12/3/9), NOT
/// the EU 12/4/8 default (Pitfall 4). The +5 / +10 dB evening/night penalties
/// are END-fixed and identical across schemes; only the hour weights change.
///
/// # Errors
///
/// [`CaseLoadError::NonFinite`] if any period level is non-finite.
pub fn l_den_scheme(
    day: f64,
    evening: f64,
    night: f64,
    scheme: HourScheme,
) -> Result<f64, CaseLoadError> {
    let mut num = 0.0;
    let mut hours = 0.0;
    for (level, period) in [
        (day, Period::Day),
        (evening, Period::Evening),
        (night, Period::Night),
    ] {
        if !level.is_finite() {
            return Err(CaseLoadError::NonFinite {
                context: "weather route 1 (L_den)".to_string(),
                what: format!("{period:?} level"),
            });
        }
        let (h, penalty) = period.weighting_scheme(scheme);
        num += h * 10f64.powf((level + penalty) / 10.0);
        hours += h;
    }
    Ok(10.0 * (num / hours).log10())
}

/// Trimmed string content of a cell, if it is a string cell.
fn cell_str(range: &Range<Data>, row: u32, col: u32) -> Option<String> {
    match range.get_value((row, col)) {
        Some(Data::String(s)) => {
            let t = s.trim();
            (!t.is_empty()).then(|| t.to_string())
        }
        _ => None,
    }
}

/// Numeric content of a cell, if it is a float/int cell.
fn cell_num(range: &Range<Data>, row: u32, col: u32) -> Option<f64> {
    match range.get_value((row, col)) {
        Some(Data::Float(f)) => Some(*f),
        Some(Data::Int(i)) => Some(*i as f64),
        _ => None,
    }
}

/// Read the `Met. statistics` sheet and return the M1–M25 occurrence
/// probabilities (fractions, summing to ≈1) for a `period` at `direction_deg`,
/// linearly interpolated between the tabulated direction columns (MET-05).
///
/// The parse is **label-anchored** (like the FORCE `.xls` loader): it scans
/// column A for the three `"Direction"` header rows (day / evening / night
/// blocks, in order) and reads the 25 `M…` rows following each. Fixed row
/// offsets are never assumed — this survives layout drift.
///
/// # Errors
///
/// - [`CaseLoadError::Workbook`] if the file cannot be opened or the sheet is
///   missing (e.g. `refs/` absent → the caller fail-softs to `Skipped`).
/// - [`CaseLoadError::MissingLabel`] if a period's `Direction` header or its 25
///   class rows are not found.
/// - [`CaseLoadError::NonFinite`] if a probability cell is non-finite.
pub fn load_met_probabilities(
    xls_path: &Path,
    direction_deg: f64,
    period: Period,
) -> Result<Vec<f64>, CaseLoadError> {
    let mut wb =
        open_workbook::<Xls<std::io::BufReader<std::fs::File>>, _>(xls_path).map_err(|e| {
            CaseLoadError::Workbook {
                path: xls_path.to_path_buf(),
                message: e.to_string(),
            }
        })?;
    const SHEET: &str = "Met. statistics";
    let range = wb
        .worksheet_range(SHEET)
        .map_err(|e| CaseLoadError::Workbook {
            path: xls_path.to_path_buf(),
            message: format!("sheet {SHEET:?}: {e}"),
        })?;

    // Find the three "Direction" header rows in block order.
    let mut header_rows: Vec<u32> = Vec::new();
    let max_row = range.height().min(MAX_STAT_ROWS as usize) as u32;
    for r in 0..max_row {
        if let Some(s) = cell_str(&range, r, 0)
            && s.eq_ignore_ascii_case("Direction")
        {
            header_rows.push(r);
        }
    }
    let block = match period {
        Period::Day => 0usize,
        Period::Evening => 1,
        Period::Night => 2,
    };
    let header = *header_rows
        .get(block)
        .ok_or_else(|| CaseLoadError::MissingLabel {
            sheet: SHEET.to_string(),
            label: format!("Direction header for {period:?} (block {block})"),
        })?;

    // Direction columns (row = header, cols ≥ 1 while numeric & ascending).
    let mut dirs: Vec<(u32, f64)> = Vec::new();
    let width = range.width() as u32;
    for c in 1..width {
        match cell_num(&range, header, c) {
            Some(v) => dirs.push((c, v)),
            None => break,
        }
    }
    if dirs.len() < 2 {
        return Err(CaseLoadError::MissingLabel {
            sheet: SHEET.to_string(),
            label: format!("direction columns for {period:?}"),
        });
    }

    // Bracket the requested direction (clamped to the tabulated range).
    let d = direction_deg.clamp(dirs[0].1, dirs[dirs.len() - 1].1);
    let (mut lo, mut hi) = (dirs[0], dirs[dirs.len() - 1]);
    for w in dirs.windows(2) {
        if w[0].1 <= d && d <= w[1].1 {
            lo = w[0];
            hi = w[1];
            break;
        }
    }
    let frac = if (hi.1 - lo.1).abs() < 1e-12 {
        0.0
    } else {
        (d - lo.1) / (hi.1 - lo.1)
    };

    // The 25 class rows immediately below the header (each column-A label starts
    // with 'M'). Interpolate each class probability between the bracket columns.
    let mut probs = Vec::with_capacity(N_MET_CLASSES);
    for k in 0..N_MET_CLASSES as u32 {
        let row = header + 1 + k;
        let label = cell_str(&range, row, 0).ok_or_else(|| CaseLoadError::MissingLabel {
            sheet: SHEET.to_string(),
            label: format!("class row M{} for {period:?}", k + 1),
        })?;
        if !label.starts_with('M') && !label.starts_with('m') {
            return Err(CaseLoadError::MissingLabel {
                sheet: SHEET.to_string(),
                label: format!(
                    "expected class row M{} for {period:?}, found {label:?}",
                    k + 1
                ),
            });
        }
        let p_lo = cell_num(&range, row, lo.0).unwrap_or(0.0);
        let p_hi = cell_num(&range, row, hi.0).unwrap_or(0.0);
        let pct = p_lo + frac * (p_hi - p_lo);
        if !pct.is_finite() || pct < 0.0 {
            return Err(CaseLoadError::NonFinite {
                context: format!("weather route 1 (Met. statistics, {period:?})"),
                what: format!("probability M{}", k + 1),
            });
        }
        probs.push(pct / 100.0); // sheet stores percentages
    }
    Ok(probs)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Energy-weighting is the method-defined combination (MET-05 core). A single
    // dominant class returns its own level; equal weights give the energy mean.
    #[test]
    fn energy_weighted_level_reduces_to_the_single_dominant_class() {
        // One class with all the probability ⇒ its own level, exactly.
        let l = energy_weighted_level(&[70.0, 50.0, 40.0], &[1.0, 0.0, 0.0]).unwrap();
        assert!(
            (l - 70.0).abs() < 1e-9,
            "dominant class ⇒ its own level, got {l}"
        );
    }

    #[test]
    fn energy_weighted_level_is_the_energy_mean_for_equal_weights() {
        // Two equal-probability 60 dB and 66.0206 dB (=60 dB + 3.0103×2) sources.
        // 10·lg((10^6 + 10^6.60206)/2). Hand-check: energy mean of 60 and ~66.02.
        let l = energy_weighted_level(&[60.0, 66.0], &[0.5, 0.5]).unwrap();
        let want = 10.0 * (0.5 * 1e6 + 0.5 * 10f64.powf(6.6)).log10();
        assert!(
            (l - want).abs() < 1e-9,
            "energy mean mismatch: {l} vs {want}"
        );
        // Energy average sits above the arithmetic mean (63) — dominated by the
        // louder class.
        assert!(l > 63.0, "energy average must exceed the arithmetic mean");
    }

    // Probabilities need not be pre-normalized: scaling all by a constant leaves
    // the energy-weighted level unchanged (the loader passes percentages).
    #[test]
    fn energy_weighted_level_is_scale_invariant_in_the_probabilities() {
        let a = energy_weighted_level(&[70.0, 55.0], &[0.3, 0.7]).unwrap();
        let b = energy_weighted_level(&[70.0, 55.0], &[30.0, 70.0]).unwrap();
        assert!(
            (a - b).abs() < 1e-9,
            "probability scale must not matter: {a} vs {b}"
        );
    }

    // The MetClass wrapper pairs each level with its class probability and
    // reduces to the same energy-weighted combination.
    #[test]
    fn energy_weighted_over_classes_uses_class_probabilities() {
        let classes = [
            MetClass {
                a: 1.0,
                b: 0.05,
                probability: 0.25,
            },
            MetClass {
                a: 0.0,
                b: 0.0,
                probability: 0.75,
            },
        ];
        let levels = [70.0, 55.0];
        let via_classes = energy_weighted_over_classes(&levels, &classes).unwrap();
        let direct = energy_weighted_level(&levels, &[0.25, 0.75]).unwrap();
        assert!((via_classes - direct).abs() < 1e-12);
    }

    // Malformed inputs are typed errors, never panics or silent wrong levels.
    #[test]
    fn energy_weighted_level_rejects_bad_inputs() {
        assert!(matches!(
            energy_weighted_level(&[70.0], &[1.0, 0.0]),
            Err(CaseLoadError::Invalid { .. })
        ));
        assert!(matches!(
            energy_weighted_level(&[f64::NAN], &[1.0]),
            Err(CaseLoadError::NonFinite { .. })
        ));
        assert!(matches!(
            energy_weighted_level(&[70.0, 60.0], &[f64::INFINITY, 0.0]),
            Err(CaseLoadError::NonFinite { .. })
        ));
        // All-zero probabilities ⇒ no valid normalization.
        assert!(matches!(
            energy_weighted_level(&[70.0, 60.0], &[0.0, 0.0]),
            Err(CaseLoadError::NonFinite { .. })
        ));
    }

    // L_den penalties: with equal period levels, L_den > L (night +10 dominates).
    #[test]
    fn l_den_applies_evening_and_night_penalties() {
        let flat = l_den(60.0, 60.0, 60.0).unwrap();
        // Hand value: 10·lg[(12·10^6 + 4·10^6.5 + 8·10^7)/24].
        let want =
            10.0 * ((12.0 * 1e6 + 4.0 * 10f64.powf(6.5) + 8.0 * 10f64.powf(7.0)) / 24.0).log10();
        assert!(
            (flat - want).abs() < 1e-9,
            "L_den mismatch: {flat} vs {want}"
        );
        assert!(flat > 60.0, "penalties must raise L_den above a flat 60 dB");
        // A louder night raises L_den more than the same increment by day.
        let noisy_night = l_den(60.0, 60.0, 70.0).unwrap();
        let noisy_day = l_den(70.0, 60.0, 60.0).unwrap();
        assert!(
            noisy_night > noisy_day,
            "the +10 dB night penalty must weight night energy more heavily"
        );
    }

    // Pitfall 4: the Danish 12/3/9 hour split must give a DIFFERENT L_den from
    // the EU 12/4/8 default whenever the evening/night levels differ (FORCE
    // TestYearlyAverage is Danish). A regression guarding against a silent revert
    // to the EU default.
    #[test]
    fn danish_hours_differ_from_the_eu_default_l_den() {
        // Distinct day/evening/night levels so the 4h→3h evening and 8h→9h night
        // reweighting actually moves the combined level.
        let (day, eve, night) = (60.0, 65.0, 55.0);
        let eu = l_den_scheme(day, eve, night, HourScheme::EndDefault).unwrap();
        let dk = l_den_scheme(day, eve, night, HourScheme::Danish).unwrap();
        assert!(
            (eu - dk).abs() > 1e-3,
            "Danish 12/3/9 must differ from EU 12/4/8: eu={eu} dk={dk}"
        );
        // The default entry point is the EU split (backward compatible).
        assert!((l_den(day, eve, night).unwrap() - eu).abs() < 1e-12);
        // Hand value for the Danish split (12/3/9, +0/+5/+10):
        let want_dk = 10.0
            * ((12.0 * 10f64.powf(day / 10.0)
                + 3.0 * 10f64.powf((eve + 5.0) / 10.0)
                + 9.0 * 10f64.powf((night + 10.0) / 10.0))
                / 24.0)
                .log10();
        assert!(
            (dk - want_dk).abs() < 1e-9,
            "Danish L_den: {dk} vs {want_dk}"
        );
        // The Danish weights sum to 24 h too.
        let (hd, _) = Period::Day.weighting_scheme(HourScheme::Danish);
        let (he, _) = Period::Evening.weighting_scheme(HourScheme::Danish);
        let (hn, _) = Period::Night.weighting_scheme(HourScheme::Danish);
        assert_eq!(hd + he + hn, 24.0);
    }

    #[test]
    fn l_den_rejects_non_finite_periods() {
        assert!(matches!(
            l_den(f64::NAN, 60.0, 60.0),
            Err(CaseLoadError::NonFinite { .. })
        ));
    }

    // Structural, fail-soft read of the (git-ignored, SHA-pinned) FORCE workbook:
    // when present, every period's 25 class probabilities are finite, non-negative
    // and sum to ≈1 after the /100. When absent the loader returns a typed error
    // (never a panic) — the honest-green fail-soft posture (D-03).
    #[test]
    fn met_statistics_probabilities_are_a_valid_distribution_when_refs_present() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .join("refs")
            .join("TestYearlyAverage.xls");
        if !path.exists() {
            eprintln!("SKIP: refs/TestYearlyAverage.xls absent (fail-soft, D-03)");
            return;
        }
        for period in Period::all() {
            let probs = load_met_probabilities(&path, 45.0, period)
                .expect("Met. statistics must parse when refs present");
            assert_eq!(probs.len(), N_MET_CLASSES);
            assert!(probs.iter().all(|p| p.is_finite() && *p >= 0.0));
            let sum: f64 = probs.iter().sum();
            assert!(
                (sum - 1.0).abs() < 0.02,
                "{period:?} class probabilities must sum to ≈1, got {sum}"
            );
        }
    }
}
