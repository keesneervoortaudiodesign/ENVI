//! The shared application state (06-RESEARCH "AppState").
//!
//! [`AppState`] is designed to be shared as `Arc<AppState>` across every handler
//! (axum `State` extractor). In Phase 6 it holds only the [`ProjectStore`]; plan
//! 06-04 extends it with the job registry and the calc/tensor-stub registries —
//! the struct is the single seam those additions bolt onto.

use envi_store::project_dir::ProjectStore;

/// Process-wide application state, shared behind `Arc<AppState>`.
///
/// Handlers reach persistence through [`AppState::store`]; they never touch the
/// filesystem directly (storage logic lives in `envi-store`).
#[derive(Debug, Clone)]
pub struct AppState {
    /// The folder-backed project store (create/open/save/duplicate/delete +
    /// reopen-last, atomic writes).
    pub store: ProjectStore,

    // NOTE (plan 06-04): the job registry (`RwLock<HashMap<JobId, JobHandle>>`)
    // and the calc/tensor-stub registries attach here. Kept out of Phase 6 so the
    // recondition/recompute + SSE contracts land as a single coherent addition.
}

impl AppState {
    /// Build the state from an open [`ProjectStore`].
    #[must_use]
    pub fn new(store: ProjectStore) -> Self {
        Self { store }
    }
}
