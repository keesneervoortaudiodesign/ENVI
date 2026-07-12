---
phase: 11-results-fast-recalc
plan: 04
subsystem: export-encoders
tags: [wasm, export, geotiff, geojson, csv, grid-05, d-20, d-21, d-22, attribution]
status: complete

# Dependency graph
requires:
  - phase: 11-results-fast-recalc
    provides: "11-01 ReceiverReadout + FreqAxis-driven readout; 11-02 LevelGrid (grid.rs) + trace_isobands (isoband.rs)"
  - phase: 10-calculation-service
    provides: "envi-compute pure core, envi-compute-wasm boundary discipline, ts-rs wire.ts no-drift mechanism"
  - phase: 06-service-crs
    provides: "envi-geo — the ONE reprojection boundary (ProjectCrs::to_wgs84, GEOX-04)"
provides:
  - "envi-compute::export::{ExportMeta, geotiff::encode_geotiff, geojson::encode_isophone_geojson, csv::encode_spectra_csv} — the three self-describing byte encoders (D-21/D-22)"
  - "Hand-rolled minimal single-strip Float32 GeoTIFF (GeoKeyDirectory + ModelPixelScale + ModelTiepoint + EPSG + GDAL_NODATA) — ZERO new dependency (D-20/D-21)"
  - "IsoBand::fill_polygons — the tracer's containment classification exposed for correct GeoJSON MultiPolygon/hole nesting"
  - "envi-compute-wasm::export — #[wasm_bindgen] export(req) dispatching ExportFormat {GeoTiff,GeoJson,Csv} → Vec<u8> browser-download bytes (D-20); reprojects SceneXY→LonLat via envi-geo (the one CRS seam)"
  - "export_filename / sanitize_export_filename — program-derived, path-traversal-safe download name (V12, T-11-04-02)"
  - "ExportReq/ExportFormat/ExportCrsDto/ExportGridDto ts-rs wire types (no-drift green)"
affects: [11-09]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Hand-rolled minimal GeoTIFF: a little-endian TIFF header + one IFD + the three GeoTIFF tags + one uncompressed Float32 strip, ~200 LOC owned outright — the tiff/geotiff-writer crates deliberately NOT added (RESEARCH Package Legitimacy), matching the iso-band-vs-contour hand-roll call"
    - "Export encoders are coordinate-agnostic: GeoTIFF stays in projected UTM meters (EPSG identifies the CRS), GeoJSON takes already-reprojected WGS84 lon/lat — SceneXY→LonLat happens ONCE in the WASM boundary through envi-geo (GEOX-04)"
    - "Every export embeds the ExportMeta footer (CRS + dB weighting label + engine version + tensor_hash + OSM/Overture/ESA WorldCover/Copernicus attribution): GeoTIFF ImageDescription, GeoJSON FeatureCollection foreign members, CSV # header comments (D-22)"
    - "CSV identity is the BAND INDEX + exact Hz from FreqAxis::centres; the nominal 1/3-oct labels (25, 31.5, …) are never written as the identity (RESEARCH Pitfall 3)"
    - "envi-geo (in-tree pure-Rust proj4rs) added to the wasm graph via envi-compute-wasm — builds for wasm32-unknown-unknown; engine 3-dep quarantine untouched"

key-files:
  created:
    - crates/envi-compute/src/export/mod.rs
    - crates/envi-compute/src/export/geotiff.rs
    - crates/envi-compute/src/export/geojson.rs
    - crates/envi-compute/src/export/csv.rs
    - crates/envi-compute-wasm/src/export.rs
  modified:
    - crates/envi-compute/src/lib.rs
    - crates/envi-compute/src/isoband.rs
    - crates/envi-compute-wasm/Cargo.toml
    - crates/envi-compute-wasm/src/lib.rs
    - crates/envi-compute-wasm/src/dto.rs
    - crates/envi-service/tests/wire_no_drift.rs
    - web/src/generated/wire.ts

key-decisions:
  - "GeoTIFF hand-rolled (zero-dep), NOT the tiff crate: RESEARCH offered (a) hand-roll or (b) add tiff behind a wasm32-build verify checkpoint. Chose (a) — a minimal single-strip Float32 GeoTIFF is ~200 LOC we own, keeps the export path zero new-crate, and avoids a package-legitimacy gate. The tiff/geotiff-writer/tiff-writer crates are absent from `cargo tree -p envi-compute`."
  - "The export boundary carries the payload in the request DTO (grid + breaks for GeoTIFF/GeoJSON, receivers for CSV) rather than reading the OPFS tensor itself — the worker already has the cached grid/readouts from the 11-01/11-02 paths, so export is a pure byte-generation step. The UI/worker wiring is 11-09."
  - "encode_geotiff takes the EPSG via ExportMeta (single source), not a separate epsg param as the plan's illustrative signature showed — the plan's must_haves only require `pub fn encode_geotiff`; folding EPSG into the meta keeps one identity object."
  - "encode_spectra_csv takes an extra `labels: &[String]` (receiver column names) beyond the plan's `&[ReceiverReadout]` — ReceiverReadout carries no id, so the TS-minted UUIDs are passed alongside; unlabelled columns fall back to receiver_<i>."
  - "GeoTIFF is north-up: grid rows (row 0 = min_y, south) are written flipped so raster row 0 is the northernmost row; ModelTiepoint maps raster (0,0,0) → NW node, GTRasterTypeGeoKey = RasterPixelIsPoint (grid values are node point-samples). NaN no-data holes ride through as Float32 NaN + a GDAL_NODATA=\"nan\" tag."

patterns-established:
  - "ExportMeta.one_line() (GeoTIFF ImageDescription) + ExportMeta.csv_comment_lines() (CSV # header) + meta_members() (GeoJSON foreign members) — one footer, three renderings"
  - "sanitize_export_filename: replace non-[A-Za-z0-9-_.] with _, collapse .. and __ runs, trim, fall back to `export`, append the format extension — the program-derived, Blob-download filename (V12)"

metrics:
  duration: ~40min
  tasks: 2
  files_created: 5
  files_modified: 7
  completed: 2026-07-12
---

# Phase 11 Plan 04: WASM Export Encoders (GeoTIFF / GeoJSON / CSV) Summary

Three self-describing WASM export encoders (GRID-05, D-20/21/22): a hand-rolled zero-dependency single-strip Float32 GeoTIFF of the continuous level grid, RFC-7946 isophone fill polygons via the in-tree `geojson` crate, and a band-index + exact-Hz spectra CSV — all generated in WASM as browser-download bytes (nothing leaves the device), every export carrying the full CRS + dB-weighting + engine/scene-identity + open-data attribution footer.

## What was built

**Task 1 — GeoTIFF + CSV encoders (commit 8fd9582):**
- `envi-compute::export::ExportMeta` — the shared CRS/weighting/engine/tensor/attribution footer, with `one_line()` (GeoTIFF) and `csv_comment_lines()` (CSV) renderings.
- `export/geotiff.rs::encode_geotiff` — a hand-rolled minimal single-strip Float32 GeoTIFF: 16-tag IFD, `GeoKeyDirectoryTag` (Projected + PixelIsPoint + `ProjectedCSTypeGeoKey = EPSG`), `ModelPixelScaleTag`, `ModelTiepointTag`, `GDAL_NODATA`, metadata in `ImageDescription`. North-up (grid rows flipped). Zero new dependency. A dev-only byte-offset TIFF reader round-trips the Float32 pixels, geo-keys, and geotransform in-test.
- `export/csv.rs::encode_spectra_csv` — `band_index, exact_hz, <receiver…>` rows (exact Hz from `FreqAxis::centres`, never nominal) + `dBA_total`/`dBC_total` footer rows + the `#`-comment attribution header.

**Task 2 — GeoJSON encoder + WASM boundary (commit 786f3ad):**
- `export/geojson.rs::encode_isophone_geojson` — one RFC-7946 `MultiPolygon` `Feature` per iso-band (band range + fill colour + weighting in properties) via the in-tree `geojson` crate, with the `ExportMeta` footer as `FeatureCollection` foreign members.
- `IsoBand::fill_polygons` (isoband.rs) — reuses the tracer's `rings_to_multipolygon` containment classification so an exporter gets correct exterior/hole nesting from the flat ring list.
- `envi-compute-wasm::export` — `#[wasm_bindgen] export(req)` dispatching `ExportFormat {GeoTiff, GeoJson, Csv}`: GeoTIFF encodes the grid in projected meters; GeoJSON traces iso-bands then reprojects SceneXY→LonLat through `envi_geo::ProjectCrs::to_wgs84` (the one CRS seam) and encodes; CSV maps `ReceiverReadoutDto`→`ReceiverReadout` and encodes. Returns `Vec<u8>` for a `Blob` download.
- `export_filename`/`sanitize_export_filename` — program-derived, path-traversal-safe download name (V12, T-11-04-02).
- `ExportReq`/`ExportFormat`/`ExportCrsDto`/`ExportGridDto` ts-rs DTOs; `ReceiverReadoutDto` gained `Deserialize` (wire shape unchanged); `wire.ts` regenerated, no-drift green.

## Verification

- `cargo test -p envi-compute` (63) + `-p envi-compute-wasm` (48) + `-p envi-service` (all suites incl. `wire_no_drift`) — green, including the GeoTIFF round-trip decode, CSV column/exact-Hz/attribution asserts, GeoJSON RFC-7946 validity + attribution foreign member, the WASM `export` boundary test for all three formats (WGS84 reprojection asserted), and the filename-sanitization test.
- `cargo clippy -p envi-compute -p envi-compute-wasm --all-targets -- -D warnings` — clean.
- `cargo fmt --check` (workspace) — clean.
- `cargo build --release` (workspace) — clean.
- `cargo build -p envi-compute-wasm --target wasm32-unknown-unknown` — clean (envi-geo compiles for wasm32).
- `cargo tree -p envi-compute` — NO `tiff`/`geotiff-writer`/`tiff-writer` (zero new dep, hand-rolled GeoTIFF).
- `cargo tree -p envi-engine --depth 1` — exactly `ndarray + num-complex + thiserror` (3-dep quarantine unchanged).
- Acceptance greps: `grep -c "ExportFormat" web/src/generated/wire.ts` = 2 (≥ 1); `grep -c "envi_geo\|LonLat" crates/envi-compute-wasm/src/export.rs` = 8 (≥ 1).

## Deviations from Plan

**Auto-fixed / adjustments (Rule 2/3 — no architectural change):**

**1. [Rule 3 - Blocking] `ReceiverReadoutDto` needed `Deserialize`.**
- **Found during:** Task 2 — `ExportReq` embeds `Option<Vec<ReceiverReadoutDto>>` and derives `Deserialize`, but the DTO was `Serialize`-only (a result-facing type).
- **Fix:** Added `Deserialize` to `ReceiverReadoutDto`. The ts-rs wire shape is unchanged (ts-rs output does not depend on which of Serialize/Deserialize is derived); no-drift stays green.
- **Files:** `crates/envi-compute-wasm/src/dto.rs`.
- **Commit:** 786f3ad.

**2. [Signature] `encode_geotiff(grid, meta)` folds EPSG into `ExportMeta`** rather than the plan's illustrative `encode_geotiff(grid, epsg, meta)` — one identity object, no redundant param. The must_haves `contains: "pub fn encode_geotiff"` is satisfied.

**3. [Signature] `encode_spectra_csv(labels, receivers, axis, meta)`** adds a receiver-labels slice beyond the plan's `(receivers, axis, meta)` — `ReceiverReadout` carries no id, so the TS-minted receiver UUIDs are passed alongside (unlabelled columns fall back to `receiver_<i>`).

None of these changed the architecture, the engine, or the wire contract's TS shape.

## Threat mitigations (from the plan threat_model)

- **T-11-04-01 (info disclosure):** `export` returns `Vec<u8>` only — no server write, no network; the browser Blob-downloads. Nothing leaves the device (D-20).
- **T-11-04-02 (filename tampering):** `sanitize_export_filename` strips path separators / `..` / control chars and appends the format extension — a program-derived name, tested against traversal inputs.
- **T-11-04-03 (GeoTIFF byte encoding):** dims come from the level grid (bounded single-strip encode); every boundary failure is a typed `ComputeError::Export`, never a panic on data.

## Known Stubs

None. All three encoders produce valid, self-describing bytes; the boundary returns typed errors for missing payloads.

## Follow-ups / notes for downstream

- **web/dist wasm bundle NOT rebuilt** (per plan: "if wasm rebuild is needed for downstream, note it — the export UI is 11-09"). The `export`/`export_filename` `#[wasm_bindgen]` boundaries are compiled and native-tested, but the committed `web/dist` threaded-wasm bundle and the export-menu UI wiring land in **11-09** (ExportMenu). No `web/dist` change was made here.
- The boundary takes the cached grid/breaks/receivers in the request DTO; 11-09 supplies them from the worker's cached `LevelGrid` (11-02) and the `readout_receivers` results (11-05).

## Self-Check: PASSED

All 5 created source files and both task commits (8fd9582, 786f3ad) verified present.
