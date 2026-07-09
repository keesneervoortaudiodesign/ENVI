//! The frozen tensor-identity content hash (D-07).
//!
//! # Frozen encoding
//!
//! `tensor_hash` is `blake3` over a **canonical byte encoding** — never over
//! serialized JSON text (a serializer's float formatting is an invisible
//! cross-version coupling; 06-RESEARCH anti-pattern). The encoding, frozen so the
//! scheme itself can evolve behind the version prefix:
//!
//! 1. version prefix `b"envi-tensor-hash-v1"`;
//! 2. domain-separated ASCII field tags (`feat`, `met`, `recv`, …);
//! 3. `u64` little-endian **length prefixes** on every sequence;
//! 4. every `f64` as `to_bits().to_le_bytes()` (bit-exact, serializer-free);
//! 5. features hashed in **feature-uuid sort order** so identity is independent
//!    of feature ordering in the file.
//!
//! # What identity covers — and what it must NEVER cover (D-07)
//!
//! The hash covers three input groups. **Geometry**: each feature's kind, id,
//! geometry coordinates, and acoustic properties (heights, thickness, impedance
//! class, spectrum arrays). **Met**: the `MetDto` fields. The **receiver set**:
//! sorted id + position. These are exactly the inputs from which Phase 9
//! constructs each `envi_engine::solver::SolveJob`.
//!
//! Conditioning (gain / delay / filter / mute) is a **readout** parameter — it
//! appears nowhere in `SolveJob`, only at readout via
//! `envi_engine::tensor::compose_gain`. It is therefore **structurally
//! unhashable**: [`tensor_hash`] accepts no conditioning argument, and hashing it
//! would make Tier-1 recondition requests self-invalidating.
//! <!-- The `ConditioningDto` type is deliberately absent from every signature and
//! code path in this module; that structural exclusion IS the D-07 guarantee. -->

use geojson::FeatureCollection;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::dto::{MetDto, ReceiverDto};
use crate::geojson::geometry_positions;

/// Compute the frozen `blake3` tensor-identity hash over geometry + met +
/// receiver-set. Returns a 64-character lowercase hex digest.
///
/// The signature takes **only** the identity inputs — no conditioning type is
/// accepted (D-07). See the module `# Frozen encoding` header for the byte layout.
#[must_use]
pub fn tensor_hash(scene: &FeatureCollection, met: &MetDto, receivers: &[ReceiverDto]) -> String {
    let mut h = blake3::Hasher::new();
    h.update(b"envi-tensor-hash-v1");

    // --- geometry: features in uuid-sorted order (order-independence) ---
    h.update(b"scene");
    let mut features: Vec<(Uuid, &geojson::Feature)> = scene
        .features
        .iter()
        .map(|f| (feature_sort_key(f), f))
        .collect();
    features.sort_by_key(|a| a.0);
    put_len(&mut h, features.len());
    for (id, feature) in features {
        write_feature(&mut h, id, feature);
    }

    // --- met contribution ---
    h.update(b"met");
    put_f64(&mut h, met.temperature_c);
    put_f64(&mut h, met.humidity_pct);

    // --- receiver-set: sorted by id ---
    h.update(b"recv");
    let mut recv: Vec<&ReceiverDto> = receivers.iter().collect();
    recv.sort_by_key(|r| r.id);
    put_len(&mut h, recv.len());
    for r in recv {
        h.update(r.id.as_bytes());
        for c in r.position {
            put_f64(&mut h, c);
        }
    }

    h.finalize().to_hex().to_string()
}

/// Hash one feature: kind, id, geometry coordinates, then a fixed allowlist of
/// acoustic properties (each with a presence marker so absent != zero).
fn write_feature(h: &mut blake3::Hasher, id: Uuid, feature: &geojson::Feature) {
    h.update(b"feat");
    h.update(id.as_bytes());
    put_str(h, feature_kind_str(feature));

    // Geometry coordinates (skipped cleanly if geometry/type is unsupported —
    // identity then rests on id + kind + properties, still deterministic).
    match &feature.geometry {
        Some(g) => match geometry_positions(&g.value) {
            Ok(positions) => {
                put_len(h, positions.len());
                for p in positions {
                    let slice = p.as_slice();
                    put_len(h, slice.len());
                    for c in slice {
                        put_f64(h, *c);
                    }
                }
            }
            Err(_) => put_len(h, 0),
        },
        None => put_len(h, 0),
    }

    // Numeric acoustic properties, fixed order, presence-flagged.
    for key in ["eaves_height_m", "height_m", "thickness_m", "z_m"] {
        if let Some(v) = prop_f64(feature, key) {
            h.update(b"Pn");
            put_str(h, key);
            put_f64(h, v);
        }
    }
    // Impedance class letter (ground_zone).
    if let Some(s) = prop_str(feature, "impedance_class") {
        h.update(b"Ps");
        put_str(h, s);
    }
    // Dense spectrum array (source).
    if let Some(arr) = prop_f64_array(feature, "spectrum_band_db") {
        h.update(b"Pa");
        put_len(h, arr.len());
        for v in arr {
            put_f64(h, v);
        }
    }
}

/// The feature's uuid for sort ordering (nil if absent/unparseable — such
/// features are still hashed, just ordered first and deterministically).
fn feature_sort_key(feature: &geojson::Feature) -> Uuid {
    prop_str(feature, "id")
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or(Uuid::nil())
}

/// The feature's `kind` string (empty if absent).
fn feature_kind_str(feature: &geojson::Feature) -> &str {
    prop_str(feature, "kind").unwrap_or("")
}

fn prop_str<'a>(feature: &'a geojson::Feature, key: &str) -> Option<&'a str> {
    feature
        .properties
        .as_ref()
        .and_then(|p| p.get(key))
        .and_then(JsonValue::as_str)
}

fn prop_f64(feature: &geojson::Feature, key: &str) -> Option<f64> {
    feature
        .properties
        .as_ref()
        .and_then(|p| p.get(key))
        .and_then(JsonValue::as_f64)
}

fn prop_f64_array(feature: &geojson::Feature, key: &str) -> Option<Vec<f64>> {
    feature
        .properties
        .as_ref()
        .and_then(|p| p.get(key))
        .and_then(JsonValue::as_array)
        .map(|a| a.iter().filter_map(JsonValue::as_f64).collect())
}

/// `u64` little-endian length prefix.
fn put_len(h: &mut blake3::Hasher, n: usize) {
    h.update(&(n as u64).to_le_bytes());
}

/// One `f64` as its raw bits, little-endian (bit-exact, serializer-free).
fn put_f64(h: &mut blake3::Hasher, v: f64) {
    h.update(&v.to_bits().to_le_bytes());
}

/// A length-prefixed UTF-8 string.
fn put_str(h: &mut blake3::Hasher, s: &str) {
    put_len(h, s.len());
    h.update(s.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene_with_wall(wall_x: f64) -> FeatureCollection {
        let json = format!(
            r#"{{"type":"FeatureCollection","features":[
              {{"type":"Feature","geometry":{{"type":"LineString","coordinates":[[{wall_x},52.3733],[4.8945,52.3736]]}},
               "properties":{{"kind":"wall","id":"00000000-0000-0000-0000-000000000003","height_m":3.0}}}}
            ]}}"#
        );
        serde_json::from_str(&json).expect("valid FeatureCollection")
    }

    fn receiver(x: f64) -> ReceiverDto {
        ReceiverDto {
            id: Uuid::from_u128(42),
            position: [x, 20.0, 1.5],
        }
    }

    #[test]
    fn tensor_hash_ignores_conditioning_covers_identity_inputs() {
        let scene = scene_with_wall(4.8940);
        let met = MetDto::default();
        let receivers = vec![receiver(10.0)];

        // 64-char lowercase hex.
        let base = tensor_hash(&scene, &met, &receivers);
        assert_eq!(base.len(), 64, "blake3 hex is 64 chars");
        assert!(
            base.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "lowercase hex only"
        );

        // Deterministic: identical inputs -> identical hash.
        assert_eq!(base, tensor_hash(&scene, &met, &receivers));

        // Moving one receiver by 1 mm -> different hash.
        let moved = vec![receiver(10.0 + 0.001)];
        assert_ne!(
            base,
            tensor_hash(&scene, &met, &moved),
            "receiver move changes identity"
        );

        // Changing met temperature -> different hash.
        let warmer = MetDto {
            temperature_c: 25.0,
            humidity_pct: 70.0,
        };
        assert_ne!(
            base,
            tensor_hash(&scene, &warmer, &receivers),
            "met change changes identity"
        );

        // Editing a wall coordinate -> different hash.
        let edited = scene_with_wall(4.8941);
        assert_ne!(
            base,
            tensor_hash(&edited, &met, &receivers),
            "geometry change changes identity"
        );

        // Structural D-07 proof: `tensor_hash` has signature
        // `(scene, met, receivers)` — there is no parameter of the conditioning
        // type, so a `ConditioningDto` value simply cannot be threaded into
        // identity. Conditioning is a readout parameter (compose_gain), never a
        // tensor-identity input. This is a compile-time fact, asserted here in
        // prose because the exclusion is the absence of an argument.
    }

    #[test]
    fn order_independent_over_receiver_order() {
        let scene = scene_with_wall(4.8940);
        let met = MetDto::default();
        let a = ReceiverDto {
            id: Uuid::from_u128(1),
            position: [1.0, 2.0, 3.0],
        };
        let b = ReceiverDto {
            id: Uuid::from_u128(2),
            position: [4.0, 5.0, 6.0],
        };
        let forward = tensor_hash(&scene, &met, &[a.clone(), b.clone()]);
        let reversed = tensor_hash(&scene, &met, &[b, a]);
        assert_eq!(
            forward, reversed,
            "receiver ordering must not change identity"
        );
    }
}
