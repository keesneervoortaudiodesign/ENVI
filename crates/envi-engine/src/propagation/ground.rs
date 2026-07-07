//! Ground reflection-coefficient chain (AV 1106/07 §5.6, Eqs. 57–77).
//!
//! Delany–Bazley impedance `Ẑ_G` (Eq. 57) → plane-wave `Γ̂_p` (Eq. 59) →
//! boundary-loss factor `Ê(ρ̂)` (Eq. 60) → spherical-wave reflection coefficient
//! `Q̂` (Eq. 58); plus the incoherent reflection coefficient `ρᵢ` (Eqs. 75–76).
//!
//! # Convention
//!
//! Nord2000-native: time e^{−jωt}, impedance **Im > 0** (do NOT flip the sign
//! here — conversion to ENVI's e^{+jωt} convention is a single `conj()` at the
//! `TransferSpectrum` boundary in plan 02-05). See [`super::special`].
//!
//! # Deviation from the plan interface block
//!
//! [`ground_impedance`] returns `Result` (not a bare `Complex<f64>`): σ crosses
//! from untrusted case-file data into the complex numerics, so σ ≤ 0 / non-finite
//! is rejected with a typed error rather than producing NaN/Inf that would
//! poison every downstream `Q̂` (threat T-02-01, a definition-of-done must_have).

use num_complex::Complex;
use std::f64::consts::PI;

use super::PropagationError;
use super::special::faddeeva_w;

/// Delany–Bazley normalized ground impedance `Ẑ_G` (AV 1106/07 Eq. 57).
///
/// ```text
/// X   = f / σ           (σ in kPa·s·m⁻²; the SI-Pa form's 1000·f/σ is identical)
/// Ẑ_G = 1 + 9.08·X^(−0.75) + j·11.9·X^(−0.73)
/// ```
///
/// The kPa unit matches the FORCE `.xls` σ column and Table 2. `Im(Ẑ_G) > 0`
/// in the Nord2000 e^{−jωt} convention — not a sign to "fix" downstream.
///
/// # Errors
///
/// [`PropagationError::InvalidFlowResistivity`] if `sigma_kpa` is not strictly
/// positive and finite (untrusted terrain data — threat T-02-01).
pub fn ground_impedance(f_hz: f64, sigma_kpa: f64) -> Result<Complex<f64>, PropagationError> {
    if !(sigma_kpa.is_finite() && sigma_kpa > 0.0) {
        return Err(PropagationError::InvalidFlowResistivity { sigma_kpa });
    }
    let x = f_hz / sigma_kpa; // ≡ 1000·f/σ_SI (Eq. 57 unit note)
    Ok(Complex::new(
        1.0 + 9.08 * x.powf(-0.75),
        11.9 * x.powf(-0.73),
    ))
}

/// Plane-wave reflection coefficient `Γ̂_p = (sin ψ_G − 1/Ẑ_G)/(sin ψ_G + 1/Ẑ_G)`
/// (AV 1106/07 Eq. 59).
#[must_use]
pub fn gamma_p(psi_g: f64, z_g: Complex<f64>) -> Complex<f64> {
    let s = psi_g.sin();
    let inv_z = z_g.inv();
    (s - inv_z) / (s + inv_z)
}

/// Spherical-wave reflection coefficient `Q̂` (AV 1106/07 Eqs. 58 + 60).
///
/// ```text
/// ρ̂ = ((1+j)/2)·√(ω·τ₂)·(sin ψ_G + 1/Ẑ_G),   ω = 2πf
/// Ê = 1 + j·√π·ρ̂·w(ρ̂)
/// Q̂ = Γ̂_p + (1 − Γ̂_p)·Ê
/// ```
///
/// `Q̂` is parameterized by travel time `τ₂` (not `k·R₂`) so Phase 3 refraction
/// modifies it through `τ`. **`|Q̂|` is never clamped** — it legitimately exceeds
/// 1 in the surface-wave regime (02-RESEARCH anti-pattern; anchor σ=200/250 Hz
/// gives `|Q̂| = 1.257`).
#[must_use]
pub fn spherical_q(f_hz: f64, tau2_s: f64, psi_g: f64, z_g: Complex<f64>) -> Complex<f64> {
    let s = psi_g.sin();
    let inv_z = z_g.inv();
    let gp = (s - inv_z) / (s + inv_z); // Γ̂_p (Eq. 59)
    let omega_tau = 2.0 * PI * f_hz * tau2_s;
    // ρ̂ = ((1+j)/2)·√(ω·τ₂)·(sin ψ_G + 1/Ẑ_G)  (Eq. 60)
    let rho = Complex::new(0.5, 0.5) * omega_tau.sqrt() * (s + inv_z);
    // Ê = 1 + j·√π·ρ̂·w(ρ̂)  (Eq. 60)
    let e_hat = Complex::new(1.0, 0.0) + Complex::<f64>::I * PI.sqrt() * rho * faddeeva_w(rho);
    gp + (Complex::new(1.0, 0.0) - gp) * e_hat // Q̂ (Eq. 58) — never clamped
}

/// Incoherent (random-incidence) reflection coefficient `ρᵢ = √(1 − ᾱ_ri)`
/// (AV 1106/07 Eqs. 75–76).
///
/// Angle-independent — a function of `Ẑ_G` only (which already carries `f`), so
/// callers may precompute one value per (impedance class, frequency). `f_hz` is
/// accepted for signature symmetry with the rest of the chain.
#[must_use]
pub fn incoherent_rho(f_hz: f64, z_g: Complex<f64>) -> f64 {
    let _ = f_hz; // ρᵢ depends on f only through Ẑ_G
    // Random-incidence (Paris) absorption coefficient ᾱ_ri in X = Re Ẑ_G,
    // Y = Im Ẑ_G, m = |Ẑ_G|² (AV 1106/07 Eq. 76). This is the standard
    // closed-form statistical-absorption result; the pypdf text dump garbles
    // the arctan/ln arguments, so the canonical Paris form is used — it
    // reduces correctly to the real-impedance limit 8/X·[1 + 1/(1+X) −
    // (2/X)·ln(1+X)] as Y→0 (02-RESEARCH Assumption A1). Verified ᾱ_ri ∈ (0,1)
    // for every impedance class A–H at the FORCE bands.
    let x = z_g.re;
    let y = z_g.im;
    let m = x * x + y * y;
    let alpha_ri = (8.0 * x / m)
        * (1.0 - (x / m) * (1.0 + 2.0 * x + m).ln()
            + ((x * x - y * y) / (y * m)) * (y / (1.0 + x)).atan());
    // ρᵢ = √(1 − ᾱ_ri): ρᵢ² is the reflected-energy fraction in Eq. 120's
    // incoherent residual (the radical is the physically consistent reading,
    // AV 1106/07 Eq. 75). Clamp the radicand to [0, 1] for numerical safety.
    (1.0 - alpha_ri).clamp(0.0, 1.0).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::propagation::sound_speed_ms;
    use crate::scene::impedance_class;
    use approx::assert_relative_eq;

    fn assert_c(got: Complex<f64>, want: Complex<f64>, rel: f64) {
        let err = (got - want).norm();
        assert!(
            err / want.norm().max(1e-12) <= rel,
            "rel err {:.2e} > {:.1e}: got {got}, want {want}",
            err / want.norm().max(1e-12),
            rel
        );
    }

    /// Geometry anchor (hS=0.5, hR=1.5, d=97.5): (τ₂, ψ_G) with c₀ = 340.348.
    fn anchor_geometry() -> (f64, f64) {
        let (h_s, h_r, d) = (0.5_f64, 1.5_f64, 97.5_f64);
        let r2 = (d * d + (h_s + h_r).powi(2)).sqrt();
        let tau2 = r2 / sound_speed_ms(15.0);
        let psi_g = ((h_s + h_r) / d).atan();
        (tau2, psi_g)
    }

    // Test 1: Ẑ_G anchors (Eq. 57), tol 1e-4 relative; Im positive.
    #[test]
    fn ground_impedance_matches_delany_bazley_anchors() {
        assert_c(
            ground_impedance(1000.0, 200.0).unwrap(),
            Complex::new(3.715553, 3.675351),
            1e-4,
        );
        assert_c(
            ground_impedance(1000.0, 12.5).unwrap(),
            Complex::new(1.339444, 0.485614),
            1e-4,
        );
        assert_c(
            ground_impedance(100.0, 200.0).unwrap(),
            Complex::new(16.270679, 19.737805),
            1e-4,
        );
        assert_c(
            ground_impedance(1000.0, 20000.0).unwrap(),
            Complex::new(86.873338, 105.998290),
            1e-4,
        );
        assert_c(
            ground_impedance(63.0, 200000.0).unwrap(),
            Complex::new(3841.191953, 4283.317397),
            1e-4,
        );
        // Im must be positive (Nord2000 sign) — a hard directional assert.
        assert!(ground_impedance(1000.0, 200.0).unwrap().im > 0.0);
    }

    // Test 2: Q̂ chain anchors (Eqs. 58–60), tol 1e-4 relative.
    #[test]
    fn spherical_q_matches_research_anchors() {
        let (tau2, psi_g) = anchor_geometry();
        let cases = [
            (12.5, 1000.0, Complex::new(-0.947630, 0.020506)),
            (200.0, 250.0, Complex::new(-0.838688, 0.936625)),
            (200.0, 1000.0, Complex::new(-0.873836, 0.135191)),
            (200.0, 4000.0, Complex::new(-0.922626, 0.051218)),
            (20000.0, 1000.0, Complex::new(0.797560, 0.416202)),
            (20000.0, 4000.0, Complex::new(-0.004250, 0.608435)),
        ];
        for (sigma, f, want) in cases {
            let z_g = ground_impedance(f, sigma).unwrap();
            assert_c(spherical_q(f, tau2, psi_g, z_g), want, 1e-4);
        }
    }

    // Test 3: surface-wave regime — |Q̂| legitimately exceeds 1, no clamp.
    #[test]
    fn spherical_q_exceeds_unity_in_surface_wave_regime() {
        let (tau2, psi_g) = anchor_geometry();
        let z_g = ground_impedance(250.0, 200.0).unwrap();
        let q = spherical_q(250.0, tau2, psi_g, z_g);
        assert_relative_eq!(q.norm(), 1.257, epsilon = 1e-3);
        assert!(q.norm() > 1.0, "|Q̂| must not be clamped to ≤ 1");
    }

    // Test 4: ρᵢ ∈ (0,1) for every class A–H at 63/250/1000/4000 Hz;
    // angle-independent (the signature carries no ψ_G).
    #[test]
    fn incoherent_rho_is_bounded_and_angle_independent() {
        for class in ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'] {
            let sigma = impedance_class(class).unwrap();
            for &f in &[63.0, 250.0, 1000.0, 4000.0] {
                let z_g = ground_impedance(f, sigma).unwrap();
                let rho = incoherent_rho(f, z_g);
                assert!(
                    rho > 0.0 && rho < 1.0,
                    "ρᵢ out of (0,1): class {class} f={f} → {rho}"
                );
            }
        }
    }

    // Test 5: impedance class B corrected to 31.5 (Table 2 verified).
    #[test]
    fn impedance_class_b_is_corrected_to_31_5() {
        assert_eq!(impedance_class('B'), Some(31.5));
        assert_eq!(impedance_class('A'), Some(12.5));
        assert_eq!(impedance_class('C'), Some(80.0));
        assert_eq!(impedance_class('D'), Some(200.0));
        assert_eq!(impedance_class('E'), Some(500.0));
        assert_eq!(impedance_class('F'), Some(2000.0));
        assert_eq!(impedance_class('G'), Some(20000.0));
        assert_eq!(impedance_class('H'), Some(200000.0));
    }

    // Threat T-02-01: σ ≤ 0 / non-finite rejected with a typed error.
    #[test]
    fn ground_impedance_rejects_invalid_sigma() {
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(matches!(
                ground_impedance(1000.0, bad),
                Err(PropagationError::InvalidFlowResistivity { .. })
            ));
        }
    }
}
