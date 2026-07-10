// basemap.ts — the dark-vector basemap configuration (D-13/D-13a). The basemap recedes so the drawn
// scene carries the colour; OSM attribution is always displayed (CLAUDE.md data-hygiene).
//
// # Module I/O
// - Input  none (static config + a MapLibre map handle for the attribution helper).
// - Output `DARK_BASEMAP_STYLE` — the OpenFreeMap `styles/dark` URL (MIT style, OSM data, NO API key),
//   fetched over the network at runtime (D-13a: "no API key" != "no network"); `ALT_BASEMAP_STYLE` —
//   an inline dark style used by the temporary Gate-1 "switch basemap" control to exercise
//   `map.setStyle()` + `style.load` re-hydration entirely offline; `attachOsmAttribution` — attaches a
//   MapLibre `AttributionControl` mentioning OpenStreetMap and returns its teardown.
// - Valid input range: the network style is intercepted by Playwright (`installBasemapMocks`) so the
//   E2E suite never touches `tiles.openfreemap.org`.

import { AttributionControl, type Map as MapLibreMap, type StyleSpecification } from "maplibre-gl";

// D-13a: the runtime basemap. A MapLibre XHR, not an index.html asset — the Phase-6 "zero external
// assets in index.html" gate stays green. Dark so the chrome and basemap share a luminance band (D-13).
export const DARK_BASEMAP_STYLE = "https://tiles.openfreemap.org/styles/dark";

// The OSM data-attribution string (CLAUDE.md). A fixed constant — never a user-derived value — so it
// is safe as MapLibre attribution markup (threat T-07-06-02: only user strings are an XSS concern).
export const OSM_ATTRIBUTION =
  '© <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noreferrer">OpenStreetMap</a> contributors';

// A minimal inline dark style (background only, no external sources) used purely to exercise a basemap
// switch in the Gate-1 spike/E2E: switching to it fires `style.load` (proving re-hydration) without a
// second network fetch, keeping the switch offline. Stays dark (D-13, --color-bg #0b0d10).
export const ALT_BASEMAP_STYLE: StyleSpecification = {
  version: 8,
  name: "envi-dark-fallback",
  sources: {},
  layers: [
    {
      id: "background",
      type: "background",
      paint: { "background-color": "#0b0d10" },
    },
  ],
};

// Attach an OSM-mentioning AttributionControl to the map and return a teardown that removes it. The
// caller pairs this with its effect cleanup (every imperative map subscription is torn down — T-07-06-03).
export function attachOsmAttribution(map: MapLibreMap): () => void {
  const control = new AttributionControl({ compact: true, customAttribution: OSM_ATTRIBUTION });
  map.addControl(control);
  return () => {
    // removeControl throws if the map is already torn down; the guard keeps cleanup idempotent.
    try {
      map.removeControl(control);
    } catch {
      /* map already disposed */
    }
  };
}
