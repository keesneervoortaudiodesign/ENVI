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
use envi_engine::propagation::air_absorption::{Atmosphere, alpha_db_per_m, band_attenuation_db};
use envi_engine::scene::BandSpectrum;

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
        use std::fmt::Write as _;

        let mut out = String::new();
        let _ = writeln!(
            out,
            "{:>4} {:>10} {:>10} {:>10} {:>8}  verdict",
            "band", "nom. Hz", "got dB", "want dB", "dev dB"
        );
        for (d, v) in self.deviations.iter().zip(&self.verdicts) {
            let verdict = match v {
                BandVerdict::Ok => "ok",
                BandVerdict::DipShiftWarning => "WARN (dip shift)",
                BandVerdict::Fail => "FAIL",
            };
            let _ = writeln!(
                out,
                "{:>4} {:>10} {:>10.3} {:>10.3} {:>+8.3}  {verdict}",
                d.band, d.nominal_hz, d.got_db, d.want_db, d.dev_db
            );
        }
        let _ = writeln!(
            out,
            "max |dev| = {:.3} dB (band tol {:.1} dB)",
            self.max_abs_dev_db, self.tol_band_db
        );
        if let Some(overall) = self.overall_dev_db {
            let _ = writeln!(
                out,
                "overall dev = {overall:+.3} dB (tol {:.1} dB)",
                self.tol_overall_db
            );
        }
        for w in &self.warnings {
            let _ = writeln!(out, "warning: {w}");
        }
        let _ = writeln!(out, "verdict: {}", if self.pass { "PASS" } else { "FAIL" });
        out
    }
}

/// Per-band comparison per the RESEARCH example: signed deviations plus a
/// simple all-bands-within-tolerance verdict.
///
/// `got` and `want` must be equal-length band-level slices (27-band FORCE
/// space); indices map to [`NOMINAL_THIRD_OCT`] labels for display.
#[must_use]
pub fn compare_spectrum(got: &[f64], want: &[f64], tol_db: f64) -> (Vec<BandDeviation>, bool) {
    debug_assert_eq!(got.len(), want.len(), "band spectra must be equal length");
    let deviations: Vec<BandDeviation> = got
        .iter()
        .zip(want)
        .enumerate()
        .map(|(i, (&g, &w))| BandDeviation {
            band: i,
            nominal_hz: NOMINAL_THIRD_OCT.get(i).copied().unwrap_or(f64::NAN),
            got_db: g,
            want_db: w,
            dev_db: g - w,
        })
        .collect();
    let pass = deviations.iter().all(|d| d.dev_db.abs() <= tol_db);
    (deviations, pass)
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
    let (deviations, _) = compare_spectrum(got, want, FORCE_TOL_BAND_DB);

    let mut warnings = Vec::new();
    let verdicts: Vec<BandVerdict> = deviations
        .iter()
        .map(|d| {
            if d.dev_db.abs() <= FORCE_TOL_BAND_DB {
                return BandVerdict::Ok;
            }
            if dip_shift_rule {
                // EP1335 §6 dip-shift allowance: does the got-spectrum
                // shifted by ±1 band index explain this band?
                let i = d.band;
                let shifted_ok = [i.checked_sub(1), i.checked_add(1)]
                    .into_iter()
                    .flatten()
                    .filter_map(|j| got.get(j))
                    .any(|&g| (g - d.want_db).abs() <= FORCE_TOL_BAND_DB);
                if shifted_ok {
                    warnings.push(format!(
                        "band {} ({} Hz): {:+.2} dB deviation covered by the \
                         ±1-band dip-shift allowance (EP1335 §6)",
                        d.band, d.nominal_hz, d.dev_db
                    ));
                    return BandVerdict::DipShiftWarning;
                }
            }
            BandVerdict::Fail
        })
        .collect();

    let max_abs_dev_db = deviations
        .iter()
        .map(|d| d.dev_db.abs())
        .fold(0.0_f64, f64::max);

    let overall_dev_db = overall.map(|(g, w)| g - w);
    let overall_ok = overall_dev_db.is_none_or(|dev| dev.abs() <= FORCE_TOL_OVERALL_DB);
    if let Some(dev) = overall_dev_db
        && dev.abs() > FORCE_INVESTIGATE_DB
        && dev.abs() <= FORCE_TOL_OVERALL_DB
    {
        warnings.push(format!(
            "overall deviation {dev:+.2} dB exceeds the {FORCE_INVESTIGATE_DB} dB \
             investigation threshold (EP1335 §6)"
        ));
    }

    let pass = overall_ok && !verdicts.contains(&BandVerdict::Fail);

    ComparisonReport {
        deviations,
        verdicts,
        max_abs_dev_db,
        overall_dev_db,
        tol_band_db: FORCE_TOL_BAND_DB,
        tol_overall_db: FORCE_TOL_OVERALL_DB,
        warnings,
        pass,
    }
}

/// Independent dB-domain free-field reference — the test oracle (threat
/// T-01-09, float/precision integrity).
///
/// Computes the expected per-band receiver level PURELY in the dB domain:
///
/// ```text
/// L_p(f) = L_W(f) + (−10·log10(4πr²)) − band_attenuation_db(α(f)·r)
/// ```
///
/// It reuses the engine's α / band-correction (anchored independently by the
/// engine unit tests) but **NOT** the engine's complex polar path — so the
/// end-to-end comparison catches wiring, normalization and magnitude/phase
/// roundtrip errors. Design: formula errors are caught by the engine unit
/// anchors; this reference catches integration errors. Returns one value per
/// 1/12-octave point ([`N_BANDS`]).
#[must_use]
pub fn analytic_freefield_reference(
    r_m: f64,
    atmos: &Atmosphere,
    spectrum: &BandSpectrum,
    axis: &FreqAxis,
) -> Vec<f64> {
    let divergence_db = -10.0 * (4.0 * std::f64::consts::PI * r_m * r_m).log10();
    axis.centres
        .iter()
        .zip(spectrum.as_slice())
        .map(|(&f, &lw_db)| {
            let a0 = alpha_db_per_m(f, atmos) * r_m;
            lw_db + divergence_db - band_attenuation_db(a0)
        })
        .collect()
}

/// Compare two equal-length pointwise spectra in the 105-point 1/12-octave
/// space at a strict analytic tolerance, producing a [`ComparisonReport`].
///
/// The `nominal_hz` column carries the EXACT grid centre (there is no
/// 1/3-octave nominal label at 1/12 resolution); no dip-shift rule applies
/// (synthetic analytic identity, not a FORCE reference). `pass` is true iff
/// every point is within `tol_db`.
#[must_use]
pub fn compare_pointwise(
    got: &[f64],
    want: &[f64],
    tol_db: f64,
    centres: &[f64],
) -> ComparisonReport {
    debug_assert_eq!(
        got.len(),
        want.len(),
        "pointwise spectra must be equal length"
    );
    let deviations: Vec<BandDeviation> = got
        .iter()
        .zip(want)
        .enumerate()
        .map(|(i, (&g, &w))| BandDeviation {
            band: i,
            nominal_hz: centres.get(i).copied().unwrap_or(f64::NAN),
            got_db: g,
            want_db: w,
            dev_db: g - w,
        })
        .collect();
    let verdicts: Vec<BandVerdict> = deviations
        .iter()
        .map(|d| {
            if d.dev_db.abs() <= tol_db {
                BandVerdict::Ok
            } else {
                BandVerdict::Fail
            }
        })
        .collect();
    let max_abs_dev_db = deviations
        .iter()
        .map(|d| d.dev_db.abs())
        .fold(0.0_f64, f64::max);
    let pass = !verdicts.contains(&BandVerdict::Fail);
    ComparisonReport {
        deviations,
        verdicts,
        max_abs_dev_db,
        overall_dev_db: None,
        tol_band_db: tol_db,
        tol_overall_db: FORCE_TOL_OVERALL_DB,
        warnings: Vec::new(),
        pass,
    }
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
    let f2 = f_hz * f_hz;
    let r_a = 12_194.0_f64.powi(2) * f2 * f2
        / ((f2 + 20.6_f64.powi(2))
            * ((f2 + 107.7_f64.powi(2)) * (f2 + 737.9_f64.powi(2))).sqrt()
            * (f2 + 12_194.0_f64.powi(2)));
    20.0 * r_a.log10() + 2.00
}

/// Seconds in 24 h — the LAeq,24h averaging denominator.
pub const SECONDS_PER_DAY: f64 = 86_400.0;

/// Sound exposure level `LAE` from a 27-band 1/3-octave pass-by exposure
/// spectrum `LE(f)`: the A-weighted energy sum over the exact `FreqAxis`
/// third-octave centres (never nominal Hz).
///
/// `LAE = 10·lg Σ_k 10^{(LE_k + A(f_k))/10}`.
#[must_use]
pub fn l_ae(le_third_octave_db: &[f64], axis: &FreqAxis) -> f64 {
    let mut energy = 0.0_f64;
    for (k, &l) in le_third_octave_db.iter().enumerate() {
        if l.is_finite() {
            energy += 10f64.powf((l + a_weighting_db(axis.third_octave_pick(k))) / 10.0);
        }
    }
    10.0 * energy.max(f64::MIN_POSITIVE).log10()
}

/// Equivalent continuous A-weighted level over 24 h from the single-event sound
/// exposure level `LAE` and the vehicle count `N`:
///
/// `LAeq,24h = LAE + 10·lg N − 10·lg 86400`.
///
/// Case-1 anchor: with the workbook's `LAE` and traffic `N`, this reproduces the
/// full-precision `LAeq,24h = 39.398…` cell (see the FORCE loader test).
#[must_use]
pub fn l_aeq_24h(l_ae_db: f64, n_vehicles: f64) -> f64 {
    l_ae_db + 10.0 * n_vehicles.max(f64::MIN_POSITIVE).log10() - 10.0 * SECONDS_PER_DAY.log10()
}

/// Maximum A-weighted pass-by level `LAmax` from the closest-approach (θ = 0)
/// 1/3-octave immission spectrum `L_max(f)` — the A-weighted overall of the
/// instantaneous spectrum at the receiver's nearest point.
#[must_use]
pub fn l_amax(lmax_third_octave_db: &[f64], axis: &FreqAxis) -> f64 {
    // Same A-weighted overall reduction as LAE (an instantaneous, not integrated,
    // spectrum — the caller supplies the θ = 0 immission spectrum).
    l_ae(lmax_third_octave_db, axis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::{assert_abs_diff_eq, assert_relative_eq};

    #[test]
    fn laeq_24h_conversion_matches_the_formula() {
        // N = 10 000 ⇒ offset = 40 − 49.3651 = −9.3651 dB.
        let lae = 48.77;
        let laeq = l_aeq_24h(lae, 10_000.0);
        assert_relative_eq!(
            laeq,
            lae + 40.0 - 10.0 * SECONDS_PER_DAY.log10(),
            epsilon = 1e-12
        );
        assert_relative_eq!(laeq - lae, -9.365_1, epsilon = 1e-3);
    }

    #[test]
    fn a_weighted_le_sums_over_exact_centres() {
        let axis = FreqAxis::new();
        // A flat 50 dB LE spectrum: LAE = 50 + 10·lg Σ 10^{A(f_k)/10}.
        let flat = vec![50.0_f64; N_THIRD_OCT];
        let got = l_ae(&flat, &axis);
        let mut e = 0.0;
        for k in 0..N_THIRD_OCT {
            e += 10f64.powf(a_weighting_db(axis.third_octave_pick(k)) / 10.0);
        }
        assert_relative_eq!(got, 50.0 + 10.0 * e.log10(), epsilon = 1e-9);
    }

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
        assert_relative_eq!(report.max_abs_dev_db, 0.9, epsilon = 1e-12);
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
        assert!(
            lenient.pass,
            "dip shift by one band is acceptable per EP1335 §6"
        );
        assert!(!lenient.warnings.is_empty());
        assert!(lenient.verdicts.contains(&BandVerdict::DipShiftWarning));
    }

    #[test]
    fn analytic_reference_is_lw_plus_divergence_minus_band_absorption() {
        let axis = FreqAxis::new();
        let atmos = Atmosphere::new(15.0, 70.0, 101.325).unwrap();
        let r = 100.0;

        // Unit spectrum: the reference at the 1000 Hz grid point is exactly
        // divergence − band-corrected absorption (independent hand assembly).
        let unit = BandSpectrum::uniform(0.0);
        let ref_unit = analytic_freefield_reference(r, &atmos, &unit, &axis);
        assert_eq!(ref_unit.len(), N_BANDS);
        let f = axis.centres[64];
        let want = -50.992_099 - band_attenuation_db(alpha_db_per_m(f, &atmos) * r);
        assert_relative_eq!(ref_unit[64], want, epsilon = 1e-6);

        // A per-band L_W adds linearly: ramp(i) shifts the reference by L_W(i).
        let ramp: [f64; N_BANDS] = std::array::from_fn(|i| 80.0 + 0.1 * i as f64);
        let ref_ramp =
            analytic_freefield_reference(r, &atmos, &BandSpectrum::from_values(ramp), &axis);
        for i in 0..N_BANDS {
            assert_relative_eq!(ref_ramp[i] - ref_unit[i], ramp[i], epsilon = 1e-9);
        }
    }

    #[test]
    fn compare_pointwise_passes_on_identity_and_flags_a_point() {
        let axis = FreqAxis::new();
        let want: Vec<f64> = (0..N_BANDS).map(|i| 60.0 + i as f64).collect();
        let report = compare_pointwise(&want, &want, 1e-9, &axis.centres);
        assert!(report.pass);
        assert_eq!(report.deviations.len(), N_BANDS);

        let mut got = want.clone();
        got[70] += 1e-6; // beyond the 1e-9 analytic tolerance
        let report = compare_pointwise(&got, &want, 1e-9, &axis.centres);
        assert!(!report.pass);
        assert!(report.max_abs_dev_db > 1e-9);
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
