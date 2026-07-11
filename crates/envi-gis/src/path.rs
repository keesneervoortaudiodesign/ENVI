//! The Phase-9 → Phase-10 **path-assembly seam** and the L1 path-cache shape.
//!
//! # Module I/O
//! - **Inputs:** the per-`(sub_source, receiver)` geometry + weather this phase
//!   already built — a screen-injected, impedance-segmented
//!   [`TerrainProfile`], the source/receiver positions, the per-azimuth
//!   [`SoundSpeedProfile`] selector, an optional [`ForestCrossing`] seam, and a
//!   [`CorridorFingerprint`] capturing the geometry features intersecting the
//!   path corridor (the 09-02 rstar query result).
//! - **Output:** a pure-data [`PropagationPathInputs`] bundle (it runs NO solve,
//!   NO fan-out, and stores NO tensor — all of that is Phase 10) plus a
//!   [`PathCacheKey`] via [`PropagationPathInputs::cache_key`].
//!
//! # The L1 path-cache contract (ARCHITECTURE Data-Flow, Phase-11 Tier-2/Tier-3)
//!
//! This bundle is the input a Phase-10 assembler consumes to construct an
//! `envi_engine::solver::SolveJob`; it deliberately mirrors the `SolveJob`
//! *inputs* (profile, src/rcv, weather, forest) WITHOUT owning the solve. The
//! recalc router keys the L1 path cache on:
//!
//! > **`PathCacheKey = hash(geometry features ∩ path corridor)`**
//!
//! computed over the **geometry-derived identity only**: `src`, `rcv`, the
//! `TerrainProfile` geometry (its ordered `(x, z)` points — including injected
//! screen vertices — and each segment's `(σ, roughness)`), the `forest` crossing
//! geometry, and the [`CorridorFingerprint`]. The per-azimuth `weather` selector
//! is **DELIBERATELY EXCLUDED**, and `path_azimuth_deg` is excluded because it is
//! a deterministic function of `(src, rcv)` already hashed. The consequences are
//! the two recalc tiers this shape must stay compatible with:
//!
//! - **Tier-2 (weather what-if).** Changing only the atmosphere re-selects a new
//!   `weather` profile but leaves every `PathCacheKey` **unchanged**, so a
//!   weather re-solve reuses the cached extracted paths and never re-runs the
//!   (expensive) geometry extraction (Phase-11 weather difference maps).
//! - **Tier-3 (geometry dirty-diff).** Editing scene geometry changes the
//!   `CorridorFingerprint` (and/or the profile/forest geometry) of exactly the
//!   paths whose corridor a moved feature intersects, so their `PathCacheKey`
//!   changes and only those paths are re-extracted; untouched corridors keep
//!   their key and their cached paths.
//!
//! # Deterministic hashing (reproducible across runs)
//!
//! The key is a fixed **FNV-1a 64-bit** fold over the canonicalized IEEE-754 bits
//! of every geometry value (`−0.0` normalized to `+0.0`; the upstream builders
//! reject non-finite values, so no NaN reaches here). FNV-1a is chosen over
//! `DefaultHasher` for a hash that is stable across process runs and does not
//! depend on the standard library's SipHash keying — the cache key must be
//! reproducible for a persistent L1 cache.
//!
//! # Scope (NOT this plan)
//!
//! No solve, no `N × M` fan-out, no tensor store — those are Phase 10. The forest
//! seam carries only the **geometric** crossing (`ForestCrossing`); the Sub-Model
//! 10 `Fs` coherence factor stays deferred (Phase-5 05-01, "revisit Phase 9").

use envi_engine::forest::ForestCrossing;
use envi_engine::propagation::refraction::SoundSpeedProfile;
use envi_engine::scene::TerrainProfile;

/// FNV-1a 64-bit offset basis (the standard constant).
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
/// FNV-1a 64-bit prime (the standard constant).
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// A tiny incremental FNV-1a 64-bit hasher over `u64` words. Deterministic and
/// std-independent, so a [`PathCacheKey`] is reproducible across process runs
/// (unlike `std::collections::hash_map::DefaultHasher`, whose SipHash keying is
/// an implementation detail).
#[derive(Debug, Clone, Copy)]
struct Fnv1a(u64);

impl Fnv1a {
    /// A fresh hasher seeded with the FNV offset basis.
    fn new() -> Self {
        Self(FNV_OFFSET_BASIS)
    }

    /// Fold one 64-bit word in, byte-by-byte (little-endian), the FNV-1a way.
    fn write_u64(&mut self, word: u64) {
        for byte in word.to_le_bytes() {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(FNV_PRIME);
        }
    }

    /// Fold a canonicalized `f64` (see [`canonical_bits`]) in.
    fn write_f64(&mut self, x: f64) {
        self.write_u64(canonical_bits(x));
    }

    /// The accumulated 64-bit digest.
    fn finish(self) -> u64 {
        self.0
    }
}

/// Canonical IEEE-754 bits of a finite geometry value: `−0.0` is normalized to
/// `+0.0` so the two zero encodings hash identically. Upstream builders
/// (`TerrainProfile::new`, `ForestCrossing::new`, the extractors) reject
/// non-finite values, so a NaN never reaches this path; were one to, its bit
/// pattern would simply hash as itself (no panic).
fn canonical_bits(x: f64) -> u64 {
    let x = if x == 0.0 { 0.0 } else { x };
    x.to_bits()
}

/// One geometry feature intersecting a path corridor: a stable scene-feature id
/// plus a digest of its geometry. The id pins *which* feature it is; the
/// `geometry_digest` changes when that feature is edited (moved / reshaped), so a
/// Tier-3 dirty-diff sees the change through the [`CorridorFingerprint`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorridorFeature {
    /// Stable scene-feature identifier (assigned upstream; never re-minted here).
    pub feature_id: u64,
    /// A digest of the feature's geometry — changes when the feature is edited.
    pub geometry_digest: u64,
}

/// The stable identity of the geometry features whose footprint intersects a
/// path's corridor — the `geometry features ∩ path corridor` set the L1 cache key
/// hashes (the 09-02 rstar corridor query result, reduced to identities).
///
/// The fingerprint is **order-independent**: [`Self::digest`] sorts the features
/// before folding, so the same feature set produces the same digest regardless of
/// the order the corridor query returned them in.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CorridorFingerprint {
    /// The features intersecting the corridor (order-independent for hashing).
    pub features: Vec<CorridorFeature>,
}

impl CorridorFingerprint {
    /// An empty fingerprint (no geometry features in the corridor).
    #[must_use]
    pub fn new(features: Vec<CorridorFeature>) -> Self {
        Self { features }
    }

    /// An order-independent 64-bit digest of the corridor feature set: the
    /// features are sorted by `(feature_id, geometry_digest)` before folding, so
    /// two corridors with the same features in a different query order hash equal.
    #[must_use]
    fn digest(&self) -> u64 {
        let mut sorted: Vec<(u64, u64)> = self
            .features
            .iter()
            .map(|f| (f.feature_id, f.geometry_digest))
            .collect();
        sorted.sort_unstable();
        let mut h = Fnv1a::new();
        h.write_u64(sorted.len() as u64);
        for (id, digest) in sorted {
            h.write_u64(id);
            h.write_u64(digest);
        }
        h.finish()
    }
}

/// The L1 path-cache key — a 64-bit `hash(geometry features ∩ path corridor)`
/// (see the module docs). Weather-invariant by construction (Tier-2), and
/// sensitive to any corridor-geometry change (Tier-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PathCacheKey(pub u64);

/// The pure-data per-`(sub_source, receiver)` input bundle a Phase-10 assembler
/// consumes to build a `SolveJob` (it runs NO solve/fan-out itself).
///
/// It mirrors the `SolveJob` **inputs** produced by this phase; see the module
/// docs for the cache-key contract.
#[derive(Debug, Clone, PartialEq)]
pub struct PropagationPathInputs {
    /// The screen-injected, impedance-segmented cut-plane (09-01/02/03 output).
    pub profile: TerrainProfile,
    /// Source position `[x, y, z]`, meters (ground z; acoustic height at assembly).
    pub src: [f64; 3],
    /// Receiver position `[x, y, z]`, meters.
    pub rcv: [f64; 3],
    /// The path azimuth, degrees clockwise from north (a function of `src`/`rcv`).
    pub path_azimuth_deg: f64,
    /// The per-azimuth sound-speed selector for this path (`None` = homogeneous
    /// atmosphere). A **Tier-2 readout input** — EXCLUDED from [`Self::cache_key`].
    pub weather: Option<SoundSpeedProfile>,
    /// Optional forest-zone crossing on this path (geometric crossing only; the
    /// `Fs` coherence factor stays deferred). Its geometry IS part of the key.
    pub forest: Option<ForestCrossing>,
    /// The geometry features intersecting this path's corridor (the L1 key input).
    pub corridor: CorridorFingerprint,
}

impl PropagationPathInputs {
    /// Assemble the bundle from the Phase-9-built inputs. Pure data — it performs
    /// no solve, no fan-out, and no tensor allocation.
    #[must_use]
    pub fn new(
        profile: TerrainProfile,
        src: [f64; 3],
        rcv: [f64; 3],
        path_azimuth_deg: f64,
        weather: Option<SoundSpeedProfile>,
        forest: Option<ForestCrossing>,
        corridor: CorridorFingerprint,
    ) -> Self {
        Self {
            profile,
            src,
            rcv,
            path_azimuth_deg,
            weather,
            forest,
            corridor,
        }
    }

    /// Compute the L1 [`PathCacheKey`] = `hash(geometry features ∩ path corridor)`.
    ///
    /// Hashes ONLY the geometry-derived identity — `src`, `rcv`, the
    /// `TerrainProfile` geometry (ordered `(x, z)` points incl. injected screen
    /// vertices + per-segment `(σ, roughness)`), the `forest` crossing geometry,
    /// and the [`CorridorFingerprint`]. It **excludes** `weather` (a Tier-2 readout
    /// input) and `path_azimuth_deg` (a deterministic function of `src`/`rcv`).
    /// The result is therefore invariant under a weather-only change and changes
    /// under any corridor-geometry change.
    #[must_use]
    pub fn cache_key(&self) -> PathCacheKey {
        let mut h = Fnv1a::new();

        // Positions.
        for &c in self.src.iter().chain(self.rcv.iter()) {
            h.write_f64(c);
        }

        // TerrainProfile geometry: ordered (x, z) points (incl. injected screen
        // vertices) + each segment's impedance (σ) and roughness.
        let points = self.profile.points();
        h.write_u64(points.len() as u64);
        for p in points {
            h.write_f64(p[0]);
            h.write_f64(p[1]);
        }
        let segments = self.profile.segments();
        h.write_u64(segments.len() as u64);
        for s in segments {
            h.write_f64(s.flow_resistivity);
            h.write_f64(s.roughness);
        }

        // Forest crossing geometry (extent + physical/material params — never a
        // weather factor; a ForestCrossing carries none). `None` folds a marker.
        match &self.forest {
            None => h.write_u64(0),
            Some(fc) => {
                h.write_u64(1);
                h.write_f64(fc.d_m);
                h.write_f64(fc.density_per_m2);
                h.write_f64(fc.stem_radius_m);
                h.write_f64(fc.absorption);
                h.write_f64(fc.height_m);
            }
        }

        // Corridor feature set (order-independent).
        h.write_u64(self.corridor.digest());

        // NB: `weather` and `path_azimuth_deg` are DELIBERATELY not folded in.
        PathCacheKey(h.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use envi_engine::scene::{GroundSegment, TerrainProfile};

    /// A minimal two-point flat profile with one soft-ground segment.
    fn flat_profile() -> TerrainProfile {
        TerrainProfile::new(
            vec![[0.0, 0.0], [100.0, 0.0]],
            vec![GroundSegment {
                flow_resistivity: 200.0,
                roughness: 0.0,
            }],
        )
        .expect("valid flat profile")
    }

    /// A profile with an injected screen vertex (moved) — a corridor-geometry
    /// change relative to [`flat_profile`].
    fn screened_profile() -> TerrainProfile {
        TerrainProfile::new(
            vec![[0.0, 0.0], [50.0, 4.0], [100.0, 0.0]],
            vec![
                GroundSegment {
                    flow_resistivity: 200.0,
                    roughness: 0.0,
                },
                GroundSegment {
                    flow_resistivity: 200.0,
                    roughness: 0.0,
                },
            ],
        )
        .expect("valid screened profile")
    }

    fn ssp(a: f64) -> SoundSpeedProfile {
        SoundSpeedProfile {
            a,
            b: 0.0,
            c: 340.0,
            s_a: 0.0,
            s_b: 0.0,
            z0: 0.01,
        }
    }

    fn corridor(id: u64, geom: u64) -> CorridorFingerprint {
        CorridorFingerprint::new(vec![CorridorFeature {
            feature_id: id,
            geometry_digest: geom,
        }])
    }

    fn bundle(
        profile: TerrainProfile,
        weather: Option<SoundSpeedProfile>,
        forest: Option<ForestCrossing>,
        corridor: CorridorFingerprint,
    ) -> PropagationPathInputs {
        PropagationPathInputs::new(
            profile,
            [0.0, 0.0, 0.5],
            [100.0, 0.0, 1.5],
            90.0,
            weather,
            forest,
            corridor,
        )
    }

    /// The load-bearing Tier-2/Tier-3 contract: the key is a pure function of the
    /// geometry — a weather-only change leaves it EQUAL; any corridor-geometry
    /// change (moved screen vertex, changed impedance, changed corridor
    /// fingerprint, added/moved forest crossing) changes it.
    #[test]
    fn path_cache_key_is_geometry_only() {
        let base = bundle(flat_profile(), Some(ssp(1.0)), None, corridor(7, 100));
        let base_key = base.cache_key();

        // Tier-2: swap ONLY the weather selector → key unchanged.
        let weather_swapped = bundle(flat_profile(), Some(ssp(9.0)), None, corridor(7, 100));
        assert_eq!(
            base_key,
            weather_swapped.cache_key(),
            "a weather-only change must leave the path-cache key EQUAL (Tier-2)"
        );
        // Dropping weather entirely is still weather-invariant.
        let weather_none = bundle(flat_profile(), None, None, corridor(7, 100));
        assert_eq!(
            base_key,
            weather_none.cache_key(),
            "removing the weather selector must not change the key (Tier-2)"
        );

        // Tier-3a: a moved screen vertex (profile geometry) → key changes.
        let moved_vertex = bundle(screened_profile(), Some(ssp(1.0)), None, corridor(7, 100));
        assert_ne!(
            base_key,
            moved_vertex.cache_key(),
            "an injected/moved screen vertex must change the key (Tier-3)"
        );

        // Tier-3b: a changed segment impedance class (σ) → key changes.
        let harder_ground = TerrainProfile::new(
            vec![[0.0, 0.0], [100.0, 0.0]],
            vec![GroundSegment {
                flow_resistivity: 20_000.0,
                roughness: 0.0,
            }],
        )
        .unwrap();
        let impedance_changed = bundle(harder_ground, Some(ssp(1.0)), None, corridor(7, 100));
        assert_ne!(
            base_key,
            impedance_changed.cache_key(),
            "a changed per-segment impedance must change the key (Tier-3)"
        );

        // Tier-3c: a changed corridor fingerprint (feature edited) → key changes.
        let corridor_changed = bundle(flat_profile(), Some(ssp(1.0)), None, corridor(7, 999));
        assert_ne!(
            base_key,
            corridor_changed.cache_key(),
            "a changed corridor feature geometry must change the key (Tier-3)"
        );
    }

    /// The corridor fingerprint is order-independent: the same feature set in a
    /// different query order produces the same key.
    #[test]
    fn corridor_fingerprint_is_order_independent() {
        let a = CorridorFingerprint::new(vec![
            CorridorFeature {
                feature_id: 1,
                geometry_digest: 10,
            },
            CorridorFeature {
                feature_id: 2,
                geometry_digest: 20,
            },
        ]);
        let b = CorridorFingerprint::new(vec![
            CorridorFeature {
                feature_id: 2,
                geometry_digest: 20,
            },
            CorridorFeature {
                feature_id: 1,
                geometry_digest: 10,
            },
        ]);
        let ka = bundle(flat_profile(), None, None, a).cache_key();
        let kb = bundle(flat_profile(), None, None, b).cache_key();
        assert_eq!(ka, kb, "corridor feature order must not affect the key");
    }

    /// A forest crossing is part of the geometry identity: adding one changes the
    /// key, but it is still weather-invariant.
    #[test]
    fn forest_crossing_is_part_of_geometry_identity() {
        let no_forest = bundle(flat_profile(), Some(ssp(1.0)), None, corridor(3, 30));
        let fc = ForestCrossing::new(40.0, 0.4, 0.1, 0.2, 12.0).unwrap();
        let with_forest = bundle(flat_profile(), Some(ssp(1.0)), Some(fc), corridor(3, 30));
        assert_ne!(
            no_forest.cache_key(),
            with_forest.cache_key(),
            "adding a forest crossing must change the key (geometry identity)"
        );
        // Still weather-invariant with a forest present.
        let with_forest_other_weather =
            bundle(flat_profile(), Some(ssp(5.0)), Some(fc), corridor(3, 30));
        assert_eq!(
            with_forest.cache_key(),
            with_forest_other_weather.cache_key(),
            "a weather-only change with a forest present must leave the key EQUAL"
        );
    }
}
