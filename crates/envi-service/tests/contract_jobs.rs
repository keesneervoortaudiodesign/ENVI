//! Contract tests for the SC5 job state machine: live SSE progress to `Done`,
//! working cancel to `Cancelled`, and an observable `Failed(reason)` — oneshot
//! against the FULL app router, with the job submitted directly on the shared
//! `AppState` (06-RESEARCH Pattern 4, Pitfall 6).
//!
//! SSE bodies are read **incrementally** (frame by frame via `BodyExt::frame`),
//! never collected whole: the stream ends only when the worker's `watch::Sender`
//! drops at a terminal state, and every read is bounded by a `tokio::time::
//! timeout` so a regression fails loudly instead of hanging. Assertions check the
//! ordered MILESTONES (queued/running -> running(progress) -> terminal), never
//! every coalesced tick (Pitfall 6).

use std::time::Duration;

use axum::body::Body;
use axum::http::StatusCode;
use http_body_util::BodyExt;
use tokio::time::timeout;
use tower::ServiceExt; // oneshot

use envi_service::jobs::{StubJobSpec, submit_stub_job};

mod common;
use common::{app_of, delete, get, test_state};

/// Pull the next `JobStatus` JSON off an SSE body, draining `data:` lines from a
/// running buffer and reading more frames (each bounded by `secs`) as needed.
/// Returns `None` when the stream ends or a read times out.
async fn next_state(body: &mut Body, buf: &mut String, secs: u64) -> Option<serde_json::Value> {
    loop {
        // Emit any complete SSE block already buffered.
        if let Some(pos) = buf.find("\n\n") {
            let block: String = buf.drain(..pos + 2).collect();
            for line in block.lines() {
                if let Some(rest) = line.strip_prefix("data:")
                    && let Ok(v) = serde_json::from_str::<serde_json::Value>(rest.trim())
                {
                    return Some(v);
                }
            }
            continue; // keep-alive-only block; look for the next
        }
        // Otherwise read another frame (bounded so a hang fails loudly).
        let framed = timeout(Duration::from_secs(secs), body.frame())
            .await
            .ok()?;
        match framed {
            Some(Ok(frame)) => {
                if let Ok(data) = frame.into_data() {
                    buf.push_str(&String::from_utf8_lossy(&data));
                }
            }
            _ => return None,
        }
    }
}

fn state_of(v: &serde_json::Value) -> &str {
    v["state"].as_str().unwrap_or_default()
}

fn is_terminal(v: &serde_json::Value) -> bool {
    matches!(state_of(v), "done" | "failed" | "cancelled")
}

#[tokio::test]
async fn stub_job_streams_progress_to_done() {
    let (state, _tmp) = test_state();
    // ~8 steps x 25 ms ≈ 200 ms — long enough to reliably observe running ticks.
    let id = submit_stub_job(
        &state,
        StubJobSpec {
            steps: 8,
            step_ms: 25,
            fail_at: None,
        },
    )
    .await;
    let app = app_of(state.clone());

    let resp = app
        .oneshot(get(&format!("/api/v1/jobs/{}/events", id.0)))
        .await
        .expect("router responds");
    assert_eq!(resp.status(), StatusCode::OK, "SSE route is 200");
    let ct = resp
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(
        ct.contains("text/event-stream"),
        "SSE content-type is text/event-stream, got {ct:?}"
    );

    let mut body = resp.into_body();
    let mut buf = String::new();
    let mut states = Vec::new();
    while let Some(v) = next_state(&mut body, &mut buf, 5).await {
        let terminal = is_terminal(&v);
        states.push(v);
        if terminal {
            break;
        }
    }

    let seq: Vec<&str> = states.iter().map(state_of).collect();
    assert!(
        matches!(seq.first(), Some(&"queued") | Some(&"running")),
        "first observed milestone is queued or running, got {seq:?}"
    );
    assert!(
        states.iter().any(|v| state_of(v) == "running"
            && v["progress"].as_f64().is_some_and(|p| p > 0.0 && p <= 1.0)),
        "at least one running with progress in (0,1], got {seq:?}"
    );
    assert_eq!(
        seq.last(),
        Some(&"done"),
        "terminal milestone is done: {seq:?}"
    );

    // GET /jobs/{id} now echoes the terminal done state.
    let app = app_of(state.clone());
    let resp = app
        .oneshot(get(&format!("/api/v1/jobs/{}", id.0)))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(state_of(&v), "done", "GET job status is done");
}

#[tokio::test]
async fn stub_job_cancel_yields_cancelled() {
    let (state, _tmp) = test_state();
    // A long job so it is reliably still running when we cancel.
    let id = submit_stub_job(
        &state,
        StubJobSpec {
            steps: 200,
            step_ms: 20,
            fail_at: None,
        },
    )
    .await;

    // Observe it running over SSE.
    let resp = app_of(state.clone())
        .oneshot(get(&format!("/api/v1/jobs/{}/events", id.0)))
        .await
        .expect("router responds");
    assert_eq!(resp.status(), StatusCode::OK);
    let mut body = resp.into_body();
    let mut buf = String::new();

    let mut saw_running = false;
    for _ in 0..10 {
        match next_state(&mut body, &mut buf, 5).await {
            Some(v) if state_of(&v) == "running" => {
                saw_running = true;
                break;
            }
            Some(_) => continue,
            None => break,
        }
    }
    assert!(saw_running, "observed the job running before cancel");

    // DELETE cancels the token.
    let del = app_of(state.clone())
        .oneshot(delete(&format!("/api/v1/jobs/{}", id.0)))
        .await
        .unwrap();
    assert_eq!(del.status(), StatusCode::ACCEPTED, "DELETE is 202");

    // A cancelled milestone arrives on the same stream.
    let mut cancelled = false;
    for _ in 0..50 {
        match next_state(&mut body, &mut buf, 5).await {
            Some(v) => {
                if state_of(&v) == "cancelled" {
                    cancelled = true;
                    break;
                }
                if is_terminal(&v) {
                    break;
                }
            }
            None => break,
        }
    }
    assert!(cancelled, "the worker transitioned to cancelled");

    // DELETE is idempotent — a second cancel also returns 202.
    let del2 = app_of(state.clone())
        .oneshot(delete(&format!("/api/v1/jobs/{}", id.0)))
        .await
        .unwrap();
    assert_eq!(
        del2.status(),
        StatusCode::ACCEPTED,
        "repeated DELETE is still 202"
    );

    // GET echoes the terminal cancelled state.
    let resp = app_of(state.clone())
        .oneshot(get(&format!("/api/v1/jobs/{}", id.0)))
        .await
        .unwrap();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(state_of(&v), "cancelled", "GET job status is cancelled");
}

#[tokio::test]
async fn stub_job_failure_is_observable() {
    let (state, _tmp) = test_state();
    let id = submit_stub_job(
        &state,
        StubJobSpec {
            steps: 6,
            step_ms: 20,
            fail_at: Some(2),
        },
    )
    .await;

    let resp = app_of(state.clone())
        .oneshot(get(&format!("/api/v1/jobs/{}/events", id.0)))
        .await
        .expect("router responds");
    assert_eq!(resp.status(), StatusCode::OK);
    let mut body = resp.into_body();
    let mut buf = String::new();

    let mut failed_reason: Option<String> = None;
    while let Some(v) = next_state(&mut body, &mut buf, 5).await {
        if state_of(&v) == "failed" {
            failed_reason = Some(v["reason"].as_str().unwrap_or_default().to_string());
            break;
        }
        if is_terminal(&v) {
            break;
        }
    }
    let reason = failed_reason.expect("a failed milestone was observed");
    assert!(!reason.is_empty(), "failure carries a non-empty reason");

    // GET echoes the failure with its reason.
    let resp = app_of(state.clone())
        .oneshot(get(&format!("/api/v1/jobs/{}", id.0)))
        .await
        .unwrap();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(state_of(&v), "failed", "GET job status is failed");
    assert!(
        !v["reason"].as_str().unwrap_or_default().is_empty(),
        "GET failure echoes a reason"
    );
}
