//! Terrain-effect composition ΔL_t (AV 1106/07 §5.10–5.16, §5.22 Eq. 332):
//! Sub-models 1/2 (ground) and 4/5/6/7 (screens), combined by the §5.21
//! transition parameters into the per-band terrain excess-attenuation.
//!
//! Plan 02-02 lands the ground sub-models ([`submodel1`], [`submodel2`]) and the
//! load-bearing two-channel [`GroundResult`]. Screen sub-models + the Eq. 332
//! composition arrive in plan 02-04.
//!
//! Nord2000-native complex convention (e^{−jωt}); the single conjugation to
//! ENVI's e^{+jωt} transfer convention happens in `transfer.rs` (plan 02-05).
//!
//! # The two-channel contract (user-locked, PROJECT.md Key Decisions 2026-07-07)
//!
//! Every terrain-effect sub-model returns a [`GroundResult`] with **two
//! separate channels** relative to the free-field direct path `p̂₀`:
//!
//! - [`GroundResult::h_coh_factor`] is a genuine `Complex<f64>`: the
//!   coherence-weighted coherent sum `1 + Σ F·(Rᵢ)·e^{+j2πfΔτ}·Q̂` with the Δτ
//!   interference **phase LIVE**. This is the factor that multiplies into `H_coh`
//!   at the 02-05 transfer boundary; the ground-reflected contribution keeps its
//!   phase and combines as complex pressure — dips emerge from the phase, not
//!   from band-energy bookkeeping.
//! - [`GroundResult::p_incoh`] is a real, non-negative energy: **only** the
//!   turbulence-decorrelated `(1−F²)·|ρᵢ·p̂ᵢ/p̂₀|²` residual. It is added at
//!   final-level readout and **never overwrites phase**. When the field is fully
//!   coherent (`F → 1`) this channel is exactly `0`.
//!
//! The band value is `delta_l_db = 10·lg(|h_coh_factor|² + p_incoh)`. Nothing in
//! this module family collapses complex pressure to magnitude/energy along the
//! chain — that separation is what makes ENG-07 (phase-preserving combination)
//! and ENG-02 (segmented soft↔hard ground) correct.

use num_complex::Complex;

pub mod screen;
pub mod submodel1;
pub mod submodel2;

/// Two-channel result of any terrain-effect sub-model, normalized relative to
/// the free-field direct path `p̂₀`.
///
/// See the [module docs](self) for the user-locked contract. Nord2000-native
/// convention (e^{−jωt}) inside the engine; conversion to the transfer
/// convention happens once, in plan 02-05.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundResult {
    /// The Nord2000 band value `10·lg(|h_coh_factor|² + p_incoh)`, dB.
    pub delta_l_db: f64,
    /// The F-weighted coherent complex sum, phase LIVE (multiplies into `H_coh`).
    pub h_coh_factor: Complex<f64>,
    /// The `(1−F²)·|ρᵢ·p̂ᵢ/p̂₀|²` turbulence-decorrelated residual — real,
    /// non-negative, added at readout only. Exactly `0` when `F → 1`.
    pub p_incoh: f64,
}

impl GroundResult {
    /// Assemble a result from the two channels, deriving the band value from the
    /// two-channel identity `delta_l_db = 10·lg(|h_coh|² + p_incoh)`.
    #[must_use]
    pub fn from_channels(h_coh_factor: Complex<f64>, p_incoh: f64) -> Self {
        let energy = h_coh_factor.norm_sqr() + p_incoh;
        Self {
            delta_l_db: 10.0 * energy.log10(),
            h_coh_factor,
            p_incoh,
        }
    }
}
