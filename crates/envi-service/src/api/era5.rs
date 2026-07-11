//! `POST /api/v1/era5/import` — the flagged-off ERA5/CDS retrieval endpoint
//! (D-04/D-05, METX-02 transport groundwork). **Compiled only under
//! `--features era5`**; the default build has no ERA5 route (a contract test
//! asserts its absence).
//!
//! # What this ships now vs. what stays flagged off (D-04)
//!
//! This phase ships the *security posture* and the async-job shape for the CDS
//! retrieval, NOT the live multi-minute fetch:
//!
//! - **Derivation (live now).** The endpoint runs the pure-Rust
//!   [`envi_gis::era5`] wind×stability → Obukhov → occurrence-statistics
//!   derivation on a batch of already-retrieved ERA5 hours, on the Phase-6
//!   [`crate::jobs`] `JobStatus` state machine (Queued → Running → Done/Failed).
//!   No CDS key is needed for this path (the contract test drives it with
//!   committed synthetic hours).
//! - **Live CDS retrieval (flagged off).** A real queued CDS submission would go
//!   through [`resolve_cds_upstream`] — the SSRF chokepoint copied from
//!   `api/proxy.rs` with the CDS host **hardcoded** — using a `CDS_API_KEY` read
//!   from the **server environment only**. Absent a key, retrieval reports a
//!   generic "disabled" error and touches no network. Even with a key, the
//!   NetCDF decode is future work, so no partial data is fabricated.
//!
//! # Security (T-09-04-01/02, load-bearing)
//!
//! - **SSRF:** [`resolve_cds_upstream`] validates the request path against a
//!   hardcoded prefix and rejects `..` traversal **before any I/O**; the upstream
//!   host is the hardcoded [`CDS_HOST`], never user-supplied — exactly the
//!   `proxy.rs` chokepoint shape.
//! - **Secret hygiene:** the CDS key lives in `env` only; it is sent to CDS via
//!   TLS `bearer_auth` and NEVER placed in a wire response, a log line, or the
//!   client bundle. Upstream/transport faults map to a generic `500` (the
//!   `From<reqwest::Error>` boundary logs full server-side, leaks nothing).

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::error::ApiError;
use crate::jobs::{JobHandle, JobId, JobStatus};
use crate::state::AppState;

// Re-export the derivation types so the (feature-gated) contract test can drive
// the derivation without a separate envi-gis dev-dependency.
pub use envi_gis::GisError;
pub use envi_gis::era5::{ClassOccurrence, Era5Hour};

/// The env var holding the Copernicus CDS API key. Read on the server ONLY; the
/// value never crosses the wire, a log line, or the client bundle (T-09-04-02).
pub const CDS_API_KEY_ENV: &str = "CDS_API_KEY";

/// The hardcoded Copernicus CDS host — the ENTIRE set of hosts the retrieval may
/// reach (V10 SSRF control). The caller never supplies a host; scheme is always
/// `https`. Mirrors the `proxy.rs` `SOURCES` allowlist shape, pinned to one host.
pub const CDS_HOST: &str = "cds.climate.copernicus.eu";

/// The allowlisted upstream path prefix on [`CDS_HOST`]. A request path that does
/// not start with this (or contains `..`) is rejected before any I/O.
pub const CDS_PATH_PREFIX: &str = "/api/";

/// One ERA5 single-level hour on the wire (request-facing). Mirrors
/// [`Era5Hour`]'s fields; kept a local `Deserialize` DTO so the endpoint accepts
/// already-retrieved hours without deriving serde on the engine-adjacent core
/// type. `deny_unknown_fields` so a typo'd key is a loud `400`.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Era5HourInput {
    /// Eastward turbulent surface stress `iews`, N/m².
    pub iews: f64,
    /// Northward turbulent surface stress `inss`, N/m².
    pub inss: f64,
    /// Instantaneous surface sensible heat flux `ishf`, W/m² (downward-positive).
    pub ishf: f64,
    /// 2 m air temperature `2t`, K.
    pub t2m_k: f64,
    /// 2 m dewpoint temperature `2d`, K.
    pub d2m_k: f64,
    /// Surface pressure `sp`, Pa.
    pub sp_pa: f64,
    /// Std-dev of sub-grid orography `sdfor`, m (reliability gate).
    pub sdfor_m: f64,
    /// Eastward 10 m wind component `u10`, m/s.
    pub u10_ms: f64,
    /// Northward 10 m wind component `v10`, m/s.
    pub v10_ms: f64,
}

impl Era5HourInput {
    /// Marshal into the engine-adjacent [`Era5Hour`] (pure field copy).
    #[must_use]
    pub fn to_hour(self) -> Era5Hour {
        Era5Hour {
            iews: self.iews,
            inss: self.inss,
            ishf: self.ishf,
            t2m_k: self.t2m_k,
            d2m_k: self.d2m_k,
            sp_pa: self.sp_pa,
            sdfor_m: self.sdfor_m,
            u10_ms: self.u10_ms,
            v10_ms: self.v10_ms,
        }
    }
}

/// `POST /era5/import` request body: either an inline batch of already-retrieved
/// ERA5 `hours` (the derivation path, no CDS key needed) or, when `hours` is
/// empty, a flagged live retrieval of `dataset_path` from the hardcoded CDS host.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Era5ImportReq {
    /// Already-retrieved ERA5 hours to derive occurrence statistics from. When
    /// non-empty, no network retrieval happens.
    #[serde(default)]
    pub hours: Vec<Era5HourInput>,
    /// The CDS API path to retrieve when `hours` is empty (validated by
    /// [`resolve_cds_upstream`] — the host is hardcoded, the path is checked).
    #[serde(default)]
    pub dataset_path: String,
}

/// `POST /era5/import` response: the id of the derivation job on the SC5 state
/// machine (poll `GET /jobs/{id}` / stream `GET /jobs/{id}/events`).
#[derive(Debug, Clone, Serialize)]
pub struct Era5ImportResponse {
    /// The async derivation job driving the ERA5 occurrence-statistics compute.
    pub job_id: Uuid,
}

/// Validate a CDS request `path` against the hardcoded host + allowlisted prefix
/// and build the upstream URL — the single SSRF chokepoint (pure, no I/O). The
/// exact `proxy.rs::resolve_upstream` shape, pinned to one host.
///
/// # Errors
/// - [`ApiError::BadRequest`] — the path escapes [`CDS_PATH_PREFIX`] or contains a
///   `..` traversal. The `detail` is generic (no host leak, T-09-04-02).
pub fn resolve_cds_upstream(path: &str) -> Result<String, ApiError> {
    let upstream_path = format!("/{path}");
    if path.contains("..") || !upstream_path.starts_with(CDS_PATH_PREFIX) {
        return Err(ApiError::BadRequest {
            detail: "path outside allowlisted prefix".to_string(),
        });
    }
    // Build from the HARDCODED scheme + host + validated path only (no user host).
    Ok(format!("https://{CDS_HOST}{upstream_path}"))
}

/// Run the pure-Rust ERA5 occurrence-statistics derivation over a batch of hours
/// (the flagged endpoint's compute payload, D-05 — occurrence statistics only).
///
/// # Errors
/// Propagates [`GisError`] for a non-finite / degenerate hour (never a panic).
pub fn derive_occurrence(hours: &[Era5Hour]) -> Result<ClassOccurrence, GisError> {
    envi_gis::era5::occurrence_stats(hours)
}

/// The flagged live CDS retrieval (D-04, disabled by default). Reads the CDS key
/// from the server env ONLY, builds the SSRF-safe upstream URL, and issues the
/// authenticated request through the shared client. Absent a key, it reports a
/// generic "disabled" error and touches no network.
///
/// # Errors
/// - [`ApiError::BadRequest`] — no server-side `CDS_API_KEY` (live retrieval
///   disabled) or a path that fails the SSRF chokepoint.
/// - [`ApiError::Internal`] — any upstream/transport fault (generic `500`; the
///   `From<reqwest::Error>` boundary logs full server-side, leaks no host/path),
///   or the not-yet-implemented NetCDF decode.
async fn retrieve_era5_hours(
    app: &AppState,
    req: &Era5ImportReq,
) -> Result<Vec<Era5Hour>, ApiError> {
    // Secret hygiene: the CDS key lives in the server env ONLY. Absent it, live
    // retrieval is disabled (the flagged-off state) — a generic message, no leak.
    let key = std::env::var(CDS_API_KEY_ENV).map_err(|_| ApiError::BadRequest {
        detail: "ERA5 live retrieval is disabled".to_string(),
    })?;

    // SSRF chokepoint: hardcoded host, validated path, BEFORE any request.
    let url = resolve_cds_upstream(&req.dataset_path)?;

    // Issue the authenticated request through the shared (redirect-none, timeout)
    // client. `bearer_auth` sends the key to CDS over TLS only — it is never
    // logged or surfaced. `From<reqwest::Error>` maps any fault to a generic 500.
    let _resp = app.http.get(&url).bearer_auth(&key).send().await?;

    // The queued-job NetCDF retrieval + decode is future work (D-04); do not
    // fabricate partial data. Report a generic internal error.
    Err(ApiError::Internal {
        detail: "internal error".to_string(),
    })
}

/// Spawn the ERA5 occurrence-statistics derivation as a job on the SC5 state
/// machine (Anti-Pattern 5: a dedicated `std::thread`, never the tokio pool). The
/// worker drives `Queued → Running → Done` (or `Failed` on a degenerate hour).
async fn submit_era5_job(state: &AppState, hours: Vec<Era5Hour>) -> JobId {
    let id = JobId(Uuid::new_v4());
    let (tx, rx) = watch::channel(JobStatus::Queued);
    let token = CancellationToken::new();
    let worker_token = token.clone();

    std::thread::spawn(move || run_era5_job(&tx, &worker_token, &hours));

    state.jobs.write().await.insert(
        id,
        JobHandle {
            status: rx,
            cancel: token,
        },
    );
    id
}

/// The ERA5 derivation worker (dedicated thread). Emits a `Running` tick, honors
/// a cooperative cancel, runs [`derive_occurrence`], and lands on `Done`
/// (success) or `Failed` (a degenerate hour — generic reason, no leak).
fn run_era5_job(tx: &watch::Sender<JobStatus>, token: &CancellationToken, hours: &[Era5Hour]) {
    tx.send_replace(JobStatus::Running {
        progress: 0.0,
        message: "deriving ERA5 occurrence statistics".to_string(),
    });
    if token.is_cancelled() {
        tx.send_replace(JobStatus::Cancelled);
        return;
    }
    // A tiny yield so a DELETE landing immediately is observed as Cancelled.
    std::thread::sleep(Duration::from_millis(1));
    if token.is_cancelled() {
        tx.send_replace(JobStatus::Cancelled);
        return;
    }
    match derive_occurrence(hours) {
        Ok(_occ) => tx.send_replace(JobStatus::Done),
        Err(_) => tx.send_replace(JobStatus::Failed {
            reason: "ERA5 derivation failed".to_string(),
        }),
    };
}

/// `POST /era5/import` → `202 { job_id }`. Gathers the ERA5 hours (inline, or a
/// flagged live retrieval when `hours` is empty) and spawns the derivation job.
///
/// # Errors
/// - `400` when live retrieval is requested but disabled (no server key) or the
///   `dataset_path` fails the SSRF chokepoint.
/// - `500` (generic) on an upstream/transport fault.
pub async fn import_era5(
    State(app): State<Arc<AppState>>,
    Json(req): Json<Era5ImportReq>,
) -> Result<(StatusCode, Json<Era5ImportResponse>), ApiError> {
    let hours: Vec<Era5Hour> = if req.hours.is_empty() {
        // Flagged-off live CDS retrieval (disabled by default).
        retrieve_era5_hours(&app, &req).await?
    } else {
        req.hours.iter().map(|h| h.to_hour()).collect()
    };

    let job_id = submit_era5_job(&app, hours).await;
    Ok((
        StatusCode::ACCEPTED,
        Json(Era5ImportResponse { job_id: job_id.0 }),
    ))
}
