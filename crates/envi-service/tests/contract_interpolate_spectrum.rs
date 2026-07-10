//! Contract tests: `POST /api/v1/meta/interpolate-spectrum` (D-05).
//!
//! In-process oneshot against the FULL [`envi_service::api::app`] router
//! (06-RESEARCH Validation Architecture: `tower::ServiceExt::oneshot` — no
//! socket, no credentials). Building the full router also proves every route
//! uses the axum 0.8 brace syntax (a `/:id` colon straggler panics here).
//!
//! Coverage (the plan's must-have #1):
//! - valid octave(9) / third(27) / twelfth(105) → 200 with dense `r_db[105]`;
//!   octave anchors 4,16,…,100 echo the inputs **bit-for-bit** (the shared
//!   `envi_store::interpolate` core copies anchors verbatim);
//! - wrong length → structured `bad_request` 4xx (via `From<StoreError>`);
//! - out-of-range `R > 1000` → structured `bad_request` 4xx — REJECTED by the
//!   engine `IsolationSpectrum::new` range gate, never silently clamped (07-01
//!   removed the clamp from `interpolate` for exactly this);
//! - a non-finite number → 4xx client error, never HTML / 500 / panic. (JSON
//!   itself cannot carry NaN/±∞ and `serde_json` rejects an overflow literal as
//!   `NumberOutOfRange`, so a non-finite value is refused at the transport
//!   boundary; the store's finiteness gate is defense-in-depth for the internal
//!   call paths.)

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt; // oneshot

mod common;
use common::{json_req, read_json, test_app};

const URI: &str = "/api/v1/meta/interpolate-spectrum";

/// A valid octave body (9 anchors) expands to a dense `[105]` grid, and the
/// octave grid indices 4,16,…,100 echo the inputs bit-for-bit.
#[tokio::test]
async fn octave_expands_to_105_with_bit_exact_anchors() {
    let (app, _tmp) = test_app();

    // Integer-valued f64 anchors: exactly representable AND lossless through JSON.
    let values: Vec<f64> = (0..9).map(|k| 20.0 + k as f64).collect();
    let body = serde_json::json!({ "resolution": "octave", "values": values });

    let (status, json) = read_json(
        app.oneshot(json_req(Method::POST, URI, &body))
            .await
            .expect("router responds"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "valid octave is 200");

    let r_db = json["r_db"].as_array().expect("r_db array");
    assert_eq!(r_db.len(), 105, "dense 1/12-octave grid");

    let anchor_idx = [4usize, 16, 28, 40, 52, 64, 76, 88, 100];
    for (k, &idx) in anchor_idx.iter().enumerate() {
        let got = r_db[idx].as_f64().expect("r_db entry is a number");
        assert_eq!(
            got.to_bits(),
            values[k].to_bits(),
            "octave anchor band {idx} must echo input {k} bit-for-bit"
        );
    }
}

/// A valid third-octave body (27 anchors) expands to a dense `[105]` grid; the
/// third-octave grid indices 0,4,…,104 echo the inputs bit-for-bit.
#[tokio::test]
async fn third_octave_expands_to_105_with_bit_exact_anchors() {
    let (app, _tmp) = test_app();

    let values: Vec<f64> = (0..27).map(|k| 5.0 + k as f64).collect();
    let body = serde_json::json!({ "resolution": "third", "values": values });

    let (status, json) = read_json(
        app.oneshot(json_req(Method::POST, URI, &body))
            .await
            .expect("router responds"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "valid third is 200");

    let r_db = json["r_db"].as_array().expect("r_db array");
    assert_eq!(r_db.len(), 105, "dense grid");
    for (k, v) in values.iter().enumerate() {
        let got = r_db[4 * k].as_f64().expect("number");
        assert_eq!(
            got.to_bits(),
            v.to_bits(),
            "third anchor band {} bit-for-bit",
            4 * k
        );
    }
}

/// A valid twelfth body (105 values) is the identity: every band echoed verbatim.
#[tokio::test]
async fn twelfth_is_identity() {
    let (app, _tmp) = test_app();

    let values: Vec<f64> = (0..105).map(|k| 1.0 + k as f64).collect();
    let body = serde_json::json!({ "resolution": "twelfth", "values": values });

    let (status, json) = read_json(
        app.oneshot(json_req(Method::POST, URI, &body))
            .await
            .expect("router responds"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "valid twelfth is 200");
    let r_db = json["r_db"].as_array().expect("r_db array");
    assert_eq!(r_db.len(), 105);
    for (b, v) in values.iter().enumerate() {
        assert_eq!(
            r_db[b].as_f64().expect("number").to_bits(),
            v.to_bits(),
            "twelfth band {b} verbatim"
        );
    }
}

/// Wrong `values` length ⇒ structured `bad_request` 4xx (never a 200 with a
/// mis-sized grid, never HTML/500). Surfaces the store's `BadBandCount`.
#[tokio::test]
async fn wrong_length_is_structured_bad_request() {
    let (app, _tmp) = test_app();

    // 8 values for octave (expects 9).
    let body = serde_json::json!({ "resolution": "octave", "values": vec![10.0; 8] });
    let (status, json) = read_json(
        app.oneshot(json_req(Method::POST, URI, &body))
            .await
            .expect("router responds"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "wrong length is 400");
    assert_eq!(
        json["error"], "bad_request",
        "structured JSON error envelope, not HTML"
    );
}

/// An out-of-range value (`R = 2000 > MAX_R_DB`) is REJECTED as a structured
/// `bad_request` 4xx by the engine range gate — never silently clamped to a
/// wrong 200 (the load-bearing 07-01 property; must-have #1).
#[tokio::test]
async fn out_of_range_r_is_rejected_never_clamped() {
    let (app, _tmp) = test_app();

    // All anchors in-range except one wildly out of range.
    let mut values = vec![10.0_f64; 9];
    values[4] = 2000.0;
    let body = serde_json::json!({ "resolution": "octave", "values": values });

    let (status, json) = read_json(
        app.oneshot(json_req(Method::POST, URI, &body))
            .await
            .expect("router responds"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "R>1000 is a 400, NOT a clamped 200"
    );
    assert_eq!(json["error"], "bad_request", "structured JSON envelope");
}

/// A non-finite number in `values` is refused with a 4xx client error — never
/// HTML, never a 500 / panic. JSON has no NaN/±∞ literal and `serde_json`
/// rejects an overflow literal (`1e400`) as `NumberOutOfRange`, so the transport
/// boundary refuses it before it can poison the dense grid.
#[tokio::test]
async fn non_finite_value_is_client_error_never_html() {
    let (app, _tmp) = test_app();

    // Raw body: 1e400 overflows f64 -> serde_json NumberOutOfRange (a non-finite
    // value cannot be expressed as a finite JSON number).
    let raw = r#"{"resolution":"octave","values":[1e400,10,10,10,10,10,10,10,10]}"#;
    let req = Request::builder()
        .method(Method::POST)
        .uri(URI)
        .header("content-type", "application/json")
        .body(Body::from(raw))
        .expect("request");

    let resp = app.oneshot(req).await.expect("router responds");
    let status = resp.status();
    assert!(
        status.is_client_error(),
        "non-finite input is a 4xx client error, got {status}"
    );
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("collect")
        .to_bytes();
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        !text.contains("<html") && !text.contains("<!DOCTYPE"),
        "body is never an HTML page: {text}"
    );
}
