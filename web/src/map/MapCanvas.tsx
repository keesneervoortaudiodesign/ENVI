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
import type { Map as MapLibreMap } from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";

import { DARK_BASEMAP_STYLE } from "./basemap";
import { useTerraDraw } from "./useTerraDraw";
import { useSceneStore } from "../store/sceneStore";

// The Terra Draw controller + Gate-1 scene overlay. Rendered as a child of <Map> so `useMap()` (used by
// the hook) resolves to this map instance.
function SceneOverlay(): ReactElement {
  const { switchBasemap, addTestFeature, tdFeatureCount, ready } = useTerraDraw();
  const storeFeatureCount = useSceneStore((s) => Object.keys(s.features).length);
  const drawInstancesLive = useSceneStore((s) => s.drawInstancesLive);
  const rehydrations = useSceneStore((s) => s.rehydrations);

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
      </dl>
    </div>
  );
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
      <SceneOverlay />
    </Map>
  );
}
