//! Shared scaffolding for the `envi-service` contract tests.
//!
//! Every contract test drives the FULL app router in-process via
//! `tower::ServiceExt::oneshot` over a `TempDir`-backed store — no sockets, no
//! credentials (06-RESEARCH Validation Architecture). The builders, request
//! helpers, JSON reader, and one shared scene fixture live here so the four
//! `contract_*` files share one copy.
//!
//! `#![allow(dead_code)]`: each test binary `mod common;`-includes this file and
//! uses only a subset, so unused-in-this-binary helpers are expected.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt; // oneshot

use envi_service::api;
use envi_service::state::AppState;
use envi_store::project_dir::ProjectStore;

/// Repo `web/dist`: two levels up from this crate's manifest dir
/// (`crates/envi-service` -> workspace root -> `web/dist`).
pub fn repo_web_dist() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root two levels up")
        .join("web")
        .join("dist")
}

/// Build the full app over an existing store root (used to simulate a fresh
/// process over the same directory).
pub fn app_over(root: &Path) -> Router {
    let store = ProjectStore::new(root.to_path_buf()).expect("store");
    let state = Arc::new(AppState::new(store));
    api::app(state, &repo_web_dist())
}

/// Build the full app over a fresh `TempDir` root (kept alive by the guard).
pub fn test_app() -> (Router, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    (app_over(tmp.path()), tmp)
}

/// A fresh shared `AppState` over a `TempDir` root, for tests that submit jobs
/// directly on the state before building a router (Pitfall 6).
pub fn test_state() -> (Arc<AppState>, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let store = ProjectStore::new(tmp.path().to_path_buf()).expect("store");
    (Arc::new(AppState::new(store)), tmp)
}

/// Build a router over an already-shared `AppState`.
pub fn app_of(state: Arc<AppState>) -> Router {
    api::app(state, &repo_web_dist())
}

/// A `GET` request with an empty body.
pub fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("request")
}

/// A `DELETE` request with an empty body.
pub fn delete(uri: &str) -> Request<Body> {
    Request::builder()
        .method(Method::DELETE)
        .uri(uri)
        .body(Body::empty())
        .expect("request")
}

/// A JSON request (`content-type: application/json`) with the given method/body.
pub fn json_req(method: Method, uri: &str, body: &serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(body).expect("serialize")))
        .expect("request")
}

/// Collect a response into `(status, json_body)`; an empty body reads as `Null`.
pub async fn read_json(resp: axum::response::Response) -> (StatusCode, serde_json::Value) {
    let status = resp.status();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("collect")
        .to_bytes();
    let json = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("json body")
    };
    (status, json)
}

/// A minimal scene: one source + one receiver near Amsterdam. The receiver
/// longitude is a parameter so a test can move it (recompute identity change).
pub fn source_receiver_scene(receiver_lon: f64) -> serde_json::Value {
    serde_json::json!({
      "type": "FeatureCollection",
      "features": [
        {"type":"Feature","geometry":{"type":"Point","coordinates":[4.8936,52.3731]},
         "properties":{"kind":"source","id":"00000000-0000-0000-0000-000000000001","height_m":0.5}},
        {"type":"Feature","geometry":{"type":"Point","coordinates":[receiver_lon,52.3740]},
         "properties":{"kind":"receiver","id":"00000000-0000-0000-0000-000000000002","height_m":1.5}}
      ]
    })
}

/// Create a project + persist a source/receiver scene; return its project id.
pub async fn make_project(app: &Router, receiver_lon: f64) -> String {
    let create_body = serde_json::json!({
        "name": "Calc Scene",
        "origin": { "lon_deg": 4.8936, "lat_deg": 52.3731 }
    });
    let (status, created) = read_json(
        app.clone()
            .oneshot(json_req(Method::POST, "/api/v1/projects", &create_body))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "project created");
    let id = created["id"].as_str().expect("project id").to_string();

    let resp = app
        .clone()
        .oneshot(json_req(
            Method::PUT,
            &format!("/api/v1/projects/{id}/scene"),
            &source_receiver_scene(receiver_lon),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT, "scene persisted");
    id
}
