//! Re-import merge by per-feature identity with a `user_modified` guard (D-09).
//!
//! # Module I/O
//! - **Inputs:** the `existing` scene features and a fresh `incoming` import set.
//! - **Output:** the merged feature list.
//! - **Invariant (load-bearing, D-09):** merge matches on the
//!   `(source, source_ref)` identity ([`crate::provenance::merge_key`]). Crucially
//!   this identity is **many-to-one** for raster layers: terrain / land-cover
//!   stamp ONE tile-level `source_ref` onto *every* feature of a tile, so a key
//!   names a **GROUP** of features, not a single one. The merge therefore works
//!   per key-GROUP (never assuming a 1:1 mapping — that assumption panicked the
//!   WASM on re-import, CR-01):
//!   * if **any** existing member of a key's group is flagged `user_modified`, the
//!     whole existing group is **kept as-is** and the incoming group for that key
//!     is **suppressed** — user edits always survive re-import, and the untouched
//!     siblings are never dropped (no data loss) nor duplicated;
//!   * otherwise, an untouched existing group whose key is present in the incoming
//!     import is **replaced** wholesale by the incoming group (raster samples have
//!     no stable per-point identity across imports, so the group is the unit);
//!   * an existing import group absent from this re-import is **retained** (a
//!     smaller re-import bbox never deletes prior features);
//!   * a genuinely-new incoming key is **added**;
//!   * a user-created feature without provenance has no import identity and is
//!     always kept.
//!
//! There is no parallel "import ledger" — identity and the edit flag live in the
//! feature properties (Pattern 4), so the merge cannot drift from the scene. The
//! whole operation is total: no `.expect`/`unwrap` on the data path, so a shared
//! key can never trap the WASM.

use std::collections::HashSet;

use geojson::Feature;

use crate::provenance::{is_user_modified, merge_key};

/// Merge a fresh `incoming` import into the `existing` scene features (D-09).
///
/// Order is deterministic: existing features keep their relative order; genuinely
/// new (or refreshed) incoming features are appended in their original order.
#[must_use]
pub fn merge(existing: Vec<Feature>, incoming: Vec<Feature>) -> Vec<Feature> {
    // Keys "locked" by a user edit: any existing feature of the key's group is
    // user-modified. The whole existing group survives and the incoming group for
    // that key is suppressed (edits win; untouched siblings are neither dropped
    // nor duplicated by an incoming twin).
    let locked_keys: HashSet<(String, String)> = existing
        .iter()
        .filter(|f| is_user_modified(f))
        .filter_map(merge_key)
        .collect();

    // Which keys the incoming import carries at all (group membership, not a
    // single index) — an existing untouched group whose key appears here is
    // replaced wholesale by the incoming group.
    let incoming_keys: HashSet<(String, String)> = incoming.iter().filter_map(merge_key).collect();

    let mut out = Vec::new();
    for e in existing {
        match merge_key(&e) {
            // Locked group (a member was user-edited) → keep the entire existing
            // group verbatim.
            Some(k) if locked_keys.contains(&k) => out.push(e),
            // Untouched existing group whose key is refreshed this round → drop
            // here; the incoming group is appended below.
            Some(k) if incoming_keys.contains(&k) => { /* replaced by incoming */ }
            // Existing import absent from this re-import, or a user-created feature
            // with no import identity → retain.
            _ => out.push(e),
        }
    }

    // Append incoming features that refresh or newly add a key, in original order.
    // Incoming whose key is locked by a user edit is suppressed so a user-modified
    // feature is never clobbered or duplicated by its incoming twin.
    for f in incoming {
        match merge_key(&f) {
            Some(k) if locked_keys.contains(&k) => { /* user edit wins; suppress */ }
            _ => out.push(f),
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

    // --- CR-01 regression: the terrain / land-cover many-features-per-key shape ---
    //
    // Terrain and land-cover stamp ONE tile-level `source_ref` onto EVERY feature
    // of a tile, so all ~4000 elevation points of a tile share a single merge key.
    // The old 1:1 merge `.expect()`ed a per-feature-unique key and trapped the WASM
    // (`unreachable`) on re-import of a populated scene. These tests lock in the
    // group-replace semantics that fix it. All features here share the ONE key
    // `("osm-overpass", "M_19FN2")`.

    #[test]
    fn many_features_sharing_one_key_do_not_panic_and_preserve_user_edits() {
        let existing = vec![
            feature("M_19FN2", false, "old-a"),
            feature("M_19FN2", true, "user"), // one user-edited point in the group
            feature("M_19FN2", false, "old-b"),
        ];
        let incoming = vec![
            feature("M_19FN2", false, "fresh-a"),
            feature("M_19FN2", false, "fresh-b"),
        ];

        // The 1:1 merge panicked here; the group merge must not.
        let merged = merge(existing, incoming);

        // The user edit survives exactly once (never clobbered, never duplicated).
        let users: Vec<_> = merged.iter().filter(|f| marker_of(f) == "user").collect();
        assert_eq!(users.len(), 1, "user edit preserved exactly once");
        assert!(is_user_modified(users[0]), "user_modified flag preserved");
        // The untouched siblings of a locked group are NOT dropped (no data loss),
        // and the incoming twins are suppressed rather than duplicated.
        assert!(
            merged.iter().any(|f| marker_of(f) == "old-a"),
            "untouched sibling of a locked group retained"
        );
        assert!(
            merged.iter().all(|f| !marker_of(f).starts_with("fresh")),
            "incoming group suppressed while the key is user-locked"
        );

        // Re-import is idempotent: merging the same incoming again neither grows the
        // set unboundedly nor loses the user edit.
        let again = merge(
            merged.clone(),
            vec![
                feature("M_19FN2", false, "fresh-a"),
                feature("M_19FN2", false, "fresh-b"),
            ],
        );
        assert_eq!(again.len(), merged.len(), "re-import is idempotent");
        assert_eq!(
            again.iter().filter(|f| marker_of(f) == "user").count(),
            1,
            "user edit still preserved on second re-import"
        );
    }

    #[test]
    fn shared_key_group_without_user_edits_is_fully_refreshed() {
        let existing = vec![
            feature("M_19FN2", false, "old-a"),
            feature("M_19FN2", false, "old-b"),
        ];
        let incoming = vec![
            feature("M_19FN2", false, "fresh-a"),
            feature("M_19FN2", false, "fresh-b"),
            feature("M_19FN2", false, "fresh-c"),
        ];

        let merged = merge(existing, incoming);

        // Stale group dropped, refreshed group appended wholesale — no panic, no
        // stale survivors, correct new cardinality.
        assert!(
            merged.iter().all(|f| marker_of(f).starts_with("fresh")),
            "stale group replaced by the incoming group"
        );
        assert_eq!(merged.len(), 3, "refreshed group cardinality");
    }
}
