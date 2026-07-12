// results.ts — the client-side RESULTS state slice (WEB-11 spectrum panel + the
// shell the isophone map / conditioning / scenarios / export later slot into).
// Sibling of `store/calc.ts`: the calc slice OWNS the solve lifecycle; this slice
// OWNS what the user inspects afterwards — the selected receiver, the display /
// weighting / split toggles, and the per-receiver readouts CACHED from the WASM
// readout core over the OPFS tensor.
//
// # Module I/O
// - Input  a `ResultsManifest` (`setManifest`, from the calc feed: the tensor
//   identity + scene + per-source conditioning + the receiver→chunk map), the
//   user's receiver selection (`selectReceiver`, from a map click OR the synced
//   list — one entry point), and the display/weighting/split toggles.
// - Output the state the `SpectrumPanel` renders: `selectedReceiverId`,
//   `displayMode` (third | twelfth), `weighting` (A | C), `showSplit`, the cached
//   `readouts` (one `ReceiverReadoutDto` per receiver id), plus loading/error.
// - Valid input range: a receiver id present in the manifest; toggles are their
//   small enums.
//
// # D-01 — ZERO acoustic math here
// EVERY acoustic value (band levels, dB(A)/dB(C) totals, the coherent/incoherent
// split) is WASM-produced by `readout_receivers` and cached verbatim. This slice
// only selects, caches, and toggles: a display/weighting/split toggle mutates
// state and triggers NO worker round-trip and NO recompute (the weightings + split
// are precomputed in the readout, D-09). There is no log/pow/exp dB arithmetic in
// this file — the D-01 grep gate (no such Math calls) asserts it.

import { create } from "zustand";

import type {
  ConditioningDto,
  PrepareSolveReq,
  ReadoutResult,
  ReceiverReadoutDto,
} from "../generated/wire";
import { readChunk } from "../compute/opfs";

// The band-level display resolution: 1/3-octave (default, the 27 third-octave
// band indices) ⇄ the full 105-point 1/12-octave expert view (D-06). Aggregation
// is by BAND INDEX (the panel picks the third-octave indices), never nominal Hz.
export type DisplayMode = "third" | "twelfth";

// The frequency weighting whose TOTAL is shown; both are precomputed in the
// readout so the toggle is instant (D-09).
export type Weighting = "A" | "C";

// One receiver's placement + where its column lives in the OPFS tensor.
export interface ReceiverRef {
  readonly id: string;
  readonly globalIndex: number;
  // Scene-XY position (meters) for the synced receiver list + mini-map markers.
  readonly position: readonly [number, number];
  // Which OPFS chunk file holds this receiver, and its local column within it.
  readonly chunkIndex: number;
  readonly rLocal: number;
}

// One OPFS chunk's receiver ids in receiver-major (column) order — the
// `receiver_ids` the readout request carries so the WASM aligns columns 1:1.
export interface ChunkSpanRef {
  readonly chunkIndex: number;
  readonly receiverIds: readonly string[];
}

// The results feed: everything the readout needs, keyed by the frozen tensor
// identity (D-09). `scene` re-mints the identity for the WASM hash gate; the
// per-source conditioning is the readout drive.
export interface ResultsManifest {
  readonly projectId: string;
  readonly tensorHash: string;
  readonly scene: PrepareSolveReq;
  readonly perSourceConditioning: readonly ConditioningDto[];
  readonly receivers: readonly ReceiverRef[];
  readonly spans: readonly ChunkSpanRef[];
}

// One readout request the client fulfils (reads the covering chunk, calls WASM).
export interface ReadoutRequest {
  readonly projectId: string;
  readonly tensorHash: string;
  readonly scene: PrepareSolveReq;
  readonly perSourceConditioning: readonly ConditioningDto[];
  readonly chunkIndex: number;
  readonly rLocal: number;
  readonly chunkReceiverIds: readonly string[];
}

// The readout collaborator (injectable so the store is unit-testable in Node
// without the wasm module — mirrors calc's `CalcClient` seam).
export interface ReadoutClient {
  readout(req: ReadoutRequest): Promise<ReceiverReadoutDto>;
}

export interface ResultsState {
  readonly manifest: ResultsManifest | null;
  readonly selectedReceiverId: string | null;
  readonly displayMode: DisplayMode;
  readonly weighting: Weighting;
  readonly showSplit: boolean;
  readonly readouts: Readonly<Record<string, ReceiverReadoutDto>>;
  readonly loadingReceiverId: string | null;
  readonly readoutError: string | null;
  readonly client: ReadoutClient | null;

  attachReadoutClient(client: ReadoutClient): void;
  setManifest(manifest: ResultsManifest): void;
  selectReceiver(id: string | null): void;
  setDisplayMode(mode: DisplayMode): void;
  setWeighting(weighting: Weighting): void;
  toggleSplit(): void;
  reset(): void;
}

export const useResultsStore = create<ResultsState>((set, get) => ({
  manifest: null,
  selectedReceiverId: null,
  displayMode: "third",
  weighting: "A",
  showSplit: false,
  readouts: {},
  loadingReceiverId: null,
  readoutError: null,
  client: null,

  attachReadoutClient: (client) => set({ client }),

  // A fresh manifest supersedes the prior results (a new solve): drop the cached
  // readouts + selection so a stale receiver spectrum is never shown against a new
  // tensor identity.
  setManifest: (manifest) =>
    set({
      manifest,
      readouts: {},
      selectedReceiverId: null,
      loadingReceiverId: null,
      readoutError: null,
    }),

  // The SINGLE selection entry point — a map-marker click AND the synced list both
  // call this. If the receiver's readout is not cached yet, fetch it (read the
  // covering OPFS chunk + call the WASM readout core); the toggles never refetch.
  selectReceiver: (id) => {
    set({ selectedReceiverId: id, readoutError: null });
    if (id === null) {
      return;
    }
    const { manifest, readouts, client } = get();
    if (!manifest || !client || readouts[id]) {
      return;
    }
    const rref = manifest.receivers.find((r) => r.id === id);
    if (!rref) {
      return;
    }
    const span = manifest.spans.find((s) => s.chunkIndex === rref.chunkIndex);
    if (!span) {
      return;
    }
    set({ loadingReceiverId: id });
    client
      .readout({
        projectId: manifest.projectId,
        tensorHash: manifest.tensorHash,
        scene: manifest.scene,
        perSourceConditioning: manifest.perSourceConditioning,
        chunkIndex: rref.chunkIndex,
        rLocal: rref.rLocal,
        chunkReceiverIds: span.receiverIds,
      })
      .then((readout) =>
        set((s) => ({
          readouts: { ...s.readouts, [id]: readout },
          loadingReceiverId: s.loadingReceiverId === id ? null : s.loadingReceiverId,
        })),
      )
      .catch((err: unknown) =>
        set((s) => ({
          readoutError: err instanceof Error ? err.message : String(err),
          loadingReceiverId: s.loadingReceiverId === id ? null : s.loadingReceiverId,
        })),
      );
  },

  // Toggles: pure state mutations — NO readout call, NO acoustic math (D-09/D-01).
  setDisplayMode: (displayMode) => set({ displayMode }),
  setWeighting: (weighting) => set({ weighting }),
  toggleSplit: () => set((s) => ({ showSplit: !s.showSplit })),

  reset: () =>
    set({
      manifest: null,
      selectedReceiverId: null,
      readouts: {},
      loadingReceiverId: null,
      readoutError: null,
    }),
}));

// The REAL readout client: lazily initialises the compute wasm module on the main
// thread (the same module CalcPanel already uses for cost/plan — the dev/prod
// server sends COOP/COEP so `crossOriginIsolated` holds), reads the covering OPFS
// chunk bytes, and calls the `readout_receivers` export. The wasm is a DYNAMIC
// import inside the factory so importing this store in Node (the vitest unit test)
// never pulls the browser-only wasm/OPFS graph.
export function createWasmReadoutClient(): ReadoutClient {
  let glue: Promise<typeof import("../generated/wasm-compute/envi_compute_wasm")> | null = null;
  const ensureGlue = (): Promise<typeof import("../generated/wasm-compute/envi_compute_wasm")> => {
    glue ??= (async () => {
      const g = await import("../generated/wasm-compute/envi_compute_wasm");
      await g.default();
      return g;
    })();
    return glue;
  };
  return {
    async readout(req) {
      const g = await ensureGlue();
      const { tensor, pincoh } = await readChunk(req.projectId, req.tensorHash, req.chunkIndex);
      const request = {
        tensor_hash: req.tensorHash,
        per_source_conditioning: req.perSourceConditioning,
        receiver_ids: req.chunkReceiverIds,
      };
      const result = g.readout_receivers(req.scene, request, tensor, pincoh) as ReadoutResult;
      const readout = result.receivers[req.rLocal];
      if (!readout) {
        throw new Error("result data unavailable for the selected receiver");
      }
      return readout;
    },
  };
}
