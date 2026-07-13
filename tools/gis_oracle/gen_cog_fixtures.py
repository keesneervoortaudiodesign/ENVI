"""Generate the committed COG decode oracle fixtures for ``envi-gis``.

Writes a small set of synthetic Cloud-Optimized-GeoTIFF fixtures plus a
per-fixture expected-window TOML under
``crates/envi-gis/tests/fixtures/cog/``. These pin the pure-Rust ``tiff``-crate
decode path (``envi-gis::cog``) against **GDAL** ground truth (via rasterio),
the same committed-oracle pattern as ``tools/crs_oracle/gen_utm.py`` and
``tools/nord2000_oracle``.

The set deliberately spans the format edge cases from 08-RESEARCH:

* ``ahn_bigtiff.tif``      — **BigTIFF** container (AHN analog). First bytes are
  the BigTIFF magic ``49 49 2B 00``; the suite therefore CANNOT pass without
  BigTIFF support (Pitfall 1).
* ``glo30_predictor3.tif`` — DEFLATE + floating-point **predictor 3** (GLO-30
  analog), with non-nominal, non-square pixel sizes so a decoder that assumes a
  nominal grid is caught (Pitfall 5 / A3).
* ``nodata_edge.tif``      — a ``GDAL_NODATA`` sentinel plus dimensions that are
  not a tile multiple, so the bottom/right tiles are **padded beyond
  ImageWidth/Length** (Pitfall 7).
* ``bomb.tif``             — a decompression bomb: large declared IFD dimensions
  vs constant data (a few KB on disk). Exercises the pre-decode ``max_decoded_px``
  budget (threat T-08-02-01); the test passes a small budget so the IFD dwarfs
  it. No expected-window TOML (it is a reject case).
* ``worldcover_palette.tif`` — a **PALETTE** (``PhotometricInterpretation =
  RGBPalette``) 8-bit class raster, exactly like the REAL ESA WorldCover v200
  product (whose IFD carries photometric 3 + a 768-entry ColorMap). This is the
  fixture the suite was missing: a plain grayscale u8 fixture decodes fine and so
  hid the fact that the real tile was rejected outright by the ``tiff`` crate.
  The palette is only a colour lookup — the CLASS CODE is the sample value, so
  the decoder must return the raw index (10, 20, ..., 95), never an RGB triple.

Regeneration is **operator-driven** (``python gen_cog_fixtures.py``): rasterio /
GDAL are a **dev-time install only, NOT a build or test dependency** — the Rust
tests read the committed bytes + TOMLs. Each TOML header records a sha256 of
this generator so a fixture traces to the exact oracle that produced it.
"""

from __future__ import annotations

import hashlib
import math
from pathlib import Path

import numpy as np
import rasterio
from rasterio.transform import Affine
from rasterio.windows import Window

# Output directory (repo-root relative).
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
OUT = ROOT / "crates/envi-gis/tests/fixtures/cog"

GEN_SHA = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()[:16]
GDAL_VER = rasterio.__gdal_version__
RIO_VER = rasterio.__version__

# Oracle agreement tolerance (f32 round-trip through DEFLATE/predictor is exact,
# but keep a small slack for the predictor-3 byte reshuffle).
TOL = 1e-4


def _fmt(x: float) -> str:
    """Format a float for TOML: exact repr, or the bare ``nan`` token."""
    if isinstance(x, float) and math.isnan(x):
        return "nan"
    return repr(float(x))


def _samples_block(rows: list[list[float]]) -> str:
    """Render a row-major 2-D sample grid as a nested TOML array."""
    inner = ",\n  ".join("[" + ", ".join(_fmt(v) for v in row) + "]" for row in rows)
    return "[\n  " + inner + ",\n]"


def _write_cog(
    name: str,
    data: np.ndarray,
    transform: Affine,
    epsg: int,
    *,
    bigtiff: bool,
    predictor: int,
    nodata: float | None,
    blocksize: int,
) -> Path:
    """Write one tiled DEFLATE GeoTIFF fixture with rasterio/GDAL and return its path."""
    path = OUT / name
    height, width = data.shape
    profile = {
        "driver": "GTiff",
        "dtype": "float32",
        "count": 1,
        "width": width,
        "height": height,
        "crs": rasterio.crs.CRS.from_epsg(epsg),
        "transform": transform,
        "tiled": True,
        "blockxsize": blocksize,
        "blockysize": blocksize,
        "compress": "deflate",
        "predictor": predictor,
        "BIGTIFF": "YES" if bigtiff else "NO",
    }
    if nodata is not None:
        profile["nodata"] = nodata
    with rasterio.open(path, "w", **profile) as dst:
        dst.write(data.astype("float32"), 1)
    return path


def _expected_cases(path: Path, cases: list[dict], nodata: float | None) -> list[dict]:
    """Read each pixel window back from GDAL as ground truth (nodata -> nan)."""
    out = []
    with rasterio.open(path) as src:
        for c in cases:
            win = Window(c["col_off"], c["row_off"], c["win_width"], c["win_height"])
            arr = src.read(1, window=win).astype("float64")
            if nodata is not None:
                arr = np.where(arr == float(nodata), np.nan, arr)
            out.append({**c, "samples": arr.tolist()})
    return out


def _write_toml(
    name: str,
    *,
    fixture: str,
    bigtiff: bool,
    predictor: int,
    width: int,
    height: int,
    transform: Affine,
    nodata: float | None,
    cases: list[dict],
) -> None:
    lines: list[str] = []
    lines.append("# generated by tools/gis_oracle/gen_cog_fixtures.py — DO NOT EDIT")
    lines.append("# COG decode oracle: GDAL (via rasterio) ground truth vs the pure-Rust")
    lines.append("# `tiff` crate decode path in envi-gis::cog. Python is NOT a test dep.")
    lines.append("")
    lines.append("[meta]")
    lines.append(f'oracle = "GDAL {GDAL_VER} via rasterio {RIO_VER}"')
    lines.append(f'provenance = "gen_cog_fixtures.py sha256:{GEN_SHA}"')
    lines.append(f'fixture = "{fixture}"')
    lines.append(f"bigtiff = {'true' if bigtiff else 'false'}")
    lines.append(f"predictor = {predictor}")
    lines.append(f"tol = {_fmt(TOL)}")
    lines.append(f"width = {width}")
    lines.append(f"height = {height}")
    # North-up geotransform derived from ModelPixelScale/ModelTiepoint (Pitfall 5:
    # never assumed — the Rust geo_tags parser must reproduce these exact values).
    lines.append(f"origin_x = {_fmt(transform.c)}")
    lines.append(f"origin_y = {_fmt(transform.f)}")
    lines.append(f"pixel_size_x = {_fmt(transform.a)}")
    lines.append(f"pixel_size_y = {_fmt(transform.e)}")
    lines.append(f"nodata = {_fmt(nodata if nodata is not None else math.nan)}")
    lines.append(f"has_nodata = {'true' if nodata is not None else 'false'}")
    lines.append("")
    for c in cases:
        lines.append("[[case]]")
        lines.append(f'name = "{c["name"]}"')
        lines.append(f"col_off = {c['col_off']}")
        lines.append(f"row_off = {c['row_off']}")
        lines.append(f"win_width = {c['win_width']}")
        lines.append(f"win_height = {c['win_height']}")
        lines.append(f"samples = {_samples_block(c['samples'])}")
        lines.append("")
    (OUT / name).write_text("\n".join(lines) + "\n", encoding="utf-8")


def gen_ahn_bigtiff() -> None:
    """BigTIFF float32 DTM analog: plane z = 10 + 0.5*col + 0.25*row, 64x48 px."""
    width, height = 64, 48
    col = np.arange(width, dtype="float64")[None, :]
    row = np.arange(height, dtype="float64")[:, None]
    data = 10.0 + 0.5 * col + 0.25 * row
    # RD-New-like north-up transform, 0.5 m pixels (AHN 0.5 m raster).
    transform = Affine(0.5, 0.0, 100000.0, 0.0, -0.5, 463000.0)
    path = _write_cog(
        "ahn_bigtiff.tif", data, transform, 28992,
        bigtiff=True, predictor=1, nodata=None, blocksize=16,
    )
    cases = [
        {"name": "interior 4x4", "col_off": 8, "row_off": 12, "win_width": 4, "win_height": 4},
        # Bottom-right corner catches any row/col (x/y) transposition.
        {"name": "bottom-right 3x2", "col_off": 61, "row_off": 46, "win_width": 3, "win_height": 2},
        # Window spanning a tile boundary (tiles are 16 px).
        {"name": "cross-tile 6x6", "col_off": 13, "row_off": 13, "win_width": 6, "win_height": 6},
    ]
    _write_toml(
        "ahn_bigtiff.toml", fixture="ahn_bigtiff.tif", bigtiff=True, predictor=1,
        width=width, height=height, transform=transform, nodata=None,
        cases=_expected_cases(path, cases, None),
    )


def gen_glo30_predictor3() -> None:
    """Classic TIFF, DEFLATE + float predictor 3, non-square non-nominal pixels."""
    width, height = 96, 72
    col = np.arange(width, dtype="float64")[None, :]
    row = np.arange(height, dtype="float64")[:, None]
    # Fractional variation so predictor-3 byte reshuffle is genuinely exercised.
    data = 100.0 + 0.1 * col - 0.07 * row + 0.001 * col * row
    # GLO-30-like geographic transform: longitudinal spacing != latitudinal
    # (Pitfall 5 — the decoder must READ these, not assume 1/3600 square).
    transform = Affine(0.0002777778, 0.0, 4.0, 0.0, -0.0004000000, 52.0)
    path = _write_cog(
        "glo30_predictor3.tif", data, transform, 4326,
        bigtiff=False, predictor=3, nodata=None, blocksize=32,
    )
    cases = [
        {"name": "interior 5x5", "col_off": 10, "row_off": 9, "win_width": 5, "win_height": 5},
        # Spans four 32x32 tiles: exercises predictor-3 decode + tile stitch.
        {"name": "four-tile 8x8", "col_off": 28, "row_off": 28, "win_width": 8, "win_height": 8},
    ]
    _write_toml(
        "glo30_predictor3.toml", fixture="glo30_predictor3.tif", bigtiff=False, predictor=3,
        width=width, height=height, transform=transform, nodata=None,
        cases=_expected_cases(path, cases, None),
    )


def gen_nodata_edge() -> None:
    """Nodata sentinel + edge tiles padded beyond ImageWidth/Length (Pitfall 7)."""
    width, height = 40, 40  # blocksize 32 -> bottom/right tiles padded to 32
    col = np.arange(width, dtype="float64")[None, :]
    row = np.arange(height, dtype="float64")[:, None]
    data = 5.0 + 0.3 * col + 0.2 * row
    nodata = -9999.0
    # Punch a nodata rectangle (a "water body"/coastline hole) at cols 6..10, rows 4..8.
    data[4:9, 6:11] = nodata
    transform = Affine(0.5, 0.0, 100000.0, 0.0, -0.5, 463000.0)
    path = _write_cog(
        "nodata_edge.tif", data, transform, 28992,
        bigtiff=False, predictor=1, nodata=nodata, blocksize=32,
    )
    cases = [
        # Window straddling the nodata rectangle: sentinel cells must become holes.
        {"name": "over-nodata 6x6", "col_off": 5, "row_off": 3, "win_width": 6, "win_height": 6},
        # Bottom-right window landing inside the PADDED edge tiles (cols/rows 32..39):
        # the decoder must crop padding and return only real samples.
        {"name": "padded-edge 8x8", "col_off": 32, "row_off": 32, "win_width": 8, "win_height": 8},
    ]
    _write_toml(
        "nodata_edge.toml", fixture="nodata_edge.tif", bigtiff=False, predictor=1,
        width=width, height=height, transform=transform, nodata=nodata,
        cases=_expected_cases(path, cases, nodata),
    )


# The ESA WorldCover v200 legend: class code -> RGB (the product's own colour
# table). Only the CODE is data; the RGB is presentation. Reproducing it here is
# what makes the fixture a faithful palette TIFF.
WORLDCOVER_LEGEND: dict[int, tuple[int, int, int]] = {
    10: (0, 100, 0),  # tree cover
    20: (255, 187, 34),  # shrubland
    30: (255, 255, 76),  # grassland
    40: (240, 150, 255),  # cropland
    50: (250, 0, 0),  # built-up
    60: (180, 180, 180),  # bare / sparse vegetation
    70: (240, 240, 240),  # snow and ice
    80: (0, 100, 200),  # permanent water bodies
    90: (0, 150, 160),  # herbaceous wetland
    95: (0, 207, 117),  # mangroves
    100: (250, 230, 160),  # moss and lichen
}


def gen_worldcover_palette() -> None:
    """PALETTE (photometric 3) 8-bit class raster — the REAL WorldCover encoding.

    The committed e2e fixture was a plain grayscale (photometric 1) u8 TIFF, so
    every offline test passed while the live tile — ``PhotometricInterpretation =
    RGBPalette`` — was rejected by the ``tiff`` crate. This fixture reproduces the
    real product's IFD (photometric 3 + ColorMap + a ``0`` nodata sentinel) so the
    palette path can never regress.
    """
    width, height = 40, 30
    codes = sorted(WORLDCOVER_LEGEND)
    col = np.arange(width, dtype="int64")[None, :]
    row = np.arange(height, dtype="int64")[:, None]
    # A deterministic patchwork of every legend class (never one flat class, so a
    # decoder that returned the palette's RGB — or a constant — is caught).
    data = np.take(codes, (col // 4 + row // 3) % len(codes)).astype("uint8")
    # Punch a nodata (0) hole: WorldCover's own GDAL_NODATA is 0.
    data[6:10, 5:11] = 0
    # WorldCover is a 10 m geographic grid (1/12000 deg), north-up.
    step = 1.0 / 12000.0
    transform = Affine(step, 0.0, 3.0, 0.0, -step, 52.0)

    path = OUT / "worldcover_palette.tif"
    profile = {
        "driver": "GTiff",
        "dtype": "uint8",
        "count": 1,
        "width": width,
        "height": height,
        "crs": rasterio.crs.CRS.from_epsg(4326),
        "transform": transform,
        "tiled": True,
        "blockxsize": 16,
        "blockysize": 16,
        "compress": "deflate",
        "predictor": 1,
        "nodata": 0,
        "BIGTIFF": "NO",
        # REQUIRED: `write_colormap` alone leaves GDAL on photometric = BlackIsZero
        # (it emits the ColorMap tag but keeps interpretation 1) — which is exactly
        # the too-easy fixture that hid this bug. Only this creation option makes
        # GDAL declare PhotometricInterpretation = 3 (RGBPalette), as the real
        # WorldCover product does.
        "photometric": "palette",
    }
    with rasterio.open(path, "w", **profile) as dst:
        dst.write(data, 1)
        dst.write_colormap(1, {c: (*rgb, 255) for c, rgb in WORLDCOVER_LEGEND.items()})

    # Fail loudly here rather than committing a fixture that is not what it claims.
    with open(path, "rb") as fh:
        head = fh.read(4096)
    if not _photometric_is_palette(head):
        raise SystemExit("worldcover_palette.tif is NOT palette-encoded — GDAL wrote photometric != 3")

    cases = [
        {"name": "interior 6x5", "col_off": 9, "row_off": 4, "win_width": 6, "win_height": 5},
        # Straddles the nodata hole: the 0 sentinel must decode to a hole.
        {"name": "over-nodata 8x6", "col_off": 4, "row_off": 5, "win_width": 8, "win_height": 6},
        # Crosses the 16 px tile grid in both axes.
        {"name": "cross-tile 8x8", "col_off": 12, "row_off": 12, "win_width": 8, "win_height": 8},
    ]
    _write_toml(
        "worldcover_palette.toml", fixture="worldcover_palette.tif", bigtiff=False, predictor=1,
        width=width, height=height, transform=transform, nodata=0.0,
        cases=_expected_cases(path, cases, 0.0),
    )


def gen_range_tiled() -> None:
    """A REAL multi-tile COG for the windowed **range-read** planner.

    The other fixtures are a few KB, so their header dwarfs their pixel data and a
    "we only fetched the overlapping tiles" claim could not be measured on them —
    which is exactly how the whole-tile-GET defect shipped (a 330 MB AHN download
    for a 1 km² window). This one is deliberately PIXEL-DATA-DOMINATED: 256x256
    float32 in an 8x8 grid of 32 px tiles, filled with high-entropy noise so
    DEFLATE cannot collapse it. A small window therefore touches a handful of the
    64 tiles, and the planned byte fraction is a meaningful, assertable number.

    The TOML records each tile's declared byte range (from the IFD's TileOffsets /
    TileByteCounts) so the Rust test compares the planner's ranges against GDAL's
    own view of the file, not against itself.
    """
    width = height = 256
    block = 32
    rng = np.random.default_rng(20260713)
    # High-entropy but plausible terrain: a smooth ramp plus strong noise. The
    # noise is what keeps the tiles incompressible (and so the file honest).
    col = np.arange(width, dtype="float64")[None, :]
    row = np.arange(height, dtype="float64")[:, None]
    data = 12.0 + 0.02 * col - 0.01 * row + rng.normal(0.0, 3.0, size=(height, width))
    # RD-New-like north-up transform, 0.5 m pixels (AHN 0.5 m raster).
    transform = Affine(0.5, 0.0, 120000.0, 0.0, -0.5, 487000.0)
    path = _write_cog(
        "range_tiled.tif", data, transform, 28992,
        bigtiff=False, predictor=1, nodata=None, blocksize=block,
    )

    cases = [
        # Inside ONE tile: the planner must select exactly 1 of the 64 chunks.
        {"name": "single-tile 10x10", "col_off": 36, "row_off": 68, "win_width": 10, "win_height": 10},
        # Straddles a 2x2 block of tiles: exactly 4 chunks, never the file.
        {"name": "four-tile 20x20", "col_off": 54, "row_off": 118, "win_width": 20, "win_height": 20},
        # A wide-but-short window: one tile row (8 chunks) — proves the row is
        # coalesced into few requests without pulling the rows above/below.
        {"name": "tile-row 256x8", "col_off": 0, "row_off": 100, "win_width": 256, "win_height": 8},
    ]

    with rasterio.open(path) as src:
        # GDAL's own view of the chunk layout — the oracle for the planner.
        tiff_meta = src.tags(ns="IMAGE_STRUCTURE")
        assert src.block_shapes[0] == (block, block), src.block_shapes
    file_bytes = path.stat().st_size

    lines: list[str] = [
        "# generated by tools/gis_oracle/gen_cog_fixtures.py — DO NOT EDIT",
        "# Range-read planning oracle: a real multi-tile COG whose pixel data dominates",
        "# the file, so 'only the overlapping tiles were planned' is a measurable claim.",
        "",
        "[meta]",
        f'oracle = "GDAL {GDAL_VER} via rasterio {RIO_VER}"',
        f'provenance = "gen_cog_fixtures.py sha256:{GEN_SHA}"',
        'fixture = "range_tiled.tif"',
        "bigtiff = false",
        "predictor = 1",
        f"tol = {_fmt(TOL)}",
        f"width = {width}",
        f"height = {height}",
        f"block = {block}",
        f"file_bytes = {file_bytes}",
        f'compression = "{tiff_meta.get("COMPRESSION", "DEFLATE")}"',
        f"origin_x = {_fmt(transform.c)}",
        f"origin_y = {_fmt(transform.f)}",
        f"pixel_size_x = {_fmt(transform.a)}",
        f"pixel_size_y = {_fmt(transform.e)}",
        "nodata = nan",
        "has_nodata = false",
        "",
    ]
    for c in _expected_cases(path, cases, None):
        lines.append("[[case]]")
        lines.append(f'name = "{c["name"]}"')
        lines.append(f"col_off = {c['col_off']}")
        lines.append(f"row_off = {c['row_off']}")
        lines.append(f"win_width = {c['win_width']}")
        lines.append(f"win_height = {c['win_height']}")
        # The chunk indices GDAL's own block grid says this window overlaps.
        across = -(-width // block)
        tx0, tx1 = c["col_off"] // block, (c["col_off"] + c["win_width"] - 1) // block
        ty0, ty1 = c["row_off"] // block, (c["row_off"] + c["win_height"] - 1) // block
        chunks = [ty * across + tx for ty in range(ty0, ty1 + 1) for tx in range(tx0, tx1 + 1)]
        lines.append(f"chunks = {chunks}")
        lines.append(f"samples = {_samples_block(c['samples'])}")
        lines.append("")
    (OUT / "range_tiled.toml").write_text("\n".join(lines) + "\n", encoding="utf-8")


def _photometric_is_palette(head: bytes) -> bool:
    """True if the classic-TIFF IFD0 in ``head`` declares photometric = 3 (RGBPalette)."""
    import struct

    if head[:2] != b"II" or struct.unpack("<H", head[2:4])[0] != 42:
        return False
    off = struct.unpack("<I", head[4:8])[0]
    n = struct.unpack("<H", head[off : off + 2])[0]
    for i in range(n):
        e = off + 2 + i * 12
        tag, typ = struct.unpack("<HH", head[e : e + 4])
        if tag == 262 and typ == 3:
            return struct.unpack("<H", head[e + 8 : e + 10])[0] == 3
    return False


def gen_bomb() -> None:
    """Decompression bomb: 2048x2048 declared (4_194_304 px), constant data, KB
    on disk.

    Written block-by-block so peak memory stays ~1 block. The point is the ratio
    of declared IFD pixels to on-disk bytes: the Rust test passes a SMALL
    ``max_decoded_px`` budget and asserts ``decode_window`` rejects it from the
    IFD dimensions BEFORE any ``read_chunk`` — never allocating the full raster
    (threat T-08-02-01). Keeping the declared dims modest keeps the constant
    DEFLATE fixture tiny while the IFD still dwarfs the test budget. No
    expected-window TOML — it is a reject case.
    """
    width = height = 2048
    block = 512
    transform = Affine(30.0, 0.0, 0.0, 0.0, -30.0, 0.0)
    profile = {
        "driver": "GTiff", "dtype": "float32", "count": 1,
        "width": width, "height": height,
        "crs": rasterio.crs.CRS.from_epsg(4326), "transform": transform,
        "tiled": True, "blockxsize": block, "blockysize": block,
        "compress": "deflate", "predictor": 1, "BIGTIFF": "YES",
    }
    const_block = np.full((block, block), 42.0, dtype="float32")
    with rasterio.open(OUT / "bomb.tif", "w", **profile) as dst:
        for r in range(0, height, block):
            for c in range(0, width, block):
                bh = min(block, height - r)
                bw = min(block, width - c)
                dst.write(const_block[:bh, :bw], 1, window=Window(c, r, bw, bh))
    # Minimal meta so the test knows the declared dims without hardcoding.
    lines = [
        "# generated by tools/gis_oracle/gen_cog_fixtures.py — DO NOT EDIT",
        "# Decompression bomb: huge IFD dims, tiny file. Reject-before-decode case.",
        "",
        "[meta]",
        f'provenance = "gen_cog_fixtures.py sha256:{GEN_SHA}"',
        'fixture = "bomb.tif"',
        f"width = {width}",
        f"height = {height}",
        "",
    ]
    (OUT / "bomb.toml").write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    gen_ahn_bigtiff()
    gen_glo30_predictor3()
    gen_nodata_edge()
    gen_worldcover_palette()
    gen_range_tiled()
    gen_bomb()
    total = sum(p.stat().st_size for p in OUT.glob("*.tif"))
    print(f"wrote fixtures to {OUT} (sha256:{GEN_SHA}, GDAL {GDAL_VER})")
    print(f"total .tif bytes: {total} ({total / 1024:.1f} KiB)")


if __name__ == "__main__":
    main()
