// dgmTrigger.ts — the SC1 DGM re-triangulation PRODUCER (D-08). A committed change to the elevation set
// fires a DEBOUNCED `POST /api/v1/dgm/triangulate`; its outcome is written to the `dgm` store slice.
//
// # Module I/O
// - Input  the canonical scene store's elevation features (`elevation_point` vertices +
//   `elevation_line` breaklines). The trigger subscribes to the store and DEBOUNCES (~750 ms), reusing
//   the D-04 finish-not-`change` discipline — it is decoupled from Terra Draw's raw drag path, so it
//   never fires per drag frame (drag frames coalesce into one call once edits go quiet).
// - Output on ≥3 non-collinear points: exactly one debounced triangulate request whose success mesh →
//   `dgm.setTriangulation` (rendered by `DgmOverlay`) and whose 4xx → `dgm.setReject` (the 07-09
//   ValidationPanel crit source — never a silent swallow). Fewer than 3 non-collinear points → skip the
//   doomed request and `dgm.clearDgm` (clears the overlay). The in-flight request is aborted when
//   superseded, and the subscription + timer + controller are torn down in the effect cleanup.
// - Valid input range: WGS84 elevation geometry (points as `[lng, lat]` + `z_m`; breaklines as
//   `[lng, lat]` vertex lists). The [lng, lat, z] triples are the Phase-7 preview inputs; the geodetic
//   projection into SceneXY meters lands with terrain import (Phase 8) — the endpoint shape is unchanged.

import { useEffect } from "react";

import { triangulateDgm, ApiError } from "../api/client";
import { useSceneStore } from "../store/sceneStore";
import { useDgmStore } from "../store/dgm";
import type { DgmReq } from "../generated/wire";
import type { GeoJSONStoreFeatures } from "terra-draw";

const DEBOUNCE_MS = 750;

// Assemble the triangulate request from the store's elevation features. Non-elevation kinds are ignored.
function collectElevation(features: Record<string, GeoJSONStoreFeatures>): DgmReq {
  const points: [number, number, number][] = [];
  const breaklines: [number, number][][] = [];
  for (const f of Object.values(features)) {
    const props = (f.properties ?? {}) as Record<string, unknown>;
    const geometry = f.geometry;
    if (props["kind"] === "elevation_point" && geometry?.type === "Point") {
      const [lng, lat] = geometry.coordinates as [number, number];
      const z = typeof props["z_m"] === "number" ? (props["z_m"] as number) : 0;
      points.push([lng, lat, z]);
    } else if (props["kind"] === "elevation_line" && geometry?.type === "LineString") {
      const line = (geometry.coordinates as [number, number][]).map(
        ([lng, lat]): [number, number] => [lng, lat],
      );
      if (line.length >= 2) {
        breaklines.push(line);
      }
    }
  }
  return { points, breaklines };
}

// True when every point is collinear (degenerate for triangulation) — cross-product of each point vs the
// first spanning edge is ~0. Fewer than 3 points is trivially degenerate.
function allCollinear(points: readonly [number, number, number][]): boolean {
  if (points.length < 3) {
    return true;
  }
  const [ax, ay] = points[0];
  // Find a second distinct anchor to span the reference edge.
  let bx = ax;
  let by = ay;
  for (let i = 1; i < points.length; i++) {
    if (points[i][0] !== ax || points[i][1] !== ay) {
      bx = points[i][0];
      by = points[i][1];
      break;
    }
  }
  if (bx === ax && by === ay) {
    return true; // all points coincide
  }
  const ex = bx - ax;
  const ey = by - ay;
  const EPS = 1e-12;
  for (const [px, py] of points) {
    const cross = ex * (py - ay) - ey * (px - ax);
    if (Math.abs(cross) > EPS) {
      return false;
    }
  }
  return true;
}

// A cheap signature of the elevation set — changes iff an elevation point/line is added, moved, or
// removed, so non-elevation edits (a forest density change, a source move) never schedule a triangulate.
function elevationSignature(features: Record<string, GeoJSONStoreFeatures>): string {
  const parts: string[] = [];
  for (const f of Object.values(features)) {
    const props = (f.properties ?? {}) as Record<string, unknown>;
    const kind = props["kind"];
    if (kind === "elevation_point" || kind === "elevation_line") {
      parts.push(`${String(f.id)}:${JSON.stringify(f.geometry)}:${String(props["z_m"] ?? "")}`);
    }
  }
  return parts.sort().join("|");
}

// Subscribe the DGM producer to committed elevation-set changes. Mount once (e.g. in the map canvas).
export function useDgmTrigger(): void {
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    let controller: AbortController | undefined;

    const run = (): void => {
      const req = collectElevation(useSceneStore.getState().features);
      if (req.points.length < 3 || allCollinear(req.points)) {
        useDgmStore.getState().clearDgm(); // skip the doomed request; clear the overlay
        return;
      }
      controller?.abort();
      controller = new AbortController();
      const signal = controller.signal;
      triangulateDgm(req, signal)
        .then((resp) => {
          if (!signal.aborted) {
            useDgmStore.getState().setTriangulation(resp);
          }
        })
        .catch((err: unknown) => {
          if (signal.aborted) {
            return; // superseded by a newer edit — not an error
          }
          if (err instanceof ApiError) {
            useDgmStore.getState().setReject({ status: err.status, detail: err.detail });
          }
          // Non-HTTP failures (offline/transport) are left for the caller to observe; no false mesh.
        });
    };

    const schedule = (): void => {
      if (timer) {
        clearTimeout(timer);
      }
      timer = setTimeout(run, DEBOUNCE_MS);
    };

    // The store is NOT `subscribeWithSelector`, so this listener fires on EVERY mutation — selection, tool,
    // dirty, and every drag frame of ANY object. Zustand replaces the `features` object only on a feature
    // mutation, so a same-ref `features` guarantees the elevation set is unchanged: skip the per-frame
    // `elevationSignature` scan (features iterate + `JSON.stringify`) unless `features` actually changed.
    let prevFeatures = useSceneStore.getState().features;
    let prevSig = elevationSignature(prevFeatures);
    const unsubscribe = useSceneStore.subscribe((state) => {
      if (state.features === prevFeatures) {
        return;
      }
      prevFeatures = state.features;
      const sig = elevationSignature(state.features);
      if (sig !== prevSig) {
        prevSig = sig;
        schedule();
      }
    });

    return () => {
      unsubscribe();
      if (timer) {
        clearTimeout(timer);
      }
      controller?.abort();
    };
  }, []);
}
