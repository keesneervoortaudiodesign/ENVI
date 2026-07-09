---
phase: 06-service-foundation-persistence
plan: 03
subsystem: service
tags: [axum, http-api, self-check, crs, freq-axis, project-crud, geojson, spa, restart-survival, oneshot-tests]

# Dependency graph
requires:
  - phase: 06-service-foundation-persistence (plan 01)
    provides: "envi-geo — LonLat/ProjectCrs (for_location/to_utm/to_wgs84), the one reprojection seam driving the D-08 self-check"
  - phase: 06-service-foundation-persistence (plan 02)
    provides: "envi-store — ProjectStore folder CRUD + reopen-last, validate_feature_collection, ProjectMetaDto/CrsDto/SettingsDto, StoreError"
  - phase: 01-engine-foundations
    provides: "envi-engine freq::{FREQ_AXIS, N_BANDS, N_THIRD_OCT, NOMINAL_THIRD_OCT} — the runtime source of the freq-axis DTO"
provides:
  - "envi-service crate — the single deployable axum binary (SVC-03/04): /api/v1 + the web/dist bundle, localhost-by-default bind, refuse-to-start self-check"
  - "selfcheck::crs_self_check — the D-08 pure-Rust CRS landmark round-trip (<=1 m, refuse-to-start)"
  - "AppState { store } (Arc-shared) + ApiError -> IntoResponse (structured JSON) — the seams plan 06-04 bolts jobs/calc onto"
  - "GET /api/v1/meta/freq-axis — the band-index wire anchor (105 centres, runtime-built from the engine)"
  - "Frozen project + scene endpoint set: projects CRUD/duplicate/last + scene GET/PUT over envi-store"
  - "web/dist/index.html placeholder + the SPA static-bundle fallback contract"
affects: [phase-07-ui, 06-04-jobs-recondition-recompute, scene-persistence, deployment]

# Tech tracking
tech-stack:
  added: ["axum 0.8.9", "tokio 1 (rt-multi-thread/macros/net/signal/sync/time)", "tower-http 0.7 (fs)", "tokio-util 0.7", "tokio-stream 0.1 (sync)", "tracing 0.1 + tracing-subscriber 0.3 (env-filter)", "tower 0.5 (util, dev)", "http-body-util 0.1 (dev)"]
  patterns:
    - "Thin-handler delegation: HTTP parse/validate/status only; storage in envi-store, CRS in envi-geo, acoustics NEVER in the service (SVC-07)"
    - "axum 0.8 brace path syntax /{id} everywhere; contract tests build the FULL router so a colon straggler panics the suite (Pitfall 2)"
    - "Path<Uuid> extractors on every id route — parse IS the path-traversal gate (Pitfall 7)"
    - "Refuse-to-start: main returns Err (non-zero exit) unless the pure-Rust CRS round-trip self-check passes (D-08)"
    - "In-process oneshot contract tests (tower::ServiceExt::oneshot + http-body-util) — no sockets, no credentials"
    - "lib+bin split so integration tests reach the router (api::app) that a binary-only crate would hide"

key-files:
  created:
    - crates/envi-service/Cargo.toml
    - crates/envi-service/src/lib.rs
    - crates/envi-service/src/main.rs
    - crates/envi-service/src/selfcheck.rs
    - crates/envi-service/src/state.rs
    - crates/envi-service/src/error.rs
    - crates/envi-service/src/api/mod.rs
    - crates/envi-service/src/api/meta.rs
    - crates/envi-service/src/api/projects.rs
    - crates/envi-service/src/api/scene.rs
    - crates/envi-service/tests/contract_meta_static.rs
    - crates/envi-service/tests/contract_projects.rs
    - web/dist/index.html
  modified:
    - .gitignore
    - Cargo.lock

key-decisions:
  - "envi-service is a lib+bin: src/lib.rs exposes the modules so integration tests can drive api::app; main.rs is a thin binary over the lib (a binary-only crate hides its modules from tests/)"
  - "geojson added as a direct dep so scene handlers can name geojson::FeatureCollection at the HTTP boundary (already transitive via envi-store)"
  - "tracing-subscriber gains the env-filter feature for the RUST_LOG-aware EnvFilter (default info) per the D-08 startup-logging requirement"
  - "The /api/v1 namespace carries its OWN 404 JSON fallback so unknown API paths never return the SPA HTML; the outer fallback_service(ServeDir) serves index.html for GET / and non-API deep links"
  - "SC2 wording ADJUSTED per D-08: the startup self-check is the pure-Rust CRS landmark round-trip (<=1 m, zone logged), NOT a GDAL/PROJ check — GDAL is deferred to Phase 8"

requirements-completed: [SVC-03, SVC-04, SVC-05, SVC-07]

# Metrics
duration: 13min
completed: 2026-07-09
status: complete
---

# Phase 6 Plan 03: envi-service axum Binary Summary

**The single deployable `envi-service` axum binary: a refuse-to-start pure-Rust CRS self-check (D-08), the band-index freq-axis wire anchor built at runtime from the engine, project CRUD + scene GET/PUT delegating onto envi-store, and a placeholder web/dist served with SPA fallback — all localhost-bound and contract-tested in-process, with restart-survival proven end-to-end.**

## Performance

- **Duration:** 13 min
- **Started:** 2026-07-09T17:39:39Z
- **Completed:** 2026-07-09T17:52:32Z
- **Tasks:** 3
- **Files modified:** 13 created + 2 modified (.gitignore, Cargo.lock)

## Accomplishments
- New `envi-service` crate — the milestone's single self-hosted deployable (SVC-03/04): one axum 0.8 binary serves `/api/v1` AND the `web/dist` bundle, binds `127.0.0.1:8080` by default (loud warning on non-loopback `ENVI_BIND`), and **refuses to start** unless the D-08 self-check passes.
- `selfcheck::crs_self_check` proves the pure-Rust CRS seam at every startup: Dam Square WGS84 -> UTM 31N -> WGS84 within 1 m (measured well under 1e-3 m), the zone/error logged — the SC2 adjustment (GDAL/PROJ check deferred to Phase 8).
- `GET /api/v1/meta/freq-axis` serves the 105-point 1/12-octave axis built at runtime from `envi_engine::freq` (never hardcoded); `centres_hz[64] == 1000.0` bit-exact, `third_octave_indices == [0,4,…,104]` — the band-index wire contract (SVC-07).
- Full project lifecycle over HTTP — list/create/open/update/delete/duplicate + reopen-last (`/projects/last`) — thin handlers delegating to `envi-store`; `POST /projects` pins the UTM CRS from the origin and returns the `CrsDto`.
- Scene GET/PUT round-trips WGS84 GeoJSON; invalid vocabulary/uuids/coordinates are rejected with structured JSON 400s BEFORE persistence (bad input never reaches disk), and a project **survives a simulated service restart** (SC1) — proven by building a fresh `AppState` over the same directory and getting the identical FeatureCollection back.

## Task Commits

Each task was committed atomically:

1. **Task 1: Binary scaffold — main, self-check, state, error, placeholder bundle** - `bd15d58` (feat)
2. **Task 2: Router + freq-axis meta endpoint + static bundle serving, contract-tested** - `3fad902` (feat)
3. **Task 3: Project CRUD + scene GET/PUT endpoints with restart-survival proof** - `723a460` (feat)

## Frozen Endpoint Table (for plan 06-04 + Phase 7 consumers)

All paths are under `/api/v1`. Bodies are JSON; scene bodies are RFC 7946 GeoJSON in **WGS84**. Errors are `{ "error": "<code>", "detail": <value> }` with codes `not_found` (404), `bad_request` (400), `conflict` (409, reserved 06-04), `internal` (500).

| Method | Path | Request DTO | Success | Response DTO |
|--------|------|-------------|---------|--------------|
| GET | `/meta/freq-axis` | — | 200 | `FreqAxisDto { n_bands, centres_hz[105], third_octave_indices[27], nominal_third_octave_hz[27] }` |
| GET | `/projects` | — | 200 | `[ProjectMetaDto]` |
| POST | `/projects` | `CreateProjectRequest { name, description?, origin { lon_deg, lat_deg } }` (deny_unknown_fields) | 201 | `ProjectMetaDto` (carries pinned `CrsDto`) |
| GET | `/projects/last` | — | 200 / 404 | `ProjectMetaDto` |
| GET | `/projects/{id}` | — (records reopen-last) | 200 / 404 | `ProjectMetaDto` |
| PUT | `/projects/{id}` | `UpdateProjectRequest { name?, description?, settings? }` (deny_unknown_fields) | 200 | `ProjectMetaDto` (modified_at bumped) |
| DELETE | `/projects/{id}` | — | 204 | — |
| POST | `/projects/{id}/duplicate` | — | 201 | `ProjectMetaDto` (excludes `calc/`) |
| GET | `/projects/{id}/scene` | — | 200 / 404 | `FeatureCollection` (WGS84) |
| PUT | `/projects/{id}/scene` | `FeatureCollection` (WGS84) | 204 / 400 | — (validated before persist) |

Non-API paths fall through to `ServeDir(web/dist)` with an `index.html` SPA fallback. Unknown `/api/v1/*` paths return 404 JSON (never the HTML shell).

**Env vars:** `ENVI_BIND` (default `127.0.0.1:8080`), `ENVI_PROJECTS_DIR` (default `./projects`), `ENVI_WEB_DIST` (default `web/dist`). **Run:** `cargo run -p envi-service`.

## Decisions Made
- **lib+bin split (blocking, Rule 3):** `src/lib.rs` exposes `pub mod {api, error, selfcheck, state}` so the `tests/*.rs` oneshot suites can build `api::app`. A binary-only crate exposes no modules to integration tests, which the plan's full-router contract tests require. `main.rs` is a thin binary over the lib and carries its own `#![deny(unsafe_code)]` (grep on main.rs = 1).
- **geojson as a direct dep (blocking, Rule 3):** the scene handlers name `geojson::FeatureCollection` at the wire boundary to delegate to `ProjectStore::{load,save}_scene`; geojson was already transitive via envi-store.
- **tracing-subscriber `env-filter` feature:** required by `EnvFilter` for the "env filter, default info" startup logging (D-08).
- **API-namespace 404 fallback:** `api_router` sets its own `fallback(-> ApiError::NotFound)` so unmatched `/api/v1/*` returns structured JSON, decoupled from the outer SPA `ServeDir` fallback — the behavior the plan's static-bundle test pins.
- **SC2 adjustment wording (D-08):** the self-check is documented, in code and headers, as the pure-Rust CRS round-trip that REPLACES the roadmap's literal "GDAL/PROJ self-check"; the GDAL/PROJ version/`proj.db`/`GDAL_DATA` check moves to Phase 8 with the C dependency.

## Deviations from Plan

The three "Decisions Made" infrastructure items above are the only departures from the plan's literal file/dep list, all under Rule 3 (blocking — needed to complete the tasks):

**1. [Rule 3 - Blocking] Added `crates/envi-service/src/lib.rs`**
- **Found during:** Task 1 (surfaced fully in Task 2 when the contract test needed `api::app`).
- **Issue:** The plan's file list names only `main.rs` + modules; a binary-only crate hides its modules from `tests/`, so the plan's full-router oneshot contract tests could not compile.
- **Fix:** Added a `lib.rs` re-exporting the modules; `main.rs` became a thin binary over `envi_service::*`. Both carry `#![deny(unsafe_code)]`.
- **Files modified:** crates/envi-service/src/lib.rs (new), main.rs
- **Commit:** bd15d58 / 3fad902

**2. [Rule 3 - Blocking] Added `geojson = "1"` to envi-service deps**
- **Found during:** Task 3.
- **Issue:** Scene handlers must name `geojson::FeatureCollection` to bridge the wire to the store; the RESEARCH Installation block omitted geojson for envi-service.
- **Fix:** Declared geojson as a direct dep (already a transitive dep via envi-store — no new external package).
- **Commit:** 723a460

**3. [Rule 3 - Minor] Added `env-filter` feature to tracing-subscriber**
- **Found during:** Task 1.
- **Issue:** `EnvFilter` (needed for the "env filter, default info" startup logging) requires the `env-filter` feature, not enabled by the bare `"0.3"` in the RESEARCH block.
- **Fix:** `tracing-subscriber = { version = "0.3", features = ["env-filter"] }`.
- **Commit:** bd15d58

No bugs, no missing-critical functionality, and no architectural changes were needed. Handlers stayed thin; no acoustic math entered the service; the engine was untouched.

## Threat Surface

All eight threats in the plan's `<threat_model>` are addressed as designed; no new security surface beyond it:
- **T-06-03-01 (LAN exposure):** default `127.0.0.1:8080`; non-loopback `ENVI_BIND` logs a prominent no-auth warning (verified by the warn branch in main.rs).
- **T-06-03-02 (no authn/authz):** accepted per PROJECT.md — localhost bind is the access control.
- **T-06-03-03 (path traversal):** every id route uses `Path<Uuid>`; grep confirms no `Path<String>` on id routes; store-side containment guards are defense in depth.
- **T-06-03-04 (oversized bodies):** axum default ~2 MB limit retained + `deny_unknown_fields` strict DTOs (documented in api/mod.rs).
- **T-06-03-05 (invalid scenes corrupting projects):** `save_scene` validates before the atomic write — `scene_put_rejects_invalid` proves the previous scene is unchanged after a rejected PUT.
- **T-06-03-06 (static-path traversal):** `tower-http` ServeDir does traversal-safe resolution; no hand-rolled file reads.
- **T-06-03-07 (broken CRS at startup):** the D-08 refuse-to-start self-check gates every launch.
- **T-06-SC (supply chain):** all packages are the 06-RESEARCH-audited, version-pinned crates.

## Verification Evidence
- `cargo test -p envi-service`: 7 tests green — 2 selfcheck unit (zone 31 healthy path + refusal propagation), 2 contract_meta_static (105-band axis / centres_hz[64] bit-exact / SPA vs 404-JSON), 3 contract_projects (CRUD+reopen-last, restart-survival SC1, invalid-scene rejection).
- Full workspace `cargo test`: green (engine + harness untouched; FORCE case skip-honest, tensor_budget/terrain heavy tests pass).
- `cargo clippy --all-targets -- -D warnings`: clean. `cargo fmt --check`: clean.
- `cargo tree -p envi-engine -e normal --depth 1`: exactly `ndarray`, `num-complex`, `thiserror` — serde/axum never entered the engine.
- `git diff --stat HEAD -- crates/envi-engine/`: empty (engine byte-identical).
- Gates: no `Path<String>` on id routes (only a doc-comment mention); conj quarantine `grep -rn '\.conj(' crates/envi-engine/src/propagation/ | grep -v '//'` = 0 (no propagation touched); no `.github/workflows/` added; `web/dist/index.html` loads zero external assets.

## Next Phase Readiness
- **Plan 06-04** (jobs + recondition/recompute + SSE) bolts onto the frozen seams: `AppState` extends with the job/calc registries; `ApiError::Conflict` is reserved for the 409 tensor-hash-mismatch path; `api_router()` gains the calc/jobs routes; the endpoint table above is the forward contract.
- **Phase 7** (frontend) consumes exactly the endpoints in the table; the placeholder `web/dist/index.html` is replaced by the real MapLibre/React bundle served by this same binary.
- No blockers.

## Self-Check: PASSED

All 13 created files exist on disk; all 3 task commits (`bd15d58`, `3fad902`, `723a460`) are present in git history.

---
*Phase: 06-service-foundation-persistence*
*Completed: 2026-07-09*
