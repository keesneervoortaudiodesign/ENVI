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
    todo!("GREEN")
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
