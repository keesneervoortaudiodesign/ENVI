//! The reprojection transforms: WGS84 lon/lat <-> project-local UTM meters.
//!
//! This is the one place in the milestone where `proj4rs` is actually driven
//! (GEOX-04, D-03). [`ProjectCrs::to_utm`] and [`ProjectCrs::to_wgs84`] are the
//! functions the D-08 startup self-check and the SC3 landmark accuracy criterion
//! ride on.
//!
//! # Convention
//!
//! proj4rs longlat coordinates are in **RADIANS**, not degrees (06-RESEARCH
//! Pitfall 1). Degrees are converted to radians on entry and radians back to
//! degrees on exit **inside this module and ONLY here** — the crate's public API
//! ([`LonLat`] in degrees, [`SceneXY`] in meters) never exposes a radian. Feeding
//! degrees straight into proj4rs would silently produce coordinates ~57x off, so
//! the conversion is load-bearing, not cosmetic.
//!
//! # SC3 loud rejection (threat T-06-01-01)
//!
//! [`ProjectCrs::to_wgs84`] rejects **degree-magnitude** scene coordinates
//! (`|x| <= 360` AND `|y| <= 90` m) with a typed [`GeoError`] *before* any
//! transform: such values are almost certainly WGS84 degrees mislabeled as scene
//! meters, and reprojecting them would yield plausible-looking garbage. Valid UTM
//! eastings are ~166_000..834_000 m and northings 0..10_000_000 m — many orders
//! of magnitude above the guard, so real scene coordinates are never rejected.

use crate::crs::proj_err;
use crate::{GeoError, LonLat, ProjectCrs, SceneXY};

impl ProjectCrs {
    /// Project a WGS84 lon/lat (degrees) into project-local UTM meters.
    ///
    /// Rejects non-finite and out-of-range inputs with typed errors **before**
    /// proj4rs sees them (threat T-06-01-02: never panics on data).
    pub fn to_utm(&self, p: LonLat) -> Result<SceneXY, GeoError> {
        if !p.lon_deg.is_finite() || !p.lat_deg.is_finite() {
            return Err(GeoError::NonFinite {
                what: format!("lon/lat = ({}, {})", p.lon_deg, p.lat_deg),
            });
        }
        if !(-180.0..=180.0).contains(&p.lon_deg) || !(-90.0..=90.0).contains(&p.lat_deg) {
            return Err(GeoError::LonLatOutOfRange {
                lon: p.lon_deg,
                lat: p.lat_deg,
            });
        }
        // UTM is undefined outside [-80, 84]° latitude (LOW-1) — reject before
        // proj4rs silently distorts near the poles.
        if !(crate::crs::UTM_LAT_MIN..=crate::crs::UTM_LAT_MAX).contains(&p.lat_deg) {
            return Err(GeoError::LatitudeOutsideUtm { lat: p.lat_deg });
        }
        // proj4rs longlat is RADIANS — converted here and ONLY here (Pitfall 1).
        let mut pt = (p.lon_deg.to_radians(), p.lat_deg.to_radians(), 0.0);
        proj4rs::transform::transform(&self.wgs84, &self.proj, &mut pt).map_err(proj_err)?;
        // proj4rs UTM output is already meters.
        Ok(SceneXY {
            x_m: pt.0,
            y_m: pt.1,
        })
    }

    /// Project project-local UTM meters back to a WGS84 lon/lat (degrees).
    ///
    /// Rejects non-finite and **degree-magnitude** inputs with typed errors
    /// before any transform (SC3 loud rejection; threat T-06-01-01).
    pub fn to_wgs84(&self, p: SceneXY) -> Result<LonLat, GeoError> {
        if !p.x_m.is_finite() || !p.y_m.is_finite() {
            return Err(GeoError::NonFinite {
                what: format!("scene x/y = ({}, {})", p.x_m, p.y_m),
            });
        }
        // SC3: degree-magnitude values are NOT scene meters — reject loudly
        // BEFORE reaching proj4rs so garbage never round-trips as plausible.
        if p.x_m.abs() <= 360.0 && p.y_m.abs() <= 90.0 {
            return Err(GeoError::DegreeMagnitudeSceneCoord { x: p.x_m, y: p.y_m });
        }
        let mut pt = (p.x_m, p.y_m, 0.0);
        proj4rs::transform::transform(&self.proj, &self.wgs84, &mut pt).map_err(proj_err)?;
        // proj4rs longlat is RADIANS — converted back to degrees here and ONLY here.
        Ok(LonLat {
            lon_deg: pt.0.to_degrees(),
            lat_deg: pt.1.to_degrees(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Great-circle-ish meter error between two lon/lat points (small-angle,
    /// mirrors the D-08 self-check math in 06-RESEARCH Pattern 6).
    fn error_m(a: LonLat, b: LonLat) -> f64 {
        let dlat_m = (a.lat_deg - b.lat_deg).abs() * 111_320.0;
        let dlon_m = (a.lon_deg - b.lon_deg).abs() * 111_320.0 * a.lat_deg.to_radians().cos();
        (dlat_m.powi(2) + dlon_m.powi(2)).sqrt()
    }

    #[test]
    fn landmark_round_trip_within_one_meter() {
        // Dam Square, Amsterdam -> UTM 31N (D-08 landmark).
        let dam = LonLat {
            lon_deg: 4.8936,
            lat_deg: 52.3731,
        };
        let crs = ProjectCrs::for_location(dam).expect("valid landmark");
        let utm = crs.to_utm(dam).expect("forward transform");
        let back = crs.to_wgs84(utm).expect("inverse transform");
        let err = error_m(dam, back);
        // SC3 criterion is <= 1 m; etmerc has ~3 orders of headroom, so also
        // assert <= 1e-3 m so a silent accuracy regression is loud.
        assert!(err <= 1.0, "round-trip error {err} m > 1 m");
        assert!(
            err <= 1e-3,
            "round-trip error {err} m > 1e-3 m (accuracy regression?)"
        );
    }

    #[test]
    fn to_wgs84_rejects_degree_magnitude_input() {
        let crs = ProjectCrs::from_zone(31, false).expect("valid zone");
        // Degrees mislabeled as scene meters.
        let bad = SceneXY {
            x_m: 4.9,
            y_m: 52.4,
        };
        let got = crs.to_wgs84(bad);
        assert!(
            matches!(got, Err(GeoError::DegreeMagnitudeSceneCoord { .. })),
            "got {got:?}"
        );
    }

    #[test]
    fn to_utm_rejects_out_of_range_and_nonfinite() {
        let crs = ProjectCrs::from_zone(31, false).expect("valid zone");

        let hi_lon = crs.to_utm(LonLat {
            lon_deg: 181.0,
            lat_deg: 52.0,
        });
        assert!(
            matches!(hi_lon, Err(GeoError::LonLatOutOfRange { .. })),
            "got {hi_lon:?}"
        );

        let hi_lat = crs.to_utm(LonLat {
            lon_deg: 4.9,
            lat_deg: 91.0,
        });
        assert!(
            matches!(hi_lat, Err(GeoError::LonLatOutOfRange { .. })),
            "got {hi_lat:?}"
        );

        let nan = crs.to_utm(LonLat {
            lon_deg: f64::NAN,
            lat_deg: 52.0,
        });
        assert!(
            matches!(nan, Err(GeoError::NonFinite { .. })),
            "got {nan:?}"
        );

        // Geographically valid but outside the UTM domain -> loud rejection (LOW-1).
        let polar = crs.to_utm(LonLat {
            lon_deg: 4.9,
            lat_deg: 86.0,
        });
        assert!(
            matches!(polar, Err(GeoError::LatitudeOutsideUtm { .. })),
            "got {polar:?}"
        );
    }

    #[test]
    fn southern_hemisphere_round_trips() {
        // Sydney -> UTM 56S; false-northing 10_000_000 m applies (positive northing).
        let syd = LonLat {
            lon_deg: 151.2093,
            lat_deg: -33.8688,
        };
        let crs = ProjectCrs::for_location(syd).expect("valid landmark");
        assert!(crs.south, "Sydney is southern hemisphere");
        let utm = crs.to_utm(syd).expect("forward transform");
        assert!(
            utm.y_m > 0.0 && utm.y_m < 10_000_000.0,
            "southern northing should be positive (false northing): {}",
            utm.y_m
        );
        let back = crs.to_wgs84(utm).expect("inverse transform");
        assert!(
            error_m(syd, back) <= 1e-3,
            "southern round-trip not within 1e-3 m"
        );
    }

    #[test]
    fn utm_output_is_meter_magnitude() {
        // Catches the radians pitfall by 6 orders of magnitude: feeding degrees
        // (or forgetting the radian conversion) collapses easting to ~thousands.
        let dam = LonLat {
            lon_deg: 4.8936,
            lat_deg: 52.3731,
        };
        let crs = ProjectCrs::for_location(dam).expect("valid landmark");
        let utm = crs.to_utm(dam).expect("forward transform");
        assert!(
            (166_000.0..=834_000.0).contains(&utm.x_m),
            "easting {} m outside plausible UTM range",
            utm.x_m
        );
        assert!(
            (0.0..=10_000_000.0).contains(&utm.y_m),
            "northing {} m outside plausible UTM range",
            utm.y_m
        );
    }
}
