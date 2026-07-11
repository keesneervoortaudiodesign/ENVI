//! Overpass JSON → building footprints with a locked height chain + provenance.
//!
//! # Module I/O
//! - **Inputs:** an Overpass `out geom` JSON string (untrusted third-party
//!   geometry/tags), a user-default height, and an ISO-8601 retrieval timestamp.
//! - **Output:** a `(Vec<Feature>, Vec<SkipReport>)` — `building`-kind GeoJSON
//!   features carrying `eaves_height_m` (the exact key the Phase-6
//!   `building_from_feature` reads, `envi-store/src/geojson.rs:316`),
//!   `height_provenance`, and the D-11 provenance stamp; plus a per-feature skip
//!   report for invalid geometry (never fails the whole layer — D-07).
//! - **Invariants (load-bearing):**
//!   1. **No panic on data** (threat T-08-04-02): malformed JSON is a typed
//!      [`GisError::Json`]; poisoned geometry (open/short/non-finite rings) is
//!      skipped-and-reported, never a panic.
//!   2. **Locked height fallback chain** (D-10): measured → `height` tag →
//!      `building:levels`×3+1.5 → user default; non-finite/negative heights are
//!      rejected and fall through.
//!   3. **`eaves_height_m`, not a parallel key**: imported buildings emit the
//!      exact property the existing 9-kind schema consumes.
//!   4. **No Rust UUIDs**: features carry no `id`; TS assigns it.

use std::collections::HashMap;

use geojson::{Feature, JsonValue, Position};
use serde::Deserialize;

use crate::GisError;
use crate::geojson_util::polygon_feature;
use crate::provenance::Provenance;

/// Meters per building level (OSM convention ≈ 3 m/storey; D-10 locked formula).
pub const LEVEL_HEIGHT_M: f64 = 3.0;
/// Roof allowance added to the levels estimate (D-10 locked `×3 + 1.5`).
pub const ROOF_ALLOWANCE_M: f64 = 1.5;

/// A resolved building height plus how it was derived (the `height_provenance`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeightResult {
    /// Eaves height, meters.
    pub height_m: f64,
    /// Provenance tag: `"height_tag"`, `"levels"`, or `"default"` (the reserved
    /// `"measured"` branch has no source this phase).
    pub provenance: &'static str,
}

/// A skipped feature and why (D-07 skip-and-report; never fails the layer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkipReport {
    /// The `(type/id)` reference of the skipped element (`"way/123"`).
    pub source_ref: String,
    /// Why it was skipped.
    pub reason: String,
}

/// Resolve a building's eaves height via the LOCKED fallback chain (D-10):
/// 1. **measured** — reserved, no source this phase (empty branch);
/// 2. **`height` / `building:height` tag** — tolerant meter parse
///    (`"12"`, `"12 m"`, `"12.5m"`); non-finite/negative rejected;
/// 3. **`building:levels` × [`LEVEL_HEIGHT_M`] + [`ROOF_ALLOWANCE_M`]**;
/// 4. **`user_default_m`**.
#[must_use]
pub fn height_from_tags(tags: &HashMap<String, String>, user_default_m: f64) -> HeightResult {
    // (1) measured — reserved; no measured-height source this phase.

    // (2) height / building:height tag.
    if let Some(h) = tags
        .get("height")
        .or_else(|| tags.get("building:height"))
        .and_then(|s| parse_leading_positive(s))
    {
        return HeightResult {
            height_m: h,
            provenance: "height_tag",
        };
    }

    // (3) building:levels × 3 + 1.5.
    if let Some(levels) = tags
        .get("building:levels")
        .and_then(|s| parse_leading_positive(s))
    {
        return HeightResult {
            height_m: levels * LEVEL_HEIGHT_M + ROOF_ALLOWANCE_M,
            provenance: "levels",
        };
    }

    // (4) user default.
    HeightResult {
        height_m: user_default_m,
        provenance: "default",
    }
}

/// Parse the leading positive-number prefix of a tag value (meters or levels),
/// tolerating a trailing unit/space (`"12 m"`, `"12.5m"`). Rejects empty,
/// non-finite, zero, and negative values.
fn parse_leading_positive(s: &str) -> Option<f64> {
    let s = s.trim();
    let end = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
        .unwrap_or(s.len());
    let v: f64 = s[..end].parse().ok()?;
    (v.is_finite() && v > 0.0).then_some(v)
}

/// Parse an Overpass `out geom` response into `building` features + skip reports.
///
/// Ways and multipolygon relations tagged `building` become Polygon features;
/// every other element is ignored. Invalid geometry is skipped-and-reported.
///
/// # Errors
/// [`GisError::Json`] if the response is not valid Overpass JSON.
pub fn buildings_from_overpass(
    json: &str,
    user_default_height_m: f64,
    retrieved_at: &str,
) -> Result<(Vec<Feature>, Vec<SkipReport>), GisError> {
    let resp: OverpassResponse = serde_json::from_str(json).map_err(|e| GisError::Json {
        message: e.to_string(),
    })?;

    let mut features = Vec::new();
    let mut skips = Vec::new();

    for el in &resp.elements {
        // Only building-tagged ways/relations are footprints.
        if el.tags.get("building").is_none_or(|v| v == "no") {
            continue;
        }
        let source_ref = format!("{}/{}", el.element_type, el.id);

        let rings = match el.element_type.as_str() {
            "way" => match &el.geometry {
                Some(coords) => match validate_ring(coords) {
                    Ok(ring) => vec![ring],
                    Err(reason) => {
                        skips.push(SkipReport { source_ref, reason });
                        continue;
                    }
                },
                None => {
                    skips.push(SkipReport {
                        source_ref,
                        reason: "building way has no geometry".to_string(),
                    });
                    continue;
                }
            },
            "relation" => match relation_rings(el) {
                Ok(rings) => rings,
                Err(reason) => {
                    skips.push(SkipReport { source_ref, reason });
                    continue;
                }
            },
            _ => continue,
        };

        let height = height_from_tags(&el.tags, user_default_height_m);
        let prov = Provenance {
            source: "osm-overpass",
            source_ref,
            license: "ODbL-1.0",
            retrieved_at: retrieved_at.to_string(),
            height_provenance: Some(height.provenance),
            vertical_datum: None,
        };
        let mut props = prov.into_properties("building");
        // The exact key building_from_feature reads (never a parallel height_m).
        props.insert(
            "eaves_height_m".to_string(),
            JsonValue::from(height.height_m),
        );

        features.push(polygon_feature(rings, props));
    }

    Ok((features, skips))
}

/// Build the polygon rings (outer + inner holes) for a multipolygon relation.
/// Uses the first valid outer ring as the exterior (extra outers are dropped this
/// phase) and every valid inner ring as a hole.
fn relation_rings(el: &OverpassElement) -> Result<Vec<Vec<Position>>, String> {
    let Some(members) = &el.members else {
        return Err("building relation has no members".to_string());
    };
    let mut outer = None;
    let mut inners = Vec::new();
    for m in members {
        if m.element_type != "way" {
            continue;
        }
        let Some(coords) = &m.geometry else {
            continue;
        };
        let Ok(ring) = validate_ring(coords) else {
            continue; // skip an individual bad member ring
        };
        if m.role == "inner" {
            inners.push(ring);
        } else if outer.is_none() {
            outer = Some(ring);
        }
    }
    let Some(outer) = outer else {
        return Err("building relation has no valid outer ring".to_string());
    };
    let mut rings = vec![outer];
    rings.extend(inners);
    Ok(rings)
}

/// Validate an Overpass ring (Pitfall 12): finite coordinates, ≥ 3 vertices, and
/// closed (RFC 7946) with ≥ 4 positions. Returns the closed `[lon, lat]` ring or
/// a skip reason.
fn validate_ring(coords: &[LatLon]) -> Result<Vec<Position>, String> {
    if coords.len() < 3 {
        return Err(format!("ring has {} vertices, need ≥ 3", coords.len()));
    }
    for p in coords {
        if !p.lat.is_finite() || !p.lon.is_finite() {
            return Err("ring has a non-finite coordinate".to_string());
        }
    }
    let mut ring: Vec<Position> = coords
        .iter()
        .map(|p| Position::from([p.lon, p.lat]))
        .collect();
    let (first, last) = (&coords[0], &coords[coords.len() - 1]);
    if first.lon != last.lon || first.lat != last.lat {
        ring.push(Position::from([first.lon, first.lat]));
    }
    if ring.len() < 4 {
        return Err(format!(
            "ring has {} positions after closing, need ≥ 4",
            ring.len()
        ));
    }
    Ok(ring)
}

// --- Overpass JSON shapes (untrusted; only the fields we need). ---

#[derive(Debug, Deserialize)]
struct OverpassResponse {
    #[serde(default)]
    elements: Vec<OverpassElement>,
}

#[derive(Debug, Deserialize)]
struct OverpassElement {
    #[serde(rename = "type")]
    element_type: String,
    id: i64,
    #[serde(default)]
    tags: HashMap<String, String>,
    #[serde(default)]
    geometry: Option<Vec<LatLon>>,
    #[serde(default)]
    members: Option<Vec<OverpassMember>>,
}

#[derive(Debug, Deserialize)]
struct OverpassMember {
    #[serde(rename = "type")]
    element_type: String,
    #[serde(default)]
    role: String,
    #[serde(default)]
    geometry: Option<Vec<LatLon>>,
}

#[derive(Debug, Deserialize)]
struct LatLon {
    lat: f64,
    lon: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use geojson::GeometryValue;

    fn tags(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn height_chain_covers_each_branch_with_tolerant_parsing() {
        // (2) height tag — plain, trailing unit with/without space.
        assert_eq!(
            height_from_tags(&tags(&[("height", "12")]), 6.0),
            HeightResult {
                height_m: 12.0,
                provenance: "height_tag"
            }
        );
        assert_eq!(
            height_from_tags(&tags(&[("height", "12 m")]), 6.0).height_m,
            12.0
        );
        assert_eq!(
            height_from_tags(&tags(&[("height", "12.5m")]), 6.0).height_m,
            12.5
        );
        // building:height synonym.
        assert_eq!(
            height_from_tags(&tags(&[("building:height", "9")]), 6.0).height_m,
            9.0
        );

        // (3) levels × 3 + 1.5, decimal levels.
        let r = height_from_tags(&tags(&[("building:levels", "2.5")]), 6.0);
        assert_eq!(r.height_m, 2.5 * LEVEL_HEIGHT_M + ROOF_ALLOWANCE_M);
        assert_eq!(r.provenance, "levels");

        // (4) user default when nothing usable.
        assert_eq!(
            height_from_tags(&tags(&[]), 6.0),
            HeightResult {
                height_m: 6.0,
                provenance: "default"
            }
        );
    }

    #[test]
    fn non_finite_and_negative_heights_are_rejected_and_fall_through() {
        // Negative height → rejected → falls to levels.
        let r = height_from_tags(&tags(&[("height", "-3"), ("building:levels", "3")]), 6.0);
        assert_eq!(r.provenance, "levels");
        assert_eq!(r.height_m, 3.0 * LEVEL_HEIGHT_M + ROOF_ALLOWANCE_M);

        // Garbage height + no levels → user default.
        let r = height_from_tags(&tags(&[("height", "tall")]), 6.0);
        assert_eq!(r.provenance, "default");
        assert_eq!(r.height_m, 6.0);

        // Zero height rejected too.
        let r = height_from_tags(&tags(&[("height", "0")]), 7.0);
        assert_eq!(r.provenance, "default");
        assert_eq!(r.height_m, 7.0);
    }

    #[test]
    fn a_valid_building_way_emits_eaves_height_m_not_height_m() {
        let json = r#"{"elements":[
          {"type":"way","id":123,"tags":{"building":"yes","height":"10"},
           "geometry":[{"lat":52.37,"lon":4.89},{"lat":52.37,"lon":4.8905},
                       {"lat":52.3705,"lon":4.8905},{"lat":52.37,"lon":4.89}]}
        ]}"#;
        let (feats, skips) =
            buildings_from_overpass(json, 6.0, "2026-07-10T00:00:00Z").expect("parses");
        assert_eq!(feats.len(), 1);
        assert!(skips.is_empty());
        let f = &feats[0];
        // Emits the exact key building_from_feature reads.
        assert_eq!(
            f.property("eaves_height_m").and_then(|v| v.as_f64()),
            Some(10.0)
        );
        // Never a parallel height_m for buildings.
        assert!(f.property("height_m").is_none());
        assert_eq!(
            f.property("kind").and_then(|v| v.as_str()),
            Some("building")
        );
        assert_eq!(
            f.property("source").and_then(|v| v.as_str()),
            Some("osm-overpass")
        );
        assert_eq!(
            f.property("height_provenance").and_then(|v| v.as_str()),
            Some("height_tag")
        );
        assert!(f.id.is_none(), "no Rust-assigned UUID");
        // Geometry is a closed Polygon exterior ring.
        let GeometryValue::Polygon { coordinates } = &f.geometry.as_ref().unwrap().value else {
            panic!("building must be a Polygon");
        };
        assert_eq!(coordinates.len(), 1, "one exterior ring");
        assert!(coordinates[0].len() >= 4, "ring closed with ≥ 4 positions");
    }

    #[test]
    fn invalid_geometry_is_skipped_and_reported_never_panics() {
        let json = r#"{"elements":[
          {"type":"way","id":1,"tags":{"building":"yes"},
           "geometry":[{"lat":52.37,"lon":4.89},{"lat":52.37,"lon":4.8905}]},
          {"type":"way","id":2,"tags":{"building":"yes"}},
          {"type":"relation","id":3,"tags":{"building":"yes","type":"multipolygon"},"members":[]}
        ]}"#;
        let (feats, skips) =
            buildings_from_overpass(json, 6.0, "2026-07-10T00:00:00Z").expect("parses");
        assert!(feats.is_empty(), "no valid footprints");
        assert_eq!(skips.len(), 3, "each invalid element reported");
        let refs: Vec<_> = skips.iter().map(|s| s.source_ref.as_str()).collect();
        assert!(refs.contains(&"way/1"));
        assert!(refs.contains(&"way/2"));
        assert!(refs.contains(&"relation/3"));
    }

    #[test]
    fn multipolygon_relation_builds_outer_ring_with_inner_hole() {
        let json = r#"{"elements":[
          {"type":"relation","id":42,"tags":{"building":"yes","type":"multipolygon"},
           "members":[
             {"type":"way","role":"outer","geometry":[
               {"lat":0.0,"lon":0.0},{"lat":0.0,"lon":0.001},
               {"lat":0.001,"lon":0.001},{"lat":0.0,"lon":0.0}]},
             {"type":"way","role":"inner","geometry":[
               {"lat":0.0002,"lon":0.0002},{"lat":0.0002,"lon":0.0006},
               {"lat":0.0006,"lon":0.0006},{"lat":0.0002,"lon":0.0002}]}
           ]}
        ]}"#;
        let (feats, skips) =
            buildings_from_overpass(json, 6.0, "2026-07-10T00:00:00Z").expect("parses");
        assert!(skips.is_empty());
        assert_eq!(feats.len(), 1);
        let GeometryValue::Polygon { coordinates } = &feats[0].geometry.as_ref().unwrap().value
        else {
            panic!("Polygon expected");
        };
        assert_eq!(coordinates.len(), 2, "outer + one inner hole");
        // Default height (no height/levels tags).
        assert_eq!(
            feats[0]
                .property("height_provenance")
                .and_then(|v| v.as_str()),
            Some("default")
        );
    }

    #[test]
    fn malformed_json_is_a_typed_error_not_a_panic() {
        let got = buildings_from_overpass("{ not json", 6.0, "2026-07-10T00:00:00Z");
        assert!(matches!(got, Err(GisError::Json { .. })), "got {got:?}");
    }
}
