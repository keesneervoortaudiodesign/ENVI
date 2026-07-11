//! Ground-impedance segmentation along a cut-profile (GEOX-02).
//!
//! # Module I/O
//! - **Inputs:** the ascending-x `(x, z)` cut-profile points (from
//!   [`crate::profile::cut_profile`]) and their planar `(x, y)` preimages on the
//!   S→R line, plus user-**drawn** `ground_zone` polygons, **imported** land-cover
//!   `ground_zone` polygons, and a project **default** impedance class.
//! - **Output:** a [`GroundSegmentation`] — the profile with polygon-boundary
//!   crossings spliced into the ascending-x point list, and one
//!   [`envi_engine::scene::GroundSegment`] per interval (so `segments.len() ==
//!   points.len() - 1`). Each interval resolves its class **drawn > imported >
//!   default**; the flow resistivity σ is resolved ONLY through
//!   `envi_engine::scene::impedance_class` — never restated as a literal.
//! - **Invariants (load-bearing):**
//!   1. **Per-INTERVAL, not per-point** (09-RESEARCH anti-pattern): a
//!      [`GroundSegment`] belongs to the span between two vertices, classified at a
//!      representative interior (midpoint) planar point. Sampling at vertices would
//!      silently shift every ground reflection off-by-one.
//!   2. **Boundaries land on real edges**: a `ground_zone` boundary that crosses
//!      the S→R line splices a new profile vertex at the crossing (kept strictly
//!      ascending), so each interval lies wholly inside one zone (or outside all).
//!   3. **One source of truth for σ** (SC2/SC3): the class LETTER comes from the
//!      priority resolution; σ comes from `impedance_class(letter)` (class B = 31.5,
//!      never re-derived). Imported land cover resolves its letter via
//!      [`crate::impedance_table::worldcover_to_class`]. An unresolvable class is a
//!      typed [`GisError::UnresolvableClass`], never a fabricated σ.
//!   4. **Roughness** defaults to class N (`0.0 m`) unless a drawn zone carries an
//!      explicit roughness (in meters). The engine has no roughness→σ ladder to
//!      restate here, so roughness is passed through as meters, not re-derived.

use envi_engine::scene::{GroundSegment, impedance_class};
use geo::line_intersection::{LineIntersection, line_intersection};
use geo::{Contains, Coord, Line, Point, Polygon};

use crate::GisError;
use crate::impedance_table::worldcover_to_class;

/// Minimum strictly-ascending-x separation between spliced/kept vertices (meters),
/// matching [`crate::profile`]'s dedupe epsilon so a crossing that lands on an
/// existing vertex is absorbed rather than duplicating x.
const X_EPSILON_M: f64 = 1e-6;

/// A user-drawn ground-impedance zone: a planar polygon (scene meters) carrying an
/// explicit Nord2000 class letter and an optional roughness (meters, class N = 0).
#[derive(Debug, Clone, PartialEq)]
pub struct DrawnZone {
    /// The zone footprint in planar scene coordinates `(x, y)`.
    pub polygon: Polygon<f64>,
    /// The Nord2000 impedance class letter (`A..=H`).
    pub class: char,
    /// Terrain roughness in meters (class N = `0.0`).
    pub roughness_m: f64,
}

/// An imported land-cover ground zone: a planar polygon carrying an ESA WorldCover
/// class code, resolved to a Nord2000 letter via [`worldcover_to_class`].
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedZone {
    /// The zone footprint in planar scene coordinates `(x, y)`.
    pub polygon: Polygon<f64>,
    /// ESA WorldCover v200 class code (resolved to a letter, never a σ literal).
    pub worldcover_code: u8,
}

/// The result of [`segment_ground`]: the (boundary-spliced) profile plus its
/// per-interval ground segments. Returned together because splicing changes the
/// point list — a bare `Vec<GroundSegment>` would desync from the caller's points
/// and break `TerrainProfile::new` (which requires `segments.len() == points - 1`).
#[derive(Debug, Clone, PartialEq)]
pub struct GroundSegmentation {
    /// The `(x, z)` profile points, strictly ascending, with boundary crossings
    /// spliced in.
    pub points: Vec<[f64; 2]>,
    /// The planar `(x, y)` preimage of each point in [`Self::points`].
    pub planar_xy: Vec<[f64; 2]>,
    /// One segment per interval; `len() == points.len() - 1`.
    pub segments: Vec<GroundSegment>,
}

/// Segment the cut-profile into per-interval [`GroundSegment`]s, resolving each
/// interval's class **drawn > imported > default** and splicing `ground_zone`
/// boundary crossings into the ascending-x point list first.
///
/// `points` and `planar_xy` are the parallel outputs of the extractor: `points[i]`
/// is `(x, z)` (x = horizontal distance) and `planar_xy[i]` is the planar `(x, y)`
/// on the S→R line. The endpoints of `planar_xy` define the line the zone
/// boundaries are intersected against.
///
/// # Errors
/// - [`GisError::DegenerateProfile`] — mismatched `points`/`planar_xy` lengths, or
///   fewer than two points.
/// - [`GisError::UnresolvableClass`] — a resolved class letter (drawn or default)
///   does not map to a σ through `impedance_class`.
pub fn segment_ground(
    points: &[[f64; 2]],
    planar_xy: &[[f64; 2]],
    drawn_zones: &[DrawnZone],
    imported_zones: &[ImportedZone],
    default_class: char,
) -> Result<GroundSegmentation, GisError> {
    if points.len() != planar_xy.len() {
        return Err(GisError::DegenerateProfile {
            what: format!(
                "points ({}) and planar_xy ({}) length mismatch",
                points.len(),
                planar_xy.len()
            ),
        });
    }
    if points.len() < 2 {
        return Err(GisError::DegenerateProfile {
            what: format!("need >= 2 profile points to segment, got {}", points.len()),
        });
    }

    // The S→R line in both frames. `x` spans [x0, xN]; planar spans [S, R].
    let x0 = points[0][0];
    let xn = points[points.len() - 1][0];
    let span = xn - x0;
    let s = planar_xy[0];
    let r = planar_xy[planar_xy.len() - 1];
    let sr_line = Line::new(Coord { x: s[0], y: s[1] }, Coord { x: r[0], y: r[1] });

    // --- Collect polygon-boundary ∩ S→R crossings as interior x values. ---
    let mut crossing_xs: Vec<f64> = Vec::new();
    if span > 0.0 {
        let mut collect = |poly: &Polygon<f64>| {
            for edge in polygon_edges(poly) {
                if let Some(LineIntersection::SinglePoint { intersection, .. }) =
                    line_intersection(sr_line, edge)
                {
                    // Project the crossing onto the line → distance-x. It already
                    // lies on the segment, so the fraction is in [0, 1].
                    let frac = ((intersection.x - s[0]) * (r[0] - s[0])
                        + (intersection.y - s[1]) * (r[1] - s[1]))
                        / ((r[0] - s[0]).powi(2) + (r[1] - s[1]).powi(2));
                    let x = x0 + frac * span;
                    if x > x0 + X_EPSILON_M && x < xn - X_EPSILON_M {
                        crossing_xs.push(x);
                    }
                }
            }
        };
        for z in drawn_zones {
            collect(&z.polygon);
        }
        for z in imported_zones {
            collect(&z.polygon);
        }
    }

    // --- Splice crossings into the ascending-x (x, z) + planar lists. ---
    let (pts, planar) = splice_points(points, crossing_xs, x0, span, s, r);

    // --- One segment per interval, classified at the interval midpoint. ---
    let mut segments = Vec::with_capacity(pts.len() - 1);
    for w in planar.windows(2) {
        let mid = Point::new((w[0][0] + w[1][0]) / 2.0, (w[0][1] + w[1][1]) / 2.0);
        let (letter, roughness) = resolve_class(mid, drawn_zones, imported_zones, default_class);
        let sigma = impedance_class(letter).ok_or(GisError::UnresolvableClass { class: letter })?;
        segments.push(GroundSegment {
            flow_resistivity: sigma,
            roughness,
        });
    }

    Ok(GroundSegmentation {
        points: pts,
        planar_xy: planar,
        segments,
    })
}

/// The boundary edges of a polygon (exterior + every interior ring) as `geo`
/// [`Line`] segments.
fn polygon_edges(poly: &Polygon<f64>) -> Vec<Line<f64>> {
    let mut edges: Vec<Line<f64>> = poly.exterior().lines().collect();
    for ring in poly.interiors() {
        edges.extend(ring.lines());
    }
    edges
}

/// Splice `crossing_xs` into the profile, keeping x strictly ascending. Each
/// inserted vertex takes its `z` by linear interpolation on the profile polyline
/// and its planar `(x, y)` from the affine S→R map.
fn splice_points(
    points: &[[f64; 2]],
    mut crossing_xs: Vec<f64>,
    x0: f64,
    span: f64,
    s: [f64; 2],
    r: [f64; 2],
) -> (Vec<[f64; 2]>, Vec<[f64; 2]>) {
    crossing_xs.sort_by(f64::total_cmp);
    crossing_xs.dedup_by(|a, b| (*a - *b).abs() <= X_EPSILON_M);

    let mut pts: Vec<[f64; 2]> = Vec::with_capacity(points.len() + crossing_xs.len());
    let mut planar: Vec<[f64; 2]> = Vec::with_capacity(points.len() + crossing_xs.len());
    let mut ci = 0usize;
    let mut last_x = f64::NEG_INFINITY;

    let mut push = |x: f64, z: f64, pts: &mut Vec<[f64; 2]>, planar: &mut Vec<[f64; 2]>| {
        if x > last_x + X_EPSILON_M {
            pts.push([x, z]);
            let frac = if span > 0.0 { (x - x0) / span } else { 0.0 };
            planar.push([s[0] + frac * (r[0] - s[0]), s[1] + frac * (r[1] - s[1])]);
            last_x = x;
        }
    };

    for seg in points.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        push(a[0], a[1], &mut pts, &mut planar);
        // Insert any crossings strictly inside (a.x, b.x), z linearly interpolated.
        while ci < crossing_xs.len() && crossing_xs[ci] < b[0] - X_EPSILON_M {
            let cx = crossing_xs[ci];
            if cx > a[0] + X_EPSILON_M {
                let t = (cx - a[0]) / (b[0] - a[0]);
                let cz = a[1] + t * (b[1] - a[1]);
                push(cx, cz, &mut pts, &mut planar);
            }
            ci += 1;
        }
    }
    // The final endpoint.
    let last = points[points.len() - 1];
    push(last[0], last[1], &mut pts, &mut planar);

    (pts, planar)
}

/// Resolve an interval's `(class letter, roughness_m)` at its representative planar
/// midpoint, in priority order **drawn > imported > default**. A drawn/imported
/// zone whose polygon contains the midpoint wins; imported letters resolve through
/// [`worldcover_to_class`] (an unknown code falls through to the next priority).
fn resolve_class(
    mid: Point<f64>,
    drawn_zones: &[DrawnZone],
    imported_zones: &[ImportedZone],
    default_class: char,
) -> (char, f64) {
    for z in drawn_zones {
        if z.polygon.contains(&mid) {
            return (z.class, z.roughness_m);
        }
    }
    for z in imported_zones {
        if z.polygon.contains(&mid)
            && let Some(letter) = worldcover_to_class(z.worldcover_code)
        {
            // Imported zones default to roughness class N (no roughness in land cover).
            return (letter, 0.0);
        }
    }
    (default_class, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use envi_engine::scene::TerrainProfile;
    use geo::polygon;

    /// A y=0 profile from x=0 to x=10, flat z, with its planar preimage on the
    /// x-axis — the shared frame for the segmentation tests.
    fn flat_profile() -> (Vec<[f64; 2]>, Vec<[f64; 2]>) {
        let points = vec![[0.0, 5.0], [10.0, 5.0]];
        let planar = vec![[0.0, 0.0], [10.0, 0.0]];
        (points, planar)
    }

    /// An axis-straddling rectangle spanning `x ∈ [x0, x1]`, `y ∈ [-1, 1]`.
    fn rect(x0: f64, x1: f64) -> Polygon<f64> {
        polygon![
            (x: x0, y: -1.0),
            (x: x1, y: -1.0),
            (x: x1, y: 1.0),
            (x: x0, y: 1.0),
            (x: x0, y: -1.0),
        ]
    }

    #[test]
    fn drawn_imported_default_resolve_per_interval_with_spliced_boundaries() {
        let (points, planar) = flat_profile();
        // Drawn class H over x∈[2,4]; imported grassland (code 30 → D) over x∈[6,8].
        let drawn = vec![DrawnZone {
            polygon: rect(2.0, 4.0),
            class: 'H',
            roughness_m: 0.0,
        }];
        let imported = vec![ImportedZone {
            polygon: rect(6.0, 8.0),
            worldcover_code: 30,
        }];

        let out = segment_ground(&points, &planar, &drawn, &imported, 'A').unwrap();

        // Crossings at x = 2,4,6,8 splice into the 2-point profile → 6 points.
        let xs: Vec<f64> = out.points.iter().map(|p| p[0]).collect();
        assert_eq!(
            xs,
            vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0],
            "boundaries spliced"
        );
        assert_eq!(
            out.segments.len(),
            out.points.len() - 1,
            "segments == points - 1"
        );

        // Per-interval σ, resolved ONLY through the engine (drawn > imported > default).
        let sig: Vec<f64> = out.segments.iter().map(|s| s.flow_resistivity).collect();
        assert_eq!(
            sig,
            vec![
                impedance_class('A').unwrap(), // [0,2] default
                impedance_class('H').unwrap(), // [2,4] drawn
                impedance_class('A').unwrap(), // [4,6] default
                impedance_class('D').unwrap(), // [6,8] imported grassland
                impedance_class('A').unwrap(), // [8,10] default
            ]
        );
    }

    #[test]
    fn drawn_overrides_imported_on_overlap() {
        let (points, planar) = flat_profile();
        // Drawn (B) and imported (water, code 80 → H) both cover x∈[3,7]; drawn wins.
        let drawn = vec![DrawnZone {
            polygon: rect(3.0, 7.0),
            class: 'B',
            roughness_m: 0.02,
        }];
        let imported = vec![ImportedZone {
            polygon: rect(3.0, 7.0),
            worldcover_code: 80,
        }];
        let out = segment_ground(&points, &planar, &drawn, &imported, 'A').unwrap();

        let mid_seg = out
            .segments
            .iter()
            .zip(out.points.windows(2))
            .find(|(_, w)| w[0][0] >= 3.0 && w[1][0] <= 7.0)
            .map(|(s, _)| s)
            .expect("an interval inside [3,7]");
        // Class B = 31.5 via the engine (never a σ literal), and the drawn roughness.
        assert_eq!(mid_seg.flow_resistivity, impedance_class('B').unwrap());
        assert_eq!(mid_seg.flow_resistivity, 31.5, "class B resolves to 31.5");
        assert_eq!(
            mid_seg.roughness, 0.02,
            "drawn zone roughness carried through"
        );
    }

    #[test]
    fn no_zones_yields_one_default_segment_and_no_sigma_literal() {
        let (points, planar) = flat_profile();
        let out = segment_ground(&points, &planar, &[], &[], 'D').unwrap();
        assert_eq!(out.points.len(), 2, "no crossings → unchanged points");
        assert_eq!(out.segments.len(), 1);
        assert_eq!(
            out.segments[0].flow_resistivity,
            impedance_class('D').unwrap()
        );
    }

    #[test]
    fn unknown_imported_code_falls_through_to_default() {
        let (points, planar) = flat_profile();
        // Code 200 is not a WorldCover class → the interval falls through to default.
        let imported = vec![ImportedZone {
            polygon: rect(2.0, 8.0),
            worldcover_code: 200,
        }];
        let out = segment_ground(&points, &planar, &[], &imported, 'C').unwrap();
        for s in &out.segments {
            assert_eq!(s.flow_resistivity, impedance_class('C').unwrap());
        }
    }

    #[test]
    fn unresolvable_default_class_is_typed_error_not_fabricated_sigma() {
        let (points, planar) = flat_profile();
        // 'Z' is not A..=H → typed error, never a fabricated σ.
        let err = segment_ground(&points, &planar, &[], &[], 'Z').unwrap_err();
        assert_eq!(err, GisError::UnresolvableClass { class: 'Z' });
    }

    #[test]
    fn segmentation_builds_a_valid_terrain_profile() {
        let (points, planar) = flat_profile();
        let drawn = vec![DrawnZone {
            polygon: rect(4.0, 6.0),
            class: 'G',
            roughness_m: 0.0,
        }];
        let out = segment_ground(&points, &planar, &drawn, &[], 'D').unwrap();
        // The spliced points + segments compose a valid engine cut plane.
        let tp = TerrainProfile::new(out.points.clone(), out.segments.clone());
        assert!(
            tp.is_ok(),
            "spliced segmentation is a valid TerrainProfile: {tp:?}"
        );
    }

    #[test]
    fn length_mismatch_and_too_few_points_are_typed_errors() {
        let bad = segment_ground(&[[0.0, 0.0], [1.0, 0.0]], &[[0.0, 0.0]], &[], &[], 'D');
        assert!(matches!(bad, Err(GisError::DegenerateProfile { .. })));
        let one = segment_ground(&[[0.0, 0.0]], &[[0.0, 0.0]], &[], &[], 'D');
        assert!(matches!(one, Err(GisError::DegenerateProfile { .. })));
    }

    #[test]
    fn boundary_crossing_splices_a_vertex_on_a_multi_point_profile() {
        // A 3-point profile; a single zone boundary at x=1.5 must splice one vertex.
        let points = vec![[0.0, 0.0], [1.0, 1.0], [3.0, 3.0]];
        let planar = vec![[0.0, 0.0], [1.0, 0.0], [3.0, 0.0]];
        let drawn = vec![DrawnZone {
            polygon: rect(1.5, 5.0),
            class: 'H',
            roughness_m: 0.0,
        }];
        let out = segment_ground(&points, &planar, &drawn, &[], 'A').unwrap();
        let xs: Vec<f64> = out.points.iter().map(|p| p[0]).collect();
        assert!(xs.contains(&1.5), "the x=1.5 boundary is spliced: {xs:?}");
        // z at the spliced vertex is linearly interpolated on the [1,3] segment.
        let spliced = out
            .points
            .iter()
            .find(|p| (p[0] - 1.5).abs() < 1e-9)
            .unwrap();
        assert!((spliced[1] - 1.5).abs() < 1e-9, "z interpolated to 1.5");
    }
}
