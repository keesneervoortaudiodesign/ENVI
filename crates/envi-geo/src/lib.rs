//! # envi-geo
//!
//! The milestone's **one reprojection boundary** (GEOX-04): pure-Rust WGS84
//! lon/lat <-> project-local UTM meters. This crate owns the `LonLat` / `SceneXY`
//! newtypes, auto-UTM-zone selection, and the `to_utm` / `to_wgs84` transforms.
//!
//! # Boundary statement (GEOX-04, D-01, D-03)
//!
//! There is **exactly one reprojection boundary** in the entire milestone and it
//! lives here. `envi-store`, `envi-service`, and later `envi-gis` (Phase 8) all
//! import this crate — **no other crate may call `proj4rs`**. Reprojection is
//! pure Rust on `proj4rs` (D-01): zero C toolchain, no `proj.db`, no `GDAL_DATA`
//! (the C `gdal`/`proj` stack is deferred to Phase 8 per D-02).
//!
//! proj4rs's one sharp edge — longlat coordinates are **radians** — is
//! quarantined inside [`transform`]: the public API speaks only **degrees**
//! ([`LonLat`]) and **meters** ([`SceneXY`]).
//!
//! # House rules
//! - `f64` throughout; typed errors ([`GeoError`]), never panics on data.
//! - `_deg` / `_m` suffixes carry the units (naming discipline, no units crate).
//!
#![deny(unsafe_code)]

pub mod crs;
pub mod transform;

use thiserror::Error;

// Re-export the public surface so callers use `envi_geo::ProjectCrs` etc.
// without submodule paths. `to_utm` / `to_wgs84` are inherent methods on
// `ProjectCrs` (defined in `transform`), reachable through this re-export.
pub use crs::{ProjectCrs, utm_zone_for};

/// WGS84 geographic coordinate, in **degrees**. The ONLY wire-facing coordinate
/// type — every coordinate that crosses the network (GeoJSON) is a `LonLat`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LonLat {
    /// Longitude, degrees east, valid range `[-180, 180]`.
    pub lon_deg: f64,
    /// Latitude, degrees north, valid range `[-90, 90]`.
    pub lat_deg: f64,
}

/// Project-local UTM coordinate, in **meters**. The ONLY scene-facing coordinate
/// type — every coordinate in engine/scene space is a `SceneXY`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneXY {
    /// Easting, meters (UTM false-easting 500 000 m applied).
    pub x_m: f64,
    /// Northing, meters (UTM false-northing 10 000 000 m applied in the south).
    pub y_m: f64,
}

/// Errors from the CRS boundary out of untrusted coordinate data.
///
/// Coordinates arrive from HTTP bodies and files (via `envi-store` /
/// `envi-service`); every malformed input yields one of these — the boundary
/// never panics on data (threats T-06-01-01/02/03). Struct variants carry the
/// offending values so callers and tests can report got/expected.
///
/// `PartialEq` is derived so tests can `assert_eq!` / `matches!` on variants;
/// proj4rs errors are wrapped as message strings so the equality holds.
#[derive(Debug, Error, PartialEq)]
pub enum GeoError {
    /// A lon/lat fell outside `[-180, 180]` x `[-90, 90]`.
    #[error(
        "lon/lat out of range: got ({lon}, {lat}), expected lon in [-180, 180], lat in [-90, 90]"
    )]
    LonLatOutOfRange {
        /// Offending longitude, degrees.
        lon: f64,
        /// Offending latitude, degrees.
        lat: f64,
    },
    /// A latitude fell outside the UTM domain (`lat` not in `[-80, 84]`). UTM /
    /// `etmerc` is undefined toward the poles (that band is UPS territory), where
    /// the projection silently produces increasingly distorted eastings/northings
    /// with no error. Rejected loudly rather than projected to garbage (LOW-1;
    /// consistent with the SC3 degree-magnitude loud-rejection style).
    #[error(
        "latitude {lat}° is outside the UTM domain (valid band is [-80, 84]; polar UPS is out of scope)"
    )]
    LatitudeOutsideUtm {
        /// Offending latitude, degrees.
        lat: f64,
    },
    /// A `SceneXY` had degree-magnitude components (`|x| <= 360` AND
    /// `|y| <= 90`) — almost certainly WGS84 degrees mislabeled as scene meters.
    /// Loudly rejected rather than silently reprojected to garbage (SC3).
    #[error(
        "degree-magnitude scene coordinate: got ({x}, {y}) m — valid UTM eastings are ~166_000..834_000 m, northings 0..10_000_000 m (SC3)"
    )]
    DegreeMagnitudeSceneCoord {
        /// Offending easting component, meters.
        x: f64,
        /// Offending northing component, meters.
        y: f64,
    },
    /// A coordinate component was NaN or infinite.
    #[error("non-finite value: {what}")]
    NonFinite {
        /// What the offending value was.
        what: String,
    },
    /// A longitude did not map into a valid UTM zone `1..=60`.
    #[error("no valid UTM zone for longitude {lon}")]
    BadZone {
        /// Offending longitude, degrees.
        lon: f64,
    },
    /// proj4rs failed to build a projection or transform a point. The underlying
    /// error is captured as a message so `GeoError` stays `PartialEq`.
    #[error("proj4rs error: {message}")]
    Proj {
        /// Human-readable proj4rs error text.
        message: String,
    },
}
