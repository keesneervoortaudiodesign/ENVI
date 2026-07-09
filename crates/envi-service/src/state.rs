//! The shared application state (06-RESEARCH "AppState").
//!
//! [`AppState`] is shared as `Arc<AppState>` across every handler (axum `State`
//! extractor). Plan 06-03 gave it the [`ProjectStore`]; plan 06-04 bolts on the
//! two in-memory registries that back the walking-skeleton compute contracts:
//!
//! - [`AppState::jobs`] — the SC5 job state machine registry (`JobId -> JobHandle`),
//!   each handle carrying a `watch::Receiver<JobStatus>` (live progress) and a
//!   `CancellationToken` (cooperative cancel). See [`crate::jobs`].
//! - [`AppState::calcs`] — the in-memory `calc_id -> project_id` index (SC4,
//!   D-07). It resolves a `calc_id` to its owning project and nothing else:
//!   **tensor identity is always derived on read** from the current
//!   scene/met/receivers, never cached here. [`CalcRecord`] lives here.
//!
//! Both registries live behind an `Arc<RwLock<..>>` so `AppState` stays `Clone`
//! (it is always shared as `Arc<AppState>`; the inner `Arc` keeps the derive and
//! costs nothing meaningful for a single-user localhost service).

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use uuid::Uuid;

use envi_store::project_dir::ProjectStore;

use crate::jobs::{JobHandle, JobId};

/// The in-memory registry entry for one calculation: a `calc_id -> project_id`
/// index and nothing more (D-07).
///
/// **Tensor identity is always derived on read** — the `recondition` 409 gate
/// re-mints the content hash (geometry + met + receivers) from the CURRENT
/// scene/met/receivers per request via `load_and_mint`, and the persisted
/// `calc/<id>/manifest.json` holds the authoritative stored hash. No cached hash
/// is kept here, and **no cached hash may ever be used for the 409 decision** —
/// that is what keeps a scene edit from leaving a stale identity reconditionable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalcRecord {
    /// The project this calculation belongs to.
    pub project_id: Uuid,
}

/// Process-wide application state, shared behind `Arc<AppState>`.
///
/// Handlers reach persistence through [`AppState::store`] (they never touch the
/// filesystem directly — storage logic lives in `envi-store`), and the compute
/// contracts through the two registries below.
#[derive(Debug, Clone)]
pub struct AppState {
    /// The folder-backed project store (create/open/save/duplicate/delete +
    /// reopen-last, atomic writes).
    pub store: ProjectStore,
    /// The SC5 job registry: `JobId -> JobHandle { watch::Receiver, cancel }`.
    /// Completed jobs stay queryable (terminal status readable via the receiver);
    /// registry eviction is Phase-10 business (T-06-04-03).
    pub jobs: Arc<RwLock<HashMap<JobId, JobHandle>>>,
    /// The in-memory calc registry: a `calc_id -> project_id` index (SC4, D-07).
    /// It carries no cached identity — the `recondition` 409 gate always re-mints
    /// the tensor hash from the current scene per request (`load_and_mint`).
    pub calcs: Arc<RwLock<HashMap<Uuid, CalcRecord>>>,
}

impl AppState {
    /// Build the state from an open [`ProjectStore`] with empty registries.
    #[must_use]
    pub fn new(store: ProjectStore) -> Self {
        Self {
            store,
            jobs: Arc::new(RwLock::new(HashMap::new())),
            calcs: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
