//! A **sparse** (partially-fetched) view of a COG file — the range-read seam.
//!
//! # Module I/O
//! - **Inputs:** the byte ranges of a COG the TypeScript orchestrator has actually
//!   fetched (each a file offset + the bytes at that offset), as [`BytePart`]s.
//! - **Output:** a [`CogBytes`] the `tiff` decoder can read through
//!   ([`crate::cog::header::open_cog`]) exactly as if the whole file were in
//!   memory — provided it only touches bytes inside the fetched parts, which is
//!   precisely what [`crate::cog::plan`] guarantees.
//! - **Invariant (load-bearing): an unfetched byte is an ERROR, never a zero.**
//!   [`CogBytes`] does NOT zero-fill the gaps between parts. A read that starts
//!   inside a gap is an [`io::Error`] (which the `tiff` crate turns into a typed
//!   [`GisError::Tiff`]), so a mis-planned range surfaces as a loud decode failure
//!   instead of a raster quietly full of zeros — the same "never a silent 0.0"
//!   discipline the nodata path follows.
//! - **Invariant:** a read never crosses a part boundary; it returns a SHORT read
//!   at the end of a part. That is legal for [`Read`] (`read_exact` loops), and it
//!   is what keeps a gap from being spliced silently into the middle of a value.
//!
//! A whole in-memory tile is just the degenerate one-part case
//! ([`CogBytes::whole`]), so the pre-existing whole-file decode path and the new
//! windowed range path share ONE reader.

use std::io::{self, Read, Seek, SeekFrom};

use crate::GisError;

/// A half-open byte range `[start, end)` of the source file — what a caller must
/// `Range`-GET, and what a fetched [`BytePart`] occupies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ByteRange {
    /// First byte (inclusive).
    pub start: u64,
    /// One past the last byte (exclusive).
    pub end: u64,
}

impl ByteRange {
    /// Length in bytes (`0` for a degenerate/inverted range).
    #[must_use]
    pub fn len(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    /// Whether the range carries no bytes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }
}

/// One contiguous piece of the file the caller has already fetched: the bytes
/// found at file offset `offset`.
#[derive(Debug, Clone, Copy)]
pub struct BytePart<'a> {
    /// File offset of `bytes[0]`.
    pub offset: u64,
    /// The fetched bytes.
    pub bytes: &'a [u8],
}

impl BytePart<'_> {
    /// The file range this part covers.
    #[must_use]
    pub fn range(&self) -> ByteRange {
        ByteRange {
            start: self.offset,
            end: self.offset + self.bytes.len() as u64,
        }
    }
}

/// A partially-fetched COG: the sorted, disjoint [`BytePart`]s of the file that
/// are actually in memory.
///
/// The part list MUST begin at offset `0` — the TIFF/BigTIFF header and the IFD
/// chain live there, and every entry point ([`crate::cog::plan`],
/// [`crate::cog::header`], [`crate::cog::window`]) needs them.
#[derive(Debug, Clone)]
pub struct CogBytes<'a> {
    parts: Vec<BytePart<'a>>,
    /// Virtual file length: the end of the LAST part. Deliberately not the real
    /// file size — the browser cannot read `Content-Range` cross-origin (PDOK sends
    /// no `Access-Control-Expose-Headers`), so the real length is not knowable
    /// client-side, and the `tiff` decoder never needs it: it only ever
    /// `seek(SeekFrom::Start(..))` to an IFD-declared offset.
    len: u64,
}

impl<'a> CogBytes<'a> {
    /// The whole tile in memory — the degenerate single-part case (the legacy
    /// whole-tile decode path, and any fixture/test that holds the full file).
    #[must_use]
    pub fn whole(bytes: &'a [u8]) -> Self {
        Self {
            len: bytes.len() as u64,
            parts: vec![BytePart { offset: 0, bytes }],
        }
    }

    /// Build a sparse view from the fetched parts.
    ///
    /// Parts are sorted and **coalesced** (touching/overlapping parts are only
    /// accepted when they are exactly adjacent; an overlap is rejected rather than
    /// silently trusted, because two fetches of the same range that disagree mean
    /// the upstream changed under us).
    ///
    /// # Errors
    /// [`GisError::InvalidByteParts`] if the list is empty, does not start at
    /// offset `0` (no TIFF header), or contains overlapping parts.
    pub fn new(mut parts: Vec<BytePart<'a>>) -> Result<Self, GisError> {
        parts.retain(|p| !p.bytes.is_empty());
        if parts.is_empty() {
            return Err(GisError::InvalidByteParts {
                what: "no fetched byte parts".to_string(),
            });
        }
        parts.sort_by_key(|p| p.offset);
        if parts[0].offset != 0 {
            return Err(GisError::InvalidByteParts {
                what: format!(
                    "first part starts at offset {} — the TIFF header at offset 0 is required",
                    parts[0].offset
                ),
            });
        }
        let mut end = 0u64;
        for p in &parts {
            if p.offset < end {
                return Err(GisError::InvalidByteParts {
                    what: format!("part at offset {} overlaps the previous part", p.offset),
                });
            }
            end = p.range().end;
        }
        Ok(Self { parts, len: end })
    }

    /// The header prefix: the bytes of the part anchored at offset `0`.
    #[must_use]
    pub fn header_prefix(&self) -> &'a [u8] {
        self.parts[0].bytes
    }

    /// Virtual file length (end of the last part) — see the [`CogBytes::len`] field note.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.len
    }

    /// Whether the view carries no bytes at all (never true for a constructed view).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The ranges this view holds (what a cache already has — the planner subtracts
    /// them from what it would otherwise ask the caller to fetch).
    #[must_use]
    pub fn ranges(&self) -> Vec<ByteRange> {
        self.parts.iter().map(BytePart::range).collect()
    }

    /// A `Read + Seek` cursor over the view, with the palette photometric patch
    /// applied (see [`crate::cog::header`]).
    fn reader(&self, patches: Vec<(u64, u8)>) -> SparseReader<'a> {
        SparseReader {
            parts: self.parts.clone(),
            len: self.len,
            pos: 0,
            patches,
        }
    }

    /// Build the decoder-facing reader for this view.
    pub(crate) fn open_reader(&self, patches: Vec<(u64, u8)>) -> SparseReader<'a> {
        self.reader(patches)
    }
}

/// The `Read + Seek` cursor `tiff` decodes through. See the module invariants: a
/// gap is an error, never zeros; a read never crosses a part boundary.
#[derive(Debug)]
pub struct SparseReader<'a> {
    parts: Vec<BytePart<'a>>,
    len: u64,
    pos: u64,
    /// `(absolute file offset, replacement byte)` — the palette photometric
    /// rewrite, applied as bytes leave the reader (empty for a non-palette tile).
    patches: Vec<(u64, u8)>,
}

impl SparseReader<'_> {
    /// Index of the part containing `pos`, if any.
    fn part_at(&self, pos: u64) -> Option<&BytePart<'_>> {
        // Parts are sorted + disjoint: the candidate is the last one starting at
        // or before `pos`.
        let i = self.parts.partition_point(|p| p.offset <= pos);
        let p = self.parts.get(i.checked_sub(1)?)?;
        (pos < p.range().end).then_some(p)
    }
}

impl Read for SparseReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() || self.pos >= self.len {
            return Ok(0); // EOF past the last fetched byte.
        }
        let Some(part) = self.part_at(self.pos) else {
            // The decoder asked for a byte we never fetched. Loud, not zeros.
            return Err(io::Error::other(format!(
                "COG byte {} was not fetched (outside the planned ranges)",
                self.pos
            )));
        };
        let local = (self.pos - part.offset) as usize;
        let avail = part.bytes.len() - local;
        let n = avail.min(buf.len()); // SHORT read at the part boundary — never spans a gap.
        buf[..n].copy_from_slice(&part.bytes[local..local + n]);

        let start = self.pos;
        self.pos += n as u64;
        for &(off, byte) in &self.patches {
            if let Some(i) = off.checked_sub(start)
                && (i as usize) < n
            {
                buf[i as usize] = byte;
            }
        }
        Ok(n)
    }
}

impl Seek for SparseReader<'_> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let target = match pos {
            SeekFrom::Start(n) => Some(n),
            SeekFrom::End(d) => self.len.checked_add_signed(d),
            SeekFrom::Current(d) => self.pos.checked_add_signed(d),
        };
        let target = target.ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "seek to a negative COG offset")
        })?;
        self.pos = target;
        Ok(target)
    }
}

/// Sort + coalesce byte ranges, merging any two that overlap or sit within
/// `gap_tolerance` bytes of each other.
///
/// Merging across a small gap trades a few unused bytes for one fewer HTTP
/// request — COG tiles of one row are laid out consecutively, so this collapses a
/// window's tile row into a single `Range` GET. Empty ranges are dropped.
#[must_use]
pub fn coalesce(mut ranges: Vec<ByteRange>, gap_tolerance: u64) -> Vec<ByteRange> {
    ranges.retain(|r| !r.is_empty());
    ranges.sort_unstable();
    let mut out: Vec<ByteRange> = Vec::with_capacity(ranges.len());
    for r in ranges {
        match out.last_mut() {
            Some(prev) if r.start <= prev.end.saturating_add(gap_tolerance) => {
                prev.end = prev.end.max(r.end);
            }
            _ => out.push(r),
        }
    }
    out
}

/// `wanted` minus `have`: the sub-ranges of `wanted` not already covered.
///
/// This is what turns "the tiles this window needs" into "the bytes still to
/// fetch" against an OPFS cache — a re-import of the same viewport subtracts to
/// nothing and touches no network (DATA-04).
#[must_use]
pub fn subtract(wanted: &[ByteRange], have: &[ByteRange]) -> Vec<ByteRange> {
    let have = coalesce(have.to_vec(), 0);
    let mut out = Vec::new();
    for w in wanted {
        let mut cursor = w.start;
        for h in &have {
            if h.end <= cursor {
                continue; // entirely before the remaining piece
            }
            if h.start >= w.end {
                break; // `have` is sorted: nothing further can overlap
            }
            if h.start > cursor {
                out.push(ByteRange {
                    start: cursor,
                    end: h.start.min(w.end),
                });
            }
            cursor = cursor.max(h.end);
            if cursor >= w.end {
                break;
            }
        }
        if cursor < w.end {
            out.push(ByteRange {
                start: cursor,
                end: w.end,
            });
        }
    }
    out.retain(|r| !r.is_empty());
    out
}

/// Total bytes spanned by a (coalesced) range list.
#[must_use]
pub fn total_bytes(ranges: &[ByteRange]) -> u64 {
    ranges.iter().map(ByteRange::len).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(start: u64, end: u64) -> ByteRange {
        ByteRange { start, end }
    }

    #[test]
    fn coalesce_merges_overlapping_and_near_ranges() {
        let got = coalesce(vec![r(100, 200), r(0, 50), r(190, 260), r(300, 320)], 16);
        assert_eq!(got, vec![r(0, 50), r(100, 260), r(300, 320)]);
        // With a gap tolerance spanning the 40-byte hole, the last two merge too.
        let got = coalesce(vec![r(100, 260), r(300, 320)], 64);
        assert_eq!(got, vec![r(100, 320)]);
    }

    #[test]
    fn subtract_removes_cached_coverage() {
        // Fully cached -> nothing to fetch (the DATA-04 offline replay property).
        assert!(subtract(&[r(100, 200)], &[r(0, 500)]).is_empty());
        // Partially cached -> only the holes.
        assert_eq!(
            subtract(&[r(100, 400)], &[r(0, 150), r(200, 300)]),
            vec![r(150, 200), r(300, 400)]
        );
        // Nothing cached -> everything.
        assert_eq!(subtract(&[r(100, 200)], &[]), vec![r(100, 200)]);
    }

    #[test]
    fn parts_must_start_at_zero_and_not_overlap() {
        let bytes = [0u8; 16];
        assert!(matches!(
            CogBytes::new(vec![BytePart {
                offset: 8,
                bytes: &bytes
            }]),
            Err(GisError::InvalidByteParts { .. })
        ));
        assert!(matches!(
            CogBytes::new(vec![
                BytePart {
                    offset: 0,
                    bytes: &bytes
                },
                BytePart {
                    offset: 8,
                    bytes: &bytes
                },
            ]),
            Err(GisError::InvalidByteParts { .. })
        ));
        assert!(CogBytes::new(vec![]).is_err());
    }

    #[test]
    fn reader_reads_parts_and_errors_in_a_gap() {
        let head = [1u8, 2, 3, 4];
        let tail = [9u8, 8, 7];
        let cog = CogBytes::new(vec![
            BytePart {
                offset: 0,
                bytes: &head,
            },
            BytePart {
                offset: 100,
                bytes: &tail,
            },
        ])
        .expect("valid parts");
        assert_eq!(cog.len(), 103);

        let mut rd = cog.open_reader(Vec::new());
        let mut buf = [0u8; 4];
        rd.read_exact(&mut buf).expect("header part reads");
        assert_eq!(buf, head);

        // Seek into the fetched tail: reads fine.
        rd.seek(SeekFrom::Start(100)).unwrap();
        let mut t = [0u8; 3];
        rd.read_exact(&mut t).expect("tail part reads");
        assert_eq!(t, tail);

        // Seek into the GAP: an error, never zeros (the load-bearing invariant).
        rd.seek(SeekFrom::Start(50)).unwrap();
        let err = rd.read(&mut buf).expect_err("a gap byte must be an error");
        assert!(err.to_string().contains("not fetched"), "{err}");
    }

    #[test]
    fn reader_short_reads_at_a_part_boundary_instead_of_spanning_a_gap() {
        let head = [1u8, 2, 3, 4];
        let tail = [9u8, 8, 7];
        let cog = CogBytes::new(vec![
            BytePart {
                offset: 0,
                bytes: &head,
            },
            BytePart {
                offset: 100,
                bytes: &tail,
            },
        ])
        .unwrap();
        let mut rd = cog.open_reader(Vec::new());
        let mut buf = [0u8; 64];
        // Asking for 64 bytes at offset 0 yields only the 4 real ones.
        assert_eq!(rd.read(&mut buf).unwrap(), 4);
    }

    #[test]
    fn whole_is_a_single_part_view() {
        let bytes = [1u8, 2, 3, 4, 5];
        let cog = CogBytes::whole(&bytes);
        assert_eq!(cog.len(), 5);
        assert_eq!(cog.ranges(), vec![r(0, 5)]);
        assert_eq!(cog.header_prefix(), &bytes);
    }
}
