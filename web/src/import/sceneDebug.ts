// sceneDebug.ts — the debug-geometry compute behind the WeatherPanel's debug overlays (GEOX-01/02/03,
// GRID-01, UI half). Runs the REAL `envi-gis` geometry shims (cut-profile → impedance segmentation → screen
// injection, and the receiver grid) over the current scene, so the receiver grid, the impedance-segmented
// path, and the injected screen-top vertices can be drawn on the map.
//
// # Module I/O
// - Input  the canonical scene store (`calc_area`, `building`, `wall`, `source`, `receiver`, `ground_zone`
//   features, WGS84) + the DGM store's server-triangulated TIN (its `[lng, lat, z]` vertices). No props.
// - Output a `DebugGeometry` in WGS84 `[lng, lat]` (receiver points, per-segment impedance polylines, screen-
//   top vertices) plus honest `notes` naming any missing scene input — never a fabricated geometry. ALL of
//   the geometry math runs in the WASM shims; this module only marshals coordinates in/out.
// - Valid input range: features are WGS84 GeoJSON. Coordinates are projected into a LOCAL equirectangular
//   meters frame (origin at the scene reference point) ONLY for this DEBUG visualization — this is NOT the
//   GEOX-04 solve-space reprojection boundary (Phase 10 assembles solve inputs through `envi_geo`). The frame
//   keeps the shims' meter-based spacing/heights meaningful over a small scene.

import type { GeoJSONStoreFeatures } from "terra-draw";

import { useSceneStore } from "../store/sceneStore";
import { useDgmStore } from "../store/dgm";
import {
  buildReceiverGrid,
  extractCutProfile,
  injectScreenEdges,
  segmentCutProfile,
} from "./wasm";
import type { DrawnZoneDto, GroundSegmentationDto, ScreenObjectDto } from "../generated/wire";

// The debug lattice spacing (m) and the cut-profile sampling step (m). Named debug constants — the real
// solve spacing is a Phase-10 user setting; these are sensible visualization defaults.
const DEBUG_SPACING_M = 10;
const DEBUG_PROFILE_STEP_M = 5;
const DEFAULT_GROUND_CLASS = "D";

// One impedance-segmented interval of the debug path: its WGS84 polyline + the σ (flow resistivity) the
// engine resolved for it (drives the soft→hard colour ramp).
export interface DebugSegment {
  readonly line: readonly [number, number][];
  readonly sigma: number;
}

// The debug geometry the overlays render, all in WGS84 `[lng, lat]`.
export interface DebugGeometry {
  readonly receivers: readonly [number, number][];
  readonly segments: readonly DebugSegment[];
  readonly screenVertices: readonly [number, number][];
}

// The compute result: the geometry (may be partially populated) + honest notes for whatever was skipped.
export interface DebugResult {
  readonly geometry: DebugGeometry;
  readonly notes: readonly string[];
}

type LngLat = [number, number];
type Local = [number, number];

// A local equirectangular frame (meters) about a reference lng/lat — the standard small-area tangent-plane
// approximation. Debug-only (see module header); keeps the shims' meter spacing/heights sensible.
interface Frame {
  readonly toLocal: (p: LngLat) => Local;
  readonly toLngLat: (p: Local) => LngLat;
}

const M_PER_DEG_LAT = 111_320;

function makeFrame(lng0: number, lat0: number): Frame {
  const mPerDegLon = M_PER_DEG_LAT * Math.cos((lat0 * Math.PI) / 180);
  const safeLon = Math.abs(mPerDegLon) < 1 ? 1 : mPerDegLon; // guard the poles (never divide by ~0)
  return {
    toLocal: ([lng, lat]) => [(lng - lng0) * safeLon, (lat - lat0) * M_PER_DEG_LAT],
    toLngLat: ([x, y]) => [lng0 + x / safeLon, lat0 + y / M_PER_DEG_LAT],
  };
}

// The outer ring (WGS84 [lng,lat], closing duplicate dropped) of a Polygon feature, or null. The terra-draw
// store type narrows to Polygon; an imported ground_zone/building may be a MultiPolygon, so the discriminant
// is read as a string (mirrors `impedanceOverlay`) to admit the first sub-polygon's outer ring too.
function outerRing(feature: GeoJSONStoreFeatures | undefined): LngLat[] | null {
  const geometry = feature?.geometry;
  const gtype = geometry?.type as string | undefined;
  if (!geometry || (gtype !== "Polygon" && gtype !== "MultiPolygon")) {
    return null;
  }
  const raw = (geometry as { coordinates?: unknown }).coordinates;
  const coords = (
    gtype === "Polygon" ? (raw as number[][][])[0] : (raw as number[][][][])[0]?.[0]
  ) as number[][] | undefined;
  if (!Array.isArray(coords) || coords.length < 4) {
    return null;
  }
  const ring = coords.map((c) => [c[0], c[1]] as LngLat);
  const first = ring[0];
  const last = ring[ring.length - 1];
  if (first[0] === last[0] && first[1] === last[1]) {
    ring.pop();
  }
  return ring;
}

// The [lng,lat] of a Point feature, or null.
function pointOf(feature: GeoJSONStoreFeatures | undefined): LngLat | null {
  const geometry = feature?.geometry;
  if (geometry?.type !== "Point") {
    return null;
  }
  const c = geometry.coordinates;
  return [c[0], c[1]];
}

// The [lng,lat] vertices of a LineString feature, or null.
function lineOf(feature: GeoJSONStoreFeatures | undefined): LngLat[] | null {
  const geometry = feature?.geometry;
  if (geometry?.type !== "LineString") {
    return null;
  }
  return geometry.coordinates.map((c) => [c[0], c[1]] as LngLat);
}

// A finite numeric property, or undefined.
function finiteProp(feature: GeoJSONStoreFeatures, key: string): number | undefined {
  const v = feature.properties?.[key];
  return typeof v === "number" && Number.isFinite(v) ? v : undefined;
}

// The ring centroid (arithmetic mean of the vertices) — the debug frame reference.
function centroid(ring: readonly LngLat[]): LngLat {
  let sx = 0;
  let sy = 0;
  for (const [x, y] of ring) {
    sx += x;
    sy += y;
  }
  return [sx / ring.length, sy / ring.length];
}

// The planar `(x, y)` preimage of each cut-profile point on the S→R line (x = distance along the line), so
// `segment_ground` can resolve each interval against the drawn/imported zones.
function planarAlong(sLocal: Local, rLocal: Local, xs: readonly number[]): Local[] {
  const dx = rLocal[0] - sLocal[0];
  const dy = rLocal[1] - sLocal[1];
  const d = Math.hypot(dx, dy) || 1;
  return xs.map((x) => [sLocal[0] + (dx * x) / d, sLocal[1] + (dy * x) / d]);
}

// Collect features of a given `kind` from the canonical scene store.
function featuresOfKind(
  features: Readonly<Record<string, GeoJSONStoreFeatures>>,
  kind: string,
): GeoJSONStoreFeatures[] {
  return Object.values(features).filter((f) => f.properties?.["kind"] === kind);
}

// Compute the debug geometry over the current scene. Missing inputs are reported in `notes`, never faked.
export async function computeDebugGeometry(): Promise<DebugResult> {
  const features = useSceneStore.getState().features;
  const triangulation = useDgmStore.getState().triangulation;
  const notes: string[] = [];

  const calcAreaFeature = featuresOfKind(features, "calc_area")[0];
  const calcRing = outerRing(calcAreaFeature);
  const buildings = featuresOfKind(features, "building");
  const walls = featuresOfKind(features, "wall");
  const groundZones = featuresOfKind(features, "ground_zone");
  const source = pointOf(featuresOfKind(features, "source")[0]);
  const receiver = pointOf(featuresOfKind(features, "receiver")[0]);

  // The debug frame reference: the calc-area centroid, else the first available geometry, else abort.
  const refRing =
    calcRing ??
    outerRing(buildings[0]) ??
    (source ? [source] : null) ??
    (triangulation?.vertices[0] ? [[triangulation.vertices[0][0], triangulation.vertices[0][1]]] : null);
  if (!refRing) {
    return {
      geometry: { receivers: [], segments: [], screenVertices: [] },
      notes: ["No scene geometry to derive debug overlays from — draw a calc area, source and receiver."],
    };
  }
  const [lng0, lat0] = centroid(refRing);
  const frame = makeFrame(lng0, lat0);

  // TIN points from the server-triangulated DGM surface (projected into the local frame). Required to sample
  // ground z for the profile + grid.
  const tinPoints: [number, number, number][] = (triangulation?.vertices ?? []).map((v) => {
    const [x, y] = frame.toLocal([v[0], v[1]]);
    return [x, y, v[2]];
  });
  const haveTin = tinPoints.length >= 3;
  if (!haveTin) {
    notes.push("No DGM TIN yet — import terrain (elevation points) to enable the receiver grid + profile.");
  }

  let receivers: LngLat[] = [];
  let segments: DebugSegment[] = [];
  let screenVertices: LngLat[] = [];

  // ── Receiver grid (GRID-01) ────────────────────────────────────────────────────────────────────────
  if (calcRing && haveTin) {
    try {
      const footprints = buildings
        .map(outerRing)
        .filter((r): r is LngLat[] => r !== null)
        .map((r) => r.map(frame.toLocal));
      const res = await buildReceiverGrid({
        calc_area: calcRing.map(frame.toLocal),
        footprints,
        spacing_m: DEBUG_SPACING_M,
        discrete_points: [],
        tin_points: tinPoints,
        tin_breaklines: [],
      });
      receivers = res.receivers.map((r) => frame.toLngLat([r[0], r[1]]));
    } catch (err) {
      notes.push(`Receiver grid skipped: ${errText(err)}`);
    }
  } else if (!calcRing) {
    notes.push("No calc area — draw one to see the receiver grid.");
  }

  // ── Impedance segmentation + screen injection along a source→receiver path (GEOX-01/02/03) ───────────
  if (source && receiver && haveTin) {
    try {
      const sLocal = frame.toLocal(source);
      const rLocal = frame.toLocal(receiver);
      const profile = await extractCutProfile({
        tin_points: tinPoints,
        tin_breaklines: [],
        s_xy: sLocal,
        r_xy: rLocal,
        step_m: DEBUG_PROFILE_STEP_M,
      });
      const xs = profile.points.map((p) => p[0]);
      const planar = planarAlong(sLocal, rLocal, xs);

      const drawnZones: DrawnZoneDto[] = groundZones
        .map((z) => {
          const ring = outerRing(z);
          if (!ring) {
            return null;
          }
          const cls = z.properties?.["impedance_class"];
          return {
            polygon: ring.map(frame.toLocal),
            class: typeof cls === "string" && cls.length === 1 ? cls : DEFAULT_GROUND_CLASS,
            // roughness_m is a coherence quantity independent of the σ-class boundaries this overlay shows;
            // 0.0 is a documented debug simplification (the class letter still resolves σ through the engine).
            roughness_m: 0,
          } satisfies DrawnZoneDto;
        })
        .filter((z): z is DrawnZoneDto => z !== null);

      const base = await segmentCutProfile({
        points: profile.points,
        planar_xy: planar,
        drawn_zones: drawnZones,
        imported_zones: [],
        default_class: DEFAULT_GROUND_CLASS,
      });
      segments = segmentsToWgs84(base, frame);

      // Screen injection (GEOX-03): buildings at eaves height, walls/barriers at height.
      const screens = collectScreens(buildings, walls, frame);
      if (screens.length > 0) {
        const injected = await injectScreenEdges({
          base,
          screens,
          tin_points: tinPoints,
          tin_breaklines: [],
        });
        screenVertices = newVertices(base, injected).map((p) => frame.toLngLat(p));
        segments = segmentsToWgs84(injected, frame); // the screen-augmented profile
      } else {
        notes.push("No height-bearing screens (buildings/walls) on the path — screen overlay empty.");
      }
    } catch (err) {
      notes.push(`Impedance/screen overlay skipped: ${errText(err)}`);
    }
  } else if (!source || !receiver) {
    notes.push("Place a source and a receiver to see the impedance segmentation + screen vertices.");
  }

  return { geometry: { receivers, segments, screenVertices }, notes };
}

// Render a segmentation's per-interval planar polyline back to WGS84, tagged with the interval σ.
function segmentsToWgs84(seg: GroundSegmentationDto, frame: Frame): DebugSegment[] {
  const out: DebugSegment[] = [];
  for (let i = 0; i < seg.segments.length; i++) {
    const a = seg.planar_xy[i];
    const b = seg.planar_xy[i + 1];
    if (a && b) {
      out.push({
        line: [frame.toLngLat([a[0], a[1]]), frame.toLngLat([b[0], b[1]])],
        sigma: seg.segments[i].flow_resistivity,
      });
    }
  }
  return out;
}

// The planar `(x,y)` points present in `injected` but not in `base` — the newly-spliced screen-top crossings.
function newVertices(base: GroundSegmentationDto, injected: GroundSegmentationDto): Local[] {
  const eps = 1e-6;
  const inBase = (p: number[]): boolean =>
    base.planar_xy.some((q) => Math.abs(q[0] - p[0]) < eps && Math.abs(q[1] - p[1]) < eps);
  return injected.planar_xy.filter((p) => !inBase(p)).map((p) => [p[0], p[1]]);
}

// Marshal the height-bearing screening objects (buildings at eaves height, walls/barriers at height). Only
// features carrying a FINITE height participate — never a fabricated height.
function collectScreens(
  buildings: GeoJSONStoreFeatures[],
  walls: GeoJSONStoreFeatures[],
  frame: Frame,
): ScreenObjectDto[] {
  const screens: ScreenObjectDto[] = [];
  for (const b of buildings) {
    const ring = outerRing(b);
    const eaves = finiteProp(b, "eaves_height_m");
    if (ring && eaves !== undefined) {
      screens.push({ building: { footprint: ring.map(frame.toLocal), eaves_height_m: eaves } });
    }
  }
  for (const w of walls) {
    const line = lineOf(w);
    const height = finiteProp(w, "height_m") ?? finiteProp(w, "z_m");
    if (line && height !== undefined) {
      screens.push({ barrier: { line: line.map(frame.toLocal), height_m: height } });
    }
  }
  return screens;
}

// A short message from an unknown thrown value (a WASM `GisError` surfaces as a plain `Error`).
function errText(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}
