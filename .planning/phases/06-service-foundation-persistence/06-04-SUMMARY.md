---
phase: 06-service-foundation-persistence
plan: 04
subsystem: service
tags: [axum, jobs, sse, state-machine, cancellation, recondition, recompute, tensor-hash, 409, band-index, honest-stubs, docs]

# Dependency graph
requires:
  - phase: 06-service-foundation-persistence (plan 03)
    provides: "envi-service — AppState (store), ApiError::Conflict reserved for the 409, api_router()/app() seams, Path<Uuid> + oneshot test idiom, the frozen project/scene endpoint table"
  - phase: 06-service-foundation-persistence (plan 02)
    provides: "envi-store — tensor_hash (geometry+met+receivers, conditioning excluded), CalcManifest + chunk_receivers + write_manifest, ConditioningDto/ReceiverDto/MetDto, scene_to_engine"
  - phase: 01-engine-foundations
    provides: "envi-engine freq::N_BANDS + tensor budget consts (BYTES_PER_CELL_PAIR, DEFAULT_TENSOR_BUDGET_BYTES) driving the manifest chunk layout"
provides:
  - "The SC5 job state machine — JobStatus (state-tagged snake_case wire shape), JobHandle (watch::Receiver + CancellationToken), submit_stub_job on a dedicated std::thread (Anti-Pattern 5/D-08), live SSE + working cancel + observable Failed"
  - "The SC4 recondition/recompute structural split with the ENFORCED 409 content-hash gate (D-07) — recondition 200 canned [105] band-index spectra on hash match | 409 {error,expected,got,hint} on mismatch; recompute 202 re-mints identity"
  - "AppState.jobs + AppState.calcs registries + CalcRecord (the in-memory stub tensor identity)"
  - "The frozen Phase-6 compute endpoint set (calculations submit/recondition/recompute + jobs status/events/cancel)"
  - "crates/README.md (created) + root README service run section — the CLAUDE.md doc contract"
affects: [phase-07-ui, phase-10-calculation-service, phase-11-results-recalc, deployment]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "SC5 job machine: watch::channel(JobStatus) + CancellationToken per job, driven by a dedicated std::thread worker (send_replace never blocks/faults); SSE = WatchStream(rx) -> Event::json_data with 15 s KeepAlive"
    - "Anti-Pattern 5 grep-gated: zero literal spawn_blocking in jobs.rs (even in prose), >=1 std::thread::spawn"
    - "Enforced 409: ApiError::Conflict serves the frozen top-level body verbatim (NOT the {error,detail} envelope) so the SC4 contract shape is exact"
    - "D-07 structural exclusion realized end-to-end: recondition compares request.tensor_hash to the stored CalcRecord — conditioning never re-hashes, so any conditioning yields the same identity"
    - "SSE contract tests read the body frame-by-frame (BodyExt::frame) with per-read tokio timeouts, asserting ordered MILESTONES not every coalesced tick (Pitfall 6)"
    - "Honest stubs: every canned [105] spectrum is all-zero and every response/manifest carries stub:true"

key-files:
  created:
    - crates/envi-service/src/jobs.rs
    - crates/envi-service/src/api/jobs.rs
    - crates/envi-service/src/api/calc.rs
    - crates/envi-service/tests/contract_jobs.rs
    - crates/envi-service/tests/contract_calc.rs
    - crates/README.md
  modified:
    - crates/envi-service/src/state.rs
    - crates/envi-service/src/lib.rs
    - crates/envi-service/src/api/mod.rs
    - crates/envi-service/src/error.rs
    - crates/envi-store/src/project_dir.rs
    - README.md

key-decisions:
  - "ApiError::Conflict renders the frozen 409 body VERBATIM (top-level {error:'tensor_hash_mismatch',expected,got,hint}), not wrapped in the {error,detail} envelope — the plan's Task-2 action directs exactly this so the SC4 contract body is exact"
  - "CalcRecord lives in state.rs (per the plan artifact list): project_id + tensor_hash + dims — the in-memory stub tensor the 409 gate checks against"
  - "submit_stub_job is async (writes the registry via RwLock::write().await) and inserts AFTER spawning the detached std::thread worker; the worker's watch::Sender lives on its stack so the SSE stream ends naturally at a terminal state"
  - "Added ProjectStore::project_dir(id) accessor to envi-store so the service can address calc/<cid>/ for manifest writes (root is otherwise private) — a Rule-3 blocking interface extension, engine untouched"
  - "Receiver-axis identity is extracted from the scene by reprojecting receiver-kind features to SceneXY via the pinned ProjectCrs (id + [x,y,z]); source count from source-kind features; dims = [max(1,S), R, 105]"

requirements-completed: [SVC-03, SVC-07]

# Metrics
duration: 25min
completed: 2026-07-09
status: complete
---

# Phase 6 Plan 04: Jobs, Recondition/Recompute + SSE Summary

**The walking skeleton's last non-retrofittable compute contracts: the SC5 job state machine (Queued→Running→Done/Failed/Cancelled) running for real on a dedicated worker thread with live SSE progress and a working cancel, and the SC4 recondition/recompute structural split with content-hash tensor identity and an ACTUALLY-enforced 409 mismatched-hash rejection — plus the closed CLAUDE.md documentation contract (crates/README.md created, root README service section).**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-07-09T19:29:48Z
- **Completed:** 2026-07-09T19:55:00Z
- **Tasks:** 3
- **Files modified:** 6 created + 6 modified

## Accomplishments

- **SC5 is real and observable.** `jobs.rs` implements `JobStatus` (serde `tag = "state"`, snake_case — the frozen wire shape), `JobHandle { watch::Receiver, CancellationToken }`, and `submit_stub_job` which spawns a **dedicated `std::thread`** worker (Anti-Pattern 5, D-08 — the hour-scale Phase-9 solve inherits this exact boundary). `send_replace` emits per-step progress; the token is checked each iteration; `fail_at` drives `Failed(reason)`. `api/jobs.rs` serves `GET /jobs/{id}` (status), `GET /jobs/{id}/events` (SSE over `WatchStream` with a 15 s keep-alive), and an idempotent `DELETE /jobs/{id}` (202). Three contract tests prove submit→observe-live→Done, cancel→Cancelled (repeat DELETE still 202), and an observable non-empty `Failed(reason)`.
- **SC4 is frozen AND enforced.** `api/calc.rs` splits `POST /projects/{id}/calculations` (202 `{calc_id, job_id, tensor_hash}` — mints content-hash identity, writes an honest-stub `CalcManifest`, registers the `CalcRecord`, launches the SC5 job), `POST /calculations/{cid}/recondition` (200 dense `[105]` band-index spectra per receiver flagged `stub:true` on a hash match | **409** `{error:"tensor_hash_mismatch", expected, got, hint}` on a mismatch), and `POST /calculations/{cid}/recompute` (202, re-mints identity from the CURRENT scene). Five contract tests pin the 409, the matched stub spectra, D-07 conditioning-exclusion, recompute mint+invalidate, and the on-disk stub manifest.
- **D-07 realized end-to-end.** Identity is `tensor_hash(scene, met, receivers)` — conditioning is never an argument, so the `conditioning_never_moves_identity` test drives gain −30/muted/filter vs 0/unmuted against the same hash and gets identical `200`/`tensor_hash` both times.
- **Doc contract closed.** `crates/README.md` (which never existed) now carries the five-crate table (role + boundary rule + entry points), the dependency-direction diagram, and the engine quarantine gates; the root README lists the three new crates and documents `cargo run -p envi-service` + the three env overrides + the D-08 refuse-to-start self-check.

## Task Commits

Each task was committed atomically:

1. **Task 1: SC5 job state machine on a dedicated worker thread with live SSE** — `9055959` (feat)
2. **Task 2: recondition/recompute split with the enforced 409 hash gate (SC4)** — `02abe56` (feat)
3. **Task 3: documentation contract — crates/README.md + root README** — `1c0753e` (docs)

## Frozen Phase-6 Endpoint Table (compute half — for Phase 7 + Phases 10/11)

All paths under `/api/v1`. Errors are `{ "error": "<code>", "detail": <value> }` EXCEPT the 409 tensor-hash-mismatch body, which is served verbatim (below).

| Method | Path | Request | Success | Response |
|--------|------|---------|---------|----------|
| POST | `/projects/{id}/calculations` | `{}` | 202 | `{ calc_id, job_id, tensor_hash }` |
| POST | `/calculations/{cid}/recondition` | `{ tensor_hash, conditioning: { <src_uuid>: ConditioningDto } }` (deny_unknown_fields) | 200 / 409 | 200 `{ spectra: { <recv_uuid>: { band_db:[105] } }, tensor_hash, stub:true }` |
| POST | `/calculations/{cid}/recompute` | `{ reason: "geometry"\|"met"\|"receivers" }` (deny_unknown_fields) | 202 | `{ job_id, tensor_hash }` |
| GET | `/jobs/{id}` | — | 200 / 404 | `JobStatus` |
| GET | `/jobs/{id}/events` | — | 200 (SSE) | `text/event-stream` of `JobStatus` JSON, 15 s keep-alive |
| DELETE | `/jobs/{id}` | — | 202 | — (idempotent cancel) |

**`JobStatus` wire shape** (serde `tag="state"`, snake_case):
`{"state":"queued"}` · `{"state":"running","progress":<f32 in (0,1]>,"message":"step N"}` · `{"state":"done"}` · `{"state":"failed","reason":"<non-empty>"}` · `{"state":"cancelled"}`.

**409 body (frozen, top-level — NOT enveloped):**
`{ "error": "tensor_hash_mismatch", "expected": "<minted hex>", "got": "<sent>", "hint": "scene/met/receivers changed — POST /api/v1/calculations/{cid}/recompute" }`.

## Decisions Made

- **`ApiError::Conflict` serves the frozen 409 body verbatim.** 06-03 reserved `Conflict` with a `{ error:"conflict", detail:<body> }` envelope; the SC4 contract requires the mismatch fields at TOP level, so `into_response` now special-cases `Conflict` to return `(409, Json(body))` directly. The plan's Task-2 action directs this exact shape; it is the realization of the reserved variant, not a redesign.
- **`CalcRecord` in `state.rs`** (per the plan artifact list): `{ project_id, tensor_hash, dims }` — the in-memory stub tensor. The 409 gate is a string compare against `CalcRecord::tensor_hash`; recompute overwrites it (invalidating the old hash for recondition).
- **`submit_stub_job` is async**, inserting the handle after spawning the detached `std::thread`. The worker owns the `watch::Sender`; at a terminal state the sender drops and every SSE stream ends after delivering the final status — so tests read frame-by-frame with a timeout and stop on the terminal milestone.
- **Receiver-axis identity extraction:** receiver-kind features are reprojected to SceneXY through the pinned `ProjectCrs` (id + `[x,y,z]`) for the hash's receiver-set; source count is the source-kind feature count; `dims = [max(1,S), R, 105]`.

## Deviations from Plan

**1. [Rule 3 - Blocking] Added `ProjectStore::project_dir(id)` to envi-store**
- **Found during:** Task 2.
- **Issue:** The calc submit/recompute handlers must write `calc/<cid>/manifest.json` under the project folder via `envi_store::manifest::write_manifest(project_dir, …)`, but `ProjectStore`'s `root` and `dir_of` were private — the service had no way to address the project directory.
- **Fix:** Added a `#[must_use] pub fn project_dir(&self, id: Uuid) -> PathBuf` accessor (delegates to the existing `dir_of`; the id is a `Uuid`, so the Pitfall-7 path-traversal posture is preserved). No behavior change to existing store methods; engine untouched.
- **Files modified:** crates/envi-store/src/project_dir.rs
- **Commit:** 02abe56

**2. [Rule 3 - Blocking] `ApiError::Conflict` render adjusted (see Decisions)**
- **Found during:** Task 2.
- **Issue:** 06-03's `Conflict` wrapped its body in the `{ error, detail }` envelope; the frozen SC4 409 body requires `error`/`expected`/`got`/`hint` at the top level.
- **Fix:** `into_response` special-cases `Conflict` to serve the body verbatim with status 409. Removed the now-obsolete `#[allow(dead_code)]` on the variant.
- **Files modified:** crates/envi-service/src/error.rs
- **Commit:** 02abe56

No bugs (Rule 1), no missing-critical functionality (Rule 2), and no architectural changes (Rule 4) were needed. Handlers stayed thin; no acoustic math entered the service; the engine is byte-identical.

## Threat Surface

All seven threats in the plan's `<threat_model>` are addressed as designed; no new security surface beyond it:
- **T-06-04-01 (Integrity — stale/mismatched tensor served):** the 409 content-hash gate IS the mitigation — contract-tested with `expected`/`got` in the body; identity excludes conditioning so the gate cannot be gamed via readout params.
- **T-06-04-02 (DoS — SSE exhaustion):** accepted (localhost single-user); `watch` receivers are cheap (no per-subscriber queues), 15 s keep-alive; the job semaphore is Phase-10.
- **T-06-04-03 (DoS — registry growth / thread-per-job leak):** accepted-bounded; cancel is idempotent via the token, workers exit on the token check; eviction + semaphore are Phase-10 (documented in `jobs.rs`).
- **T-06-04-04 (DoS — malformed recondition bodies):** `deny_unknown_fields` on `ReconditionRequest`, filter-length-105 validation, hashes compared as opaque strings (no hex parsing of attacker input), axum default body limit retained.
- **T-06-04-05 (Repudiation — stub mistaken for real):** `stub:true` in every canned response and manifest; test-asserted.
- **T-06-04-06 (Tampering — CPU starving the runtime):** dedicated `std::thread` worker (grep-gated zero `spawn_blocking`); `send_replace` never blocks on slow consumers.
- **T-06-SC (supply chain):** no new packages added this plan (tokio-util/tokio-stream were already vetted + pinned in 06-03).

## Deferred-to-later-phase Seams

- **Recalc tier router (Tiers 0-3)** — the dirty-diff logic deciding *what* an edit invalidates → **Phase 10/11**. This plan freezes only the split + hash identity + 409 (D-07 scope guard); no tier routing exists in `calc.rs`.
- **Job-registry eviction + submission semaphore** → **Phase 10** (documented in the `jobs.rs` header; completed jobs stay queryable now).
- **Real tensor payload + MAC readout (`compose_gain`)** → **Phases 9-11**; the canned all-zero `[105]` spectra are honest stubs.
- **GDAL/PROJ startup self-check** → **Phase 8** (the pure-Rust CRS round-trip stands in per D-08/06-03).
- **Directional phase into the coherent composition path** → Milestone 2 Phases 10-11 (unchanged pending todo).

## Verification Evidence

- `cargo test -p envi-service`: 17 tests green — 4 unit (2 jobs wire-shape/terminal + 2 selfcheck), 3 contract_jobs (progress→Done, cancel→Cancelled, Failed observable), 5 contract_calc (409 mismatch, matched stub spectra, conditioning-excluded identity, recompute mint+invalidate, stub manifest on disk), 2 contract_meta_static, 3 contract_projects.
- Full workspace `cargo test`: green (35 `test result: ok`; engine + harness untouched; FORCE cases skip-honest, no false Pass — `cargo run -p envi-harness -- report` still honest).
- `cargo clippy --all-targets -- -D warnings`: clean. `cargo fmt --check`: clean.
- **Anti-Pattern 5 gate:** `grep -c 'spawn_blocking' crates/envi-service/src/jobs.rs` = **0**; `grep -c 'std::thread::spawn' …` = **2** (>= 1).
- `cargo tree -p envi-engine -e normal --depth 1`: exactly `ndarray`, `num-complex`, `thiserror`. `git diff --stat HEAD -- crates/envi-engine/`: empty (engine byte-identical).
- `grep -rn '\.conj(' crates/envi-engine/src/propagation/ | grep -v '//' | wc -l`: **0**. `cargo tree | grep -ci 'proj-sys\|gdal'`: **0** (zero C toolchain, D-01/D-02).

## Next Phase Readiness

- **Phase 7 (frontend)** consumes exactly the endpoint table above: EventSource on `/jobs/{id}/events` for live progress, the recondition/recompute split, dense `[105]` band-index spectra keyed off `/meta/freq-axis`.
- **Phase 10 (calculation service)** replaces the stub worker body with the real rayon solve inside the same `std::thread` boundary and adds the registry eviction + semaphore; **Phase 11** fills the real MAC readout behind `recondition`.
- No blockers.

## Self-Check: PASSED

All 6 created files exist on disk; all 3 task commits (`9055959`, `02abe56`, `1c0753e`) are present in git history.

---
*Phase: 06-service-foundation-persistence*
*Completed: 2026-07-09*
