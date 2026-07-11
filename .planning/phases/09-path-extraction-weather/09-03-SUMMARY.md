---
phase: 09-path-extraction-weather
plan: 03
subsystem: gis
tags: [rust, metx, weather, obukhov, era5, open-meteo, lift, wasm-safe, nord2000, assumed-quarantine]

# Dependency graph
requires:
  - phase: 03-meteorology-refraction
    provides: "Route-2/3 [ASSUMED] weather model + the 3×3 Cramer fit_profile + WeatherComponents/profile_for_bearing/ReflectionProfiles (the pure math this plan lifts) + SoundSpeedProfile A/B/C target"
  - phase: 09-path-extraction-weather
    provides: "09-01/02 envi-gis GEOX modules + the GisError typed-error / committed-fixture-oracle conventions this plan extends"
  - phase: 08-gis-ingestion-dgm
    provides: "envi-gis pure-Rust WASM-safe boundary + committed-fixture (cog_window) provenance/SHA pattern"
provides:
  - "METX-01 (derivation): envi_gis::weather — the LIFTED single-source log-lin LSQ (fit_profile) + WeatherComponents/WeatherProfile/profile_for_bearing/ReflectionProfiles, now WASM-safe; components_from_levels (Open-Meteo multi-level → bearing-independent A/B/C via the Route-2 separable model) + levels_from_openmeteo (JSON parse, AMSL→AGL) + sound_speed_profile_for_azimuth (solver seam)"
  - "envi-harness now depends on + consumes the lift: weather/mod.rs re-exports the types, route3.rs delegates fit_profile (GisError→CaseLoadError) — no duplicate LSQ, all Phase-3 tests unchanged"
  - "METX-02 (derivation): envi_gis::era5 — obukhov (1/L from ERA5 single-level fields) + occurrence_stats (wind×stability class-occurrence table + sdfor reliability); occurrence statistics ONLY (D-05)"
  - "Committed fixtures: openmeteo_{archive,forecast}.json, era5_synthetic.toml (independent-recipe 1/L oracle); new GisError::{WeatherFit, Era5Field}"
affects: [09-04-wasm-service, 09-05-web-weather-panel, 10-solve-tensor, 11-results-isophones]

# Tech tracking
tech-stack:
  added: ["envi-gis (new envi-harness dependency — the weather LSQ lifted here; pure-Rust, WASM-safe, no cycle)"]
  patterns:
    - "Lift-not-fork: the validated Phase-3 3×3 LSQ moves into the WASM-safe crate ONCE; the harness re-exports the types and delegates fit_profile (error-mapping shim, not a copy) so the round-trip oracle still covers it and every Phase-3 call site/test is byte-unchanged"
    - "Well-conditioned [ASSUMED] separable weather model (a_temp=0, linear temperature gradient B/C, neutral-log-law a_wind) instead of a naive 3-param fit of both terms — real Open-Meteo pressure levels (sparse, ≥90 m AGL) make [ln(z),z,1] singular"
    - "AMSL geopotential → AGL via site-elevation subtract; near-surface 2m/10m anchor conditions + enriches the fit (Pitfall 5)"
    - "Independent-recipe committed fixture oracle: the era5 1/L expected value is a from-scratch reimplementation of the ECMWF recipe, not an echo of the module"
    - "[ASSUMED] quarantine carried from Phase 3: structural / direction / round-trip / sign / counting tests ONLY — never a false FORCE numeric weather pass"

key-files:
  created:
    - crates/envi-gis/src/weather.rs
    - crates/envi-gis/src/era5.rs
    - crates/envi-gis/tests/weather_openmeteo.rs
    - crates/envi-gis/tests/era5_occurrence.rs
    - crates/envi-gis/tests/fixtures/openmeteo_archive.json
    - crates/envi-gis/tests/fixtures/openmeteo_forecast.json
    - crates/envi-gis/tests/fixtures/era5_synthetic.toml
  modified:
    - crates/envi-gis/src/lib.rs
    - crates/envi-harness/src/weather/mod.rs
    - crates/envi-harness/src/weather/route3.rs
    - crates/envi-harness/Cargo.toml
    - Cargo.lock

key-decisions:
  - "LIFT the Phase-3 weather math into envi_gis::weather (single source of truth); harness re-exports types + delegates fit_profile via a GisError→CaseLoadError shim (NOT a re-export, because the error type changes) so all Phase-3 call sites/tests stay literally unchanged and there is no duplicate LSQ"
  - "components_from_levels uses the validated Route-2 [ASSUMED] SEPARABLE model (a_temp=0, linear temp gradient, neutral-log-law wind) rather than the research sketch's naive 3-param fit of both terms — the naive fit is singular over sparse ≥90 m AGL pressure levels; fit_linear (2-param) + fit_log_coeff (1-param) are well-conditioned and physically match route2. fit_profile (the lifted 3×3 LSQ) stays the single-source fit consumed by harness Route 3 + covered by the round-trip oracle"
  - "obukhov sign convention pinned to the documented behaviour (ECMWF downward-positive ⇒ daytime ishf<0 ⇒ 1/L<0 unstable): 1/L = −κ·g·θ*/(T_v·u*²) with θ*=−ishf/(ρ·c_p·u*) and u*=√(|τ|/ρ), T_v from Tetens vapour pressure. The research's literal recipe omitted the leading minus (would flip the sign vs its own stated behaviour) — the documented behaviour wins"
  - "occurrence_stats returns Result (non-finite/degenerate hour → typed error) and exposes counts[wind_bin][stability] + total + sdfor reliability; occurrence statistics ONLY — no class→A/B/C, no L_den (D-05, GRID-03)"

patterns-established:
  - "envi-harness → envi-gis dependency (no cycle: envi-gis never depends on envi-harness); the harness is now a consumer of the WASM-safe weather core"
  - "New GisError variants WeatherFit (fit under-determined/singular) + Era5Field (degenerate ERA5 state), both message-wrapped to keep PartialEq"

requirements-completed: []

# Metrics
duration: 40min
completed: 2026-07-11
status: complete
---

# Phase 9 Plan 03: Open-Meteo → per-azimuth A/B/C (lift) + ERA5 Obukhov/occurrence-stats (METX-01/02 derivation) Summary

**The weather half's pure math as WASM-safe `envi-gis` modules: the validated Phase-3 log-lin fit (3×3 Cramer LSQ + `WeatherComponents`/`profile_for_bearing`/`ReflectionProfiles`) is LIFTED into `envi-gis::weather` as the single source of truth — the harness now depends on and consumes it (re-export + a thin `fit_profile` delegator, no duplicate LSQ) — plus `components_from_levels` turning an Open-Meteo multi-level profile into per-azimuth A/B/C, and `envi-gis::era5` deriving the Obukhov length + a wind×stability class-occurrence table. All validated against committed fixtures with structural/sign/counting tests only — the `[ASSUMED]` weather-route constants stay quarantined, no false FORCE numeric pass.**

## Performance

- **Duration:** ~40 min
- **Completed:** 2026-07-11
- **Tasks:** 2 (both `type=auto tdd=true`)
- **Files modified:** 12 (7 created, 5 modified)

## Accomplishments

- **METX-01 lift (single source of truth).** `fit_profile` (the 3×3 Cramer normal-equations LSQ), `WeatherComponents`, `WeatherProfile`, `profile_for_bearing`, `SubPathCollapse` and `ReflectionProfiles` were lifted **verbatim** out of `envi-harness/src/weather/{mod,route3}.rs` into the WASM-safe `envi_gis::weather` (they depend only on `envi_engine`). `envi-harness` now depends on `envi-gis`: `weather/mod.rs` `pub use`-re-exports the types and `route3.rs` **delegates** `fit_profile` (mapping `GisError` → `CaseLoadError`) so every Phase-3 call site and test compiles and passes **unchanged** — there is exactly one copy of the LSQ.
- **METX-01 derivation.** `components_from_levels(&[Level], phi_wind_deg, z0)` fits the bearing-independent `WeatherComponents` from an Open-Meteo multi-level profile using the validated Route-2 `[ASSUMED]` **separable** model: temperature → linear gradient (`B`, `C`) with `a_temp = 0`; wind projected onto the reference downwind bearing → neutral-log-law `a_wind`. `levels_from_openmeteo` parses the Archive/Forecast pressure-level JSON, converts **AMSL geopotential → AGL** (subtract site elevation, Pitfall 5), and prepends a near-surface 2 m/10 m anchor to condition the fit; `sound_speed_profile_for_azimuth` emits the engine `SoundSpeedProfile { a,b,c,s_a:0,s_b:0,z0 }` per path azimuth (the solver seam).
- **METX-02 derivation.** `envi_gis::era5::obukhov` computes the inverse Obukhov length `1/L` from ERA5 single-level fields (`iews`, `inss`, `ishf`, `2t`, `2d`, `sp`) via `u* = √(|τ|/ρ)`, `θ* = −ishf/(ρ·c_p·u*)`, `1/L = −κ·g·θ*/(T_v·u*²)` (`T_v` from Tetens vapour pressure), with the ECMWF downward-positive sign convention ⇒ daytime unstable / night stable. `occurrence_stats` bins each hour into a wind-speed × stability class-occurrence table with an `sdfor` reliability count — **occurrence statistics only** (D-05): no class → A/B/C, no `L_den`.
- **Quarantine + honesty.** Committed `openmeteo_{archive,forecast}.json` + `era5_synthetic.toml` fixtures (the ERA5 `1/L` oracle computed by an **independent** reimplementation of the recipe); tests assert direction/inversion/round-trip/sign/counting properties **only** — no FORCE numeric assertion. `envi-engine` still exactly 3 deps; `envi-gis` gains no async/network/browser edge; only `envi-harness` gains the (cycle-free) `envi-gis` dependency.

## Task Commits

Each task was committed atomically:

1. **Task 1: Lift Phase-3 fit into envi-gis::weather + Open-Meteo → per-azimuth A/B/C (METX-01)** — `cf52309` (feat)
2. **Task 2: ERA5 Obukhov + wind×stability occurrence statistics (METX-02)** — `bb06210` (feat)

_Both tasks are `tdd=true`; each was implemented with its unit + committed-fixture integration tests and committed atomically (single feat commit per task — MVP+TDD gate inactive this phase, matching the 09-01/02 precedent)._

## Files Created/Modified

- `crates/envi-gis/src/weather.rs` — the lifted `WeatherComponents`/`WeatherProfile`/`profile_for_bearing`/`ReflectionProfiles`/`fit_profile` + `components_from_levels`/`levels_from_openmeteo`/`sound_speed_profile_for_azimuth` + 11 unit tests
- `crates/envi-gis/src/era5.rs` — `obukhov`, `occurrence_stats`, `ClassOccurrence`, `Stability`, named `[ASSUMED]` bin-edge + physical consts + 5 unit tests
- `crates/envi-gis/tests/weather_openmeteo.rs` — committed-fixture METX-01 derivation test (5 tests)
- `crates/envi-gis/tests/era5_occurrence.rs` — committed-fixture METX-02 Obukhov/occurrence test (3 tests)
- `crates/envi-gis/tests/fixtures/{openmeteo_archive.json, openmeteo_forecast.json, era5_synthetic.toml}` — synthetic, real-shaped fixtures with provenance banners
- `crates/envi-gis/src/lib.rs` — `pub mod weather; pub mod era5;` + new `GisError::{WeatherFit, Era5Field}` variants
- `crates/envi-harness/src/weather/mod.rs` — re-exports the lifted types (local definitions removed)
- `crates/envi-harness/src/weather/route3.rs` — `fit_profile` now delegates to `envi_gis::weather::fit_profile` (LSQ body + `solve_3x3` removed); module doc updated
- `crates/envi-harness/Cargo.toml` — `envi-gis` dependency + rationale comment
- `Cargo.lock` — envi-gis → envi-harness edge

## Decisions Made

- **Lift-not-fork with a delegating shim.** The plan said "re-export"; a pure `pub use` can't change `fit_profile`'s error type (the lifted fn returns `GisError`, the harness API returns `CaseLoadError`). Keeping a **delegating shim** in `route3.rs` (map `GisError::NonFinite → CaseLoadError::NonFinite`, else `→ Invalid`) preserves the exact Phase-3 signature so every route3 test (`fit_profile_singular_is_typed_error`, `..._rejects_non_finite_samples`, round-trips) passes **literally unchanged**, while the LSQ math lives only in `envi-gis`. The shim is a 5-line error map, not a copy — no duplicate LSQ.
- **Separable [ASSUMED] model over a naive 3-param fit** (see Deviations, Rule 1). Real Open-Meteo pressure levels are sparse and all ≥ ~90 m AGL, so the `[ln(z/z₀+1), z, 1]` basis is ill-conditioned (empirically singular under the shared scale guard). The validated Route-2 model (`a_temp = 0`, linear temperature gradient, neutral-log-law wind) is well-conditioned for any height spread and is the physically-honest choice; `fit_profile` remains the single-source LSQ (harness Route 3 + round-trip oracle).
- **obukhov sign pinned to documented behaviour.** The research's literal recipe (`θ* = −ishf/…`, `1/L = +κg θ*/…`) contradicts its own stated behaviour (daytime unstable). Implemented `1/L = −κ·g·θ*/(T_v·u*²)` so daytime `ishf < 0` ⇒ `1/L < 0` (unstable) and night ⇒ stable — the physically-certain part the fixture test pins.

## Deviations from Plan

### Auto-fixed / refinements

**1. [Rule 1 — Bug] `components_from_levels` uses the Route-2 separable model, not a naive 3-param fit of both terms**
- **Found during:** Task 1 (the committed Open-Meteo fixtures failed to derive).
- **Issue:** The 09-RESEARCH code sketch fit `(a_temp, b, c)` and `a_wind` with the full 3×3 `fit_profile`. Over **real** Open-Meteo pressure levels (6 levels, all ≥ ~90 m AGL, up to 1450 m) the `[ln(z/z₀+1), z, 1]` design matrix is ill-conditioned — the log and linear columns are not separable there — and the shared singular guard rejects it (`WeatherFit: singular normal matrix`). A near-surface 2 m/10 m anchor widened the range but was still marginally singular for the 3-param fit.
- **Fix:** Fit temperature with a 2-param linear LSQ (`fit_linear` → `B`, `C`; `a_temp = 0`) and wind with a 1-param neutral-log-law LSQ (`fit_log_coeff` → `a_wind`) — the exact `[ASSUMED]` decomposition the **validated** harness Route 2 (`route2.rs`) already uses. Both are well-conditioned for any height spread. `fit_profile` (the lifted 3×3 LSQ) stays the single-source fit, consumed by harness Route 3 and covered by the round-trip oracle (unit test).
- **Files modified:** crates/envi-gis/src/weather.rs
- **Verification:** `archive_fixture_derives_direction_and_inversion_properties` + `forecast_…` green (downwind A > upwind A, inversion ⇒ B > 0, lapse ⇒ B < 0, round-trip identity, crosswind ⇒ A = a_temp).
- **Committed in:** cf52309 (Task 1)

**2. [Rule 2 — Missing Critical] Near-surface AGL anchor + `NEAR_SURFACE_HEIGHT_M` const**
- **Found during:** Task 1.
- **Issue:** Pressure levels alone start at ~90 m AGL; the profile needs a low sample both for conditioning and physical fidelity (the sound-relevant layer is near-ground). 09-RESEARCH Pattern 5 explicitly lists `temperature_2m` / `wind_speed_10m` in the request.
- **Fix:** `levels_from_openmeteo` reads the near-surface AGL variables and prepends a 10 m anchor (best-effort; absent ⇒ pressure levels only). Named `[ASSUMED]` const with rationale.
- **Files modified:** crates/envi-gis/src/weather.rs + fixtures
- **Committed in:** cf52309 (Task 1)

**3. [Rule 1 — Bug] `obukhov` sign correction vs the research's literal recipe**
- **Found during:** Task 2.
- **Issue:** The research recipe as written (`1/L = +κ·g·θ*/(T_v·u*²)` with `θ* = −ishf/…`) yields daytime **stable**, contradicting its own documented "daytime unstable" behaviour.
- **Fix:** `1/L = −κ·g·θ*/(T_v·u*²)` so the ECMWF downward-positive convention gives daytime `ishf < 0` ⇒ `1/L < 0` (unstable). Documented in the module header.
- **Files modified:** crates/envi-gis/src/era5.rs
- **Verification:** `daytime_unstable_night_stable_sign` + the fixture sign tests green.
- **Committed in:** bb06210 (Task 2)

**Total deviations:** 3 (all correctness-necessary refinements). **Impact:** No scope creep — the WASM boundary/service endpoint (09-04), web panel (09-05) and Playwright journey (09-06) are untouched. All prohibitions honored: no false FORCE numeric pass (structural/sign/counting/round-trip only); no forked 3×3 LSQ (lifted once, harness consumes); geopotential AMSL→AGL applied; no reqwest/tokio/web-sys in envi-gis; ERA5 output is occurrence statistics only (no class→A/B/C, no L_den); no panic on non-finite weather/ERA5 fields (typed errors); engine still exactly 3 deps.

## Issues Encountered

- **Windows file lock on `envi-service.exe`** during the full `cargo test --workspace`: a user-launched server holds the running binary, so cargo cannot replace its test executable (`Access is denied (os error 5)`). Per project hygiene rules the process was **NOT** killed. Verified green via `cargo test --workspace --exclude envi-service` (all 39 test blocks OK, 0 failed) plus `cargo clippy --all-targets -- -D warnings` (which compiled `envi-service` clean) and `cargo fmt --check`. `envi-service` does not consume the new `weather`/`era5` modules this plan (its flagged ERA5 endpoint is 09-04), so it is unaffected.

## Quality Gates

- `cargo test -p envi-gis`: 96 lib + 8 (cog) + 1 (profile_oracle) + 5 (weather_openmeteo) + 3 (era5_occurrence) green (weather: 11 unit; era5: 5 unit).
- `cargo test -p envi-harness weather`: 34 passed (the delegated `fit_profile` + re-exported types — all Phase-3 weather tests unchanged) + `oracle_refraction` green.
- `cargo test --workspace --exclude envi-service`: 39 test blocks OK, 0 failed (envi-service excluded only by the running-binary file lock; compiled clean under clippy).
- `cargo clippy --all-targets -- -D warnings`: clean (whole workspace, incl. envi-service).
- `cargo fmt --check`: clean.
- `cargo tree -p envi-engine`: exactly `ndarray + num-complex + thiserror` (quarantine byte-identical).
- `cargo tree -p envi-gis`: no HTTP client / async runtime / browser crate.

## Next Phase Readiness

- The weather derivation is complete and WASM-safe: `envi_gis::weather::{components_from_levels, sound_speed_profile_for_azimuth}` produce the per-azimuth `SoundSpeedProfile` a real `SolveJob` consumes, and `envi_gis::era5::{obukhov, occurrence_stats}` produce the METX-02 occurrence table. 09-04 can add the WASM boundary DTOs/shims over these pure functions and the flagged-off ERA5/CDS service endpoint; 09-05 the web weather-import panel + OPFS cache; 09-06 the offline Playwright zero-egress proof.
- METX-01/METX-02 remain **Pending** in REQUIREMENTS.md — this plan delivers only the derivation half; the full requirements (browser fetch + OPFS cache + zero-egress what-if for METX-01; the WASM/service transport for METX-02) land in 09-04/05/06.
- The `[ASSUMED]` quarantine is intact and documented in both modules; the reviewer/`/gsd-secure` should confirm no numeric weather Pass was introduced.

## Self-Check: PASSED

All claimed artifacts verified on disk (`crates/envi-gis/src/{weather,era5}.rs`, both integration tests, all three fixtures) and both task commits (`cf52309`, `bb06210`) present in git history; the new modules + `GisError` variants compile and all new tests pass.

---
*Phase: 09-path-extraction-weather*
*Completed: 2026-07-11*
