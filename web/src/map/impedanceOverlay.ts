// impedanceOverlay.ts — the impedance DEBUG overlay (SC3): styles imported `ground_zone` features by their
// Nord2000 impedance class letter (A–H), over a "no data → project default" wash everywhere else, so the
// user can see the effective ground class across the whole scene. Toggleable from the ImportPanel.
//
// # Module I/O
// - Input  the canonical scene store's `ground_zone` features (their `impedance_class` letter) + the import
//   store's `debugOverlay` toggle. No GIS math here — it only styles features the WASM landcover path
//   produced.
// - Output two MapLibre layers on the current style: a low-opacity world "wash" tinted by the project
//   default ground class (the no-data fallback), and a fill layer colouring each `ground_zone` by its class
//   letter. Both are (re)created after a basemap `style.load` (setStyle destroys sources — the DgmOverlay
//   discipline) and shown only while `debugOverlay` is on. Torn down in the effect cleanup (T-07-06-03).
// - Valid input range: a `ground_zone` with an unknown/absent class letter falls to the default colour, so
//   the overlay never renders a blank hole.

import { useEffect, type ReactElement } from "react";
import { useMap } from "react-map-gl/maplibre";
import type { Map as MapLibreMap, GeoJSONSource } from "maplibre-gl";

import { useSceneStore } from "../store/sceneStore";
import { useImportStore } from "../store/import";

const WASH_SOURCE = "envi-impedance-wash";
const WASH_LAYER = "envi-impedance-wash-fill";
const ZONE_SOURCE = "envi-impedance-zones";
const ZONE_LAYER = "envi-impedance-zone-fill";

// The project default ground class letter (SettingsDto default is `'D'`) — the "no data" wash tint. The
// scene store does not carry project settings this phase, so the documented default is used.
const DEFAULT_GROUND_CLASS = "D";

// A per-class debug palette A (soft) → H (hard). Fixed debug colours (not the design-system chart palette);
// distinct enough to read the effective ground class at a glance.
const IMPEDANCE_COLORS: Readonly<Record<string, string>> = {
  A: "#2c7fb8",
  B: "#41b6c4",
  C: "#7fcdbb",
  D: "#c7e9b4",
  E: "#ffffcc",
  F: "#fed976",
  G: "#fd8d3c",
  H: "#e31a1c",
};
const FALLBACK_COLOR = IMPEDANCE_COLORS[DEFAULT_GROUND_CLASS];

// The MapLibre `match` expression: class letter → colour, with the project default as the fallback.
function classColorExpression(): unknown[] {
  const match: unknown[] = ["match", ["get", "impedance_class"]];
  for (const [letter, color] of Object.entries(IMPEDANCE_COLORS)) {
    match.push(letter, color);
  }
  match.push(FALLBACK_COLOR); // default (unknown/absent class)
  return match;
}

// A world-covering polygon for the "no data → project default" wash beneath the zones.
function washGeoJson(): GeoJSON.FeatureCollection {
  return {
    type: "FeatureCollection",
    features: [
      {
        type: "Feature",
        properties: {},
        geometry: {
          type: "Polygon",
          coordinates: [
            [
              [-180, -85],
              [180, -85],
              [180, 85],
              [-180, 85],
              [-180, -85],
            ],
          ],
        },
      },
    ],
  };
}

// Build a FeatureCollection of the scene's `ground_zone` polygons carrying only their class letter.
function zoneGeoJson(): GeoJSON.FeatureCollection {
  const out: GeoJSON.Feature[] = [];
  for (const feature of Object.values(useSceneStore.getState().features)) {
    const props = (feature.properties ?? {}) as Record<string, unknown>;
    if (props["kind"] !== "ground_zone") {
      continue;
    }
    const geometry = feature.geometry;
    // Imported ground_zone features are (Multi)Polygon; the terra-draw store type narrows to Polygon, so
    // read the discriminant as a string to admit an imported MultiPolygon too.
    const geometryType = geometry?.type as string | undefined;
    if (geometryType === "Polygon" || geometryType === "MultiPolygon") {
      out.push({
        type: "Feature",
        properties: { impedance_class: props["impedance_class"] ?? DEFAULT_GROUND_CLASS },
        geometry: geometry as GeoJSON.Geometry,
      });
    }
  }
  return { type: "FeatureCollection", features: out };
}

// The imperative overlay controller — a child of <Map>, so `useMap()` resolves to this map instance.
export function ImpedanceOverlay(): ReactElement | null {
  const map = useMap();
  const features = useSceneStore((s) => s.features);
  const debugOverlay = useImportStore((s) => s.debugOverlay);

  useEffect(() => {
    const instance = map.current?.getMap() as unknown as MapLibreMap | undefined;
    if (!instance) {
      return;
    }

    const ensureAndSet = (): void => {
      const visibility = debugOverlay ? "visible" : "none";
      // Wash (beneath the zones).
      const washSrc = instance.getSource(WASH_SOURCE) as GeoJSONSource | undefined;
      if (washSrc) {
        washSrc.setData(washGeoJson());
      } else {
        instance.addSource(WASH_SOURCE, { type: "geojson", data: washGeoJson() });
        instance.addLayer({
          id: WASH_LAYER,
          type: "fill",
          source: WASH_SOURCE,
          paint: { "fill-color": FALLBACK_COLOR, "fill-opacity": 0.12 },
        });
      }
      // Zones.
      const zoneData = zoneGeoJson();
      const zoneSrc = instance.getSource(ZONE_SOURCE) as GeoJSONSource | undefined;
      if (zoneSrc) {
        zoneSrc.setData(zoneData);
      } else {
        instance.addSource(ZONE_SOURCE, { type: "geojson", data: zoneData });
        instance.addLayer({
          id: ZONE_LAYER,
          type: "fill",
          source: ZONE_SOURCE,
          paint: {
            "fill-color": classColorExpression() as never,
            "fill-opacity": 0.4,
            "fill-outline-color": "#0b0d10",
          },
        });
      }
      for (const layer of [WASH_LAYER, ZONE_LAYER]) {
        if (instance.getLayer(layer)) {
          instance.setLayoutProperty(layer, "visibility", visibility);
        }
      }
    };

    const onStyleLoad = (): void => ensureAndSet();
    if (instance.isStyleLoaded()) {
      ensureAndSet();
    } else {
      instance.once("load", ensureAndSet);
    }
    instance.on("style.load", onStyleLoad);

    return () => {
      instance.off("style.load", onStyleLoad);
      instance.off("load", ensureAndSet);
      try {
        for (const layer of [ZONE_LAYER, WASH_LAYER]) {
          if (instance.getLayer(layer)) {
            instance.removeLayer(layer);
          }
        }
        for (const source of [ZONE_SOURCE, WASH_SOURCE]) {
          if (instance.getSource(source)) {
            instance.removeSource(source);
          }
        }
      } catch {
        /* style already torn down — nothing to remove */
      }
    };
  }, [map, features, debugOverlay]);

  return null;
}
