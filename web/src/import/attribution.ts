// attribution.ts — the SC5 map-credit strings for imported GIS sources and the MapLibre
// `AttributionControl` wiring (D-11 data hygiene: attribute OSM / ESA WorldCover / Copernicus / AHN).
//
// # Module I/O
// - Input  a set of registry source ids that have contributed imported features (the import store's
//   `attributedSources`). These are FIXED registry vocabularies, never user input.
// - Output the SC5 attribution strings (`attributionStrings`), and `attachImportAttribution` which adds a
//   MapLibre `AttributionControl` carrying them and returns its teardown (paired with an effect cleanup,
//   T-07-06-03). The strings mirror `envi_gis::registry`'s `'static` `attribution` fields verbatim (one
//   source of truth); they are fixed constants, so — like `basemap.ts`'s `OSM_ATTRIBUTION` — they are safe
//   as MapLibre attribution markup (only USER strings are an XSS concern, T-07-06-02). The ImportPanel also
//   renders these as React text children.
// - Valid input range: `sourceIds` ⊆ the four import source ids below; an unknown id is ignored.

import { AttributionControl, type Map as MapLibreMap } from "maplibre-gl";

// The four import sources' SC5 credit lines, keyed by registry source id. These mirror the `'static`
// `attribution` strings in `crates/envi-gis/src/registry.rs` (kept in sync there as the source of truth).
export const IMPORT_ATTRIBUTIONS: Readonly<Record<string, string>> = {
  "ahn4-dtm": "AHN (Actueel Hoogtebestand Nederland), PDOK — CC0",
  glo30: "Copernicus DEM GLO-30 © ESA / Copernicus (credit required)",
  worldcover: "ESA WorldCover 2021 v200 — © ESA WorldCover, CC BY 4.0",
  "osm-overpass": "© OpenStreetMap contributors (ODbL)",
};

// The ordered, de-duplicated SC5 attribution strings for a set of contributing source ids (unknown ids
// are dropped). Order follows `IMPORT_ATTRIBUTIONS`' declaration for a stable credit line.
export function attributionStrings(sourceIds: Iterable<string>): string[] {
  const active = new Set(sourceIds);
  return Object.entries(IMPORT_ATTRIBUTIONS)
    .filter(([id]) => active.has(id))
    .map(([, text]) => text);
}

// Attach a MapLibre `AttributionControl` carrying the imported-source credits and return its teardown.
// Returns a no-op teardown when there is nothing to credit (no control added). The strings are fixed
// constants (safe as attribution markup); the control is recreated by the caller when the set changes.
export function attachImportAttribution(map: MapLibreMap, sourceIds: Iterable<string>): () => void {
  const custom = attributionStrings(sourceIds);
  if (custom.length === 0) {
    return () => {
      /* nothing credited yet — no control to remove */
    };
  }
  const control = new AttributionControl({ compact: true, customAttribution: custom });
  map.addControl(control);
  return () => {
    try {
      map.removeControl(control);
    } catch {
      /* map already disposed */
    }
  };
}
