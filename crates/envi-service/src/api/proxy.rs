//! `GET /api/v1/proxy/{source}/{*path}` — the allowlisted byte relay (D-02,
//! Pattern 5). A thin **bytes-only** pass-through: it forwards a `GET` (with an
//! optional `Range`) to ONE of a hardcoded set of GIS hosts and streams the reply
//! back. It performs NO compute on the relayed bytes (SVC-07 in spirit — the
//! service never transforms payloads here) and never accepts a full user-supplied
//! URL, so it is SSRF-proof by construction.
//!
//! # Why this endpoint exists (D-02)
//!
//! Two source hosts — Copernicus GLO-30 (`copernicus-dem-30m` S3) and ESA
//! WorldCover (`esa-worldcover` S3) — do NOT send CORS headers, so the browser
//! cannot fetch them directly. Every other source (PDOK AHN, Overpass) is fetched
//! cross-origin straight from the browser and NEVER touches this relay. The relay
//! is the single new server-side network surface this phase.
//!
//! # SSRF / DoS defense (V10, T-08-03-0x — load-bearing)
//!
//! 1. `{source}` selects a row in the hardcoded [`SOURCES`] `(id, host, prefix)`
//!    allowlist — an unknown id is a `404` **before any network call**.
//! 2. The reconstructed upstream path MUST `starts_with` that row's prefix and
//!    contain no `..` traversal — a violation is a `400`, again before any I/O.
//! 3. The upstream URL is built from the **hardcoded** scheme + host + validated
//!    path; the user never supplies a host. See [`resolve_upstream`].
//! 4. The shared client ([`AppState::http`](crate::state::AppState::http)) follows
//!    NO redirects and has a connect timeout; the body is streamed under a
//!    [`MAX_RELAY_BYTES`] cap.
//! 5. GET-only: the route is registered with `get(relay)`, so any other method is
//!    a `405` at the router — it never reaches this handler.
//!
//! # Error hygiene (MED-1, T-08-03-03)
//!
//! Upstream/transport failures are logged in full server-side via `tracing::error!`
//! (see `From<reqwest::Error> for ApiError`) but return a GENERIC
//! `Internal { detail: "internal error" }` — the upstream host/path never appears
//! in a client-facing body.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use tokio_stream::StreamExt;

use crate::error::ApiError;
use crate::state::AppState;

/// The hardcoded `(source_id, host, path_prefix)` allowlist — the ENTIRE set of
/// hosts this relay may ever reach (V10 SSRF control). Exactly the two CORS-blocked
/// GIS sources from 08-RESEARCH; there is no mechanism to add a host at runtime and
/// no way to pass a full URL. Scheme is always `https` (see [`resolve_upstream`]).
const SOURCES: &[(&str, &str, &str)] = &[
    (
        "glo30",
        "copernicus-dem-30m.s3.amazonaws.com",
        "/Copernicus_DSM_COG_",
    ),
    (
        "worldcover",
        "esa-worldcover.s3.eu-central-1.amazonaws.com",
        "/v200/2021/map/",
    ),
];

/// Response size cap for a single relayed body (~128 MiB): enough for the largest
/// GLO-30 / WorldCover tile with headroom, but a hard DoS bound on a hostile or
/// runaway upstream (T-08-03-02). Enforced both from `Content-Length` (pre-check)
/// and while streaming (cumulative).
const MAX_RELAY_BYTES: u64 = 128 * 1024 * 1024;

/// Validate `(source, path)` against the [`SOURCES`] allowlist and build the
/// **hardcoded-host** upstream URL — the single SSRF chokepoint (pure, testable,
/// no network I/O).
///
/// `path` is the `{*path}` wildcard capture (no leading slash). The reconstructed
/// upstream path (`/{path}`) must start with the source's allowlisted prefix and
/// contain no `..` traversal segment. The returned URL always uses the hardcoded
/// `https` scheme and the allowlisted host — the caller never supplies a host.
///
/// # Errors
/// - [`ApiError::NotFound`] — `source` is not in the allowlist.
/// - [`ApiError::BadRequest`] — the path escapes the allowlisted prefix or contains
///   a `..` traversal. The `detail` is generic (no host leak).
pub fn resolve_upstream(source: &str, path: &str) -> Result<String, ApiError> {
    // (1) Allowlist lookup — unknown source is a 404 before any I/O.
    let (_, host, prefix) = SOURCES
        .iter()
        .find(|(id, _, _)| *id == source)
        .ok_or(ApiError::NotFound)?;

    // (2) Prefix + traversal guard on the reconstructed upstream path.
    let upstream_path = format!("/{path}");
    if path.contains("..") || !upstream_path.starts_with(prefix) {
        return Err(ApiError::BadRequest {
            detail: "path outside allowlisted prefix".to_string(),
        });
    }

    // (3) Build the URL from the HARDCODED scheme + host + validated path only.
    Ok(format!("https://{host}{upstream_path}"))
}

/// `GET /api/v1/proxy/{source}/{*path}` — relay the allowlisted upstream bytes.
///
/// Rejects unknown source / prefix-escape / traversal via [`resolve_upstream`]
/// BEFORE issuing any outbound request, forwards a GET (passing a `Range` header
/// through verbatim), and streams the 200/206 body back under [`MAX_RELAY_BYTES`]
/// with the upstream content headers. Non-2xx upstream and transport faults are
/// logged in full and surfaced as a generic `500` (MED-1).
///
/// # Errors
/// - `404` unknown source, `400` prefix escape / traversal (before any I/O).
/// - `500` (generic) on transport failure, non-2xx upstream, oversized body, or a
///   response-build failure — the upstream host/path is never leaked.
pub async fn relay(
    State(state): State<Arc<AppState>>,
    Path((source, path)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    // Guard-first (SSRF): resolve + validate before touching the network.
    let url = resolve_upstream(&source, &path)?;

    // GET-only, with the client's redirect::Policy::none() (no cross-host bounce).
    let mut req = state.http.get(&url);
    if let Some(range) = headers.get(header::RANGE) {
        req = req.header(header::RANGE, range);
    }
    let upstream = req.send().await?; // From<reqwest::Error>: logs full, returns generic 500.

    // Only a successful GET/Range reply is relayed; anything else is a generic 500
    // (the real status is logged, never surfaced — MED-1).
    let status = upstream.status();
    if status != StatusCode::OK && status != StatusCode::PARTIAL_CONTENT {
        tracing::error!(%status, "proxy upstream returned non-2xx");
        return Err(ApiError::Internal {
            detail: "internal error".to_string(),
        });
    }

    // Size cap — reject early when the upstream advertises an oversized body.
    if let Some(len) = upstream.content_length()
        && len > MAX_RELAY_BYTES
    {
        tracing::error!(len, "proxy upstream body exceeds size cap");
        return Err(ApiError::Internal {
            detail: "internal error".to_string(),
        });
    }

    // Pass the upstream content headers through (before consuming `upstream`).
    let mut builder = Response::builder().status(status);
    for name in [
        header::CONTENT_TYPE,
        header::CONTENT_RANGE,
        header::ACCEPT_RANGES,
        header::CONTENT_LENGTH,
    ] {
        if let Some(value) = upstream.headers().get(&name) {
            builder = builder.header(name, value.clone());
        }
    }

    // Stream the body back, aborting if the cumulative size exceeds the cap even
    // when the upstream lied about (or omitted) Content-Length.
    let mut total: u64 = 0;
    let capped = upstream.bytes_stream().map(move |chunk| {
        let bytes = chunk.map_err(std::io::Error::other)?;
        total += bytes.len() as u64;
        if total > MAX_RELAY_BYTES {
            return Err(std::io::Error::other("relay response exceeds size cap"));
        }
        Ok(bytes)
    });

    builder.body(Body::from_stream(capped)).map_err(|e| {
        tracing::error!(error = %e, "proxy response build failed");
        ApiError::Internal {
            detail: "internal error".to_string(),
        }
    })
}
