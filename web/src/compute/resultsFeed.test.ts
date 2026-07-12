// resultsFeed.test.ts — the production calc→results feed assembly (Vitest, Node; no
// browser/worker/wasm). Proves `buildResultsManifest` maps a finished FINE tier's
// spans onto receiver refs + chunk spans correctly, and that `applyResultsFeed`
// pushes it into the results store (and no-ops for a non-fine tier).

import { beforeEach, describe, expect, it } from "vitest";

import type { PrepareSolveReq, TierComplete } from "../generated/wire";
import type { CalcJobSpec } from "./worker";
import { applyResultsFeed, buildResultsManifest } from "./resultsFeed";
import { useResultsStore } from "../store/results";

function spec(): CalcJobSpec {
  const scene = {
    tensor_hash: "hash-abc",
    n_sub: 2,
    receivers: [
      { global_index: 0, position: [100, 0, 1.5] },
      { global_index: 1, position: [110, 0, 1.5] },
      { global_index: 2, position: [120, 0, 1.5] },
      { global_index: 3, position: [130, 0, 1.5] },
    ],
  } as unknown as PrepareSolveReq;
  return {
    projectId: "proj-1",
    tensorHash: "hash-abc",
    planReq: {} as CalcJobSpec["planReq"],
    scene,
    receiverIds: ["r0", "r1", "r2", "r3"],
    nSub: 2,
    chunkReceivers: 2,
  };
}

function fineEvent(): TierComplete {
  return {
    kind: "tier_complete",
    tier: "fine",
    tier_index: 2,
    spacing_m: 10,
    tensor_hash: "hash-abc",
    receiver_ids: ["r0", "r1", "r2", "r3"],
    spans: [
      { chunk_index: 4, r_offset: 0, len: 2, tensor_file: "t/4", pincoh_file: "p/4" },
      { chunk_index: 5, r_offset: 2, len: 2, tensor_file: "t/5", pincoh_file: "p/5" },
    ],
  } as unknown as TierComplete;
}

describe("buildResultsManifest", () => {
  it("maps the fine spans onto receiver refs (chunk/rLocal/position) and chunk spans", () => {
    const m = buildResultsManifest(spec(), fineEvent());
    expect(m.projectId).toBe("proj-1");
    expect(m.tensorHash).toBe("hash-abc");
    expect(m.perSourceConditioning).toHaveLength(2);
    expect(m.receivers).toHaveLength(4);

    // Second span's first receiver: global 2 → chunk 5, rLocal 0, its scene position.
    const r2 = m.receivers.find((r) => r.id === "r2");
    expect(r2).toBeDefined();
    expect(r2).toMatchObject({ globalIndex: 2, chunkIndex: 5, rLocal: 0, position: [120, 0] });

    // The chunk spans carry the per-chunk receiver-id order (column alignment).
    expect(m.spans).toEqual([
      { chunkIndex: 4, receiverIds: ["r0", "r1"] },
      { chunkIndex: 5, receiverIds: ["r2", "r3"] },
    ]);
  });
});

describe("applyResultsFeed", () => {
  beforeEach(() => useResultsStore.getState().reset());

  it("pushes the manifest into the results store on a fine tier", () => {
    applyResultsFeed(spec(), fineEvent());
    const m = useResultsStore.getState().manifest;
    expect(m).not.toBeNull();
    expect(m?.tensorHash).toBe("hash-abc");
    expect(m?.receivers).toHaveLength(4);
    expect(useResultsStore.getState().client).not.toBeNull();
  });

  it("no-ops for a non-fine tier", () => {
    const coarse = { ...fineEvent(), tier: "coarse" } as unknown as TierComplete;
    applyResultsFeed(spec(), coarse);
    expect(useResultsStore.getState().manifest).toBeNull();
  });
});
