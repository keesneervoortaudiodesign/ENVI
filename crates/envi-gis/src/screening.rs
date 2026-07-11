//! Screening-edge derivation → cut-profile vertices (GEOX-03).
//!
//! # Module I/O
//! - **Inputs:** a base [`GroundSegmentation`] (the boundary-spliced cut-profile
//!   from [`crate::impedance::segment_ground`], carrying the parallel `(x, z)`
//!   ground points and their planar `(x, y)` preimages on the S→R line), a set of
//!   height-bearing [`ScreenObject`]s (buildings + walls + barriers, D-08), and the
//!   DGM [`Tin`] used to sample the ground beneath each screen crossing.
//! - **Output:** an augmented [`GroundSegmentation`] with each cut-plane ∩ object
//!   crossing spliced in as an `(x, z)` vertex at `z = ground_z(crossing) +
//!   object_height` (the prism top, D-09), and the intervals **spanned by** an
//!   object retagged as **hard** ground (class H, roughness 0). It is ready for
//!   `TerrainProfile::new` — **there is no separate screen list**.
//! - **Invariants (load-bearing):**
//!   1. **No separate `screens` field** (09-RESEARCH Pitfall 1, the single biggest
//!      reshape): the engine `SolveJob` carries only `profile: &TerrainProfile`;
//!      `terrain_interpretation` derives the ≤2 diffracting edges FROM the profile
//!      interior points. GEOX-03 therefore injects `(x, z)` vertices into the SAME
//!      profile — it never emits a `Vec<ScreenEdge>` to the solver.
//!   2. **Screen tops ride on terrain** (D-09): the injected `z` is
//!      `ground_z(crossing) + object_height` (`eaves_height_m` for buildings,
//!      `height_m` for walls/barriers) — never collapsed to a source/receiver
//!      height, never a fabricated `0.0` (a crossing outside the TIN hull is a typed
//!      [`GisError::OutsideHull`]).
//!   3. **Thick screen = two tops** (D-09, Sub-model 5): a line passing THROUGH a
//!      building yields TWO top vertices (entry + exit) and the span between them is
//!      hard ground. Multi-edge is preserved (all crossing tops inserted) — but
//!      Nord2000's `terrain_interpretation` caps the diffraction at **≤2 screens /
//!      one thick two-edge screen** (Sub-model 4/5/6). This is a documented
//!      standard-conformant limitation, NOT an N-edge combiner (09-RESEARCH
//!      Pitfall 4).
//!   4. **Bounded candidate set** (threat T-09-02-01): the R*-tree corridor query is
//!      capped at [`MAX_CORRIDOR_CANDIDATES`]; an over-large set is a typed
//!      [`GisError::CorridorCandidatesExceeded`], rejected before per-candidate work.
//!
//! # Corridor half-width (`[ASSUMED]` engineering constant, 09-RESEARCH A1)
//! Candidate objects are pre-filtered to those whose AABB intersects a corridor of
//! half-width `max(`[`CORRIDOR_MIN_HALF_WIDTH_M`]`, r_F1)` around the S→R line,
//! where `r_F1` is the first-Fresnel-zone radius at [`CORRIDOR_REF_FREQ_HZ`] at the
//! path midpoint. Objects outside the first Fresnel zone contribute negligibly to
//! diffraction; the fixed floor covers short paths where the Fresnel radius is tiny
//! but a nearby wall still screens. The exact constants are a defensible engineering
//! choice (the principle is standard acoustics), so they are named `pub const`s the
//! reviewer can sanity-check, not silent magic numbers.

use envi_dgm::tin::Tin;
use geo::line_intersection::{LineIntersection, line_intersection};
use geo::{BoundingRect, Contains, Coord, Line, LineString, Point, Polygon};
use rstar::{AABB, RTree, RTreeObject};

use crate::impedance::GroundSegmentation;
use crate::{GisError, X_EPSILON_M};

use envi_engine::scene::{GroundSegment, impedance_class};

/// Minimum corridor half-width (meters) for the candidate-object query
/// (`[ASSUMED]`, 09-RESEARCH A1). A floor under the Fresnel radius so short paths
/// still capture a nearby screening wall whose first-Fresnel-zone radius is tiny.
pub const CORRIDOR_MIN_HALF_WIDTH_M: f64 = 20.0;

/// Reference frequency (Hz) at which the first-Fresnel-zone corridor radius is
/// evaluated (`[ASSUMED]`, 09-RESEARCH A1). A low band where diffraction matters
/// most; `λ ≈ 1.36 m` at 250 Hz.
pub const CORRIDOR_REF_FREQ_HZ: f64 = 250.0;

/// Reference sound speed (m/s) used ONLY to size the geometric screening corridor
/// (`λ = c / f`). This is not an acoustic result — it never enters a level
/// computation — so a fixed nominal value is adequate for the candidate filter.
const CORRIDOR_SOUND_SPEED_MS: f64 = 340.0;

/// Hard cap on screening-corridor candidate objects (`[ASSUMED]` DoS bound, threat
/// T-09-02-01). Far above any realistic scene's building/wall count within a single
/// path corridor; it only trips on a pathological geometry set, which is rejected
/// before any per-candidate cut-plane ∩ prism work (mirrors
/// `terrain::MAX_TERRAIN_POINTS` / `envi_dgm::tin::MAX_POINTS`).
pub const MAX_CORRIDOR_CANDIDATES: usize = 100_000;

/// A height-bearing scene object that screens along a path (D-08). Buildings screen
/// at their eaves height with their footprint exterior ring; walls and barriers
/// screen at their top height with a linestring.
#[derive(Debug, Clone, PartialEq)]
pub enum ScreenObject {
    /// A building: the footprint exterior ring screens at `eaves_height_m`. A path
    /// passing THROUGH the footprint yields two top vertices (a thick screen).
    Building {
        /// Footprint polygon in planar scene coordinates `(x, y)`.
        footprint: Polygon<f64>,
        /// Eaves height above local ground, meters.
        eaves_height_m: f64,
    },
    /// A wall or barrier: the linestring screens at `height_m`. Every crossed
    /// segment yields one top vertex (a thin screen).
    Barrier {
        /// Barrier/wall top polyline in planar scene coordinates `(x, y)`.
        line: LineString<f64>,
        /// Screen top height above local ground, meters.
        height_m: f64,
    },
}

impl ScreenObject {
    /// The object's axis-aligned bounding box in planar `(x, y)`, or `None` for an
    /// empty geometry (no vertices → nothing to index).
    fn aabb(&self) -> Option<AABB<[f64; 2]>> {
        let rect = match self {
            ScreenObject::Building { footprint, .. } => footprint.bounding_rect(),
            ScreenObject::Barrier { line, .. } => line.bounding_rect(),
        }?;
        Some(AABB::from_corners(
            [rect.min().x, rect.min().y],
            [rect.max().x, rect.max().y],
        ))
    }
}

/// An R*-tree entry keyed by a screen object's AABB, referring back into the caller's
/// object slice by index (so the geometry is not duplicated into the tree).
#[derive(Debug, Clone, Copy)]
struct AabbEntry {
    idx: usize,
    envelope: AABB<[f64; 2]>,
}

impl RTreeObject for AabbEntry {
    type Envelope = AABB<[f64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

/// First-Fresnel-zone radius (meters) at the path midpoint for a source→receiver
/// distance `d` and reference frequency `f_ref`.
///
/// `r_F1 = sqrt(λ · d1 · d2 / (d1 + d2))` with `d1 = d2 = d/2` collapses to
/// `sqrt(λ · d / 4)`. Used only to size the geometric candidate corridor.
#[must_use]
fn fresnel_half_width(d: f64, f_ref: f64) -> f64 {
    let lambda = CORRIDOR_SOUND_SPEED_MS / f_ref;
    (lambda * d / 4.0).max(0.0).sqrt()
}

/// Inject screening edges into a base cut-profile as `(x, z)` vertices (GEOX-03).
///
/// `base` is the boundary-spliced [`GroundSegmentation`] from GEOX-02; its
/// `planar_xy` endpoints define the S→R line. `screens` are the height-bearing
/// objects (buildings + walls + barriers). `tin` samples the ground beneath each
/// crossing so a screen top rides on terrain (`z = ground_z + object_height`).
///
/// Only objects whose AABB intersects the S→R corridor (half-width
/// `max(`[`CORRIDOR_MIN_HALF_WIDTH_M`]`, first-Fresnel@`[`CORRIDOR_REF_FREQ_HZ`]`)`)
/// are processed; the candidate count is capped at [`MAX_CORRIDOR_CANDIDATES`]. Each
/// building/wall crossing splices a top vertex; a building the line passes through
/// contributes two tops (thick screen) and the span between them becomes hard
/// ground (class H). Non-screen intervals keep their base segment. The result is a
/// valid `TerrainProfile` input — **no separate screen list is produced**.
///
/// # Errors
/// - [`GisError::DegenerateProfile`] — `base` has mismatched points/planar lengths,
///   fewer than two points, or a zero-length S→R line.
/// - [`GisError::CorridorCandidatesExceeded`] — the wire-supplied `screens` set,
///   or the corridor query result, exceeded [`MAX_CORRIDOR_CANDIDATES`] (the input
///   bound is checked *before* the R*-tree is allocated).
/// - [`GisError::OutsideHull`] — a screen crossing's ground point left the TIN hull.
/// - [`GisError::UnresolvableClass`] — the hard-ground class H failed to resolve
///   (unreachable in practice; guards against an engine table change).
pub fn inject_screens(
    base: &GroundSegmentation,
    screens: &[ScreenObject],
    tin: &Tin,
) -> Result<GroundSegmentation, GisError> {
    if base.points.len() != base.planar_xy.len() {
        return Err(GisError::DegenerateProfile {
            what: format!(
                "points ({}) and planar_xy ({}) length mismatch",
                base.points.len(),
                base.planar_xy.len()
            ),
        });
    }
    if base.points.len() < 2 {
        return Err(GisError::DegenerateProfile {
            what: format!(
                "need >= 2 profile points to screen, got {}",
                base.points.len()
            ),
        });
    }

    let x0 = base.points[0][0];
    let xn = base.points[base.points.len() - 1][0];
    let span = xn - x0;
    let s = base.planar_xy[0];
    let r = base.planar_xy[base.planar_xy.len() - 1];
    let d = (r[0] - s[0]).hypot(r[1] - s[1]);
    if d <= 0.0 || span <= 0.0 {
        return Err(GisError::DegenerateProfile {
            what: "zero-length S→R line; cannot derive screening edges".to_string(),
        });
    }
    let sr_line = Line::new(Coord { x: s[0], y: s[1] }, Coord { x: r[0], y: r[1] });

    // --- Corridor candidate query (bounded, threat T-09-02-01). ---
    // Bound the WIRE-SUPPLIED input set BEFORE allocating the entry vector or
    // building the R*-tree (WR-02): `screens` comes straight off `InjectScreensReq`
    // (unbounded), so checking only the post-query candidate count would still let a
    // pathological set drive `bulk_load` over every object first. Mirror
    // `grid::receiver_grid`'s check-before-allocate posture.
    if screens.len() > MAX_CORRIDOR_CANDIDATES {
        return Err(GisError::CorridorCandidatesExceeded {
            got: screens.len(),
            limit: MAX_CORRIDOR_CANDIDATES,
        });
    }
    let entries: Vec<AabbEntry> = screens
        .iter()
        .enumerate()
        .filter_map(|(idx, obj)| obj.aabb().map(|envelope| AabbEntry { idx, envelope }))
        .collect();
    let tree = RTree::bulk_load(entries);

    let half_w = fresnel_half_width(d, CORRIDOR_REF_FREQ_HZ).max(CORRIDOR_MIN_HALF_WIDTH_M);
    let corridor = AABB::from_corners(
        [s[0].min(r[0]) - half_w, s[1].min(r[1]) - half_w],
        [s[0].max(r[0]) + half_w, s[1].max(r[1]) + half_w],
    );
    let candidates: Vec<usize> = tree
        .locate_in_envelope_intersecting(corridor)
        .map(|e: &AabbEntry| e.idx)
        .collect();
    if candidates.len() > MAX_CORRIDOR_CANDIDATES {
        return Err(GisError::CorridorCandidatesExceeded {
            got: candidates.len(),
            limit: MAX_CORRIDOR_CANDIDATES,
        });
    }

    // --- Per-candidate cut-plane ∩ prism → top vertices + hard spans. ---
    // A screen top vertex: distance-x along the line and its elevated z (ground +
    // object height). A hard span: [x_a, x_b] the line spends inside a building.
    let mut tops: Vec<[f64; 2]> = Vec::new(); // (x, z_top)
    let mut hard_spans: Vec<[f64; 2]> = Vec::new(); // (x_a, x_b)

    for &idx in &candidates {
        match &screens[idx] {
            ScreenObject::Building {
                footprint,
                eaves_height_m,
            } => {
                let mut xs = ring_crossings(&sr_line, footprint, s, r, x0, span);
                xs.sort_by(f64::total_cmp);
                xs.dedup_by(|a, b| (*a - *b).abs() <= X_EPSILON_M);
                for &x in &xs {
                    let z = screen_top_z(tin, s, r, x0, span, x, *eaves_height_m)?;
                    tops.push([x, z]);
                }
                // Pair crossings (entry→exit) into hard spans. `ring_crossings`
                // excludes the path endpoints, so if S or R lies *inside* the
                // footprint the crossing count is ODD and a naive `chunks_exact(2)`
                // would silently drop the unpaired interior span (IN-02). Anchor the
                // sequence with x0 when S is inside and xn when R is inside so the
                // list is always even and every inside interval is tagged hard.
                let mut bounds = xs.clone();
                if footprint.contains(&Point::new(s[0], s[1])) {
                    bounds.insert(0, x0);
                }
                if footprint.contains(&Point::new(r[0], r[1])) {
                    bounds.push(xn);
                }
                for pair in bounds.chunks_exact(2) {
                    hard_spans.push([pair[0], pair[1]]);
                }
            }
            ScreenObject::Barrier { line, height_m } => {
                for seg in line.lines() {
                    if let Some(x) = line_cross_x(&sr_line, seg, s, r, x0, span) {
                        let z = screen_top_z(tin, s, r, x0, span, x, *height_m)?;
                        tops.push([x, z]);
                    }
                }
            }
        }
    }

    // --- Merge screen tops into the base profile (strictly ascending x). ---
    let (points, planar_xy) = merge_tops(base, &mut tops, x0, span, s, r);

    // --- One segment per interval: hard (class H) inside a span, else inherited. ---
    // The merged `points` and `base.points` are both strictly ascending in x, so the
    // inherited base segment is found with a single monotonic cursor (`bi`) advancing
    // as `mid_x` increases — O(n) total instead of re-scanning `base.points` from the
    // start for every interval. `bi` indexes the base interval `[base.points[bi],
    // base.points[bi + 1]]`; it is capped at the last interval (past-the-end x clamps
    // there), reproducing the former `base_segment_at` result exactly.
    let sigma_hard = impedance_class('H').ok_or(GisError::UnresolvableClass { class: 'H' })?;
    let mut segments = Vec::with_capacity(points.len() - 1);
    let mut bi = 0usize;
    for w in points.windows(2) {
        let mid_x = (w[0][0] + w[1][0]) / 2.0;
        let seg = if hard_spans
            .iter()
            .any(|s| mid_x > s[0] - X_EPSILON_M && mid_x < s[1] + X_EPSILON_M)
        {
            GroundSegment {
                flow_resistivity: sigma_hard,
                roughness: 0.0,
            }
        } else {
            while bi + 1 < base.segments.len() && mid_x >= base.points[bi + 1][0] {
                bi += 1;
            }
            base.segments[bi]
        };
        segments.push(seg);
    }

    Ok(GroundSegmentation {
        points,
        planar_xy,
        segments,
    })
}

/// Distance-x along the S→R line for a planar crossing point already known to lie on
/// the line (fraction clamped to `[0, 1]`).
fn crossing_to_x(p: Coord<f64>, s: [f64; 2], r: [f64; 2], x0: f64, span: f64) -> f64 {
    let denom = (r[0] - s[0]).powi(2) + (r[1] - s[1]).powi(2);
    let frac = if denom > 0.0 {
        ((p.x - s[0]) * (r[0] - s[0]) + (p.y - s[1]) * (r[1] - s[1])) / denom
    } else {
        0.0
    };
    x0 + frac.clamp(0.0, 1.0) * span
}

/// All exterior-ring ∩ S→R crossings of a building footprint as interior distance-x
/// values (strictly inside the path, endpoints excluded).
fn ring_crossings(
    sr_line: &Line<f64>,
    footprint: &Polygon<f64>,
    s: [f64; 2],
    r: [f64; 2],
    x0: f64,
    span: f64,
) -> Vec<f64> {
    let mut xs = Vec::new();
    for edge in footprint.exterior().lines() {
        if let Some(x) = line_cross_x(sr_line, edge, s, r, x0, span) {
            xs.push(x);
        }
    }
    xs
}

/// The interior distance-x of a single-point ∩ between the S→R line and `edge`, or
/// `None` for no crossing / collinear overlap / an endpoint-only touch.
fn line_cross_x(
    sr_line: &Line<f64>,
    edge: Line<f64>,
    s: [f64; 2],
    r: [f64; 2],
    x0: f64,
    span: f64,
) -> Option<f64> {
    if let Some(LineIntersection::SinglePoint { intersection, .. }) =
        line_intersection(*sr_line, edge)
    {
        let x = crossing_to_x(intersection, s, r, x0, span);
        if x > x0 + X_EPSILON_M && x < (x0 + span) - X_EPSILON_M {
            return Some(x);
        }
    }
    None
}

/// The elevated screen-top z at distance-x: ground sampled from the TIN at the
/// crossing's planar point, plus the object height. A hull miss is a typed error.
fn screen_top_z(
    tin: &Tin,
    s: [f64; 2],
    r: [f64; 2],
    x0: f64,
    span: f64,
    x: f64,
    height_m: f64,
) -> Result<f64, GisError> {
    let frac = if span > 0.0 { (x - x0) / span } else { 0.0 };
    let px = s[0] + frac * (r[0] - s[0]);
    let py = s[1] + frac * (r[1] - s[1]);
    let ground = tin.interpolate_z(px, py).ok_or(GisError::OutsideHull)?;
    Ok(ground + height_m)
}

/// Merge elevated screen-top vertices into the base `(x, z)` + planar lists, keeping
/// x strictly ascending. A screen top that lands on an existing base vertex (within
/// [`X_EPSILON_M`]) replaces it (the elevated z wins — it is the diffracting edge).
fn merge_tops(
    base: &GroundSegmentation,
    tops: &mut [[f64; 2]],
    x0: f64,
    span: f64,
    s: [f64; 2],
    r: [f64; 2],
) -> (Vec<[f64; 2]>, Vec<[f64; 2]>) {
    tops.sort_by(|a, b| a[0].total_cmp(&b[0]));

    let mut pts: Vec<[f64; 2]> = Vec::with_capacity(base.points.len() + tops.len());
    let mut planar: Vec<[f64; 2]> = Vec::with_capacity(base.points.len() + tops.len());
    let mut last_x = f64::NEG_INFINITY;
    let mut ti = 0usize;

    let push =
        |x: f64, z: f64, pts: &mut Vec<[f64; 2]>, planar: &mut Vec<[f64; 2]>, last_x: &mut f64| {
            if x > *last_x + X_EPSILON_M {
                pts.push([x, z]);
                let frac = if span > 0.0 { (x - x0) / span } else { 0.0 };
                planar.push([s[0] + frac * (r[0] - s[0]), s[1] + frac * (r[1] - s[1])]);
                *last_x = x;
            } else if let Some(last) = pts.last_mut() {
                // Coincident x: keep the higher vertex (an elevated screen top wins).
                if z > last[1] {
                    last[1] = z;
                }
            }
        };

    for (i, p) in base.points.iter().enumerate() {
        // Insert any screen tops strictly before this base vertex.
        while ti < tops.len() && tops[ti][0] < p[0] - X_EPSILON_M {
            push(tops[ti][0], tops[ti][1], &mut pts, &mut planar, &mut last_x);
            ti += 1;
        }
        push(p[0], p[1], &mut pts, &mut planar, &mut last_x);
        // Absorb a top coincident with this base vertex (raise its z).
        while ti < tops.len() && (tops[ti][0] - p[0]).abs() <= X_EPSILON_M {
            push(tops[ti][0], tops[ti][1], &mut pts, &mut planar, &mut last_x);
            ti += 1;
        }
        // Keep i live for clarity; the loop consumes every base point.
        let _ = i;
    }
    // Any remaining tops past the last base vertex (shouldn't happen — endpoints are
    // excluded — but keep x ascending defensively).
    while ti < tops.len() {
        push(tops[ti][0], tops[ti][1], &mut pts, &mut planar, &mut last_x);
        ti += 1;
    }

    (pts, planar)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::impedance::GroundSegmentation;
    use envi_engine::scene::TerrainProfile;
    use geo::{LineString, polygon};

    /// A flat plane z = 0 over `[−50, 50]²` so any crossing samples ground 0.0.
    fn flat_tin() -> Tin {
        let pts = [
            [-50.0, -50.0, 0.0],
            [50.0, -50.0, 0.0],
            [50.0, 50.0, 0.0],
            [-50.0, 50.0, 0.0],
        ];
        envi_dgm::tin::build_tin(&pts, &[]).expect("flat square builds a TIN")
    }

    /// A base cut-profile from S=(0,0) to R=(20,0) along the x-axis, flat ground 0,
    /// default class A everywhere — the shared frame for the screening tests.
    fn base_along_x() -> GroundSegmentation {
        GroundSegmentation {
            points: vec![[0.0, 0.0], [20.0, 0.0]],
            planar_xy: vec![[0.0, 0.0], [20.0, 0.0]],
            segments: vec![GroundSegment {
                flow_resistivity: impedance_class('A').unwrap(),
                roughness: 0.0,
            }],
        }
    }

    /// A footprint square straddling the x-axis over x ∈ [x0, x1].
    fn building(x0: f64, x1: f64, eaves: f64) -> ScreenObject {
        ScreenObject::Building {
            footprint: polygon![
                (x: x0, y: -2.0),
                (x: x1, y: -2.0),
                (x: x1, y: 2.0),
                (x: x0, y: 2.0),
                (x: x0, y: -2.0),
            ],
            eaves_height_m: eaves,
        }
    }

    #[test]
    fn through_building_injects_two_tops_at_eaves_height() {
        let tin = flat_tin();
        let base = base_along_x();
        let screens = vec![building(8.0, 12.0, 6.0)];
        let out = inject_screens(&base, &screens, &tin).unwrap();

        // Two entry/exit tops spliced at x = 8 and x = 12, each at ground(0)+6 = 6.
        let tops: Vec<[f64; 2]> = out
            .points
            .iter()
            .filter(|p| (p[1] - 6.0).abs() < 1e-9)
            .copied()
            .collect();
        assert_eq!(
            tops.len(),
            2,
            "a through-building yields two tops: {tops:?}"
        );
        assert!(tops.iter().any(|p| (p[0] - 8.0).abs() < 1e-9));
        assert!(tops.iter().any(|p| (p[0] - 12.0).abs() < 1e-9));
    }

    #[test]
    fn thick_span_between_tops_is_hard_ground_class_h() {
        let tin = flat_tin();
        let base = base_along_x();
        let screens = vec![building(8.0, 12.0, 6.0)];
        let out = inject_screens(&base, &screens, &tin).unwrap();

        // The interval whose midpoint is inside [8, 12] must be class H (hard).
        let sigma_h = impedance_class('H').unwrap();
        let inside = out
            .segments
            .iter()
            .zip(out.points.windows(2))
            .find(|(_, w)| {
                let mid = (w[0][0] + w[1][0]) / 2.0;
                mid > 8.0 && mid < 12.0
            })
            .map(|(s, _)| s)
            .expect("an interval inside the building");
        assert_eq!(inside.flow_resistivity, sigma_h, "screen span is hard σ");
        assert_eq!(inside.roughness, 0.0);

        // Intervals outside stay class A (inherited from the base).
        let sigma_a = impedance_class('A').unwrap();
        let outside = out
            .segments
            .iter()
            .zip(out.points.windows(2))
            .find(|(_, w)| (w[0][0] + w[1][0]) / 2.0 < 8.0)
            .map(|(s, _)| s)
            .expect("an interval before the building");
        assert_eq!(outside.flow_resistivity, sigma_a, "non-screen σ inherited");
    }

    #[test]
    fn screened_profile_is_a_valid_terrain_profile() {
        let tin = flat_tin();
        let base = base_along_x();
        let screens = vec![building(8.0, 12.0, 6.0)];
        let out = inject_screens(&base, &screens, &tin).unwrap();
        assert_eq!(
            out.segments.len(),
            out.points.len() - 1,
            "segments == points - 1"
        );
        let tp = TerrainProfile::new(out.points.clone(), out.segments.clone());
        assert!(
            tp.is_ok(),
            "screened profile builds a TerrainProfile: {tp:?}"
        );
    }

    #[test]
    fn barrier_injects_one_thin_top_no_hard_span() {
        let tin = flat_tin();
        let base = base_along_x();
        // A wall crossing the x-axis at x = 10, height 4.
        let screens = vec![ScreenObject::Barrier {
            line: LineString::from(vec![(10.0, -3.0), (10.0, 3.0)]),
            height_m: 4.0,
        }];
        let out = inject_screens(&base, &screens, &tin).unwrap();

        let tops: Vec<[f64; 2]> = out
            .points
            .iter()
            .filter(|p| (p[1] - 4.0).abs() < 1e-9)
            .copied()
            .collect();
        assert_eq!(tops.len(), 1, "a thin wall yields one top: {tops:?}");
        assert!((tops[0][0] - 10.0).abs() < 1e-9);
        // No interval is hard: a thin wall spans no width, so every σ stays A.
        let sigma_a = impedance_class('A').unwrap();
        assert!(
            out.segments.iter().all(|s| s.flow_resistivity == sigma_a),
            "a thin barrier introduces no hard span"
        );
    }

    #[test]
    fn object_outside_the_corridor_is_not_selected() {
        let tin = flat_tin();
        let base = base_along_x();
        // A building far off the path (y ≈ 40) — well outside max(20, Fresnel) corridor.
        let screens = vec![ScreenObject::Building {
            footprint: polygon![
                (x: 8.0, y: 38.0),
                (x: 12.0, y: 38.0),
                (x: 12.0, y: 42.0),
                (x: 8.0, y: 42.0),
                (x: 8.0, y: 38.0),
            ],
            eaves_height_m: 6.0,
        }];
        let out = inject_screens(&base, &screens, &tin).unwrap();
        // Nothing spliced: the profile is unchanged (still the two ground endpoints).
        assert_eq!(out.points, base.points, "off-corridor object is ignored");
    }

    #[test]
    fn crossing_outside_the_tin_hull_is_typed_error_not_zero() {
        // A tiny TIN whose hull does NOT cover the crossing at x = 10.
        let small = envi_dgm::tin::build_tin(
            &[
                [0.0, -1.0, 0.0],
                [3.0, -1.0, 0.0],
                [3.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            &[],
        )
        .unwrap();
        let base = base_along_x();
        let screens = vec![building(8.0, 12.0, 6.0)];
        let err = inject_screens(&base, &screens, &small).unwrap_err();
        assert_eq!(err, GisError::OutsideHull, "hull miss is OutsideHull");
    }

    // IN-02: the SOURCE lies inside a footprint, so the exterior-ring query yields
    // an ODD crossing count (only the exit at x = 5; the entry endpoint is
    // excluded). The interior span [x0, exit] must still be tagged hard — the old
    // `chunks_exact(2)` dropped it, leaving the ground under the building class A.
    #[test]
    fn source_inside_footprint_tags_interior_span_hard() {
        let tin = flat_tin();
        let base = base_along_x(); // S = (0,0), R = (20,0)
        // A footprint x ∈ [−3, 5] straddling the x-axis: it CONTAINS S = (0,0) and
        // the path exits it at x = 5 (a single interior crossing).
        let screens = vec![building(-3.0, 5.0, 6.0)];
        let out = inject_screens(&base, &screens, &tin).unwrap();

        let sigma_h = impedance_class('H').unwrap();
        let interior = out
            .segments
            .iter()
            .zip(out.points.windows(2))
            .find(|(_, w)| {
                let mid = (w[0][0] + w[1][0]) / 2.0;
                mid > 0.0 && mid < 5.0
            })
            .map(|(s, _)| s)
            .expect("an interval between the source and the exit crossing");
        assert_eq!(
            interior.flow_resistivity, sigma_h,
            "the span from an interior source to the footprint exit must be hard"
        );
        // Beyond the exit the ground reverts to the inherited class A.
        let sigma_a = impedance_class('A').unwrap();
        let beyond = out
            .segments
            .iter()
            .zip(out.points.windows(2))
            .find(|(_, w)| (w[0][0] + w[1][0]) / 2.0 > 5.0)
            .map(|(s, _)| s)
            .expect("an interval past the exit crossing");
        assert_eq!(beyond.flow_resistivity, sigma_a);
    }

    // WR-02: an over-large wire-supplied `screens` set is rejected BEFORE the
    // R*-tree is allocated (input bound, not just the post-query candidate count).
    #[test]
    fn oversized_screen_input_is_rejected_before_allocation() {
        let tin = flat_tin();
        let base = base_along_x();
        let screens = vec![building(8.0, 12.0, 6.0); MAX_CORRIDOR_CANDIDATES + 1];
        assert!(matches!(
            inject_screens(&base, &screens, &tin),
            Err(GisError::CorridorCandidatesExceeded { got, limit })
                if got == MAX_CORRIDOR_CANDIDATES + 1 && limit == MAX_CORRIDOR_CANDIDATES
        ));
    }

    #[test]
    fn degenerate_base_is_typed_error() {
        let tin = flat_tin();
        let one = GroundSegmentation {
            points: vec![[0.0, 0.0]],
            planar_xy: vec![[0.0, 0.0]],
            segments: vec![],
        };
        assert!(matches!(
            inject_screens(&one, &[], &tin),
            Err(GisError::DegenerateProfile { .. })
        ));
    }

    #[test]
    fn fresnel_half_width_grows_with_distance_and_has_a_floor() {
        // At 250 Hz, λ ≈ 1.36 m; r_F1 = sqrt(λ d / 4).
        let near = fresnel_half_width(10.0, CORRIDOR_REF_FREQ_HZ);
        let far = fresnel_half_width(1000.0, CORRIDOR_REF_FREQ_HZ);
        assert!(far > near, "Fresnel radius grows with path length");
        // The floor dominates for short paths.
        let half_w = near.max(CORRIDOR_MIN_HALF_WIDTH_M);
        assert_eq!(half_w, CORRIDOR_MIN_HALF_WIDTH_M);
    }
}
