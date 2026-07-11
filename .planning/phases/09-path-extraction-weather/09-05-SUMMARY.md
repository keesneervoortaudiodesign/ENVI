---
phase: 09-path-extraction-weather
plan: 05
subsystem: ui
tags: [react, typescript, zustand, maplibre, open-meteo, opfs, wasm, weather, overlays, ts-rs]

# Dependency graph
requires:
  - phase: 09-path-extraction-weather
    provides: "09-04 WASM boundary — derive_weather/extract_cut_profile/segment_cut_profile/inject_screen_edges/build_receiver_grid shims + generated wire.ts DTOs"
  - phase: 08-gis-ingestion-dgm
    provides: "web/ frontend (ImportPanel/store/overlay analogs), opfs.ts safeSeg cache, fetchers.ts direct-vs-proxy pattern, ApiError (api/client)"
  - phase: 04-solver-tensor
    provides: "SoundSpeedProfile (A/B/C) the per-azimuth readout mirrors"
provides:
  - "Browser weather-import journey (METX-01 UI half): WeatherPanel date+hour+z0 → date-switched Open-Meteo fetch → OPFS cache keyed (lat,lon,timestamp) → WASM derive_weather → per-azimuth A/B/C + call-cost log; what-if reads OPFS only (zero API calls, SC4)"
  - "web/src/import/weather.ts (fetchWeather/deriveAbc), opfs.ts getWeather/putWeather, store/weather.ts (zustand state machine), WeatherPanel.tsx, weatherOverlay.ts (receiver-grid/impedance/screen debug overlays), sceneDebug.ts (geometry-shim marshaller)"
  - "web/src/import/wasm.ts typed facades for all 6 Phase-9 geometry/weather shims; rebuilt committed web/dist"
affects: [09-06-playwright, 10-solve-tensor, 11-results-isophones]

# Tech tracking
tech-stack:
  added:
    - "No new npm deps — reuses react/zustand/maplibre/react-map-gl already in web/; the WASM bundle is rebuilt from the Phase-9 Rust shims"
  patterns:
    - "Browser fetch → OPFS cache → WASM math: TS fetches + caches + marshals; all acoustic A/B/C stays in envi-gis WASM (derive_weather) — TypeScript never does acoustic arithmetic"
    - "Date-switched product selection (Archive vs Forecast) by requested date; hosts hard-coded (SSRF); direct CORS first, same-origin byte-proxy fallback"
    - "Debug-overlay geometry: run the real geometry shims over the scene in a LOCAL equirectangular debug frame; render shim outputs, honest notes for missing inputs, never fabricated geometry"
    - "Imperative MapLibre debug overlay (source/layer, style.load rebuild, toggle visibility, effect teardown) shared by three toggleable layers"

key-files:
  created:
    - web/src/import/weather.ts
    - web/src/store/weather.ts
    - web/src/panels/WeatherPanel.tsx
    - web/src/map/weatherOverlay.ts
    - web/src/import/sceneDebug.ts
    - .planning/phases/09-path-extraction-weather/09-05-SUMMARY.md
  modified:
    - web/src/import/opfs.ts
    - web/src/import/wasm.ts
    - web/src/App.tsx
    - web/src/map/MapCanvas.tsx
    - web/dist/

key-decisions:
  - "Site (lat,lon) = the current map-viewport centre (import store viewport). It is always available and the honest 'weather at this location' proxy for the single-hour import; the real per-receiver site binding is Phase-10 solve territory."
  - "Display azimuths = the 8 fixed compass sectors (0/45/…/315). The real per-path fan-out is Phase 10; 8 sectors make downwind>upwind A visible in the UI half without inventing a path set."
  - "phi_wind_deg (the WASM shim's required reference downwind bearing) is read from the surface wind DIRECTION field + the +180° downwind convention — a data extraction, NOT acoustic/A-B-C arithmetic (which stays wholly in WASM). Falls back to 1000 hPa dir, then 0°, so the fit never depends on a fabricated value."
  - "Debug overlays run the real geometry shims over the scene in a LOCAL equirectangular frame (debug-only, NOT the GEOX-04 solve boundary) so meter-based spacing/heights stay meaningful; roughness_m=0 for the segmentation drawn-zones is a documented debug simplification (σ class boundaries are roughness-independent)."
  - "fetchWeather takes projectId as its first argument (the OPFS cache is per-project); the plan's (lat,lon,timestamp) key is preserved as the cache key."

patterns-established:
  - "getWeather/putWeather: OPFS text cache under projects/<uuid>/cache/weather/<key> reusing safeSeg — path-traversal-safe key, cache-hit = zero network (SC4)"
  - "wasm.ts typed facade: one wire-typed wrapper per #[wasm_bindgen] shim, the single audited any→DTO cast site + wasm-trap recovery"

requirements-completed: [METX-01]

# Metrics
duration: 50min
completed: 2026-07-11
status: complete
---

# Phase 9 Plan 05: Web Weather Panel + Debug Overlays Summary

**The browser-side METX-01 journey: a WeatherPanel (date+hour+z₀ → date-switched Open-Meteo Archive/Forecast fetch → OPFS cache keyed (lat,lon,timestamp) → WASM `derive_weather` → per-azimuth A/B/C + call-cost log, with what-if reads served from OPFS at zero API cost) plus receiver-grid / impedance-segmentation / screen-vertex debug overlays driven by the real `envi-gis` geometry shims — no acoustic math in TypeScript.**

## Performance

- **Duration:** ~50 min
- **Completed:** 2026-07-11
- **Tasks:** 2 (both `type=auto`)
- **Files modified:** 10 (5 created, 4 modified, + rebuilt `web/dist`)

## Accomplishments

- **Task 1 — weather fetch + OPFS cache + WASM deriveAbc.** `web/src/import/weather.ts`: `fetchWeather(projectId, lat, lon, date, hour)` date-switches the endpoint (recent/near-future → `api.open-meteo.com/v1/forecast`, historical → `archive-api.open-meteo.com/v1/archive`, hosts hard-coded — SSRF T-09-05-03), requests the 27-variable pressure-level + height-AGL set (`wind_speed_unit=ms`, `timezone=UTC`), direct-CORS first with a same-origin byte-proxy fallback, and throws the existing `ApiError` (never redeclared) on non-2xx. It reads OPFS FIRST — a cache hit returns the cached body with **zero network calls** (SC4) — and logs an estimated call-cost weight (`n_vars × 24 / 100`) per network fetch. `deriveAbc` marshals the cached JSON into the `derive_weather` WASM shim (no acoustic arithmetic in TS). `opfs.ts` gained `getWeather`/`putWeather` over `projects/<uuid>/cache/weather/<key>` reusing `safeSeg` (path-traversal-safe, T-09-05-02). `wasm.ts` gained typed facades for all 6 Phase-9 shims.
- **Task 2 — WeatherPanel + store + debug overlays.** `store/weather.ts`: a zustand state machine (idle→fetching→cached→derived/error) holding the single-hour selection, per-azimuth A/B/C, `fromCache`/call-cost visibility, error, and the three debug toggles + computed geometry; `runWeatherImport` orchestrates fetch→OPFS→WASM, `runDebugGeometry` runs the geometry shims. `WeatherPanel.tsx`: a date+hour+z₀ picker → Import → per-azimuth A/B/C table, every control `data-testid`'d and every fetched/derived string a React text child (never `innerHTML`, T-09-05-01). `sceneDebug.ts`: runs the real `extract_cut_profile → segment_cut_profile → inject_screen_edges` and `build_receiver_grid` shims over the scene in a local debug frame, returning WGS84 geometry + honest notes. `weatherOverlay.ts`: receiver-grid (green points), impedance-segmentation (σ soft→hard ramp), and screen-vertex (amber) MapLibre overlays, each toggled from the panel. Wired the panel into the right rail and the three overlays into `MapCanvas`; rebuilt the committed `web/dist`.

## Task Commits

Each task was committed atomically:

1. **Task 1: date-switched Open-Meteo fetch + OPFS weather cache + WASM deriveAbc** — `7270bd7` (feat)
2. **Task 2: WeatherPanel + weather store + receiver-grid/impedance/screen debug overlays** — `ae45b66` (feat)

## Files Created/Modified

- `web/src/import/weather.ts` (created) — `fetchWeather`/`deriveAbc`, date-switch, direct→proxy fetch, OPFS cache, call-cost log, `downwindBearingDeg` (data read, not acoustic math).
- `web/src/store/weather.ts` (created) — zustand weather store (state machine + per-azimuth A/B/C + debug toggles/geometry + `runWeatherImport`/`runDebugGeometry`).
- `web/src/panels/WeatherPanel.tsx` (created) — date+hour+z₀ picker + Import + per-azimuth A/B/C table + call-cost + debug toggles (all `data-testid`'d, no innerHTML).
- `web/src/map/weatherOverlay.ts` (created) — `ReceiverGridOverlay`/`ImpedanceSegOverlay`/`ScreenVertexOverlay` + shared `useDebugLayer` controller + σ colour ramp.
- `web/src/import/sceneDebug.ts` (created) — `computeDebugGeometry`: local-frame marshaller running the geometry shims over the scene, honest `notes`.
- `web/src/import/opfs.ts` (modified) — `getWeather`/`putWeather` (weather text cache, `safeSeg`).
- `web/src/import/wasm.ts` (modified) — typed facades for the 6 Phase-9 geometry/weather shims.
- `web/src/App.tsx` (modified) — `<WeatherPanel />` in the right rail.
- `web/src/map/MapCanvas.tsx` (modified) — the three debug overlays as `<Map>` children.
- `web/dist/` (rebuilt) — the served bundle (freshly built WASM + new panel/overlays; zero external assets).

## Decisions Made

- **Site = viewport centre; display azimuths = 8 compass sectors; phi_wind read from surface wind direction.** See frontmatter `key-decisions` — each is the lean, honest UI-half choice that defers the real per-receiver/per-path binding to Phase 10 while making downwind>upwind A visible now.
- **Debug overlays run the real shims in a local equirectangular frame** (debug-only, not the GEOX-04 solve boundary), rendering genuine shim outputs with honest notes for missing scene inputs — never fabricated geometry.

## Deviations from Plan

### Auto-fixed / refinements

**1. [Rule 3 — Blocking] `fetchWeather` takes `projectId` (the OPFS cache is per-project)**
- **Found during:** Task 1.
- **Issue:** The plan's `fetchWeather(lat, lon, timestamp)` cannot address the per-project OPFS cache (`projects/<uuid>/cache/...`).
- **Fix:** Signature is `fetchWeather(projectId, lat, lon, date, hour)`; the `(lat, lon, timestamp)` cache KEY is preserved exactly (D-01).
- **Files modified:** web/src/import/weather.ts.
- **Verification:** `npx tsc --noEmit` clean; the panel passes `projectId` from the scene store.
- **Committed in:** `7270bd7` (Task 1).

**2. [Rule 2 — Missing Critical] `web/src/import/sceneDebug.ts` added (geometry-shim marshaller)**
- **Found during:** Task 2 (debug overlays).
- **Issue:** The overlays must feed the geometry shims real scene coordinates (calc_area/footprints/source/receiver/DGM) in a meter frame; putting that marshalling in the overlay/panel would bloat them and mix concerns.
- **Fix:** A dedicated `sceneDebug.ts` extracts + projects scene geometry into a local debug frame, calls the shims, and converts outputs back to WGS84 — a thin marshaller, all math in WASM.
- **Files modified:** web/src/import/sceneDebug.ts (new).
- **Verification:** `npx tsc --noEmit` clean; `npm run build` succeeds.
- **Committed in:** `ae45b66` (Task 2).

**3. [Rule 2 — Missing Critical] `wind_speed_unit=ms` on the Open-Meteo request**
- **Found during:** Task 1.
- **Issue:** Open-Meteo defaults wind speed to km/h, but the shim reads `wind_speed_ms` — a silent unit bug (wind ≈ 3.6× too large → wrong A).
- **Fix:** The request pins `wind_speed_unit=ms` (and `timezone=UTC` so `hour_index === hour`).
- **Files modified:** web/src/import/weather.ts.
- **Verification:** URL construction unit-obvious; wire type unchanged.
- **Committed in:** `7270bd7` (Task 1).

---

**Total deviations:** 3 (1 blocking signature fix, 2 missing-critical). **Impact:** No scope creep. All prohibitions honored — no acoustic/A-B-C math in TS (the fit is wholly in `derive_weather`); no Open-Meteo call on a cache hit (OPFS-first, SC4); no `innerHTML` (every string a React text child); the cache key is strictly `(lat,lon,timestamp)` via `safeSeg` (no path traversal); `ApiError` reused, never redeclared; hosts hard-coded (SSRF); wire types are the generated `wire.ts` (no hand-authored TS mirror — no-drift green).

## Issues Encountered

- **The generated WASM bundle (`web/src/generated/wasm/*`) is gitignored** and did not yet expose the six Phase-9 shims (09-04 was Rust-only). Rebuilt it with `npm run build:wasm` (wasm32 + wasm-bindgen 0.2.126, both present) so the new exports exist; the bundle ships inside the tracked `web/dist`, so no separate commit of the glue is needed.
- **Windows file lock on `envi-service.exe`** (the user-launched server). Per the project hygiene HARD RULE the process was NOT killed. The ts-rs no-drift check (`wire_no_drift`) was run under an isolated `CARGO_TARGET_DIR=target-w905` (removed afterward) — `wire_ts_matches_committed_source` is green, confirming the committed `wire.ts` has no drift.

## Quality Gates

- `cd web && npx tsc --noEmit` — clean (the web typecheck; no eslint config in this repo, so tsc is the lint gate).
- `cd web && npm run build` — succeeds; `web/dist` rebuilt (WASM 1.43 MB bundled, zero external assets).
- `cargo test -p envi-service --test wire_no_drift` (isolated target dir) — `wire_ts_matches_committed_source` + `job_status_is_a_discriminated_union` green; committed `wire.ts` unchanged, no hand-authored TS mirror.
- Playwright (`09-06`) is the SC4 zero-egress-on-what-if proof and was intentionally NOT run here (out of this plan's scope, per the environment note).

## Next Phase Readiness

- **09-06 (Playwright)** builds directly on this: the panel controls are all `data-testid`'d, the two Open-Meteo hosts are the mock targets, and the OPFS-first path is the zero-egress-on-what-if journey to assert (`fromCache` + zero new host matches + the call-cost line).
- **Phase 10 (solve/tensor)** consumes the same geometry shims (now browser-reachable and exercised by the debug overlays) plus `PropagationPathInputs`/`PathCacheKey` (09-04) to assemble `SolveJob`s; the site/azimuth/phi_wind bindings this UI half fixes pragmatically become real per-receiver/per-path bindings there.

## Self-Check: PASSED

All five created source files verified on disk (`weather.ts`, `store/weather.ts`, `WeatherPanel.tsx`, `weatherOverlay.ts`, `sceneDebug.ts`) plus the two modified (`opfs.ts`, `wasm.ts`); both task commits (`7270bd7`, `ae45b66`) present in git history; `web/dist` tracked + rebuilt.

---
*Phase: 09-path-extraction-weather*
*Completed: 2026-07-11*
