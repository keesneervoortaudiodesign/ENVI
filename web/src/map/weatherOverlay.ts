// weatherOverlay.ts — the Phase-9 debug overlays: the building-aware receiver grid (GRID-01), the impedance-
// segmented source→receiver path (GEOX-02), and the injected screen-top vertices (GEOX-03), each toggled from
// the WeatherPanel. All geometry is produced by the `envi-gis` WASM shims (via `sceneDebug`); these overlays
// only STYLE it on the MapLibre canvas — no GIS math here.
//
// # Module I/O
// - Input  the weather store's `debug` geometry (WGS84 receiver points, per-σ segment polylines, screen-top
//   vertices) + its three overlay toggles. No props.
// - Output MapLibre sources/layers on the current style, (re)created after a basemap `style.load` (setStyle
//   destroys sources — the DgmOverlay discipline) and shown only while their toggle is on; torn down in the
//   effect cleanup (T-07-06-03). The σ ramp colours each segment soft (blue) → hard (red).
// - Valid input range: absent geometry renders an empty collection (the overlay clears) — never a fabricated
//   feature.

import { useEffect, useMemo, type ReactElement } from "react";
import { useMap } from "react-map-gl/maplibre";
import type { Map as MapLibreMap, GeoJSONSource } from "maplibre-gl";

import { useWeatherStore } from "../store/weather";
import type { DebugSegment } from "../import/sceneDebug";
import { IMPEDANCE_COLORS } from "./impedanceOverlay";

const GRID_SOURCE = "envi-weather-grid";
const GRID_LAYER = "envi-weather-grid-pts";
const SEG_SOURCE = "envi-weather-seg";
const SEG_LAYER = "envi-weather-seg-lines";
const SCREEN_SOURCE = "envi-weather-screens";
const SCREEN_LAYER = "envi-weather-screen-pts";

// Soft (class A) and hard (class H) σ endpoints on a log scale. A segment's σ is mapped soft→hard for a
// readable ground-class ramp.
const SIGMA_SOFT = 12.5;
const SIGMA_HARD = 200_000;

// The debug ramp endpoints reuse the impedance overlay's class-A (soft) and class-H (hard) palette colours —
// one source of truth for the two ground-class endpoints, not a second pair of hex literals.
function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace("#", "");
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ];
}
const SOFT_RGB: [number, number, number] = hexToRgb(IMPEDANCE_COLORS.A);
const HARD_RGB: [number, number, number] = hexToRgb(IMPEDANCE_COLORS.H);

// Map a σ (flow resistivity) to a soft→hard debug colour on a log scale.
function sigmaColor(sigma: number): string {
  const lo = Math.log10(SIGMA_SOFT);
  const hi = Math.log10(SIGMA_HARD);
  const s = sigma > 0 ? Math.log10(sigma) : lo;
  const t = Math.max(0, Math.min(1, (s - lo) / (hi - lo)));
  const c = (i: number): number => Math.round(SOFT_RGB[i] + t * (HARD_RGB[i] - SOFT_RGB[i]));
  return `rgb(${c(0)}, ${c(1)}, ${c(2)})`;
}

// A FeatureCollection of the receiver points (or an empty one when the overlay has no geometry).
function pointsGeoJson(points: readonly [number, number][]): GeoJSON.FeatureCollection {
  return {
    type: "FeatureCollection",
    features: points.map((p) => ({
      type: "Feature",
      properties: {},
      geometry: { type: "Point", coordinates: [p[0], p[1]] },
    })),
  };
}

// A FeatureCollection of the σ-tagged segment polylines.
function segmentsGeoJson(segments: readonly DebugSegment[]): GeoJSON.FeatureCollection {
  return {
    type: "FeatureCollection",
    features: segments.map((seg) => ({
      type: "Feature",
      properties: { color: sigmaColor(seg.sigma) },
      geometry: { type: "LineString", coordinates: seg.line.map((c) => [c[0], c[1]]) },
    })),
  };
}

// Shared imperative overlay controller: (re)create a source + layer on the current style, push the latest
// data when the toggle is on, hide it when off, and tear down on unmount. Mirrors DgmOverlay/impedanceOverlay.
function useDebugLayer(
  sourceId: string,
  layerId: string,
  visible: boolean,
  data: GeoJSON.FeatureCollection,
  addLayer: (instance: MapLibreMap) => void,
): void {
  const map = useMap();
  useEffect(() => {
    const instance = map.current?.getMap() as unknown as MapLibreMap | undefined;
    if (!instance) {
      return;
    }

    const ensureAndSet = (): void => {
      const alreadyRendered = instance.getSource(sourceId) !== undefined;
      if (!visible && !alreadyRendered) {
        return; // never build a hidden overlay's source from scratch
      }
      const src = instance.getSource(sourceId) as GeoJSONSource | undefined;
      if (src) {
        src.setData(data);
      } else {
        instance.addSource(sourceId, { type: "geojson", data });
        addLayer(instance);
      }
      if (instance.getLayer(layerId)) {
        instance.setLayoutProperty(layerId, "visibility", visible ? "visible" : "none");
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
        if (instance.getLayer(layerId)) {
          instance.removeLayer(layerId);
        }
        if (instance.getSource(sourceId)) {
          instance.removeSource(sourceId);
        }
      } catch {
        /* style already torn down — nothing to remove */
      }
    };
    // `data` identity changes when the store's debug geometry changes; `visible` toggles the overlay.
  }, [map, sourceId, layerId, visible, data, addLayer]);
}

// The receiver-grid overlay (GRID-01): the building-aware lattice of receiver points.
export function ReceiverGridOverlay(): ReactElement | null {
  const debug = useWeatherStore((s) => s.debug);
  const visible = useWeatherStore((s) => s.showGrid);
  // Rebuild the FeatureCollection only when the receiver geometry itself changes, so an
  // unrelated store update does not re-fire the overlay's `setData` (the `data` identity
  // is stable across renders otherwise).
  const data = useMemo(() => pointsGeoJson(debug?.receivers ?? []), [debug?.receivers]);
  useDebugLayer(
    GRID_SOURCE,
    GRID_LAYER,
    visible,
    data,
    (instance) =>
      instance.addLayer({
        id: GRID_LAYER,
        type: "circle",
        source: GRID_SOURCE,
        paint: {
          "circle-radius": 2.5,
          "circle-color": "#22c55e",
          "circle-opacity": 0.8,
          "circle-stroke-width": 0.5,
          "circle-stroke-color": "#0b0d10",
        },
      }),
  );
  return null;
}

// The impedance-segmentation overlay (GEOX-02): the source→receiver path coloured per-interval by σ.
export function ImpedanceSegOverlay(): ReactElement | null {
  const debug = useWeatherStore((s) => s.debug);
  const visible = useWeatherStore((s) => s.showImpedance);
  const data = useMemo(() => segmentsGeoJson(debug?.segments ?? []), [debug?.segments]);
  useDebugLayer(
    SEG_SOURCE,
    SEG_LAYER,
    visible,
    data,
    (instance) =>
      instance.addLayer({
        id: SEG_LAYER,
        type: "line",
        source: SEG_SOURCE,
        paint: { "line-color": ["get", "color"] as never, "line-width": 3, "line-opacity": 0.85 },
      }),
  );
  return null;
}

// The screen-vertex overlay (GEOX-03): the injected screen-top crossings on the path.
export function ScreenVertexOverlay(): ReactElement | null {
  const debug = useWeatherStore((s) => s.debug);
  const visible = useWeatherStore((s) => s.showScreens);
  const data = useMemo(() => pointsGeoJson(debug?.screenVertices ?? []), [debug?.screenVertices]);
  useDebugLayer(
    SCREEN_SOURCE,
    SCREEN_LAYER,
    visible,
    data,
    (instance) =>
      instance.addLayer({
        id: SCREEN_LAYER,
        type: "circle",
        source: SCREEN_SOURCE,
        paint: {
          "circle-radius": 4,
          "circle-color": "#f5a623",
          "circle-opacity": 0.9,
          "circle-stroke-width": 1,
          "circle-stroke-color": "#0b0d10",
        },
      }),
  );
  return null;
}
