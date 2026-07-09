//! `GET /api/v1/meta/freq-axis` — the band-index wire anchor (SVC-07).
//!
//! The 105-point 1/12-octave axis is served **once**, built at runtime from
//! `envi_engine::freq` (Pitfall 9: never hardcode the axis). It is the ONLY place
//! Hz values appear on the wire — every spectrum elsewhere is a dense `[105]`
//! array keyed by band **index**, never nominal Hz. Serialization happens HERE
//! because serde never enters `envi-engine` (D-05).

use axum::Json;
use serde::Serialize;

use envi_engine::freq::{FREQ_AXIS, N_BANDS, N_THIRD_OCT, NOMINAL_THIRD_OCT};

/// The frequency-axis wire payload, built at runtime from the engine constants.
///
/// `centres_hz` and `nominal_third_octave_hz` are the only Hz values that ever
/// cross the network; they are display/charting data, never keys. Spectra are
/// keyed by position in the 105-point grid (`centres_hz[i]` is band index `i`).
#[derive(Debug, Clone, Serialize)]
pub struct FreqAxisDto {
    /// Number of 1/12-octave bands (105).
    pub n_bands: usize,
    /// Exact centre frequencies, Hz, one per band index `0..n_bands`.
    pub centres_hz: Vec<f64>,
    /// Grid indices of the 27 exact 1/3-octave centres: `0, 4, 8, …, 104`.
    pub third_octave_indices: Vec<usize>,
    /// Nominal 1/3-octave labels (25, 31.5, …, 10000) — **display only**.
    pub nominal_third_octave_hz: Vec<f64>,
}

impl FreqAxisDto {
    /// Build the DTO from the engine's shared axis (runtime, never hardcoded).
    #[must_use]
    pub fn from_engine() -> Self {
        Self {
            n_bands: N_BANDS,
            centres_hz: FREQ_AXIS.centres.to_vec(),
            third_octave_indices: (0..N_THIRD_OCT).map(|i| i * 4).collect(),
            nominal_third_octave_hz: NOMINAL_THIRD_OCT.to_vec(),
        }
    }
}

/// Handler: serve the frequency axis as JSON.
pub async fn freq_axis() -> Json<FreqAxisDto> {
    Json(FreqAxisDto::from_engine())
}
