"""Generate the committed offline E2E import fixtures for the web suite.

Writes three tiny, provenance-headed GIS fixtures plus a `viewport.json`
descriptor under ``web/tests/e2e/fixtures/``. The 08-08 Playwright import
journey serves these bytes from `page.route` mocks so a full viewport import
(terrain + land cover + buildings) runs FULLY OFFLINE against real decode paths:

* ``ahn_dtm_fixture.tif``    — an AHN-analog terrain COG: EPSG:28992 (RD New)
  float32, tiled DEFLATE predictor 1. Its geotransform sits inside the committed
  AHN kaartblad ``M_25GN1`` so ``plan_tiles`` selects that tile and
  ``window_for_bbox`` resolves a real pixel window for the reprojected viewport.
  Coarse 10 m pixels (NOT real AHN 0.5 m — this is a synthetic E2E crop) keep the
  decimated point count small.
* ``worldcover_fixture.tif`` — an ESA-WorldCover-analog class raster: EPSG:4326
  uint8 class codes (grassland 30, built-up 50, water 80), tiled DEFLATE. Covers
  the same WGS84 viewport so ``map_landcover`` vectorizes it into ``ground_zone``
  polygons carrying the reviewed Nord2000 impedance class letters.
* ``overpass_buildings.json`` — a minimal Overpass ``out geom`` response with two
  building ways near the viewport, parsed by ``parse_buildings`` into editable
  ``building`` features.

``viewport.json`` records the WGS84 import bbox (derived by reprojecting the RD
viewport box through pyproj) plus the resolved tile names, so the spec imports
one committed source of truth rather than hardcoding coordinates.

Regeneration is operator-driven (``python gen_e2e_fixtures.py``): rasterio /
GDAL / pyproj are a DEV-TIME install only, never a build or test dependency —
the Playwright specs read only the committed bytes + ``viewport.json``. Each
artifact records a sha256 of this generator so a fixture traces to its oracle.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

import numpy as np
import pyproj
import rasterio
from rasterio.crs import CRS
from rasterio.transform import Affine

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
OUT = ROOT / "web/tests/e2e/fixtures"

GEN_SHA = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()[:16]
GDAL_VER = rasterio.__gdal_version__
RIO_VER = rasterio.__version__

# --- geometry: an RD-New viewport box inside the committed kaartblad M_25GN1 ---
# M_25GN1 committed index bbox: RD x[115000, 120000], y[455000, 461250].
# The import viewport (RD) — a ~100 m box well inside that kaartblad.
RD_VIEW = (117050.0, 458050.0, 117150.0, 458150.0)  # min_x, min_y, max_x, max_y
KAARTBLAD = "M_25GN1"
WORLDCOVER_TILE = "N51E003"  # 3-degree SW corner (3E, 51N) covers the viewport

# The AHN terrain fixture geotransform (RD New), a 400 m box around the viewport
# with generous margin so the reprojected window always lands inside the tile.
AHN_ORIGIN_X = 117000.0
AHN_ORIGIN_Y = 458300.0
AHN_PIXEL = 10.0  # synthetic 10 m pixels (NOT real AHN 0.5 m) — keeps points small
AHN_W = 40
AHN_H = 40

_RD_TO_WGS84 = pyproj.Transformer.from_crs(28992, 4326, always_xy=True)


def _wgs84_viewport() -> dict[str, float]:
    """Reproject the RD viewport box corners to WGS84 and take the extent."""
    min_x, min_y, max_x, max_y = RD_VIEW
    lons: list[float] = []
    lats: list[float] = []
    for x, y in [(min_x, min_y), (min_x, max_y), (max_x, min_y), (max_x, max_y)]:
        lon, lat = _RD_TO_WGS84.transform(x, y)
        lons.append(lon)
        lats.append(lat)
    return {
        "min_lon": min(lons),
        "min_lat": min(lats),
        "max_lon": max(lons),
        "max_lat": max(lats),
    }


def _write_tif(name: str, data: np.ndarray, transform: Affine, epsg: int, dtype: str,
               *, predictor: int, blocksize: int) -> None:
    path = OUT / name
    height, width = data.shape
    profile = {
        "driver": "GTiff",
        "dtype": dtype,
        "count": 1,
        "width": width,
        "height": height,
        "crs": CRS.from_epsg(epsg),
        "transform": transform,
        "tiled": True,
        "blockxsize": blocksize,
        "blockysize": blocksize,
        "compress": "deflate",
        "predictor": predictor,
        "BIGTIFF": "NO",
    }
    with rasterio.open(path, "w", **profile) as dst:
        dst.write(data.astype(dtype), 1)


def gen_ahn_terrain() -> None:
    """AHN-analog DTM: a gentle non-collinear plane, RD New float32."""
    col = np.arange(AHN_W, dtype="float64")[None, :]
    row = np.arange(AHN_H, dtype="float64")[:, None]
    # A tilted plane + mild curvature so the sampled points are never collinear
    # (the DGM producer needs a genuine 2-D point cloud to triangulate).
    data = 5.0 + 0.03 * col + 0.02 * row + 0.0005 * col * row
    transform = Affine(AHN_PIXEL, 0.0, AHN_ORIGIN_X, 0.0, -AHN_PIXEL, AHN_ORIGIN_Y)
    _write_tif("ahn_dtm_fixture.tif", data, transform, 28992, "float32",
               predictor=1, blocksize=16)


def gen_worldcover(view: dict[str, float]) -> None:
    """WorldCover-analog class raster (EPSG:4326 uint8) covering the viewport."""
    margin = 0.0006
    pixel = 0.00008  # ~8 m — near WorldCover's 10 m, keeps the window small
    origin_lon = view["min_lon"] - margin
    origin_lat = view["max_lat"] + margin
    span_lon = (view["max_lon"] + margin) - origin_lon
    span_lat = origin_lat - (view["min_lat"] - margin)
    width = int(np.ceil(span_lon / pixel))
    height = int(np.ceil(span_lat / pixel))
    # Grassland (30 -> impedance D) everywhere, with a built-up block (50 -> G)
    # and a water block (80 -> H) so vectorization yields multiple ground_zones.
    data = np.full((height, width), 30, dtype="uint8")
    data[height // 4: height // 2, width // 4: width // 2] = 50
    data[height // 2: 3 * height // 4, width // 2: 3 * width // 4] = 80
    transform = Affine(pixel, 0.0, origin_lon, 0.0, -pixel, origin_lat)
    _write_tif("worldcover_fixture.tif", data, transform, 4326, "uint8",
               predictor=1, blocksize=32)


def gen_overpass(view: dict[str, float]) -> None:
    """Two building ways (Overpass `out geom`) near the viewport centre."""
    clon = (view["min_lon"] + view["max_lon"]) / 2.0
    clat = (view["min_lat"] + view["max_lat"]) / 2.0
    d = 0.00015

    def _way(wid: int, lon0: float, lat0: float, tags: dict[str, str]) -> dict:
        # A small closed quad footprint (Overpass repeats the first node last).
        geom = [
            {"lat": lat0, "lon": lon0},
            {"lat": lat0, "lon": lon0 + d},
            {"lat": lat0 + d, "lon": lon0 + d},
            {"lat": lat0 + d, "lon": lon0},
            {"lat": lat0, "lon": lon0},
        ]
        return {"type": "way", "id": wid, "tags": tags, "geometry": geom}

    doc = {
        "version": 0.6,
        "generator": "envi-e2e-fixture (synthetic)",
        "osm3s": {"copyright": "OpenStreetMap contributors, ODbL"},
        "elements": [
            _way(1001, clon - 0.0004, clat - 0.0002,
                 {"building": "yes", "height": "9"}),
            _way(1002, clon + 0.0002, clat + 0.0001,
                 {"building": "residential", "building:levels": "3"}),
        ],
    }
    (OUT / "overpass_buildings.json").write_text(
        json.dumps(doc, indent=2) + "\n", encoding="utf-8"
    )


def gen_viewport(view: dict[str, float]) -> None:
    doc = {
        "_comment": (
            "generated by tools/gis_oracle/gen_e2e_fixtures.py — DO NOT EDIT. "
            "WGS84 import viewport (reprojected from the RD box inside kaartblad "
            f"{KAARTBLAD}) + the resolved tile names the fixtures are served for."
        ),
        "provenance": f"gen_e2e_fixtures.py sha256:{GEN_SHA}",
        "oracle": f"GDAL {GDAL_VER} via rasterio {RIO_VER}, pyproj {pyproj.__version__}",
        "viewport": view,
        "kaartblad": KAARTBLAD,
        "worldcover_tile": WORLDCOVER_TILE,
    }
    (OUT / "viewport.json").write_text(
        json.dumps(doc, indent=2) + "\n", encoding="utf-8"
    )


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    view = _wgs84_viewport()
    gen_ahn_terrain()
    gen_worldcover(view)
    gen_overpass(view)
    gen_viewport(view)
    total = sum(p.stat().st_size for p in OUT.glob("*.tif"))
    print(f"wrote E2E fixtures to {OUT} (sha256:{GEN_SHA}, GDAL {GDAL_VER})")
    print(f"viewport: {json.dumps(view)}")
    print(f"total .tif bytes: {total} ({total / 1024:.1f} KiB)")


if __name__ == "__main__":
    main()
