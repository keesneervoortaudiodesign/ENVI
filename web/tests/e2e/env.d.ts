// env.d.ts — ambient typing for the DEV-only E2E bridges the offline specs drive (07-07, 11-10).
// The production bundle never defines `window.__enviTest` / `window.__enviMap`; they exist only under
// `import.meta.env.DEV`.

import type { Map as MapLibreMap } from "maplibre-gl";

import type { EnviTestBridge } from "../../src/testBridge";

declare global {
  interface Window {
    __enviTest: EnviTestBridge;
    // The live MapLibre instance (`src/map/MapCanvas.tsx` → `DevMapProbe`). The object-styling UAT reads
    // the map's REAL rendered state through it — resolved paint expressions, `queryRenderedFeatures`, and
    // the canvas pixels — because telemetry alone cannot see what colour a scene object actually renders.
    __enviMap?: MapLibreMap;
  }
}

export {};
