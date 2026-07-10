// DgmOverlay.tsx — the DGM TIN overlay: renders the server-triangulated mesh (D-08, SC1) as recessive
// edges on the MapLibre canvas, driven by the `dgm` store slice.
//
// # Module I/O
// - Input  the `dgm` store's `triangulation` (vertices + triangle index triples). Vertices are
//   `[lng, lat, z]` (the Phase-7 preview coordinate space) — the overlay draws their [lng, lat] edges.
// - Output a single GeoJSON line source/layer (`envi-dgm-tin`) of the triangle edges, coloured from the
//   recessive `--color-info` token (resolved from the theme, no invented token). The layer is (re)created
//   after a basemap `style.load` (setStyle destroys sources — the same rebuild discipline as Terra Draw)
//   and its data is refreshed whenever the slice changes. Torn down in the effect cleanup.
// - Valid input range: an empty / null triangulation renders an empty collection (the overlay clears).

import { useEffect, type ReactElement } from "react";
import { useMap } from "react-map-gl/maplibre";
import type { Map as MapLibreMap, GeoJSONSource } from "maplibre-gl";

import { useDgmStore } from "../store/dgm";
import type { DgmResp } from "../generated/wire";

const SOURCE_ID = "envi-dgm-tin";
const LAYER_ID = "envi-dgm-tin-edges";

// The recessive edge colour, resolved from the theme token (D-11 — no invented token). Falls back to the
// token's known value only if the property is unavailable (e.g. a bare test DOM).
function edgeColor(): string {
  const value = getComputedStyle(document.documentElement).getPropertyValue("--color-info").trim();
  return value || "#4ea8ff";
}

// Build a FeatureCollection of the triangle edges (three LineStrings per triangle) from the TIN.
function edgesGeoJson(tin: DgmResp | null): GeoJSON.FeatureCollection {
  const features: GeoJSON.Feature[] = [];
  if (tin) {
    for (const [i, j, k] of tin.triangles) {
      const a = tin.vertices[i];
      const b = tin.vertices[j];
      const c = tin.vertices[k];
      if (a && b && c) {
        features.push({
          type: "Feature",
          properties: {},
          geometry: {
            type: "LineString",
            coordinates: [
              [a[0], a[1]],
              [b[0], b[1]],
              [c[0], c[1]],
              [a[0], a[1]],
            ],
          },
        });
      }
    }
  }
  return { type: "FeatureCollection", features };
}

// The imperative overlay controller: a child of <Map>, so `useMap()` resolves to this map instance.
export function DgmOverlay(): ReactElement | null {
  const map = useMap();
  const triangulation = useDgmStore((s) => s.triangulation);

  useEffect(() => {
    const instance = map.current?.getMap() as unknown as MapLibreMap | undefined;
    if (!instance) {
      return;
    }

    // Ensure the source + layer exist on the CURRENT style, then push the latest edges. Idempotent — safe
    // to call on initial load, on triangulation change, and after a basemap style.load rebuild.
    const ensureAndSet = (): void => {
      const data = edgesGeoJson(useDgmStore.getState().triangulation);
      const existing = instance.getSource(SOURCE_ID) as GeoJSONSource | undefined;
      if (existing) {
        existing.setData(data);
        return;
      }
      instance.addSource(SOURCE_ID, { type: "geojson", data });
      instance.addLayer({
        id: LAYER_ID,
        type: "line",
        source: SOURCE_ID,
        paint: { "line-color": edgeColor(), "line-width": 1, "line-opacity": 0.5 },
      });
    };

    // setStyle() destroys every source/layer; re-add on the single style.load hook (SC4 discipline).
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
        if (instance.getLayer(LAYER_ID)) {
          instance.removeLayer(LAYER_ID);
        }
        if (instance.getSource(SOURCE_ID)) {
          instance.removeSource(SOURCE_ID);
        }
      } catch {
        /* style already torn down — nothing to remove */
      }
    };
  }, [map, triangulation]);

  return null;
}
