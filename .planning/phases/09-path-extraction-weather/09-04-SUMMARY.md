---
phase: 09-path-extraction-weather
plan: 04
subsystem: api
tags: [rust, wasm-bindgen, ts-rs, wire, era5, cds, ssrf, axum, jobs, path-cache, fnv, nord2000]

# Dependency graph
requires:
  - phase: 09-path-extraction-weather
    provides: "09-01/02/03 envi-gis core — cut_profile, segment_ground, inject_screens, receiver_grid, components_from_levels/levels_from_openmeteo/sound_speed_profile_for_azimuth, era5::{obukhov, occurrence_stats}"
  - phase: 08-gis-ingestion-dgm
    provides: "envi-gis-wasm boundary (ts-rs DTOs + logic-free shims + wire_no_drift), envi-service proxy.rs SSRF allowlist"
  - phase: 06-service-skeleton
    provides: "envi-service jobs.rs JobStatus state machine (Queued/Running/Done/Failed/Cancelled + watch::channel worker + SSE)"
  - phase: 04-solver-tensor
    provides: "envi_engine::solver::SolveJob (the PropagationPath seam PropagationPathInputs mirrors as inputs) + ForestCrossing + SoundSpeedProfile"
provides:
  - "WASM boundary: every Phase-9 core fn (cut_profile/segment_ground/inject_screens/receiver_grid/derive_weather/derive_era5) reachable from the browser via ts-rs-generated, drift-checked DTOs in the single committed web/src/generated/wire.ts"
  - "METX-02 transport: flagged-off (feature=era5) POST /era5/import async job on the Phase-6 JobStatus machine, SSRF-allowlisted CDS host, no key/host/path leak; absent by default (contract-tested)"
  - "envi_gis::path::PropagationPathInputs — pure-data Phase-9→10 path-assembly bundle + PathCacheKey = FNV-1a hash(geometry features ∩ corridor), weather-invariant (Tier-2) / corridor-sensitive (Tier-3)"
affects: [09-05-web-weather-panel, 09-06-playwright, 10-solve-tensor, 11-results-isophones]

# Tech tracking
tech-stack:
  added:
    - "envi-dgm + geo + envi-engine (envi-gis-wasm deps — pure-Rust/WASM-safe TIN + polygon input marshalling for the geometry shims)"
    - "envi-gis (envi-service OPTIONAL dep behind feature=era5 — the ERA5 occurrence-stats derivation)"
  patterns:
    - "Geometry WASM shim: rebuild the sans-I/O envi_dgm TIN + geo polygons from coordinate data at the boundary; delegate to exactly one envi_gis:: fn (all geometry/A-B-C math stays in envi-gis)"
    - "Feature-gated endpoint: #[cfg(feature=era5)] module + conditional route in api_router (let-rebind), default build byte-unchanged, route-absent-by-default contract test"
    - "Deterministic FNV-1a 64 cache key over canonicalized f64 bits (−0.0→+0.0), order-independent corridor fingerprint, weather DELIBERATELY excluded"

key-files:
  created:
    - crates/envi-gis/src/path.rs
    - crates/envi-service/src/api/era5.rs
    - crates/envi-service/tests/contract_era5.rs
  modified:
    - crates/envi-gis-wasm/src/dto.rs
    - crates/envi-gis-wasm/src/lib.rs
    - crates/envi-gis-wasm/Cargo.toml
    - crates/envi-gis/src/lib.rs
    - crates/envi-service/src/api/mod.rs
    - crates/envi-service/Cargo.toml
    - crates/envi-service/tests/wire_no_drift.rs
    - web/src/generated/wire.ts

key-decisions:
  - "reqwest NOT gated behind feature=era5 (the plan's literal instruction): the Phase-8 byte relay proxy.rs already requires reqwest unconditionally, so gating it optional would break the default build. Gated the genuinely-new envi-gis dep instead (era5 = [dep:envi-gis]); the default envi-service dep graph/bin stays byte-unchanged and the ERA5 endpoint is still absent+flagged."
  - "New wasm ProfileSegmentDto (not GroundSegmentDto) to avoid a wire-name collision with envi-store's existing GroundSegmentDto — one Rust source per TS wire name kept intact."
  - "PathCacheKey uses a fixed FNV-1a 64 (not std DefaultHasher) so the L1 cache key is reproducible across process runs, independent of std's SipHash keying."
  - "PathCacheKey excludes both weather (Tier-2 readout input) AND path_azimuth_deg (a deterministic function of src/rcv already hashed) — hashes only src/rcv, TerrainProfile geometry (points incl. injected screen vertices + per-segment σ/roughness), forest crossing geometry, and the order-independent CorridorFingerprint."
  - "Geometry shims rebuild the TIN from tin_points each call (no persistent handle crosses the wire) — the honest sans-I/O boundary; a persistent-TIN perf optimization is deferred to the web-wiring plan."

patterns-established:
  - "Logic-free geometry marshaller: build_gis_tin / polygon_from_ring / line_from_pts + one envi_gis:: core call per #[wasm_bindgen] shim"
  - "SSRF chokepoint reuse: resolve_cds_upstream mirrors proxy.rs::resolve_upstream (single hardcoded host, prefix + .. guard before any I/O, generic host-free error)"
  - "Two-mode contract test split by feature cfg: route-absent-by-default (feature off) vs SSRF/derivation/202/disabled (feature on)"

requirements-completed: [METX-02]

# Metrics
duration: 55min
completed: 2026-07-11
status: complete
---

# Phase 9 Plan 04: WASM boundary + flagged ERA5 endpoint + PropagationPathInputs/PathCacheKey Summary

**The Phase-9 pure-Rust core made browser-reachable through ts-rs-generated, drift-checked wire DTOs (logic-free wasm-bindgen shims for cut-profile/impedance/screening/grid/weather/ERA5), plus a default-off SSRF-allowlisted ERA5/CDS async-job endpoint reusing the Phase-6 state machine, and a pure-data `PropagationPathInputs` bundle whose weather-invariant FNV-1a `PathCacheKey` pins the Phase-11 Tier-2/Tier-3 recalc contract.**

## Performance

- **Duration:** ~55 min
- **Completed:** 2026-07-11
- **Tasks:** 3 (all `type=auto`)
- **Files modified:** 12 (3 created, 8 modified, + Cargo.lock)

## Accomplishments

- **Task 1 — WASM boundary (GEOX/GRID/METX).** Added six Phase-9 request/result DTO families to `envi-gis-wasm/dto.rs` (cut-profile, impedance-segmentation, screening, receiver-grid, weather per-azimuth A/B/C, ERA5 occurrence/`1/L`) — all `ts_rs::TS`, `deny_unknown_fields` on requests, geometry as plain `Vec<[f64;2/3]>` coordinate lists (no GeoJSON `Value`). Added one logic-free `#[wasm_bindgen]` shim per core fn (`extract_cut_profile`, `segment_cut_profile`, `inject_screen_edges`, `build_receiver_grid`, `derive_weather`, `derive_era5`); each rebuilds the sans-I/O `envi_dgm` TIN / `geo` polygons at the boundary and delegates to exactly one `envi_gis::` function (all geometry + A/B/C math stays in `envi-gis`). Registered every DTO in `wire_no_drift.rs` and regenerated the committed `web/src/generated/wire.ts` — the no-drift test is green.
- **Task 2 — flagged ERA5/CDS endpoint (METX-02 transport).** Added `envi-service/src/api/era5.rs` behind `#[cfg(feature="era5")]`: `POST /era5/import` spawns the `envi_gis::era5::occurrence_stats` derivation as a job on the Phase-6 `JobStatus` machine (dedicated `std::thread`, Queued→Running→Done/Failed). `resolve_cds_upstream` is the `proxy.rs`-shaped SSRF chokepoint (CDS host hardcoded, reject prefix-escape / `..` before any I/O); the CDS key is read from server env only and sent via TLS `bearer_auth`, never in a response/log/bundle. Live retrieval is flagged off (absent `CDS_API_KEY` ⇒ generic "disabled" 400, no network). The route is registered under the same feature gate; the default build has no ERA5 route (contract-tested).
- **Task 3 — `PropagationPathInputs` + `PathCacheKey`.** Added `envi-gis/src/path.rs`: a pure-data Phase-9→10 assembly bundle (screen-injected+segmented `TerrainProfile`, `src`/`rcv`, `path_azimuth_deg`, per-azimuth `Option<SoundSpeedProfile>` selector, optional `ForestCrossing` seam, `CorridorFingerprint`) with **no solve / no fan-out / no tensor store**. `cache_key()` is a deterministic FNV-1a 64 over the geometry-derived identity only (src/rcv, profile points incl. injected screen vertices + per-segment σ/roughness, forest crossing geometry, order-independent corridor fingerprint), **excluding** `weather` and `path_azimuth_deg`. The module doc pins the L1 path-cache shape + Tier-2 (weather-invariant reuse) / Tier-3 (corridor dirty-diff) contract; unit tests prove the key is EQUAL under a weather-only change and CHANGES under a moved screen vertex / changed impedance / changed corridor fingerprint / added forest crossing.

## Task Commits

Each task was committed atomically:

1. **Task 1: WASM boundary DTOs + logic-free shims (ts-rs no-drift)** — `acdd3d6` (feat)
2. **Task 3: PropagationPathInputs bundle + weather-invariant PathCacheKey** — `6e9646e` (feat)
3. **Task 2: flagged-off ERA5/CDS retrieval endpoint (async job, SSRF-allowlisted)** — `4ecd92a` (feat)

_(Task 3 was implemented and committed before Task 2 — both were ready in parallel; commit order does not affect atomicity.)_

## Files Created/Modified

- `crates/envi-gis-wasm/src/dto.rs` — Phase-9 boundary DTOs (ProfileSegmentDto, GroundSegmentationDto, DrawnZoneDto, ImportedZoneDto, ScreenObjectDto, WeatherComponentsDto, SoundSpeedProfileDto, Era5HourDto, ClassOccurrenceDto + the 6 request/result pairs)
- `crates/envi-gis-wasm/src/lib.rs` — 6 logic-free shims + marshalling helpers (build_gis_tin, polygon_from_ring, line_from_pts, class_char, zone/screen/segmentation converters)
- `crates/envi-gis-wasm/Cargo.toml` — envi-dgm + geo + envi-engine deps (pure-Rust, WASM-safe; no reqwest/tokio)
- `crates/envi-gis/src/path.rs` — PropagationPathInputs, CorridorFeature/CorridorFingerprint, PathCacheKey, cache_key() + 3 unit tests
- `crates/envi-gis/src/lib.rs` — `pub mod path;`
- `crates/envi-service/src/api/era5.rs` — flagged ERA5 endpoint, resolve_cds_upstream SSRF chokepoint, derive_occurrence, submit/run_era5_job
- `crates/envi-service/src/api/mod.rs` — feature-gated `era5` module + conditional route registration
- `crates/envi-service/Cargo.toml` — `[features] era5 = ["dep:envi-gis"]`, optional envi-gis dep
- `crates/envi-service/tests/wire_no_drift.rs` — registered the new wasm DTOs in export_all_wire_types
- `crates/envi-service/tests/contract_era5.rs` — route-absent-by-default + SSRF + derivation + 202 + disabled tests
- `web/src/generated/wire.ts` — regenerated with the 19 new wire types (committed, no-drift)

## Decisions Made

- **`reqwest` stays a hard dependency (deviation from the plan's literal `era5 = ["dep:reqwest"]`).** The plan assumed reqwest was ERA5-only, but the Phase-8 byte relay (`proxy.rs` + `AppState::http`) already requires it unconditionally. Making it optional would break the default build. I gated the genuinely-new `envi-gis` dep behind `era5` instead; the security intent (endpoint absent + flagged, reqwest confined to `envi-service` and never on the wasm/gis path) is fully preserved, and `cargo tree -p envi-service` shows `envi-gis` absent by default / present only under `--features era5`.
- **`ProfileSegmentDto` (not `GroundSegmentDto`).** `envi-store` already exports a `GroundSegmentDto` to `wire.ts`; a second Rust type of that name would collide (Rust import + one-source-per-wire-name). The geometry pipeline's per-interval segment is a distinct wire type.
- **FNV-1a over `DefaultHasher`.** The L1 cache key must be reproducible across process runs for a persistent cache; `DefaultHasher`'s SipHash keying is an implementation detail, so a fixed FNV-1a is used and documented.

## Deviations from Plan

### Auto-fixed / refinements

**1. [Rule 3 — Blocking] `reqwest` cannot be made optional; gated `envi-gis` behind `feature=era5` instead**
- **Found during:** Task 2 (ERA5 Cargo.toml feature).
- **Issue:** The plan's `[features] era5 = ["dep:reqwest", ...]` assumes reqwest is ERA5-only, but the Phase-8 proxy relay (`api/proxy.rs`, `AppState::http`) uses reqwest unconditionally. Making it optional + default-off would fail the default build (proxy would not compile).
- **Fix:** Left reqwest a hard dep (proxy needs it) and gated the new `envi-gis` derivation dep behind the `era5` feature (`era5 = ["dep:envi-gis"]`). The endpoint module + route are still `#[cfg(feature="era5")]`, default-off, contract-tested absent.
- **Files modified:** crates/envi-service/Cargo.toml
- **Verification:** `cargo tree -p envi-service` (default) shows `envi-gis` absent; `--features era5` shows it present; default build byte-unchanged; `era5_route_absent_by_default` green.
- **Committed in:** 4ecd92a (Task 2)

**2. [Rule 3 — Blocking] envi-gis-wasm gained envi-dgm + geo + envi-engine for input marshalling**
- **Found during:** Task 1 (geometry shims).
- **Issue:** The geometry core fns take a `&Tin` (envi-dgm) and `geo::Polygon`/`LineString`; the screening `base` marshals `GroundSegmentation` whose segments are `envi_engine::scene::GroundSegment`. envi-gis-wasm did not depend on these directly.
- **Fix:** Added `envi-dgm`, `geo = "0.30"` (matching envi-gis), and `envi-engine` as direct deps — all pure-Rust / WASM-safe. No async/network/browser edge added (reqwest/tokio remain absent from the wasm graph).
- **Files modified:** crates/envi-gis-wasm/Cargo.toml
- **Verification:** `cargo build -p envi-gis-wasm` green; `cargo tree -p envi-gis-wasm` shows no reqwest/tokio/hyper/mio.
- **Committed in:** acdd3d6 (Task 1)

**3. [Rule 2 — Missing Critical] Renamed the wasm segment DTO to avoid a wire-name collision**
- **Found during:** Task 1 (no-drift build).
- **Issue:** A new `GroundSegmentDto` in envi-gis-wasm collided with envi-store's existing `GroundSegmentDto` (E0252 in the shared no-drift test; and two Rust sources for one TS wire name).
- **Fix:** Renamed to `ProfileSegmentDto` (distinct wire type for the geometry pipeline).
- **Files modified:** crates/envi-gis-wasm/src/{dto,lib}.rs, crates/envi-service/tests/wire_no_drift.rs
- **Verification:** `wire_ts_matches_committed_source` green.
- **Committed in:** acdd3d6 (Task 1)

---

**Total deviations:** 3 (2 blocking dep additions, 1 naming fix — all correctness-necessary). **Impact:** No scope creep. All prohibitions honored: no hand-authored TS (ts-rs single source, no-drift green); ERA5 default-off + contract-tested absent; no CDS key/host/path leak (generic errors); no user-supplied fetch URL (hardcoded CDS host); no acoustic/A-B-C math in the shims (all in envi-gis); reqwest confined to envi-service (never on the gis/wasm/engine path); engine 3-dep quarantine byte-identical.

## Issues Encountered

- **Windows file lock on `envi-service.exe`** (a user-launched server holds the built binary). Per the project hygiene HARD RULE the process was **NOT** killed. `cargo test -p envi-service` in the shared target dir fails only at the final `envi-service.exe` link (`Access is denied (os error 5)`); clippy compiles the crate clean regardless. Worked around by running the envi-service tests (both `default` and `--features era5`) under an isolated `--target-dir target-w9` (a fresh build directory that never touches the locked bin) — all green, then the scratch dir was removed. The default `envi-service` bin is byte-unchanged (ERA5 flagged off), so the running server is unaffected.

## Quality Gates

- `cargo test -p envi-gis --lib path::` — 3 path unit tests green (weather-invariance + Tier-3 sensitivity + order-independence).
- `cargo test -p envi-service` (isolated target dir, default) — full suite green incl. `contract_era5::era5_route_absent_by_default` and `wire_no_drift` (2 + 39 blocks).
- `cargo test -p envi-service --features era5 --test contract_era5` — 6 tests green (SSRF chokepoint, committed-hours derivation, 202 happy path, disabled-without-key).
- `cargo test --workspace --exclude envi-service` — 39 test blocks OK, 0 failed.
- `cargo clippy --all-targets -- -D warnings` (default) + `cargo clippy -p envi-service --all-targets --features era5 -- -D warnings` — both clean.
- `cargo fmt --check` — clean.
- `cargo tree -p envi-engine` — exactly `ndarray + num-complex + thiserror` (quarantine byte-identical).
- `cargo tree -p envi-gis` / `-p envi-gis-wasm` — no HTTP client / async runtime / browser crate; `envi-service` reqwest present, `envi-gis` absent by default (present only under `--features era5`).

## Next Phase Readiness

- The browser can now reach the full Phase-9 geometry + weather core through generated, drift-checked DTOs — 09-05 (web weather-import panel + OPFS cache) and 09-06 (offline Playwright) build directly on `derive_weather` / `derive_era5` and the geometry shims; METX-01's full browser-fetch + zero-egress what-if lands there (it stays **Pending** — this plan delivers only the boundary transport).
- METX-02 is **complete**: the derivation (09-03) + the flagged async endpoint + the WASM boundary together deliver the occurrence-statistics groundwork (full L_den stays deferred to GRID-03).
- `PropagationPathInputs` + `PathCacheKey` are the frozen Phase-10 path-assembly seam: a Phase-10 assembler constructs `SolveJob`s from the bundle, and the L1 cache is keyed by `cache_key()` — Tier-2 weather what-ifs reuse cached paths, Tier-3 geometry edits re-extract only corridor-dirty paths.

## Self-Check: PASSED

All claimed artifacts verified on disk (`crates/envi-gis/src/path.rs`, `crates/envi-service/src/api/era5.rs`, `crates/envi-service/tests/contract_era5.rs`, the modified dto/lib/mod/Cargo/wire files) and all three task commits (`acdd3d6`, `6e9646e`, `4ecd92a`) present in git history.

---
*Phase: 09-path-extraction-weather*
*Completed: 2026-07-11*
