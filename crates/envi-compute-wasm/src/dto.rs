//! Serde + ts-rs boundary DTOs for the WASM compute API (SVC-02 / GRID-02).
//!
//! # Module I/O
//! - **Inputs:** the request DTOs (`*Req`) deserialized from a JS object by
//!   [`serde_wasm_bindgen`] in [`crate`]'s `#[wasm_bindgen]` functions. Every
//!   request-facing type is `#[serde(deny_unknown_fields)]` so a typo'd key is a
//!   loud error at the boundary, never a silently-ignored field.
//! - **Output:** the result DTOs (`*Result`) serialized back to a JS value, plus
//!   the [`TierComplete`] event payload the Web Worker posts to the UI (D-07).
//! - **Invariant (load-bearing, Phase-7 D-10):** every type here derives
//!   [`ts_rs::TS`] with `#[ts(export_to = "wire.ts")]` and is registered in the
//!   `wire_no_drift` no-drift test — the TypeScript mirror is **generated from
//!   this Rust source and committed**, never hand-authored. A renamed/added field
//!   fails the Rust test, not the browser.
//! - **Reused wire shape (D-10):** the `JobStatus` union
//!   (`queued`/`running`/`done`/`failed`/`cancelled`) the compute worker posts is
//!   the SAME `envi_service::jobs::JobStatus` already generated into `wire.ts`
//!   (Phase 6/7). It is reused client-side verbatim — this crate defines NO second
//!   `JobStatus` (a duplicate `export type JobStatus` would break `tsc`), so the
//!   Rust side never restates that shape.
//! - **No ids, no getrandom (Pitfall 9):** receiver `id`s are minted in TS via
//!   `crypto.randomUUID()`; the tier layer emits integer global indices only.
//!   `TierComplete::receiver_ids` are the TS-assigned UUID strings, passed through.
//! - **Band-index rule:** tensor spans are `[s][r_local][f=0..105]` by index; a
//!   receiver's identity is its UUID/position, never a frequency (Pitfall 6).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// --- Cost estimate + guardrail (SC1) --------------------------------------

/// `estimate_cost` request: the pure grid spec the cost model keys off. The
/// estimate keys off the FINAL (fine) spacing (D-06) — coarse tiers add no
/// receivers (they are a subset of fine, D-05). Request-facing.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct EstimateCostReq {
    /// Calc-area footprint, m² (building footprints already subtracted).
    pub area_m2: f64,
    /// The user's final (fine) lattice spacing, meters (D-06).
    pub spacing_fine_m: f64,
    /// Count of explicit discrete receiver points (not double-counted by the grid).
    pub discrete_points: u32,
    /// Sub-source count (≥ 1 enforced by the core).
    pub n_sub: u32,
    /// Worker-pool size (`navigator.hardwareConcurrency`; ≥ 1 enforced).
    pub n_workers: u32,
    /// Hard byte budget (e.g. the OPFS quota from `navigator.storage.estimate()`).
    /// A tensor over this is a `Block` verdict.
    pub budget_bytes: f64,
}

/// The guardrail severity (mirror of `envi_compute::cost::GuardrailLevel`).
/// Serialized `snake_case` so the wire tags are `ok`/`warn`/`block`. Result-facing.
#[derive(Debug, Clone, Copy, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "wire.ts")]
pub enum GuardrailLevelDto {
    /// Within all soft thresholds and under the hard budget.
    Ok,
    /// Over a soft threshold (large/long/very-fine) but under the hard budget.
    Warn,
    /// Over the hard budget — the run must not proceed as specified.
    Block,
}

/// `estimate_cost` result: the pure pre-run estimate + guardrail verdict (SC1).
/// Byte counts are `f64` (JS-number-safe past `u32`; exact for integers ≤ 2^53).
/// Result-facing.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct CostEstimateResult {
    /// Total receivers `N = discrete_points + floor(area / spacing_fine²)`.
    pub receiver_count: u32,
    /// Full OPFS on-disk tensor footprint, bytes
    /// (`n_sub · N · 105 · BYTES_PER_CELL_PAIR`).
    pub tensor_bytes: f64,
    /// Resident RAM working set, bytes (`n_workers · chunk · n_sub · 105 · 24`, SC3).
    pub working_set_bytes: f64,
    /// Wall-clock time estimate, milliseconds (`n_sub · N · t_pair / n_workers`).
    pub time_estimate_ms: f64,
    /// The guardrail severity level.
    pub guardrail_level: GuardrailLevelDto,
    /// Human-readable guardrail detail (surfaced as text; always states the exact
    /// "halving the final spacing quadruples the cost" relation).
    pub guardrail_detail: String,
}

// --- Hierarchical tier partition (D-05/D-06) ------------------------------

/// `plan_tiers` request: the grid spec the hierarchical partition consumes. The
/// calc area is an axis-aligned rectangle in SceneXY meters. Request-facing.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct PlanTiersReq {
    /// The user's final (fine) lattice spacing, meters (D-06).
    pub fine_spacing_m: f64,
    /// Shared lattice origin `[x, y]` (anchors coarse ⊂ fine, D-05).
    pub lattice_origin: [f64; 2],
    /// Calc-area minimum corner `[x, y]`, meters.
    pub area_min: [f64; 2],
    /// Calc-area maximum corner `[x, y]`, meters.
    pub area_max: [f64; 2],
    /// Explicit discrete receiver positions `[x, y]` (the points tier).
    #[serde(default)]
    pub discrete_points: Vec<[f64; 2]>,
    /// Integer coarse factors `k` (e.g. `[10]` → one 100 m preview; `[10, 5]` →
    /// 100 m + 50 m). Factors `< 2` are ignored; the list is de-duplicated.
    #[serde(default)]
    pub coarse_multiples: Vec<u32>,
}

/// Which resolution tier a receiver belongs to (mirror of
/// `envi_compute::tiers::TierKind`). Serialized `snake_case`. Result-facing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "wire.ts")]
pub enum TierKindDto {
    /// Explicit discrete receiver points (no spacing).
    Points,
    /// A coarse preview lattice (`k · fine`).
    Coarse,
    /// The final fine lattice (gap points only — coarse excluded).
    Fine,
}

/// One receiver in a tier: its global index (receiver-major) and SceneXY `[x, y]`.
/// Result-facing.
#[derive(Debug, Clone, Copy, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct TierReceiverDto {
    /// Global receiver index (unique, assigned in emission order).
    pub global_index: u32,
    /// SceneXY position `[x, y]`, meters.
    pub position: [f64; 2],
}

/// One emitted tier: its kind, spacing (`null` for discrete points), and the
/// receivers it introduces (NOT already carried by a coarser tier). Result-facing.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct TierDto {
    /// The tier kind.
    pub kind: TierKindDto,
    /// Lattice spacing in meters (`null` for the discrete-points tier).
    pub spacing_m: Option<f64>,
    /// The receivers this tier introduces, row-major, with sequential indices.
    pub receivers: Vec<TierReceiverDto>,
}

/// `plan_tiers` result: the ordered tiers `[points, coarse…, fine]` (D-05).
/// Result-facing.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct TierPlanResult {
    /// The ordered tiers in solve order.
    pub tiers: Vec<TierDto>,
}

// --- Tier-complete event payload (D-07) -----------------------------------

/// Where one tier's data lives in OPFS — a receiver-axis span in a chunk file
/// pair. Byte layout is the frozen `[s][r_local][f]` interleaved-LE format the
/// OPFS sink writes. Both request- and result-facing (Phase-11 reads it back).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct ChunkSpanDto {
    /// The OPFS chunk index (unique per disjoint receiver range).
    pub chunk_index: u32,
    /// Receiver-axis offset of this span within the tensor.
    pub r_offset: u32,
    /// Receiver-axis length of this span.
    pub len: u32,
    /// The H_coh chunk file, e.g. `"tensor/chunk_00042.bin"`.
    pub tensor_file: String,
    /// The P_incoh chunk file, e.g. `"pincoh/chunk_00042.bin"`.
    pub pincoh_file: String,
}

/// The `tier_complete` event the compute worker posts once a tier's chunk files
/// are flushed (D-07) — it carries everything Phase 11 needs to read the chunks
/// and render points → coarse map → refined map. **Phase 10 emits; Phase 11
/// renders.** The `tensor_hash` ties every span to the manifest identity (D-09).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct TierComplete {
    /// The fixed discriminant `"tier_complete"` (a postMessage-envelope tag).
    #[ts(type = "\"tier_complete\"")]
    pub kind: String,
    /// Which tier just finished.
    pub tier: TierKindDto,
    /// The tier's index in solve order (`0` = points, `1…` = coarse, last = fine).
    pub tier_index: u32,
    /// The tier's lattice spacing, meters (`null` for the discrete-points tier).
    pub spacing_m: Option<f64>,
    /// The frozen tensor-identity content hash (hex; D-09).
    pub tensor_hash: String,
    /// Stable receiver UUIDs in this tier, in receiver-major order (TS-minted).
    pub receiver_ids: Vec<String>,
    /// Where this tier's data lives in OPFS (receiver-axis spans).
    pub spans: Vec<ChunkSpanDto>,
}

impl TierComplete {
    /// The fixed `kind` discriminant value.
    pub const KIND: &'static str = "tier_complete";
}

// --- solve_chunk_range request (signature seam; wired in 10-04) ------------

/// `solve_chunk_range` request: one disjoint receiver-chunk range to solve on the
/// pool (D-08 caller-side rayon sharding). The full field set + the rayon pool
/// driver land in plan 10-04; this crate declares the boundary signature so the
/// wire shape is fixed. Request-facing.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct SolveChunkRangeReq {
    /// The frozen tensor-identity hash the chunk files are keyed under (D-09).
    pub tensor_hash: String,
    /// This range's OPFS chunk index (disjoint across ranges → disjoint files).
    pub chunk_index: u32,
    /// Receiver-axis offset of the first receiver in this range.
    pub r_offset: u32,
    /// Receiver-axis length of this range.
    pub len: u32,
}
