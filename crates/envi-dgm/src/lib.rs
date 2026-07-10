//! # envi-dgm
//!
//! The milestone's **server-side digital-ground-model boundary** (D-08): a
//! pure-Rust, constrained-Delaunay TIN built with `spade` from user-drawn
//! `elevation_point` vertices and `elevation_line` breaklines, with barycentric
//! Z interpolation. It turns an untrusted scatter of points + polylines into a
//! queryable terrain surface (SC1: elevation points/lines "re-triangulate the
//! DGM"). No terrain import and no C `gdal` here — Phase 8 later extends the
//! SAME seam by feeding imported samples as additional vertices.
//!
//! # Boundary statement (D-08)
//!
//! `spade` lives **HERE and nowhere else**. This crate does **not** depend on
//! `envi-engine`, so `spade` can never reach the engine's 3-dependency
//! quarantine (`ndarray` + `num-complex` + `thiserror`). `spade` is pure Rust —
//! zero C toolchain (no `gdal`/`proj`/`proj-sys`), zero I/O, and no serde in the
//! runtime graph. Callers pass plain `[f64; 3]` / `[f64; 2]` at the boundary.
//!
//! # Panic safety (07-RESEARCH Pitfall 3 — load-bearing)
//!
//! `spade`'s `add_constraint_edges` **panics** on a breakline whose interior
//! intersects an already-inserted constraint. A panic inside an axum handler
//! thread is a real defect, so [`tin::build_tin`] pre-checks every breakline
//! segment (`can_add_constraint`) and returns a typed
//! [`DgmError::IntersectingConstraint`] instead — a 4xx-mappable client fault,
//! never a thread abort. Degenerate input (0/1/2 points, all-collinear,
//! non-finite, or above the DoS cap) is likewise rejected with a typed error.
//!
//! # House rules
//! - `f64` throughout; typed errors ([`DgmError`]), never panics on data.
#![deny(unsafe_code)]

pub mod tin;

use thiserror::Error;

/// Errors from the DGM boundary out of untrusted elevation data.
///
/// Elevation points and breaklines arrive from HTTP bodies (via `envi-service`);
/// every malformed or hostile input yields one of these — the boundary never
/// panics on data (threats T-07-02-01/02/03). Struct variants carry the
/// offending values so callers and tests can report what went wrong.
///
/// `PartialEq` is derived so tests can `assert_eq!` / `matches!` on variants;
/// `spade` insertion errors are wrapped as message strings so the equality
/// holds (mirroring `envi_geo::GeoError::Proj`).
#[derive(Debug, Error, PartialEq)]
pub enum DgmError {
    /// Fewer than 3 distinct, non-collinear points were supplied. A TIN needs at
    /// least one triangle; 0/1/2 points or an all-collinear set produce no
    /// faces and cannot interpolate.
    #[error("degenerate TIN: {got} distinct point(s) yield no triangle (need >= 3 non-collinear)")]
    TooFewPoints {
        /// Number of distinct points found (after `spade` dedup). A value >= 3
        /// here means the points were all collinear, so no triangle formed.
        got: usize,
    },
    /// Two breakline segments cross in their interior. `spade`'s
    /// `add_constraint_edges` would panic here, so the offending pair is
    /// rejected up front (Pitfall 3).
    #[error("breaklines intersect in their interior: segment {a:?} crosses {b:?}")]
    IntersectingConstraint {
        /// One endpoint of the segment that could not be added.
        a: [f64; 2],
        /// The other endpoint of the segment that could not be added.
        b: [f64; 2],
    },
    /// A coordinate component was NaN or infinite.
    #[error("non-finite coordinate: {what}")]
    NonFinite {
        /// What the offending value was.
        what: String,
    },
    /// The point or breakline-vertex count exceeded the documented DoS bound
    /// before triangulation (threat T-07-02-02).
    #[error("input too large: {got} {kind}, limit is {limit}")]
    TooLarge {
        /// What was counted (`"points"` or `"breakline vertices"`).
        kind: &'static str,
        /// The count that exceeded the cap.
        got: usize,
        /// The documented maximum.
        limit: usize,
    },
    /// `spade` failed to insert a vertex or constraint. The underlying error is
    /// captured as a message so `DgmError` stays `PartialEq`.
    #[error("triangulation error: {message}")]
    Triangulation {
        /// Human-readable `spade` error text.
        message: String,
    },
}
