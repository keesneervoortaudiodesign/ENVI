// MapCanvas.tsx — the react-map-gl map surface for the ENVI scene editor (WEB-01). Renders the
// dark-vector basemap, drives the Gate-1 Terra Draw lifecycle, and surfaces the store-canonical state
// the E2E asserts against.
//
// # Module I/O
// - Input  none (reads the canonical store internally). The basemap style comes from `basemap.ts`.
// - Output the `<Map>` filling the shell's map slot with an OSM AttributionControl (attached in the
//   hook), a Terra Draw controlled view, and temporary Gate-1 controls ("switch basemap" to exercise
//   setStyle/style.load, "add test point" to seed the store) plus DOM readouts (store feature count, TD
//   render count, live TerraDraw instance count) that the offline E2E reads. The temporary controls +
//   readouts are removed once real drawing/inspector UI lands in 07-07+.
// - Valid input range: the basemap network fetch is Playwright-intercepted so tests run fully offline.

import { useEffect, useRef, type ReactElement } from "react";
import { Map, useMap } from "react-map-gl/maplibre";
import type {
  ExpressionSpecification,
  GeoJSONSource,
  LngLatBoundsLike,
  Map as MapLibreMap,
} from "maplibre-gl";
import type { GeoJSONStoreFeatures } from "terra-draw";
import "maplibre-gl/dist/maplibre-gl.css";

import { DARK_BASEMAP_STYLE } from "./basemap";
import { useTerraDraw } from "./useTerraDraw";
import { DgmOverlay } from "./DgmOverlay";
import { ImpedanceOverlay } from "./impedanceOverlay";
import { IsophoneLayer, ISOPHONE_LAYER, MapLegend } from "./isophoneLayer";
import { DifferenceLayer, DifferenceLegend } from "./differenceLayer";
import {
  hatchImageId,
  objectStyles,
  pointIconId,
  setObjectLayerTelemetry,
  type AreaStyle,
  type KindDisplayStyle,
  type LineStyle,
  type PointStyle,
} from "./objectStyles";
import { hatchPattern, pointMarker } from "./hatchPatterns";
import { KINDS, type Kind } from "../draw/kinds";
import {
  ReceiverGridOverlay,
  ImpedanceSegOverlay,
  ScreenVertexOverlay,
} from "./weatherOverlay";
import { useDgmTrigger } from "../dgm/dgmTrigger";
import { useSceneStore } from "../store/sceneStore";
import { useDgmStore } from "../store/dgm";
import { useImportStore } from "../store/import";
import { evaluateGuardrail } from "../import/importJob";
import { attachImportAttribution } from "../import/attribution";

// The Terra Draw controller + Gate-1 scene overlay. Rendered as a child of <Map> so `useMap()` (used by
// the hook) resolves to this map instance.
function SceneOverlay(): ReactElement {
  const { switchBasemap, addTestFeature, tdFeatureCount, ready } = useTerraDraw();
  // The SC1 DGM producer: committed elevation edits → debounced POST /dgm/triangulate → dgm slice.
  useDgmTrigger();
  const storeFeatureCount = useSceneStore((s) => Object.keys(s.features).length);
  const drawInstancesLive = useSceneStore((s) => s.drawInstancesLive);
  const rehydrations = useSceneStore((s) => s.rehydrations);
  const tinTriangles = useDgmStore((s) => s.triangulation?.triangles.length ?? 0);
  const dgmReject = useDgmStore((s) => s.rejectReason);
  const zoomTarget = useSceneStore((s) => s.zoomRequest?.featureId ?? null);

  return (
    <div className="map-overlay">
      <div className="map-controls">
        <button
          type="button"
          className="btn"
          data-testid="switch-basemap"
          onClick={switchBasemap}
          disabled={!ready}
        >
          Switch basemap
        </button>
        <button
          type="button"
          className="btn"
          data-testid="add-test-point"
          onClick={addTestFeature}
          disabled={!ready}
        >
          Add test point
        </button>
      </div>
      {/* Store-canonical readouts the offline E2E asserts on (Gate-1 observability). */}
      <dl className="map-readout mono">
        <div>
          <dt>ready</dt>
          <dd data-testid="map-ready">{ready ? "yes" : "no"}</dd>
        </div>
        <div>
          <dt>store</dt>
          <dd data-testid="store-feature-count">{storeFeatureCount}</dd>
        </div>
        <div>
          <dt>td</dt>
          <dd data-testid="td-feature-count">{tdFeatureCount}</dd>
        </div>
        <div>
          <dt>instances</dt>
          <dd data-testid="td-instances-live">{drawInstancesLive}</dd>
        </div>
        <div>
          <dt>rehydrations</dt>
          <dd data-testid="rehydration-count">{rehydrations}</dd>
        </div>
        <div>
          <dt>tin</dt>
          <dd data-testid="dgm-triangle-count">{tinTriangles}</dd>
        </div>
        <div>
          <dt>dgm-reject</dt>
          <dd data-testid="dgm-reject">{dgmReject ? String(dgmReject.status) : "none"}</dd>
        </div>
        <div>
          <dt>zoom</dt>
          <dd data-testid="zoom-target">{zoomTarget ?? "none"}</dd>
        </div>
      </dl>
    </div>
  );
}

// The [w, s, e, n] bounds of a feature's geometry (validation click-to-zoom + reject "zoom to conflict"),
// or null if it has no usable coordinates. A degenerate (point / zero-area) extent is padded so
// `fitBounds` still produces a sensible camera rather than throwing.
function featureBounds(feature: GeoJSONStoreFeatures | undefined): LngLatBoundsLike | null {
  const geometry = feature?.geometry;
  if (!geometry) {
    return null;
  }
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  const visit = (coords: unknown): void => {
    if (typeof coords === "number") {
      return;
    }
    if (Array.isArray(coords) && typeof coords[0] === "number" && typeof coords[1] === "number") {
      const [x, y] = coords as [number, number];
      minX = Math.min(minX, x);
      minY = Math.min(minY, y);
      maxX = Math.max(maxX, x);
      maxY = Math.max(maxY, y);
      return;
    }
    if (Array.isArray(coords)) {
      for (const c of coords) {
        visit(c);
      }
    }
  };
  visit((geometry as { coordinates?: unknown }).coordinates);
  if (!Number.isFinite(minX) || !Number.isFinite(minY)) {
    return null;
  }
  const pad = maxX === minX && maxY === minY ? 0.0008 : 0; // a lone point → a small window
  return [
    [minX - pad, minY - pad],
    [maxX + pad, maxY + pad],
  ];
}

// Fit the map to a requested feature's bounds whenever `zoomRequest` changes (store-canonical zoom, kept
// off the panel/banner components so those never touch the map instance). Torn down implicitly by effect.
function ZoomController(): null {
  const map = useMap();
  const zoomRequest = useSceneStore((s) => s.zoomRequest);
  useEffect(() => {
    if (!zoomRequest) {
      return;
    }
    const instance = map.current?.getMap() as unknown as MapLibreMap | undefined;
    const feature = useSceneStore.getState().features[zoomRequest.featureId];
    const bounds = featureBounds(feature);
    if (!instance || !bounds) {
      return;
    }
    instance.fitBounds(bounds, { padding: 64, duration: 300, maxZoom: 18 });
  }, [zoomRequest, map]);
  return null;
}

// Track the map viewport into the import store (D-06: import region = current viewport) and keep the
// max-area guardrail live as the user pans/zooms. Updates on load + every `moveend`; torn down on unmount.
function ViewportTracker(): null {
  const map = useMap();
  const setViewport = useImportStore((s) => s.setViewport);
  const setGuardrail = useImportStore((s) => s.setGuardrail);
  useEffect(() => {
    const instance = map.current?.getMap() as unknown as MapLibreMap | undefined;
    if (!instance) {
      return;
    }
    const update = (): void => {
      const b = instance.getBounds();
      const bbox = {
        min_lon: b.getWest(),
        min_lat: b.getSouth(),
        max_lon: b.getEast(),
        max_lat: b.getNorth(),
      };
      setViewport(bbox);
      setGuardrail(evaluateGuardrail(bbox));
    };
    if (instance.isStyleLoaded()) {
      update();
    } else {
      instance.once("load", update);
    }
    instance.on("moveend", update);
    return () => {
      instance.off("moveend", update);
      instance.off("load", update);
    };
  }, [map, setViewport, setGuardrail]);
  return null;
}

// Sync a MapLibre AttributionControl with the SC5 credits for the sources that have contributed imported
// features (recreated when the set changes). Torn down in the effect cleanup (T-07-06-03).
function ImportAttribution(): null {
  const map = useMap();
  const sources = useImportStore((s) => s.attributedSources);
  useEffect(() => {
    const instance = map.current?.getMap() as unknown as MapLibreMap | undefined;
    if (!instance) {
      return;
    }
    return attachImportAttribution(instance, sources);
  }, [map, sources]);
  return null;
}

// A one-shot resize nudge: the map mounts into a flex/grid slot whose final size settles after first
// paint; without this the canvas can render at 0px. Torn down on unmount (subscription discipline).
function ResizeOnMount(): null {
  const map = useMap();
  useEffect(() => {
    const instance = map.current?.getMap() as unknown as MapLibreMap | undefined;
    if (!instance) {
      return;
    }
    const raf = requestAnimationFrame(() => instance.resize());
    return () => cancelAnimationFrame(raf);
  }, [map]);
  return null;
}

// --- Scene-object DISPLAY layers (D-17/D-18/D-19) -------------------------------------------------------
//
// The NoizCalc §4.6.3 display styling for the scene objects, rendered as MapLibre fill/line/symbol layers
// driven by `objectStyles.ts` and sitting ABOVE the 11-06 isophone fill (D-18). This is the DISPLAY system
// only — it reads the canonical store FeatureCollection and NEVER touches Terra Draw's draw-time
// styling/validation (`useTerraDraw.ts` is untouched by this plan — D-19 / Pitfall 8). Draw order within the
// group: areas (translucent fill → hatch → border) → lines → point markers (top).

const SCENE_OBJECT_SOURCE = "envi-scene-objects";
// Every object display layer id starts with this prefix so the isophone + difference fills insert BELOW
// them (their `SCENE_LAYER_PREFIXES` include `envi-object`), keeping objects on top (D-18).
const OBJ_AREA_FILL = "envi-object-area-fill";
const OBJ_AREA_HATCH = "envi-object-area-hatch";
const OBJ_AREA_BORDER = "envi-object-area-border";
const OBJ_AREA_BORDER_DASHED = "envi-object-area-border-dashed";
const OBJ_LINE = "envi-object-line";
const OBJ_LINE_DASHED = "envi-object-line-dashed";
const OBJ_POINT = "envi-object-point";
const OBJ_LAYER_IDS = [
  OBJ_AREA_FILL,
  OBJ_AREA_HATCH,
  OBJ_AREA_BORDER,
  OBJ_AREA_BORDER_DASHED,
  OBJ_LINE,
  OBJ_LINE_DASHED,
  OBJ_POINT,
];

const areaKinds = KINDS.filter((k): k is Kind => objectStyles[k].geometry === "area");
const lineKinds = KINDS.filter((k): k is Kind => objectStyles[k].geometry === "line");
const pointKinds = KINDS.filter((k): k is Kind => objectStyles[k].geometry === "point");

// A MapLibre `match` expression on `properties.kind`, one arm per kind in `kinds`, with `fallback` for any
// feature whose kind is absent/unknown. `value(style, kind)` extracts the per-kind paint value from
// `objectStyles` (the style subtype `S` is inferred from the extractor's annotated parameter).
function kindMatch<S extends KindDisplayStyle, T>(
  kinds: Kind[],
  value: (style: S, kind: Kind) => T,
  fallback: T,
): ExpressionSpecification {
  const arms: (string | T)[] = [];
  for (const kind of kinds) {
    arms.push(kind, value(objectStyles[kind] as S, kind));
  }
  return ["match", ["get", "kind"], ...arms, fallback] as unknown as ExpressionSpecification;
}

// Register (idempotently) the runtime hatch + point-marker rasters as MapLibre images. `map.addImage`
// accepts the `{ width, height, data }` raster shape directly — no DOM canvas needed (offline-safe).
function ensureObjectImages(map: MapLibreMap): string[] {
  const ids: string[] = [];
  for (const kind of areaKinds) {
    const id = hatchImageId(kind);
    if (!map.hasImage(id)) {
      map.addImage(id, hatchPattern(kind), { pixelRatio: 2 });
    }
    ids.push(id);
  }
  for (const kind of pointKinds) {
    const id = pointIconId(kind);
    if (!map.hasImage(id)) {
      map.addImage(id, pointMarker(kind), { pixelRatio: 2 });
    }
    ids.push(id);
  }
  return ids;
}

// Add the object source + the fill/line/symbol layers in the D-18 draw order (all ABOVE the isophone fill,
// appended to the top of the style so they sit over every data fill). Idempotent: skips layers already
// present (a basemap `style.load` rebuild re-adds after `setStyle` destroyed them).
function ensureObjectLayers(map: MapLibreMap, data: GeoJSON.FeatureCollection): void {
  if (!map.getSource(SCENE_OBJECT_SOURCE)) {
    map.addSource(SCENE_OBJECT_SOURCE, { type: "geojson", data });
  }
  const dashedLineKinds = lineKinds.filter((k) => (objectStyles[k] as LineStyle).dash !== null);
  const solidLineKinds = lineKinds.filter((k) => (objectStyles[k] as LineStyle).dash === null);

  if (!map.getLayer(OBJ_AREA_FILL)) {
    map.addLayer({
      id: OBJ_AREA_FILL,
      type: "fill",
      source: SCENE_OBJECT_SOURCE,
      filter: ["==", ["geometry-type"], "Polygon"],
      paint: {
        "fill-color": kindMatch(areaKinds, (s: AreaStyle) => s.color, "rgba(0,0,0,0)"),
        "fill-opacity": kindMatch(areaKinds, (s: AreaStyle) => s.fillOpacity, 0),
      },
    });
  }
  if (!map.getLayer(OBJ_AREA_HATCH)) {
    map.addLayer({
      id: OBJ_AREA_HATCH,
      type: "fill",
      source: SCENE_OBJECT_SOURCE,
      filter: ["==", ["geometry-type"], "Polygon"],
      paint: { "fill-pattern": kindMatch(areaKinds, (_s: AreaStyle, kind) => hatchImageId(kind), "") },
    });
  }
  if (!map.getLayer(OBJ_AREA_BORDER)) {
    map.addLayer({
      id: OBJ_AREA_BORDER,
      type: "line",
      source: SCENE_OBJECT_SOURCE,
      filter: ["all", ["==", ["geometry-type"], "Polygon"], ["!=", ["get", "kind"], "calc_area"]],
      paint: {
        "line-color": kindMatch(areaKinds, (s: AreaStyle) => s.border, "rgba(0,0,0,0)"),
        "line-width": kindMatch(areaKinds, (s: AreaStyle) => s.borderWidth, 1),
      },
    });
  }
  // calc_area's DASHED frame (§4.6.3 "dashed border") — its own layer (line-dasharray is not data-driven).
  if (!map.getLayer(OBJ_AREA_BORDER_DASHED)) {
    const ca = objectStyles.calc_area as AreaStyle;
    map.addLayer({
      id: OBJ_AREA_BORDER_DASHED,
      type: "line",
      source: SCENE_OBJECT_SOURCE,
      filter: ["all", ["==", ["geometry-type"], "Polygon"], ["==", ["get", "kind"], "calc_area"]],
      paint: {
        "line-color": ca.border,
        "line-width": ca.borderWidth,
        "line-dasharray": [...(ca.borderDash ?? [3, 2])],
      },
    });
  }
  if (!map.getLayer(OBJ_LINE)) {
    map.addLayer({
      id: OBJ_LINE,
      type: "line",
      source: SCENE_OBJECT_SOURCE,
      filter: [
        "all",
        ["==", ["geometry-type"], "LineString"],
        ["in", ["get", "kind"], ["literal", solidLineKinds]],
      ],
      layout: { "line-cap": "round", "line-join": "round" },
      paint: {
        "line-color": kindMatch(lineKinds, (s: LineStyle) => s.color, "rgba(0,0,0,0)"),
        "line-width": kindMatch(lineKinds, (s: LineStyle) => s.width, 2),
      },
    });
  }
  // Dashed lines (elevation_line) — a dedicated layer for the same non-data-driven-dasharray reason.
  if (!map.getLayer(OBJ_LINE_DASHED) && dashedLineKinds.length > 0) {
    const el = objectStyles.elevation_line as LineStyle;
    map.addLayer({
      id: OBJ_LINE_DASHED,
      type: "line",
      source: SCENE_OBJECT_SOURCE,
      filter: [
        "all",
        ["==", ["geometry-type"], "LineString"],
        ["in", ["get", "kind"], ["literal", dashedLineKinds]],
      ],
      paint: {
        "line-color": kindMatch(lineKinds, (s: LineStyle) => s.color, "rgba(0,0,0,0)"),
        "line-width": kindMatch(lineKinds, (s: LineStyle) => s.width, 2),
        "line-dasharray": [...(el.dash ?? [2, 2])],
      },
    });
  }
  if (!map.getLayer(OBJ_POINT)) {
    map.addLayer({
      id: OBJ_POINT,
      type: "symbol",
      source: SCENE_OBJECT_SOURCE,
      filter: ["==", ["geometry-type"], "Point"],
      layout: {
        "icon-image": kindMatch(pointKinds, (_s: PointStyle, kind) => pointIconId(kind), ""),
        // The generated marker raster is 24 px at pixelRatio 2 → 12 CSS px; scale to the style `size`.
        "icon-size": kindMatch(pointKinds, (s: PointStyle) => s.size / 12, 1),
        "icon-allow-overlap": true,
        "icon-ignore-placement": true,
      },
    });
  }
}

// Refresh the object-layer telemetry snapshot (the offline UAT reads it to prove the layers exist + sit
// ABOVE the isophone fill in the style draw order — D-18).
function refreshObjectTelemetry(map: MapLibreMap, featureCount: number, imageIds: string[]): void {
  const order = (map.getStyle()?.layers ?? []).map((l) => l.id);
  const registered = OBJ_LAYER_IDS.filter((id) => map.getLayer(id));
  const isoIdx = order.indexOf(ISOPHONE_LAYER);
  const aboveIsophone =
    isoIdx < 0 ? null : registered.every((id) => order.indexOf(id) > isoIdx);
  setObjectLayerTelemetry({
    registeredLayers: registered,
    registeredImages: imageIds,
    featureCount,
    layerOrder: order,
    aboveIsophone,
  });
}

// The imperative scene-object display-layer controller — a child of <Map>. Subscribes to the canonical
// store FeatureCollection and (re-)registers the images + fill/line/symbol layers on load + after a basemap
// `style.load` (setStyle destroys sources/layers). NEVER edits Terra Draw (D-19). Returns null.
function SceneObjectLayers(): null {
  const map = useMap();
  const features = useSceneStore((s) => s.features);
  // Keep the latest FeatureCollection available to the style.load handler without re-registering it.
  const fcRef = useRef<GeoJSON.FeatureCollection>({ type: "FeatureCollection", features: [] });
  fcRef.current = {
    type: "FeatureCollection",
    features: Object.values(features) as unknown as GeoJSON.Feature[],
  };

  useEffect(() => {
    const instance = map.current?.getMap() as unknown as MapLibreMap | undefined;
    if (!instance) {
      return;
    }
    const apply = (): void => {
      try {
        const imageIds = ensureObjectImages(instance);
        ensureObjectLayers(instance, fcRef.current);
        const src = instance.getSource(SCENE_OBJECT_SOURCE) as GeoJSONSource | undefined;
        src?.setData(fcRef.current);
        refreshObjectTelemetry(instance, fcRef.current.features.length, imageIds);
      } catch {
        /* style momentarily torn down (mid basemap switch) — style.load re-applies */
      }
    };
    const onStyleLoad = (): void => apply();
    if (instance.isStyleLoaded()) {
      apply();
    } else {
      instance.once("load", apply);
    }
    instance.on("style.load", onStyleLoad);
    return () => {
      instance.off("style.load", onStyleLoad);
      instance.off("load", apply);
    };
  }, [map, features]);

  return null;
}

export function MapCanvas(): ReactElement {
  return (
    <Map
      initialViewState={{ longitude: 4.9041, latitude: 52.3676, zoom: 12 }}
      mapStyle={DARK_BASEMAP_STYLE}
      attributionControl={false}
      reuseMaps
      style={{ position: "absolute", inset: 0 }}
    >
      <ResizeOnMount />
      <ZoomController />
      <ViewportTracker />
      <ImportAttribution />
      {/* Isophone FILL layer (D-02) below the scene objects (D-18) + docked legend. */}
      <IsophoneLayer />
      <MapLegend />
      {/* Scenario DIFFERENCE fill layer (diverging A − B, D-16) + its signed-dB legend. */}
      <DifferenceLayer />
      <DifferenceLegend />
      {/* Scene objects at FULL styling ON TOP of the isophone/difference fills (D-17/D-18). */}
      <SceneObjectLayers />
      <DgmOverlay />
      <ImpedanceOverlay />
      <ReceiverGridOverlay />
      <ImpedanceSegOverlay />
      <ScreenVertexOverlay />
      <SceneOverlay />
    </Map>
  );
}
