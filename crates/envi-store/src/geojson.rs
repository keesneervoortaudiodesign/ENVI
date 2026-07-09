//! The scene GeoJSON boundary: the locked 9-kind `properties.kind` vocabulary,
//! unknown-kind preservation, and the one reprojection call site (GEOX-04).
//!
//! # Convention
//!
//! `scene.geojson` is persisted in **WGS84** (06-RESEARCH assumption A3): the
//! file stays valid RFC 7946 GeoJSON for external viewers, positions are
//! `[longitude, latitude]` (Pitfall 5). The pinned project CRS (project.json) is
//! the reprojection **target**; WGS84 -> SceneXY conversion happens on load into
//! engine space, through the single `envi_geo::ProjectCrs::to_utm` seam below
//! (GEOX-04: exactly one reprojection boundary). This function never mutates its
//! input — non-engine-mappable kinds stay untouched in the persisted collection.
//!
//! # The 9-kind vocabulary
//!
//! [`KINDS`] is the locked ARCHITECTURE.md object palette. Only `source`,
//! `receiver`, `wall`, and `building` map onto engine [`Scene`] types in Phase 6;
//! `forest`, `ground_zone`, `elevation_point`, `elevation_line`, and `calc_area`
//! are **persisted-but-not-yet-consumed** (Phase 7 draws them, Phases 8-9 consume
//! them) — they are validated and preserved, never dropped. Unknown ADDITIONAL
//! properties on a feature are allowed; unknown KIND strings are loudly rejected.

use geojson::FeatureCollection;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use envi_engine::scene::{
    BandSpectrum, Barrier, Building, CrsInfo, Receiver, Scene, Source, SubSource,
};
use envi_geo::{LonLat, ProjectCrs};

use crate::StoreError;
use crate::dto::BandSpectrumDto;

/// The locked scene object vocabulary (ARCHITECTURE.md, aligned with the
/// NoizCalc TI 386 palette). Engine-mappable in Phase 6: `source`, `receiver`,
/// `wall`, `building`. The rest are persisted-but-not-yet-engine-consumed.
pub const KINDS: [&str; 9] = [
    "source",
    "receiver",
    "wall",
    "building",
    "forest",
    "ground_zone",
    "elevation_point",
    "elevation_line",
    "calc_area",
];

/// Validate a scene FeatureCollection against the ENVI schema.
///
/// Every feature must have `properties.kind` in [`KINDS`] (else
/// [`StoreError::UnknownKind`]), a `properties.id` parsing as a [`Uuid`] (else
/// [`StoreError::MissingProperty`]), a present geometry, and every WGS84 position
/// finite with lon in `[-180, 180]`, lat in `[-90, 90]`. Unknown additional
/// properties are permitted; unknown kind strings are not.
///
/// # Errors
/// Returns the first schema violation found.
pub fn validate_feature_collection(fc: &FeatureCollection) -> Result<(), StoreError> {
    for feature in &fc.features {
        let kind = feature_kind(feature)?;
        // id must parse as a Uuid (the path-safe key; Pitfall 7 posture).
        let _id = feature_uuid(feature, kind)?;
        let geometry = feature
            .geometry
            .as_ref()
            .ok_or_else(|| StoreError::GeoJson {
                message: format!("feature of kind {kind:?} has no geometry"),
            })?;
        for pos in geometry_positions(&geometry.value)? {
            check_wgs84(pos)?;
        }
    }
    Ok(())
}

/// Convert a WGS84 scene FeatureCollection into an engine [`Scene`], reprojecting
/// every coordinate through `crs` (the single reprojection call site, GEOX-04).
///
/// Maps the four engine-mappable kinds — `source` (point -> one [`Source`] with a
/// single [`SubSource`]; spectrum from `properties.spectrum_band_db` [105] if
/// present, else uniform 0 dB), `receiver` (point -> [`Receiver`]), `wall`
/// (LineString + `height_m` [+ optional `thickness_m`] -> [`Barrier`]), and
/// `building` (Polygon exterior ring + `eaves_height_m` -> [`Building`]).
/// Non-mappable kinds are skipped here but remain untouched in the input.
/// `ground_zone` impedance class letters `A..=H` resolve via
/// `envi_engine::scene::impedance_class` when consumed in later phases — engine
/// `Scene.terrain` stays empty in Phase 6.
///
/// # Errors
/// [`StoreError`] on any schema violation or reprojection failure.
pub fn scene_to_engine(fc: &FeatureCollection, crs: &ProjectCrs) -> Result<Scene, StoreError> {
    // Validate up front so partial scenes never reach engine space.
    validate_feature_collection(fc)?;

    let mut sources = Vec::new();
    let mut receivers = Vec::new();
    let mut barriers = Vec::new();
    let mut buildings = Vec::new();

    for feature in &fc.features {
        let kind = feature_kind(feature)?;
        // Every geometry is guaranteed present by validation above.
        let value = &feature.geometry.as_ref().expect("validated present").value;
        match kind {
            "source" => sources.push(source_from_feature(feature, value, crs)?),
            "receiver" => receivers.push(receiver_from_feature(feature, value, crs)?),
            "wall" => barriers.push(barrier_from_feature(feature, value, crs)?),
            "building" => buildings.push(building_from_feature(feature, value, crs)?),
            // Persisted-but-not-engine-mapped in Phase 6: skip, never mutate.
            _ => {}
        }
    }

    Ok(Scene {
        crs: CrsInfo { label: crs.label() },
        sources,
        receivers,
        barriers,
        buildings,
        terrain: Vec::new(),
    })
}

/// Extract the validated `properties.kind` of a feature.
fn feature_kind(feature: &geojson::Feature) -> Result<&str, StoreError> {
    let props = feature
        .properties
        .as_ref()
        .ok_or_else(|| StoreError::GeoJson {
            message: "feature has no properties object".to_string(),
        })?;
    let kind = props
        .get("kind")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| StoreError::MissingProperty {
            kind: "<unknown>".to_string(),
            property: "kind".to_string(),
        })?;
    if !KINDS.contains(&kind) {
        return Err(StoreError::UnknownKind {
            kind: kind.to_string(),
        });
    }
    Ok(kind)
}

/// Parse the feature's `properties.id` as a [`Uuid`] (the path-traversal gate).
fn feature_uuid(feature: &geojson::Feature, kind: &str) -> Result<Uuid, StoreError> {
    let props = feature
        .properties
        .as_ref()
        .ok_or_else(|| StoreError::MissingProperty {
            kind: kind.to_string(),
            property: "id".to_string(),
        })?;
    let raw =
        props
            .get("id")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| StoreError::MissingProperty {
                kind: kind.to_string(),
                property: "id".to_string(),
            })?;
    Uuid::parse_str(raw).map_err(|_| StoreError::MissingProperty {
        kind: kind.to_string(),
        property: "id (valid uuid)".to_string(),
    })
}

/// Read an optional finite f64 property.
fn opt_f64(feature: &geojson::Feature, key: &str) -> Option<f64> {
    feature
        .properties
        .as_ref()
        .and_then(|p| p.get(key))
        .and_then(JsonValue::as_f64)
}

/// Read a required finite f64 property, else [`StoreError::MissingProperty`].
fn req_f64(feature: &geojson::Feature, kind: &str, key: &str) -> Result<f64, StoreError> {
    opt_f64(feature, key).ok_or_else(|| StoreError::MissingProperty {
        kind: kind.to_string(),
        property: key.to_string(),
    })
}

/// Build a [`Source`] from a point feature (one sub-source at the point).
fn source_from_feature(
    feature: &geojson::Feature,
    value: &geojson::GeometryValue,
    crs: &ProjectCrs,
) -> Result<Source, StoreError> {
    let pos = expect_point(value, "source")?;
    let xy = crs.to_utm(check_wgs84(pos)?)?;
    let z = opt_f64(feature, "height_m").unwrap_or(0.0);
    let spectrum = spectrum_of(feature)?;
    Ok(Source {
        sub_sources: vec![SubSource {
            position: [xy.x_m, xy.y_m, z],
            spectrum,
        }],
    })
}

/// Build a [`Receiver`] from a point feature.
fn receiver_from_feature(
    feature: &geojson::Feature,
    value: &geojson::GeometryValue,
    crs: &ProjectCrs,
) -> Result<Receiver, StoreError> {
    let pos = expect_point(value, "receiver")?;
    let xy = crs.to_utm(check_wgs84(pos)?)?;
    let z = opt_f64(feature, "height_m").unwrap_or(0.0);
    Ok(Receiver {
        position: [xy.x_m, xy.y_m, z],
    })
}

/// Build a [`Barrier`] from a LineString feature at `z = height_m`.
fn barrier_from_feature(
    feature: &geojson::Feature,
    value: &geojson::GeometryValue,
    crs: &ProjectCrs,
) -> Result<Barrier, StoreError> {
    let geojson::GeometryValue::LineString { coordinates: line } = value else {
        return Err(StoreError::GeoJson {
            message: "wall feature must be a LineString".to_string(),
        });
    };
    let height = req_f64(feature, "wall", "height_m")?;
    let thickness = opt_f64(feature, "thickness_m");
    let mut top_edge = Vec::with_capacity(line.len());
    for pos in line {
        let xy = crs.to_utm(check_wgs84(pos)?)?;
        top_edge.push([xy.x_m, xy.y_m, height]);
    }
    Ok(Barrier {
        top_edge,
        thickness_m: thickness,
    })
}

/// Build a [`Building`] from a Polygon exterior ring + `eaves_height_m`.
fn building_from_feature(
    feature: &geojson::Feature,
    value: &geojson::GeometryValue,
    crs: &ProjectCrs,
) -> Result<Building, StoreError> {
    let geojson::GeometryValue::Polygon { coordinates: rings } = value else {
        return Err(StoreError::GeoJson {
            message: "building feature must be a Polygon".to_string(),
        });
    };
    let exterior = rings.first().ok_or_else(|| StoreError::GeoJson {
        message: "building polygon has no exterior ring".to_string(),
    })?;
    let eaves = req_f64(feature, "building", "eaves_height_m")?;
    let mut footprint = Vec::with_capacity(exterior.len());
    for pos in exterior {
        let xy = crs.to_utm(check_wgs84(pos)?)?;
        footprint.push([xy.x_m, xy.y_m]);
    }
    // RFC 7946 rings repeat the first vertex as the last; drop the closing
    // duplicate so the engine footprint carries each vertex once.
    if footprint.len() > 1 && footprint.first() == footprint.last() {
        footprint.pop();
    }
    Ok(Building {
        footprint,
        eaves_height_m: eaves,
    })
}

/// Resolve a source spectrum from `properties.spectrum_band_db` [105], else
/// uniform 0 dB. Length/finiteness validated through [`BandSpectrumDto`].
fn spectrum_of(feature: &geojson::Feature) -> Result<BandSpectrum, StoreError> {
    let Some(raw) = feature
        .properties
        .as_ref()
        .and_then(|p| p.get("spectrum_band_db"))
    else {
        return Ok(BandSpectrum::uniform(0.0));
    };
    let band_db: Vec<f64> =
        serde_json::from_value(raw.clone()).map_err(|e| StoreError::GeoJson {
            message: format!("source spectrum_band_db is not an array of numbers: {e}"),
        })?;
    BandSpectrum::try_from(&BandSpectrumDto { band_db })
}

/// A point feature's single position, else a typed geometry error.
fn expect_point<'a>(
    value: &'a geojson::GeometryValue,
    kind: &str,
) -> Result<&'a geojson::Position, StoreError> {
    match value {
        geojson::GeometryValue::Point { coordinates } => Ok(coordinates),
        _ => Err(StoreError::GeoJson {
            message: format!("{kind} feature must be a Point"),
        }),
    }
}

/// All positions in a geometry value (Point/LineString/Polygon and their multi
/// variants). Used by validation and by the tensor hash; unsupported geometry
/// types are rejected.
pub(crate) fn geometry_positions(
    value: &geojson::GeometryValue,
) -> Result<Vec<&geojson::Position>, StoreError> {
    use geojson::GeometryValue::{
        LineString, MultiLineString, MultiPoint, MultiPolygon, Point, Polygon,
    };
    let out = match value {
        Point { coordinates } => vec![coordinates],
        MultiPoint { coordinates } | LineString { coordinates } => coordinates.iter().collect(),
        MultiLineString { coordinates } | Polygon { coordinates } => {
            coordinates.iter().flatten().collect()
        }
        MultiPolygon { coordinates } => coordinates.iter().flatten().flatten().collect(),
        other => {
            return Err(StoreError::GeoJson {
                message: format!("unsupported geometry type: {other:?}"),
            });
        }
    };
    Ok(out)
}

/// Validate a WGS84 position and return it as a [`LonLat`].
fn check_wgs84(pos: &geojson::Position) -> Result<LonLat, StoreError> {
    if pos.len() < 2 {
        return Err(StoreError::GeoJson {
            message: format!("position needs >= 2 components, got {}", pos.len()),
        });
    }
    let (lon, lat) = (pos[0], pos[1]);
    if !lon.is_finite() || !lat.is_finite() {
        return Err(StoreError::NonFinite {
            what: format!("WGS84 position ({lon}, {lat})"),
        });
    }
    if !(-180.0..=180.0).contains(&lon) || !(-90.0..=90.0).contains(&lat) {
        return Err(StoreError::GeoJson {
            message: format!(
                "WGS84 position out of range: ({lon}, {lat}) — lon in [-180, 180], lat in [-90, 90]"
            ),
        });
    }
    Ok(LonLat {
        lon_deg: lon,
        lat_deg: lat,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A FeatureCollection with one feature of every one of the 9 kinds, all
    /// with plausible WGS84 coordinates near Amsterdam and uuid ids.
    fn nine_kind_fc() -> FeatureCollection {
        let json = r#"{
          "type": "FeatureCollection",
          "features": [
            {"type":"Feature","geometry":{"type":"Point","coordinates":[4.8936,52.3731]},
             "properties":{"kind":"source","id":"00000000-0000-0000-0000-000000000001","height_m":0.5}},
            {"type":"Feature","geometry":{"type":"Point","coordinates":[4.8950,52.3740]},
             "properties":{"kind":"receiver","id":"00000000-0000-0000-0000-000000000002","height_m":1.5}},
            {"type":"Feature","geometry":{"type":"LineString","coordinates":[[4.8940,52.3733],[4.8945,52.3736]]},
             "properties":{"kind":"wall","id":"00000000-0000-0000-0000-000000000003","height_m":3.0}},
            {"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[4.8930,52.3730],[4.8933,52.3730],[4.8933,52.3733],[4.8930,52.3730]]]},
             "properties":{"kind":"building","id":"00000000-0000-0000-0000-000000000004","eaves_height_m":6.0}},
            {"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[4.8960,52.3750],[4.8963,52.3750],[4.8963,52.3753],[4.8960,52.3750]]]},
             "properties":{"kind":"forest","id":"00000000-0000-0000-0000-000000000005"}},
            {"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[4.8920,52.3720],[4.8923,52.3720],[4.8923,52.3723],[4.8920,52.3720]]]},
             "properties":{"kind":"ground_zone","id":"00000000-0000-0000-0000-000000000006","impedance_class":"D"}},
            {"type":"Feature","geometry":{"type":"Point","coordinates":[4.8910,52.3710]},
             "properties":{"kind":"elevation_point","id":"00000000-0000-0000-0000-000000000007","z_m":2.0}},
            {"type":"Feature","geometry":{"type":"LineString","coordinates":[[4.8900,52.3700],[4.8905,52.3702]]},
             "properties":{"kind":"elevation_line","id":"00000000-0000-0000-0000-000000000008"}},
            {"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[4.8970,52.3760],[4.8973,52.3760],[4.8973,52.3763],[4.8970,52.3760]]]},
             "properties":{"kind":"calc_area","id":"00000000-0000-0000-0000-000000000009"}}
          ]
        }"#;
        serde_json::from_str(json).expect("valid FeatureCollection")
    }

    #[test]
    fn all_nine_kinds_validate_and_round_trip() {
        let fc = nine_kind_fc();
        validate_feature_collection(&fc).expect("all 9 kinds validate");

        // Serialize -> reparse: feature count and kind set identical.
        let text = serde_json::to_string(&fc).expect("serialize");
        let reparsed: FeatureCollection = serde_json::from_str(&text).expect("reparse");
        assert_eq!(reparsed.features.len(), 9);
        let kinds: std::collections::BTreeSet<String> = reparsed
            .features
            .iter()
            .map(|f| feature_kind(f).unwrap().to_string())
            .collect();
        assert_eq!(kinds.len(), 9, "all 9 kinds preserved");
        for k in KINDS {
            assert!(kinds.contains(k), "kind {k} preserved");
        }
    }

    #[test]
    fn unknown_kind_is_rejected_loudly() {
        let json = r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","geometry":{"type":"Point","coordinates":[4.89,52.37]},
           "properties":{"kind":"sourc","id":"00000000-0000-0000-0000-000000000001"}}]}"#;
        let fc: FeatureCollection = serde_json::from_str(json).unwrap();
        let err = validate_feature_collection(&fc).unwrap_err();
        assert!(
            matches!(err, StoreError::UnknownKind { ref kind } if kind == "sourc"),
            "got {err:?}"
        );
    }

    #[test]
    fn dto_engine_round_trip_and_unknown_kind_preservation() {
        let fc = nine_kind_fc();
        let crs = ProjectCrs::for_location(LonLat {
            lon_deg: 4.8936,
            lat_deg: 52.3731,
        })
        .expect("Amsterdam CRS");

        let scene = scene_to_engine(&fc, &crs).expect("scene converts");
        // Exactly the four engine-mappable kinds are mapped.
        assert_eq!(scene.sources.len(), 1, "one source");
        assert_eq!(scene.receivers.len(), 1, "one receiver");
        assert_eq!(scene.barriers.len(), 1, "one wall -> barrier");
        assert_eq!(scene.buildings.len(), 1, "one building");
        assert!(scene.terrain.is_empty(), "no terrain in Phase 6");

        // One reprojected coordinate checked against an independent envi-geo call.
        let expected = crs
            .to_utm(LonLat {
                lon_deg: 4.8936,
                lat_deg: 52.3731,
            })
            .expect("reproject");
        let got = scene.sources[0].sub_sources[0].position;
        approx::assert_relative_eq!(got[0], expected.x_m, epsilon = 1e-6);
        approx::assert_relative_eq!(got[1], expected.y_m, epsilon = 1e-6);
        approx::assert_relative_eq!(got[2], 0.5, epsilon = 1e-12);

        // The input FeatureCollection still contains all 9 kinds (never mutated).
        assert_eq!(fc.features.len(), 9);
    }

    #[test]
    fn missing_id_or_bad_coords_rejected() {
        // Missing id -> MissingProperty.
        let no_id = r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","geometry":{"type":"Point","coordinates":[4.89,52.37]},
           "properties":{"kind":"source"}}]}"#;
        let fc: FeatureCollection = serde_json::from_str(no_id).unwrap();
        assert!(
            matches!(
                validate_feature_collection(&fc).unwrap_err(),
                StoreError::MissingProperty { .. }
            ),
            "missing id must be rejected"
        );

        // lat 95 -> out-of-range error.
        let bad_lat = r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","geometry":{"type":"Point","coordinates":[4.89,95.0]},
           "properties":{"kind":"source","id":"00000000-0000-0000-0000-000000000001"}}]}"#;
        let fc: FeatureCollection = serde_json::from_str(bad_lat).unwrap();
        assert!(
            validate_feature_collection(&fc).is_err(),
            "lat 95 must be rejected"
        );
    }
}
