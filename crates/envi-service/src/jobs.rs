//! The SC5 job state machine: registry + dedicated worker thread + live progress
//! (06-RESEARCH Pattern 4).
//!
//! # Anti-Pattern 5 — the worker runs on a dedicated `std::thread` (D-08)
//!
//! The stub worker deliberately runs on a **dedicated OS thread**
//! ([`std::thread::spawn`]), NEVER on tokio's blocking-task pool or the async
//! runtime. That pool is sized for *bounded* blocking I/O; the hour-scale Phase-9
//! Nord2000 solves this skeleton establishes the shape for are unbounded CPU —
//! parking them on the blocking pool would starve the pool that request I/O
//! shares and degrade SSE-progress and cancel latency (architecture
//! Anti-Pattern 5). The Phase-9 real solve inherits this exact boundary: rayon
//! goes *inside* the thread later; the `std::thread` boundary stays.
//!
//! Both cross-thread primitives are runtime-free by design: `watch::Sender::
//! send_replace` never blocks on a slow consumer (it keeps only the latest value)
//! and never fails when every receiver has dropped (a fire-and-forget worker must
//! not die because the browser closed the tab); `CancellationToken::is_cancelled`
//! is a plain atomic load.
//!
//! # Registry lifetime
//!
//! Completed jobs stay in [`AppState::jobs`] — their terminal status remains
//! queryable through the retained `watch::Receiver`. An eviction policy (and a
//! submission semaphore bounding concurrent workers) is Phase-10 scope
//! (T-06-04-03); on localhost single-user load the unbounded map is an accepted,
//! documented growth.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;
use uuid::Uuid;

use crate::state::AppState;

/// A job identifier. Serializes transparently as its inner UUID (the wire shape
/// for `job_id` in the 202 calc responses).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export_to = "wire.ts")]
pub struct JobId(pub Uuid);

/// The SC5 job status — the **frozen wire shape**. Internally tagged on `state`
/// in `snake_case`, so the JSON is `{"state":"running","progress":0.5,"message":
/// "step 4"}` / `{"state":"done"}` / `{"state":"failed","reason":".."}` etc. The
/// literal tag values `queued`/`running`/`done`/`failed`/`cancelled` are the
/// contract Phase-7 binds its EventSource handling to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "snake_case")]
#[ts(export_to = "wire.ts")]
pub enum JobStatus {
    /// Submitted, worker not yet started.
    Queued,
    /// Running: `progress` in `(0, 1]`, `message` a human-readable step label.
    Running {
        /// Fractional progress, `(0, 1]`.
        progress: f32,
        /// Human-readable step label.
        message: String,
    },
    /// Completed successfully (terminal).
    Done,
    /// Failed with a reason (terminal).
    Failed {
        /// Non-empty failure reason.
        reason: String,
    },
    /// Cancelled cooperatively via the token (terminal).
    Cancelled,
}

impl JobStatus {
    /// Whether this is a terminal state (`Done`/`Failed`/`Cancelled`).
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobStatus::Done | JobStatus::Failed { .. } | JobStatus::Cancelled
        )
    }
}

/// A registry entry: the clone-source of the live status stream plus the cancel
/// token. SSE subscribers clone [`JobHandle::status`]; `DELETE /jobs/{id}` fires
/// [`JobHandle::cancel`].
#[derive(Debug)]
pub struct JobHandle {
    /// The latest-value status channel (clone per SSE subscriber).
    pub status: watch::Receiver<JobStatus>,
    /// Cooperative cancellation token, checked each worker step.
    pub cancel: CancellationToken,
}

/// The synthetic stub-job shape: `steps` fake work-steps, `step_ms` per step, and
/// an optional `fail_at` step index that drives the worker into `Failed`.
#[derive(Debug, Clone, Copy)]
pub struct StubJobSpec {
    /// Number of work-steps.
    pub steps: u32,
    /// Milliseconds slept per step (the fake work).
    pub step_ms: u64,
    /// If `Some(k)`, the worker fails at step `k` instead of continuing.
    pub fail_at: Option<u32>,
}

/// Submit a stub job: register a fresh [`JobHandle`] and spawn the dedicated
/// worker thread that drives `Queued -> Running(progress) -> Done` (or
/// `-> Cancelled` / `-> Failed`). Returns the new [`JobId`].
///
/// The worker is a detached `std::thread` (Anti-Pattern 5); its `watch::Sender`
/// lives on the worker stack, so when the worker reaches a terminal state and
/// returns, the sender drops and every SSE stream ends after delivering the final
/// status.
pub async fn submit_stub_job(state: &AppState, spec: StubJobSpec) -> JobId {
    let id = JobId(Uuid::new_v4());
    let (tx, rx) = watch::channel(JobStatus::Queued);
    let token = CancellationToken::new();
    let worker_token = token.clone();

    // Dedicated OS thread — NEVER the tokio blocking pool / async pool (Anti-Pattern 5).
    std::thread::spawn(move || run_stub_job(&tx, &worker_token, spec));

    state.jobs.write().await.insert(
        id,
        JobHandle {
            status: rx,
            cancel: token,
        },
    );
    id
}

/// The worker loop (runs on the dedicated thread). Emits a `Running` per step via
/// `send_replace`, checks the cancel token each iteration, honors `fail_at`, and
/// lands on `Done` after the final step.
fn run_stub_job(tx: &watch::Sender<JobStatus>, token: &CancellationToken, spec: StubJobSpec) {
    tx.send_replace(JobStatus::Running {
        progress: 0.0,
        message: "started".to_string(),
    });

    let steps = spec.steps.max(1);
    for step in 0..steps {
        if token.is_cancelled() {
            tx.send_replace(JobStatus::Cancelled);
            return;
        }
        std::thread::sleep(Duration::from_millis(spec.step_ms));
        if spec.fail_at == Some(step) {
            tx.send_replace(JobStatus::Failed {
                reason: format!("stub job failed at step {step}"),
            });
            return;
        }
        // One more cancel check after the sleep so a DELETE landing mid-step is
        // observed promptly rather than being overwritten by a Running tick.
        if token.is_cancelled() {
            tx.send_replace(JobStatus::Cancelled);
            return;
        }
        let progress = f64::from(step + 1) as f32 / steps as f32;
        tx.send_replace(JobStatus::Running {
            progress,
            message: format!("step {}", step + 1),
        });
    }

    tx.send_replace(JobStatus::Done);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_status_wire_shape_is_state_tagged_snake_case() {
        let running = serde_json::to_value(JobStatus::Running {
            progress: 0.5,
            message: "step 4".to_string(),
        })
        .expect("serialize");
        assert_eq!(running["state"], "running");
        assert_eq!(running["progress"], 0.5);
        assert_eq!(running["message"], "step 4");

        assert_eq!(
            serde_json::to_value(JobStatus::Queued).unwrap()["state"],
            "queued"
        );
        assert_eq!(
            serde_json::to_value(JobStatus::Done).unwrap()["state"],
            "done"
        );
        assert_eq!(
            serde_json::to_value(JobStatus::Cancelled).unwrap()["state"],
            "cancelled"
        );
        let failed = serde_json::to_value(JobStatus::Failed {
            reason: "boom".to_string(),
        })
        .unwrap();
        assert_eq!(failed["state"], "failed");
        assert_eq!(failed["reason"], "boom");
    }

    #[test]
    fn terminal_predicate() {
        assert!(!JobStatus::Queued.is_terminal());
        assert!(
            !JobStatus::Running {
                progress: 0.1,
                message: String::new()
            }
            .is_terminal()
        );
        assert!(JobStatus::Done.is_terminal());
        assert!(JobStatus::Cancelled.is_terminal());
        assert!(
            JobStatus::Failed {
                reason: String::new()
            }
            .is_terminal()
        );
    }
}
