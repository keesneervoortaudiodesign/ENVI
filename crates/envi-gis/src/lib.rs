//! # envi-gis
//!
//! The milestone's **client-side GIS-ingestion boundary** (Phase-8 pivot
//! DATA-01..04): a pure-Rust, sans-I/O core that decodes cached COG/BigTIFF
//! terrain tiles into `f32` rasters, and (in later plans) maps land-cover to
//! impedance and OSM JSON to building features. It turns untrusted remote GIS
//! bytes — already fetched and cached by the TypeScript orchestrator — into
//! scene-ready geometry.
//!
//! # Boundary statement (08-CONTEXT D-01/D-02/D-03, 08-RESEARCH Pattern 1)
//!
//! This crate is **sans-I/O and WASM-safe**. It performs **no network access, no
//! OPFS access, and imports no `web-sys`**; its entire public API is
//! **synchronous over byte slices** (`&[u8]`). TypeScript owns every side effect
//! — `fetch`, OPFS read/write, `crypto.randomUUID()` — and hands this core the
//! bytes to parse. The consequence is honesty: the whole ingestion path runs
//! under native `cargo test` against committed GDAL-generated fixtures, exactly
//! like the engine's oracle fixtures, and Playwright only ever mocks plain
//! `fetch`. Reprojection goes through [`envi_geo`] and nowhere else (GEOX-04's
//! single reprojection boundary) — this crate adds no second `proj4rs` edge.
//!
//! # Security posture (untrusted bytes — load-bearing)
//!
//! Cached tile bytes are **untrusted** (proxied or direct remote COGs decoded in
//! the browser's memory). The decode path is guard-first, mirroring
//! `envi_dgm::tin`: a decompression bomb is rejected by [`cog::MAX_DECODED_PX`]
//! computed from the IFD dimensions **before** any pixel is decoded
//! (threat T-08-02-01); a malicious IFD chain is capped (T-08-02-02); nodata
//! sentinels and edge-tile padding are dropped, never fed downstream as real
//! elevations (T-08-02-03); the geotransform is always derived from the
//! `ModelPixelScale`/`ModelTiepoint` tags, never assumed from nominal pixel
//! counts (T-08-02-04). Every `tiff::TiffError` becomes a typed [`GisError`] —
//! this crate never panics on data.
//!
//! # House rules
//! - `f64`/`f32` numerics as the format dictates; typed errors ([`GisError`]),
//!   never `unwrap()`/`panic!` on a data path.
//! - Foreign errors ([`tiff::TiffError`]) are wrapped as message strings so
//!   [`GisError`] stays `PartialEq` (mirroring `DgmError::Triangulation` /
//!   `GeoError::Proj`).
#![deny(unsafe_code)]

pub mod buildings;
pub mod cog;
pub mod impedance_table;
pub mod merge;
pub mod provenance;
pub mod registry;
pub mod terrain;

use thiserror::Error;

/// Errors from the GIS-ingestion boundary out of untrusted tile bytes.
///
/// Cached COG bytes arrive from the TS orchestrator (fetched from PDOK / the
/// proxy). Every malformed, hostile, or unsupported input yields one of these —
/// the boundary never panics on data (threats T-08-02-01..04). Struct variants
/// carry the offending values so callers and tests can report what went wrong.
///
/// `PartialEq` is derived so tests can `assert_eq!` / `matches!` on variants;
/// [`tiff::TiffError`] is wrapped as a message string so the equality holds
/// (mirroring `envi_dgm::DgmError::Triangulation` and `envi_geo::GeoError::Proj`).
#[derive(Debug, Error, PartialEq)]
pub enum GisError {
    /// A decompression bomb was rejected: the decoded pixel count implied by the
    /// IFD dimensions exceeds the [`cog::MAX_DECODED_PX`] budget. Enforced
    /// **before** any decode allocates the output raster (threat T-08-02-01).
    #[error("decoded-pixel budget exceeded: window implies {requested_px} px, limit is {limit} px")]
    DecodeBudgetExceeded {
        /// Pixel count the requested window/tile would decode to.
        requested_px: usize,
        /// The documented maximum ([`cog::MAX_DECODED_PX`]).
        limit: usize,
    },
    /// The IFD chain (overview pyramid) carried more images than the cap. Guards
    /// a malicious/cyclic IFD chain (threat T-08-02-02).
    #[error("too many IFD images: {got} exceeds the cap of {limit}")]
    TooManyImages {
        /// Number of images walked before the cap tripped.
        got: usize,
        /// The documented maximum.
        limit: usize,
    },
    /// A required GeoTIFF tag was absent, so the geotransform could not be built.
    /// Never assume nominal pixel geometry (threat T-08-02-04, Pitfall 5).
    #[error("missing GeoTIFF tag: {tag}")]
    MissingGeoTag {
        /// The absent tag name (e.g. `"ModelPixelScale"`).
        tag: &'static str,
    },
    /// A GeoTIFF tag was present but malformed (wrong arity, zero or non-finite
    /// pixel scale, ...), so the geotransform would be degenerate.
    #[error("invalid GeoTIFF geotransform: {what}")]
    InvalidGeoTransform {
        /// What was wrong with the tag values.
        what: String,
    },
    /// The requested window does not intersect the tile's image bounds, or has
    /// non-positive extent — never silently return an empty/zero raster.
    #[error("window out of bounds: {what}")]
    WindowOutOfBounds {
        /// Description of the offending window vs image bounds.
        what: String,
    },
    /// The decoded chunk was not the expected `f32` sample format (terrain COGs
    /// are float32). Reported rather than silently reinterpreted.
    #[error("unexpected sample format: expected f32, got {got}")]
    UnexpectedSampleFormat {
        /// The `DecodingResult` variant actually returned by the decoder.
        got: String,
    },
    /// A tag value or nodata sentinel was NaN/infinite where a finite number was
    /// required.
    #[error("non-finite value: {what}")]
    NonFinite {
        /// What the offending value was.
        what: String,
    },
    /// The `tiff` crate failed to parse or decode the bytes. The underlying error
    /// is captured as a message so [`GisError`] stays `PartialEq`.
    #[error("tiff decode error: {message}")]
    Tiff {
        /// Human-readable `tiff` error text.
        message: String,
    },
    /// Reprojection through [`envi_geo`] failed (the single reprojection boundary,
    /// GEOX-04). The underlying `GeoError` is captured as a message so [`GisError`]
    /// stays `PartialEq` (mirroring [`GisError::Tiff`]).
    #[error("reprojection error: {message}")]
    Reproject {
        /// Human-readable `envi_geo::GeoError` text.
        message: String,
    },
    /// Untrusted third-party JSON (Overpass) failed to parse, or a required field
    /// was malformed. The parser error is captured as a message.
    #[error("json parse error: {message}")]
    Json {
        /// Human-readable parse error text.
        message: String,
    },
}

impl From<tiff::TiffError> for GisError {
    /// Wrap any `tiff` crate error as [`GisError::Tiff`] so decode paths can use
    /// `?` while keeping `GisError: PartialEq` (the error text, not the foreign
    /// non-`PartialEq` type, is stored).
    fn from(e: tiff::TiffError) -> Self {
        GisError::Tiff {
            message: e.to_string(),
        }
    }
}
