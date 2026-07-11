//! Provenance stamping as plain GeoJSON properties (D-11).
//!
//! # Module I/O
//! - **Inputs:** a [`Provenance`] record (source id, ref, license, retrieval
//!   time, optional height/vertical-datum notes).
//! - **Output:** those fields written into a feature's [`JsonObject`] property map
//!   as the plain key set [`PROVENANCE_KEYS`], plus read helpers the D-09 merge
//!   uses to recover a feature's `(source, source_ref)` identity and its
//!   `user_modified` guard.
//! - **Invariant (load-bearing):** provenance is **plain properties**, not a new
//!   schema. The Phase-6 store already preserves unknown additional properties
//!   (`envi-store/src/geojson.rs:50-57`), so every imported feature carries its
//!   provenance with **zero store schema change** (08-RESEARCH Pattern 4). Feature
//!   `id` is NOT set here — UUIDs are assigned in TS via `crypto.randomUUID()`
//!   (the wasm getrandom hazard is avoided; 08-RESEARCH Don't-Hand-Roll).

use geojson::{Feature, JsonObject, JsonValue};

/// Property key: originating source id (`"ahn4-dtm"`, `"osm-overpass"`, ...).
pub const KEY_SOURCE: &str = "source";
/// Property key: per-feature source reference (tile name / OSM element ref).
pub const KEY_SOURCE_REF: &str = "source_ref";
/// Property key: license tag of the source.
pub const KEY_LICENSE: &str = "license";
/// Property key: ISO-8601 retrieval timestamp.
pub const KEY_RETRIEVED_AT: &str = "retrieved_at";
/// Property key: `true` for imported features (vs user-drawn).
pub const KEY_IMPORTED: &str = "imported";
/// Property key: the D-09 merge guard — `true` once the user edits the feature.
pub const KEY_USER_MODIFIED: &str = "user_modified";
/// Property key: how a building height was derived (`"height_tag"` / `"levels"` /
/// `"default"` / `"measured"`).
pub const KEY_HEIGHT_PROVENANCE: &str = "height_provenance";
/// Property key: vertical datum note for terrain samples.
pub const KEY_VERTICAL_DATUM: &str = "vertical_datum";

/// The full provenance key set stamped onto imported features (Pattern 4).
pub const PROVENANCE_KEYS: [&str; 8] = [
    KEY_SOURCE,
    KEY_SOURCE_REF,
    KEY_LICENSE,
    KEY_RETRIEVED_AT,
    KEY_IMPORTED,
    KEY_USER_MODIFIED,
    KEY_HEIGHT_PROVENANCE,
    KEY_VERTICAL_DATUM,
];

/// A per-feature provenance record (D-11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    /// Originating source id (registry `SourceId`).
    pub source: &'static str,
    /// Per-feature source reference (tile name / `"way/123"` / `"relation/45"`).
    pub source_ref: String,
    /// License tag (registry `license`).
    pub license: &'static str,
    /// ISO-8601 retrieval timestamp (assigned by the caller/TS).
    pub retrieved_at: String,
    /// How the height was derived (buildings only), else `None`.
    pub height_provenance: Option<&'static str>,
    /// Vertical datum note (terrain only), else `None`.
    pub vertical_datum: Option<&'static str>,
}

impl Provenance {
    /// Stamp this provenance onto an existing property map. Always sets
    /// `imported = true` and `user_modified = false`; optional keys are written
    /// only when present.
    pub fn stamp(&self, props: &mut JsonObject) {
        props.insert(KEY_SOURCE.to_string(), JsonValue::from(self.source));
        props.insert(
            KEY_SOURCE_REF.to_string(),
            JsonValue::from(self.source_ref.clone()),
        );
        props.insert(KEY_LICENSE.to_string(), JsonValue::from(self.license));
        props.insert(
            KEY_RETRIEVED_AT.to_string(),
            JsonValue::from(self.retrieved_at.clone()),
        );
        props.insert(KEY_IMPORTED.to_string(), JsonValue::from(true));
        props.insert(KEY_USER_MODIFIED.to_string(), JsonValue::from(false));
        if let Some(hp) = self.height_provenance {
            props.insert(KEY_HEIGHT_PROVENANCE.to_string(), JsonValue::from(hp));
        }
        if let Some(vd) = self.vertical_datum {
            props.insert(KEY_VERTICAL_DATUM.to_string(), JsonValue::from(vd));
        }
    }

    /// Build a fresh property map for a feature of `kind`, provenance stamped.
    /// (No `id` — TS assigns the UUID.)
    #[must_use]
    pub fn into_properties(self, kind: &str) -> JsonObject {
        let mut props = JsonObject::new();
        props.insert("kind".to_string(), JsonValue::from(kind));
        self.stamp(&mut props);
        props
    }
}

/// Read a string property from a feature, if present.
#[must_use]
pub fn read_str<'a>(feature: &'a Feature, key: &str) -> Option<&'a str> {
    feature.property(key).and_then(JsonValue::as_str)
}

/// The `(source, source_ref)` merge identity of a feature, if both are present.
/// Features lacking either key (e.g. user-drawn features) have no import identity
/// and are always kept as-is by the merge.
#[must_use]
pub fn merge_key(feature: &Feature) -> Option<(String, String)> {
    let source = read_str(feature, KEY_SOURCE)?;
    let source_ref = read_str(feature, KEY_SOURCE_REF)?;
    Some((source.to_string(), source_ref.to_string()))
}

/// Whether the user has edited this feature (the D-09 guard). Absent key ⇒ `false`.
#[must_use]
pub fn is_user_modified(feature: &Feature) -> bool {
    feature
        .property(KEY_USER_MODIFIED)
        .and_then(JsonValue::as_bool)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Provenance {
        Provenance {
            source: "osm-overpass",
            source_ref: "way/123".to_string(),
            license: "ODbL-1.0",
            retrieved_at: "2026-07-10T00:00:00Z".to_string(),
            height_provenance: Some("levels"),
            vertical_datum: None,
        }
    }

    #[test]
    fn stamp_writes_plain_keys_with_imported_and_user_modified_defaults() {
        let props = sample().into_properties("building");
        assert_eq!(props.get("kind").unwrap().as_str(), Some("building"));
        assert_eq!(
            props.get(KEY_SOURCE).unwrap().as_str(),
            Some("osm-overpass")
        );
        assert_eq!(props.get(KEY_SOURCE_REF).unwrap().as_str(), Some("way/123"));
        assert_eq!(props.get(KEY_LICENSE).unwrap().as_str(), Some("ODbL-1.0"));
        assert_eq!(props.get(KEY_IMPORTED).unwrap().as_bool(), Some(true));
        assert_eq!(props.get(KEY_USER_MODIFIED).unwrap().as_bool(), Some(false));
        assert_eq!(
            props.get(KEY_HEIGHT_PROVENANCE).unwrap().as_str(),
            Some("levels")
        );
        // Absent optional key is not written.
        assert!(!props.contains_key(KEY_VERTICAL_DATUM));
        // No id is assigned in Rust (TS owns UUIDs).
        assert!(!props.contains_key("id"));
    }

    #[test]
    fn merge_key_and_user_modified_round_trip_through_a_feature() {
        let feature = Feature {
            bbox: None,
            geometry: None,
            id: None,
            properties: Some(sample().into_properties("building")),
            foreign_members: None,
        };
        assert_eq!(
            merge_key(&feature),
            Some(("osm-overpass".to_string(), "way/123".to_string()))
        );
        assert!(!is_user_modified(&feature));

        // A feature without provenance has no merge identity.
        let bare = Feature {
            bbox: None,
            geometry: None,
            id: None,
            properties: None,
            foreign_members: None,
        };
        assert_eq!(merge_key(&bare), None);
        assert!(!is_user_modified(&bare));
    }
}
