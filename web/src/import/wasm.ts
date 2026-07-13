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
  plan_cog_reads as wasmPlanCogReads,
  window_for_bbox as wasmWindowForBbox,
  reproject_ring as wasmReprojectRing,
  terrain_features as wasmTerrainFeatures,
  sample_base_elevation as wasmSampleBaseElevation,
  map_landcover as wasmMapLandcover,
  parse_buildings as wasmParseBuildings,
  merge_features as wasmMergeFeatures,
  extract_cut_profile as wasmExtractCutProfile,
  segment_cut_profile as wasmSegmentCutProfile,
  inject_screen_edges as wasmInjectScreenEdges,
  build_receiver_grid as wasmBuildReceiverGrid,
  derive_weather as wasmDeriveWeather,
  derive_weather_friendly as wasmDeriveWeatherFriendly,
  difference_dba as wasmDifferenceDba,
} from "../generated/wasm/envi_gis_wasm";
import type {
  BaseElevationReq,
  BaseElevationResult,
  BuildingsResult,
  CutProfileReq,
  CutProfileResult,
  GroundSegmentationDto,
  ImportPlanReq,
  ImportPlanResult,
  InjectScreensReq,
  LandcoverResult,
  MapLandcoverReq,
  MergeReq,
  MergeResult,
  ParseBuildingsReq,
  PlanCogReadsReq,
  PlanCogReadsResult,
  PlanTilesReq,
  PlanTilesResult,
  ReceiverGridReq,
  ReceiverGridResult,
  FriendlyWeatherReq,
  ReprojectRingReq,
  ReprojectRingResult,
  SegmentGroundReq,
  TerrainFeaturesReq,
  TerrainFeaturesResult,
  WeatherDeriveReq,
  WeatherDeriveResult,
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

// A wasm TRAP (a Rust panic aborts to `unreachable`) leaves the linear-memory module in an unrecoverable
// state — every subsequent call into the same instance is undefined. wasm-bindgen surfaces a trap as a
// `WebAssembly.RuntimeError` (a normal `Result::Err`/`GisError` is a plain thrown `Error`, NOT a
// RuntimeError — so this discriminates a poison-trap from a recoverable domain error). On a trap we drop the
// cached instance so the NEXT call re-inits a FRESH module, letting a per-layer Retry re-enter a clean
// instance instead of the poisoned one (WR-03).
function isWasmTrap(err: unknown): boolean {
  return typeof WebAssembly !== "undefined" && err instanceof WebAssembly.RuntimeError;
}
function onWasmError(err: unknown): void {
  if (isWasmTrap(err)) {
    ready = null; // poison recovery: force a fresh init on the next call
  }
}

// The generated exports type `req`/return as `any`; the wire DTOs are the real shapes. These wrappers are
// the single, audited cast site (kept local so no caller casts) and the single wasm-trap recovery point.
async function call<Req, Res>(fn: (req: Req) => unknown, req: Req): Promise<Res> {
  await ensureWasm();
  try {
    return fn(req) as Res;
  } catch (err) {
    onWasmError(err);
    throw err;
  }
}
async function callBytes<Req, Res>(
  fn: (bytes: Uint8Array, req: Req) => unknown,
  bytes: Uint8Array,
  req: Req,
): Promise<Res> {
  await ensureWasm();
  try {
    return fn(bytes, req) as Res;
  } catch (err) {
    onWasmError(err);
    throw err;
  }
}

/** Pick each layer's source for a WGS84 viewport (registry coverage lookup, D-04). */
export function planImport(req: ImportPlanReq): Promise<ImportPlanResult> {
  return call<ImportPlanReq, ImportPlanResult>(wasmPlanImport, req);
}

/** Enumerate the covering source tiles for a viewport (terrain + land cover). */
export function planTiles(req: PlanTilesReq): Promise<PlanTilesResult> {
  return call<PlanTilesReq, PlanTilesResult>(wasmPlanTiles, req);
}

/**
 * Plan the WINDOWED RANGE READ of a COG tile: given its fetched header prefix, return the viewport's pixel
 * window AND exactly the byte ranges of the internal COG tiles that window overlaps, minus the ranges the
 * caller's OPFS cache already holds. This is what replaced the whole-tile GET (330 MB for a 1 km² Amsterdam
 * window). ALL of the reasoning — header-fit, geotransform, chunk grid, cache subtraction, fetch budget —
 * is `envi_gis`'s; TypeScript only issues the `Range` GETs the plan asks for.
 */
export function planCogReads(
  headerBytes: Uint8Array,
  req: PlanCogReadsReq,
): Promise<PlanCogReadsResult> {
  return callBytes<PlanCogReadsReq, PlanCogReadsResult>(wasmPlanCogReads, headerBytes, req);
}

/** Resolve the pixel window of a viewport within a cached tile (`null` = no overlap). */
export function windowForBbox(
  tileBytes: Uint8Array,
  req: WindowForBboxReq,
): Promise<WindowForBboxResult> {
  return callBytes<WindowForBboxReq, WindowForBboxResult>(wasmWindowForBbox, tileBytes, req);
}

/** Reproject a WGS84 footprint ring into a terrain tile's source CRS (GEOX-04). */
export function reprojectRing(req: ReprojectRingReq): Promise<ReprojectRingResult> {
  return call<ReprojectRingReq, ReprojectRingResult>(wasmReprojectRing, req);
}

/** Decode a terrain window and build WGS84 `elevation_point` features. */
export function terrainFeatures(
  tileBytes: Uint8Array,
  req: TerrainFeaturesReq,
): Promise<TerrainFeaturesResult> {
  return callBytes<TerrainFeaturesReq, TerrainFeaturesResult>(wasmTerrainFeatures, tileBytes, req);
}

/** Footprint-boundary median base elevation from a decoded terrain window (`null` when absent). */
export function sampleBaseElevation(
  tileBytes: Uint8Array,
  req: BaseElevationReq,
): Promise<BaseElevationResult> {
  return callBytes<BaseElevationReq, BaseElevationResult>(
    wasmSampleBaseElevation,
    tileBytes,
    req,
  );
}

/** Decode a WorldCover `u8` window and vectorize it into `ground_zone` features. */
export function mapLandcover(tileBytes: Uint8Array, req: MapLandcoverReq): Promise<LandcoverResult> {
  return callBytes<MapLandcoverReq, LandcoverResult>(wasmMapLandcover, tileBytes, req);
}

/** Parse Overpass JSON into `building` features + per-element skip reports. */
export function parseBuildings(req: ParseBuildingsReq): Promise<BuildingsResult> {
  return call<ParseBuildingsReq, BuildingsResult>(wasmParseBuildings, req);
}

/** Merge a fresh import into the existing scene by feature identity (D-09). */
export function mergeFeatures(req: MergeReq): Promise<MergeResult> {
  return call<MergeReq, MergeResult>(wasmMergeFeatures, req);
}

/** Extract the source→receiver DEM cut-profile (GEOX-01): strictly-ascending `(x, z)` ground points. */
export function extractCutProfile(req: CutProfileReq): Promise<CutProfileResult> {
  return call<CutProfileReq, CutProfileResult>(wasmExtractCutProfile, req);
}

/** Segment the cut-profile into per-interval ground impedance segments (GEOX-02, drawn > imported > default). */
export function segmentCutProfile(req: SegmentGroundReq): Promise<GroundSegmentationDto> {
  return call<SegmentGroundReq, GroundSegmentationDto>(wasmSegmentCutProfile, req);
}

/** Inject screening edges (building/wall/barrier tops) into a base segmentation as `(x, z)` vertices (GEOX-03). */
export function injectScreenEdges(req: InjectScreensReq): Promise<GroundSegmentationDto> {
  return call<InjectScreensReq, GroundSegmentationDto>(wasmInjectScreenEdges, req);
}

/** Build the building-aware constrained-Delaunay receiver grid (GRID-01): receiver positions `[x, y, z]`. */
export function buildReceiverGrid(req: ReceiverGridReq): Promise<ReceiverGridResult> {
  return call<ReceiverGridReq, ReceiverGridResult>(wasmBuildReceiverGrid, req);
}

/**
 * Derive the per-azimuth sound-speed profiles from an Open-Meteo multi-level profile (METX-01). ALL of the
 * A/B/C acoustic math runs here in WASM (`envi_gis::weather`) — TypeScript never does acoustic arithmetic
 * (the wire contract, threat T-09-05-04). Callers pass the OPFS-cached Open-Meteo JSON verbatim.
 */
export function deriveWeather(req: WeatherDeriveReq): Promise<WeatherDeriveResult> {
  return call<WeatherDeriveReq, WeatherDeriveResult>(wasmDeriveWeather, req);
}

/**
 * Derive the per-azimuth sound-speed profiles from FRIENDLY what-if met knobs (METX-03/04, D-14): a surface
 * temperature + gradient, a wind speed + direction, a roughness length, and the downwind-worst-case toggle
 * (D-15) — or a raw per-azimuth `(A, B, C, z₀)` advanced override. ALL of the A/B/C math runs here in WASM
 * (`envi_gis::weather`, the SAME derivation the Open-Meteo path uses); TypeScript never does acoustic
 * arithmetic (D-01, the wire contract).
 */
export function deriveWeatherFriendly(req: FriendlyWeatherReq): Promise<WeatherDeriveResult> {
  return call<FriendlyWeatherReq, WeatherDeriveResult>(wasmDeriveWeatherFriendly, req);
}

/**
 * The per-receiver signed dB(A) difference `A − B` between two scenarios' cached readouts (METX-04 / D-16,
 * D-01). Both inputs are WASM-produced weighted totals; the subtraction runs in WASM so `store/difference.ts`
 * performs ZERO acoustic arithmetic — it only marshals the two number arrays here and renders the returned
 * deltas. A length mismatch / non-finite total throws.
 */
export async function differenceDba(
  aDba: readonly number[],
  bDba: readonly number[],
): Promise<number[]> {
  await ensureWasm();
  try {
    const out = wasmDifferenceDba(Float64Array.from(aDba), Float64Array.from(bDba)) as Float64Array;
    return Array.from(out);
  } catch (err) {
    onWasmError(err);
    throw err;
  }
}
