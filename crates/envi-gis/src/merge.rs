//! Re-import merge by per-feature identity with a `user_modified` guard (D-09).
//!
//! # Module I/O
//! - **Inputs:** the `existing` scene features and a fresh `incoming` import set.
//! - **Output:** the merged feature list.
//! - **Invariant (load-bearing, D-09):** merge matches on the
//!   `(source, source_ref)` identity ([`crate::provenance::merge_key`]):
//!   * an existing feature flagged `user_modified` is **kept as-is** (user edits
//!     always survive re-import);
//!   * an existing untouched import is **refreshed** from the matching incoming
//!     feature (geometry/props updated);
//!   * an existing import absent from this re-import is **retained** (a smaller
//!     re-import bbox never deletes prior features);
//!   * a genuinely-new incoming feature is **added**;
//!   * a user-created feature without provenance has no import identity and is
//!     always kept.
//!
//! There is no parallel "import ledger" — identity and the edit flag live in the
//! feature properties (Pattern 4), so the merge cannot drift from the scene.

use std::collections::HashMap;

use geojson::Feature;

use crate::provenance::{is_user_modified, merge_key};

/// Merge a fresh `incoming` import into the `existing` scene features (D-09).
///
/// Order is deterministic: existing features keep their relative order; genuinely
/// new incoming features are appended in their original order.
#[must_use]
pub fn merge(existing: Vec<Feature>, incoming: Vec<Feature>) -> Vec<Feature> {
    // Index incoming keyed features by identity → position.
    let mut key_to_idx: HashMap<(String, String), usize> = HashMap::new();
    for (i, f) in incoming.iter().enumerate() {
        if let Some(k) = merge_key(f) {
            key_to_idx.insert(k, i);
        }
    }

    let mut incoming_slots: Vec<Option<Feature>> = incoming.into_iter().map(Some).collect();
    let mut consumed = vec![false; incoming_slots.len()];

    let mut out = Vec::new();
    for e in existing {
        match merge_key(&e) {
            Some(k) => {
                let matched = key_to_idx.get(&k).copied();
                if is_user_modified(&e) {
                    // User edits win; consume any matching incoming so it is not
                    // re-added as "new".
                    if let Some(i) = matched {
                        consumed[i] = true;
                    }
                    out.push(e);
                } else if let Some(i) = matched {
                    // Untouched import → refresh from incoming.
                    consumed[i] = true;
                    out.push(incoming_slots[i].take().expect("matched incoming present"));
                } else {
                    // Existing import not in this re-import → retain.
                    out.push(e);
                }
            }
            // User-created (no import identity) → always keep.
            None => out.push(e),
        }
    }

    // Append genuinely-new incoming features in original order.
    for (i, slot) in incoming_slots.into_iter().enumerate() {
        if !consumed[i]
            && let Some(f) = slot
        {
            out.push(f);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use geojson::{Feature, JsonObject, JsonValue};

    use crate::provenance::Provenance;

    /// A building feature with the given source_ref, user_modified flag, and a
    /// `marker` property used to tell "old" vs "refreshed" versions apart.
    fn feature(source_ref: &str, user_modified: bool, marker: &str) -> Feature {
        let prov = Provenance {
            source: "osm-overpass",
            source_ref: source_ref.to_string(),
            license: "ODbL-1.0",
            retrieved_at: "2026-07-10T00:00:00Z".to_string(),
            height_provenance: Some("levels"),
            vertical_datum: None,
        };
        let mut props = prov.into_properties("building");
        props.insert("user_modified".to_string(), JsonValue::from(user_modified));
        props.insert("marker".to_string(), JsonValue::from(marker));
        Feature {
            bbox: None,
            geometry: None,
            id: None,
            properties: Some(props),
            foreign_members: None,
        }
    }

    /// A user-drawn feature with no provenance (no import identity).
    fn user_created(marker: &str) -> Feature {
        let mut props = JsonObject::new();
        props.insert("kind".to_string(), JsonValue::from("building"));
        props.insert("marker".to_string(), JsonValue::from(marker));
        Feature {
            bbox: None,
            geometry: None,
            id: None,
            properties: Some(props),
            foreign_members: None,
        }
    }

    fn marker_of(f: &Feature) -> &str {
        f.property("marker").and_then(|v| v.as_str()).unwrap()
    }

    fn find<'a>(feats: &'a [Feature], source_ref: &str) -> Option<&'a Feature> {
        feats
            .iter()
            .find(|f| f.property("source_ref").and_then(|v| v.as_str()) == Some(source_ref))
    }

    #[test]
    fn untouched_import_is_refreshed_user_edits_survive_new_added_absent_retained() {
        let existing = vec![
            feature("way/1", false, "old"), // untouched → refresh
            feature("way/2", true, "user"), // user_modified → survive
            feature("way/4", false, "old"), // absent from re-import → retained
            user_created("drawn"),          // no identity → keep
        ];
        let incoming = vec![
            feature("way/1", false, "fresh"), // refreshes way/1
            feature("way/2", false, "fresh"), // must NOT overwrite the user edit
            feature("way/3", false, "new"),   // genuinely new → added
        ];

        let merged = merge(existing, incoming);

        // Untouched import refreshed.
        assert_eq!(marker_of(find(&merged, "way/1").unwrap()), "fresh");
        // User-modified survives untouched.
        let w2 = find(&merged, "way/2").unwrap();
        assert_eq!(marker_of(w2), "user");
        assert!(is_user_modified(w2), "user_modified flag preserved");
        // New feature added.
        assert_eq!(marker_of(find(&merged, "way/3").unwrap()), "new");
        // Absent-from-import existing retained.
        assert_eq!(marker_of(find(&merged, "way/4").unwrap()), "old");
        // User-created survives.
        assert!(
            merged.iter().any(|f| marker_of(f) == "drawn"),
            "user-created feature kept"
        );
        // way/2 appears once (incoming twin not re-added).
        let count_w2 = merged
            .iter()
            .filter(|f| f.property("source_ref").and_then(|v| v.as_str()) == Some("way/2"))
            .count();
        assert_eq!(
            count_w2, 1,
            "user feature not duplicated by its incoming twin"
        );
    }

    #[test]
    fn empty_incoming_preserves_everything() {
        let existing = vec![feature("way/1", false, "old"), user_created("drawn")];
        let merged = merge(existing, Vec::new());
        assert_eq!(merged.len(), 2);
        assert_eq!(marker_of(find(&merged, "way/1").unwrap()), "old");
    }

    #[test]
    fn empty_existing_adds_all_incoming() {
        let incoming = vec![
            feature("way/1", false, "new"),
            feature("way/2", false, "new"),
        ];
        let merged = merge(Vec::new(), incoming);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn user_modified_survives_even_when_absent_from_reimport() {
        let existing = vec![feature("way/9", true, "user")];
        let merged = merge(existing, vec![feature("way/1", false, "new")]);
        let w9 = find(&merged, "way/9").unwrap();
        assert_eq!(marker_of(w9), "user");
        assert!(is_user_modified(w9));
        assert!(find(&merged, "way/1").is_some(), "new feature added");
    }
}
