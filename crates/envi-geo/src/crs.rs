//! UTM zone selection + the pinned per-project CRS (GEOX-04, D-03).
//!
//! [`utm_zone_for`] hand-rolls the plain zone formula; [`ProjectCrs`] pins a UTM
//! zone + hemisphere at project creation and holds the two `proj4rs` projections
//! ([`transform`](crate::transform) drives them).
//!
//! # Deviation
//!
//! The Norway (32V) and Svalbard (31X/33X/35X/37X) UTM grid **exceptions are
//! deliberately skipped** (06-RESEARCH Pattern 1). Those exceptions are
//! cartographic conventions for map-sheet tiling, **not accuracy requirements**:
//! a project pinned to the plain-formula zone stays within ~3 deg of a central
//! meridian, where the `etmerc` (exact transverse Mercator) scale error is
//! negligible for acoustics. The plain formula `floor((lon + 180) / 6) + 1`
//! (clamped `1..=60`) is used for every longitude, Norway and Svalbard included.

use proj4rs::proj::Proj;

use crate::{GeoError, LonLat};

/// The WGS84 geographic projection string (proj4rs longlat, radians on the wire
/// — converted in [`transform`](crate::transform) and ONLY there).
pub(crate) const WGS84_PROJ: &str = "+proj=longlat +ellps=WGS84 +datum=WGS84 +no_defs";

/// The Dutch **RD New** (EPSG:28992) projected CRS string.
///
/// Oblique-stereographic on the Bessel 1841 ellipsoid, Amersfoort origin, with
/// the 7-parameter Helmert `towgs84` datum shift so the transform speaks WGS84 at
/// the boundary. Values transcribed from <https://epsg.io/28992> (matching the
/// proj4rs `towgs84` example). Accuracy: the 7-parameter Helmert is ~0.5 m
/// against the official RDNAPTRANS™ grid transform — well inside the ≤ 1.0 m
/// oracle tolerance this crate pins RD New to, and adequate for terrain import.
///
/// This is the single reprojection boundary for RD New (GEOX-04): AHN terrain
/// (published in RD New) reprojects to WGS84 here and nowhere else.
const RD_NEW: &str = "+proj=sterea +lat_0=52.15616055555555 +lon_0=5.38763888888889 \
+k=0.9999079 +x_0=155000 +y_0=463000 +ellps=bessel \
+towgs84=565.417,50.3319,465.552,-0.398957,0.343988,-1.8774,4.0725 +units=m +no_defs";

/// Southern edge of the UTM domain, degrees. Below this (toward the South Pole)
/// UTM is undefined (UPS territory) — see [`GeoError::LatitudeOutsideUtm`].
pub(crate) const UTM_LAT_MIN: f64 = -80.0;
/// Northern edge of the UTM domain, degrees. Above this UTM is undefined (UPS).
pub(crate) const UTM_LAT_MAX: f64 = 84.0;

/// Auto-select the UTM zone for a WGS84 lon/lat (GEOX-04: pinned at project
/// creation, never re-derived per request).
///
/// Validates finiteness and range **first** (threat T-06-01-02: NaN/Inf and
/// out-of-range inputs are rejected with typed errors before any zone math),
/// then applies `zone = floor((lon + 180) / 6) + 1`, clamped to `1..=60` so the
/// `lon = +180` edge maps to zone 60.
pub fn utm_zone_for(p: LonLat) -> Result<u8, GeoError> {
    if !p.lon_deg.is_finite() {
        return Err(GeoError::NonFinite {
            what: format!("lon_deg = {}", p.lon_deg),
        });
    }
    if !p.lat_deg.is_finite() {
        return Err(GeoError::NonFinite {
            what: format!("lat_deg = {}", p.lat_deg),
        });
    }
    if !(-180.0..=180.0).contains(&p.lon_deg) || !(-90.0..=90.0).contains(&p.lat_deg) {
        return Err(GeoError::LonLatOutOfRange {
            lon: p.lon_deg,
            lat: p.lat_deg,
        });
    }
    // UTM is only defined in [-80, 84]° latitude; beyond that is UPS territory
    // where etmerc distorts silently (LOW-1). Reject loudly.
    if !(UTM_LAT_MIN..=UTM_LAT_MAX).contains(&p.lat_deg) {
        return Err(GeoError::LatitudeOutsideUtm { lat: p.lat_deg });
    }
    // Plain formula; clamp folds the lon = +180 edge back into zone 60.
    let zone = ((p.lon_deg + 180.0) / 6.0).floor() as i32 + 1;
    let zone = zone.clamp(1, 60) as u8;
    Ok(zone)
}

/// A project's pinned coordinate reference system: one UTM zone + hemisphere,
/// carrying the two `proj4rs` projections that [`to_utm`](Self::to_utm) /
/// [`to_wgs84`](Self::to_wgs84) drive.
pub struct ProjectCrs {
    /// UTM zone `1..=60`.
    pub utm_zone: u8,
    /// Southern hemisphere (adds `+south`, false-northing 10 000 000 m).
    pub south: bool,
    /// The pinned UTM projection (`+proj=utm ...`).
    pub(crate) proj: Proj,
    /// The WGS84 longlat projection (radians).
    pub(crate) wgs84: Proj,
}

impl ProjectCrs {
    /// Auto-pick the zone + hemisphere from a lon/lat and build the CRS
    /// (GEOX-04: pinned at project creation). `south = lat_deg < 0`.
    pub fn for_location(p: LonLat) -> Result<Self, GeoError> {
        let zone = utm_zone_for(p)?;
        Self::from_zone(zone, p.lat_deg < 0.0)
    }

    /// Build a CRS from an explicit zone + hemisphere. Validates `1..=60`.
    pub fn from_zone(utm_zone: u8, south: bool) -> Result<Self, GeoError> {
        if !(1..=60).contains(&utm_zone) {
            // Central meridian of the (invalid) zone, for a useful error value.
            let lon = (utm_zone as f64 - 1.0) * 6.0 - 180.0 + 3.0;
            return Err(GeoError::BadZone { lon });
        }
        let proj = Proj::from_proj_string(&utm_proj_string(utm_zone, south)).map_err(proj_err)?;
        let wgs84 = Proj::from_proj_string(WGS84_PROJ).map_err(proj_err)?;
        Ok(Self {
            utm_zone,
            south,
            proj,
            wgs84,
        })
    }

    /// The pinned-CRS label, e.g. `"utm-31n"` / `"utm-33s"` — later feeds
    /// `envi_engine::CrsInfo.label` and `project.json`. Descriptive only.
    #[must_use]
    pub fn label(&self) -> String {
        format!(
            "utm-{}{}",
            self.utm_zone,
            if self.south { "s" } else { "n" }
        )
    }

    /// The UTM proj string for persistence/logging.
    #[must_use]
    pub fn proj_string(&self) -> String {
        utm_proj_string(self.utm_zone, self.south)
    }
}

impl std::fmt::Debug for ProjectCrs {
    // `proj4rs::proj::Proj` is not `Debug`; surface the load-bearing identity.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectCrs")
            .field("utm_zone", &self.utm_zone)
            .field("south", &self.south)
            .field("label", &self.label())
            .finish()
    }
}

/// The Dutch **RD New** (EPSG:28992) projected CRS — a *source* CRS for
/// importing AHN terrain, distinct from the pinned project CRS ([`ProjectCrs`]).
///
/// Holds the RD projection plus WGS84 so [`transform`](crate::transform) can
/// drive WGS84 ⇄ RD New. The import path reprojects RD samples to WGS84 here,
/// then feeds them to the project's [`ProjectCrs`] — RD New never becomes a
/// second reprojection site (GEOX-04: one boundary).
pub struct RdNewCrs {
    /// The RD New projection (`+proj=sterea ... +towgs84=...`).
    pub(crate) proj: Proj,
    /// The WGS84 longlat projection (driven only by [`transform`](crate::transform)).
    pub(crate) wgs84: Proj,
}

impl RdNewCrs {
    /// Build the RD New (EPSG:28992) CRS.
    ///
    /// Fallible: a malformed proj string surfaces [`GeoError::Proj`], never a
    /// panic (threat T-08-01-02).
    pub fn new() -> Result<Self, GeoError> {
        Self::from_proj_string(RD_NEW)
    }

    /// Build the CRS from an explicit RD proj string. Private so the public
    /// surface only ever constructs the pinned [`RD_NEW`] definition; exists so
    /// the error path (bad proj string ⇒ [`GeoError::Proj`]) is unit-testable.
    fn from_proj_string(rd_proj: &str) -> Result<Self, GeoError> {
        let proj = Proj::from_proj_string(rd_proj).map_err(proj_err)?;
        let wgs84 = Proj::from_proj_string(WGS84_PROJ).map_err(proj_err)?;
        Ok(Self { proj, wgs84 })
    }

    /// The RD New proj string for persistence/logging.
    #[must_use]
    pub fn proj_string(&self) -> &'static str {
        RD_NEW
    }
}

impl std::fmt::Debug for RdNewCrs {
    // `proj4rs::proj::Proj` is not `Debug`; surface the load-bearing identity.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RdNewCrs")
            .field("epsg", &28992)
            .field("label", &"rd-new")
            .finish()
    }
}

/// Build the UTM proj string for a zone/hemisphere.
fn utm_proj_string(zone: u8, south: bool) -> String {
    format!(
        "+proj=utm +zone={zone}{} +ellps=WGS84 +units=m +no_defs",
        if south { " +south" } else { "" }
    )
}

/// Wrap a proj4rs error into a `PartialEq`-friendly `GeoError::Proj`.
pub(crate) fn proj_err<E: std::fmt::Display>(err: E) -> GeoError {
    GeoError::Proj {
        message: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utm_zone_selection_edges() {
        // lon = -180 -> zone 1; lon = +180 -> zone 60 (clamped edge); lon = 0 -> 31.
        assert_eq!(
            utm_zone_for(LonLat {
                lon_deg: -180.0,
                lat_deg: 0.0
            }),
            Ok(1)
        );
        assert_eq!(
            utm_zone_for(LonLat {
                lon_deg: 180.0,
                lat_deg: 0.0
            }),
            Ok(60)
        );
        assert_eq!(
            utm_zone_for(LonLat {
                lon_deg: 0.0,
                lat_deg: 0.0
            }),
            Ok(31)
        );
        // Dam Square, Amsterdam -> zone 31 (GEOX-04, D-03 landmark).
        assert_eq!(
            utm_zone_for(LonLat {
                lon_deg: 4.8936,
                lat_deg: 52.3731
            }),
            Ok(31)
        );
    }

    #[test]
    fn for_location_pins_zone_and_hemisphere() {
        // Dam Square -> 31 north.
        let dam = ProjectCrs::for_location(LonLat {
            lon_deg: 4.8936,
            lat_deg: 52.3731,
        })
        .expect("valid landmark");
        assert_eq!(dam.utm_zone, 31);
        assert!(!dam.south, "Amsterdam is northern hemisphere");
        assert_eq!(dam.label(), "utm-31n");

        // Sydney (151.21 E, -33.87 N) -> zone 56, south == true.
        let syd = ProjectCrs::for_location(LonLat {
            lon_deg: 151.2093,
            lat_deg: -33.8688,
        })
        .expect("valid landmark");
        assert_eq!(syd.utm_zone, 56);
        assert!(syd.south, "Sydney is southern hemisphere");
        assert_eq!(syd.label(), "utm-56s");
        assert!(syd.proj_string().contains("+south"));
    }

    #[test]
    fn zone_selection_rejects_nonfinite_and_out_of_range() {
        let nan = utm_zone_for(LonLat {
            lon_deg: f64::NAN,
            lat_deg: 0.0,
        });
        assert!(
            matches!(nan, Err(GeoError::NonFinite { .. })),
            "got {nan:?}"
        );

        let inf = utm_zone_for(LonLat {
            lon_deg: 0.0,
            lat_deg: f64::INFINITY,
        });
        assert!(
            matches!(inf, Err(GeoError::NonFinite { .. })),
            "got {inf:?}"
        );

        let hi_lon = utm_zone_for(LonLat {
            lon_deg: 181.0,
            lat_deg: 0.0,
        });
        assert!(
            matches!(hi_lon, Err(GeoError::LonLatOutOfRange { .. })),
            "got {hi_lon:?}"
        );

        let hi_lat = utm_zone_for(LonLat {
            lon_deg: 0.0,
            lat_deg: 91.0,
        });
        assert!(
            matches!(hi_lat, Err(GeoError::LonLatOutOfRange { .. })),
            "got {hi_lat:?}"
        );
    }

    #[test]
    fn zone_selection_rejects_latitude_outside_utm_band() {
        // In-range latitude (91) is caught by the range check; a latitude that is
        // valid geographically but outside UTM's [-80, 84] band is rejected with
        // the dedicated typed error (LOW-1).
        let too_far_north = utm_zone_for(LonLat {
            lon_deg: 15.0,
            lat_deg: 85.0,
        });
        assert!(
            matches!(too_far_north, Err(GeoError::LatitudeOutsideUtm { .. })),
            "got {too_far_north:?}"
        );
        let too_far_south = utm_zone_for(LonLat {
            lon_deg: 15.0,
            lat_deg: -81.0,
        });
        assert!(
            matches!(too_far_south, Err(GeoError::LatitudeOutsideUtm { .. })),
            "got {too_far_south:?}"
        );
        // The band edges themselves are still accepted.
        assert!(
            utm_zone_for(LonLat {
                lon_deg: 15.0,
                lat_deg: 84.0
            })
            .is_ok(),
            "84° N is the inclusive UTM edge"
        );
    }

    #[test]
    fn from_zone_rejects_invalid_zone() {
        assert!(matches!(
            ProjectCrs::from_zone(0, false),
            Err(GeoError::BadZone { .. })
        ));
        assert!(matches!(
            ProjectCrs::from_zone(61, false),
            Err(GeoError::BadZone { .. })
        ));
    }

    #[test]
    fn rd_new_malformed_proj_string_is_typed_error_not_panic() {
        // Fallible construction: a bad proj string surfaces GeoError::Proj, never
        // a panic (threat T-08-01-02).
        let got = RdNewCrs::from_proj_string("+proj=totally_bogus +no_defs");
        assert!(matches!(got, Err(GeoError::Proj { .. })), "got {got:?}");
    }
}
