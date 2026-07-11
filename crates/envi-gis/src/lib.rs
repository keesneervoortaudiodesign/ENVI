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
pub mod era5;
mod geojson_util;
pub mod grid;
pub mod impedance;
pub mod impedance_table;
pub mod landcover;
pub mod merge;
pub mod path;
pub mod profile;
pub mod provenance;
pub mod registry;
pub mod screening;
pub mod terrain;
pub mod tiles;
pub mod weather;

use thiserror::Error;

/// Minimum strictly-ascending-x separation between spliced/kept cut-profile
/// vertices (meters). Shared by [`profile`], [`impedance`], and [`screening`] so a
/// crossing/screen top that lands on an existing vertex is absorbed rather than
/// duplicating x — the three stages MUST agree on this epsilon (they compose on one
/// `(x, z)` frame), so it lives here as a single `pub(crate)` source of truth.
pub(crate) const X_EPSILON_M: f64 = 1e-6;

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
    /// A cut-profile sample's planar point fell outside the DGM TIN convex hull
    /// (`Tin::interpolate_z` returned `None`). Surfaced rather than defaulting a
    /// fabricated `0.0` elevation (GEOX-01, D-07, threat T-09-01-02).
    #[error("cut-profile sample fell outside the TIN hull (no fabricated 0.0)")]
    OutsideHull,
    /// The requested cut-profile would exceed the [`profile::MAX_PROFILE_POINTS`]
    /// sample cap — a pathological tiny-step / huge-distance request, rejected
    /// before allocation (GEOX-01, threat T-09-01-01, mirrors
    /// `terrain::MAX_TERRAIN_POINTS`).
    #[error("cut-profile too long: {got} sample points exceeds the cap of {limit}")]
    ProfileTooLong {
        /// Sample-point count the request would produce.
        got: usize,
        /// The documented maximum ([`profile::MAX_PROFILE_POINTS`]).
        limit: usize,
    },
    /// A cut-profile could not be built into a valid multi-point section: a
    /// non-positive sampling step, a zero-length (coincident source/receiver)
    /// path, or fewer than two strictly-ascending points (GEOX-01).
    #[error("degenerate cut-profile: {what}")]
    DegenerateProfile {
        /// What made the profile degenerate.
        what: String,
    },
    /// A ground-impedance class letter did not resolve to a flow resistivity σ
    /// through `envi_engine::scene::impedance_class` (expected `A..=H`). Surfaced
    /// rather than defaulting a fabricated σ (GEOX-02, threat T-09-01-03) — σ is
    /// resolved ONLY through the engine, never restated as a literal here.
    #[error(
        "ground impedance class {class:?} does not resolve through the engine (expected A..=H)"
    )]
    UnresolvableClass {
        /// The offending class letter.
        class: char,
    },
    /// The screening corridor query returned more candidate objects than the
    /// [`screening::MAX_CORRIDOR_CANDIDATES`] cap. Rejected **before** any
    /// per-candidate cut-plane ∩ prism work so a pathological geometry set cannot
    /// exhaust CPU/memory (GEOX-03, threat T-09-02-01).
    #[error("screening corridor selected {got} candidates, exceeding the cap of {limit}")]
    CorridorCandidatesExceeded {
        /// Candidate count the corridor query returned.
        got: usize,
        /// The documented maximum ([`screening::MAX_CORRIDOR_CANDIDATES`]).
        limit: usize,
    },
    /// The requested receiver-grid spacing was below the
    /// [`grid::MIN_SPACING_M`] guardrail (or was non-finite). A finer grid
    /// explodes receiver count × sub-sources on the client-side WASM solve, so it
    /// is rejected before any lattice is generated (GRID-01, D-06, threat
    /// T-09-02-02).
    #[error("receiver-grid spacing {got} m is below the minimum of {min} m")]
    SpacingTooSmall {
        /// The requested spacing (meters).
        got: f64,
        /// The documented minimum ([`grid::MIN_SPACING_M`]).
        min: f64,
    },
    /// The receiver grid would contain more points than the
    /// [`grid::MAX_RECEIVERS`] cap. Enforced against the bounding-box lattice
    /// count **before** the full set is allocated so the browser cannot OOM
    /// (GRID-01, threat T-09-02-02).
    #[error("receiver grid would produce {got} receivers, exceeding the cap of {limit}")]
    ReceiverCapExceeded {
        /// Receiver count the request would produce (upper bound).
        got: usize,
        /// The documented maximum ([`grid::MAX_RECEIVERS`]).
        limit: usize,
    },
    /// Building-aware receiver-grid construction failed the constrained-Delaunay
    /// validity guard: `envi_dgm::build_tin` rejected intersecting/degenerate
    /// footprint rings or the `calc_area` boundary (GRID-01, threat T-09-02-03).
    /// The underlying `DgmError` is captured as a message so [`GisError`] stays
    /// `PartialEq` (mirroring [`GisError::Tiff`]/[`GisError::Reproject`]).
    #[error("receiver-grid region is invalid: {message}")]
    InvalidGridRegion {
        /// Human-readable `envi_dgm::DgmError` text.
        message: String,
    },
    /// The weather-route log-lin least-squares fit could not be solved: fewer
    /// than three matched samples, mismatched lengths, or a singular normal
    /// matrix (collinear heights). A typed error, never a panic or a NaN profile
    /// (METX-01, threat T-09-03-01). `envi-harness` Route 2/3 delegate to the
    /// single [`weather::fit_profile`] and map this back to their `CaseLoadError`.
    #[error("weather-route profile fit failed: {message}")]
    WeatherFit {
        /// What made the fit unsolvable.
        message: String,
    },
    /// An ERA5 single-level field was non-finite or physically degenerate (zero
    /// friction velocity, non-positive air density / virtual temperature), so the
    /// Obukhov derivation could not proceed. A typed error, never a panic
    /// (METX-02, threat T-09-03-02).
    #[error("ERA5 field error: {message}")]
    Era5Field {
        /// What made the ERA5 derivation fail.
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
