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

use envi_gis::GisError;
use envi_gis::buildings::buildings_from_overpass;
use envi_gis::cog::{MAX_DECODED_PX, PixelWindow, Raster, decode_window_u8};
use envi_gis::landcover::{DEFAULT_MIN_AREA_PX, DEFAULT_SIMPLIFY_TOL_PX, vectorize_landcover};
use envi_gis::merge::merge;
use envi_gis::provenance::Provenance;
use envi_gis::registry::{self, Bbox, Cors, SourceDescriptor, SourceKind};
use envi_gis::terrain::{TerrainSourceCrs, base_elevation_on_raster};
use envi_gis::tiles::{self, TileRef};

use dto::{
    BaseElevationReq, BaseElevationResult, BboxDto, BuildingsResult, CorsDto, DecodeWindowReq,
    DecodeWindowResult, GeoTransformDto, ImportPlanReq, ImportPlanResult, LandcoverResult,
    MapLandcoverReq, MergeReq, MergeResult, ParseBuildingsReq, PixelWindowDto, PlanTilesReq,
    PlanTilesResult, ProvenanceReqDto, ReprojectRingReq, ReprojectRingResult, SkipReportDto,
    SourceDescriptorDto, SourceKindDto, TerrainFeaturesReq, TerrainFeaturesResult,
    TerrainSourceCrsDto, TileRefDto, VerticalDatumDto, WindowForBboxReq, WindowForBboxResult,
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
