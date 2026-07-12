//! # envi-gis-wasm
//!
//! The **WASM boundary** for the client-side GIS-ingestion core (DATA-01..03).
//! This is the first WASM crate in the repo: a thin `cdylib` that exposes
//! [`envi_gis`] to the browser over `wasm-bindgen`.
//!
//! # Boundary ONLY — no logic here (mirror of `envi-service`'s thin-handler rule)
//!
//! Every `#[wasm_bindgen]` function does exactly three things: deserialize a JS
//! value into a serde DTO ([`serde_wasm_bindgen`]), call the corresponding
//! [`envi_gis`] core function, and serialize the result back. **No parsing,
//! geometry, decode, or GIS math lives here** — that all belongs to `envi-gis`
//! (`decode_window` / `terrain_features` / `sample_base_elevation` /
//! `vectorize_landcover` / `buildings_from_overpass` / `merge`). Tile bytes cross
//! as a direct `&[u8]` parameter (efficient typed-array marshalling), never as a
//! serde field.
//!
//! # Wire-type discipline (Phase-7 D-10)
//!
//! All boundary DTOs live in [`dto`], derive `ts_rs::TS`, and are generated into
//! the committed `web/src/generated/wire.ts` with a no-drift test — no
//! hand-authored TS mirror. Request DTOs are `#[serde(deny_unknown_fields)]`.
//!
//! # No `getrandom`/`uuid` (Pitfall 9)
//!
//! Feature `id`s are assigned in TypeScript via `crypto.randomUUID()`; this crate
//! mints no ids and pulls in no random-number source.
#![deny(unsafe_code)]

pub mod dto;

use geojson::{Feature, FeatureCollection};
use serde::Serialize;
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;

use geo::{Coord, LineString, Polygon};

use envi_dgm::tin::{Tin, build_tin};
use envi_gis::GisError;
use envi_gis::buildings::buildings_from_overpass;
use envi_gis::cog::{MAX_DECODED_PX, PixelWindow, Raster, decode_window_u8};
use envi_gis::era5::{Era5Hour, N_STABILITY, occurrence_stats_with_inv_l};
use envi_gis::grid;
use envi_gis::impedance::{DrawnZone, GroundSegmentation, ImportedZone, segment_ground};
use envi_gis::landcover::{DEFAULT_MIN_AREA_PX, DEFAULT_SIMPLIFY_TOL_PX, vectorize_landcover};
use envi_gis::merge::merge;
use envi_gis::profile::cut_profile;
use envi_gis::provenance::Provenance;
use envi_gis::registry::{self, Bbox, Cors, SourceDescriptor, SourceKind};
use envi_gis::screening::{ScreenObject, inject_screens};
use envi_gis::terrain::{TerrainSourceCrs, base_elevation_on_raster};
use envi_gis::tiles::{self, TileRef};
use envi_gis::weather::{
    components_from_friendly, components_from_levels, levels_from_openmeteo,
    sound_speed_profile_for_azimuth,
};

use dto::{
    BaseElevationReq, BaseElevationResult, BboxDto, BuildingsResult, ClassOccurrenceDto, CorsDto,
    CutProfileReq, CutProfileResult, DecodeWindowReq, DecodeWindowResult, DrawnZoneDto,
    Era5DeriveReq, Era5DeriveResult, Era5HourDto, FriendlyWeatherReq, GeoTransformDto,
    GroundSegmentationDto, ImportPlanReq, ImportPlanResult, ImportedZoneDto, InjectScreensReq,
    LandcoverResult, MapLandcoverReq, MergeReq, MergeResult, ParseBuildingsReq, PixelWindowDto,
    PlanTilesReq, PlanTilesResult, ProfileSegmentDto, ProvenanceReqDto, RawProfileDto,
    ReceiverGridReq, ReceiverGridResult, ReprojectRingReq, ReprojectRingResult, ScreenObjectDto,
    SegmentGroundReq, SkipReportDto, SoundSpeedProfileDto, SourceDescriptorDto, SourceKindDto,
    TerrainFeaturesReq, TerrainFeaturesResult, TerrainSourceCrsDto, TileRefDto, VerticalDatumDto,
    WeatherComponentsDto, WeatherDeriveReq, WeatherDeriveResult, WindowForBboxReq,
    WindowForBboxResult,
};

// --- Marshalling helpers (the ONLY glue; no domain logic) -----------------

/// Deserialize a JS value into a request DTO, mapping a shape error to `JsValue`.
fn from_js<T: DeserializeOwned>(v: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(v).map_err(|e| js_err(&e.to_string()))
}

/// Serialize a result DTO back to a JS value.
///
/// `serde_wasm_bindgen` serializes maps — including every `serde_json::Value::Object`
/// inside the GeoJSON `features` payloads — as a JS `Map` by default. The TS import
/// path reads result DTOs as PLAIN objects (`res.features.features`, `res.window`, …),
/// so maps MUST serialize as plain JS objects or the whole compute pipeline reads
/// `undefined` off a `Map` (DATA-01..03). Structs/enums already serialize as objects;
/// only the nested `serde_json::Value` GeoJSON payloads depend on this flag.
fn to_js<T: Serialize>(v: &T) -> Result<JsValue, JsValue> {
    let serializer = serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true);
    v.serialize(&serializer).map_err(|e| js_err(&e.to_string()))
}

/// Build a `JsValue` error carrying `msg` (an `Error` on the JS side).
fn js_err(msg: &str) -> JsValue {
    JsError::new(msg).into()
}

/// Map an [`envi_gis::GisError`] to a `JsValue` error (the typed-error core keeps
/// the boundary from panicking on data — threat T-08-06-03).
fn gis_err(e: GisError) -> JsValue {
    js_err(&e.to_string())
}

/// The effective decoded-pixel budget (override or the core default).
fn budget(max_decoded_px: Option<u32>) -> usize {
    max_decoded_px.map_or(MAX_DECODED_PX, |v| v as usize)
}

/// A core [`PixelWindow`] from its DTO.
fn pixel_window(w: PixelWindowDto) -> PixelWindow {
    PixelWindow {
        col_off: w.col_off,
        row_off: w.row_off,
        width: w.width,
        height: w.height,
    }
}

/// A [`PixelWindowDto`] from a core [`PixelWindow`] (the resolved-window result).
fn pixel_window_dto(w: PixelWindow) -> PixelWindowDto {
    PixelWindowDto {
        col_off: w.col_off,
        row_off: w.row_off,
        width: w.width,
        height: w.height,
    }
}

/// The core [`TerrainSourceCrs`] from its DTO.
fn terrain_source_crs(c: TerrainSourceCrsDto) -> TerrainSourceCrs {
    match c {
        TerrainSourceCrsDto::RdNew => TerrainSourceCrs::RdNew,
        TerrainSourceCrsDto::Wgs84 => TerrainSourceCrs::Wgs84,
    }
}

/// A [`TileRefDto`] from a core [`TileRef`].
fn tile_ref_dto(t: &TileRef) -> TileRefDto {
    TileRefDto {
        source_id: t.source_id.to_string(),
        tile: t.tile.clone(),
        url: t.url.clone(),
    }
}

/// Build a `'static`-string [`Provenance`] from the DTO by resolving the source id
/// through the registry (single source of truth for source id + license — this
/// crate never restates a license literal). Terrain vertical datum maps to a
/// fixed label; `height_provenance` is set by the buildings core, not here.
fn build_provenance(p: &ProvenanceReqDto) -> Result<Provenance, JsValue> {
    // `registry::source` keys on `&'static str`; resolve a runtime id by scanning
    // the static table so the boundary can look up by an owned string.
    let desc = registry::registry()
        .iter()
        .find(|d| d.id == p.source_id.as_str())
        .ok_or_else(|| js_err(&format!("unknown registry source id: {}", p.source_id)))?;
    Ok(Provenance {
        source: desc.id,
        source_ref: p.source_ref.clone(),
        license: desc.license,
        retrieved_at: p.retrieved_at.clone(),
        height_provenance: None,
        vertical_datum: p.vertical_datum.map(|d| match d {
            VerticalDatumDto::Nap => "NAP",
            VerticalDatumDto::Egm2008 => "EGM2008",
        }),
    })
}

/// Wrap core features as a GeoJSON `FeatureCollection` JSON value for the wire.
fn features_to_value(features: Vec<Feature>) -> Result<serde_json::Value, JsValue> {
    let fc = FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    };
    serde_json::to_value(&fc).map_err(|e| js_err(&e.to_string()))
}

/// Parse a GeoJSON `FeatureCollection` JSON value into core features.
fn value_to_features(v: serde_json::Value) -> Result<Vec<Feature>, JsValue> {
    let fc: FeatureCollection = serde_json::from_value(v)
        .map_err(|e| js_err(&format!("invalid FeatureCollection: {e}")))?;
    Ok(fc.features)
}

/// Map a registry descriptor to its wire DTO (no coverage polygon on the wire).
fn descriptor_dto(d: &SourceDescriptor) -> SourceDescriptorDto {
    SourceDescriptorDto {
        id: d.id.to_string(),
        kind: match d.kind {
            SourceKind::Dtm => SourceKindDto::Dtm,
            SourceKind::Dsm => SourceKindDto::Dsm,
            SourceKind::Landcover => SourceKindDto::Landcover,
            SourceKind::Buildings => SourceKindDto::Buildings,
        },
        crs: d.crs.to_string(),
        tile_scheme: d.tile_scheme.to_string(),
        endpoint_template: d.endpoint_template.to_string(),
        cors: match d.cors {
            Cors::Direct => CorsDto::Direct,
            Cors::Proxy => CorsDto::Proxy,
        },
        license: d.license.to_string(),
        attribution: d.attribution.to_string(),
    }
}

/// Build the sans-I/O DGM [`Tin`] from the boundary's elevation points +
/// breaklines (input marshalling — the geometry math is `envi_dgm`'s, not the
/// shim's). A degenerate/oversized/self-crossing point set is a typed error.
fn build_gis_tin(points: &[[f64; 3]], breaklines: &[Vec<[f64; 2]>]) -> Result<Tin, JsValue> {
    build_tin(points, breaklines).map_err(|e| js_err(&e.to_string()))
}

/// A geo [`Polygon`] from a planar exterior ring (no holes — footprints are their
/// own rings). Pure marshalling of the coordinate list.
fn polygon_from_ring(ring: &[[f64; 2]]) -> Polygon<f64> {
    let coords: Vec<Coord<f64>> = ring.iter().map(|p| Coord { x: p[0], y: p[1] }).collect();
    Polygon::new(LineString::from(coords), Vec::new())
}

/// A geo [`LineString`] from a planar polyline. Pure marshalling.
fn line_from_pts(pts: &[[f64; 2]]) -> LineString<f64> {
    LineString::from(
        pts.iter()
            .map(|p| Coord { x: p[0], y: p[1] })
            .collect::<Vec<_>>(),
    )
}

/// Parse a single class letter out of a DTO string (never a fabricated σ — the
/// engine resolves the letter). A non-single-char string is a boundary error.
fn class_char(s: &str) -> Result<char, JsValue> {
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Ok(c),
        _ => Err(js_err(&format!(
            "impedance class must be a single letter, got {s:?}"
        ))),
    }
}

/// A core [`DrawnZone`] from its DTO.
fn drawn_zone(z: &DrawnZoneDto) -> Result<DrawnZone, JsValue> {
    Ok(DrawnZone {
        polygon: polygon_from_ring(&z.polygon),
        class: class_char(&z.class)?,
        roughness_m: z.roughness_m,
    })
}

/// A core [`ImportedZone`] from its DTO.
fn imported_zone(z: &ImportedZoneDto) -> ImportedZone {
    ImportedZone {
        polygon: polygon_from_ring(&z.polygon),
        worldcover_code: z.worldcover_code,
    }
}

/// A core [`ScreenObject`] from its DTO.
fn screen_object(s: &ScreenObjectDto) -> ScreenObject {
    match s {
        ScreenObjectDto::Building {
            footprint,
            eaves_height_m,
        } => ScreenObject::Building {
            footprint: polygon_from_ring(footprint),
            eaves_height_m: *eaves_height_m,
        },
        ScreenObjectDto::Barrier { line, height_m } => ScreenObject::Barrier {
            line: line_from_pts(line),
            height_m: *height_m,
        },
    }
}

/// Wire DTO from a core [`GroundSegmentation`].
fn segmentation_dto(seg: &GroundSegmentation) -> GroundSegmentationDto {
    GroundSegmentationDto {
        points: seg.points.clone(),
        planar_xy: seg.planar_xy.clone(),
        segments: seg
            .segments
            .iter()
            .map(|s| ProfileSegmentDto {
                flow_resistivity: s.flow_resistivity,
                roughness: s.roughness,
            })
            .collect(),
    }
}

/// Core [`GroundSegmentation`] from its wire DTO (the screening `base` input).
fn segmentation_from_dto(dto: &GroundSegmentationDto) -> GroundSegmentation {
    GroundSegmentation {
        points: dto.points.clone(),
        planar_xy: dto.planar_xy.clone(),
        segments: dto
            .segments
            .iter()
            .map(|s| envi_engine::scene::GroundSegment {
                flow_resistivity: s.flow_resistivity,
                roughness: s.roughness,
            })
            .collect(),
    }
}

/// Convert a core `f32` [`Raster`] into its wire DTO.
fn raster_dto(r: &Raster<f32>) -> DecodeWindowResult {
    DecodeWindowResult {
        width: r.width as u32,
        height: r.height as u32,
        geo: GeoTransformDto {
            origin_x: r.geo.origin_x,
            origin_y: r.geo.origin_y,
            pixel_size_x: r.geo.pixel_size_x,
            pixel_size_y: r.geo.pixel_size_y,
        },
        samples: r.samples.clone(),
    }
}

// --- Boundary functions (each delegates to exactly one envi-gis core path) --

/// Pick each layer's source for a WGS84 viewport (delegates to
/// `envi_gis::registry::plan`).
///
/// # Errors
/// A shape error in the request DTO.
#[wasm_bindgen]
pub fn plan_import(req: JsValue) -> Result<JsValue, JsValue> {
    let req: ImportPlanReq = from_js(req)?;
    let ImportPlanReq { bbox } = req;
    let BboxDto {
        min_lon,
        min_lat,
        max_lon,
        max_lat,
    } = bbox;
    let plan = registry::plan(Bbox {
        min_lon,
        min_lat,
        max_lon,
        max_lat,
    });
    to_js(&ImportPlanResult {
        terrain: descriptor_dto(plan.terrain),
        landcover: descriptor_dto(plan.landcover),
        buildings: descriptor_dto(plan.buildings),
    })
}

/// Enumerate the covering source tiles for a WGS84 viewport (delegates to
/// `envi_gis::tiles::plan_tiles`). Buildings are a bbox-query (TS-side), so the
/// result carries only the raster layers.
///
/// # Errors
/// A shape error in the request DTO.
#[wasm_bindgen]
pub fn plan_tiles(req: JsValue) -> Result<JsValue, JsValue> {
    let req: PlanTilesReq = from_js(req)?;
    let BboxDto {
        min_lon,
        min_lat,
        max_lon,
        max_lat,
    } = req.bbox;
    let plan = tiles::plan_tiles(Bbox {
        min_lon,
        min_lat,
        max_lon,
        max_lat,
    });
    to_js(&PlanTilesResult {
        terrain: plan.terrain.iter().map(tile_ref_dto).collect(),
        landcover: plan.landcover.iter().map(tile_ref_dto).collect(),
    })
}

/// Resolve the pixel window of a WGS84 viewport within a cached terrain/land-cover
/// tile (delegates to `envi_gis::tiles::window_for_bbox`), reprojecting through
/// `envi_geo` for RD-New sources. `null` window = the viewport does not overlap
/// this tile (never a guessed clamp).
///
/// # Errors
/// A shape error, or any [`GisError`] from the IFD read / reprojection / budget.
#[wasm_bindgen]
pub fn window_for_bbox(tile_bytes: &[u8], req: JsValue) -> Result<JsValue, JsValue> {
    let req: WindowForBboxReq = from_js(req)?;
    let BboxDto {
        min_lon,
        min_lat,
        max_lon,
        max_lat,
    } = req.bbox;
    let window = tiles::window_for_bbox(
        tile_bytes,
        Bbox {
            min_lon,
            min_lat,
            max_lon,
            max_lat,
        },
        terrain_source_crs(req.source_crs),
        budget(req.max_decoded_px),
    )
    .map_err(gis_err)?;
    to_js(&WindowForBboxResult {
        window: window.map(pixel_window_dto),
    })
}

/// Reproject a WGS84 `[lon, lat]` footprint ring into a terrain tile's source CRS
/// (delegates to `envi_gis::tiles::reproject_ring_to_source`) so it can feed
/// `sample_base_elevation` — the buildings layer's base-elevation seam (SC4).
///
/// # Errors
/// A shape error, or [`GisError::Reproject`] on a failed RD-New reprojection.
#[wasm_bindgen]
pub fn reproject_ring(req: JsValue) -> Result<JsValue, JsValue> {
    let req: ReprojectRingReq = from_js(req)?;
    let ring = tiles::reproject_ring_to_source(&req.ring, terrain_source_crs(req.source_crs))
        .map_err(gis_err)?;
    to_js(&ReprojectRingResult { ring })
}

/// Decode an `f32` terrain window from cached COG bytes (delegates to
/// `envi_gis::cog::decode_window`).
///
/// # Errors
/// A shape error, or any [`GisError`] from the guard-first decode.
#[wasm_bindgen]
pub fn decode_window(tile_bytes: &[u8], req: JsValue) -> Result<JsValue, JsValue> {
    let req: DecodeWindowReq = from_js(req)?;
    let raster = envi_gis::cog::decode_window(
        tile_bytes,
        pixel_window(req.window),
        budget(req.max_decoded_px),
    )
    .map_err(gis_err)?;
    to_js(&raster_dto(&raster))
}

/// Decode a terrain window and build WGS84 `elevation_point` features (delegates
/// to `envi_gis::cog::decode_window` + `envi_gis::terrain::terrain_features`).
///
/// # Errors
/// A shape error, an unknown source id, or any [`GisError`].
#[wasm_bindgen]
pub fn terrain_features(tile_bytes: &[u8], req: JsValue) -> Result<JsValue, JsValue> {
    let req: TerrainFeaturesReq = from_js(req)?;
    let raster = envi_gis::cog::decode_window(
        tile_bytes,
        pixel_window(req.window),
        budget(req.max_decoded_px),
    )
    .map_err(gis_err)?;
    let provenance = build_provenance(&req.provenance)?;
    let crs = terrain_source_crs(req.source_crs);
    let feats =
        envi_gis::terrain::terrain_features(&raster, req.target_points as usize, crs, &provenance)
            .map_err(gis_err)?;
    to_js(&TerrainFeaturesResult {
        features: features_to_value(feats)?,
    })
}

/// Footprint-boundary median base elevation from a decoded terrain window
/// (delegates to `envi_gis::cog::decode_window` +
/// `envi_gis::terrain::base_elevation_on_raster`).
///
/// # Errors
/// A shape error, or any [`GisError`] from the decode.
#[wasm_bindgen]
pub fn sample_base_elevation(tile_bytes: &[u8], req: JsValue) -> Result<JsValue, JsValue> {
    let req: BaseElevationReq = from_js(req)?;
    let raster = envi_gis::cog::decode_window(
        tile_bytes,
        pixel_window(req.window),
        budget(req.max_decoded_px),
    )
    .map_err(gis_err)?;
    let base = base_elevation_on_raster(&req.ring, req.max_spacing_m, &raster);
    to_js(&BaseElevationResult {
        base_elevation_m: base,
    })
}

/// Decode a WorldCover `u8` window and vectorize it into `ground_zone` features
/// (delegates to `envi_gis::cog::decode_window_u8` +
/// `envi_gis::landcover::vectorize_landcover`).
///
/// # Errors
/// A shape error, an unknown source id, or any [`GisError`].
#[wasm_bindgen]
pub fn map_landcover(tile_bytes: &[u8], req: JsValue) -> Result<JsValue, JsValue> {
    let req: MapLandcoverReq = from_js(req)?;
    let raster = decode_window_u8(
        tile_bytes,
        pixel_window(req.window),
        budget(req.max_decoded_px),
    )
    .map_err(gis_err)?;
    let provenance = build_provenance(&req.provenance)?;
    let feats = vectorize_landcover(
        &raster,
        req.min_area_px.unwrap_or(DEFAULT_MIN_AREA_PX),
        req.simplify_tol_px.unwrap_or(DEFAULT_SIMPLIFY_TOL_PX),
        &provenance,
    );
    to_js(&LandcoverResult {
        features: features_to_value(feats)?,
    })
}

/// Parse Overpass JSON into `building` features + skip reports (delegates to
/// `envi_gis::buildings::buildings_from_overpass`).
///
/// # Errors
/// A shape error, or [`GisError::Json`] for malformed Overpass JSON.
#[wasm_bindgen]
pub fn parse_buildings(req: JsValue) -> Result<JsValue, JsValue> {
    let req: ParseBuildingsReq = from_js(req)?;
    let (feats, skips) = buildings_from_overpass(
        &req.overpass_json,
        req.user_default_height_m,
        &req.retrieved_at,
    )
    .map_err(gis_err)?;
    to_js(&BuildingsResult {
        features: features_to_value(feats)?,
        skipped: skips
            .into_iter()
            .map(|s| SkipReportDto {
                source_ref: s.source_ref,
                reason: s.reason,
            })
            .collect(),
    })
}

/// Merge a fresh import into the existing scene by feature identity (delegates to
/// `envi_gis::merge::merge`).
///
/// # Errors
/// A shape error, or an invalid `FeatureCollection` on either side.
#[wasm_bindgen]
pub fn merge_features(req: JsValue) -> Result<JsValue, JsValue> {
    let req: MergeReq = from_js(req)?;
    let existing = value_to_features(req.existing)?;
    let incoming = value_to_features(req.incoming)?;
    let merged = merge(existing, incoming);
    to_js(&MergeResult {
        features: features_to_value(merged)?,
    })
}

// --- Phase-9 geometry + weather boundary shims (GEOX/GRID/METX) ------------
//
// Each shim marshals its inputs (rebuilding the sans-I/O TIN / geo polygons from
// coordinate data) and delegates the actual geometry / acoustic math to exactly
// one `envi_gis::` core function — no cut-profile, impedance, screening, grid, or
// A/B/C math lives here.

/// Extract the source→receiver DEM cut-profile (GEOX-01, delegates to
/// `envi_gis::profile::cut_profile`). The TIN is rebuilt from `tin_points`.
///
/// # Errors
/// A shape error, a TIN-build error, or any [`GisError`] from the extractor.
#[wasm_bindgen]
pub fn extract_cut_profile(req: JsValue) -> Result<JsValue, JsValue> {
    let req: CutProfileReq = from_js(req)?;
    let tin = build_gis_tin(&req.tin_points, &req.tin_breaklines)?;
    let points = cut_profile(&tin, req.s_xy, req.r_xy, req.step_m).map_err(gis_err)?;
    to_js(&CutProfileResult { points })
}

/// Segment the cut-profile into per-interval ground segments (GEOX-02, delegates
/// to `envi_gis::impedance::segment_ground`).
///
/// # Errors
/// A shape error, a non-single-char class, or any [`GisError`] from segmentation.
#[wasm_bindgen]
pub fn segment_cut_profile(req: JsValue) -> Result<JsValue, JsValue> {
    let req: SegmentGroundReq = from_js(req)?;
    let drawn: Vec<DrawnZone> = req
        .drawn_zones
        .iter()
        .map(drawn_zone)
        .collect::<Result<_, _>>()?;
    let imported: Vec<ImportedZone> = req.imported_zones.iter().map(imported_zone).collect();
    let default_class = class_char(&req.default_class)?;
    let seg = segment_ground(
        &req.points,
        &req.planar_xy,
        &drawn,
        &imported,
        default_class,
    )
    .map_err(gis_err)?;
    to_js(&segmentation_dto(&seg))
}

/// Inject screening edges into a base segmentation as `(x, z)` vertices (GEOX-03,
/// delegates to `envi_gis::screening::inject_screens`). The TIN is rebuilt from
/// `tin_points` so each screen top rides on terrain.
///
/// # Errors
/// A shape error, a TIN-build error, or any [`GisError`] from screening.
#[wasm_bindgen]
pub fn inject_screen_edges(req: JsValue) -> Result<JsValue, JsValue> {
    let req: InjectScreensReq = from_js(req)?;
    let base = segmentation_from_dto(&req.base);
    let screens: Vec<ScreenObject> = req.screens.iter().map(screen_object).collect();
    let tin = build_gis_tin(&req.tin_points, &req.tin_breaklines)?;
    let seg = inject_screens(&base, &screens, &tin).map_err(gis_err)?;
    to_js(&segmentation_dto(&seg))
}

/// Build the building-aware receiver grid (GRID-01, delegates to
/// `envi_gis::grid::receiver_grid`). The TIN is rebuilt from `tin_points` so each
/// receiver z is sampled from the DGM surface.
///
/// # Errors
/// A shape error, a TIN-build error, or any [`GisError`] from the grid.
#[wasm_bindgen]
pub fn build_receiver_grid(req: JsValue) -> Result<JsValue, JsValue> {
    let req: ReceiverGridReq = from_js(req)?;
    let calc_area = polygon_from_ring(&req.calc_area);
    let footprints: Vec<Polygon<f64>> = req
        .footprints
        .iter()
        .map(|r| polygon_from_ring(r))
        .collect();
    let tin = build_gis_tin(&req.tin_points, &req.tin_breaklines)?;
    let receivers = grid::receiver_grid(
        &calc_area,
        &footprints,
        req.spacing_m,
        &req.discrete_points,
        &tin,
    )
    .map_err(gis_err)?;
    to_js(&ReceiverGridResult { receivers })
}

/// Derive the per-azimuth sound-speed profiles from an Open-Meteo multi-level
/// profile (METX-01). Marshals the Open-Meteo JSON → levels → bearing-independent
/// components → one profile per path azimuth; ALL of the A/B/C math stays in
/// `envi_gis::weather` (the shim only projects each requested azimuth through the
/// core `sound_speed_profile_for_azimuth`).
///
/// # Errors
/// A shape error, or any [`GisError`] from the parse / fit.
#[wasm_bindgen]
pub fn derive_weather(req: JsValue) -> Result<JsValue, JsValue> {
    let req: WeatherDeriveReq = from_js(req)?;
    let levels = levels_from_openmeteo(req.openmeteo_json.as_bytes(), req.hour_index as usize)
        .map_err(gis_err)?;
    let comp = components_from_levels(&levels, req.phi_wind_deg, req.z0).map_err(gis_err)?;
    let profiles: Vec<SoundSpeedProfileDto> = req
        .path_azimuths_deg
        .iter()
        .map(|&az| {
            let p = sound_speed_profile_for_azimuth(&comp, az, req.phi_wind_deg);
            SoundSpeedProfileDto {
                a: p.a,
                b: p.b,
                c: p.c,
                s_a: p.s_a,
                s_b: p.s_b,
                z0: p.z0,
            }
        })
        .collect();
    to_js(&WeatherDeriveResult {
        components: WeatherComponentsDto {
            a_temp: comp.a_temp,
            a_wind: comp.a_wind,
            b: comp.b,
            c: comp.c,
            s_a: comp.s_a,
            s_b: comp.s_b,
            z0: comp.z0,
        },
        profiles,
    })
}

/// Derive the per-azimuth sound-speed profiles from FRIENDLY what-if met knobs
/// (METX-03/04, D-14). Marshals the friendly knobs → the SAME `envi_gis::weather`
/// per-azimuth A/B/C derivation `derive_weather` uses (the friendly path only
/// synthesises the level profile; no forked met math). The downwind-worst-case
/// envelope (D-15) is a per-bearing projection: each azimuth is treated as the
/// downwind bearing so `A = a_temp + a_wind` independently. A raw override bypasses
/// the derivation and emits `(A, B, C, z₀)` for every azimuth verbatim.
///
/// # Errors
/// A shape error, or any [`GisError`] from the range gate / fit (T-11-08-01).
#[wasm_bindgen]
pub fn derive_weather_friendly(req: JsValue) -> Result<JsValue, JsValue> {
    let req: FriendlyWeatherReq = from_js(req)?;

    // Advanced raw override (METX-03): the expert sets (A, B, C, z₀) directly, the
    // same profile for every azimuth — no derivation, no wind projection.
    if let Some(RawProfileDto { a, b, c, z0 }) = req.raw_override {
        for (v, what) in [(a, "a"), (b, "b"), (c, "c"), (z0, "z0")] {
            if !v.is_finite() {
                return Err(js_err(&format!("raw override {what} is not finite")));
            }
        }
        // Clamp through the engine's single source of truth (IN-02) so this raw
        // override never drifts from `envi_gis::weather`'s Z0_MIN_M floor.
        let z0 = z0.max(envi_engine::propagation::refraction::profile::Z0_MIN_M);
        let profile = SoundSpeedProfileDto {
            a,
            b,
            c,
            s_a: 0.0,
            s_b: 0.0,
            z0,
        };
        let profiles = vec![profile; req.path_azimuths_deg.len()];
        return to_js(&WeatherDeriveResult {
            components: WeatherComponentsDto {
                a_temp: a,
                a_wind: 0.0,
                b,
                c,
                s_a: 0.0,
                s_b: 0.0,
                z0,
            },
            profiles,
        });
    }

    let comp = components_from_friendly(
        req.temperature_c,
        req.temp_gradient_c_per_m,
        req.wind_speed_ms,
        req.wind_from_deg,
        req.z0,
    )
    .map_err(gis_err)?;
    // The reference downwind bearing φ_u the wind part is projected relative to.
    let phi_u = req.wind_from_deg + 180.0;
    let profiles: Vec<SoundSpeedProfileDto> = req
        .path_azimuths_deg
        .iter()
        .map(|&az| {
            // Downwind worst-case (D-15): treat EACH bearing as the downwind bearing
            // (φ_u = az) so the wind part is fully favourable per azimuth — the
            // standard Nord2000 worst-case envelope. Otherwise project the real wind.
            let phi = if req.downwind_worst_case { az } else { phi_u };
            let p = sound_speed_profile_for_azimuth(&comp, az, phi);
            SoundSpeedProfileDto {
                a: p.a,
                b: p.b,
                c: p.c,
                s_a: p.s_a,
                s_b: p.s_b,
                z0: p.z0,
            }
        })
        .collect();
    to_js(&WeatherDeriveResult {
        components: WeatherComponentsDto {
            a_temp: comp.a_temp,
            a_wind: comp.a_wind,
            b: comp.b,
            c: comp.c,
            s_a: comp.s_a,
            s_b: comp.s_b,
            z0: comp.z0,
        },
        profiles,
    })
}

/// The per-receiver signed dB(A) difference `A − B` between two scenarios' cached
/// readouts (METX-04 / D-16, D-01). Both inputs are WASM-produced weighted totals
/// (`ReceiverReadoutDto::total_dba`); this returns the elementwise difference so
/// the frontend performs ZERO acoustic arithmetic — TypeScript only marshals the
/// two number arrays in and renders the returned deltas. Kept a pure numeric op on
/// already-weighted totals (no tensor access), so it lives in this stable-toolchain
/// GIS boundary rather than forcing a nightly compute-wasm rebuild.
///
/// # Errors
/// The two arrays must be the same length (aligned receiver-for-receiver) and
/// finite — a mismatch/non-finite is a typed boundary error, never a silent NaN.
#[wasm_bindgen]
pub fn difference_dba(a_dba: &[f64], b_dba: &[f64]) -> Result<Vec<f64>, JsValue> {
    if a_dba.len() != b_dba.len() {
        return Err(js_err(&format!(
            "difference_dba length mismatch: {} vs {}",
            a_dba.len(),
            b_dba.len()
        )));
    }
    let mut out = Vec::with_capacity(a_dba.len());
    for (i, (&a, &b)) in a_dba.iter().zip(b_dba).enumerate() {
        if !a.is_finite() || !b.is_finite() {
            return Err(js_err(&format!(
                "difference_dba received a non-finite total at receiver {i}"
            )));
        }
        out.push(a - b);
    }
    Ok(out)
}

/// Derive the ERA5 wind×stability occurrence table + per-hour `1/L` (METX-02, D-05
/// — occurrence statistics only). Delegates to
/// `envi_gis::era5::occurrence_stats_with_inv_l`, which bins the classes and
/// returns each hour's inverse Obukhov length from a single pass (no double
/// `obukhov` recompute).
///
/// # Errors
/// A shape error, or any [`GisError`] from the derivation.
#[wasm_bindgen]
pub fn derive_era5(req: JsValue) -> Result<JsValue, JsValue> {
    let req: Era5DeriveReq = from_js(req)?;
    let hours: Vec<Era5Hour> = req.hours.iter().map(Era5Hour::from).collect();
    // Single pass: the occurrence binning already computes each hour's `1/L`, so
    // collect it here instead of a second `obukhov` sweep over the same hours (IN-03a).
    let (occ, inv_l) = occurrence_stats_with_inv_l(&hours).map_err(gis_err)?;
    let counts: Vec<Vec<u32>> = occ
        .counts
        .iter()
        .map(|row| {
            debug_assert_eq!(row.len(), N_STABILITY);
            row.to_vec()
        })
        .collect();
    to_js(&Era5DeriveResult {
        occurrence: ClassOccurrenceDto {
            counts,
            total: occ.total,
            reliable: occ.reliable,
        },
        inv_l,
    })
}

/// A core [`Era5Hour`] from its wire DTO (pure field marshalling; the field list
/// lives once, at this boundary).
impl From<&Era5HourDto> for Era5Hour {
    fn from(h: &Era5HourDto) -> Self {
        Era5Hour {
            iews: h.iews,
            inss: h.inss,
            ishf: h.ishf,
            t2m_k: h.t2m_k,
            d2m_k: h.d2m_k,
            sp_pa: h.sp_pa,
            sdfor_m: h.sdfor_m,
            u10_ms: h.u10_ms,
            v10_ms: h.v10_ms,
        }
    }
}
