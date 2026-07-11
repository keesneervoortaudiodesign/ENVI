//! Terrain decimation → WGS84 elevation features; footprint-boundary base height.
//!
//! # Module I/O
//! - **Inputs:** a decoded [`Raster<f32>`] window (from [`crate::cog`], carrying
//!   its own source-CRS geotransform), a target sample count, and the source CRS
//!   ([`TerrainSourceCrs`] — RD New for AHN, WGS84 for GLO-30).
//! - **Output:** [`decimate_window`] yields a bounded set of source-CRS
//!   [`ElevSample`]s (nodata holes dropped); [`terrain_features`] reprojects each
//!   kept sample to WGS84 lon/lat through [`envi_geo`] and wraps it as an editable
//!   `elevation_point` GeoJSON [`Feature`] — **WGS84 on the wire** per the Phase-6
//!   contract (the server converts WGS84 → SceneXY; this crate never emits
//!   project/UTM coordinates). [`sample_base_elevation`] returns a building's
//!   footprint-boundary median ground height.
//! - **Invariants (load-bearing):**
//!   1. **Bounded output** (threat T-08-04-01): the sample count is auto-decimated
//!      to `target_points`, hard-capped at [`MAX_TERRAIN_POINTS`] — well under
//!      `envi_dgm::tin::MAX_POINTS` (500k). Grid striding, not truncation, so the
//!      viewport stays evenly covered.
//!   2. **No silent 0.0** (D-07): nodata / non-finite samples are dropped as holes
//!      by the decode; [`sample_base_elevation`] returns `None` when no boundary
//!      sample exists, never a fabricated 0.0.
//!   3. **Footprint-boundary base** (threat T-08-04-04): building base elevation is
//!      the **median of samples along the footprint boundary**, never a single
//!      DSM read under the roof.
//!   4. **One reprojection boundary** (GEOX-04): RD New / WGS84 → WGS84 goes
//!      through [`envi_geo`] only — no inline proj strings here.

use geojson::{Feature, Geometry, JsonValue};

use envi_geo::{LonLat, RdNewCrs, SceneXY};

use crate::GisError;
use crate::cog::Raster;
use crate::provenance::Provenance;

/// Hard cap on decimated terrain samples per import (threat T-08-04-01). Far
/// under `envi_dgm::tin::MAX_POINTS` (500k) so the downstream TIN build is never
/// stressed; the research target is 2–10k points/import.
pub const MAX_TERRAIN_POINTS: usize = 50_000;

/// A single decimated terrain sample in the **source** CRS (RD New meters for
/// AHN, WGS84 degrees for GLO-30). Reprojected to WGS84 by [`terrain_features`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElevSample {
    /// Source-CRS x (easting meters or longitude degrees).
    pub x: f64,
    /// Source-CRS y (northing meters or latitude degrees).
    pub y: f64,
    /// Elevation, meters.
    pub z: f64,
}

/// The CRS a terrain raster's samples live in — the reprojection input side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainSourceCrs {
    /// Dutch RD New (EPSG:28992) meters — AHN.
    RdNew,
    /// WGS84 (EPSG:4326) lon/lat degrees — GLO-30 (already geographic).
    Wgs84,
}

/// Grid-stride a decoded window into a bounded set of source-CRS samples, dropping
/// nodata holes. Output count ≤ `target_points` and ≤ [`MAX_TERRAIN_POINTS`].
///
/// Spacing is auto-picked from the window size so coverage stays uniform (not a
/// prefix truncation). Sample positions are pixel centers mapped through the
/// raster's own geotransform (never nominal geometry).
#[must_use]
pub fn decimate_window(raster: &Raster<f32>, target_points: usize) -> Vec<ElevSample> {
    let (width, height) = (raster.width, raster.height);
    let total = width.saturating_mul(height);
    if total == 0 {
        return Vec::new();
    }
    let effective = target_points.clamp(1, MAX_TERRAIN_POINTS);

    // Pick a stride so the sampled grid does not exceed `effective`, guaranteeing
    // the bound BEFORE hole-dropping (which only shrinks the result further).
    let mut stride = ((total as f64 / effective as f64).sqrt().ceil() as usize).max(1);
    while stride < total {
        let nc = width.div_ceil(stride);
        let nr = height.div_ceil(stride);
        if nc.saturating_mul(nr) <= effective {
            break;
        }
        stride += 1;
    }

    let mut out = Vec::new();
    let mut row = 0;
    while row < height {
        let mut col = 0;
        while col < width {
            if let Some(z) = raster.get(col, row) {
                // Pixel center in source-CRS map coordinates.
                let (x, y) = raster.geo.pixel_to_map(col as f64 + 0.5, row as f64 + 0.5);
                out.push(ElevSample {
                    x,
                    y,
                    z: f64::from(z),
                });
            }
            col += stride;
        }
        row += stride;
    }
    out
}

/// Decimate a terrain window and build editable WGS84 `elevation_point` features.
///
/// Each kept sample is reprojected from `source_crs` to WGS84 lon/lat through
/// [`envi_geo`] (the single reprojection boundary) and wrapped as a GeoJSON
/// [`Feature`] carrying `kind = "elevation_point"`, `z_m`, and the D-11
/// provenance stamp. No feature `id` is assigned — TS owns UUIDs.
///
/// # Errors
/// [`GisError::Reproject`] if the source CRS cannot be built or a sample cannot be
/// reprojected (never panics on data).
pub fn terrain_features(
    raster: &Raster<f32>,
    target_points: usize,
    source_crs: TerrainSourceCrs,
    provenance: &Provenance,
) -> Result<Vec<Feature>, GisError> {
    let samples = decimate_window(raster, target_points);

    // Build the RD New CRS once (only needed for the AHN path).
    let rd = match source_crs {
        TerrainSourceCrs::RdNew => Some(reproject_crs()?),
        TerrainSourceCrs::Wgs84 => None,
    };

    let mut features = Vec::with_capacity(samples.len());
    for s in samples {
        let lonlat = match source_crs {
            TerrainSourceCrs::Wgs84 => LonLat {
                lon_deg: s.x,
                lat_deg: s.y,
            },
            TerrainSourceCrs::RdNew => rd
                .as_ref()
                .expect("rd built for RdNew source")
                .to_wgs84(SceneXY { x_m: s.x, y_m: s.y })
                .map_err(|e| GisError::Reproject {
                    message: e.to_string(),
                })?,
        };

        let mut props = provenance.clone().into_properties("elevation_point");
        props.insert("z_m".to_string(), JsonValue::from(s.z));
        features.push(Feature {
            bbox: None,
            geometry: Some(Geometry::new(geojson::GeometryValue::new_point([
                lonlat.lon_deg,
                lonlat.lat_deg,
            ]))),
            id: None,
            properties: Some(props),
            foreign_members: None,
        });
    }
    Ok(features)
}

/// Build the RD New CRS, mapping the CRS-construction error to [`GisError`].
fn reproject_crs() -> Result<RdNewCrs, GisError> {
    RdNewCrs::new().map_err(|e| GisError::Reproject {
        message: e.to_string(),
    })
}

/// The footprint-boundary median ground elevation for a building (threat
/// T-08-04-04): densify the exterior `ring` to ≤ `max_spacing` vertex spacing,
/// sample the terrain at each boundary point via `sample`, and return the
/// **median** of the finite samples — robust against DSM spikes from adjacent
/// trees/vehicles, and NEVER a read from under the roof.
///
/// `ring` and `sample` share the terrain CRS (RD/UTM meters). Returns `None` when
/// no boundary sample is available (never a silent 0.0, D-07).
#[must_use]
pub fn sample_base_elevation<F>(ring: &[[f64; 2]], max_spacing: f64, sample: F) -> Option<f64>
where
    F: Fn(f64, f64) -> Option<f64>,
{
    if ring.len() < 2 {
        // Degenerate ring — still try the single vertex if present.
        if let Some(p) = ring.first() {
            return sample(p[0], p[1]).filter(|z| z.is_finite());
        }
        return None;
    }

    let mut values = Vec::new();
    for pair in ring.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let len = dx.hypot(dy);
        // Sub-divisions along this segment; sample the start vertex + interiors
        // (the next segment samples its own start, so shared vertices aren't
        // double-counted).
        let n = if max_spacing.is_finite() && max_spacing > 0.0 && len.is_finite() {
            ((len / max_spacing).ceil() as usize).max(1)
        } else {
            1
        };
        for i in 0..n {
            let t = i as f64 / n as f64;
            let (x, y) = (a[0] + t * dx, a[1] + t * dy);
            if let Some(z) = sample(x, y)
                && z.is_finite()
            {
                values.push(z);
            }
        }
    }
    median(values)
}

/// Footprint-boundary median base elevation sampled from a decoded terrain
/// [`Raster<f32>`] (threat T-08-04-04) — the raster-backed adapter over
/// [`sample_base_elevation`].
///
/// `ring` shares the terrain raster's **source CRS** (RD/UTM meters for AHN,
/// WGS84 degrees for GLO-30): each boundary point is mapped to the nearest raster
/// pixel through the raster's own geotransform and read as the ground height.
/// Holes (nodata) and out-of-window points contribute no sample, so the result is
/// the median of the finite boundary reads — `None` when none exist (never a
/// fabricated `0.0`, D-07).
///
/// This lives in the core (not the WASM boundary) so the boundary stays a
/// logic-free marshaller: it decodes the window and calls this.
#[must_use]
pub fn base_elevation_on_raster(
    ring: &[[f64; 2]],
    max_spacing: f64,
    terrain: &Raster<f32>,
) -> Option<f64> {
    sample_base_elevation(ring, max_spacing, |x, y| {
        sample_raster_nearest(terrain, x, y)
    })
}

/// Nearest-pixel terrain read at source-CRS map coordinates `(x, y)` via the
/// raster's inverse geotransform. `None` for a non-finite/out-of-window point or a
/// hole. The transform is north-up affine (no rotation), so the inverse is a
/// direct division per axis.
fn sample_raster_nearest(terrain: &Raster<f32>, x: f64, y: f64) -> Option<f64> {
    let g = &terrain.geo;
    if g.pixel_size_x == 0.0 || g.pixel_size_y == 0.0 {
        return None;
    }
    let col = ((x - g.origin_x) / g.pixel_size_x).floor();
    let row = ((y - g.origin_y) / g.pixel_size_y).floor();
    if !col.is_finite() || !row.is_finite() || col < 0.0 || row < 0.0 {
        return None;
    }
    terrain.get(col as usize, row as usize).map(f64::from)
}

/// Median of a set of finite values, or `None` if empty.
fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).expect("values are finite"));
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cog::geo_tags::GeoTransform;

    /// Build a synthetic raster: `width`×`height`, all samples `z` except the
    /// cells listed in `holes` (row, col), with the given geotransform.
    fn raster(
        width: usize,
        height: usize,
        z: f32,
        holes: &[(usize, usize)],
        geo: GeoTransform,
    ) -> Raster<f32> {
        let mut samples = vec![Some(z); width * height];
        for &(r, c) in holes {
            samples[r * width + c] = None;
        }
        Raster {
            width,
            height,
            geo,
            samples,
        }
    }

    fn rd_geo() -> GeoTransform {
        // Amsterdam-ish RD New origin; 0.5 m pixels, north-up.
        GeoTransform {
            origin_x: 121_000.0,
            origin_y: 487_000.0,
            pixel_size_x: 0.5,
            pixel_size_y: -0.5,
        }
    }

    #[test]
    fn decimate_is_bounded_and_drops_nodata_holes() {
        let r = raster(100, 100, 12.0, &[(0, 0), (5, 5), (9, 9)], rd_geo());
        let full = decimate_window(&r, usize::MAX); // effective clamps to the cap
        assert!(
            full.len() <= MAX_TERRAIN_POINTS,
            "hard cap enforced: {}",
            full.len()
        );

        let target = 2000;
        let s = decimate_window(&r, target);
        assert!(s.len() <= target, "≤ target: {}", s.len());
        assert!(s.len() <= MAX_TERRAIN_POINTS, "≤ cap: {}", s.len());
        assert!(!s.is_empty(), "some samples survive");
        // Every returned sample is a finite elevation (holes never leak).
        assert!(s.iter().all(|e| e.z.is_finite()));
    }

    #[test]
    fn decimate_drops_all_holes_when_window_is_fully_nodata() {
        let holes: Vec<(usize, usize)> = (0..4).flat_map(|r| (0..4).map(move |c| (r, c))).collect();
        let r = raster(4, 4, 1.0, &holes, rd_geo());
        assert!(
            decimate_window(&r, 100).is_empty(),
            "all-nodata window yields no samples (never a silent 0.0)"
        );
    }

    #[test]
    fn terrain_features_emit_wgs84_degree_range_points() {
        let r = raster(20, 20, 10.0, &[], rd_geo());
        let prov = Provenance {
            source: "ahn4-dtm",
            source_ref: "M_19FN2".to_string(),
            license: "CC0-1.0",
            retrieved_at: "2026-07-10T00:00:00Z".to_string(),
            height_provenance: None,
            vertical_datum: Some("NAP"),
        };
        let feats =
            terrain_features(&r, 64, TerrainSourceCrs::RdNew, &prov).expect("reprojects to WGS84");
        assert!(!feats.is_empty());
        for f in &feats {
            assert_eq!(
                f.property("kind").and_then(|v| v.as_str()),
                Some("elevation_point")
            );
            // Provenance rides through as plain properties.
            assert_eq!(
                f.property("source").and_then(|v| v.as_str()),
                Some("ahn4-dtm")
            );
            assert!(f.property("z_m").and_then(|v| v.as_f64()).is_some());
            // No id assigned in Rust.
            assert!(f.id.is_none());
            // Coordinates are WGS84 degrees near Amsterdam, NOT RD/UTM meters.
            let geojson::GeometryValue::Point { coordinates } = &f.geometry.as_ref().unwrap().value
            else {
                panic!("elevation_point must be a Point");
            };
            let (lon, lat) = (coordinates[0], coordinates[1]);
            assert!(
                (3.0..8.0).contains(&lon) && (50.0..54.0).contains(&lat),
                "WGS84 degree range near NL, got ({lon}, {lat})"
            );
            assert!(lon.abs() <= 180.0 && lat.abs() <= 90.0, "valid WGS84 range");
        }
    }

    #[test]
    fn terrain_features_wgs84_source_is_identity() {
        // GLO-30 samples are already lon/lat.
        let geo = GeoTransform {
            origin_x: 4.90,
            origin_y: 52.40,
            pixel_size_x: 0.000277,
            pixel_size_y: -0.000277,
        };
        let r = raster(10, 10, 5.0, &[], geo);
        let prov = Provenance {
            source: "glo30",
            source_ref: "N52E004".to_string(),
            license: "Copernicus-Free",
            retrieved_at: "2026-07-10T00:00:00Z".to_string(),
            height_provenance: None,
            vertical_datum: Some("EGM2008"),
        };
        let feats =
            terrain_features(&r, 64, TerrainSourceCrs::Wgs84, &prov).expect("identity WGS84");
        let geojson::GeometryValue::Point { coordinates } =
            &feats[0].geometry.as_ref().unwrap().value
        else {
            panic!("Point expected");
        };
        assert!((4.9..4.91).contains(&coordinates[0]));
        assert!((52.39..52.41).contains(&coordinates[1]));
    }

    #[test]
    fn base_elevation_is_boundary_median_ignoring_dsm_spike_under_roof() {
        // Square footprint 10×10 m; a DSM spike (100 m) sits under the roof, the
        // boundary ground is 10 m. The rule must read only the boundary.
        let ring = [
            [0.0, 0.0],
            [10.0, 0.0],
            [10.0, 10.0],
            [0.0, 10.0],
            [0.0, 0.0],
        ];
        let sample = |x: f64, y: f64| -> Option<f64> {
            let interior = x > 1.0 && x < 9.0 && y > 1.0 && y < 9.0;
            Some(if interior { 100.0 } else { 10.0 })
        };
        let base = sample_base_elevation(&ring, 2.0, sample).expect("has boundary samples");
        assert!(
            (base - 10.0).abs() < 1e-9,
            "base must be the 10 m boundary median, got {base} (DSM spike leaked?)"
        );
    }

    #[test]
    fn base_elevation_returns_none_when_no_sample_available() {
        let ring = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 0.0]];
        // Terrain absent everywhere → None, never a fabricated 0.0 (D-07).
        let base = sample_base_elevation(&ring, 2.0, |_x, _y| None);
        assert_eq!(base, None);
    }

    #[test]
    fn base_elevation_on_raster_reads_the_boundary_from_a_decoded_window() {
        // A 20×20 RD raster of uniform 12 m ground. A footprint ring inside the
        // window samples the raster and yields the 12 m median; the boundary
        // adapter turns a decoded window into base elevation with no boundary logic.
        let r = raster(20, 20, 12.0, &[], rd_geo());
        // Ring in RD meters near the raster origin (0.5 m pixels from 121_000,487_000).
        let ring = [
            [121_001.0, 486_999.0],
            [121_004.0, 486_999.0],
            [121_004.0, 486_996.0],
            [121_001.0, 486_996.0],
            [121_001.0, 486_999.0],
        ];
        let base = base_elevation_on_raster(&ring, 0.5, &r).expect("boundary samples exist");
        assert!(
            (base - 12.0).abs() < 1e-9,
            "boundary median is 12 m, got {base}"
        );
    }

    #[test]
    fn base_elevation_on_raster_is_none_when_ring_is_outside_the_window() {
        // A ring far outside the raster extent samples no pixel → None (D-07).
        let r = raster(8, 8, 5.0, &[], rd_geo());
        let ring = [
            [200_000.0, 400_000.0],
            [200_010.0, 400_000.0],
            [200_010.0, 400_010.0],
            [200_000.0, 400_000.0],
        ];
        assert_eq!(base_elevation_on_raster(&ring, 1.0, &r), None);
    }

    #[test]
    fn base_elevation_median_is_robust_to_a_single_high_boundary_reading() {
        // Median (not mean) resists one anomalous boundary sample.
        let ring = [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0], [0.0, 0.0]];
        let sample = |x: f64, y: f64| -> Option<f64> {
            // One corner reads absurdly high; every other boundary point is 5 m.
            if x == 0.0 && y == 0.0 {
                Some(999.0)
            } else {
                Some(5.0)
            }
        };
        let base = sample_base_elevation(&ring, 1.0, sample).expect("samples");
        assert!(
            (base - 5.0).abs() < 1e-9,
            "median resists the outlier, got {base}"
        );
    }
}
