//! Reference comparison: per-band signed deviations against a 27-band FORCE
//! spectrum, with the FORCE tolerances and the ground-dip shift allowance.
//!
//! Tolerances (Env. Project 1335 §6): overall A-weighted levels within
//! **1 dB** (deviations > 0.5 dB flagged for investigation); per 1/3-octave
//! band within **1 dB**; a shift of a ground-interference dip by one
//! 1/3-octave band is acceptable (dip-shift rule, applied to
//! `ForceStraightRoad` cases only).
//!
//! Band mapping between the engine's 105-point 1/12-octave grid and the
//! 27-band FORCE space goes through `FreqAxis::third_octave_pick` — by
//! INDEX, never by float equality on nominal frequencies (Pitfall 3).

use envi_engine::freq::{FreqAxis, N_BANDS, N_THIRD_OCT, NOMINAL_THIRD_OCT};

/// FORCE tolerance for overall A-weighted levels (LAeq,24h / LAE / LAmax):
/// ≤ 1 dB deviation. Source: Env. Project 1335 §6.
pub const FORCE_TOL_OVERALL_DB: f64 = 1.0;

/// FORCE tolerance per 1/3-octave band: ≤ 1 dB deviation.
/// Source: Env. Project 1335 §6.
pub const FORCE_TOL_BAND_DB: f64 = 1.0;

/// Deviation threshold above which a passing overall level is still flagged
/// for investigation. Source: Env. Project 1335 §6.
pub const FORCE_INVESTIGATE_DB: f64 = 0.5;

/// One band's signed deviation against the reference.
#[derive(Debug, Clone, Copy)]
pub struct BandDeviation {
    /// 27-band index (0 = nominal 25 Hz … 26 = 10 kHz).
    pub band: usize,
    /// Nominal band label, display only.
    pub nominal_hz: f64,
    /// Computed level, dB.
    pub got_db: f64,
    /// Reference level, dB.
    pub want_db: f64,
    /// Signed deviation `got − want`, dB.
    pub dev_db: f64,
}

/// How a band comparison resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandVerdict {
    /// Within tolerance.
    Ok,
    /// Outside tolerance, but the ±1-band dip-shift rule covers it.
    DipShiftWarning,
    /// Outside tolerance.
    Fail,
}

/// Full comparison result for one case.
#[derive(Debug, Clone)]
pub struct ComparisonReport {
    /// Per-band signed deviations (27 entries for FORCE comparisons).
    pub deviations: Vec<BandDeviation>,
    /// Per-band verdicts, parallel to `deviations`.
    pub verdicts: Vec<BandVerdict>,
    /// Maximum |deviation| over all bands, dB.
    pub max_abs_dev_db: f64,
    /// Overall-level signed deviation (got − want), if overalls were given.
    pub overall_dev_db: Option<f64>,
    /// Band tolerance used, dB.
    pub tol_band_db: f64,
    /// Overall tolerance used, dB.
    pub tol_overall_db: f64,
    /// Human-readable warnings (dip shifts, investigation flags).
    pub warnings: Vec<String>,
    /// Verdict: no band failures and overall within tolerance.
    pub pass: bool,
}

impl ComparisonReport {
    /// Render a human-readable per-band table (used by both the failing-test
    /// error path and the CLI report).
    #[must_use]
    pub fn render_table(&self) -> String {
        todo!("Task 3 GREEN: per-band deviation table")
    }
}

/// Per-band comparison per the RESEARCH example: signed deviations plus a
/// simple all-bands-within-tolerance verdict.
///
/// `got` and `want` must be equal-length band-level slices (27-band FORCE
/// space); indices map to [`NOMINAL_THIRD_OCT`] labels for display.
#[must_use]
pub fn compare_spectrum(got: &[f64], want: &[f64], tol_db: f64) -> (Vec<BandDeviation>, bool) {
    let _ = (got, want, tol_db);
    todo!("Task 3 GREEN: per-band signed deviations")
}

/// Full FORCE-style comparison: per-band deviations with the dip-shift
/// allowance and an optional overall-level check.
///
/// `overall` is `(got, want)` for the overall A-weighted level.
/// `dip_shift_rule` enables the Env. Project 1335 §6 allowance: a band
/// failure is downgraded to a warning if shifting the got-spectrum by ±1
/// band index brings that band within tolerance (interference dips move
/// slightly in frequency without invalidating the model). Default ON for
/// `ForceStraightRoad` cases only.
#[must_use]
pub fn compare_27_band(
    got: &[f64],
    want: &[f64],
    overall: Option<(f64, f64)>,
    dip_shift_rule: bool,
) -> ComparisonReport {
    let _ = (got, want, overall, dip_shift_rule);
    todo!("Task 3 GREEN: full comparison report")
}

/// Pick the 27 exact-1/3-octave band values out of a 105-point 1/12-octave
/// spectrum — by index (every 4th point), the same rule as
/// [`FreqAxis::third_octave_pick`]. Under Nord2000's "band level = value at
/// the centre frequency" semantics this pick IS the band value.
#[must_use]
pub fn pick_third_octave(levels_105: &[f64; N_BANDS]) -> [f64; N_THIRD_OCT] {
    std::array::from_fn(|i| levels_105[i * 4])
}

/// The 27 exact 1/3-octave centre frequencies from the engine axis
/// (index-picked via [`FreqAxis::third_octave_pick`]).
#[must_use]
pub fn exact_third_octave_centres(axis: &FreqAxis) -> [f64; N_THIRD_OCT] {
    std::array::from_fn(|i| axis.third_octave_pick(i))
}

/// IEC 61672-1 A-weighting, dB.
///
/// `A(f) = 20·log10(R_A(f)) + 2.00` with
/// `R_A(f) = 12194²·f⁴ / ((f²+20.6²)·√((f²+107.7²)·(f²+737.9²))·(f²+12194²))`.
///
/// Anchors: `A(1000) ≈ 0.0001`, `A(100) = −19.145`, `A(8000) = −1.147`.
/// Needed by the comparator to form LAeq-style totals from band spectra
/// (Phase 4 gate; implemented now, anchored by tests).
#[must_use]
pub fn a_weighting_db(f_hz: f64) -> f64 {
    let _ = f_hz;
    todo!("Task 3 GREEN: IEC 61672-1 A-weighting")
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::{assert_abs_diff_eq, assert_relative_eq};

    #[test]
    fn spectrum_compared_against_itself_passes_with_zero_deviation() {
        let levels: Vec<f64> = (0..N_THIRD_OCT).map(|i| 30.0 + i as f64).collect();
        let (devs, pass) = compare_spectrum(&levels, &levels, FORCE_TOL_BAND_DB);
        assert!(pass);
        assert_eq!(devs.len(), N_THIRD_OCT);
        for d in &devs {
            assert_abs_diff_eq!(d.dev_db, 0.0);
        }
        // nominal labels attached for display
        assert_abs_diff_eq!(devs[16].nominal_hz, 1000.0);
    }

    #[test]
    fn a_band_shifted_by_1_5_db_fails_with_that_deviation() {
        let want: Vec<f64> = (0..N_THIRD_OCT).map(|i| 30.0 + i as f64).collect();
        let mut got = want.clone();
        got[7] += 1.5;
        let (devs, pass) = compare_spectrum(&got, &want, FORCE_TOL_BAND_DB);
        assert!(!pass);
        assert_abs_diff_eq!(devs[7].dev_db, 1.5);
    }

    #[test]
    fn a_weighting_anchors() {
        assert!(a_weighting_db(1000.0).abs() < 0.001);
        assert_abs_diff_eq!(a_weighting_db(100.0), -19.145, epsilon = 0.01);
        assert_abs_diff_eq!(a_weighting_db(8000.0), -1.147, epsilon = 0.01);
    }

    #[test]
    fn report_carries_max_deviation_and_overall() {
        let want = vec![30.0; N_THIRD_OCT];
        let mut got = want.clone();
        got[3] += 0.4;
        got[20] -= 0.9;
        let report = compare_27_band(&got, &want, Some((40.2, 40.0)), false);
        assert!(report.pass);
        assert_relative_eq!(report.max_abs_dev_db, 0.9);
        assert_relative_eq!(report.overall_dev_db.unwrap(), 0.2, epsilon = 1e-12);
        let table = report.render_table();
        assert!(table.contains("1000"), "table lists nominal bands: {table}");
    }

    #[test]
    fn overall_deviation_beyond_1_db_fails() {
        let want = vec![30.0; N_THIRD_OCT];
        let report = compare_27_band(&want, &want, Some((41.5, 40.0)), false);
        assert!(!report.pass);
    }

    #[test]
    fn dip_shift_rule_downgrades_a_shifted_dip_to_a_warning() {
        // Reference has an interference dip at band 10; ours landed at 11.
        let mut want = vec![30.0; N_THIRD_OCT];
        want[10] = 22.0;
        let mut got = vec![30.0; N_THIRD_OCT];
        got[11] = 22.0;
        // Without the rule: bands 10 and 11 both deviate by 8 dB -> fail.
        let strict = compare_27_band(&got, &want, None, false);
        assert!(!strict.pass);
        // With the rule: shifting by ±1 band explains both -> warnings, pass.
        let lenient = compare_27_band(&got, &want, None, true);
        assert!(lenient.pass, "dip shift by one band is acceptable per EP1335 §6");
        assert!(!lenient.warnings.is_empty());
        assert!(lenient.verdicts.contains(&BandVerdict::DipShiftWarning));
    }

    #[test]
    fn third_octave_pick_maps_105_to_27_by_index() {
        let axis = FreqAxis::new();
        let levels: [f64; N_BANDS] = std::array::from_fn(|i| i as f64);
        let picked = pick_third_octave(&levels);
        assert_abs_diff_eq!(picked[0], 0.0);
        assert_abs_diff_eq!(picked[16], 64.0);
        assert_abs_diff_eq!(picked[26], 104.0);
        let centres = exact_third_octave_centres(&axis);
        assert_relative_eq!(centres[16], 1000.0, max_relative = 1e-12);
        assert_relative_eq!(centres[26], 10_000.0, max_relative = 1e-12);
        assert_eq!(NOMINAL_THIRD_OCT.len(), centres.len());
    }
}
