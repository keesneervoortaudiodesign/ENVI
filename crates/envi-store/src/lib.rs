//! # envi-store
//!
//! The persistence half of the ENVI service: the **serde DTO mirror** (D-05) and
//! the **project-as-folder** flat-file storage layer (D-04), plus the frozen
//! tensor-identity content hash (D-07).
//!
//! # The serde quarantine boundary (D-05, Anti-Pattern 1)
//!
//! Serde lives **HERE**, never in `envi-engine`. Every engine scene type is
//! twinned by a serde DTO in [`dto`]; DTO -> engine conversion goes through the
//! engine's *validating constructors* (`BandSpectrum::from_values`,
//! `TerrainProfile::new`) via `TryFrom` — the engine's private fields force this
//! path, which is exactly what keeps the three-dep engine quarantine
//! (`ndarray` + `num-complex` + `thiserror`) byte-identical.
//!
//! # The flat-file ethos (D-04)
//!
//! A project **is a folder**, not a SQLite database:
//! `projects/<uuid>/project.json` (metadata + settings + the pinned UTM CRS) +
//! `scene.geojson` (an RFC 7946 FeatureCollection persisted **in WGS84**) +
//! `calc/<calc_id>/manifest.json` (dims `[S, R, 105]`, receiver-axis chunk
//! layout, content hashes, honest `stub` provenance; `tensor/` + `pincoh/` dirs
//! reserved for Phases 9/10). Git-diffable, human-inspectable, copyable. SQLite
//! is the documented upgrade path only — the DTO mirror keeps that swap
//! mechanical.
//!
//! # One reprojection seam (GEOX-04)
//!
//! WGS84 <-> project UTM reprojection happens exclusively through
//! `envi_geo::ProjectCrs` in [`geojson::scene_to_engine`]. This crate never
//! calls the underlying pure-Rust projection library directly — the
//! `envi-geo` seam is the only reprojection boundary in the milestone.
//!
//! # House rules
//! - `f64` throughout; typed errors ([`StoreError`]), never panics on data.
//! - Every file mutation is atomic (temp-in-dir + `sync_all` + `persist`).
//!
#![deny(unsafe_code)]

pub mod calibrate;
pub mod dto;
pub mod geojson;
pub mod hash;
pub mod interpolate;
pub mod manifest;
pub mod project_dir;

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

/// Current unix epoch seconds (wall clock; saturates at 0 before the epoch).
///
/// The single home for the store's timestamp convention, reused by the service
/// layer so `created_at`/`modified_at`/manifest timestamps agree byte-for-byte
/// (LOW-4: replaces three byte-identical private copies).
#[must_use]
pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Typed error for the persistence + DTO boundary (Archetype B — the I/O-crate
/// error that wraps sources and always carries the offending `PathBuf`).
///
/// Every malformed input — HTTP body, file on disk, untrusted id — yields one of
/// these; the store never panics on data (threat register T-06-02-01..06). No
/// `PartialEq` is derived because [`StoreError::Io`] wraps a non-`PartialEq`
/// `std::io::Error`; tests match variants with `matches!`.
#[derive(Debug, Error)]
pub enum StoreError {
    /// Filesystem error reading or writing a project file.
    #[error("I/O error at {path}: {source}")]
    Io {
        /// Offending path.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
    /// JSON syntax / schema error (serde_json), naming the file involved.
    #[error("JSON error in {path}: {message}")]
    Json {
        /// Offending path.
        path: PathBuf,
        /// Serde error text.
        message: String,
    },
    /// A GeoJSON document violated the scene schema (RFC 7946 or the ENVI
    /// `properties.kind` vocabulary).
    #[error("GeoJSON error: {message}")]
    GeoJson {
        /// Human-readable reason.
        message: String,
    },
    /// No project folder exists for this id.
    #[error("project not found: {project_id}")]
    NotFound {
        /// Requested project id.
        project_id: uuid::Uuid,
    },
    /// No calculation manifest exists for this calc id.
    #[error("calculation not found: {calc_id}")]
    CalcNotFound {
        /// Requested calc id.
        calc_id: uuid::Uuid,
    },
    /// A spectrum DTO was not exactly [`envi_engine::freq::N_BANDS`] long.
    #[error(
        "band spectrum has {got} values, expected {} (dense [105] by band index)",
        envi_engine::freq::N_BANDS
    )]
    BadBandCount {
        /// The wrong length received.
        got: usize,
    },
    /// A value that must be finite was NaN or infinite.
    #[error("non-finite value: {what}")]
    NonFinite {
        /// What the offending value was.
        what: String,
    },
    /// A GeoJSON feature carried a `properties.kind` outside the locked
    /// 9-kind vocabulary (loud rejection — catches vocabulary drift).
    #[error("unknown feature kind: {kind:?} (not in the ENVI 9-kind vocabulary)")]
    UnknownKind {
        /// The offending kind string.
        kind: String,
    },
    /// A GeoJSON feature of a known kind lacked a required property.
    #[error("feature of kind {kind:?} is missing required property {property:?}")]
    MissingProperty {
        /// The feature kind.
        kind: String,
        /// The missing property name.
        property: String,
    },
    /// A resolved project path escaped the store root (symlink/traversal guard).
    #[error("path escapes the store root: {path}")]
    PathEscape {
        /// The offending resolved path.
        path: PathBuf,
    },
    /// A reprojection failed at the `envi-geo` seam.
    #[error(transparent)]
    Geo(#[from] envi_geo::GeoError),
    /// An engine validating constructor rejected DTO input (e.g.
    /// `TerrainProfile::new` on a non-ascending profile).
    #[error("engine validation rejected DTO input: {message}")]
    Engine {
        /// The engine's `SceneError` message.
        message: String,
    },
}
