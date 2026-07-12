//! Isophone fill-polygon GeoJSON encoder (D-21/D-22) — reuses the in-tree
//! `geojson` crate (RFC-7946), never hand-writes JSON.
//!
//! # What it encodes
//! One RFC-7946 `Feature` per iso-band (a `MultiPolygon` of its fill regions,
//! exterior-then-holes per component), carrying the band range `[lower, upper)`,
//! an optional fill colour, and the [`ExportMeta`] weighting label in its
//! properties. The [`ExportMeta`] footer (CRS + engine/scene identity +
//! attribution, D-22) rides as `FeatureCollection` foreign members so the whole
//! export is self-identifying.
//!
//! # Coordinates are ALREADY WGS84 (GEOX-04)
//! GeoJSON is WGS84 lon/lat by definition (RFC-7946 §4). The iso-band tracer works
//! in SceneXY meters; the SceneXY→LonLat reprojection is done ONCE in the WASM
//! boundary (`envi-compute-wasm::export`) through `envi-geo` — this encoder only
//! ever receives already-reprojected [`IsoBandLonLat`] polygons, so it adds no
//! second reprojection seam.

use geojson::{
    Feature, FeatureCollection, Geometry, GeometryValue, JsonObject, JsonValue, Position,
};

use crate::export::ExportMeta;

/// A WGS84 lon/lat fill polygon: one exterior ring plus any hole rings. Each
/// vertex is `[lon_deg, lat_deg]` (RFC-7946 axis order).
#[derive(Debug, Clone, PartialEq)]
pub struct PolygonLonLat {
    /// The exterior ring (closed; `[lon, lat]` vertices).
    pub exterior: Vec<[f64; 2]>,
    /// The hole rings (closed; `[lon, lat]` vertices).
    pub holes: Vec<Vec<[f64; 2]>>,
}

/// One iso-band's fill regions in WGS84 lon/lat, ready to serialize as a GeoJSON
/// `MultiPolygon` feature. The band covers the value interval `[lower, upper)`.
#[derive(Debug, Clone, PartialEq)]
pub struct IsoBandLonLat {
    /// Inclusive lower break of the band (dB).
    pub lower: f64,
    /// Exclusive upper break of the band (dB).
    pub upper: f64,
    /// Optional fill colour (e.g. a `#rrggbb` string) assigned by the colour scale.
    pub fill: Option<String>,
    /// The band's disjoint fill polygons.
    pub polygons: Vec<PolygonLonLat>,
}

/// Encoding an isophone `FeatureCollection` failed (IN-04) — surfaced instead of
/// silently substituting an empty document (which would drop every band with no
/// signal to the caller).
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum GeoJsonEncodeError {
    /// A ring carried a non-finite `[lon, lat]` vertex. `serde_json` would encode a
    /// `NaN`/`inf` as JSON `null`, producing a structurally-invalid position rather
    /// than erroring — so reject it up front.
    #[error("non-finite coordinate in an isophone ring: [{lon}, {lat}]")]
    NonFiniteCoord {
        /// The offending longitude.
        lon: f64,
        /// The offending latitude.
        lat: f64,
    },
    /// The `geojson`/`serde_json` serialization itself failed.
    #[error("GeoJSON serialization failed: {0}")]
    Serialize(String),
}

/// Encode iso-band fill polygons as an RFC-7946 GeoJSON `FeatureCollection` string
/// (D-21), each feature carrying its band range + fill colour + weighting label,
/// with the [`ExportMeta`] footer as collection foreign members (D-22).
///
/// # Errors
/// [`GeoJsonEncodeError::NonFiniteCoord`] if any ring vertex is non-finite (which
/// `serde_json` would otherwise emit as an invalid `null` position), or
/// [`GeoJsonEncodeError::Serialize`] if serialization fails — never a silently-empty
/// `FeatureCollection` that drops every band (IN-04).
pub fn encode_isophone_geojson(
    bands: &[IsoBandLonLat],
    meta: &ExportMeta,
) -> Result<String, GeoJsonEncodeError> {
    // Guard non-finite coordinates BEFORE serialization (IN-04).
    for b in bands {
        for p in &b.polygons {
            check_ring_finite(&p.exterior)?;
            for h in &p.holes {
                check_ring_finite(h)?;
            }
        }
    }

    let features = bands.iter().map(|b| band_feature(b, meta)).collect();
    let fc = FeatureCollection {
        bbox: None,
        features,
        foreign_members: Some(meta_members(meta)),
    };
    // Serialize through the geojson crate's serde impl — never a hand-written JSON
    // string (the RFC-7946 winding/structure is the crate's responsibility). A
    // serialization failure is PROPAGATED, not swallowed into an empty document.
    serde_json::to_string(&fc).map_err(|e| GeoJsonEncodeError::Serialize(e.to_string()))
}

/// Reject any non-finite `[lon, lat]` vertex in a ring (IN-04).
fn check_ring_finite(ring: &[[f64; 2]]) -> Result<(), GeoJsonEncodeError> {
    for &[lon, lat] in ring {
        if !lon.is_finite() || !lat.is_finite() {
            return Err(GeoJsonEncodeError::NonFiniteCoord { lon, lat });
        }
    }
    Ok(())
}

/// Build one `MultiPolygon` feature for an iso-band.
fn band_feature(band: &IsoBandLonLat, meta: &ExportMeta) -> Feature {
    let coordinates: Vec<Vec<Vec<Position>>> = band
        .polygons
        .iter()
        .map(|p| {
            let mut rings = Vec::with_capacity(1 + p.holes.len());
            rings.push(ring(&p.exterior));
            for h in &p.holes {
                rings.push(ring(h));
            }
            rings
        })
        .collect();

    let mut props = JsonObject::new();
    props.insert("kind".to_string(), JsonValue::from("isophone"));
    props.insert("lower_db".to_string(), JsonValue::from(band.lower));
    props.insert("upper_db".to_string(), JsonValue::from(band.upper));
    props.insert(
        "weighting".to_string(),
        JsonValue::from(meta.weighting_label.clone()),
    );
    if let Some(fill) = &band.fill {
        props.insert("fill".to_string(), JsonValue::from(fill.clone()));
    }

    Feature {
        bbox: None,
        geometry: Some(Geometry::new(GeometryValue::MultiPolygon { coordinates })),
        id: None,
        properties: Some(props),
        foreign_members: None,
    }
}

/// The [`ExportMeta`] footer as `FeatureCollection` foreign members (D-22).
fn meta_members(meta: &ExportMeta) -> JsonObject {
    let mut m = JsonObject::new();
    m.insert(
        "crs".to_string(),
        JsonValue::from(format!("EPSG:{}", meta.epsg)),
    );
    m.insert(
        "weighting".to_string(),
        JsonValue::from(meta.weighting_label.clone()),
    );
    m.insert(
        "engine".to_string(),
        JsonValue::from(meta.engine_version.clone()),
    );
    m.insert(
        "tensor_hash".to_string(),
        JsonValue::from(meta.tensor_hash.clone()),
    );
    m.insert(
        "attribution".to_string(),
        JsonValue::from(meta.attribution.clone()),
    );
    m
}

/// A `[lon, lat]` ring to GeoJSON `Position`s.
fn ring(r: &[[f64; 2]]) -> Vec<Position> {
    r.iter()
        .map(|&[lon, lat]| Position::from([lon, lat]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use geojson::GeoJson;

    fn meta() -> ExportMeta {
        ExportMeta {
            epsg: 32631,
            weighting_label: "dB(A)".to_string(),
            engine_version: "envi-test".to_string(),
            tensor_hash: "abc123".to_string(),
            attribution: "© OpenStreetMap contributors; ESA WorldCover".to_string(),
        }
    }

    /// A square exterior ring with a square hole, in lon/lat near Amsterdam.
    fn band(lower: f64, upper: f64) -> IsoBandLonLat {
        IsoBandLonLat {
            lower,
            upper,
            fill: Some("#ff0000".to_string()),
            polygons: vec![PolygonLonLat {
                exterior: vec![
                    [4.90, 52.40],
                    [4.92, 52.40],
                    [4.92, 52.42],
                    [4.90, 52.42],
                    [4.90, 52.40],
                ],
                holes: vec![vec![
                    [4.905, 52.405],
                    [4.915, 52.405],
                    [4.915, 52.415],
                    [4.905, 52.415],
                    [4.905, 52.405],
                ]],
            }],
        }
    }

    #[test]
    fn output_is_valid_rfc7946_with_band_props_and_attribution_member() {
        let bands = [band(50.0, 55.0), band(55.0, 60.0)];
        let s = encode_isophone_geojson(&bands, &meta()).unwrap();

        // Parses as valid GeoJSON via the crate (RFC-7946 structure).
        let gj: GeoJson = s.parse().expect("valid GeoJSON");
        let GeoJson::FeatureCollection(fc) = gj else {
            panic!("expected a FeatureCollection");
        };
        assert_eq!(fc.features.len(), 2, "one feature per band");

        // Each feature is a MultiPolygon carrying its band range.
        for (i, f) in fc.features.iter().enumerate() {
            let GeometryValue::MultiPolygon { coordinates } = &f.geometry.as_ref().unwrap().value
            else {
                panic!("isophone must be a MultiPolygon");
            };
            // One polygon, exterior + one hole ring.
            assert_eq!(coordinates.len(), 1);
            assert_eq!(coordinates[0].len(), 2, "exterior + one hole");
            assert_eq!(
                f.property("kind").and_then(|v| v.as_str()),
                Some("isophone")
            );
            let lower = f.property("lower_db").and_then(JsonValue::as_f64).unwrap();
            assert_eq!(lower, 50.0 + i as f64 * 5.0);
            assert_eq!(f.property("fill").and_then(|v| v.as_str()), Some("#ff0000"));
        }

        // The attribution/metadata footer rides as collection foreign members (D-22).
        let fm = fc.foreign_members.expect("foreign members present");
        assert_eq!(fm.get("crs").and_then(|v| v.as_str()), Some("EPSG:32631"));
        assert!(
            fm.get("attribution")
                .and_then(|v| v.as_str())
                .unwrap()
                .contains("OpenStreetMap")
        );
        assert_eq!(
            fm.get("tensor_hash").and_then(|v| v.as_str()),
            Some("abc123")
        );
    }

    #[test]
    fn empty_bands_encode_to_an_empty_feature_collection() {
        let s = encode_isophone_geojson(&[], &meta()).unwrap();
        let gj: GeoJson = s.parse().expect("valid GeoJSON");
        let GeoJson::FeatureCollection(fc) = gj else {
            panic!("expected a FeatureCollection");
        };
        assert!(fc.features.is_empty());
        // The attribution footer is still present on an empty export.
        assert!(fc.foreign_members.is_some());
    }

    #[test]
    fn non_finite_coordinate_is_a_typed_error_not_a_null_position() {
        // IN-04: a NaN/inf vertex must error, never ride out as an invalid JSON
        // `null` position that a GeoJSON consumer would choke on.
        let mut b = band(50.0, 55.0);
        b.polygons[0].exterior[1] = [f64::NAN, 52.40];
        let err = encode_isophone_geojson(&[b], &meta()).unwrap_err();
        assert!(matches!(err, GeoJsonEncodeError::NonFiniteCoord { .. }));
    }
}
