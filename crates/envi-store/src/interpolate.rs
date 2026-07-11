//! Re-export shim: the band-index interpolation core moved to `envi-compute`
//! (10-06) so the browser solve boundary can share it without dragging
//! `std::fs`/`tempfile` into wasm. `envi-store` re-exports [`Resolution`] and a
//! thin [`interpolate`] wrapper that maps the compute-side error back into
//! [`StoreError`], so this crate's public API (`envi_store::interpolate::{…}`)
//! and every server call site stay source-compatible, exactly as before the move.
//!
//! The single-interpolation-core guarantee (D-05) is unchanged: there is still
//! ONE implementation — now in `envi_compute::interpolate` — that the read-path
//! `r_db[105]` derivation, the Phase-7 interpolation endpoint, and `PUT /scene`
//! validation all consume, so they cannot drift apart.

use envi_engine::freq::N_BANDS;

pub use envi_compute::interpolate::Resolution;

use crate::StoreError;

/// Interpolate an authored coarse spectrum onto the dense `[105]` band-index
/// grid (the moved [`envi_compute::interpolate::interpolate`] core), mapping its
/// [`envi_compute::scene_dto::SceneDtoError`] into a [`StoreError`] so server
/// call sites keep using `?` into `StoreError` unchanged.
///
/// # Errors
/// - [`StoreError::BadBandCount`] if `values.len()` ≠ the resolution's anchor
///   count.
/// - [`StoreError::NonFinite`] if any authored value is NaN or `±∞`.
pub fn interpolate(resolution: Resolution, values: &[f64]) -> Result<[f64; N_BANDS], StoreError> {
    envi_compute::interpolate::interpolate(resolution, values).map_err(StoreError::from)
}
