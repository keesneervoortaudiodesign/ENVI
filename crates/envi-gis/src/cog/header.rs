//! TIFF/BigTIFF IFD parse: image dimensions, chunk grid, IFD-chain cap.
//!
//! # Module I/O
//! - **Inputs:** whole cached COG/BigTIFF tile bytes (`&[u8]`).
//! - **Output:** a [`Decoder`] over the bytes (via [`open`]) and a [`CogHeader`]
//!   describing the base image's pixel dimensions and internal chunk (tile or
//!   strip) grid.
//! - **Invariant (load-bearing):** the IFD overview chain is **capped**
//!   ([`MAX_IFD_IMAGES`]) so a malicious/cyclic IFD chain cannot spin forever
//!   (threat T-08-02-02); every `tiff` error becomes a typed [`GisError`], never
//!   a panic. `tiff::Decoder::new` transparently handles both classic TIFF
//!   (`II*\0`) and BigTIFF (`II+\0`) containers, so BigTIFF AHN tiles and
//!   classic-TIFF GLO-30 tiles flow through the same path.

use std::io::Cursor;

use tiff::decoder::Decoder;

use crate::GisError;

/// Maximum IFD images (base + overview levels) walked before rejecting the
/// chain (DoS bound, threat T-08-02-02). Real COG overview pyramids are a
/// handful of levels; a chain longer than this is pathological or cyclic.
pub const MAX_IFD_IMAGES: usize = 64;

/// The base image's geometry, read from the IFD (never assumed).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CogHeader {
    /// Image width in pixels (`ImageWidth`).
    pub width: u32,
    /// Image height in pixels (`ImageLength`).
    pub height: u32,
    /// Internal chunk width (tile width, or image width for strips).
    pub chunk_w: u32,
    /// Internal chunk height (tile height, or `RowsPerStrip` for strips).
    pub chunk_h: u32,
}

impl CogHeader {
    /// Number of chunks spanning the image width (row of the chunk grid).
    /// Handles tiles (`ceil(width / tile_w)`) and strips (`chunk_w == width`
    /// ⇒ `1`) uniformly, so chunk index `= chunk_row * chunks_across + chunk_col`.
    #[must_use]
    pub fn chunks_across(&self) -> u32 {
        self.width.div_ceil(self.chunk_w.max(1))
    }
}

/// Open a decoder over the whole tile bytes (classic TIFF or BigTIFF).
///
/// # Errors
/// [`GisError::Tiff`] if the bytes are not a valid TIFF/BigTIFF header.
pub fn open(bytes: &[u8]) -> Result<Decoder<Cursor<&[u8]>>, GisError> {
    Ok(Decoder::new(Cursor::new(bytes))?)
}

/// Read the base image [`CogHeader`] from an open decoder.
///
/// # Errors
/// - [`GisError::Tiff`] on a dimension-read failure.
/// - [`GisError::InvalidGeoTransform`] never (see `geo_tags`); dimension `0`
///   chunk sizes are rejected as [`GisError::Tiff`]-adjacent via `WindowOut`.
pub fn read_header(dec: &mut Decoder<Cursor<&[u8]>>) -> Result<CogHeader, GisError> {
    let (width, height) = dec.dimensions()?;
    let (chunk_w, chunk_h) = dec.chunk_dimensions();
    if chunk_w == 0 || chunk_h == 0 {
        return Err(GisError::WindowOutOfBounds {
            what: format!("degenerate chunk dimensions {chunk_w}x{chunk_h}"),
        });
    }
    Ok(CogHeader {
        width,
        height,
        chunk_w,
        chunk_h,
    })
}

/// Walk the IFD (overview) chain, returning the image count, capped at
/// [`MAX_IFD_IMAGES`].
///
/// Uses a fresh decoder so it does not disturb a decode in progress. A chain
/// that reaches the cap is rejected with [`GisError::TooManyImages`] — the guard
/// against a cyclic or absurdly deep IFD chain (threat T-08-02-02).
///
/// # Errors
/// - [`GisError::TooManyImages`] if the chain exceeds [`MAX_IFD_IMAGES`].
/// - [`GisError::Tiff`] on an IFD-navigation failure.
pub fn guard_image_count(bytes: &[u8]) -> Result<usize, GisError> {
    let mut dec = open(bytes)?;
    let mut count = 1; // Decoder::new has already read the first IFD.
    while dec.more_images() {
        if count >= MAX_IFD_IMAGES {
            return Err(GisError::TooManyImages {
                got: count,
                limit: MAX_IFD_IMAGES,
            });
        }
        dec.next_image()?;
        count += 1;
    }
    Ok(count)
}
