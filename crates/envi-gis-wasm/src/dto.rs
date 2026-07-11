//! Serde + ts-rs boundary DTOs for the WASM import API (DATA-01..03).
//!
//! # Module I/O
//! - **Inputs:** the request DTOs (`*Req`) deserialized from a JS object by
//!   [`serde_wasm_bindgen`] in [`crate`]'s `#[wasm_bindgen]` functions. Every
//!   request-facing type is `#[serde(deny_unknown_fields)]` so a typo'd key is a
//!   loud error at the boundary, never a silently-ignored field.
//! - **Output:** the result DTOs (`*Result`) serialized back to a JS value.
//! - **Invariant (load-bearing, Phase-7 D-10):** every type here derives
//!   [`ts_rs::TS`] with `#[ts(export_to = "wire.ts")]` and is registered in the
//!   `wire_no_drift` no-drift test — the TypeScript mirror is **generated from
//!   this Rust source and committed**, never hand-authored. A renamed/added field
//!   fails the Rust test, not the browser.
//! - **No raw bytes here (perf):** tile bytes cross the boundary as a direct
//!   `&[u8]` function parameter (efficient typed-array marshalling), NOT as a
//!   field of a serde DTO (which would be a slow `Array<number>`).
//! - **No ids, no getrandom (Pitfall 9):** feature `id`s are assigned in TS via
//!   `crypto.randomUUID()`; nothing here mints an id.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// --- Shared value objects -------------------------------------------------

/// A pixel-space window into a COG base image (mirrors
/// `envi_gis::cog::PixelWindow`). Both request-facing (decode inputs) and
/// result-facing (the resolved [`WindowForBboxResult`] window), so it derives
/// `Serialize` too; `deny_unknown_fields` still guards the deserialize side.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct PixelWindowDto {
    /// Leftmost column (inclusive).
    pub col_off: u32,
    /// Topmost row (inclusive).
    pub row_off: u32,
    /// Window width in pixels (`> 0`).
    pub width: u32,
    /// Window height in pixels (`> 0`).
    pub height: u32,
}

/// A WGS84 lon/lat bounding box in degrees (mirrors `envi_gis::registry::Bbox`).
/// Request-facing.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct BboxDto {
    /// Western edge, degrees east.
    pub min_lon: f64,
    /// Southern edge, degrees north.
    pub min_lat: f64,
    /// Eastern edge, degrees east.
    pub max_lon: f64,
    /// Northern edge, degrees north.
    pub max_lat: f64,
}

/// The source CRS of a terrain raster (mirrors
/// `envi_gis::terrain::TerrainSourceCrs`). Request-facing.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "wire.ts")]
pub enum TerrainSourceCrsDto {
    /// Dutch RD New (EPSG:28992) meters — AHN.
    RdNew,
    /// WGS84 (EPSG:4326) lon/lat degrees — GLO-30.
    Wgs84,
}

/// A vertical datum note for terrain samples. Maps to a `'static` label, so the
/// provenance stays owned by fixed strings (no leaked runtime `&'static str`).
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "wire.ts")]
pub enum VerticalDatumDto {
    /// Dutch NAP (AHN terrain).
    Nap,
    /// EGM2008 (GLO-30 terrain).
    Egm2008,
}

/// Per-feature provenance the TS orchestrator supplies for an imported layer
/// (D-11). `source_id` is a registry id (`"ahn4-dtm"`, `"glo30"`, `"worldcover"`,
/// `"osm-overpass"`); the boundary resolves the `'static` source + license from
/// the registry so this crate never restates a license literal. Request-facing.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct ProvenanceReqDto {
    /// Registry source id (resolves the `'static` source + license).
    pub source_id: String,
    /// Per-feature source reference (tile name / OSM element ref).
    pub source_ref: String,
    /// ISO-8601 retrieval timestamp (assigned by TS).
    pub retrieved_at: String,
    /// Optional vertical datum note (terrain only).
    #[serde(default)]
    pub vertical_datum: Option<VerticalDatumDto>,
}

// --- Request DTOs ---------------------------------------------------------

/// `plan_import` request: pick each layer's source for a viewport.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct ImportPlanReq {
    /// The WGS84 import viewport.
    pub bbox: BboxDto,
}

/// `plan_tiles` request: enumerate the covering source tiles for a viewport.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct PlanTilesReq {
    /// The WGS84 import viewport.
    pub bbox: BboxDto,
}

/// `window_for_bbox` request (tile bytes are a separate `&[u8]` parameter): the
/// WGS84 viewport + the tile's source CRS, resolved to a [`PixelWindowDto`].
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct WindowForBboxReq {
    /// The WGS84 import viewport to window into this tile.
    pub bbox: BboxDto,
    /// The tile raster's source CRS (RD New reprojects through `envi_geo`).
    pub source_crs: TerrainSourceCrsDto,
    /// Optional decoded-pixel budget override (defaults to `MAX_DECODED_PX`).
    #[serde(default)]
    pub max_decoded_px: Option<u32>,
}

/// `decode_window` request (tile bytes are a separate `&[u8]` parameter).
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct DecodeWindowReq {
    /// The pixel window to decode.
    pub window: PixelWindowDto,
    /// Optional decoded-pixel budget override (defaults to `MAX_DECODED_PX`).
    #[serde(default)]
    pub max_decoded_px: Option<u32>,
}

/// `terrain_features` request (tile bytes are a separate `&[u8]` parameter).
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct TerrainFeaturesReq {
    /// The terrain pixel window to decode.
    pub window: PixelWindowDto,
    /// Target decimated sample count (clamped to the engine cap).
    pub target_points: u32,
    /// The raster's source CRS.
    pub source_crs: TerrainSourceCrsDto,
    /// Provenance to stamp on each emitted `elevation_point`.
    pub provenance: ProvenanceReqDto,
    /// Optional decoded-pixel budget override.
    #[serde(default)]
    pub max_decoded_px: Option<u32>,
}

/// `sample_base_elevation` request (tile bytes are a separate `&[u8]` parameter).
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct BaseElevationReq {
    /// The terrain pixel window to decode.
    pub window: PixelWindowDto,
    /// Footprint exterior ring `[x, y]` in the terrain raster's source CRS.
    pub ring: Vec<[f64; 2]>,
    /// Maximum boundary-sample spacing (source-CRS units).
    pub max_spacing_m: f64,
    /// Optional decoded-pixel budget override.
    #[serde(default)]
    pub max_decoded_px: Option<u32>,
}

/// `map_landcover` request (WorldCover tile bytes are a separate `&[u8]` param).
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct MapLandcoverReq {
    /// The land-cover pixel window to decode (`u8` class raster).
    pub window: PixelWindowDto,
    /// Optional minimum polygon area in pixels² (defaults to the module const).
    #[serde(default)]
    pub min_area_px: Option<f64>,
    /// Optional Douglas–Peucker tolerance in pixels (defaults to the module const).
    #[serde(default)]
    pub simplify_tol_px: Option<f64>,
    /// Provenance to stamp on each emitted `ground_zone`.
    pub provenance: ProvenanceReqDto,
    /// Optional decoded-pixel budget override.
    #[serde(default)]
    pub max_decoded_px: Option<u32>,
}

/// `parse_buildings` request: an Overpass `out geom` JSON string.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct ParseBuildingsReq {
    /// The Overpass `out geom` response body.
    pub overpass_json: String,
    /// Fallback eaves height (meters) when no height/levels tag resolves.
    pub user_default_height_m: f64,
    /// ISO-8601 retrieval timestamp.
    pub retrieved_at: String,
}

/// `merge_features` request: re-import merge of two GeoJSON `FeatureCollection`s.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct MergeReq {
    /// The existing scene features as a GeoJSON `FeatureCollection`.
    #[ts(type = "unknown")]
    pub existing: serde_json::Value,
    /// The fresh import as a GeoJSON `FeatureCollection`.
    #[ts(type = "unknown")]
    pub incoming: serde_json::Value,
}

// --- Result DTOs ----------------------------------------------------------

/// The browser-side reach of a source (mirrors `envi_gis::registry::Cors`).
#[derive(Debug, Clone, Copy, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "wire.ts")]
pub enum CorsDto {
    /// Fetched cross-origin directly from the browser.
    Direct,
    /// Routed through the same-origin allowlisted byte proxy.
    Proxy,
}

/// The GIS data kind of a source (mirrors `envi_gis::registry::SourceKind`).
#[derive(Debug, Clone, Copy, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "wire.ts")]
pub enum SourceKindDto {
    /// Bare-earth digital terrain model.
    Dtm,
    /// Digital surface model.
    Dsm,
    /// Land-cover raster.
    Landcover,
    /// Vector building footprints.
    Buildings,
}

/// One selected source descriptor (mirrors `envi_gis::registry::SourceDescriptor`
/// minus the coverage polygon). Everything TS needs to fetch + attribute a layer.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct SourceDescriptorDto {
    /// Stable identifier.
    pub id: String,
    /// The data kind this source provides.
    pub kind: SourceKindDto,
    /// Native CRS EPSG label.
    pub crs: String,
    /// How tiles/queries are addressed.
    pub tile_scheme: String,
    /// The fetch URL template (TS fills the `{...}` slots).
    pub endpoint_template: String,
    /// Verified browser CORS reachability.
    pub cors: CorsDto,
    /// SPDX-ish license tag.
    pub license: String,
    /// Human-readable attribution string.
    pub attribution: String,
}

/// `plan_import` result: the per-layer source selection for a viewport.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct ImportPlanResult {
    /// Terrain source (AHN4 in NL, GLO-30 fallback).
    pub terrain: SourceDescriptorDto,
    /// Land-cover source (WorldCover).
    pub landcover: SourceDescriptorDto,
    /// Buildings source (OSM Overpass).
    pub buildings: SourceDescriptorDto,
}

/// A north-up geotransform (mirrors `envi_gis::cog::geo_tags::GeoTransform`).
#[derive(Debug, Clone, Copy, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct GeoTransformDto {
    /// Map x of the top-left corner of pixel `(0, 0)`.
    pub origin_x: f64,
    /// Map y of the top-left corner of pixel `(0, 0)`.
    pub origin_y: f64,
    /// Signed pixel size in x (`> 0` north-up).
    pub pixel_size_x: f64,
    /// Signed pixel size in y (`< 0` north-up).
    pub pixel_size_y: f64,
}

/// `decode_window` result: a windowed `f32` raster with its geotransform. Holes
/// (nodata / non-finite) are `null` samples — never a silent `0.0`.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct DecodeWindowResult {
    /// Window width in pixels.
    pub width: u32,
    /// Window height in pixels.
    pub height: u32,
    /// Geotransform of this window (origin at its top-left pixel).
    pub geo: GeoTransformDto,
    /// Row-major samples; `null` = hole (dropped nodata / non-finite).
    pub samples: Vec<Option<f32>>,
}

/// `terrain_features` result: a GeoJSON `FeatureCollection` of WGS84
/// `elevation_point` features (typed `unknown`; validated by the store on PUT).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct TerrainFeaturesResult {
    /// GeoJSON `FeatureCollection` of `elevation_point` features.
    #[ts(type = "unknown")]
    pub features: serde_json::Value,
}

/// `sample_base_elevation` result: the footprint-boundary median ground height,
/// or `null` when terrain is absent (never a fabricated `0.0`, D-07).
#[derive(Debug, Clone, Copy, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct BaseElevationResult {
    /// Boundary-median base elevation (meters), or `null`.
    pub base_elevation_m: Option<f64>,
}

/// `map_landcover` result: a GeoJSON `FeatureCollection` of `ground_zone`
/// features (typed `unknown`; validated by the store on PUT).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct LandcoverResult {
    /// GeoJSON `FeatureCollection` of `ground_zone` features.
    #[ts(type = "unknown")]
    pub features: serde_json::Value,
}

/// A skipped Overpass element and why (mirrors `envi_gis::buildings::SkipReport`).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct SkipReportDto {
    /// The `(type/id)` reference of the skipped element.
    pub source_ref: String,
    /// Why it was skipped.
    pub reason: String,
}

/// `parse_buildings` result: `building` features plus per-element skip reports.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct BuildingsResult {
    /// GeoJSON `FeatureCollection` of `building` features.
    #[ts(type = "unknown")]
    pub features: serde_json::Value,
    /// Elements skipped (invalid geometry), never failing the whole layer.
    pub skipped: Vec<SkipReportDto>,
}

/// `merge_features` result: the merged GeoJSON `FeatureCollection`.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct MergeResult {
    /// GeoJSON `FeatureCollection` after the D-09 re-import merge.
    #[ts(type = "unknown")]
    pub features: serde_json::Value,
}

/// One covering source tile (mirrors `envi_gis::tiles::TileRef`): the absolute
/// upstream fetch URL + the stable per-tile cache key / provenance `source_ref`.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct TileRefDto {
    /// Registry source id (`"ahn4-dtm"`, `"glo30"`, `"worldcover"`).
    pub source_id: String,
    /// Tile identifier — cache key + provenance `source_ref`.
    pub tile: String,
    /// Absolute upstream fetch URL (Direct sources fetch it as-is; Proxy sources
    /// are rewritten to `/api/v1/proxy/{source_id}/{path}` by the TS fetcher).
    pub url: String,
}

/// `plan_tiles` result: the covering tiles per raster layer (buildings are a
/// bbox-query, handled TS-side, so they carry no tile list here).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct PlanTilesResult {
    /// Covering terrain tiles (AHN kaartblads in NL, else GLO-30 1° cells).
    pub terrain: Vec<TileRefDto>,
    /// Covering WorldCover 3° land-cover tiles.
    pub landcover: Vec<TileRefDto>,
}

/// `window_for_bbox` result: the resolved pixel window, or `null` when the
/// viewport covers no in-image pixel (never a guessed full-tile clamp, D-07).
#[derive(Debug, Clone, Copy, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct WindowForBboxResult {
    /// The pixel window to decode, or `null` for no overlap.
    pub window: Option<PixelWindowDto>,
}
