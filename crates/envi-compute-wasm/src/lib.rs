//! # envi-compute-wasm
//!
//! The **WASM boundary** for the client-side Nord2000 grid solve (SVC-02 /
//! GRID-02). A thin `cdylib` that exposes the pure [`envi_compute`] core (cost
//! model + hierarchical tier partition) and the OPFS-backed tensor store to the
//! browser over `wasm-bindgen`, mirroring the [`envi_gis_wasm`](../envi_gis_wasm)
//! cdylib discipline.
//!
//! # Boundary ONLY — no logic here (mirror of `envi-service`'s thin-handler rule)
//!
//! Every `#[wasm_bindgen]` function does exactly three things: deserialize a JS
//! value into a serde DTO ([`serde_wasm_bindgen`]), call the corresponding
//! [`envi_compute`] core function (or the FORCE-validated
//! `envi_engine::solver::solve`), and serialize the result back. **No cost/tier/
//! acoustic math lives here** — that all belongs to `envi-compute` and the engine.
//! The engine stays byte-identical (D-02): this crate adds nothing to the engine's
//! `ndarray + num-complex + thiserror` quarantine; the OPFS sink is a NEW impl of
//! the existing [`envi_engine::tensor::TensorSink`] trait, not an engine edit.
//!
//! # Wire-type discipline (Phase-7 D-10)
//!
//! All boundary DTOs live in [`dto`], derive `ts_rs::TS`, and are generated into
//! the committed `web/src/generated/wire.ts` with a no-drift test — no
//! hand-authored TS mirror. The reused `JobStatus` union the worker posts is the
//! SAME `envi_service::jobs::JobStatus` already in `wire.ts` (Phase 6/7); this
//! crate defines no second copy.
//!
//! # No `getrandom`/`uuid` (Pitfall 9)
//!
//! Receiver `id`s are assigned in TypeScript via `crypto.randomUUID()`; this crate
//! mints no ids and pulls in no random-number source. The tier layer emits integer
//! global indices only.
//!
//! # Threaded pool (feature `threads`, Pitfall 1)
//!
//! The `wasm-bindgen-rayon` pool + the `initThreadPool` glue are behind the
//! `threads` feature so the stable/native builds never compile the atomics
//! toolchain. Only the `build:wasm:compute` nightly + `-Zbuild-std` + atomics
//! script (10-03 Task 3) enables it. The rayon sharding driver lands in plan 10-04.
#![deny(unsafe_code)]

pub mod dto;
pub mod opfs_sink;
// The caller-side rayon sharding driver (GRID-02). Compiled for every NATIVE build
// (so `cargo test pool` exercises it) and for the THREADED wasm build; excluded
// only from the stable, single-threaded wasm build where rayon is absent.
#[cfg(any(not(target_arch = "wasm32"), feature = "threads"))]
pub mod pool;

use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use serde::de::DeserializeOwned;
use thiserror::Error;
use wasm_bindgen::prelude::*;

use dto::{
    CostEstimateResult, EstimateCostReq, GuardrailLevelDto, PlanTiersReq, SolveChunkRangeReq,
    TierDto, TierKindDto, TierPlanResult, TierReceiverDto,
};

/// Re-export the `wasm-bindgen-rayon` `initThreadPool` glue so the browser worker
/// can size the SharedArrayBuffer pool to `navigator.hardwareConcurrency` before
/// the first parallel solve (Pitfall 2 — await it once). Only present in the
/// threaded (`threads`-feature) build; the rayon driver that uses it lands in 10-04.
#[cfg(feature = "threads")]
pub use wasm_bindgen_rayon::init_thread_pool;

// --- Cooperative cancel flag (D-11) ---------------------------------------

/// The process-wide cooperative cancel flag. Because `wasm-bindgen-rayon`'s linear
/// memory IS a SharedArrayBuffer, this single atomic is visible to every pool
/// thread; the rayon driver (10-04) checks it between chunk ranges and stops
/// before the next `solve()` — abort at chunk boundaries, never `worker.terminate()`.
static CANCEL: AtomicBool = AtomicBool::new(false);

/// Request cooperative cancellation of the running solve (D-11). Sets the shared
/// flag the pool driver checks at each chunk boundary. Idempotent.
#[wasm_bindgen]
pub fn request_cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}

/// Clear the cancel flag before starting a fresh solve (the pool is reusable after
/// a cooperative abort — D-11). Called by the worker at submit time.
#[wasm_bindgen]
pub fn reset_cancel() {
    CANCEL.store(false, Ordering::SeqCst);
}

/// Whether cancellation has been requested (read by the 10-04 pool driver between
/// chunk ranges).
#[must_use]
pub fn is_cancel_requested() -> bool {
    CANCEL.load(Ordering::SeqCst)
}

// --- Marshalling helpers (the ONLY glue; no domain logic) -----------------

/// Deserialize a JS value into a request DTO, mapping a shape error to `JsValue`.
fn from_js<T: DeserializeOwned>(v: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(v).map_err(|e| js_err(&e.to_string()))
}

/// Serialize a result DTO back to a JS value (plain objects, not JS `Map`s — the
/// TS import path reads result DTOs as plain objects).
fn to_js<T: Serialize>(v: &T) -> Result<JsValue, JsValue> {
    let serializer = serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true);
    v.serialize(&serializer).map_err(|e| js_err(&e.to_string()))
}

/// Build a `JsValue` error carrying `msg` (an `Error` on the JS side).
fn js_err(msg: &str) -> JsValue {
    JsError::new(msg).into()
}

/// A typed boundary error (mirrors `envi-gis-wasm`'s `gis_err` discipline — the
/// boundary never panics on data). The rayon pool driver ([`pool`]) surfaces
/// `Cancelled`, `Sink`, and `Solve` through this type.
#[derive(Debug, Error)]
pub enum ComputeError {
    /// A boundary path not yet wired (e.g. the per-range scene marshalling).
    #[error("not yet wired: {0}")]
    Pending(&'static str),
    /// Cooperative cancellation was observed at a chunk boundary (D-11) — the tier
    /// stopped with already-emitted tiers intact; not an error condition per se.
    #[error("cancelled at a chunk boundary")]
    Cancelled,
    /// An OPFS chunk-file I/O failure surfaced by a range sink's `finish`.
    #[error("chunk sink I/O: {0}")]
    Sink(#[from] opfs_sink::OpfsError),
    /// A wrapped engine `PropagationError` from `envi_engine::solver::solve`.
    #[error("engine solve failed: {0}")]
    Solve(String),
}

/// Map a [`ComputeError`] to a `JsValue` error (mirrors `gis_err`).
fn compute_err(e: &ComputeError) -> JsValue {
    js_err(&e.to_string())
}

// --- Boundary functions (each delegates to exactly one core path) ---------

/// Pre-run cost estimate + guardrail for a grid solve (SC1). Delegates to
/// `envi_compute::cost::{estimate, guardrail}` — no cost math inline here.
///
/// # Errors
/// A shape error in the request DTO.
#[wasm_bindgen]
pub fn estimate_cost(req: JsValue) -> Result<JsValue, JsValue> {
    let req: EstimateCostReq = from_js(req)?;
    let est = envi_compute::cost::estimate(
        req.area_m2,
        req.spacing_fine_m,
        req.discrete_points as usize,
        req.n_sub as usize,
        req.n_workers as usize,
    );
    let guard = envi_compute::cost::guardrail(&est, req.budget_bytes as usize);
    to_js(&CostEstimateResult {
        receiver_count: est.receiver_count as u32,
        tensor_bytes: est.tensor_bytes as f64,
        working_set_bytes: est.working_set_bytes as f64,
        time_estimate_ms: est.time_estimate_ms,
        guardrail_level: match guard.level {
            envi_compute::cost::GuardrailLevel::Ok => GuardrailLevelDto::Ok,
            envi_compute::cost::GuardrailLevel::Warn => GuardrailLevelDto::Warn,
            envi_compute::cost::GuardrailLevel::Block => GuardrailLevelDto::Block,
        },
        guardrail_detail: guard.detail,
    })
}

/// Partition a calc area into the hierarchical `[points, coarse…, fine]` tier plan
/// (D-05/D-06). Delegates to `envi_compute::tiers::partition` — no lattice math
/// inline here.
///
/// # Errors
/// A shape error in the request DTO.
#[wasm_bindgen]
pub fn plan_tiers(req: JsValue) -> Result<JsValue, JsValue> {
    let req: PlanTiersReq = from_js(req)?;
    let multiples: Vec<usize> = req.coarse_multiples.iter().map(|&k| k as usize).collect();
    let plan = envi_compute::tiers::partition(
        req.fine_spacing_m,
        req.lattice_origin,
        envi_compute::tiers::Rect {
            min: req.area_min,
            max: req.area_max,
        },
        &req.discrete_points,
        &multiples,
    );
    let tiers = plan
        .tiers
        .iter()
        .map(|t| TierDto {
            kind: tier_kind_dto(t.kind),
            spacing_m: t.spacing_m,
            receivers: t
                .receivers
                .iter()
                .map(|r| TierReceiverDto {
                    global_index: r.global_index as u32,
                    position: r.position,
                })
                .collect(),
        })
        .collect();
    to_js(&TierPlanResult { tiers })
}

/// Solve one disjoint receiver-chunk range on the rayon pool (D-08/GRID-02).
///
/// The parallel sharding driver itself now lives in [`pool::solve_tier`] — it runs
/// the UNCHANGED `envi_engine::solver::solve` per disjoint range into that range's
/// own OPFS chunk file, checks the cooperative cancel flag ([`request_cancel`]) at
/// each chunk boundary (D-11), and holds only `workers × chunk` resident (SC3), all
/// proven natively by `cargo test -p envi-compute-wasm pool`.
///
/// The remaining seam is per-range JOB ASSEMBLY from a marshalled scene: the
/// current [`SolveChunkRangeReq`] carries only the tensor-identity + range span,
/// not the terrain/atmosphere/source geometry `pool::solve_tier`'s `assemble`
/// closure needs. That scene-context DTO (and the `open_sink` OPFS wiring via
/// [`opfs_sink::OpfsChunkSink::open_opfs`]) is the next integration step; until it
/// lands this boundary validates the range and returns a typed [`ComputeError`].
///
/// # Errors
/// A shape error in the request DTO, or [`ComputeError::Pending`] until the
/// scene-context marshalling lands.
#[wasm_bindgen]
pub fn solve_chunk_range(req: JsValue) -> Result<JsValue, JsValue> {
    let _req: SolveChunkRangeReq = from_js(req)?;
    Err(compute_err(&ComputeError::Pending(
        "solve_chunk_range: the pool driver + OPFS sink are wired (pool::solve_tier); \
         per-range scene-context marshalling (SolveCtx DTO) is the next step",
    )))
}

/// A [`TierKindDto`] from the core [`envi_compute::tiers::TierKind`].
fn tier_kind_dto(k: envi_compute::tiers::TierKind) -> TierKindDto {
    match k {
        envi_compute::tiers::TierKind::Points => TierKindDto::Points,
        envi_compute::tiers::TierKind::Coarse => TierKindDto::Coarse,
        envi_compute::tiers::TierKind::Fine => TierKindDto::Fine,
    }
}
