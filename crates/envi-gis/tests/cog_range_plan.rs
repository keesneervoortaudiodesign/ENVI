//! Windowed COG **range-read** oracle: the planner must select ONLY the chunks a
//! window overlaps, and decoding from just those fetched ranges must reproduce a
//! whole-file decode of the same window, bit for bit.
//!
//! # Why this test exists (the defect it pins)
//!
//! ENVI decoded COGs but never range-read them: an Amsterdam terrain import issued
//! a plain whole-tile GET for AHN `M_25GN1` — `Content-Length: 346,218,632`
//! (**330 MB**) — to serve a ~1 km² window (54 MB for ESA WorldCover, same cause).
//! The offline suite was green throughout, because every fixture was a few KB:
//! at that size, "we fetched the whole file" and "we fetched the tiles we needed"
//! are indistinguishable. So this suite runs on `range_tiled.tif`, a
//! **pixel-data-dominated** 256x256 / 32 px-tile COG (8x8 = 64 chunks, ~228 KB of
//! incompressible float32), where the byte fraction is a real, assertable number —
//! plus the BigTIFF fixture, because AHN is BigTIFF and its chunk offsets are
//! `LONG8`.
//!
//! The load-bearing properties, in order:
//! 1. the planner picks EXACTLY the chunks GDAL's own block grid says the window
//!    overlaps (the `chunks` list in the committed TOML, not our own arithmetic);
//! 2. the planned bytes are a small fraction of the file;
//! 3. the window decoded from ONLY those ranges equals the whole-file decode;
//! 4. an unfetched byte is a loud error, never a silent zero.

use envi_gis::GisError;
use envi_gis::cog::plan::{
    DEFAULT_MAX_FETCH_BYTES, MAX_HEADER_BYTES, header_needed_bytes, plan_window_reads,
};
use envi_gis::cog::sparse::{BytePart, ByteRange, CogBytes, total_bytes};
use envi_gis::cog::window::PixelWindow;
use envi_gis::cog::{MAX_DECODED_PX, decode_window, decode_window_cog};
use serde::Deserialize;

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cog/");

#[derive(Deserialize)]
struct Fixture {
    meta: Meta,
    case: Vec<Case>,
}

#[derive(Deserialize)]
struct Meta {
    fixture: String,
    file_bytes: u64,
    block: u32,
    width: u32,
    height: u32,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    col_off: u32,
    row_off: u32,
    win_width: u32,
    win_height: u32,
    /// The chunk indices GDAL's block grid says this window overlaps (the oracle).
    chunks: Vec<usize>,
}

impl Case {
    fn window(&self) -> PixelWindow {
        PixelWindow {
            col_off: self.col_off,
            row_off: self.row_off,
            width: self.win_width,
            height: self.win_height,
        }
    }
}

fn load_fixture() -> (Fixture, Vec<u8>) {
    let text = std::fs::read_to_string(format!("{DIR}range_tiled.toml"))
        .expect("range_tiled.toml must exist (regenerate with tools/gis_oracle)");
    let fx: Fixture = toml::from_str(&text).expect("range_tiled.toml parses");
    let bytes = std::fs::read(format!("{DIR}{}", fx.meta.fixture)).expect("range_tiled.tif exists");
    (fx, bytes)
}

/// Simulate the TS two-pass fetch **without any network**: hand the planner a
/// prefix, honour a `need_header` re-ask, then "fetch" the planned ranges by
/// slicing the file — exactly what a `Range` GET returns. Returns the parts (as
/// owned byte vectors, mirroring what TS caches in OPFS) and the plan's numbers.
struct RangeFetch {
    parts: Vec<(u64, Vec<u8>)>,
    fetched_bytes: u64,
    requests: usize,
    chunks: usize,
    window_bytes: u64,
}

fn range_fetch(file: &[u8], window: PixelWindow, prefix_len: usize) -> RangeFetch {
    let mut prefix_len = prefix_len.min(file.len());
    let mut requests = 1; // the header GET

    // Pass 1: the header prefix, re-asked until the whole header fits (bounded).
    let plan = loop {
        let prefix = &file[..prefix_len];
        let plan = plan_window_reads(prefix, window, &[], DEFAULT_MAX_FETCH_BYTES)
            .expect("planning succeeds");
        match plan.need_header {
            Some(n) => {
                assert!(
                    n > prefix_len as u64,
                    "a header re-ask must ASK FOR MORE than it had ({n} vs {prefix_len}) — \
                     otherwise the two-pass loop cannot converge"
                );
                assert!(n <= MAX_HEADER_BYTES);
                prefix_len = (n as usize).min(file.len());
                requests += 1;
            }
            None => break plan,
        }
    };

    // Pass 2: fetch exactly the planned ranges (one request each — a single-range
    // GET, which is what keeps the CORS request-header safelist happy).
    let mut parts = vec![(0u64, file[..prefix_len].to_vec())];
    let mut fetched_bytes = prefix_len as u64;
    for r in &plan.fetch {
        let (s, e) = (r.start as usize, (r.end as usize).min(file.len()));
        parts.push((r.start, file[s..e].to_vec()));
        fetched_bytes += (e - s) as u64;
        requests += 1;
    }
    assert_eq!(
        total_bytes(&plan.fetch),
        plan.fetch_bytes,
        "fetch_bytes must equal the sum of the emitted ranges"
    );
    RangeFetch {
        parts,
        fetched_bytes,
        requests,
        chunks: plan.chunks,
        window_bytes: plan.window_bytes,
    }
}

fn cog_of(parts: &[(u64, Vec<u8>)]) -> CogBytes<'_> {
    CogBytes::new(
        parts
            .iter()
            .map(|(offset, bytes)| BytePart {
                offset: *offset,
                bytes: bytes.as_slice(),
            })
            .collect(),
    )
    .expect("the fetched parts form a valid sparse view")
}

/// PROPERTY 1 + 2 + 3, per committed case: the planner selects exactly the chunks
/// GDAL says overlap, fetches a small fraction of the file, and the window decoded
/// from ONLY those ranges is IDENTICAL to the whole-file decode.
#[test]
fn range_plan_selects_only_overlapping_tiles_and_decodes_identically() {
    let (fx, file) = load_fixture();
    assert_eq!(
        file.len() as u64,
        fx.meta.file_bytes,
        "fixture size drifted from the oracle"
    );

    for case in &fx.case {
        let window = case.window();
        let got = range_fetch(&file, window, 8 * 1024);

        // (1) EXACTLY the overlapping chunks — compared against GDAL's block grid,
        //     not against our own arithmetic restated.
        assert_eq!(
            got.chunks,
            case.chunks.len(),
            "{}: planner selected {} chunks, GDAL's block grid says {} ({:?})",
            case.name,
            got.chunks,
            case.chunks.len(),
            case.chunks
        );
        let total_chunks = (fx.meta.width.div_ceil(fx.meta.block)
            * fx.meta.height.div_ceil(fx.meta.block)) as usize;
        assert!(
            got.chunks < total_chunks,
            "{}: a window must never need every chunk in the image",
            case.name
        );

        // (2) The byte reduction — the whole point. A small window must cost a
        //     small fraction of the file (the defect fetched 100 %).
        let fraction = got.fetched_bytes as f64 / file.len() as f64;
        assert!(
            fraction < 0.35,
            "{}: fetched {} of {} bytes ({:.1} %) — a windowed range read must be a \
             SMALL fraction of the file",
            case.name,
            got.fetched_bytes,
            file.len(),
            fraction * 100.0
        );
        assert!(
            got.window_bytes < file.len() as u64,
            "{}: the window's chunk footprint must be smaller than the file",
            case.name
        );

        // (3) Correctness: the sparse decode == the whole-file decode, exactly.
        let sparse = decode_window_cog(&cog_of(&got.parts), window, MAX_DECODED_PX)
            .unwrap_or_else(|e| panic!("{}: sparse decode failed: {e}", case.name));
        let whole = decode_window(&file, window, MAX_DECODED_PX)
            .unwrap_or_else(|e| panic!("{}: whole-file decode failed: {e}", case.name));
        assert_eq!(
            sparse, whole,
            "{}: the range-read window must equal the whole-file window",
            case.name
        );
        // ... and it is not vacuously empty.
        assert_eq!(
            sparse.samples.len(),
            (case.win_width * case.win_height) as usize
        );
        assert!(sparse.samples.iter().all(Option::is_some));
    }
}

/// The headline number, stated as a test: a small window costs a few percent of
/// the file, in a handful of requests — the shape of the fix that takes AHN's
/// 330 MB whole-tile GET down to a windowed read.
#[test]
fn a_small_window_costs_a_few_percent_of_the_file_in_a_few_requests() {
    let (fx, file) = load_fixture();
    let case = fx
        .case
        .iter()
        .find(|c| c.name == "four-tile 20x20")
        .expect("the four-tile case is committed");

    let got = range_fetch(&file, case.window(), 8 * 1024);
    assert_eq!(
        got.chunks, 4,
        "a 20x20 window straddling 2x2 tiles = 4 chunks"
    );
    // 4 of 64 chunks ~ 6 % of the pixel data; the 8 KB header prefix is the rest.
    assert!(
        got.fetched_bytes * 4 < file.len() as u64,
        "fetched {} of {} bytes — expected well under a quarter of the file",
        got.fetched_bytes,
        file.len()
    );
    assert!(
        got.requests <= 6,
        "a 4-chunk window must not explode into {} requests (adjacent ranges are coalesced)",
        got.requests
    );
    println!(
        "range read: {} bytes in {} requests ({:.1} % of the {}-byte file), {} chunks",
        got.fetched_bytes,
        got.requests,
        100.0 * got.fetched_bytes as f64 / file.len() as f64,
        file.len(),
        got.chunks
    );
}

/// The BigTIFF path: AHN is BigTIFF, whose chunk offsets are `LONG8` and whose IFD
/// entries are 20 bytes. A planner that only understood classic TIFF would plan
/// garbage ranges here — and AHN is the 330 MB tile that started this.
#[test]
fn bigtiff_chunk_offsets_are_planned_and_decoded() {
    let file = std::fs::read(format!("{DIR}ahn_bigtiff.tif")).expect("ahn_bigtiff.tif exists");
    assert_eq!(
        &file[0..4],
        &[0x49, 0x49, 0x2B, 0x00],
        "coverage: the fixture must genuinely be BigTIFF"
    );
    // 64x48 image, 16 px tiles => 4x3 = 12 chunks. A 6x6 window at (13,13)
    // straddles the tile grid in both axes => 4 chunks.
    let window = PixelWindow {
        col_off: 13,
        row_off: 13,
        width: 6,
        height: 6,
    };
    let got = range_fetch(&file, window, 1024);
    assert_eq!(
        got.chunks, 4,
        "the cross-tile window covers 4 BigTIFF tiles"
    );
    assert!(got.chunks < 12, "never the whole 4x3 chunk grid");

    let sparse = decode_window_cog(&cog_of(&got.parts), window, MAX_DECODED_PX)
        .expect("BigTIFF sparse decode");
    let whole = decode_window(&file, window, MAX_DECODED_PX).expect("BigTIFF whole decode");
    assert_eq!(
        sparse, whole,
        "BigTIFF range read must match the whole read"
    );
}

/// The palette (ESA WorldCover) path survives a range read: the photometric patch
/// lives in the IFD, which is always inside the header prefix, so the sparse view
/// re-interprets it exactly as the whole-file view does.
#[test]
fn palette_worldcover_decodes_through_a_range_read() {
    use envi_gis::cog::{decode_window_u8, decode_window_u8_cog};

    let file =
        std::fs::read(format!("{DIR}worldcover_palette.tif")).expect("worldcover fixture exists");
    let window = PixelWindow {
        col_off: 12,
        row_off: 12,
        width: 8,
        height: 8,
    };
    let got = range_fetch(&file, window, 1024);
    let sparse =
        decode_window_u8_cog(&cog_of(&got.parts), window, MAX_DECODED_PX).expect("sparse u8");
    let whole = decode_window_u8(&file, window, MAX_DECODED_PX).expect("whole u8");
    assert_eq!(sparse, whole, "the palette class codes must survive intact");
    assert!(sparse.samples.iter().all(Option::is_some));
}

/// A cache that already holds the window's ranges plans ZERO fetches — the DATA-04
/// network-off replay property, decided in the planner rather than hoped for.
#[test]
fn a_cached_window_plans_no_fetch_at_all() {
    let (fx, file) = load_fixture();
    let case = &fx.case[0];
    let window = case.window();

    let prefix_len = 8 * 1024usize;
    let first =
        plan_window_reads(&file[..prefix_len], window, &[], DEFAULT_MAX_FETCH_BYTES).expect("plan");
    assert!(first.need_header.is_none() && !first.fetch.is_empty());

    // Re-plan with those ranges declared as cached: nothing left to fetch.
    let again = plan_window_reads(
        &file[..prefix_len],
        window,
        &first.fetch,
        DEFAULT_MAX_FETCH_BYTES,
    )
    .expect("re-plan");
    assert!(
        again.fetch.is_empty() && again.fetch_bytes == 0,
        "a warm cache must plan no fetch, got {:?}",
        again.fetch
    );
    // But it still knows the window's true footprint (cache state does not lie
    // about how big the window is).
    assert_eq!(again.window_bytes, first.window_bytes);
}

/// A prefix too short to hold the header does NOT fall back to the whole file: it
/// asks for exactly the bytes the header needs, and the ask converges.
#[test]
fn a_short_prefix_asks_for_the_exact_header_length() {
    let (_fx, file) = load_fixture();
    // 64 bytes: enough for the TIFF magic, nowhere near the IFD. Each re-ask must
    // STRICTLY grow (otherwise the two-pass loop spins forever) and terminate fast.
    let mut have = 64u64;
    let mut rounds = 0;
    let header_len = loop {
        match header_needed_bytes(&file[..have as usize])
            .expect("a short prefix re-asks, never errors")
        {
            None => break have,
            Some(need) => {
                assert!(need > have, "a re-ask must grow: {need} <= {have}");
                assert!(need <= MAX_HEADER_BYTES && need <= file.len() as u64);
                have = need;
                rounds += 1;
                assert!(rounds < 8, "the header ask must converge, not creep");
            }
        }
    };
    // The header is a tiny fraction of the file — the whole reason a prefix GET
    // (and then only the overlapping tiles) beats downloading the file.
    assert!(
        header_len * 20 < file.len() as u64,
        "header {header_len} B of {} B — a COG header must be small",
        file.len()
    );
    // And the DEFAULT prefix (256 KiB) covers it outright: one header request.
    assert_eq!(
        header_needed_bytes(
            &file[..file
                .len()
                .min(envi_gis::cog::plan::DEFAULT_HEADER_PREFIX_BYTES as usize)]
        )
        .expect("default prefix walk"),
        None,
        "the default header prefix must hold this COG's header in ONE request"
    );
}

/// REGRESSION: the **out-of-line tag arrays** must be part of the header ask.
///
/// In a tiled COG, `TileOffsets` (324) and `TileByteCounts` (325) do NOT fit in an
/// IFD entry's 4/8-byte value slot — the entry holds a FILE OFFSET and the values
/// live elsewhere (hundreds of KB for a big tile). `ColorMap` (320, 768 entries) is
/// the same, and the palette rewrite depends on reaching it. A planner that only
/// counted the IFD *directories* would declare the header "complete" while
/// `TileOffsets` was still unfetched, then plan garbage byte ranges from an
/// unreadable array — or never converge.
///
/// So: assert from the RAW IFD that these arrays really are out-of-line in the
/// fixtures, that a prefix stopping short of them is reported as insufficient, and
/// that the ask reaches PAST each array's end (offset + length) and converges.
#[test]
fn out_of_line_tag_arrays_are_included_in_the_header_ask() {
    use envi_gis::cog::header::{IfdEntry, walk_ifds};

    const TILE_OFFSETS: u16 = 324;
    const TILE_BYTE_COUNTS: u16 = 325;
    const COLOR_MAP: u16 = 320;

    for (name, tags) in [
        ("range_tiled.tif", vec![TILE_OFFSETS, TILE_BYTE_COUNTS]),
        (
            "worldcover_palette.tif",
            vec![TILE_OFFSETS, TILE_BYTE_COUNTS, COLOR_MAP],
        ),
        ("ahn_bigtiff.tif", vec![TILE_OFFSETS, TILE_BYTE_COUNTS]),
    ] {
        let file = std::fs::read(format!("{DIR}{name}")).expect("fixture exists");
        let mut seen: Vec<IfdEntry> = Vec::new();
        let walk = walk_ifds(&file, |e| seen.push(*e));
        assert_eq!(
            walk.truncated_at, None,
            "{name}: whole file holds the header"
        );

        for tag in tags {
            let entries: Vec<&IfdEntry> = seen.iter().filter(|e| e.tag == tag).collect();
            assert!(!entries.is_empty(), "{name}: tag {tag} must be present");
            // Coverage: at least the base image's array is genuinely OUT-OF-LINE —
            // otherwise this test is vacuous and would not have caught the bug.
            let out_of_line: Vec<&&IfdEntry> = entries.iter().filter(|e| !e.inline).collect();
            assert!(
                !out_of_line.is_empty(),
                "{name}: tag {tag} must be out-of-line (value in the entry would make \
                 this fixture unlike the real product)"
            );
            for e in out_of_line {
                let end = e.value_offset + e.value_len;
                // The header ask must reach past the ARRAY, not merely past the IFD.
                assert!(
                    walk.header_end >= end,
                    "{name}: header_end {} does not cover tag {tag}'s array [{}, {})",
                    walk.header_end,
                    e.value_offset,
                    end
                );
                // And a prefix that stops one byte short of the array's end is
                // reported as INSUFFICIENT (not silently accepted).
                let short = header_needed_bytes(&file[..(end - 1) as usize])
                    .expect("a short prefix re-asks, never errors");
                assert_eq!(
                    short,
                    Some(walk.header_end),
                    "{name}: a prefix ending inside tag {tag}'s out-of-line array must ask \
                     for the full header ({}), got {short:?}",
                    walk.header_end
                );
            }
        }

        // And the ask converges: fetching exactly `header_end` bytes completes it.
        assert_eq!(
            header_needed_bytes(&file[..walk.header_end as usize]).expect("walk"),
            None,
            "{name}: the header ask must converge at header_end"
        );
    }
}

/// The network-side DoS guard: a window whose chunks exceed the fetch budget is a
/// typed error BEFORE any range is emitted — the guard that makes an accidental
/// 330 MB download unreachable (the network sibling of `MAX_DECODED_PX`).
#[test]
fn an_oversized_window_is_refused_before_any_range_is_emitted() {
    let (fx, file) = load_fixture();
    let full = PixelWindow {
        col_off: 0,
        row_off: 0,
        width: fx.meta.width,
        height: fx.meta.height,
    };
    let budget = 4096; // far below the whole image's chunk bytes
    let err = plan_window_reads(&file[..32 * 1024], full, &[], budget).unwrap_err();
    match err {
        GisError::FetchBudgetExceeded { requested, limit } => {
            assert_eq!(limit, budget);
            assert!(requested > budget, "{requested} vs {budget}");
        }
        other => panic!("expected FetchBudgetExceeded, got {other:?}"),
    }
}

/// PROPERTY 4: a byte the plan did not fetch is a LOUD error, never a silent zero.
/// This is what makes a mis-planned range a failed import instead of a raster full
/// of fake 0.0 m elevations.
#[test]
fn an_unfetched_chunk_is_an_error_not_a_field_of_zeros() {
    let (fx, file) = load_fixture();
    let case = fx
        .case
        .iter()
        .find(|c| c.name == "four-tile 20x20")
        .expect("committed");
    let window = case.window();

    // Fetch the header ONLY — none of the chunk ranges.
    let prefix = file[..32 * 1024].to_vec();
    let cog = CogBytes::new(vec![BytePart {
        offset: 0,
        bytes: &prefix,
    }])
    .expect("header-only view");

    let err = decode_window_cog(&cog, window, MAX_DECODED_PX)
        .expect_err("decoding an unfetched chunk must fail");
    assert!(
        matches!(err, GisError::Tiff { .. }),
        "expected a typed decode error, got {err:?}"
    );
}

/// Byte parts that do not start at the TIFF header, or that overlap, are rejected —
/// a malformed cache can never be handed to the decoder as if it were a file.
#[test]
fn malformed_byte_parts_are_rejected() {
    let bytes = [0u8; 32];
    assert!(matches!(
        CogBytes::new(vec![BytePart {
            offset: 64,
            bytes: &bytes
        }]),
        Err(GisError::InvalidByteParts { .. })
    ));
}

/// A range list is emitted sorted, disjoint and coalesced — the contract the TS
/// fetcher relies on to issue ONE single-range GET per entry (a multi-range
/// request is not CORS-safelisted and would trigger a preflight PDOK refuses).
#[test]
fn emitted_ranges_are_sorted_disjoint_and_nonempty() {
    let (fx, file) = load_fixture();
    for case in &fx.case {
        let plan = plan_window_reads(
            &file[..32 * 1024],
            case.window(),
            &[],
            DEFAULT_MAX_FETCH_BYTES,
        )
        .expect("plan");
        let mut prev = ByteRange { start: 0, end: 0 };
        for r in &plan.fetch {
            assert!(!r.is_empty(), "{}: empty range emitted", case.name);
            assert!(
                r.start >= prev.end,
                "{}: ranges must be sorted + disjoint ({:?} after {:?})",
                case.name,
                r,
                prev
            );
            prev = *r;
        }
    }
}
