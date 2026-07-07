//! Coherence coefficient F (AV 1106/07 §5.9, Eqs. 110–114).
//!
//! `F = Ff · FΔν · Fc · Fr · Fs` (Eq. 110) weights the coherent two-ray sum in
//! the partial-coherence combination `⟨|p₁+p₂|²⟩ = |p₁ + F·p₂|² + (1−F²)|p₂|²`.
//! Even in a homogeneous, zero-turbulence atmosphere `Ff` (1/3-octave band
//! averaging) is active; the other factors are 1 there.
//!
//! # Injected seams (not silent scope reduction)
//!
//! - **FΔν** (Eq. 112, refraction decoherence) is *injected* via
//!   [`CoherenceInputs::f_delta_nu`] — Phase 2 callers pass `1.0`; Phase 3 drops
//!   in `Δτ⁺` under the A⁺ profile without touching any call site.
//! - **Fs** (scattering-zone factor) is a documented stub `= 1.0` — not active
//!   for the Phase 2 target cases (02-RESEARCH §6 / Open Question 2).
//!
//! # Assumptions (transcription)
//!
//! The exact `Fc`/`Fr` constants are AV 1106/07 Assumptions A3/A4 (garbled in
//! the text dump; PDF page-image transcription was not available at execution).
//! The **structural forms are fixed** and used here with the research's stated
//! constants; for every Phase 2 target case (FORCE 1–8) `Cv²=CT²=0` and
//! roughness `r=0`, so `Fc=Fr=1` exactly and the exact constants are inert. The
//! turbulence cases are gated by *property* tests (Fc<1, monotonic), not by an
//! oracle that would pin the constant.
//!
//! # Deviation from the plan interface block
//!
//! [`CoherenceInputs`] carries an extra `d_m` (propagation distance): the `Fc`
//! turbulence integral (Eq. 113) is `∝ … · d`, which the interface block
//! omitted. Geometry stays caller-injected (`coherence.rs` is geometry-free);
//! `d_m` is provided by the caller alongside `rho_sep`.

use super::special::exp_clamped;
use std::f64::consts::TAU;

/// Weather / surface inputs to the coherence coefficient. All geometry is
/// caller-injected so this module stays geometry-free.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoherenceInputs {
    /// Wind-velocity structure parameter `Cv²` (m^{4/3}·s⁻²). `0` = no
    /// turbulence.
    pub cv2: f64,
    /// Temperature structure parameter `CT²` (K²·m^{−2/3}). `0` = no turbulence.
    pub ct2: f64,
    /// Air temperature, °C (the `273.15 + t` term of `Fc`).
    pub t_air_c: f64,
    /// Speed of sound, m/s (`k₀ = 2πf/c₀` and the `Cv²/c²` term).
    pub c0: f64,
    /// Terrain roughness `r`, meters (feeds `Fr` only; `0` for all Phase 2
    /// targets).
    pub roughness_r: f64,
    /// Injected refraction-decoherence factor `FΔν` (Eq. 112). `1.0` in Phase 2.
    pub f_delta_nu: f64,
    /// Propagation distance `d`, meters (the `Fc` turbulence-integral length).
    pub d_m: f64,
}

/// The 1/3-octave band-averaging coherence factor `Ff` (AV 1106/07 Eq. 111).
///
/// `x = 0.23·π·f·Δτ`; `Ff = 1` at `x=0`, `sin(x)/x` for `0 < |x| ≤ π`, `0`
/// beyond. The `0.23` constant is glyph-verified (02-RESEARCH §6).
#[must_use]
pub fn coherence_ff(f_hz: f64, dtau: f64) -> f64 {
    let x = 0.23 * std::f64::consts::PI * f_hz * dtau;
    let xa = x.abs();
    if xa <= 1e-15 {
        1.0
    } else if xa <= std::f64::consts::PI {
        xa.sin() / xa
    } else {
        0.0
    }
}

/// The coherence coefficient `F = Ff · FΔν · Fc · Fr · Fs` (AV 1106/07 Eq. 110).
///
/// - `dtau` — interference time difference `Δτ` (from [`super::rays`]).
/// - `rho_sep` — transversal separation `ρ` (caller responsibility: flat ground
///   `ρ = 2·hS·hR/(hS+hR)`, Eq. 119; screen sub-models per Eqs. 178/180).
/// - `psi_g` — grazing angle (feeds `Fr`).
#[must_use]
pub fn coherence_f(f_hz: f64, dtau: f64, rho_sep: f64, psi_g: f64, inputs: &CoherenceInputs) -> f64 {
    let _ = (f_hz, dtau, rho_sep, psi_g, inputs);
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zero_turbulence() -> CoherenceInputs {
        CoherenceInputs {
            cv2: 0.0,
            ct2: 0.0,
            t_air_c: 15.0,
            c0: 340.348,
            roughness_r: 0.0,
            f_delta_nu: 1.0,
            d_m: 97.5,
        }
    }

    // Test 4: Ff behavior.
    #[test]
    fn ff_matches_sinc_form_and_cutoffs() {
        assert_eq!(coherence_ff(1000.0, 0.0), 1.0); // Δτ=0 ⇒ Ff=1
        // spot-check f=646.7 Hz, Δτ=4.519660952e-5 ⇒ Ff ≈ 0.99993
        assert!((coherence_ff(646.7, 4.519660952e-5) - 0.9999256618).abs() < 1e-6);
        // x > π ⇒ Ff = 0 (huge Δτ)
        assert_eq!(coherence_ff(100000.0, 1.0e-2), 0.0);
    }

    // Test 5a: zero turbulence ⇒ F == Ff exactly (FORCE cases 1–8 regime).
    #[test]
    fn f_equals_ff_with_zero_turbulence() {
        let inp = zero_turbulence();
        for &f in &[63.0, 250.0, 646.7, 1000.0, 4000.0] {
            let dtau = 4.519660952e-5;
            let f_total = coherence_f(f, dtau, 0.75, 0.0205, &inp);
            assert!(
                (f_total - coherence_ff(f, dtau)).abs() < 1e-12,
                "F must equal Ff at zero turbulence (f={f})"
            );
        }
    }

    // Acceptance anchor: coherence_f(646.7, Δτ, ρ, ψ, zero-turb) ≈ 0.99993.
    #[test]
    fn coherence_f_anchor_at_dip_frequency() {
        let inp = zero_turbulence();
        let got = coherence_f(646.7, 4.519660952e-5, 0.75, 0.0205, &inp);
        assert!((got - 0.99993).abs() < 1e-4, "got {got}");
    }

    // Test 5b: with turbulence, Fc < 1 and F < Ff at high f; F non-increasing.
    #[test]
    fn turbulence_reduces_f_monotonically() {
        let inp = CoherenceInputs {
            cv2: 0.12,
            ct2: 0.008,
            ..zero_turbulence()
        };
        let dtau = 4.519660952e-5;
        let freqs = [63.0, 125.0, 250.0, 500.0, 1000.0];
        let mut prev = f64::INFINITY;
        for &f in &freqs {
            let f_total = coherence_f(f, dtau, 0.75, 0.0205, &inp);
            let ff = coherence_ff(f, dtau);
            assert!(f_total <= ff + 1e-15, "F must not exceed Ff (f={f})");
            assert!(f_total <= prev + 1e-12, "F must be non-increasing (f={f})");
            prev = f_total;
        }
        // strict: at 1000 Hz turbulence has bitten (F < Ff)
        let hi = coherence_f(1000.0, dtau, 0.75, 0.0205, &inp);
        assert!(hi < coherence_ff(1000.0, dtau), "Fc must reduce F at high f");
    }

    // k₀ helper sanity (used by Fr): k₀ = 2πf/c₀.
    #[test]
    fn roughness_zero_leaves_fr_unity() {
        // roughness_r = 0 ⇒ Fr = 1 ⇒ F = Ff at zero turbulence (already covered),
        // but assert a nonzero-roughness case still returns finite ≤ Ff.
        let inp = CoherenceInputs {
            roughness_r: 0.5,
            ..zero_turbulence()
        };
        let f = coherence_f(1000.0, 4.519660952e-5, 0.75, 0.5, &inp);
        assert!(f.is_finite() && f <= coherence_ff(1000.0, 4.519660952e-5) + 1e-15);
    }
}
