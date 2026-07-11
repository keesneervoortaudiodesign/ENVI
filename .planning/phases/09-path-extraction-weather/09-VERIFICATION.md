---
phase: 09-path-extraction-weather
verified: 2026-07-11T19:05:00Z
status: passed
score: 5/5 must-haves verified
behavior_unverified: 0
overrides_applied: 0
requirements_covered: [GEOX-01, GEOX-02, GEOX-03, GRID-01, METX-01, METX-02]
---

# Phase 9: Path Extraction & Weather — Verification Report

**Phase Goal:** The GIS-to-engine geometry pipeline exists — cut profiles, impedance segmentation, screening edges, and receiver grids feed real-world `PropagationPath`s — and real weather flows in from Open-Meteo to drive the per-azimuth A/B/C meteorology.
**Verified:** 2026-07-11
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth (Success Criterion) | Status | Evidence |
|---|---------------------------|--------|----------|
| SC1 | DEM cut-profile matches GRASS `r.profile` oracle within stated tolerance on real DEM data | ✓ VERIFIED | `profile.rs:63` `cut_profile` (TIN barycentric sampler, typed `OutsideHull`, bounded, ground-z-only); `tests/profile_oracle.rs` passes against `fixtures/rprofile/` — an **independent raster-bilinear** walk (tol=0.05 m documented in CSV header, 63 rows, non-self-referential, non-zero-delta assertion). No GRASS/Python at test time. |
| SC2 | Impedance segmented drawn>imported>default; screening edges derived from building/wall/barrier via rstar corridor, injected as profile vertices | ✓ VERIFIED | `impedance.rs:89` `segment_ground` (per-interval midpoint resolution drawn>imported>default; σ resolved ONLY via `impedance_class`, class B=31.5; boundary crossings spliced). `screening.rs:166` `inject_screens` (rstar corridor query, bounded; thick screen = 2 tops + hard span class H; **no separate `screens` field** — vertices injected into `TerrainProfile`; ≤2-edge left to engine `terrain_interpretation`). All unit tests pass. |
| SC3 | Building-aware constrained-Delaunay receiver grid inside calc_area + discrete points, respecting footprints | ✓ VERIFIED | `grid.rs:73` `receiver_grid` (spade CDT region validation via `build_tin` with footprint rings + calc_area as constraint edges → typed `InvalidGridRegion`; footprints excluded as holes, D-07; discrete points appended; ground-z only; `MIN_SPACING_M`/`MAX_RECEIVERS` guardrails checked before allocation). Unit tests pass. |
| SC4 | Open-Meteo fetched once per (site,window), cached with project, per-azimuth A/B/C derived; what-if edits = zero API calls (network log); call cost logged | ✓ VERIFIED | `web/src/import/weather.ts` OPFS cache keyed strictly `(lat,lon,timestamp)`, cache hit returns `{fromCache:true, callCostWeight:undefined}` (zero network); call-cost logged per network fetch (`console.info … call-cost weight`). WASM `derive_weather` owns all A/B/C math. `weather-import.spec.ts` **inverts the network** (both hosts + proxy re-registered to record+abort) and asserts `omEgress.toEqual([])` on the what-if path, call-cost logged exactly once (not re-logged on cache hit), and A changes with z₀ (re-derivation genuinely ran). Both date-switch branches exercised. |
| SC5 | ERA5/CDS groundwork: async-job reanalysis retrieval + wind×stability occurrence stats (Obukhov); full L_den deferred with GRID-03 | ✓ VERIFIED | `era5.rs` `obukhov` + `occurrence_stats` — **occurrence statistics ONLY** (no class→A/B/C, no L_den, D-05); `[ASSUMED]` bin edges quarantined. `tests/era5_occurrence.rs` passes vs `era5_synthetic.toml` (**independent-recipe** ECMWF-Obukhov oracle, tol 1e-4). `envi-service/src/api/era5.rs` flagged endpoint (`--features era5` only) on the SC5 job state machine with SSRF chokepoint + env-only key; `contract_era5.rs::era5_route_absent_by_default` asserts route is absent in default build. |

**Score:** 5/5 truths verified (0 present-behavior-unverified)

### Required Artifacts

| Artifact | Provides | Status |
|----------|----------|--------|
| `crates/envi-gis/src/profile.rs` | GEOX-01 cut-profile extractor | ✓ VERIFIED |
| `crates/envi-gis/src/impedance.rs` | GEOX-02 impedance segmentation | ✓ VERIFIED |
| `crates/envi-gis/src/screening.rs` | GEOX-03 screening-edge injection | ✓ VERIFIED |
| `crates/envi-gis/src/grid.rs` | GRID-01 CDT receiver grid | ✓ VERIFIED |
| `crates/envi-gis/src/weather.rs` | METX-01 Open-Meteo → per-azimuth A/B/C fit | ✓ VERIFIED |
| `crates/envi-gis/src/era5.rs` | METX-02 Obukhov + occurrence stats | ✓ VERIFIED |
| `crates/envi-gis/src/path.rs` | Phase-9→10 PropagationPath assembly seam + L1 cache key | ✓ VERIFIED (public, 3 internal tests; solve deferred to Phase 10 by design) |
| `crates/envi-gis-wasm/src/lib.rs` | WASM boundary: extract_cut_profile / segment_cut_profile / inject_screen_edges / build_receiver_grid / derive_weather / derive_era5 | ✓ VERIFIED (all 6 exposed) |
| `crates/envi-service/src/api/era5.rs` | flagged ERA5/CDS retrieval endpoint | ✓ VERIFIED (feature-gated) |
| `web/src/import/weather.ts`, `store/weather.ts`, `panels/WeatherPanel.tsx` | weather import + OPFS cache + per-azimuth readout + call-cost | ✓ VERIFIED (store→fetchWeather→deriveAbc WASM chain wired) |

### Key Link Verification

| From | To | Via | Status |
|------|----|----|--------|
| `store/weather.ts` `runWeatherImport` | `import/weather.ts` `fetchWeather`/`deriveAbc` | OPFS cache + WASM `derive_weather` | ✓ WIRED |
| `import/weather.ts` `deriveWeather` | `envi-gis-wasm derive_weather` → `weather::components_from_levels` | serde DTO boundary | ✓ WIRED |
| `WeatherPanel.tsx` `AbcTable` | store `abc` (per-azimuth SoundSpeedProfile) | React text children (no innerHTML) | ✓ WIRED |
| `era5.rs` derivation | `envi-service era5::submit_era5_job` | Phase-6 JobStatus state machine | ✓ WIRED (feature era5) |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| GEOX-01/02/03 + METX-01/02 Rust suite | `cargo test -p envi-gis --tests` | profile_oracle 1/1, era5_occurrence 3/3, weather_openmeteo 7/7 pass | ✓ PASS |
| SC4 zero-egress what-if (network-inverted) | `weather-import.spec.ts` (established-context: 2/2 pass) | egress collector `[]`; call-cost logged once; A changes with z₀ | ✓ PASS |

### Requirements Coverage

| Requirement | Description | Status | Evidence |
|-------------|-------------|--------|----------|
| GEOX-01 | DEM cut-profile (r.profile oracle) | ✓ SATISFIED | SC1 |
| GEOX-02 | Impedance segmentation drawn>imported>default | ✓ SATISFIED | SC2 |
| GEOX-03 | Screening edges from building/wall/barrier | ✓ SATISFIED | SC2 |
| GRID-01 | Building-aware CDT receiver grid + discrete pts | ✓ SATISFIED | SC3 |
| METX-01 | Open-Meteo import, cached, what-if zero API | ✓ SATISFIED | SC4 |
| METX-02 | ERA5/CDS occurrence-statistics groundwork | ✓ SATISFIED | SC5 |

### Anti-Patterns Found

None. No `TODO`/`FIXME`/`XXX`/`TBD`/stub markers in phase source (only documented `future work`/`deferred to GRID-03`/`Phase 10/11` forward-references, which are legitimate scope boundaries). Absence handling is uniformly typed errors, never fabricated `0.0` (D-07): `OutsideHull`, `ProfileTooLong`, `UnresolvableClass`, `WeatherFit`, `Era5Field`, missing-`elevation` rejection.

### [ASSUMED] Quarantine (special-scrutiny)

Confirmed intact — no false FORCE numeric pass:
- `weather.rs` and `era5.rs` carry explicit `[ASSUMED]` banners; the Route-2 A/B split and the stability/wind bin edges are validated by **direction/structure/count property tests only** (downwind A > upwind A, inversion ⇒ B>0, round-trip identity, Obukhov sign). No class→A/B/C mapping, no L_den (deferred to GRID-03).
- Oracle independence: both the SC1 `r.profile` fixture (independent raster-bilinear sampler) and the SC5 Obukhov fixture (independent ECMWF-recipe reimplementation) are non-self-referential with documented tolerances. Honest caveat (as with the Phase-2 scipy oracles): the independent reimplementations share the same equation *transcription* as the engine, so they cross-check implementation, not the spec reading — acceptable for this derivation-groundwork phase.

### Notes (non-blocking)

- `path.rs` (`PropagationPathInputs`/`CorridorFingerprint`/`PathCacheKey`) is public, tested scaffolding for the Phase-10 solve assembly + Phase-11 recalc router; it deliberately runs no solve and is not yet consumed by a WASM binding. This matches the phase design (the `SolveJob` seam lives engine-side; Phase 9 builds inputs) — not a gap.

### Gaps Summary

None. All five success criteria are observably true in the shipped code with passing automated coverage; all six requirements delivered; the `[ASSUMED]` weather-constant quarantine holds.

---

_Verified: 2026-07-11T19:05:00Z_
_Verifier: Claude (gsd-verifier)_
