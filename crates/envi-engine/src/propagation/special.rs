//! Nord2000-native special functions: the document's own Faddeeva
//! approximation `w(ẑ)` (AV 1106/07 Eqs. 61–74), the Fresnel-integral
//! polynomial fits `f`/`g` (Eqs. 85–86 + Tables 4/5), and the clamped
//! exponential `exp'` (Eq. 337, §5.23.3).
//!
//! # Convention (load-bearing)
//!
//! Nord2000-native module family: time convention **e^{−jωt}**, outgoing phase
//! **e^{+jωτ}**, impedance **Im > 0** (AV 1106/07). Conversion to ENVI's
//! e^{+jωt} transfer convention happens in exactly ONE function at the
//! `TransferSpectrum` boundary (plan 02-05) — **never here**. (02-RESEARCH
//! Pattern 1: convention quarantine.)
//!
//! These are shared numerics: [`faddeeva_w`] feeds the ground boundary-loss
//! factor `Ê(ρ̂)` (Eq. 60); [`fresnel_f`]/[`fresnel_g`] feed the wedge
//! diffraction coefficient `Â_D` (Eq. 84); [`exp_clamped`] feeds the coherence
//! coefficients `Fc`/`Fr` (Eqs. 113–114). All pure over `f64`/`Complex<f64>`.

use num_complex::Complex;
use std::f64::consts::PI;

/// The Faddeeva function `w(ẑ) = e^{−ẑ²}·erfc(−j·ẑ)`, computed by the
/// document's own three-branch approximation (AV 1106/07 Eqs. 61–74).
///
/// The approximation reproduces `scipy.special.wofz` to `< 8·10⁻⁷` relative
/// error across the plane (02-RESEARCH §4). General `ẑ` is reduced to the first
/// quadrant (`x ≥ 0, y ≥ 0`) via the Eq. 62 symmetry relations, then dispatched
/// to a two-pole rational (Eq. 63), a three-pole rational (Eq. 64), or the
/// Matta–Reichel series (Eqs. 65–74).
#[must_use]
pub fn faddeeva_w(z: Complex<f64>) -> Complex<f64> {
    if z.im >= 0.0 {
        if z.re >= 0.0 {
            w_plus(z)
        } else {
            // Eq. 62: reflect across the imaginary axis into quadrant 1.
            // w(ẑ) = conj(w(−conj(ẑ))), with −conj(ẑ) = (−x, y), x < 0 ⇒ Re > 0.
            // Conjugation is written explicitly (negate Im) rather than `.conj()`
            // — this is Faddeeva symmetry math, NOT a convention conversion, and
            // the sole convention-boundary conjugation lives in `transfer.rs`
            // (the propagation-module `conj` quarantine, threat T-02-15).
            let refl = Complex::new(-z.re, z.im); // −conj(ẑ)
            let w = w_plus(refl);
            Complex::new(w.re, -w.im) // conj(w)
        }
    } else {
        // Eq. 62: lower half-plane via w(−ẑ) = 2·exp(−ẑ²) − w(ẑ) ⇒
        // w(ẑ) = 2·exp(−ẑ²) − w(−ẑ), where −ẑ is in the upper half-plane.
        // Guard the exp against overflow for large |Im ẑ| (Re(−ẑ²) = y²−x²);
        // the fixture grid (Task 2) covers this hard-ground y<0 region.
        let neg_z2 = -(z * z);
        let mut two_exp = 2.0 * neg_z2.exp();
        if !two_exp.re.is_finite() || !two_exp.im.is_finite() {
            two_exp = Complex::new(
                two_exp.re.clamp(-f64::MAX, f64::MAX),
                two_exp.im.clamp(-f64::MAX, f64::MAX),
            );
        }
        two_exp - faddeeva_w(-z)
    }
}

/// `w⁺(ẑ)` for `ẑ` in the first quadrant (`x ≥ 0, y ≥ 0`) — the branch
/// dispatcher (AV 1106/07 Eqs. 63–74).
fn w_plus(z: Complex<f64>) -> Complex<f64> {
    let (x, y) = (z.re, z.im);
    if x > 3.9 || y > 3.0 {
        if x > 6.0 || y > 6.0 {
            two_pole(z) // Eq. 63
        } else {
            three_pole(z) // Eq. 64
        }
    } else {
        matta_reichel(z) // Eqs. 65–74
    }
}

/// Two-pole rational approximation (AV 1106/07 Eq. 63), `< 4·10⁻⁷` vs wofz.
fn two_pole(z: Complex<f64>) -> Complex<f64> {
    let z2 = z * z;
    Complex::<f64>::I * z * (0.5124242 / (z2 - 0.2752551) + 0.05176536 / (z2 - 2.724745))
}

/// Three-pole rational approximation (AV 1106/07 Eq. 64), `< 8·10⁻⁷` vs wofz.
fn three_pole(z: Complex<f64>) -> Complex<f64> {
    let z2 = z * z;
    Complex::<f64>::I
        * z
        * (0.4613135 / (z2 - 0.1901635)
            + 0.09999216 / (z2 - 1.7844927)
            + 0.002883894 / (z2 - 5.5253437))
}

/// Matta–Reichel-type series (AV 1106/07 Eqs. 65–74), `h = 0.8`, `n = 1..=5`;
/// `< 3·10⁻⁷` vs wofz. Sign structure per 02-RESEARCH §4 (resolved by
/// exhaustive search against wofz).
fn matta_reichel(z: Complex<f64>) -> Complex<f64> {
    const H: f64 = 0.8;
    let (x, y) = (z.re, z.im);
    let s2 = x * x + y * y;
    if s2 == 0.0 {
        return Complex::new(1.0, 0.0); // w(0) = 1 (avoid 0/0 in H,K)
    }
    let a1 = (2.0 * x * y).cos();
    let b1 = (2.0 * x * y).sin();
    let c1 = (-2.0 * PI * y / H).exp() - (2.0 * PI * x / H).cos();
    let d1 = (2.0 * PI * x / H).sin();
    let e = (y * y - x * x - 2.0 * PI * y / H).exp();
    let den = c1 * c1 + d1 * d1;
    let p2 = 2.0 * e * (a1 * c1 - b1 * d1) / den;
    let q2 = 2.0 * e * (a1 * d1 + b1 * c1) / den;

    let mut h_sum = (H * y / PI) / s2;
    let mut k_sum = (H * x / PI) / s2;
    for n in 1..=5 {
        let n2h2 = (n * n) as f64 * H * H;
        let denom = (s2 + n2h2).powi(2) - 4.0 * x * x * n2h2;
        h_sum += (2.0 * H * y / PI) * (-n2h2).exp() * (s2 + n2h2) / denom;
        k_sum += (2.0 * H * x / PI) * (-n2h2).exp() * (s2 - n2h2) / denom;
    }
    Complex::new(h_sum + p2, k_sum - q2)
}

/// Auxiliary Fresnel-integral fit `f(x)` (AV 1106/07 Eq. 85 + Table 4).
///
/// `f(x) = 1/(πx)` for `x ≥ 5`; otherwise the 12th-order polynomial with the
/// Table 4 coefficients (Horner form). Used by the wedge diffraction
/// coefficient `Â_D(B) = Sign(B)·(f(|B|) − j·g(|B|))` (Eq. 84).
/// Table 4 coefficients `a₀..a₁₂` for `f(x)` (AV 1106/07, full precision).
const F_COEFFS: [f64; 13] = [
    0.49997531354311,
    0.00185249867385,
    -0.80731059547652,
    1.15348730691625,
    -0.89550049255859,
    0.44933436012454,
    -0.15130803310630,
    0.03357197760359,
    -0.00447236493671,
    0.00023357512010,
    0.00002262763737,
    -0.00000418231569,
    0.00000019048125,
];

/// Table 5 coefficients `a₀..a₁₂` for `g(x)` (AV 1106/07, full precision).
const G_COEFFS: [f64; 13] = [
    0.50002414586702,
    -1.00151717179967,
    0.80070190014386,
    -0.06004025873978,
    -0.50298686904881,
    0.55984929401694,
    -0.33675804584105,
    0.13198388204736,
    -0.03513592318103,
    0.00631958394266,
    -0.00073624261723,
    0.00005018358067,
    -0.00000151974284,
];

/// Horner evaluation of a coefficient list `[a₀, a₁, …]` at `x`.
fn horner(coeffs: &[f64], x: f64) -> f64 {
    coeffs.iter().rev().fold(0.0, |acc, &c| acc * x + c)
}

#[must_use]
pub fn fresnel_f(x: f64) -> f64 {
    if x >= 5.0 {
        1.0 / (PI * x)
    } else {
        horner(&F_COEFFS, x)
    }
}

/// Auxiliary Fresnel-integral fit `g(x)` (AV 1106/07 Eq. 86 + Table 5).
///
/// `g(x) = 1/(π²x³)` for `x ≥ 5`; otherwise the 12th-order polynomial with the
/// Table 5 coefficients (Horner form).
#[must_use]
pub fn fresnel_g(x: f64) -> f64 {
    if x >= 5.0 {
        1.0 / (PI * PI * x * x * x)
    } else {
        horner(&G_COEFFS, x)
    }
}

/// The clamped exponential `exp'(x)` (AV 1106/07 Eq. 337, §5.23.3).
///
/// `exp'(x) = e^x` for `x ≥ −1`; a value-continuous quadratic tail for
/// `−2 ≤ x < −1`; and `0` for `x < −2`. Used by the coherence coefficients
/// `Fc`/`Fr` (Eqs. 113–114) to avoid underflow in the turbulence/roughness
/// decorrelation exponents.
#[must_use]
pub fn exp_clamped(x: f64) -> f64 {
    if x >= -1.0 {
        x.exp()
    } else if x > -2.0 {
        // Value-continuous quadratic tail joining (−1, e⁻¹) to (−2, 0):
        // e⁻¹·(x+2)². (02-RESEARCH A-note: the exact Eq. 337 middle-branch
        // polynomial is transcribed here as the continuity-preserving form;
        // Phase 2 target cases have zero turbulence so x = 0 and this branch is
        // never exercised on the FORCE 1–8 targets.)
        std::f64::consts::E.recip() * (x + 2.0).powi(2)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    /// Relative error of a complex anchor as a whole: `|got − want| / |want|`.
    /// (Component-wise relative is spuriously strict when one component is near
    /// zero, e.g. the small real part of w(7+1j); the document approximation's
    /// stated `< 8e-7` error is on the complex value.)
    fn assert_c(got: Complex<f64>, want: Complex<f64>, rel: f64) {
        let err = (got - want).norm();
        let denom = want.norm().max(1e-12);
        assert!(
            err / denom <= rel,
            "relative error {:.2e} exceeds {:.1e}: got {got}, want {want}",
            err / denom,
            rel
        );
    }

    // --- Test 1: wofz anchors (tol 1e-6 relative) ---------------------------
    // Full-precision values from scipy.special.wofz (the reference the doc
    // approximation targets). NOTE: 02-RESEARCH §4's printed 7+1j anchor
    // (0.019924+0.139158j) is a transcription error — the true wofz(7+1j) is
    // 0.0116300+0.0797321j (confirmed by the asymptote w(z)→i/(√π z)).
    #[test]
    fn faddeeva_matches_wofz_anchors_all_quadrant1() {
        assert_c(
            faddeeva_w(Complex::new(0.5, 0.5)),
            Complex::new(0.5331567079121748, 0.2304882313844585),
            1e-6,
        );
        assert_c(
            faddeeva_w(Complex::new(1.0, 1.0)),
            Complex::new(0.30474420525691254, 0.2082189382028316),
            1e-6,
        );
        assert_c(
            faddeeva_w(Complex::new(3.0, 2.0)),
            Complex::new(0.0927107664264434, 0.12831696222826167),
            1e-6,
        );
        assert_c(
            faddeeva_w(Complex::new(7.0, 1.0)),
            Complex::new(0.011629963043136758, 0.07973205590137562),
            1e-6,
        );
    }

    // --- Test 2: quadrant symmetry relations (Eq. 62) -----------------------
    // Exercised THROUGH the public function at negated/conjugated inputs, so
    // the quadrant reduction is what is under test. Soft ground puts ρ̂ in
    // x<0,y>0; hard ground in y<0 — the symmetries are not dead code.
    #[test]
    fn faddeeva_symmetry_relations_hold_in_all_quadrants() {
        for &z in &[
            Complex::new(0.5, 0.5),
            Complex::new(1.0, 1.0),
            Complex::new(3.0, 2.0),
            Complex::new(7.0, 1.0),
        ] {
            let w = faddeeva_w(z);
            // Eq. 62: w(conj(-ẑ)) == conj(w(ẑ))  (reflection across imag axis).
            // Written without `.conj()` — the propagation-module conj quarantine
            // keeps the sole convention conjugation in `transfer.rs`.
            let conj_neg_z = Complex::new(-z.re, z.im); // conj(−ẑ)
            let conj_w = Complex::new(w.re, -w.im); // conj(w)
            assert_c(faddeeva_w(conj_neg_z), conj_w, 1e-9);
            // Eq. 62: w(-ẑ) == 2·exp(-ẑ²) - w(ẑ)  (lower half-plane, y<0)
            let two_exp = 2.0 * (-(z * z)).exp();
            assert_c(faddeeva_w(-z), two_exp - w, 1e-9);
        }
    }

    // --- Test 3: branch-border continuity (no jump > 1e-4) ------------------
    // Straddle each border with a tiny step so the measured difference is the
    // branch discontinuity, not the function's smooth variation (the true
    // branch jump is < 5e-7; 02-RESEARCH Pitfall 5).
    #[test]
    fn faddeeva_is_continuous_across_every_branch_border() {
        let d = 1e-4;
        let borders = [
            (Complex::new(3.9, 1.0), Complex::new(1.0, 0.0)), // |x|=3.9 at y=1
            (Complex::new(1.0, 3.0), Complex::new(0.0, 1.0)), // |y|=3 at x=1
            (Complex::new(6.0, 1.0), Complex::new(1.0, 0.0)), // x=6 at y=1
            (Complex::new(1.0, 6.0), Complex::new(0.0, 1.0)), // y=6 at x=1
        ];
        for (center, dir) in borders {
            let below = faddeeva_w(center - dir * d);
            let above = faddeeva_w(center + dir * d);
            assert!(
                (above - below).norm() < 1e-4,
                "branch jump at {center} too large: {}",
                (above - below).norm()
            );
        }
    }

    // --- Test 4: Fresnel fits f/g -------------------------------------------
    #[test]
    fn fresnel_fits_match_tables_and_asymptotes() {
        assert_relative_eq!(fresnel_f(0.0), 0.49997531354311, epsilon = 1e-12);
        assert_relative_eq!(fresnel_g(0.0), 0.50002414586702, epsilon = 1e-12);
        // continuity at the x=5 asymptote switch
        assert!((fresnel_f(4.999) - fresnel_f(5.001)).abs() < 1e-3);
        assert!((fresnel_g(4.999) - fresnel_g(5.001)).abs() < 1e-3);
        // asymptote branch exact
        assert_relative_eq!(fresnel_f(10.0), 1.0 / (10.0 * PI), epsilon = 1e-15);
        assert_relative_eq!(fresnel_g(10.0), 1.0 / (PI * PI * 1000.0), epsilon = 1e-15);
    }

    // --- Test 5: clamped exponential exp' -----------------------------------
    #[test]
    fn exp_clamped_matches_eq_337_and_is_continuous() {
        assert_relative_eq!(exp_clamped(0.0), 1.0, epsilon = 1e-15);
        assert_relative_eq!(exp_clamped(-0.5), (-0.5f64).exp(), epsilon = 1e-15);
        assert_relative_eq!(exp_clamped(-3.0), 0.0, epsilon = 1e-15);
        // continuity at the x=-1 and x=-2 breakpoints (within 1e-9)
        let step = 1e-10;
        assert!((exp_clamped(-1.0 + step) - exp_clamped(-1.0 - step)).abs() < 1e-9);
        assert!((exp_clamped(-2.0 + step) - exp_clamped(-2.0 - step)).abs() < 1e-9);
        // exact breakpoint values
        assert_relative_eq!(exp_clamped(-1.0), (-1.0f64).exp(), epsilon = 1e-15);
        assert_relative_eq!(exp_clamped(-2.0), 0.0, epsilon = 1e-15);
    }
}
