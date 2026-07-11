---
phase: 09-path-extraction-weather
plan: 06
subsystem: test
tags: [playwright, e2e, offline, open-meteo, opfs, weather, sc4, metx-01, zero-egress]

# Dependency graph
requires:
  - phase: 09-path-extraction-weather
    provides: "09-05 WeatherPanel (date+hour+z₀ → date-switched Open-Meteo → OPFS cache → WASM derive_weather → per-azimuth A/B/C + call-cost log); all controls data-testid'd; OPFS-first fetchWeather (cache hit = zero network)"
  - phase: 08-gis-ingestion-dgm
    provides: "web/ offline Playwright harness: bootOffline (guard-first offline stack), _mocks.ts route installers, the DATA-04 record+route.abort zero-egress replay pattern, window.__enviTest DEV bridge (openProject)"
provides:
  - "web/tests/e2e/weather-import.spec.ts — the automated, credential-free, offline METX-01/SC4 proof: an online import derives per-azimuth A/B/C + logs the call-cost, then a what-if edit (change z₀ → re-Import) issues ZERO Open-Meteo calls (egress collector []), mirroring the Phase-8 DATA-04 replay"
  - "installWeatherMocks + committed openmeteo_{forecast,archive}.json fixtures — both date-switched Open-Meteo hosts mocked by host for the real bundle, fully offline"
affects: [10-solve-tensor, 11-results-isophones]

# Tech tracking
tech-stack:
  added:
    - "No new deps — reuses @playwright/test (devDependency only, never bundled) + the existing bootOffline/_mocks offline harness"
  patterns:
    - "Host-anchored Open-Meteo mocks: `//api.open-meteo.com/` matched WITH the leading `//` so it does not also swallow `archive-api.open-meteo.com` (whose `api` is preceded by `-`) — two distinct date-switched products, one fixture each"
    - "SC4 zero-egress proof = the DATA-04 inversion applied to weather: after a cached import, re-register both Open-Meteo hosts (+ the proxy fallback) to record+route.abort, then a what-if edit must leave the egress collector [] (OPFS-only read)"
    - "What-if = a z₀ change + re-Import: the weather body is served from OPFS (cache hit, zero network) while A/B/C is RE-DERIVED in WASM at the new z₀ — asserted by the downwind A changing across the what-if (a real re-compute, not a replayed chip)"

key-files:
  created:
    - web/tests/e2e/weather-import.spec.ts
    - web/tests/e2e/fixtures/openmeteo_forecast.json
    - web/tests/e2e/fixtures/openmeteo_archive.json
    - .planning/phases/09-path-extraction-weather/09-06-SUMMARY.md
  modified:
    - web/tests/e2e/_mocks.ts

key-decisions:
  - "The 'what-if edit' is a z₀ (roughness) change + re-Import, not a path/receiver edit: the WeatherPanel's display azimuths are a fixed set (09-05 D), so z₀ is the panel's genuine what-if knob that re-derives A/B/C while reading the SAME (site,date,hour) weather body from OPFS — the exact SC4 property (D-03: what-if reads OPFS only, zero API calls)."
  - "The spec drives the Vite DEV-served real bundle (the playwright.config webServer runs `npm run dev`), so the DEV `window.__enviTest` bridge exists for openProject and the CURRENT WeatherPanel source is served — no web/dist rebuild is required for the test (the dev server serves src live)."
  - "Fixtures are synthetic (no copyrighted weather data): two committed openmeteo_{forecast,archive}.json with a full 24-hour hourly block (any hour_index resolves) and distinguishable values (different elevation/wind/temperature) so the two date-switch branches are provably distinct. Generated deterministically; committed as the mock source of truth."
  - "Site (lat/lon) comes from the map viewport (written on map load); an ARMED Import button is the readiness signal (project open + viewport known), waited on before driving the picker."

patterns-established:
  - "installWeatherMocks(page): host-keyed page.route fulfilling the committed Open-Meteo fixtures, per-product counters (forecast/archive) so a test asserts the correct date-switched product was hit"
  - "Offline weather journey: bootOffline (unmocked collector must end []) + installWeatherMocks, drive the real panel by data-testid, assert derived A/B/C + call-cost, then invert the network for the what-if zero-egress assertion"

requirements-completed: [METX-01]

# Metrics
duration: 12min
completed: 2026-07-11
status: complete
---

# Phase 9 Plan 06: Offline Playwright Weather-Import + SC4 Zero-Egress Proof Summary

**The automated, credential-free, fully-offline METX-01/SC4 proof: a Playwright journey drives the REAL WeatherPanel (by `data-testid`) over committed Open-Meteo fixtures — an online import fetches the date-switched product, derives the per-azimuth A/B/C shown in the panel, and logs the call-cost weight; then the network is inverted (both Open-Meteo hosts record+abort) and a what-if edit (change z₀ → re-Import) re-derives A/B/C from OPFS with ZERO Open-Meteo calls (egress collector `[]`), mirroring the Phase-8 DATA-04 replay. Both date-switch branches (Forecast recent-date + Archive/ERA5 historical-date) are exercised; the offline suite is 19/19 green.**

## Performance

- **Duration:** ~12 min
- **Completed:** 2026-07-11
- **Tasks:** 1 (`type=auto`)
- **Files:** 4 (3 created, 1 modified)

## Test Result (actual run)

- **`npx playwright test weather-import`** — **2 passed** (2/2), ~7.6 s, fully offline, no credentials.
- **`npx playwright test`** (full offline suite) — **19 passed** (19/19), ~47 s. The two new weather-import tests plus every prior offline spec stay green.
- Drives the REAL Vite-served bundle; the only network intercepted is the offline guard + basemap + `/api` + the two Open-Meteo hosts (all via `page.route`); the unmocked-egress collector ends `[]` in every test.

## Accomplishments

- **`web/tests/e2e/weather-import.spec.ts` (new).** Two offline tests:
  1. **METX-01/SC4 Forecast journey + zero-egress what-if.** `bootOffline(page)` (unmocked collector must end `[]`) + `installWeatherMocks(page)`; open a project through `window.__enviTest.openProject`; wait for the `weather-import` button to ARM (site from the map viewport). Drive the `weather-date`/`weather-hour`/Import controls → assert the Forecast product was hit exactly once (the Archive host never), the `weather-abc-*` per-azimuth A/B/C rows render, the cache chip reads `network fetch`, the `weather-callcost` line is visible, and the `[weather] Open-Meteo fetch call-cost weight …` log fired once. Then the **SC4 inversion**: re-register both Open-Meteo hosts (`//api.open-meteo.com/`, `archive-api.open-meteo.com/`) and the `/api/v1/proxy/openmeteo-` fallback to **record + `route.abort()`**; perform the what-if (change `weather-z0` → re-Import) → assert the cache chip flips to `OPFS cache (no call)`, the Open-Meteo egress collector stays `[]`, the product counters are unchanged, no new call-cost line was logged, and the downwind (az 90) `A` **changed** across the what-if (a real WASM re-derivation on the cached body, not a replayed chip).
  2. **Archive date-switch branch.** A historical date (`2024-01-15`) hits the Archive host exactly once, renders per-azimuth A/B/C + the call-cost line, offline.
- **`web/tests/e2e/_mocks.ts` (modified).** Added `installWeatherMocks` + the `WeatherMockControl` counter interface: host-keyed `page.route` fulfilling the committed `openmeteo_{forecast,archive}.json` for the two hard-coded Open-Meteo products. The Forecast route is matched WITH its leading `//` so it does not also swallow the Archive host.
- **`web/tests/e2e/fixtures/openmeteo_{forecast,archive}.json` (new).** Two synthetic committed Open-Meteo pressure-level responses (6 levels + near-surface anchors, a full 24-hour hourly block) with distinguishable elevation/wind/temperature so the two date-switch branches are provably distinct. They are the exact bytes the real WASM `derive_weather` parse+fit path consumes.

## Deviations from Plan

**None material.** The plan's suggested what-if ("change a path azimuth / receiver") was realised as a **z₀ change + re-Import** — the WeatherPanel exposes the fixed 8-sector display azimuths (09-05 decision), so z₀ is the panel's genuine what-if knob; changing it re-derives A/B/C from the SAME OPFS-cached body, which is exactly the SC4 property under test (D-03: what-if reads OPFS only, zero API calls). This is a faithful realisation of the plan's intent on the shipped UI, not a scope change.

All prohibitions honoured: the unmocked collector ends `[]` (no live-network escape); the test drives the real built bundle (mocks only `/api` + the two third-party hosts); no credentials / no live Open-Meteo/CDS connection; Playwright stays a `devDependency` (never bundled by `vite build`); `test-results/` + `playwright-report/` remain git-ignored.

## Threat Coverage (from the plan's threat register)

- **T-09-06-01 (live-network escape masking a real fetch) — mitigated.** `bootOffline`'s guard-first stack records any unmocked external URL; both Open-Meteo hosts are mocked; every test asserts `unmocked.toEqual([])`.
- **T-09-06-02 (SC4 regression: what-if re-fetch) — mitigated.** After the cached import, both hosts + the proxy fallback are re-registered to record + `route.abort()`; the what-if leaves the egress collector `[]` and the product counters unchanged.
- **T-09-06-03 (test needs credentials / live API) — accepted by design.** Mocked hosts + committed synthetic fixtures; no credentials, no live CDS/Open-Meteo.

## Quality Gates

- `cd web && npx tsc --noEmit` — clean (the web typecheck; tsc is the lint gate in this repo).
- `cd web && npx playwright test weather-import` — 2/2 passed, offline.
- `cd web && npx playwright test` — 19/19 passed, offline.
- Playwright present only in `web/package.json` `devDependencies` (`@playwright/test ^1.61.1`); `.gitignore` covers `web/test-results/` + `web/playwright-report/`; the two fixtures + the spec are committable.

## Next Phase Readiness

- **METX-01 is now proven end-to-end and automatically:** the browser journey (09-05) + this offline SC4 proof close the requirement's "what-if edits issue zero API calls (verified by network log)" acceptance property in CI-less local runs.
- **Phase 9 close-out:** all six plans are executed. The five CLAUDE.md phase-completion gates (code-review · simplify · secure · verify · doc-consistency) remain to run before the phase is marked complete.
- **Phase 10 (solve/tensor)** consumes the same weather A/B/C seam (per-azimuth `SoundSpeedProfile`) and the geometry shims; the offline `installWeatherMocks` + fixtures are reusable when the solve path binds real per-receiver/per-path weather.

## Self-Check: PASSED

- Created files verified on disk: `web/tests/e2e/weather-import.spec.ts`, `web/tests/e2e/fixtures/openmeteo_forecast.json`, `web/tests/e2e/fixtures/openmeteo_archive.json`.
- Modified file verified: `web/tests/e2e/_mocks.ts` (contains `installWeatherMocks`).
- Task commit `22033b5` present in git history; the 2 weather-import tests + the full 19-test suite ran and passed (counts pasted above).

---
*Phase: 09-path-extraction-weather*
*Completed: 2026-07-11*
