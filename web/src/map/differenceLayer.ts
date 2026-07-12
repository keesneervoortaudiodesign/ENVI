// differenceLayer.ts — the MapLibre scenario DIFFERENCE fill layer + docked legend
// (METX-04 / D-16). Renders the per-receiver dB(A) delta (scenario A − B) as fill
// polygons traced by the SAME 11-02 WASM iso-band tracer the isophone layer uses
// (fill, never a heatmap raster — D-02), on a DIVERGING blue↔gray↔red scale with a
// symmetric clamp about 0 and a ±0.5 dB neutral dead-zone (UI-SPEC §2, the
// dataviz-validated diverging pair). The delta itself is WASM-produced
// (`store/difference`); this layer only maps it to the display palette + contours it.
//
// # Module I/O
// - Input  the difference store's `delta`/`grid`/`crs` + the A/B labels. The delta is
//   the WASM `A − B`; the DISPLAY math here (the symmetric clamp + the diverging
//   palette sampling) is colour-scale arithmetic, NOT acoustics — the same class of
//   display math `colorScale.ts` does (Math.abs/round/palette RGB lerp), never a dB.
// - Output the `DifferenceLayer` map child (imperative fill layer, returns null) + the
//   `DifferenceLegend` docked overlay (signed-dB rows). `differenceTelemetry()`
//   exposes the trace count + feature count + paint type + the neutral-midpoint colour
//   for the offline UAT (asserting gray-at-0, never a hue, and the brand accent is not
//   a pole).
//
// # Diverging scale (never a hue at 0)
// The scale is symmetric about 0: `M = max(round(max|Δ|), 1)` dB per arm, a central
// `[−0.5, +0.5]` dead-zone band rendered NEUTRAL GRAY (below the just-noticeable
// difference), blue for A quieter (Δ < 0), red for A louder (Δ > 0). The band count is
// odd so the midpoint band is EXACTLY the gray stop, never an interpolated hue.

import { createElement, useEffect, useMemo, useRef, type ReactElement } from "react";
import { useMap } from "react-map-gl/maplibre";
import type { Map as MapLibreMap } from "maplibre-gl";

import type { ExportCrsDto, ExportGridDto, TraceIsophonesReq } from "../generated/wire";
import { contourBreaks, samplePalette } from "../store/colorScale";
import { createWasmTraceClient, type IsophoneTraceClient } from "./isophoneLayer";
import { removeGeoJsonFillLayer, upsertGeoJsonFillLayer } from "./fillOverlay";
import { useDifferenceStore } from "../store/difference";

const DIFFERENCE_SOURCE = "envi-difference";
export const DIFFERENCE_LAYER = "envi-difference-fill";

// The UI-SPEC §2 validated diverging pair (D-16): blue (A quieter) ↔ gray (Δ≈0) ↔
// red (A louder). The brand selection-accent blue is deliberately NOT a pole here
// (avoid confusing UI-selection blue with the data-negative pole blue below).
export const DIFF_NEGATIVE_POLE = "#2a78d6"; // blue — A quieter
export const DIFF_NEUTRAL = "#383835"; // gray dark neutral — Δ ≈ 0
export const DIFF_POSITIVE_POLE = "#d03b3b"; // red — A louder

// The neutral dead-zone half-width (dB): |Δ| ≤ 0.5 renders as the gray midpoint
// (below the just-noticeable difference).
const DEAD_ZONE_DB = 0.5;
// Steps per arm — an ODD total band count (2·ARMS+3) so the midpoint band is EXACTLY
// the gray stop, never an interpolated hue.
const ARMS = 3;

// --- Telemetry (offline UAT observability) --------------------------------

interface DifferenceTelemetry {
  traceCount: number;
  featureCount: number;
  layerType: string | null;
  // The colour rendered at Δ ≈ 0 — MUST be the neutral gray, never a hue or the
  // brand accent (the diverging-midpoint invariant).
  midpointColor: string | null;
  error: string | null;
}

const TELEMETRY: DifferenceTelemetry = {
  traceCount: 0,
  featureCount: 0,
  layerType: null,
  midpointColor: null,
  error: null,
};

export function differenceTelemetry(): DifferenceTelemetry {
  return { ...TELEMETRY };
}

// --- Diverging scale (display-colour math; NO acoustics) -------------------

// The symmetric diverging scale for a delta field: the cap-rounded clamp `M`, the
// ascending break edges (with the ±0.5 dead-zone in the centre), and the class
// colours (odd count ⇒ the midpoint band is exactly the gray neutral).
export interface DivergingScale {
  readonly clampDb: number;
  readonly edges: number[];
  readonly colors: string[];
}

// Round the clamp to the nearest 1 dB (min 1) from the delta's magnitude extent.
function clampFor(delta: readonly number[]): number {
  let maxAbs = 0;
  for (const d of delta) {
    if (Number.isFinite(d)) {
      const a = Math.abs(d);
      if (a > maxAbs) {
        maxAbs = a;
      }
    }
  }
  return Math.max(1, Math.round(maxAbs));
}

// Build the symmetric diverging scale (display math only — Math.abs/round + a palette
// RGB lerp, the SAME class of colour arithmetic `colorScale.ts` does; never a dB).
export function buildDivergingScale(delta: readonly number[]): DivergingScale {
  const clampDb = clampFor(delta);
  const arm = clampDb - DEAD_ZONE_DB;
  const edges: number[] = [];
  // Negative arm: −M … −0.5 (ascending).
  for (let i = ARMS; i >= 1; i -= 1) {
    edges.push(-(DEAD_ZONE_DB + (arm * i) / ARMS));
  }
  edges.push(-DEAD_ZONE_DB, DEAD_ZONE_DB);
  // Positive arm: +0.5 … +M.
  for (let i = 1; i <= ARMS; i += 1) {
    edges.push(DEAD_ZONE_DB + (arm * i) / ARMS);
  }
  // N edges → N+1 bands; the count is odd so the middle band is exactly gray.
  const bandCount = edges.length + 1;
  const colors = samplePalette([DIFF_NEGATIVE_POLE, DIFF_NEUTRAL, DIFF_POSITIVE_POLE], bandCount);
  return { clampDb, edges, colors };
}

// The signed-dB legend rows (highest Δ first, as printed maps read).
export interface DiffLegendClass {
  readonly index: number;
  readonly label: string;
  readonly color: string;
}

export function diffLegendClasses(scale: DivergingScale): DiffLegendClass[] {
  const { edges, colors } = scale;
  const fmt = (v: number): string => `${v > 0 ? "+" : ""}${v.toFixed(1)}`;
  const out: DiffLegendClass[] = [];
  for (let i = 0; i < colors.length; i += 1) {
    let label: string;
    if (i === 0) {
      label = `≤ ${fmt(edges[0])}`;
    } else if (i === colors.length - 1) {
      label = `≥ ${fmt(edges[edges.length - 1])}`;
    } else if (edges[i - 1] === -DEAD_ZONE_DB && edges[i] === DEAD_ZONE_DB) {
      label = "≈ 0 (no change)";
    } else {
      label = `${fmt(edges[i - 1])} … ${fmt(edges[i])}`;
    }
    out.push({ index: i, label, color: colors[i] });
  }
  return out;
}

// --- Trace request assembly (pure) ----------------------------------------

export function buildDifferenceTraceRequest(
  grid: ExportGridDto,
  crs: ExportCrsDto,
  scale: DivergingScale,
  labelA: string,
  labelB: string,
): TraceIsophonesReq {
  return {
    grid,
    crs,
    breaks: contourBreaks(scale.edges),
    band_fills: scale.colors,
    weighting_label: `Δ dB(A) · ${labelA} − ${labelB}`,
  };
}

// --- Imperative layer management ------------------------------------------

let traceClient: IsophoneTraceClient | null = null;
function ensureClient(): IsophoneTraceClient {
  traceClient ??= createWasmTraceClient();
  return traceClient;
}

function applyDifferenceGeoJson(map: MapLibreMap, geojson: GeoJSON.FeatureCollection): void {
  upsertGeoJsonFillLayer(map, DIFFERENCE_SOURCE, DIFFERENCE_LAYER, 0.6, geojson);
  TELEMETRY.featureCount = geojson.features.length;
  TELEMETRY.layerType = map.getLayer(DIFFERENCE_LAYER)?.type ?? null;
}

function removeDifferenceLayer(map: MapLibreMap): void {
  removeGeoJsonFillLayer(map, DIFFERENCE_SOURCE, DIFFERENCE_LAYER);
  TELEMETRY.featureCount = 0;
  TELEMETRY.layerType = null;
}

// The imperative difference layer controller — a child of <Map>. Subscribes to the
// difference store and re-contours the delta field on any change via the WASM tracer
// (the SAME fill-not-raster pipeline as the isophone layer, D-02).
export function DifferenceLayer(): null {
  const map = useMap();
  const delta = useDifferenceStore((s) => s.delta);
  const grid = useDifferenceStore((s) => s.grid);
  const crs = useDifferenceStore((s) => s.crs);
  const labelA = useDifferenceStore((s) => s.labelA);
  const labelB = useDifferenceStore((s) => s.labelB);
  const gen = useRef(0);

  useEffect(() => {
    const instance = map.current?.getMap() as unknown as MapLibreMap | undefined;
    if (!instance) {
      return;
    }

    const recontour = (): void => {
      // Bump the generation on EVERY run (including a clear) so any pending deferred
      // apply from an earlier trace is superseded — a clear must invalidate an
      // in-flight/queued re-add, not just a newer trace (WR-03).
      const myGen = (gen.current += 1);
      if (!delta || !grid || !crs) {
        removeDifferenceLayer(instance);
        return;
      }
      const scale = buildDivergingScale(delta);
      // The midpoint band's colour (must be the neutral gray, never a hue).
      const midIndex = Math.floor(scale.colors.length / 2);
      TELEMETRY.midpointColor = scale.colors[midIndex] ?? null;
      const req = buildDifferenceTraceRequest(grid, crs, scale, labelA, labelB);
      TELEMETRY.traceCount += 1;
      ensureClient()
        .trace(req)
        .then((json) => {
          if (myGen !== gen.current) {
            return;
          }
          const fc = JSON.parse(json) as GeoJSON.FeatureCollection;
          if (instance.isStyleLoaded()) {
            applyDifferenceGeoJson(instance, fc);
          } else {
            // Re-check the generation when the deferred `load` finally fires so a queued
            // callback cannot re-add a superseded delta fill after a clear (WR-03).
            instance.once("load", () => {
              if (myGen !== gen.current) {
                return;
              }
              applyDifferenceGeoJson(instance, fc);
            });
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
  }, [map, delta, grid, crs, labelA, labelB]);

  return null;
}

// --- The docked legend (signed dB) ----------------------------------------

// The docked difference legend: one row per diverging class (swatch + signed-dB
// range), highest Δ first. Renders nothing when no difference is cached.
export function DifferenceLegend(): ReactElement | null {
  const delta = useDifferenceStore((s) => s.delta);
  const labelA = useDifferenceStore((s) => s.labelA);
  const labelB = useDifferenceStore((s) => s.labelB);
  // Rebuild the diverging scale + its legend rows only when the delta field changes, not on
  // every unrelated re-render.
  const scale = useMemo(() => (delta ? buildDivergingScale(delta) : null), [delta]);
  const classes = useMemo(() => (scale ? diffLegendClasses(scale) : []), [scale]);

  if (!delta || !scale) {
    return null;
  }

  return createElement(
    "div",
    {
      className: "isophone-legend difference-legend",
      "data-testid": "difference-legend",
      "data-band-count": classes.length,
      "data-clamp": scale.clampDb,
    },
    createElement(
      "div",
      { className: "isophone-legend-head mono" },
      `Δ dB(A): ${labelA} − ${labelB}`,
    ),
    createElement(
      "ul",
      { className: "isophone-legend-list" },
      [...classes].reverse().map((c) =>
        createElement(
          "li",
          {
            key: c.index,
            className: "isophone-legend-row mono",
            "data-testid": `difference-legend-row-${c.index}`,
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
