// useTerraDraw.ts — the Gate-1 Terra Draw lifecycle hook: creates ONE Terra Draw instance bound to the
// react-map-gl map, held in a ref, and keeps the canonical Zustand store (D-03) as the source of truth.
//
// # Module I/O
// - Input  the react-map-gl map via `useMap()`; the canonical store (`useSceneStore`); the basemap
//   config (`DARK`/`ALT` styles + attribution).
// - Output an instance-in-ref Terra Draw wired so: user edits → store (`applyTerraDrawChange`), the
//   store's own `addFeatures` echoes (`change` context.origin === "api") are IGNORED (no feedback
//   loop), a committed edit fires `finish` (D-04 autosave lands in 07-09 — stubbed here), and after
//   `map.setStyle()` the features re-hydrate from the store on the SINGLE `style.load` hook (SC4 — not
//   the repeatedly-firing per-tile style-data event). Returns `{ switchBasemap, addTestFeature }` (the
//   latter two are temporary Gate-1
//   affordances the E2E drives; real per-kind drawing arrives in 07-07). Every imperative subscription
//   (map `.on`, TD `.on`, AttributionControl) is torn down in the effect cleanup (T-07-06-03).
// - Valid input range: StrictMode double-mounts the effect in dev — the `drawRef` guard + `draw.stop()`
//   cleanup guarantee exactly ONE live instance.

import { useCallback, useEffect, useRef, useState } from "react";
import { useMap } from "react-map-gl/maplibre";
import type { Map as MapLibreMap } from "maplibre-gl";
import {
  TerraDraw,
  TerraDrawLineStringMode,
  TerraDrawPointMode,
  TerraDrawPolygonMode,
  TerraDrawSelectMode,
  type GeoJSONStoreFeatures,
  type TerraDrawEventListeners,
} from "terra-draw";
import { TerraDrawMapLibreGLAdapter } from "terra-draw-maplibre-gl-adapter";

import { ALT_BASEMAP_STYLE, DARK_BASEMAP_STYLE, attachOsmAttribution } from "./basemap";
import { useSceneStore } from "../store/sceneStore";

export interface TerraDrawController {
  // Toggle the basemap style — exercises `map.setStyle()` + `style.load` re-hydration (Gate-1/SC4).
  switchBasemap(): void;
  // Seed one canonical point feature (store + TD) — the temporary affordance the Gate-1 E2E drives to
  // prove store-canonical persistence across a basemap switch. Superseded by real drawing in 07-07.
  addTestFeature(): void;
  // Live Terra Draw render count (getSnapshot length), reflected for the E2E.
  tdFeatureCount: number;
  // True once the map has loaded and the instance is built (E2E readiness gate).
  ready: boolean;
}

// Re-add the store's canonical features into Terra Draw. Fires `change` with origin:"api" (ignored by
// the change handler), so this never writes back into the store — the loop is broken by design (D-03).
function rehydrate(draw: TerraDraw): void {
  const features = useSceneStore.getState().terraDrawFeatures();
  // `map.setStyle()` wiped the adapter's rendered source/layers but TD's internal store still holds the
  // features; clear it first so re-adding the same ids does not collide, then re-add (the adapter
  // re-creates its source/layers on the new style during the resulting render). Research Pattern 2.
  try {
    draw.clear();
    if (features.length > 0) {
      draw.addFeatures(features);
    }
  } catch {
    // Defensive: if the adapter could not re-render onto the fresh style, rebuild its layers via a
    // stop/start cycle and re-add. Keeps SC4 robust without leaving the scene invisible.
    try {
      draw.stop();
      draw.start();
      draw.clear();
      if (features.length > 0) {
        draw.addFeatures(features);
      }
    } catch {
      /* leave TD empty rather than throw during a style swap */
    }
  }
}

export function useTerraDraw(): TerraDrawController {
  const drawRef = useRef<TerraDraw | null>(null);
  const altStyleRef = useRef(false);
  const map = useMap();
  const mapRef = map.current;
  const [tdFeatureCount, setTdFeatureCount] = useState(0);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    // react-map-gl's MapRef.getMap() returns its MapInstance; structurally it is the maplibre-gl Map
    // (the single boundary cast for the whole hook). setStyle/on/once/addControl live on it.
    const instance = mapRef?.getMap() as unknown as MapLibreMap | undefined;
    if (!instance) {
      return;
    }

    let disposed = false;

    // Reflect TD's rendered feature count on every change (any origin) — the E2E observes it to prove
    // re-hydration re-populated Terra Draw after a basemap switch.
    const onChange: TerraDrawEventListeners["change"] = (ids, type, context) => {
      const snapshot = drawRef.current?.getSnapshot() ?? [];
      setTdFeatureCount(snapshot.length);
      if (context && "origin" in context && context.origin === "api") {
        return; // our own addFeatures echo — do NOT write back (no feedback loop, D-03)
      }
      useSceneStore.getState().applyTerraDrawChange(ids, type, snapshot);
    };

    // Committed-edit trigger (D-04). Autosave scheduling lands in 07-09; here it only marks dirty so the
    // trigger is proven to be `finish` (drag RELEASE), never `change` (every drag frame).
    const onFinish: TerraDrawEventListeners["finish"] = () => {
      useSceneStore.getState().markDirty();
    };

    const build = (): void => {
      if (disposed || drawRef.current) {
        return; // StrictMode double-mount / already built — exactly one live instance
      }
      const draw = new TerraDraw({
        adapter: new TerraDrawMapLibreGLAdapter({ map: instance }),
        modes: [
          new TerraDrawSelectMode(),
          new TerraDrawPointMode(),
          new TerraDrawLineStringMode(),
          new TerraDrawPolygonMode(),
        ],
      });
      draw.start();
      draw.on("change", onChange);
      draw.on("finish", onFinish);
      drawRef.current = draw;
      useSceneStore.getState().noteDrawBuilt();
      // Hydrate any features the store already holds (e.g. a reopened project in a later plan).
      const existing = useSceneStore.getState().terraDrawFeatures();
      if (existing.length > 0) {
        draw.addFeatures(existing);
      }
      setReady(true);
    };

    // Re-hydrate after a basemap switch: setStyle destroys sources/layers, so re-add from the store on
    // the SINGLE style.load hook (not the repeatedly-firing per-tile style-data event).
    const onStyleLoad = (): void => {
      const draw = drawRef.current;
      if (draw) {
        rehydrate(draw);
      }
    };

    if (instance.isStyleLoaded()) {
      build();
    } else {
      instance.once("load", build);
    }
    instance.on("style.load", onStyleLoad);
    const detachAttribution = attachOsmAttribution(instance);

    return () => {
      disposed = true;
      instance.off("style.load", onStyleLoad);
      instance.off("load", build);
      detachAttribution();
      const draw = drawRef.current;
      if (draw) {
        draw.off("change", onChange);
        draw.off("finish", onFinish);
        draw.stop();
        useSceneStore.getState().noteDrawStopped();
      }
      drawRef.current = null;
    };
  }, [mapRef]);

  const switchBasemap = useCallback((): void => {
    const instance = mapRef?.getMap() as unknown as MapLibreMap | undefined;
    if (!instance) {
      return;
    }
    altStyleRef.current = !altStyleRef.current;
    // style.load (registered in the effect) re-hydrates the scene from the store once the new style
    // has parsed. Switching between the network dark style and the inline dark fallback both fire it.
    instance.setStyle(altStyleRef.current ? ALT_BASEMAP_STYLE : DARK_BASEMAP_STYLE);
  }, [mapRef]);

  const addTestFeature = useCallback((): void => {
    const draw = drawRef.current;
    if (!draw) {
      return;
    }
    const id = crypto.randomUUID();
    const feature = {
      id,
      type: "Feature",
      geometry: { type: "Point", coordinates: [4.9041, 52.3676] },
      properties: { mode: "point" },
    } as unknown as GeoJSONStoreFeatures;
    // Write the canonical store directly (this is the user-intent path); then render in TD. The TD add
    // fires `change` with origin:"api", which the handler ignores for the store — so the store is not
    // double-written (proving the loop guard even on the seed path).
    useSceneStore.getState().applyTerraDrawChange([id], "create", [feature]);
    draw.addFeatures([feature]);
  }, []);

  return { switchBasemap, addTestFeature, tdFeatureCount, ready };
}
