//! The shared application state (06-RESEARCH "AppState").
//!
//! [`AppState`] is shared as `Arc<AppState>` across every handler (axum `State`
//! extractor). Plan 06-03 gave it the [`ProjectStore`]; plan 06-04 bolts on the
//! two in-memory registries that back the walking-skeleton compute contracts:
//!
//! - [`AppState::jobs`] — the SC5 job state machine registry (`JobId -> JobHandle`),
//!   each handle carrying a `watch::Receiver<JobStatus>` (live progress) and a
//!   `CancellationToken` (cooperative cancel). See [`crate::jobs`].
//! - [`AppState::calcs`] — the in-memory **stub tensor** registry (`calc_id ->
//!   CalcRecord`) resolving a `calc_id` to its owning project + last-minted
//!   identity (SC4, D-07). The `recondition` 409 gate re-mints identity from the
//!   current scene per request (HIGH-1a) rather than trusting the cached hash;
//!   the cached `tensor_hash` is kept fresh on scene edits (HIGH-1b) so it never
//!   lags disk. [`CalcRecord`] lives here.
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

/// The in-memory **stub tensor** identity record for one calculation (D-07). It
/// carries only what the `recondition` 409 gate and the `[S, R, 105]` spectra
/// readout need: the owning project, the content `tensor_hash` (geometry + met +
/// receivers — conditioning excluded), and the tensor `dims`. The real tensor
/// payload is Phase 9-11; here identity alone is the frozen contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalcRecord {
    /// The project this calculation belongs to.
    pub project_id: Uuid,
    /// The frozen content hash keying tensor identity (see
    /// `envi_store::hash::tensor_hash`).
    pub tensor_hash: String,
    /// `[S, R, 105]` — sub-source count, receiver count, band count.
    pub dims: [usize; 3],
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
    /// The in-memory stub-tensor registry: `calc_id -> CalcRecord` (project id +
    /// last-minted `tensor_hash` + dims). The `recondition` 409 gate re-mints
    /// identity from the current scene per request (HIGH-1a); this map resolves
    /// `calc_id -> project_id` and caches the last identity (SC4, D-07).
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
