//! Shared band-index spectrum interpolation core (D-05 / D-06, SCN-03).
//!
//! # Module I/O
//! - **Input:** a [`Resolution`] (Octave / Third / Twelfth) + an authored
//!   `values: &[f64]` slice whose length MUST match the resolution's anchor
//!   count (9 / 27 / 105). Values are the per-band sound-reduction index `R`
//!   (dB) at the resolution's grid anchors.
//! - **Output:** a dense `[f64; 105]` grid on the frozen 1/12-octave band index
//!   `0..=104` (see [`envi_engine::freq`]), linearly interpolated **in band
//!   index** between bracketing anchors and flat-hold extrapolated outside the
//!   authored span.
//! - **Valid input range:** `values.len() ∈ {9, 27, 105}` (per resolution);
//!   every value must be finite. Out-of-length ⇒ [`StoreError::BadBandCount`];
//!   non-finite ⇒ [`StoreError::NonFinite`].
//!
//! # The single interpolation core (D-05, no-divergence)
//!
//! This is the ONE place band-index interpolation lives. The read-path
//! `r_db[105]` derivation (D-06 — `r_db` is DERIVED on read, never a second
//! persisted field), the Phase-7 interpolation endpoint, and `PUT /scene`
//! validation all consume this function so they cannot drift apart.
//!
//! # Exact stride math (07-RESEARCH Pattern 6 — verified)
//!
//! Compare / interpolate **BY BAND INDEX, never nominal Hz** (Pitfall 4). Linear
//! in band index equals linear in log-frequency on the x/12 grid, which is the
//! SCN-03 contract.
//! - **Octave** → 9 anchors at grid indices `4 + 12·k` for `k ∈ 0..9`
//!   (4, 16, 28, 40, 52, 64, 76, 88, 100). Each is an exact 1/1-octave centre.
//! - **Third** → 27 anchors at grid indices `4·k` for `k ∈ 0..27`
//!   (0, 4, 8, …, 104). Every 4th 1/12 point is an exact 1/3-octave centre.
//! - **Twelfth** → identity over all 105 band indices (`0..=104`).
//!
//! # Flat-hold extrapolation (07-RESEARCH Assumptions Log A1 — adopted)
//!
//! The octave grid spans indices 4..=100, so bands `0..=3` and `101..=104` fall
//! outside the authored span. Those unspanned bands are **flat-hold clamped** to
//! the nearest endpoint anchor value (band 0..3 ⇒ `values[0]`; band 101..104 ⇒
//! `values[8]`). This is a deliberate, documented choice (A1): a measured coarse
//! spectrum carries no information beyond its endpoints, and flat-hold is the
//! least-surprising, monotone-preserving extrapolation.
//!
//! # No range clamp here — the engine constructor owns `[0, MAX_R_DB]`
//!
//! This core does NOT clamp values into `[0, 1000]`. Range enforcement is the
//! sole responsibility of `IsolationSpectrum::new` (private `r_db`,
//! `MAX_R_DB = 1000`): an authored value of e.g. `2000` must reach `new()` and be
//! REJECTED, never silently clamped to a wrong `1000` (07-01 must-have #2; threat
//! T-07-01-02). Clamping here would defeat that gate, so this function only
//! validates length + finiteness (threat T-07-01-01) and leaves range to the
//! validating constructor.

use envi_engine::freq::N_BANDS;
use serde::{Deserialize, Serialize};

use crate::StoreError;

/// Authoring resolution of a coarse spectrum: how many anchors the `values`
/// slice carries, and where they land on the dense 1/12-octave band index.
///
/// Serialized lowercase (`"octave"` / `"third"` / `"twelfth"`) so the persisted
/// [`crate::dto::AuthoredSpectrumDto`] is human-inspectable (D-06).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Resolution {
    /// 1/1-octave — 9 anchors at band indices 4, 16, …, 100.
    Octave,
    /// 1/3-octave — 27 anchors at band indices 0, 4, …, 104.
    Third,
    /// 1/12-octave — the dense grid itself (105 anchors, identity).
    Twelfth,
}

impl Resolution {
    /// The dense band indices this resolution's `values` land on, strictly
    /// ascending. `.len()` is the required authored `values.len()`.
    #[must_use]
    fn anchors(self) -> Vec<usize> {
        match self {
            Resolution::Octave => (0..9).map(|k| 4 + 12 * k).collect(),
            Resolution::Third => (0..27).map(|k| 4 * k).collect(),
            Resolution::Twelfth => (0..N_BANDS).collect(),
        }
    }

    /// The number of authored anchor values this resolution expects.
    #[must_use]
    pub fn anchor_count(self) -> usize {
        match self {
            Resolution::Octave => 9,
            Resolution::Third => 27,
            Resolution::Twelfth => N_BANDS,
        }
    }
}

/// Interpolate an authored coarse spectrum onto the dense `[105]` band-index grid.
///
/// Linear **in band index** between bracketing anchors; flat-hold clamp to the
/// nearest endpoint outside the authored span (A1). Anchor bands are copied
/// bit-for-bit (no arithmetic), so a round-trip through an exact grid is lossless.
///
/// # Errors
/// - [`StoreError::BadBandCount`] if `values.len()` ≠ the resolution's anchor
///   count (rejected before any allocation over an attacker-controlled length —
///   threat T-07-01-01).
/// - [`StoreError::NonFinite`] if any authored value is NaN or `±∞`.
///
/// Range (`[0, MAX_R_DB]`) is intentionally NOT enforced here — see the module
/// header; the engine `IsolationSpectrum::new` constructor is the sole gate.
pub fn interpolate(resolution: Resolution, values: &[f64]) -> Result<[f64; N_BANDS], StoreError> {
    let expected = resolution.anchor_count();
    if values.len() != expected {
        return Err(StoreError::BadBandCount { got: values.len() });
    }
    for (i, v) in values.iter().enumerate() {
        if !v.is_finite() {
            return Err(StoreError::NonFinite {
                what: format!("authored spectrum values[{i}] = {v}"),
            });
        }
    }

    let anchors = resolution.anchors();
    let last = expected - 1;
    let mut out = [0.0_f64; N_BANDS];

    for (b, slot) in out.iter_mut().enumerate() {
        *slot = if b <= anchors[0] {
            // Below the authored span (or exactly the first anchor): flat-hold.
            values[0]
        } else if b >= anchors[last] {
            // Above the authored span (or exactly the last anchor): flat-hold.
            values[last]
        } else {
            // Find the bracketing anchors i, i+1 with anchors[i] <= b <= anchors[i+1].
            let mut i = 0;
            while anchors[i + 1] < b {
                i += 1;
            }
            if anchors[i] == b {
                values[i] // exact anchor — copy verbatim (bit-for-bit).
            } else if anchors[i + 1] == b {
                values[i + 1] // exact anchor — copy verbatim.
            } else {
                // Strictly linear in band index between the two anchors.
                let span = (anchors[i + 1] - anchors[i]) as f64;
                let t = (b - anchors[i]) as f64 / span;
                values[i] + t * (values[i + 1] - values[i])
            }
        };
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Octave anchors land on 4,16,…,100 bit-for-bit; flat-hold on the ends;
    /// strictly linear in index between anchors.
    #[test]
    fn octave_anchors_land_exactly_and_flat_hold_ends() {
        let vals: Vec<f64> = (0..9).map(|k| k as f64 * 3.0 + 1.0).collect(); // 1,4,7,…,25
        let dense = interpolate(Resolution::Octave, &vals).expect("valid octave");

        let anchor_idx = [4usize, 16, 28, 40, 52, 64, 76, 88, 100];
        for (k, &idx) in anchor_idx.iter().enumerate() {
            assert_eq!(
                dense[idx].to_bits(),
                vals[k].to_bits(),
                "octave anchor {idx} must equal input {k} bit-for-bit"
            );
        }

        // Flat-hold: bands 0..=3 = values[0]; bands 101..=104 = values[8].
        for (b, val) in dense[0..=3].iter().enumerate() {
            assert_eq!(val.to_bits(), vals[0].to_bits(), "low flat-hold band {b}");
        }
        for (offset, val) in dense[101..=104].iter().enumerate() {
            let b = 101 + offset;
            assert_eq!(val.to_bits(), vals[8].to_bits(), "high flat-hold band {b}");
        }

        // Strict linearity between anchor 4 (=1.0) and anchor 16 (=4.0): a 12-band
        // span, so each index step adds 3.0/12 = 0.25.
        for (offset, val) in dense[4..=16].iter().enumerate() {
            let b = 4 + offset;
            let expected = 1.0 + offset as f64 * (3.0 / 12.0);
            assert!(
                (val - expected).abs() < 1e-12,
                "band {b} expected {expected}, got {val}"
            );
        }
    }

    /// Third-octave anchors land on 0,4,…,104 bit-for-bit.
    #[test]
    fn third_octave_anchors_land_exactly() {
        let vals: Vec<f64> = (0..27).map(|k| k as f64 * 0.5 - 2.0).collect();
        let dense = interpolate(Resolution::Third, &vals).expect("valid third");
        for (k, v) in vals.iter().enumerate() {
            let idx = 4 * k;
            assert_eq!(
                dense[idx].to_bits(),
                v.to_bits(),
                "third anchor {idx} must equal input {k} bit-for-bit"
            );
        }
        // A between-anchor band (e.g. 5, between 4 and 8) is the linear midpoint-ish.
        let expected5 = vals[1] + 0.25 * (vals[2] - vals[1]);
        assert!((dense[5] - expected5).abs() < 1e-12);
    }

    /// Twelfth resolution is the identity: input echoed verbatim across all 105.
    #[test]
    fn twelfth_is_identity() {
        let vals: Vec<f64> = (0..N_BANDS).map(|k| k as f64 * 1.25 - 10.0).collect();
        let dense = interpolate(Resolution::Twelfth, &vals).expect("valid twelfth");
        for (b, &v) in vals.iter().enumerate() {
            assert_eq!(dense[b].to_bits(), v.to_bits(), "twelfth band {b} verbatim");
        }
    }

    /// Wrong length ⇒ BadBandCount carrying the wrong length.
    #[test]
    fn wrong_length_is_bad_band_count() {
        let short = vec![0.0; 8];
        assert!(
            matches!(
                interpolate(Resolution::Octave, &short),
                Err(StoreError::BadBandCount { got: 8 })
            ),
            "8 values for octave must be BadBandCount {{ got: 8 }}"
        );
        let wrong_third = vec![0.0; 26];
        assert!(matches!(
            interpolate(Resolution::Third, &wrong_third),
            Err(StoreError::BadBandCount { got: 26 })
        ));
    }

    /// A non-finite authored value ⇒ NonFinite (never NaN into the dense grid).
    #[test]
    fn non_finite_value_is_rejected() {
        let mut vals = vec![1.0; 9];
        vals[3] = f64::NAN;
        assert!(
            matches!(
                interpolate(Resolution::Octave, &vals),
                Err(StoreError::NonFinite { .. })
            ),
            "NaN authored value must be rejected"
        );
        vals[3] = f64::INFINITY;
        assert!(matches!(
            interpolate(Resolution::Octave, &vals),
            Err(StoreError::NonFinite { .. })
        ));
    }

    /// The core does NOT clamp: an out-of-range authored value passes through so
    /// the engine constructor (not this fn) is the `[0, MAX_R_DB]` gate.
    #[test]
    fn does_not_clamp_out_of_range() {
        let mut vals = vec![10.0; 9];
        vals[4] = 2000.0; // anchor index 52
        let dense = interpolate(Resolution::Octave, &vals).expect("valid length/finite");
        assert_eq!(
            dense[52].to_bits(),
            2000.0_f64.to_bits(),
            "2000 passes through unclamped"
        );
    }
}
