// calc.test.ts — the client-side calc store slice (Vitest, no browser/worker).
// Asserts the default final spacing (D-06), the worker-driven job lifecycle
// (idle→running→done from forwarded JobStatus events), a guardrail block, and the
// per-tier receiver counts filled from TierComplete.

import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  CostEstimateResult,
  JobStatus,
  PrepareSolveReq,
  TierComplete,
} from "../generated/wire";
import { DEFAULT_FINE_SPACING_M, useCalcStore } from "./calc";

// A minimal (unvalidated) transfer scene — the store forwards the spec to a mocked
// submit, so the scene content is irrelevant beyond satisfying the CalcJobSpec type.
function minimalScene(): PrepareSolveReq {
  return {
    tensor_hash: "deadbeef",
    n_sub: 1,
    terrain: {
      points: [
        [0, 0],
        [100, 0],
      ],
      segments: [{ flow_resistivity: 200, roughness: 0 }],
    },
    atmosphere: { temperature_c: 15, humidity_pct: 70, pressure_kpa: 101.325 },
    coherence: { cv2: 0, ct2: 0, t_air_c: 15, c0: 340.348, roughness_r: 0, f_delta_nu: 1, d_m: 97.5 },
    weather: null,
    sub_sources: [{ position: [2.5, 0, 0.5], directivity: null }],
    receivers: [],
    forest: null,
    forest_path_length_m: null,
    isolation: null,
  };
}

function costWith(level: CostEstimateResult["guardrail_level"]): CostEstimateResult {
  return {
    receiver_count: 1000,
    tensor_bytes: 5_000_000,
    working_set_bytes: 100_000,
    time_estimate_ms: 1234,
    guardrail_level: level,
    guardrail_detail: "halving the final spacing quadruples the cost",
  };
}

beforeEach(() => {
  useCalcStore.getState().reset();
  useCalcStore.setState({
    spacing_fine_m: DEFAULT_FINE_SPACING_M,
    costEstimate: null,
    guardrail: null,
    crossOriginIsolated: false,
    client: null,
  });
});

describe("calc store slice", () => {
  it("defaults the final (fine) spacing to 10 m (D-06)", () => {
    expect(DEFAULT_FINE_SPACING_M).toBe(10);
    expect(useCalcStore.getState().spacing_fine_m).toBe(10);
  });

  it("transitions idle → running → done from forwarded worker JobStatus events", () => {
    const store = useCalcStore.getState();
    expect(store.jobState).toBe("idle");

    const running: JobStatus = { state: "running", progress: 0.5, message: "tier 2 · chunk 4/8" };
    useCalcStore.getState().applyStatus(running);
    expect(useCalcStore.getState().jobState).toBe("running");
    expect(useCalcStore.getState().progress).toBe(0.5);
    expect(useCalcStore.getState().message).toBe("tier 2 · chunk 4/8");

    useCalcStore.getState().applyStatus({ state: "done" });
    expect(useCalcStore.getState().jobState).toBe("done");
    expect(useCalcStore.getState().progress).toBe(1);
  });

  it("records the failure reason on a failed status", () => {
    useCalcStore.getState().applyStatus({ state: "failed", reason: "engine solve failed" });
    expect(useCalcStore.getState().jobState).toBe("failed");
    expect(useCalcStore.getState().failureReason).toBe("engine solve failed");
  });

  it("a guardrail block sets blocked: true and refuses run", () => {
    useCalcStore.getState().setCostEstimate(costWith("block"));
    const g = useCalcStore.getState().guardrail;
    expect(g).not.toBeNull();
    expect(g?.blocked).toBe(true);
    expect(g?.level).toBe("block");

    // run() must be a no-op while blocked (defence-in-depth beside the panel gate).
    const submit = vi.fn();
    useCalcStore.getState().attachClient({ submit, cancel: vi.fn() });
    useCalcStore.getState().run({
      projectId: "p",
      tensorHash: "deadbeef",
      planReq: {
        fine_spacing_m: 10,
        lattice_origin: [0, 0],
        area_min: [0, 0],
        area_max: [10, 10],
        discrete_points: [],
        coarse_multiples: [],
      },
      scene: minimalScene(),
      receiverIds: [],
      nSub: 1,
      chunkReceivers: 256,
    });
    expect(submit).not.toHaveBeenCalled();
    expect(useCalcStore.getState().jobState).toBe("idle");
  });

  it("a non-block estimate leaves the run enabled and submits to the client", () => {
    useCalcStore.getState().setCostEstimate(costWith("warn"));
    expect(useCalcStore.getState().guardrail?.blocked).toBe(false);
    const submit = vi.fn();
    useCalcStore.getState().attachClient({ submit, cancel: vi.fn() });
    useCalcStore.getState().run({
      projectId: "p",
      tensorHash: "deadbeef",
      planReq: {
        fine_spacing_m: 10,
        lattice_origin: [0, 0],
        area_min: [0, 0],
        area_max: [10, 10],
        discrete_points: [],
        coarse_multiples: [],
      },
      scene: minimalScene(),
      receiverIds: [],
      nSub: 1,
      chunkReceivers: 256,
    });
    expect(submit).toHaveBeenCalledOnce();
    expect(useCalcStore.getState().jobState).toBe("queued");
  });

  it("fills per-tier receiver counts from TierComplete events", () => {
    const points: TierComplete = {
      kind: "tier_complete",
      tier: "points",
      tier_index: 0,
      spacing_m: null,
      tensor_hash: "deadbeef",
      receiver_ids: ["r0", "r1"],
      spans: [],
    };
    useCalcStore.getState().applyTierComplete(points);
    expect(useCalcStore.getState().tierCounts.points).toBe(2);
    expect(useCalcStore.getState().tierCounts.fine).toBe(0);
  });

  it("setSpacing updates the spacing and invalidates the stale estimate", () => {
    useCalcStore.getState().setCostEstimate(costWith("ok"));
    expect(useCalcStore.getState().costEstimate).not.toBeNull();
    useCalcStore.getState().setSpacing(5);
    expect(useCalcStore.getState().spacing_fine_m).toBe(5);
    expect(useCalcStore.getState().costEstimate).toBeNull();
    expect(useCalcStore.getState().guardrail).toBeNull();
  });

  it("a cancel delegates to the client (cooperative abort, D-11)", () => {
    const cancel = vi.fn();
    useCalcStore.getState().attachClient({ submit: vi.fn(), cancel });
    useCalcStore.getState().abort();
    expect(cancel).toHaveBeenCalledOnce();
  });

  it("setCapability toggles the cross-origin-isolation flag that gates Run", () => {
    expect(useCalcStore.getState().crossOriginIsolated).toBe(false);
    useCalcStore.getState().setCapability(true);
    expect(useCalcStore.getState().crossOriginIsolated).toBe(true);
  });
});
