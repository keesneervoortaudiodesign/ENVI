"""Generate the committed r.profile cut-profile oracle fixture for ``envi-gis``.

Writes a small synthetic-but-non-planar DEM extract (a float32 GeoTIFF) plus a
reference CSV of ``(x, z)`` cut-profile samples under
``crates/envi-gis/tests/fixtures/rprofile/``. Together they pin the pure-Rust
``envi_gis::profile::cut_profile`` (GEOX-01) against a **GRASS ``r.profile``-
faithful** oracle: an INDEPENDENT raster-bilinear walk of a source→receiver line
at cell resolution. This is the same committed-oracle pattern as
``tools/gis_oracle/gen_cog_fixtures.py`` and ``tools/nord2000_oracle`` — the Rust
test reads only the committed bytes + CSV; **Python / GDAL / GRASS are NOT a
build or test dependency**.

Why this is a faithful, non-self-referential oracle
---------------------------------------------------
* ``r.profile`` walks a line at the raster resolution and reads the cell value
  (interpolated). This generator reproduces that with a **bilinear** sampler over
  the raster grid — the sampler is written here, from scratch, and never calls the
  ENVI extractor (09-01 prohibition: no self-referential fixture).
* The ENVI extractor samples the DGM **TIN** (piecewise-linear over triangles),
  not the raster. On a DEM with genuine curvature the two kernels differ by a
  small, bounded delta, so the test pins a **documented tolerance**
  (``tol``), not bit-equality (09-RESEARCH assumption A3).
* The DEM carries a bilinear cross-term (``d*col*row``) — exactly the shape of the
  committed ``glo30_predictor3`` COG fixture — so the TIN's linear-per-triangle
  approximation genuinely deviates from the raster bilinear, making the tolerance
  meaningful rather than a rubber stamp.

Regeneration is operator-driven::

    python -m pip install rasterio        # dev-time only; wheel bundles GDAL
    python tools/rprofile_oracle/gen_rprofile_fixture.py

The CSV header records a sha256 of this generator so a fixture traces to the exact
oracle that produced it.
"""

from __future__ import annotations

import hashlib
import math
from pathlib import Path

import numpy as np
import rasterio
from rasterio.transform import Affine

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
OUT = ROOT / "crates/envi-gis/tests/fixtures/rprofile"

GEN_SHA = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()[:16]
GDAL_VER = rasterio.__gdal_version__
RIO_VER = rasterio.__version__

# --- DEM geometry: RD-New-like, 0.5 m north-up pixels (an AHN analog). ---
WIDTH, HEIGHT = 64, 48
ORIGIN_X, ORIGIN_Y = 121_000.0, 487_000.0
PIXEL = 0.5
TRANSFORM = Affine(PIXEL, 0.0, ORIGIN_X, 0.0, -PIXEL, ORIGIN_Y)
EPSG = 28992
DEM_NAME = "rprofile_dem.tif"
CSV_NAME = "rprofile.csv"

# --- Source/receiver as pixel-CENTER map coordinates (well inside the hull so
#     every sample stays within the TIN's convex hull). ---
S_COL, S_ROW = 6, 6
R_COL, R_ROW = 57, 41
STEP_M = PIXEL  # cell resolution — the r.profile walk step.

# Documented agreement tolerance (meters): the TIN-linear vs raster-bilinear
# kernel delta on the curved DEM. Comfortably above the observed max delta; the
# generator asserts the reference is self-consistent, the Rust test asserts ENVI
# ⊂ tol of this reference.
TOL = 0.05


def dem_surface() -> np.ndarray:
    """A float32 DEM with a bilinear cross-term so TIN ≠ raster-bilinear.

    ``z = 30 + 0.08*col - 0.05*row + 0.0015*col*row`` — the same shape as the
    committed ``glo30_predictor3`` COG fixture, guaranteeing a genuine (small)
    interpolation-kernel delta the tolerance must absorb.
    """
    col = np.arange(WIDTH, dtype="float64")[None, :]
    row = np.arange(HEIGHT, dtype="float64")[:, None]
    z = 30.0 + 0.08 * col - 0.05 * row + 0.0015 * col * row
    return z.astype("float32")


def write_dem(data: np.ndarray) -> Path:
    path = OUT / DEM_NAME
    profile = {
        "driver": "GTiff",
        "dtype": "float32",
        "count": 1,
        "width": WIDTH,
        "height": HEIGHT,
        "crs": rasterio.crs.CRS.from_epsg(EPSG),
        "transform": TRANSFORM,
        "tiled": True,
        "blockxsize": 16,
        "blockysize": 16,
        "compress": "deflate",
        "predictor": 1,
        "BIGTIFF": "NO",
    }
    with rasterio.open(path, "w", **profile) as dst:
        dst.write(data, 1)
    return path


def pixel_center_to_map(col: float, row: float) -> tuple[float, float]:
    """Map coordinates of a pixel CENTER (matches Rust ``pixel_to_map(col+0.5,
    row+0.5)`` and ``rasterio`` ``transform * (col+0.5, row+0.5)``)."""
    x, y = TRANSFORM * (col + 0.5, row + 0.5)
    return x, y


def map_to_pixel_center(x: float, y: float) -> tuple[float, float]:
    """Inverse: map coordinate → fractional pixel-center coordinates."""
    fcol = (x - ORIGIN_X) / PIXEL - 0.5
    frow = (ORIGIN_Y - y) / PIXEL - 0.5
    return fcol, frow


def bilinear(arr: np.ndarray, fcol: float, frow: float) -> float:
    """r.profile-faithful bilinear read of ``arr`` at fractional pixel-center
    coords, clamped to the grid. Independent of the ENVI TIN extractor."""
    h, w = arr.shape
    c0 = int(math.floor(fcol))
    r0 = int(math.floor(frow))
    c1, r1 = c0 + 1, r0 + 1
    tc = fcol - c0
    tr = frow - r0
    cc0 = min(max(c0, 0), w - 1)
    cc1 = min(max(c1, 0), w - 1)
    rr0 = min(max(r0, 0), h - 1)
    rr1 = min(max(r1, 0), h - 1)
    v00 = float(arr[rr0, cc0])
    v10 = float(arr[rr0, cc1])
    v01 = float(arr[rr1, cc0])
    v11 = float(arr[rr1, cc1])
    top = v00 * (1 - tc) + v10 * tc
    bot = v01 * (1 - tc) + v11 * tc
    return top * (1 - tr) + bot * tr


def walk(arr: np.ndarray) -> tuple[list[tuple[float, float]], dict]:
    """Walk S→R at cell resolution, bilinear-sampling ground z at each point.

    Mirrors ``cut_profile``'s sampling geometry exactly (``n = ceil(d/step)``,
    ``t = i/n``) so the x positions align 1:1 with the ENVI extractor — the only
    intended difference is the interpolation KERNEL (bilinear vs TIN-linear)."""
    sx, sy = pixel_center_to_map(S_COL, S_ROW)
    rx, ry = pixel_center_to_map(R_COL, R_ROW)
    dx, dy = rx - sx, ry - sy
    d = math.hypot(dx, dy)
    n = max(1, math.ceil(d / STEP_M))
    rows: list[tuple[float, float]] = []
    last_x = -math.inf
    for i in range(n + 1):
        t = i / n
        x = t * d
        px, py = sx + t * dx, sy + t * dy
        fcol, frow = map_to_pixel_center(px, py)
        z = bilinear(arr, fcol, frow)
        if x > last_x + 1e-6:
            rows.append((x, z))
            last_x = x
    meta = {"s_x": sx, "s_y": sy, "r_x": rx, "r_y": ry}
    return rows, meta


def write_csv(rows: list[tuple[float, float]], meta: dict) -> None:
    lines: list[str] = []
    lines.append("# generated by tools/rprofile_oracle/gen_rprofile_fixture.py — DO NOT EDIT")
    lines.append("# r.profile-faithful cut-profile oracle: an INDEPENDENT raster-bilinear")
    lines.append("# walk of the S->R line at cell resolution, vs the ENVI TIN extractor.")
    lines.append("# Python / GDAL / GRASS are NOT a test dependency — the CSV is committed data.")
    lines.append(f'# oracle = "raster bilinear (GDAL {GDAL_VER} via rasterio {RIO_VER})"')
    lines.append(f'# provenance = "gen_rprofile_fixture.py sha256:{GEN_SHA}"')
    lines.append(f"# dem = {DEM_NAME}")
    lines.append(f"# width = {WIDTH}")
    lines.append(f"# height = {HEIGHT}")
    lines.append(f"# s_x = {meta['s_x']!r}")
    lines.append(f"# s_y = {meta['s_y']!r}")
    lines.append(f"# r_x = {meta['r_x']!r}")
    lines.append(f"# r_y = {meta['r_y']!r}")
    lines.append(f"# step_m = {STEP_M!r}")
    lines.append(f"# tol = {TOL!r}")
    lines.append("x,z")
    for x, z in rows:
        lines.append(f"{x!r},{z!r}")
    (OUT / CSV_NAME).write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    data = dem_surface()
    path = write_dem(data)
    # Read the stored float32 back so the oracle samples the EXACT values the Rust
    # decoder will see (no float64-vs-float32 skew between oracle and extractor).
    with rasterio.open(path) as src:
        arr = src.read(1).astype("float32")
    rows, meta = walk(arr)
    write_csv(rows, meta)
    print(f"wrote {DEM_NAME} ({path.stat().st_size} B) + {CSV_NAME} "
          f"({len(rows)} samples) to {OUT}")
    print(f"provenance sha256:{GEN_SHA}, GDAL {GDAL_VER}, tol {TOL} m")


if __name__ == "__main__":
    main()
