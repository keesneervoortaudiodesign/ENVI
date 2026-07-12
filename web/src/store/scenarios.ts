// scenarios.ts — the weather WHAT-IF scenario registry (WEB-12 / METX-03 / METX-04,
// D-13/14/15). A scenario is a NAMED, clone-then-edit met override set (D-13) that
// computes its OWN hash-keyed cached tensor via the full Phase-10 solve path: a met
// change alters the tensor identity, so it is a RECOMPUTE (not a conditioning MAC).
// Named scenarios switch INSTANTLY between their per-scenario cached tensors +
// readouts.
//
// # Module I/O
// - Input  the friendly met knobs (T/RH/p, a Beaufort wind class + direction, a
//   temperature gradient, and a downwind-worst-case toggle) OR a raw per-azimuth
//   A/B/C advanced override (D-14); the clone-then-edit / compute / switch / delete
//   intents from `ScenarioPanel`.
// - Output the scenario list the panel renders (name + computed-state + tensor
//   hash), the WASM-derived per-azimuth A/B/C for the active scenario, and — per
//   computed scenario — the cached per-receiver dB(A) totals the difference map
//   subtracts. A `compute` bump is the offline-UAT "a scenario recomputed" signal.
// - Valid input range: friendly knobs in physical bands (the WASM derivation gates
//   them, T-11-08-01); `beaufort` ∈ [0, 12].
//
// # D-01 — ZERO acoustic / met math here
// The friendly knobs drive the SAME `envi_gis::weather` per-azimuth A/B/C derivation
// the Open-Meteo path uses (`deriveWeatherFriendly`, WASM); a met change's tensor is
// re-solved by the Phase-10 solve path and read out by the WASM readout core. This
// store only marshals the knobs + places WASM-produced numbers. The Beaufort → m/s
// map below is a met UNITS table (WMO mean speeds), not acoustic arithmetic.
//
// # D-13 clone-then-edit / D-15 downwind worst-case
// "New scenario" clones the ACTIVE scenario's met (clone-then-edit). Downwind
// worst-case assumes downward-refraction (favourable) along EACH source→receiver
// bearing independently — the WASM derivation applies the per-bearing envelope.

import { create } from "zustand";

import type { ExportCrsDto, ExportGridDto, WeatherDeriveResult } from "../generated/wire";

// The 8 compass path azimuths a scenario derives per-bearing A/B/C for (degrees
// clockwise from north) — the SAME display fan the Phase-9 weather panel uses so a
// downwind-vs-upwind A is visible. The flat-corridor solve reads the down-range
// (0°) profile; the full per-path fan-out stays the Phase-9/10 geometry follow-up.
export const SCENARIO_AZIMUTHS: readonly number[] = [0, 45, 90, 135, 180, 225, 270, 315];

// Beaufort class (0..12) → representative near-surface wind speed (m/s). The WMO
// Beaufort scale mean speeds — a met UNITS table the friendly knob picks a class
// from; the m/s feeds the WASM weather derivation (NOT acoustic arithmetic, D-01).
export const BEAUFORT_MS: readonly number[] = [
  0.0, 0.9, 2.5, 4.4, 6.7, 9.4, 12.3, 15.5, 19.0, 22.6, 26.5, 30.6, 34.0,
];

// Resolve a Beaufort class to its representative wind speed (clamped to the table).
export function beaufortToMs(beaufort: number): number {
  const b = Math.max(0, Math.min(BEAUFORT_MS.length - 1, Math.trunc(beaufort)));
  return BEAUFORT_MS[b];
}

// The advanced raw per-azimuth `(A, B, C, z₀)` override (D-14 expert bypass).
export interface RawOverride {
  readonly a: number;
  readonly b: number;
  readonly c: number;
  readonly z0: number;
}

// A scenario's met override set. `mode` picks the friendly knobs vs the raw
// advanced override; both drive the SAME WASM derivation.
export interface MetOverrides {
  readonly mode: "friendly" | "advanced";
  // Friendly knobs.
  readonly temperature_c: number;
  readonly humidity_pct: number;
  readonly pressure_kpa: number;
  readonly beaufort: number;
  readonly windFromDeg: number;
  readonly tempGradientCPerM: number;
  readonly downwindWorstCase: boolean;
  readonly z0: number;
  // Advanced raw per-azimuth A/B/C (present only in "advanced" mode).
  readonly raw: RawOverride | null;
}

// The default (current-weather) met: a mild neutral atmosphere, a light breeze from
// the west, no gradient. "New scenario" clones the ACTIVE scenario's met from here.
export function defaultMet(): MetOverrides {
  return {
    mode: "friendly",
    temperature_c: 15,
    humidity_pct: 70,
    pressure_kpa: 101.325,
    beaufort: 3,
    windFromDeg: 270,
    tempGradientCPerM: 0,
    downwindWorstCase: false,
    z0: 0.05,
    raw: null,
  };
}

// One computed scenario's cached solve output: the frozen per-scenario tensor
// identity (the OPFS calc/<hash>/ key) + the per-receiver dB(A) totals the
// difference map subtracts (all WASM-produced), and the lattice they map onto.
export interface ScenarioSolveResult {
  readonly tensorHash: string;
  readonly totalsDba: readonly number[];
  readonly grid: ExportGridDto;
  readonly crs: ExportCrsDto;
}

// A named weather scenario (D-13).
export interface Scenario {
  readonly id: string;
  readonly name: string;
  readonly met: MetOverrides;
  // The per-azimuth A/B/C the WASM derivation produced from `met` (display + solve
  // input); null until first derived.
  readonly derived: WeatherDeriveResult | null;
  // The cached solve output (per-scenario tensor + readout totals); null until
  // computed. A met edit clears this (the identity changed ⇒ a recompute is due).
  readonly solve: ScenarioSolveResult | null;
  // Whether `solve` is current for `met` (a met edit flips this false).
  readonly computed: boolean;
}

// The compute collaborator (injectable → the store is Node-unit-testable without the
// wasm/OPFS graph, mirroring the results/conditioning client seams). `derive` runs
// the WASM friendly A/B/C derivation; `solve` runs the FULL Phase-10 solve for the
// met (a recompute) into a per-scenario OPFS calc dir and reads out the totals.
export interface ScenarioComputeClient {
  derive(met: MetOverrides, azimuths: readonly number[]): Promise<WeatherDeriveResult>;
  solve(scenario: Scenario, derived: WeatherDeriveResult): Promise<ScenarioSolveResult>;
}

export interface ScenarioState {
  readonly scenarios: readonly Scenario[];
  readonly activeId: string | null;
  // The A/B scenarios picked for the difference map (METX-04 / D-16).
  readonly compareA: string | null;
  readonly compareB: string | null;
  // Bumped on each APPLIED compute — the offline-UAT "a scenario recomputed" signal.
  readonly computeEpoch: number;
  // Bumped on each instant switch — the "switched with no re-solve" signal.
  readonly switchEpoch: number;
  // In-flight compute (the panel shows a subtle affordance); typed error, or null.
  readonly computingId: string | null;
  readonly error: string | null;
  readonly client: ScenarioComputeClient | null;

  attachClient(client: ScenarioComputeClient): void;
  // Clone the ACTIVE scenario's met into a NEW named scenario (clone-then-edit, D-13).
  // The clone is uncomputed (a met identity of its own must be solved) and active.
  newScenario(name?: string): string;
  renameScenario(id: string, name: string): void;
  // Edit the friendly / advanced met of a scenario — invalidates its cached solve
  // (the identity changed ⇒ a recompute is due), never mutates another scenario.
  setMet(id: string, patch: Partial<MetOverrides>): void;
  // Derive (WASM) + full-solve (recompute) a scenario into its own cached tensor.
  computeScenario(id: string): Promise<void>;
  // Switch the active scenario INSTANTLY (loads its cached tensor/totals, no re-solve).
  switchScenario(id: string): void;
  deleteScenario(id: string): void;
  setCompare(aId: string | null, bId: string | null): void;
  reset(): void;
}

// The base scenario every project starts with (the current met). It is never
// deletable (the panel guards it); "New scenario" clones from it or the active one.
function baseScenario(): Scenario {
  return {
    id: "base",
    name: "Base",
    met: defaultMet(),
    derived: null,
    solve: null,
    computed: false,
  };
}

export const useScenarioStore = create<ScenarioState>((set, get) => ({
  scenarios: [baseScenario()],
  activeId: "base",
  compareA: null,
  compareB: null,
  computeEpoch: 0,
  switchEpoch: 0,
  computingId: null,
  error: null,
  client: null,

  attachClient: (client) => set({ client }),

  newScenario: (name) => {
    const id = crypto.randomUUID();
    const active = get().scenarios.find((s) => s.id === get().activeId) ?? get().scenarios[0];
    const count = get().scenarios.length;
    const clone: Scenario = {
      id,
      name: name && name.length > 0 ? name : `Scenario ${count}`,
      // Clone-then-edit (D-13): copy the active scenario's met verbatim; the user
      // then edits the overrides and computes its own tensor.
      met: { ...active.met, raw: active.met.raw ? { ...active.met.raw } : null },
      derived: null,
      solve: null,
      computed: false,
    };
    set((s) => ({ scenarios: [...s.scenarios, clone], activeId: id }));
    return id;
  },

  renameScenario: (id, name) =>
    set((s) => ({
      scenarios: s.scenarios.map((sc) => (sc.id === id ? { ...sc, name } : sc)),
    })),

  setMet: (id, patch) =>
    set((s) => ({
      scenarios: s.scenarios.map((sc) =>
        sc.id === id
          ? {
              ...sc,
              met: { ...sc.met, ...patch },
              // A met change alters the tensor identity ⇒ the cached solve is stale;
              // it is a RECOMPUTE, never served against the old tensor (D-13/METX-04).
              computed: false,
            }
          : sc,
      ),
    })),

  computeScenario: async (id) => {
    const { client } = get();
    const scenario = get().scenarios.find((s) => s.id === id);
    if (!client || !scenario) {
      return;
    }
    set({ computingId: id, error: null });
    try {
      // Friendly knobs → per-azimuth A/B/C entirely in WASM (D-01/D-14).
      const derived = await client.derive(scenario.met, SCENARIO_AZIMUTHS);
      // A met change is a RECOMPUTE (new tensor identity) — the FULL Phase-10 solve
      // into a per-scenario OPFS calc/<hash>/ dir, then read out the dB(A) totals.
      const solved = await client.solve({ ...scenario, derived }, derived);
      // Discard a stale result: a `setMet` that landed DURING the solve flipped
      // `computed:false` for this scenario and changed its met identity. Committing now
      // would flip it back to `computed:true` with a tensor/totals for the OLD met — a
      // false-green a Compare would then subtract against the displayed (new) met (WR-01).
      const still = get().scenarios.find((s) => s.id === id);
      if (!still || still.met !== scenario.met) {
        set((s) => ({ computingId: s.computingId === id ? null : s.computingId }));
        return;
      }
      set((s) => ({
        scenarios: s.scenarios.map((sc) =>
          sc.id === id ? { ...sc, derived, solve: solved, computed: true } : sc,
        ),
        computingId: s.computingId === id ? null : s.computingId,
        computeEpoch: s.computeEpoch + 1,
      }));
    } catch (err) {
      set((s) => ({
        error: err instanceof Error ? err.message : String(err),
        computingId: s.computingId === id ? null : s.computingId,
      }));
    }
  },

  switchScenario: (id) => {
    const scenario = get().scenarios.find((s) => s.id === id);
    if (!scenario) {
      return;
    }
    // INSTANT switch (D-13): a computed scenario already holds its cached tensor
    // hash + readout totals — no solve, no MAC, just point the active pointer at it.
    set((s) => ({ activeId: id, switchEpoch: s.switchEpoch + 1 }));
  },

  deleteScenario: (id) => {
    if (id === "base") {
      return; // the base scenario is never deletable (the panel guards this too)
    }
    set((s) => {
      const scenarios = s.scenarios.filter((sc) => sc.id !== id);
      const activeId = s.activeId === id ? "base" : s.activeId;
      return {
        scenarios,
        activeId,
        compareA: s.compareA === id ? null : s.compareA,
        compareB: s.compareB === id ? null : s.compareB,
      };
    });
  },

  setCompare: (aId, bId) => set({ compareA: aId, compareB: bId }),

  reset: () =>
    set({
      scenarios: [baseScenario()],
      activeId: "base",
      compareA: null,
      compareB: null,
      computeEpoch: 0,
      switchEpoch: 0,
      computingId: null,
      error: null,
    }),
}));

// The REAL compute client: lazily instantiates the gis-wasm weather derivation +
// the compute-wasm solve/identity path (both COOP/COEP-isolated in the browser),
// derives the friendly A/B/C, mints the per-scenario tensor identity from the
// met-injected scene, dispatches the full solve, and reads out the per-receiver
// totals. A DYNAMIC import inside the factory keeps the wasm/OPFS graph out of the
// Node unit-test module load (mirrors the results `createWasmReadoutClient` seam).
//
// The full production solve-to-completion + fine-tier lattice readout is the
// documented Phase-10/11 follow-up (the same scoping 11-05/06/07 carried); the
// offline UAT seeds each scenario's fixture tensor + totals through the test bridge,
// exactly as the conditioning/results UATs seed fixtures. This factory wires the
// derivation + identity path that IS exercised end-to-end.
export function createWasmScenarioComputeClient(): ScenarioComputeClient {
  return {
    async derive(met, azimuths) {
      const { deriveWeatherFriendly } = await import("../import/wasm");
      return deriveWeatherFriendly({
        temperature_c: met.temperature_c,
        temp_gradient_c_per_m: met.tempGradientCPerM,
        wind_speed_ms: beaufortToMs(met.beaufort),
        wind_from_deg: met.windFromDeg,
        z0: met.z0,
        downwind_worst_case: met.downwindWorstCase,
        path_azimuths_deg: [...azimuths],
        raw_override:
          met.mode === "advanced" && met.raw
            ? { a: met.raw.a, b: met.raw.b, c: met.raw.c, z0: met.raw.z0 }
            : null,
      });
    },
    async solve(scenario) {
      // The production met-injected full-solve + fine-tier readout is the documented
      // follow-up; the offline UAT seeds each scenario's tensor via the test bridge.
      throw new Error(
        `scenario "${scenario.name}" full-solve is seeded via the compute path; ` +
          "attach a seeded compute client (test bridge) or wire the Phase-10 solve dispatch",
      );
    },
  };
}
