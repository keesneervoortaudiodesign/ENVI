//! Pure path geometry (GEO-03): azimuth and image-source reflection.
//!
//! This module is **pure 2-D / 3-D vector math** over scene positions — no
//! physics, no I/O (01-RESEARCH "Architectural Responsibility Map"). It exposes
//! the geometric primitives later phases consume:
//!
//! - [`azimuth_deg`] — source→receiver bearing, clockwise from north.
//! - [`PathGeometry::direct`] — straight-line range `R`, horizontal distance,
//!   azimuth. Propagation derives travel time `τ = R/c`; geometry only exposes
//!   `R` (01-RESEARCH "Verified Physics" §6).
//! - [`reflect_over_segment`] — image-source reflection in the vertical cut
//!   plane: reflection point, path legs `r1 + r2`, grazing angle `ψ_G` (the
//!   Phase 2 spherical-wave reflection-coefficient consumer).
//!
//! For vertical-obstacle reflectors AV 1106/07 unfolds the path S→O→R; Phase 1
//! only needs these primitives (image point, `r1 + r2`, incidence angle).

use thiserror::Error;

/// Errors from path-geometry construction.
#[derive(Debug, Error, PartialEq)]
pub enum GeometryError {
    /// Source and receiver are coincident (or sub-nanometre apart): the path
    /// range `R → 0`. A domain error, never a clamp (01-RESEARCH §6 guard).
    #[error("degenerate path: source and receiver are coincident (R = {r_m} m)")]
    DegeneratePath {
        /// The computed (near-zero) range in meters.
        r_m: f64,
    },
}

/// Minimum physical path length; below this a path is treated as degenerate.
const MIN_PATH_M: f64 = 1e-9;

/// Azimuth from `from` to `to` in the horizontal plane, **degrees clockwise
/// from north**, normalized to `[0, 360)`.
///
/// `az = atan2(dx_east, dy_north)`. Positions are `[x_east, y_north]` in the
/// projected metric CRS. North is +y, east is +x.
///
/// # Why clockwise-from-north
///
/// This matches the FORCE wind convention `φ_u re north`. The Phase 3 wind
/// projection consumes it as `A ∝ u·cos(az − φ_u)` — the convention has a
/// stated downstream reason (GEO-03 forward-compat), not an arbitrary choice.
///
/// # Examples (hand-computed anchors)
///
/// - `(0,0) → (0,100)` = `0.0` (due north)
/// - `(0,0) → (100,0)` = `90.0` (east)
/// - `(0,0) → (100,100)` = `45.0`
/// - `(0,0) → (−100,0)` = `270.0`
#[must_use]
pub fn azimuth_deg(from: [f64; 2], to: [f64; 2]) -> f64 {
    let dx_east = to[0] - from[0];
    let dy_north = to[1] - from[1];
    let deg = dx_east.atan2(dy_north).to_degrees();
    deg.rem_euclid(360.0)
}

/// Straight-line path geometry between a source and a receiver.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathGeometry {
    /// 3-D straight-line range `R`, meters.
    pub r_m: f64,
    /// Horizontal (XY-plane) distance, meters.
    pub horizontal_m: f64,
    /// Source→receiver azimuth, degrees clockwise from north.
    pub azimuth_deg: f64,
}

impl PathGeometry {
    /// Direct-path geometry for a 3-D source and receiver `[x, y, z]`.
    ///
    /// # Errors
    ///
    /// [`GeometryError::DegeneratePath`] if the range `R` is below
    /// [`MIN_PATH_M`] (source and receiver coincident) — a domain error, not a
    /// clamp.
    pub fn direct(src: [f64; 3], rcv: [f64; 3]) -> Result<Self, GeometryError> {
        todo!("GREEN")
    }
}

/// Image-source reflection geometry in a vertical cut plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReflectionGeometry {
    /// X-coordinate of the reflection point on the segment line.
    pub point_x: f64,
    /// Length of the incident leg S→reflection-point, meters.
    pub r1_m: f64,
    /// Length of the reflected leg reflection-point→R, meters.
    pub r2_m: f64,
    /// Grazing angle `ψ_G` between the incident ray and the segment line,
    /// radians in `[0, π/2]` (Phase 2's reflection-coefficient input).
    pub grazing_angle_rad: f64,
    /// `true` iff the reflection point lies **within** the segment (not the
    /// infinite line): `0 ≤ u ≤ 1` along `seg_a → seg_b`.
    pub valid: bool,
}

/// Image-source reflection of `s`→`r` over the (possibly sloped) ground
/// segment `seg_a`→`seg_b`, in the vertical cut plane.
///
/// Reflects `s` across the **line containing** the segment, intersects the
/// image-S→R line with that line, and reports the reflection point, path legs
/// and grazing angle. Handles a sloped segment via general line reflection
/// (not z-negation).
///
/// # Return / validity contract (chosen API — documented)
///
/// - Returns `None` when no reflection is geometrically defined: a degenerate
///   (zero-length) segment, or an image-S→R line parallel to the segment line
///   (no intersection).
/// - Returns `Some(ReflectionGeometry)` otherwise. The `valid` flag reports
///   whether the reflection point lies **within** the segment; an
///   out-of-segment intersection is flagged `valid = false` rather than
///   extrapolated or discarded, so callers may inspect the geometry (threat
///   T-01-06: reflections are flagged invalid, never silently extrapolated).
#[must_use]
pub fn reflect_over_segment(
    s: [f64; 2],
    r: [f64; 2],
    seg_a: [f64; 2],
    seg_b: [f64; 2],
) -> Option<ReflectionGeometry> {
    todo!("GREEN")
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn azimuth_is_clockwise_from_north_normalized() {
        assert_relative_eq!(azimuth_deg([0.0, 0.0], [0.0, 100.0]), 0.0, epsilon = 1e-12);
        assert_relative_eq!(azimuth_deg([0.0, 0.0], [100.0, 0.0]), 90.0, epsilon = 1e-12);
        assert_relative_eq!(
            azimuth_deg([0.0, 0.0], [100.0, 100.0]),
            45.0,
            epsilon = 1e-12
        );
        assert_relative_eq!(
            azimuth_deg([0.0, 0.0], [-100.0, 0.0]),
            270.0,
            epsilon = 1e-12
        );
    }

    #[test]
    fn direct_path_reports_range_horizontal_and_azimuth() {
        let g = PathGeometry::direct([0.0, 0.0, 0.5], [100.0, 0.0, 1.5]).unwrap();
        assert_relative_eq!(g.r_m, (100.0_f64 * 100.0 + 1.0).sqrt(), max_relative = 1e-12);
        assert_relative_eq!(g.horizontal_m, 100.0, max_relative = 1e-12);
        assert_relative_eq!(g.azimuth_deg, 90.0, epsilon = 1e-12);
    }

    #[test]
    fn flat_ground_reflection_hits_midpoint_with_hand_computed_anchors() {
        // S=(0,2), R=(10,2) over flat ground (0,0)-(10,0).
        // Image S' = (0,-2); line S'→R crosses y=0 at x=5.
        // r1 = r2 = sqrt(5² + 2²) = sqrt(29); total = sqrt(116).
        // Grazing = atan(2/5) (drop 2 over run 5).
        let g = reflect_over_segment([0.0, 2.0], [10.0, 2.0], [0.0, 0.0], [10.0, 0.0]).unwrap();
        assert_relative_eq!(g.point_x, 5.0, epsilon = 1e-12);
        assert_relative_eq!(g.r1_m + g.r2_m, 116.0_f64.sqrt(), max_relative = 1e-12);
        assert_relative_eq!(g.r1_m, 29.0_f64.sqrt(), max_relative = 1e-12);
        assert_relative_eq!(
            g.grazing_angle_rad,
            (2.0_f64 / 5.0).atan(),
            max_relative = 1e-12
        );
        assert!(g.valid);
    }

    #[test]
    fn reflection_outside_the_segment_is_flagged_invalid() {
        // Same S/R, but the segment (6,0)-(10,0) does not contain x=5.
        let g = reflect_over_segment([0.0, 2.0], [10.0, 2.0], [6.0, 0.0], [10.0, 0.0]).unwrap();
        assert_relative_eq!(g.point_x, 5.0, epsilon = 1e-12);
        assert!(!g.valid, "x=5 is outside [6,10] → invalid");
    }

    #[test]
    fn sloped_segment_reflection_matches_hand_derivation() {
        // Segment (0,0)-(10,10): the line y = x.
        // S=(0,4) reflects across y=x to image S'=(4,0) (coordinate swap).
        // R=(6,10) is on the same (y>x) side as S — a valid reflection.
        // Line S'=(4,0) → R=(6,10): (4+2s, 10s); y=x ⇒ 10s = 4+2s ⇒ s=0.5.
        //   reflection point = (5, 5), inside the segment (u=0.5).
        // r1 = dist((0,4),(5,5)) = sqrt(26); r2 = dist((5,5),(6,10)) = sqrt(26);
        //   total = r1+r2 = sqrt(104) = dist(S',R) = sqrt(2²+10²). ✓
        // Grazing: incident dir (5,1) vs segment dir (1,1) ⇒ atan2(4,6)=atan(2/3).
        let g = reflect_over_segment([0.0, 4.0], [6.0, 10.0], [0.0, 0.0], [10.0, 10.0]).unwrap();
        assert_relative_eq!(g.point_x, 5.0, max_relative = 1e-12);
        assert_relative_eq!(g.r1_m, 26.0_f64.sqrt(), max_relative = 1e-12);
        assert_relative_eq!(g.r2_m, 26.0_f64.sqrt(), max_relative = 1e-12);
        assert_relative_eq!(g.r1_m + g.r2_m, 104.0_f64.sqrt(), max_relative = 1e-12);
        assert_relative_eq!(
            g.grazing_angle_rad,
            (2.0_f64 / 3.0).atan(),
            max_relative = 1e-12
        );
        assert!(g.valid);
    }

    #[test]
    fn degenerate_segment_returns_none() {
        assert!(reflect_over_segment([0.0, 2.0], [10.0, 2.0], [5.0, 0.0], [5.0, 0.0]).is_none());
    }

    #[test]
    fn coincident_source_receiver_is_a_domain_error() {
        let err = PathGeometry::direct([1.0, 2.0, 3.0], [1.0, 2.0, 3.0]).unwrap_err();
        assert!(
            matches!(err, GeometryError::DegeneratePath { .. }),
            "got {err:?}"
        );
    }
}
