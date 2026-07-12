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

// The WASM-safe scene DTOs the solve marshals live in `envi_compute::scene_dto`
// (factored from envi-store/envi-gis-wasm in 10-06) — reused here, never
// hand-mirrored, so there is exactly one wire type per shape.
use envi_compute::scene_dto::{
    ForestParamsDto, IsolationSpectrumDto, SoundSpeedProfileDto, TerrainProfileDto,
};
// The per-source conditioning readout DTO (WEB-05) — reused verbatim from the
// pure core, never forked. It is structurally excluded from tensor identity
// (Phase-6 D-07), so a conditioning edit never stales the reused tensor.
use envi_compute::readout::ConditioningDto;

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

// --- prepare_solve request (the scene marshalled ONCE per submit, 10-06) ----

/// `prepare_solve` request: the ENTIRE transfer scene marshalled ONCE per submit
/// (D-08). The boundary builds an owned `PreparedScene` keyed by `tensor_hash`;
/// every subsequent [`SolveChunkRangeReq`] carrying the same hash solves a
/// receiver range against it. Request-facing (`deny_unknown_fields`).
///
/// The solve marshals the TRANSFER scene only — source sound-power spectra
/// ([`BandSpectrumDto`](envi_store::dto::BandSpectrumDto)) are readout inputs
/// (Phase 11) and are NOT carried here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct PrepareSolveReq {
    /// The frozen tensor-identity hash this scene is keyed under (must match the
    /// later `solve_chunk_range` requests, D-09).
    pub tensor_hash: String,
    /// Sub-source count (tensor rows; `≥ 1`).
    pub n_sub: u32,
    /// The `src→rcv` cut-plane terrain profile (points + ground segments).
    pub terrain: TerrainProfileDto,
    /// Atmospheric state (built via the validating `Atmosphere::new`).
    pub atmosphere: AtmosphereDto,
    /// Coherence inputs (turbulence + sound speed).
    pub coherence: CoherenceInputsDto,
    /// Optional refraction profile (`None` = homogeneous atmosphere).
    #[serde(default)]
    pub weather: Option<SoundSpeedProfileDto>,
    /// The sub-source placements (tensor rows).
    pub sub_sources: Vec<SubSourcePlacementDto>,
    /// The receiver placements (tensor columns), carrying global indices.
    pub receivers: Vec<ReceiverPlacementDto>,
    /// Optional forest crossing on this corridor (ENG-09) — the authored subset.
    #[serde(default)]
    pub forest: Option<ForestParamsDto>,
    /// The through-forest crossing length `d_m` (Phase-9 geometry) supplied here
    /// so the solve sees a real crossing length rather than the `ForestParamsDto`
    /// `0.0` placeholder. Ignored when `forest` is `None`.
    #[serde(default)]
    pub forest_path_length_m: Option<f64>,
    /// Optional semi-transparent partition on this corridor (ENG-10).
    #[serde(default)]
    pub isolation: Option<IsolationSpectrumDto>,
}

/// Atmospheric state for the free-field direct path (built via the validating
/// `Atmosphere::new`, which rejects non-finite / out-of-range values).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct AtmosphereDto {
    /// Air temperature, °C.
    pub temperature_c: f64,
    /// Relative humidity, %.
    pub humidity_pct: f64,
    /// Atmospheric pressure, kPa.
    pub pressure_kpa: f64,
}

/// Coherence inputs (mirror of `envi_engine::propagation::coherence::CoherenceInputs`);
/// every value is finiteness-checked when the `PreparedScene` is built.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct CoherenceInputsDto {
    /// Wind-velocity structure parameter `Cv²` (m^{4/3}·s⁻²). `0` = no turbulence.
    pub cv2: f64,
    /// Temperature structure parameter `CT²` (K²·m^{−2/3}). `0` = no turbulence.
    pub ct2: f64,
    /// Air temperature, °C.
    pub t_air_c: f64,
    /// Speed of sound, m/s (also the terrain sound speed).
    pub c0: f64,
    /// Terrain roughness `r`, meters.
    pub roughness_r: f64,
    /// Relative bandwidth `Δν/ν` of the analysis filter.
    pub f_delta_nu: f64,
    /// Reference path length `d_m`, meters.
    pub d_m: f64,
}

/// A receiver placement: its global (receiver-major) index and position.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct ReceiverPlacementDto {
    /// Global receiver index (the tensor / sink receiver-axis coordinate).
    pub global_index: u32,
    /// Receiver position `[x, y, z]`, meters.
    pub position: [f64; 3],
}

/// A sub-source placement: its position and optional directivity (balloon +
/// source orientation). `None` directivity is an omnidirectional sub-source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct SubSourcePlacementDto {
    /// Sub-source position `[x, y, z]`, meters.
    pub position: [f64; 3],
    /// Optional directional pattern for this sub-source.
    #[serde(default)]
    pub directivity: Option<DirectionalDto>,
}

/// A directional pattern: a per-band balloon plus the source→local-frame rotation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct DirectionalDto {
    /// The per-band spherical directivity balloon (may carry phase).
    pub balloon: DirectivityBalloonDto,
    /// Rotation from the world `src→rcv` direction into the balloon's local frame.
    pub orientation: RotationDto,
}

/// A per-band spherical directivity balloon (mirror of the engine
/// `DirectivityBalloon` inputs). `grid_db` / `phase_grid_rad` are flattened
/// **row-major** `(azimuth, polar, band=105)`; the builder reshapes them into an
/// `Array3` and routes through the engine's validating `DirectivityBalloon::new`
/// / `new_with_phase` (which reject non-ascending axes, a wrong band count, and
/// non-finite values). A phase-free balloon leaves `phase_grid_rad` `None`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct DirectivityBalloonDto {
    /// Azimuth grid nodes, degrees (strictly ascending, `≥ 2`).
    pub azimuths_deg: Vec<f64>,
    /// Polar grid nodes, degrees (strictly ascending, `≥ 2`).
    pub polars_deg: Vec<f64>,
    /// Flattened row-major `(az, pol, band=105)` gain grid, dB.
    pub grid_db: Vec<f64>,
    /// Optional flattened row-major `(az, pol, band=105)` phase grid, radians
    /// (source-local, ENVI `e^{+jωt}`). `None` = magnitude-only balloon.
    #[serde(default)]
    pub phase_grid_rad: Option<Vec<f64>>,
}

/// A 3×3 rotation matrix (row-major), built via `Rotation3::from_matrix`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct RotationDto {
    /// Row-major 3×3 rotation matrix.
    pub matrix: [[f64; 3]; 3],
}

/// `solve_chunk_range` result: progress for one solved range — its OPFS chunk
/// index and the receivers written. The worker folds these into a `Running`
/// status (chunks done / total). Result-facing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "wire.ts")]
pub struct RangeProgressDto {
    /// The range's OPFS chunk index.
    pub chunk_index: u32,
    /// Receivers written for this range.
    pub receivers: u32,
}

// --- recondition MAC request/result (SVC-06 / D-01, client-side 409) ---------

/// `recondition` request (SVC-06 / D-01): the claimed tensor identity, the
/// per-source conditioning drive, and the target receiver ids. Request-facing
/// (`deny_unknown_fields`).
///
/// `tensor_hash` is the identity the client believes it is reconditioning; the
/// boundary re-mints the CURRENT tensor identity and refuses a mismatch with a
/// typed `HashMismatch` (D-12, the honest client-side 409). `per_source_conditioning`
/// reuses [`ConditioningDto`] (never a forked shape) and must carry exactly one
/// entry per tensor sub-source; `receiver_ids` are the TS-minted UUIDs of the
/// chunk's receivers in receiver-major order (index = position), aligned 1:1 with
/// [`ReconditionResult::spectra`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct ReconditionReq {
    /// The tensor identity the client believes it is reconditioning (the 409 gate).
    pub tensor_hash: String,
    /// Per-source conditioning (gain/delay/filter/mute), one entry per sub-source.
    pub per_source_conditioning: Vec<ConditioningDto>,
    /// The chunk's receiver UUIDs in receiver-major order (index = tensor column).
    pub receiver_ids: Vec<String>,
}

/// `recondition` result: per-receiver dense `[105]` band-index level spectra (dB),
/// aligned 1:1 with the request's `receiver_ids`, plus a `stale` flag. Result-facing.
///
/// `stale` is ALWAYS `false` on this path: a hash mismatch is refused outright as a
/// typed `HashMismatch` (D-12 honest state), never served as a stale-flagged
/// readout, and conditioning is excluded from tensor identity (D-07) so a
/// conditioning edit can never stale a matched tensor. The field documents that
/// invariant on the wire.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct ReconditionResult {
    /// One dense `[105]` band-index level spectrum (dB) per receiver, in
    /// `receiver_ids` order.
    pub spectra: Vec<Vec<f64>>,
    /// Always `false` — a mismatched hash is refused, never served stale (D-12/D-07).
    pub stale: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use envi_compute::interpolate::Resolution;
    use envi_compute::scene_dto::{AuthoredSpectrumDto, GroundSegmentDto};

    fn omni_scene_json() -> &'static str {
        r#"{
          "tensor_hash": "deadbeef",
          "n_sub": 1,
          "terrain": { "points": [[2.5,0.0],[200.0,0.0]], "segments": [{ "flow_resistivity": 200.0, "roughness": 0.0 }] },
          "atmosphere": { "temperature_c": 15.0, "humidity_pct": 70.0, "pressure_kpa": 101.325 },
          "coherence": { "cv2": 0.0, "ct2": 0.0, "t_air_c": 15.0, "c0": 340.348, "roughness_r": 0.0, "f_delta_nu": 1.0, "d_m": 97.5 },
          "sub_sources": [{ "position": [2.5,0.0,0.5] }],
          "receivers": [{ "global_index": 0, "position": [100.0,0.0,1.5] }]
        }"#
    }

    #[test]
    fn prepare_solve_req_round_trips_and_defaults_optionals() {
        let req: PrepareSolveReq = serde_json::from_str(omni_scene_json()).expect("parse scene");
        assert_eq!(req.tensor_hash, "deadbeef");
        assert_eq!(req.n_sub, 1);
        // Optional transfer inputs default to None.
        assert!(req.weather.is_none());
        assert!(req.forest.is_none());
        assert!(req.forest_path_length_m.is_none());
        assert!(req.isolation.is_none());
        assert_eq!(req.sub_sources.len(), 1);
        assert!(req.sub_sources[0].directivity.is_none());
        assert_eq!(req.receivers[0].global_index, 0);

        // Serialize → deserialize is lossless.
        let s = serde_json::to_string(&req).expect("serialize");
        assert_eq!(
            serde_json::from_str::<PrepareSolveReq>(&s).unwrap(),
            req,
            "PrepareSolveReq round-trips losslessly"
        );
    }

    #[test]
    fn prepare_solve_req_carries_forest_isolation_and_directivity() {
        let iso = IsolationSpectrumDto {
            authored: AuthoredSpectrumDto {
                resolution: Resolution::Octave,
                values: vec![10.0; 9],
            },
        };
        let forest = ForestParamsDto {
            density_per_m2: 0.4,
            stem_radius_m: 0.12,
            height_m: 12.0,
            absorption: Some(0.2),
        };
        let balloon = DirectivityBalloonDto {
            azimuths_deg: vec![0.0, 180.0],
            polars_deg: vec![0.0, 180.0],
            grid_db: vec![0.0; 2 * 2 * 105],
            phase_grid_rad: Some(vec![0.0; 2 * 2 * 105]),
        };
        let req = PrepareSolveReq {
            tensor_hash: "abc123".to_string(),
            n_sub: 1,
            terrain: TerrainProfileDto {
                points: vec![[2.5, 0.0], [200.0, 0.0]],
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
            weather: Some(SoundSpeedProfileDto {
                a: 0.0,
                b: 0.0,
                c: 340.348,
                s_a: 0.0,
                s_b: 0.0,
                z0: 0.03,
            }),
            sub_sources: vec![SubSourcePlacementDto {
                position: [2.5, 0.0, 0.5],
                directivity: Some(DirectionalDto {
                    balloon,
                    orientation: RotationDto {
                        matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                    },
                }),
            }],
            receivers: vec![ReceiverPlacementDto {
                global_index: 0,
                position: [100.0, 0.0, 1.5],
            }],
            forest: Some(forest),
            forest_path_length_m: Some(80.0),
            isolation: Some(iso),
        };
        let s = serde_json::to_string(&req).expect("serialize");
        assert_eq!(
            serde_json::from_str::<PrepareSolveReq>(&s).unwrap(),
            req,
            "a forest + isolation + phased-directivity scene round-trips"
        );
    }

    #[test]
    fn prepare_solve_req_rejects_unknown_fields() {
        // A typo'd top-level key is a loud boundary error, never silently ignored.
        let bad = r#"{
          "tensor_hash": "deadbeef", "n_sub": 1,
          "terrain": { "points": [[0.0,0.0],[1.0,0.0]], "segments": [{ "flow_resistivity": 200.0, "roughness": 0.0 }] },
          "atmosphere": { "temperature_c": 15.0, "humidity_pct": 70.0, "pressure_kpa": 101.325 },
          "coherence": { "cv2": 0.0, "ct2": 0.0, "t_air_c": 15.0, "c0": 340.348, "roughness_r": 0.0, "f_delta_nu": 1.0, "d_m": 97.5 },
          "sub_sources": [], "receivers": [], "typo_field": 1
        }"#;
        assert!(
            serde_json::from_str::<PrepareSolveReq>(bad).is_err(),
            "unknown top-level field must be rejected"
        );
    }

    #[test]
    fn atmosphere_and_coherence_dtos_reject_unknown_fields() {
        assert!(
            serde_json::from_str::<AtmosphereDto>(
                r#"{ "temperature_c": 15.0, "humidity_pct": 70.0, "pressure_kpa": 101.325, "x": 1 }"#
            )
            .is_err(),
            "AtmosphereDto rejects unknown fields"
        );
        assert!(
            serde_json::from_str::<CoherenceInputsDto>(
                r#"{ "cv2":0,"ct2":0,"t_air_c":15,"c0":340.348,"roughness_r":0,"f_delta_nu":1,"d_m":97.5,"x":1 }"#
            )
            .is_err(),
            "CoherenceInputsDto rejects unknown fields"
        );
    }

    #[test]
    fn directional_and_rotation_dtos_round_trip() {
        let d = DirectionalDto {
            balloon: DirectivityBalloonDto {
                azimuths_deg: vec![0.0, 90.0, 180.0],
                polars_deg: vec![0.0, 90.0],
                grid_db: vec![1.0; 3 * 2 * 105],
                phase_grid_rad: None,
            },
            orientation: RotationDto {
                matrix: [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]],
            },
        };
        let s = serde_json::to_string(&d).expect("serialize");
        assert_eq!(serde_json::from_str::<DirectionalDto>(&s).unwrap(), d);
        // phase_grid_rad defaults to None when absent.
        let no_phase = r#"{ "azimuths_deg":[0.0,180.0], "polars_deg":[0.0,180.0], "grid_db":[] }"#;
        let b: DirectivityBalloonDto = serde_json::from_str(no_phase).expect("parse");
        assert!(b.phase_grid_rad.is_none());
    }
}
