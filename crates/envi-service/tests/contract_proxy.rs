//! Contract tests: `GET /api/v1/proxy/{source}/{*path}` — the D-02 allowlisted
//! byte relay (Pattern 5, T-08-03-0x).
//!
//! Two layers, both fully OFFLINE (no socket, no S3, no credentials):
//!
//! 1. **Full-app router tests** drive the real [`envi_service::api::app`] router
//!    in-process via `tower::ServiceExt::oneshot` — building the full router proves
//!    the `/proxy/{source}/{*path}` brace+wildcard route uses valid axum 0.8 syntax
//!    (a straggler colon route would PANIC the whole suite at construction). Every
//!    case here is an SSRF-rejection path that returns BEFORE any network I/O
//!    (unknown source, prefix escape) or is refused by method routing (non-GET), so
//!    the suite never touches the network.
//! 2. **Unit tests on [`resolve_upstream`]** pin the SSRF chokepoint directly: the
//!    happy path builds the HARDCODED-host `https://…` URL (offline — it constructs
//!    a string, it does not fetch), and the rejection variants (unknown source,
//!    prefix escape, `..` traversal) each map to the right typed [`ApiError`].
//!
//! The load-bearing property (must-have #2 / T-08-03-01): an unknown source, a path
//! escaping the allowlisted prefix, and a non-GET method are all rejected before any
//! outbound request — the relay can only ever reach an allowlisted `(host, prefix)`.

use axum::http::{Method, StatusCode};
use tower::ServiceExt; // oneshot

use envi_service::api::proxy::resolve_upstream;
use envi_service::error::ApiError;

mod common;
use common::{get, json_req, read_json, test_app};

// ---------------------------------------------------------------------------
// Full-app router contract (offline: every case rejects before network I/O)
// ---------------------------------------------------------------------------

/// An unknown `{source}` is a structured `404` JSON envelope — rejected by the
/// allowlist lookup BEFORE any outbound request, and never the HTML SPA shell.
#[tokio::test]
async fn unknown_source_is_404_json_not_html() {
    let (app, _tmp) = test_app();

    let (status, json) = read_json(
        app.oneshot(get("/api/v1/proxy/evil-host/anything/at/all"))
            .await
            .expect("router responds"),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "unknown source is a 404, rejected before any network call"
    );
    assert_eq!(
        json["error"], "not_found",
        "structured JSON error envelope, not HTML"
    );
}

/// A `{*path}` that does not start with the allowlisted prefix is a structured
/// `400` — rejected before any outbound request, and the body leaks no upstream
/// host (MED-1, T-08-03-03).
#[tokio::test]
async fn prefix_escape_is_400_json_and_leaks_no_host() {
    let (app, _tmp) = test_app();

    // Valid source, but the path does not start with "/Copernicus_DSM_COG_".
    let (status, json) = read_json(
        app.oneshot(get("/api/v1/proxy/glo30/etc/passwd"))
            .await
            .expect("router responds"),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a path outside the allowlisted prefix is a 400, before any network call"
    );
    assert_eq!(json["error"], "bad_request", "structured JSON envelope");
    let detail = json["detail"].as_str().unwrap_or_default();
    assert!(
        !detail.contains("amazonaws") && !detail.contains("s3") && !detail.contains("http"),
        "error detail must not leak the upstream host (MED-1); got: {detail:?}"
    );
}

/// A non-GET method on the proxy route is rejected by axum method routing with a
/// `405` — the request never reaches the handler, so no outbound call is made
/// (GET-only prohibition). This is enforced by registering `get(relay)` only.
#[tokio::test]
async fn non_get_method_is_405_before_handler() {
    let (app, _tmp) = test_app();

    // A well-formed, allowlisted path — the ONLY reason this is refused is the
    // method. A POST must never be relayed.
    let (status, _json) = read_json(
        app.oneshot(json_req(
            Method::POST,
            "/api/v1/proxy/glo30/Copernicus_DSM_COG_10_N52_00_E004_00_DEM.tif",
            &serde_json::json!({}),
        ))
        .await
        .expect("router responds"),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::METHOD_NOT_ALLOWED,
        "non-GET on the proxy route is a 405 (GET-only), rejected before the handler"
    );
}

// ---------------------------------------------------------------------------
// Unit contract on the SSRF chokepoint (offline URL builder)
// ---------------------------------------------------------------------------

/// A well-formed allowlisted request builds the HARDCODED scheme+host URL — the
/// user never supplies a host, so the relay can only ever reach an allowlisted S3.
#[test]
fn resolve_upstream_builds_hardcoded_https_url() {
    let glo30 = resolve_upstream(
        "glo30",
        "Copernicus_DSM_COG_10_N52_00_E004_00_DEM/Copernicus_DSM_COG_10_N52_00_E004_00_DEM.tif",
    )
    .expect("valid glo30 path resolves");
    assert_eq!(
        glo30,
        "https://copernicus-dem-30m.s3.amazonaws.com/Copernicus_DSM_COG_10_N52_00_E004_00_DEM/Copernicus_DSM_COG_10_N52_00_E004_00_DEM.tif",
        "glo30 resolves to the hardcoded copernicus S3 host"
    );

    let worldcover = resolve_upstream(
        "worldcover",
        "v200/2021/map/ESA_WorldCover_10m_2021_v200_N51E003_Map.tif",
    )
    .expect("valid worldcover path resolves");
    assert_eq!(
        worldcover,
        "https://esa-worldcover.s3.eu-central-1.amazonaws.com/v200/2021/map/ESA_WorldCover_10m_2021_v200_N51E003_Map.tif",
        "worldcover resolves to the hardcoded esa S3 host"
    );

    // Both are https and carry the hardcoded host — a caller cannot inject one.
    assert!(glo30.starts_with("https://copernicus-dem-30m.s3.amazonaws.com/"));
    assert!(worldcover.starts_with("https://esa-worldcover.s3.eu-central-1.amazonaws.com/"));
}

/// An unknown source is a typed `NotFound` — the allowlist has no runtime add path.
#[test]
fn resolve_upstream_rejects_unknown_source() {
    let err = resolve_upstream("attacker", "internal.host/secret").unwrap_err();
    assert!(
        matches!(err, ApiError::NotFound),
        "unknown source is a NotFound, not a resolved URL"
    );
}

/// A path escaping the allowlisted prefix is a typed `BadRequest` with a generic,
/// host-free detail (MED-1).
#[test]
fn resolve_upstream_rejects_prefix_escape() {
    let err = resolve_upstream("glo30", "etc/passwd").unwrap_err();
    match err {
        ApiError::BadRequest { detail } => assert!(
            !detail.contains("amazonaws") && !detail.contains("http"),
            "prefix-escape detail must not leak the upstream host; got: {detail:?}"
        ),
        other => panic!("expected BadRequest for a prefix escape, got {other:?}"),
    }
}

/// A `..` traversal inside an otherwise prefix-valid path is a typed `BadRequest` —
/// even though it starts with the allowlisted prefix, the traversal is refused.
#[test]
fn resolve_upstream_rejects_dot_dot_traversal() {
    // Starts with the glo30 prefix, but embeds a "../" traversal.
    let err = resolve_upstream("glo30", "Copernicus_DSM_COG_../../../etc/passwd").unwrap_err();
    assert!(
        matches!(err, ApiError::BadRequest { .. }),
        "a `..` traversal is a BadRequest even under a valid prefix"
    );
}
