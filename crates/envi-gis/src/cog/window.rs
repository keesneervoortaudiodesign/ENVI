//! Windowed `f32` decode from a cached COG tile (guard-first, no-panic).
//!
//! # Module I/O
//! - **Inputs:** the cached COG/BigTIFF tile bytes (from OPFS via TS) — either the
//!   WHOLE file (`&[u8]`) or, on the range-read path, only the header prefix + the
//!   chunk byte ranges [`crate::cog::plan`] selected, as a
//!   [`CogBytes`](crate::cog::sparse::CogBytes) sparse view; plus a [`PixelWindow`]
//!   into the base image and a `max_decoded_px` budget.
//! - **Output:** a [`Raster<f32>`] of exactly the window's pixels, carrying its
//!   own geotransform (origin shifted to the window's top-left) and holes
//!   ([`None`]) wherever the source was nodata or non-finite.
//! - **Invariants (load-bearing), mirroring `envi_dgm::tin::build_tin` ordering:**
//!   1. **DoS budget FIRST** — `max_decoded_px` is enforced from the window's
//!      pixel count *before* any `read_chunk` allocates output (threat
//!      T-08-02-01; the analog of `MAX_POINTS`). Overhead-free: no pixels are
//!      decoded on the reject path.
//!   2. **Bounds SECOND** — a window outside `ImageWidth/Length` is a typed
//!      [`GisError::WindowOutOfBounds`], never a silent empty/zero raster; this
//!      also means only in-bounds pixels are read, so edge-tile padding beyond
//!      the image is never returned (threat T-08-02-03, 08-RESEARCH Pitfall 7).
//!   3. **Then work** — read the covering chunks once, drop nodata / non-finite
//!      samples to holes (never a silent `0.0`, threat T-08-02-03), and map every
//!      `tiff::TiffError` to a [`GisError`].

use std::collections::HashMap;

use tiff::decoder::DecodingResult;

use crate::GisError;
use crate::cog::geo_tags::{self, GeoTransform};
use crate::cog::header::{self, CogHeader};
use crate::cog::sparse::CogBytes;

/// A pixel-space window into the base image: `[col_off, col_off+width)` ×
/// `[row_off, row_off+height)`, top-left origin. This is the deterministic
/// windowing contract; [`GeoTransform::pixel_to_map`] (and [`bbox_to_pixel_window`])
/// bridge a source-CRS bbox to one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelWindow {
    /// Leftmost column (inclusive).
    pub col_off: u32,
    /// Topmost row (inclusive).
    pub row_off: u32,
    /// Window width in pixels (`> 0`).
    pub width: u32,
    /// Window height in pixels (`> 0`).
    pub height: u32,
}

/// A decoded raster window. `samples` is row-major, length `width * height`;
/// `None` marks a hole (nodata / non-finite) — a caller MUST treat holes as
/// "no elevation here", never as `0.0` (the TIN interpolates across holes).
#[derive(Debug, Clone, PartialEq)]
pub struct Raster<T> {
    /// Window width in pixels.
    pub width: usize,
    /// Window height in pixels.
    pub height: usize,
    /// Geotransform of *this window* (origin at the window's top-left pixel).
    pub geo: GeoTransform,
    /// Row-major samples; `None` = hole (dropped nodata / non-finite).
    pub samples: Vec<Option<T>>,
}

impl<T: Copy> Raster<T> {
    /// Sample at `(col, row)` within the window, or `None` for a hole /
    /// out-of-window index.
    #[must_use]
    pub fn get(&self, col: usize, row: usize) -> Option<T> {
        if col >= self.width || row >= self.height {
            return None;
        }
        self.samples[row * self.width + col]
    }
}

/// Convert a source-CRS bbox to the [`PixelWindow`] of every pixel whose
/// **center** falls within `[min_x, max_x] × [min_y, max_y]`, clamped to the
/// image bounds. Pure geometry over the IFD geotransform (never nominal dims —
/// threat T-08-02-04). Returns `None` if the bbox does not cover any in-image
/// pixel center.
#[must_use]
pub fn bbox_to_pixel_window(
    geo: &GeoTransform,
    header: &CogHeader,
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
) -> Option<PixelWindow> {
    // Column center x(c) = origin_x + (c + 0.5) * psx  => c = (x - origin_x)/psx - 0.5.
    let cx = |x: f64| (x - geo.origin_x) / geo.pixel_size_x - 0.5;
    let ry = |y: f64| (y - geo.origin_y) / geo.pixel_size_y - 0.5;
    // psx > 0, psy < 0: x grows with col, y shrinks with row.
    let c_lo = cx(min_x).ceil();
    let c_hi = cx(max_x).floor();
    // For psy < 0, larger y => smaller row: max_y gives the low row bound.
    let r_lo = ry(max_y).ceil();
    let r_hi = ry(min_y).floor();

    let w = header.width as f64;
    let h = header.height as f64;
    let c_lo = c_lo.max(0.0);
    let r_lo = r_lo.max(0.0);
    let c_hi = c_hi.min(w - 1.0);
    let r_hi = r_hi.min(h - 1.0);
    if c_hi < c_lo || r_hi < r_lo {
        return None;
    }
    Some(PixelWindow {
        col_off: c_lo as u32,
        row_off: r_lo as u32,
        width: (c_hi - c_lo) as u32 + 1,
        height: (r_hi - r_lo) as u32 + 1,
    })
}

/// Decode `window` from a cached whole COG tile into an [`f32`] [`Raster`].
///
/// See the module invariants for the guard-first ordering. `max_decoded_px` is
/// the caller's memory budget (default [`crate::cog::MAX_DECODED_PX`]); a window
/// exceeding it is rejected before any decode.
///
/// # Errors
/// - [`GisError::WindowOutOfBounds`] — zero-extent window, or a window reaching
///   past `ImageWidth/Length`.
/// - [`GisError::DecodeBudgetExceeded`] — window pixel count over `max_decoded_px`.
/// - [`GisError::TooManyImages`] — the IFD overview chain is too long/cyclic.
/// - [`GisError::UnexpectedSampleFormat`] — a chunk was not `f32`.
/// - [`GisError::MissingGeoTag`] / [`GisError::InvalidGeoTransform`] — geo-tags.
/// - [`GisError::Tiff`] — any underlying `tiff` decode failure.
pub fn decode_window(
    tile_bytes: &[u8],
    window: PixelWindow,
    max_decoded_px: usize,
) -> Result<Raster<f32>, GisError> {
    decode_window_cog(&CogBytes::whole(tile_bytes), window, max_decoded_px)
}

/// Decode `window` from a **partially fetched** COG (the range-read path): the
/// header prefix plus exactly the chunk byte ranges [`crate::cog::plan`] selected.
///
/// Identical guards and semantics to [`decode_window`] — only the byte source
/// differs. A chunk the plan failed to fetch is a loud
/// [`GisError::Tiff`] (the sparse view errors on an unfetched byte rather than
/// serving zeros), never a window quietly full of `0.0`.
///
/// # Errors
/// Same set as [`decode_window`].
pub fn decode_window_cog(
    cog: &CogBytes<'_>,
    window: PixelWindow,
    max_decoded_px: usize,
) -> Result<Raster<f32>, GisError> {
    decode_window_generic(
        cog,
        window,
        max_decoded_px,
        |r| match r {
            DecodingResult::F32(v) => Ok(v),
            other => Err(GisError::UnexpectedSampleFormat {
                got: decoding_result_kind(&other).to_string(),
            }),
        },
        // Drop nodata sentinels + any non-finite value: a hole, never 0.0.
        |sample, nodata| sample.is_finite() && nodata.is_none_or(|nd| (sample as f64) != nd),
    )
}

/// The type-generic decode shared by [`decode_window`] (f32) and
/// [`decode_window_u8`] (u8). The guard ORDERING is load-bearing and identical
/// across sample types — image-count guard → open → header → zero-extent → DoS
/// budget → bounds → geo/nodata → chunk-cache loop → sample assembly → Raster
/// build (mirroring `envi_dgm::tin::build_tin`; do NOT reorder). Only the sample
/// type varies, injected as two closures:
/// - `extract`: pull the concrete `Vec<T>` out of a decoded chunk, or a typed
///   [`GisError::UnexpectedSampleFormat`] when the chunk is the wrong format.
/// - `keep`: whether a sample survives (vs. a hole) given the optional nodata
///   sentinel — f32 additionally drops non-finite values, u8 only the sentinel.
fn decode_window_generic<T, E, K>(
    cog: &CogBytes<'_>,
    window: PixelWindow,
    max_decoded_px: usize,
    extract: E,
    keep: K,
) -> Result<Raster<T>, GisError>
where
    T: Copy,
    E: Fn(DecodingResult) -> Result<Vec<T>, GisError>,
    K: Fn(T, Option<f64>) -> bool,
{
    // Cap the IFD chain up front (T-08-02-02) before trusting any navigation.
    header::guard_image_count(cog)?;

    let mut dec = header::open_cog(cog)?;
    let hdr = header::read_header(&mut dec)?;

    // --- (2 is cheap; do zero-extent check first) ---
    if window.width == 0 || window.height == 0 {
        return Err(GisError::WindowOutOfBounds {
            what: format!("zero-extent window {}x{}", window.width, window.height),
        });
    }

    // --- (1) DoS budget FIRST: reject from the window pixel count, computed
    //     from IFD-derived geometry, BEFORE any read_chunk allocates output. ---
    let requested_px = (window.width as usize)
        .checked_mul(window.height as usize)
        .ok_or(GisError::DecodeBudgetExceeded {
            requested_px: usize::MAX,
            limit: max_decoded_px,
        })?;
    if requested_px > max_decoded_px {
        return Err(GisError::DecodeBudgetExceeded {
            requested_px,
            limit: max_decoded_px,
        });
    }

    // --- (2) Bounds: crop to ImageWidth/Length; reject overreach (no padding). ---
    let col_end = window.col_off.checked_add(window.width);
    let row_end = window.row_off.checked_add(window.height);
    match (col_end, row_end) {
        (Some(ce), Some(re)) if ce <= hdr.width && re <= hdr.height => {}
        _ => {
            return Err(GisError::WindowOutOfBounds {
                what: format!(
                    "window [{}+{}, {}+{}] exceeds image {}x{}",
                    window.col_off,
                    window.width,
                    window.row_off,
                    window.height,
                    hdr.width,
                    hdr.height
                ),
            });
        }
    }

    // --- (3) Work: geo-tags, then read the covering chunks once and assemble. ---
    let geo = geo_tags::read_geotransform(&mut dec)?;
    let nodata = geo_tags::read_nodata(&mut dec)?;

    let chunks_across = hdr.chunks_across();
    let (cw, ch) = (hdr.chunk_w, hdr.chunk_h);

    // Read each covering chunk exactly once. `read_chunk` returns the CROPPED
    // (unpadded) tile — row stride = data_w — so edge padding beyond
    // ImageWidth/Length is never materialized (Pitfall 7). Value = (data, data_w).
    let (tx0, tx1) = (
        window.col_off / cw,
        (window.col_off + window.width - 1) / cw,
    );
    let (ty0, ty1) = (
        window.row_off / ch,
        (window.row_off + window.height - 1) / ch,
    );
    let mut cache: HashMap<u32, (Vec<T>, u32)> = HashMap::new();
    for ty in ty0..=ty1 {
        for tx in tx0..=tx1 {
            let idx = ty * chunks_across + tx;
            let (data_w, _data_h) = dec.chunk_data_dimensions(idx);
            let data = extract(dec.read_chunk(idx)?)?;
            cache.insert(idx, (data, data_w));
        }
    }

    let mut samples: Vec<Option<T>> = Vec::with_capacity(requested_px);
    for r in 0..window.height {
        let row = window.row_off + r;
        let (ty, local_row) = (row / ch, row % ch);
        for c in 0..window.width {
            let col = window.col_off + c;
            let (tx, local_col) = (col / cw, col % cw);
            let idx = ty * chunks_across + tx;
            let (data, data_w) = &cache[&idx];
            let sample = data[(local_row as usize) * (*data_w as usize) + local_col as usize];
            samples.push(keep(sample, nodata).then_some(sample));
        }
    }

    let (origin_x, origin_y) = geo.pixel_to_map(window.col_off as f64, window.row_off as f64);
    Ok(Raster {
        width: window.width as usize,
        height: window.height as usize,
        geo: GeoTransform {
            origin_x,
            origin_y,
            pixel_size_x: geo.pixel_size_x,
            pixel_size_y: geo.pixel_size_y,
        },
        samples,
    })
}

/// Decode `window` from a cached whole COG tile into a [`u8`] [`Raster`].
///
/// The `u8` sibling of [`decode_window`] for **class rasters** (ESA WorldCover
/// v200 land-cover codes are single-byte). The guard-first ordering, bounds
/// check, geotransform derivation, and nodata-drop semantics are identical — only
/// the decoded sample type differs (a chunk that is not `u8` is a typed
/// [`GisError::UnexpectedSampleFormat`]). This is the decode producer
/// [`crate::landcover::vectorize_landcover`] consumes (it takes a `Raster<u8>`),
/// closing the seam left open in 08-05 where only the `f32` terrain path existed.
///
/// # Errors
/// Same set as [`decode_window`], except the sample-format guard requires `u8`.
pub fn decode_window_u8(
    tile_bytes: &[u8],
    window: PixelWindow,
    max_decoded_px: usize,
) -> Result<Raster<u8>, GisError> {
    decode_window_u8_cog(&CogBytes::whole(tile_bytes), window, max_decoded_px)
}

/// Decode a `u8` class-raster `window` from a **partially fetched** COG (the
/// range-read sibling of [`decode_window_u8`], as [`decode_window_cog`] is to
/// [`decode_window`]). This is the path the 54 MB ESA WorldCover tile takes.
///
/// # Errors
/// Same set as [`decode_window_u8`].
pub fn decode_window_u8_cog(
    cog: &CogBytes<'_>,
    window: PixelWindow,
    max_decoded_px: usize,
) -> Result<Raster<u8>, GisError> {
    decode_window_generic(
        cog,
        window,
        max_decoded_px,
        |r| match r {
            DecodingResult::U8(v) => Ok(v),
            other => Err(GisError::UnexpectedSampleFormat {
                got: decoding_result_kind(&other).to_string(),
            }),
        },
        // Drop the nodata sentinel to a hole — never a silent class `0`.
        |sample, nodata| nodata.is_none_or(|nd| f64::from(sample) != nd),
    )
}

/// Human-readable kind of a non-`f32` [`DecodingResult`], for error reporting.
fn decoding_result_kind(r: &DecodingResult) -> &'static str {
    match r {
        DecodingResult::U8(_) => "u8",
        DecodingResult::U16(_) => "u16",
        DecodingResult::U32(_) => "u32",
        DecodingResult::U64(_) => "u64",
        DecodingResult::F16(_) => "f16",
        DecodingResult::F32(_) => "f32",
        DecodingResult::F64(_) => "f64",
        DecodingResult::I8(_) => "i8",
        DecodingResult::I16(_) => "i16",
        DecodingResult::I32(_) => "i32",
        DecodingResult::I64(_) => "i64",
    }
}
