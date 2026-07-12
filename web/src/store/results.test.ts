// results.test.ts — the WEB-11 results-slice contract (D-01 / D-09). Drives the
// store with a MOCK ReadoutClient (no wasm, no OPFS) and asserts: a selection
// fetches + caches a readout ONCE; the display/weighting/split toggles mutate
// state and NEVER invoke the readout client (no recompute, D-09); a fresh
// manifest supersedes the cache; a readout failure surfaces an honest error.

import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ReceiverReadoutDto } from "../generated/wire";
import {
  useResultsStore,
  type ReadoutClient,
  type ResultsManifest,
} from "./results";

const N_BANDS = 105;

function readoutFixture(seed: number): ReceiverReadoutDto {
  const band = Array.from({ length: N_BANDS }, (_, i) => 40 + ((i + seed) % 20));
  return {
    band_levels_db: band,
    coherent_db: band.map((v) => v - 1),
    incoherent_db: band.map((v) => v - 10),
    total_dba: 60 + seed,
    total_dbc: 63 + seed,
    total_coherent_db: 59 + seed,
    total_incoherent_db: 50 + seed,
  };
}

function manifestFixture(): ResultsManifest {
  return {
    projectId: "proj-1",
    tensorHash: "abc123",
    scene: { tensor_hash: "abc123", n_sub: 1 } as ResultsManifest["scene"],
    perSourceConditioning: [{ gain_db: 80, delay_ms: 0, filter_band_db: null, muted: false }],
    receivers: [
      { id: "r0", globalIndex: 0, position: [100, 0], chunkIndex: 0, rLocal: 0 },
      { id: "r1", globalIndex: 1, position: [110, 0], chunkIndex: 0, rLocal: 1 },
    ],
    spans: [{ chunkIndex: 0, receiverIds: ["r0", "r1"] }],
  };
}

// Flush the pending microtasks the async selectReceiver readout promise chains on.
const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  useResultsStore.getState().reset();
  useResultsStore.setState({
    client: null,
    displayMode: "third",
    weighting: "A",
    showSplit: false,
  });
});

describe("results store", () => {
  it("fetches + caches a readout on selection, and does not refetch a cached receiver", async () => {
    const readout = vi.fn(async () => readoutFixture(1));
    const client: ReadoutClient = { readout };
    useResultsStore.getState().attachReadoutClient(client);
    useResultsStore.getState().setManifest(manifestFixture());

    useResultsStore.getState().selectReceiver("r0");
    await flush();
    expect(readout).toHaveBeenCalledTimes(1);
    expect(useResultsStore.getState().readouts["r0"]?.total_dba).toBe(61);
    expect(useResultsStore.getState().loadingReceiverId).toBeNull();

    // Re-selecting a cached receiver never calls the client again.
    useResultsStore.getState().selectReceiver("r1");
    await flush();
    useResultsStore.getState().selectReceiver("r0");
    await flush();
    expect(readout).toHaveBeenCalledTimes(2); // r0 + r1, not a third for r0
  });

  it("display/weighting/split toggles mutate state with NO readout call (D-09, no recompute)", async () => {
    const readout = vi.fn(async () => readoutFixture(1));
    useResultsStore.getState().attachReadoutClient({ readout });
    useResultsStore.getState().setManifest(manifestFixture());
    useResultsStore.getState().selectReceiver("r0");
    await flush();
    readout.mockClear();

    useResultsStore.getState().setDisplayMode("twelfth");
    useResultsStore.getState().setWeighting("C");
    useResultsStore.getState().toggleSplit();

    expect(useResultsStore.getState().displayMode).toBe("twelfth");
    expect(useResultsStore.getState().weighting).toBe("C");
    expect(useResultsStore.getState().showSplit).toBe(true);
    // The toggles are pure re-render — the readout client was never touched.
    expect(readout).not.toHaveBeenCalled();
  });

  it("a fresh manifest supersedes the cached readouts + selection", async () => {
    const readout = vi.fn(async () => readoutFixture(1));
    useResultsStore.getState().attachReadoutClient({ readout });
    useResultsStore.getState().setManifest(manifestFixture());
    useResultsStore.getState().selectReceiver("r0");
    await flush();
    expect(Object.keys(useResultsStore.getState().readouts)).toHaveLength(1);

    useResultsStore.getState().setManifest({ ...manifestFixture(), tensorHash: "def456" });
    expect(useResultsStore.getState().readouts).toEqual({});
    expect(useResultsStore.getState().selectedReceiverId).toBeNull();
  });

  it("surfaces an honest error when the readout fails", async () => {
    const readout = vi.fn(async () => {
      throw new Error("result data unavailable");
    });
    useResultsStore.getState().attachReadoutClient({ readout });
    useResultsStore.getState().setManifest(manifestFixture());
    useResultsStore.getState().selectReceiver("r0");
    await flush();
    expect(useResultsStore.getState().readoutError).toBe("result data unavailable");
    expect(useResultsStore.getState().loadingReceiverId).toBeNull();
  });
});
