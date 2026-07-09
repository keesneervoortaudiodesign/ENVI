//! Job status + live SSE progress + cancel handlers (SC5).
//!
//! Three routes back the SC5 state machine (06-RESEARCH Pattern 4):
//! `GET /jobs/{id}` (current status snapshot), `GET /jobs/{id}/events` (live SSE
//! stream of every status transition), and `DELETE /jobs/{id}` (cooperative
//! cancel). Every id route uses a `Path<Uuid>` extractor — the parse IS the
//! path-traversal gate (Pitfall 7); no handler takes a raw `Path<String>`.
//!
//! The SSE stream is a [`WatchStream`] over the job's `watch::Receiver` mapped to
//! JSON `data:` events, with a 15 s keep-alive. `watch` keeps only the latest
//! value, so a slow browser coalesces intermediate progress ticks but never
//! blocks the worker (Pitfall 6: assert milestones, not every tick).

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Sse;
use axum::response::sse::{Event, KeepAlive};
use tokio_stream::wrappers::WatchStream;
use tokio_stream::{Stream, StreamExt};
use uuid::Uuid;

use crate::error::ApiError;
use crate::jobs::{JobId, JobStatus};
use crate::state::AppState;

/// `GET /jobs/{id}` -> 200 the current [`JobStatus`] (404 for an unknown id).
pub async fn get_job(
    State(app): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<JobStatus>, ApiError> {
    let jobs = app.jobs.read().await;
    let handle = jobs.get(&JobId(id)).ok_or(ApiError::NotFound)?;
    let status = handle.status.borrow().clone();
    Ok(Json(status))
}

/// `GET /jobs/{id}/events` -> a `text/event-stream` SSE of every status change
/// (each `data:` line a [`JobStatus`] JSON), with a 15 s keep-alive. 404 for an
/// unknown id.
pub async fn job_events(
    State(app): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let rx = {
        let jobs = app.jobs.read().await;
        jobs.get(&JobId(id))
            .ok_or(ApiError::NotFound)?
            .status
            .clone()
    };
    let stream = WatchStream::new(rx).map(|status| {
        // JobStatus is a plain serde enum — serialization cannot fail.
        Ok(Event::default()
            .json_data(&status)
            .expect("JobStatus is always serializable"))
    });
    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

/// `DELETE /jobs/{id}` -> 202. Fires the cancel token if the job exists; a no-op
/// for an unknown or already-terminal job. **Idempotent**: repeated DELETE also
/// returns 202 (cancellation is a `CancellationToken::cancel`, itself idempotent).
pub async fn cancel_job(State(app): State<Arc<AppState>>, Path(id): Path<Uuid>) -> StatusCode {
    if let Some(handle) = app.jobs.read().await.get(&JobId(id)) {
        handle.cancel.cancel();
    }
    StatusCode::ACCEPTED
}
