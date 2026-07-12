// wasm.ts — the typed facade over the generated THREADED `envi-compute-wasm` boundary,
// for the main-thread callers (results readout, conditioning recalc, the stale re-mint,
// the isophone tracer, the pre-run cost estimate, the export encoder, and the scene
// marshaller). Lazily initialises the compute module ONCE and re-exposes each used
// `#[wasm_bindgen]` export with the committed `../generated/wire` DTO types instead of the
// generated `any`. This is the SINGLE place the untyped compute glue is touched off the
// worker; every caller works in wire-typed values. Mirrors `../import/wasm.ts`.
//
// # Module I/O
// - Input  the committed compute glue (`../generated/wasm-compute/envi_compute_wasm`, a
//   build artifact) + wire-typed request DTOs. Tensor/pincoh chunk bytes cross as
//   `Uint8Array` (efficient typed-array marshalling), never a serde field.
// - Output wire-typed result DTOs (`TierPlanResult`, `ReadoutResult`, `CostEstimateResult`,
//   …) / the minted identity string / the traced GeoJSON string / the encoded export bytes.
//   Every wrapper awaits `ensureCompute()`, so a caller never sees an uninitialised module.
// - Valid input range: request DTOs exactly as generated in `wire.ts`.
//
// The glue is a DYNAMIC import inside the initialiser so importing this module (or any store
// that uses it) in Node — the vitest unit graph — never pulls the browser-only wasm/worker/
// OPFS graph. Only pure fns run here on the main thread; the rayon pool (`initThreadPool`)
// is the worker's job (Pitfall 2). Idempotent: wasm-bindgen caches the instance.

import type {
  CostEstimateResult,
  EstimateCostReq,
  ExportFormat,
  ExportGridDto,
  ExportReq,
  PlanTiersReq,
  PrepareSolveReq,
  ReadoutResult,
  ReconditionReq,
  TierPlanResult,
  TraceIsophonesReq,
} from "../generated/wire";

// The generated glue module's type — used only as a type (never a static import), so
// referencing it never triggers a load of the browser-only wasm graph.
type ComputeGlue = typeof import("../generated/wasm-compute/envi_compute_wasm");

let gluePromise: Promise<ComputeGlue> | null = null;

// Initialise the threaded compute wasm module exactly once (idempotent across concurrent
// callers). `g.default()` instantiates the module (defining the pure exports) without
// spawning the rayon pool. The DYNAMIC import keeps the wasm graph out of the Node load.
export function ensureCompute(): Promise<ComputeGlue> {
  gluePromise ??= (async () => {
    const g = await import("../generated/wasm-compute/envi_compute_wasm");
    await g.default();
    return g;
  })();
  return gluePromise;
}

// --- Typed export wrappers (the SINGLE audited cast site) ------------------
// The generated glue types every `req`/return as `any`; the wire DTOs are the real
// shapes. These wrappers localise the casts so no caller casts.

/** Plan the tiered receiver lattice for a grid spec (pure — no rayon pool). */
export async function planTiers(req: PlanTiersReq): Promise<TierPlanResult> {
  const g = await ensureCompute();
  return g.plan_tiers(req) as TierPlanResult;
}

/** Mint the blake3 tensor identity of a marshalled scene (the hasher excludes the field). */
export async function tensorHash(scene: PrepareSolveReq): Promise<string> {
  const g = await ensureCompute();
  return g.tensor_hash(scene) as string;
}

/** Readout/recondition one covering chunk's receivers over the cached OPFS tensor bytes. */
export async function readoutReceivers(
  scene: PrepareSolveReq,
  req: ReconditionReq,
  hiBytes: Uint8Array,
  piBytes: Uint8Array,
): Promise<ReadoutResult> {
  const g = await ensureCompute();
  return g.readout_receivers(scene, req, hiBytes, piBytes) as ReadoutResult;
}

/** Re-contour a cached level grid into a WGS84 GeoJSON FeatureCollection string. */
export async function traceIsophones(req: TraceIsophonesReq): Promise<string> {
  const g = await ensureCompute();
  return g.trace_isophones(req) as string;
}

/**
 * Reconstruct the fine-tier lattice into a dense 2-D level grid (GRID-04) from the
 * SAME `PlanTiersReq` the solve used plus a receiver-major `dba` readout vector
 * (indexed by global receiver index; `NaN` = no-data hole). The dB values are
 * WASM-produced (the caller assembles `dba` from `readoutReceivers` totals), so no
 * acoustic math happens in TS; this only scatters them onto the lattice via the
 * tested pure `reconstruct_level_grid`. The result is the `LevelGridInput`/
 * `ExportGridDto` the colour-scale store contours (SC3).
 */
export async function reconstructLevelGrid(
  planReq: PlanTiersReq,
  dba: Float64Array | readonly number[],
): Promise<ExportGridDto> {
  const g = await ensureCompute();
  return g.reconstruct_level_grid(planReq, Float64Array.from(dba)) as ExportGridDto;
}

/**
 * Project a WGS84 `[lng, lat]` to the auto-selected project UTM zone (GEOX-04) — the
 * anchor that places a reconstructed level grid at the drawn site. Returns easting +
 * northing (meters) + the UTM zone + hemisphere. Pure geometry (no acoustic math).
 */
export async function projectToUtm(
  lng: number,
  lat: number,
): Promise<{ easting: number; northing: number; utmZone: number; south: boolean }> {
  const g = await ensureCompute();
  const [easting, northing, zone, south] = g.project_to_utm(lng, lat) as Float64Array;
  return { easting, northing, utmZone: zone, south: south === 1 };
}

/** Compute the pre-run cost estimate + guardrail from the grid spec (SC1). */
export async function estimateCost(req: EstimateCostReq): Promise<CostEstimateResult> {
  const g = await ensureCompute();
  return g.estimate_cost(req) as CostEstimateResult;
}

/**
 * Encode the current result to the selected format's download bytes. `export` is exposed
 * as `_export` by wasm-bindgen (`export` is a reserved JS word).
 */
export async function exportEncode(req: ExportReq): Promise<Uint8Array> {
  const g = await ensureCompute();
  return g._export(req) as Uint8Array;
}

/** Sanitize a program-derived download filename base for a format (V12, T-11-09-02). */
export async function exportFilename(base: string, format: ExportFormat): Promise<string> {
  const g = await ensureCompute();
  return g.export_filename(base, format) as string;
}
