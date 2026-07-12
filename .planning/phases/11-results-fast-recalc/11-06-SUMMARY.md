---
phase: 11-results-fast-recalc
plan: 06
subsystem: results-ui
tags: [web, maplibre, isophone, fill-layer, color-scale, wasm, tracer, d-02, d-04, web-06, grid-04, playwright]

# Dependency graph
requires:
  - phase: 11-results-fast-recalc
    provides: "11-02 iso-band tracer core (reconstruct_level_grid + trace_isobands + fill_polygons); 11-05 results shell + ColorScaleEditor slot + wasm main-thread compute pattern"
  - phase: 10-calculation-service
    provides: "envi-compute-wasm boundary + threaded compute WASM build recipe; envi-geo in the wasm graph (11-04)"
provides:
  - "trace_isophones WASM boundary — cached level grid → WGS84 GeoJSON iso-band FeatureCollection (re-contour, no re-solve; SC3)"
  - "web/src/store/colorScale.ts — the single breaks[]/colors[] source of truth + END/viridis/turbo presets + V5 validation"
  - "web/src/map/isophoneLayer.ts — MapLibre FILL layer (D-02) below the scene objects (D-18) + docked MapLegend"
  - "web/src/panels/ColorScaleEditor.tsx — preset picker + editable breaks (NoizCalc §4.6.5) driving live re-contour"
affects: [11-09, 11-10]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Isophones are MapLibre FILL polygons painted by a per-band `fill` property (D-02), NEVER a raster density layer — enforced by a `grep -v '^//' | grep -c heatmap == 0` gate"
    - "One breaks[]/colors[] source of truth drives the tracer + fill layer + legend (legend ≡ contour ≡ class colour); colors.length === breaks.length + 1"
    - "Editing the scale re-runs ONLY the main-thread WASM tracer over the CACHED grid (SC3) — propagation is never re-run"
    - "N editable break edges → N+1 CLOSED bands via cap-extension (contourBreaks) so the below-lowest/above-highest classes render (the tracer's V5 rejects non-finite ±∞)"
    - "trace_isophones returns a GeoJSON FeatureCollection string a `geojson` source consumes directly — the display sibling of the ExportReq GeoJSON arm, sharing the trace_bands_lonlat core"

key-files:
  created:
    - web/src/store/colorScale.ts
    - web/src/store/colorScale.test.ts
    - web/src/map/isophoneLayer.ts
    - web/tests/e2e/isophone.spec.ts
  modified:
    - crates/envi-compute-wasm/src/dto.rs
    - crates/envi-compute-wasm/src/export.rs
    - crates/envi-service/tests/wire_no_drift.rs
    - web/src/generated/wire.ts
    - web/src/panels/ColorScaleEditor.tsx
    - web/src/map/MapCanvas.tsx
    - web/src/app.css
    - web/src/testBridge.ts
    - web/dist
    - Cargo.lock

key-decisions:
  - "trace_isophones is a NEW thin WASM boundary (returns a GeoJSON string, no new result DTO) rather than repurposing the 11-04 `export` GeoJson arm: the export path is footer/identity-heavy (engine_version/tensor_hash/attribution) for downloadable artifacts, while the live layer needs only the weighting label. The GeoJson export arm was refactored to share a single trace_bands_lonlat core (no duplication)."
  - "The single source of truth is the N EDITABLE break edges + N+1 class colours; the tracer receives contourBreaks(breaks) = [LOW_CAP, …breaks, HIGH_CAP] (finite ±1e6 sentinels) so the below-lowest/above-highest EU-END classes render as closed bands. colors.length === breaks.length + 1 is the enforced invariant."
  - "The cached level grid + CRS + weighting label live in colorScale.ts (setIsophoneInput) so the whole isophone surface is one store file; the live production feed from a finished readout's fine-tier lattice is a documented follow-up (mirrors 11-05's DEV-bridge-seeded manifest)."
  - "MapLegend is written with React.createElement so isophoneLayer.ts stays a JSX-free `.ts` module (the plan's declared filename); it is mounted alongside IsophoneLayer in MapCanvas."

requirements-completed: [WEB-06, GRID-04]

# Metrics
duration: 18 min
completed: 2026-07-12
status: complete
---

# Phase 11 Plan 06: Isophone Fill Layer + Colour-Scale Editor (WEB-06 / GRID-04) Summary

**The noise map now renders as MapLibre FILL polygons traced by the 11-02 WASM iso-band tracer (never a raster density layer, D-02), below the scene objects (D-18), with a docked legend; a colour-scale editor (EU-END default + viridis/turbo presets + editable NoizCalc §4.6.5 breaks) re-contours the CACHED level grid on every edit with NO re-solve (SC3) — a single `breaks[]`/`colors[]` array drives the tracer, the fill layer, and the legend so legend ≡ contour ≡ class colour, proven by an offline Playwright UAT on the real bundle + real WASM tracer.**

## Performance

- **Duration:** ~18 min (incl. a nightly `-Zbuild-std` compute-WASM rebuild + a `vite build` dist rebuild)
- **Tasks:** 3 `type=auto` + 1 pre-approved `checkpoint:human-verify` (A4)
- **Files:** 4 created, 10 modified

## Accomplishments

- **`trace_isophones` WASM boundary (Task 2).** A new thin `#[wasm_bindgen]` export re-contours the cached level grid into a WGS84 GeoJSON `FeatureCollection` string a MapLibre `geojson` source consumes directly. It shares a factored `trace_bands_lonlat` core with the 11-04 GeoJSON export arm (`trace_isobands` → `fill_polygons` → SceneXY→LonLat at the one CRS seam, GEOX-04), stamping each band's class colour as a `fill` property so a single fill layer paints by `["get","fill"]`. Request DTO `TraceIsophonesReq` is ts-rs generated (no-drift green); the result is a plain string (no new result DTO). Native tests assert WGS84 bands + fills near NL and a typed error (never a panic) on an invalid break scale.
- **Colour-scale store — the single source of truth (Task 1).** `web/src/store/colorScale.ts` holds `preset` + `breaks` + `colors` (one array pair, `colors.length === breaks.length + 1`), the cached `grid`/`crs`/`weightingLabel` (SC3 re-contour input), and inline V5 validation. The EU-END default `[55,60,65,70,75]` + the six green→violet hex ship as the default (A4, approved below); viridis/turbo sample their canonical ramps at the class count; `contourBreaks` cap-extends the N editable edges into N+1 closed bands. `validateBreaks` rejects non-monotonic/<2/non-finite. 13 unit tests green; NO acoustic math (display-colour interpolation only).
- **MapLibre isophone FILL layer + legend (Task 2).** `web/src/map/isophoneLayer.ts` (`IsophoneLayer` child-of-`<Map>`) subscribes to the single scale and re-contours the cached grid on any change via the WASM tracer, inserting a `fill` layer BELOW the scene-object layers (D-18) and re-adding it after a basemap `style.load`. `MapLegend` docks the six classes (swatch + range label + metadata weighting) bottom-left. Both are driven by the SAME `breaks`/`colors` — legend ≡ contour ≡ class colour.
- **ColorScaleEditor (Task 2).** `web/src/panels/ColorScaleEditor.tsx` fills the 11-05 ResultsPanel slot with the NoizCalc §4.6.5 controls: a preset picker, editable per-edge break rows (with class swatches + a draft buffer so intermediate invalid states show the error without reverting), the uniform generator (smallest interval / interval magnitude / number of intervals + Apply), and ascending / keep-color-sequence toggles. An inline `.form-error` surfaces V5 failures. Every edit mutates the single source of truth → the layer + legend re-render.
- **Offline Playwright UAT (Task 3).** `web/tests/e2e/isophone.spec.ts` drives the real vite-served bundle + real `trace_isophones` WASM over a seeded fixture grid, fully offline: it asserts a `fill` paint type (never a raster density layer), a preset EU-END→viridis recolour, a break-edit re-contour of the cached grid (trace count advances) with ZERO network egress / no re-solve (SC3), and that the legend break labels ≡ the contour breaks ≡ the class colours.

## Authentication Gates

None.

## Checkpoint — A4 EU-END default (pre-approved)

The plan's `checkpoint:human-verify` (gate="blocking") for the EU-END default break edges + palette hex was **PRE-APPROVED by the orchestrator on the user's standing "do not stop" mandate**. Approved values shipped verbatim in `web/src/store/colorScale.ts`:
- **Break edges:** `[55, 60, 65, 70, 75]` dB — the canonical ~5 dB EU-END reporting bands, source **Directive 2002/49/EC (Environmental Noise Directive) Annex VI** Lden reporting scheme.
- **Palette (6 classes):** `#7fbf7f` (< 55, green) · `#bfdc6b` · `#f6e04b` · `#f4a637` · `#e34948` · `#8a2be2` (≥ 75, violet) — the dataviz-validated EU-END sequential ramp (11-UI-SPEC §Data Visualization Palettes #1).

The A4 assumption is carried as an explicit code comment. No correction was supplied; the defaults shipped as approved.

## Task Commits

1. `592b906` feat(11-06): colour-scale store — single breaks/colors source of truth + presets (D-03/04/19)
2. `e4674cb` feat(11-06): trace_isophones WASM boundary — live re-contour, no re-solve (SC3/D-04)
3. `a549b57` feat(11-06): isophone fill layer + colour-scale editor (D-02/04/18, re-contour no re-solve)
4. `77fa0d7` test(11-06): offline Playwright isophone UAT (WEB-06/GRID-04, SC3)

_Plan metadata commit follows this SUMMARY._

## Deviations from Plan

### Auto-fixed / structural (Rule 2/3)

**1. [Rule 3 — blocking] New `trace_isophones` WASM boundary (the live re-contour path did not exist).**
- **Found during:** Task 2. The plan assumed the layer would "re-run the WASM tracer over the cached grid", but the only grid→polygon WASM export was `export` (GeoJson) — a download-artifact path with a mandatory identity/attribution footer, and the current committed dev glue did not expose even that. A live layer needs a lightweight tracer callable per break edit.
- **Fix:** Added a thin `trace_isophones` boundary returning a GeoJSON string (no new result DTO), factored the GeoJSON export arm's tracing into a shared `trace_bands_lonlat` core (removes duplication), added `TraceIsophonesReq` (ts-rs no-drift green), and rebuilt the threaded compute WASM. Engine 3-dep quarantine untouched.
- **Files:** crates/envi-compute-wasm/src/{dto.rs,export.rs}, crates/envi-service/tests/wire_no_drift.rs, web/src/generated/wire.ts.
- **Committed in:** `e4674cb`.

**2. [Rule 2 — missing critical support] Mount `IsophoneLayer` + `MapLegend` in MapCanvas.tsx; token-only legend/editor CSS; testBridge `seedIsophone`.**
- **Issue:** The fill layer/legend must mount inside `<Map>` (MapCanvas is the only site), the panel/legend need styling per the UI-SPEC, and the offline UAT needs a way to seed a fixture grid + read telemetry. None of MapCanvas.tsx / app.css / testBridge.ts were in the declared file list.
- **Fix:** Added `<IsophoneLayer/>` + `<MapLegend/>` to MapCanvas (below the scene overlays, D-18), a token-only `.isophone-legend`/`.colorscale-*` CSS block, and DEV-only `seedIsophone`/`isophoneTelemetry`/`colorScaleState` bridge helpers. All additive; no existing behaviour changed.
- **Files:** web/src/map/MapCanvas.tsx, web/src/app.css, web/src/testBridge.ts.
- **Committed in:** `a549b57`.

**3. [Rule 3 — lockfile sync] Cargo.lock `envi-geo` edge for envi-compute-wasm.**
- **Issue:** 11-04 added `envi-geo` to `envi-compute-wasm` but left the Cargo.lock edge uncommitted (the 11-02 `ndarray`-edge precedent). The wasm rebuild materialised it.
- **Fix:** Committed the lock edge — a workspace crate, **no new-to-repo package**.
- **Committed in:** `a549b57`.

**Total deviations:** 3 (1 blocking new boundary, 1 missing support, 1 lockfile sync). No engine-core or frozen-wire regressions; `cargo tree -p envi-engine` unchanged.

## Threat Model Coverage

- **T-11-06-01 (Tampering, break input):** `validateBreaks` (TS, inline `.form-error`) AND the WASM tracer's V5 (`trace_isophones` → `trace_isobands` typed error, native test `trace_isophones_rejects_an_invalid_break_scale_without_panicking`) — never a panic on user input.
- **T-11-06-02 (Integrity, rendering):** the single `breaks[]`/`colors[]` source of truth drives tracer + fill + legend; a `fill`-only layer with the `grep -c heatmap == 0` gate (0) — no raster density layer (D-02/D-04).
- **T-11-06-03 (Availability, re-contour):** the re-contour path is a pure main-thread WASM tracer call over the cached grid; the offline UAT asserts a break edit advances the trace count with ZERO network egress / no calc-worker solve (SC3).

## Verification

Rust gates at the workspace root; frontend gates in `web/`; on `main`.

- `cargo fmt --check` → **clean** (exit 0).
- `cargo clippy --all-targets -- -D warnings` → **clean** (whole workspace).
- `cargo test` → **all pass** (exit 0), incl. the new `envi-compute-wasm` export tests (`trace_isophones_returns_wgs84_bands_with_fills_for_the_live_layer`, `trace_isophones_rejects_an_invalid_break_scale_without_panicking`) and `wire_no_drift` (`wire_ts_matches_committed_source`).
- `cargo tree -p envi-engine` → `ndarray + num-complex + thiserror` — **unchanged** (engine 3-dep quarantine intact, D-02).
- `cd web && npx tsc --noEmit` → **clean** (exit 0).
- `cd web && npx vitest run` → **43 passed** (incl. the 13 new `colorScale` tests: END default breaks/hex, viridis/turbo sampling at N, V5 validation, single-source invariant).
- `cd web && npm run build:web` → **built** (dist rebuilt + committed with the `trace_isophones` glue).
- `cd web && npx playwright test isophone` → **1 passed** (offline, real bundle, real WASM tracer over a seeded grid).
- **D-02 grep gate:** `grep -v '^//' web/src/map/isophoneLayer.ts | grep -c heatmap` → **0**.

## Requirements

- **WEB-06** (isophone fill polygons + editable colour scale + legend) — **COMPLETE**: fill layer + END/viridis/turbo presets + editable breaks + docked legend, legend ≡ contour ≡ class.
- **GRID-04** (contour results into isophone fill polygons) — **COMPLETE**: the 11-02 tracer core is now shipped as a live MapLibre fill layer that re-contours the cached grid.

## Next Phase Readiness

- **11-09 (export UI)** reuses the same cached grid + `breaks[]`/`colors[]` for the GeoJSON export (`export` GeoJson arm already shares `trace_bands_lonlat`).
- **11-10 (scene-object restyle)** must keep the isophone fill below the styled objects — `IsophoneLayer` already inserts before the `SCENE_LAYER_PREFIXES` layers (best-effort until the full restyle lands).
- **Open follow-up:** the level grid is seeded via the DEV bridge for the UAT; wiring the production feed — reconstruct the fine-tier lattice into a `LevelGrid` from a finished readout and call `setIsophoneInput` — is the natural companion (mirrors 11-05's manifest feed follow-up). `reconstruct_level_grid` (11-02) has no WASM boundary yet; add one when the live feed lands.

## Self-Check: PASSED

- Created files exist on disk: `web/src/store/colorScale.ts`, `web/src/store/colorScale.test.ts`, `web/src/map/isophoneLayer.ts`, `web/tests/e2e/isophone.spec.ts`.
- Task commits present in `git log`: `592b906`, `e4674cb`, `a549b57`, `77fa0d7`.
- All gate commands re-run green (fmt/clippy/test/tree/tsc/vitest/build/playwright/grep above); wire no-drift green; engine quarantine unchanged.

---
*Phase: 11-results-fast-recalc*
*Completed: 2026-07-12*
