// importJob.ts — the per-layer import orchestrator (D-06/D-07/D-08). Client-side state machines (NOT the
// Phase-6 SSE job machine) that, per independently-toggleable layer, evaluate the viewport guardrail, then
// fetch (cache-first from OPFS) → WASM window/decode/map/parse → assign feature ids → WASM merge (D-09) →
// commit through the Phase-7 scene path, with AbortController supersession, retry, and partial success.
//
// # Module I/O
// - Input  a project id + the current WGS84 viewport + the per-layer toggles (from the import store). All
//   GIS math (windowing, decode, reprojection, vectorization, height chain, merge) is delegated to the
//   `wasm` facade — this module only orchestrates fetch/OPFS/commit (the plan's "compute lives in WASM").
// - Output committed editable scene features (each an ordinary 9-kind object carrying provenance) via
//   `loadImportedScene` + `saveScene`; per-layer status/progress/error written to the import store; the
//   contributing source ids added for the SC5 attribution. A failed layer records its error + stays
//   retryable WITHOUT touching a sibling (D-07); a partial import lands what succeeded.
// - Valid input range: an open project (a `projectId`); a non-null viewport within the guardrail budget.
//   Post-import the compute path reads tiles from OPFS (bytes are read back from the cache after write,
//   DATA-04). Terrain retains its decoded tiles in memory so the buildings layer can sample footprint base
//   elevation (typed `null`/absent, never `0.0`, when terrain is missing — D-07).
//
// # Raster tiles are RANGE-READ, never downloaded whole (see `cogWindow.ts`)
//
// Each covering COG tile goes through `loadCogWindow`: fetch the header, plan the internal tiles the
// viewport overlaps, fetch only those byte ranges. The previous whole-tile GET pulled 330 MB of AHN (and
// 54 MB of WorldCover) for a ~1 km² viewport and hung the import. A layer therefore carries a `CogWindow`
// — the fetched byte parts + the pixel window — into every WASM decode call, not a whole tile.

import { ApiError, errorText, toStatusError, type SceneCollection } from "../api/client";
import { useSceneStore } from "../store/sceneStore";
import {
  useImportStore,
  type GuardrailState,
  type LayerError,
  type LayerKey,
} from "../store/import";
import { fetchOverpass, overpassQuery } from "./fetchers";
import { loadCogWindow, type CogWindow } from "./cogWindow";
import {
  mapLandcover,
  mergeFeatures,
  parseBuildings,
  planImport,
  planTiles,
  reprojectRing,
  sampleBaseElevation,
  terrainFeatures,
} from "./wasm";
import type {
  BboxDto,
  SourceDescriptorDto,
  TerrainSourceCrsDto,
  VerticalDatumDto,
} from "../generated/wire";

// A minimal structural view of a GeoJSON feature / collection crossing the WASM boundary (typed `unknown`
// on the wire; validated server-side on PUT). We only read/assign `id`, `properties`, and geometry rings.
interface RawFeature {
  type: "Feature";
  id?: string;
  geometry: { type: string; coordinates: unknown } | null;
  properties: Record<string, unknown> | null;
}
interface RawFeatureCollection {
  type: "FeatureCollection";
  features: RawFeature[];
}

// The terrain windows retained in memory after a terrain import, so the buildings layer can sample
// footprint-boundary base elevation off them (SC4). Keyed per project. This is the RANGE-READ window (the
// fetched byte parts + the pixel window), not a whole tile.
interface TerrainBaseSource {
  readonly cog: CogWindow;
  readonly sourceCrs: TerrainSourceCrsDto;
}

// Decimated elevation target per import (well under envi-dgm's cap + scene-PUT practicality, 08-RESEARCH
// Open-Q4). Buildings default eaves height when no OSM height/levels tag resolves (~2 storeys NL rowhouse).
const TERRAIN_TARGET_POINTS = 4000;
const USER_DEFAULT_HEIGHT_M = 6;

// Footprint-boundary sample spacing for base elevation, in the terrain source CRS's units (meters for RD
// New / AHN; degrees for WGS84 / GLO-30 — ~3 m near NL latitudes).
const BASE_ELEV_SPACING_M = 3.0;
const BASE_ELEV_SPACING_DEG = 3.0 / 111_320;

// Viewport-area guardrail thresholds (D-06). Warn above `WARN`, refuse above `BLOCK` — a coarse pre-fetch
// gate; the exact per-tile decoded-pixel budget is enforced in `window_for_bbox` (T-08-02-01).
const GUARDRAIL_WARN_KM2 = 25;
const GUARDRAIL_BLOCK_KM2 = 100;

// The retained terrain tiles per project (the terrain→buildings base-elevation handoff, D-07).
const terrainBaseByProject = new Map<string, TerrainBaseSource[]>();

// One AbortController per layer: a new run for a layer supersedes its in-flight predecessor (dgmTrigger
// discipline). Sibling layers are untouched (D-07 independence).
const controllers: Record<LayerKey, AbortController | null> = {
  terrain: null,
  landcover: null,
  buildings: null,
};

// The last import invocation, so a per-layer retry re-runs with the same project + viewport.
let lastRun: { projectId: string; bbox: BboxDto } | null = null;

// Serialize the read→merge→load→save critical section across concurrently-running layers so two layers
// never lose each other's features to an interleaved read-modify-write of the scene store.
let commitChain: Promise<unknown> = Promise.resolve();
function serializeCommit<T>(fn: () => Promise<T>): Promise<T> {
  const run = commitChain.then(fn, fn);
  commitChain = run.catch(() => undefined);
  return run;
}

// --- guardrail --------------------------------------------------------------

// Approximate viewport area in km² (equirectangular; a coarse UI guardrail, not a projection).
function viewportAreaKm2(bbox: BboxDto): number {
  const midLat = ((bbox.min_lat + bbox.max_lat) / 2) * (Math.PI / 180);
  const widthKm = (bbox.max_lon - bbox.min_lon) * 111.32 * Math.cos(midLat);
  const heightKm = (bbox.max_lat - bbox.min_lat) * 110.574;
  return Math.abs(widthKm * heightKm);
}

// Evaluate the max-area guardrail for a viewport (D-06): null within budget, a warn or a block above it.
export function evaluateGuardrail(bbox: BboxDto): GuardrailState | null {
  // Antimeridian / inverted viewport (IN-01): a viewport crossing ±180° arrives as min_lon > max_lon and
  // the tile planner's `[lo, hi]` grid walk would silently yield ZERO tiles. Report it as unsupported
  // rather than importing nothing. (An inverted latitude is likewise degenerate.)
  if (bbox.min_lon > bbox.max_lon || bbox.min_lat > bbox.max_lat) {
    return {
      blocked: true,
      detail:
        "This viewport crosses the antimeridian (±180°), which import does not support — pan so the view stays on one side.",
    };
  }
  const areaKm2 = viewportAreaKm2(bbox);
  if (areaKm2 > GUARDRAIL_BLOCK_KM2) {
    return {
      blocked: true,
      detail: `Import area ~${areaKm2.toFixed(0)} km² exceeds the ${GUARDRAIL_BLOCK_KM2} km² limit — zoom in before importing.`,
    };
  }
  if (areaKm2 > GUARDRAIL_WARN_KM2) {
    return {
      blocked: false,
      detail: `Large import area ~${areaKm2.toFixed(0)} km² — this may fetch several large tiles.`,
    };
  }
  return null;
}

// --- shared helpers ---------------------------------------------------------

function sourceCrsOf(descriptor: SourceDescriptorDto): TerrainSourceCrsDto {
  return descriptor.crs === "EPSG:28992" ? "rd_new" : "wgs84";
}

function verticalDatumOf(sourceId: string): VerticalDatumDto | null {
  if (sourceId === "ahn4-dtm") {
    return "nap";
  }
  if (sourceId === "glo30") {
    return "egm2008";
  }
  return null;
}

function toLayerError(err: unknown): LayerError {
  return toStatusError(err, "Import failed.");
}

// Assign a fresh UUID to every feature (WASM mints none — Pitfall 9). Sets both the top-level `id` and
// `properties.id` so the store + the server `feature_uuid` gate both see it.
function assignIds(features: RawFeature[]): RawFeatureCollection {
  for (const f of features) {
    const id = crypto.randomUUID();
    f.id = id;
    f.properties = { ...(f.properties ?? {}), id };
  }
  return { type: "FeatureCollection", features };
}

// The read→merge→load→save commit of a layer's features into the scene (D-09 merge preserves user edits).
// Returns the committed feature count. Runs inside the commit mutex so concurrent layers don't clobber.
//
// A persist failure is NOT swallowed (WR-01): the whole-scene PUT is the durability signal, and a large
// import (thousands of terrain points) can be rejected by the scene-PUT body limit. Reporting `done, N
// features` while the PUT failed would silently lose the import on reload. So a failed `saveScene` throws
// here — the layer records a real error (retryable) instead of a false success. The merged features remain
// in the in-memory store (visible until reload) but are explicitly flagged as un-persisted.
function commitFeatures(features: RawFeature[], sourceId: string): Promise<number> {
  return serializeCommit(async () => {
    if (features.length === 0) {
      return 0;
    }
    const incoming = assignIds(features);
    const existing = useSceneStore.getState().sceneFeatureCollection();
    const merged = await mergeFeatures({
      existing: existing as unknown,
      incoming: incoming as unknown,
    });
    useSceneStore.getState().loadImportedScene(merged.features as unknown as SceneCollection);
    useImportStore.getState().addAttributedSources([sourceId]);
    // Persist (whole-scene PUT). Surface a persist failure as the layer's failure — never a false "done".
    try {
      await useSceneStore.getState().saveScene();
    } catch (err) {
      if (err instanceof ApiError && (err.status === 413 || err.status === 400)) {
        throw new ApiError(
          err.status,
          "Import is too large to save — zoom in to a smaller area and re-import.",
        );
      }
      throw new ApiError(
        err instanceof ApiError ? err.status : 0,
        errorText(err, "Imported features could not be saved."),
      );
    }
    return incoming.features.length;
  });
}

// The exterior ring (`[lon, lat][]`) of a Polygon/MultiPolygon feature, or null if it has none.
function exteriorRing(feature: RawFeature): [number, number][] | null {
  const geometry = feature.geometry;
  if (!geometry) {
    return null;
  }
  if (geometry.type === "Polygon") {
    const rings = geometry.coordinates as [number, number][][];
    return rings[0] ?? null;
  }
  if (geometry.type === "MultiPolygon") {
    const polys = geometry.coordinates as [number, number][][][];
    return polys[0]?.[0] ?? null;
  }
  return null;
}

// --- layer machines ---------------------------------------------------------

async function runTerrain(projectId: string, bbox: BboxDto, retrievedAt: string): Promise<void> {
  const store = useImportStore.getState();
  const controller = supersede("terrain");
  const { signal } = controller;
  store.startLayer("terrain");
  try {
    const plan = await planImport({ bbox });
    if (signal.aborted) return;
    const descriptor = plan.terrain;
    const sourceCrs = sourceCrsOf(descriptor);
    const tiles = (await planTiles({ bbox })).terrain;
    if (signal.aborted) return;
    if (tiles.length === 0) {
      throw new ApiError(0, "No terrain tiles cover this viewport.");
    }

    const features: RawFeature[] = [];
    const baseSources: TerrainBaseSource[] = [];
    for (let i = 0; i < tiles.length; i++) {
      const tile = tiles[i];
      store.setLayerProgress("terrain", 0.1 + 0.6 * (i / tiles.length), `tile ${i + 1}/${tiles.length}`);
      // Windowed range read: the header, then only the COG tiles this viewport overlaps.
      const cog = await loadCogWindow(projectId, tile, descriptor.cors, bbox, sourceCrs, signal);
      if (signal.aborted) return;
      if (!cog) {
        continue; // this tile is adjacent, not overlapping — nothing fetched, nothing to decode
      }
      const res = await terrainFeatures(cog.bytes, {
        window: cog.window,
        parts: cog.parts,
        target_points: TERRAIN_TARGET_POINTS,
        source_crs: sourceCrs,
        provenance: {
          source_id: descriptor.id,
          source_ref: tile.tile,
          retrieved_at: retrievedAt,
          vertical_datum: verticalDatumOf(descriptor.id),
        },
        max_decoded_px: null,
      });
      if (signal.aborted) return;
      features.push(...(res.features as RawFeatureCollection).features);
      baseSources.push({ cog, sourceCrs });
    }

    store.setLayerProgress("terrain", 0.85, "committing");
    const count = await commitFeatures(features, descriptor.id);
    if (signal.aborted) return;
    // Retain the decoded terrain tiles so the buildings layer can sample base elevation (SC4).
    terrainBaseByProject.set(projectId, baseSources);
    store.finishLayer("terrain", { featureCount: count, surfaceModel: descriptor.kind === "dsm" });
  } catch (err) {
    if (signal.aborted) return; // superseded — not an error
    store.failLayer("terrain", toLayerError(err));
  }
}

async function runLandcover(projectId: string, bbox: BboxDto, retrievedAt: string): Promise<void> {
  const store = useImportStore.getState();
  const controller = supersede("landcover");
  const { signal } = controller;
  store.startLayer("landcover");
  try {
    const plan = await planImport({ bbox });
    if (signal.aborted) return;
    const descriptor = plan.landcover; // WorldCover, EPSG:4326
    const tiles = (await planTiles({ bbox })).landcover;
    if (signal.aborted) return;
    if (tiles.length === 0) {
      throw new ApiError(0, "No land-cover tiles cover this viewport.");
    }

    const features: RawFeature[] = [];
    for (let i = 0; i < tiles.length; i++) {
      const tile = tiles[i];
      store.setLayerProgress("landcover", 0.1 + 0.6 * (i / tiles.length), `tile ${i + 1}/${tiles.length}`);
      // Windowed range read (the 54 MB WorldCover tile is never downloaded whole).
      const cog = await loadCogWindow(projectId, tile, descriptor.cors, bbox, "wgs84", signal);
      if (signal.aborted) return;
      if (!cog) {
        continue;
      }
      const res = await mapLandcover(cog.bytes, {
        window: cog.window,
        parts: cog.parts,
        min_area_px: null,
        simplify_tol_px: null,
        provenance: {
          source_id: descriptor.id,
          source_ref: tile.tile,
          retrieved_at: retrievedAt,
          vertical_datum: null,
        },
        max_decoded_px: null,
      });
      if (signal.aborted) return;
      features.push(...(res.features as RawFeatureCollection).features);
    }

    store.setLayerProgress("landcover", 0.85, "committing");
    const count = await commitFeatures(features, descriptor.id);
    if (signal.aborted) return;
    store.finishLayer("landcover", { featureCount: count });
  } catch (err) {
    if (signal.aborted) return;
    store.failLayer("landcover", toLayerError(err));
  }
}

// Populate each building's `base_elevation_m` from the retained terrain tiles (footprint-boundary median,
// SC4). Left ABSENT (typed None, never 0.0) when no terrain covers the footprint (D-07 — completable on a
// later terrain import).
async function populateBaseElevations(
  buildings: RawFeature[],
  baseSources: TerrainBaseSource[],
  signal: AbortSignal,
): Promise<void> {
  if (baseSources.length === 0) {
    return;
  }
  for (const building of buildings) {
    if (signal.aborted) return;
    const ring = exteriorRing(building);
    if (!ring) {
      continue;
    }
    for (const source of baseSources) {
      const spacing = source.sourceCrs === "rd_new" ? BASE_ELEV_SPACING_M : BASE_ELEV_SPACING_DEG;
      const { ring: sourceRing } = await reprojectRing({ ring, source_crs: source.sourceCrs });
      if (signal.aborted) return;
      const res = await sampleBaseElevation(source.cog.bytes, {
        window: source.cog.window,
        parts: source.cog.parts,
        ring: sourceRing,
        max_spacing_m: spacing,
        max_decoded_px: null,
      });
      // `serde_wasm_bindgen` marshals `Option::None` as `undefined`, not `null` (wire.ts types it
      // `number | null`). Normalise, or a footprint with NO terrain coverage reads as "a value", writes
      // `base_elevation_m: undefined`, and stops the search at the first non-covering tile (D-07 says the
      // property must be ABSENT, never fabricated).
      const baseElevation = res.base_elevation_m ?? null;
      if (baseElevation !== null) {
        building.properties = { ...(building.properties ?? {}), base_elevation_m: baseElevation };
        break; // first covering terrain tile wins
      }
    }
  }
}

async function runBuildings(projectId: string, bbox: BboxDto, retrievedAt: string): Promise<void> {
  const store = useImportStore.getState();
  const controller = supersede("buildings");
  const { signal } = controller;
  store.startLayer("buildings");
  try {
    const plan = await planImport({ bbox });
    if (signal.aborted) return;
    const descriptor = plan.buildings; // OSM Overpass, bbox-query
    store.setLayerProgress("buildings", 0.2, "querying overpass");
    const json = await fetchOverpass(
      descriptor.endpoint_template,
      overpassQuery(bbox.min_lon, bbox.min_lat, bbox.max_lon, bbox.max_lat),
      signal,
    );
    if (signal.aborted) return;
    const parsed = await parseBuildings({
      overpass_json: json,
      user_default_height_m: USER_DEFAULT_HEIGHT_M,
      retrieved_at: retrievedAt,
    });
    if (signal.aborted) return;
    const features = (parsed.features as RawFeatureCollection).features;

    store.setLayerProgress("buildings", 0.6, "sampling base elevation");
    await populateBaseElevations(features, terrainBaseByProject.get(projectId) ?? [], signal);
    if (signal.aborted) return;

    store.setLayerProgress("buildings", 0.85, "committing");
    const count = await commitFeatures(features, descriptor.id);
    if (signal.aborted) return;
    store.finishLayer("buildings", { featureCount: count });
  } catch (err) {
    if (signal.aborted) return;
    store.failLayer("buildings", toLayerError(err));
  }
}

// Abort a layer's in-flight controller and install a fresh one (supersession). Sibling layers untouched.
function supersede(layer: LayerKey): AbortController {
  controllers[layer]?.abort();
  const controller = new AbortController();
  controllers[layer] = controller;
  return controller;
}

const RUNNERS: Record<
  LayerKey,
  (projectId: string, bbox: BboxDto, retrievedAt: string) => Promise<void>
> = {
  terrain: runTerrain,
  landcover: runLandcover,
  buildings: runBuildings,
};

// --- public API -------------------------------------------------------------

// Import the current viewport: evaluate the guardrail (precondition skip — a blocked viewport clears any
// running state and does NOT fetch), then fire every ENABLED layer independently (D-07). Returns after
// dispatch; layers report their own progress/outcome to the store.
export function runImport(projectId: string, bbox: BboxDto): void {
  const store = useImportStore.getState();
  const guardrail = evaluateGuardrail(bbox);
  store.setGuardrail(guardrail);
  if (guardrail?.blocked) {
    return; // doomed request — skip the fetch, surface the guardrail (dgmTrigger clear-and-skip)
  }
  lastRun = { projectId, bbox };
  const retrievedAt = new Date().toISOString();
  for (const layer of Object.keys(RUNNERS) as LayerKey[]) {
    if (store.layers[layer].enabled) {
      void RUNNERS[layer](projectId, bbox, retrievedAt);
    }
  }
}

// Retry a single failed layer WITHOUT touching its siblings (D-07). No-op if no import has run yet.
export function retryLayer(layer: LayerKey): void {
  if (!lastRun) {
    return;
  }
  const retrievedAt = new Date().toISOString();
  void RUNNERS[layer](lastRun.projectId, lastRun.bbox, retrievedAt);
}

// Abort every in-flight layer and drop retained terrain (effect-cleanup teardown for the app unmount).
export function teardownImport(): void {
  for (const layer of Object.keys(controllers) as LayerKey[]) {
    controllers[layer]?.abort();
    controllers[layer] = null;
  }
  terrainBaseByProject.clear();
}
