// stale.test.ts — the D-12 honest-stale contract. Drives the stale store with a MOCK
// IdentityClient (no wasm) and asserts: a divergent re-minted identity sets `isStale`;
// a matching identity clears it; and a CONDITIONING-only edit never touches `isStale`
// (the never-stale invariant, D-07 — the conditioning store performs no re-mint).

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { PrepareSolveReq } from "../generated/wire";
import { useStaleStore, type IdentityClient } from "./stale";
import {
  RECONDITION_DEBOUNCE_MS,
  useConditioningStore,
  type ConditioningClient,
} from "./conditioning";
import { useResultsStore, type ResultsManifest } from "./results";

const CACHED_HASH = "cached-identity";
const scene = { tensor_hash: "", n_sub: 1 } as unknown as PrepareSolveReq;

function manifestFixture(): ResultsManifest {
  return {
    projectId: "proj-1",
    tensorHash: CACHED_HASH,
    scene: { tensor_hash: CACHED_HASH, n_sub: 1 } as ResultsManifest["scene"],
    perSourceConditioning: [{ gain_db: 0, delay_ms: 0, filter_band_db: null, muted: false }],
    receivers: [{ id: "r0", globalIndex: 0, position: [100, 0], chunkIndex: 0, rLocal: 0 }],
    spans: [{ chunkIndex: 0, receiverIds: ["r0"] }],
  };
}

beforeEach(() => {
  useStaleStore.getState().reset();
  useConditioningStore.getState().reset();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("stale store", () => {
  it("sets isStale when the re-minted identity diverges from the cached hash", async () => {
    const client: IdentityClient = { remint: async () => "a-DIFFERENT-identity" };
    useStaleStore.getState().attachIdentityClient(client);

    await useStaleStore.getState().checkStale(scene, CACHED_HASH);
    expect(useStaleStore.getState().isStale).toBe(true);
  });

  it("clears isStale when the re-minted identity matches the cached hash", async () => {
    const client: IdentityClient = { remint: async () => CACHED_HASH };
    useStaleStore.getState().attachIdentityClient(client);
    useStaleStore.setState({ isStale: true });

    await useStaleStore.getState().checkStale(scene, CACHED_HASH);
    expect(useStaleStore.getState().isStale).toBe(false);
  });

  it("a conditioning-only edit NEVER stales (D-07 — no re-mint on the conditioning path)", async () => {
    vi.useFakeTimers();
    // A fresh, matching result — not stale.
    useStaleStore.setState({ isStale: false });
    useResultsStore.getState().reset();
    useResultsStore.getState().setManifest(manifestFixture());
    useConditioningStore.getState().seedFromManifest(manifestFixture().perSourceConditioning);
    const recondition = vi.fn<ConditioningClient["recondition"]>(async () => ({ readouts: {} }));
    useConditioningStore.getState().attachConditioningClient({ recondition });

    // A conditioning edit → debounced MAC. The stale badge must not flip.
    useConditioningStore.getState().setGain("0", 12);
    await vi.advanceTimersByTimeAsync(RECONDITION_DEBOUNCE_MS + 1);

    expect(recondition).toHaveBeenCalledTimes(1);
    expect(useStaleStore.getState().isStale).toBe(false);
  });
});
