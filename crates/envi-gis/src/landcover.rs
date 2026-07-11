//! WorldCover class raster → editable `ground_zone` polygon features (DATA-02).
//!
//! # Module I/O
//! - **Inputs:** a decoded [`Raster<u8>`] window of ESA WorldCover v200 class
//!   codes (from [`crate::cog`], carrying its own EPSG:4326 geotransform), a
//!   minimum polygon area (pixels²), a Douglas–Peucker simplification tolerance
//!   (pixels), and a D-11 [`Provenance`] record.
//! - **Output:** [`vectorize_landcover`] yields editable WGS84 `ground_zone`
//!   GeoJSON [`Feature`]s — one polygon per contiguous same-class region — each
//!   carrying the reviewed Nord2000 impedance **class letter** (via
//!   [`worldcover_to_class`], never a restated σ), a default roughness class
//!   [`DEFAULT_ROUGHNESS_CLASS`], and the provenance stamp. No feature `id` is
//!   assigned — TS owns UUIDs.
//! - **Invariants (load-bearing):**
//!   1. **Partition vectorization ⇒ no crossings** (threat T-08-05-01): the
//!      polygons are boundary loops of a per-class pixel partition, so any two
//!      zones are **adjacent** (share a boundary), **nested** (one contains the
//!      other via a hole), or **disjoint** — never partially crossing. This is
//!      exactly the geometry the Phase-7 draw-time crossing check accepts.
//!   2. **One source of truth for σ** (SC3): the impedance class letter comes from
//!      [`worldcover_to_class`]; the flow resistivity σ lives only in the engine
//!      (`envi_engine::scene::impedance_class`) — never restated here.
//!   3. **Bounded work** (resource-exhaustion): output is capped at
//!      [`MAX_GROUND_ZONES`] polygons and [`MAX_TOTAL_VERTICES`] vertices; regions
//!      under `min_area_px` are dropped. A malformed mask never panics or spins.
//!   4. **No silent empties** (D-07): an unknown WorldCover class code (absent from
//!      the reviewed table) and nodata holes are skipped deliberately, never
//!      mapped to a silent default zone.
//!
//! # No `contour` crate (supply-chain decision, threat T-08-05-SC)
//!
//! 08-RESEARCH flagged the `contour` crate (contour-rs) as SUS (download count
//! only). The plan gated it behind a blocking human-verify checkpoint; the
//! checkpoint was **declined in favor of this hand-rolled marching-squares
//! fallback**, so `envi-gis` gains **no new third-party dependency** — only the
//! already-present `geo` (Douglas–Peucker simplification, topology relate).
//!
//! # Reprojection (GEOX-04)
//!
//! WorldCover v200 is published in **EPSG:4326 (WGS84)**, so the raster
//! geotransform maps a pixel corner **directly** to lon/lat — the identity
//! reprojection case (mirroring the GLO-30 `Wgs84` path in [`crate::terrain`]).
//! Coordinates are routed through [`envi_geo::LonLat`]; this module contains **no
//! inline proj string** and adds no second reprojection boundary.

use std::collections::HashMap;

use geo::{Area, Contains, Coord, InteriorPoint, LineString, Polygon, Simplify};
use geojson::{Feature, Geometry, GeometryValue, JsonValue, Position};

use envi_geo::LonLat;

use crate::cog::Raster;
use crate::impedance_table::{DEFAULT_ROUGHNESS_CLASS, worldcover_to_class};
use crate::provenance::Provenance;

/// The scene `properties.kind` this module emits (one of the locked 9 kinds;
/// `envi-store/src/geojson.rs`).
pub const GROUND_ZONE_KIND: &str = "ground_zone";

/// Property key carrying the reviewed Nord2000 impedance class letter (`A..=H`),
/// the exact key `envi-store/src/geojson.rs` reads for a `ground_zone`.
pub const KEY_IMPEDANCE_CLASS: &str = "impedance_class";
/// Property key carrying the default roughness class letter for imported zones.
pub const KEY_ROUGHNESS_CLASS: &str = "roughness_class";

/// Default minimum polygon area, in **pixels²**, below which a region is dropped
/// as noise. At WorldCover's 10 m resolution, `4 px²` ≈ 400 m² — small enough to
/// keep real zones, large enough to shed single-pixel speckle (research Open-Q5,
/// discretion). Filtering in pixel space keeps the threshold CRS-independent.
pub const DEFAULT_MIN_AREA_PX: f64 = 4.0;

/// Default Douglas–Peucker tolerance in **pixels** (research: ~1–2 px). Applied in
/// pixel space (a north-up affine of the output), so the tolerance is a direct
/// pixel count rather than a degree quantity.
pub const DEFAULT_SIMPLIFY_TOL_PX: f64 = 1.5;

/// Hard cap on emitted `ground_zone` polygons per import (bounded work). A single
/// WorldCover window vectorizes to far fewer; the cap guards a pathological
/// (e.g. checkerboard) mask from exhausting memory.
pub const MAX_GROUND_ZONES: usize = 10_000;

/// Hard cap on total emitted ring vertices across all zones (bounded work). Guards
/// against an adversarial mask producing enormous rings before simplification.
pub const MAX_TOTAL_VERTICES: usize = 200_000;

/// Vectorize a WorldCover class raster window into editable `ground_zone`
/// features — one polygon per contiguous same-class region, holes preserved.
///
/// For each distinct WorldCover class code present, a binary mask is traced into
/// boundary loops (hand-rolled marching squares over the pixel partition),
/// classified into exteriors + holes by signed area, area-filtered at
/// `min_area_px`, simplified with Douglas–Peucker at `simplify_tol_px`, mapped to
/// WGS84 through the raster geotransform, and wrapped as a `ground_zone` feature
/// carrying the class's Nord2000 letter (from [`worldcover_to_class`]), a default
/// roughness class, and `provenance`. Unknown class codes and nodata are skipped.
///
/// Never panics on data and never returns an unexplained empty result — an empty
/// vector means the window held no known-class region above `min_area_px`.
#[must_use]
pub fn vectorize_landcover(
    raster: &Raster<u8>,
    min_area_px: f64,
    simplify_tol_px: f64,
    provenance: &Provenance,
) -> Vec<Feature> {
    let (w, h) = (raster.width, raster.height);
    if w == 0 || h == 0 {
        return Vec::new();
    }

    // Distinct class codes present, sorted for deterministic output ordering.
    let mut codes: Vec<u8> = raster.samples.iter().filter_map(|s| *s).collect();
    codes.sort_unstable();
    codes.dedup();

    let mut features = Vec::new();
    let mut total_vertices = 0usize;

    'classes: for code in codes {
        // Unknown class code → skip (never a silent default zone; D-07).
        let Some(letter) = worldcover_to_class(code) else {
            continue;
        };

        // Binary mask of this class over the window (nodata / other classes out).
        let mask: Vec<bool> = raster.samples.iter().map(|s| *s == Some(code)).collect();

        // Trace boundary loops, split into exteriors (area > 0) and holes.
        let loops = extract_loops(&mask, w, h);
        let mut exteriors: Vec<Vec<(i64, i64)>> = Vec::new();
        let mut holes: Vec<Vec<(i64, i64)>> = Vec::new();
        for ring in loops {
            let area = signed_area_px(&ring);
            if area.abs() < min_area_px {
                continue; // drop sub-threshold speckle (both exteriors and holes)
            }
            if area > 0.0 {
                exteriors.push(ring);
            } else {
                holes.push(ring);
            }
        }
        if exteriors.is_empty() {
            continue;
        }

        // Exterior-only polygons (pixel space) for hole containment tests.
        let ext_polys: Vec<Polygon<f64>> = exteriors
            .iter()
            .map(|r| Polygon::new(ring_to_ls_px(r), vec![]))
            .collect();

        // Assign each hole to the smallest-area exterior that contains it.
        let mut ext_holes: Vec<Vec<LineString<f64>>> = vec![Vec::new(); exteriors.len()];
        for hole in &holes {
            let Some(pt) = Polygon::new(ring_to_ls_px(hole), vec![]).interior_point() else {
                continue;
            };
            let mut chosen: Option<usize> = None;
            let mut chosen_area = f64::INFINITY;
            for (i, ep) in ext_polys.iter().enumerate() {
                if ep.contains(&pt) {
                    let a = ep.unsigned_area();
                    if a < chosen_area {
                        chosen_area = a;
                        chosen = Some(i);
                    }
                }
            }
            if let Some(i) = chosen {
                ext_holes[i].push(ring_to_ls_px(hole));
            }
        }

        // Simplify, reproject, and emit one feature per exterior.
        for (i, ext) in exteriors.iter().enumerate() {
            if features.len() >= MAX_GROUND_ZONES {
                break 'classes;
            }
            let poly = Polygon::new(ring_to_ls_px(ext), std::mem::take(&mut ext_holes[i]))
                .simplify(&simplify_tol_px);

            // A ring needs ≥ 4 positions (≥ 3 distinct + closing repeat).
            if poly.exterior().0.len() < 4 {
                continue;
            }
            let mut rings: Vec<Vec<Position>> = vec![map_ring(poly.exterior(), raster)];
            for interior in poly.interiors() {
                if interior.0.len() >= 4 {
                    rings.push(map_ring(interior, raster));
                }
            }

            let vcount: usize = rings.iter().map(Vec::len).sum();
            if total_vertices + vcount > MAX_TOTAL_VERTICES {
                break 'classes;
            }
            total_vertices += vcount;

            let mut props = provenance.clone().into_properties(GROUND_ZONE_KIND);
            props.insert(
                KEY_IMPEDANCE_CLASS.to_string(),
                JsonValue::from(letter.to_string()),
            );
            props.insert(
                KEY_ROUGHNESS_CLASS.to_string(),
                JsonValue::from(DEFAULT_ROUGHNESS_CLASS.to_string()),
            );

            features.push(Feature {
                bbox: None,
                geometry: Some(Geometry::new(GeometryValue::Polygon { coordinates: rings })),
                id: None,
                properties: Some(props),
                foreign_members: None,
            });
        }
    }

    features
}

/// Trace the boundary of a binary mask into closed pixel-corner loops.
///
/// Emits, for every in-region pixel side facing outside, a directed unit edge with
/// the interior on its right (clockwise cell traversal in the image's y-down pixel
/// space). Edges are stitched into loops; at a saddle (a corner where the region
/// pinches to a point) the sharpest right turn is taken so the loops **touch at a
/// point rather than cross**. Each returned ring is closed (first == last).
fn extract_loops(mask: &[bool], w: usize, h: usize) -> Vec<Vec<(i64, i64)>> {
    let inside = |c: i64, r: i64| -> bool {
        c >= 0
            && r >= 0
            && (c as usize) < w
            && (r as usize) < h
            && mask[(r as usize) * w + (c as usize)]
    };

    // Directed unit boundary edges, keyed by start corner.
    let mut edges: HashMap<(i64, i64), Vec<(i64, i64)>> = HashMap::new();
    for r in 0..h as i64 {
        for c in 0..w as i64 {
            if !inside(c, r) {
                continue;
            }
            let (tl, tr, br, bl) = ((c, r), (c + 1, r), (c + 1, r + 1), (c, r + 1));
            if !inside(c, r - 1) {
                edges.entry(tl).or_default().push(tr); // top: TL → TR
            }
            if !inside(c + 1, r) {
                edges.entry(tr).or_default().push(br); // right: TR → BR
            }
            if !inside(c, r + 1) {
                edges.entry(br).or_default().push(bl); // bottom: BR → BL
            }
            if !inside(c - 1, r) {
                edges.entry(bl).or_default().push(tl); // left: BL → TL
            }
        }
    }

    // Total edge count bounds the stitch work (a hard anti-spin guard).
    let edge_budget: usize = edges.values().map(Vec::len).sum::<usize>() + 1;

    let mut starts: Vec<(i64, i64)> = edges.keys().copied().collect();
    starts.sort_unstable();

    let mut loops = Vec::new();
    for s in starts {
        while edges.get(&s).is_some_and(|v| !v.is_empty()) {
            let mut ring = vec![s];
            let mut cur = s;
            let mut prev_dir: Option<(i64, i64)> = None;
            let mut steps = 0usize;
            // Follow outgoing edges until the loop closes; the empty/removed-key
            // case ends a malformed (open) boundary safely.
            while let Some(outs) = edges.get_mut(&cur) {
                if outs.is_empty() {
                    edges.remove(&cur);
                    break;
                }
                let idx = choose_outgoing(cur, outs, prev_dir);
                let next = outs.swap_remove(idx);
                if outs.is_empty() {
                    edges.remove(&cur);
                }
                ring.push(next);
                prev_dir = Some((next.0 - cur.0, next.1 - cur.1));
                cur = next;
                steps += 1;
                if cur == s || steps > edge_budget {
                    break;
                }
            }
            if ring.len() >= 4 && ring.first() == ring.last() {
                loops.push(ring);
            }
        }
    }
    loops
}

/// Pick the outgoing edge index that makes the sharpest right (clockwise) turn
/// relative to the incoming direction — the saddle-resolution rule that keeps
/// pinched loops touching at a point instead of crossing. In y-down pixel space
/// the cross product `din × dout` is positive for a clockwise turn, so the maximum
/// cross is the sharpest right turn. With no incoming direction (loop start) or a
/// single option, the first edge is taken.
fn choose_outgoing(cur: (i64, i64), outs: &[(i64, i64)], prev_dir: Option<(i64, i64)>) -> usize {
    if outs.len() == 1 {
        return 0;
    }
    let Some((dix, diy)) = prev_dir else {
        return 0;
    };
    let mut best = 0;
    let mut best_cross = i64::MIN;
    for (i, &(ex, ey)) in outs.iter().enumerate() {
        let (dox, doy) = (ex - cur.0, ey - cur.1);
        let cross = dix * doy - diy * dox;
        if cross > best_cross {
            best_cross = cross;
            best = i;
        }
    }
    best
}

/// Shoelace signed area of a closed pixel-corner ring, in pixels². Positive for an
/// exterior loop (clockwise cell traversal in y-down space), negative for a hole.
/// Accumulated in `i128` so a large window's exact area never overflows.
fn signed_area_px(ring: &[(i64, i64)]) -> f64 {
    let mut acc: i128 = 0;
    for pair in ring.windows(2) {
        let (x0, y0) = pair[0];
        let (x1, y1) = pair[1];
        acc += i128::from(x0) * i128::from(y1) - i128::from(x1) * i128::from(y0);
    }
    acc as f64 / 2.0
}

/// A closed pixel-corner ring as a `geo` [`LineString`] in pixel space.
fn ring_to_ls_px(ring: &[(i64, i64)]) -> LineString<f64> {
    LineString::from(
        ring.iter()
            .map(|&(x, y)| Coord {
                x: x as f64,
                y: y as f64,
            })
            .collect::<Vec<_>>(),
    )
}

/// Map a pixel-space ring to WGS84 GeoJSON positions through the raster
/// geotransform. WorldCover is EPSG:4326, so the geotransform yields lon/lat
/// directly (identity reprojection), routed through [`LonLat`] — no proj string.
fn map_ring(ls: &LineString<f64>, raster: &Raster<u8>) -> Vec<Position> {
    ls.0.iter()
        .map(|c| {
            let (x, y) = raster.geo.pixel_to_map(c.x, c.y);
            let ll = LonLat {
                lon_deg: x,
                lat_deg: y,
            };
            Position::from([ll.lon_deg, ll.lat_deg])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cog::geo_tags::GeoTransform;
    use geo::Relate;

    /// A WGS84 north-up geotransform: ~10 m pixels near Amsterdam.
    fn wgs84_geo() -> GeoTransform {
        GeoTransform {
            origin_x: 4.90,
            origin_y: 52.40,
            pixel_size_x: 0.0001,
            pixel_size_y: -0.0001,
        }
    }

    /// Build a `Raster<u8>` from a row-major class grid; `0` marks nodata (hole).
    fn raster(width: usize, height: usize, codes: &[u8]) -> Raster<u8> {
        assert_eq!(codes.len(), width * height);
        let samples = codes
            .iter()
            .map(|&c| if c == 0 { None } else { Some(c) })
            .collect();
        Raster {
            width,
            height,
            geo: wgs84_geo(),
            samples,
        }
    }

    fn prov() -> Provenance {
        Provenance {
            source: "esa-worldcover",
            source_ref: "N51E004".to_string(),
            license: "CC-BY-4.0",
            retrieved_at: "2026-07-10T00:00:00Z".to_string(),
            height_provenance: None,
            vertical_datum: None,
        }
    }

    fn geo_poly(f: &Feature) -> Polygon<f64> {
        let GeometryValue::Polygon { coordinates } = &f.geometry.as_ref().unwrap().value else {
            panic!("ground_zone must be a Polygon");
        };
        let ring = |r: &Vec<Position>| {
            LineString::from(
                r.iter()
                    .map(|p| Coord { x: p[0], y: p[1] })
                    .collect::<Vec<_>>(),
            )
        };
        Polygon::new(
            ring(&coordinates[0]),
            coordinates[1..].iter().map(ring).collect(),
        )
    }

    fn class_of(f: &Feature) -> &str {
        f.property(KEY_IMPEDANCE_CLASS)
            .and_then(|v| v.as_str())
            .unwrap()
    }

    #[test]
    fn adjacent_classes_vectorize_to_expected_letters_and_do_not_cross() {
        // Left half class 30 (Grassland → D), right half class 80 (Water → H).
        #[rustfmt::skip]
        let codes = vec![
            30, 30, 30, 80, 80, 80,
            30, 30, 30, 80, 80, 80,
            30, 30, 30, 80, 80, 80,
            30, 30, 30, 80, 80, 80,
        ];
        let feats =
            vectorize_landcover(&raster(6, 4, &codes), 0.0, DEFAULT_SIMPLIFY_TOL_PX, &prov());
        assert_eq!(feats.len(), 2, "one zone per class");

        let letters: std::collections::BTreeSet<&str> = feats.iter().map(class_of).collect();
        assert!(
            letters.contains("D"),
            "grassland → D via worldcover_to_class"
        );
        assert!(letters.contains("H"), "water → H via worldcover_to_class");

        // Adjacency, not a partial crossing (Phase-7 draw-time rule).
        let (a, b) = (geo_poly(&feats[0]), geo_poly(&feats[1]));
        let im = a.relate(&b);
        assert!(!im.is_overlaps(), "adjacent zones must not partially cross");
        assert!(im.is_touches(), "adjacent zones share a boundary");
    }

    #[test]
    fn enclosed_class_becomes_a_hole_plus_nested_zone_containment_not_crossing() {
        // 6×6 grassland (D) with a 2×2 tree-cover (B) block fully enclosed.
        let mut codes = vec![30u8; 36];
        for (r, c) in [(2, 2), (2, 3), (3, 2), (3, 3)] {
            codes[r * 6 + c] = 10; // Tree cover → B
        }
        let feats =
            vectorize_landcover(&raster(6, 6, &codes), 0.0, DEFAULT_SIMPLIFY_TOL_PX, &prov());
        assert_eq!(feats.len(), 2, "outer D zone + inner B zone");

        let outer = feats.iter().find(|f| class_of(f) == "D").unwrap();
        let inner = feats.iter().find(|f| class_of(f) == "B").unwrap();

        // The outer grassland carries a hole where the tree block sits.
        assert_eq!(geo_poly(outer).interiors().len(), 1, "D zone has one hole");

        // Containment/adjacency only — never a partial crossing.
        let im = geo_poly(outer).relate(&geo_poly(inner));
        assert!(!im.is_overlaps(), "nested zones must not partially cross");
    }

    #[test]
    fn every_pair_of_output_polygons_is_non_crossing() {
        // A three-class mosaic exercising several shared boundaries at once.
        #[rustfmt::skip]
        let codes = vec![
            30, 30, 80, 80,
            30, 30, 80, 80,
            50, 50, 50, 80,
            50, 50, 50, 80,
        ];
        let feats = vectorize_landcover(&raster(4, 4, &codes), 0.0, 0.0, &prov());
        assert!(feats.len() >= 3, "at least three zones");
        for i in 0..feats.len() {
            for j in (i + 1)..feats.len() {
                let im = geo_poly(&feats[i]).relate(&geo_poly(&feats[j]));
                assert!(
                    !im.is_overlaps(),
                    "zones {i} and {j} partially cross (Phase-7 rule violated)"
                );
            }
        }
    }

    #[test]
    fn impedance_class_letter_comes_from_the_table_and_no_sigma_is_stated() {
        // Built-up → G. The feature carries the letter, never a σ number.
        let feats = vectorize_landcover(&raster(3, 3, &[50; 9]), 0.0, 0.0, &prov());
        assert_eq!(feats.len(), 1);
        let f = &feats[0];
        assert_eq!(class_of(f), "G");
        assert_eq!(worldcover_to_class(50), Some('G'));
        // Roughness defaults to N; no σ / resistivity property is emitted.
        assert_eq!(
            f.property(KEY_ROUGHNESS_CLASS).and_then(|v| v.as_str()),
            Some("N")
        );
        assert!(f.property("sigma").is_none());
        assert!(f.property("resistivity").is_none());
    }

    #[test]
    fn unknown_class_and_nodata_are_skipped_never_a_silent_zone() {
        // Code 200 is not a WorldCover class; 0 is nodata. Neither yields a zone.
        let codes = vec![200u8, 200, 0, 0];
        let feats = vectorize_landcover(&raster(2, 2, &codes), 0.0, 0.0, &prov());
        assert!(feats.is_empty(), "unknown code + nodata → no zones (D-07)");
    }

    #[test]
    fn sub_threshold_speckle_is_dropped() {
        // A single grassland pixel in a nodata sea: area 1 px < 4 px min → dropped.
        let codes = vec![0u8, 0, 0, 0, 30, 0, 0, 0, 0];
        let feats = vectorize_landcover(&raster(3, 3, &codes), DEFAULT_MIN_AREA_PX, 0.0, &prov());
        assert!(feats.is_empty(), "1 px region below min-area is dropped");

        // With no threshold the same pixel survives as a zone.
        let kept = vectorize_landcover(&raster(3, 3, &codes), 0.0, 0.0, &prov());
        assert_eq!(kept.len(), 1);
    }

    #[test]
    fn features_carry_provenance_kind_and_no_rust_id() {
        let feats = vectorize_landcover(&raster(3, 3, &[30; 9]), 0.0, 0.0, &prov());
        let f = &feats[0];
        assert_eq!(
            f.property("kind").and_then(|v| v.as_str()),
            Some("ground_zone")
        );
        assert_eq!(
            f.property("source").and_then(|v| v.as_str()),
            Some("esa-worldcover")
        );
        assert_eq!(f.property("imported").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            f.property("user_modified").and_then(|v| v.as_bool()),
            Some(false)
        );
        assert!(f.id.is_none(), "no Rust-assigned UUID (TS owns ids)");

        // Coordinates are WGS84 degrees near Amsterdam.
        let GeometryValue::Polygon { coordinates } = &f.geometry.as_ref().unwrap().value else {
            panic!("Polygon expected");
        };
        for pos in &coordinates[0] {
            assert!((4.8..5.0).contains(&pos[0]), "lon in WGS84 range near NL");
            assert!((52.3..52.5).contains(&pos[1]), "lat in WGS84 range near NL");
        }
    }

    #[test]
    fn simplification_reduces_collinear_boundary_vertices() {
        // A 1×5 grassland strip: the long edges are collinear pixel-corner runs.
        // Douglas–Peucker drops the redundant midpoints; the ring stays closed.
        let ring_len = |tol: f64| -> usize {
            let feats = vectorize_landcover(&raster(5, 1, &[30; 5]), 0.0, tol, &prov());
            assert_eq!(feats.len(), 1);
            let GeometryValue::Polygon { coordinates } = &feats[0].geometry.as_ref().unwrap().value
            else {
                panic!("Polygon expected");
            };
            let ring = &coordinates[0];
            assert_eq!(ring.first(), ring.last(), "ring stays closed");
            ring.len()
        };
        let raw = ring_len(0.0);
        let simplified = ring_len(DEFAULT_SIMPLIFY_TOL_PX);
        assert!(
            simplified < raw && simplified >= 5,
            "simplify reduces {raw} → {simplified} vertices (still a valid ring)"
        );
    }

    #[test]
    fn empty_raster_yields_no_features_without_panic() {
        let empty = Raster {
            width: 0,
            height: 0,
            geo: wgs84_geo(),
            samples: Vec::new(),
        };
        assert!(vectorize_landcover(&empty, 0.0, 0.0, &prov()).is_empty());
    }
}
