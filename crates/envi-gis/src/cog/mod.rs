//! Cloud-Optimized GeoTIFF decode core (sans-I/O).
//!
//! # Module I/O
//! - **Inputs:** whole cached COG/BigTIFF tile bytes (`&[u8]`), fetched and
//!   cached by the TS orchestrator; a source-CRS window; a decoded-pixel budget.
//! - **Output:** an `f32` [`window::Raster`] windowed from the tile, with the
//!   geotransform derived from the IFD geo-tags and nodata/edge samples dropped.
//! - **Invariant (load-bearing):** decode is guard-first and never panics on
//!   data — the [`MAX_DECODED_PX`] budget is enforced from IFD dimensions BEFORE
//!   any pixel is decoded (threat T-08-02-01).
//!
//! Submodules: [`header`] (IFD/BigTIFF parse + overview navigation + IFD-chain
//! cap), [`geo_tags`] (geotransform + nodata), [`window`] (`decode_window`).

pub mod geo_tags;
pub mod header;
pub mod window;

pub use window::{PixelWindow, Raster, decode_window};

/// Maximum decoded pixel count accepted per window (DoS budget, threat
/// T-08-02-01). Enforced from IFD dimensions *before* any decode allocates the
/// output raster — the exact analog of `envi_dgm::tin::MAX_POINTS`. ~64 Mpx of
/// `f32` is ~256 MB, the browser-tab budget from 08-RESEARCH Pitfall 4; over
/// budget, callers pick a coarser overview rather than allocating.
pub const MAX_DECODED_PX: usize = 64 * 1024 * 1024;
