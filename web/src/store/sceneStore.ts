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
import { initEdgeIds, ringDiff, type Coord } from "./edges";
import { classifyGroundZone, type GroundZoneOutcome } from "../validate/groundZone";
import type { AuthoredSpectrumDto } from "../generated/wire";

// One authored isolation spectrum lives here, keyed by feature/edge id (D-02/D-06): a building's
// `default_isolation` under its FEATURE id, each per-façade override under its EDGE UUID, a wall/screen or
// source L_W under its feature id. Only the AUTHORED coarse representation is stored — the dense `r_db[105]`
// is DERIVED on read via the server (`POST /meta/interpolate-spectrum`), never a second persisted field
// (D-06). A 105-band spectrum is scene data; it lives HERE, never in Terra Draw feature properties (D-03).
export type StoredSpectrum = AuthoredSpectrumDto;

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
  // A monotonically-increasing COMMITTED-edit counter (D-04): bumped ONLY on committed mutations — a
  // finished shape (`tagCreatedFeature`/`commitFeature`), a released vertex drag (`noteCommit` from the
  // Terra Draw `finish` handler), a property change, a spectrum edit, or an accepted ground_zone. It is
  // NEVER bumped on the raw `applyTerraDrawChange` (change/drag) path, so autosave keys off THIS (not the
  // dirty flag) and can never fire per drag frame.
  readonly commitEpoch: number;
  // The active palette tool (pointer or a drawing kind). Terra Draw's mode tracks this (07-07); a
  // newly-finished feature is tagged with the active kind's `properties.kind`.
  readonly activeTool: DrawTool;
  // The open project id + display name (whole-scene PUT target / delete confirmation). Project
  // open/create lands in 07-10; until then a placeholder key is used so Save can exercise the PUT path.
  readonly projectId: string | null;
  readonly projectName: string | null;

  // The transient ground-zone hard-reject signal (D-07, Surface B): a partial-cross reverts the geometry
  // (never commits) and raises this carrying the id of the EXISTING crossed zone. `nonce` re-triggers the
  // banner even when the same zone is crossed twice. Null when there is no active reject.
  readonly groundReject: { readonly conflictId: string; readonly nonce: number } | null;
  // A zoom-to-fit request (the map ZoomController fits the feature's bounds). `nonce` re-triggers a zoom
  // to the same feature (e.g. clicking the same validation row twice). Null when nothing is pending.
  readonly zoomRequest: { readonly featureId: string; readonly nonce: number } | null;

  // Which spectrum (feature or edge UUID) the isolation/L_W editor is open for, plus its display title.
  // Null when the editor is closed. Opened from the source / wall / façade "Edit spectrum" triggers.
  readonly spectrumEditor: { readonly key: string; readonly title: string } | null;

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
  // Classify a just-upserted (still-untagged) ground_zone candidate against the existing zones (D-07) and
  // either COMMIT it (`ok`/`contained` — tag + select + clear any reject) or HARD-REJECT it
  // (`partial-cross` — remove the candidate geometry from the store so it never commits, and raise the
  // transient `groundReject` referencing the crossed zone). Returns the outcome so the Terra Draw path can
  // additionally remove the reverted feature from its own view.
  commitGroundZoneCandidate(id: string): GroundZoneOutcome;
  // Dismiss the transient ground-zone reject banner (auto-dismiss timeout / next action).
  dismissGroundReject(): void;
  // Request a zoom-to-fit of a feature's geometry (validation click-to-zoom + reject "zoom to conflict").
  zoomToFeature(id: string): void;
  // Select a feature AND zoom-to-fit it (a validation-panel row click: select + zoom + open inspector).
  selectAndZoom(id: string): void;
  // Note a committed edit that carries no property/kind change of its own (a released vertex drag): bump
  // the commit epoch + mark dirty so autosave schedules (called from the Terra Draw `finish` handler).
  noteCommit(): void;
  // Set the open project identity (id + display name) — the delete-confirmation gate compares against the
  // name. Clears dirty. (07-10 owns real open/create; this is the seam the delete dialog + tests use.)
  setProject(id: string, name: string): void;
  // Route to the empty/no-project state after a delete: clear the scene + project identity + selection.
  resetProject(): void;
  // Merge a non-geometric property patch into a feature; every patched field clears its "inherited"
  // marker and updates the kind's last-object inheritance source. Marks the scene dirty.
  updateProperties(id: string, patch: KindProps): void;
  // Set (or clear, when `authored` is null) the AUTHORED isolation/L_W spectrum for a feature or edge id
  // (D-06 — only the authored coarse form is stored; the dense grid is derived server-side). Marks dirty.
  setSpectrum(key: string, authored: AuthoredSpectrumDto | null): void;
  // Open / close the isolation-spectrum editor overlay for a feature or edge UUID (WEB-10).
  openSpectrumEditor(key: string, title: string): void;
  closeSpectrumEditor(): void;
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

// The ordered DISTINCT footprint vertices of a polygon feature (the outer ring minus the closing
// duplicate), or null if the feature is not a usable polygon. This is the ring the D-02 ring-diff operates
// on: `n` vertices ⇒ `n` edges (including the wrap edge).
function ringOf(feature: GeoJSONStoreFeatures | undefined): Coord[] | null {
  const geometry = feature?.geometry;
  if (!geometry || geometry.type !== "Polygon") {
    return null;
  }
  const outer = geometry.coordinates[0];
  if (!Array.isArray(outer) || outer.length < 4) {
    return null; // a valid closed ring needs ≥3 distinct vertices + the closing duplicate
  }
  // Drop the closing duplicate vertex if present.
  const last = outer.length - 1;
  const closed = sameXY(outer[0], outer[last]);
  const verts = closed ? outer.slice(0, last) : outer.slice();
  return verts.map((c) => [c[0], c[1]] as Coord);
}

function sameXY(a: readonly number[], b: readonly number[]): boolean {
  return a[0] === b[0] && a[1] === b[1];
}

// The building's per-edge UUID list from `properties.edge_ids`, or null if absent/malformed.
function edgeIdsOf(feature: GeoJSONStoreFeatures | undefined): string[] | null {
  const raw = feature?.properties?.["edge_ids"];
  if (Array.isArray(raw) && raw.every((x) => typeof x === "string")) {
    return raw as string[];
  }
  return null;
}

// Reconcile a building's per-edge UUIDs + per-façade spectra when its footprint geometry changes (D-02,
// RESEARCH Pattern 5). Mutates `spectra` in place (drop/rekey façade overrides by UUID) and returns the
// next feature with refreshed `properties.edge_ids`. Returns `nextFeature` unchanged when the feature is
// not a building or the rings are unusable. This is the SOLE call site of `ringDiff` (grep gate: the
// ring-diff runs in the store's applyTerraDrawChange, never in a Terra Draw callback).
function reconcileBuildingEdges(
  prevFeature: GeoJSONStoreFeatures | undefined,
  nextFeature: GeoJSONStoreFeatures,
  spectra: Record<string, StoredSpectrum>,
): GeoJSONStoreFeatures {
  if (prevFeature?.properties?.["kind"] !== "building") {
    return nextFeature;
  }
  const prevRing = ringOf(prevFeature);
  const nextRing = ringOf(nextFeature);
  if (!prevRing || !nextRing) {
    return nextFeature;
  }
  const prevEdgeIds = edgeIdsOf(prevFeature) ?? initEdgeIds(prevRing);
  if (prevEdgeIds.length !== prevRing.length) {
    return nextFeature; // base is inconsistent — leave geometry, don't risk a bad re-point
  }

  const diff = ringDiff(prevRing, prevEdgeIds, nextRing);

  // Reconcile only THIS building's per-edge spectra (keyed by its prev edge UUIDs) through the diff's
  // mapping instruction, then splice the result back into the shared spectra channel.
  const facadeSubset: Record<string, StoredSpectrum> = {};
  for (const edgeId of prevEdgeIds) {
    if (Object.prototype.hasOwnProperty.call(spectra, edgeId)) {
      facadeSubset[edgeId] = spectra[edgeId];
      delete spectra[edgeId];
    }
  }
  const reconciled = diff.reconcileFacade(facadeSubset);
  for (const [edgeId, authored] of Object.entries(reconciled)) {
    spectra[edgeId] = authored;
  }

  return {
    ...nextFeature,
    properties: { ...nextFeature.properties, edge_ids: diff.edgeIds },
  } as GeoJSONStoreFeatures;
}

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
  // A building's per-façade isolation is keyed by stable per-edge UUIDs (D-02): mint one per footprint
  // edge at draw time so the ring-diff has a base to reconcile against on the first geometry edit.
  const edgeProps: { edge_ids?: string[] } = {};
  if (kind === "building") {
    const ring = ringOf(existing);
    if (ring) {
      edgeProps.edge_ids = initEdgeIds(ring);
    }
  }
  features[id] = {
    ...existing,
    properties: { ...existing.properties, ...props, ...edgeProps, kind, id },
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
  commitEpoch: 0,
  activeTool: "select",
  projectId: null,
  projectName: null,
  groundReject: null,
  zoomRequest: null,
  spectrumEditor: null,
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
          // On a geometry update of an existing building, run the D-02 ring-diff so per-edge UUIDs +
          // per-façade spectra reconcile (a vertex insert must NOT re-point an existing façade spectrum).
          const prevFeature = state.features[id];
          features[id] = reconcileBuildingEdges(prevFeature, feature, spectra); // create or update
        } else {
          // Deletion: absent from the snapshot. Drop the feature's own spectrum + every per-edge override.
          const prevFeature = state.features[id];
          for (const edgeId of edgeIdsOf(prevFeature) ?? []) {
            delete spectra[edgeId];
          }
          delete features[id];
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
      return { features, inheritedFields, selection: id, dirty: true, commitEpoch: state.commitEpoch + 1 };
    }),

  commitFeature: (kind, feature) =>
    set((state) => {
      const id = String(feature.id);
      const features: Record<string, GeoJSONStoreFeatures> = { ...state.features, [id]: feature };
      const inheritedFields: Record<string, readonly string[]> = { ...state.inheritedFields };
      tagFeature(features, inheritedFields, id, kind);
      return { features, inheritedFields, selection: id, dirty: true, commitEpoch: state.commitEpoch + 1 };
    }),

  commitGroundZoneCandidate: (id) => {
    const state = get();
    const candidate = state.features[id];
    if (!candidate) {
      return "ok";
    }
    // The existing zones to test against: every OTHER feature already tagged `ground_zone`.
    const existingZones = Object.entries(state.features)
      .filter(([fid, f]) => fid !== id && f.properties?.["kind"] === "ground_zone")
      .map(([fid, f]) => ({ id: fid, feature: f }));
    const { outcome, conflictId } = classifyGroundZone(candidate, existingZones);

    if (outcome === "partial-cross" && conflictId) {
      // HARD REJECT (D-07): remove the candidate geometry so it never commits (revert to last-valid) and
      // raise the transient banner referencing the EXISTING crossed zone. No commitEpoch bump → no PUT.
      set((s) => {
        const features: Record<string, GeoJSONStoreFeatures> = { ...s.features };
        const inheritedFields: Record<string, readonly string[]> = { ...s.inheritedFields };
        delete features[id];
        delete inheritedFields[id];
        const nonce = (s.groundReject?.nonce ?? 0) + 1;
        return {
          features,
          inheritedFields,
          selection: s.selection === id ? null : s.selection,
          groundReject: { conflictId, nonce },
        };
      });
      return "partial-cross";
    }

    // Accepted (disjoint or contained — innermost wins): tag + select + clear any stale reject banner.
    set((s) => {
      const features: Record<string, GeoJSONStoreFeatures> = { ...s.features };
      const inheritedFields: Record<string, readonly string[]> = { ...s.inheritedFields };
      tagFeature(features, inheritedFields, id, "ground_zone");
      return {
        features,
        inheritedFields,
        selection: id,
        dirty: true,
        commitEpoch: s.commitEpoch + 1,
        groundReject: null,
      };
    });
    return outcome;
  },

  dismissGroundReject: () => set({ groundReject: null }),

  zoomToFeature: (id) =>
    set((state) => ({ zoomRequest: { featureId: id, nonce: (state.zoomRequest?.nonce ?? 0) + 1 } })),

  selectAndZoom: (id) =>
    set((state) => ({
      selection: id,
      zoomRequest: { featureId: id, nonce: (state.zoomRequest?.nonce ?? 0) + 1 },
    })),

  noteCommit: () => set((state) => ({ dirty: true, commitEpoch: state.commitEpoch + 1 })),

  setProject: (id, name) => set({ projectId: id, projectName: name, dirty: false }),

  resetProject: () =>
    set({
      features: {},
      spectra: {},
      inheritedFields: {},
      selection: null,
      dirty: false,
      projectId: null,
      projectName: null,
      groundReject: null,
      zoomRequest: null,
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
      return { features, inheritedFields, dirty: true, commitEpoch: state.commitEpoch + 1 };
    }),

  setSpectrum: (key, authored) =>
    set((state) => {
      const spectra: Record<string, StoredSpectrum> = { ...state.spectra };
      if (authored === null) {
        delete spectra[key];
      } else {
        spectra[key] = authored;
      }
      return { spectra, dirty: true, commitEpoch: state.commitEpoch + 1 };
    }),

  openSpectrumEditor: (key, title) => set({ spectrumEditor: { key, title } }),
  closeSpectrumEditor: () => set({ spectrumEditor: null }),

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
