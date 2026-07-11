// worker.ts — the dedicated compute Web Worker that OWNS the client-side calc job
// lifecycle (D-10, SVC-02). It initialises the threaded wasm module + the
// `wasm-bindgen-rayon` pool ONCE (Pitfall 2), asserts cross-origin isolation
// (Pitfall 3), runs the hierarchical tier loop (points → coarse → fine) posting
// the reused `JobStatus` vocabulary + `TierComplete` events over `postMessage`,
// and aborts COOPERATIVELY via the wasm `request_cancel()` atomic flag — NEVER
// `worker.terminate()` (D-11).
//
// # Module I/O
// - Input  `WorkerInbound` messages from the main-thread client (`submit` a job
//   spec / `cancel`), plus the threaded wasm exports (dynamically imported in the
//   real worker; injected as `deps` in unit tests).
// - Output `WorkerOutbound` messages: a `capability` snapshot, `JobStatus`
//   transitions (`queued`/`running`/`done`/`failed`/`cancelled`), and a
//   `TierComplete` event after each tier's chunk files flush (D-07).
// - Valid input range: a `CalcJobSpec` whose `planReq` is the generated
//   `PlanTiersReq`; a `cancel` at any time (lands at the next chunk boundary).
//
// # Why not the SSE machine (D-10)
// The solve is CLIENT-SIDE; there is no server round-trip. The job status is
// in-app state driven by these `postMessage`s (mirrors the Phase-8 import slice),
// never the Phase-6 server SSE job machine (that stays for ERA5/CDS).
//
// # Testability
// The job machine is `createJobMachine(deps)` with the wasm module + the `post`
// sink injected, so a Vitest unit test drives the whole tier loop with a mock wasm
// and a fake capability flag — no browser, no real OPFS. The real worker wiring at
// the bottom is guarded so importing this module under Vitest (Node) is inert.

import type {
  JobStatus,
  PlanTiersReq,
  PrepareSolveReq,
  TierComplete,
  TierPlanResult,
} from "../generated/wire";
import {
  chunkRelativePath,
  preopenChunk,
  releasePreopenedChunk,
  setActiveProject,
} from "./opfs";

// A calc job the client submits to the worker. `planReq` is the generated tier
// partition request; `scene` is the generated `PrepareSolveReq` marshalled ONCE per
// submit (the whole transfer scene); `receiverIds` are the TS-minted
// (crypto.randomUUID) receiver UUIDs indexed by GLOBAL receiver index (Pitfall 9 —
// the wasm mints no ids); `chunkReceivers` is the receiver-axis shard size (one OPFS
// file per chunk).
export interface CalcJobSpec {
  readonly projectId: string;
  readonly tensorHash: string;
  readonly planReq: PlanTiersReq;
  readonly scene: PrepareSolveReq;
  readonly receiverIds: readonly string[];
  readonly nSub: number;
  readonly chunkReceivers: number;
}

// Messages the main-thread client posts to the worker.
export type WorkerInbound =
  | { readonly type: "submit"; readonly spec: CalcJobSpec }
  | { readonly type: "cancel" };

// Messages the worker posts back to the client (forwarded to the calc store).
export type WorkerOutbound =
  | { readonly type: "capability"; readonly crossOriginIsolated: boolean }
  | { readonly type: "status"; readonly status: JobStatus }
  | { readonly type: "tier"; readonly event: TierComplete };

// The threaded wasm exports the job machine drives. Mirrors the generated
// `envi_compute_wasm` glue surface (typed here rather than `any`); the machine
// calls exactly these — never a second solve path (Pattern 1).
export interface ComputeWasm {
  plan_tiers(req: PlanTiersReq): TierPlanResult;
  // Marshal the whole transfer scene ONCE per submit, keyed by tensor_hash. Called
  // before the tier loop; every solve_chunk_range then solves against it.
  prepare_solve(req: PrepareSolveReq): unknown;
  solve_chunk_range(req: {
    tensor_hash: string;
    chunk_index: number;
    r_offset: number;
    len: number;
  }): unknown;
  request_cancel(): void;
  reset_cancel(): void;
}

// The environment + collaborators the job machine needs (all injectable for tests).
export interface JobDeps {
  readonly wasm: ComputeWasm;
  readonly post: (msg: WorkerOutbound) => void;
  // `self.crossOriginIsolated` — must be true for SharedArrayBuffer / the pool.
  readonly crossOriginIsolated: boolean;
  // Whether `SharedArrayBuffer` is defined (the pool's backing memory).
  readonly hasSharedArrayBuffer: boolean;
}

// The honest capability-failure reason (UI-SPEC S1) — NOT a generic failure.
const CAPABILITY_FAILURE =
  "This browser session is not cross-origin isolated, so the multi-threaded " +
  "calculation cannot start. Reload the app from its server (it sends the required " +
  "COOP/COEP headers). If this keeps happening, your browser may not support " +
  "SharedArrayBuffer.";

// The job machine: a single in-flight client-side calc job at a time (T-10-04-05 —
// a new submit supersedes the previous run). `submit` runs the tier loop; `cancel`
// flips the cooperative flag so the run lands `cancelled` at the next chunk
// boundary with already-emitted tiers intact.
export function createJobMachine(deps: JobDeps): {
  submit(spec: CalcJobSpec): Promise<void>;
  cancel(): void;
} {
  let cancelled = false;

  // Post the capability snapshot once at construction so the UI can gate Run.
  deps.post({ type: "capability", crossOriginIsolated: deps.crossOriginIsolated });

  function post(status: JobStatus): void {
    deps.post({ type: "status", status });
  }

  function cancel(): void {
    cancelled = true;
    // Cooperative abort ONLY (D-11): flip the wasm SharedArrayBuffer atomic flag
    // the rayon pool checks between chunk ranges. NEVER tear the worker down.
    deps.wasm.request_cancel();
  }

  async function submit(spec: CalcJobSpec): Promise<void> {
    // Capability gate (Pitfall 3): refuse honestly when not cross-origin isolated.
    if (!deps.crossOriginIsolated || !deps.hasSharedArrayBuffer) {
      deps.post({ type: "capability", crossOriginIsolated: false });
      post({ state: "failed", reason: CAPABILITY_FAILURE });
      return;
    }

    cancelled = false;
    deps.wasm.reset_cancel();
    post({ state: "queued" });

    // Marshal the whole transfer scene ONCE per submit, BEFORE the tier loop (D-08):
    // build the owned PreparedScene keyed by tensor_hash the wasm side solves against,
    // and point OPFS at this project so per-chunk handles resolve. A shape/validation
    // throw lands the job `failed` (never a half-run).
    try {
      setActiveProject(spec.projectId);
      deps.wasm.prepare_solve(spec.scene);
    } catch (err) {
      const reason = err instanceof Error ? err.message : String(err);
      post({ state: "failed", reason: reason.length > 0 ? reason : "scene preparation failed" });
      return;
    }

    post({ state: "running", progress: 0.0001, message: "planning tiers" });

    try {
      const plan = deps.wasm.plan_tiers(spec.planReq);
      const chunkSize = Math.max(1, spec.chunkReceivers);
      const totalChunks = Math.max(
        1,
        plan.tiers.reduce((n, t) => n + Math.ceil(t.receivers.length / chunkSize), 0),
      );

      let globalChunk = 0;
      let chunksDone = 0;

      for (let t = 0; t < plan.tiers.length; t += 1) {
        const tier = plan.tiers[t];
        const spans: TierComplete["spans"] = [];
        let offset = 0;

        while (offset < tier.receivers.length) {
          // Cooperative abort at the chunk boundary (D-11).
          if (cancelled) {
            deps.wasm.request_cancel();
            post({ state: "cancelled" });
            return;
          }
          const len = Math.min(chunkSize, tier.receivers.length - offset);
          const chunkIndex = globalChunk;
          globalChunk += 1;
          const rOffsetGlobal = tier.receivers[offset].global_index;

          // Run one disjoint range on the pool (its own OPFS chunk file). The
          // engine `solve()` runs UNCHANGED behind this boundary (pool.rs).
          await deps.wasm.solve_chunk_range({
            tensor_hash: spec.tensorHash,
            chunk_index: chunkIndex,
            r_offset: rOffsetGlobal,
            len,
          });

          // A cancel that arrived DURING the range lands here too.
          if (cancelled) {
            post({ state: "cancelled" });
            return;
          }

          spans.push({
            chunk_index: chunkIndex,
            r_offset: rOffsetGlobal,
            len,
            tensor_file: chunkRelativePath("tensor", chunkIndex),
            pincoh_file: chunkRelativePath("pincoh", chunkIndex),
          });

          chunksDone += 1;
          offset += len;
          post({
            state: "running",
            progress: Math.min(1, chunksDone / totalChunks),
            message: `tier ${t + 1}/${plan.tiers.length} · chunk ${chunksDone}/${totalChunks}`,
          });
        }

        // Tier files flushed → emit the tier-complete event (D-07). Phase 11 reads
        // these spans to render points → coarse map → refined map.
        const receiver_ids = tier.receivers.map((r) => spec.receiverIds[r.global_index] ?? "");
        const event: TierComplete = {
          kind: "tier_complete",
          tier: tier.kind,
          tier_index: t,
          spacing_m: tier.spacing_m,
          tensor_hash: spec.tensorHash,
          receiver_ids,
          spans,
        };
        deps.post({ type: "tier", event });
      }

      post({ state: "done" });
    } catch (err) {
      if (cancelled) {
        post({ state: "cancelled" });
        return;
      }
      const reason = err instanceof Error ? err.message : String(err);
      post({ state: "failed", reason: reason.length > 0 ? reason : "calculation failed" });
    }
  }

  return { submit, cancel };
}

// --- Real dedicated-worker wiring (guarded so Vitest/Node import is inert) --------

// A minimal structural view of the dedicated-worker global scope (avoids depending
// on the WebWorker TS lib, which tsconfig does not include).
interface WorkerScope {
  postMessage(message: unknown): void;
  onmessage: ((ev: MessageEvent<WorkerInbound>) => void) | null;
  crossOriginIsolated?: boolean;
}

// Are we actually running inside a dedicated Web Worker? `WorkerGlobalScope` +
// `importScripts` exist only there — never in the Vitest/Node unit environment.
function inDedicatedWorker(): boolean {
  const g = globalThis as { WorkerGlobalScope?: unknown; importScripts?: unknown };
  return typeof g.WorkerGlobalScope !== "undefined" && typeof g.importScripts === "function";
}

// Boot the real worker: init the threaded wasm module + the pool ONCE (Pitfall 2),
// then bind the job machine to `postMessage` and the inbound message stream.
async function bootstrapWorker(scope: WorkerScope): Promise<void> {
  const isolated = scope.crossOriginIsolated === true;
  const hasSAB = typeof SharedArrayBuffer !== "undefined";

  // The threaded glue is imported dynamically so the static module graph (and thus
  // the Vitest import) never pulls the wasm/worker snippets.
  const glue = await import("../generated/wasm-compute/envi_compute_wasm");

  if (isolated && hasSAB) {
    // init the module, THEN size the SharedArrayBuffer pool to the device core
    // count ONCE before any solve (Pitfall 2 — await both).
    await glue.default();
    await glue.initThreadPool(navigator.hardwareConcurrency);
  }

  const wasm: ComputeWasm = {
    plan_tiers: (req) => glue.plan_tiers(req) as TierPlanResult,
    prepare_solve: (req) => glue.prepare_solve(req),
    // Hoist the async OPFS open ahead of the synchronous engine solve (D-08): open
    // the chunk's tensor + pincoh sync handles, run the solve (Rust's OpfsChunkSink
    // closes them on finish), then release any handle still registered on an early
    // error so no OPFS lock leaks.
    solve_chunk_range: async (req) => {
      await preopenChunk(req.tensor_hash, "tensor", req.chunk_index);
      await preopenChunk(req.tensor_hash, "pincoh", req.chunk_index);
      try {
        return glue.solve_chunk_range(req);
      } finally {
        releasePreopenedChunk("tensor", req.chunk_index);
        releasePreopenedChunk("pincoh", req.chunk_index);
      }
    },
    request_cancel: () => glue.request_cancel(),
    reset_cancel: () => glue.reset_cancel(),
  };

  const machine = createJobMachine({
    wasm,
    post: (msg) => scope.postMessage(msg),
    crossOriginIsolated: isolated,
    hasSharedArrayBuffer: hasSAB,
  });

  scope.onmessage = (ev: MessageEvent<WorkerInbound>) => {
    const msg = ev.data;
    if (msg.type === "submit") {
      void machine.submit(msg.spec);
    } else if (msg.type === "cancel") {
      machine.cancel();
    }
  };
}

if (inDedicatedWorker()) {
  void bootstrapWorker(self as unknown as WorkerScope);
}
