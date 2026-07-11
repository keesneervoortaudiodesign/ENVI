---
phase: 08-gis-ingestion-dgm
plan: 04
subsystem: gis
tags: [envi-gis, geojson, geo, worldcover, overpass, ahn, glo30, provenance, wasm-safe, rust]

# Dependency graph
requires:
  - phase: 08-01
    provides: envi-geo RD New (RdNewCrs to_rd/to_wgs84) — the terrain reprojection boundary
  - phase: 08-02
    provides: envi-gis COG core (decode_window → Raster<f32> + GeoTransform, nodata holes)
provides:
  - "envi_gis::registry — SourceDescriptor table as data (AHN4/GLO-30/WorldCover/Overpass) with coverage lookup, verified CORS mode, committed AHN kaartblad index"
  - "envi_gis::terrain — decimate_window (bounded samples), terrain_features (WGS84 elevation_point features), sample_base_elevation (footprint-boundary median)"
  - "envi_gis::impedance_table — 11-row reviewed WorldCover→Nord2000 class table (σ resolved via the engine, never restated)"
  - "envi_gis::buildings — Overpass JSON → building features with locked height chain + eaves_height_m + provenance, skip-and-report"
  - "envi_gis::merge — D-09 re-import merge by (source, source_ref) with user_modified guard"
  - "envi_gis::provenance — D-11 provenance stamping as plain GeoJSON properties"
affects: [08-05, 08-06, 08-07, 08-08, gis-ingestion, wasm-boundary, import-pipeline]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Registry-as-data (D-04): new national DTMs slot in as SourceDescriptor rows + coverage polygons, no control-flow change"
    - "Committed-artifact (AHN kaartblad index) with DO-NOT-EDIT + sha256 banner, embedded via include_str!, parsed only in tests"
    - "One-source-of-truth σ: impedance table maps to engine letters; per-row test resolves σ through envi_engine::scene::impedance_class"
    - "Skip-and-report over untrusted third-party geometry (D-07): invalid Overpass features skipped with a per-feature report, never a layer failure"
    - "Provenance/merge identity as plain properties (Pattern 4): no store schema change, no parallel import ledger"

key-files:
  created:
    - crates/envi-gis/src/registry.rs
    - crates/envi-gis/src/registry/ahn_index.toml
    - crates/envi-gis/src/terrain.rs
    - crates/envi-gis/src/impedance_table.rs
    - crates/envi-gis/src/provenance.rs
    - crates/envi-gis/src/buildings.rs
    - crates/envi-gis/src/merge.rs
  modified:
    - crates/envi-gis/src/lib.rs

key-decisions:
  - "Coverage lookup gates AHN vs GLO-30 by a coarse WGS84 NL hull (bbox-center point-in-polygon); precise tile selection uses the committed ahn_index.toml — the hull is a gate, the index is the truth"
  - "terrain_features emits WGS84 lon/lat elevation_point features (no id, TS assigns UUID); RD New / WGS84 → WGS84 via envi_geo only (zero inline proj strings; grep proj= = 0)"
  - "sample_base_elevation is the footprint-boundary median (never a DSM read under the roof); typed None when terrain absent (never a silent 0.0)"
  - "Buildings emit eaves_height_m (the exact key building_from_feature reads) — never a parallel height_m"
  - "GisError gained Reproject + Json variants (additive; no external exhaustive match on GisError exists yet)"

patterns-established:
  - "Registry-as-data source table with per-source CORS capability"
  - "σ resolved through the engine in the per-row test, never restated in envi-gis"
  - "D-09 merge keyed on (source, source_ref) with user_modified guard, deterministic ordering"

requirements-completed: []

# Metrics
duration: 28min
completed: 2026-07-11
status: complete
---

# Phase 8 Plan 4: envi-gis Feature Layer Summary

**Deterministic envi-gis feature layer — source registry (AHN/GLO-30/WorldCover/Overpass as data), terrain decimation → WGS84 elevation_point features + footprint-boundary base elevation, the SC3 reviewed WorldCover→σ table (σ via the engine), Overpass buildings with the locked height chain + eaves_height_m, and D-09 re-import merge — all pure-Rust, WASM-safe, provenance-stamped.**

## Performance

- **Duration:** ~28 min
- **Started:** 2026-07-11T03:19:00Z
- **Completed:** 2026-07-11T03:40:17Z
- **Tasks:** 3
- **Files modified:** 8 (7 created, 1 modified)

## Accomplishments
- **Source registry (D-04):** `SourceDescriptor` table as pure data with `SourceKind`, `Coverage` (Global | NL hull), verified D-02 CORS mode, license + attribution; `plan(bbox)`/`terrain_source(bbox)` select AHN4 DTM inside the NL hull and GLO-30 DSM globally. Committed `registry/ahn_index.toml` kaartblad index with the DO-NOT-EDIT + sha256 provenance banner.
- **Terrain:** `decimate_window` grid-strides a decoded window to a bounded sample set (≤ target, hard-capped `MAX_TERRAIN_POINTS = 50_000`, well under envi-dgm's 500k), dropping nodata holes; `terrain_features` reprojects each sample to WGS84 via `envi_geo` and builds editable `elevation_point` features (WGS84 on the wire, no UTM/project coords, no id); `sample_base_elevation` returns the footprint-boundary median, typed `None` when terrain is absent.
- **Impedance table (SC3):** 11-row reviewed WorldCover→Nord2000 class table + `worldcover_to_class`; the per-row test resolves σ through `envi_engine::scene::impedance_class` (never restated). `DEFAULT_ROUGHNESS_CLASS = 'N'`.
- **Buildings + merge:** Overpass JSON (ways + multipolygon relations) → building features with the locked D-10 height chain (measured → `height`/`building:height` tag → `building:levels`×3+1.5 → user default), emitting `eaves_height_m` + `height_provenance` + provenance; ring validation with skip-and-report. D-09 re-import merge keyed on `(source, source_ref)` with the `user_modified` guard.
- **Provenance:** D-11 stamping as plain GeoJSON properties (unknown-props-preserved contract; zero store schema change) + `merge_key`/`is_user_modified` readers.

## Task Commits

Each task was committed atomically:

1. **Task 1: Source registry (D-04) + committed AHN kaartblad index** — `aa7eb20` (feat)
2. **Task 2: Terrain decimation + base elevation; SC3 impedance table; provenance** — `4625505` (feat)
3. **Task 3: Overpass buildings (height chain + provenance) + re-import merge** — `573bbe1` (feat)

## Files Created/Modified
- `crates/envi-gis/src/registry.rs` — `SourceDescriptor` registry-as-data, coverage lookup, `plan`/`terrain_source`, embedded AHN index.
- `crates/envi-gis/src/registry/ahn_index.toml` — committed kaartblad tile↔RD bbox index with sha256 provenance banner.
- `crates/envi-gis/src/terrain.rs` — `decimate_window`, `terrain_features` (WGS84), `sample_base_elevation` (footprint-boundary median).
- `crates/envi-gis/src/impedance_table.rs` — reviewed 11-row WorldCover→Nord2000 table + `worldcover_to_class`; σ via the engine.
- `crates/envi-gis/src/provenance.rs` — provenance key set, `Provenance::stamp`/`into_properties`, `merge_key`/`is_user_modified`.
- `crates/envi-gis/src/buildings.rs` — Overpass parse, `height_from_tags` chain, `eaves_height_m` emit, ring validation + `SkipReport`.
- `crates/envi-gis/src/merge.rs` — D-09 re-import merge by `(source, source_ref)` with `user_modified` guard.
- `crates/envi-gis/src/lib.rs` — new `pub mod`s; `GisError::Reproject` + `GisError::Json` variants.

## Decisions Made
- Coverage lookup uses a coarse WGS84 NL hull to gate AHN vs GLO-30; the committed `ahn_index.toml` is the precise tile↔RD source (documented in the module). The hull excludes neighbouring capitals (Brussels/Paris/Berlin/London) — tested.
- `terrain_features` carries WGS84 lon/lat per the Phase-6 wire contract; the server converts WGS84→SceneXY. No project/UTM coordinates leave this crate; a test asserts degree-range coordinates.
- σ is never restated in envi-gis — the per-row test asserts every WorldCover class resolves through `envi_engine::scene::impedance_class`. The only σ numbers in the crate live in a `//!` doc comment (explicitly permitted).
- `GisError` gained `Reproject` and `Json` variants (additive); no crate outside envi-gis references `GisError`, so no exhaustive-match breakage.

## Deviations from Plan

None - plan executed exactly as written.

The plan's `Cargo.toml` file entry required no change — `envi-engine`, `envi-geo`, `geo`, `geojson`, `serde`/`serde_json`, `thiserror` (runtime) and `approx`, `toml` (dev) were already present from 08-02. No new dependency was installed (threat T-08-04-SC preserved).

**Total deviations:** 0.
**Impact on plan:** None — all `must_haves`, acceptance criteria, and threat-model mitigations satisfied as written.

## Threat Model Coverage
- **T-08-04-01 (unbounded terrain points):** `decimate_window` auto-decimates to `target_points`, hard-capped at `MAX_TERRAIN_POINTS = 50_000` (test asserts ≤ target and ≤ cap).
- **T-08-04-02 (poisoned buildings):** ring validation (finite, ≥3 verts, closed ≥4 positions) + skip-and-report; non-finite/negative heights rejected; malformed JSON → typed `GisError::Json`; no panic on data.
- **T-08-04-03 (wrong ground effect):** per-row test resolves σ via the engine; `TABLE.len() == 11` pinned; table is user-reviewed (SC3 mechanism).
- **T-08-04-04 (inflated base heights):** `sample_base_elevation` uses the footprint-boundary median, never a DSM read under the roof (test proves a spike under the footprint does not move the base).
- **T-08-04-SC (crate deps):** no new install this plan.

## Issues Encountered
None. Two clippy nits (collapsible-if → let-chain, manual-clamp, nonminimal-bool) were resolved during the fmt/clippy gate before each commit.

## Verification
- `cargo test -p envi-gis` — **28 unit + 8 integration tests pass** (registry, terrain, impedance, provenance, buildings, merge).
- `cargo clippy -p envi-gis --all-targets -- -D warnings` — clean.
- `cargo fmt --check -p envi-gis` — clean.
- `grep -rn "proj=" crates/envi-gis/src` — **zero** (single reprojection boundary preserved).
- No σ literal restated in production code (only in a `//!` doc comment).

## User Setup Required
None - no external service configuration required (this crate is sans-I/O; TS owns fetch/OPFS).

## Next Phase Readiness
- The deterministic feature layer is complete and consumed by later Wave-2/3 plans:
  - **08-05** (TS fetchers/OPFS/import job) drives these functions from the browser.
  - **08-06** may build `terrain_features`/TIN wiring on `decimate_window` + `sample_base_elevation`.
  - **08-07** (ImportPanel) surfaces the registry attribution/CORS + GLO-30 surface-model badge and the impedance debug overlay.
- **DATA-02/DATA-03** requirements: their deterministic core (WorldCover→σ table, Overpass building parse + height chain) landed here; the *fetch* half (TS fetchers/panel) completes in later phase-08 plans, so DATA-02/03 are finalized at phase close — left `Pending` in REQUIREMENTS.md to avoid over-claiming (multi-plan requirements).
- **No blockers.** Feature ids are intentionally TS-assigned (`crypto.randomUUID()`); imported terrain/building features are store-valid only after TS stamps the `id` — the documented cross-boundary contract.

## Self-Check: PASSED
- All 7 created files exist on disk; `lib.rs` modified.
- Commits `aa7eb20`, `4625505`, `573bbe1` present in `git log`.
- All plan `<acceptance_criteria>` and `<verification>` re-run green.

---
*Phase: 08-gis-ingestion-dgm*
*Completed: 2026-07-11*
