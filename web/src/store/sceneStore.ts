// sceneStore.ts — the canonical Zustand scene store (D-03): the single source of truth for the
// scene FeatureCollection and its acoustic properties. Terra Draw is a controlled *view* re-hydrated
// from here; user edits flow back in via `applyTerraDrawChange` / `commitFeature` / `updateProperties`.
//
// # Module I/O
// - Input  Terra Draw `change`-event snapshots (user-driven geometry) via `applyTerraDrawChange`;
//   newly-drawn or programmatically-placed features via `commitFeature` / `tagCreatedFeature` (which tag
//   `properties.kind` and seed last-object inheritance, WEB-04); inspector edits via `updateProperties`;
//   the active palette tool via `setActiveTool`; Gate-1 lifecycle diagnostics.
// - Output the authoritative `features` (by id) re-added into Terra Draw through `terraDrawFeatures`,
//   the `selection`, the `dirty` autosave flag, the per-feature `inheritedFields` (which non-geometric
//   fields are still "inherited from last {kind}" — cleared on edit), and `sceneFeatureCollection()` /
//   `saveScene()` (whole-scene PUT, D-04). A 105-band isolation spectrum is SCENE data — it lives HERE
//   keyed by feature/edge id (`spectra`), NEVER inside Terra Draw feature properties (D-03 anti-pattern).
// - Valid input range: TD feature ids are UUID strings; `applyTerraDrawChange` treats any id absent from
//   the passed snapshot as a deletion. `commitFeature` requires a kind ∈ the 9 frozen KINDS.

import { create } from "zustand";
import type { GeoJSONStoreFeatures } from "terra-draw";

import type { DrawTool, Kind } from "../draw/kinds";
import { recordLast, seedProps, type KindProps } from "./inheritance";
import { putScene, type SceneCollection } from "../api/client";

// One authored isolation spectrum lives here, keyed by feature/edge id (D-02/D-06). The skeleton only
// reserves the channel so the "spectrum data lives in the store, never in TD props" invariant holds; the
// editor + per-edge UUID diffing land in later 07-plans.
export type StoredSpectrum = readonly number[];

export interface SceneState {
  // Canonical geometry: the FeatureCollection keyed by feature id.
  readonly features: Readonly<Record<string, GeoJSONStoreFeatures>>;
  // Isolation spectra by feature/edge id — scene data, never in TD properties (D-03).
  readonly spectra: Readonly<Record<string, StoredSpectrum>>;
  // Per-feature list of non-geometric field names still seeded from the last object of the kind (WEB-04).
  // Present only while a field is untouched; editing a field removes it so the inspector chip clears.
  readonly inheritedFields: Readonly<Record<string, readonly string[]>>;
  // Current selection (feature id) and the debounced-autosave dirty flag (D-04).
  readonly selection: string | null;
  readonly dirty: boolean;
  // The active palette tool (pointer or a drawing kind). Terra Draw's mode tracks this (07-07); a
  // newly-finished feature is tagged with the active kind's `properties.kind`.
  readonly activeTool: DrawTool;
  // The open project id (whole-scene PUT target). Project open/create lands in 07-10; until then a
  // placeholder key is used so an explicit Save can exercise the PUT path.
  readonly projectId: string | null;

  // Gate-1 lifecycle diagnostics (07-06 spike): live/built Terra Draw instance counts + re-hydrations.
  readonly drawInstancesLive: number;
  readonly drawInstancesBuilt: number;
  readonly rehydrations: number;

  // Write user-driven Terra Draw geometry into the store. `snapshot` is TD's FULL current feature list;
  // every id in `ids` present in the snapshot is upserted, every id absent is a deletion.
  applyTerraDrawChange(
    ids: readonly (string | number)[],
    type: string,
    snapshot: readonly GeoJSONStoreFeatures[],
  ): void;
  // Enrich an already-upserted, newly-created feature: set `properties.kind` + seeded inheritance and
  // select it (the Terra Draw draw path — the geometry is already in `features`).
  tagCreatedFeature(id: string, kind: Kind): void;
  // Place a complete feature (geometry + id) of `kind`: upsert its geometry, tag kind + seed inheritance,
  // select it. The single-call path used by programmatic placement / tests.
  commitFeature(kind: Kind, feature: GeoJSONStoreFeatures): void;
  // Merge a non-geometric property patch into a feature; every patched field clears its "inherited"
  // marker and updates the kind's last-object inheritance source. Marks the scene dirty.
  updateProperties(id: string, patch: KindProps): void;
  // The feature's kind (`properties.kind`) or null if absent/unknown.
  kindOf(id: string): Kind | null;

  // The authoritative features to re-add into Terra Draw (e.g. after `style.load` re-hydration).
  terraDrawFeatures(): GeoJSONStoreFeatures[];
  // The whole-scene GeoJSON FeatureCollection for a PUT.
  sceneFeatureCollection(): SceneCollection;
  // Coalesced whole-scene save (D-04 target; autosave scheduling lands in 07-09). Clears dirty on success.
  saveScene(): Promise<void>;

  setActiveTool(tool: DrawTool): void;
  select(id: string | null): void;
  markDirty(): void;
  noteDrawBuilt(): void;
  noteDrawStopped(): void;
  noteRehydration(): void;
}

// The 9 frozen kinds as a lookup — a `properties.kind` string must be one of these to count as a kind.
const KIND_SET = new Set<string>([
  "source",
  "receiver",
  "wall",
  "building",
  "forest",
  "ground_zone",
  "elevation_point",
  "elevation_line",
  "calc_area",
]);

// Merge kind + seeded inheritance onto an existing feature's properties (shared by the draw path and the
// single-call `commitFeature`). Returns the next `features`/`inheritedFields` maps and the seeded props.
function tagFeature(
  features: Record<string, GeoJSONStoreFeatures>,
  inheritedFields: Record<string, readonly string[]>,
  id: string,
  kind: Kind,
): { seeded: KindProps } {
  const existing = features[id];
  if (!existing) {
    return { seeded: {} };
  }
  const { props, inheritedFields: inherited } = seedProps(kind);
  features[id] = {
    ...existing,
    properties: { ...existing.properties, ...props, kind, id },
  } as GeoJSONStoreFeatures;
  inheritedFields[id] = inherited;
  // This finished object becomes the inheritance source for the next object of the kind.
  recordLast(kind, props);
  return { seeded: props };
}

export const useSceneStore = create<SceneState>((set, get) => ({
  features: {},
  spectra: {},
  inheritedFields: {},
  selection: null,
  dirty: false,
  activeTool: "select",
  projectId: null,
  drawInstancesLive: 0,
  drawInstancesBuilt: 0,
  rehydrations: 0,

  applyTerraDrawChange: (ids, _type, snapshot) =>
    set((state) => {
      const byId = new Map(snapshot.map((f) => [String(f.id), f]));
      const features: Record<string, GeoJSONStoreFeatures> = { ...state.features };
      const spectra: Record<string, StoredSpectrum> = { ...state.spectra };
      const inheritedFields: Record<string, readonly string[]> = { ...state.inheritedFields };
      for (const raw of ids) {
        const id = String(raw);
        const feature = byId.get(id);
        if (feature) {
          features[id] = feature; // create or update
        } else {
          delete features[id]; // deletion: absent from the snapshot
          delete spectra[id];
          delete inheritedFields[id];
        }
      }
      return { features, spectra, inheritedFields, dirty: true };
    }),

  tagCreatedFeature: (id, kind) =>
    set((state) => {
      const features: Record<string, GeoJSONStoreFeatures> = { ...state.features };
      const inheritedFields: Record<string, readonly string[]> = { ...state.inheritedFields };
      tagFeature(features, inheritedFields, id, kind);
      return { features, inheritedFields, selection: id, dirty: true };
    }),

  commitFeature: (kind, feature) =>
    set((state) => {
      const id = String(feature.id);
      const features: Record<string, GeoJSONStoreFeatures> = { ...state.features, [id]: feature };
      const inheritedFields: Record<string, readonly string[]> = { ...state.inheritedFields };
      tagFeature(features, inheritedFields, id, kind);
      return { features, inheritedFields, selection: id, dirty: true };
    }),

  updateProperties: (id, patch) =>
    set((state) => {
      const existing = state.features[id];
      if (!existing) {
        return {};
      }
      const features: Record<string, GeoJSONStoreFeatures> = {
        ...state.features,
        [id]: {
          ...existing,
          properties: { ...existing.properties, ...patch },
        } as GeoJSONStoreFeatures,
      };
      // Clear the "inherited" marker on every edited field (the chip clears on edit, WEB-04).
      const inheritedFields: Record<string, readonly string[]> = { ...state.inheritedFields };
      const prev = inheritedFields[id];
      if (prev) {
        const patched = new Set(Object.keys(patch));
        const remaining = prev.filter((f) => !patched.has(f));
        if (remaining.length > 0) {
          inheritedFields[id] = remaining;
        } else {
          delete inheritedFields[id];
        }
      }
      // Keep the kind's inheritance source current, so the NEXT object inherits the edited values.
      const kind = get().kindOf(id);
      if (kind) {
        const props = features[id].properties as KindProps | null;
        if (props) {
          const { kind: _k, id: _i, ...nonGeom } = props as Record<string, unknown>;
          recordLast(kind, nonGeom);
        }
      }
      return { features, inheritedFields, dirty: true };
    }),

  kindOf: (id) => {
    const props = get().features[id]?.properties as Record<string, unknown> | null | undefined;
    const kind = props?.["kind"];
    return typeof kind === "string" && KIND_SET.has(kind) ? (kind as Kind) : null;
  },

  terraDrawFeatures: () => Object.values(get().features),

  sceneFeatureCollection: () => ({
    type: "FeatureCollection",
    features: Object.values(get().features),
  }),

  saveScene: async () => {
    const projectId = get().projectId ?? "current";
    await putScene(projectId, get().sceneFeatureCollection());
    set({ dirty: false });
  },

  setActiveTool: (tool) => set({ activeTool: tool }),
  select: (id) => set({ selection: id }),
  markDirty: () => set({ dirty: true }),

  noteDrawBuilt: () =>
    set((state) => ({
      drawInstancesLive: state.drawInstancesLive + 1,
      drawInstancesBuilt: state.drawInstancesBuilt + 1,
    })),
  noteDrawStopped: () =>
    set((state) => ({ drawInstancesLive: Math.max(0, state.drawInstancesLive - 1) })),
  noteRehydration: () => set((state) => ({ rehydrations: state.rehydrations + 1 })),
}));
