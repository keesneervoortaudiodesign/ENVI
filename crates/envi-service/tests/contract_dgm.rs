//! Contract tests: `POST /api/v1/dgm/triangulate` (D-08).
//!
//! In-process oneshot against the FULL [`envi_service::api::app`] router — no
//! socket, no credentials (06-RESEARCH Validation Architecture). Building the
//! full router proves every route uses the axum 0.8 brace syntax.
//!
//! The load-bearing property (Pitfall 3 / must-have #2): an interior-crossing
//! breakline — which would PANIC `spade`'s `add_constraint_edges` — returns a
//! structured `4xx` instead, and reaching the assertion at all proves the
//! request thread did NOT abort. Degenerate input is likewise a `4xx`, never a
//! `500`.

use axum::http::{Method, StatusCode};
use tower::ServiceExt; // oneshot

mod common;
use common::{json_req, read_json, test_app};

const URI: &str = "/api/v1/dgm/triangulate";

/// A 10×10 square (Z == y) triangulates to ≥1 triangle and returns a
/// self-contained mesh: `vertices` + vertex-index `triangles`.
#[tokio::test]
async fn valid_square_returns_non_empty_triangulation() {
    let (app, _tmp) = test_app();

    let body = serde_json::json!({
        "points": [
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 10.0],
            [0.0, 10.0, 10.0]
        ],
        "breaklines": []
    });

    let (status, json) = read_json(
        app.oneshot(json_req(Method::POST, URI, &body))
            .await
            .expect("router responds"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "valid square is 200");

    let triangles = json["triangles"].as_array().expect("triangles array");
    assert!(!triangles.is_empty(), "at least one triangle");
    let vertices = json["vertices"].as_array().expect("vertices array");
    assert_eq!(vertices.len(), 4, "four distinct elevation vertices");
    // Every triangle index refers into the vertex list.
    for tri in triangles {
        for idx in tri.as_array().expect("index triple") {
            let i = idx.as_u64().expect("index is an integer") as usize;
            assert!(i < vertices.len(), "vertex index {i} in range");
        }
    }
}

/// Two breaklines that cross in their interior return a structured `bad_request`
/// 4xx — proving the process did NOT panic/abort (Pitfall 3). The status is a
/// 400, never a 500 and never a thread crash.
#[tokio::test]
async fn interior_crossing_breaklines_are_400_not_panic() {
    let (app, _tmp) = test_app();

    // Square corners; the two diagonals cross at (5,5) in their interior.
    let body = serde_json::json!({
        "points": [
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 10.0],
            [0.0, 10.0, 10.0]
        ],
        "breaklines": [
            [[0.0, 0.0], [10.0, 10.0]],
            [[10.0, 0.0], [0.0, 10.0]]
        ]
    });

    let (status, json) = read_json(
        app.oneshot(json_req(Method::POST, URI, &body))
            .await
            .expect("router responds without aborting"),
    )
    .await;
    // Reaching here at all proves no panic/abort; the status must be a client 4xx.
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "interior-crossing breaklines are a 400, never a 500/panic"
    );
    assert_eq!(
        json["error"], "bad_request",
        "structured JSON error envelope, not HTML"
    );
}

/// A degenerate 2-point input (no triangle can form) returns a structured
/// `bad_request` 4xx, never a 500.
#[tokio::test]
async fn degenerate_two_point_input_is_400() {
    let (app, _tmp) = test_app();

    let body = serde_json::json!({
        "points": [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
        "breaklines": []
    });

    let (status, json) = read_json(
        app.oneshot(json_req(Method::POST, URI, &body))
            .await
            .expect("router responds"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "2 points is a 400");
    assert_eq!(json["error"], "bad_request", "structured JSON envelope");
}

/// `breaklines` is optional (defaults to none): a points-only body still builds.
#[tokio::test]
async fn breaklines_field_is_optional() {
    let (app, _tmp) = test_app();

    let body = serde_json::json!({
        "points": [
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [5.0, 10.0, 5.0]
        ]
    });

    let (status, json) = read_json(
        app.oneshot(json_req(Method::POST, URI, &body))
            .await
            .expect("router responds"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "points-only body is 200");
    assert!(
        !json["triangles"].as_array().expect("triangles").is_empty(),
        "one triangle from three non-collinear points"
    );
}
