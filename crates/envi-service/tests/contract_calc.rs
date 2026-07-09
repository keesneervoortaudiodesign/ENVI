//! Contract tests for the SC4 recondition/recompute split with the enforced 409
//! content-hash gate (D-07) — oneshot against the FULL app router, TempDir store,
//! no sockets.
//!
//! Proven here: a mismatched `tensor_hash` on recondition is ACTUALLY rejected
//! with 409 (never silently served); a matched hash returns dense `[105]`
//! band-index spectra flagged `stub: true`; conditioning variation never moves
//! identity (D-07); recompute mints a new hash + job and invalidates the old hash;
//! and calc submit writes an honest-stub manifest with the reserved chunk layout.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt; // oneshot

use envi_service::api;
use envi_service::state::AppState;
use envi_store::project_dir::ProjectStore;

fn repo_web_dist() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root two levels up")
        .join("web")
        .join("dist")
}

fn app_over(root: &Path) -> Router {
    let store = ProjectStore::new(root.to_path_buf()).expect("store");
    let state = Arc::new(AppState::new(store));
    api::app(state, &repo_web_dist())
}

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("request")
}

fn json_req(method: Method, uri: &str, body: &serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(body).expect("serialize")))
        .expect("request")
}

async fn read_json(resp: axum::response::Response) -> (StatusCode, serde_json::Value) {
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
fn source_receiver_scene(receiver_lon: f64) -> serde_json::Value {
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
async fn make_project(app: &Router, receiver_lon: f64) -> String {
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

/// POST a calculation; return `(calc_id, job_id, tensor_hash)`.
async fn submit_calc(app: &Router, project_id: &str) -> (String, String, String) {
    let (status, body) = read_json(
        app.clone()
            .oneshot(json_req(
                Method::POST,
                &format!("/api/v1/projects/{project_id}/calculations"),
                &serde_json::json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "calc submit is 202");
    (
        body["calc_id"].as_str().expect("calc_id").to_string(),
        body["job_id"].as_str().expect("job_id").to_string(),
        body["tensor_hash"]
            .as_str()
            .expect("tensor_hash")
            .to_string(),
    )
}

/// Poll `GET /jobs/{id}` until it reaches `done` (bounded), returning success.
async fn job_reaches_done(app: &Router, job_id: &str) -> bool {
    for _ in 0..60 {
        let (status, body) = read_json(
            app.clone()
                .oneshot(get(&format!("/api/v1/jobs/{job_id}")))
                .await
                .unwrap(),
        )
        .await;
        if status == StatusCode::OK && body["state"].as_str() == Some("done") {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

#[tokio::test]
async fn recondition_rejects_mismatched_tensor_hash_with_409() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let app = app_over(tmp.path());
    let project_id = make_project(&app, 4.8950).await;
    let (calc_id, _job, h) = submit_calc(&app, &project_id).await;

    // A deliberately wrong hash -> 409 with the frozen mismatch body.
    let bad = serde_json::json!({ "tensor_hash": "deadbeef", "conditioning": {} });
    let (status, body) = read_json(
        app.clone()
            .oneshot(json_req(
                Method::POST,
                &format!("/api/v1/calculations/{calc_id}/recondition"),
                &bad,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "mismatched hash -> 409");
    assert_eq!(body["error"], "tensor_hash_mismatch", "frozen error code");
    assert_eq!(body["expected"], h, "expected is the minted hash");
    assert_eq!(body["got"], "deadbeef", "got echoes the sent value");
    assert!(
        body["hint"]
            .as_str()
            .unwrap_or_default()
            .contains("recompute"),
        "hint points at recompute: {:?}",
        body["hint"]
    );

    // The stub tensor is UNCHANGED: a follow-up matched recondition succeeds.
    let good = serde_json::json!({ "tensor_hash": h, "conditioning": {} });
    let (status, body) = read_json(
        app.clone()
            .oneshot(json_req(
                Method::POST,
                &format!("/api/v1/calculations/{calc_id}/recondition"),
                &good,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "matched hash still succeeds after the 409"
    );
    assert_eq!(body["stub"], true, "honest-stub provenance");
}

#[tokio::test]
async fn recondition_with_matching_hash_returns_stub_spectra() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let app = app_over(tmp.path());
    let project_id = make_project(&app, 4.8950).await;
    let (calc_id, _job, h) = submit_calc(&app, &project_id).await;

    let req = serde_json::json!({
        "tensor_hash": h,
        "conditioning": { "00000000-0000-0000-0000-000000000001": { "gain_db": -3.0 } }
    });
    let (status, body) = read_json(
        app.clone()
            .oneshot(json_req(
                Method::POST,
                &format!("/api/v1/calculations/{calc_id}/recondition"),
                &req,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "matched hash -> 200");
    assert_eq!(body["stub"], true, "stub provenance");
    assert_eq!(body["tensor_hash"], h, "tensor_hash echoed");

    let spectra = body["spectra"].as_object().expect("spectra map");
    assert_eq!(spectra.len(), 1, "one spectrum per scene receiver");
    let receiver_entry = &spectra["00000000-0000-0000-0000-000000000002"];
    let band_db = receiver_entry["band_db"].as_array().expect("band_db array");
    assert_eq!(
        band_db.len(),
        105,
        "dense [105] band-index spectrum (SVC-07)"
    );
}

#[tokio::test]
async fn conditioning_never_moves_identity() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let app = app_over(tmp.path());
    let project_id = make_project(&app, 4.8950).await;
    let (calc_id, _job, h) = submit_calc(&app, &project_id).await;

    // Wildly different conditioning, same hash -> both 200 with the SAME identity.
    let quiet = serde_json::json!({
        "tensor_hash": h,
        "conditioning": { "00000000-0000-0000-0000-000000000001":
            { "gain_db": -30.0, "muted": true, "filter_band_db": vec![0.0_f64; 105] } }
    });
    let loud = serde_json::json!({
        "tensor_hash": h,
        "conditioning": { "00000000-0000-0000-0000-000000000001":
            { "gain_db": 0.0, "muted": false } }
    });

    let (s1, b1) = read_json(
        app.clone()
            .oneshot(json_req(
                Method::POST,
                &format!("/api/v1/calculations/{calc_id}/recondition"),
                &quiet,
            ))
            .await
            .unwrap(),
    )
    .await;
    let (s2, b2) = read_json(
        app.clone()
            .oneshot(json_req(
                Method::POST,
                &format!("/api/v1/calculations/{calc_id}/recondition"),
                &loud,
            ))
            .await
            .unwrap(),
    )
    .await;

    assert_eq!(s1, StatusCode::OK, "quiet conditioning -> 200");
    assert_eq!(s2, StatusCode::OK, "loud conditioning -> 200");
    assert_eq!(
        b1["tensor_hash"], b2["tensor_hash"],
        "conditioning does not move tensor identity (D-07)"
    );
    assert_eq!(b1["tensor_hash"], h, "identity is still the minted hash");
}

#[tokio::test]
async fn recompute_mints_identity_and_job() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let app = app_over(tmp.path());
    let project_id = make_project(&app, 4.8950).await;
    let (calc_id, _job, h) = submit_calc(&app, &project_id).await;

    // Move the receiver (PUT a modified scene), then recompute.
    let resp = app
        .clone()
        .oneshot(json_req(
            Method::PUT,
            &format!("/api/v1/projects/{project_id}/scene"),
            &source_receiver_scene(4.8965),
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NO_CONTENT,
        "moved receiver saved"
    );

    let (status, body) = read_json(
        app.clone()
            .oneshot(json_req(
                Method::POST,
                &format!("/api/v1/calculations/{calc_id}/recompute"),
                &serde_json::json!({ "reason": "receivers" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "recompute is 202");
    let new_hash = body["tensor_hash"]
        .as_str()
        .expect("new tensor_hash")
        .to_string();
    assert_ne!(new_hash, h, "moving a receiver mints a NEW identity");
    let new_job = body["job_id"].as_str().expect("job_id").to_string();
    assert!(
        job_reaches_done(&app, &new_job).await,
        "the recompute job reaches done via the registry"
    );

    // The OLD hash now 409s; the NEW hash succeeds.
    let (old_status, _) = read_json(
        app.clone()
            .oneshot(json_req(
                Method::POST,
                &format!("/api/v1/calculations/{calc_id}/recondition"),
                &serde_json::json!({ "tensor_hash": h, "conditioning": {} }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(old_status, StatusCode::CONFLICT, "stale hash now 409s");

    let (new_status, _) = read_json(
        app.clone()
            .oneshot(json_req(
                Method::POST,
                &format!("/api/v1/calculations/{calc_id}/recondition"),
                &serde_json::json!({ "tensor_hash": new_hash, "conditioning": {} }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(new_status, StatusCode::OK, "the new hash reconditions");
}

#[tokio::test]
async fn calc_submit_writes_stub_manifest() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let app = app_over(tmp.path());
    let project_id = make_project(&app, 4.8950).await;
    let (calc_id, _job, h) = submit_calc(&app, &project_id).await;

    let calc_dir = tmp.path().join(&project_id).join("calc").join(&calc_id);
    let manifest_path = calc_dir.join("manifest.json");
    assert!(manifest_path.is_file(), "manifest.json written to disk");

    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    assert_eq!(manifest["stub"], true, "honest-stub manifest");
    assert_eq!(
        manifest["tensor_hash"], h,
        "manifest carries the minted hash"
    );
    let dims = manifest["dims"].as_array().expect("dims array");
    assert_eq!(dims.len(), 3, "dims is [S, R, F]");
    assert_eq!(dims[0].as_u64(), Some(1), "one source -> S = 1");
    assert_eq!(dims[1].as_u64(), Some(1), "one receiver -> R = 1");
    assert_eq!(dims[2].as_u64(), Some(105), "F axis is always 105");
    assert!(
        manifest["chunk_receivers"].as_u64().unwrap_or(0) > 0,
        "chunk_receivers > 0"
    );

    // Reserved channel dirs present + empty.
    assert!(calc_dir.join("tensor").is_dir(), "tensor/ reserved");
    assert!(calc_dir.join("pincoh").is_dir(), "pincoh/ reserved");
    assert_eq!(
        std::fs::read_dir(calc_dir.join("tensor")).unwrap().count(),
        0,
        "tensor/ is empty (Phase 9 fills it)"
    );
}
