// testBridge.ts — a DEV-ONLY window bridge that lets the offline Playwright suite drive programmatic
// scene commits (the plan permits "programmatic store commits" for the draw-each-kind E2E).
//
// # Module I/O
// - Input  none at import; `installTestBridge()` attaches `window.__enviTest` with commit/save/read
//   helpers backed by the canonical store (D-03). Terra Draw drawing is unreliable in headless WebGL, so
//   the E2E commits geometry through the same store path a finished draw uses (`commitFeature`).
// - Output `window.__enviTest.{commitActive, commit, state, save}` — placing a feature of the active (or
//   an explicit) kind, a store snapshot (feature-id → kind, selection, inherited-field lists), and the
//   whole-scene PUT. Installed ONLY under `import.meta.env.DEV`; the production `vite build` bundle (what
//   ships in web/dist and the contract test serves) never contains it.
// - Valid input range: WGS84 `lng`/`lat`; `kind` ∈ the 9 frozen KINDS.

import type { GeoJSONStoreFeatures } from "terra-draw";

import { KIND_META, isKind, type Kind } from "./draw/kinds";
import { useSceneStore } from "./store/sceneStore";
import { reopenLast } from "./store/projectActions";
import { useImportStore, type LayerKey, type LayerStatus } from "./store/import";
import { runImport, retryLayer } from "./import/importJob";
import { getTile, removeTile } from "./import/opfs";
import type { GroundZoneOutcome } from "./validate/groundZone";
import type { AuthoredSpectrumDto, BboxDto } from "./generated/wire";

// A JSON-safe per-layer import status snapshot (the 08-08 offline E2E asserts on these).
export interface ImportLayerSnapshot {
  readonly status: LayerStatus;
  readonly featureCount: number;
  readonly surfaceModel: boolean;
  readonly error: { readonly status: number; readonly detail: string } | null;
}

// Build a minimal valid geometry for a kind's Terra Draw geometry mode, offset around [lng, lat].
function geometryFor(kind: Kind, lng: number, lat: number): GeoJSONStoreFeatures["geometry"] {
  const d = 0.0005;
  switch (KIND_META[kind].mode) {
    case "point":
      return { type: "Point", coordinates: [lng, lat] };
    case "linestring":
      return { type: "LineString", coordinates: [[lng, lat], [lng + d, lat + d]] };
    case "polygon":
      return {
        type: "Polygon",
        coordinates: [[[lng, lat], [lng + d, lat], [lng + d, lat + d], [lng, lat]]],
      };
  }
}

function commit(kind: Kind, lng: number, lat: number): string {
  const id = crypto.randomUUID();
  const feature = {
    id,
    type: "Feature",
    geometry: geometryFor(kind, lng, lat),
    properties: {},
  } as unknown as GeoJSONStoreFeatures;
  useSceneStore.getState().commitFeature(kind, feature);
  return id;
}

export interface EnviTestBridge {
  // Commit a feature of the CURRENTLY active palette kind (proves palette selection → kind tag).
  commitActive(lng: number, lat: number): string;
  // Commit a feature of an explicit kind.
  commit(kind: Kind, lng: number, lat: number): string;
  // A JSON-safe snapshot of the store for assertions.
  state(): {
    kinds: Record<string, string | null>;
    selection: string | null;
    inherited: Record<string, readonly string[]>;
  };
  // The per-edge UUIDs (D-02) of a building feature, in ring order.
  buildingEdges(id: string): string[];
  // Set an authored isolation spectrum for a feature/edge key (a façade override or a wall/screen spectrum).
  setSpectrum(key: string, authored: AuthoredSpectrumDto): void;
  // The authored spectrum stored under a key, or null.
  spectrum(key: string): AuthoredSpectrumDto | null;
  // Apply a building geometry update (a new footprint ring) through the same store path a Terra Draw edit
  // uses, so the D-02 ring-diff reconciles edge_ids + façade spectra. `ring` is a CLOSED ring `[x, y][]`.
  applyBuildingRing(id: string, ring: [number, number][]): void;
  // The current [x, y] endpoints of the edge whose UUID is `edgeId` on building `id` (for re-point checks).
  edgeSegment(id: string, edgeId: string): { from: [number, number]; to: [number, number] } | null;
  // Draw a ground_zone from a CLOSED ring `[x, y][]` through the SAME draw-time classification path a
  // finished Terra Draw polygon takes (D-07): the geometry is upserted then classified. Returns the
  // outcome, the id (present only when committed), and the crossed zone's id on a partial-cross reject.
  commitGroundZone(ring: [number, number][]): {
    outcome: GroundZoneOutcome;
    id: string | null;
    conflictId: string | null;
  };
  // Merge a non-geometric property patch into a feature (a committed inspector edit path).
  update(id: string, patch: Record<string, unknown>): void;
  // Open a project (id + display name) — the Delete-project dialog compares the typed name to this.
  openProject(id: string, name: string): void;
  // Close the current project (route to the empty/no-project state) — the SC4 close-before-reopen step.
  closeProject(): void;
  // Reopen the last-opened project (GET /projects/last → scene) — the SC4 reopen-last step. Resolves to
  // whether a project was restored.
  reopenLast(): Promise<boolean>;
  // The canonical feature ids currently in the store (SC4 round-trip fidelity checks).
  featureIds(): string[];
  // Trigger the whole-scene PUT.
  save(): Promise<void>;

  // --- GIS import (08-08 offline E2E). Drives the REAL import orchestrator (D-06/D-07), not a stub. ---
  // Run a viewport import for the currently-open project over the given WGS84 bbox (every enabled layer).
  runImport(bbox: BboxDto): void;
  // Enable/disable a layer for the next import/retry (D-06 per-layer toggles).
  setImportLayerEnabled(layer: LayerKey, enabled: boolean): void;
  // Retry a single failed layer without touching its siblings (D-07).
  retryImportLayer(layer: LayerKey): void;
  // A JSON-safe snapshot of every layer's import status (assertions).
  importState(): {
    layers: Record<LayerKey, ImportLayerSnapshot>;
    attributedSources: string[];
    debugOverlay: boolean;
  };
  // Toggle the SC3 impedance debug overlay.
  toggleImpedanceOverlay(): void;
  // The scene load epoch — bumped on every import commit (loadImportedScene). A re-run signal for the
  // DATA-04 replay that is independent of the D-09 idempotent-merge feature count.
  sceneEpoch(): number;
  // Whether a source tile is cached in this project's OPFS (DATA-04 replay asserts an OPFS hit).
  cachedTile(source: string, tile: string): Promise<boolean>;
  // Evict a cached source tile — the DATA-04 negative guard (removed entry ⇒ the compute read fails).
  evictTile(source: string, tile: string): Promise<void>;
}

export function installTestBridge(): void {
  const bridge: EnviTestBridge = {
    commitActive(lng, lat) {
      const tool = useSceneStore.getState().activeTool;
      if (!isKind(tool)) {
        throw new Error("commitActive: the active tool is the pointer, not a drawing kind");
      }
      return commit(tool, lng, lat);
    },
    commit,
    state() {
      const s = useSceneStore.getState();
      const kinds: Record<string, string | null> = {};
      for (const id of Object.keys(s.features)) {
        kinds[id] = s.kindOf(id);
      }
      return { kinds, selection: s.selection, inherited: { ...s.inheritedFields } };
    },
    buildingEdges(id) {
      const raw = useSceneStore.getState().features[id]?.properties?.["edge_ids"];
      return Array.isArray(raw) ? (raw.filter((x) => typeof x === "string") as string[]) : [];
    },
    setSpectrum(key, authored) {
      useSceneStore.getState().setSpectrum(key, authored);
    },
    spectrum(key) {
      return (useSceneStore.getState().spectra[key] ?? null) as AuthoredSpectrumDto | null;
    },
    applyBuildingRing(id, ring) {
      const prev = useSceneStore.getState().features[id];
      const feature = {
        id,
        type: "Feature",
        geometry: { type: "Polygon", coordinates: [ring] },
        properties: { ...(prev?.properties ?? {}) },
      } as unknown as GeoJSONStoreFeatures;
      useSceneStore.getState().applyTerraDrawChange([id], "update", [feature]);
    },
    edgeSegment(id, edgeId) {
      const s = useSceneStore.getState();
      const edges = this.buildingEdges(id);
      const pos = edges.indexOf(edgeId);
      if (pos < 0) {
        return null;
      }
      const geometry = s.features[id]?.geometry;
      if (!geometry || geometry.type !== "Polygon") {
        return null;
      }
      const outer = geometry.coordinates[0];
      const closed =
        outer.length > 1 &&
        outer[0][0] === outer[outer.length - 1][0] &&
        outer[0][1] === outer[outer.length - 1][1];
      const verts = closed ? outer.slice(0, outer.length - 1) : outer.slice();
      const from = verts[pos];
      const to = verts[(pos + 1) % verts.length];
      return { from: [from[0], from[1]], to: [to[0], to[1]] };
    },
    commitGroundZone(ring) {
      const id = crypto.randomUUID();
      const feature = {
        id,
        type: "Feature",
        geometry: { type: "Polygon", coordinates: [ring] },
        properties: {},
      } as unknown as GeoJSONStoreFeatures;
      // Upsert the raw geometry (the draw path), then run the D-07 draw-time classification.
      useSceneStore.getState().applyTerraDrawChange([id], "create", [feature]);
      const outcome = useSceneStore.getState().commitGroundZoneCandidate(id);
      const conflictId = useSceneStore.getState().groundReject?.conflictId ?? null;
      return { outcome, id: outcome === "partial-cross" ? null : id, conflictId };
    },
    update(id, patch) {
      useSceneStore.getState().updateProperties(id, patch);
    },
    openProject(id, name) {
      useSceneStore.getState().setProject(id, name);
    },
    closeProject() {
      useSceneStore.getState().resetProject();
    },
    reopenLast() {
      return reopenLast();
    },
    featureIds() {
      return Object.keys(useSceneStore.getState().features);
    },
    save() {
      return useSceneStore.getState().saveScene();
    },
    runImport(bbox) {
      const projectId = useSceneStore.getState().projectId;
      if (!projectId) {
        throw new Error("runImport: no project is open");
      }
      runImport(projectId, bbox);
    },
    setImportLayerEnabled(layer, enabled) {
      useImportStore.getState().setLayerEnabled(layer, enabled);
    },
    retryImportLayer(layer) {
      retryLayer(layer);
    },
    importState() {
      const s = useImportStore.getState();
      const layers = {} as Record<LayerKey, ImportLayerSnapshot>;
      for (const layer of Object.keys(s.layers) as LayerKey[]) {
        const l = s.layers[layer];
        layers[layer] = {
          status: l.status,
          featureCount: l.featureCount,
          surfaceModel: l.surfaceModel,
          error: l.error ? { status: l.error.status, detail: l.error.detail } : null,
        };
      }
      return { layers, attributedSources: [...s.attributedSources], debugOverlay: s.debugOverlay };
    },
    toggleImpedanceOverlay() {
      useImportStore.getState().toggleDebugOverlay();
    },
    sceneEpoch() {
      return useSceneStore.getState().loadEpoch;
    },
    async cachedTile(source, tile) {
      const projectId = useSceneStore.getState().projectId;
      if (!projectId) {
        return false;
      }
      return (await getTile(projectId, source, tile)) !== null;
    },
    async evictTile(source, tile) {
      const projectId = useSceneStore.getState().projectId;
      if (projectId) {
        await removeTile(projectId, source, tile);
      }
    },
  };
  (window as unknown as { __enviTest?: EnviTestBridge }).__enviTest = bridge;
}
