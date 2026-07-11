---
phase: 08-gis-ingestion-dgm
plan: 07
subsystem: gis-ingestion
tags: [opfs, wasm, maplibre, react, fetch, cors, import, cog, overpass, worldcover]

# Dependency graph
requires:
  - phase: 08-03
    provides: the allowlisted /api/v1/proxy byte relay (GLO-30 + WorldCover)
  - phase: 08-06
    provides: the envi-gis-wasm boundary + committed generated wire.ts
  - phase: 07 (07-05..07-10)
    provides: the React shell, canonical scene store, Terra Draw view, dgm producer/overlay
provides:
  - "Browser import path: OPFS per-project cache, direct-vs-proxy fetchers, per-layer import state machines, ImportPanel, impedance debug overlay, GLO-30 badge, SC5 attribution"
  - "Two+one thin WASM boundary exports (plan_tiles, window_for_bbox, reproject_ring) so TS can plan tiles + compute windows without doing GIS math"
  - "loadImportedScene commit seam preserving imported provenance/class/height properties"
affects: [08-08, phase-9, phase-10]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Sans-I/O tile planning + window resolution in envi-gis; TS owns fetch/OPFS only"
    - "Per-layer AbortController state machines (dgmTrigger discipline) with serialized merge critical section"
    - "Cache-first read-back-from-OPFS so the compute path literally reads OPFS (DATA-04)"

key-files:
  created:
    - crates/envi-gis/src/tiles.rs
    - web/src/import/wasm.ts
    - web/src/import/opfs.ts
    - web/src/import/fetchers.ts
    - web/src/import/attribution.ts
    - web/src/import/importJob.ts
    - web/src/store/import.ts
    - web/src/map/impedanceOverlay.ts
    - web/src/panels/ImportPanel.tsx
  modified:
    - crates/envi-gis/src/lib.rs
    - crates/envi-gis-wasm/src/dto.rs
    - crates/envi-gis-wasm/src/lib.rs
    - crates/envi-service/tests/wire_no_drift.rs
    - web/src/generated/wire.ts
    - web/src/store/sceneStore.ts
    - web/src/App.tsx
    - web/src/map/MapCanvas.tsx
    - web/dist/*

key-decisions:
  - "Extended the 08-06 WASM boundary (Rule-3, coordinator-approved) with plan_tiles + window_for_bbox + reproject_ring — the only way to keep all GIS geometry in WASM while giving TS the per-tile windows the plan's pipeline needs"
  - "AHN kaartblad index parsed at runtime by a hand-rolled line reader over the committed include_str! blob — ZERO new envi-gis runtime deps (dep-quarantine banner intact); GLO-30 1deg / WorldCover 3deg names are integer-grid math"
  - "Imported features commit via a new loadImportedScene (whole-scene D-09 merge result), NOT commitFeature — which would reseed kind defaults over imported impedance_class / z_m / eaves_height_m"
  - "Compute reads tile bytes back from OPFS after write, so the import compute path is literally OPFS-sourced (DATA-04); the network-off proof is the 08-08 Playwright replay"

patterns-established:
  - "Typed WASM facade (wasm.ts): the single audited cast site over the any-typed generated glue"
  - "Per-layer independence: one AbortController machine per layer; a failure records error + retry without touching siblings (D-07)"

requirements-completed: [DATA-01, DATA-02, DATA-03, DATA-04]

# Metrics
duration: 165min
completed: 2026-07-11
status: complete
---

# Phase 8 Plan 7: Web Import Path (OPFS + Fetchers + ImportPanel) Summary

**The browser import pivot: a viewport import fetches terrain/land-cover/buildings (direct or via the byte proxy per the CORS map), caches whole tiles in per-project OPFS, and commits editable 9-kind scene features through the WASM window/decode/map/parse/merge path — with per-layer toggles, live progress, retry, a max-area guardrail, a GLO-30 surface-model badge, an impedance debug overlay, and SC5 attribution.**

## Performance

- **Duration:** ~165 min
- **Completed:** 2026-07-11
- **Tasks:** 3 (plan) + a coordinator-approved boundary extension
- **Files:** 9 created, 9 modified (incl. rebuilt web/dist)

## Accomplishments

- **Runtime-real compute path.** The terrain layer runs `plan_tiles → cache-first fetch → window_for_bbox → terrain_features → crypto.randomUUID → merge → commit` (never stops at a decoded raster); land cover uses the u8 `map_landcover` path → `ground_zone`; buildings parse Overpass then sample footprint-boundary base elevation off the retained terrain tiles (typed `null`/absent, never `0.0`, when terrain is missing — D-07).
- **Boundary gap resolved.** The 08-06 boundary exposed no way for TS to enumerate covering tiles or compute a per-tile pixel window (both pure geometry the plan's prohibition keeps out of TS). Added three thin `envi-gis-wasm` exports delegating to existing core (`plan_tiles`, `window_for_bbox`, `reproject_ring`), rebuilt the committed wasm bundle, regenerated `wire.ts`, and kept `wire_no_drift` green.
- **DATA-04 network discipline.** OPFS per-project cache (async main-thread API only), and the compute path reads tile bytes back from OPFS after write, so post-import reads come from the local cache.
- **UI surface (SC1/SC3/SC5).** ImportPanel (per-layer toggles/status/progress/retry, guardrail warning, GLO-30 badge, impedance-overlay toggle, attribution) + a MapLibre impedance overlay colouring `ground_zone` by class letter over a project-default wash + the SC5 AttributionControl.

## Task Commits

1. **Boundary: plan_tiles + window_for_bbox** — `b2c5d75` (feat)
2. **Boundary: reproject_ring (building base elevation)** — `1157726` (feat)
3. **Task 1-2: OPFS cache + fetchers + import orchestrator** — `ac20dc5` (feat)
4. **Task 3: ImportPanel + impedance overlay + attribution + web/dist** — `d7ae4a2` (feat)

## Files Created/Modified

- `crates/envi-gis/src/tiles.rs` — tile enumeration (AHN index parse + integer grids) + `window_for_bbox` + `reproject_ring_to_source`
- `crates/envi-gis-wasm/src/{dto,lib}.rs` — 3 thin exports + their DTOs (`PlanTiles*`, `WindowForBbox*`, `ReprojectRing*`, `TileRefDto`)
- `crates/envi-service/tests/wire_no_drift.rs` + `web/src/generated/wire.ts` — registered + regenerated (no-drift green)
- `web/src/import/{wasm,opfs,fetchers,attribution,importJob}.ts`, `web/src/store/import.ts` — the client import engine
- `web/src/map/impedanceOverlay.ts`, `web/src/panels/ImportPanel.tsx` — SC3/SC5 UI
- `web/src/store/sceneStore.ts` (`loadImportedScene`), `web/src/App.tsx`, `web/src/map/MapCanvas.tsx` — wiring

## Deviations from Plan

### Auto-fixed / approved

**1. [Rule 3 - Blocking capability] Extended the WASM boundary (coordinator-approved)**
- **Found during:** design analysis (before any TS was written)
- **Issue:** Task 2's per-tile pipeline (`decode_window`/`terrain_features` need a `PixelWindowDto`) was unimplementable against the committed 08-06 boundary — TS had no way to enumerate covering tiles (esp. the AHN kaartblad index), reproject the WGS84 viewport into the source CRS, or compute a per-tile window from the COG header. All three are pure geometry the plan's own prohibition keeps out of TS. Surfaced as a checkpoint; coordinator approved Option 1 (extend the boundary, folded into 08-07).
- **Fix:** Added `envi_gis::tiles::{plan_tiles, window_for_bbox, reproject_ring_to_source}` (delegating to existing `registry`/`cog`/`envi_geo` core; AHN index parsed via a hand-rolled reader — no `toml`/`serde` runtime dep) + their `envi-gis-wasm` exports/DTOs; rebuilt the committed wasm + regenerated `wire.ts`; registered the 6 new DTOs in `wire_no_drift`.
- **Files modified:** `crates/envi-gis/src/{lib,tiles}.rs`, `crates/envi-gis-wasm/src/{dto,lib}.rs`, `crates/envi-service/tests/wire_no_drift.rs`, `web/src/generated/wire.ts`, `web/src/generated/wasm/*`
- **Verification:** `cargo test -p envi-gis` (10 tiles tests), `wire_no_drift` green, `cargo clippy`/`fmt` clean
- **Commits:** `b2c5d75`, `1157726`

**2. [Rule 2 - Missing commit seam] Added `sceneStore.loadImportedScene`**
- **Found during:** Task 2
- **Issue:** the existing `commitFeature` reseeds kind-default properties (`seedProps`) over an upserted feature — it would overwrite imported `impedance_class` / `z_m` / `eaves_height_m` / provenance; `loadScene` clears spectra and marks clean.
- **Fix:** added `loadImportedScene(collection)` — loads the D-09 merge result preserving properties verbatim, keeps spectra/project, marks dirty, bumps `loadEpoch` for Terra Draw re-hydration.
- **Files modified:** `web/src/store/sceneStore.ts`
- **Commit:** `ac20dc5`

**Total deviations:** 2 (1 approved boundary extension, 1 auto-added commit seam). **Impact:** the boundary extension is additive (all prior exports intact, no-drift green); it is the load-bearing enabler that makes the import compute path runtime-real rather than a stub.

## Known Stubs

None. The impedance overlay's "no data" wash uses the documented `SettingsDto` default ground class `'D'` because the scene store does not carry project settings this phase — an intentional default, not a stub (the overlay never renders a blank hole).

## Verification

- `cargo clippy --all-targets -- -D warnings` — clean (workspace)
- `cargo fmt --check` — clean
- `cargo test` — 24 test binaries pass (incl. `wire_no_drift`, `envi-gis` tiles)
- `npx tsc --noEmit` (web) — exit 0
- `npm run build` — succeeds; `web/dist` rebuilt (wasm bundled)
- `npm run test:unit` — 12 pass
- `grep -rn "createSyncAccessHandle\|dangerouslySetInnerHTML" web/src/import web/src/panels/ImportPanel.tsx` — zero
- Manual/E2E smoke deferred to 08-08 (network-off OPFS replay proof)

## Next

Ready for **08-08** (the offline Playwright E2E: import from fixtures → editable features → network-off replay proving OPFS-only reads → attribution visible).

## Self-Check: PASSED
