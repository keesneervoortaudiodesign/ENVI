//! Contract test: the served app bundle is **cross-origin isolated** (SVC-02,
//! D-04).
//!
//! The client-side threaded-WASM solve needs `SharedArrayBuffer`, which the
//! browser only exposes when the top-level document response carries BOTH
//! `Cross-Origin-Opener-Policy: same-origin` AND
//! `Cross-Origin-Embedder-Policy: credentialless` (`self.crossOriginIsolated`).
//! This drives the full [`envi_service::api::app`] router in-process via
//! `tower::ServiceExt::oneshot` (no socket, no bundle on disk needed — the
//! header layers wrap the whole router, so even the fallback's 404 carries them)
//! and pins both header values exactly.
//!
//! The COEP value is asserted to be the exact string `credentialless`, NOT
//! `require-corp`: `require-corp` would block the Phase-8 direct third-party
//! fetches (basemap/AHN/Overpass) that send no CORP header (D-04). Pinning the
//! exact value is the regression guard against a silent flip to the
//! credentialed-resource-blocking variant.

use axum::http::StatusCode;
use tower::ServiceExt; // oneshot

mod common;
use common::{get, test_app};

/// A GET on a non-API (bundle) path returns both cross-origin-isolation headers
/// with their exact values. Whether `web/dist` exists or not, the layers wrap
/// the fallback service, so the header contract holds either way.
#[tokio::test]
async fn bundle_response_is_cross_origin_isolated() {
    let (app, _tmp) = test_app();

    let resp = app.oneshot(get("/")).await.expect("router responds");

    // The bundle may be present (200) or absent in a bare checkout (404); either
    // way the isolation headers must be on the response.
    assert!(
        resp.status() == StatusCode::OK || resp.status() == StatusCode::NOT_FOUND,
        "bundle path resolves to the fallback service (got {})",
        resp.status()
    );

    let coop = resp
        .headers()
        .get("cross-origin-opener-policy")
        .expect("COOP header present on the bundle response");
    assert_eq!(
        coop, "same-origin",
        "COOP must be same-origin for cross-origin isolation"
    );

    let coep = resp
        .headers()
        .get("cross-origin-embedder-policy")
        .expect("COEP header present on the bundle response");
    // Exact value: credentialless, NEVER require-corp (D-04 — require-corp breaks
    // the Phase-8 direct third-party fetches that send no CORP header).
    assert_eq!(
        coep, "credentialless",
        "COEP must be credentialless (not require-corp) so Phase-8 direct fetches keep working"
    );
    assert_ne!(
        coep, "require-corp",
        "COEP must not be require-corp (would break Phase-8 direct GIS/basemap fetches)"
    );
}

/// The isolation headers also ride the `/api/v1` responses (the layers wrap the
/// whole router). Harmless there, but confirms the layer placement covers the
/// bundle service rather than being scoped to a single route.
#[tokio::test]
async fn api_responses_also_carry_isolation_headers() {
    let (app, _tmp) = test_app();

    let resp = app
        .oneshot(get("/api/v1/meta/freq-axis"))
        .await
        .expect("router responds");
    assert_eq!(resp.status(), StatusCode::OK, "freq-axis is a static 200");

    assert_eq!(
        resp.headers()
            .get("cross-origin-opener-policy")
            .expect("COOP present"),
        "same-origin",
    );
    assert_eq!(
        resp.headers()
            .get("cross-origin-embedder-policy")
            .expect("COEP present"),
        "credentialless",
    );
}
