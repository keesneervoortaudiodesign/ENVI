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

use super::profile::Z0_MIN_M;
use crate::propagation::PropagationError;

/// The homogeneous-shortcut threshold (AV 1106/07 §5.5.2 p.24): when `|ξ| <
/// 1e-6`, the profile is treated as homogeneous (`ξ=0`, `c₀=C`). This is the
/// **D-02 bit-for-bit anchor threshold** — distinct from the DirectRay inner
/// division guard `|ξ|<1e-10` (RESEARCH Pitfall 1).
pub const XI_HOMOGENEOUS: f64 = 1e-6;

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

    let c_of = |z: f64| a * (z / z0 + 1.0).ln() + b * z + c;
    // Eq. 18: average gradient between hS and hR.
    let dcdz = (c_of(hr_g) - c_of(hs_g)) / (hr_g - hs_g);
    // Eq. 19 / Annex F Eq. 403: average sound speed c̄ (closed form).
    let c_bar = a * (antideriv_log(hr_g, z0) - antideriv_log(hs_g, z0)) / (hr_g - hs_g)
        + b * (hr_g + hs_g) / 2.0
        + c;
    // Eq. 20: c₀ = c̄ − (∂c/∂z)·(hS+hR)/2.
    let c0 = c_bar - dcdz * (hs_g + hr_g) / 2.0;
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
}
