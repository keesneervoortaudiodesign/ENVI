//! The engine's frequency vocabulary: a fixed 105-point 1/12-octave
//! evaluation grid from 25.119 Hz to 10 kHz.
//!
//! # Grid definition
//!
//! Centre frequencies follow the IEC 61260-1:2014 base-10 octave ratio
//! `G = 10^(3/10)` with the `x/12` rule:
//!
//! ```text
//! f(x) = 1000 · G^(x/12),   x = −64 ..= 40   (105 points)
//! ```
//!
//! # Deliberate deviation from IEC 61260-1 (document it, don't "fix" it)
//!
//! `b = 12` is an even bandwidth designator, so a strictly IEC-conformant
//! 1/12-octave *filter bank* would use the even-b midband rule
//! `f_m = f_r · G^((2x+1)/24)` — half-step offset midbands that NEVER hit
//! 1000 Hz. ENVI instead uses the odd-b style `x/12` rule so that the grid
//! **contains all 27 exact 1/3-octave centres** (every point with
//! `x ≡ 0 (mod 4)`). This is an evaluation grid for Nord2000's
//! "band level = value at the centre frequency" semantics, not a bank of
//! IEC filter passbands — the standard constrains filters, not evaluation
//! grids. The alignment makes FORCE 27-band comparison a direct index pick
//! (`third_octave_pick`), no aggregation or interpolation needed.
//!
//! # Pitfall: never compare nominal frequencies as floats
//!
//! Nominal labels (25, 31.5, 40, … 10000) name the exact centres
//! (25.1189, 31.6228, 39.8107, … 10000.0). Index bands by [`BandIdx`] /
//! the `third_idx` integer; render [`NOMINAL_THIRD_OCT`] labels only for
//! display and reporting. `f == 31.5` anywhere is a bug.

use std::sync::LazyLock;

/// Number of 1/12-octave evaluation points: `x = −64 ..= 40`.
pub const N_BANDS: usize = 105;

/// Number of exact 1/3-octave centres contained in the grid (25 Hz … 10 kHz).
pub const N_THIRD_OCT: usize = 27;

/// IEC 61260-1 base-10 octave frequency ratio, `G = 10^(3/10)`.
pub const G: f64 = 1.995_262_314_968_879_5;

/// Nominal 1/3-octave band labels (IEC 61260-1 nominal frequencies).
///
/// **Display only.** These are labels for the exact centres at grid indices
/// `0, 4, 8, …, 104` — never use them as computation frequencies or float
/// keys (Pitfall 3 in 01-RESEARCH).
pub const NOMINAL_THIRD_OCT: [f64; N_THIRD_OCT] = [
    25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0, 500.0,
    630.0, 800.0, 1000.0, 1250.0, 1600.0, 2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0, 8000.0,
    10000.0,
];

/// Index into the 105-point 1/12-octave grid.
///
/// Newtype so band positions are never confused with other `usize` values
/// and band values are never looked up by float comparison on frequencies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BandIdx(pub usize);

/// The fixed 105-point 1/12-octave frequency axis.
///
/// Construct once via [`FREQ_AXIS`] (or [`FreqAxis::new`] in tests) and pass
/// by reference — the axis is the engine's shared vocabulary.
#[derive(Debug, Clone)]
pub struct FreqAxis {
    /// Exact centre frequencies in Hz: `centres[i] = 1000 · G^((i−64)/12)`.
    pub centres: [f64; N_BANDS],
}

impl FreqAxis {
    /// Build the axis: `1000 · G^(x/12)` for `x = −64 ..= 40`.
    #[must_use]
    pub fn new() -> Self {
        let mut centres = [0.0; N_BANDS];
        for (i, x) in (-64_i32..=40).enumerate() {
            centres[i] = 1000.0 * G.powf(f64::from(x) / 12.0);
        }
        Self { centres }
    }

    /// Exact 1/3-octave centre frequency for `third_idx ∈ 0..27`.
    ///
    /// Every 4th grid point is an exact 1/3-octave centre:
    /// `third_idx` 0 → nominal 25 Hz (exact 25.1189 Hz), 16 → 1000 Hz,
    /// 26 → 10 kHz. This index pick IS the Nord2000 band value under
    /// "evaluate at the centre frequency" semantics.
    #[must_use]
    pub fn third_octave_pick(&self, third_idx: usize) -> f64 {
        debug_assert!(
            third_idx < N_THIRD_OCT,
            "third_idx out of range: {third_idx}"
        );
        self.centres[third_idx * 4]
    }
}

impl Default for FreqAxis {
    fn default() -> Self {
        Self::new()
    }
}

/// Process-wide shared frequency axis singleton.
pub static FREQ_AXIS: LazyLock<FreqAxis> = LazyLock::new(FreqAxis::new);

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn axis_has_105_centres_with_exact_anchors() {
        let axis = FreqAxis::new();
        assert_eq!(axis.centres.len(), N_BANDS);
        // f(−64) = 1000·10^(0.3·(−64/12)) = 10^(3 − 1.6) = 25.118864… Hz
        assert_relative_eq!(axis.centres[0], 25.118_864, max_relative = 1e-6);
        // x = 0 ⇒ exactly 1000 Hz (G^0 = 1.0, no rounding at all)
        assert_relative_eq!(axis.centres[64], 1000.0, max_relative = 1e-12);
        // x = 40 ⇒ 1000·G^(10/3) = 10^4 Hz
        assert_relative_eq!(axis.centres[104], 10_000.0, max_relative = 1e-12);
    }

    #[test]
    fn centres_are_strictly_ascending() {
        let axis = FreqAxis::new();
        for w in axis.centres.windows(2) {
            assert!(w[0] < w[1], "centres must be strictly ascending: {w:?}");
        }
    }

    #[test]
    fn third_octave_pick_is_every_fourth_point() {
        let axis = FreqAxis::new();
        assert_relative_eq!(axis.third_octave_pick(16), 1000.0, max_relative = 1e-12);
        for i in 0..N_THIRD_OCT {
            // bit-identical: the pick IS the grid point, not a recomputation
            assert_eq!(
                axis.third_octave_pick(i).to_bits(),
                axis.centres[i * 4].to_bits()
            );
        }
    }

    #[test]
    fn nominal_labels_align_with_exact_centres() {
        let axis = FreqAxis::new();
        for (i, &nominal) in NOMINAL_THIRD_OCT.iter().enumerate() {
            let exact = axis.third_octave_pick(i);
            // nominal labels are within 2 % of the exact centres they name
            assert_relative_eq!(exact, nominal, max_relative = 0.02);
        }
    }
}
