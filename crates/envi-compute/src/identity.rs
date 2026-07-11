//! The WASM-safe tensor-identity closure (factored from `envi-store`, D-07).
//!
//! This module is the pure, `std::fs`-free heart of the calculation identity:
//! the frozen [`tensor_hash`] content hash, the [`CalcManifest`] layout struct,
//! [`chunk_receivers`] (derived from the engine's budget constants), and the two
//! identity DTOs ([`MetDto`], [`ReceiverDto`]) plus the [`geometry_positions`]
//! helper the hash consumes. `envi-store` re-exports every one of these from its
//! original module paths, so its public API is unchanged; the browser compute
//! crate calls them directly without dragging `std::fs`/`tempfile` into wasm.
//!
//! Every dependency here — `blake3`, `serde`, `serde_json`, `geojson`, `uuid`,
//! `ts-rs` — is WASM-safe. The `std::fs` manifest I/O (`write_manifest`,
//! `read_manifest`, `atomic_write`) stays in `envi-store`.
//!
//! # Frozen encoding (moved byte-for-byte)
//!
//! [`tensor_hash`] is `blake3` over a **canonical byte encoding** — never over
//! serialized JSON text (a serializer's float formatting is an invisible
//! cross-version coupling). The encoding is frozen so the scheme itself can
//! evolve behind the version prefix:
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
//! geometry coordinates, and acoustic properties. **Met**: the [`MetDto`] fields.
//! The **receiver set**: sorted id + position. Conditioning (gain / delay /
//! filter / mute) is a **readout** parameter — it appears nowhere in `SolveJob`,
//! only at readout via `envi_engine::tensor::compose_gain`. It is therefore
//! **structurally unhashable**: [`tensor_hash`] accepts no conditioning argument.

use geojson::FeatureCollection;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use ts_rs::TS;
use uuid::Uuid;

use envi_engine::freq::N_BANDS;
use envi_engine::scene::Receiver;
use envi_engine::tensor::{BYTES_PER_CELL_PAIR, DEFAULT_TENSOR_BUDGET_BYTES};

/// Typed error for the pure identity boundary (WASM-safe; no `std::io`).
///
/// The identity closure is `pub` and may see unvalidated input; every malformed
/// coordinate or geometry yields one of these — the closure never panics on
/// data. `envi-store` maps this into its own `StoreError` via a `From` impl so
/// re-exported call sites stay source-compatible.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum IdentityError {
    /// A GeoJSON geometry type was unsupported for position extraction.
    #[error("GeoJSON error: {message}")]
    GeoJson {
        /// Human-readable reason.
        message: String,
    },
    /// A value that must be finite was NaN or infinite.
    #[error("non-finite value: {what}")]
    NonFinite {
        /// What the offending value was.
        what: String,
    },
}

// ---------------------------------------------------------------------------
// Identity DTOs (moved from envi-store::dto — the inputs tensor_hash consumes).
// ---------------------------------------------------------------------------

/// Serde twin of `envi_engine::scene::Receiver`, plus a stable feature `id`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "wire.ts")]
pub struct ReceiverDto {
    /// Stable feature id.
    pub id: Uuid,
    /// `[x, y, z]` in SceneXY meters (Z-up).
    pub position: [f64; 3],
}

impl TryFrom<&ReceiverDto> for Receiver {
    type Error = IdentityError;

    fn try_from(d: &ReceiverDto) -> Result<Self, IdentityError> {
        check_finite_3(&d.position, "receiver.position")?;
        Ok(Self {
            position: d.position,
        })
    }
}

/// Meteorological readout parameters. Extensible via `#[serde(default)]`:
/// deserializing `{}` yields the Nord2000 reference atmosphere (15 °C / 70 %).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export_to = "wire.ts")]
pub struct MetDto {
    /// Air temperature, °C (default 15.0).
    #[serde(default = "default_temperature_c")]
    pub temperature_c: f64,
    /// Relative humidity, % (default 70.0).
    #[serde(default = "default_humidity_pct")]
    pub humidity_pct: f64,
}

fn default_temperature_c() -> f64 {
    15.0
}

fn default_humidity_pct() -> f64 {
    70.0
}

impl Default for MetDto {
    fn default() -> Self {
        Self {
            temperature_c: default_temperature_c(),
            humidity_pct: default_humidity_pct(),
        }
    }
}

/// Reject any NaN/∞ component in a 3-vector before it reaches engine space.
fn check_finite_3(v: &[f64; 3], what: &str) -> Result<(), IdentityError> {
    if v.iter().any(|c| !c.is_finite()) {
        return Err(IdentityError::NonFinite {
            what: what.to_string(),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// geometry_positions (moved from envi-store::geojson — consumed by the hash).
// ---------------------------------------------------------------------------

/// All positions in a geometry value (Point/LineString/Polygon and their multi
/// variants). Used by the tensor hash and by `envi-store`'s scene validation;
/// unsupported geometry types are rejected.
///
/// # Errors
/// [`IdentityError::GeoJson`] on an unsupported geometry type.
pub fn geometry_positions(
    value: &geojson::GeometryValue,
) -> Result<Vec<&geojson::Position>, IdentityError> {
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
            return Err(IdentityError::GeoJson {
                message: format!("unsupported geometry type: {other:?}"),
            });
        }
    };
    Ok(out)
}

// ---------------------------------------------------------------------------
// tensor_hash (moved byte-for-byte from envi-store::hash — frozen encoding).
// ---------------------------------------------------------------------------

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
    // Destructured WITHOUT `..` so adding a `MetDto` field is a COMPILE error
    // right here, forcing an explicit hashing decision (LOW-7). The met identity
    // set is therefore total and self-documenting, not a silently-drifting
    // allowlist; a future scheme change rides behind the `v1` version prefix.
    h.update(b"met");
    let MetDto {
        temperature_c,
        humidity_pct,
    } = met;
    put_f64(&mut h, *temperature_c);
    put_f64(&mut h, *humidity_pct);

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
    // Dense spectrum array (source). Each element is hashed EXPLICITLY and
    // totally (LOW-6): a numeric element contributes an `n` tag + its bits; a
    // non-numeric element (which validation would reject on the persist path,
    // but `tensor_hash` is `pub` and may see unvalidated input) contributes an
    // `x` tag + its canonical JSON text. Nothing is silently dropped, so
    // `[1.0, "x", 2.0]` can never collide with `[1.0, 2.0]`.
    if let Some(arr) = prop_array(feature, "spectrum_band_db") {
        h.update(b"Pa");
        put_len(h, arr.len());
        for v in arr {
            match v.as_f64() {
                Some(f) => {
                    h.update(b"n");
                    put_f64(h, f);
                }
                None => {
                    h.update(b"x");
                    put_str(h, &v.to_string());
                }
            }
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

/// The raw JSON array under `key` (elements hashed explicitly by the caller so
/// non-numeric entries are never silently dropped — LOW-6).
fn prop_array<'a>(feature: &'a geojson::Feature, key: &str) -> Option<&'a Vec<JsonValue>> {
    feature
        .properties
        .as_ref()
        .and_then(|p| p.get(key))
        .and_then(JsonValue::as_array)
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

// ---------------------------------------------------------------------------
// CalcManifest + chunk_receivers (moved from envi-store::manifest — pure parts).
// ---------------------------------------------------------------------------

/// The calculation manifest persisted at `calc/<calc_id>/manifest.json`.
///
/// The `std::fs` reader/writer (`write_manifest`/`read_manifest`) stays in
/// `envi-store`; this struct is the pure format they serialize.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalcManifest {
    /// The calculation id (also the `calc/<id>/` folder name).
    pub calc_id: Uuid,
    /// `[S, R, F]` — mirrors `TensorPair [sub_source, receiver, freq]`; `F` is
    /// always 105 ([`N_BANDS`]).
    pub dims: [usize; 3],
    /// Number of receivers per chunk (receiver-axis chunking; see
    /// [`chunk_receivers`]).
    pub chunk_receivers: usize,
    /// The frozen tensor-identity content hash (see [`tensor_hash`]).
    pub tensor_hash: String,
    /// Honest-stub provenance: `true` while compute is stubbed (Phase 6 always).
    pub stub: bool,
    /// Creation time, unix epoch seconds.
    pub created_at_unix: u64,
}

/// Receivers per chunk under the engine's streaming budget:
/// `floor(DEFAULT_TENSOR_BUDGET_BYTES / (n_sub · 105 · BYTES_PER_CELL_PAIR))`,
/// capped at the receiver count `R` (and at least 1). The constants are imported
/// from `envi_engine::tensor` — never re-derived here.
#[must_use]
pub fn chunk_receivers(n_sub: usize, n_receivers: usize) -> usize {
    let per_receiver = n_sub.max(1) * N_BANDS * BYTES_PER_CELL_PAIR;
    let raw = DEFAULT_TENSOR_BUDGET_BYTES / per_receiver;
    raw.max(1).min(n_receivers.max(1))
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
    fn tensor_hash_matches_frozen_pre_refactor_digest() {
        // Frozen-encoding regression (threat T-10-01-01): a fixed input must hash
        // to the exact digest the pre-refactor `envi-store::hash::tensor_hash`
        // produced. A silent re-encoding after the move would flip this.
        let scene = scene_with_wall(4.8940);
        let met = MetDto::default();
        let receivers = vec![receiver(10.0)];
        assert_eq!(
            tensor_hash(&scene, &met, &receivers),
            "dcb2485e921d0c4774f4c3ea0bdc44adafaff61a58bfcda6d56593490f92ee2c",
            "frozen tensor_hash digest changed — the byte encoding was NOT preserved"
        );
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
    fn spectrum_non_numeric_entries_are_hashed_not_dropped() {
        // A source feature carrying a spectrum with a non-numeric element must NOT
        // hash identically to the same spectrum with that element removed (LOW-6:
        // the identity contract stays injective for unvalidated inputs).
        let met = MetDto::default();
        let receivers = vec![receiver(10.0)];

        let with_junk: FeatureCollection = serde_json::from_str(
            r#"{"type":"FeatureCollection","features":[
              {"type":"Feature","geometry":{"type":"Point","coordinates":[4.894,52.373]},
               "properties":{"kind":"source","id":"00000000-0000-0000-0000-000000000009",
                             "spectrum_band_db":[1.0,"x",2.0]}}]}"#,
        )
        .expect("valid FC");
        let without_junk: FeatureCollection = serde_json::from_str(
            r#"{"type":"FeatureCollection","features":[
              {"type":"Feature","geometry":{"type":"Point","coordinates":[4.894,52.373]},
               "properties":{"kind":"source","id":"00000000-0000-0000-0000-000000000009",
                             "spectrum_band_db":[1.0,2.0]}}]}"#,
        )
        .expect("valid FC");

        assert_ne!(
            tensor_hash(&with_junk, &met, &receivers),
            tensor_hash(&without_junk, &met, &receivers),
            "a dropped non-numeric spectrum entry must not collide (LOW-6)"
        );
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

    #[test]
    fn met_dto_defaults_apply() {
        let met: MetDto = serde_json::from_str("{}").expect("empty object deserializes");
        assert_eq!(met.temperature_c, 15.0);
        assert_eq!(met.humidity_pct, 70.0);
    }

    #[test]
    fn receiver_dto_rejects_non_finite_position() {
        let bad = ReceiverDto {
            id: Uuid::from_u128(1),
            position: [1.0, f64::NAN, 3.0],
        };
        assert!(matches!(
            Receiver::try_from(&bad),
            Err(IdentityError::NonFinite { .. })
        ));
        let good = ReceiverDto {
            id: Uuid::from_u128(1),
            position: [1.0, 2.0, 3.0],
        };
        assert_eq!(
            Receiver::try_from(&good).expect("finite converts").position,
            [1.0, 2.0, 3.0]
        );
    }

    #[test]
    fn chunk_receivers_matches_engine_constants() {
        // Two sub-sources, 100_000 receivers: budget-bound, below R.
        let n_sub = 2;
        let per_receiver = n_sub * N_BANDS * BYTES_PER_CELL_PAIR;
        let expected = (DEFAULT_TENSOR_BUDGET_BYTES / per_receiver).min(100_000);
        assert_eq!(chunk_receivers(n_sub, 100_000), expected);

        // dims [1, 3, 105]: capped at the receiver count.
        assert_eq!(chunk_receivers(1, 3), 3, "capped at the receiver count");
    }
}
