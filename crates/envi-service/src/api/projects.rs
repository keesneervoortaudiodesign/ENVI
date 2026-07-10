//! Project CRUD + duplicate + reopen-last handlers (SVC-05, D-06).
//!
//! Every handler is a **thin delegate** onto `envi_store::ProjectStore`: parse +
//! validate the request, call the store, map the result to a status. No storage
//! logic lives here (D-06). Every id-bearing route uses a `Path<Uuid>` extractor
//! — the parse IS the path-traversal gate (Pitfall 7); no handler takes a raw
//! `Path<String>` id.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use ts_rs::TS;
use uuid::Uuid;

use envi_geo::LonLat;
use envi_store::dto::{ProjectMetaDto, SettingsDto};
use envi_store::now_unix;

use crate::error::ApiError;
use crate::state::AppState;

/// A WGS84 origin for a new project — the point whose UTM zone gets pinned (D-03).
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct OriginDto {
    /// Longitude, degrees east.
    pub lon_deg: f64,
    /// Latitude, degrees north.
    pub lat_deg: f64,
}

/// `POST /projects` body. Strict (`deny_unknown_fields`) so client drift is loud.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct CreateProjectRequest {
    /// Human-readable project name.
    pub name: String,
    /// Optional free-text description (defaults to empty).
    #[serde(default)]
    pub description: Option<String>,
    /// The WGS84 origin whose UTM zone is pinned at creation.
    pub origin: OriginDto,
}

/// `PUT /projects/{id}` body — metadata/settings patch. All fields optional;
/// absent fields are left unchanged. Strict against unknown fields.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct UpdateProjectRequest {
    /// New name (unchanged if absent).
    #[serde(default)]
    pub name: Option<String>,
    /// New description (unchanged if absent).
    #[serde(default)]
    pub description: Option<String>,
    /// New settings (unchanged if absent).
    #[serde(default)]
    pub settings: Option<SettingsDto>,
}

/// `GET /projects` -> 200 `[ProjectMetaDto]` (metadata of every stored project).
///
/// Resilient by design (MED-2): a single malformed/unreadable `project.json`
/// (e.g. a hand-edit typo — D-04 projects are human-editable flat files) is
/// skipped and logged, never failing the whole listing. Every project that loads
/// is returned.
pub async fn list(State(app): State<Arc<AppState>>) -> Result<Json<Vec<ProjectMetaDto>>, ApiError> {
    let ids = app.store.list()?;
    let mut metas = Vec::with_capacity(ids.len());
    for id in ids {
        match app.store.load_meta(id) {
            Ok(meta) => metas.push(meta),
            Err(e) => tracing::warn!(%id, error = %e, "skipping unreadable project in list"),
        }
    }
    Ok(Json(metas))
}

/// `POST /projects` -> 201 `ProjectMetaDto`. The store pins the UTM CRS from the
/// origin (D-03); the response carries the pinned `CrsDto` (zone/hemisphere).
pub async fn create(
    State(app): State<Arc<AppState>>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectMetaDto>), ApiError> {
    let origin = LonLat {
        lon_deg: req.origin.lon_deg,
        lat_deg: req.origin.lat_deg,
    };
    let description = req.description.unwrap_or_default();
    let meta = app.store.create(&req.name, &description, origin)?;
    Ok((StatusCode::CREATED, Json(meta)))
}

/// `GET /projects/last` -> 200 the last-opened project's metadata, or 404 when
/// there is no record (or it was deleted — reopen-last, D-06).
///
/// Registered BEFORE `/projects/{id}` so the literal `last` segment is never
/// captured as a uuid path param.
pub async fn last(State(app): State<Arc<AppState>>) -> Result<Json<ProjectMetaDto>, ApiError> {
    let id = app.store.last_opened()?.ok_or(ApiError::NotFound)?;
    let meta = app.store.load_meta(id)?;
    Ok(Json(meta))
}

/// `GET /projects/{id}` -> 200 metadata; opening records reopen-last (D-06).
pub async fn get(
    State(app): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProjectMetaDto>, ApiError> {
    Ok(Json(app.store.open(id)?))
}

/// `PUT /projects/{id}` -> 200. Patch name/description/settings and bump
/// `modified_at`. Returns the updated metadata.
pub async fn update(
    State(app): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateProjectRequest>,
) -> Result<Json<ProjectMetaDto>, ApiError> {
    let mut meta = app.store.load_meta(id)?;
    if let Some(name) = req.name {
        meta.name = name;
    }
    if let Some(description) = req.description {
        meta.description = description;
    }
    if let Some(settings) = req.settings {
        meta.settings = settings;
    }
    meta.modified_at_unix = now_unix();
    app.store.save_meta(&meta)?;
    Ok(Json(meta))
}

/// `DELETE /projects/{id}` -> 204.
///
/// Evicts the deleted project's in-memory `CalcRecord`s (LOW-8): a calc whose
/// project folder is gone must not linger in the registry, and its now-orphaned
/// `calc_id` must not resolve to a project that no longer exists.
pub async fn delete(
    State(app): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    app.store.delete(id)?;
    app.calcs
        .write()
        .await
        .retain(|_, rec| rec.project_id != id);
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /projects/{id}/duplicate` -> 201 the new `ProjectMetaDto` (the copy
/// excludes `calc/` so stale tensor identity never travels).
pub async fn duplicate(
    State(app): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<ProjectMetaDto>), ApiError> {
    let meta = app.store.duplicate(id)?;
    Ok((StatusCode::CREATED, Json(meta)))
}
