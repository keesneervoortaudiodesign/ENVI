//! **Windowed COG range planning** — the sans-I/O half of the two-pass range read.
//!
//! # Why this exists (the defect it fixes)
//!
//! A Cloud-Optimized GeoTIFF exists so a client can read the header, work out
//! which internal tiles overlap its window, and fetch ONLY those tiles' byte
//! ranges. Before this module, ENVI decoded COGs but never range-read them: an
//! Amsterdam import issued a plain whole-tile GET for AHN `M_25GN1`
//! (**330 MB**) — and 54 MB for ESA WorldCover — to serve a ~1 km² window. Both
//! upstreams answer `206 Partial Content`; nothing but the missing planner stood
//! in the way.
//!
//! # Module I/O (the boundary — `envi-gis` still never fetches)
//! - **Inputs:** the leading bytes of the file the caller has fetched (the header
//!   prefix, from offset `0`), a [`PixelWindow`] into the base image, the byte
//!   ranges the caller already holds (an OPFS cache), and an explicit fetch budget.
//! - **Output:** a [`ReadPlan`] — either "your prefix is too short, fetch `0..n`
//!   and ask again" ([`ReadPlan::need_header`]) or the exact, coalesced
//!   [`ByteRange`]s of the COG chunks overlapping the window, minus whatever the
//!   caller already has. **TypeScript owns `fetch`**; this crate only ever says
//!   *which bytes*.
//! - **Invariants (load-bearing):**
//!   1. **Guard-first**, mirroring [`crate::cog::window::decode_window`]: the IFD
//!      chain cap (T-08-02-02) and the fetch budget are enforced BEFORE any range
//!      is emitted, so a hostile IFD cannot make the client fetch the internet.
//!   2. **The header ask converges**: if the IFD chain or a tag array (e.g.
//!      `TileOffsets`) reaches past the prefix, the plan returns the exact byte
//!      count needed and the caller re-fetches — bounded by [`MAX_HEADER_BYTES`],
//!      beyond which it is a typed [`GisError::HeaderTooLarge`] rather than a
//!      creeping whole-file download.
//!   3. **Never a whole-file fallback.** There is no code path here that says "I
//!      could not plan, fetch everything" — that is the bug this module removes.

use crate::GisError;
use crate::cog::header::{self, CogHeader};
use crate::cog::sparse::{ByteRange, CogBytes, coalesce, subtract, total_bytes};
use crate::cog::window::PixelWindow;

/// The header prefix a caller should fetch on the FIRST pass (256 KiB).
///
/// A GDAL-written COG puts its whole IFD chain — including the `TileOffsets` /
/// `TileByteCounts` arrays and the geo tags — at the front of the file. AHN's
/// 330 MB BigTIFF kaartblad needs well under 100 KB of that; 256 KiB carries a
/// comfortable margin in a single request. When it is not enough,
/// [`ReadPlan::need_header`] says exactly how much is (invariant 2).
pub const DEFAULT_HEADER_PREFIX_BYTES: u64 = 256 * 1024;

/// Hard cap on the header prefix a plan will ever ask for (16 MiB). A file whose
/// header does not fit is not a COG in any useful sense; refusing is what keeps
/// the "fetch more header" loop from degenerating into a whole-file download.
pub const MAX_HEADER_BYTES: u64 = 16 * 1024 * 1024;

/// Default cap on the total bytes one window's plan may fetch (64 MiB) — the
/// network sibling of [`crate::cog::MAX_DECODED_PX`]. A window whose covering
/// tiles exceed this is a typed [`GisError::FetchBudgetExceeded`], not a silent
/// multi-hundred-MB download (this is the exact foot-gun the defect was).
pub const DEFAULT_MAX_FETCH_BYTES: u64 = 64 * 1024 * 1024;

/// Two adjacent COG chunk ranges separated by less than this are merged into one
/// request. COG tiles are written in chunk-index order, so a window's tile row is
/// usually contiguous; merging across a small hole trades a few unused bytes for
/// one fewer HTTP round-trip (and each round-trip to S3/PDOK costs far more than
/// 32 KiB of body).
pub const COALESCE_GAP_BYTES: u64 = 32 * 1024;

/// The plan for reading one window out of one COG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadPlan {
    /// `Some(n)` ⇒ the supplied prefix did not contain the whole header: fetch
    /// bytes `0..n` and plan again. When set, every other field is empty/zero.
    pub need_header: Option<u64>,
    /// The byte ranges the caller must still fetch (sorted, coalesced, disjoint,
    /// and already minus everything the caller said it holds). Empty when the
    /// cache already covers the window — the DATA-04 offline-replay case.
    pub fetch: Vec<ByteRange>,
    /// Bytes in [`ReadPlan::fetch`].
    pub fetch_bytes: u64,
    /// Bytes of ALL chunks the window needs (cached + to fetch) — the honest size
    /// of the window's footprint in the file, independent of cache state.
    pub window_bytes: u64,
    /// How many COG chunks the window overlaps (the count the planner selected —
    /// never "all of them").
    pub chunks: usize,
}

impl ReadPlan {
    /// The "fetch a longer header and re-plan" outcome.
    fn need_header(n: u64) -> Self {
        Self {
            need_header: Some(n),
            fetch: Vec::new(),
            fetch_bytes: 0,
            window_bytes: 0,
            chunks: 0,
        }
    }
}

/// Whether the fetched `prefix` holds the complete TIFF header (IFD chain + every
/// out-of-line tag value), and if not, how many leading bytes are needed.
///
/// # Errors
/// [`GisError::HeaderTooLarge`] if the header reaches past [`MAX_HEADER_BYTES`].
pub fn header_needed_bytes(prefix: &[u8]) -> Result<Option<u64>, GisError> {
    let walk = header::walk_ifds(prefix, |_| {});
    let need = walk.truncated_at.map(|_| walk.header_end);
    if let Some(n) = need
        && n > MAX_HEADER_BYTES
    {
        return Err(GisError::HeaderTooLarge {
            needed: n,
            limit: MAX_HEADER_BYTES,
        });
    }
    Ok(need)
}

/// Plan the byte ranges needed to decode `window` from the COG whose leading
/// bytes are `prefix`.
///
/// `have` is what the caller already holds (its OPFS cache's ranges, INCLUDING the
/// prefix — the planner adds `0..prefix.len()` itself, so a caller may pass an
/// empty slice on a cold cache).
///
/// # Errors
/// - [`GisError::HeaderTooLarge`] — the header does not fit in [`MAX_HEADER_BYTES`].
/// - [`GisError::FetchBudgetExceeded`] — the window's chunks exceed `max_fetch_bytes`.
/// - [`GisError::Tiff`] / [`GisError::MissingGeoTag`] / [`GisError::TooManyImages`] /
///   [`GisError::WindowOutOfBounds`] — from the IFD read.
pub fn plan_window_reads(
    prefix: &[u8],
    window: PixelWindow,
    have: &[ByteRange],
    max_fetch_bytes: u64,
) -> Result<ReadPlan, GisError> {
    // (1) Does the header even fit? Never guess — ask for exactly what is missing.
    if let Some(n) = header_needed_bytes(prefix)? {
        return Ok(ReadPlan::need_header(n));
    }

    let cog = CogBytes::whole(prefix);
    // (2) Guard the IFD chain BEFORE trusting any offset it declares (T-08-02-02).
    header::guard_image_count(&cog)?;
    let mut dec = header::open_cog(&cog)?;
    let hdr = header::read_header(&mut dec)?;

    // (3) Bounds: a window past ImageWidth/Length is a typed error here too, so a
    //     bad window never reaches the network (it would plan nonsense chunks).
    check_window_bounds(&hdr, window)?;

    // (4) The chunk grid -> chunk indices -> their declared byte ranges.
    let (offsets, counts) = chunk_ranges(&mut dec)?;
    let mut wanted = Vec::new();
    for idx in covering_chunks(&hdr, window) {
        let (Some(&start), Some(&len)) = (offsets.get(idx), counts.get(idx)) else {
            return Err(GisError::Tiff {
                message: format!(
                    "chunk {idx} is not declared in TileOffsets/TileByteCounts ({} entries)",
                    offsets.len()
                ),
            });
        };
        if len == 0 {
            continue; // an empty (sparse) COG tile: nothing to fetch, nothing to read
        }
        wanted.push(ByteRange {
            start,
            end: start.saturating_add(len),
        });
    }
    let chunks = wanted.len();
    let wanted = coalesce(wanted, COALESCE_GAP_BYTES);
    let window_bytes = total_bytes(&wanted);

    // (5) Fetch budget FIRST — before emitting a single range (the DoS/foot-gun
    //     guard; the analog of MAX_DECODED_PX, enforced on the network side).
    if window_bytes > max_fetch_bytes {
        return Err(GisError::FetchBudgetExceeded {
            requested: window_bytes,
            limit: max_fetch_bytes,
        });
    }

    // (6) Subtract what the caller already holds (the prefix always counts).
    let mut held = have.to_vec();
    held.push(ByteRange {
        start: 0,
        end: prefix.len() as u64,
    });
    let fetch = coalesce(subtract(&wanted, &held), COALESCE_GAP_BYTES);
    Ok(ReadPlan {
        need_header: None,
        fetch_bytes: total_bytes(&fetch),
        fetch,
        window_bytes,
        chunks,
    })
}

/// Reject a window reaching past `ImageWidth`/`ImageLength` (never plan padding).
fn check_window_bounds(hdr: &CogHeader, window: PixelWindow) -> Result<(), GisError> {
    if window.width == 0 || window.height == 0 {
        return Err(GisError::WindowOutOfBounds {
            what: format!("zero-extent window {}x{}", window.width, window.height),
        });
    }
    match (
        window.col_off.checked_add(window.width),
        window.row_off.checked_add(window.height),
    ) {
        (Some(ce), Some(re)) if ce <= hdr.width && re <= hdr.height => Ok(()),
        _ => Err(GisError::WindowOutOfBounds {
            what: format!(
                "window [{}+{}, {}+{}] exceeds image {}x{}",
                window.col_off, window.width, window.row_off, window.height, hdr.width, hdr.height
            ),
        }),
    }
}

/// The base image's chunk offsets + byte counts, from `TileOffsets`/`TileByteCounts`
/// (tiled) or `StripOffsets`/`StripByteCounts` (stripped). Read through the public
/// `tiff` tag API — the crate keeps its own copy private.
fn chunk_ranges(dec: &mut header::CogDecoder<'_>) -> Result<(Vec<u64>, Vec<u64>), GisError> {
    use tiff::tags::Tag;

    let tiled = dec.find_tag(Tag::TileOffsets)?.is_some();
    let (off_tag, len_tag) = if tiled {
        (Tag::TileOffsets, Tag::TileByteCounts)
    } else {
        (Tag::StripOffsets, Tag::StripByteCounts)
    };
    let offsets = dec
        .find_tag(off_tag)?
        .ok_or(GisError::MissingGeoTag {
            tag: "TileOffsets/StripOffsets",
        })?
        .into_u64_vec()?;
    let counts = dec
        .find_tag(len_tag)?
        .ok_or(GisError::MissingGeoTag {
            tag: "TileByteCounts/StripByteCounts",
        })?
        .into_u64_vec()?;
    if offsets.len() != counts.len() {
        return Err(GisError::Tiff {
            message: format!(
                "chunk offsets ({}) and byte counts ({}) disagree",
                offsets.len(),
                counts.len()
            ),
        });
    }
    Ok((offsets, counts))
}

/// The chunk indices the window overlaps — the SAME index arithmetic
/// [`crate::cog::window::decode_window`] uses to read them, so the planner can
/// never fetch a different set than the decoder reads.
fn covering_chunks(hdr: &CogHeader, window: PixelWindow) -> Vec<usize> {
    let across = hdr.chunks_across();
    let (cw, ch) = (hdr.chunk_w.max(1), hdr.chunk_h.max(1));
    let (tx0, tx1) = (
        window.col_off / cw,
        (window.col_off + window.width - 1) / cw,
    );
    let (ty0, ty1) = (
        window.row_off / ch,
        (window.row_off + window.height - 1) / ch,
    );
    let mut out = Vec::new();
    for ty in ty0..=ty1 {
        for tx in tx0..=tx1 {
            out.push((ty * across + tx) as usize);
        }
    }
    out
}
