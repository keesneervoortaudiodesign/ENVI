// sceneStore.ts — the canonical Zustand scene store (D-03): the single source of truth for the
// scene FeatureCollection and its acoustic properties. Terra Draw is a controlled *view* re-hydrated
// from here; user edits flow back in via `applyTerraDrawChange`.
//
// # Module I/O
// - Input  Terra Draw `change`-event snapshots (user-driven geometry) via `applyTerraDrawChange`;
//   `select`/`markDirty` UI actions; Gate-1 lifecycle diagnostics (`noteDrawBuilt`/`noteDrawStopped`).
// - Output the authoritative `features` (by id) re-added into Terra Draw through `terraDrawFeatures`,
//   plus `selection` and the `dirty` autosave flag. A 105-band isolation spectrum is SCENE data — it
//   lives HERE keyed by feature/edge id (`spectra`), NEVER inside Terra Draw feature properties (D-03
//   anti-pattern). TD properties are for styling/mode only.
// - Valid input range: TD feature ids are UUID strings; `applyTerraDrawChange` treats any id absent
//   from the passed snapshot as a deletion.

import { create } from "zustand";
import type { GeoJSONStoreFeatures } from "terra-draw";

// One authored isolation spectrum lives here, keyed by feature/edge id (D-02/D-06). The skeleton
// only reserves the channel so the "spectrum data lives in the store, never in TD props" invariant is
// established from the first UI plan; the editor + per-edge UUID diffing land in later 07-plans.
export type StoredSpectrum = readonly number[];

export interface SceneState {
  // Canonical geometry: the FeatureCollection keyed by feature id.
  readonly features: Readonly<Record<string, GeoJSONStoreFeatures>>;
  // Isolation spectra by feature/edge id — scene data, never in TD properties (D-03).
  readonly spectra: Readonly<Record<string, StoredSpectrum>>;
  // Current selection (feature id) and the debounced-autosave dirty flag (D-04).
  readonly selection: string | null;
  readonly dirty: boolean;

  // Gate-1 lifecycle diagnostics (07-06 spike): how many Terra Draw instances are currently live and
  // how many have ever been built. Under React StrictMode the effect double-invokes, so `built` may be
  // 2 while `live` must settle at exactly 1 (the instance-in-ref guard). Surfaced to the E2E via the
  // DOM; superseded once the real drawing UI lands in 07-07.
  readonly drawInstancesLive: number;
  readonly drawInstancesBuilt: number;

  // Write user-driven Terra Draw geometry into the store. `snapshot` is TD's FULL current feature list
  // (`draw.getSnapshot()`); every id in `ids` present in the snapshot is upserted, every id absent is a
  // deletion. Marks the scene dirty (autosave trigger is wired to `finish`, not here — D-04).
  applyTerraDrawChange(
    ids: readonly (string | number)[],
    type: string,
    snapshot: readonly GeoJSONStoreFeatures[],
  ): void;
  // The authoritative features to re-add into Terra Draw (e.g. after `style.load` re-hydration).
  terraDrawFeatures(): GeoJSONStoreFeatures[];
  select(id: string | null): void;
  markDirty(): void;
  noteDrawBuilt(): void;
  noteDrawStopped(): void;
}

export const useSceneStore = create<SceneState>((set, get) => ({
  features: {},
  spectra: {},
  selection: null,
  dirty: false,
  drawInstancesLive: 0,
  drawInstancesBuilt: 0,

  applyTerraDrawChange: (ids, _type, snapshot) =>
    set((state) => {
      const byId = new Map(snapshot.map((f) => [String(f.id), f]));
      const features: Record<string, GeoJSONStoreFeatures> = { ...state.features };
      const spectra: Record<string, StoredSpectrum> = { ...state.spectra };
      for (const raw of ids) {
        const id = String(raw);
        const feature = byId.get(id);
        if (feature) {
          features[id] = feature; // create or update
        } else {
          delete features[id]; // deletion: absent from the snapshot
          delete spectra[id];
        }
      }
      return { features, spectra, dirty: true };
    }),

  terraDrawFeatures: () => Object.values(get().features),

  select: (id) => set({ selection: id }),
  markDirty: () => set({ dirty: true }),

  noteDrawBuilt: () =>
    set((state) => ({
      drawInstancesLive: state.drawInstancesLive + 1,
      drawInstancesBuilt: state.drawInstancesBuilt + 1,
    })),
  noteDrawStopped: () =>
    set((state) => ({ drawInstancesLive: Math.max(0, state.drawInstancesLive - 1) })),
}));
