//! Scene GET/PUT handlers — WGS84 GeoJSON on the wire (SVC-05, SC1).
//!
//! The scene crosses the wire as an RFC 7946 FeatureCollection in **WGS84**
//! `[lon, lat]` (the browser never sees UTM meters — GEOX-04). PUT validates the
//! collection through `envi_store::geojson::validate_feature_collection`
//! (vocabulary + uuids + coordinate ranges) BEFORE persisting, so invalid scenes
//! never reach disk (the store's `save_scene` performs that validation). Handlers
//! stay thin: parse, delegate, map errors.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use geojson::FeatureCollection;
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;

/// `GET /projects/{id}/scene` -> 200 the persisted WGS84 FeatureCollection.
pub async fn get_scene(
    State(app): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<FeatureCollection>, ApiError> {
    Ok(Json(app.store.load_scene(id)?))
}

/// `PUT /projects/{id}/scene` -> 204. The store validates the collection before
/// the atomic write; a schema violation is rejected with a structured JSON error
/// and the previous scene stays on disk unchanged.
///
/// After a successful write the project's in-memory calc records are refreshed
/// to the new scene's identity (HIGH-1b): editing the scene must not leave a
/// stale `calc_id` reconditionable against a mutated receiver set. The primary
/// guard is `recondition` re-minting identity per request; this keeps the cached
/// records consistent too.
pub async fn put_scene(
    State(app): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(scene): Json<FeatureCollection>,
) -> Result<StatusCode, ApiError> {
    app.store.save_scene(id, &scene)?;
    crate::api::calc::refresh_project_calc_identity(&app, id).await;
    Ok(StatusCode::NO_CONTENT)
}
