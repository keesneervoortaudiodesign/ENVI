// scenarios.test.ts — the WEB-12 / METX-03/04 scenario-registry unit tests (Node,
// no wasm/OPFS): clone-then-edit produces a DISTINCT scenario with its OWN tensor
// hash, switching loads the cached tensor with NO re-solve, the friendly knobs route
// to the (injected) WASM weather derivation, and downwind worst-case is carried into
// the derivation per bearing (D-13/14/15).

import { beforeEach, describe, expect, it, vi } from "vitest";

import type { WeatherDeriveResult } from "../generated/wire";
import {
  beaufortToMs,
  useScenarioStore,
  type MetOverrides,
  type ScenarioComputeClient,
  type ScenarioSolveResult,
} from "./scenarios";

// A fake compute client that records every derive call and mints a DISTINCT tensor
// hash per met identity (a stand-in for the WASM derivation + Phase-10 solve). The
// hash is a stable digest of the met knobs, so two different met sets ⇒ two hashes.
function fakeClient(): {
  client: ScenarioComputeClient;
  deriveCalls: { met: MetOverrides; azimuths: readonly number[] }[];
  solveCalls: string[];
} {
  const deriveCalls: { met: MetOverrides; azimuths: readonly number[] }[] = [];
  const solveCalls: string[] = [];
  const client: ScenarioComputeClient = {
    async derive(met, azimuths) {
      deriveCalls.push({ met, azimuths });
      const derived: WeatherDeriveResult = {
        components: { a_temp: 0, a_wind: met.downwindWorstCase ? 1 : 0.5, b: 0, c: 340, s_a: 0, s_b: 0, z0: met.z0 },
        profiles: azimuths.map(() => ({ a: 0.5, b: 0, c: 340, s_a: 0, s_b: 0, z0: met.z0 })),
      };
      return derived;
    },
    async solve(scenario) {
      solveCalls.push(scenario.id);
      // A deterministic per-met identity: distinct met ⇒ distinct hash.
      const key = JSON.stringify(scenario.met);
      const hash = `tensor-${key.length}-${beaufortToMs(scenario.met.beaufort)}-${scenario.met.temperature_c}`;
      const solved: ScenarioSolveResult = {
        tensorHash: hash,
        totalsDba: [50 + scenario.met.temperature_c, 55, 60],
        grid: { rows: 1, cols: 3, origin: [0, 0], spacing_m: 10, values: [50, 55, 60] },
        crs: { utm_zone: 31, south: false },
      };
      return solved;
    },
  };
  return { client, deriveCalls, solveCalls };
}

describe("scenario registry", () => {
  beforeEach(() => {
    useScenarioStore.getState().reset();
  });

  it("starts with a single base scenario active (the 'One scenario' empty state)", () => {
    const s = useScenarioStore.getState();
    expect(s.scenarios).toHaveLength(1);
    expect(s.scenarios[0].id).toBe("base");
    expect(s.activeId).toBe("base");
  });

  it("clone-then-edit produces a distinct scenario with its OWN tensor hash", async () => {
    const { client } = fakeClient();
    useScenarioStore.getState().attachClient(client);

    // Compute the base scenario.
    await useScenarioStore.getState().computeScenario("base");
    const base = useScenarioStore.getState().scenarios.find((s) => s.id === "base");
    expect(base?.computed).toBe(true);
    const baseHash = base?.solve?.tensorHash;
    expect(baseHash).toBeTruthy();

    // Clone (clone-then-edit): the new scenario copies the base met verbatim.
    const cloneId = useScenarioStore.getState().newScenario("Warm inversion");
    expect(useScenarioStore.getState().activeId).toBe(cloneId);
    const cloned = useScenarioStore.getState().scenarios.find((s) => s.id === cloneId);
    expect(cloned?.met.temperature_c).toBe(base?.met.temperature_c);
    expect(cloned?.computed).toBe(false); // a clone must be solved for its own identity

    // Edit the met — a distinct identity ⇒ its cached solve stays cleared.
    useScenarioStore.getState().setMet(cloneId, { temperature_c: 30, tempGradientCPerM: 0.02 });
    expect(useScenarioStore.getState().scenarios.find((s) => s.id === cloneId)?.computed).toBe(false);

    // Compute the clone → a DISTINCT tensor hash (the met differs).
    await useScenarioStore.getState().computeScenario(cloneId);
    const cloneHash = useScenarioStore.getState().scenarios.find((s) => s.id === cloneId)?.solve
      ?.tensorHash;
    expect(cloneHash).toBeTruthy();
    expect(cloneHash).not.toBe(baseHash);
  });

  it("switching between named scenarios is INSTANT — no re-solve", async () => {
    const { client, solveCalls } = fakeClient();
    useScenarioStore.getState().attachClient(client);
    await useScenarioStore.getState().computeScenario("base");
    const cloneId = useScenarioStore.getState().newScenario("Downwind");
    await useScenarioStore.getState().computeScenario(cloneId);
    expect(solveCalls).toHaveLength(2); // one solve per compute

    // Switch back and forth — NO further solve is dispatched (the tensors are cached).
    const before = solveCalls.length;
    const switchBefore = useScenarioStore.getState().switchEpoch;
    useScenarioStore.getState().switchScenario("base");
    useScenarioStore.getState().switchScenario(cloneId);
    expect(solveCalls).toHaveLength(before); // NO re-solve on switch
    expect(useScenarioStore.getState().switchEpoch).toBe(switchBefore + 2);
    expect(useScenarioStore.getState().activeId).toBe(cloneId);
  });

  it("routes the friendly knobs to the derivation and carries downwind worst-case", async () => {
    const { client, deriveCalls } = fakeClient();
    useScenarioStore.getState().attachClient(client);
    useScenarioStore
      .getState()
      .setMet("base", { beaufort: 6, downwindWorstCase: true, windFromDeg: 90 });
    await useScenarioStore.getState().computeScenario("base");
    expect(deriveCalls).toHaveLength(1);
    // The friendly knobs (not TS-computed A/B/C) reach the WASM derivation verbatim.
    expect(deriveCalls[0].met.beaufort).toBe(6);
    expect(deriveCalls[0].met.downwindWorstCase).toBe(true);
    // Downwind worst-case makes the derivation report a stronger (favourable) wind
    // part — the fake mirrors the WASM per-bearing favourable envelope (D-15).
    const derived = useScenarioStore.getState().scenarios.find((s) => s.id === "base")?.derived;
    expect(derived?.components.a_wind).toBe(1);
  });

  it("deletes a user scenario (never the base) and clears its compare slot", async () => {
    const { client } = fakeClient();
    useScenarioStore.getState().attachClient(client);
    const cloneId = useScenarioStore.getState().newScenario("Temp");
    useScenarioStore.getState().setCompare("base", cloneId);
    useScenarioStore.getState().deleteScenario(cloneId);
    expect(useScenarioStore.getState().scenarios.find((s) => s.id === cloneId)).toBeUndefined();
    expect(useScenarioStore.getState().compareB).toBeNull();
    expect(useScenarioStore.getState().activeId).toBe("base");
    // The base scenario resists deletion.
    useScenarioStore.getState().deleteScenario("base");
    expect(useScenarioStore.getState().scenarios.find((s) => s.id === "base")).toBeTruthy();
  });

  it("Beaufort classes map to WMO mean wind speeds (a units table, not acoustics)", () => {
    expect(beaufortToMs(0)).toBe(0);
    expect(beaufortToMs(3)).toBeGreaterThan(beaufortToMs(1));
    expect(beaufortToMs(99)).toBe(beaufortToMs(12)); // clamped to the table
  });
});

// Guard the compute client's fake is exercised (no accidental real-client fall-through).
it("uses the injected client (never the real wasm client in Node)", async () => {
  useScenarioStore.getState().reset();
  const spy = vi.fn();
  useScenarioStore.getState().attachClient({
    derive: async (met, az) => {
      spy();
      return { components: { a_temp: 0, a_wind: 0, b: 0, c: 340, s_a: 0, s_b: 0, z0: met.z0 }, profiles: az.map(() => ({ a: 0, b: 0, c: 340, s_a: 0, s_b: 0, z0: met.z0 })) };
    },
    solve: async () => ({ tensorHash: "h", totalsDba: [1], grid: { rows: 1, cols: 1, origin: [0, 0], spacing_m: 10, values: [1] }, crs: { utm_zone: 31, south: false } }),
  });
  await useScenarioStore.getState().computeScenario("base");
  expect(spy).toHaveBeenCalledOnce();
});
