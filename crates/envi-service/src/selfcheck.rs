//! The D-08 startup self-check: a pure-Rust CRS landmark round-trip that the
//! binary refuses to start without (06-RESEARCH Pattern 6).
//!
//! # SC2 adjustment (D-08 — do not regress this wording)
//!
//! This REPLACES the roadmap's literal "GDAL/PROJ version / `proj.db` /
//! `GDAL_DATA` self-check". That GDAL/PROJ check is **deferred to Phase 8** with
//! the C dependency (D-02): Phase 6 carries ZERO C toolchain. What Phase 6 proves
//! at every startup is that the pure-Rust CRS seam ([`envi_geo`]) reprojects a
//! known landmark WGS84 -> UTM -> WGS84 to within 1 m. A broken CRS stack that
//! would silently produce garbage scene coordinates makes the service refuse to
//! start (threat T-06-03-07), rather than starting and corrupting projects.
//!
//! The landmark is Dam Square, Amsterdam (4.8936 E, 52.3731 N) -> UTM zone 31N.

use envi_geo::{LonLat, ProjectCrs};
use thiserror::Error;

/// Dam Square, Amsterdam — the fixed self-check landmark (UTM zone 31N).
const LANDMARK: LonLat = LonLat {
    lon_deg: 4.8936,
    lat_deg: 52.3731,
};

/// Meters per degree of latitude (WGS84 mean) — used to turn the lon/lat
/// round-trip residual into a metric error, matching the transform.rs test math.
const M_PER_DEG_LAT: f64 = 111_320.0;

/// The self-check outcome: the pinned zone/hemisphere and the round-trip error in
/// meters, so `main` can log a meaningful startup line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelfCheckReport {
    /// The auto-selected UTM zone `1..=60` (31 for the Dam Square landmark).
    pub utm_zone: u8,
    /// Southern hemisphere flag (false for the landmark).
    pub south: bool,
    /// Round-trip WGS84 -> UTM -> WGS84 error, meters (must be <= 1 m).
    pub err_m: f64,
}

/// Reasons the startup self-check can fail (refuse-to-start, D-08).
#[derive(Debug, Error)]
pub enum SelfCheckError {
    /// The `envi-geo` reprojection seam itself errored (proj build/transform).
    #[error("CRS self-check reprojection failed: {0}")]
    Geo(#[from] envi_geo::GeoError),
    /// The round-trip exceeded the 1 m tolerance — a silent CRS stack fault.
    #[error("CRS round-trip error {err_m} m exceeds the 1 m self-check tolerance")]
    RoundTrip {
        /// The measured round-trip error, meters.
        err_m: f64,
    },
}

/// Run the pure-Rust CRS landmark round-trip self-check (D-08).
///
/// Reprojects the Dam Square landmark WGS84 -> UTM -> WGS84 and asserts the
/// residual is <= 1 m. On success returns the pinned zone + measured error so the
/// caller can log it; on failure returns a [`SelfCheckError`] and `main` refuses
/// to start (returns `Err`, non-zero process exit).
///
/// # Errors
/// [`SelfCheckError::Geo`] if reprojection setup/transform fails;
/// [`SelfCheckError::RoundTrip`] if the round-trip error exceeds 1 m.
pub fn crs_self_check() -> Result<SelfCheckReport, SelfCheckError> {
    let crs = ProjectCrs::for_location(LANDMARK)?;
    let utm = crs.to_utm(LANDMARK)?;
    let back = crs.to_wgs84(utm)?;

    let dlat_m = (back.lat_deg - LANDMARK.lat_deg).abs() * M_PER_DEG_LAT;
    let dlon_m =
        (back.lon_deg - LANDMARK.lon_deg).abs() * M_PER_DEG_LAT * LANDMARK.lat_deg.to_radians().cos();
    let err_m = (dlat_m.powi(2) + dlon_m.powi(2)).sqrt();

    if err_m > 1.0 {
        return Err(SelfCheckError::RoundTrip { err_m });
    }
    Ok(SelfCheckReport {
        utm_zone: crs.utm_zone,
        south: crs.south,
        err_m,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_check_passes_on_healthy_stack() {
        // Healthy path: the pure-Rust CRS seam round-trips Dam Square to zone 31N
        // well within 1 m (etmerc has ~3 orders of headroom).
        let report = crs_self_check().expect("healthy CRS stack passes the self-check");
        assert_eq!(report.utm_zone, 31, "Dam Square -> UTM zone 31");
        assert!(!report.south, "Amsterdam is northern hemisphere");
        assert!(
            report.err_m <= 1.0,
            "round-trip error {} m must be <= 1 m",
            report.err_m
        );
        // Tighter regression guard: real etmerc accuracy is sub-millimeter.
        assert!(
            report.err_m <= 1e-3,
            "round-trip error {} m suggests an accuracy regression",
            report.err_m
        );
    }

    #[test]
    fn self_check_failure_refuses_start() {
        // The refuse-to-start contract: a self-check error propagates as an
        // `Err` that `main` returns as `Box<dyn Error>` (non-zero exit). We
        // synthesize the RoundTrip failure and assert it is a real std::error.
        let err = SelfCheckError::RoundTrip { err_m: 42.0 };
        let boxed: Box<dyn std::error::Error> = Box::new(err);
        assert!(
            boxed.to_string().contains("exceeds the 1 m"),
            "the refusal error must explain the tolerance breach: {boxed}"
        );
    }
}
