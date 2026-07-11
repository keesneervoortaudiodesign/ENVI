//! Building-aware constrained-Delaunay receiver grid (GRID-01).
//!
//! # Module I/O
//! - **Inputs:** the `calc_area` polygon (planar scene meters), the building
//!   `footprints` (planar polygons, excluded as holes), a user `spacing_m` (default
//!   10 m at the call site, D-06), explicit `discrete_points` `(x, y)`, and the DGM
//!   [`Tin`] used to sample each receiver's ground z.
//! - **Output:** the receiver positions `[x, y, z]` — a regular axis-aligned lattice
//!   at `spacing_m` clipped to `calc_area` **minus** every footprint, plus the
//!   discrete points. Each z is sampled from the TIN; a hull miss is a typed
//!   [`GisError::OutsideHull`], **never a fabricated `0.0`**. The **receiver
//!   acoustic height is NOT baked in here** — it is added at `SolveJob` assembly (the
//!   hSv/hRv trap).
//! - **Invariants (load-bearing):**
//!   1. **No receiver inside a footprint** (D-07): a lattice point is kept iff it is
//!      inside `calc_area` AND outside every footprint (geo point-in-polygon). The
//!      footprint rings + `calc_area` boundary enter a spade CDT (via
//!      [`envi_dgm::tin::build_tin`]) as constraint edges to define the valid region
//!      — reusing its `can_add_constraint` guard so intersecting/degenerate rings are
//!      a typed [`GisError::InvalidGridRegion`], never a panic (threat T-09-02-03).
//!   2. **Bounded work** (threat T-09-02-02): `spacing_m` below [`MIN_SPACING_M`] is
//!      a typed [`GisError::SpacingTooSmall`]; the bounding-box lattice count is
//!      checked against [`MAX_RECEIVERS`] **before** the set is allocated
//!      ([`GisError::ReceiverCapExceeded`]) so the browser cannot OOM.
//!   3. **Regular lattice, not CDT vertices** (09-RESEARCH Pattern 4, Open-Q3): the
//!      CDT defines the valid region; receivers sit on a predictable axis-aligned
//!      lattice (what noise maps and Phase-10 chunking expect), not irregular
//!      triangle vertices.
//!
//! # Guardrail values (`[ASSUMED]`, 09-RESEARCH A2)
//! [`MIN_SPACING_M`] and [`MAX_RECEIVERS`] are engineering DoS bounds, not spec
//! values — client-side WASM compute scales with receiver count × sub-sources, so a
//! 1 m grid over a 1 km² area is already 10⁶ receivers at the edge. Phase-10 cost
//! estimation is the UX gate; this builder must simply never OOM. They are named
//! `pub const`s the reviewer can sanity-check.

use envi_dgm::tin::Tin;
use geo::{BoundingRect, Contains, Point, Polygon, Rect};

use crate::GisError;

/// Minimum receiver-grid spacing (meters) — the DoS guardrail (`[ASSUMED]`,
/// 09-RESEARCH A2, D-06). At 1 m a 1 km² area is already ~10⁶ receivers; a finer
/// grid is rejected before any lattice is generated.
pub const MIN_SPACING_M: f64 = 1.0;

/// Hard cap on total receivers (`[ASSUMED]` DoS bound, 09-RESEARCH A2). Mirrors the
/// `envi_dgm::tin::MAX_POINTS` posture: the bounding-box lattice count is checked
/// against this **before** allocation so a pathological (huge area / min spacing)
/// request is a typed error, never an OOM.
pub const MAX_RECEIVERS: usize = 1_000_000;

/// Build the building-aware receiver grid (GRID-01).
///
/// Generates the `calc_area` bounding-box lattice at `spacing_m`, keeps each point
/// inside `calc_area` and outside every footprint (D-07), appends `discrete_points`,
/// and samples each receiver's ground z from `tin`. The footprint rings + `calc_area`
/// boundary are validated as constrained-Delaunay edges (via
/// [`envi_dgm::tin::build_tin`]) so intersecting/degenerate geometry is a typed error.
///
/// The returned z is GROUND elevation only; the receiver acoustic height is added at
/// `SolveJob` assembly (the hSv/hRv trap).
///
/// # Errors
/// - [`GisError::SpacingTooSmall`] — `spacing_m` is non-finite or `< `[`MIN_SPACING_M`].
/// - [`GisError::ReceiverCapExceeded`] — the bounding-box lattice (plus discrete
///   points) would exceed [`MAX_RECEIVERS`].
/// - [`GisError::InvalidGridRegion`] — `calc_area`/footprint rings are degenerate or
///   intersect (the `build_tin` validity guard rejected them).
/// - [`GisError::NonFinite`] — a `calc_area` bounding box could not be formed
///   (empty/non-finite polygon).
/// - [`GisError::OutsideHull`] — a kept receiver's planar point left the TIN hull.
pub fn receiver_grid(
    calc_area: &Polygon<f64>,
    footprints: &[Polygon<f64>],
    spacing_m: f64,
    discrete_points: &[[f64; 2]],
    tin: &Tin,
) -> Result<Vec<[f64; 3]>, GisError> {
    // --- Guardrail 1: spacing must be finite and >= the minimum (rejects NaN/inf
    //     and any below-min value before any lattice work). ---
    if !spacing_m.is_finite() || spacing_m < MIN_SPACING_M {
        return Err(GisError::SpacingTooSmall {
            got: spacing_m,
            min: MIN_SPACING_M,
        });
    }

    // --- calc_area bounding box (must be finite/non-empty). ---
    let rect = calc_area.bounding_rect().ok_or(GisError::NonFinite {
        what: "calc_area has no bounding rect (empty polygon)".to_string(),
    })?;
    let (min_x, min_y) = (rect.min().x, rect.min().y);
    let (max_x, max_y) = (rect.max().x, rect.max().y);
    if ![min_x, min_y, max_x, max_y].iter().all(|c| c.is_finite()) {
        return Err(GisError::NonFinite {
            what: "calc_area bounding box is non-finite".to_string(),
        });
    }

    // --- Guardrail 2: bounding-box lattice count, checked BEFORE allocation. ---
    let nx_f = ((max_x - min_x) / spacing_m).floor() + 1.0;
    let ny_f = ((max_y - min_y) / spacing_m).floor() + 1.0;
    let upper_f = nx_f * ny_f + discrete_points.len() as f64;
    if !upper_f.is_finite() || upper_f > MAX_RECEIVERS as f64 {
        let got = if upper_f.is_finite() && upper_f < usize::MAX as f64 {
            upper_f as usize
        } else {
            usize::MAX
        };
        return Err(GisError::ReceiverCapExceeded {
            got,
            limit: MAX_RECEIVERS,
        });
    }
    let nx = nx_f as usize;
    let ny = ny_f as usize;

    // --- Validity guard: footprint rings + calc_area boundary as CDT constraints.
    //     build_tin rejects intersecting/degenerate rings (can_add_constraint) as a
    //     typed error — no panic (threat T-09-02-03). The TIN itself is discarded;
    //     receiver z comes from the passed-in DGM `tin`. ---
    validate_region(calc_area, footprints)?;

    // --- Precompute each footprint's AABB once so the per-lattice-point exclusion
    //     prunes on the cheap bounding-box test before the exact point-in-polygon
    //     (`contains`) — a footprint whose AABB misses the point cannot contain it.
    //     A footprint with no bounding rect (empty ring) can contain no point, so it
    //     is skipped identically to the former unconditional `contains` (false). ---
    let footprint_bounds: Vec<Option<Rect<f64>>> =
        footprints.iter().map(BoundingRect::bounding_rect).collect();

    // --- Regular lattice, clipped to calc_area minus footprints (D-07). ---
    let mut receivers: Vec<[f64; 3]> = Vec::new();
    for iy in 0..ny {
        let y = min_y + iy as f64 * spacing_m;
        for ix in 0..nx {
            let x = min_x + ix as f64 * spacing_m;
            let p = Point::new(x, y);
            if !calc_area.contains(&p) {
                continue;
            }
            if footprints.iter().zip(&footprint_bounds).any(|(f, bounds)| {
                bounds.is_some_and(|r| rect_contains(&r, x, y)) && f.contains(&p)
            }) {
                continue; // D-07: no receiver inside a building footprint
            }
            let z = tin.interpolate_z(x, y).ok_or(GisError::OutsideHull)?;
            receivers.push([x, y, z]);
        }
    }

    // --- Explicit discrete receiver points (appended verbatim; z from the TIN). ---
    for p in discrete_points {
        let z = tin.interpolate_z(p[0], p[1]).ok_or(GisError::OutsideHull)?;
        receivers.push([p[0], p[1], z]);
    }

    // --- Final cap (kept lattice + discrete): the pre-check bounds this, but a
    //     defensive re-check keeps the invariant observable. ---
    if receivers.len() > MAX_RECEIVERS {
        return Err(GisError::ReceiverCapExceeded {
            got: receivers.len(),
            limit: MAX_RECEIVERS,
        });
    }

    Ok(receivers)
}

/// Inclusive point-in-AABB test — the cheap pre-filter before the exact
/// point-in-polygon. Inclusive on all edges so it never prunes a point the polygon's
/// `contains` (strict interior) would keep.
#[inline]
fn rect_contains(r: &Rect<f64>, x: f64, y: f64) -> bool {
    x >= r.min().x && x <= r.max().x && y >= r.min().y && y <= r.max().y
}

/// Validate `calc_area` + footprint rings as constrained-Delaunay edges, reusing
/// [`envi_dgm::tin::build_tin`]'s `can_add_constraint` guard. The built TIN is
/// discarded — this is a geometry-validity gate (intersecting/degenerate rings →
/// typed [`GisError::InvalidGridRegion`], never a panic).
fn validate_region(calc_area: &Polygon<f64>, footprints: &[Polygon<f64>]) -> Result<(), GisError> {
    // Points: every ring vertex at z = 0 (z is irrelevant to the validity check).
    let mut points: Vec<[f64; 3]> = Vec::new();
    let mut breaklines: Vec<Vec<[f64; 2]>> = Vec::new();

    let push_ring =
        |poly: &Polygon<f64>, points: &mut Vec<[f64; 3]>, breaklines: &mut Vec<Vec<[f64; 2]>>| {
            let ring: Vec<[f64; 2]> = poly.exterior().coords().map(|c| [c.x, c.y]).collect();
            for v in &ring {
                points.push([v[0], v[1], 0.0]);
            }
            breaklines.push(ring);
        };

    push_ring(calc_area, &mut points, &mut breaklines);
    for f in footprints {
        push_ring(f, &mut points, &mut breaklines);
    }

    envi_dgm::tin::build_tin(&points, &breaklines).map_err(|e| GisError::InvalidGridRegion {
        message: e.to_string(),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::polygon;

    /// A flat plane z = 0 over `[−5, 105]²` so any lattice/discrete point samples
    /// ground 0.0 and stays inside the TIN hull.
    fn flat_tin() -> Tin {
        let pts = [
            [-5.0, -5.0, 0.0],
            [105.0, -5.0, 0.0],
            [105.0, 105.0, 0.0],
            [-5.0, 105.0, 0.0],
        ];
        envi_dgm::tin::build_tin(&pts, &[]).expect("flat square builds a TIN")
    }

    /// A 100 × 100 calc_area square anchored at the origin.
    fn calc_area_100() -> Polygon<f64> {
        polygon![
            (x: 0.0, y: 0.0),
            (x: 100.0, y: 0.0),
            (x: 100.0, y: 100.0),
            (x: 0.0, y: 100.0),
            (x: 0.0, y: 0.0),
        ]
    }

    #[test]
    fn regular_lattice_honours_spacing_inside_calc_area() {
        let tin = flat_tin();
        let area = calc_area_100();
        let out = receiver_grid(&area, &[], 10.0, &[], &tin).unwrap();
        // Interior lattice at 10 m over (0,100) — boundary points are not "contained"
        // by geo, so the kept grid is the strict interior 10..=90 → 9 × 9 = 81.
        assert!(
            !out.is_empty(),
            "a 100 m area at 10 m spacing yields receivers"
        );
        for r in &out {
            // Every kept point is strictly inside the area and on the 10 m lattice.
            assert!(r[0] > 0.0 && r[0] < 100.0 && r[1] > 0.0 && r[1] < 100.0);
            assert!((r[0] / 10.0).fract().abs() < 1e-9);
            assert!((r[1] / 10.0).fract().abs() < 1e-9);
            assert_eq!(r[2], 0.0, "z sampled from the flat TIN");
        }
        assert_eq!(out.len(), 81, "9 × 9 interior lattice");
    }

    #[test]
    fn no_receiver_lands_inside_a_footprint() {
        let tin = flat_tin();
        let area = calc_area_100();
        // A footprint covering x∈[30,60], y∈[30,60].
        let footprint = polygon![
            (x: 30.0, y: 30.0),
            (x: 60.0, y: 30.0),
            (x: 60.0, y: 60.0),
            (x: 30.0, y: 60.0),
            (x: 30.0, y: 30.0),
        ];
        let out = receiver_grid(&area, std::slice::from_ref(&footprint), 10.0, &[], &tin).unwrap();
        assert!(
            !out.iter()
                .any(|r| footprint.contains(&Point::new(r[0], r[1]))),
            "no receiver is inside the footprint (D-07)"
        );
        // The four interior lattice points at (40,40),(40,50),(50,40),(50,50) are gone.
        assert!(
            !out.iter()
                .any(|r| (r[0] == 40.0 || r[0] == 50.0) && (r[1] == 40.0 || r[1] == 50.0)),
            "footprint-interior lattice points are excluded"
        );
    }

    #[test]
    fn discrete_points_are_included_with_tin_z() {
        let tin = flat_tin();
        let area = calc_area_100();
        let discrete = [[5.0, 5.0], [95.0, 95.0]];
        let out = receiver_grid(&area, &[], 10.0, &discrete, &tin).unwrap();
        assert!(
            out.iter()
                .any(|r| r[0] == 5.0 && r[1] == 5.0 && r[2] == 0.0)
        );
        assert!(
            out.iter()
                .any(|r| r[0] == 95.0 && r[1] == 95.0 && r[2] == 0.0)
        );
    }

    #[test]
    fn spacing_below_minimum_is_typed_error() {
        let tin = flat_tin();
        let area = calc_area_100();
        let err = receiver_grid(&area, &[], 0.5, &[], &tin).unwrap_err();
        assert_eq!(
            err,
            GisError::SpacingTooSmall {
                got: 0.5,
                min: MIN_SPACING_M
            }
        );
        // Non-finite spacing is also rejected as SpacingTooSmall (the >= negation).
        assert!(matches!(
            receiver_grid(&area, &[], f64::NAN, &[], &tin),
            Err(GisError::SpacingTooSmall { .. })
        ));
    }

    #[test]
    fn receiver_count_cap_is_enforced_before_allocation() {
        let tin = flat_tin();
        // A huge calc_area at 1 m spacing would exceed MAX_RECEIVERS; rejected up front.
        let big = polygon![
            (x: 0.0, y: 0.0),
            (x: 5000.0, y: 0.0),
            (x: 5000.0, y: 5000.0),
            (x: 0.0, y: 5000.0),
            (x: 0.0, y: 0.0),
        ];
        let err = receiver_grid(&big, &[], 1.0, &[], &tin).unwrap_err();
        assert!(
            matches!(
                err,
                GisError::ReceiverCapExceeded {
                    limit: MAX_RECEIVERS,
                    ..
                }
            ),
            "over-cap grid is ReceiverCapExceeded, got {err:?}"
        );
    }

    #[test]
    fn intersecting_footprint_ring_is_typed_error_not_panic() {
        let tin = flat_tin();
        let area = calc_area_100();
        // A self-intersecting (bow-tie) footprint ring → build_tin rejects it.
        let bad = Polygon::new(
            geo::LineString::from(vec![
                (30.0, 30.0),
                (60.0, 60.0),
                (60.0, 30.0),
                (30.0, 60.0),
                (30.0, 30.0),
            ]),
            vec![],
        );
        let err = receiver_grid(&area, std::slice::from_ref(&bad), 10.0, &[], &tin).unwrap_err();
        assert!(
            matches!(err, GisError::InvalidGridRegion { .. }),
            "self-intersecting footprint is InvalidGridRegion, got {err:?}"
        );
    }

    #[test]
    fn receiver_outside_the_tin_hull_is_typed_error_not_zero() {
        // A TIN whose hull only covers a corner of the calc_area.
        let small = envi_dgm::tin::build_tin(
            &[
                [-1.0, -1.0, 0.0],
                [20.0, -1.0, 0.0],
                [20.0, 20.0, 0.0],
                [-1.0, 20.0, 0.0],
            ],
            &[],
        )
        .unwrap();
        let area = calc_area_100();
        // Lattice points beyond (20,20) leave the small hull → typed error, not 0.0.
        let err = receiver_grid(&area, &[], 10.0, &[], &small).unwrap_err();
        assert_eq!(err, GisError::OutsideHull, "hull miss is OutsideHull");
    }

    #[test]
    fn z_is_ground_only_no_receiver_height_baked_in() {
        // A sloped plane z = y; a receiver at y = 50 must sample z = 50 (ground),
        // never 50 + a receiver height.
        let sloped = envi_dgm::tin::build_tin(
            &[
                [-5.0, -5.0, -5.0],
                [105.0, -5.0, -5.0],
                [105.0, 105.0, 105.0],
                [-5.0, 105.0, 105.0],
            ],
            &[],
        )
        .unwrap();
        let area = calc_area_100();
        let out = receiver_grid(&area, &[], 10.0, &[[50.0, 50.0]], &sloped).unwrap();
        let r = out
            .iter()
            .find(|r| r[0] == 50.0 && r[1] == 50.0)
            .expect("the discrete point at (50,50)");
        assert!(
            (r[2] - 50.0).abs() < 1e-9,
            "z = ground y = 50, got {}",
            r[2]
        );
    }
}
