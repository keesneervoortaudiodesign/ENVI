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
// The marshalled-scene tensor-identity hash (HI-01 / D-09) — the blake3 digest the
// OPFS/manifest key is derived from, over every tensor-affecting field of a
// `PrepareSolveReq`. Exposed to the client via the `tensor_hash` boundary export so
// the browser never invents its own key.
pub mod identity;
// The OPFS tensor READER (11-01) — the exact inverse of `opfs_sink::put_chunk`,
// decoding the frozen `[s][r_local][f]` chunk bytes back into `Array3`s for the
// client-side readout. Available on every build (pure decode; no OPFS/JS glue —
// the worker-side pre-open-read handles land in 11-05).
pub mod opfs_reader;
pub mod opfs_sink;
// The hash-gated recondition MAC boundary (SVC-06 / D-01 / D-12, 11-03) — decodes
// the OPFS tensor, re-mints the tensor identity, refuses a mismatched claimed hash
// with a typed `HashMismatch` (client-side 409), and drives compose_gain +
// readout_coherent over the reused tensor with no re-propagation. Available on
// every build (pure decode + engine readout; the worker OPFS glue lands in 11-05).
pub mod recondition;
// The caller-side rayon sharding driver (GRID-02). Compiled for every NATIVE build
// (so `cargo test pool` exercises it) and for the THREADED wasm build; excluded
// only from the stable, single-threaded wasm build where rayon is absent.
#[cfg(any(not(target_arch = "wasm32"), feature = "threads"))]
pub mod pool;
// The owned prepared scene + the marshalled range-solve (10-06) — closes the
// solve_chunk_range seam. Available on every build; its rayon-sharded core is
// cfg-split against a sequential fallback for the stable single-threaded wasm32.
pub mod scene;

use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use serde::de::DeserializeOwned;
use thiserror::Error;
use wasm_bindgen::prelude::*;

use dto::{
    CostEstimateResult, EstimateCostReq, GuardrailLevelDto, PlanTiersReq, PrepareSolveReq,
    RangeProgressDto, SolveChunkRangeReq, TierDto, TierKindDto, TierPlanResult, TierReceiverDto,
};
use scene::PreparedScene;

/// The process-wide prepared scene (T-10-06-04). `wasm-bindgen-rayon`'s linear
/// memory is a `SharedArrayBuffer`, so a single `static` in linear memory IS
/// shared across every pool thread (a `thread_local` would NOT be — pool threads
/// have separate thread-locals). Write-locked ONCE per submit by [`prepare_solve`]
/// (no concurrent solve), read-locked by [`solve_chunk_range`].
static PREPARED: RwLock<Option<PreparedScene>> = RwLock::new(None);

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
pub(crate) fn from_js<T: DeserializeOwned>(v: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(v).map_err(|e| js_err(&e.to_string()))
}

/// Serialize a result DTO back to a JS value (plain objects, not JS `Map`s — the
/// TS import path reads result DTOs as plain objects).
pub(crate) fn to_js<T: Serialize>(v: &T) -> Result<JsValue, JsValue> {
    let serializer = serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true);
    v.serialize(&serializer).map_err(|e| js_err(&e.to_string()))
}

/// Build a `JsValue` error carrying `msg` (an `Error` on the JS side).
pub(crate) fn js_err(msg: &str) -> JsValue {
    JsError::new(msg).into()
}

/// A typed boundary error (mirrors `envi-gis-wasm`'s `gis_err` discipline — the
/// boundary never panics on data). The rayon pool driver ([`pool`]) surfaces
/// `Cancelled`, `Sink`, and `Solve` through this type.
#[derive(Debug, Error)]
pub enum ComputeError {
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
    /// A validating engine constructor rejected the marshalled scene at
    /// `prepare_solve` (T-10-06-03) — degenerate geometry / non-finite / bad grid.
    #[error("prepare_solve rejected the scene: {0}")]
    Prepare(String),
    /// `solve_chunk_range` was called before any scene was prepared (T-10-06-04).
    #[error("no prepared scene — call prepare_solve first")]
    NotPrepared,
    /// The request's `tensor_hash` does not match the current/prepared tensor
    /// identity — never solve or read out a stale/mismatched tensor (T-10-06-04;
    /// the client-side SVC-06 409, 11-03). The `{expected, got}` fields mirror the
    /// server 409 body (`envi-service::api::calc`'s `tensor_hash_mismatch`
    /// `{expected, got, hint}`) so the client realization of the honest state is
    /// faithful (Open Q1 / D-12).
    #[error("tensor_hash mismatch: expected {expected}, got {got}")]
    HashMismatch {
        /// The freshly re-minted (current) tensor identity.
        expected: String,
        /// The identity the request claimed to be operating on.
        got: String,
    },
    /// A recondition MAC request failed validation or readout (dense `[105]` filter
    /// length / finiteness V5, a sub-source/receiver count mismatch, or an engine
    /// readout `SinkError`) — a typed error, never a panic on data (T-11-03-02).
    #[error("recondition failed: {0}")]
    Recondition(String),
    /// The requested receiver range `[r_offset, r_offset + len)` is not densely
    /// covered by the prepared scene's receivers (`local_receivers` selected fewer
    /// than `len`) — a malformed range that would otherwise slice out of bounds and
    /// trap the wasm module. Surfaced as a typed error, never a panic (WR-01 /
    /// T-10-03-02 / T-10-06-03/04: the boundary never panics on data).
    #[error(
        "receiver range [{r_offset}, {r_offset}+{len}) covers only {covered} of {len} prepared receivers"
    )]
    Range {
        /// The range's global receiver offset.
        r_offset: usize,
        /// The requested range length.
        len: usize,
        /// How many prepared receivers actually fall in the range.
        covered: usize,
    },
}

/// Map a [`ComputeError`] to a `JsValue` error (mirrors `gis_err`).
pub(crate) fn compute_err(e: &ComputeError) -> JsValue {
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
    let guard = envi_compute::cost::guardrail(&est, req.budget_bytes as u64);
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

/// Compute the marshalled-scene tensor-identity hash (HI-01 / D-09) for a
/// [`PrepareSolveReq`] — the blake3 digest over EVERY tensor-affecting field
/// (terrain, atmosphere, coherence, weather, sub-sources + directivity, receivers,
/// forest, isolation, `n_sub`; the request's own `tensor_hash` field is excluded).
/// The client uses the returned 64-char lowercase hex as the OPFS/manifest key
/// (`calc/<hash>/…`) and as the `tensor_hash` it threads into [`prepare_solve`] /
/// [`solve_chunk_range`], so the store is keyed by the TRUE tensor identity rather
/// than an ad-hoc client-side hash. A cheap standalone export (no scene build), so
/// the key is available before `prepare_solve` for the cost/estimate path.
///
/// # Errors
/// A shape error in the request DTO.
#[wasm_bindgen]
pub fn tensor_hash(req: JsValue) -> Result<String, JsValue> {
    let req: PrepareSolveReq = from_js(req)?;
    Ok(identity::marshalled_tensor_hash(&req))
}

/// Marshal the ENTIRE transfer scene ONCE per submit (D-08). Deserializes a
/// [`PrepareSolveReq`], builds an owned [`PreparedScene`] through the engine's
/// validating constructors (a rejection is a typed [`ComputeError::Prepare`],
/// never a panic — T-10-06-03), and stores it in the shared [`PREPARED`] registry
/// keyed by its `tensor_hash`. Called once before the chunk loop; every subsequent
/// [`solve_chunk_range`] carrying the same hash solves against it.
///
/// # Errors
/// A shape error in the request DTO, or a [`ComputeError::Prepare`] surfaced by a
/// validating engine constructor.
#[wasm_bindgen]
pub fn prepare_solve(req: JsValue) -> Result<(), JsValue> {
    let req: PrepareSolveReq = from_js(req)?;
    let prepared = PreparedScene::build(&req).map_err(|e| compute_err(&e))?;
    let mut guard = PREPARED
        .write()
        .map_err(|_| js_err("prepared-scene lock poisoned"))?;
    *guard = Some(prepared);
    Ok(())
}

/// Solve one disjoint receiver-chunk range against the prepared scene, writing its
/// OPFS chunk file pair (D-08/GRID-02) and returning a [`RangeProgressDto`].
///
/// It deserializes the [`SolveChunkRangeReq`], read-locks the [`PREPARED`] scene,
/// verifies the range's `tensor_hash` matches (never solves a stale/mismatched
/// scene — [`ComputeError::HashMismatch`]), then runs
/// [`scene::solve_prepared_range`]: the UNCHANGED `envi_engine::solver::solve`
/// sharded across the rayon pool ([`pool::solve_tier`], its cancel-check + SC3
/// residency) and assembled into one sub-source-major `[s][r_local][f]` chunk. The
/// cooperative cancel flag ([`request_cancel`]) short-circuits before the file
/// opens (D-11). The assembled chunk is written to its `tensor/` + `pincoh/` OPFS
/// files (pre-opened by the worker; see `web/src/compute/opfs.ts`).
///
/// # Errors
/// A shape error in the request DTO; [`ComputeError::NotPrepared`] if no scene was
/// prepared; [`ComputeError::HashMismatch`] on a stale hash;
/// [`ComputeError::Cancelled`] on a cooperative abort; a [`ComputeError::Sink`]
/// OPFS I/O failure; or a wrapped engine [`ComputeError::Solve`].
#[wasm_bindgen]
pub fn solve_chunk_range(req: JsValue) -> Result<JsValue, JsValue> {
    let req: SolveChunkRangeReq = from_js(req)?;
    let progress = run_chunk_range(&req).map_err(|e| compute_err(&e))?;
    to_js(&progress)
}

/// The typed body of [`solve_chunk_range`] (no `JsValue` — natively testable).
fn run_chunk_range(req: &SolveChunkRangeReq) -> Result<RangeProgressDto, ComputeError> {
    // Cooperative abort at the chunk boundary (D-11): stop BEFORE opening the file.
    if is_cancel_requested() {
        return Err(ComputeError::Cancelled);
    }
    let guard = PREPARED.read().map_err(|_| ComputeError::NotPrepared)?;
    let scene = guard.as_ref().ok_or(ComputeError::NotPrepared)?;
    // Never solve a stale/mismatched scene (T-10-06-04).
    if scene.tensor_hash() != req.tensor_hash {
        return Err(ComputeError::HashMismatch {
            expected: scene.tensor_hash().to_string(),
            got: req.tensor_hash.clone(),
        });
    }

    let r_offset = req.r_offset as usize;
    let len = req.len as usize;
    // WR-06: report the receivers actually covered by this range, MEASURED from the
    // prepared scene rather than trusting the requested `req.len`. With WR-01's
    // coverage gate a partial range is now a `Range` error before we get here, so
    // `solved == len` on the success path — but measuring (not assuming) keeps the
    // progress/`receivers` count honest as defence-in-depth.
    let solved = scene.local_receiver_count(r_offset, len);
    let (h, p) = scene::solve_prepared_range(scene, r_offset, len)?;
    write_chunk_to_opfs(req, scene.n_sub(), &h, &p)?;

    Ok(RangeProgressDto {
        chunk_index: req.chunk_index,
        receivers: solved as u32,
    })
}

/// Write the assembled chunk to its OPFS `tensor/` + `pincoh/` file pair. The
/// handles were pre-opened by the worker (async `createSyncAccessHandle`) and
/// keyed by the relative chunk path (`web/src/compute/opfs.ts`); the synchronous
/// `openChunk` extern returns the pre-opened handle (D-08/D-09).
#[cfg(target_arch = "wasm32")]
fn write_chunk_to_opfs(
    req: &SolveChunkRangeReq,
    n_sub: usize,
    h: &ndarray::Array3<num_complex::Complex<f64>>,
    p: &ndarray::Array3<f64>,
) -> Result<(), ComputeError> {
    use envi_engine::tensor::TensorSink;

    let tensor_path = chunk_relative_path("tensor", req.chunk_index);
    let pincoh_path = chunk_relative_path("pincoh", req.chunk_index);
    let mut sink =
        opfs_sink::OpfsChunkSink::open_opfs(&tensor_path, &pincoh_path, n_sub, req.len as usize)
            .map_err(ComputeError::Sink)?;
    sink.put_chunk(0, h.view(), p.view())
        .map_err(|e| ComputeError::Solve(e.to_string()))?;
    sink.finish().map_err(ComputeError::Sink)?;
    Ok(())
}

/// Native no-op: OPFS is worker-only (wasm). The assembled chunk tensor is
/// verified directly via [`scene::solve_prepared_range`] in `cargo test`.
#[cfg(not(target_arch = "wasm32"))]
fn write_chunk_to_opfs(
    _req: &SolveChunkRangeReq,
    _n_sub: usize,
    _h: &ndarray::Array3<num_complex::Complex<f64>>,
    _p: &ndarray::Array3<f64>,
) -> Result<(), ComputeError> {
    Ok(())
}

/// The relative chunk path within a tensor's calc directory, e.g.
/// `tensor/chunk_00042.bin` — matching `chunkRelativePath`/`chunkFileName` in
/// `web/src/compute/opfs.ts` (zero-padded to 5 digits). The worker pre-opens the
/// handle under exactly this key.
#[cfg(target_arch = "wasm32")]
fn chunk_relative_path(channel: &str, chunk_index: u32) -> String {
    format!("{channel}/chunk_{chunk_index:05}.bin")
}

/// A [`TierKindDto`] from the core [`envi_compute::tiers::TierKind`].
fn tier_kind_dto(k: envi_compute::tiers::TierKind) -> TierKindDto {
    match k {
        envi_compute::tiers::TierKind::Points => TierKindDto::Points,
        envi_compute::tiers::TierKind::Coarse => TierKindDto::Coarse,
        envi_compute::tiers::TierKind::Fine => TierKindDto::Fine,
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use super::*;
    use dto::{AtmosphereDto, CoherenceInputsDto, ReceiverPlacementDto, SubSourcePlacementDto};
    use envi_compute::scene_dto::{GroundSegmentDto, TerrainProfileDto};

    fn omni_req(tensor_hash: &str) -> PrepareSolveReq {
        PrepareSolveReq {
            tensor_hash: tensor_hash.to_string(),
            n_sub: 1,
            terrain: TerrainProfileDto {
                points: vec![[2.5, 0.0], [400.0, 0.0]],
                segments: vec![GroundSegmentDto {
                    flow_resistivity: 200.0,
                    roughness: 0.0,
                }],
            },
            atmosphere: AtmosphereDto {
                temperature_c: 15.0,
                humidity_pct: 70.0,
                pressure_kpa: 101.325,
            },
            coherence: CoherenceInputsDto {
                cv2: 0.0,
                ct2: 0.0,
                t_air_c: 15.0,
                c0: 340.348,
                roughness_r: 0.0,
                f_delta_nu: 1.0,
                d_m: 97.5,
            },
            weather: None,
            sub_sources: vec![SubSourcePlacementDto {
                position: [2.5, 0.0, 0.5],
                directivity: None,
            }],
            receivers: vec![
                ReceiverPlacementDto {
                    global_index: 0,
                    position: [100.0, 0.0, 1.5],
                },
                ReceiverPlacementDto {
                    global_index: 1,
                    position: [101.0, 0.0, 1.5],
                },
            ],
            forest: None,
            forest_path_length_m: None,
            isolation: None,
        }
    }

    /// The prepared-scene registry gate: a matching hash solves (a real, non-Pending
    /// `RangeProgress`), a mismatched hash is a typed error (never a solve), and an
    /// empty registry is `NotPrepared`. Serialized on `PREPARED` (single test).
    #[test]
    fn solve_chunk_range_registry_hash_gate() {
        reset_cancel();
        // Empty registry → NotPrepared.
        {
            let mut g = PREPARED.write().unwrap();
            *g = None;
        }
        let req_ok = SolveChunkRangeReq {
            tensor_hash: "hashA".to_string(),
            chunk_index: 0,
            r_offset: 0,
            len: 2,
        };
        assert!(matches!(
            run_chunk_range(&req_ok),
            Err(ComputeError::NotPrepared)
        ));

        // Prepare a scene keyed by "hashA".
        {
            let scene = PreparedScene::build(&omni_req("hashA")).unwrap();
            let mut g = PREPARED.write().unwrap();
            *g = Some(scene);
        }

        // Matching hash → a real RangeProgress (no Pending).
        let progress = run_chunk_range(&req_ok).expect("matching hash solves");
        assert_eq!(progress.chunk_index, 0);
        assert_eq!(progress.receivers, 2);

        // Mismatched hash → HashMismatch, never a solve.
        let req_bad = SolveChunkRangeReq {
            tensor_hash: "hashB".to_string(),
            chunk_index: 1,
            r_offset: 0,
            len: 2,
        };
        assert!(matches!(
            run_chunk_range(&req_bad),
            Err(ComputeError::HashMismatch { .. })
        ));

        // Clean up so no other test observes a stale scene.
        let mut g = PREPARED.write().unwrap();
        *g = None;
    }
}
