// conditioning.test.ts — the WEB-05 / SVC-06 conditioning-drive contract (D-01/D-10/
// D-12). Drives the store with a MOCK ReconditionClient (no wasm, no OPFS) + fake
// timers and asserts: a burst of gain/filter/delay edits DEBOUNCES to exactly ONE MAC
// dispatch (~150 ms, D-10); the dispatched request reuses the `ConditioningDto` wire
// shape (no forked shape); a successful MAC pushes the reconditioned readouts into the
// results store + bumps the recalc epoch; and a mismatched-hash MAC is REFUSED with the
// reject flag and yields NO updated spectra (SVC-06 409, never silently served).

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ReceiverReadoutDto } from "../generated/wire";
import {
  RECONDITION_DEBOUNCE_MS,
  useConditioningStore,
  type ConditioningClient,
  type ReconditionDispatch,
} from "./conditioning";
import { useResultsStore, type ResultsManifest } from "./results";

const N_BANDS = 105;

function readoutFixture(total: number): ReceiverReadoutDto {
  const band = Array.from({ length: N_BANDS }, (_, i) => 40 + (i % 20));
  return {
    band_levels_db: band,
    coherent_db: band.map((v) => v - 1),
    incoherent_db: band.map((v) => v - 10),
    total_dba: total,
    total_dbc: total + 3,
    total_coherent_db: total - 1,
    total_incoherent_db: total - 10,
  };
}

function manifestFixture(): ResultsManifest {
  return {
    projectId: "proj-1",
    tensorHash: "abc123",
    scene: { tensor_hash: "abc123", n_sub: 2 } as ResultsManifest["scene"],
    perSourceConditioning: [
      { gain_db: 0, delay_ms: 0, filter_band_db: null, muted: false },
      { gain_db: 0, delay_ms: 0, filter_band_db: null, muted: false },
    ],
    receivers: [{ id: "r0", globalIndex: 0, position: [100, 0], chunkIndex: 0, rLocal: 0 }],
    spans: [{ chunkIndex: 0, receiverIds: ["r0"] }],
  };
}

beforeEach(() => {
  vi.useFakeTimers();
  useConditioningStore.getState().reset();
  useResultsStore.getState().reset();
  useResultsStore.getState().setManifest(manifestFixture());
  useConditioningStore.getState().seedFromManifest(manifestFixture().perSourceConditioning);
});

afterEach(() => {
  vi.useRealTimers();
});

describe("conditioning store", () => {
  it("debounces a burst of gain/filter/delay edits into exactly ONE MAC dispatch (~150 ms)", async () => {
    const recondition = vi.fn(
      async (_d: ReconditionDispatch) => ({ readouts: { r0: readoutFixture(61) } }),
    );
    const client: ConditioningClient = { recondition };
    useConditioningStore.getState().attachConditioningClient(client);

    // A rapid burst across gain + delay + filter on both sources.
    useConditioningStore.getState().setGain("0", 6);
    useConditioningStore.getState().setDelay("0", 2.5);
    useConditioningStore.getState().setFilter("1", Array.from({ length: N_BANDS }, () => -1.5));
    useConditioningStore.getState().setGain("1", -3);

    // Nothing dispatched until the debounce window elapses.
    expect(recondition).not.toHaveBeenCalled();
    expect(useConditioningStore.getState().pending).toBe(true);

    await vi.advanceTimersByTimeAsync(RECONDITION_DEBOUNCE_MS + 1);
    expect(recondition).toHaveBeenCalledTimes(1);
  });

  it("dispatches the ConditioningDto wire shape (no forked shape) and applies the readout live", async () => {
    let captured: ReconditionDispatch | null = null;
    const recondition = vi.fn(async (d: ReconditionDispatch) => {
      captured = d;
      return { readouts: { r0: readoutFixture(72) } };
    });
    useConditioningStore.getState().attachConditioningClient({ recondition });

    useConditioningStore.getState().setGain("0", 9);
    await vi.advanceTimersByTimeAsync(RECONDITION_DEBOUNCE_MS + 1);

    expect(captured).not.toBeNull();
    const dispatch = captured as unknown as ReconditionDispatch;
    // One entry per sub-source, each a ConditioningDto (gain/delay/filter/mute keys).
    expect(dispatch.perSourceConditioning).toHaveLength(2);
    const first = dispatch.perSourceConditioning[0];
    expect(first).toHaveProperty("gain_db", 9);
    expect(first).toHaveProperty("delay_ms");
    expect(first).toHaveProperty("filter_band_db");
    expect(first).toHaveProperty("muted");
    // The reconditioned readout landed in the results store + the recalc epoch bumped.
    expect(useResultsStore.getState().readouts["r0"]?.total_dba).toBe(72);
    expect(useConditioningStore.getState().recalcEpoch).toBe(1);
    expect(useConditioningStore.getState().refuse).toBe(false);
  });

  it("refuses a mismatched-hash MAC with the reject flag and yields NO updated spectra (SVC-06)", async () => {
    const recondition = vi.fn(async () => {
      throw new Error("tensor_hash mismatch: expected NEW, got abc123");
    });
    useConditioningStore.getState().attachConditioningClient({ recondition });

    useConditioningStore.getState().setGain("0", 4);
    await vi.advanceTimersByTimeAsync(RECONDITION_DEBOUNCE_MS + 1);

    expect(useConditioningStore.getState().refuse).toBe(true);
    expect(useConditioningStore.getState().pending).toBe(false);
    // No readout was applied — the stale MAC was never served.
    expect(useResultsStore.getState().readouts["r0"]).toBeUndefined();
    expect(useConditioningStore.getState().recalcEpoch).toBe(0);
  });
});
