//! A- and C-frequency-weighting tables at the 105 exact 1/12-octave grid
//! centres (D-09).
//!
//! # Module I/O
//! - **Input:** the engine's [`FreqAxis`] — the 105 exact centres
//!   `1000·G^((i−64)/12)` (never nominal Hz; RESEARCH Pitfall 3).
//! - **Output:** dense `[105]` A- and C-weighting dB tables ([`a_weighting_db`] /
//!   [`c_weighting_db`]), a [`Weighting`] selector, and the energy-domain
//!   [`weighted_total_db`] aggregator.
//!
//! # Net-new code (D-09), never an engine edit
//! These tables are precomputed ONCE so the dB(A) ⇄ dB(C) toggle is instant with
//! no tensor read and no MAC recompute. This is net-new WASM math living in
//! `envi-compute`, outside the frozen `envi-engine` 3-dep quarantine.
//!
//! # Formula (IEC 61672-1:2013 analytic frequency weightings)
//! Analog pole frequencies (Hz): `f₁ = 20.6`, `f₂ = 107.7`, `f₃ = 737.9`,
//! `f₄ = 12194`. Evaluated at each exact grid centre `f`:
//!
//! ```text
//! R_A(f) = f₄²·f⁴ / [ (f²+f₁²)·√((f²+f₂²)(f²+f₃²))·(f²+f₄²) ]
//! A(f)   = 20·log₁₀(R_A(f)) + 2.00  dB     (the +2.00 offset sets A(1000 Hz)=0)
//!
//! R_C(f) = f₄²·f² / [ (f²+f₁²)(f²+f₄²) ]
//! C(f)   = 20·log₁₀(R_C(f)) + 0.06  dB     (the +0.06 offset sets C(1000 Hz)=0)
//! ```
//!
//! Cited by report (no standard text pasted, threat T-11-01-03): IEC
//! 61672-1:2013 §5.4 + Table 3 — the pole frequencies 20.6, 107.7, 737.9, 12194
//! and the +2.00/+0.06 normalization offsets are the standard's public analytic
//! constants.

use envi_engine::freq::{FreqAxis, N_BANDS};

/// A frequency-weighting curve selector (D-09).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weighting {
    /// IEC 61672-1 A-weighting.
    A,
    /// IEC 61672-1 C-weighting.
    C,
}

impl Weighting {
    /// The dense `[105]` weighting table for this curve at the grid centres.
    #[must_use]
    pub fn table(self, _axis: &FreqAxis) -> [f64; N_BANDS] {
        todo!("GREEN phase")
    }
}

/// The A-weighting dB table at the 105 exact grid centres.
#[must_use]
pub fn a_weighting_db(_axis: &FreqAxis) -> [f64; N_BANDS] {
    todo!("GREEN phase")
}

/// The C-weighting dB table at the 105 exact grid centres.
#[must_use]
pub fn c_weighting_db(_axis: &FreqAxis) -> [f64; N_BANDS] {
    todo!("GREEN phase")
}

/// The energy-domain weighted total `10·log₁₀(Σ_i 10^((levels_i + w_i)/10))`,
/// aggregated strictly by band index.
#[must_use]
pub fn weighted_total_db(_levels: &[f64], _w: &[f64]) -> f64 {
    todo!("GREEN phase")
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn one_khz_is_normalized_to_zero() {
        let axis = FreqAxis::new();
        let a = a_weighting_db(&axis);
        let c = c_weighting_db(&axis);
        // Band index 64 is exactly 1000 Hz; the +2.00 / +0.06 offsets zero it.
        assert!(a[64].abs() < 0.01, "A(1000) must be ~0, got {}", a[64]);
        assert!(c[64].abs() < 0.01, "C(1000) must be ~0, got {}", c[64]);
    }

    #[test]
    fn third_octave_anchors_match_iec_table3() {
        // IEC 61672-1 Table 3 checkpoints, well inside Class-1 tolerance.
        let axis = FreqAxis::new();
        let a = a_weighting_db(&axis);
        let c = c_weighting_db(&axis);
        // Third-octave index → grid index = third_idx * 4.
        // 100 Hz = third_idx 6 → grid 24; 10 kHz = third_idx 26 → grid 104.
        assert_relative_eq!(a[24], -19.1, epsilon = 0.5);
        assert_relative_eq!(a[104], -2.5, epsilon = 0.5);
        assert_relative_eq!(c[24], -0.3, epsilon = 0.5);
        assert_relative_eq!(c[104], -4.4, epsilon = 0.5);
    }

    #[test]
    fn weighting_selector_matches_free_functions() {
        let axis = FreqAxis::new();
        assert_eq!(Weighting::A.table(&axis), a_weighting_db(&axis));
        assert_eq!(Weighting::C.table(&axis), c_weighting_db(&axis));
    }

    #[test]
    fn weighted_total_is_energy_sum_by_band_index() {
        // Two equal bands at 0 dB with 0 weight → 10·log10(2) = +3.0103 dB.
        let levels = [0.0, 0.0];
        let w = [0.0, 0.0];
        assert_relative_eq!(
            weighted_total_db(&levels, &w),
            10.0 * 2f64.log10(),
            epsilon = 1e-12
        );
        // A single band equals its own (level + weight).
        assert_relative_eq!(weighted_total_db(&[57.0], &[-3.0]), 54.0, epsilon = 1e-12);
    }

    #[test]
    fn weighted_total_applies_weight_by_index() {
        // dB(A) total of a flat 60 dB spectrum with an A table equals
        // 60 + 10·log10(Σ 10^(A_i/10)).
        let axis = FreqAxis::new();
        let a = a_weighting_db(&axis);
        let levels = vec![60.0_f64; N_BANDS];
        let total = weighted_total_db(&levels, &a);
        let expect_offset: f64 = a.iter().map(|&ai| 10f64.powf(ai / 10.0)).sum();
        assert_relative_eq!(total, 60.0 + 10.0 * expect_offset.log10(), epsilon = 1e-9);
    }
}
