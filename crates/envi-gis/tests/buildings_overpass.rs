//! Building-height fallback chain over a REAL Overpass capture.
//!
//! The fixture `tests/fixtures/overpass/amsterdam_buildings.json` is a trimmed
//! **live** Overpass `out body geom` response for the Amsterdam-centre viewport
//! (bbox `4.894,52.363 → 4.914,52.373`) — real way/relation ids, real tags, real
//! geometry. It deliberately spans all three tiers of the locked D-10 chain:
//!
//! | tier            | fixture elements                        | expected height          |
//! |-----------------|-----------------------------------------|--------------------------|
//! | `height` tag    | `way/31980601` (14.5), `way/45038620` (20.1) | the tag, verbatim   |
//! | `building:levels` | `way/266922821` (3), `way/266927355` (4)   | levels × 3 + 1.5    |
//! | neither         | `way/31762790`, `way/31762791` (greenhouses) | the user default   |
//!
//! The reported symptom was "every imported Amsterdam building carries the SAME
//! height". This test is the guard: the three tiers must produce three DIFFERENT
//! heights and `height_provenance` must name the tier each one came from — a
//! regression to a single flat height (all-default, or a per-feature write that
//! got lost) fails here.

use std::collections::BTreeSet;

use envi_gis::buildings::{LEVEL_HEIGHT_M, ROOF_ALLOWANCE_M, buildings_from_overpass};
use geojson::Feature;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/overpass/amsterdam_buildings.json"
);

/// The user default is deliberately a value NO fixture element resolves to from
/// its tags, so "everything fell through to the default" cannot masquerade as a
/// pass.
const USER_DEFAULT_M: f64 = 6.0;

fn parse() -> Vec<Feature> {
    let json = std::fs::read_to_string(FIXTURE).expect("overpass fixture exists");
    let (feats, skips) =
        buildings_from_overpass(&json, USER_DEFAULT_M, "2026-07-13T00:00:00Z").expect("parses");
    assert!(
        skips.is_empty(),
        "no element of the real capture is invalid: {skips:?}"
    );
    feats
}

fn find(feats: &[Feature], source_ref: &str) -> Feature {
    feats
        .iter()
        .find(|f| f.property("source_ref").and_then(|v| v.as_str()) == Some(source_ref))
        .unwrap_or_else(|| panic!("{source_ref} must be in the parsed features"))
        .clone()
}

fn height_of(f: &Feature) -> f64 {
    f.property("eaves_height_m")
        .and_then(serde_json::Value::as_f64)
        .expect("every building carries eaves_height_m")
}

fn provenance_of(f: &Feature) -> String {
    f.property("height_provenance")
        .and_then(|v| v.as_str())
        .expect("every building carries height_provenance")
        .to_string()
}

#[test]
fn the_three_fallback_tiers_produce_three_different_heights() {
    let feats = parse();
    assert_eq!(feats.len(), 7, "every building element became a feature");

    // Tier 2 — the `height` tag, taken verbatim.
    let tagged = find(&feats, "way/31980601");
    assert_eq!(height_of(&tagged), 14.5);
    assert_eq!(provenance_of(&tagged), "height_tag");

    // Tier 3 — `building:levels` x 3 + 1.5 (levels = 3).
    let levelled = find(&feats, "way/266922821");
    assert_eq!(
        height_of(&levelled),
        3.0 * LEVEL_HEIGHT_M + ROOF_ALLOWANCE_M
    );
    assert_eq!(provenance_of(&levelled), "levels");

    // Tier 4 — no height data at all -> the user default.
    let bare = find(&feats, "way/31762790");
    assert_eq!(height_of(&bare), USER_DEFAULT_M);
    assert_eq!(provenance_of(&bare), "default");

    // The load-bearing property: the tiers are genuinely DIFFERENT heights.
    let three = [height_of(&tagged), height_of(&levelled), height_of(&bare)];
    assert_eq!(
        three,
        [14.5, 10.5, 6.0],
        "the three tiers must resolve to three distinct heights"
    );
    assert_eq!(
        BTreeSet::from([14.5_f64.to_bits(), 10.5_f64.to_bits(), 6.0_f64.to_bits()]).len(),
        3,
        "no two tiers may collapse onto the same height"
    );
}

#[test]
fn every_tier_is_exercised_and_heights_are_per_feature_not_uniform() {
    let feats = parse();

    // Coverage: all three provenance tiers appear in the real capture — a fixture
    // that lost its tags (or a parser that dropped them) would collapse to one.
    let tiers: BTreeSet<String> = feats.iter().map(provenance_of).collect();
    assert_eq!(
        tiers,
        BTreeSet::from([
            "default".to_string(),
            "height_tag".to_string(),
            "levels".to_string()
        ]),
        "the capture must exercise all three tiers"
    );

    // THE regression: heights must VARY across the imported features. "All
    // buildings have the same height" is precisely a set of size 1 here.
    let distinct: BTreeSet<u64> = feats.iter().map(|f| height_of(f).to_bits()).collect();
    assert!(
        distinct.len() >= 4,
        "imported heights must vary per feature, got {} distinct value(s)",
        distinct.len()
    );

    // Levels are read per-feature (3 -> 10.5, 4 -> 13.5), not once for the layer.
    assert_eq!(height_of(&find(&feats, "way/266922821")), 10.5);
    assert_eq!(height_of(&find(&feats, "way/266927355")), 13.5);

    // A multipolygon relation carries its own height tag through the same chain.
    let rel = find(&feats, "relation/3583193");
    assert_eq!(height_of(&rel), 19.6);
    assert_eq!(provenance_of(&rel), "height_tag");
}

#[test]
fn imported_buildings_carry_the_exact_keys_the_store_reads() {
    // `eaves_height_m` is what `envi_store::geojson::building_from_feature` reads;
    // a parallel `height_m` would be silently ignored there (and the building would
    // be rejected for a missing property).
    for f in parse() {
        assert!(f.property("eaves_height_m").is_some());
        assert!(f.property("height_m").is_none(), "never a parallel key");
        assert_eq!(
            f.property("kind").and_then(|v| v.as_str()),
            Some("building")
        );
        assert_eq!(
            f.property("source").and_then(|v| v.as_str()),
            Some("osm-overpass")
        );
        assert!(f.id.is_none(), "TS owns the UUID");
    }
}
