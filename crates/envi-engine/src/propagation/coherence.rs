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

/// The truncated sinc shared by both band-averaging coherence factors:
/// `1` at the origin, `sin|x|/|x|` on `0 < |x| ≤ π`, and `0` beyond the first
/// zero.
///
/// The two callers differ ONLY in the argument they pass — [`coherence_ff`]
/// (Eq. 111) uses `0.23·π·f·Δτ`, while [`coherence_f_delta_nu`] (Eq. 112) uses
/// the full `2π·f·(Δτ⁺−Δτ)`. That factor difference (`0.23π` vs `2π`, RESEARCH
/// Pitfall 5) is **deliberate** and lives in each caller's argument, never here.
fn sinc_cutoff(x: f64) -> f64 {
    let xa = x.abs();
    if xa <= 1e-15 {
        1.0
    } else if xa <= std::f64::consts::PI {
        xa.sin() / xa
    } else {
        0.0
    }
}

/// The 1/3-octave band-averaging coherence factor `Ff` (AV 1106/07 Eq. 111).
///
/// `x = 0.23·π·f·Δτ`; `Ff = 1` at `x=0`, `sin(x)/x` for `0 < |x| ≤ π`, `0`
/// beyond. The `0.23` constant is glyph-verified (02-RESEARCH §6).
#[must_use]
pub fn coherence_ff(f_hz: f64, dtau: f64) -> f64 {
    // Argument 0.23·π·f·Δτ — the 0.23 band-averaging factor is deliberate and
    // distinct from FΔν's full 2π (see [`sinc_cutoff`], Pitfall 5).
    sinc_cutoff(0.23 * std::f64::consts::PI * f_hz * dtau)
}

/// The fluctuating-refraction coherence factor `FΔν` (AV 1106/07 Eq. 112).
///
/// `x = 2π·f·|Δτ⁺ − Δτ|`, where `Δτ` is the interference travel-time difference
/// under the mean profile `(A, B)` and `Δτ⁺` the one under the upper-refraction
/// profile `A⁺ = A + 1.7·sA`, `B⁺ = B + 1.7·sB` (Eq. 10). `FΔν = 1` at `x = 0`
/// (`Δτ⁺ = Δτ` ⇒ no fluctuation ⇒ `P_incoh → 0` bit-exact), `sin(x)/x` for
/// `0 < x ≤ π`, and `0` beyond.
///
/// # Pitfall 5 (RESEARCH): the argument carries a full `2π`
///
/// Unlike [`coherence_ff`] (Eq. 111, the 1/3-octave band-averaging sinc whose
/// argument is `0.23·π·f·Δτ`), the fluctuating-refraction argument is
/// `2π·f·(Δτ⁺−Δτ)` — there is **no `0.23` factor**. Copying `coherence_ff`'s
/// constant here would be wrong.
#[must_use]
pub fn coherence_f_delta_nu(f_hz: f64, dtau: f64, dtau_plus: f64) -> f64 {
    // Argument 2π·f·(Δτ⁺−Δτ) — the FULL 2π, NOT the 0.23π of `coherence_ff`
    // (Pitfall 5). `sinc_cutoff` takes |x| internally, matching the former
    // `.abs()` here bit-for-bit.
    sinc_cutoff(TAU * f_hz * (dtau_plus - dtau))
}

/// The coherence coefficient `F = Ff · FΔν · Fc · Fr · Fs` (AV 1106/07 Eq. 110).
///
/// - `dtau` — interference time difference `Δτ` (from [`super::rays`]).
/// - `rho_sep` — transversal separation `ρ` (caller responsibility: flat ground
///   `ρ = 2·hS·hR/(hS+hR)`, Eq. 119; screen sub-models per Eqs. 178/180).
/// - `psi_g` — grazing angle (feeds `Fr`).
#[must_use]
pub fn coherence_f(
    f_hz: f64,
    dtau: f64,
    rho_sep: f64,
    psi_g: f64,
    inputs: &CoherenceInputs,
) -> f64 {
    let ff = coherence_ff(f_hz, dtau);

    // Fc — turbulence decorrelation (Eq. 113): Fc = exp'(−x),
    // x = 5.888e−3·(CT²/(273.15+t)² + (22/3)·Cv²/c²)·f²·ρ^{5/3}·d.
    // (Constant/exponents are Assumption A3; for Cv²=CT²=0, x=0 ⇒ Fc=1.)
    let fc = if inputs.cv2 == 0.0 && inputs.ct2 == 0.0 {
        1.0
    } else {
        let t_abs = 273.15 + inputs.t_air_c;
        let turb =
            inputs.ct2 / (t_abs * t_abs) + (22.0 / 3.0) * inputs.cv2 / (inputs.c0 * inputs.c0);
        let x = 5.888e-3 * turb * f_hz * f_hz * rho_sep.abs().powf(5.0 / 3.0) * inputs.d_m;
        exp_clamped(-x)
    };

    // Fr — roughness decorrelation (Eq. 114): exp'-form in (k₀·r·sin ψ_G)².
    // The g(X) polynomial is Assumption A4 (unavailable) and set to 1; for
    // roughness r=0 (all Phase 2 targets) Fr=1 exactly regardless.
    let fr = if inputs.roughness_r == 0.0 {
        1.0
    } else {
        let k0 = TAU * f_hz / inputs.c0;
        let arg = k0 * inputs.roughness_r * psi_g.sin();
        exp_clamped(-0.5 * arg * arg)
    };

    // Fs — scattering-zone factor: documented stub = 1.0 (not active Phase 2).
    let fs = 1.0;

    ff * inputs.f_delta_nu * fc * fr * fs
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
        assert!(
            hi < coherence_ff(1000.0, dtau),
            "Fc must reduce F at high f"
        );
    }

    // FΔν Test 1: Δτ⁺ = Δτ ⇒ FΔν = 1.0 bit-exact at every frequency (the
    // sA=sB=0 non-fluctuating regime ⇒ P_incoh → 0 exactly).
    #[test]
    fn f_delta_nu_is_one_bit_exact_when_dtau_plus_equals_dtau() {
        for &f in &[25.0, 63.0, 250.0, 646.7, 1000.0, 4000.0, 10000.0] {
            for &dtau in &[0.0, 1.0e-6, 4.519_660_952e-5, 1.0e-3] {
                assert_eq!(
                    coherence_f_delta_nu(f, dtau, dtau),
                    1.0,
                    "FΔν must be exactly 1.0 when Δτ⁺=Δτ (f={f}, Δτ={dtau})"
                );
            }
        }
    }

    // FΔν Test 2: the argument is 2π·f·(Δτ⁺−Δτ), NOT 0.23π (Pitfall 5). Pin the
    // exact sinc value at x = 1: choose Δτ⁺−Δτ so 2π·f·Δ = 1 ⇒ FΔν = sin(1)/1.
    #[test]
    fn f_delta_nu_uses_the_two_pi_argument_not_0_23_pi() {
        let f = 1000.0;
        let delta = 1.0 / (TAU * f); // ⇒ x = 2π·f·delta = 1 rad exactly
        let got = coherence_f_delta_nu(f, 0.0, delta);
        assert!(
            (got - 1.0_f64.sin()).abs() < 1e-12,
            "FΔν(x=1) must be sin(1)/1 = {}, got {got}",
            1.0_f64.sin()
        );
        // The 0.23π sinc at the same Δτ would give sin(0.23)/0.23 — proving the
        // constant is genuinely 2π here, not the band-averaging 0.23π.
        assert!(
            (got - coherence_ff(f, delta)).abs() > 0.1,
            "FΔν (2π arg) must differ markedly from the 0.23π Ff sinc"
        );
    }

    // FΔν Test 3: monotone non-increasing in |Δτ⁺−Δτ| across [0, π], strictly
    // < 1 once the fluctuation bites, and exactly 0 beyond x = π (property test,
    // no fixed-value oracle — the turbulence std-devs are AV Assumptions, D-11).
    #[test]
    fn f_delta_nu_monotone_and_cuts_off_past_pi() {
        let f = 1000.0;
        // Sweep Δτ⁺−Δτ so x = 2π·f·Δ runs 0 → π.
        let mut prev = f64::INFINITY;
        let mut saw_strictly_below_one = false;
        for k in 0..=20 {
            let x = std::f64::consts::PI * (k as f64) / 20.0;
            let delta = x / (TAU * f);
            let ft = coherence_f_delta_nu(f, 0.0, delta);
            assert!(ft <= 1.0 + 1e-15, "FΔν ≤ 1 (x={x})");
            assert!(ft <= prev + 1e-12, "FΔν must be non-increasing (x={x})");
            if ft < 1.0 - 1e-9 {
                saw_strictly_below_one = true;
            }
            prev = ft;
        }
        assert!(
            saw_strictly_below_one,
            "FΔν must drop below 1 as the fluctuation grows"
        );
        // Past x = π ⇒ exactly 0.
        let delta_big = (std::f64::consts::PI * 1.5) / (TAU * f);
        assert_eq!(
            coherence_f_delta_nu(f, 0.0, delta_big),
            0.0,
            "FΔν must cut off to 0 beyond x = π"
        );
    }

    // FΔν Test 4: symmetric in the sign of (Δτ⁺−Δτ) (the |·| in the argument).
    #[test]
    fn f_delta_nu_is_symmetric_in_the_difference_sign() {
        let f = 800.0;
        let d = 2.0e-4;
        assert_eq!(
            coherence_f_delta_nu(f, 0.0, d),
            coherence_f_delta_nu(f, d, 0.0),
            "FΔν must depend only on |Δτ⁺−Δτ|"
        );
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
