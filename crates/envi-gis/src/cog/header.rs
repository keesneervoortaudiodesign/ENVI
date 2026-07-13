//! TIFF/BigTIFF IFD parse: image dimensions, chunk grid, IFD-chain cap, and the
//! raw IFD walk the range planner needs *before* a decoder can be opened.
//!
//! # Module I/O
//! - **Inputs:** a [`CogBytes`] view of the tile — either the whole cached file
//!   (`CogBytes::whole`) or just the fetched header prefix + tile ranges of a
//!   windowed range read.
//! - **Output:** a [`CogDecoder`] over those bytes (via [`open`] / [`open_cog`])
//!   and a [`CogHeader`] describing the base image's pixel dimensions and internal
//!   chunk (tile or strip) grid; plus [`walk_ifds`], a decoder-free scan of the raw
//!   IFD chain used by [`crate::cog::plan`] to decide whether the header even fits
//!   in the prefix that was fetched.
//! - **Invariant (load-bearing):** the IFD overview chain is **capped**
//!   ([`MAX_IFD_IMAGES`]) so a malicious/cyclic IFD chain cannot spin forever
//!   (threat T-08-02-02); every `tiff` error becomes a typed [`GisError`], never
//!   a panic. `tiff::Decoder::new` transparently handles both classic TIFF
//!   (`II*\0`) and BigTIFF (`II+\0`) containers, so BigTIFF AHN tiles and
//!   classic-TIFF GLO-30 tiles flow through the same path.
//! - **Invariant (load-bearing): the PALETTE re-interpretation.** The real ESA
//!   WorldCover COG is a **palette** class raster (`PhotometricInterpretation =
//!   RGBPalette` + a 768-entry `ColorMap`), and the `tiff` crate refuses such an
//!   image outright — *every* read path funnels through `Image::colortype()`,
//!   which returns `UnsupportedError(InterpretationWithBits(RGBPalette, [8]))`
//!   (`tiff-0.11.3/src/decoder/image.rs:518`). There is no lower-level API that
//!   skips it. So [`open_cog`] hands the decoder a [`crate::cog::sparse::SparseReader`]
//!   — a zero-copy cursor over the same bytes that rewrites tag 262 from
//!   `RGBPalette` (3) to `BlackIsZero` (1) as those two bytes are read, for every
//!   IFD in the chain. This is a **re-interpretation, not a conversion**: in a
//!   palette TIFF the sample IS the class index and the `ColorMap` is presentation
//!   only, so the crate returns exactly the raw `u8` codes (10, 20, ... 95) that
//!   [`crate::landcover`] needs — no palette expansion to RGB, no pixel copied, no
//!   allocation of a second copy of a ~100 MB tile. Non-palette tiles are scanned,
//!   matched nothing, and pass through byte-identical.

use tiff::decoder::Decoder;

use crate::GisError;
use crate::cog::sparse::{CogBytes, SparseReader};

/// A `tiff` decoder over a sparse (or whole) view of the tile bytes.
pub type CogDecoder<'a> = Decoder<SparseReader<'a>>;

/// TIFF tag 262 — `PhotometricInterpretation`.
const TAG_PHOTOMETRIC: u16 = 262;
/// TIFF field type 3 — `SHORT` (u16).
const TYPE_SHORT: u16 = 3;
/// `PhotometricInterpretation` value 3 — `RGBPalette` (colour-mapped).
const PHOTOMETRIC_RGB_PALETTE: u16 = 3;
/// `PhotometricInterpretation` value 1 — `BlackIsZero` (plain grayscale samples).
const PHOTOMETRIC_BLACK_IS_ZERO: u16 = 1;

/// Maximum IFD images (base + overview levels) walked before rejecting the
/// chain (DoS bound, threat T-08-02-02). Real COG overview pyramids are a
/// handful of levels; a chain longer than this is pathological or cyclic.
pub const MAX_IFD_IMAGES: usize = 64;

// --- the raw IFD walk (decoder-free — runs before we know the header fits) ---

/// One IFD entry seen by [`walk_ifds`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IfdEntry {
    /// TIFF tag number.
    pub tag: u16,
    /// TIFF field type.
    pub field_type: u16,
    /// Value count.
    pub count: u64,
    /// Byte offset of the entry's value field within the file.
    pub value_offset: u64,
    /// Total byte length of the value (`count * type size`).
    pub value_len: u64,
    /// Whether the value fits inline in the entry (no out-of-line fetch needed).
    pub inline: bool,
}

/// What a raw IFD-chain walk found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfdWalk {
    /// Highest file byte offset (exclusive) the header structures reach — the IFD
    /// directories themselves plus every out-of-line tag value. This is the number
    /// of leading bytes a caller must hold to parse the header completely.
    pub header_end: u64,
    /// `Some(offset)` when the walk ran off the end of the supplied bytes: the
    /// walk is INCOMPLETE and `header_end` is only a lower bound. The caller must
    /// fetch more and walk again.
    pub truncated_at: Option<u64>,
    /// Number of IFDs (images) fully walked.
    pub images: usize,
}

/// Byte size of a TIFF field type (`0` for an unknown type — treated as "no
/// out-of-line value", so an unrecognised tag can never inflate the header ask).
fn type_size(field_type: u16) -> u64 {
    match field_type {
        1 | 2 | 6 | 7 => 1,              // BYTE, ASCII, SBYTE, UNDEFINED
        3 | 8 => 2,                      // SHORT, SSHORT
        4 | 9 | 11 | 13 => 4,            // LONG, SLONG, FLOAT, IFD
        5 | 10 | 12 | 16 | 17 | 18 => 8, // RATIONAL, SRATIONAL, DOUBLE, LONG8, SLONG8, IFD8
        _ => 0,
    }
}

/// Walk the raw IFD chain over `bytes` (which must start at file offset `0`),
/// calling `visit` for every entry, and report how far the header structures
/// reach ([`IfdWalk::header_end`]) and whether the walk was truncated.
///
/// This deliberately does NOT use the `tiff` decoder: it is what decides whether
/// the decoder can be opened at all (i.e. whether the fetched prefix contains the
/// whole header). Every read is bounds-checked; the chain is capped at
/// [`MAX_IFD_IMAGES`] so a cyclic `next IFD` pointer terminates; unrecognised
/// bytes yield an empty walk rather than an error.
///
/// Handles classic TIFF (12-byte entries, 4-byte offsets) and BigTIFF (20-byte
/// entries, 8-byte offsets), both byte orders.
pub fn walk_ifds(bytes: &[u8], mut visit: impl FnMut(&IfdEntry)) -> IfdWalk {
    let mut walk = IfdWalk {
        header_end: 0,
        truncated_at: None,
        images: 0,
    };
    let Some(order) = bytes.get(0..2) else {
        walk.truncated_at = Some(2);
        return walk;
    };
    let le = match order {
        b"II" => true,
        b"MM" => false,
        _ => return walk, // not a TIFF at all — nothing to ask for
    };
    let Some(magic) = read_u16(bytes, 2, le) else {
        walk.truncated_at = Some(4);
        return walk;
    };

    // (entry size, offset of the value field within an entry, first-IFD offset).
    let (entry_size, value_off, inline_cap, mut ifd) = match magic {
        42 => (
            12usize,
            8usize,
            4u64,
            read_u32(bytes, 4, le).map(|v| v as usize),
        ),
        43 => {
            // BigTIFF: offset size must be 8 (the only defined value).
            if read_u16(bytes, 4, le) != Some(8) {
                return walk;
            }
            walk.header_end = 16;
            (
                20usize,
                12usize,
                8u64,
                read_u64(bytes, 8, le).and_then(usize_of),
            )
        }
        _ => return walk,
    };
    walk.header_end = walk.header_end.max(if magic == 42 { 8 } else { 16 });
    if ifd.is_none() {
        walk.truncated_at = Some(walk.header_end);
        return walk;
    }

    for _ in 0..MAX_IFD_IMAGES {
        let Some(dir) = ifd.filter(|&o| o != 0) else {
            break;
        };
        // Entry count: u16 (classic) / u64 (BigTIFF).
        let count_len = if magic == 42 { 2usize } else { 8usize };
        let (count, first_entry) = match if magic == 42 {
            read_u16(bytes, dir, le).map(u64::from)
        } else {
            read_u64(bytes, dir, le)
        } {
            Some(n) => (n, dir + count_len),
            None => {
                // The directory header itself is past the prefix — ask for enough
                // to make progress next round (see PROBE_BYTES).
                walk.truncated_at = Some(dir as u64 + count_len as u64);
                walk.header_end = walk.header_end.max(dir as u64 + PROBE_BYTES);
                return walk;
            }
        };
        let Some(dir_end) = count
            .checked_mul(entry_size as u64)
            .and_then(|d| (first_entry as u64).checked_add(d))
            .and_then(|e| e.checked_add(if magic == 42 { 4 } else { 8 }))
        else {
            break; // absurd entry count — stop rather than overflow
        };
        walk.header_end = walk.header_end.max(dir_end);
        let Some(dir_end_us) = usize_of(dir_end) else {
            walk.truncated_at = Some(dir_end);
            return walk;
        };
        if dir_end_us > bytes.len() {
            // The directory's entries do not all fit: we know exactly how much we
            // need, so ask for it and re-walk.
            walk.truncated_at = Some(dir_end);
            return walk;
        }

        for i in 0..count {
            let Some(e) = usize::try_from(i)
                .ok()
                .and_then(|i| i.checked_mul(entry_size))
                .and_then(|d| first_entry.checked_add(d))
            else {
                break;
            };
            let (Some(tag), Some(field_type)) =
                (read_u16(bytes, e, le), read_u16(bytes, e + 2, le))
            else {
                break;
            };
            // Entry layout — classic: tag(2) type(2) count(4) value(4).
            //                BigTIFF: tag(2) type(2) count(8) value(8).
            // The count field starts at +4 in BOTH; only its width differs.
            let vcount = if magic == 42 {
                read_u32(bytes, e + 4, le).map(u64::from)
            } else {
                read_u64(bytes, e + 4, le)
            };
            let Some(vcount) = vcount else { break };
            let value_len = type_size(field_type).saturating_mul(vcount);
            let inline = value_len <= inline_cap;
            let value_offset = if inline {
                (e + value_off) as u64
            } else if magic == 42 {
                match read_u32(bytes, e + value_off, le) {
                    Some(v) => u64::from(v),
                    None => break,
                }
            } else {
                match read_u64(bytes, e + value_off, le) {
                    Some(v) => v,
                    None => break,
                }
            };
            if !inline {
                walk.header_end = walk.header_end.max(value_offset.saturating_add(value_len));
            }
            visit(&IfdEntry {
                tag,
                field_type,
                count: vcount,
                value_offset,
                value_len,
                inline,
            });
        }
        walk.images += 1;

        // Follow the chain to the next IFD (the pointer sits at the end of the dir).
        let next = dir_end_us - if magic == 42 { 4 } else { 8 };
        ifd = if magic == 42 {
            read_u32(bytes, next, le).map(|v| v as usize)
        } else {
            read_u64(bytes, next, le).and_then(usize_of)
        };
    }

    // Every out-of-line value must also be inside the prefix for the decoder to
    // read it — `header_end` already accounts for those, so a shortfall here means
    // the tag ARRAYS (TileOffsets/TileByteCounts/geo tags) land past the prefix.
    if walk.header_end > bytes.len() as u64 {
        walk.truncated_at = Some(walk.header_end);
    }
    walk
}

/// How much extra to ask for when even an IFD's entry count could not be read
/// (we cannot know the true requirement without it). One round of this reveals the
/// count, and the next walk asks for the exact number.
const PROBE_BYTES: u64 = 64 * 1024;

/// `u64 -> usize` without truncating on a 32-bit target (WASM): an offset past
/// `usize::MAX` cannot address these bytes anyway, so it reads as "absent".
fn usize_of(v: u64) -> Option<usize> {
    usize::try_from(v).ok()
}

fn read_u16(b: &[u8], off: usize, le: bool) -> Option<u16> {
    let s: [u8; 2] = b.get(off..off.checked_add(2)?)?.try_into().ok()?;
    Some(if le {
        u16::from_le_bytes(s)
    } else {
        u16::from_be_bytes(s)
    })
}

fn read_u32(b: &[u8], off: usize, le: bool) -> Option<u32> {
    let s: [u8; 4] = b.get(off..off.checked_add(4)?)?.try_into().ok()?;
    Some(if le {
        u32::from_le_bytes(s)
    } else {
        u32::from_be_bytes(s)
    })
}

fn read_u64(b: &[u8], off: usize, le: bool) -> Option<u64> {
    let s: [u8; 8] = b.get(off..off.checked_add(8)?)?.try_into().ok()?;
    Some(if le {
        u64::from_le_bytes(s)
    } else {
        u64::from_be_bytes(s)
    })
}

/// Scan the IFD chain for `PhotometricInterpretation = RGBPalette` and return the
/// byte patches that rewrite each to `BlackIsZero` (see the module invariant).
///
/// A SHORT of count 1 always fits inline, left-justified in the value field, so the
/// two bytes to rewrite sit in the entry itself — inside the header prefix, which is
/// exactly the region a range read always fetches.
fn photometric_patches(bytes: &[u8]) -> Vec<(u64, u8)> {
    let le = matches!(bytes.get(0..2), Some(b"II"));
    let repl = if le {
        PHOTOMETRIC_BLACK_IS_ZERO.to_le_bytes()
    } else {
        PHOTOMETRIC_BLACK_IS_ZERO.to_be_bytes()
    };
    let mut patches = Vec::new();
    walk_ifds(bytes, |e| {
        if e.tag != TAG_PHOTOMETRIC || e.field_type != TYPE_SHORT || !e.inline {
            return;
        }
        if read_u16(bytes, e.value_offset as usize, le) == Some(PHOTOMETRIC_RGB_PALETTE) {
            patches.push((e.value_offset, repl[0]));
            patches.push((e.value_offset + 1, repl[1]));
        }
    });
    patches
}

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

/// Open a decoder over the WHOLE tile bytes (classic TIFF or BigTIFF).
///
/// The convenience wrapper for the in-memory whole-file case; the windowed range
/// path uses [`open_cog`] over a sparse view.
///
/// # Errors
/// [`GisError::Tiff`] if the bytes are not a valid TIFF/BigTIFF header.
pub fn open(bytes: &[u8]) -> Result<CogDecoder<'_>, GisError> {
    open_cog(&CogBytes::whole(bytes))
}

/// Open a decoder over a (possibly sparse) [`CogBytes`] view, through the reader
/// that re-interprets a palette photometric tag (see the module invariant — this
/// is what lets the real ESA WorldCover class raster decode at all).
///
/// # Errors
/// [`GisError::Tiff`] if the bytes are not a valid TIFF/BigTIFF header.
pub fn open_cog<'a>(cog: &CogBytes<'a>) -> Result<CogDecoder<'a>, GisError> {
    let patches = photometric_patches(cog.header_prefix());
    Ok(Decoder::new(cog.open_reader(patches))?)
}

/// Read the base image [`CogHeader`] from an open decoder.
///
/// # Errors
/// - [`GisError::Tiff`] on a dimension-read failure.
/// - [`GisError::WindowOutOfBounds`] for degenerate (zero) chunk dimensions.
pub fn read_header(dec: &mut CogDecoder<'_>) -> Result<CogHeader, GisError> {
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
pub fn guard_image_count(cog: &CogBytes<'_>) -> Result<usize, GisError> {
    let mut dec = open_cog(cog)?;
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
