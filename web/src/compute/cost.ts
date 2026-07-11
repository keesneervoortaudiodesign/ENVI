// cost.ts — the thin main-thread wrapper over the wasm `estimate_cost` fn (SC1).
//
// # Module I/O
// - Input  an `EstimateCostReq` (grid spec: area, final spacing, discrete points,
//   sub-source count, worker count, and the hard OPFS byte budget). All values are
//   plain numbers the pre-run slider produces.
// - Output the generated `CostEstimateResult` (receiver count, tensor + working-set
//   bytes, a device-adaptive time estimate, and the guardrail verdict + detail).
//   The cost/guardrail MATH lives in `envi_compute::cost` (one source of truth,
//   RESEARCH Open Q3) — this module NEVER reimplements the byte/receiver arithmetic
//   in TS; it only marshals into the wasm fn. (The estimate is not acoustic math,
//   so calling it main-thread pre-run is not an SVC-07 violation.)
// - Valid input range: request DTO exactly as generated in `wire.ts`
//   (`deny_unknown_fields` on the Rust side rejects a typo'd key at the boundary).
//
// The threaded compute module is lazily initialised ONCE here (idempotent). Only
// `estimate_cost` — a pure fn — is called from the main thread; the rayon pool
// (`initThreadPool`) is never touched here (that is the worker's job, Pitfall 2).

import init, { estimate_cost as wasmEstimateCost } from "../generated/wasm-compute/envi_compute_wasm";
import type { CostEstimateResult, EstimateCostReq } from "../generated/wire";

// Initialise the threaded compute wasm module exactly once. `init()` only
// instantiates the module (defining `estimate_cost`); it does NOT spawn the pool,
// so a main-thread cost estimate is cheap and side-effect-free.
let ready: Promise<void> | null = null;
function ensureCompute(): Promise<void> {
  ready ??= init().then(() => undefined);
  return ready;
}

// Compute the pre-run cost estimate + guardrail from the grid spec (SC1). One
// audited cast site: the generated glue types `estimate_cost` as `any`; the wire
// DTO is the real shape.
export async function estimateCost(req: EstimateCostReq): Promise<CostEstimateResult> {
  await ensureCompute();
  return wasmEstimateCost(req) as CostEstimateResult;
}
