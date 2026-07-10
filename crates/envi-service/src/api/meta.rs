//! `GET /api/v1/meta/freq-axis` — the band-index wire anchor (SVC-07) — plus
//! `POST /api/v1/meta/interpolate-spectrum` — the coarse→dense band-index
//! interpolation seam (D-05).
//!
//! The 105-point 1/12-octave axis is served **once**, built at runtime from
//! `envi_engine::freq` (Pitfall 9: never hardcode the axis). It is the ONLY place
//! Hz values appear on the wire — every spectrum elsewhere is a dense `[105]`
//! array keyed by band **index**, never nominal Hz. Serialization happens HERE
//! because serde never enters `envi-engine` (D-05).
//!
//! # `interpolate-spectrum` — one interpolation impl, server-owned (D-05, SVC-07)
//!
//! The handler is a THIN wrapper: it delegates the band-index math to the single
//! shared core [`envi_store::interpolate::interpolate`] (the same fn `PUT /scene`
//! validation and the read-path `r_db` derivation call, so they cannot diverge),
//! then passes the dense grid through the engine's validating
//! [`IsolationSpectrum::new`] range gate. No interpolation arithmetic and no
//! acoustic math live here (SVC-07) — the store owns the math, the engine owns
//! the `[0, MAX_R_DB]` range rule. Because 07-01's `interpolate()` deliberately
//! does NOT clamp, an out-of-range authored `R` (e.g. `> 1000`) must reach
//! `new()` and be REJECTED as a `4xx`, never silently coerced into a wrong 200.

use std::sync::LazyLock;

use axum::Json;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use envi_engine::freq::{FREQ_AXIS, N_BANDS, N_THIRD_OCT, NOMINAL_THIRD_OCT};
use envi_engine::propagation::transmission::IsolationSpectrum;
use envi_store::interpolate::{Resolution, interpolate};

use crate::error::ApiError;

/// The freq-axis DTO is fully derived from immutable `envi_engine::freq`
/// constants, so it is built exactly once (three heap allocations) at first use
/// and served from this shared instance on every request. Still read from the
/// engine constants at init — never hardcoded (Pitfall 9).
static FREQ_AXIS_DTO: LazyLock<FreqAxisDto> = LazyLock::new(FreqAxisDto::from_engine);

/// The frequency-axis wire payload, built at runtime from the engine constants.
///
/// `centres_hz` and `nominal_third_octave_hz` are the only Hz values that ever
/// cross the network; they are display/charting data, never keys. Spectra are
/// keyed by position in the 105-point grid (`centres_hz[i]` is band index `i`).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
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

/// Handler: serve the memoized frequency axis as JSON.
pub async fn freq_axis() -> Json<FreqAxisDto> {
    Json(FREQ_AXIS_DTO.clone())
}

/// Request body for `POST /meta/interpolate-spectrum`: an authored coarse
/// spectrum to expand onto the dense 1/12-octave band-index grid (D-05).
///
/// `values.len()` MUST match `resolution`'s anchor count (Octave 9 / Third 27 /
/// Twelfth 105); any other length, a non-finite value, or a value the engine's
/// `[0, MAX_R_DB]` range gate rejects surfaces as a structured `4xx`.
/// `deny_unknown_fields` (request-facing DTO) so a typo'd key is a loud 4xx, not
/// a silently-ignored field.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct InterpolateReq {
    /// The authoring resolution the `values` anchors were drawn at.
    pub resolution: Resolution,
    /// The per-anchor sound-reduction index `R` (dB) at `resolution`'s grid
    /// anchors; length must equal the resolution's anchor count.
    pub values: Vec<f64>,
}

/// Response body for `POST /meta/interpolate-spectrum`: the dense `[105]`
/// band-index grid (`r_db[i]` is band index `i`, never nominal Hz).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct InterpolateResp {
    /// The dense sound-reduction spectrum, one value per 1/12-octave band index
    /// `0..=104`.
    pub r_db: Vec<f64>,
}

/// Handler: expand an authored coarse spectrum onto the dense `[105]` grid (D-05).
///
/// Delegates the band-index interpolation to the single shared
/// [`envi_store::interpolate::interpolate`] core (no second impl in the service —
/// SVC-07), then runs the dense grid through [`IsolationSpectrum::new`], the sole
/// `[0, MAX_R_DB]` range authority. Length/finiteness faults surface from the
/// store as [`crate::error::ApiError::BadRequest`] (via `From<StoreError>`); an
/// out-of-range `R` is rejected by the engine constructor as a `4xx`, never
/// silently clamped (07-01 removed the clamp from `interpolate` for exactly this).
///
/// # Errors
/// - `400` if `values.len()` mismatches the resolution, any value is non-finite,
///   or any value falls outside the engine's `[0, MAX_R_DB]` range.
pub async fn interpolate_spectrum(
    Json(req): Json<InterpolateReq>,
) -> Result<Json<InterpolateResp>, ApiError> {
    // Shared band-index core (D-05): length + finiteness gate, no math inlined here.
    let dense = interpolate(req.resolution, &req.values)?;
    // Range gate (SVC-07: the engine owns [0, MAX_R_DB], not the service). An
    // out-of-range value reaches new() unclamped and is rejected as a 4xx.
    let spectrum = IsolationSpectrum::new(dense).map_err(|e| ApiError::BadRequest {
        detail: e.to_string(),
    })?;
    Ok(Json(InterpolateResp {
        r_db: spectrum.as_bands().to_vec(),
    }))
}
