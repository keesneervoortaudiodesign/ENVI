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

// The `SoundSpeedProfileDto` wire type was factored into `envi_compute::scene_dto`
// (Phase 10, 10-06) so both wasm crates share ONE wire type (the solve boundary
// consumes it as a request; the weather-derive boundary here produces it as a
// result). Re-exported at its original path so `envi_gis_wasm::dto::SoundSpeedProfileDto`
// stays source-compatible and the committed `wire.ts` is byte-identical (one export).
pub use envi_compute::scene_dto::SoundSpeedProfileDto;

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

/// `reproject_ring` request: a WGS84 `[lon, lat]` footprint ring to reproject into
/// a terrain tile's source CRS (so it can feed `sample_base_elevation`).
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct ReprojectRingReq {
    /// Footprint exterior ring `[lon, lat]` in WGS84 degrees.
    pub ring: Vec<[f64; 2]>,
    /// The terrain raster's source CRS (RD New reprojects through `envi_geo`).
    pub source_crs: TerrainSourceCrsDto,
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

/// `reproject_ring` result: the ring in the terrain tile's source CRS.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct ReprojectRingResult {
    /// Footprint exterior ring `[x, y]` in the source CRS (RD meters / WGS84).
    pub ring: Vec<[f64; 2]>,
}

// --- Phase-9 geometry + weather boundary DTOs (GEOX/GRID/METX) -------------
//
// The Phase-9 `envi-gis` core (`profile`/`impedance`/`screening`/`grid`/
// `weather`/`era5`) is exposed to the browser through these DTOs + the shims in
// [`crate`]. Every geometry fn that needs the DGM surface takes the elevation
// points (`tin_points` + optional `tin_breaklines`) as data — the shim rebuilds
// the sans-I/O `envi_dgm` TIN at the boundary (no persistent handle crosses the
// wire). GeoJSON-free: rings/lines cross as `Vec<[f64; 2]>` planar coordinate
// lists, never a `serde_json::Value`.

/// One ground segment `(σ, roughness)` on a derived cut-plane — the geometry
/// pipeline's mirror of `envi_engine::scene::GroundSegment` (a distinct wire type
/// from the HTTP scene's `GroundSegmentDto`, so the two boundaries stay
/// independent single-sources). Both request- and result-facing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct ProfileSegmentDto {
    /// Ground flow resistivity σ, kNs·m⁻⁴ (resolved through the engine only).
    pub flow_resistivity: f64,
    /// Terrain roughness, meters (class N = 0).
    pub roughness: f64,
}

/// The `(x, z)` + planar cut-plane and its per-interval segments — the wire mirror
/// of `envi_gis::impedance::GroundSegmentation`. It is BOTH a result (of
/// cut/segment) and a request input (the `base` screening consumes), so it derives
/// both serde sides. `segments.len() == points.len() − 1`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct GroundSegmentationDto {
    /// The `(x, z)` profile points, strictly ascending in x.
    pub points: Vec<[f64; 2]>,
    /// The planar `(x, y)` preimage of each point on the S→R line.
    pub planar_xy: Vec<[f64; 2]>,
    /// One segment per interval (`len() == points.len() − 1`).
    pub segments: Vec<ProfileSegmentDto>,
}

/// A user-drawn ground-impedance zone (wire mirror of
/// `envi_gis::impedance::DrawnZone`): a planar exterior ring + explicit class
/// letter + roughness. Request-facing.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct DrawnZoneDto {
    /// The zone footprint exterior ring in planar scene coordinates `(x, y)`.
    pub polygon: Vec<[f64; 2]>,
    /// The Nord2000 impedance class letter (`A..=H`); a single character.
    pub class: String,
    /// Terrain roughness in meters (class N = `0.0`).
    pub roughness_m: f64,
}

/// An imported land-cover ground zone (wire mirror of
/// `envi_gis::impedance::ImportedZone`): a planar exterior ring + WorldCover code.
/// Request-facing.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct ImportedZoneDto {
    /// The zone footprint exterior ring in planar scene coordinates `(x, y)`.
    pub polygon: Vec<[f64; 2]>,
    /// ESA WorldCover v200 class code (resolved to a letter through the engine).
    pub worldcover_code: u8,
}

/// A height-bearing scene object that screens along a path (wire mirror of
/// `envi_gis::screening::ScreenObject`, D-08). Externally tagged. Request-facing.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "wire.ts")]
pub enum ScreenObjectDto {
    /// A building: the footprint exterior ring screens at `eaves_height_m`.
    Building {
        /// Footprint exterior ring in planar scene coordinates `(x, y)`.
        footprint: Vec<[f64; 2]>,
        /// Eaves height above local ground, meters.
        eaves_height_m: f64,
    },
    /// A wall or barrier: the top polyline screens at `height_m`.
    Barrier {
        /// Barrier/wall top polyline in planar scene coordinates `(x, y)`.
        line: Vec<[f64; 2]>,
        /// Screen top height above local ground, meters.
        height_m: f64,
    },
}

/// The bearing-independent weather decomposition (wire mirror of
/// `envi_gis::weather::WeatherComponents`). Result-facing.
#[derive(Debug, Clone, Copy, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct WeatherComponentsDto {
    /// Isotropic temperature part of `A` (computed once), m/s.
    pub a_temp: f64,
    /// Wind magnitude of `A` (projected per bearing), m/s.
    pub a_wind: f64,
    /// Linear coefficient `B`, s⁻¹ (bearing-independent).
    pub b: f64,
    /// Ground sound speed `C`, m/s.
    pub c: f64,
    /// Std-dev of `A`, m/s.
    pub s_a: f64,
    /// Std-dev of `B`, s⁻¹.
    pub s_b: f64,
    /// Roughness length `z₀`, m.
    pub z0: f64,
}

/// One hour of ERA5 single-level fields (wire mirror of
/// `envi_gis::era5::Era5Hour`, ECMWF short names, SI units). Request-facing.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct Era5HourDto {
    /// Eastward turbulent surface stress `iews`, N/m².
    pub iews: f64,
    /// Northward turbulent surface stress `inss`, N/m².
    pub inss: f64,
    /// Instantaneous surface sensible heat flux `ishf`, W/m² (downward-positive).
    pub ishf: f64,
    /// 2 m air temperature `2t`, K.
    pub t2m_k: f64,
    /// 2 m dewpoint temperature `2d`, K.
    pub d2m_k: f64,
    /// Surface pressure `sp`, Pa.
    pub sp_pa: f64,
    /// Std-dev of sub-grid orography `sdfor`, m (reliability gate).
    pub sdfor_m: f64,
    /// Eastward 10 m wind component `u10`, m/s.
    pub u10_ms: f64,
    /// Northward 10 m wind component `v10`, m/s.
    pub v10_ms: f64,
}

/// The wind-speed × stability occurrence table (wire mirror of
/// `envi_gis::era5::ClassOccurrence`, D-05: counts only). `counts[wind_bin]
/// [stability]`. Result-facing.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct ClassOccurrenceDto {
    /// `counts[wind_bin][stability]` — hours falling in each class.
    pub counts: Vec<Vec<u32>>,
    /// Total hours counted.
    pub total: u32,
    /// Hours flagged reliable (`sdfor < threshold`).
    pub reliable: u32,
}

/// `cut_profile` request: the DGM elevation points + the S→R endpoints + step.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct CutProfileReq {
    /// DGM elevation points `[x, y, z]` (scene meters) to build the TIN from.
    pub tin_points: Vec<[f64; 3]>,
    /// Optional breakline polylines forced as constrained edges.
    #[serde(default)]
    pub tin_breaklines: Vec<Vec<[f64; 2]>>,
    /// Source planar point `[x, y]`.
    pub s_xy: [f64; 2],
    /// Receiver planar point `[x, y]`.
    pub r_xy: [f64; 2],
    /// Sampling step along the S→R line, meters (the DEM cell size).
    pub step_m: f64,
}

/// `cut_profile` result: the strictly-ascending `(x, z)` ground points.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct CutProfileResult {
    /// The `(x, z)` cut-profile points (x = horizontal distance from the source).
    pub points: Vec<[f64; 2]>,
}

/// `segment_ground` request: the cut plane + drawn/imported zones + default class.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct SegmentGroundReq {
    /// The `(x, z)` cut-profile points (from [`CutProfileResult`]).
    pub points: Vec<[f64; 2]>,
    /// The planar `(x, y)` preimage of each point on the S→R line.
    pub planar_xy: Vec<[f64; 2]>,
    /// User-drawn zones (highest priority).
    #[serde(default)]
    pub drawn_zones: Vec<DrawnZoneDto>,
    /// Imported land-cover zones (middle priority).
    #[serde(default)]
    pub imported_zones: Vec<ImportedZoneDto>,
    /// The fallback impedance class letter (`A..=H`); a single character.
    pub default_class: String,
}

/// `inject_screens` request: a base segmentation + the screening objects + TIN.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct InjectScreensReq {
    /// The boundary-spliced base segmentation (from `segment_ground`).
    pub base: GroundSegmentationDto,
    /// The height-bearing screening objects (buildings + walls + barriers).
    #[serde(default)]
    pub screens: Vec<ScreenObjectDto>,
    /// DGM elevation points `[x, y, z]` to build the TIN the tops ride on.
    pub tin_points: Vec<[f64; 3]>,
    /// Optional breakline polylines forced as constrained edges.
    #[serde(default)]
    pub tin_breaklines: Vec<Vec<[f64; 2]>>,
}

/// `receiver_grid` request: the calc-area ring + footprint holes + spacing + TIN.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct ReceiverGridReq {
    /// The `calc_area` exterior ring in planar scene coordinates `(x, y)`.
    pub calc_area: Vec<[f64; 2]>,
    /// Building footprint exterior rings (grid holes, D-07).
    #[serde(default)]
    pub footprints: Vec<Vec<[f64; 2]>>,
    /// Lattice spacing, meters (default applied by the caller; min-guarded).
    pub spacing_m: f64,
    /// Extra discrete receiver points `[x, y]` appended verbatim.
    #[serde(default)]
    pub discrete_points: Vec<[f64; 2]>,
    /// DGM elevation points `[x, y, z]` the receiver z is sampled from.
    pub tin_points: Vec<[f64; 3]>,
    /// Optional breakline polylines forced as constrained edges.
    #[serde(default)]
    pub tin_breaklines: Vec<Vec<[f64; 2]>>,
}

/// `receiver_grid` result: the receiver positions `[x, y, z]` (ground z only).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct ReceiverGridResult {
    /// Receiver positions `[x, y, z]`; z is GROUND elevation (acoustic height is
    /// added at `SolveJob` assembly — the hSv/hRv trap).
    pub receivers: Vec<[f64; 3]>,
}

/// `derive_weather` request: an Open-Meteo multi-level JSON body + the reference
/// downwind bearing + roughness + the path azimuths to project A onto.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct WeatherDeriveReq {
    /// The Open-Meteo Archive/Forecast pressure-level JSON response body.
    pub openmeteo_json: String,
    /// The hour index into the Open-Meteo time series to derive.
    pub hour_index: u32,
    /// Reference downwind bearing φ_wind, degrees clockwise from north.
    pub phi_wind_deg: f64,
    /// Roughness length z₀, meters (clamped ≥ 0.001 m).
    pub z0: f64,
    /// Path azimuths (degrees clockwise from north) to emit a profile for.
    pub path_azimuths_deg: Vec<f64>,
}

/// `derive_weather` result: the bearing-independent components + one profile per
/// requested path azimuth.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct WeatherDeriveResult {
    /// The bearing-independent decomposition (temperature + wind parts).
    pub components: WeatherComponentsDto,
    /// One `SoundSpeedProfile` per `path_azimuths_deg` entry (same order).
    pub profiles: Vec<SoundSpeedProfileDto>,
}

/// A raw per-azimuth `(A, B, C, z₀)` advanced override (METX-03, D-14): the
/// expert bypass that sets the sound-speed profile directly, skipping the
/// friendly-knob → A/B/C derivation. Applied to EVERY requested azimuth.
/// Request-facing.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct RawProfileDto {
    /// Logarithmic coefficient `A`, m/s.
    pub a: f64,
    /// Linear coefficient `B`, s⁻¹.
    pub b: f64,
    /// Ground sound speed `C`, m/s.
    pub c: f64,
    /// Roughness length `z₀`, m (clamped ≥ 0.001 m).
    pub z0: f64,
}

/// `derive_weather_friendly` request: the FRIENDLY what-if met knobs (METX-03/04,
/// D-14) — a surface temperature, a temperature gradient, a wind speed (already
/// converted from a Beaufort class by the caller) + the direction it blows FROM, a
/// roughness length, and a downwind-worst-case toggle — that drive the SAME
/// `envi_gis::weather` per-azimuth A/B/C derivation the Open-Meteo path uses.
///
/// `downwind_worst_case` (D-15): assume downward-refraction (favourable) along
/// EACH `path_azimuths_deg` bearing INDEPENDENTLY — the standard Nord2000
/// worst-case envelope — by projecting the wind part as if every bearing were the
/// downwind bearing (`A = a_temp + a_wind` per azimuth).
///
/// `raw_override` (advanced): when present, the friendly knobs are ignored and the
/// raw `(A, B, C, z₀)` is emitted for every azimuth verbatim.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct FriendlyWeatherReq {
    /// Surface air temperature, °C.
    pub temperature_c: f64,
    /// Temperature gradient dT/dz, °C/m (positive = inversion ⇒ B > 0).
    pub temp_gradient_c_per_m: f64,
    /// Near-surface wind speed, m/s (Beaufort class → m/s is done caller-side).
    pub wind_speed_ms: f64,
    /// Meteorological wind direction it blows FROM, degrees clockwise from north.
    pub wind_from_deg: f64,
    /// Roughness length z₀, meters (clamped ≥ 0.001 m).
    pub z0: f64,
    /// Downwind worst-case (D-15): favourable downward-refraction per bearing.
    pub downwind_worst_case: bool,
    /// Path azimuths (degrees clockwise from north) to emit a profile for.
    pub path_azimuths_deg: Vec<f64>,
    /// Advanced raw per-azimuth `(A, B, C, z₀)` override (skips the derivation).
    #[serde(default)]
    pub raw_override: Option<RawProfileDto>,
}

/// `derive_era5` request: a batch of ERA5 single-level hours.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct Era5DeriveReq {
    /// The ERA5 single-level hours to derive occurrence statistics from.
    pub hours: Vec<Era5HourDto>,
}

/// `derive_era5` result: the occurrence table + the per-hour inverse Obukhov
/// length (D-05 — occurrence statistics only, no class → A/B/C, no L_den).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct Era5DeriveResult {
    /// The wind-speed × stability occurrence table.
    pub occurrence: ClassOccurrenceDto,
    /// The per-hour inverse Obukhov length `1/L` (m⁻¹), same order as the request.
    pub inv_l: Vec<f64>,
}
