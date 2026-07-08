//! The load-bearing **complex transfer convention** — the seed of the project's
//! output contract (PROJECT.md complex-transfer-tensor pillar).
//!
//! # Convention (define once, never violate)
//!
//! Source: AV 1106/07 Eq. (329)/(330)/(335). This is the contract Phase 2
//! multiplies ground/diffraction ratios into, Phase 3 replaces τ-differences
//! in, and Phase 4 stacks into the tensor. Retrofitting phase later is the
//! RESEARCH-flagged anti-pattern; it is live from day one, tested by
//! cancellation.
//!
//! - **Time convention** `e^{+jωt}`. An outgoing wave carries phase
//!   `e^{−jωτ}` with `τ = R/c` the **carried primitive** (`c = 20.05·√(t+273.15)`,
//!   Eq. 335). Computing `k·R` as `2π·f·(R/c)` and keeping `τ` means Phase 3's
//!   cancellation-safe `Δτ` reformulation slots straight in — do NOT store `kR`.
//! - **Amplitude normalization**: `|H|` is normalized so band SPL falls out of
//!   `L_W` by addition — `L_p(f) = L_W(f) + 20·log10|H(f)|`. For the free field
//!   `|H| = 1/√(4πR²)`, hence `20·log10|H| = ΔL_d = −10·log10(4πR²)` (Eq. 330).
//!   Air absorption multiplies in as the real factor `10^(−ΔLₐ/20)`.
//! - **Phase 2+ effects** (ground, diffraction) multiply `H` by their complex
//!   pressure ratio relative to free field — Nord2000 computes exactly that
//!   ratio (free-field reference `p₀ = 1/r_SR`, AV 1106/07 §5.13). So the
//!   Phase 1 `H` is the correct seed: later effects are complex multiplications,
//!   and `Δτ` interference appears in the phase automatically.
//! - **Receiver band level** from a source spectrum: `|G_s(f)| = 10^{L_W(f)/20}`
//!   (phase 0 unless conditioned), `p(f) = Σ_s H[s,r,f]·G_s(f)`,
//!   `L(f) = 20·log10|p(f)|`. That is precisely the Phase 4 MAC contract
//!   (OUT-03), seeded here by [`band_levels_db`] for the single-source case.

use ndarray::Array3;
use num_complex::Complex;

use crate::freq::N_BANDS;
use crate::scene::BandSpectrum;

/// One source→receiver complex transfer spectrum: `Complex<f64>` per
/// 1/12-octave point, length [`N_BANDS`] (105).
///
/// Phase 4 stacks these into a [`TransferTensor`].
pub type TransferSpectrum = Vec<Complex<f64>>;

/// The Phase 4 forward-contract dense tensor of transfer values.
///
/// Shape is `[sub_source, receiver, freq]` in ndarray's default **row-major**
/// (C) order, so the **frequency axis is contiguous** on the last index — the
/// PROJECT.md numerics constraint. Never construct with Fortran-order (`.f()`)
/// helpers, which would break frequency contiguity. Phase 1 produces
/// [`TransferSpectrum`] slices; Phase 4 fills the tensor in.
pub type TransferTensor = Array3<Complex<f64>>;

/// Receiver band levels `L_p(f)` from a transfer spectrum and a source
/// `L_W` spectrum (the single-source seed of the Phase 4 MAC, OUT-03).
///
/// Per band: `|G_s| = 10^{L_W/20}`, `p = H·G_s`, `L_p = 20·log10|p|`
/// (`= L_W + 20·log10|H|` since `G_s` is real and non-negative).
///
/// # Panics
///
/// Never on data; debug-asserts that `h` has [`N_BANDS`] entries.
#[must_use]
pub fn band_levels_db(h: &TransferSpectrum, spectrum: &BandSpectrum) -> Vec<f64> {
    debug_assert_eq!(h.len(), N_BANDS, "transfer spectrum must be N_BANDS long");
    h.iter()
        .zip(spectrum.as_slice())
        .map(|(&hf, &lw_db)| {
            let g = 10f64.powf(lw_db / 20.0); // |G_s| = 10^(L_W/20), phase 0
            let p = hf * g; // p = H·G_s
            20.0 * p.norm().log10() // L_p = 20·log10|p|
        })
        .collect()
}

/// **THE** single convention boundary — the only conjugation in the whole
/// propagation codebase.
///
/// Everything inside `propagation::{ground,diffraction,terrain_effect,…}` is
/// Nord2000-native (time `e^{−jωt}`, outgoing phase `e^{+jωτ}`, impedance
/// `Im > 0`; 02-RESEARCH "Complex Combination & Conventions"). ENVI's
/// [`TransferSpectrum`] froze the opposite convention (`e^{+jωt}`, outgoing
/// `e^{−jωτ}`) in Phase 1 — the two are complex conjugates. This function
/// converts a Nord2000-native complex pressure **ratio** (relative to the free
/// field) into the ENVI transfer convention so a ground/diffraction factor can
/// multiply into `H_direct`. `|ratio|` is invariant.
///
/// The grep gate `\.conj()` over `propagation/` is **zero**; the one and only
/// terrain-factor conjugation lives here (RESEARCH Pattern 1, threat T-02-15).
/// Mixing conventions silently inverts the dip **asymmetry** (`arg Q̂`) — the
/// exact failure the two-path dip-equality test pins.
#[must_use]
pub fn nord_ratio_to_transfer(ratio: Complex<f64>) -> Complex<f64> {
    ratio.conj()
}

/// Two-channel receiver band levels — the user-locked ENG-07 readout law
/// `L(f) = L_W(f) + 10·lg(|H_coh(f)|² + |H_ff(f)|²·p_incoh(f))`.
///
/// - `h_coh` is the phase-preserving coherent transfer
///   `H_coh = H_ff · nord_ratio_to_transfer(h_coh_factor)` — the Δτ interference
///   lives in its phase (ENG-07); its magnitude carries the coherent level.
/// - `h_ff` is the free-field direct transfer (Phase 1 [`direct_path`]); the
///   incoherent, turbulence-decorrelated energy `p_incoh` rides at the
///   free-field magnitude, added **only here at readout** — it never touches
///   `arg(H_coh)`.
///
/// When `p_incoh` is all-zero this equals [`band_levels_db`] of `h_coh` exactly.
/// This supersedes [`band_levels_db`] for terrain cases; Phase 4's tensor MAC
/// builds on `H_coh` + a per-band `P_incoh` store (the forward contract).
///
/// [`direct_path`]: crate::propagation::direct_path
///
/// # Panics
///
/// Never on data; debug-asserts that all inputs have [`N_BANDS`] entries.
#[must_use]
pub fn band_levels_db_two_channel(
    h_coh: &TransferSpectrum,
    h_ff: &TransferSpectrum,
    p_incoh: &[f64],
    spectrum: &BandSpectrum,
) -> Vec<f64> {
    debug_assert_eq!(h_coh.len(), N_BANDS, "H_coh must be N_BANDS long");
    debug_assert_eq!(h_ff.len(), N_BANDS, "H_ff must be N_BANDS long");
    debug_assert_eq!(p_incoh.len(), N_BANDS, "p_incoh must be N_BANDS long");
    h_coh
        .iter()
        .zip(h_ff)
        .zip(p_incoh)
        .zip(spectrum.as_slice())
        .map(|(((&hc, &hf), &pi), &lw_db)| {
            // Total energy = coherent |H_coh|² + incoherent |H_ff|²·p_incoh.
            let energy = hc.norm_sqr() + hf.norm_sqr() * pi;
            lw_db + 10.0 * energy.log10()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn band_levels_add_lw_to_transfer_magnitude() {
        // |H| = 0.1 (⇒ 20·log10|H| = −20 dB) with a live phase; L_W = 80 dB.
        // L_p must be 80 − 20 = 60 dB regardless of the phase angle.
        let h: TransferSpectrum = (0..N_BANDS)
            .map(|i| Complex::from_polar(0.1, 0.37 * i as f64))
            .collect();
        let spectrum = BandSpectrum::uniform(80.0);
        let levels = band_levels_db(&h, &spectrum);
        assert_eq!(levels.len(), N_BANDS);
        for l in levels {
            assert_relative_eq!(l, 60.0, max_relative = 1e-12);
        }
    }

    #[test]
    fn nord_ratio_to_transfer_is_conjugation_magnitude_invariant() {
        // Test 2 (conj boundary): nord_ratio_to_transfer(z) == z.conj(); |z| kept.
        for z in [
            Complex::new(0.5, 0.3),
            Complex::new(-0.9, 0.2),
            Complex::new(0.0, -1.4),
        ] {
            let t = nord_ratio_to_transfer(z);
            assert_eq!(t, z.conj());
            assert_relative_eq!(t.norm(), z.norm(), max_relative = 1e-15);
            // The imaginary part flips sign (the convention conversion).
            assert_relative_eq!(t.im, -z.im, epsilon = 1e-15);
        }
    }

    #[test]
    fn two_channel_with_zero_p_incoh_equals_band_levels_of_h_coh() {
        // Test 5 (P_incoh separation): p_incoh all-zero ⇒ identical to
        // band_levels_db(H_coh).
        let h_coh: TransferSpectrum = (0..N_BANDS)
            .map(|i| Complex::from_polar(0.1 + 0.001 * i as f64, 0.21 * i as f64))
            .collect();
        let h_ff: TransferSpectrum = (0..N_BANDS)
            .map(|i| Complex::from_polar(0.2, -0.11 * i as f64))
            .collect();
        let p0 = vec![0.0_f64; N_BANDS];
        let spectrum = BandSpectrum::uniform(75.0);
        let two = band_levels_db_two_channel(&h_coh, &h_ff, &p0, &spectrum);
        let one = band_levels_db(&h_coh, &spectrum);
        for (a, b) in two.iter().zip(&one) {
            assert_relative_eq!(a, b, epsilon = 1e-12);
        }
    }

    #[test]
    fn two_channel_adds_incoherent_energy_at_free_field_magnitude() {
        // With H_coh = 0 the level is purely 10·lg(|H_ff|²·p_incoh) + L_W.
        let zero: TransferSpectrum = vec![Complex::new(0.0, 0.0); N_BANDS];
        let h_ff: TransferSpectrum = vec![Complex::new(0.1, 0.0); N_BANDS];
        let p = vec![4.0_f64; N_BANDS];
        let spectrum = BandSpectrum::uniform(0.0);
        let levels = band_levels_db_two_channel(&zero, &h_ff, &p, &spectrum);
        // 10·lg(0.01·4) = 10·lg(0.04) = −13.9794 dB.
        for l in levels {
            assert_relative_eq!(l, 10.0 * 0.04_f64.log10(), epsilon = 1e-12);
        }
    }

    #[test]
    fn transfer_tensor_is_row_major_frequency_contiguous() {
        // [sub_source, receiver, freq] in default order ⇒ standard C layout,
        // frequency contiguous on the last axis (PROJECT.md constraint).
        let t: TransferTensor = TransferTensor::zeros((2, 3, N_BANDS));
        assert_eq!(t.shape(), &[2, 3, N_BANDS]);
        assert!(
            t.is_standard_layout(),
            "tensor must be row-major so the frequency axis is contiguous"
        );
    }
}
