//! Hand-rolled minimal single-strip Float32 GeoTIFF encoder (D-20/D-21) — ZERO
//! new dependency.
//!
//! # Why hand-rolled
//! RESEARCH Package Legitimacy removed the brand-new `geotiff-writer`/`tiff-writer`
//! crates and deliberately skipped the mature `tiff` crate to keep the export path
//! zero-dep and matching the project's hand-roll ethos (the same call made for the
//! iso-band tracer vs `contour`). A minimal single-strip Float32 GeoTIFF is a
//! little-endian TIFF header + one IFD + the three GeoTIFF tags
//! (`GeoKeyDirectoryTag` + `ModelPixelScaleTag` + `ModelTiepointTag`) + one
//! uncompressed strip — well under 200 LOC we own outright.
//!
//! # What it encodes
//! - A `cols × rows` Float32 grayscale raster of the level grid, single strip,
//!   uncompressed, `SampleFormat = IEEE float`. NaN no-data holes ride through as
//!   Float32 NaN and are declared via the `GDAL_NODATA` tag.
//! - A north-up geotransform: raster row 0 is the NORTHERNMOST grid row (the grid
//!   is written bottom-to-top flipped), `ModelTiepointTag` maps raster `(0,0,0)` to
//!   the grid's north-west node in projected meters, `ModelPixelScaleTag` is the
//!   grid spacing on both axes.
//! - A projected CRS: `GTModelTypeGeoKey = Projected`, `GTRasterTypeGeoKey =
//!   PixelIsPoint` (the grid values are point samples at lattice nodes), and
//!   `ProjectedCSTypeGeoKey = <EPSG>` from the [`ExportMeta`].
//! - The full metadata/attribution footer (D-22) in `ImageDescription`.

use crate::export::ExportMeta;
use crate::grid::LevelGrid;

// TIFF field types (TIFF 6.0 §2).
const T_ASCII: u16 = 2;
const T_SHORT: u16 = 3;
const T_LONG: u16 = 4;
const T_DOUBLE: u16 = 12;

// Baseline + GeoTIFF tag ids used here (kept ascending — a TIFF IFD MUST list its
// entries in ascending tag order).
const TAG_IMAGE_WIDTH: u16 = 256;
const TAG_IMAGE_LENGTH: u16 = 257;
const TAG_BITS_PER_SAMPLE: u16 = 258;
const TAG_COMPRESSION: u16 = 259;
const TAG_PHOTOMETRIC: u16 = 262;
const TAG_IMAGE_DESCRIPTION: u16 = 270;
const TAG_STRIP_OFFSETS: u16 = 273;
const TAG_SAMPLES_PER_PIXEL: u16 = 277;
const TAG_ROWS_PER_STRIP: u16 = 278;
const TAG_STRIP_BYTE_COUNTS: u16 = 279;
const TAG_PLANAR_CONFIG: u16 = 284;
const TAG_SAMPLE_FORMAT: u16 = 339;
const TAG_MODEL_PIXEL_SCALE: u16 = 33550;
const TAG_MODEL_TIEPOINT: u16 = 33922;
const TAG_GEO_KEY_DIRECTORY: u16 = 34735;
const TAG_GDAL_NODATA: u16 = 42113;

/// One IFD entry with its value either inline (≤ 4 bytes) or spilled into the
/// out-of-line data region (an offset patched in during layout).
struct Field {
    tag: u16,
    typ: u16,
    count: u32,
    /// The 4-byte inline value, or `None` if the value lives in `spill`.
    inline: Option<[u8; 4]>,
    /// Out-of-line bytes (word-aligned when placed); empty when `inline` is set.
    spill: Vec<u8>,
}

impl Field {
    fn short(tag: u16, v: u16) -> Self {
        // A single SHORT is left-justified in the 4-byte value field (LE).
        Self {
            tag,
            typ: T_SHORT,
            count: 1,
            inline: Some([(v & 0xff) as u8, (v >> 8) as u8, 0, 0]),
            spill: Vec::new(),
        }
    }
    fn long(tag: u16, v: u32) -> Self {
        Self {
            tag,
            typ: T_LONG,
            count: 1,
            inline: Some(v.to_le_bytes()),
            spill: Vec::new(),
        }
    }
    fn ascii(tag: u16, s: &str) -> Self {
        let mut bytes = s.as_bytes().to_vec();
        bytes.push(0); // NUL terminator counts toward the ASCII count.
        Self {
            tag,
            typ: T_ASCII,
            count: bytes.len() as u32,
            inline: None,
            spill: bytes,
        }
    }
    fn doubles(tag: u16, vals: &[f64]) -> Self {
        let mut spill = Vec::with_capacity(vals.len() * 8);
        for v in vals {
            spill.extend_from_slice(&v.to_le_bytes());
        }
        Self {
            tag,
            typ: T_DOUBLE,
            count: vals.len() as u32,
            inline: None,
            spill,
        }
    }
    fn shorts(tag: u16, vals: &[u16]) -> Self {
        let mut spill = Vec::with_capacity(vals.len() * 2);
        for v in vals {
            spill.extend_from_slice(&v.to_le_bytes());
        }
        Self {
            tag,
            typ: T_SHORT,
            count: vals.len() as u32,
            inline: None,
            spill,
        }
    }
}

/// The GeoKeyDirectory SHORT payload for a projected CRS pinned to `epsg`.
///
/// Layout (GeoTIFF 1.0 §2.4): a 4-short header `(version=1, key_rev=1, minor=0,
/// num_keys)` followed by one 4-short entry `(key_id, tag_location=0, count=1,
/// value)` per key. `tag_location = 0` means the value IS the fourth short (an
/// inline SHORT key), which is all three keys here need.
fn geo_key_directory(epsg: u16) -> Vec<u16> {
    vec![
        1, 1, 0, 3, // header: v1.1.0, 3 keys
        1024, 0, 1, 1, // GTModelTypeGeoKey = ModelTypeProjected
        1025, 0, 1, 2, // GTRasterTypeGeoKey = RasterPixelIsPoint
        3072, 0, 1, epsg, // ProjectedCSTypeGeoKey = EPSG
    ]
}

/// Encode the level `grid` as a minimal single-strip Float32 GeoTIFF carrying the
/// EPSG + geotransform + the [`ExportMeta`] footer (D-20/D-21/D-22). Zero new dep.
///
/// An empty grid yields an empty byte vector (nothing to raster). NaN no-data
/// holes are preserved as Float32 NaN and declared via the `GDAL_NODATA` tag.
#[must_use]
pub fn encode_geotiff(grid: &LevelGrid, meta: &ExportMeta) -> Vec<u8> {
    if grid.is_empty() {
        return Vec::new();
    }
    let cols = grid.cols;
    let rows = grid.rows;
    let strip_bytes = (rows * cols * 4) as u32;

    // North-up geotransform: raster (0,0) is the NORTH-WEST node. The grid is
    // row-major south→north (row 0 = min_y), so the northernmost world Y is at the
    // last grid row. PixelIsPoint ⇒ the tiepoint maps a node centre exactly.
    let max_y = grid.origin[1] + (rows as f64 - 1.0) * grid.spacing_m;
    let pixel_scale = [grid.spacing_m, grid.spacing_m, 0.0];
    let tiepoint = [0.0, 0.0, 0.0, grid.origin[0], max_y, 0.0];
    let epsg = u16::try_from(meta.epsg).unwrap_or(0);

    // The IFD fields, ascending by tag. StripOffsets is patched with the real
    // pixel offset once the layout is known.
    let mut fields = vec![
        Field::long(TAG_IMAGE_WIDTH, cols as u32),
        Field::long(TAG_IMAGE_LENGTH, rows as u32),
        Field::short(TAG_BITS_PER_SAMPLE, 32),
        Field::short(TAG_COMPRESSION, 1), // none
        Field::short(TAG_PHOTOMETRIC, 1), // BlackIsZero
        Field::ascii(TAG_IMAGE_DESCRIPTION, &meta.one_line()),
        Field::long(TAG_STRIP_OFFSETS, 0), // patched below
        Field::short(TAG_SAMPLES_PER_PIXEL, 1),
        Field::long(TAG_ROWS_PER_STRIP, rows as u32),
        Field::long(TAG_STRIP_BYTE_COUNTS, strip_bytes),
        Field::short(TAG_PLANAR_CONFIG, 1),
        Field::short(TAG_SAMPLE_FORMAT, 3), // IEEE float
        Field::doubles(TAG_MODEL_PIXEL_SCALE, &pixel_scale),
        Field::doubles(TAG_MODEL_TIEPOINT, &tiepoint),
        Field::shorts(TAG_GEO_KEY_DIRECTORY, &geo_key_directory(epsg)),
        Field::ascii(TAG_GDAL_NODATA, "nan"),
    ];

    // Layout: [header 8][IFD][out-of-line spill][pixel strip].
    let n = fields.len();
    let ifd_len = 2 + n * 12 + 4;
    let extra_start = 8 + ifd_len;

    // Place each spilled value on a word boundary and record its offset.
    let mut extra = Vec::new();
    let mut offsets = vec![None; n];
    for (i, f) in fields.iter().enumerate() {
        if f.inline.is_none() {
            if extra.len() % 2 == 1 {
                extra.push(0); // TIFF offsets must be even.
            }
            offsets[i] = Some((extra_start + extra.len()) as u32);
            extra.extend_from_slice(&f.spill);
        }
    }
    if extra.len() % 2 == 1 {
        extra.push(0);
    }
    let pixel_offset = (extra_start + extra.len()) as u32;

    // Patch StripOffsets now that the pixel region location is known.
    for f in &mut fields {
        if f.tag == TAG_STRIP_OFFSETS {
            f.inline = Some(pixel_offset.to_le_bytes());
        }
    }

    // --- Emit ---
    let mut out = Vec::with_capacity(pixel_offset as usize + strip_bytes as usize);
    // Header: little-endian ("II"), magic 42, first-IFD offset = 8.
    out.extend_from_slice(b"II");
    out.extend_from_slice(&42u16.to_le_bytes());
    out.extend_from_slice(&8u32.to_le_bytes());

    // IFD: entry count, entries, next-IFD offset (0).
    out.extend_from_slice(&(n as u16).to_le_bytes());
    for (i, f) in fields.iter().enumerate() {
        out.extend_from_slice(&f.tag.to_le_bytes());
        out.extend_from_slice(&f.typ.to_le_bytes());
        out.extend_from_slice(&f.count.to_le_bytes());
        if let Some(inline) = f.inline {
            out.extend_from_slice(&inline);
        } else {
            out.extend_from_slice(
                &offsets[i]
                    .expect("spilled field has an offset")
                    .to_le_bytes(),
            );
        }
    }
    out.extend_from_slice(&0u32.to_le_bytes()); // no next IFD

    // Out-of-line spill region, then the pixel strip (north-up: flip grid rows).
    out.extend_from_slice(&extra);
    debug_assert_eq!(out.len() as u32, pixel_offset);
    for out_row in 0..rows {
        let src_row = rows - 1 - out_row;
        for col in 0..cols {
            let v = grid.values[src_row * cols + col] as f32;
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta() -> ExportMeta {
        ExportMeta {
            epsg: 32631,
            weighting_label: "dB(A)".to_string(),
            engine_version: "envi-test".to_string(),
            tensor_hash: "abc123".to_string(),
            attribution: "© OpenStreetMap contributors; ESA WorldCover".to_string(),
        }
    }

    /// A tiny fixture grid: 2 rows × 3 cols, spacing 10 m, origin (500000, 4000000),
    /// row-major south→north, with one NaN no-data hole.
    fn fixture() -> LevelGrid {
        LevelGrid {
            rows: 2,
            cols: 3,
            origin: [500_000.0, 4_000_000.0],
            spacing_m: 10.0,
            values: vec![10.0, 20.0, 30.0, 40.0, f64::NAN, 60.0],
        }
    }

    // --- A minimal dev-only TIFF reader (byte-offset decode, LE) ---

    fn rd_u16(b: &[u8], o: usize) -> u16 {
        u16::from_le_bytes([b[o], b[o + 1]])
    }
    fn rd_u32(b: &[u8], o: usize) -> u32 {
        u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
    }
    fn rd_f64(b: &[u8], o: usize) -> f64 {
        let mut a = [0u8; 8];
        a.copy_from_slice(&b[o..o + 8]);
        f64::from_le_bytes(a)
    }

    struct Entry {
        typ: u16,
        count: u32,
        value_off: usize, // offset of the 4-byte value/offset field
    }

    fn parse_ifd(b: &[u8]) -> std::collections::HashMap<u16, Entry> {
        assert_eq!(&b[0..2], b"II", "little-endian");
        assert_eq!(rd_u16(b, 2), 42, "TIFF magic");
        let ifd = rd_u32(b, 4) as usize;
        let n = rd_u16(b, ifd) as usize;
        let mut map = std::collections::HashMap::new();
        let mut prev_tag = 0u16;
        for i in 0..n {
            let e = ifd + 2 + i * 12;
            let tag = rd_u16(b, e);
            assert!(
                tag > prev_tag,
                "IFD tags must ascend: {prev_tag} then {tag}"
            );
            prev_tag = tag;
            map.insert(
                tag,
                Entry {
                    typ: rd_u16(b, e + 2),
                    count: rd_u32(b, e + 4),
                    value_off: e + 8,
                },
            );
        }
        map
    }

    #[test]
    fn geotiff_round_trips_float32_pixels_geokeys_and_geotransform() {
        let grid = fixture();
        let bytes = encode_geotiff(&grid, &meta());
        let ifd = parse_ifd(&bytes);

        // Dimensions + Float32 single-strip layout.
        assert_eq!(rd_u32(&bytes, ifd[&256].value_off), 3, "cols");
        assert_eq!(rd_u32(&bytes, ifd[&257].value_off), 2, "rows");
        assert_eq!(rd_u16(&bytes, ifd[&258].value_off), 32, "32 bits/sample");
        assert_eq!(rd_u16(&bytes, ifd[&259].value_off), 1, "uncompressed");
        assert_eq!(
            rd_u16(&bytes, ifd[&339].value_off),
            3,
            "SampleFormat=IEEE float"
        );
        assert_eq!(rd_u16(&bytes, ifd[&277].value_off), 1, "1 sample/pixel");
        let strip_off = rd_u32(&bytes, ifd[&273].value_off) as usize;
        assert_eq!(
            rd_u32(&bytes, ifd[&279].value_off),
            (2 * 3 * 4) as u32,
            "strip bytes"
        );

        // Pixel data is north-up (grid rows flipped): raster row 0 = grid row 1.
        let px = |r: usize, c: usize| -> f32 {
            let o = strip_off + (r * 3 + c) * 4;
            f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]])
        };
        assert_eq!(px(0, 0), 40.0); // grid[1][0]
        assert!(px(0, 1).is_nan()); // grid[1][1] = NaN hole preserved
        assert_eq!(px(0, 2), 60.0); // grid[1][2]
        assert_eq!(px(1, 0), 10.0); // grid[0][0]
        assert_eq!(px(1, 2), 30.0); // grid[0][2]

        // ModelPixelScale = [10, 10, 0].
        let ps = ifd[&33550].value_off;
        let ps_off = rd_u32(&bytes, ps) as usize;
        assert_eq!(ifd[&33550].count, 3);
        assert_eq!(rd_f64(&bytes, ps_off), 10.0);
        assert_eq!(rd_f64(&bytes, ps_off + 8), 10.0);

        // ModelTiepoint maps raster (0,0,0) → NW node (origin_x, max_y, 0).
        let tp = rd_u32(&bytes, ifd[&33922].value_off) as usize;
        assert_eq!(ifd[&33922].count, 6);
        assert_eq!(rd_f64(&bytes, tp), 0.0);
        assert_eq!(rd_f64(&bytes, tp + 8), 0.0);
        assert_eq!(rd_f64(&bytes, tp + 24), 500_000.0, "tiepoint world X");
        assert_eq!(
            rd_f64(&bytes, tp + 32),
            4_000_010.0,
            "tiepoint world Y (north edge)"
        );

        // GeoKeyDirectory: projected CRS with EPSG:32631 present.
        let gk = &ifd[&34735];
        assert_eq!(gk.typ, T_SHORT);
        let gk_off = rd_u32(&bytes, gk.value_off) as usize;
        let keys: Vec<u16> = (0..gk.count as usize)
            .map(|i| rd_u16(&bytes, gk_off + i * 2))
            .collect();
        assert_eq!(keys[3], 3, "3 geo keys");
        assert_eq!(keys[4], 1024, "GTModelTypeGeoKey");
        assert_eq!(keys[7], 1, "ModelTypeProjected");
        assert_eq!(keys[12], 3072, "ProjectedCSTypeGeoKey");
        assert_eq!(keys[15], 32631, "EPSG:32631");

        // The metadata footer rides in ImageDescription.
        let id = &ifd[&270];
        let id_off = rd_u32(&bytes, id.value_off) as usize;
        let desc = std::str::from_utf8(&bytes[id_off..id_off + id.count as usize - 1]).unwrap();
        assert!(desc.contains("EPSG:32631"));
        assert!(desc.contains("dB(A)"));
        assert!(desc.contains("OpenStreetMap"));

        // GDAL_NODATA declares the NaN hole convention.
        let nd = &ifd[&42113];
        let nd_off = rd_u32(&bytes, nd.value_off) as usize;
        assert_eq!(
            std::str::from_utf8(&bytes[nd_off..nd_off + nd.count as usize - 1]).unwrap(),
            "nan"
        );
    }

    #[test]
    fn empty_grid_encodes_to_no_bytes() {
        assert!(encode_geotiff(&LevelGrid::empty(10.0), &meta()).is_empty());
    }
}
