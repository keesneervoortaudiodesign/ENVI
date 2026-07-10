//! Constrained-Delaunay TIN builder + barycentric Z interpolation.
//!
//! # Module I/O
//! - **Inputs:** scattered `elevation_point` vertices `[x, y, z]` (meters) and
//!   `elevation_line` breaklines as polylines of `[x, y]` vertices (meters). All
//!   coordinates arrive from untrusted HTTP bodies (via `envi-service`).
//! - **Output:** a queryable [`Tin`] — a constrained-Delaunay triangulation whose
//!   breakline segments are forced triangle edges — plus [`Tin::interpolate_z`],
//!   barycentric Z lookup at any `(x, y)`.
//! - **Invariant (load-bearing):** this module NEVER panics on data. `spade`'s
//!   `add_constraint_edges` panics on an interior-intersecting breakline
//!   (07-RESEARCH Pitfall 3); [`build_tin`] pre-checks every segment with
//!   `can_add_constraint` and returns [`DgmError::IntersectingConstraint`]
//!   instead. Degenerate, non-finite, and oversized input are likewise rejected
//!   with typed errors. No `unwrap()` / `panic!` on any data path.
//!
//! # Breakline Z
//! Breaklines are 2-D (`[x, y]`, no Z). Each breakline vertex takes its Z by
//! barycentric interpolation over the elevation-point surface at insertion time;
//! a breakline vertex outside the point hull falls back to the nearest known
//! vertex's Z (never a silent `0.0`).

use crate::DgmError;
use spade::{
    ConstrainedDelaunayTriangulation, FloatTriangulation, HasPosition, Point2, Triangulation,
};

/// Maximum elevation points accepted per build (DoS bound, threat T-07-02-02).
/// Far above any hand-drawn scene; the guard rejects pathological payloads
/// before the `O(n log n)` triangulation runs.
pub const MAX_POINTS: usize = 500_000;

/// Maximum total breakline vertices accepted per build (DoS bound,
/// threat T-07-02-02).
pub const MAX_BREAKLINE_VERTICES: usize = 500_000;

/// A single elevation vertex: planar position plus its Z (meters). Implements
/// `spade::HasPosition` so it can be a triangulation vertex while carrying the Z
/// payload that [`Tin::interpolate_z`] reads back out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElevVertex {
    /// Planar position `(x, y)` in scene meters.
    pub pos: Point2<f64>,
    /// Elevation at `pos`, meters.
    pub z: f64,
}

impl HasPosition for ElevVertex {
    type Scalar = f64;
    fn position(&self) -> Point2<f64> {
        self.pos
    }
}

/// A built terrain surface: a constrained-Delaunay triangulation ready for Z
/// queries. Construct via [`build_tin`]; query via [`Tin::interpolate_z`].
pub struct Tin {
    cdt: ConstrainedDelaunayTriangulation<ElevVertex>,
}

impl std::fmt::Debug for Tin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tin")
            .field("vertices", &self.cdt.num_vertices())
            .field("triangles", &self.cdt.num_inner_faces())
            .finish()
    }
}

impl Tin {
    /// Interpolate the surface elevation at `(x, y)` by barycentric interpolation
    /// over the containing triangle.
    ///
    /// Returns `None` for a query outside the convex hull (no containing face) or
    /// for a non-finite coordinate — never a silent `0.0`.
    pub fn interpolate_z(&self, x: f64, y: f64) -> Option<f64> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        self.cdt
            .barycentric()
            .interpolate(|v| v.data().z, Point2::new(x, y))
    }

    /// Number of triangles in the surface (0 means degenerate input slipped
    /// through — [`build_tin`] guarantees `>= 1`). Exposed for tests/inspection.
    pub fn num_triangles(&self) -> usize {
        self.cdt.num_inner_faces()
    }

    /// Number of distinct vertices in the surface.
    pub fn num_vertices(&self) -> usize {
        self.cdt.num_vertices()
    }

    /// The distinct surface vertices as `[x, y, z]` triples, in `spade`
    /// vertex-index order — so an index returned by [`Tin::triangles`] refers
    /// directly into this slice. Breakline vertices (inserted with a Z sampled
    /// from the point surface) are included alongside the elevation points.
    pub fn vertices(&self) -> Vec<[f64; 3]> {
        let mut out = vec![[0.0_f64; 3]; self.cdt.num_vertices()];
        for v in self.cdt.vertices() {
            let p = v.position();
            out[v.index()] = [p.x, p.y, v.data().z];
        }
        out
    }

    /// The inner triangles as vertex-index triples into [`Tin::vertices`]
    /// (outer/unbounded face excluded). Order is unspecified but every index is
    /// `< num_vertices()`; a renderable, self-contained mesh with the vertex list.
    pub fn triangles(&self) -> Vec<[usize; 3]> {
        self.cdt
            .inner_faces()
            .map(|face| {
                let [a, b, c] = face.vertices();
                [a.index(), b.index(), c.index()]
            })
            .collect()
    }
}

/// Barycentric Z at `pos` over the current triangulation, falling back to the
/// nearest vertex's Z when `pos` is outside the convex hull. Returns `Err` only
/// in the (unreachable-after-guard) empty-triangulation case — never a silent
/// `0.0`, never a panic.
fn breakline_z(
    cdt: &ConstrainedDelaunayTriangulation<ElevVertex>,
    pos: Point2<f64>,
) -> Result<f64, DgmError> {
    if let Some(z) = cdt.barycentric().interpolate(|v| v.data().z, pos) {
        return Ok(z);
    }
    cdt.vertices()
        .map(|v| {
            let p = v.position();
            let d2 = (p.x - pos.x).powi(2) + (p.y - pos.y).powi(2);
            (d2, v.data().z)
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, z)| z)
        .ok_or_else(|| DgmError::Triangulation {
            message: "empty triangulation while placing breakline vertex".to_string(),
        })
}

/// Build a constrained-Delaunay TIN from elevation points and breaklines.
///
/// # Errors
/// - [`DgmError::TooLarge`] — point or breakline-vertex count exceeds the DoS cap.
/// - [`DgmError::NonFinite`] — any coordinate (point or breakline) is NaN/inf.
/// - [`DgmError::TooFewPoints`] — fewer than 3 distinct non-collinear points
///   (no triangle can form). Covers 0/1/2 points and all-collinear sets.
/// - [`DgmError::IntersectingConstraint`] — a breakline segment crosses another
///   constraint in its interior. Returned via a pre-check, so `spade` never
///   panics (07-RESEARCH Pitfall 3).
/// - [`DgmError::Triangulation`] — a residual `spade` insertion failure.
pub fn build_tin(points: &[[f64; 3]], breaklines: &[Vec<[f64; 2]>]) -> Result<Tin, DgmError> {
    // --- DoS caps (threat T-07-02-02): reject before any O(n log n) work. ---
    if points.len() > MAX_POINTS {
        return Err(DgmError::TooLarge {
            kind: "points",
            got: points.len(),
            limit: MAX_POINTS,
        });
    }
    let breakline_vertices: usize = breaklines.iter().map(Vec::len).sum();
    if breakline_vertices > MAX_BREAKLINE_VERTICES {
        return Err(DgmError::TooLarge {
            kind: "breakline vertices",
            got: breakline_vertices,
            limit: MAX_BREAKLINE_VERTICES,
        });
    }

    // --- Finiteness (threat T-07-02-03): reject NaN/inf before insert. ---
    for p in points {
        if !p.iter().all(|c| c.is_finite()) {
            return Err(DgmError::NonFinite {
                what: format!("point {p:?}"),
            });
        }
    }
    for line in breaklines {
        for v in line {
            if !v.iter().all(|c| c.is_finite()) {
                return Err(DgmError::NonFinite {
                    what: format!("breakline vertex {v:?}"),
                });
            }
        }
    }

    // --- Insert elevation points; spade dedups exact duplicates. ---
    let mut cdt = ConstrainedDelaunayTriangulation::<ElevVertex>::new();
    for p in points {
        cdt.insert(ElevVertex {
            pos: Point2::new(p[0], p[1]),
            z: p[2],
        })
        .map_err(|e| DgmError::Triangulation {
            message: e.to_string(),
        })?;
    }

    // --- Degeneracy: need >= 3 non-collinear points => at least one triangle. ---
    if cdt.num_inner_faces() == 0 {
        return Err(DgmError::TooFewPoints {
            got: cdt.num_vertices(),
        });
    }

    // --- Breaklines: insert vertices (Z from the surface), then add each
    //     segment as a constraint ONLY after can_add_constraint passes. This is
    //     the panic guard — add_constraint is never reached for an interior
    //     crossing (07-RESEARCH Pitfall 3). ---
    for line in breaklines {
        let mut handles = Vec::with_capacity(line.len());
        for v in line {
            let pos = Point2::new(v[0], v[1]);
            let z = breakline_z(&cdt, pos)?;
            let handle =
                cdt.insert(ElevVertex { pos, z })
                    .map_err(|e| DgmError::Triangulation {
                        message: e.to_string(),
                    })?;
            handles.push((handle, *v));
        }
        for window in handles.windows(2) {
            let (from, a) = window[0];
            let (to, b) = window[1];
            if from == to {
                continue; // duplicate consecutive vertex — no segment to add
            }
            if !cdt.can_add_constraint(from, to) {
                return Err(DgmError::IntersectingConstraint { a, b });
            }
            cdt.add_constraint(from, to);
        }
    }

    Ok(Tin { cdt })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Square with Z == y at every corner, so the surface is the plane z = y and
    /// the centre (5, 5) must interpolate to exactly 5.0 regardless of the
    /// triangulation's diagonal choice.
    fn unit_square_z_eq_y() -> Vec<[f64; 3]> {
        vec![
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 10.0],
            [0.0, 10.0, 10.0],
        ]
    }

    #[test]
    fn builds_and_interpolates_interior_point() {
        let tin = build_tin(&unit_square_z_eq_y(), &[]).expect("valid square builds");
        assert!(tin.num_triangles() >= 1);
        let z = tin
            .interpolate_z(5.0, 5.0)
            .expect("centre is inside the hull");
        assert!((z - 5.0).abs() < 1e-9, "centre Z = {z}, expected 5.0");
    }

    #[test]
    fn interior_z_lies_between_surrounding_vertices() {
        let tin = build_tin(&unit_square_z_eq_y(), &[]).unwrap();
        let z = tin.interpolate_z(5.0, 2.5).expect("inside hull");
        // Plane z = y => exactly 2.5; assert it is within the vertex Z range.
        assert!((0.0..=10.0).contains(&z));
        assert!((z - 2.5).abs() < 1e-9, "z = {z}, expected 2.5");
    }

    #[test]
    fn outside_hull_returns_none_not_zero() {
        let tin = build_tin(&unit_square_z_eq_y(), &[]).unwrap();
        assert_eq!(tin.interpolate_z(100.0, 100.0), None);
        // The load-bearing property: outside the hull is None, never a silent 0.0.
        assert_ne!(tin.interpolate_z(-50.0, -50.0), Some(0.0));
    }

    #[test]
    fn two_points_are_rejected() {
        let pts = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
        assert!(matches!(
            build_tin(&pts, &[]),
            Err(DgmError::TooFewPoints { got: 2 })
        ));
    }

    #[test]
    fn zero_and_one_point_are_rejected() {
        assert!(matches!(
            build_tin(&[], &[]),
            Err(DgmError::TooFewPoints { .. })
        ));
        assert!(matches!(
            build_tin(&[[0.0, 0.0, 5.0]], &[]),
            Err(DgmError::TooFewPoints { got: 1 })
        ));
    }

    #[test]
    fn all_collinear_points_are_rejected() {
        // Three+ distinct points on the line y = x -> no triangle.
        let pts = vec![
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [2.0, 2.0, 0.0],
            [3.0, 3.0, 0.0],
        ];
        match build_tin(&pts, &[]) {
            Err(DgmError::TooFewPoints { got }) => assert!(got >= 3),
            other => panic!("expected TooFewPoints for collinear input, got {other:?}"),
        }
    }

    #[test]
    fn non_finite_coordinate_is_rejected() {
        let pts = vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [f64::NAN, 10.0, 10.0]];
        assert!(matches!(
            build_tin(&pts, &[]),
            Err(DgmError::NonFinite { .. })
        ));
        // Infinity is caught too.
        let pts_inf = vec![
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [0.0, 10.0, f64::INFINITY],
        ];
        assert!(matches!(
            build_tin(&pts_inf, &[]),
            Err(DgmError::NonFinite { .. })
        ));
    }

    #[test]
    fn interior_crossing_breaklines_are_rejected_without_panic() {
        // Square corners; two diagonals cross at (5, 5) in their interior.
        let pts = unit_square_z_eq_y();
        let breaklines = vec![
            vec![[0.0, 0.0], [10.0, 10.0]], // diagonal 1
            vec![[10.0, 0.0], [0.0, 10.0]], // diagonal 2 — crosses diagonal 1
        ];
        let result = build_tin(&pts, &breaklines);
        // Reaching this assert at all proves the process did NOT abort/panic.
        assert!(
            matches!(result, Err(DgmError::IntersectingConstraint { .. })),
            "expected IntersectingConstraint, got {result:?}"
        );
    }

    #[test]
    fn non_crossing_breakline_builds() {
        // A single boundary-parallel breakline that crosses nothing.
        let pts = unit_square_z_eq_y();
        let breaklines = vec![vec![[2.0, 0.0], [2.0, 10.0]]];
        let tin = build_tin(&pts, &breaklines).expect("non-crossing breakline builds");
        assert!(tin.num_triangles() >= 1);
        // Interpolation still works with the constraint present.
        let z = tin.interpolate_z(5.0, 5.0).expect("inside hull");
        assert!((z - 5.0).abs() < 1e-9, "z = {z}, expected 5.0");
    }

    #[test]
    fn self_intersecting_breakline_is_rejected() {
        // A single polyline whose later segment crosses an earlier one.
        let pts = unit_square_z_eq_y();
        let breaklines = vec![vec![[0.0, 0.0], [10.0, 10.0], [10.0, 0.0], [0.0, 10.0]]];
        assert!(matches!(
            build_tin(&pts, &breaklines),
            Err(DgmError::IntersectingConstraint { .. })
        ));
    }

    #[test]
    fn oversized_point_set_is_rejected() {
        // Length-only check: a zero-filled Vec above the cap trips TooLarge
        // before any triangulation work.
        let pts = vec![[0.0, 0.0, 0.0]; MAX_POINTS + 1];
        assert!(matches!(
            build_tin(&pts, &[]),
            Err(DgmError::TooLarge { kind: "points", .. })
        ));
    }
}
