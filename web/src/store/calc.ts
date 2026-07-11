// calc.ts — the client-side calculation state slice (D-10: the calc job status is
// IN-APP React/zustand state driven by the compute Web Worker's postMessage, NOT
// the Phase-6 server-side async job channel — the solve is client-side). Sibling of
// `store/import.ts` / `store/weather.ts`; the CalcPanel (10-05) binds to it.
//
// # Module I/O
// - Input  the CalcPanel's spacing edits (`setSpacing`) and Run/Abort intents
//   (`run`/`abort`), the wasm cost estimate (`setCostEstimate`, from
//   `compute/cost.ts`), and the compute worker's forwarded events
//   (`setCapability` / `applyStatus` / `applyTierComplete`, via `compute/client.ts`).
// - Output the full pre-run + job state the panel renders: the final (fine)
//   spacing (default 10, D-06), the cost estimate + guardrail, the current
//   `JobStatus` lifecycle (`idle→queued→running→done|failed|cancelled`), overall
//   progress + message, per-tier receiver counts (filled from `TierComplete`), and
//   the `crossOriginIsolated` capability flag (Run is gated on it).
// - Valid input range: `spacing_fine_m` a positive number; job events are the
//   generated `JobStatus` / `TierComplete` wire shapes.
//
// This slice performs NO acoustic math and issues NO server request; it never
// subscribes to the server-side async job stream (that channel stays for ERA5/CDS,
// D-10). Cost comes from the wasm fn (`compute/cost.ts`); the worker owns the solve.

import { create } from "zustand";

import type {
  CostEstimateResult,
  GuardrailLevelDto,
  JobStatus,
  TierComplete,
  TierKindDto,
} from "../generated/wire";
import type { CalcJobSpec } from "../compute/worker";

// The default final (fine) grid spacing, meters (D-06). The user edits this; the
// preview tiers are auto-derived as coarser multiples.
export const DEFAULT_FINE_SPACING_M = 10;

// The default coarse preview multiple (×10 → 100 m tier, D-06). Data-driven so an
// intermediate tier is a one-element change.
export const DEFAULT_COARSE_MULTIPLES: readonly number[] = [10];

// The client-side job lifecycle (the reused `JobStatus` states plus `idle`).
export type CalcJobState = "idle" | "queued" | "running" | "done" | "failed" | "cancelled";

// The guardrail sub-state (mirrors the import slice's `GuardrailState` shape): a
// hard `blocked` flag (Run refused) + the human detail + the raw severity level.
export interface CalcGuardrail {
  readonly blocked: boolean;
  readonly detail: string;
  readonly level: GuardrailLevelDto;
}

// The three tier kinds' receiver counts, filled from each `TierComplete` (D-07).
export type TierCounts = Readonly<Record<TierKindDto, number>>;

// A minimal view of the main-thread client the store drives (set via `attachClient`
// at app init). Kept an interface so the store never imports the Worker-spawning
// `client.ts` at module load — the calc store stays unit-testable in Node.
export interface CalcClient {
  submit(spec: CalcJobSpec): void;
  cancel(): void;
}

function emptyTierCounts(): TierCounts {
  return { points: 0, coarse: 0, fine: 0 };
}

export interface CalcState {
  // Pre-run grid spec.
  readonly spacing_fine_m: number;
  readonly coarseMultiples: readonly number[];
  readonly costEstimate: CostEstimateResult | null;
  readonly guardrail: CalcGuardrail | null;

  // Job lifecycle (client-side, worker-driven).
  readonly jobState: CalcJobState;
  readonly progress: number;
  readonly message: string;
  readonly failureReason: string | null;
  readonly tierCounts: TierCounts;

  // Capability: SharedArrayBuffer / cross-origin isolation (gates Run, Pitfall 3).
  readonly crossOriginIsolated: boolean;

  // The attached worker client (null until `attachClient`).
  readonly client: CalcClient | null;

  attachClient(client: CalcClient): void;
  setSpacing(spacing_fine_m: number): void;
  setCostEstimate(estimate: CostEstimateResult): void;
  setCapability(crossOriginIsolated: boolean): void;
  run(spec: CalcJobSpec): void;
  abort(): void;
  applyStatus(status: JobStatus): void;
  applyTierComplete(event: TierComplete): void;
  reset(): void;
}

// Derive the guardrail sub-state from a wasm cost estimate: `Block` is the only
// hard-`blocked` verdict (Run refused); `Warn`/`Ok` are advisory.
function guardrailFrom(estimate: CostEstimateResult): CalcGuardrail {
  return {
    blocked: estimate.guardrail_level === "block",
    detail: estimate.guardrail_detail,
    level: estimate.guardrail_level,
  };
}

export const useCalcStore = create<CalcState>((set, get) => ({
  spacing_fine_m: DEFAULT_FINE_SPACING_M,
  coarseMultiples: DEFAULT_COARSE_MULTIPLES,
  costEstimate: null,
  guardrail: null,

  jobState: "idle",
  progress: 0,
  message: "",
  failureReason: null,
  tierCounts: emptyTierCounts(),

  crossOriginIsolated: false,
  client: null,

  attachClient: (client) => set({ client }),

  setSpacing: (spacing_fine_m) =>
    set({
      spacing_fine_m,
      // A spacing change invalidates the prior estimate until the panel recomputes
      // it from the wasm fn (no acoustic/byte math is done here in TS).
      costEstimate: null,
      guardrail: null,
    }),

  setCostEstimate: (estimate) =>
    set({ costEstimate: estimate, guardrail: guardrailFrom(estimate) }),

  setCapability: (crossOriginIsolated) => set({ crossOriginIsolated }),

  run: (spec) => {
    // A guardrail block must never be run (defence-in-depth beside the panel gate).
    if (get().guardrail?.blocked) {
      return;
    }
    get().client?.submit(spec);
    set({
      jobState: "queued",
      progress: 0,
      message: "queued",
      failureReason: null,
      tierCounts: emptyTierCounts(),
    });
  },

  abort: () => {
    // Cooperative abort only — delegate to the worker's request_cancel via the
    // client; the run lands `cancelled` at the next chunk boundary (D-11).
    get().client?.cancel();
  },

  applyStatus: (status) =>
    set(() => {
      switch (status.state) {
        case "queued":
          return { jobState: "queued", progress: 0, message: "queued", failureReason: null };
        case "running":
          return {
            jobState: "running",
            progress: status.progress,
            message: status.message,
            failureReason: null,
          };
        case "done":
          return { jobState: "done", progress: 1, message: "done" };
        case "failed":
          return { jobState: "failed", message: "failed", failureReason: status.reason };
        case "cancelled":
          return { jobState: "cancelled", message: "cancelled" };
        default:
          return {};
      }
    }),

  applyTierComplete: (event) =>
    set((s) => ({
      tierCounts: { ...s.tierCounts, [event.tier]: event.receiver_ids.length },
    })),

  reset: () =>
    set({
      jobState: "idle",
      progress: 0,
      message: "",
      failureReason: null,
      tierCounts: emptyTierCounts(),
    }),
}));
