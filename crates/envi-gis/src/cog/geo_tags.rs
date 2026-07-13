//! GeoTIFF geotransform + nodata tag parsing (over untrusted tile bytes).
//!
//! # Module I/O
//! - **Inputs:** a live `tiff` [`Decoder`] positioned on the image whose geo-tags
//!   are wanted (`ModelPixelScaleTag` 33550, `ModelTiepointTag` 33922,
//!   `GDAL_NODATA` 42113).
//! - **Output:** a [`GeoTransform`] (map origin + signed pixel sizes) and an
//!   optional nodata sentinel.
//! - **Invariant (load-bearing):** the geotransform is **derived from the IFD
//!   tags, never assumed from nominal pixel counts** (threat T-08-02-04,
//!   08-RESEARCH Pitfall 5 — GLO-30 tiles above 50°N are not 3600 px wide, and
//!   their longitudinal pixel size differs from the latitudinal one). Missing or
//!   malformed tags yield a typed [`GisError`]; this module never panics.

use tiff::tags::Tag;

use crate::GisError;
use crate::cog::header::CogDecoder;

/// A north-up affine geotransform recovered from the GeoTIFF tie-point + pixel
/// scale tags. Maps a pixel `(col, row)` (top-left origin) to map coordinates:
/// `x = origin_x + col * pixel_size_x`, `y = origin_y + row * pixel_size_y`
/// (with `pixel_size_y` negative for a conventional north-up raster).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeoTransform {
    /// Map x of the top-left corner of pixel `(0, 0)`.
    pub origin_x: f64,
    /// Map y of the top-left corner of pixel `(0, 0)`.
    pub origin_y: f64,
    /// Signed pixel size in x (map units per column); `> 0` north-up.
    pub pixel_size_x: f64,
    /// Signed pixel size in y (map units per row); `< 0` north-up.
    pub pixel_size_y: f64,
}

impl GeoTransform {
    /// Map coordinates of the top-left corner of pixel `(col, row)`.
    #[must_use]
    pub fn pixel_to_map(&self, col: f64, row: f64) -> (f64, f64) {
        (
            self.origin_x + col * self.pixel_size_x,
            self.origin_y + row * self.pixel_size_y,
        )
    }
}

/// Read the north-up [`GeoTransform`] from the current image's GeoTIFF tags.
///
/// # Errors
/// - [`GisError::MissingGeoTag`] if `ModelPixelScaleTag` or `ModelTiepointTag`
///   is absent (geometry cannot be assumed — T-08-02-04).
/// - [`GisError::InvalidGeoTransform`] if the tags have too few components or a
///   non-positive / non-finite pixel scale (a degenerate transform).
/// - [`GisError::Tiff`] on a tag-read failure.
pub fn read_geotransform(dec: &mut CogDecoder<'_>) -> Result<GeoTransform, GisError> {
    let scale = dec
        .find_tag(Tag::ModelPixelScaleTag)?
        .ok_or(GisError::MissingGeoTag {
            tag: "ModelPixelScaleTag",
        })?
        .into_f64_vec()?;
    let tie = dec
        .find_tag(Tag::ModelTiepointTag)?
        .ok_or(GisError::MissingGeoTag {
            tag: "ModelTiepointTag",
        })?
        .into_f64_vec()?;

    if scale.len() < 3 {
        return Err(GisError::InvalidGeoTransform {
            what: format!("ModelPixelScale has {} values, need >= 3", scale.len()),
        });
    }
    if tie.len() < 6 {
        return Err(GisError::InvalidGeoTransform {
            what: format!("ModelTiepoint has {} values, need >= 6", tie.len()),
        });
    }

    let (sx, sy) = (scale[0], scale[1]);
    // Tie point maps pixel (i, j) -> map (x, y); GDAL writes (0,0,0, x, y, 0).
    let (i, j, x, y) = (tie[0], tie[1], tie[3], tie[4]);

    if !(sx.is_finite() && sy.is_finite() && x.is_finite() && y.is_finite()) {
        return Err(GisError::InvalidGeoTransform {
            what: "non-finite pixel scale or tie point".to_string(),
        });
    }
    if sx <= 0.0 || sy <= 0.0 {
        return Err(GisError::InvalidGeoTransform {
            what: format!("non-positive pixel scale ({sx}, {sy})"),
        });
    }

    // North-up: y decreases as row increases, so pixel_size_y = -sy.
    Ok(GeoTransform {
        origin_x: x - i * sx,
        origin_y: y + j * sy,
        pixel_size_x: sx,
        pixel_size_y: -sy,
    })
}

/// Read the `GDAL_NODATA` sentinel, if present.
///
/// Returns `Ok(None)` when the tag is absent or its ASCII payload cannot be
/// parsed as a finite number (a malformed nodata tag degrades to "no sentinel"
/// rather than failing the whole decode). A parsed sentinel is used by
/// [`crate::cog::window::decode_window`] to drop nodata samples so they never
/// reach the TIN as real elevations (threat T-08-02-03).
///
/// # Errors
/// [`GisError::Tiff`] only on an underlying tag-read failure.
pub fn read_nodata(dec: &mut CogDecoder<'_>) -> Result<Option<f64>, GisError> {
    let Some(val) = dec.find_tag(Tag::GdalNodata)? else {
        return Ok(None);
    };
    let raw = val.into_string()?;
    // GDAL writes an ASCII string, sometimes NUL-terminated / whitespace-padded.
    let trimmed = raw.trim_matches(|c: char| c == '\0' || c.is_whitespace());
    match trimmed.parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(Some(v)),
        _ => Ok(None),
    }
}
