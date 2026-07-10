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
  type GeoJSONStoreFeatures,
  type TerraDrawEventListeners,
} from "terra-draw";
import { TerraDrawMapLibreGLAdapter } from "terra-draw-maplibre-gl-adapter";

import { ALT_BASEMAP_STYLE, DARK_BASEMAP_STYLE, attachOsmAttribution } from "./basemap";
import { useSceneStore } from "../store/sceneStore";
import { buildModes, tdModeName } from "../draw/modes";
import { isKind } from "../draw/kinds";

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

export function useTerraDraw(): TerraDrawController {
  const drawRef = useRef<TerraDraw | null>(null);
  const altStyleRef = useRef(false);
  const map = useMap();
  const mapRef = map.current;
  const [tdFeatureCount, setTdFeatureCount] = useState(0);
  const [ready, setReady] = useState(false);
  const activeTool = useSceneStore((s) => s.activeTool);

  // Track the palette tool: switch the live Terra Draw instance to the matching geometry mode (07-07).
  // Runs after the build effect (drawRef populated); a no-op until the instance exists.
  useEffect(() => {
    drawRef.current?.setMode(tdModeName(activeTool));
  }, [activeTool]);

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
      const store = useSceneStore.getState();
      const known = new Set(Object.keys(store.features));
      store.applyTerraDrawChange(ids, type, snapshot);
      // Tag every NEWLY-created feature with the active kind (`properties.kind`) + seed last-object
      // inheritance (WEB-04). Detected by "not previously in the store" so it is robust to TD's `type`
      // string. When the pointer tool is active, a drawn shape stays untagged (select-mode edits).
      const active = store.activeTool;
      if (isKind(active)) {
        const tagger = useSceneStore.getState();
        for (const raw of ids) {
          const id = String(raw);
          if (!known.has(id) && tagger.features[id]) {
            if (active === "ground_zone") {
              // Draw-time topology check (D-07): a partial cross is hard-rejected — the store reverts the
              // geometry and raises the transient banner, so remove the reverted feature from TD's view.
              const outcome = tagger.commitGroundZoneCandidate(id);
              if (outcome === "partial-cross") {
                drawRef.current?.removeFeatures([id]);
              }
            } else {
              tagger.tagCreatedFeature(id, active);
            }
          }
        }
      }
    };

    // Committed-edit trigger (D-04): a drag RELEASE (never the per-frame `change`). `noteCommit` bumps the
    // committed-edit epoch that autosave keys off (07-09), so a released vertex drag schedules ONE PUT.
    const onFinish: TerraDrawEventListeners["finish"] = () => {
      useSceneStore.getState().noteCommit();
    };

    // Create ONE Terra Draw instance bound to the current map style, wire its handlers, and re-add the
    // store's canonical features. A fresh adapter registers its source/layers onto whatever style is
    // live now — which is exactly why a basemap switch rebuilds rather than reuses (see onStyleLoad).
    const buildDraw = (): void => {
      if (disposed || drawRef.current) {
        return; // StrictMode double-mount / already built — exactly one live instance
      }
      const draw = new TerraDraw({
        adapter: new TerraDrawMapLibreGLAdapter({ map: instance }),
        modes: buildModes(),
      });
      draw.start();
      draw.setMode(tdModeName(useSceneStore.getState().activeTool));
      draw.on("change", onChange);
      draw.on("finish", onFinish);
      drawRef.current = draw;
      useSceneStore.getState().noteDrawBuilt();
      // Re-add the canonical features (initial mount OR after a basemap switch). addFeatures fires
      // `change` with origin:"api" (ignored for the store), so this never writes back — no loop (D-03).
      const existing = useSceneStore.getState().terraDrawFeatures();
      if (existing.length > 0) {
        draw.addFeatures(existing);
      }
      setTdFeatureCount(draw.getSnapshot().length);
      setReady(true);
    };

    const teardownDraw = (): void => {
      const draw = drawRef.current;
      if (!draw) {
        return;
      }
      draw.off("change", onChange);
      draw.off("finish", onFinish);
      // stop() may touch layers already destroyed by setStyle — guard so cleanup never throws.
      try {
        draw.stop();
      } catch {
        /* style already torn down */
      }
      useSceneStore.getState().noteDrawStopped();
      drawRef.current = null;
    };

    // `map.setStyle()` destroys every source and layer — including the Terra Draw adapter's, which caches
    // the old style's source handles and cannot re-attach itself. So on the SINGLE `style.load` hook
    // (not the repeatedly-firing per-tile style-data event), tear the instance down and rebuild a fresh
    // one whose new adapter registers onto the new style, then re-add the canonical features (SC4).
    const onStyleLoad = (): void => {
      if (!drawRef.current) {
        return; // initial style load — the first build is driven by "load" below
      }
      teardownDraw();
      buildDraw();
      useSceneStore.getState().noteRehydration();
    };

    if (instance.isStyleLoaded()) {
      buildDraw();
    } else {
      instance.once("load", buildDraw);
    }
    instance.on("style.load", onStyleLoad);
    const detachAttribution = attachOsmAttribution(instance);

    return () => {
      disposed = true;
      instance.off("style.load", onStyleLoad);
      instance.off("load", buildDraw);
      detachAttribution();
      teardownDraw();
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
