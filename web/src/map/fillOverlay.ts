// fillOverlay.ts — the shared MapLibre FILL-overlay primitives for the result overlays
// (the isophone noise map + the scenario-difference map). Both overlays trace a cached
// level grid into a GeoJSON FeatureCollection and upsert it as ONE `fill` layer that paints
// by the per-band `fill` property and sits BELOW the styled scene objects (D-18). Factored
// here so the two overlays share ONE scene-object insert order + one upsert/teardown — the
// D-18 z-order can never drift between them.
//
// # Module I/O
// - Input  a MapLibre map instance + a source/layer id pair + the fill opacity + the traced
//   FeatureCollection.
// - Output the source/layer upserted on (or removed from) the map. No telemetry / no store
//   coupling — each caller owns its own telemetry.

import type { GeoJSONSource, Map as MapLibreMap } from "maplibre-gl";

// Scene-object display-layer id prefixes (D-18): a result fill is inserted BELOW the first
// of these so the styled scene objects always draw on top. Best-effort until the full
// object restyle lands (11-10); if none are present the fill is appended above the basemap.
export const SCENE_LAYER_PREFIXES = [
  "envi-object",
  "envi-impedance",
  "envi-receiver",
  "envi-screen",
  "envi-weather",
  "dgm-",
  "gl-draw",
  "td-",
];

// The id of the first scene-object display layer, so a result fill can be inserted BELOW it
// (D-18). Returns undefined when none is present (→ append above the basemap).
export function sceneInsertBeforeId(map: MapLibreMap): string | undefined {
  const layers = map.getStyle()?.layers ?? [];
  for (const layer of layers) {
    if (SCENE_LAYER_PREFIXES.some((p) => layer.id.startsWith(p))) {
      return layer.id;
    }
  }
  return undefined;
}

// Add (or update) a GeoJSON `fill` source + layer from a FeatureCollection. The fill paints
// by the per-band `fill` property (legend ≡ contour ≡ class colour) and sits below the scene
// objects (D-18). Idempotent: an existing source is re-fed rather than re-added.
export function upsertGeoJsonFillLayer(
  map: MapLibreMap,
  sourceId: string,
  layerId: string,
  opacity: number,
  data: GeoJSON.FeatureCollection,
): void {
  const existing = map.getSource(sourceId) as GeoJSONSource | undefined;
  if (existing) {
    existing.setData(data);
    return;
  }
  map.addSource(sourceId, { type: "geojson", data });
  map.addLayer(
    {
      id: layerId,
      type: "fill",
      source: sourceId,
      paint: {
        "fill-color": ["get", "fill"],
        "fill-opacity": opacity,
        "fill-outline-color": "#0b0d10",
      },
    },
    sceneInsertBeforeId(map),
  );
}

// Remove a GeoJSON fill source + layer (teardown / no-data). Swallows a torn-down style.
export function removeGeoJsonFillLayer(map: MapLibreMap, sourceId: string, layerId: string): void {
  try {
    if (map.getLayer(layerId)) {
      map.removeLayer(layerId);
    }
    if (map.getSource(sourceId)) {
      map.removeSource(sourceId);
    }
  } catch {
    /* style already torn down */
  }
}
