//! Contract tests: the freq-axis wire anchor + single-binary-serves-frontend.
//!
//! In-process oneshot against the FULL [`app`] router (06-RESEARCH Validation
//! Architecture: `tower::ServiceExt::oneshot` + `http_body_util` collect — no
//! socket binding, no credentials). Building the full router also proves every
//! route uses the axum 0.8 brace syntax (a colon straggler panics here).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt; // oneshot

mod common;
use common::test_app;

async fn body_bytes(resp: axum::response::Response) -> Vec<u8> {
    resp.into_body()
        .collect()
        .await
        .expect("collect body")
        .to_bytes()
        .to_vec()
}

#[tokio::test]
async fn freq_axis_meta_matches_engine() {
    let (app, _tmp) = test_app();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/meta/freq-axis")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router responds");
    assert_eq!(resp.status(), StatusCode::OK, "freq-axis is 200");

    let json: serde_json::Value =
        serde_json::from_slice(&body_bytes(resp).await).expect("json body");

    let centres = json["centres_hz"].as_array().expect("centres_hz array");
    assert_eq!(centres.len(), 105, "105-point 1/12-octave axis");
    assert_eq!(json["n_bands"].as_u64(), Some(105));

    // Bit-exact house rule: band index 64 IS exactly 1000.0 Hz (freq.rs precedent).
    let c64 = centres[64].as_f64().expect("centres_hz[64] is a number");
    assert_eq!(
        c64.to_bits(),
        1000.0_f64.to_bits(),
        "centres_hz[64] must equal 1000.0 bit-exact against the engine value"
    );

    // third_octave_indices == [0, 4, ..., 104], exactly 27 entries.
    let thirds: Vec<u64> = json["third_octave_indices"]
        .as_array()
        .expect("third_octave_indices array")
        .iter()
        .map(|v| v.as_u64().expect("index is an integer"))
        .collect();
    let expected: Vec<u64> = (0..27).map(|i| i * 4).collect();
    assert_eq!(
        thirds, expected,
        "27-element arithmetic sequence 0,4,...,104"
    );

    // Nominal list also has 27 entries.
    assert_eq!(
        json["nominal_third_octave_hz"]
            .as_array()
            .expect("nominal array")
            .len(),
        27,
        "27 nominal 1/3-octave labels"
    );

    // Every centre strictly ascending.
    let vals: Vec<f64> = centres.iter().map(|v| v.as_f64().unwrap()).collect();
    for w in vals.windows(2) {
        assert!(w[0] < w[1], "centres_hz strictly ascending: {w:?}");
    }
}

#[tokio::test]
async fn static_bundle_served_with_spa_fallback() {
    // GET / -> 200 placeholder HTML (Phase-7 heading present).
    let (app, _tmp) = test_app();
    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .expect("router responds");
    assert_eq!(resp.status(), StatusCode::OK, "GET / is 200");
    let html = String::from_utf8(body_bytes(resp).await).expect("utf8 html");
    assert!(
        html.contains("frontend arrives in Phase 7"),
        "GET / serves the placeholder heading"
    );

    // Unknown non-API deep link -> 200 index.html (SPA fallback).
    let (app, _tmp) = test_app();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/some/unknown/spa/path")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router responds");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "SPA deep link falls back to 200"
    );
    let html = String::from_utf8(body_bytes(resp).await).expect("utf8 html");
    assert!(
        html.contains("frontend arrives in Phase 7"),
        "SPA deep link serves index.html, not a 404"
    );

    // Unknown /api/v1 path -> 404 JSON (the API namespace does NOT serve HTML).
    let (app, _tmp) = test_app();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router responds");
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "unknown API path is 404, not an HTML fallback"
    );
    let json: serde_json::Value =
        serde_json::from_slice(&body_bytes(resp).await).expect("404 body is JSON");
    assert_eq!(json["error"], "not_found", "structured JSON error body");
}
