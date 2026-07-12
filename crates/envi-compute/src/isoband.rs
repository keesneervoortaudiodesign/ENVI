//! Hand-rolled interpolated marching-squares iso-band tracer (WEB-06, GRID-04,
//! D-02) — turns a cached [`LevelGrid`] into nested iso-band FILL polygons that
//! re-contour on a break edit WITHOUT re-running propagation (SC3, D-04).
//!
//! # Module I/O
//! - **Input:** a [`LevelGrid`] (the cached scalar dB field from
//!   [`crate::grid::reconstruct_level_grid`]) and a user-editable `breaks: &[f64]`
//!   colour-scale (validated: strictly increasing, finite, ≥ 2 — V5, threat
//!   T-11-02-01).
//! - **Output:** `Vec<IsoBand>` — one FILL region per band `[breaks[k],
//!   breaks[k+1])` (so `breaks.len() − 1` bands), each carrying its SceneXY `[x,
//!   y]` rings (exterior-then-holes per connected component, GeoJSON winding).
//!   Reprojection SceneXY → LonLat is the map layer's job (11-06). Isophones are
//!   FILL POLYGONS, never a heatmap layer (D-02).
//!
//! # Adapted from `envi-gis/landcover.rs`, NOT copied
//! `landcover.rs` traces the boundary of a **binary pixel partition** at integer
//! pixel corners — **no interpolation**. Isophones are thresholded iso-bands over
//! a **continuous scalar field**, so this tracer:
//! 1. **Linearly interpolates** the crossing point on each cell edge where the
//!    level crosses a break — smooth contours, not a staircase.
//! 2. Resolves ambiguous (saddle) cells with the **mean-value (cell-average)
//!    rule** so bands never self-cross or leak (Pitfall 6, threat T-11-02-03).
//! 3. **Pads** the field with an exterior sentinel below every break so every
//!    inside region closes (regions touching the grid border clip to the extent).
//!
//! It REUSES the landcover DNA via `geo`: signed-area/containment ring
//! classification, [`geo::Simplify`] (Douglas–Peucker), and the [`geo::Relate`]
//! non-crossing property (asserted in the tests). Bands are the annular set
//! difference of the strictly-nested threshold regions
//! (`{v ≥ breaks[k]} \ {v ≥ breaks[k+1]}`) via [`geo::BooleanOps`], so sibling
//! bands are **adjacent, never overlapping** — the D-02 non-crossing guarantee.
//!
//! # No `contour` crate (supply-chain decision, RESEARCH Package Legitimacy)
//! `contour`/`contour-isobands` are SUS (797/16 weekly downloads) and were
//! already declined in Phase 8. They ARE the reference algorithm (a d3-contour
//! port) — ideas ported, dep NOT added. `gdal`'s `GDALContourGenerateEx` is a
//! documented escape hatch only (server-side, D-01 tension) reserved for a failed
//! 100k-cell benchmark — the perf test keeps it unused.
//!
//! `#![deny(unsafe_code)]` (crate-wide); typed errors on bad breaks, never a panic.

use std::collections::HashMap;

use geo::algorithm::orient::{Direction, Orient};
use geo::{
    Area, BooleanOps, Contains, Coord, InteriorPoint, LineString, MultiPolygon, Point, Polygon,
    Simplify,
};
use thiserror::Error;

use crate::grid::LevelGrid;

/// Douglas–Peucker tolerance as a fraction of the grid spacing. Small enough to
/// drop only near-collinear vertices (straight band edges keep their exact
/// interpolated crossings), large enough to shed marching-squares stair-steps on
/// curved contours. Applied in SceneXY meters.
const SIMPLIFY_EPS_FRAC: f64 = 0.25;

/// A user-editable colour-scale: the strictly-increasing dB break edges that are
/// the SINGLE source of truth for `legend ≡ contour ≡ class colours` (D-04). The
/// map layer's legend and the tracer read the same `breaks`.
#[derive(Debug, Clone, PartialEq)]
pub struct BreakScale {
    /// Strictly-increasing, finite break edges (≥ 2).
    pub breaks: Vec<f64>,
}

impl BreakScale {
    /// Build a validated break scale (V5: strictly increasing, finite, ≥ 2).
    ///
    /// # Errors
    /// Returns [`IsobandError`] for fewer than two breaks, a non-finite value, or
    /// a non-monotonic sequence — never panics on user input (threat T-11-02-01).
    pub fn new(breaks: Vec<f64>) -> Result<Self, IsobandError> {
        validate_breaks(&breaks)?;
        Ok(Self { breaks })
    }
}

/// One iso-band fill region for the value interval `[lower, upper)`.
///
/// `rings` are SceneXY `[x, y]` polygon rings following GeoJSON winding per
/// connected component: each exterior ring is followed by its hole rings. A band
/// may hold several disconnected components (e.g. multiple peaks above `lower`).
#[derive(Debug, Clone, PartialEq)]
pub struct IsoBand {
    /// Inclusive lower break of the band.
    pub lower: f64,
    /// Exclusive upper break of the band.
    pub upper: f64,
    /// SceneXY fill rings (exterior-then-holes per component).
    pub rings: Vec<Vec<[f64; 2]>>,
}

/// One classified fill polygon: an exterior ring plus its hole rings, SceneXY
/// `[x, y]`. The [`IsoBand::fill_polygons`] grouping of a band's flat rings.
pub type FillPolygon = (Vec<[f64; 2]>, Vec<Vec<[f64; 2]>>);

impl IsoBand {
    /// Group this band's flat [`Self::rings`] into `(exterior, holes)` fill
    /// polygons via the SAME signed-area/containment classification the tracer
    /// uses internally ([`rings_to_multipolygon`]).
    ///
    /// [`Self::rings`] is a flat exterior-then-holes list across possibly several
    /// disconnected components; this reconstructs which holes belong to which
    /// exterior so an exporter (GeoJSON `MultiPolygon`, GRID-05) gets correct
    /// polygon/hole nesting rather than treating every ring as a solid fill. Rings
    /// are SceneXY `[x, y]`; the caller reprojects to lon/lat at the one CRS seam.
    #[must_use]
    pub fn fill_polygons(&self) -> Vec<FillPolygon> {
        let mp = rings_to_multipolygon(self.rings.clone());
        mp.0.iter()
            .map(|poly| {
                let exterior = ls_to_ring(poly.exterior());
                let holes = poly.interiors().iter().map(ls_to_ring).collect();
                (exterior, holes)
            })
            .collect()
    }
}

/// A break-scale validation error (V5) — the tracer never panics on user input.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum IsobandError {
    /// Fewer than two break edges (no band can be formed).
    #[error("break scale needs at least 2 values, got {0}")]
    TooFewBreaks(usize),
    /// A break value is not finite (NaN/±∞).
    #[error("break value at index {0} is not finite")]
    NonFinite(usize),
    /// The break sequence is not strictly increasing.
    #[error("breaks must be strictly increasing: breaks[{lo_i}]={lo} >= breaks[{hi_i}]={hi}")]
    NotMonotonic {
        /// Index of the lower (earlier) break.
        lo_i: usize,
        /// Value at `lo_i`.
        lo: f64,
        /// Index of the higher (later) break.
        hi_i: usize,
        /// Value at `hi_i`.
        hi: f64,
    },
}

/// Validate a break scale: ≥ 2 values, all finite, strictly increasing (V5).
fn validate_breaks(breaks: &[f64]) -> Result<(), IsobandError> {
    if breaks.len() < 2 {
        return Err(IsobandError::TooFewBreaks(breaks.len()));
    }
    for (i, &b) in breaks.iter().enumerate() {
        if !b.is_finite() {
            return Err(IsobandError::NonFinite(i));
        }
    }
    for i in 1..breaks.len() {
        if breaks[i] <= breaks[i - 1] {
            return Err(IsobandError::NotMonotonic {
                lo_i: i - 1,
                lo: breaks[i - 1],
                hi_i: i,
                hi: breaks[i],
            });
        }
    }
    Ok(())
}

/// Trace nested iso-band FILL polygons over a cached level grid (D-02, SC3).
///
/// Produces `breaks.len() − 1` [`IsoBand`]s, band `k` covering `[breaks[k],
/// breaks[k+1])` as the annular difference of the strictly-nested threshold
/// regions. Re-runnable on a break edit with no re-solve (the grid is the cache).
///
/// # Errors
/// Returns [`IsobandError`] if `breaks` is not a valid scale (V5). An empty grid
/// yields an empty band list (not an error).
pub fn trace_isobands(grid: &LevelGrid, breaks: &[f64]) -> Result<Vec<IsoBand>, IsobandError> {
    validate_breaks(breaks)?;
    if grid.is_empty() {
        return Ok(Vec::new());
    }

    // Exterior sentinel strictly below every break AND every finite grid value, so
    // out-of-grid pad nodes and NaN holes are "outside" for every threshold.
    let mut min_v = f64::INFINITY;
    for &v in &grid.values {
        if v.is_finite() {
            min_v = min_v.min(v);
        }
    }
    let base = if min_v.is_finite() {
        min_v.min(breaks[0])
    } else {
        breaks[0]
    };
    let pad = base - 1.0;
    let eps = grid.spacing_m * SIMPLIFY_EPS_FRAC;

    // One strictly-nested region {v ≥ breaks[k]} per break.
    let regions: Vec<MultiPolygon<f64>> = breaks
        .iter()
        .map(|&t| region_multipolygon(grid, t, pad))
        .collect();

    // Band k = region(k) \ region(k+1) — an annular, non-overlapping partition.
    let mut bands = Vec::with_capacity(breaks.len().saturating_sub(1));
    for k in 0..breaks.len() - 1 {
        let band_mp = regions[k].difference(&regions[k + 1]);
        let simplified = band_mp.simplify(&eps);
        let mut rings: Vec<Vec<[f64; 2]>> = Vec::new();
        for poly in &simplified.0 {
            rings.push(ls_to_ring(poly.exterior()));
            for hole in poly.interiors() {
                rings.push(ls_to_ring(hole));
            }
        }
        bands.push(IsoBand {
            lower: breaks[k],
            upper: breaks[k + 1],
            rings,
        });
    }
    Ok(bands)
}

/// Edge identity: `(kind, row, col)` where `kind = 0` is the horizontal edge
/// between node `(row, col)` and `(row, col+1)`, `kind = 1` the vertical edge
/// between `(row, col)` and `(row+1, col)`. `row`/`col` span the padded lattice
/// (`-1 ..= rows` / `-1 ..= cols`), so a crossing computed from either adjacent
/// cell yields the SAME key and identical coordinates — the exact-integer stitch.
type EdgeId = (u8, i64, i64);

/// Trace the region `{v ≥ t}` into a `geo` [`MultiPolygon`] via interpolated
/// marching squares over the pad-bordered lattice.
fn region_multipolygon(grid: &LevelGrid, t: f64, pad: f64) -> MultiPolygon<f64> {
    let node_value = |row: i64, col: i64| -> f64 {
        if row >= 0 && col >= 0 && (row as usize) < grid.rows && (col as usize) < grid.cols {
            let v = grid.values[row as usize * grid.cols + col as usize];
            if v.is_finite() { v } else { pad }
        } else {
            pad
        }
    };
    let node_pos = |row: i64, col: i64| -> [f64; 2] {
        [
            grid.origin[0] + col as f64 * grid.spacing_m,
            grid.origin[1] + row as f64 * grid.spacing_m,
        ]
    };
    // A "pad" node is out-of-grid (border padding) or a NaN no-data hole.
    let is_pad = |row: i64, col: i64| -> bool {
        !(row >= 0
            && col >= 0
            && (row as usize) < grid.rows
            && (col as usize) < grid.cols
            && grid.values[row as usize * grid.cols + col as usize].is_finite())
    };
    // Interpolated crossing point on an edge between two nodes at level `t`.
    //
    // When one endpoint is a pad/hole node the crossing is CLIPPED to the real
    // node's position (not interpolated toward the sentinel), so every threshold
    // region clips to the SAME grid-border / hole-edge nodes. Without it the
    // per-threshold padding overshoot differs and the band difference leaves a
    // spurious sliver at the grid edge (region(k+1) becomes an interior hole
    // rather than sharing region(k)'s border).
    let cross = |ra: i64, ca: i64, rb: i64, cb: i64| -> [f64; 2] {
        let a_pad = is_pad(ra, ca);
        let b_pad = is_pad(rb, cb);
        if a_pad && !b_pad {
            return node_pos(rb, cb);
        }
        if b_pad && !a_pad {
            return node_pos(ra, ca);
        }
        let va = node_value(ra, ca);
        let vb = node_value(rb, cb);
        let denom = vb - va;
        let frac = if denom.abs() > f64::MIN_POSITIVE {
            ((t - va) / denom).clamp(0.0, 1.0)
        } else {
            0.5
        };
        let pa = node_pos(ra, ca);
        let pb = node_pos(rb, cb);
        [
            pa[0] + frac * (pb[0] - pa[0]),
            pa[1] + frac * (pb[1] - pa[1]),
        ]
    };

    let mut coords: HashMap<EdgeId, [f64; 2]> = HashMap::new();
    let mut adjacency: HashMap<EdgeId, Vec<EdgeId>> = HashMap::new();

    let rows = grid.rows as i64;
    let cols = grid.cols as i64;
    // Padded cell range: every crossing edge is shared by exactly two cells.
    for r in -1..rows {
        for c in -1..cols {
            let inside = |row: i64, col: i64| node_value(row, col) >= t;
            let bl = inside(r, c);
            let br = inside(r, c + 1);
            let tr = inside(r + 1, c + 1);
            let tl = inside(r + 1, c);

            // Edge ids + lazily-computed crossing points for the four sides.
            let bottom = (0u8, r, c); // node(r,c)   – node(r,c+1)
            let top = (0u8, r + 1, c); // node(r+1,c) – node(r+1,c+1)
            let left = (1u8, r, c); // node(r,c)   – node(r+1,c)
            let right = (1u8, r, c + 1); // node(r,c+1) – node(r+1,c+1)

            let b_cross = bl != br;
            let r_cross = br != tr;
            let t_cross = tr != tl;
            let l_cross = tl != bl;
            let count = b_cross as u8 + r_cross as u8 + t_cross as u8 + l_cross as u8;
            if count == 0 {
                continue;
            }

            // Materialize the crossing coordinate for each crossing edge (shared
            // edges resolve identically from both cells).
            if b_cross {
                coords
                    .entry(bottom)
                    .or_insert_with(|| cross(r, c, r, c + 1));
            }
            if t_cross {
                coords
                    .entry(top)
                    .or_insert_with(|| cross(r + 1, c, r + 1, c + 1));
            }
            if l_cross {
                coords.entry(left).or_insert_with(|| cross(r, c, r + 1, c));
            }
            if r_cross {
                coords
                    .entry(right)
                    .or_insert_with(|| cross(r, c + 1, r + 1, c + 1));
            }

            let mut link = |a: EdgeId, b: EdgeId| {
                adjacency.entry(a).or_default().push(b);
                adjacency.entry(b).or_default().push(a);
            };

            if count == 2 {
                // Exactly one segment joining the two crossing edges.
                let mut ends = [bottom; 2];
                let mut n = 0;
                for (hit, id) in [
                    (b_cross, bottom),
                    (r_cross, right),
                    (t_cross, top),
                    (l_cross, left),
                ] {
                    if hit {
                        ends[n] = id;
                        n += 1;
                    }
                }
                link(ends[0], ends[1]);
            } else {
                // Saddle (all four edges cross). Pair by the mean-value rule so the
                // bands touch at a point instead of crossing (Pitfall 6).
                let center = (node_value(r, c)
                    + node_value(r, c + 1)
                    + node_value(r + 1, c + 1)
                    + node_value(r + 1, c))
                    / 4.0;
                let center_inside = center >= t;
                // case5 = bl,tr inside; case10 = br,tl inside.
                let case5 = bl && tr;
                let (p0, p1) = if case5 == center_inside {
                    // bl,tr inside & center inside  → outside corners br,tl split
                    // br,tl inside & center outside → inside corners br,tl split
                    ((bottom, right), (top, left))
                } else {
                    ((bottom, left), (right, top))
                };
                link(p0.0, p0.1);
                link(p1.0, p1.1);
            }
        }
    }

    // Stitch the degree-2 segment graph into closed rings. Orient to the geo
    // convention (exterior CCW, holes CW) — the marching stitch has no fixed
    // winding, and `BooleanOps` (i_overlay) reads winding as the fill rule.
    let rings = trace_rings(&adjacency, &coords);
    rings_to_multipolygon(rings).orient(Direction::Default)
}

/// Walk the (everywhere degree-2) segment graph into closed rings of coordinates.
fn trace_rings(
    adjacency: &HashMap<EdgeId, Vec<EdgeId>>,
    coords: &HashMap<EdgeId, [f64; 2]>,
) -> Vec<Vec<[f64; 2]>> {
    let mut starts: Vec<EdgeId> = adjacency.keys().copied().collect();
    starts.sort_unstable();

    let mut visited: std::collections::HashSet<EdgeId> = std::collections::HashSet::new();
    let mut rings = Vec::new();
    let budget = adjacency.len() + 1;

    for start in starts {
        if visited.contains(&start) {
            continue;
        }
        let mut ring_keys: Vec<EdgeId> = Vec::new();
        let mut cur = start;
        let mut prev: Option<EdgeId> = None;
        loop {
            visited.insert(cur);
            ring_keys.push(cur);
            let nbrs = &adjacency[&cur];
            let next = match (nbrs.len(), prev) {
                (1, _) => nbrs[0],
                (_, Some(p)) if nbrs[0] == p => nbrs[1],
                _ => nbrs[0],
            };
            if next == start {
                break;
            }
            if visited.contains(&next) || ring_keys.len() > budget {
                break;
            }
            prev = Some(cur);
            cur = next;
        }
        if ring_keys.len() >= 3 {
            let mut coord_ring: Vec<[f64; 2]> = ring_keys.iter().map(|k| coords[k]).collect();
            coord_ring.push(coord_ring[0]); // close the ring
            rings.push(coord_ring);
        }
    }
    rings
}

/// Classify closed rings into exterior/hole polygons by even/odd containment
/// nesting depth (direction-independent — the marching stitch has no fixed
/// winding), assigning each hole to the smallest-area exterior that contains it
/// (the `landcover.rs` pattern). Returns a `geo` [`MultiPolygon`].
fn rings_to_multipolygon(rings: Vec<Vec<[f64; 2]>>) -> MultiPolygon<f64> {
    let polys: Vec<Polygon<f64>> = rings
        .iter()
        .filter(|r| r.len() >= 4)
        .map(|r| Polygon::new(ring_to_ls(r), vec![]))
        .collect();
    if polys.is_empty() {
        return MultiPolygon::new(Vec::new());
    }

    // A strictly-interior representative point per ring.
    let pts: Vec<Option<Point<f64>>> = polys.iter().map(InteriorPoint::interior_point).collect();

    // Nesting depth = number of OTHER rings that contain this ring's point.
    let depth: Vec<usize> = (0..polys.len())
        .map(|i| {
            let Some(pt) = pts[i] else { return 0 };
            (0..polys.len())
                .filter(|&j| j != i && polys[j].contains(&pt))
                .count()
        })
        .collect();

    // Even depth → exterior; odd depth → hole.
    let mut exteriors: Vec<usize> = Vec::new();
    let mut holes: Vec<usize> = Vec::new();
    for (i, &d) in depth.iter().enumerate() {
        if pts[i].is_none() {
            continue; // degenerate ring — drop
        }
        if d % 2 == 0 {
            exteriors.push(i);
        } else {
            holes.push(i);
        }
    }

    let mut ext_holes: HashMap<usize, Vec<LineString<f64>>> = HashMap::new();
    for &h in &holes {
        let Some(pt) = pts[h] else { continue };
        // Smallest-area exterior that contains this hole's interior point.
        let parent = exteriors
            .iter()
            .filter(|&&e| polys[e].contains(&pt))
            .min_by(|&&a, &&b| {
                polys[a]
                    .unsigned_area()
                    .partial_cmp(&polys[b].unsigned_area())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied();
        if let Some(e) = parent {
            ext_holes
                .entry(e)
                .or_default()
                .push(polys[h].exterior().clone());
        }
    }

    let out: Vec<Polygon<f64>> = exteriors
        .iter()
        .map(|&e| {
            Polygon::new(
                polys[e].exterior().clone(),
                ext_holes.remove(&e).unwrap_or_default(),
            )
        })
        .collect();
    MultiPolygon::new(out)
}

/// A closed coordinate ring as a `geo` [`LineString`].
fn ring_to_ls(ring: &[[f64; 2]]) -> LineString<f64> {
    LineString::from(
        ring.iter()
            .map(|&[x, y]| Coord { x, y })
            .collect::<Vec<_>>(),
    )
}

/// A `geo` [`LineString`] back to a `[x, y]` coordinate ring.
fn ls_to_ring(ls: &LineString<f64>) -> Vec<[f64; 2]> {
    ls.0.iter().map(|c| [c.x, c.y]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::Relate;

    /// Build a `LevelGrid` from a row-major value slice.
    fn grid(rows: usize, cols: usize, spacing: f64, values: Vec<f64>) -> LevelGrid {
        assert_eq!(values.len(), rows * cols);
        LevelGrid {
            rows,
            cols,
            origin: [0.0, 0.0],
            spacing_m: spacing,
            values,
        }
    }

    /// Reassemble a band's rings into a `geo` MultiPolygon for topology asserts.
    fn band_mp(band: &IsoBand) -> MultiPolygon<f64> {
        rings_to_multipolygon(band.rings.clone())
    }

    #[test]
    fn ramp_grid_produces_expected_band_count_and_interpolated_crossings() {
        // value = col along x (0..10); a linear ramp → exact interpolated crossings.
        let cols = 11;
        let rows = 3;
        let mut values = Vec::with_capacity(rows * cols);
        for _r in 0..rows {
            for c in 0..cols {
                values.push(c as f64);
            }
        }
        let g = grid(rows, cols, 1.0, values);
        let breaks = [2.0, 5.0, 8.0];
        let bands = trace_isobands(&g, &breaks).unwrap();

        // breaks.len() - 1 = 2 bands, labelled [2,5) and [5,8).
        assert_eq!(bands.len(), 2);
        assert_eq!((bands[0].lower, bands[0].upper), (2.0, 5.0));
        assert_eq!((bands[1].lower, bands[1].upper), (5.0, 8.0));

        // Band [2,5) is the strip x ∈ [2,5]; the crossings are exact on the ramp.
        let xrange = |band: &IsoBand| -> (f64, f64) {
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for ring in &band.rings {
                for p in ring {
                    lo = lo.min(p[0]);
                    hi = hi.max(p[0]);
                }
            }
            (lo, hi)
        };
        let (lo0, hi0) = xrange(&bands[0]);
        assert!(
            (lo0 - 2.0).abs() < 1e-6,
            "band0 left crossing at x=2, got {lo0}"
        );
        assert!(
            (hi0 - 5.0).abs() < 1e-6,
            "band0 right crossing at x=5, got {hi0}"
        );
        let (lo1, hi1) = xrange(&bands[1]);
        assert!(
            (lo1 - 5.0).abs() < 1e-6,
            "band1 left crossing at x=5, got {lo1}"
        );
        assert!(
            (hi1 - 8.0).abs() < 1e-6,
            "band1 right crossing at x=8, got {hi1}"
        );
    }

    #[test]
    fn peak_grid_bands_are_nested_non_crossing_and_shrink_inward() {
        // A single central peak → concentric bands. Higher band sits in the lower
        // band's hole: they TOUCH, never overlap, and the inner band is smaller.
        let n = 9;
        let mut values = Vec::with_capacity(n * n);
        for r in 0..n {
            for c in 0..n {
                let dr = r as f64 - 4.0;
                let dc = c as f64 - 4.0;
                values.push(20.0 - (dr * dr + dc * dc).sqrt() * 2.0);
            }
        }
        let g = grid(n, n, 1.0, values);
        let bands = trace_isobands(&g, &[6.0, 10.0, 14.0]).unwrap();
        assert_eq!(bands.len(), 2);
        assert!(!bands[0].rings.is_empty() && !bands[1].rings.is_empty());

        // Non-crossing (the landcover.rs Relate property) + inward shrink.
        let a = band_mp(&bands[0]);
        let b = band_mp(&bands[1]);
        assert!(
            !a.relate(&b).is_overlaps(),
            "sibling bands must not overlap"
        );
        assert!(
            b.unsigned_area() < a.unsigned_area(),
            "higher band is smaller (nested inward)"
        );

        // Every ring is closed.
        for band in &bands {
            for ring in &band.rings {
                assert!(ring.len() >= 4);
                assert_eq!(ring.first(), ring.last(), "ring closed");
            }
        }
    }

    #[test]
    fn saddle_grid_traces_non_crossing_bands() {
        // Four peaks with a central saddle — exercises the mean-value saddle rule.
        #[rustfmt::skip]
        let values = vec![
            1.0, 1.0, 1.0, 1.0, 1.0,
            1.0, 9.0, 1.0, 9.0, 1.0,
            1.0, 1.0, 1.0, 1.0, 1.0,
            1.0, 9.0, 1.0, 9.0, 1.0,
            1.0, 1.0, 1.0, 1.0, 1.0,
        ];
        let g = grid(5, 5, 1.0, values);
        let bands = trace_isobands(&g, &[3.0, 6.0]).unwrap();
        assert_eq!(bands.len(), 1);
        // A saddle must not make the single band self-cross (valid, non-empty).
        let mp = band_mp(&bands[0]);
        assert!(mp.unsigned_area() > 0.0, "saddle band has positive area");
        // Its own components never partially overlap (no self-crossing/leak).
        for i in 0..mp.0.len() {
            for j in (i + 1)..mp.0.len() {
                assert!(
                    !mp.0[i].relate(&mp.0[j]).is_overlaps(),
                    "saddle components {i},{j} must not overlap"
                );
            }
        }
    }

    #[test]
    fn invalid_breaks_return_typed_error_never_panic() {
        let g = grid(3, 3, 1.0, vec![0.0; 9]);
        // < 2 breaks.
        assert_eq!(
            trace_isobands(&g, &[5.0]).unwrap_err(),
            IsobandError::TooFewBreaks(1)
        );
        assert_eq!(
            trace_isobands(&g, &[]).unwrap_err(),
            IsobandError::TooFewBreaks(0)
        );
        // Non-monotonic.
        assert!(matches!(
            trace_isobands(&g, &[5.0, 5.0]).unwrap_err(),
            IsobandError::NotMonotonic { .. }
        ));
        assert!(matches!(
            trace_isobands(&g, &[8.0, 3.0]).unwrap_err(),
            IsobandError::NotMonotonic { .. }
        ));
        // Non-finite.
        assert_eq!(
            trace_isobands(&g, &[f64::NAN, 5.0]).unwrap_err(),
            IsobandError::NonFinite(0)
        );
        assert!(matches!(
            trace_isobands(&g, &[1.0, f64::INFINITY]).unwrap_err(),
            IsobandError::NonFinite(1)
        ));
        // BreakScale::new applies the same gate.
        assert!(BreakScale::new(vec![1.0, 2.0, 3.0]).is_ok());
        assert!(BreakScale::new(vec![3.0, 2.0]).is_err());
    }

    #[test]
    fn empty_grid_yields_no_bands_without_panic() {
        let g = LevelGrid::empty(10.0);
        assert!(trace_isobands(&g, &[1.0, 2.0]).unwrap().is_empty());
    }

    #[test]
    fn perf_100k_cells_ten_breaks_under_budget() {
        // 316×316 (~100k cells) × 10 breaks — the roadmap-flagged perf check. The
        // gdal escape hatch stays unused (D-02).
        let n = 316;
        let mut values = Vec::with_capacity(n * n);
        for r in 0..n {
            for c in 0..n {
                let dr = r as f64 - 158.0;
                let dc = c as f64 - 158.0;
                values.push(120.0 - (dr * dr + dc * dc).sqrt() * 0.3);
            }
        }
        let g = grid(n, n, 1.0, values);
        let breaks: Vec<f64> = (0..10).map(|k| 40.0 + k as f64 * 8.0).collect();

        let start = std::time::Instant::now();
        let bands = trace_isobands(&g, &breaks).unwrap();
        let elapsed = start.elapsed();
        eprintln!(
            "isoband 316x316 x10 breaks: {} bands in {:?}",
            bands.len(),
            elapsed
        );
        assert_eq!(bands.len(), 9);
        // Release meets the <100 ms roadmap budget; debug (cargo test default,
        // HashMap-heavy) gets generous headroom while still proving no runaway.
        let budget_ms = if cfg!(debug_assertions) { 5000 } else { 100 };
        assert!(
            elapsed.as_millis() < budget_ms,
            "trace took {elapsed:?} (budget {budget_ms} ms)"
        );
    }
}
