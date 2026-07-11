# envi-gis COG decode fixtures

Committed, provenance-headed Cloud-Optimized-GeoTIFF fixtures that pin the
pure-Rust `tiff`-crate decode path in `envi-gis::cog` against **GDAL** ground
truth. This is the same committed-oracle pattern as `tools/crs_oracle/` and
`tools/nord2000_oracle/`: the fixtures are checked in, and **Python / rasterio /
GDAL are a dev-time install only — never a build or `cargo test` dependency**.
The Rust tests (`crates/envi-gis/tests/cog_window.rs`) read only the committed
bytes and the expected-window TOMLs.

## Regeneration (operator-driven)

```bash
python -m pip install rasterio        # dev-time only; wheel bundles GDAL
python tools/gis_oracle/gen_cog_fixtures.py
```

The generator is `tools/gis_oracle/gen_cog_fixtures.py`. Each `*.toml` header
records a sha256 of that generator (`provenance = "gen_cog_fixtures.py
sha256:..."`) so a fixture traces to the exact oracle that produced it. Editing
a `.tif`/`.toml` by hand is forbidden — regenerate instead.

## Method used in this environment

rasterio was **not** preinstalled here (nor was a standalone GDAL CLI); it was
installed at dev time with `pip install rasterio` (rasterio 1.5.0, bundling
**GDAL 3.12.1**) and the fixtures were generated with GDAL via rasterio — the
primary path from 08-RESEARCH, not the `tiff`-crate-encoder fallback. GDAL is
required to emit genuine floating-point **predictor 3** and **BigTIFF**
containers, which the `tiff` crate can decode but not encode.

## Fixtures (each spans a distinct format edge case)

| File | Container | Compression | Notable | Threat / pitfall |
|------|-----------|-------------|---------|------------------|
| `ahn_bigtiff.tif` | **BigTIFF** (`49 49 2B 00`) | DEFLATE, predictor 1 | 64x48 f32, 0.5 m RD-New-like grid, tiled 16 | BigTIFF support (Pitfall 1) — the suite cannot pass without it |
| `glo30_predictor3.tif` | classic TIFF (`49 49 2A 00`) | DEFLATE, **predictor 3** | 96x72 f32, non-square non-nominal pixels, tiled 32 | float predictor-3 decode (A3); geotransform-from-IFD (Pitfall 5) |
| `nodata_edge.tif` | classic TIFF | DEFLATE, predictor 1 | 40x40 f32, `GDAL_NODATA=-9999`, dims not a tile multiple | nodata sentinels dropped + edge-tile padding cropped (Pitfall 7 / T-08-02-03) |
| `bomb.tif` | BigTIFF | DEFLATE, predictor 1 | 2048x2048 declared, constant data, ~17 KB on disk | decompression-bomb reject via pre-decode `max_decoded_px` (T-08-02-01) |

Each data fixture except `bomb.tif` has a sibling `*.toml` with a `[meta]`
block (provenance sha256, tolerance `tol`, dimensions, the expected
geotransform derived from `ModelPixelScale`/`ModelTiepoint`, and the nodata
sentinel) plus `[[case]]` rows giving pixel windows (`col_off`, `row_off`,
`win_width`, `win_height`) and the expected row-major `samples`. A `nan` in
`samples` marks a cell the decoder must return as a hole (nodata), never a
silent `0.0`. `bomb.toml` carries only the declared dimensions.

Total on-disk size of the `.tif` fixtures is ~26 KiB.
