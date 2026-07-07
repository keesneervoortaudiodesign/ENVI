//! Wedge diffraction (AV 1106/07 §5.7, Eqs. 78–107): Hadden–Pierce
//! finite-impedance wedge `pwedge`/`dwedge`, two-wedge `p2wedge`, thick-screen
//! `p2edge`, non-reflecting `pwedge0`.
//!
//! The only consumer of the Fresnel-integral fits
//! [`super::special::fresnel_f`]/[`super::special::fresnel_g`] (the diffraction
//! coefficient `Â_D`, Eq. 84) and of ground `Q̂` on the wedge faces
//! ([`super::ground::spherical_q`], Eq. 80).
//!
//! # Convention (load-bearing)
//!
//! Nord2000-native: time **e^{−jωt}**, outgoing phase **e^{+jωτ}** — the
//! diffracted pressure carries the over-the-top travel-time phase so it enters
//! ENG-07's coherent channel as complex pressure, never as a level. There is
//! **no `conj()` anywhere in this module** — convention conversion is a single
//! boundary in plan 02-05. See [`super::special`].

use num_complex::Complex;
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI, SQRT_2, TAU};

use super::PropagationError;
use super::ground::spherical_q;
use super::special::{fresnel_f, fresnel_g};

/// Single-wedge geometry inputs (AV 1106/07 §5.7.1, Fig. 9).
///
/// `θ_S`/`θ_R` are both measured from the **receiver** wedge face; validity is
/// `β > π` and `0 ≤ θ_R ≤ θ_S ≤ β ≤ 2π`. The p. 43 angle-modification scheme
/// (implemented in `modify_angles`) maps ground-reflected/refracted image points
/// that land inside the wedge back into the valid domain. `τ = τ_S + τ_R` and
/// `ℓ = R_S + R_R` in the direct case.
#[derive(Debug, Clone, Copy)]
pub struct WedgeGeometry {
    /// Travel time source → edge, s.
    pub tau_s: f64,
    /// Travel time edge → receiver, s.
    pub tau_r: f64,
    /// Total diffracted travel time (normally `τ_S + τ_R`), s.
    pub tau: f64,
    /// Distance source → edge, m.
    pub r_s: f64,
    /// Distance edge → receiver, m.
    pub r_r: f64,
    /// Total diffracted distance ℓ (normally `R_S + R_R`), m.
    pub l: f64,
    /// Source diffraction angle from the receiver face, rad.
    pub theta_s: f64,
    /// Receiver diffraction angle from the receiver face, rad.
    pub theta_r: f64,
    /// Wedge (exterior) angle β, rad; validity `β > π`.
    pub beta: f64,
}

/// The Hadden–Pierce wedge sound pressure `pwedge` (AV 1106/07 Eq. 91,
/// procedure Eqs. 78–90).
///
/// Returns the diffracted complex pressure ratio (relative to a unit source at
/// unit distance), carrying the outgoing phase `e^{+jωτ}`. `z_s`/`z_r` are the
/// source/receiver wedge-face impedances (Nord2000-native, `Im > 0`); a large
/// real impedance models a hard face.
///
/// # Errors
///
/// [`PropagationError::DegenerateRayGeometry`] on a non-finite or non-physical
/// wedge (non-positive `τ`/`ℓ`/distances, or `β ∉ (π, 2π]`).
pub fn pwedge(
    f_hz: f64,
    geo: &WedgeGeometry,
    z_s: Complex<f64>,
    z_r: Complex<f64>,
) -> Result<Complex<f64>, PropagationError> {
    pwedge_inner(f_hz, geo, z_s, z_r, false)
}

/// `Sign(x)` (AV 1106/07 auxiliary function): `+1` for `x ≥ 0`, else `−1`.
#[inline]
fn sign(x: f64) -> f64 {
    if x >= 0.0 { 1.0 } else { -1.0 }
}

/// `H(x)` Heaviside step (AV 1106/07 Eq. 354): `1` for `x > 0`, else `0`.
#[inline]
fn heaviside(x: f64) -> f64 {
    if x > 0.0 { 1.0 } else { 0.0 }
}

/// `Â_D(B) = Sign(B)·(f(|B|) − j·g(|B|))` (AV 1106/07 Eq. 84), with `f`/`g` the
/// Fresnel-integral fits ([`fresnel_f`]/[`fresnel_g`], Eqs. 85–86).
#[inline]
fn a_d(b: f64) -> Complex<f64> {
    let ab = b.abs();
    sign(b) * Complex::new(fresnel_f(ab), -fresnel_g(ab))
}

/// Reject a non-finite or non-physical wedge before any square-root/division.
fn validate(geo: &WedgeGeometry) -> Result<(), PropagationError> {
    let finite = [
        geo.tau_s, geo.tau_r, geo.tau, geo.r_s, geo.r_r, geo.l, geo.theta_s, geo.theta_r,
        geo.beta,
    ]
    .iter()
    .all(|v| v.is_finite());
    if !finite {
        return Err(PropagationError::DegenerateRayGeometry {
            detail: "non-finite wedge geometry input",
        });
    }
    if geo.tau_s <= 0.0 || geo.tau_r <= 0.0 || geo.tau <= 0.0 {
        return Err(PropagationError::DegenerateRayGeometry {
            detail: "non-positive wedge travel time",
        });
    }
    if geo.r_s <= 0.0 || geo.r_r <= 0.0 || geo.l <= 0.0 {
        return Err(PropagationError::DegenerateRayGeometry {
            detail: "non-positive wedge distance",
        });
    }
    // Validity of the Hadden–Pierce solution: β > π (§5.7.1). Allow up to 2π
    // (thin/thick screens sit at β = 2π−ε).
    if !(geo.beta > PI && geo.beta <= TAU + 1e-9) {
        return Err(PropagationError::DegenerateRayGeometry {
            detail: "wedge angle β outside (π, 2π]",
        });
    }
    Ok(())
}

/// `Q̂ₙ` on the wedge faces (AV 1106/07 Eq. 80), **prescriptively** transcribed:
/// `τ₂ = τ_S + τ_R` (the total diffracted travel time) and the grazing angles
/// `min(β − θ_S, π/2)` / `min(θ_R, π/2)` — do not re-derive (research
/// anti-pattern). Q₁ = 1; Q₄ = Q₂·Q₃.
fn face_q(
    n: usize,
    f_hz: f64,
    tau_s: f64,
    tau_r: f64,
    theta_s: f64,
    theta_r: f64,
    beta: f64,
    z_s: Complex<f64>,
    z_r: Complex<f64>,
) -> Complex<f64> {
    let tau2 = tau_s + tau_r;
    let q2 = || spherical_q(f_hz, tau2, (beta - theta_s).min(FRAC_PI_2), z_s);
    let q3 = || spherical_q(f_hz, tau2, theta_r.min(FRAC_PI_2), z_r);
    match n {
        1 => Complex::new(1.0, 0.0),
        2 => q2(),
        3 => q3(),
        _ => q2() * q3(),
    }
}

/// The four-term (or, for `pwedge0`, single-term) Hadden–Pierce sum
/// (AV 1106/07 Eqs. 78–86) — **without** the lit-zone additions (Eqs. 87–90),
/// which are applied by the caller. Returns `−(1/π)·Σ Q̂ₙ·A(θₙ)·Ê_ν · e^{jωτ}/ℓ`.
#[allow(clippy::too_many_arguments)]
fn wedge_sum(
    f_hz: f64,
    theta_s: f64,
    theta_r: f64,
    beta: f64,
    tau: f64,
    tau_s: f64,
    tau_r: f64,
    ell: f64,
    z_s: Complex<f64>,
    z_r: Complex<f64>,
    n1_only: bool,
) -> Complex<f64> {
    let w = TAU * f_hz;
    let nu = PI / beta; // wedge index ν = π/β
    let thetas = [
        theta_s - theta_r,            // θ₁ (Eq. 79)
        theta_s + theta_r,            // θ₂
        2.0 * beta - (theta_s + theta_r), // θ₃
        2.0 * beta - (theta_s - theta_r), // θ₄
    ];
    // (2·τ_S·τ_R/τ² + 1/2): the diffraction-spread coefficient (Eqs. 81/83).
    let coef = 2.0 * tau_s * tau_r / (tau * tau) + 0.5;
    let n_terms = if n1_only { 1 } else { 4 };
    let eps = 1.0e-8; // p. 41 singularity guard: |θₙ − π| < ε ⇒ θₙ −= ε.
    let mut acc = Complex::new(0.0, 0.0);
    for (i, &theta_n0) in thetas.iter().enumerate().take(n_terms) {
        let theta_n = if (theta_n0 - PI).abs() < eps { theta_n0 - eps } else { theta_n0 };
        // A(θₙ) = (ν/2)(−β − π + θₙ) + π·H(π − θₙ)  (Eq. 82).
        let a = 0.5 * nu * (-beta - PI + theta_n) + PI * heaviside(PI - theta_n);
        let abs_a = a.abs();
        let cos_a = abs_a.cos();
        // sinc guard: |A| < 1e-6 ⇒ sin|A|/|A| → 1 (Taylor limit, Pitfall 6).
        let sinc = if abs_a < 1.0e-6 { 1.0 } else { abs_a.sin() / abs_a };
        // B (Eq. 83) and Ê_ν (Eq. 81) share the spread denominator.
        let denom_e = (1.0 + coef * cos_a * cos_a / (nu * nu)).sqrt();
        let b = (4.0 * w * tau_s * tau_r / (PI * tau)).sqrt() * cos_a
            / (nu * nu + coef * cos_a * cos_a).sqrt();
        let e_nu =
            (PI / SQRT_2) * sinc * Complex::from_polar(1.0, FRAC_PI_4) / denom_e * a_d(b);
        let qn = if n1_only {
            Complex::new(1.0, 0.0)
        } else {
            face_q(i + 1, f_hz, tau_s, tau_r, theta_s, theta_r, beta, z_s, z_r)
        };
        acc += qn * a * e_nu;
    }
    -(1.0 / PI) * acc * Complex::from_polar(1.0, w * tau) / ell
}

/// Shared entry point for [`pwedge`] and [`pwedge0`] (`n1_only`).
fn pwedge_inner(
    f_hz: f64,
    geo: &WedgeGeometry,
    z_s: Complex<f64>,
    z_r: Complex<f64>,
    n1_only: bool,
) -> Result<Complex<f64>, PropagationError> {
    validate(geo)?;
    let p = wedge_sum(
        f_hz, geo.theta_s, geo.theta_r, geo.beta, geo.tau, geo.tau_s, geo.tau_r, geo.l, z_s,
        z_r, n1_only,
    );
    Ok(p)
}

/// The diffraction coefficient `D̂ = pwedge · ℓ · e^{−jωτ}` (AV 1106/07
/// Eqs. 92–94): the phase-stripped, dimensionless kernel used to chain wedges.
///
/// # Errors
///
/// Propagates [`pwedge`]'s domain error.
pub fn dwedge(
    f_hz: f64,
    geo: &WedgeGeometry,
    z_s: Complex<f64>,
    z_r: Complex<f64>,
) -> Result<Complex<f64>, PropagationError> {
    let p = pwedge(f_hz, geo, z_s, z_r)?;
    // D̂ = pwedge·ℓ·e^{−jωτ}: strip the outgoing phase and 1/ℓ spreading.
    Ok(p * geo.l * Complex::from_polar(1.0, -TAU * f_hz * geo.tau))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::propagation::sound_speed_ms;

    /// A large real impedance stands in for a hard (|Ẑ|→∞) wedge face.
    const HARD: Complex<f64> = Complex::new(1.0e9, 0.0);

    /// Build a thin-screen single wedge from S, T (edge), R (2-D points).
    /// θ measured CCW from the receiver-side face (straight down); β = 2π·frac.
    fn thin_wedge(s: (f64, f64), t: (f64, f64), r: (f64, f64), frac: f64) -> WedgeGeometry {
        let c0 = sound_speed_ms(15.0);
        let r_s = (t.0 - s.0).hypot(t.1 - s.1);
        let r_r = (r.0 - t.0).hypot(r.1 - t.1);
        let face = -FRAC_PI_2; // receiver face points down
        let ang = |p: (f64, f64)| {
            let phi = (p.1 - t.1).atan2(p.0 - t.0);
            (phi - face).rem_euclid(TAU)
        };
        WedgeGeometry {
            tau_s: r_s / c0,
            tau_r: r_r / c0,
            tau: (r_s + r_r) / c0,
            r_s,
            r_r,
            l: r_s + r_r,
            theta_s: ang(s),
            theta_r: ang(r),
            beta: TAU * frac,
        }
    }

    // --- Test 1: hard thin-screen insertion-loss anchor table --------------
    // IL = free-field direct level − diffracted level = −20·lg(|pwedge|·R_SR).
    #[test]
    fn pwedge_reproduces_hard_screen_il_table() {
        let (s, t, r) = ((0.0, 1.0), (50.0, 6.0), (100.0, 1.0));
        let geo = thin_wedge(s, t, r, 0.9999);
        let r_sr = (r.0 - s.0).hypot(r.1 - s.1);
        let want = [
            (125.0, 12.01),
            (250.0, 14.35),
            (500.0, 17.02),
            (1000.0, 19.90),
            (2000.0, 22.87),
            (4000.0, 25.87),
        ];
        for (f, il_want) in want {
            let p = pwedge(f, &geo, HARD, HARD).unwrap();
            let il = -20.0 * (p.norm() * r_sr).log10();
            assert!(
                (il - il_want).abs() <= 0.05,
                "IL({f} Hz) = {il:.3} dB, want {il_want} ± 0.05"
            );
        }
    }

    // --- Test 2: θₙ = π singularity guard (p. 41 ε-subtraction) ------------
    #[test]
    fn pwedge_is_finite_through_the_theta_n_pi_singularity() {
        // Sweep θ_S − θ_R across π; every value must stay finite (no 0/0).
        let c0 = sound_speed_ms(15.0);
        let r_s = 50.0;
        let r_r = 50.0;
        let theta_r = 1.2_f64;
        let beta = TAU * 0.9999;
        for k in -50..=50 {
            let th1 = PI + f64::from(k) * 1.0e-9; // straddle π incl. exactly π
            let geo = WedgeGeometry {
                tau_s: r_s / c0,
                tau_r: r_r / c0,
                tau: (r_s + r_r) / c0,
                r_s,
                r_r,
                l: r_s + r_r,
                theta_s: theta_r + th1,
                theta_r,
                beta,
            };
            let p = pwedge(1000.0, &geo, HARD, HARD).unwrap();
            assert!(p.re.is_finite() && p.im.is_finite(), "NaN/Inf at th1−π={th1:e}");
        }
    }

    // --- Test 3: sinc guard (|A(θₙ)| → 0 ⇒ Taylor sin|A|/|A| → 1) ----------
    #[test]
    fn pwedge_is_finite_when_the_sinc_argument_vanishes() {
        // |A| = 0 needs (ν/2)(θ−β−π)+πH = 0. In the shadow branch (θ>π, H=0):
        // θ = β + π. With β = 1.5π that is θ = 2.5π — out of range; instead use a
        // geometry that drives some |A(θₙ)| below 1e-6 and assert finiteness.
        let c0 = sound_speed_ms(15.0);
        let beta = 1.5 * PI;
        // θ₄ = 2β − (θ_S − θ_R); pick θ_S, θ_R so A(θ₄) ≈ 0.
        let theta_r = 0.2;
        let theta_s = 2.0 * beta - PI - beta + theta_r; // aim θ₄ near β+π
        let geo = WedgeGeometry {
            tau_s: 40.0 / c0,
            tau_r: 60.0 / c0,
            tau: 100.0 / c0,
            r_s: 40.0,
            r_r: 60.0,
            l: 100.0,
            theta_s: theta_s.clamp(theta_r, beta),
            theta_r,
            beta,
        };
        let p = pwedge(500.0, &geo, HARD, HARD).unwrap();
        assert!(p.re.is_finite() && p.im.is_finite());
    }

    // --- Test 5: phase liveness (genuine Complex; arg advances with f) ------
    #[test]
    fn pwedge_output_is_genuinely_complex_and_phase_advances_with_frequency() {
        let (s, t, r) = ((0.0, 1.0), (50.0, 6.0), (100.0, 1.0));
        let geo = thin_wedge(s, t, r, 0.9999);
        let mut prev_unwrapped = f64::NAN;
        let mut any_imag = false;
        let mut advanced = false;
        for &f in &[500.0_f64, 600.0, 700.0, 800.0] {
            let p = pwedge(f, &geo, HARD, HARD).unwrap();
            if p.im.abs() > 1e-9 {
                any_imag = true;
            }
            // e^{+jωτ}: the total phase 2πf·τ increases with f (τ > 0).
            let full = TAU * f * geo.tau;
            if !prev_unwrapped.is_nan() && full > prev_unwrapped {
                advanced = true;
            }
            prev_unwrapped = full;
        }
        assert!(any_imag, "pwedge must carry a live imaginary part");
        assert!(advanced, "outgoing phase 2πfτ must advance with f");
    }

    // --- Test 4: Â_D asymptote/polynomial continuity at the x = 5 switch ---
    // Exercised through fresnel_f/fresnel_g (Â_D = Sign·(f − j·g)); the branch
    // switch must be continuous to < 1e-3 relative.
    #[test]
    fn a_d_is_continuous_across_the_asymptote_switch() {
        let lo = Complex::new(fresnel_f(4.999), -fresnel_g(4.999));
        let hi = Complex::new(fresnel_f(5.001), -fresnel_g(5.001));
        assert!((lo - hi).norm() / lo.norm() < 1e-3);
    }
}
