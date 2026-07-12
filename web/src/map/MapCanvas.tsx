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

import { useEffect, type ReactElement } from "react";
import { Map, useMap } from "react-map-gl/maplibre";
import type { LngLatBoundsLike, Map as MapLibreMap } from "maplibre-gl";
import type { GeoJSONStoreFeatures } from "terra-draw";
import "maplibre-gl/dist/maplibre-gl.css";

import { DARK_BASEMAP_STYLE } from "./basemap";
import { useTerraDraw } from "./useTerraDraw";
import { DgmOverlay } from "./DgmOverlay";
import { ImpedanceOverlay } from "./impedanceOverlay";
import { IsophoneLayer, MapLegend } from "./isophoneLayer";
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
      <DgmOverlay />
      <ImpedanceOverlay />
      <ReceiverGridOverlay />
      <ImpedanceSegOverlay />
      <ScreenVertexOverlay />
      <SceneOverlay />
    </Map>
  );
}
