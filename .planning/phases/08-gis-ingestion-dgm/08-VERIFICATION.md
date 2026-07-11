---
phase: 08-gis-ingestion-dgm
verified: 2026-07-11T00:00:00Z
status: passed
score: 5/5 success criteria verified (DATA-01..04 all realized)
behavior_unverified: 0
overrides_applied: 0
gates:
  cargo_test_gis: "pass (envi-gis / envi-gis-wasm / envi-geo — 0 failed)"
  proxy_ssrf_contract: "pass (contract_proxy 7/7)"
  tsc_noemit: "pass (exit 0)"
  playwright_e2e: "pass (17/17, incl. import + DATA-04 offline replay + D-07 retry)"
notes:
  - "The re-import WASM trap logged in deferred-items.md (08-08) was subsequently CLOSED: crates/envi-gis/src/merge.rs is now total (zero unwrap/expect/unreachable/panic on the data path) with the CR-01 group-key regression tests. Not a residual gap."
  - "Scope boundaries deliberately honored (NOT gaps): no auth this phase (project-level amendment deferred), DSM→DTM flattening deferred (D-05 flag-only), Overture buildings deferred behind OSM/Overpass (D-10), national DTMs beyond AHN deferred (pluggable registry built)."
informational:
  - "Doc-consistency (gate 5, runs after verify): ROADMAP.md Progress table (line 404) still shows Phase 8 '5/8 In Progress' and Wave 6 08-08 unchecked (line 318), while ROADMAP line 160 + REQUIREMENTS.md mark DATA-01..04 Complete. Reconcile in the doc-consistency scan."
---

# Phase 8: GIS Ingestion & DGM — Verification Report

**Phase Goal:** The NoizCalc "Import" moment — users pull real-world terrain, ground cover, and buildings for the viewport onto a triangulated DGM TIN, and everything imported is an ordinary editable object ("check and complete").
**Verified:** 2026-07-11
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (Success Criteria)

| # | Success Criterion | Status | Evidence |
|---|-------------------|--------|----------|
| SC1 | Viewport import fetches DTM/DSM (AHN4 preferred, GLO-30 fallback flagged surface model) + WorldCover + OSM buildings → editable scene objects on a DGM TIN, with live progress + clear failure states | ✓ VERIFIED | Source selection: `registry.rs:228 terrain_source` (AHN in NL hull, GLO-30 elsewhere as `SourceKind::Dsm`) + test `nl_interior_bbox_selects_ahn4_dtm_non_nl_selects_glo30_dsm`. Materialization: `terrain.rs:121 terrain_features`→`elevation_point`, `landcover.rs:vectorize_landcover`→`ground_zone`, `buildings.rs:114 buildings_from_overpass`→`building`. Orchestration/progress/partial-failure/retry: `importJob.ts` per-layer machines (`startLayer`/`setLayerProgress`/`failLayer`/`retryLayer`, independent `AbortController` per layer, D-07). E2E `import.spec.ts:22` asserts all 3 kinds land + `dgm-triangle-count`=1; `import.spec.ts:68` proves Overpass-429 errors only buildings + retry recovers. |
| SC2 | All fetched data cached per project in OPFS; compute path reads ONLY the local cache (verified network-off); network touched only at ingestion | ✓ VERIFIED | `opfs.ts` per-project cache (`putTile`/`getTile` under `projects/<uuid>/cache/<source>/<tile>`); `importJob.ts:184 loadTileBytes` is cache-first + reads back from OPFS after write. Proof: `import-offline-replay.spec.ts` — Phase 1 populates OPFS, Phase 2 inverts network (record+abort all GIS routes), re-runs compute → `gisEgress.toEqual([])` AND `terrainCount > 0` (real OPFS read), then evicts the cached tile → same path errors + a blocked PDOK fetch is recorded (negative guard: success came from OPFS, not memory). Ends with `unmocked.toEqual([])`. Passing E2E. |
| SC3 | WorldCover class → Nordtest σ/impedance is a reviewed table with a per-row unit test asserting every row, plus an impedance debug overlay | ✓ VERIFIED | `impedance_table.rs:40 WORLDCOVER_TABLE` — all 11 WorldCover v200 classes → engine letter A–H with review rationale. Per-row test `every_row_resolves_sigma_through_the_engine_never_restated` resolves σ via `envi_engine::scene::impedance_class` for every row (one source of truth; no σ literal in envi-gis — confirmed by grep). Overlay: `impedanceOverlay.ts ImpedanceOverlay` (per-class fill + no-data wash, style.load re-hydrate); E2E toggles `import-debug-toggle` and asserts `debugOverlay===true`. |
| SC4 | Building height fallback chain (measured→height tag→levels×3+1.5→user default) with per-feature provenance (source+license+retrieval date); base elevations from footprint-boundary ground, never DSM-under-building | ✓ VERIFIED | Chain: `buildings.rs:62 height_from_tags` (locked order, tolerant parse, negative/non-finite rejected) + tests `height_chain_covers_each_branch...`, `non_finite_and_negative_heights_are_rejected_and_fall_through`. Provenance: `Provenance{source,license,retrieved_at,height_provenance}` stamped; emits `eaves_height_m` (exact key `building_from_feature` reads), no parallel `height_m`, no Rust UUID. Base elevation: `terrain.rs:174 sample_base_elevation` = footprint-boundary median; test `base_elevation_is_boundary_median_ignoring_dsm_spike_under_roof` (100 m under-roof spike ignored, 10 m boundary median), `..._returns_none_when_no_sample_available` (never a fabricated 0.0). |
| SC5 | Map shows attribution for OSM/Overture/ESA WorldCover/Copernicus | ✓ VERIFIED | `attribution.ts IMPORT_ATTRIBUTIONS` credits AHN, GLO-30 (Copernicus/ESA), ESA WorldCover (CC BY 4.0), OSM (ODbL), mirroring `registry.rs` `attribution` fields (one source of truth); `attachImportAttribution` adds a MapLibre `AttributionControl`; `ImportPanel.tsx` renders them as React text children. E2E asserts `import-attribution` contains "AHN", "ESA WorldCover", "OpenStreetMap". Overture credit is N/A this phase — Overture buildings are deferred (D-10); the sources actually used are all attributed. |

**Score:** 5/5 success criteria verified (0 present-but-behavior-unverified).

### Requirements Coverage

| Requirement | Description | Status | Evidence |
|-------------|-------------|--------|----------|
| DATA-01 | Fetch terrain (GLO-30 + national LiDAR DTM), whole-tile browser fetch, OPFS cache, WASM window | ✓ SATISFIED | `registry.rs` terrain sources + `cog/window.rs decode_window` (BigTIFF AHN + classic-TIFF GLO-30, decode-bomb budget) + `fetchers.ts` direct/proxy + `opfs.ts`. Rust `cog_window` tests pass incl. `bigtiff_fixture_decodes_against_gdal`. |
| DATA-02 | Fetch ESA WorldCover → Nordtest σ/impedance reviewed mapping table | ✓ SATISFIED | `impedance_table.rs` (11-row reviewed table + per-row test) + `landcover.rs` vectorization → `ground_zone`. |
| DATA-03 | Fetch buildings (OSM/Overpass) with height-resolution fallback chain | ✓ SATISFIED | `buildings.rs buildings_from_overpass` + locked chain + skip-and-report (D-07). |
| DATA-04 | Cache tiles/data locally (OPFS) per project; compute reads only local cache (network-off verified) | ✓ SATISFIED | `opfs.ts` + `import-offline-replay.spec.ts` zero-egress + negative-guard proof. |

### Required Artifacts

| Artifact | Provides | Status |
|----------|----------|--------|
| `crates/envi-geo/src/crs.rs` (RD_NEW) | EPSG:28992 boundary for AHN | ✓ VERIFIED (envi-geo tests pass) |
| `crates/envi-gis/src/cog/window.rs` | sans-I/O windowed COG decode + decode-bomb budget | ✓ VERIFIED (cog_window 8/8) |
| `crates/envi-gis/src/impedance_table.rs` | 11-row reviewed WorldCover→class table + per-row test | ✓ VERIFIED |
| `crates/envi-gis/src/terrain.rs` | terrain→elevation_point + footprint-boundary base elevation | ✓ VERIFIED |
| `crates/envi-gis/src/buildings.rs` | Overpass→building + height chain + provenance | ✓ VERIFIED |
| `crates/envi-gis/src/landcover.rs` | WorldCover→ground_zone (hand-rolled marching squares, contour dep declined) | ✓ VERIFIED |
| `crates/envi-gis/src/registry.rs` | pluggable source registry (AHN/GLO-30/WorldCover/Overpass, CORS map) | ✓ VERIFIED |
| `crates/envi-gis/src/merge.rs` | D-09 re-import merge, total/panic-free (CR-01 group-key fix) | ✓ VERIFIED |
| `crates/envi-service/src/api/proxy.rs` | allowlisted SSRF-proof byte proxy | ✓ VERIFIED (contract_proxy 7/7) |
| `crates/envi-gis-wasm/src/lib.rs` | logic-free wasm-bindgen boundary → envi-gis | ✓ VERIFIED (drives real E2E) |
| `web/src/import/opfs.ts` | per-project OPFS cache | ✓ VERIFIED |
| `web/src/import/importJob.ts` | per-layer import state machine | ✓ VERIFIED |
| `web/src/panels/ImportPanel.tsx` | toggles/status/retry/guardrail/GLO-30 badge/attribution | ✓ VERIFIED |
| `web/src/map/impedanceOverlay.ts` | impedance debug overlay | ✓ VERIFIED |
| `web/tests/e2e/import.spec.ts` + `import-offline-replay.spec.ts` | SC1/3/5 journey + DATA-04 replay | ✓ VERIFIED (pass) |

### Key Link Verification

| From | To | Via | Status |
|------|----|----|--------|
| `impedance_table.rs` | `envi_engine::scene::impedance_class` | per-row σ resolved through engine, never restated | ✓ WIRED (test + grep: no σ literal in envi-gis) |
| `terrain.rs` | `envi_geo` (RdNewCrs/LonLat) | single reprojection boundary, no inline proj | ✓ WIRED |
| `importJob.ts` | `opfs.ts` | cache-first read, miss→fetch→write→read-back; compute reads OPFS only | ✓ WIRED (offline replay proves) |
| `importJob.ts` | `crates/envi-service/src/api/proxy.rs` | proxy-required sources route through `/api/v1/proxy/{source}/{path}` | ✓ WIRED |
| `attribution.ts` | `registry.rs` attribution fields | credit strings mirror the registry source of truth | ✓ WIRED |

### Behavioral Spot-Checks / Gates

| Check | Command | Result | Status |
|-------|---------|--------|--------|
| GIS crate tests | `cargo test -p envi-gis -p envi-gis-wasm -p envi-geo` | 0 failed (cog_window 8/8, impedance/buildings/terrain/merge/registry unit tests) | ✓ PASS |
| Proxy SSRF contract | `contract_proxy` binary | 7/7 (unknown source 404, prefix-escape 400, non-GET 405, dot-dot rejected) | ✓ PASS |
| TypeScript typecheck | `npx tsc --noEmit` | exit 0 | ✓ PASS |
| Playwright E2E | `npx playwright test` | 17/17 (incl. import journey, DATA-04 offline replay, D-07 retry) | ✓ PASS |

### Anti-Patterns Found

| File | Pattern | Severity | Impact |
|------|---------|----------|--------|
| — | No stubs/placeholders/TODO-FIXME-XXX in Phase-8 changed files driving goal behavior | ℹ️ Info | Merge data path is total (0 unwrap/expect/unreachable outside `#[cfg(test)]`); base elevation returns None not 0.0; unknown WorldCover code → None not silent default. |

### Deferred / Scope Boundaries (NOT gaps)

- **Re-import into a populated scene** — the WASM `unreachable` trap logged in `deferred-items.md` was **closed**: `merge.rs` is now total with the CR-01 group-key regression tests. Verified resolved.
- **No auth** this phase (WASM/login-server re-architecture is a project-level amendment deferred before Phase 10).
- **DSM→DTM flattening / under-footprint exclusion** — GLO-30 handled flag-only (D-05 surface-model badge), acoustic correction deferred to a later phase.
- **Overture GeoParquet buildings** — deferred behind OSM/Overpass (D-10); SC5's "Overture" credit is therefore N/A this phase.
- **National DTMs beyond AHN** — the pluggable data-driven registry is built (D-04); more countries are pure-data additions later.
- **Efficiency/altitude follow-ups** (simplify gate) and the low-priority world-copy antimeridian edge — logged in `deferred-items.md`, behavior-neutral, non-blocking.

### Human Verification Required

None. Every success criterion has automated code + passing-test evidence. Imported features are ordinary 9-kind scene objects committed through the Phase-7 scene path (`loadImportedScene`), so their editability is by construction and is exercised by the Phase-7 "edit every kind" E2E; no additional manual check is required to certify the Phase-8 goal.

### Gaps Summary

No blocking gaps. All 5 success criteria and all 4 DATA requirements are realized in shipped code with passing gates. The only follow-up is an informational doc-consistency reconciliation (ROADMAP Progress table / Wave-6 checkbox still show "In Progress") that the gate-5 documentation scan handles as part of close-out.

---

_Verified: 2026-07-11_
_Verifier: Claude (gsd-verifier)_
