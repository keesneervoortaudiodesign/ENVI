---
phase: 09-path-extraction-weather
plan: 01
subsystem: gis
tags: [rust, geox, cut-profile, tin, spade, geo, impedance, nord2000, oracle, wasm-safe]

# Dependency graph
requires:
  - phase: 08-gis-ingestion-dgm
    provides: envi-gis pure-Rust COG decode core (Raster/decode_window), impedance_table (worldcover_to_class), ground_zone vocabulary, provenance/oracle-fixture pattern
  - phase: 08-gis-ingestion-dgm
    provides: envi-dgm constrained-Delaunay TIN (build_tin + Tin::interpolate_z, None outside hull)
  - phase: 02-ground-effect
    provides: envi_engine::scene TerrainProfile / GroundSegment / impedance_class (class B = 31.5, single source of truth for σ)
provides:
  - "GEOX-01: envi_gis::profile::cut_profile — S→R DEM cut-profile over the DGM TIN as strictly-ascending ground-z points (TerrainProfile input)"
  - "GEOX-02: envi_gis::impedance::segment_ground — per-interval GroundSegment resolution (drawn > imported > default) with boundary crossings spliced into the point list"
  - "Committed r.profile-faithful oracle fixture (real-DEM extract + reference CSV) + tools/rprofile_oracle generator; no GRASS/Python at test time"
  - "GroundSegmentation { points, planar_xy, segments } — the spliced cut-plane ready for TerrainProfile::new"
affects: [09-02-grid, 09-03-screening, 09-04-path-assembly, 10-solve-tensor]

# Tech tracking
tech-stack:
  added: [envi-dgm (new envi-gis dependency, pure spade — WASM-safe)]
  patterns:
    - "TIN-sampled cut-profile validated against an INDEPENDENT raster-bilinear oracle within a documented tolerance (not bit-equality) — mirrors the Phase-8 committed-COG oracle"
    - "Per-INTERVAL impedance classification with polygon-boundary vertices spliced into the ascending-x list before segments are built"
    - "σ resolved ONLY through envi_engine::scene::impedance_class; no σ literal in the GIS crate"

key-files:
  created:
    - crates/envi-gis/src/profile.rs
    - crates/envi-gis/src/impedance.rs
    - crates/envi-gis/tests/profile_oracle.rs
    - crates/envi-gis/tests/fixtures/rprofile/ (rprofile_dem.tif, rprofile.csv, README.md)
    - tools/rprofile_oracle/gen_rprofile_fixture.py
  modified:
    - crates/envi-gis/src/lib.rs
    - crates/envi-gis/Cargo.toml
    - Cargo.lock

key-decisions:
  - "GEOX-01 samples the DGM TIN (barycentric), not the raw raster; the oracle pins a documented TIN-vs-bilinear tolerance (0.05 m), not bit-equality (09-RESEARCH A3)"
  - "cut_profile guarantees >= 2 strictly-ascending ground-z points or a typed DegenerateProfile; heights are NEVER baked into a profile z (hSv/hRv trap)"
  - "segment_ground returns GroundSegmentation { points, planar_xy, segments } (not a bare Vec<GroundSegment>) so boundary-spliced points stay in sync with segments for TerrainProfile::new"
  - "Roughness passes through as meters (class N = 0); the engine has no roughness ladder to restate, so none is invented"

patterns-established:
  - "Independent-oracle self-build: the r.profile reference is a from-scratch raster-bilinear walk (tools/rprofile_oracle), never the ENVI extractor — no self-referential fixture"
  - "New envi-gis DoS caps + typed errors: MAX_PROFILE_POINTS, OutsideHull, ProfileTooLong, DegenerateProfile, UnresolvableClass"

requirements-completed: [GEOX-01, GEOX-02]

# Metrics
duration: 45min
completed: 2026-07-11
status: complete
---

# Phase 9 Plan 01: Cut-profile extractor (GEOX-01) + impedance segmentation (GEOX-02) Summary

**The first two links of the GIS→engine geometry pipeline as pure-Rust, WASM-safe `envi-gis` modules: a DGM-TIN cut-profile extractor pinned by a committed r.profile-faithful oracle, and per-interval ground-impedance segmentation (drawn > imported > default) with polygon boundaries spliced onto the cut plane.**

## Performance

- **Duration:** ~45 min
- **Completed:** 2026-07-11
- **Tasks:** 2 (both `type=auto tdd=true`)
- **Files modified:** 10 (7 created, 3 modified)

## Accomplishments
- **GEOX-01** `cut_profile(&Tin, s_xy, r_xy, step_m) -> Result<Vec<[f64;2]>, GisError>`: walks the source→receiver line at cell resolution over the DGM TIN, emitting strictly-ascending `(x, z)` **ground** points. A hull miss maps `Tin::interpolate_z`'s `None` to `GisError::OutsideHull` (never a fabricated `0.0`); over-length requests are rejected before allocation as `ProfileTooLong`; degenerate step / coincident endpoints are `DegenerateProfile`.
- **Committed offline oracle**: `tools/rprofile_oracle/gen_rprofile_fixture.py` generates a real-DEM extract (`rprofile_dem.tif`) plus an **independent raster-bilinear** reference CSV; `tests/profile_oracle.rs` decodes the DEM through the real COG path, builds a TIN, runs `cut_profile`, and asserts the TIN-vs-bilinear delta is non-zero yet within the documented `0.05 m` tolerance — no GRASS/Python at test time.
- **GEOX-02** `segment_ground(...) -> Result<GroundSegmentation, GisError>`: splices `ground_zone` boundary ∩ S→R crossings into the ascending-x point list, then classifies each **interval** at its midpoint in priority **drawn > imported > default**, resolving σ only through `impedance_class` (class B = 31.5). `segments.len() == points.len() - 1`; the spliced result composes a valid `TerrainProfile`.
- Added `envi-dgm` (pure spade) as an `envi-gis` dependency with the quarantine intact: engine still exactly 3 deps, `envi-gis` gains no async/network/browser edge.

## Task Commits

1. **Task 1: Cut-profile extractor (GEOX-01) + committed r.profile oracle** — `1220261` (feat)
2. **Task 2: Impedance segmentation (GEOX-02)** — `bfb5b86` (feat)

_Both tasks are `tdd=true`; each was implemented with its unit tests and committed atomically (single feat commit per task, MVP+TDD gate inactive)._

## Files Created/Modified
- `crates/envi-gis/src/profile.rs` — GEOX-01 cut-profile extractor + unit tests
- `crates/envi-gis/src/impedance.rs` — GEOX-02 segmentation (DrawnZone/ImportedZone/GroundSegmentation) + unit tests
- `crates/envi-gis/tests/profile_oracle.rs` — committed-oracle integration test (decode → TIN → cut_profile → compare)
- `crates/envi-gis/tests/fixtures/rprofile/` — `rprofile_dem.tif`, `rprofile.csv` (provenance/SHA header), `README.md`
- `tools/rprofile_oracle/gen_rprofile_fixture.py` — offline oracle generator (not a test dependency)
- `crates/envi-gis/src/lib.rs` — `pub mod profile; pub mod impedance;` + new `GisError` variants (`OutsideHull`, `ProfileTooLong`, `DegenerateProfile`, `UnresolvableClass`)
- `crates/envi-gis/Cargo.toml` — `envi-dgm` dependency (runtime + dev)
- `Cargo.lock` — envi-dgm → envi-gis edge

## Decisions Made
- **TIN sampling over raster resampling** (GEOX-01): the scene ground model is a TIN, so barycentric sampling is the exact-on-the-mesh kernel; the oracle tolerates the documented TIN-vs-bilinear delta rather than chasing bit-equality (09-RESEARCH A3, confirmed: max delta non-zero and well under 0.05 m).
- **`GroundSegmentation` return type** (GEOX-02): a bare `Vec<GroundSegment>` would desync from the caller's points once boundary crossings are spliced. Returning the spliced `points` + `planar_xy` alongside `segments` is the only way to satisfy "segment count == points−1" observably and to feed `TerrainProfile::new`. Documented as a signature refinement of the plan's suggested `-> Vec<GroundSegment>`.
- **Roughness as meters, not a re-derived ladder**: drawn zones carry an explicit `roughness_m` (class N = 0 default); imported land cover has no roughness, so it defaults to N. No physical roughness constant is restated in the GIS crate.

## Deviations from Plan

### Auto-fixed / refinements

**1. [Rule 2 — Missing Critical] `segment_ground` return type widened to `GroundSegmentation`**
- **Found during:** Task 2 (impedance segmentation)
- **Issue:** The plan sketched `segment_ground(...) -> Result<Vec<GroundSegment>, GisError>`, but the must-have "boundary crossings are spliced into the ascending-x point list" changes the point count, so a bare segment vector would desync from the caller's points and break `TerrainProfile::new` (segments.len() must equal points.len()−1).
- **Fix:** Return `GroundSegmentation { points, planar_xy, segments }` — the spliced cut plane, ready for `TerrainProfile::new`.
- **Files modified:** crates/envi-gis/src/impedance.rs
- **Verification:** `segmentation_builds_a_valid_terrain_profile` constructs a `TerrainProfile` from the returned points+segments.
- **Committed in:** bfb5b86 (Task 2 commit)

**2. [Rule 2 — Missing Critical] Extra typed error variants beyond the two named**
- **Found during:** Tasks 1 & 2
- **Issue:** The plan named `OutsideHull` and `ProfileTooLong`; correct no-panic-on-data handling also needs a variant for degenerate profiles (non-positive step / coincident endpoints / <2 points) and for an unresolvable impedance class (never a fabricated σ).
- **Fix:** Added `GisError::DegenerateProfile { what }` and `GisError::UnresolvableClass { class }` in the existing struct-variant style (the plan's artifact note explicitly allowed "any unresolvable-class variant used by segmentation").
- **Files modified:** crates/envi-gis/src/lib.rs
- **Verification:** dedicated unit tests assert each variant; `PartialEq` preserved.
- **Committed in:** 1220261 / bfb5b86

---

**Total deviations:** 2 refinements (both correctness-necessary). **Impact:** No scope creep — GEOX-03/GRID-01/METX are untouched (later plans). All prohibitions honored: no engine dep added (3-dep quarantine byte-identical), no async/network edge in envi-gis, no σ literal, ground-z-only (no hSv/hRv leak), no self-referential oracle.

## Issues Encountered
- **Windows file lock on `envi-service.exe`** during the full `cargo test`: a running server (PID 49432, user-launched) holds the binary, so cargo cannot replace its test executable. Per project hygiene rules the process was **not** killed. Verified green via `cargo test --workspace --exclude envi-service` (37 test blocks OK, 0 failed) plus `cargo clippy --all-targets -- -D warnings` (which compiled `envi-service` clean) and `cargo fmt --check`. `envi-service` does not consume the new modules, so it is unaffected.

## Quality Gates
- `cargo test -p envi-gis`: profile (7) + impedance (8) unit tests + `profile_oracle` (1) green.
- `cargo clippy --all-targets -- -D warnings`: clean (whole workspace, incl. envi-service).
- `cargo fmt --check`: clean.
- `cargo tree -p envi-engine`: exactly `ndarray + num-complex + thiserror` (quarantine intact).
- `cargo tree -p envi-gis`: no HTTP client / async runtime / browser crate.

## Next Phase Readiness
- `cut_profile` + `segment_ground` are the load-bearing inputs every real `SolveJob` consumes. 09-02 (receiver grid) and 09-03 (screening edges, which splice `(x,z)` screen tops into the same profile) can build directly on these seams; 09-04 path assembly composes `TerrainProfile::new(points, segments)` from `GroundSegmentation`.
- The `planar_xy` companion list is the seam 09-03 needs to intersect building/wall geometry against the same S→R line.

## Self-Check: PASSED

All claimed artifacts verified on disk (`profile.rs`, `impedance.rs`, `profile_oracle.rs`, `rprofile_dem.tif`, `rprofile.csv`, `gen_rprofile_fixture.py`) and both task commits (`1220261`, `bfb5b86`) present in git history.

---
*Phase: 09-path-extraction-weather*
*Completed: 2026-07-11*
