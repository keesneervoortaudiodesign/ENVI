---
phase: 08-gis-ingestion-dgm
plan: 02
subsystem: gis
tags: [envi-gis, cog, bigtiff, geotiff, tiff, gdal, wasm, sans-io, deflate, predictor3, dos-budget]

# Dependency graph
requires:
  - phase: 07-*
    provides: envi-dgm guard-first typed-error boundary pattern (tin.rs) reused for the COG decode core
  - phase: 08-01
    provides: envi-geo RD New EPSG:28992 seam (the single reprojection boundary envi-gis will call, no second proj4rs edge)
provides:
  - New pure-Rust, sans-I/O, #![deny(unsafe_code)], WASM-safe envi-gis crate
  - GisError typed error enum (PartialEq, tiff::TiffError wrapped as { message })
  - cog::header (TIFF+BigTIFF IFD parse, chunk grid, IFD-chain cap)
  - cog::geo_tags (ModelPixelScale/Tiepoint -> north-up GeoTransform, GDAL_NODATA)
  - cog::window::decode_window(&[u8], PixelWindow, max_decoded_px) -> Raster<f32> with pre-decode DoS budget, nodata/edge safety
  - cog::MAX_DECODED_PX DoS budget; bbox_to_pixel_window bridge
  - tools/gis_oracle/gen_cog_fixtures.py + committed COG fixtures (BigTIFF, predictor-3, nodata-edge, bomb) + expected-window TOMLs
affects: [08-04, 08-07, 08-landcover, 08-buildings, envi-gis-wasm, terrain-import]

# Tech tracking
tech-stack:
  added: [tiff 0.11.3, geo 0.30, geojson 1, rasterio (dev-time GDAL 3.12.1 fixture generator)]
  patterns:
    - "Sans-I/O core: synchronous decode over &[u8]; TS owns fetch+OPFS (no network/OPFS/browser deps in the crate graph — cargo tree gate)"
    - "Guard-first no-panic decode mirroring tin::build_tin: DoS budget FIRST (from IFD dims, before read_chunk), bounds SECOND, work THIRD"
    - "Committed GDAL-generated COG fixtures + expected-window TOMLs (sha256 provenance); Python NOT a test dependency"
    - "Geotransform derived from IFD ModelPixelScale/Tiepoint, never nominal pixel counts (non-square GLO-30 fixture proves it)"

key-files:
  created:
    - crates/envi-gis/Cargo.toml
    - crates/envi-gis/src/lib.rs
    - crates/envi-gis/src/cog/mod.rs
    - crates/envi-gis/src/cog/header.rs
    - crates/envi-gis/src/cog/geo_tags.rs
    - crates/envi-gis/src/cog/window.rs
    - crates/envi-gis/tests/cog_window.rs
    - tools/gis_oracle/gen_cog_fixtures.py
    - crates/envi-gis/tests/fixtures/cog/README.md
    - crates/envi-gis/tests/fixtures/cog/{ahn_bigtiff,glo30_predictor3,nodata_edge,bomb}.{tif,toml}
  modified:
    - crates/README.md
    - Cargo.lock

key-decisions:
  - "envi-gis is sans-I/O and WASM-safe: no network, no OPFS, no browser DOM bindings; synchronous over &[u8]; enforced by a cargo tree -p envi-gis boundary check (mirrors the engine 3-dep gate). Depends on envi-geo + envi-engine (path deps, legal — mirrors envi-store; engine gains nothing)."
  - "decode_window takes an explicit pixel-space PixelWindow (deterministic, matches the fixture TOMLs) rather than a source-CRS bbox; a separately-tested pure bbox_to_pixel_window(geo, header, ...) bridge provides the geo-aware entry so fixture assertions are not coupled to bbox rounding. (Window shape is Claude's discretion per 08-RESEARCH.)"
  - "Raster<f32> stores Vec<Option<f32>>: nodata sentinels AND any non-finite value become None holes, never a silent 0.0 (T-08-02-03); callers/TIN interpolate across holes."
  - "COG fixtures generated with real GDAL 3.12.1 (via a dev-time pip install rasterio), not the tiff-crate-encoder fallback — GDAL is required to emit genuine float predictor-3 and BigTIFF, which the tiff crate decodes but cannot encode."
  - "Decompression-bomb fixture kept tiny (2048x2048 constant, ~17 KB) with the test passing a small max_decoded_px, instead of a >64 Mpx monster fixture — a 64 Mpx constant-deflate tile is inherently hundreds of KB; the pre-decode reject is proven the same way."

patterns-established:
  - "Module I/O header on every cog module (Inputs/Output/Invariant naming threat IDs T-08-02-01..04)"
  - "read_chunk returns cropped (unpadded) tile data with stride = chunk_data_dimensions — the correct edge-tile crop; a wrong nominal-width stride panics on the padded-edge fixture"

requirements-completed: [DATA-01]

# Metrics
duration: ~55 min
completed: 2026-07-11
status: complete
---

# Phase 8 Plan 02: envi-gis sans-I/O COG/BigTIFF decode core Summary

**A new pure-Rust, sans-I/O, WASM-safe `envi-gis` crate whose `cog` core windows cached BigTIFF/predictor-3 COG tiles into `f32` rasters — geotransform read from the IFD, a pre-decode `max_decoded_px` DoS budget, and nodata/edge-padding safety — all green against committed GDAL-generated fixtures with no I/O, no unsafe, no panic.**

## Performance

- **Duration:** ~55 min
- **Completed:** 2026-07-11
- **Tasks:** 3
- **Files created/modified:** ~22 (crate scaffold + 3 cog modules + test + generator + 9 fixture files + README/Cargo.lock)

## Accomplishments
- Scaffolded the sans-I/O `envi-gis` crate with a typed `GisError` and the `#![deny(unsafe_code)]` boundary; `cargo tree -p envi-gis` proves no network/async/browser crate in the graph.
- Built the COG decode core (`header`/`geo_tags`/`window`) over the `tiff` crate: one path decodes both classic-TIFF and BigTIFF; geotransform is read from `ModelPixelScale`/`ModelTiepoint`; `decode_window` enforces the `max_decoded_px` budget from IFD dims before any `read_chunk`, crops edge-tile padding, and drops nodata to holes.
- Generated + committed a GDAL oracle fixture set (BigTIFF, DEFLATE+predictor-3, nodata+edge, decompression-bomb) with expected-window TOMLs; 8 fixture tests pass with Python never a test dependency.
- envi-engine stayed byte-identical (`cargo tree -p envi-engine` still ndarray + num-complex + thiserror only).

## Task Commits

1. **Task 1: Scaffold envi-gis crate + GisError + sans-I/O boundary doc** — `0cc2446` (feat)
2. **Task 2: Generate + commit COG fixtures (BigTIFF, predictor-3, nodata-edge)** — `d451ce1` (test)
3. **Task 3: COG header/geo_tags/window decode with guard-first DoS caps + fixture tests** — `15f9f69` (feat)

**Plan metadata:** (this SUMMARY + STATE/ROADMAP/REQUIREMENTS commit)

## Files Created/Modified
- `crates/envi-gis/Cargo.toml` — crate manifest: tiff/geo/geojson/serde/thiserror + envi-geo/envi-engine path deps; boundary-rule comment; dev-deps approx/toml.
- `crates/envi-gis/src/lib.rs` — crate doc (sans-I/O + security posture), `#![deny(unsafe_code)]`, `GisError` enum + `From<tiff::TiffError>`.
- `crates/envi-gis/src/cog/mod.rs` — module doc + `MAX_DECODED_PX` DoS budget const + submodule wiring/re-exports.
- `crates/envi-gis/src/cog/header.rs` — IFD dims + chunk grid; `guard_image_count` caps the overview/IFD chain (T-08-02-02).
- `crates/envi-gis/src/cog/geo_tags.rs` — `read_geotransform` (north-up from tags, T-08-02-04), `read_nodata` (GDAL_NODATA).
- `crates/envi-gis/src/cog/window.rs` — `decode_window` (guard-first), `Raster<f32>`, `PixelWindow`, `bbox_to_pixel_window`.
- `crates/envi-gis/tests/cog_window.rs` — 8 tests: oracle compare + per-edge-case + BigTIFF coverage.
- `tools/gis_oracle/gen_cog_fixtures.py` — operator-driven rasterio/GDAL fixture generator (sha256 provenance).
- `crates/envi-gis/tests/fixtures/cog/*` — 4 committed COGs (~26 KiB) + expected-window TOMLs + README.
- `crates/README.md` — added the `envi-gis` crate row (doc contract).

## Decisions Made
See `key-decisions` in the frontmatter. Headline: sans-I/O pixel-window decode with a separately-tested geo bbox bridge; nodata/non-finite → holes; real GDAL fixtures via a dev-time rasterio install.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Created a `cog` module stub in Task 1 so the crate compiles**
- **Found during:** Task 1 (scaffold)
- **Issue:** Task 1 declares `pub mod cog;` but the `cog` submodules are Task 3 files; without a module the crate would not build (Task 1's verify is `cargo build -p envi-gis`).
- **Fix:** Created `crates/envi-gis/src/cog/mod.rs` as a documented stub carrying the module doc + `MAX_DECODED_PX`; Task 3 filled in the submodules.
- **Verification:** `cargo build -p envi-gis` + `cargo clippy` clean at Task 1.
- **Committed in:** `0cc2446` (Task 1 commit)

**2. [Rule 3 - Blocking] Reworded the Cargo.toml boundary comment to satisfy the literal acceptance grep**
- **Found during:** Task 1 (acceptance check)
- **Issue:** The acceptance criterion `grep -rn "web-sys|reqwest|tokio" crates/envi-gis/Cargo.toml returns zero matches` was tripped by my own prose ("no web-sys") in the boundary comment/description — a false positive, no such dependency existed.
- **Fix:** Reworded to "no browser DOM bindings / no HTTP client / no async runtime"; the boundary is proven structurally by `cargo tree -p envi-gis` instead.
- **Verification:** grep now returns zero matches; tree scan clean.
- **Committed in:** `0cc2446` (Task 1 commit)

**3. [Rule 2 - Missing Critical] Indexed the new envi-gis crate in `crates/README.md`**
- **Found during:** Task 3
- **Issue:** Adding a new crate left `crates/README.md`'s crate table stale; CLAUDE.md's documentation contract (gate 5) requires the README reflect new crates.
- **Fix:** Added an `envi-gis` row (role, boundary rule, entry points).
- **Verification:** Table row present and accurate; consistent with the crate's actual public surface.
- **Committed in:** `15f9f69` (Task 3 commit)

**4. [Rule 3 - Blocking] Installed rasterio at dev time to generate the fixtures**
- **Found during:** Task 2
- **Issue:** rasterio/GDAL were absent (research flagged this). The plan's fallback (tiff-crate encoder) cannot emit float predictor-3 or BigTIFF, which the fixtures require.
- **Fix:** `pip install rasterio` (rasterio 1.5.0 bundling GDAL 3.12.1) — the plan/research primary path, explicitly named; a dev-time tool only, never a build/test dependency. Recorded in the fixtures README.
- **Verification:** Fixtures generated with correct BigTIFF magic, predictor=3, nodata tags (verified via rasterio inspection); `cargo test` reads only committed bytes.
- **Committed in:** `d451ce1` (Task 2 commit)

---

**Total deviations:** 4 auto-fixed (1 missing-critical doc, 3 blocking). **Impact:** All necessary for the crate to build, pass its acceptance greps, keep the docs consistent, and produce faithful fixtures. No scope creep — the crate remains exactly the sans-I/O COG decode core the plan specified.

## Issues Encountered
- The decompression-bomb fixture was initially ~600 KB (a >64 Mpx constant tile compresses to hundreds of KB regardless of tiling). Resolved by shrinking the fixture to 2048×2048 and having the test pass a small `max_decoded_px` budget — the pre-decode reject is proven identically while total fixture size dropped to ~26 KiB.
- A running `envi-service.exe` may hold a Windows link lock; all build/test was correctly scoped to `-p envi-gis` (a library), so no workspace-binary relink was needed and nothing was killed.

## User Setup Required
None - no external service configuration required. (rasterio is a dev-time-only fixture generator; regeneration is operator-driven and not needed to build or test.)

## Next Phase Readiness
- `envi-gis::cog::decode_window` is the ready seam for whole-tile ingest (08-07) and windowed feature construction (08-04); `Raster<f32>` holes flow into the TIN as gaps, never false 0.0.
- The `envi-gis-wasm` cdylib bindings crate and the TS orchestrator (fetch/OPFS) are the next slices; they wrap this core, which stays synchronous and I/O-free.
- No blockers. Directional-phase seam and other milestone TODOs are untouched by this plan.

## Self-Check: PASSED

- All created key files verified present on disk (crate, 3 cog modules, test, generator, fixtures).
- All three task commits verified in git log (`0cc2446`, `d451ce1`, `15f9f69`).
- Plan verification: `cargo test -p envi-gis` (8 passed), `cargo clippy -p envi-gis --all-targets -- -D warnings` clean, `cargo fmt --check` clean, `cargo tree -p envi-gis` no network/async/browser crate, zero `unwrap/expect/panic` on `src/cog/` data paths, `cargo tree -p envi-engine` unchanged (ndarray + num-complex + thiserror).

---
*Phase: 08-gis-ingestion-dgm*
*Completed: 2026-07-11*
