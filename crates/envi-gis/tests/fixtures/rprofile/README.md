# envi-gis r.profile cut-profile oracle fixture

Committed, provenance-headed fixtures that pin the pure-Rust GEOX-01 cut-profile
extractor (`envi_gis::profile::cut_profile`) against a GRASS `r.profile`-faithful
oracle. Same committed-oracle pattern as `../cog/` and `tools/nord2000_oracle/`:
the fixtures are checked in, and **Python / GDAL / GRASS are a dev-time install
only — never a build or `cargo test` dependency**. The Rust test
(`crates/envi-gis/tests/profile_oracle.rs`) reads only the committed bytes + CSV.

## Files

| File | Role |
|------|------|
| `rprofile_dem.tif` | The real-DEM extract: a 64×48 float32 GeoTIFF, RD-New-like 0.5 m north-up grid (EPSG:28992), DEFLATE. A smooth surface with a bilinear cross-term (`z = 30 + 0.08·col − 0.05·row + 0.0015·col·row`) — the same shape as the committed `glo30_predictor3` COG — so the TIN's linear-per-triangle interpolation genuinely deviates from the raster bilinear. |
| `rprofile.csv` | The reference `(x, z)` cut-profile from an **independent raster-bilinear** walk of the source→receiver line at cell resolution. A `# key = value` comment header carries the DEM dimensions, the S→R endpoints, the sampling step, the tolerance, and the generator provenance sha256. |

## Method (why this is a faithful, non-self-referential oracle)

`r.profile` walks a line at the raster resolution and reads the (interpolated)
cell value. The generator reproduces that with a **bilinear** sampler written from
scratch — it never calls the ENVI extractor, so the fixture is not
self-referential (09-01 prohibition). The ENVI extractor instead samples the DGM
**TIN** (barycentric, piecewise-linear). On a curved DEM the two kernels differ by
a small, bounded delta, so the test pins a **documented tolerance** (`tol` in the
CSV header, meters), not bit-equality (09-RESEARCH assumption A3). The test also
asserts the delta is non-zero, proving the DEM is genuinely non-planar.

The generator samples the float32 values read back from the written `.tif`, so the
oracle and the Rust decoder see identical grid values — the only intended
difference is the interpolation kernel.

## Regeneration (operator-driven)

```bash
python -m pip install rasterio        # dev-time only; wheel bundles GDAL
python tools/rprofile_oracle/gen_rprofile_fixture.py
```

Editing a `.tif`/`.csv` by hand is forbidden — regenerate instead. The CSV header's
`provenance = "gen_rprofile_fixture.py sha256:..."` traces each fixture to the exact
generator revision that produced it.
