// difference.ts — the scenario DIFFERENCE-MAP state (METX-04 / D-16, D-01). Holds the
// per-receiver signed dB(A) delta `A − B` between two computed scenarios' cached
// readouts, plus the lattice + CRS it maps onto, so the `differenceLayer` can render a
// diverging blue↔gray↔red fill.
//
// # Module I/O
// - Input  the two scenarios' cached per-receiver dB(A) totals (WASM-produced
//   `ReceiverReadoutDto::total_dba`) + their shared lattice/CRS + display labels,
//   via `compute(...)` (from the ScenarioPanel's "Compare scenarios" action).
// - Output the `delta` array + `grid`/`crs` the difference layer contours, the A/B
//   labels the legend shows, an `epoch` bump per applied compute (the offline-UAT
//   "the difference map rendered" signal), and a typed `error`.
// - Valid input range: the two total arrays must be the same length (aligned
//   receiver-for-receiver); the WASM boundary rejects a mismatch.
//
// # D-01 — ZERO acoustic arithmetic here
// The `A − B` subtraction runs in WASM (`difference_dba`): this store only marshals
// the two number arrays across the boundary and stores the returned delta verbatim.
// There is NO dB subtraction / weighting / logarithmic dB arithmetic in this file —
// the D-01 grep gate (no such Math calls) asserts it, and there is no arithmetic on
// acoustic values either (the delta is WASM-produced, placed by index only).

import { create } from "zustand";

import type { ExportCrsDto, ExportGridDto } from "../generated/wire";

// The WASM difference collaborator (injectable → the store stays Node-testable and
// the wasm graph stays out of module load, mirroring the results/conditioning seams).
export interface DifferenceClient {
  // Returns the elementwise signed dB(A) delta `a[i] − b[i]`, computed in WASM.
  difference(aDba: readonly number[], bDba: readonly number[]): Promise<number[]>;
}

// The real client: lazily dynamic-imports the gis-wasm `difference_dba` boundary
// (kept out of the Node unit-test module load). The subtraction is a pure numeric op
// on already-weighted WASM totals — TypeScript performs zero acoustic arithmetic.
export function createWasmDifferenceClient(): DifferenceClient {
  return {
    async difference(aDba, bDba) {
      const { differenceDba } = await import("../import/wasm");
      return differenceDba(aDba, bDba);
    },
  };
}

// The inputs a "Compare scenarios" action supplies: the two scenarios' cached
// per-receiver dB(A) totals, the shared lattice + CRS, and the display labels.
export interface DifferenceInput {
  readonly aDba: readonly number[];
  readonly bDba: readonly number[];
  readonly grid: ExportGridDto;
  readonly crs: ExportCrsDto;
  readonly labelA: string;
  readonly labelB: string;
}

export interface DifferenceState {
  // The WASM-produced per-receiver signed dB(A) delta (A − B), or null.
  readonly delta: readonly number[] | null;
  // The lattice + CRS the delta maps onto (the difference layer contours these).
  readonly grid: ExportGridDto | null;
  readonly crs: ExportCrsDto | null;
  readonly labelA: string;
  readonly labelB: string;
  // Bumped on each applied compute — the offline-UAT "difference rendered" signal.
  readonly epoch: number;
  readonly error: string | null;
  readonly client: DifferenceClient | null;

  attachClient(client: DifferenceClient): void;
  // Compute the A − B delta in WASM and cache it for the difference layer.
  compute(input: DifferenceInput): Promise<void>;
  clear(): void;
}

export const useDifferenceStore = create<DifferenceState>((set, get) => ({
  delta: null,
  grid: null,
  crs: null,
  labelA: "",
  labelB: "",
  epoch: 0,
  error: null,
  client: null,

  attachClient: (client) => set({ client }),

  compute: async (input) => {
    const { client } = get();
    if (!client) {
      return;
    }
    set({ error: null });
    try {
      // The A − B subtraction happens in WASM (D-01); this store only marshals the
      // two arrays and stores the returned delta (never a TS dB subtraction).
      const delta = await client.difference(input.aDba, input.bDba);
      set((s) => ({
        delta,
        // Map the WASM-produced delta onto the shared lattice (a plain array
        // assignment — no arithmetic on acoustic values).
        grid: { ...input.grid, values: delta },
        crs: input.crs,
        labelA: input.labelA,
        labelB: input.labelB,
        epoch: s.epoch + 1,
      }));
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
    }
  },

  clear: () => set({ delta: null, grid: null, crs: null, labelA: "", labelB: "", error: null }),
}));
