//! The `/api/v1` router assembly + static-bundle serving (SVC-03).
//!
//! # Path syntax (Pitfall 2 — load-bearing)
//!
//! Every parameterized route uses the axum 0.8 **brace** syntax `/{id}`. The 0.7
//! colon syntax `/:id` panics at router construction, so the contract tests build
//! the FULL [`app`] router — any straggler panics the whole suite immediately.
//!
//! # Request body limit (Security Domain V5, accepted)
//!
//! The axum default request-body limit (~2 MB) is left in place globally: it is
//! ample for authored Nord2000 scenes (hundreds of features) and, with the strict
//! `deny_unknown_fields` DTOs, bounds malformed-body DoS.
//!
//! The one exception is the whole-scene PUT (`/projects/{id}/scene`): a Phase-8
//! GIS import decimates terrain to thousands of `elevation_point` features per
//! tile plus land-cover polygons, so a multi-tile import serializes tens of
//! thousands of features — well past 2 MB. That single route carries a raised
//! [`SCENE_BODY_LIMIT`] so a genuine import persists instead of being silently
//! rejected (WR-01); every other route keeps the tight default. The limit is still
//! bounded (not unlimited), so it caps import-body DoS.
//!
//! # SPA fallback (SVC-03)
//!
//! Non-API paths fall through to `ServeDir(web/dist)` with an `index.html`
//! fallback, so frontend deep links resolve to the SPA shell. The `/api/v1`
//! namespace has its OWN 404 fallback ([`api_fallback`]) so unknown API paths
//! return structured JSON, never the HTML shell.

pub mod calc;
pub mod dgm;
// D-04: the flagged ERA5/CDS retrieval endpoint compiles ONLY under `--features
// era5`. The default build has no `era5` module and no route (contract-tested).
#[cfg(feature = "era5")]
pub mod era5;
pub mod jobs;
pub mod meta;
pub mod projects;
pub mod proxy;
pub mod scene;

use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::HeaderValue;
use axum::http::header::HeaderName;
use axum::routing::{get, post};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;

use crate::error::ApiError;
use crate::state::AppState;

/// Request-body limit for the whole-scene PUT only (WR-01). A GIS import can
/// serialize tens of thousands of decimated terrain/land-cover features; 32 MiB
/// gives ample headroom while still bounding import-body DoS. Every other route
/// keeps the axum ~2 MB default.
const SCENE_BODY_LIMIT: usize = 32 * 1024 * 1024;

/// Build the `/api/v1` sub-router (state-generic; mounted by [`app`]).
///
/// All routes use axum 0.8 brace path syntax. `/projects/last` is registered
/// before `/projects/{id}` so the literal `last` segment is never captured as a
/// uuid param (matchit prioritizes static segments, but the ordering is explicit
/// for clarity). The router carries its own 404 fallback so unmatched
/// `/api/v1/*` paths return JSON, not the SPA HTML.
pub fn api_router() -> Router<Arc<AppState>> {
    let router = Router::new()
        .route("/meta/freq-axis", get(meta::freq_axis))
        .route(
            "/meta/interpolate-spectrum",
            post(meta::interpolate_spectrum),
        )
        .route("/meta/spl-to-lw", post(meta::spl_to_lw_handler))
        .route("/dgm/triangulate", post(dgm::triangulate))
        .route("/projects", get(projects::list).post(projects::create))
        .route("/projects/last", get(projects::last))
        .route(
            "/projects/{id}",
            get(projects::get)
                .put(projects::update)
                .delete(projects::delete),
        )
        .route("/projects/{id}/duplicate", post(projects::duplicate))
        .route(
            "/projects/{id}/scene",
            get(scene::get_scene)
                .put(scene::put_scene)
                // Scope the raised limit to this route's handlers only (WR-01).
                .layer(DefaultBodyLimit::max(SCENE_BODY_LIMIT)),
        )
        .route("/projects/{id}/calculations", post(calc::submit))
        .route("/calculations/{cid}/recondition", post(calc::recondition))
        .route("/calculations/{cid}/recompute", post(calc::recompute))
        .route("/jobs/{id}", get(jobs::get_job).delete(jobs::cancel_job))
        .route("/jobs/{id}/events", get(jobs::job_events))
        // D-02 allowlisted byte relay: GET-only (any other method -> 405), axum 0.8
        // brace + `{*path}` wildcard captures the full upstream key segment.
        .route("/proxy/{source}/{*path}", get(proxy::relay));

    // D-04: the flagged ERA5/CDS retrieval endpoint is registered ONLY under
    // `--features era5`. With the feature off the route is absent — an unknown
    // `/era5/import` falls through to `api_fallback` (404), contract-tested.
    #[cfg(feature = "era5")]
    let router = router.route("/era5/import", post(era5::import_era5));

    router.fallback(api_fallback)
}

/// Assemble the full application: `/api/v1` nested under a static-bundle
/// fallback service (SPA deep-link support), with the shared state attached.
///
/// `web_dist` is the directory holding `index.html` (default `web/dist`).
///
/// # Cross-origin isolation (SVC-02, D-04)
///
/// The bundle response carries `Cross-Origin-Opener-Policy: same-origin` +
/// `Cross-Origin-Embedder-Policy: credentialless` so the served top-level
/// document is cross-origin isolated (`self.crossOriginIsolated === true`),
/// which is the platform prerequisite for `SharedArrayBuffer` and the
/// wasm-bindgen-rayon thread pool the client-side solve spawns. The COEP value
/// is deliberately **`credentialless`, not `require-corp`**: credentialless
/// strips credentials on no-cors sub-resource loads, so the Phase-8 direct
/// third-party fetches (basemap/AHN/Overpass) keep working without every source
/// having to emit a CORP header — `require-corp` would break them. The layers
/// wrap the whole router: harmless on `/api/v1` responses, load-bearing on the
/// fallback bundle response.
pub fn app(state: Arc<AppState>, web_dist: &Path) -> Router {
    let serve_dir = ServeDir::new(web_dist).fallback(ServeFile::new(web_dist.join("index.html")));
    let coop = SetResponseHeaderLayer::overriding(
        HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );
    let coep = SetResponseHeaderLayer::overriding(
        HeaderName::from_static("cross-origin-embedder-policy"),
        HeaderValue::from_static("credentialless"),
    );
    Router::new()
        .nest("/api/v1", api_router())
        .fallback_service(serve_dir)
        .layer(coop)
        .layer(coep)
        .with_state(state)
}

/// 404 fallback for the `/api/v1` namespace — structured JSON, never HTML.
async fn api_fallback() -> ApiError {
    ApiError::NotFound
}
