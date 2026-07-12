// conditioning.ts — the FLAGSHIP interactive fast-recalc state (WEB-05 / SVC-06,
// SC2). Holds the per-source `ConditioningDto` drive (Gain dB + Delay ms + the
// reused SpectrumEditor Filter, D-11) and a DEBOUNCED (~150 ms, D-10) recondition
// MAC dispatch over the CACHED OPFS tensor (the 11-03 boundary) — spectra + the
// isophone map update live with NO re-propagation.
//
// # Module I/O
// - Input  per-source Gain/Delay/Mute edits + a dense `[105]` filter (materialised
//   by the SERVER from the SpectrumEditor's authored coarse spectrum — D-11), plus
//   the results manifest (the cached tensor identity + scene + receiver→chunk map).
// - Output the `perSource` drive the panel renders, a `refuse` flag (the SVC-06 409
//   reject banner — a MAC against a mismatched tensor hash is never silently served),
//   `pending` (the in-flight debounce), and `recalcEpoch` (bumped on each applied
//   recalc — the offline-UAT "the map/spectra updated live" signal). On a successful
//   MAC it pushes the reconditioned readouts into the results store + re-feeds the
//   isophone grid from the WASM-produced lattice totals.
// - Valid input range: finite gain/delay; a dense `[105]` filter or null; a source
//   id present in the seeded drive.
//
// # D-01 — ZERO acoustic math here
// The MAC runs entirely in WASM (the 11-03 `readout_receivers` boundary): this store
// only marshals the `ConditioningDto` (gain/delay/filter/mute — the SAME wire shape,
// never a forked one) and places WASM-produced numbers into the results + isophone
// stores. There is NO log/pow/exp dB arithmetic in this file (the D-01 grep gate
// asserts it); the dB→complex filter conversion + the readout law live in the engine.
//
// # Conditioning never stales (D-07/D-12)
// A conditioning edit is a READOUT parameter, structurally excluded from the tensor
// identity — it NEVER re-mints the identity and so never trips the stale badge
// (`store/stale.ts`). Only a scene/terrain/ground/met edit can stale a result.

import { create } from "zustand";

import type {
  ConditioningDto,
  ReconditionReq,
  ReceiverReadoutDto,
} from "../generated/wire";
import { readChunk } from "../compute/opfs";
import { readoutReceivers } from "../compute/wasm";
import { useResultsStore, type ResultsManifest } from "./results";
import { useColorScaleStore } from "./colorScale";

// The debounce interval (D-10): ~150 ms coalesces rapid slider / filter / delay edits
// into exactly ONE recondition MAC dispatch so the tensor is not re-read per keystroke.
export const RECONDITION_DEBOUNCE_MS = 150;

// A default (no-op) conditioning drive for one source — reads out identically to the
// plain 11-01 readout (the never-stale invariant, D-07): unit gain, no delay/filter.
export function defaultConditioning(): ConditioningDto {
  return { gain_db: 0, delay_ms: 0, filter_band_db: null, muted: false };
}

// The client seam (injectable → the store is Node-unit-testable without the wasm/OPFS
// graph, mirroring the results `ReadoutClient` seam). The real client reconditions
// each covering OPFS chunk via the `readout_receivers` WASM boundary (hash-gated).
export interface ReconditionDispatch {
  readonly manifest: ResultsManifest;
  readonly perSourceConditioning: readonly ConditioningDto[];
}

export interface ReconditionOutcome {
  // The full reconditioned readout per receiver id (spectrum panel) — WASM-produced.
  readonly readouts: Readonly<Record<string, ReceiverReadoutDto>>;
}

export interface ConditioningClient {
  recondition(dispatch: ReconditionDispatch): Promise<ReconditionOutcome>;
}

// The honest client-side 409 (SVC-06 / D-12): the WASM boundary throws a JS `Error`
// whose message is `tensor_hash mismatch: expected {expected}, got {got}` when the
// re-minted CURRENT identity ≠ the claimed hash. Detect it so the store surfaces the
// reject banner rather than treating it as a generic failure.
export function isHashMismatch(err: unknown): boolean {
  const message = err instanceof Error ? err.message : String(err);
  return message.includes("tensor_hash mismatch");
}

export interface ConditioningState {
  // The per-source drive, keyed by a stable source id (the sub-source index string).
  readonly perSource: Readonly<Record<string, ConditioningDto>>;
  // The source ids in the manifest's sub-source order (the MAC array order).
  readonly order: readonly string[];
  // The SVC-06 409 reject banner: a MAC against a mismatched tensor hash was refused
  // (never silently served). Cleared on the next successful recalc.
  readonly refuse: boolean;
  // A debounce is in flight (the panel shows a subtle in-flight affordance).
  readonly pending: boolean;
  // Bumped on each APPLIED recalc (spectra + map updated) — the offline-UAT live-recalc signal.
  readonly recalcEpoch: number;
  readonly client: ConditioningClient | null;

  attachConditioningClient(client: ConditioningClient): void;
  // Seed the drive from the manifest's per-source conditioning (one entry per sub-source).
  seedFromManifest(perSourceConditioning: readonly ConditioningDto[]): void;
  setGain(sourceId: string, gainDb: number): void;
  setDelay(sourceId: string, delayMs: number): void;
  setMuted(sourceId: string, muted: boolean): void;
  // Set (or clear, when null) the dense `[105]` filter for a source (from the reused
  // SpectrumEditor via the server interpolation — the frontend authors no dB itself).
  setFilter(sourceId: string, filterBandDb: number[] | null): void;
  // Fire the debounced recondition MAC now (called internally on every edit).
  scheduleRecondition(): void;
  reset(): void;
}

// The debounce timer lives at module scope (a single coalescing window across all
// per-source edits) so a burst of Gain+Delay+Filter edits collapses to ONE dispatch.
let debounceTimer: ReturnType<typeof setTimeout> | null = null;

// A monotonic dispatch generation (module scope). Each `runRecondition` stamps the
// generation it fired under; a result is applied ONLY if it is still the newest
// dispatch (CR-01: the real client awaits per-span OPFS reads and can resolve out of
// order — a superseded MAC must not overwrite a newer readout/epoch). `reset()` and a
// manifest swap bump it so any in-flight dispatch is discarded on completion (WR-03).
let reconditionGen = 0;

// Re-feed the isophone map from the reconditioned lattice totals (WASM-produced dB
// per lattice point — NO TS acoustic math). Only when a grid is cached AND its cell
// count matches the reconditioned receiver set (the fixture seeds them 1:1; the full
// production fine-tier lattice feed remains the 11-05/06 follow-up). This re-runs the
// isophone tracer over the new grid (SC3, no re-solve) so the map updates live.
function feedIsophoneFromReadouts(
  manifest: ResultsManifest,
  readouts: Readonly<Record<string, ReceiverReadoutDto>>,
): void {
  const scale = useColorScaleStore.getState();
  if (!scale.grid || !scale.crs) {
    return;
  }
  const ordered = [...manifest.receivers].sort((a, b) => a.globalIndex - b.globalIndex);
  if (ordered.length !== scale.grid.values.length) {
    return;
  }
  const values: number[] = new Array<number>(ordered.length);
  for (let i = 0; i < ordered.length; i += 1) {
    const rd = readouts[ordered[i].id];
    if (!rd) {
      return; // an incomplete batch — leave the map on its last-good grid
    }
    values[i] = rd.total_dba; // a WASM-produced weighted total, placed by index only
  }
  useColorScaleStore.getState().setIsophoneInput(
    { ...scale.grid, values },
    scale.crs,
    scale.weightingLabel,
  );
}

// Patch one field of a source's conditioning drive (seeding a default when the source is
// not yet in the drive), then fire the debounced recalc — the shared body of the four
// per-source setters (the identical `current ?? default` → spread → schedule dance). A
// synchronous `get().perSource` read is equivalent to the functional-updater form here
// (no concurrent `set` runs between within a single edit).
function patchSource(
  get: () => ConditioningState,
  set: (partial: Partial<ConditioningState>) => void,
  sourceId: string,
  patch: Partial<ConditioningDto>,
): void {
  const current = get().perSource[sourceId] ?? defaultConditioning();
  set({ perSource: { ...get().perSource, [sourceId]: { ...current, ...patch } } });
  get().scheduleRecondition();
}

export const useConditioningStore = create<ConditioningState>((set, get) => ({
  perSource: {},
  order: [],
  refuse: false,
  pending: false,
  recalcEpoch: 0,
  client: null,

  attachConditioningClient: (client) => set({ client }),

  seedFromManifest: (perSourceConditioning) => {
    const perSource: Record<string, ConditioningDto> = {};
    const order: string[] = [];
    perSourceConditioning.forEach((c, i) => {
      const id = String(i);
      perSource[id] = { ...c };
      order.push(id);
    });
    set({ perSource, order, refuse: false });
  },

  setGain: (sourceId, gainDb) => patchSource(get, set, sourceId, { gain_db: gainDb }),

  setDelay: (sourceId, delayMs) => patchSource(get, set, sourceId, { delay_ms: delayMs }),

  setMuted: (sourceId, muted) => patchSource(get, set, sourceId, { muted }),

  setFilter: (sourceId, filterBandDb) =>
    patchSource(get, set, sourceId, { filter_band_db: filterBandDb }),

  scheduleRecondition: () => {
    if (debounceTimer !== null) {
      clearTimeout(debounceTimer);
    }
    set({ pending: true });
    debounceTimer = setTimeout(() => {
      debounceTimer = null;
      void runRecondition(get, set);
    }, RECONDITION_DEBOUNCE_MS);
  },

  reset: () => {
    if (debounceTimer !== null) {
      clearTimeout(debounceTimer);
      debounceTimer = null;
    }
    // Invalidate any in-flight dispatch so a MAC that already fired cannot resurrect
    // the epoch or re-apply stale readouts after the reset (WR-03).
    reconditionGen += 1;
    set({ perSource: {}, order: [], refuse: false, pending: false, recalcEpoch: 0 });
  },
}));

// The debounced dispatch body (module-scoped so the timer can call it). Builds the
// ordered `ConditioningDto[]` (the SAME wire shape, never forked), runs the WASM MAC,
// and on success pushes the reconditioned readouts into the results store + re-feeds
// the isophone map; a mismatched hash is refused with the reject banner (SVC-06).
async function runRecondition(
  get: () => ConditioningState,
  set: (partial: Partial<ConditioningState>) => void,
): Promise<void> {
  const gen = (reconditionGen += 1);
  const { client, order, perSource } = get();
  const manifest = useResultsStore.getState().manifest;
  if (!client || !manifest) {
    set({ pending: false });
    return;
  }
  const perSourceConditioning = order.map((id) => perSource[id] ?? defaultConditioning());
  try {
    const { readouts } = await client.recondition({ manifest, perSourceConditioning });
    // Drop a superseded or resurrected result: a newer dispatch (or a reset/new-solve
    // that bumped the generation) has taken over, or the manifest was swapped under us
    // by a fresh solve — applying now would serve a stale spectrum/map as current and
    // double-bump the honest-state epoch (CR-01 / WR-03).
    if (gen !== reconditionGen || useResultsStore.getState().manifest !== manifest) {
      return;
    }
    useResultsStore.getState().applyConditioning(perSourceConditioning, readouts);
    feedIsophoneFromReadouts(manifest, readouts);
    set({ pending: false, refuse: false, recalcEpoch: get().recalcEpoch + 1 });
  } catch (err) {
    // A late failure from a superseded dispatch must not clobber the current flags.
    if (gen !== reconditionGen) {
      return;
    }
    if (isHashMismatch(err)) {
      // The honest client-side 409 (SVC-06 / D-12): refuse, never serve stale spectra.
      set({ pending: false, refuse: true });
    } else {
      set({ pending: false });
    }
  }
}

// The REAL recondition client: reads each covering OPFS chunk and drives the shared
// compute facade's `readout_receivers` MAC (hash-gated; the same module results/cost use —
// COOP/COEP holds `crossOriginIsolated`). The facade's dynamic import keeps the wasm/OPFS
// graph out of the Node unit-test module load (mirrors the results `createWasmReadoutClient`).
export function createWasmConditioningClient(): ConditioningClient {
  return {
    async recondition({ manifest, perSourceConditioning }) {
      const readouts: Record<string, ReceiverReadoutDto> = {};
      for (const span of manifest.spans) {
        const { tensor, pincoh } = await readChunk(
          manifest.projectId,
          manifest.tensorHash,
          span.chunkIndex,
        );
        // Annotate against the generated DTO so a Rust-side field rename is a `tsc`
        // error, not a silent `deny_unknown_fields` failure in the browser (WR-02 / D-10).
        const request: ReconditionReq = {
          tensor_hash: manifest.tensorHash,
          per_source_conditioning: [...perSourceConditioning],
          receiver_ids: [...span.receiverIds],
        };
        // readout_receivers re-mints the identity of `manifest.scene` and refuses a
        // mismatch by THROWING (the message the store detects as the 409). On a match
        // it returns the full two-channel readout per receiver — every dB is WASM.
        const result = await readoutReceivers(manifest.scene, request, tensor, pincoh);
        span.receiverIds.forEach((id, i) => {
          const rd = result.receivers[i];
          if (rd) {
            readouts[id] = rd;
          }
        });
      }
      return { readouts };
    },
  };
}
