// isophoneLayer.ts — the MapLibre isophone FILL layer + docked legend (WEB-06 /
// GRID-04 / D-02/D-04/D-18). Renders the noise map as fill polygons traced by the
// 11-02 WASM iso-band tracer (NEVER a heatmap raster — D-02), reprojected to LonLat
// in WASM at the one CRS seam, sitting BELOW the full-styled scene objects (D-18;
// the object restyle is 11-10). Editing the colour scale RE-CONTOURS the cached
// level grid (SC3): the layer re-runs ONLY the WASM tracer over the cached grid and
// re-paints — propagation is never re-run.
//
// # Module I/O
// - Input  the colour-scale store's single `breaks[]`/`colors[]` source of truth +
//   the cached `grid`/`crs`/`weightingLabel` it also holds (D-04). No acoustic math
//   here (D-01): the re-contour is a pure WASM tracer call; this file only marshals
//   the request and manages the MapLibre source/layer.
// - Output the `IsophoneLayer` map child (imperative fill layer, returns null) + the
//   `MapLegend` docked overlay (break values + class swatches + the metadata
//   weighting label) — both driven by the ONE scale, so legend ≡ contour ≡ class
//   colour. `isophoneTelemetry()` exposes the trace count + rendered feature count +
//   the layer paint type for the offline UAT.
// - Valid input range: a cached grid + a valid (V5) break scale; a null grid renders
//   nothing.
//
// # Legend ≡ contour ≡ class colours (enforced)
// The tracer receives `contourBreaks(breaks)` (the cap-extended edges) with `colors`
// as `band_fills`; the fill layer paints by the traced `fill` property; the legend
// derives its rows from the SAME `breaks`/`colors`. One array pair, three consumers.

import { createElement, useEffect, useRef, type ReactElement } from "react";
import { useMap } from "react-map-gl/maplibre";
import type { GeoJSONSource, Map as MapLibreMap } from "maplibre-gl";

import type { TraceIsophonesReq } from "../generated/wire";
import {
  contourBreaks,
  useColorScaleStore,
  validateBreaks,
  type ColorScaleState,
} from "../store/colorScale";

const ISOPHONE_SOURCE = "envi-isophone";
export const ISOPHONE_LAYER = "envi-isophone-fill";

// Scene-object display-layer id prefixes (D-18): the isophone fill is inserted
// BELOW the first of these so the styled scene objects always draw on top. Best-
// effort until the full object restyle lands (11-10); if none are present the fill
// is appended above the basemap.
const SCENE_LAYER_PREFIXES = [
  "envi-impedance",
  "envi-receiver",
  "envi-screen",
  "envi-weather",
  "dgm-",
  "gl-draw",
  "td-",
];

// --- Telemetry (offline UAT observability) --------------------------------

interface IsophoneTelemetry {
  // How many times the tracer has re-contoured the cached grid (a break edit / a
  // preset switch each increments this — the SC3 "re-contour, no re-solve" signal).
  traceCount: number;
  // The number of band features currently rendered (0 when no grid is cached).
  featureCount: number;
  // The MapLibre paint type of the isophone layer — MUST be `fill`, never a raster
  // density layer (D-02, the fill-not-raster invariant).
  layerType: string | null;
  // The last tracer error surfaced (null on success).
  error: string | null;
}

const TELEMETRY: IsophoneTelemetry = {
  traceCount: 0,
  featureCount: 0,
  layerType: null,
  error: null,
};

// A snapshot of the isophone telemetry for the offline UAT / the test bridge.
export function isophoneTelemetry(): IsophoneTelemetry {
  return { ...TELEMETRY };
}

// --- WASM tracer client (browser-only; injectable seam) -------------------

export interface IsophoneTraceClient {
  // Re-contour the cached grid into a WGS84 GeoJSON FeatureCollection string.
  trace(req: TraceIsophonesReq): Promise<string>;
}

// The real client: lazily instantiates the compute wasm module on the MAIN THREAD
// (the same module CalcPanel/results use — COOP/COEP holds `crossOriginIsolated`)
// and calls the `trace_isophones` export. A dynamic import inside the factory keeps
// the wasm/OPFS graph out of the Node unit-test module load.
export function createWasmTraceClient(): IsophoneTraceClient {
  let glue: Promise<typeof import("../generated/wasm-compute/envi_compute_wasm")> | null = null;
  const ensureGlue = (): Promise<typeof import("../generated/wasm-compute/envi_compute_wasm")> => {
    glue ??= (async () => {
      const g = await import("../generated/wasm-compute/envi_compute_wasm");
      await g.default();
      return g;
    })();
    return glue;
  };
  return {
    async trace(req) {
      const g = await ensureGlue();
      // Returns a GeoJSON FeatureCollection string a `geojson` source consumes.
      return g.trace_isophones(req) as string;
    },
  };
}

// The single-writer trace client (module-level, browser-only). Kept out of the
// store so the store stays Node-testable (mirrors the results ReadoutClient seam).
let traceClient: IsophoneTraceClient | null = null;
function ensureClient(): IsophoneTraceClient {
  traceClient ??= createWasmTraceClient();
  return traceClient;
}

// --- Trace request assembly (pure) ----------------------------------------

// Build the tracer request from the single source of truth: the cap-extended
// contour edges (N+1 closed bands) + the class colours as `band_fills` + the
// metadata weighting label. Returns null when there is nothing to contour (no grid
// or an invalid break scale — the layer then renders nothing rather than a broken
// contour).
export function buildTraceRequest(scale: ColorScaleState): TraceIsophonesReq | null {
  if (!scale.grid || !scale.crs) {
    return null;
  }
  if (validateBreaks(scale.breaks) !== null) {
    return null;
  }
  return {
    grid: scale.grid,
    crs: scale.crs,
    breaks: contourBreaks(scale.breaks),
    band_fills: scale.colors,
    weighting_label: scale.weightingLabel,
  };
}

// --- Legend classes (pure; legend ≡ contour ≡ class) ----------------------

export interface LegendClass {
  // The band index (0 = below-lowest, last = above-highest).
  index: number;
  // The human range label ("< 55", "55–60", …, "≥ 75").
  label: string;
  // The class fill colour (a `#rrggbb` from the single `colors[]`).
  color: string;
}

// Derive the legend rows from the SAME `breaks`/`colors` the tracer contours: N
// editable edges → N+1 classes (below-lowest cap, N−1 interiors, above-highest cap).
export function legendClasses(breaks: number[], colors: string[]): LegendClass[] {
  const out: LegendClass[] = [];
  for (let i = 0; i < colors.length; i += 1) {
    let label: string;
    if (i === 0) {
      label = `< ${breaks[0]}`;
    } else if (i === colors.length - 1) {
      label = `≥ ${breaks[breaks.length - 1]}`;
    } else {
      label = `${breaks[i - 1]}–${breaks[i]}`;
    }
    out.push({ index: i, label, color: colors[i] });
  }
  return out;
}

// --- Imperative layer management ------------------------------------------

// Find the id of the first scene-object display layer, so the isophone fill can be
// inserted BELOW it (D-18). Returns undefined when none is present (→ append).
function sceneInsertBeforeId(map: MapLibreMap): string | undefined {
  const layers = map.getStyle()?.layers ?? [];
  for (const layer of layers) {
    if (SCENE_LAYER_PREFIXES.some((p) => layer.id.startsWith(p))) {
      return layer.id;
    }
  }
  return undefined;
}

// Add (or update) the isophone fill source + layer from a GeoJSON FeatureCollection.
// The fill paints by the per-band `fill` property (legend ≡ contour ≡ class colour)
// and sits below the scene objects (D-18).
function applyIsophoneGeoJson(map: MapLibreMap, geojson: GeoJSON.FeatureCollection): void {
  const existing = map.getSource(ISOPHONE_SOURCE) as GeoJSONSource | undefined;
  if (existing) {
    existing.setData(geojson);
  } else {
    map.addSource(ISOPHONE_SOURCE, { type: "geojson", data: geojson });
    map.addLayer(
      {
        id: ISOPHONE_LAYER,
        type: "fill",
        source: ISOPHONE_SOURCE,
        paint: {
          "fill-color": ["get", "fill"],
          "fill-opacity": 0.5,
          "fill-outline-color": "#0b0d10",
        },
      },
      sceneInsertBeforeId(map),
    );
  }
  TELEMETRY.featureCount = geojson.features.length;
  TELEMETRY.layerType = map.getLayer(ISOPHONE_LAYER)?.type ?? null;
}

// Remove the isophone source + layer (teardown / no-grid).
function removeIsophoneLayer(map: MapLibreMap): void {
  try {
    if (map.getLayer(ISOPHONE_LAYER)) {
      map.removeLayer(ISOPHONE_LAYER);
    }
    if (map.getSource(ISOPHONE_SOURCE)) {
      map.removeSource(ISOPHONE_SOURCE);
    }
  } catch {
    /* style already torn down */
  }
  TELEMETRY.featureCount = 0;
  TELEMETRY.layerType = null;
}

// --- The map child: re-contour on scale change (SC3) ----------------------

// The imperative isophone layer controller — a child of <Map>, so `useMap()`
// resolves to this map instance. Subscribes to the single colour-scale source of
// truth (grid + breaks + colors + crs + weighting) and re-contours the CACHED grid
// on any change via the WASM tracer (NO re-solve, SC3), re-adding the layer after a
// basemap `style.load` (setStyle destroys sources — the overlay discipline).
export function IsophoneLayer(): null {
  const map = useMap();
  const grid = useColorScaleStore((s) => s.grid);
  const crs = useColorScaleStore((s) => s.crs);
  const breaks = useColorScaleStore((s) => s.breaks);
  const colors = useColorScaleStore((s) => s.colors);
  const weightingLabel = useColorScaleStore((s) => s.weightingLabel);
  // A generation counter guards against an out-of-order async trace result
  // overwriting a newer one.
  const gen = useRef(0);

  useEffect(() => {
    const instance = map.current?.getMap() as unknown as MapLibreMap | undefined;
    if (!instance) {
      return;
    }
    const req = buildTraceRequest(useColorScaleStore.getState());

    const recontour = (): void => {
      if (!req) {
        removeIsophoneLayer(instance);
        return;
      }
      const myGen = (gen.current += 1);
      TELEMETRY.traceCount += 1;
      ensureClient()
        .trace(req)
        .then((json) => {
          if (myGen !== gen.current) {
            return; // a newer edit superseded this trace
          }
          const fc = JSON.parse(json) as GeoJSON.FeatureCollection;
          if (instance.isStyleLoaded()) {
            applyIsophoneGeoJson(instance, fc);
          } else {
            instance.once("load", () => applyIsophoneGeoJson(instance, fc));
          }
          TELEMETRY.error = null;
        })
        .catch((err: unknown) => {
          TELEMETRY.error = err instanceof Error ? err.message : String(err);
        });
    };

    const onStyleLoad = (): void => recontour();
    if (instance.isStyleLoaded()) {
      recontour();
    } else {
      instance.once("load", recontour);
    }
    instance.on("style.load", onStyleLoad);

    return () => {
      instance.off("style.load", onStyleLoad);
      instance.off("load", recontour);
    };
    // Re-run on ANY scale change — a break/colour/preset edit re-contours the cached
    // grid (SC3). `grid`/`crs` change on a fresh readout.
  }, [map, grid, crs, breaks, colors, weightingLabel]);

  return null;
}

// --- The docked legend (bottom-left over the map) -------------------------

// The docked isophone legend: one row per class (swatch + range label) + the
// metadata weighting label. Rendered from the SAME `breaks`/`colors` as the contour
// (legend ≡ contour ≡ class colour). Written with `createElement` so this stays a
// JSX-free `.ts` module. Renders nothing when no grid is cached.
export function MapLegend(): ReactElement | null {
  const breaks = useColorScaleStore((s) => s.breaks);
  const colors = useColorScaleStore((s) => s.colors);
  const grid = useColorScaleStore((s) => s.grid);
  const weightingLabel = useColorScaleStore((s) => s.weightingLabel);

  if (!grid) {
    return null;
  }
  const classes = legendClasses(breaks, colors);

  return createElement(
    "div",
    { className: "isophone-legend", "data-testid": "isophone-legend", "data-band-count": classes.length },
    createElement(
      "div",
      { className: "isophone-legend-head mono" },
      `Noise level (${weightingLabel})`,
    ),
    createElement(
      "ul",
      { className: "isophone-legend-list" },
      // Highest class first (top of the legend), as printed maps read.
      [...classes].reverse().map((c) =>
        createElement(
          "li",
          {
            key: c.index,
            className: "isophone-legend-row mono",
            "data-testid": `isophone-legend-row-${c.index}`,
            "data-color": c.color,
            "data-label": c.label,
          },
          createElement("span", {
            className: "isophone-legend-swatch",
            style: { background: c.color },
          }),
          createElement("span", { className: "isophone-legend-label" }, c.label),
        ),
      ),
    ),
  );
}
