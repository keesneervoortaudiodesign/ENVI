//! The engine's canonical **semantic 2.5D scene** vocabulary (GEO-01).
//!
//! # Coordinate convention (read this before touching a position)
//!
//! All coordinates are expressed in a **projected metric CRS** (site-local;
//! v2 auto-picks a UTM zone), in **meters**, **Z-up**. Positions are
//! `[x, y, z]` (or `[x, z]` in a vertical cut plane). Phase 1 sources are
//! FORCE case files, which are already local metric 2-D sections — so
//! [`CrsInfo`] is a descriptive tag only; `proj`/CRS transforms are explicitly
//! deferred to v2 and NO `proj` dependency is added here.
//!
//! # Naming discipline instead of a units type-system
//!
//! Per 01-RESEARCH "Anti-Patterns to Avoid": ENVI uses `f64` with `_m` / `_db`
//! / `_hz` suffixes rather than a `Quantity` wrapper. A units crate adds
//! friction across every equation transcription; the Nord2000-implementation
//! norm is naming discipline. Domain constructors are the guard rail: they
//! validate and return typed [`SceneError`], never panic on data.
//!
//! # The hSv / hRv off-by-metres trap
//!
//! Source height is measured above the **FIRST** terrain-profile point,
//! receiver height above the **LAST** (AV 1106/07 §5.3.1). Getting this wrong
//! silently shifts geometry by metres and masquerades as a physics bug in
//! later phases. It is encoded in exactly one place — [`TerrainProfile::endpoints`].

use crate::freq::N_BANDS;

use thiserror::Error;

/// Errors from constructing scene domain types out of untrusted case data.
///
/// Every malformed input yields one of these — the scene constructors never
/// panic on data (threat T-01-05, DoS via malformed-input panic).
#[derive(Debug, Error, PartialEq)]
pub enum SceneError {
    /// A terrain profile must contain at least one point.
    #[error("terrain profile is empty")]
    EmptyProfile,
    /// Profile X coordinates must be strictly ascending along the cut plane.
    #[error("terrain profile X not strictly ascending at point {index} (x = {x} after {prev_x})")]
    NonAscendingX {
        /// Offending point index (0-based).
        index: usize,
        /// Previous X value.
        prev_x: f64,
        /// Offending X value.
        x: f64,
    },
    /// `N` points require exactly `N − 1` ground segments.
    #[error("terrain profile has {points} points but {segments} segments (expected {expected})")]
    SegmentCountMismatch {
        /// Number of profile points.
        points: usize,
        /// Number of ground segments supplied.
        segments: usize,
        /// Number of segments required (`points − 1`).
        expected: usize,
    },
    /// A coordinate or segment property was NaN or infinite.
    #[error("non-finite value: {what}")]
    NonFinite {
        /// What the offending value was.
        what: String,
    },
}

/// Descriptive CRS tag. A real projection transform is v2 — Phase 1 inputs are
/// already site-local metric, so this only records the convention in one place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrsInfo {
    /// Human-readable CRS label, e.g. `"local-metric"`.
    pub label: String,
}

impl CrsInfo {
    /// The Phase 1 default: a site-local metric CRS, Z-up, meters.
    #[must_use]
    pub fn local_metric() -> Self {
        Self {
            label: "local-metric".to_string(),
        }
    }
}

impl Default for CrsInfo {
    fn default() -> Self {
        Self::local_metric()
    }
}

/// Per-1/12-octave sound-power spectrum `L_W` (dB re 1 pW), length
/// [`N_BANDS`](crate::freq::N_BANDS).
///
/// Wraps a fixed-length array so a spectrum can never be the wrong length on
/// the engine's frequency grid.
#[derive(Debug, Clone, PartialEq)]
pub struct BandSpectrum {
    values_db: [f64; N_BANDS],
}

impl BandSpectrum {
    /// A flat spectrum with the same `L_W` in every band (e.g. `uniform(0.0)`
    /// is the unit transfer-test source).
    #[must_use]
    pub fn uniform(db: f64) -> Self {
        Self {
            values_db: [db; N_BANDS],
        }
    }

    /// Build from an explicit per-band array of `L_W` values.
    #[must_use]
    pub fn from_values(values_db: [f64; N_BANDS]) -> Self {
        Self { values_db }
    }

    /// Per-band `L_W` values as a slice (length [`N_BANDS`]).
    #[must_use]
    pub fn as_slice(&self) -> &[f64] {
        &self.values_db
    }
}

/// A directional point sub-source: a position plus its per-band `L_W`.
///
/// Composition-ready for SRC-01 (plan 01-03 makes it functional). A complex
/// source is a `Vec<SubSource>`; directivity and the `PropagationCorrection`
/// hook arrive in later phases.
#[derive(Debug, Clone, PartialEq)]
pub struct SubSource {
    /// `[x, y, z]` in the local metric CRS, Z-up, meters.
    pub position: [f64; 3],
    /// Per-1/12-octave sound-power spectrum.
    pub spectrum: BandSpectrum,
}

/// A complex sound source built from one or more directional sub-sources.
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    /// The sub-sources composing this source.
    pub sub_sources: Vec<SubSource>,
}

/// A receiver point.
#[derive(Debug, Clone, PartialEq)]
pub struct Receiver {
    /// `[x, y, z]` in the local metric CRS, Z-up, meters.
    pub position: [f64; 3],
}

/// A vertical screen (noise barrier).
///
/// Phase 1 only needs a `Barrier` to be a semantic object able to LIST its
/// screen edges via [`Barrier::edges`]. The merge-into-terrain-profile
/// algorithm (screens become part of the terrain section — AV 1106/07 §5.1)
/// is Phase 2.
#[derive(Debug, Clone, PartialEq)]
pub struct Barrier {
    /// Polyline of top-edge vertices `[x, y, z]`.
    pub top_edge: Vec<[f64; 3]>,
    /// `None` = thin screen; `Some(t)` = thick screen of thickness `t` m
    /// (FORCE thin/thick/double screen groups).
    pub thickness_m: Option<f64>,
}

impl Barrier {
    /// The screen's top-edge segments as `(from, to)` vertex pairs.
    ///
    /// An `n`-vertex polyline yields `n − 1` edges; fewer than two vertices
    /// yields no edges.
    #[must_use]
    pub fn edges(&self) -> Vec<([f64; 3], [f64; 3])> {
        self.top_edge.windows(2).map(|w| (w[0], w[1])).collect()
    }
}

/// A 2.5D building: a footprint polygon extruded to an eaves height.
#[derive(Debug, Clone, PartialEq)]
pub struct Building {
    /// Footprint polygon vertices `[x, y]` in the local metric CRS.
    pub footprint: Vec<[f64; 2]>,
    /// Eaves height above local ground, meters.
    pub eaves_height_m: f64,
}

/// One ground segment between two consecutive terrain-profile points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundSegment {
    /// Ground flow resistivity, kNs·m⁻⁴ (Nordtest σ; see [`impedance_class`]).
    pub flow_resistivity: f64,
    /// Terrain roughness, meters (class N = 0).
    pub roughness: f64,
}

/// A terrain profile: the vertical cut-plane section source→receiver.
///
/// AV 1106/07 §5.3.1 contract: `points` are `(x, z)` with **strictly
/// ascending** x along the cut plane, and `N` points define `N − 1`
/// [`GroundSegment`]s (each segment carries the impedance/roughness of the
/// ground it spans). Build via [`TerrainProfile::new`], which validates the
/// contract and returns a typed [`SceneError`] on violation.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainProfile {
    points: Vec<[f64; 2]>,
    segments: Vec<GroundSegment>,
}

impl TerrainProfile {
    /// Validate and construct a terrain profile.
    ///
    /// # Errors
    ///
    /// - [`SceneError::EmptyProfile`] if `points` is empty
    /// - [`SceneError::SegmentCountMismatch`] if `segments.len() != points.len() − 1`
    /// - [`SceneError::NonFinite`] if any coordinate or segment property is NaN/∞
    /// - [`SceneError::NonAscendingX`] if profile X is not strictly ascending
    pub fn new(points: Vec<[f64; 2]>, segments: Vec<GroundSegment>) -> Result<Self, SceneError> {
        if points.is_empty() {
            return Err(SceneError::EmptyProfile);
        }
        let expected = points.len() - 1;
        if segments.len() != expected {
            return Err(SceneError::SegmentCountMismatch {
                points: points.len(),
                segments: segments.len(),
                expected,
            });
        }
        for (i, p) in points.iter().enumerate() {
            if !p[0].is_finite() {
                return Err(SceneError::NonFinite {
                    what: format!("profile X (point {i})"),
                });
            }
            if !p[1].is_finite() {
                return Err(SceneError::NonFinite {
                    what: format!("profile Z (point {i})"),
                });
            }
            if i > 0 {
                let prev_x = points[i - 1][0];
                if p[0] <= prev_x {
                    return Err(SceneError::NonAscendingX {
                        index: i,
                        prev_x,
                        x: p[0],
                    });
                }
            }
        }
        for (i, s) in segments.iter().enumerate() {
            if !s.flow_resistivity.is_finite() {
                return Err(SceneError::NonFinite {
                    what: format!("flow resistivity (segment {i})"),
                });
            }
            if !s.roughness.is_finite() {
                return Err(SceneError::NonFinite {
                    what: format!("roughness (segment {i})"),
                });
            }
        }
        Ok(Self { points, segments })
    }

    /// Profile points `(x, z)`.
    #[must_use]
    pub fn points(&self) -> &[[f64; 2]] {
        &self.points
    }

    /// Ground segments (length `points().len() − 1`).
    #[must_use]
    pub fn segments(&self) -> &[GroundSegment] {
        &self.segments
    }

    /// Absolute source and receiver positions from local heights, encoding the
    /// **hSv / hRv convention** (AV 1106/07 §5.3.1): the source height `h_s` is
    /// measured above the **FIRST** profile point, the receiver height `h_r`
    /// above the **LAST**. Returns `(source_xz, receiver_xz)`.
    ///
    /// This is the single place the off-by-metres trap lives (01-RESEARCH
    /// Pitfall 5): a naive "height above z = 0" or "receiver above the first
    /// point" is wrong. Heights are expected non-negative (`debug_assert`ed).
    #[must_use]
    pub fn endpoints(&self, h_s: f64, h_r: f64) -> ([f64; 2], [f64; 2]) {
        debug_assert!(h_s >= 0.0, "source height must be non-negative: {h_s}");
        debug_assert!(h_r >= 0.0, "receiver height must be non-negative: {h_r}");
        // Invariant: `new` rejects an empty profile, so `first`/`last` exist.
        let first = self.points[0];
        let last = self.points[self.points.len() - 1];
        let source = [first[0], first[1] + h_s];
        let receiver = [last[0], last[1] + h_r];
        (source, receiver)
    }
}

/// The canonical semantic 2.5D scene the whole engine consumes.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    /// CRS convention tag (descriptive in Phase 1).
    pub crs: CrsInfo,
    /// Sound sources.
    pub sources: Vec<Source>,
    /// Receivers.
    pub receivers: Vec<Receiver>,
    /// Noise barriers / screens.
    pub barriers: Vec<Barrier>,
    /// Buildings (2.5D footprints).
    pub buildings: Vec<Building>,
    /// Terrain profiles (one per source→receiver cut plane).
    pub terrain: Vec<TerrainProfile>,
}

/// Nordtest ground-impedance class → flow resistivity σ (kNs·m⁻⁴).
///
/// Classes A..H map to 12.5 / 31.5 / 80 / 200 / 500 / 2000 / 20000 / 200000.
///
/// # Provenance
///
/// **All eight classes VERIFIED** against AV 1106/07 Table 2 (02-RESEARCH §2,
/// this phase — resolves Phase 1 Assumption A1). Class **B is 31.5**, not the
/// 31.6 Phase 1 assumed; corrected here.
///
/// Returns `None` for any character outside `A..=H`.
#[must_use]
pub fn impedance_class(class: char) -> Option<f64> {
    match class {
        'A' => Some(12.5),
        'B' => Some(31.5),
        'C' => Some(80.0),
        'D' => Some(200.0),
        'E' => Some(500.0),
        'F' => Some(2000.0),
        'G' => Some(20000.0),
        'H' => Some(200000.0),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn seg(sigma: f64) -> GroundSegment {
        GroundSegment {
            flow_resistivity: sigma,
            roughness: 0.0,
        }
    }

    #[test]
    fn terrain_profile_rejects_malformed_input_with_typed_errors() {
        // empty profile
        assert_eq!(
            TerrainProfile::new(Vec::new(), Vec::new()).unwrap_err(),
            SceneError::EmptyProfile
        );

        // segment count mismatch: 2 points require exactly 1 segment
        let err = TerrainProfile::new(vec![[0.0, 0.0], [10.0, 0.0]], vec![seg(12.5), seg(12.5)])
            .unwrap_err();
        assert!(
            matches!(err, SceneError::SegmentCountMismatch { .. }),
            "got {err:?}"
        );

        // non-ascending X
        let err = TerrainProfile::new(vec![[10.0, 0.0], [5.0, 0.0]], vec![seg(12.5)]).unwrap_err();
        assert!(
            matches!(err, SceneError::NonAscendingX { .. }),
            "got {err:?}"
        );

        // duplicate X (not strictly ascending)
        let err = TerrainProfile::new(vec![[3.25, 0.0], [3.25, 0.0]], vec![seg(12.5)]).unwrap_err();
        assert!(
            matches!(err, SceneError::NonAscendingX { .. }),
            "got {err:?}"
        );

        // non-finite z
        let err =
            TerrainProfile::new(vec![[0.0, f64::NAN], [10.0, 0.0]], vec![seg(12.5)]).unwrap_err();
        assert!(matches!(err, SceneError::NonFinite { .. }), "got {err:?}");

        // a valid profile constructs
        let ok = TerrainProfile::new(vec![[0.0, 10.0], [100.0, 20.0]], vec![seg(12.5)]);
        assert!(ok.is_ok(), "valid profile must construct: {ok:?}");
    }

    #[test]
    fn endpoints_encode_hsv_hrv_above_first_and_last_point() {
        // Profile from (0, 10) to (100, 20).
        let profile =
            TerrainProfile::new(vec![[0.0, 10.0], [100.0, 20.0]], vec![seg(12.5)]).unwrap();
        let (s, r) = profile.endpoints(0.5, 1.5);
        // Source is above the FIRST point: z = 10 + 0.5 = 10.5.
        assert_relative_eq!(s[0], 0.0, epsilon = 1e-12);
        assert_relative_eq!(s[1], 10.5, epsilon = 1e-12);
        // Receiver is above the LAST point: z = 20 + 1.5 = 21.5.
        assert_relative_eq!(r[0], 100.0, epsilon = 1e-12);
        assert_relative_eq!(r[1], 21.5, epsilon = 1e-12);
    }

    #[test]
    fn impedance_classes_resolve_with_verified_and_assumed_values() {
        // Verified this research cycle:
        assert_eq!(impedance_class('A'), Some(12.5));
        assert_eq!(impedance_class('D'), Some(200.0));
        assert_eq!(impedance_class('G'), Some(20000.0));
        // Table 2 VERIFIED this phase (02-RESEARCH §2 — resolves A1):
        assert_eq!(impedance_class('B'), Some(31.5));
        assert_eq!(impedance_class('C'), Some(80.0));
        assert_eq!(impedance_class('E'), Some(500.0));
        assert_eq!(impedance_class('F'), Some(2000.0));
        assert_eq!(impedance_class('H'), Some(200000.0));
        // Out of range:
        assert_eq!(impedance_class('Z'), None);
    }

    #[test]
    fn band_spectrum_is_grid_length_and_addressable() {
        let s = BandSpectrum::uniform(0.0);
        assert_eq!(s.as_slice().len(), N_BANDS);
        assert!(s.as_slice().iter().all(|&v| v == 0.0));
        let mut vals = [0.0; N_BANDS];
        vals[3] = 90.0;
        let s2 = BandSpectrum::from_values(vals);
        assert_relative_eq!(s2.as_slice()[3], 90.0);
    }

    #[test]
    fn barrier_lists_its_screen_edges() {
        let b = Barrier {
            top_edge: vec![[0.0, 0.0, 3.0], [10.0, 0.0, 3.0], [10.0, 5.0, 3.0]],
            thickness_m: None,
        };
        assert_eq!(b.edges().len(), 2);
        // single-vertex degenerate barrier has no edges
        let thin = Barrier {
            top_edge: vec![[0.0, 0.0, 3.0]],
            thickness_m: Some(0.5),
        };
        assert!(thin.edges().is_empty());
    }
}
