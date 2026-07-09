# Phase 6: Service Foundation & Persistence - Research

**Researched:** 2026-07-09
**Domain:** Rust self-hosted HTTP service skeleton (axum) + flat-file persistence + pure-Rust CRS boundary + job/SSE state machine, wrapping the frozen `envi-engine` contracts
**Confidence:** HIGH (stack + patterns verified against crates.io/docs.rs this session; existing codebase read directly)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**CRS boundary — pure Rust; GDAL C deferred (GEOX-04, SVC-04)**
- **D-01:** Phase 6 needs **only coordinate reprojection** (WGS84 lon/lat ↔ project-local UTM meters), not raster/vector I/O. Reprojection is implemented in **pure Rust** — `proj4rs` or `geographiclib-rs` (research picks the exact crate and validates the SC3 landmark round-trip to ≤ 1 m). **No C toolchain, no `proj.db`/`GDAL_DATA`, no vcpkg/OSGeo4W in Phase 6.** This honors the CLAUDE.md house rule ("native Rust preferred over FFI wherever a mature one exists; accept `gdal`/`proj` only where no viable pure-Rust option exists").
- **D-02:** The **C-linked `gdal` dependency and its Windows provisioning are DEFERRED to Phase 8** (GIS ingestion), where raster I/O (GLO-30 COG `/vsicurl`, WorldCover, `GDALContourGenerateEx`) has no mature pure-Rust equivalent. `envi-gis` is **not created in Phase 6**. SC2's "GDAL/PROJ self-check" becomes a pure-Rust CRS self-check here.
- **D-03:** The CRS boundary lives in a new **dedicated pure-Rust crate `envi-geo`** owning the `LonLat` / `SceneXY` newtypes, `to_utm`/`to_wgs84`, and auto-UTM-zone selection. Imported by `envi-store`, `envi-service`, and later `envi-gis` (Phase 8) — one crate, one seam (GEOX-04's "exactly one reprojection boundary"), reused rather than re-derived downstream. Degree-magnitude scene coordinates are loudly rejected at this seam (SC3).

**Persistence — flat files (SVC-01, SVC-05)**
- **D-04:** A project **is a folder** of **flat files**, not a SQLite database: `projects/<uuid>/project.json` (metadata + settings + the pinned UTM CRS) + `scene.geojson` (the FeatureCollection) + `calc/<calc_id>/manifest.json` (dims `[S,R,F=105]`, chunk layout, content hashes; `tensor/` + `pincoh/` dirs reserved for Phase 9/10). Git-diffable, human-inspectable, copyable/packable — the NoizCalc "project IS a directory" ethos. Justified by scale: an authored Nord2000 scene is small (hundreds of features), and bulk imported GIS data caches **separately** in `cache/` (Phase 8, DATA-04), so the authored project never grows large.
- **D-05:** The **serde DTO mirror stays in `envi-store`** (architecture Pattern 2) — engine scene types are never serde-derived (the three-dep engine quarantine holds byte-identical). Because the in-memory/wire model is the DTO, a future flat-file→SQLite swap (if per-feature querying ever demands it) is a mechanical storage-layer change, not a rewrite. SQLite is the documented upgrade path, not the Phase-6 choice.
- **D-06:** CRUD covers create / open / save (autosave) / duplicate / delete + reopen-last; a project survives service restart and round-trips scene GET/PUT (SC1). Autosave/reopen-last mechanics are Claude's discretion within this contract.

**Skeleton contract depth — full walking-skeleton, stubbed compute (SVC-07, SC4/SC5)**
- **D-07:** Freeze **and exercise end-to-end** every compute-facing contract with **fake compute**:
  - **recondition/recompute split (SC4):** both endpoints exist with frozen request/response DTOs. **Tensor identity is keyed by content hash** (geometry + met + receiver-set); a `recondition` (conditioning→MAC) request whose `tensor_hash` mismatches the (stub) tensor is **actually rejected with 409** — contract-tested against an in-memory stub tensor. `recompute` (scene/terrain/ground/met→propagation) is the separate path. Conditioning is a **readout parameter, never hashed into tensor identity**. The full dirty-diff recalc **router (Tiers 0–3)** is Phase 10/11 — Phase 6 builds the **split + hash identity + rejection**, not the tier logic.
  - **band-index wire (SC4/SVC-07):** spectra cross the wire as dense `[105]` arrays **keyed by band index**; the 105-point 1/12-octave axis is served **once** at `GET /api/v1/meta/freq-axis`. No client-side acoustic math; `recondition` returns a canned spectrum by band index.
  - **job state machine (SC5):** a **synthetic stub job** genuinely runs `Queued → Running → Done` with **live SSE progress** and a **working cancel → Cancelled** (and `Failed(reason)`); submit / observe-live / cancel are demonstrable.
  - **single binary (SC2):** one axum binary binds **localhost** by default and serves a **placeholder `web/dist`** (the real frontend is Phase 7) — proving the single-deployable-serves-frontend contract. Embedding mechanism (`tower-http` ServeDir vs `include_dir!`) is Claude's discretion.
- **D-08:** **Startup self-check (SC2, adjusted):** the binary refuses to start unless a **pure-Rust CRS round-trip self-check** passes (reproject a known landmark WGS84→UTM→WGS84, assert ≤ 1 m; log the CRS/zone). The **GDAL/PROJ** version/`proj.db`/`GDAL_DATA` self-check moves to Phase 8 with the C dependency (D-02). Long CPU work (none real yet) uses a **dedicated worker/rayon**, never tokio's blocking pool (architecture Anti-Pattern 5) — the stub job establishes this shape.

### Claude's Discretion
- Exact pure-Rust reprojection crate (`proj4rs` vs `geographiclib-rs` vs `utm`) — research picks by the SC3 ≤1 m landmark accuracy + API fit; axum module/router layout and `/api/v1` versioning; the DTO / GeoJSON `properties.kind` feature-property schema (align with the architecture doc's kind list); autosave/reopen-last mechanics; `web/dist` embedding mechanism; SSE keep-alive details.

### Deferred Ideas (OUT OF SCOPE)
- **[ROADMAP COORDINATION] GDAL/PROJ Windows provisioning → Phase 8.** SC2's literal "GDAL/PROJ startup self-check" is replaced in Phase 6 by a pure-Rust CRS round-trip self-check (D-08). The C `gdal` dependency, its Windows provisioning decision (vcpkg / OSGeo4W / bundled), `proj.db`/`GDAL_DATA` resolution, and the GDAL/PROJ startup self-check all move to **Phase 8 (GIS Ingestion)**. Phase 8's discuss-phase must pick this up. The planner should note the SC2 adjustment so verification checks the pure-Rust self-check, not a GDAL one.
- **SQLite persistence** — the documented upgrade path if authored-scene per-feature querying ever demands it; the DTO mirror keeps the swap mechanical (D-05). Not now.
- **The full recalc router (Tiers 0–3)** — Phase 6 builds only the recondition/recompute split + content-hash identity + 409 rejection; the dirty-diff tier routing is Phase 10/11 (D-07).
- **Real tensor/MAC + propagation** behind the endpoints — Phases 9–11 (stubbed here).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SVC-01 | Persist projects as a project folder (scene + settings + chunked cached tensors) | §Persistence layout: `projects/<uuid>/{project.json, scene.geojson, calc/<id>/manifest.json}`; atomic save via `tempfile::NamedTempFile::new_in(dir)` + `persist` (verified atomic-replace semantics); tensor/pincoh dirs reserved by manifest schema |
| SVC-03 | Rust HTTP API backend (axum), serving the built frontend bundle | axum 0.8.9 verified current; `Router::nest("/api/v1", …)` + `tower-http 0.7` `ServeDir("web/dist").fallback(ServeFile index.html)` as `fallback_service`; 0.8 `{id}` path syntax pitfall documented |
| SVC-04 | Single self-hosted deployable service (localhost bind; startup self-check) | Bind `127.0.0.1` default with env override; D-08-adjusted self-check = pure-Rust CRS landmark round-trip ≤ 1 m, refuse-to-start pattern (return `Err` from `main` → non-zero exit); versions/zone logged via `tracing` |
| SVC-05 | Project CRUD lifecycle — create/open/save(autosave)/delete/duplicate + reopen-last | Endpoint set + folder ops in §Architecture Patterns; reopen-last via `projects/.envi-state.json` (server-side last-opened record); duplicate = recursive folder copy + new uuid + rewritten project.json |
| SVC-07 | All acoustics server-side; spectra keyed by band index; 1/12-oct grid served once | `GET /api/v1/meta/freq-axis` DTO built from `envi_engine::freq::FREQ_AXIS` (105 `centres`, `N_BANDS`); dense `[105]` `band_db` arrays in all spectrum DTOs; no Hz keys anywhere on the wire |
| GEOX-04 | Reproject inputs to auto-selected local metric CRS (UTM), pinned per project, one boundary | `envi-geo` crate on **proj4rs 0.1.10** (pure Rust, `utm` projection = exact `etmerc`); `LonLat`/`SceneXY` newtypes; hand-rolled zone formula; degree-magnitude rejection guard; CRS pinned in project.json at creation |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

Actionable directives binding this phase (the planner must verify compliance):

1. **Native Rust over FFI** wherever a mature crate exists — the basis for D-01/D-03; Phase 6 adds **zero C-linked crates**.
2. **Engine dependency quarantine:** `envi-engine` stays at `ndarray` + `num-complex` + `thiserror` **only**, enforced by a `cargo tree -p envi-engine` check; **serde never enters the engine**; engine must remain **byte-identical** this phase (verify-only, no edits).
3. **Rust edition 2024**, workspace `members = ["crates/*"]` (new crates join automatically), `rust-version = 1.96`.
4. **`#![deny(unsafe_code)]`** on pure-math crates; `unsafe` only at genuine FFI boundaries (none exist in Phase 6 — all three new crates should carry `#![deny(unsafe_code)]`).
5. **Quality gates before "done":** `cargo clippy --all-targets -- -D warnings` (zero warnings), `cargo fmt --check`, `cargo test` (all pass, including the FORCE harness which must stay green/skip-honest).
6. **105-point 1/12-octave band-index framework:** compare/key by **band index, never nominal Hz** — this is precisely the SVC-07 wire contract.
7. **conj-quarantine:** no `.conj()` outside `transfer::nord_ratio_to_transfer` — Phase 6 touches no propagation code, so the grep gate must stay at zero trivially.
8. **English-only** for all code/comments/docs/commits; conventional commit style with the `Co-Authored-By: Claude Opus 4.8` trailer; **commit/push only when the user asks**.
9. **No GitHub Actions/CI scaffolding** — do not create `.github/workflows/`.
10. **Never kill VS Code / Java / broad process names**; only PID-targeted children we started (relevant when stopping a dev server started for manual verification).
11. **Documentation contract at close-out:** Module I/O headers, `crates/README.md`, and root `README.md` must reflect the three new crates and the service run command.
12. **Five mandatory GSD completion gates** (code-review, simplify, secure, verify, doc-consistency scan) at phase end.

## Summary

Phase 6 stands up three new pure-Rust crates — `envi-geo` (CRS boundary), `envi-store` (serde DTO persistence), `envi-service` (axum binary) — and freezes the milestone's non-retrofittable wire contracts against stubbed compute. Every load-bearing library choice was verified on crates.io/docs.rs this session: **axum 0.8.9** (note the 0.8 breaking `/{id}` path syntax), **tower-http 0.7.0** ServeDir for the placeholder bundle, **tokio-util CancellationToken** + a dedicated worker thread + `watch` channel for the job state machine bridged to **axum SSE**, **blake3 1.8.5** over a hand-rolled canonical byte encoding for tensor identity, and **tempfile 3.27** `NamedTempFile::new_in(dir)` + `persist` for atomic project-file saves (verified: atomically replaces the destination; must be created in the project dir, not the system temp dir, to stay on one volume).

The pivotal D-01 research question resolves decisively: **`geographiclib-rs` is eliminated** — it ports only the geodesic subset of GeographicLib (Geodesic/GeodesicLine/PolygonArea) and has **no TransverseMercator/UTMUPS at all** [VERIFIED: docs.rs/geographiclib-rs]. The **`utm` crate (0.1.6)** is a thin, WGS84-only implementation with unspecified algorithm, no accuracy statement, and a misspelled public API (`wsg84_utm_to_lat_lon`) — usable only as a reference for the zone formula. **`proj4rs` 0.1.10 is the recommendation**: a pure-Rust PROJ.4 adaptation (no C, no `proj.db`, no external data files) that implements the `utm` projection via `etmerc` — the *exact* transverse Mercator (Poder/Engsager), the same algorithm family C PROJ uses for UTM [VERIFIED: 3liz/proj4rs projections.md]. Its one sharp edge is that longlat coordinates are **in radians** (the official example converts with `to_degrees()`), which the `envi-geo` newtype API hides behind `LonLat { lon_deg, lat_deg }`.

The phase's structural risks are not libraries but contracts: the recondition/recompute DTOs must stay forward-compatible with `envi_engine::{tensor::TensorPair/TensorSink, solver::SolveJob}` (read this session — shapes documented below); the content hash must never include conditioning; and the engine crate must remain byte-identical. The plan should treat the freq-axis DTO, the 409 hash-rejection test, the SSE observe/cancel test, and the restart-survival round-trip as the phase's acceptance backbone.

**Primary recommendation:** Build `envi-geo` on **proj4rs 0.1.10** with hand-rolled UTM zone selection and a pyproj-oracle fixture (house pattern); build `envi-service` on **axum 0.8.9 + tower-http 0.7 + tokio-util CancellationToken + SSE**; hash tensor identity with **blake3 over explicit `f64::to_bits` little-endian canonical bytes**, never JSON text.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| CRS reprojection (WGS84 ↔ UTM) | Backend library (`envi-geo`) | API boundary (`envi-service` calls it on write/read) | GEOX-04: exactly one seam; the browser only ever sees WGS84 GeoJSON |
| UTM zone auto-selection + per-project pin | Backend (`envi-geo` selects; `envi-store` persists in project.json) | — | Pinned at project creation; never re-derived per request |
| Project persistence (folder CRUD, atomic save) | Storage layer (`envi-store`) | API handlers delegate | D-04/D-05: DTO mirror + flat files live together |
| Scene GeoJSON ⇄ DTO ⇄ engine types | Storage layer (`envi-store::{dto,geojson}`) | — | Serde quarantine seam (Anti-Pattern 1) |
| HTTP API, routing, static bundle | API/Backend (`envi-service`) | — | Thin: HTTP + jobs + contracts only, no physics |
| Job state machine, SSE progress, cancel | API/Backend (`envi-service::jobs`) | Dedicated worker thread (not tokio pool) | Anti-Pattern 5: CPU work off the async runtime |
| Tensor content-hash identity | Storage layer (`envi-store` computes/stores in manifest) | Service checks on recondition | Hash lives next to the manifest it keys |
| Band-index wire format / freq-axis meta | API/Backend (`envi-service` DTO from `envi_engine::freq`) | — | Engine is the source; service serializes (no serde in engine) |
| Acoustic math | Engine (Phases 9–11; **stubbed** here) | — | SVC-07: server-side only, never the client |

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `proj4rs` | 0.1.10 | WGS84 ↔ UTM reprojection in `envi-geo` | Only mature pure-Rust option implementing `utm`/`etmerc` (exact TM); zero C, zero `proj.db` [VERIFIED: crates.io + 3liz/proj4rs projections.md] |
| `axum` | 0.8.9 | HTTP API + SSE in `envi-service` | tokio-rs official framework, already the locked architecture choice [VERIFIED: crates.io] |
| `tokio` | 1.52 (features: `rt-multi-thread`, `macros`, `net`, `signal`, `sync`, `time`) | Async runtime | Required by axum [VERIFIED: crates.io] |
| `tower-http` | 0.7.0 (feature: `fs`) | `ServeDir`/`ServeFile` for `web/dist` | Standard axum static serving; 0.7 is current, tower 0.5/http 1.x — axum 0.8-compatible [VERIFIED: crates.io + changelog] |
| `tokio-util` | 0.7.18 | `CancellationToken` for job cancel | The canonical cooperative-cancellation primitive [VERIFIED: crates.io] |
| `tokio-stream` | 0.1.18 | `WatchStream` to bridge `watch::Receiver` → SSE stream | tokio-rs official stream adapters [VERIFIED: crates.io] |
| `serde` / `serde_json` | 1.0.228 / 1.0.150 | DTO mirror + wire JSON in `envi-store`/`envi-service` | The Rust serialization standard [VERIFIED: crates.io] |
| `geojson` | 1.0.0 | `scene.geojson` FeatureCollection parse/build | georust official; RFC 7946 types [VERIFIED: crates.io] |
| `blake3` | 1.8.5 | Tensor identity content hash | Fast, stable output, official BLAKE3 team crate [VERIFIED: crates.io] |
| `tempfile` | 3.27.0 | Atomic save: temp-in-dir + `persist` | `persist` "will atomically replace" an existing target [VERIFIED: docs.rs/tempfile] |
| `uuid` | 1.23 (features: `v4`, `serde`) | Project / feature / job / calc ids | Standard [VERIFIED: crates.io] |
| `thiserror` | 2.0.18 | Typed errors in all three new crates | Already the house error crate (engine uses it) [VERIFIED: crates.io] |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tracing` + `tracing-subscriber` | 0.1.44 / 0.3.23 | Startup self-check logging (SC2: "versions logged, CRS/zone logged"), request/job logs | Init once in `main`; `tracing::info!` elsewhere [VERIFIED: crates.io] |
| `tower` (dev-dep, feature `util`) | 0.5.3 | `ServiceExt::oneshot` for in-process router contract tests | SC4 409 tests, freq-axis test — no socket binding needed [VERIFIED: crates.io] |
| `http-body-util` (dev-dep) | 0.1.3 | Collect response bodies (incl. SSE frames) in tests | Contract tests [VERIFIED: crates.io] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `proj4rs` | `geographiclib-rs` 0.2.7 | **Eliminated:** geodesic-only port — no TransverseMercator/UTMUPS whatsoever [VERIFIED: docs.rs] |
| `proj4rs` | `utm` 0.1.6 | Algorithm/accuracy undocumented, WGS84-only, misspelled API (`wsg84_…`), 0.1.x maturity; keep only as cross-check reference for the zone formula [VERIFIED: docs.rs] |
| `proj4rs` | `proj` (C-linked) | Violates D-01/D-02 — deferred to Phase 8 with GDAL |
| `RwLock<HashMap>` job registry | `dashmap` 6.2.1 | DashMap is fine, but single-user localhost load never contends; `tokio::sync::RwLock<HashMap>` in `AppState` is simpler and dependency-free. **If DashMap is chosen anyway: pin 6.x — `cargo search` surfaces `7.0.0-rc2`, a release candidate; do not adopt an RC** [VERIFIED: docs.rs — 6.2.1 stable] |
| `tower-http` ServeDir | `include_dir!` / `rust-embed` compile-time embed | Embedding gives a literally-single-file binary, but forces a service rebuild for every Phase-7 frontend iteration and bloats compile time. **Recommendation: ServeDir now** (folder shipped next to the binary satisfies SVC-04's "single self-hosted deployable *service*"); revisit embedding at ship time. |
| `blake3` | `sha2` (SHA-256) | Both fine for content identity (integrity, not adversarial security); blake3 is faster and the digest is shorter to eyeball; no interop requirement forces SHA-2 |

**Installation** (workspace-level; new crates declare what they need):

```toml
# crates/envi-geo/Cargo.toml
[dependencies]
proj4rs = "0.1.10"
thiserror = "2"

# crates/envi-store/Cargo.toml
[dependencies]
envi-engine = { path = "../envi-engine" }
envi-geo = { path = "../envi-geo" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
geojson = "1"
uuid = { version = "1", features = ["v4", "serde"] }
tempfile = "3"
blake3 = "1"
thiserror = "2"

# crates/envi-service/Cargo.toml
[dependencies]
envi-engine = { path = "../envi-engine" }
envi-geo = { path = "../envi-geo" }
envi-store = { path = "../envi-store" }
axum = "0.8"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "net", "signal", "sync", "time"] }
tokio-util = "0.7"
tokio-stream = { version = "0.1", features = ["sync"] }
tower-http = { version = "0.7", features = ["fs"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4", "serde"] }
tracing = "0.1"
tracing-subscriber = "0.3"
thiserror = "2"
[dev-dependencies]
tower = { version = "0.5", features = ["util"] }
http-body-util = "0.1"
```

**Version verification:** all versions above confirmed via `cargo search` against crates.io on 2026-07-09 (correct ecosystem registry). `envi-engine`'s `Cargo.toml` is **not touched** — the `cargo tree -p envi-engine` gate must print exactly `ndarray`, `num-complex`, `thiserror` (+ transitive) before and after this phase.

## Package Legitimacy Audit

All packages run through `gsd-tools query package-legitimacy check --ecosystem crates` this session.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| proj4rs | crates.io | since 2023 | ~24k/wk | github.com/3liz/proj4rs | OK | Approved |
| axum | crates.io | since 2021 | ~7.4M/wk | github.com/tokio-rs/axum | OK | Approved |
| tower-http | crates.io | since 2017 | ~7.8M/wk | github.com/tower-rs/tower-http | OK | Approved |
| tokio | crates.io | since 2016 | ~14.3M/wk | github.com/tokio-rs/tokio | OK | Approved |
| tokio-util | crates.io | since 2018 | ~10.5M/wk | github.com/tokio-rs/tokio | OK | Approved |
| tokio-stream | crates.io | — | ~6.5M/wk | github.com/tokio-rs/tokio | OK | Approved |
| blake3 | crates.io | since 2019 | ~2.5M/wk | github.com/BLAKE3-team/BLAKE3 | OK | Approved |
| tempfile | crates.io | since 2015 | ~11.5M/wk | github.com/Stebalien/tempfile | OK | Approved |
| geojson | crates.io | since 2014 | ~130k/wk | github.com/georust/geojson | OK | Approved |
| uuid | crates.io | since 2014 | ~11.4M/wk | github.com/uuid-rs/uuid | OK | Approved |
| serde / serde_json / thiserror | crates.io | — | multi-M/wk | dtolnay | OK | Approved |
| tracing / tracing-subscriber | crates.io | — | ~12.2M / ~9.5M/wk | tokio-rs/tracing | OK | Approved |
| http-body-util | crates.io | — | ~8.0M/wk | hyperium | OK | Approved (dev-dep) |
| tower | crates.io | — | high | tower-rs | OK | Approved (dev-dep) |
| dashmap | crates.io | — | high | xacrimon/dashmap | OK | Approved but **not recommended** (see Alternatives; if used, pin 6.x, never 7.0.0-rc) |
| geographiclib-rs | crates.io | — | — | georust | OK (legitimacy) | **Rejected on capability** — no TM/UTMUPS |
| utm | crates.io | — | — | — | OK (legitimacy) | **Rejected on maturity/opacity** — reference only |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none
**Postinstall-script check:** not applicable — crates.io has no install scripts; `proj4rs` is the only sub-100k-downloads runtime dep and is the QGIS-ecosystem (3liz) maintained PROJ port, cross-confirmed via its official GitHub org.

## Architecture Patterns

### System Architecture Diagram

```
                     (Phase 7 — placeholder only this phase)
  Browser ── GET /  ──────────────────────────────────────────────┐
     │                                                            │
     │  REST /api/v1 (JSON; GeoJSON in WGS84)      SSE /jobs/{id}/events
     ▼                                                            ▼
┌──────────────────────────────── envi-service (axum binary) ───────────────────┐
│ main.rs: tracing init → SELF-CHECK (CRS round-trip ≤1 m, refuse start on fail)│
│          → Router::nest("/api/v1") → fallback_service(ServeDir web/dist)      │
│          → bind 127.0.0.1:8080 (env-overridable)                              │
│                                                                               │
│ api/meta      GET /meta/freq-axis ── DTO from envi_engine::freq::FREQ_AXIS    │
│ api/projects  CRUD + reopen-last ──► envi-store (folder ops, atomic save)     │
│ api/scene     GET/PUT FeatureCollection ──► envi-store::geojson ⇄ dto         │
│                      │ WGS84 ↔ project UTM at THIS boundary only              │
│                      ▼                                                        │
│                 envi-geo::{to_utm, to_wgs84}  (the ONE CRS seam)              │
│ api/calc      POST /calculations → 202 {calc_id, job_id}                      │
│               POST /calculations/{cid}/recondition ── hash check ──► 409 |200 │
│               POST /calculations/{cid}/recompute  ──► new stub job            │
│ jobs.rs       registry: RwLock<HashMap<JobId, JobHandle>>                     │
│               JobHandle { watch::Receiver<JobStatus>, CancellationToken }     │
│               submit ──► std::thread dedicated worker (NEVER spawn_blocking)  │
│                          loop { work-step; watch.send_replace(progress);      │
│                                 if token.is_cancelled() → Cancelled }         │
│               SSE = WatchStream(rx) → Event::json_data → KeepAlive            │
└───────────────────────────────────────────────────────────────────────────────┘
     │                                    │
     ▼                                    ▼
┌── envi-store ─────────────────┐   ┌── envi-engine (FROZEN, verify-only) ──┐
│ dto.rs   serde mirror + From/ │   │ freq::FREQ_AXIS (105 centres)         │
│          TryFrom to engine    │──►│ scene::{Scene,…} (DTO twins these)    │
│ geojson.rs properties.kind    │   │ tensor::{TensorPair,TensorSink}       │
│ project_dir.rs folder CRUD,   │   │ solver::SolveJob                      │
│   atomic temp+persist saves   │   │ (stub contracts stay forward-        │
│ hash.rs blake3 canonical bytes│   │  compatible; NO code changes)         │
│ manifest.rs calc manifest     │   └───────────────────────────────────────┘
└───────┬───────────────────────┘
        ▼
  DISK  projects/<uuid>/{project.json, scene.geojson, calc/<cid>/manifest.json}
        projects/.envi-state.json  (reopen-last)
```

Primary use case trace (scene save): browser PUT `/api/v1/projects/{id}/scene` (WGS84 GeoJSON) → handler validates DTO → `envi-geo::to_utm` per coordinate (pinned project CRS) → `envi-store` writes `scene.geojson` atomically (temp-in-dir + persist) → 200; GET reverses through `to_wgs84`.

### Recommended Project Structure

```
crates/
├── envi-geo/                    # NEW — pure-Rust CRS boundary (GEOX-04)
│   └── src/
│       ├── lib.rs               # #![deny(unsafe_code)]; LonLat, SceneXY newtypes, GeoError
│       ├── crs.rs               # ProjectCrs { utm_zone, hemisphere, proj_string() }, zone_for()
│       └── transform.rs         # to_utm / to_wgs84 via proj4rs (radians hidden inside)
├── envi-store/                  # NEW — persistence + serde DTO mirror (SVC-01/05)
│   └── src/
│       ├── lib.rs               # #![deny(unsafe_code)]; StoreError
│       ├── dto.rs               # serde twins of engine scene types + From/TryFrom
│       ├── geojson.rs           # FeatureCollection ⇄ DTO (properties.kind vocabulary)
│       ├── project_dir.rs       # folder layout, create/open/save/duplicate/delete, atomic write
│       ├── manifest.rs          # calc manifest schema (dims, chunk layout, hashes)
│       └── hash.rs              # blake3 canonical-byte content hash (tensor identity)
└── envi-service/                # NEW — the deployable binary (SVC-03/04/07)
    └── src/
        ├── main.rs              # tracing init, self-check, router, bind, serve
        ├── state.rs             # AppState { projects_root, jobs, open project cache }
        ├── selfcheck.rs         # CRS landmark round-trip (D-08)
        ├── error.rs             # ApiError → IntoResponse (status + JSON body)
        ├── jobs.rs              # registry, JobStatus, worker spawn, cancellation
        └── api/
            ├── mod.rs           # /api/v1 router assembly
            ├── meta.rs          # GET /meta/freq-axis
            ├── projects.rs      # project CRUD + reopen-last
            ├── scene.rs         # scene GET/PUT (CRS mapping at this boundary)
            ├── calc.rs          # calculations submit + recondition/recompute split
            └── jobs.rs          # GET /jobs/{id}, GET /jobs/{id}/events (SSE), DELETE /jobs/{id}
web/
└── dist/                        # placeholder index.html (real frontend = Phase 7)
```

### Pattern 1: The `envi-geo` seam (one CRS boundary, radians quarantined)

**What:** All reprojection goes through two functions on newtypes; proj4rs's radian convention never leaks.
**When to use:** Every wire read/write that touches coordinates. No other crate may call proj4rs.

```rust
// Source: proj4rs crate-root example, docs.rs/proj4rs (radians convention verified)
use proj4rs::proj::Proj;

/// WGS84 geographic coordinate, degrees. The ONLY wire-facing coordinate type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LonLat { pub lon_deg: f64, pub lat_deg: f64 }

/// Project-local UTM coordinate, meters. The ONLY scene-facing coordinate type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneXY { pub x_m: f64, pub y_m: f64 }

pub struct ProjectCrs { pub utm_zone: u8, pub south: bool, proj: Proj, wgs84: Proj }

impl ProjectCrs {
    /// Auto-pick the UTM zone from a lon/lat (pinned at project creation).
    pub fn for_location(p: LonLat) -> Result<Self, GeoError> {
        let zone = utm_zone_for(p)?; // ((lon+180)/6).floor()+1, clamped 1..=60
        let south = p.lat_deg < 0.0;
        let proj_string = format!(
            "+proj=utm +zone={zone}{} +ellps=WGS84 +units=m +no_defs",
            if south { " +south" } else { "" }
        );
        Ok(Self {
            utm_zone: zone, south,
            proj: Proj::from_proj_string(&proj_string)?,
            wgs84: Proj::from_proj_string("+proj=longlat +ellps=WGS84 +datum=WGS84 +no_defs")?,
        })
    }

    pub fn to_utm(&self, p: LonLat) -> Result<SceneXY, GeoError> {
        if !(-180.0..=180.0).contains(&p.lon_deg) || !(-90.0..=90.0).contains(&p.lat_deg) {
            return Err(GeoError::LonLatOutOfRange { lon: p.lon_deg, lat: p.lat_deg });
        }
        // proj4rs longlat is RADIANS — converted here and ONLY here.
        let mut pt = (p.lon_deg.to_radians(), p.lat_deg.to_radians(), 0.0);
        proj4rs::transform::transform(&self.wgs84, &self.proj, &mut pt)?;
        Ok(SceneXY { x_m: pt.0, y_m: pt.1 })
    }

    pub fn to_wgs84(&self, p: SceneXY) -> Result<LonLat, GeoError> {
        // SC3 loud rejection: degree-magnitude values are NOT scene meters.
        // Valid UTM eastings are ~166_000..834_000 m; northings 0..10_000_000 m.
        if p.x_m.abs() <= 360.0 && p.y_m.abs() <= 90.0 {
            return Err(GeoError::DegreeMagnitudeSceneCoord { x: p.x_m, y: p.y_m });
        }
        let mut pt = (p.x_m, p.y_m, 0.0);
        proj4rs::transform::transform(&self.proj, &self.wgs84, &mut pt)?;
        Ok(LonLat { lon_deg: pt.0.to_degrees(), lat_deg: pt.1.to_degrees() })
    }
}
```

**Accuracy basis:** proj4rs's `utm` maps to `etmerc` — the extended (exact) transverse Mercator [VERIFIED: 3liz/proj4rs projections.md]. The Poder/Engsager algorithm family is accurate to well below a millimeter within UTM zone extents [ASSUMED — established PROJ literature; the SC3 landmark test verifies ≤ 1 m empirically, and the pyproj oracle fixture (below) pins it to ~1e-6 m]. A WGS84→UTM→WGS84 round trip is therefore expected at sub-millimeter error; the ≤ 1 m criterion has ~3 orders of magnitude of headroom.

**Zone selection:** hand-roll `zone = ((lon_deg + 180.0) / 6.0).floor() as u8 + 1`, clamp to `1..=60` (lon = +180 edge). **Documented deviation:** the Norway (32V) and Svalbard (31X/33X/35X/37X) grid-exception rules are cartographic conventions, not accuracy requirements — a project pinned to the plain-formula zone is still ≤ 3° from a central meridian, where etmerc scale error is negligible for acoustics. Skip the exceptions; record the deviation in the module header.

### Pattern 2: DTO mirror + `properties.kind` GeoJSON schema (quarantine-preserving)

**What:** `envi-store::dto` holds serde twins; GeoJSON features carry `properties.kind` from the locked vocabulary; `From`/`TryFrom` prove engine-convertibility.
**When to use:** Always (Anti-Pattern 1). Phase 6 only needs the conversions unit-tested — real solves are Phase 9+.

Locked `properties.kind` vocabulary (ARCHITECTURE.md, aligned with the NoizCalc TI 386 object palette):
`source, receiver, wall, building, forest, ground_zone, elevation_point, elevation_line, calc_area`
plus kind-specific properties (heights, impedance class A–H, roughness N/S/M/L, reflection loss, per-source spectrum/balloon/conditioning refs) and a stable UUID `id` per feature. Phase 6 must round-trip **all** kinds structurally (store/serve them faithfully) even though only a subset maps onto engine types today — unknown-to-engine kinds are *persisted*, not dropped (Phase 7 draws them; Phases 8–9 consume them).

```rust
// envi-store/src/dto.rs — serde lives HERE, never in envi-engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandSpectrumDto {
    /// Dense per-band values keyed by band INDEX 0..=104 (the wire contract).
    pub band_db: Vec<f64>, // validated len == envi_engine::freq::N_BANDS
}

impl TryFrom<&BandSpectrumDto> for envi_engine::scene::BandSpectrum {
    type Error = DtoError;
    fn try_from(d: &BandSpectrumDto) -> Result<Self, DtoError> {
        let arr: [f64; envi_engine::freq::N_BANDS] = d.band_db.as_slice()
            .try_into().map_err(|_| DtoError::BadBandCount(d.band_db.len()))?;
        Ok(Self::from_values(arr))
    }
}
```

### Pattern 3: Atomic project save (temp-in-dir + persist)

**What:** Every file write goes through one helper: write to a `NamedTempFile` created **in the project directory**, `sync_all`, then `persist` over the target.
**When to use:** project.json, scene.geojson, manifest.json, .envi-state.json — every mutation (autosave = every PUT persists immediately; the server is authoritative, no dirty state).

```rust
// Source: docs.rs/tempfile NamedTempFile::persist — "If a file exists at the
// target path, persist will atomically replace it." Temp files cannot be
// persisted ACROSS filesystems → MUST be created in the destination dir.
pub fn atomic_write(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), StoreError> {
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?; // same volume as target
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;   // durability: contents flushed before rename
    tmp.persist(dir.join(name))?; // atomic replace
    Ok(())
}
```

**Reopen-last:** persist `{"last_project_id": "<uuid>", "opened_at": "..."}` to `projects/.envi-state.json` on every project open; `GET /api/v1/projects/last` reads it (404 if none/stale). Duplicate = recursive copy of the folder **excluding `calc/`** (stale tensor identity must not travel), new uuid, rewritten `project.json`.

### Pattern 4: Job registry + dedicated worker + SSE

**What:** The SC5 state machine. Registry maps `JobId → JobHandle { watch::Receiver<JobStatus>, CancellationToken }`. The worker is a **`std::thread`** (Anti-Pattern 5: never `spawn_blocking`, never the tokio pool). Progress flows worker → `watch::Sender::send_replace` → per-subscriber `WatchStream` → SSE.

```rust
// Source: docs.rs/axum (response::sse), docs.rs/tokio-util (CancellationToken),
// docs.rs/tokio-stream (WatchStream) — assembled pattern
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running { progress: f32, message: String },
    Done,
    Failed { reason: String },
    Cancelled,
}

pub fn submit_stub_job(state: &AppState) -> JobId {
    let id = JobId(uuid::Uuid::new_v4());
    let (tx, rx) = tokio::sync::watch::channel(JobStatus::Queued);
    let token = tokio_util::sync::CancellationToken::new();
    let t = token.clone();
    // Dedicated OS thread — the Phase-9 real solve inherits this exact shape
    // (rayon inside the thread later; the thread boundary stays).
    std::thread::spawn(move || {
        tx.send_replace(JobStatus::Running { progress: 0.0, message: "started".into() });
        for step in 0..100 {
            if t.is_cancelled() {
                tx.send_replace(JobStatus::Cancelled);
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50)); // fake work
            tx.send_replace(JobStatus::Running {
                progress: (step + 1) as f32 / 100.0, message: format!("step {}", step + 1),
            });
        }
        tx.send_replace(JobStatus::Done);
    });
    state.jobs.blocking_or_async_insert(id, JobHandle { status: rx, cancel: token });
    id
}

// SSE endpoint: GET /api/v1/jobs/{id}/events
async fn job_events(State(app): State<Arc<AppState>>, Path(id): Path<Uuid>)
    -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError>
{
    let rx = app.jobs.read().await.get(&JobId(id)).ok_or(ApiError::NotFound)?.status.clone();
    let stream = tokio_stream::wrappers::WatchStream::new(rx)
        .map(|status| Ok(Event::default().json_data(&status).expect("serializable")));
    Ok(Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

// DELETE /api/v1/jobs/{id} → cancel
async fn cancel_job(State(app): State<Arc<AppState>>, Path(id): Path<Uuid>) -> StatusCode {
    if let Some(h) = app.jobs.read().await.get(&JobId(id)) { h.cancel.cancel(); }
    StatusCode::ACCEPTED
}
```

Notes: `watch` keeps only the **latest** value — exactly right for progress (a slow SSE consumer skips intermediate ticks, never blocks the worker). `send_replace` never fails even with zero receivers (a fire-and-forget worker must not die because the browser closed the tab). Completed jobs stay in the in-memory registry for this phase (terminal status remains queryable); an eviction policy is Phase-10 business.

### Pattern 5: Content-hash tensor identity + 409 rejection (D-07)

**What:** `tensor_hash = blake3(canonical_bytes(geometry_dto, met_dto, receiver_set))`, hex-encoded, stored in `calc/<cid>/manifest.json` and checked by `recondition`. Conditioning is **never** hashed.
**When to use:** From the first calculation endpoint — retrofitting invalidates stored tensors (architecture Pattern 3).

```rust
// envi-store/src/hash.rs — canonical bytes, NOT serialized JSON text.
// f64 → to_bits() → little-endian; length-prefixed sequences; domain-separated
// field tags. Deterministic forever, independent of any serializer's float
// formatting. Version-prefixed so the scheme itself can evolve.
pub fn tensor_hash(geometry: &SceneDto, met: &MetDto, receivers: &[ReceiverDto]) -> String {
    let mut h = blake3::Hasher::new();
    h.update(b"envi-tensor-hash-v1");
    write_scene(&mut h, geometry);     // tag+len prefixed, f64::to_bits().to_le_bytes()
    write_met(&mut h, met);
    h.update(b"receivers");
    h.update(&(receivers.len() as u64).to_le_bytes());
    for r in receivers { write_receiver(&mut h, r); }
    h.finalize().to_hex().to_string()
}
```

```
POST /api/v1/calculations/{cid}/recondition
  { "tensor_hash": "<hex>", "conditioning": { "<source_id>": { "gain_db": -3.0,
      "delay_ms": 1.5, "filter_band_db": [/*105*/], "muted": false } } }
  → 200 { "spectra": { "<receiver_id>": { "band_db": [/*105 canned*/] } },
          "tensor_hash": "<hex>" }
  → 409 { "error": "tensor_hash_mismatch", "expected": "<hex>", "got": "<hex>",
          "hint": "scene/met/receivers changed — POST /recompute" }

POST /api/v1/calculations/{cid}/recompute
  { "reason": "geometry" | "met" | "receivers" }        // request DTO frozen; body minimal
  → 202 { "job_id": "<uuid>", "tensor_hash": "<new hex>" }   // stub job runs the SC5 machine
```

**Forward compatibility with the engine (verified this session, read from source):**
- `TensorPair { h_coh: Array3<Complex<f64>>, p_incoh_abs: Array3<f64> }`, row-major `[sub_source, receiver, freq]` — the manifest's `dims: [S, R, 105]` and receiver-axis `chunk_layout` mirror this exactly.
- `TensorSink::put_chunk(r_offset, ArrayView3<Complex<f64>>, ArrayView3<f64>)` — the Phase-9 file-backed store implements this; Phase 6's manifest reserves `tensor/` + `pincoh/` dirs and records `chunk_receivers` (e.g. 1024) so chunk file naming is already decided.
- `SolveJob` carries per-path inputs (profile, src/rcv, atmosphere, coherence, axis, weather, directivity, forest, isolation) — none of these enter the wire DTOs directly; the hash covers the *DTO-level* geometry/met/receivers from which Phase 9 will construct `SolveJob`s. Conditioning (gain/delay/filter/mute) maps onto `compose_gain` inputs — readout-side, confirming it must stay out of the hash.

### Pattern 6: Startup self-check (D-08, refuse-to-start)

```rust
// envi-service/src/selfcheck.rs — main() returns Err → process exits non-zero.
pub fn crs_self_check() -> Result<(), SelfCheckError> {
    // Landmark: Dam Square, Amsterdam — 4.8936°E, 52.3731°N → UTM 31N
    let landmark = LonLat { lon_deg: 4.8936, lat_deg: 52.3731 };
    let crs = ProjectCrs::for_location(landmark)?;
    let utm = crs.to_utm(landmark)?;
    let back = crs.to_wgs84(utm)?;
    // ≤ 1 m: 1 deg lat ≈ 111_320 m; lon scaled by cos(lat)
    let dlat_m = (back.lat_deg - landmark.lat_deg).abs() * 111_320.0;
    let dlon_m = (back.lon_deg - landmark.lon_deg).abs() * 111_320.0
        * landmark.lat_deg.to_radians().cos();
    let err_m = (dlat_m.powi(2) + dlon_m.powi(2)).sqrt();
    if err_m > 1.0 {
        return Err(SelfCheckError::RoundTrip { err_m });
    }
    tracing::info!(zone = crs.utm_zone, err_m, "CRS self-check passed (pure-Rust proj4rs, UTM {}N)", crs.utm_zone);
    Ok(())
}
```

### Anti-Patterns to Avoid

- **Serde derives on engine types** (architecture Anti-Pattern 1): the DTO mirror exists precisely to prevent this. The `cargo tree -p envi-engine` gate is the enforcement; run it in the phase's tests.
- **`spawn_blocking` for job work** (Anti-Pattern 5): the stub job **establishes the shape** for hour-scale Phase-9 solves; putting it on the blocking pool now normalizes the wrong pattern. Dedicated `std::thread` + `CancellationToken` + `watch`.
- **Hashing serialized JSON for tensor identity:** `serde_json` float formatting is stable today, but tying tensor identity to a serializer's text output is an invisible cross-version coupling. Hash explicit `f64::to_bits` bytes.
- **Conditioning in the tensor hash:** structurally forbidden (D-07); it would make Tier-1 MAC requests self-invalidating.
- **axum 0.7 `/:id` route syntax:** panics at router construction in 0.8. Use `/{id}`, `/{*path}` [VERIFIED: tokio.rs axum 0.8 announcement].
- **`NamedTempFile::new()` (system temp) + persist into the project dir:** fails or falls back non-atomically across volumes — `new_in(project_dir)` always [VERIFIED: docs.rs/tempfile].
- **Nominal-Hz keys on the wire:** spectra are `[105]` dense arrays by band index; the only Hz values anywhere are the `centres_hz` in the freq-axis meta payload (display/charting data, not keys).
- **Serving raw scene coordinates in UTM to the client:** the browser sees WGS84 GeoJSON only; reprojection happens exactly once, server-side (GEOX-04).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Transverse Mercator math | Krüger series transcription | `proj4rs` (`utm`/`etmerc`) | Exact TM has decades of edge cases (meridian convergence, scale, e′² series); a transcription bug here poisons every scene coordinate silently |
| HTTP routing/extractors/SSE | Hand-rolled hyper service | `axum` 0.8 | Router, typed extractors, SSE with keep-alive built in |
| Static file serving | Manual file reads + MIME table | `tower-http::ServeDir` | Range requests, ETag/If-None-Match (new in 0.7), MIME, traversal-safe path resolution |
| Cooperative cancellation | AtomicBool flags | `tokio_util::sync::CancellationToken` | Child tokens, `cancelled()` future for async waits, correct memory ordering |
| Atomic file replace | `fs::rename` with retry loops | `tempfile` persist | Windows `rename`-onto-existing semantics differ from POSIX; tempfile handles the replace atomically |
| GeoJSON parsing/validation | serde_json poking | `geojson` crate | RFC 7946 geometry validation, typed FeatureCollection, position order handled |
| Content hashing | Custom FNV/xor | `blake3` | Stable, collision-resistant, fast; identity bugs are unfindable |
| UUIDs | timestamp+random strings | `uuid` v4 | Parsing gate doubles as the path-traversal guard |

**Deliberately hand-rolled (small, ownership matters):** UTM zone formula (3 lines; avoids the opaque `utm` 0.1.6 dep), canonical hash byte encoding (determinism must be owned by this repo, not a serializer), the job registry (a `HashMap` behind `RwLock` — queue infra like apalis/Redis is explicitly out of scope for a self-hosted single-user tool).

## Common Pitfalls

### Pitfall 1: proj4rs longlat coordinates are radians
**What goes wrong:** Feeding degrees produces coordinates ~57× off (or NaN-adjacent garbage) with no error.
**Why it happens:** proj4rs keeps PROJ.4's internal radian convention; the crate-root example converts with `to_degrees()` after transforming [VERIFIED: docs.rs/proj4rs].
**How to avoid:** Convert inside `envi-geo::transform` only; the `LonLat` newtype carries `_deg` fields so no caller ever touches radians.
**Warning signs:** UTM eastings in the low thousands; landmark test failing by ~6 orders of magnitude.

### Pitfall 2: axum 0.8 path syntax
**What goes wrong:** `route("/projects/:id", …)` panics at startup (router construction), not at compile time.
**Why it happens:** 0.8 moved to matchit 0.8 with `/{id}` / `/{*path}` syntax; the old syntax deliberately panics [VERIFIED: tokio.rs blog].
**How to avoid:** `/{id}` everywhere; the self-check + a smoke test that constructs the full router catches any straggler.
**Warning signs:** Service panics on boot with a matchit route-parse message.

### Pitfall 3: Long CPU work on tokio's pools
**What goes wrong:** `spawn_blocking` (or worse, plain `spawn`) for the job worker starves the pool that file I/O shares; progress reporting and cancel latency degrade.
**Why it happens:** `spawn_blocking` is designed for *bounded* blocking I/O, not hour-scale CPU (architecture Anti-Pattern 5).
**How to avoid:** `std::thread::spawn` per job (Phase 9 adds a semaphore + rayon inside); `watch`/`CancellationToken` are both sync-friendly (`send_replace`, `is_cancelled` need no runtime).
**Warning signs:** SSE keep-alives stall while the stub job runs; cancel takes seconds.

### Pitfall 4: Atomic save across volumes / without sync
**What goes wrong:** `NamedTempFile::new()` creates in `%TEMP%`; `persist` into `projects/` on another volume fails ("cannot be persisted across filesystems") — or a crash right after persist loses content because nothing was flushed.
**How to avoid:** `new_in(project_dir)` + `as_file().sync_all()` before `persist` [VERIFIED: docs.rs/tempfile].
**Warning signs:** Save works on the dev box, fails when `projects/` is on a different drive; empty files after a hard kill.

### Pitfall 5: GeoJSON position order and CRS assumptions
**What goes wrong:** Swapping to `[lat, lon]`, or storing UTM meters in scene.geojson.
**Why it happens:** RFC 7946 mandates `[longitude, latitude]` in WGS84 — which conveniently *is* the wire contract, but the persisted scene must pick one convention and document it.
**How to avoid:** Decision for the planner: persist `scene.geojson` **in WGS84** (RFC-conformant, git-diffable against map tools) and reproject to UTM on load into engine DTO space; the pinned CRS in project.json tells the loader the target. This keeps the file valid GeoJSON for external viewers.
**Warning signs:** Scene renders mirrored/offset in QGIS; degree-magnitude guard fires on load.

### Pitfall 6: watch-channel semantics in SSE tests
**What goes wrong:** A test asserting it saw *every* progress tick flakes — `watch` coalesces to latest-value.
**How to avoid:** SC5 tests assert the *ordered milestones* (Queued → Running(any) → Done / Cancelled), never the full tick sequence.
**Warning signs:** CI-style flaky SSE tests on a loaded machine.

### Pitfall 7: Path traversal through project ids
**What goes wrong:** `projects_root.join(user_supplied_id)` with `id = "../../secrets"` escapes the root.
**How to avoid:** Parse ids as `Uuid::parse_str` **before** any path join (the 409/404 error paths test this); never join raw request strings into paths.
**Warning signs:** Any handler signature taking `Path<String>` for a project/calc/job id instead of `Path<Uuid>`.

### Pitfall 8: dashmap RC on the registry (if chosen over RwLock)
**What goes wrong:** `cargo add dashmap` today resolves 7.0.0-rc2 as newest — an API-unstable release candidate.
**How to avoid:** Prefer `tokio::sync::RwLock<HashMap>`; if DashMap, pin `dashmap = "6"` [VERIFIED: docs.rs — 6.2.1 stable].

### Pitfall 9: Freq-axis DTO drifting from the engine
**What goes wrong:** Hard-coding 105 centre frequencies in the service; later engine changes (never expected, but) or transcription typos silently desynchronize the wire from the engine.
**How to avoid:** Build the DTO from `envi_engine::freq::FREQ_AXIS.centres` at runtime (the array is `pub`); test asserts `centres_hz[64] == 1000.0` exactly, `len == N_BANDS`, and third-octave indices `= [0,4,…,104]`.

## Code Examples

Key patterns are embedded in §Architecture Patterns above (CRS seam, atomic write, job registry + SSE, tensor hash, self-check). Two more wire-contract examples:

### Freq-axis meta endpoint (SVC-07)

```rust
// envi-service/src/api/meta.rs — DTO built FROM the engine, serialized HERE
// (serde never enters envi-engine). Source: crates/envi-engine/src/freq.rs (read).
#[derive(Serialize)]
struct FreqAxisDto {
    n_bands: usize,               // 105
    centres_hz: Vec<f64>,         // FREQ_AXIS.centres — exact values, display/chart use
    third_octave_indices: Vec<usize>,   // [0, 4, 8, …, 104] — the 27 exact 1/3-oct picks
    nominal_third_octave_hz: Vec<f64>,  // NOMINAL_THIRD_OCT — labels only
}

async fn freq_axis() -> Json<FreqAxisDto> {
    use envi_engine::freq::{FREQ_AXIS, N_BANDS, N_THIRD_OCT, NOMINAL_THIRD_OCT};
    Json(FreqAxisDto {
        n_bands: N_BANDS,
        centres_hz: FREQ_AXIS.centres.to_vec(),
        third_octave_indices: (0..N_THIRD_OCT).map(|i| i * 4).collect(),
        nominal_third_octave_hz: NOMINAL_THIRD_OCT.to_vec(),
    })
}
```

### Router assembly + static bundle (SVC-03/04)

```rust
// Source: docs.rs/axum 0.8 + docs.rs/tower-http 0.7 (fs) — standard SPA pattern
let api = Router::new()
    .route("/meta/freq-axis", get(meta::freq_axis))
    .route("/projects", get(projects::list).post(projects::create))
    .route("/projects/last", get(projects::reopen_last))
    .route("/projects/{id}", get(projects::get).put(projects::update).delete(projects::delete))
    .route("/projects/{id}/duplicate", post(projects::duplicate))
    .route("/projects/{id}/scene", get(scene::get).put(scene::put))
    .route("/projects/{id}/calculations", post(calc::submit))
    .route("/calculations/{cid}/recondition", post(calc::recondition))
    .route("/calculations/{cid}/recompute", post(calc::recompute))
    .route("/jobs/{id}", get(jobs::status).delete(jobs::cancel))
    .route("/jobs/{id}/events", get(jobs::events))
    .with_state(app_state);

let app = Router::new()
    .nest("/api/v1", api)
    .fallback_service(
        ServeDir::new("web/dist")
            .fallback(ServeFile::new("web/dist/index.html")), // SPA deep links
    );

// SVC-04: localhost by default; ENVI_BIND overrides for LAN use.
let bind = std::env::var("ENVI_BIND").unwrap_or_else(|_| "127.0.0.1:8080".into());
let listener = tokio::net::TcpListener::bind(&bind).await?;
axum::serve(listener, app).await?;
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| axum 0.7 `/:id` routes | axum 0.8 `/{id}` / `/{*path}` (matchit 0.8) | axum 0.8.0, announced 2025-01-01 | Old syntax panics at router build; all examples online pre-2025 are wrong for 0.8 |
| axum 0.7 `#[async_trait]` extractors | Native async-fn-in-trait | axum 0.8.0 | Custom extractors (if any) drop the macro |
| tower-http 0.6 | tower-http 0.7.0 | current | ServeDir: trailing-slash file requests now 404; strong ETag/If-None-Match added |
| C `proj` bindings for any reprojection | Pure-Rust `proj4rs` for projection math (C PROJ only where datum grids/raster I/O demand it) | proj4rs matured 2023→ | Enables the D-01 zero-C-toolchain service skeleton |
| dashmap 6.x | 7.0 in RC | 7.0.0-rc2 current | Do not adopt the RC |

**Deprecated/outdated:**
- `geographiclib-rs` for projections: never supported them (geodesic-only) — common misassumption from the C++ library's broader scope.
- Storing tensors in SQLite blobs: rejected in ARCHITECTURE.md (kills streaming); Phase 6's manifest reserves flat binary chunk dirs instead.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | proj4rs's `utm`/`etmerc` achieves sub-mm round-trip accuracy (Poder/Engsager-class) | Pattern 1 | LOW — SC3's landmark test + the pyproj oracle fixture verify empirically before anything ships; if accuracy were somehow > 1 m the self-check fails loudly and crate choice is revisited (fallback: transcribe Karney's TM from GeographicLib docs — high effort) |
| A2 | Skipping Norway/Svalbard UTM zone exceptions has negligible accuracy impact for acoustics (≤ 3° from a central meridian in all cases) | Pattern 1 (zone selection) | LOW — worst-case extra scale distortion far below acoustic relevance; documented deviation; trivially added later without breaking pinned projects |
| A3 | Persisting `scene.geojson` in WGS84 (reprojecting on load) is the right file convention | Pitfall 5 | LOW — the alternative (UTM in file) is a contained change inside `envi-store::geojson` before any UI binds; recommend surfacing to user at plan time only if the planner disagrees |
| A4 | `watch`-channel latest-value coalescing is acceptable progress semantics (no per-tick guarantee) | Pattern 4 | LOW — matches SSE consumer needs; if event-log semantics are ever needed, swap to `broadcast` behind the same JobHandle |
| A5 | tower-http 0.7.0 is fully compatible with axum 0.8.9 (both http 1.x / tower 0.5) | Standard Stack | LOW — changelog confirms tower 0.5 since 0.6.0 and no http major bump in 0.7.0; a `cargo build` in the first plan task verifies conclusively; fallback pin `tower-http = "0.6"` |
| A6 | The Dam Square landmark UTM values need no committed absolute anchor — round-trip + pyproj oracle fixture suffice | Validation Architecture | LOW — the oracle fixture (house pattern) IS the absolute anchor once generated with pyproj |

## Open Questions

1. **Does `recompute` need a request body at all in Phase 6?**
   - What we know: D-07 freezes the *split* + DTOs; the real dirty-diff router is Phase 10/11. The stub just spawns the SC5 job and mints a new `tensor_hash`.
   - What's unclear: how much of the eventual Tier-2/3 request shape (which inputs changed) to freeze now.
   - Recommendation: freeze a minimal, extensible body (`{ "reason": … }` plus room for future fields via `#[serde(default)]`), and version the DTO module so Phase 10 extends rather than breaks. Don't speculate tier fields now.

2. **Where does the projects root live on disk?**
   - What we know: D-04 fixes the *per-project* layout; nothing fixes the root.
   - Recommendation: `./projects/` relative to the working directory by default, `ENVI_PROJECTS_DIR` env override; log the resolved absolute path at startup. Add `projects/` to `.gitignore` (user data, not repo content).

3. **Placeholder `web/dist` content.**
   - Recommendation: a single committed `index.html` ("ENVI service running — frontend arrives in Phase 7") + a tiny fetch of `/api/v1/meta/freq-axis` rendered as text, which doubles as a manual smoke check of the bundle-serving contract. Keep it dependency-free (no npm this phase).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| rustc / cargo | build (edition 2024, `rust-version = 1.96`) | ✓ | 1.96.0 | — |
| clippy | quality gate | ✓ | 0.1.96 | — |
| rustfmt | quality gate | ✓ | 1.9.0 | — |
| C toolchain / vcpkg / OSGeo4W | **not needed** (D-01/D-02 — the point of this phase) | n/a | — | — |
| Node.js | **not needed this phase** (placeholder dist is static; real frontend = Phase 7) | ✓ (v24.15.0) | — | — |
| crates.io network access | first `cargo build` of new deps | ✓ (verified this session) | — | — |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** none — this phase was explicitly shaped (D-01/D-02) to need zero provisioning beyond the existing Rust toolchain.

## Validation Architecture

> `workflow.nyquist_validation` is `false` in config; this section is included at the orchestrator's explicit request, framed as "how each Success Criterion is testable" so the planner can derive per-task verification.

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness (`cargo test`) — same as Phases 1–5; no new framework |
| Handler-level HTTP tests | `tower::ServiceExt::oneshot` against the assembled `Router` (dev-deps `tower`/`http-body-util`) — no socket binding, fast, deterministic |
| Config file | none needed (workspace tests) |
| Quick run command | `cargo test -p envi-geo -p envi-store -p envi-service` |
| Full suite command | `cargo test` (workspace — includes the FORCE harness, which must stay green/skip-honest) |

### Phase Requirements → Test Map

| Req / SC | Behavior | Test Type | Test (suggested name) |
|----------|----------|-----------|------------------------|
| GEOX-04 / SC3 | Landmark WGS84→UTM→WGS84 ≤ 1 m (self-check function) | unit (envi-geo) | `landmark_round_trip_within_one_meter` |
| GEOX-04 / SC3 | proj4rs matches C-PROJ ground truth | oracle fixture (house pattern) | `tools/crs_oracle/gen_utm.py` (pyproj) → committed TOML of ~10 world-spread lon/lats + expected UTM easting/northing; Rust test asserts ≤ 1e-3 m against fixture (no Python at test time) |
| GEOX-04 / SC3 | Degree-magnitude scene coordinates loudly rejected | unit | `to_wgs84_rejects_degree_magnitude_input` |
| GEOX-04 | Zone auto-selection correct incl. edges (lon −180/+180, equator, southern hemisphere `+south`) | unit | `utm_zone_selection_edges` |
| SVC-01/05 / SC1 | Project survives restart: create → PUT scene → drop state → reopen from disk → GET scene bit-identical DTO | integration (envi-service, two AppState lifetimes over one tempdir) | `project_round_trips_across_restart` |
| SVC-05 / SC1 | create/open/duplicate/delete/reopen-last lifecycle; duplicate excludes `calc/` | integration | `crud_lifecycle_and_reopen_last` |
| SVC-01 | Atomic save: write is temp-in-dir + persist; interrupted write leaves old file intact | unit (envi-store) | `atomic_write_replaces_never_truncates` |
| SVC-03 / SC2 | `GET /` serves placeholder index.html; unknown path falls back (SPA) | oneshot | `static_bundle_served_with_spa_fallback` |
| SVC-04 / SC2 | Self-check failure → `main` refuses to start (function returns Err; exit path unit-tested) | unit | `self_check_failure_refuses_start` |
| SVC-07 / SC4 | `GET /api/v1/meta/freq-axis`: 105 centres from `FREQ_AXIS`, `centres_hz[64] == 1000.0` exact, third-oct indices `[0,4,…,104]` | oneshot | `freq_axis_meta_matches_engine` |
| SVC-07 / SC4 | Spectra DTOs are dense `[105]` by band index; wrong-length rejected in TryFrom | unit (envi-store) | `band_spectrum_dto_validates_length` |
| SC4 (SVC-06 designed) | recondition with matching hash → 200 canned spectrum; mismatched hash → **409** with expected/got | oneshot vs in-memory stub tensor | `recondition_rejects_mismatched_tensor_hash_with_409` |
| SC4 | Conditioning fields do NOT change `tensor_hash`; geometry/met/receiver changes DO | unit (envi-store::hash) | `tensor_hash_ignores_conditioning_covers_identity_inputs` |
| SC5 / SVC-07 | Submit stub job → SSE stream yields Queued→Running(progress↑)→Done milestones | async integration (oneshot streaming body or ephemeral `127.0.0.1:0` bind) | `stub_job_streams_progress_to_done` |
| SC5 | Cancel mid-run → Cancelled observed on SSE; DELETE idempotent | async integration | `stub_job_cancel_yields_cancelled` |
| Engine quarantine | `envi-engine` byte-identical, deps unchanged | existing gate | `cargo tree -p envi-engine` check + full FORCE suite green |
| DTO mirror | engine `Scene` ⇄ DTO round-trip lossless for engine-mappable kinds; unknown kinds persisted untouched | unit (envi-store) | `dto_engine_round_trip_and_unknown_kind_preservation` |

### Sampling Rate
- **Per task commit:** `cargo test -p <touched crate>` + `cargo clippy --all-targets -- -D warnings` + `cargo fmt --check`
- **Per plan/wave merge:** `cargo test` (full workspace incl. FORCE harness)
- **Phase gate:** full suite green + `cargo run -p envi-service` manual smoke (self-check log line, `GET /` placeholder, one SSE session observed) before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `tools/crs_oracle/gen_utm.py` + committed fixture (pyproj ground truth — generated once on the dev machine, mirrors the existing `tools/nord2000_oracle/` pattern; no Python at test time)
- [ ] `web/dist/index.html` placeholder (needed before the static-serving test)
- [ ] Dev-deps `tower` (util) + `http-body-util` in envi-service

## Security Domain

ASVS Level 1 scope, per `.planning/PROJECT.md`: self-hosted internal tool, **light/no auth**, localhost — the threat model is malformed input + local mistakes, not adversaries.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no (locked: light/no auth, localhost) | Localhost bind **is** the access control; `ENVI_BIND` override logged loudly at startup |
| V3 Session Management | no | Stateless API; no sessions this milestone |
| V4 Access Control | partial | Default bind `127.0.0.1` (SVC-04); no cross-project access paths beyond uuid-keyed folders |
| V5 Input Validation | **yes** | serde DTO strict typing; `Uuid::parse_str` on every id before path use; `BandSpectrumDto` length validation; GeoJSON validated by the `geojson` crate; `axum::extract::DefaultBodyLimit` (default 2 MB is fine for authored scenes — leave default, document it) |
| V6 Cryptography | n/a as security | blake3 is content *identity*, not auth; no secrets exist in this phase |
| V12 File handling | **yes** | Path traversal: ids parsed as `Uuid` before `join`; `ServeDir` handles static-path traversal; atomic writes prevent partial-file corruption |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Path traversal via project/calc/job id in URL | Tampering / Info disclosure | `Path<Uuid>` extractors (parse = gate); never `Path<String>` joined to disk paths |
| Degree-magnitude / garbage coordinates corrupting scenes | Tampering (integrity) | `envi-geo` loud rejection (SC3) + `LonLat` range check + GeoJSON validation |
| Malformed JSON / oversized body DoS | DoS | axum's default body limit + serde strict deserialization (`deny_unknown_fields` on request DTOs is recommended for the frozen contracts — catches client drift early) |
| SSE connection exhaustion (many EventSource tabs) | DoS | Localhost single-user = low risk; `watch` receivers are cheap (no per-subscriber queue growth); keep-alive interval 15 s; document as accepted-risk if no cap is added |
| Stub job thread leak on cancel-spam | DoS | Cancel is idempotent (token); worker threads exit on token check; job submission is user-driven on localhost — acceptable; Phase 10 adds the semaphore bound |
| Serving stale/mismatched tensor results | Integrity (the domain-critical one) | The 409 hash-rejection contract (SC4) — this IS the mitigation, contract-tested |
| Accidental LAN exposure | Spoofing/Info disclosure | Default `127.0.0.1`; non-localhost `ENVI_BIND` values logged as a prominent warning |

## Sources

### Primary (registry/codebase-verified, HIGH)
- `crates/envi-engine/src/{freq.rs, scene.rs, tensor.rs, solver.rs}` — read this session; freq-axis, DTO twins, TensorPair/TensorSink/SolveJob forward-compat shapes
- `.planning/research/ARCHITECTURE.md` — locked topology, endpoint list, `properties.kind` vocabulary, recalc tiers, anti-patterns
- `docs/references/dbaudio-ti386-1.6-en.md` §3–4 — project-as-folder + reopen-last ("At startup, the last-used project loads automatically"), object palette
- crates.io registry via `cargo search` (2026-07-09): all versions in §Standard Stack
- `gsd-tools query package-legitimacy check` — all OK verdicts (§Package Legitimacy Audit)

### Secondary (official docs via WebFetch, MEDIUM)
- [docs.rs/proj4rs](https://docs.rs/proj4rs/latest/proj4rs/) + [3liz/proj4rs projections.md](https://github.com/3liz/proj4rs) — pure-Rust, no proj.db; `utm`/`etmerc`/`tmerc` implemented; radians convention from the crate-root example
- [docs.rs/geographiclib-rs](https://docs.rs/geographiclib-rs/latest/geographiclib_rs/) — geodesic-only public API (no TM/UTMUPS)
- [docs.rs/utm](https://docs.rs/utm/latest/utm/) — API surface, undocumented algorithm/accuracy
- [tokio.rs — Announcing axum 0.8.0](https://tokio.rs/blog/2025-01-01-announcing-axum-0-8-0) + [axum CHANGELOG](https://github.com/tokio-rs/axum/blob/main/axum/CHANGELOG.md) — `/{id}` path syntax, panic on old syntax
- [tower-http CHANGELOG](https://github.com/tower-rs/tower-http/blob/main/tower-http/CHANGELOG.md) — 0.7.0 ServeDir changes, tower 0.5
- [docs.rs/tempfile NamedTempFile::persist](https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html) — atomic replace, same-filesystem constraint, no implicit sync
- [docs.rs/dashmap](https://docs.rs/dashmap) — 6.2.1 stable vs 7.0.0-rc2

### Tertiary (LOW — validated against the above)
- Web search corroboration of the axum 0.8 migration details (multiple community sources agreeing with the official blog)

## Metadata

**Confidence breakdown:**
- CRS crate selection: HIGH — capability elimination is definitive (docs-verified public APIs); accuracy has a small [ASSUMED] residue (A1) covered by the SC3 test + oracle fixture before anything depends on it
- Standard stack/versions: HIGH — every crate verified on crates.io + legitimacy-checked this session
- Architecture/patterns: HIGH — grounded in the locked ARCHITECTURE.md + engine source read directly
- Pitfalls: HIGH for the doc-verified ones (radians, `/{id}`, persist semantics); MEDIUM for the operational ones (watch semantics in tests)

**Research date:** 2026-07-09
**Valid until:** ~2026-08-09 (stable ecosystem; re-verify axum/tower-http minor versions if planning slips a month)
