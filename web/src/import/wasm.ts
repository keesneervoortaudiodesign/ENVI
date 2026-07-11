// wasm.ts — the typed facade over the generated `envi-gis-wasm` boundary (DATA-01..03). Lazily
// initialises the wasm module once and re-exposes each `#[wasm_bindgen]` export with the committed
// `../generated/wire` DTO types instead of the generated `any`. This is the ONLY place the untyped glue
// is touched; every caller works in wire-typed values.
//
// # Module I/O
// - Input  the committed wasm glue (`../generated/wasm/envi_gis_wasm`, a build artifact regenerated from
//   the Rust crate) + wire-typed request DTOs. Tile bytes cross as a `Uint8Array` (efficient typed-array
//   marshalling), never a serde field. All GIS math lives behind these calls in WASM — this module only
//   marshals (no geometry, no decode) per the plan's "compute lives in WASM" contract.
// - Output wire-typed result DTOs (`ImportPlanResult`, `PlanTilesResult`, …). `ensureWasm()` resolves the
//   initialised module; every export awaits it, so a caller never sees an uninitialised module.
// - Valid input range: request DTOs exactly as generated in `wire.ts` (`deny_unknown_fields` on the Rust
//   side rejects a typo'd key at the boundary). A `GisError` from the core surfaces as a thrown `Error`.

import init, {
  plan_import as wasmPlanImport,
  plan_tiles as wasmPlanTiles,
  window_for_bbox as wasmWindowForBbox,
  reproject_ring as wasmReprojectRing,
  terrain_features as wasmTerrainFeatures,
  sample_base_elevation as wasmSampleBaseElevation,
  map_landcover as wasmMapLandcover,
  parse_buildings as wasmParseBuildings,
  merge_features as wasmMergeFeatures,
} from "../generated/wasm/envi_gis_wasm";
import type {
  BaseElevationReq,
  BaseElevationResult,
  BuildingsResult,
  ImportPlanReq,
  ImportPlanResult,
  LandcoverResult,
  MapLandcoverReq,
  MergeReq,
  MergeResult,
  ParseBuildingsReq,
  PlanTilesReq,
  PlanTilesResult,
  ReprojectRingReq,
  ReprojectRingResult,
  TerrainFeaturesReq,
  TerrainFeaturesResult,
  WindowForBboxReq,
  WindowForBboxResult,
} from "../generated/wire";

// Initialise the wasm module exactly once (idempotent across concurrent layer imports). `init()` fetches
// the sibling `.wasm` via `new URL(..., import.meta.url)`, which Vite bundles/serves natively.
let ready: Promise<void> | null = null;
function ensureWasm(): Promise<void> {
  ready ??= init().then(() => undefined);
  return ready;
}

// The generated exports type `req`/return as `any`; the wire DTOs are the real shapes. These wrappers are
// the single, audited cast site (kept local so no caller casts).
async function call<Req, Res>(fn: (req: Req) => unknown, req: Req): Promise<Res> {
  await ensureWasm();
  return fn(req) as Res;
}
async function callBytes<Req, Res>(
  fn: (bytes: Uint8Array, req: Req) => unknown,
  bytes: Uint8Array,
  req: Req,
): Promise<Res> {
  await ensureWasm();
  return fn(bytes, req) as Res;
}

/** Pick each layer's source for a WGS84 viewport (registry coverage lookup, D-04). */
export function planImport(req: ImportPlanReq): Promise<ImportPlanResult> {
  return call<ImportPlanReq, ImportPlanResult>(wasmPlanImport as (r: ImportPlanReq) => unknown, req);
}

/** Enumerate the covering source tiles for a viewport (terrain + land cover). */
export function planTiles(req: PlanTilesReq): Promise<PlanTilesResult> {
  return call<PlanTilesReq, PlanTilesResult>(wasmPlanTiles as (r: PlanTilesReq) => unknown, req);
}

/** Resolve the pixel window of a viewport within a cached tile (`null` = no overlap). */
export function windowForBbox(
  tileBytes: Uint8Array,
  req: WindowForBboxReq,
): Promise<WindowForBboxResult> {
  return callBytes<WindowForBboxReq, WindowForBboxResult>(
    wasmWindowForBbox as (b: Uint8Array, r: WindowForBboxReq) => unknown,
    tileBytes,
    req,
  );
}

/** Reproject a WGS84 footprint ring into a terrain tile's source CRS (GEOX-04). */
export function reprojectRing(req: ReprojectRingReq): Promise<ReprojectRingResult> {
  return call<ReprojectRingReq, ReprojectRingResult>(
    wasmReprojectRing as (r: ReprojectRingReq) => unknown,
    req,
  );
}

/** Decode a terrain window and build WGS84 `elevation_point` features. */
export function terrainFeatures(
  tileBytes: Uint8Array,
  req: TerrainFeaturesReq,
): Promise<TerrainFeaturesResult> {
  return callBytes<TerrainFeaturesReq, TerrainFeaturesResult>(
    wasmTerrainFeatures as (b: Uint8Array, r: TerrainFeaturesReq) => unknown,
    tileBytes,
    req,
  );
}

/** Footprint-boundary median base elevation from a decoded terrain window (`null` when absent). */
export function sampleBaseElevation(
  tileBytes: Uint8Array,
  req: BaseElevationReq,
): Promise<BaseElevationResult> {
  return callBytes<BaseElevationReq, BaseElevationResult>(
    wasmSampleBaseElevation as (b: Uint8Array, r: BaseElevationReq) => unknown,
    tileBytes,
    req,
  );
}

/** Decode a WorldCover `u8` window and vectorize it into `ground_zone` features. */
export function mapLandcover(tileBytes: Uint8Array, req: MapLandcoverReq): Promise<LandcoverResult> {
  return callBytes<MapLandcoverReq, LandcoverResult>(
    wasmMapLandcover as (b: Uint8Array, r: MapLandcoverReq) => unknown,
    tileBytes,
    req,
  );
}

/** Parse Overpass JSON into `building` features + per-element skip reports. */
export function parseBuildings(req: ParseBuildingsReq): Promise<BuildingsResult> {
  return call<ParseBuildingsReq, BuildingsResult>(
    wasmParseBuildings as (r: ParseBuildingsReq) => unknown,
    req,
  );
}

/** Merge a fresh import into the existing scene by feature identity (D-09). */
export function mergeFeatures(req: MergeReq): Promise<MergeResult> {
  return call<MergeReq, MergeResult>(wasmMergeFeatures as (r: MergeReq) => unknown, req);
}
