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
  // Trigger the whole-scene PUT.
  save(): Promise<void>;
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
    save() {
      return useSceneStore.getState().saveScene();
    },
  };
  (window as unknown as { __enviTest?: EnviTestBridge }).__enviTest = bridge;
}
