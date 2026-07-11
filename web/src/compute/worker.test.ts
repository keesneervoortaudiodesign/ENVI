// worker.test.ts — the compute worker job machine (Vitest, mock wasm + no browser).
// Asserts the tier loop posts queued → running → tier-complete → done, that a
// cancel routes to the cooperative `request_cancel()` flag (NOT worker.terminate),
// and that an un-isolated session yields an honest capability failure (Pitfall 3).

import { describe, expect, it, vi } from "vitest";

import type { PlanTiersReq, TierPlanResult } from "../generated/wire";
import {
  type CalcJobSpec,
  type ComputeWasm,
  type WorkerOutbound,
  createJobMachine,
} from "./worker";

// A two-tier plan (points + one fine tier) with contiguous global indices.
function fakePlan(): TierPlanResult {
  return {
    tiers: [
      {
        kind: "points",
        spacing_m: null,
        receivers: [
          { global_index: 0, position: [5, 5] },
          { global_index: 1, position: [15, 25] },
        ],
      },
      {
        kind: "fine",
        spacing_m: 10,
        receivers: [
          { global_index: 2, position: [0, 0] },
          { global_index: 3, position: [10, 0] },
          { global_index: 4, position: [20, 0] },
        ],
      },
    ],
  };
}

function fakeSpec(overrides: Partial<CalcJobSpec> = {}): CalcJobSpec {
  const planReq: PlanTiersReq = {
    fine_spacing_m: 10,
    lattice_origin: [0, 0],
    area_min: [0, 0],
    area_max: [20, 0],
    discrete_points: [
      [5, 5],
      [15, 25],
    ],
    coarse_multiples: [],
  };
  return {
    projectId: "11111111-1111-1111-1111-111111111111",
    tensorHash: "deadbeef",
    planReq,
    receiverIds: ["r0", "r1", "r2", "r3", "r4"],
    nSub: 2,
    chunkReceivers: 2,
    ...overrides,
  };
}

// A mock wasm whose `plan_tiers` returns the fake plan and whose `solve_chunk_range`
// resolves; `onRange` lets a test observe / interfere between ranges.
function mockWasm(
  onRange?: (req: { chunk_index: number }) => void,
): ComputeWasm & {
  requestCancel: ReturnType<typeof vi.fn>;
  resetCancel: ReturnType<typeof vi.fn>;
} {
  const requestCancel = vi.fn();
  const resetCancel = vi.fn();
  return {
    plan_tiers: () => fakePlan(),
    solve_chunk_range: (req) => {
      onRange?.(req);
      return undefined;
    },
    request_cancel: requestCancel,
    reset_cancel: resetCancel,
    requestCancel,
    resetCancel,
  };
}

function collect(): { msgs: WorkerOutbound[]; post: (m: WorkerOutbound) => void } {
  const msgs: WorkerOutbound[] = [];
  return { msgs, post: (m) => msgs.push(m) };
}

// The status `state` labels in emission order (drops non-status messages).
function states(msgs: WorkerOutbound[]): string[] {
  return msgs.filter((m) => m.type === "status").map((m) => (m as { status: { state: string } }).status.state);
}

describe("compute worker job machine", () => {
  it("posts capability then queued → running → tier-complete → done", async () => {
    const { msgs, post } = collect();
    const wasm = mockWasm();
    const machine = createJobMachine({
      wasm,
      post,
      crossOriginIsolated: true,
      hasSharedArrayBuffer: true,
    });

    await machine.submit(fakeSpec());

    // Capability snapshot is posted at construction.
    expect(msgs[0]).toEqual({ type: "capability", crossOriginIsolated: true });

    // JobStatus vocabulary: queued, running (≥1), then done — no failed/cancelled.
    const seq = states(msgs);
    expect(seq[0]).toBe("queued");
    expect(seq).toContain("running");
    expect(seq[seq.length - 1]).toBe("done");
    expect(seq).not.toContain("failed");
    expect(seq).not.toContain("cancelled");

    // reset_cancel was armed before the run; the pool cancel flag was NOT set.
    expect(wasm.resetCancel).toHaveBeenCalledTimes(1);
    expect(wasm.requestCancel).not.toHaveBeenCalled();

    // A tier-complete event per tier, in order, carrying spans + receiver ids.
    const tiers = msgs.filter((m) => m.type === "tier") as Extract<
      WorkerOutbound,
      { type: "tier" }
    >[];
    expect(tiers.map((t) => t.event.tier)).toEqual(["points", "fine"]);
    expect(tiers[0].event.receiver_ids).toEqual(["r0", "r1"]);
    expect(tiers[0].event.kind).toBe("tier_complete");
    // points(2)/chunk 2 = 1 span; fine(3)/chunk 2 = 2 spans, disjoint chunk indices.
    expect(tiers[0].event.spans).toHaveLength(1);
    expect(tiers[1].event.spans).toHaveLength(2);
    const chunkIndices = tiers.flatMap((t) => t.event.spans.map((s) => s.chunk_index));
    expect(new Set(chunkIndices).size).toBe(chunkIndices.length); // all disjoint
    expect(tiers[1].event.spans[0].tensor_file).toMatch(/^tensor\/chunk_\d+\.bin$/);
  });

  it("cancel routes to request_cancel (cooperative) and lands cancelled", async () => {
    const { msgs, post } = collect();
    // Cancel from inside the first range callback → the next chunk boundary aborts.
    let machineRef: { cancel: () => void } | null = null;
    const wasm = mockWasm(() => machineRef?.cancel());
    const machine = createJobMachine({
      wasm,
      post,
      crossOriginIsolated: true,
      hasSharedArrayBuffer: true,
    });
    machineRef = machine;

    await machine.submit(fakeSpec());

    // Cooperative flag flipped; terminal state is cancelled, never done.
    expect(wasm.requestCancel).toHaveBeenCalled();
    const seq = states(msgs);
    expect(seq[seq.length - 1]).toBe("cancelled");
    expect(seq).not.toContain("done");
  });

  it("refuses honestly when the session is not cross-origin isolated (Pitfall 3)", async () => {
    const { msgs, post } = collect();
    const wasm = mockWasm();
    const machine = createJobMachine({
      wasm,
      post,
      crossOriginIsolated: false,
      hasSharedArrayBuffer: false,
    });

    await machine.submit(fakeSpec());

    // A capability-false snapshot + a failed status carrying the honest reason;
    // the solve never starts (no reset_cancel / plan).
    const caps = msgs.filter((m) => m.type === "capability") as Extract<
      WorkerOutbound,
      { type: "capability" }
    >[];
    expect(caps.some((c) => !c.crossOriginIsolated)).toBe(true);
    const failed = states(msgs);
    expect(failed[failed.length - 1]).toBe("failed");
    expect(wasm.resetCancel).not.toHaveBeenCalled();
  });
});
