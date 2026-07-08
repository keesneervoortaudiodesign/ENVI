//! Equivalent-linear sound-speed collapse CalcEqSSP (AV 1106/07 §5.5.2,
//! Eqs. 15–21 + Annex F Eq. 403).
//!
//! # Convention
//!
//! Nord2000-native (time e^{−jωt}); real-valued. Collapses the log-lin profile
//! `c(z)=A·ln(z/z₀+1)+B·z+C` to an equivalent-linear profile `cₑ(z)=c₀(1+ξz)`
//! (Eqs. 15–16) parameterised by the relative gradient `ξ=(∂c/∂z)/c₀` (Eq. 17)
//! and the ground sound speed `c₀`. The average gradient (Eq. 18) is taken
//! between `hS` and `hR`; the average sound speed `c̄` (Eq. 19) uses the Annex F
//! closed form (Eq. 403) — **never** numerical integration.
//!
//! # Deviation from a bare-tuple interface
//!
//! [`calc_eq_ssp`] returns `Result` (not `(f64, f64)`): `A/B/C/z₀/hS/hR` cross
//! from untrusted weather-route data into the ray numerics, so a non-finite /
//! non-physical input is rejected with a typed [`PropagationError`] rather than
//! producing a NaN `ξ` that would poison every downstream ray (threat
//! T-03-01-02).

use std::f64::consts::PI;

use super::profile::{Z0_MIN_M, sound_speed_profile};
use crate::propagation::PropagationError;
use crate::propagation::terrain_effect::submodel2::phase_diff_freq;

/// The homogeneous-shortcut threshold (AV 1106/07 §5.5.2 p.24): when `|ξ| <
/// 1e-6`, the profile is treated as homogeneous (`ξ=0`, `c₀=C`). This is the
/// **D-02 bit-for-bit anchor threshold** — distinct from the DirectRay inner
/// division guard `|ξ|<1e-10` (RESEARCH Pitfall 1).
pub const XI_HOMOGENEOUS: f64 = 1e-6;

/// Soft-ground flow-resistivity threshold (AV 1106/07 §5.5.3 p.25): a surface
/// with `σ < 10⁷ Pa·s·m⁻²` is **soft** and uses the frequency-dependent
/// [`calc_eq_ssp_ground`]; `σ ≥ 10⁷` is **hard** and uses the
/// frequency-independent [`calc_eq_ssp`] (RESEARCH Pitfall 4). The engine's σ
/// is in **kPa·s·m⁻²** (see [`crate::propagation::ground::ground_impedance`]),
/// so the threshold is `10⁴ kPa·s·m⁻²`.
pub const SOFT_GROUND_SIGMA_KPA: f64 = 1.0e4;

/// Antiderivative `L(h) = (h+z₀)·ln(h/z₀+1) − h` of `A·ln(z/z₀+1)` — the
/// Annex F Eq. 403 logarithmic term. `c̄`'s log part is `A·(L(hR)−L(hS))/(hR−hS)`.
#[inline]
fn antideriv_log(h: f64, z0: f64) -> f64 {
    (h + z0) * (h / z0 + 1.0).ln() - h
}

/// CalcEqSSP: collapse the log-lin profile to `(ξ, c₀)` (Eqs. 15–21).
///
/// Returns `(ξ, c₀)`. When `|ξ| < 1e-6` returns `(0.0, C)` **exactly** (the
/// homogeneous shortcut, D-02 anchor — below it the straight-ray path runs
/// unchanged). `hS = hR` uses the ±0.005 m modified-height gradient (Eq. 18);
/// `hS`/`hR` are floored at `hmin = 5·z₀` when applying Eqs. 18–20.
///
/// # Errors
///
/// [`PropagationError::DegenerateProfile`] on a non-finite input, a negative
/// height, a non-positive ground sound speed `C`, or a non-positive/non-finite
/// equivalent `c₀`.
pub fn calc_eq_ssp(
    h_s: f64,
    h_r: f64,
    z0: f64,
    a: f64,
    b: f64,
    c: f64,
) -> Result<(f64, f64), PropagationError> {
    let terms = eq_ssp_terms(h_s, h_r, z0, a, b, c)?;
    collapse_gradient(terms.dcdz, &terms, c)
}

/// Geometry-dependent intermediates of the CalcEqSSP collapse: the average
/// gradient `∂c/∂z` (Eq. 18), the average sound speed `c̄` (Eq. 19 / Annex F
/// Eq. 403), and the `hmin`-floored heights used for both. Split out so
/// [`calc_eq_ssp_ground`] can reuse `c̄`/heights while overriding the gradient
/// (Eq. 23), keeping the two entry points bit-identical in their trivial limit.
struct EqSspTerms {
    /// Average gradient `∂c/∂z` between `hS` and `hR` (Eq. 18).
    dcdz: f64,
    /// Average sound speed `c̄` (Eq. 19).
    c_bar: f64,
    /// `hmin`-floored (or ±0.005-modified) source height.
    hs_g: f64,
    /// `hmin`-floored (or ±0.005-modified) receiver height.
    hr_g: f64,
}

/// Compute the [`EqSspTerms`] (Eqs. 18–19) after validating the profile inputs.
///
/// # Errors
///
/// [`PropagationError::DegenerateProfile`] on a non-finite input, a negative
/// height, or a non-positive ground sound speed `C`.
fn eq_ssp_terms(
    h_s: f64,
    h_r: f64,
    z0: f64,
    a: f64,
    b: f64,
    c: f64,
) -> Result<EqSspTerms, PropagationError> {
    // Input guards (untrusted weather-route data → typed error, never NaN).
    if ![h_s, h_r, z0, a, b, c].iter().all(|v| v.is_finite()) {
        return Err(PropagationError::DegenerateProfile {
            detail: "non-finite profile input",
        });
    }
    if !(h_s >= 0.0 && h_r >= 0.0) {
        return Err(PropagationError::DegenerateProfile {
            detail: "source/receiver heights must be non-negative",
        });
    }
    if !(c.is_finite() && c > 0.0) {
        return Err(PropagationError::DegenerateProfile {
            detail: "ground sound speed C must be positive and finite",
        });
    }
    let z0 = z0.max(Z0_MIN_M);
    let hmin = 5.0 * z0;

    // hS = hR → the gradient at the height, via the modified heights
    // hS − 0.005 m and hR + 0.005 m (Eq. 18); otherwise the raw heights,
    // floored at hmin (§5.5.2 "hS or hR shall not be less than hmin").
    let (hs_g, hr_g) = if (h_r - h_s).abs() < 1e-9 {
        ((h_s - 0.005).max(hmin), (h_r + 0.005).max(hmin + 0.01))
    } else {
        (h_s.max(hmin), h_r.max(hmin))
    };
    // If flooring collapsed the two heights, re-open with the ±0.005 fallback.
    let (hs_g, hr_g) = if (hr_g - hs_g).abs() < 1e-12 {
        (hs_g - 0.005, hr_g + 0.005)
    } else {
        (hs_g, hr_g)
    };

    // Eq. 2 profile c(z); reuse the single-source `sound_speed_profile` (z₀ is
    // already floored to Z0_MIN_M above, so its internal clamp is a no-op here).
    let c_of = |z: f64| sound_speed_profile(z, a, b, c, z0);
    // Eq. 18: average gradient between hS and hR.
    let dcdz = (c_of(hr_g) - c_of(hs_g)) / (hr_g - hs_g);
    // Eq. 19 / Annex F Eq. 403: average sound speed c̄ (closed form).
    let c_bar = a * (antideriv_log(hr_g, z0) - antideriv_log(hs_g, z0)) / (hr_g - hs_g)
        + b * (hr_g + hs_g) / 2.0
        + c;
    Ok(EqSspTerms {
        dcdz,
        c_bar,
        hs_g,
        hr_g,
    })
}

/// Collapse a (possibly frequency-modified) gradient `dcdz` to `(ξ, c₀)` via
/// Eqs. 17 + 20, applying the `|ξ| < 1e-6` homogeneous shortcut. `c̄` and the
/// heights come from [`EqSspTerms`]; only `dcdz` varies between CalcEqSSP and
/// CalcEqSSPGround.
///
/// # Errors
///
/// [`PropagationError::DegenerateProfile`] on a non-positive/non-finite `c₀` or
/// non-finite `ξ`.
fn collapse_gradient(
    dcdz: f64,
    terms: &EqSspTerms,
    c: f64,
) -> Result<(f64, f64), PropagationError> {
    // Eq. 20: c₀ = c̄ − (∂c/∂z)·(hS+hR)/2.
    let c0 = terms.c_bar - dcdz * (terms.hs_g + terms.hr_g) / 2.0;
    if !(c0.is_finite() && c0 > 0.0) {
        return Err(PropagationError::DegenerateProfile {
            detail: "equivalent c₀ non-positive or non-finite",
        });
    }
    // Eq. 17: ξ = (∂c/∂z)/c₀.
    let xi = dcdz / c0;
    if !xi.is_finite() {
        return Err(PropagationError::DegenerateProfile {
            detail: "non-finite ξ",
        });
    }
    // Homogeneous shortcut (p.24): |ξ| < 1e-6 ⇒ (0, C) exactly.
    if xi.abs() < XI_HOMOGENEOUS {
        return Ok((0.0, c));
    }
    Ok((xi, c0))
}

/// CalcEqSSPGround: the **frequency-dependent** soft-ground collapse (AV
/// 1106/07 §5.5.3, Eqs. 22–28). Returns `(ξ(f), c₀(f))`.
///
/// Over a **soft** surface (`σ < 10⁷ Pa·s·m⁻²`, i.e. `sigma_kpa <
/// [SOFT_GROUND_SIGMA_KPA]`) the sound-speed gradient is frequency-dependent:
/// above `fH` it is the plain [`calc_eq_ssp`] gradient, below `fL` it is zero
/// (`ξ = 0`), and between `fL`/`fH` it is **log-interpolated** (Eq. 23) — so
/// `ξ(f)` varies monotonically with **band index** on the 105-point grid (D-14).
/// Over **hard** ground (`σ ≥ 10⁷`) this delegates to the frequency-independent
/// [`calc_eq_ssp`] (RESEARCH Pitfall 4) — `ξ(f)` is flat across all bands.
///
/// `fL`/`fH` come from the existing [`phase_diff_freq`] aux (Eqs. 24–27) at
/// phases `Ψ = π` and `2Ψ = 2π`, with the Eq. 24/25 clamps `d ≤ 400 m` and
/// `hS, hR ≥ 0.5 m`. `Ẑ_G(f)` is derived from `sigma_kpa` inside
/// `phase_diff_freq` (exactly as Sub-model 2 does) — hence this takes `σ`
/// directly rather than a precomputed complex impedance (interface deviation
/// from the plan's `Ẑ_G` argument: `phase_diff_freq` owns the impedance and
/// needs `σ` to sweep it over the 1/3-octave bracket).
///
/// # Errors
///
/// [`PropagationError::DegenerateProfile`] on a non-finite/non-positive `f_hz`
/// or `d`, or a degenerate profile; [`PropagationError::InvalidFlowResistivity`]
/// on a non-positive/non-finite `σ`.
#[allow(clippy::too_many_arguments)]
pub fn calc_eq_ssp_ground(
    f_hz: f64,
    d: f64,
    h_s: f64,
    h_r: f64,
    sigma_kpa: f64,
    z0: f64,
    a: f64,
    b: f64,
    c: f64,
) -> Result<(f64, f64), PropagationError> {
    if !(f_hz.is_finite() && f_hz > 0.0) {
        return Err(PropagationError::DegenerateProfile {
            detail: "CalcEqSSPGround requires a positive finite frequency",
        });
    }
    if !(d.is_finite() && d > 0.0) {
        return Err(PropagationError::DegenerateProfile {
            detail: "CalcEqSSPGround requires a positive finite distance",
        });
    }
    if !(sigma_kpa.is_finite() && sigma_kpa > 0.0) {
        return Err(PropagationError::InvalidFlowResistivity { sigma_kpa });
    }

    // Hard ground: frequency-independent CalcEqSSP (Pitfall 4 — do NOT run the
    // fL/fH machinery for σ ≥ 10⁷ Pa·s·m⁻²).
    if sigma_kpa >= SOFT_GROUND_SIGMA_KPA {
        return calc_eq_ssp(h_s, h_r, z0, a, b, c);
    }

    // Soft ground: reuse c̄/heights, override only the gradient (Eq. 23).
    let terms = eq_ssp_terms(h_s, h_r, z0, a, b, c)?;

    // fL/fH via PhaseDiffFreq at Ψ = π and 2Ψ = 2π (Eqs. 24–27). Eq. 24/25
    // clamps: d ≤ 400 m and hS, hR ≥ 0.5 m. c0 is the ground sound speed C.
    let d_c = d.min(400.0);
    let hs_c = h_s.max(0.5);
    let hr_c = h_r.max(0.5);
    let f_psi = phase_diff_freq(d_c, hs_c, hr_c, sigma_kpa, c, PI);
    let f_2psi = phase_diff_freq(d_c, hs_c, hr_c, sigma_kpa, c, 2.0 * PI);

    // Δc₁₀ = c(10) − c(0) (Eq. 26); C cancels, so this is A·ln(10/z₀+1) + 10·B.
    let dc10 = sound_speed_profile(10.0, a, b, c, z0) - sound_speed_profile(0.0, a, b, c, z0);
    // Eq. 26 fL piecewise in Δc₁₀ (page-image verified): the (43 − 3·Δc₁₀)/40
    // middle branch is C⁰-continuous at both knots — factor = 1 at Δc₁₀ = 1 and
    // factor = 0.7 at Δc₁₀ = 5 — confirming the transcribed sign/constants.
    let f_l_factor = if dc10 <= 1.0 {
        1.0
    } else if dc10 >= 5.0 {
        0.7
    } else {
        (43.0 - 3.0 * dc10) / 40.0
    };
    let f_l = f_l_factor * f_psi;
    let f_h = f_2psi.max(1.25 * f_l); // Eq. 27: fH = max(f2Ψ, 1.25·fL) ⇒ fH > fL.

    // Eq. 23 modified gradient: 0 below fL, full above fH, log-interpolated
    // between (evaluated natively at f_hz, i.e. by band index — D-14).
    let dcdz_eff = if f_hz >= f_h {
        terms.dcdz
    } else if f_hz <= f_l {
        0.0
    } else {
        let k = (f_hz.ln() - f_l.ln()) / (f_h.ln() - f_l.ln());
        k * terms.dcdz
    };

    collapse_gradient(dcdz_eff, &terms, c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::propagation::sound_speed_ms;
    use approx::assert_relative_eq;

    // Homogeneous shortcut: A=B=0 ⇒ ξ=0 and c₀=C exactly (D-02 anchor trigger).
    #[test]
    fn homogeneous_profile_returns_zero_xi_and_c() {
        let c = sound_speed_ms(15.0);
        let (xi, c0) = calc_eq_ssp(0.5, 1.5, 0.001, 0.0, 0.0, c).unwrap();
        assert_eq!(xi, 0.0);
        assert_eq!(c0, c);
    }

    // A tiny gradient below the 1e-6 threshold still collapses to (0, C).
    #[test]
    fn near_homogeneous_below_threshold_snaps_to_zero() {
        let c = sound_speed_ms(15.0);
        // A microscopic linear gradient → |ξ| < 1e-6.
        let (xi, c0) = calc_eq_ssp(0.5, 1.5, 0.001, 0.0, 1e-5, c).unwrap();
        assert_eq!(xi, 0.0);
        assert_eq!(c0, c);
    }

    // A downward-refraction profile (B>0) yields ξ>0 and a physical c₀≈C.
    #[test]
    fn downward_profile_gives_positive_xi() {
        let c = sound_speed_ms(15.0);
        let (xi, c0) = calc_eq_ssp(0.5, 5.0, 0.001, 0.5, 0.05, c).unwrap();
        assert!(xi > 0.0, "downward refraction ξ must be positive, got {xi}");
        assert!((c0 - c).abs() < 5.0, "c₀ {c0} should be near C {c}");
    }

    // ∂c/∂z is the AVERAGE gradient between hS and hR (Eq. 18): ξ·c₀ equals
    // (c(hR)−c(hS))/(hR−hS) computed independently.
    #[test]
    fn xi_matches_average_gradient_definition() {
        let c = sound_speed_ms(15.0);
        let (a, b, z0, h_s, h_r) = (1.2, 0.03, 0.02, 1.0, 4.0);
        let (xi, c0) = calc_eq_ssp(h_s, h_r, z0, a, b, c).unwrap();
        let c_of = |z: f64| a * (z / z0 + 1.0).ln() + b * z + c;
        let dcdz = (c_of(h_r) - c_of(h_s)) / (h_r - h_s);
        assert_relative_eq!(xi * c0, dcdz, max_relative = 1e-9);
    }

    // hS == hR uses the ±0.005 m modified-height gradient, never divide-by-zero.
    #[test]
    fn equal_heights_use_modified_gradient() {
        let c = sound_speed_ms(15.0);
        let (xi, c0) = calc_eq_ssp(2.0, 2.0, 0.01, 1.0, 0.02, c).unwrap();
        assert!(xi.is_finite() && c0.is_finite() && c0 > 0.0);
    }

    #[test]
    fn non_finite_and_bad_inputs_are_typed_errors() {
        let c = sound_speed_ms(15.0);
        assert!(matches!(
            calc_eq_ssp(f64::NAN, 1.5, 0.01, 1.0, 0.0, c),
            Err(PropagationError::DegenerateProfile { .. })
        ));
        assert!(matches!(
            calc_eq_ssp(0.5, -1.5, 0.01, 1.0, 0.0, c),
            Err(PropagationError::DegenerateProfile { .. })
        ));
        assert!(matches!(
            calc_eq_ssp(0.5, 1.5, 0.01, 1.0, 0.0, -1.0),
            Err(PropagationError::DegenerateProfile { .. })
        ));
    }

    // ----- CalcEqSSPGround (Eqs. 22–28) -----

    // Representative soft-ground grassland geometry (FORCE straight-road-like).
    const SOFT_SIGMA_KPA: f64 = 200.0; // < 1e4 ⇒ soft
    const HARD_SIGMA_KPA: f64 = 20000.0; // ≥ 1e4 ⇒ hard (asphalt-class)

    // Hard ground: ξ(f) is FLAT across every band and equals plain CalcEqSSP
    // (Pitfall 4 — the fL/fH machinery must not run for σ ≥ 1e7 Pa·s·m⁻²).
    #[test]
    fn hard_ground_is_frequency_independent() {
        let c = sound_speed_ms(15.0);
        let (a, b, z0, h_s, h_r, d) = (0.6, 0.05, 0.001, 0.5, 1.5, 97.5);
        let axis = crate::freq::FreqAxis::new();
        let want = calc_eq_ssp(h_s, h_r, z0, a, b, c).unwrap();
        for &f in axis.centres.iter() {
            let got = calc_eq_ssp_ground(f, d, h_s, h_r, HARD_SIGMA_KPA, z0, a, b, c).unwrap();
            assert_eq!(got, want, "hard-ground ξ(f) must be flat at {f} Hz");
        }
    }

    // Soft ground: ξ(f) is 0 below fL, the full CalcEqSSP value above fH, and
    // monotonically increasing (by band index) in between — the MET-04 contract.
    #[test]
    fn soft_ground_xi_varies_monotonically_by_band_index() {
        let c = sound_speed_ms(15.0);
        let (a, b, z0, h_s, h_r, d) = (0.6, 0.05, 0.02, 0.5, 1.5, 97.5);
        let axis = crate::freq::FreqAxis::new();
        let (xi_full, _) = calc_eq_ssp(h_s, h_r, z0, a, b, c).unwrap();
        assert!(xi_full > 0.0, "downward gradient must give ξ > 0");

        let xis: Vec<f64> = axis
            .centres
            .iter()
            .map(|&f| {
                calc_eq_ssp_ground(f, d, h_s, h_r, SOFT_SIGMA_KPA, z0, a, b, c)
                    .unwrap()
                    .0
            })
            .collect();

        // Bottom band sits below fL ⇒ ξ = 0. (fH can exceed 10 kHz for this
        // geometry, so the top *grid* band need not reach the full value —
        // "above fH" is checked separately with an out-of-grid frequency below.)
        assert_eq!(xis[0], 0.0, "below fL ξ must be exactly 0");
        let (xi_above, _) =
            calc_eq_ssp_ground(1.0e6, d, h_s, h_r, SOFT_SIGMA_KPA, z0, a, b, c).unwrap();
        assert_relative_eq!(xi_above, xi_full, max_relative = 1e-9);

        // Non-decreasing across band index (log-interpolated gradient, Eq. 23).
        for w in xis.windows(2) {
            assert!(
                w[1] >= w[0] - 1e-12,
                "ξ(f) must be non-decreasing by band index: {} then {}",
                w[0],
                w[1]
            );
        }
        // And it genuinely spans the range (not a flat 0 or flat full).
        assert!(
            xis.iter().any(|&x| x > 0.0 && x < xi_full),
            "ξ(f) must take intermediate values between fL and fH"
        );
    }

    // Homogeneous profile (A=B=0) over soft ground collapses to (0, C) at EVERY
    // band — the regression guard that CalcEqSSPGround reduces to the 03-01 limit.
    #[test]
    fn soft_ground_homogeneous_collapses_at_every_band() {
        let c = sound_speed_ms(15.0);
        let axis = crate::freq::FreqAxis::new();
        for &f in axis.centres.iter() {
            let (xi, c0) =
                calc_eq_ssp_ground(f, 97.5, 0.5, 1.5, SOFT_SIGMA_KPA, 0.001, 0.0, 0.0, c).unwrap();
            assert_eq!((xi, c0), (0.0, c), "homogeneous must give (0, C) at {f} Hz");
        }
    }

    // The soft/hard branch flips exactly at the σ threshold (10⁷ Pa·s·m⁻² =
    // 10⁴ kPa·s·m⁻²): a soft profile with a real gradient produces a band where
    // ξ differs from the hard (flat) value.
    #[test]
    fn soft_and_hard_branches_differ_on_a_real_gradient() {
        let c = sound_speed_ms(15.0);
        let (a, b, z0, h_s, h_r, d) = (0.6, 0.05, 0.02, 0.5, 1.5, 97.5);
        // A low band (below fL) is 0 on soft ground but the full value on hard.
        let f_low = crate::freq::FreqAxis::new().centres[0];
        let soft = calc_eq_ssp_ground(f_low, d, h_s, h_r, SOFT_GROUND_SIGMA_KPA - 1.0, z0, a, b, c)
            .unwrap();
        let hard =
            calc_eq_ssp_ground(f_low, d, h_s, h_r, SOFT_GROUND_SIGMA_KPA, z0, a, b, c).unwrap();
        assert_eq!(soft.0, 0.0, "soft ground below fL ⇒ ξ = 0");
        assert!(hard.0 > 0.0, "hard ground ⇒ full frequency-independent ξ");
    }

    #[test]
    fn ground_variant_rejects_bad_inputs() {
        let c = sound_speed_ms(15.0);
        assert!(matches!(
            calc_eq_ssp_ground(-1.0, 97.5, 0.5, 1.5, 200.0, 0.02, 0.6, 0.05, c),
            Err(PropagationError::DegenerateProfile { .. })
        ));
        assert!(matches!(
            calc_eq_ssp_ground(1000.0, 0.0, 0.5, 1.5, 200.0, 0.02, 0.6, 0.05, c),
            Err(PropagationError::DegenerateProfile { .. })
        ));
        assert!(matches!(
            calc_eq_ssp_ground(1000.0, 97.5, 0.5, 1.5, -200.0, 0.02, 0.6, 0.05, c),
            Err(PropagationError::InvalidFlowResistivity { .. })
        ));
    }
}
