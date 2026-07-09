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
//! The axum default request-body limit (~2 MB) is left in place: it is ample for
//! authored Nord2000 scenes (hundreds of features) and, with the strict
//! `deny_unknown_fields` DTOs, bounds malformed-body DoS. No streaming uploads
//! exist in Phase 6.
//!
//! # SPA fallback (SVC-03)
//!
//! Non-API paths fall through to `ServeDir(web/dist)` with an `index.html`
//! fallback, so frontend deep links resolve to the SPA shell. The `/api/v1`
//! namespace has its OWN 404 fallback ([`api_fallback`]) so unknown API paths
//! return structured JSON, never the HTML shell.

pub mod meta;
pub mod projects;
pub mod scene;

use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use tower_http::services::{ServeDir, ServeFile};

use crate::error::ApiError;
use crate::state::AppState;

/// Build the `/api/v1` sub-router (state-generic; mounted by [`app`]).
///
/// All routes use axum 0.8 brace path syntax. `/projects/last` is registered
/// before `/projects/{id}` so the literal `last` segment is never captured as a
/// uuid param (matchit prioritizes static segments, but the ordering is explicit
/// for clarity). The router carries its own 404 fallback so unmatched
/// `/api/v1/*` paths return JSON, not the SPA HTML.
pub fn api_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/meta/freq-axis", get(meta::freq_axis))
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
            get(scene::get_scene).put(scene::put_scene),
        )
        .fallback(api_fallback)
}

/// Assemble the full application: `/api/v1` nested under a static-bundle
/// fallback service (SPA deep-link support), with the shared state attached.
///
/// `web_dist` is the directory holding `index.html` (default `web/dist`).
pub fn app(state: Arc<AppState>, web_dist: &Path) -> Router {
    let serve_dir = ServeDir::new(web_dist).fallback(ServeFile::new(web_dist.join("index.html")));
    Router::new()
        .nest("/api/v1", api_router())
        .fallback_service(serve_dir)
        .with_state(state)
}

/// 404 fallback for the `/api/v1` namespace — structured JSON, never HTML.
async fn api_fallback() -> ApiError {
    ApiError::NotFound
}
