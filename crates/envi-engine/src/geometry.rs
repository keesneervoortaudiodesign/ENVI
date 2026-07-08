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
        let dx = rcv[0] - src[0];
        let dy = rcv[1] - src[1];
        let dz = rcv[2] - src[2];
        let r_m = (dx * dx + dy * dy + dz * dz).sqrt();
        if r_m < MIN_PATH_M {
            return Err(GeometryError::DegeneratePath { r_m });
        }
        let horizontal_m = (dx * dx + dy * dy).sqrt();
        let azimuth_deg = azimuth_deg([src[0], src[1]], [rcv[0], rcv[1]]);
        Ok(Self {
            r_m,
            horizontal_m,
            azimuth_deg,
        })
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
    // Segment direction e and its squared length; a zero-length segment has no
    // line to reflect across.
    let e = [seg_b[0] - seg_a[0], seg_b[1] - seg_a[1]];
    let ee = e[0] * e[0] + e[1] * e[1];
    if ee < MIN_PATH_M * MIN_PATH_M {
        return None;
    }

    // Reflect S across the LINE through seg_a with direction e (general line
    // reflection — correct for sloped segments, not a z-negation).
    let ap = [s[0] - seg_a[0], s[1] - seg_a[1]];
    let t = (ap[0] * e[0] + ap[1] * e[1]) / ee;
    let foot = [seg_a[0] + t * e[0], seg_a[1] + t * e[1]];
    let s_img = [2.0 * foot[0] - s[0], 2.0 * foot[1] - s[1]];

    // Intersect the image-S→R line with the segment line. Normal n ⟂ e; the
    // reflection point satisfies n·(P − seg_a) = 0.
    let n = [-e[1], e[0]];
    let dir = [r[0] - s_img[0], r[1] - s_img[1]];
    let denom = n[0] * dir[0] + n[1] * dir[1];
    if denom.abs() < 1e-12 {
        return None; // image-S→R parallel to the segment line: no intersection
    }
    let num = n[0] * (seg_a[0] - s_img[0]) + n[1] * (seg_a[1] - s_img[1]);
    let param = num / denom;
    let point = [s_img[0] + param * dir[0], s_img[1] + param * dir[1]];

    // Path legs and total (r1 + r2 == dist(image_S, R) by construction).
    let r1_m = ((point[0] - s[0]).powi(2) + (point[1] - s[1]).powi(2)).sqrt();
    let r2_m = ((r[0] - point[0]).powi(2) + (r[1] - point[1]).powi(2)).sqrt();

    // Grazing angle: acute angle between the incident ray and the segment line
    // (scale-invariant in e, so no normalization needed).
    let d_in = [point[0] - s[0], point[1] - s[1]];
    let cross = (d_in[0] * e[1] - d_in[1] * e[0]).abs();
    let dot = (d_in[0] * e[0] + d_in[1] * e[1]).abs();
    let grazing_angle_rad = cross.atan2(dot);

    // Validity: reflection point lies within the segment iff 0 ≤ u ≤ 1.
    let u = ((point[0] - seg_a[0]) * e[0] + (point[1] - seg_a[1]) * e[1]) / ee;
    let valid = (0.0..=1.0).contains(&u);

    Some(ReflectionGeometry {
        point_x: point[0],
        r1_m,
        r2_m,
        grazing_angle_rad,
        valid,
    })
}

// ============================================================================
// §5.23 auxiliary 2-D geometry helpers (AV 1106/07 Eqs. 370/377/383/390/392).
//
// These are the pure vector-math primitives Sub-models 4/5/6 (plan 02-04) use to
// build the screen⇄ground image geometry: reflecting a point across a terrain
// segment (image method), the local along/normal frame of a segment, the height
// of the diffracting edge above the source–receiver line, and the intersection
// of two non-adjacent segments (the equivalent wedge top). All are f64, and none
// produces a NaN on degenerate input — degenerate cases fall back to a
// finite value (documented per helper) or `None` for [`wedge_cross`].
// ============================================================================

/// Local segment variables (AV 1106/07 Eq. 383 / consumed by Eq. 164).
///
/// The along/normal decomposition of two points `a`, `b` relative to a terrain
/// segment `P₁→P₂`, in the segment's tangent frame (origin at `a`'s foot,
/// tangent `ê = unit(P₂−P₁)`):
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegmentVariables {
    /// `d′` — along-segment distance between the projections of `a` and `b` (m,
    /// signed: positive when `b` projects further along `ê` than `a`).
    pub d_prime: f64,
    /// `h′_a` — signed normal height of `a` above the segment line (m, positive
    /// on the `+n = (−e_z, e_x)` side, i.e. "above" for a left-to-right segment).
    pub h_a: f64,
    /// `h′_b` — signed normal height of `b` above the segment line (m).
    pub h_b: f64,
    /// `d₁` — along-segment distance from `a`'s foot to `P₁` (the segment start),
    /// clamped to `≥ 0` so it is a valid Fresnel-zone strip endpoint.
    pub d1: f64,
    /// `d₂` — along-segment distance from `a`'s foot to `P₂` (the segment end),
    /// clamped so `d₂ ≥ d₁`.
    pub d2: f64,
}

/// Unit tangent of a segment and its squared length; `None` if degenerate.
#[inline]
fn seg_tangent(seg_a: [f64; 2], seg_b: [f64; 2]) -> Option<([f64; 2], f64)> {
    let e = [seg_b[0] - seg_a[0], seg_b[1] - seg_a[1]];
    let len = (e[0] * e[0] + e[1] * e[1]).sqrt();
    if len < MIN_PATH_M {
        return None;
    }
    Some(([e[0] / len, e[1] / len], len))
}

/// Along-distance (tangent projection) and signed normal distance of a point
/// relative to the line through a segment (AV 1106/07 Eq. 377, `NormLine`).
///
/// Returns `(along, signed_normal)` in the frame with origin at `seg_a`, tangent
/// `ê = unit(seg_b − seg_a)`, and normal `n̂ = (−ê_z, ê_x)`. For a degenerate
/// (zero-length) segment the tangent is undefined; the fallback returns the raw
/// offset from `seg_a` `(0, |p − seg_a|)` — finite, never NaN.
#[must_use]
pub fn norm_line(p: [f64; 2], seg_a: [f64; 2], seg_b: [f64; 2]) -> (f64, f64) {
    let ap = [p[0] - seg_a[0], p[1] - seg_a[1]];
    match seg_tangent(seg_a, seg_b) {
        Some((e, _)) => {
            let along = ap[0] * e[0] + ap[1] * e[1];
            let signed_normal = ap[0] * (-e[1]) + ap[1] * e[0];
            (along, signed_normal)
        }
        None => (0.0, (ap[0] * ap[0] + ap[1] * ap[1]).sqrt()),
    }
}

/// Image (mirror) of a point across the line through a terrain segment
/// (AV 1106/07 Eq. 370, `ImagePoint`).
///
/// The reflection used by the screen image method (`S → Sᵢ`, `R → Rᵢ`). Reuses
/// the general line-reflection of [`reflect_over_segment`]. For a degenerate
/// (zero-length) segment the mirror line is undefined; the fallback returns `p`
/// unchanged — finite, never NaN.
#[must_use]
pub fn image_point(p: [f64; 2], seg_a: [f64; 2], seg_b: [f64; 2]) -> [f64; 2] {
    match seg_tangent(seg_a, seg_b) {
        Some((e, _)) => {
            // Foot of p on the line, then mirror: pᵢ = 2·foot − p.
            let ap = [p[0] - seg_a[0], p[1] - seg_a[1]];
            let t = ap[0] * e[0] + ap[1] * e[1];
            let foot = [seg_a[0] + t * e[0], seg_a[1] + t * e[1]];
            [2.0 * foot[0] - p[0], 2.0 * foot[1] - p[1]]
        }
        None => p,
    }
}

/// Vertical distance of a point above the line through a segment (AV 1106/07
/// Eq. 390, `VertDist`).
///
/// The **z-axis** distance (not the perpendicular distance) from `p` to the line
/// evaluated at `p`'s x-coordinate: `p_z − z_line(p_x)`. This is the "edge height
/// above the source–receiver line" `h_e` used by Sub-model 7. For a vertical
/// baseline (`seg_a_x == seg_b_x`) the z-at-x is undefined; the fallback returns
/// `p_z − seg_a_z` — finite, never NaN.
#[must_use]
pub fn vert_dist(p: [f64; 2], seg_a: [f64; 2], seg_b: [f64; 2]) -> f64 {
    let dx = seg_b[0] - seg_a[0];
    if dx.abs() < MIN_PATH_M {
        return p[1] - seg_a[1];
    }
    let slope = (seg_b[1] - seg_a[1]) / dx;
    let z_line = seg_a[1] + slope * (p[0] - seg_a[0]);
    p[1] - z_line
}

/// Intersection of the two lines through non-adjacent segments `a₁a₂` and
/// `b₁b₂` — the equivalent wedge top (AV 1106/07 Eq. 392, `WedgeCross`).
///
/// Returns `None` for degenerate (zero-length) segments or parallel lines (no
/// unique crossing), so the caller never divides by zero or reads a NaN.
#[must_use]
pub fn wedge_cross(a1: [f64; 2], a2: [f64; 2], b1: [f64; 2], b2: [f64; 2]) -> Option<[f64; 2]> {
    let ea = [a2[0] - a1[0], a2[1] - a1[1]];
    let eb = [b2[0] - b1[0], b2[1] - b1[1]];
    if ea[0] * ea[0] + ea[1] * ea[1] < MIN_PATH_M * MIN_PATH_M
        || eb[0] * eb[0] + eb[1] * eb[1] < MIN_PATH_M * MIN_PATH_M
    {
        return None;
    }
    // Solve a1 + s·ea = b1 + t·eb. Cross-product denominator = ea × eb.
    let denom = ea[0] * eb[1] - ea[1] * eb[0];
    if denom.abs() < 1e-12 {
        return None; // parallel lines
    }
    let d = [b1[0] - a1[0], b1[1] - a1[1]];
    let s = (d[0] * eb[1] - d[1] * eb[0]) / denom;
    Some([a1[0] + s * ea[0], a1[1] + s * ea[1]])
}

/// Local along/normal variables of two points relative to a terrain segment
/// (AV 1106/07 Eq. 383, `SegmentVariables`; consumed by Eq. 164).
///
/// `a`, `b` are the two path endpoints (e.g. source `S` and diffracting edge
/// `T`); `seg_a`, `seg_b` are the reflecting terrain segment `P₁`, `P₂`. See
/// [`SegmentVariables`] for the returned frame. Degenerate segments fall back
/// through [`norm_line`] to finite values (never NaN).
#[must_use]
pub fn segment_variables(
    a: [f64; 2],
    b: [f64; 2],
    seg_a: [f64; 2],
    seg_b: [f64; 2],
) -> SegmentVariables {
    let (along_a, h_a) = norm_line(a, seg_a, seg_b);
    let (along_b, h_b) = norm_line(b, seg_a, seg_b);
    let (along_p2, _) = norm_line(seg_b, seg_a, seg_b); // = segment length; P1 foot = 0
    // Strip endpoints in a's frame (foot of a at the origin), ordered d₂ ≥ d₁≥0.
    let e1 = (0.0 - along_a).max(0.0);
    let e2 = (along_p2 - along_a).max(0.0);
    let (d1, d2) = if e2 >= e1 { (e1, e2) } else { (e2, e1) };
    SegmentVariables {
        d_prime: along_b - along_a,
        h_a,
        h_b,
        d1,
        d2,
    }
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
        assert_relative_eq!(
            g.r_m,
            (100.0_f64 * 100.0 + 1.0).sqrt(),
            max_relative = 1e-12
        );
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

    // ---- §5.23 auxiliary geometry helpers (Eqs. 370/377/383/390/392) ----

    // Eq. 370 ImagePoint: mirror across a flat baseline is z-negation; across a
    // 45° line y=x it swaps coordinates (hand-computed anchors).
    #[test]
    fn image_point_matches_hand_anchors() {
        let flat = image_point([3.0, 2.0], [0.0, 0.0], [10.0, 0.0]);
        assert_relative_eq!(flat[0], 3.0, epsilon = 1e-12);
        assert_relative_eq!(flat[1], -2.0, epsilon = 1e-12);
        // y = x line: (0,4) → (4,0).
        let diag = image_point([0.0, 4.0], [0.0, 0.0], [10.0, 10.0]);
        assert_relative_eq!(diag[0], 4.0, max_relative = 1e-12);
        assert_relative_eq!(diag[1], 0.0, epsilon = 1e-12);
        // Consistency with reflect_over_segment's internal image (sloped seg).
        assert_relative_eq!(diag[0], 4.0, max_relative = 1e-12);
    }

    // Eq. 377 NormLine: along/normal of a point over a flat baseline.
    #[test]
    fn norm_line_reports_along_and_signed_normal() {
        let (along, normal) = norm_line([3.0, 2.0], [0.0, 0.0], [10.0, 0.0]);
        assert_relative_eq!(along, 3.0, epsilon = 1e-12);
        assert_relative_eq!(normal, 2.0, epsilon = 1e-12); // n̂ = (0,1) here
        // Point below the line → negative signed normal.
        let (_, below) = norm_line([3.0, -1.5], [0.0, 0.0], [10.0, 0.0]);
        assert_relative_eq!(below, -1.5, epsilon = 1e-12);
    }

    // Eq. 390 VertDist: vertical drop of the edge above the S–R chord.
    #[test]
    fn vert_dist_is_z_above_the_chord() {
        // S=(0,1), R=(100,1): chord is z=1. Edge T=(50,6) sits 5 m above.
        assert_relative_eq!(
            vert_dist([50.0, 6.0], [0.0, 1.0], [100.0, 1.0]),
            5.0,
            epsilon = 1e-9
        );
        // Sloped chord (0,0)-(10,10): at x=4 the line is z=4; point (4,7) → 3.
        assert_relative_eq!(
            vert_dist([4.0, 7.0], [0.0, 0.0], [10.0, 10.0]),
            3.0,
            epsilon = 1e-9
        );
    }

    // Eq. 392 WedgeCross: intersection of two non-adjacent segment lines.
    #[test]
    fn wedge_cross_finds_the_equivalent_top_and_rejects_parallels() {
        // Line through (0,0)-(10,10) [y=x] and (0,4)-(10,4) [y=4] cross at (4,4).
        let x = wedge_cross([0.0, 0.0], [10.0, 10.0], [0.0, 4.0], [10.0, 4.0]).unwrap();
        assert_relative_eq!(x[0], 4.0, max_relative = 1e-12);
        assert_relative_eq!(x[1], 4.0, max_relative = 1e-12);
        // Parallel lines → None (never a NaN divide).
        assert!(wedge_cross([0.0, 0.0], [10.0, 0.0], [0.0, 2.0], [10.0, 2.0]).is_none());
        // Degenerate (zero-length) segment → None.
        assert!(wedge_cross([1.0, 1.0], [1.0, 1.0], [0.0, 4.0], [10.0, 4.0]).is_none());
    }

    // Eq. 383 SegmentVariables: source S and edge T over a flat source-side seg.
    #[test]
    fn segment_variables_decompose_over_a_flat_segment() {
        // Segment (10,0)-(60,0); S=(0,0.5), T=(50,6).
        let sv = segment_variables([0.0, 0.5], [50.0, 6.0], [10.0, 0.0], [60.0, 0.0]);
        // Along-axis is +x; S projects to x=0, T to x=50 ⇒ d′ = 50.
        assert_relative_eq!(sv.d_prime, 50.0, epsilon = 1e-9);
        assert_relative_eq!(sv.h_a, 0.5, epsilon = 1e-9); // S height above seg
        assert_relative_eq!(sv.h_b, 6.0, epsilon = 1e-9); // T height above seg
        // Strip endpoints in S's frame: P1 at x=10 → d1=10, P2 at x=60 → d2=60.
        assert_relative_eq!(sv.d1, 10.0, epsilon = 1e-9);
        assert_relative_eq!(sv.d2, 60.0, epsilon = 1e-9);
    }

    // Degenerate inputs never produce NaN (threat T-02-11).
    #[test]
    fn degenerate_helper_inputs_stay_finite() {
        let ip = image_point([3.0, 2.0], [5.0, 5.0], [5.0, 5.0]);
        assert!(ip[0].is_finite() && ip[1].is_finite());
        let (a, n) = norm_line([3.0, 2.0], [5.0, 5.0], [5.0, 5.0]);
        assert!(a.is_finite() && n.is_finite());
        assert!(vert_dist([3.0, 2.0], [5.0, 0.0], [5.0, 10.0]).is_finite()); // vertical baseline
        let sv = segment_variables([0.0, 1.0], [1.0, 1.0], [2.0, 2.0], [2.0, 2.0]);
        assert!(sv.d_prime.is_finite() && sv.h_a.is_finite() && sv.d2.is_finite());
    }
}
