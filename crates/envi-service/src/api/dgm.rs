//! `POST /api/v1/dgm/triangulate` — the server-side digital-ground-model seam
//! (D-08). A thin wrapper over `envi_dgm::tin::build_tin`; NO geometry math lives
//! here (SVC-07 — the TIN crate owns constrained-Delaunay triangulation, this
//! layer only shuttles JSON and maps typed errors).
//!
//! # Statelessness (07-RESEARCH Open Question 3)
//!
//! The endpoint is **stateless**: it triangulates the supplied points/breaklines
//! and returns the mesh in the response — no TIN is persisted this phase. The
//! authored `elevation_point` / `elevation_line` scene features are the persisted
//! truth; the TIN is a derived, recomputable surface.
//!
//! # Panic safety (Pitfall 3 — load-bearing)
//!
//! `build_tin` pre-checks every breakline segment (`can_add_constraint`) so
//! `spade` never panics on an interior-crossing constraint; the resulting
//! [`envi_dgm::DgmError`] maps to a `400` via `From<DgmError> for ApiError`.
//! This handler therefore never `unwrap()`s / `panic!`s on request data — a
//! hostile body yields a structured `4xx`, never a thread abort or a `500`.

use axum::Json;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use envi_dgm::tin::build_tin;

use crate::error::ApiError;

/// Request body for `POST /dgm/triangulate`: scattered elevation points plus
/// breakline polylines to force as constrained edges (D-08).
///
/// `points` are `[x, y, z]` in scene meters; `breaklines` are polylines of
/// `[x, y]` vertices (Z is sampled from the point surface inside the TIN, never
/// authored). `deny_unknown_fields` (request-facing) so a typo'd key is a loud
/// `4xx`. Count / finiteness / crossing bounds are enforced by `build_tin`, not
/// re-implemented here (SVC-07).
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct DgmReq {
    /// Elevation points `[x, y, z]` (meters).
    pub points: Vec<[f64; 3]>,
    /// Breakline polylines, each a list of `[x, y]` vertices (meters). Optional;
    /// defaults to none when omitted.
    #[serde(default)]
    pub breaklines: Vec<Vec<[f64; 2]>>,
}

/// Response body for `POST /dgm/triangulate`: a self-contained, renderable mesh.
///
/// `vertices` are `[x, y, z]` triples in TIN vertex-index order (elevation points
/// plus breakline vertices with their sampled Z); each entry of `triangles` is a
/// vertex-index triple into `vertices`.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct DgmResp {
    /// Distinct surface vertices `[x, y, z]`, in vertex-index order.
    pub vertices: Vec<[f64; 3]>,
    /// Inner triangles as vertex-index triples into `vertices`.
    pub triangles: Vec<[usize; 3]>,
}

/// Handler: build a constrained-Delaunay TIN and return its mesh (D-08).
///
/// Delegates entirely to [`envi_dgm::tin::build_tin`] (no triangulation math in
/// the service — SVC-07); every typed [`envi_dgm::DgmError`] maps to a `400` via
/// `From<DgmError>`, so degenerate, non-finite, oversized, or interior-crossing
/// input is a structured client fault, never a `500`/panic (Pitfall 3).
///
/// # Errors
/// - `400` for degenerate (fewer than 3 non-collinear points), non-finite,
///   oversized, or interior-crossing-breakline input.
pub async fn triangulate(Json(req): Json<DgmReq>) -> Result<Json<DgmResp>, ApiError> {
    let tin = build_tin(&req.points, &req.breaklines)?;
    Ok(Json(DgmResp {
        vertices: tin.vertices(),
        triangles: tin.triangles(),
    }))
}
