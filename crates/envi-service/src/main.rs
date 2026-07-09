//! `envi-service` — the ENVI self-hosted deployable.
//!
//! Usage: `cargo run -p envi-service`
//!
//! Startup order (D-08): init tracing -> run the pure-Rust CRS self-check and
//! **refuse to start** on failure -> resolve config from the environment (bind,
//! projects root, web bundle) -> build the router -> bind localhost -> serve.
//!
//! Env vars: `ENVI_BIND` (default `127.0.0.1:8080`), `ENVI_PROJECTS_DIR`
//! (default `./projects`), `ENVI_WEB_DIST` (default `web/dist`).
#![deny(unsafe_code)]

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::routing::get;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use envi_service::error::ApiError;
use envi_service::selfcheck;
use envi_service::state::AppState;

/// Default bind address — **loopback only** (SVC-04). Only an explicit
/// `ENVI_BIND` override may widen it, and doing so logs a prominent warning.
const DEFAULT_BIND: &str = "127.0.0.1:8080";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    // D-08: refuse to start unless the pure-Rust CRS round-trip self-check passes.
    match selfcheck::crs_self_check() {
        Ok(report) => tracing::info!(
            utm_zone = report.utm_zone,
            south = report.south,
            err_m = report.err_m,
            "CRS self-check passed (pure-Rust proj4rs, UTM zone {}{})",
            report.utm_zone,
            if report.south { "S" } else { "N" }
        ),
        Err(e) => {
            tracing::error!("CRS self-check FAILED — refusing to start: {e}");
            return Err(e.into());
        }
    }

    // Resolve config from the environment.
    let bind = resolve_bind()?;
    if !bind.ip().is_loopback() {
        tracing::warn!(
            %bind,
            "ENVI_BIND is NOT a loopback address — the service has NO authentication \
             and is now reachable beyond localhost (SVC-04, accepted no-auth posture)"
        );
    }

    let projects_dir = resolve_projects_dir();
    std::fs::create_dir_all(&projects_dir)?;
    let projects_display = projects_dir
        .canonicalize()
        .unwrap_or_else(|_| projects_dir.clone());
    tracing::info!(projects_dir = %projects_display.display(), "projects root");

    let web_dist = resolve_web_dist();
    if web_dist.is_dir() {
        tracing::info!(web_dist = %web_dist.display(), "serving static bundle");
    } else {
        tracing::warn!(
            web_dist = %web_dist.display(),
            "web/dist not found — the placeholder bundle will not be served (the API still works)"
        );
    }

    let store = envi_store::project_dir::ProjectStore::new(projects_dir)?;
    let state = Arc::new(AppState::new(store));

    // NOTE: Task 2 replaces this scaffold router with `envi_service::api::app`.
    // For now a single health route keeps the binary a runnable, testable shell.
    let app = Router::new()
        .route("/", get(scaffold_health))
        .with_state(state);

    let listener = TcpListener::bind(bind).await?;
    tracing::info!(%bind, "envi-service listening");
    axum::serve(listener, app).await?;
    Ok(())
}

/// Initialize `tracing` with an env filter (RUST_LOG-aware), defaulting to `info`.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

/// Resolve the bind address from `ENVI_BIND`, defaulting to loopback.
fn resolve_bind() -> Result<SocketAddr, Box<dyn std::error::Error>> {
    let raw = std::env::var("ENVI_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string());
    Ok(raw.parse::<SocketAddr>()?)
}

/// Resolve the projects root from `ENVI_PROJECTS_DIR`, defaulting to `./projects`.
fn resolve_projects_dir() -> PathBuf {
    std::env::var_os("ENVI_PROJECTS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./projects"))
}

/// Resolve the web bundle root from `ENVI_WEB_DIST`, defaulting to `web/dist`.
fn resolve_web_dist() -> PathBuf {
    std::env::var_os("ENVI_WEB_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("web/dist"))
}

/// Scaffold health handler (Task 1 only): confirms the store is reachable.
/// Replaced by the real router in Task 2.
async fn scaffold_health(State(app): State<Arc<AppState>>) -> Result<String, ApiError> {
    let n = app.store.list()?.len();
    Ok(format!("envi-service scaffold: {n} projects"))
}
