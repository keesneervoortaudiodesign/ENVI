//! Cloud-Optimized GeoTIFF decode core (sans-I/O), with **windowed range reads**.
//!
//! # Module I/O
//! - **Inputs:** COG/BigTIFF tile bytes fetched and cached by the TS orchestrator —
//!   either the whole file, or (the normal path) just the header prefix plus the
//!   chunk byte ranges this module's [`plan`] selected; a source-CRS window; a
//!   decoded-pixel budget.
//! - **Output:** [`plan::plan_window_reads`] returns the [`ByteRange`]s to fetch
//!   ("which bytes", never the fetch itself — TS owns `fetch`/OPFS), and
//!   [`window::decode_window_cog`] turns those fetched ranges into an `f32`
//!   [`window::Raster`] with the geotransform derived from the IFD geo-tags and
//!   nodata/edge samples dropped.
//! - **Invariant (load-bearing):** decode is guard-first and never panics on
//!   data — the [`MAX_DECODED_PX`] budget is enforced from IFD dimensions BEFORE
//!   any pixel is decoded (threat T-08-02-01), and its network sibling
//!   [`plan::DEFAULT_MAX_FETCH_BYTES`] is enforced before any range is emitted.
//! - **Invariant (load-bearing):** there is **no whole-file fallback**. A COG is
//!   read by planning its overlapping tiles and fetching only those; a byte that
//!   was not planned is an error at the sparse reader, never a silent zero.
//!
//! Submodules: [`header`] (IFD/BigTIFF parse + the raw IFD walk + overview
//! navigation + IFD-chain cap), [`geo_tags`] (geotransform + nodata), [`sparse`]
//! (the partially-fetched byte view), [`plan`] (window → byte ranges), [`window`]
//! (`decode_window`).

pub mod geo_tags;
pub mod header;
pub mod plan;
pub mod sparse;
pub mod window;

pub use plan::{
    COALESCE_GAP_BYTES, DEFAULT_HEADER_PREFIX_BYTES, DEFAULT_MAX_FETCH_BYTES, MAX_HEADER_BYTES,
    ReadPlan, plan_window_reads,
};
pub use sparse::{BytePart, ByteRange, CogBytes};
pub use window::{
    PixelWindow, Raster, decode_window, decode_window_cog, decode_window_u8, decode_window_u8_cog,
};

/// Maximum decoded pixel count accepted per window (DoS budget, threat
/// T-08-02-01). Enforced from IFD dimensions *before* any decode allocates the
/// output raster — the exact analog of `envi_dgm::tin::MAX_POINTS`. ~64 Mpx of
/// `f32` is ~256 MB, the browser-tab budget from 08-RESEARCH Pitfall 4; over
/// budget, callers pick a coarser overview rather than allocating.
pub const MAX_DECODED_PX: usize = 64 * 1024 * 1024;
