---
phase: 06
verified: 2026-07-09T00:00:00Z
status: passed
criteria_total: 5
criteria_met: 5
score: 5/5 success criteria verified
behavior_unverified: 0
method: goal-backward — read + RAN code (cargo test/clippy/fmt + gate greps), not SUMMARY-trust
gates:
  cargo_test_workspace: pass (exit 0)
  cargo_clippy_all_targets_deny_warnings: pass (exit 0)
  cargo_fmt_check: pass (exit 0)
  engine_quarantine_deps: "ndarray + num-complex + thiserror only (exact)"
  engine_harness_git_diff: empty (engine untouched)
  c_crates_proj_gdal: 0
  spawn_blocking_in_service: 0
  std_thread_spawn_in_service: 2
  conj_in_engine_propagation_code: 0 (9 grep hits are all comments/doc-comments)
requirements:
  - id: SVC-01
    status: satisfied
  - id: SVC-03
    status: satisfied
  - id: SVC-04
    status: satisfied
  - id: SVC-05
    status: satisfied
  - id: SVC-07
    status: satisfied
  - id: GEOX-04
    status: satisfied
residual_risks:
  - "SC2: 'verified on a clean Windows machine' NOT performed this pass — the single-binary + localhost + refuse-to-start behaviour is proven by unit/contract tests and code, but a from-scratch clean-Windows deployment run is an untested residual (honestly reported, not claimed)."
  - "SC4 re-mint gate semantics: HIGH-1 fix is now contract-tested for Phase 6 (scene_edit_invalidates_previously_valid_recondition_hash). The reviewer's 'requires human verification before Phase 11 binds real spectra' note carries forward to Phase 11 (stub spectra are all-zero, so Phase-6 values are unaffected)."
  - "Accepted risks AR-1 (bounded std::fs I/O inside async handlers) and AR-2 (id-less tensor_hash order-dependence, unreachable on persisted paths) documented in 06-REVIEW.md — genuinely accepted within the Phase-6 posture."
---

# Phase 06: Service Foundation & Persistence — Verification Report

**Phase Goal:** A self-hosted service skeleton exists with the milestone's non-retrofittable contracts locked — project persistence, one CRS boundary, the band-index wire format, the recondition/recompute API split, and the job state machine — before any UI binds to them.

**Verified:** 2026-07-09
**Status:** passed (5/5 success criteria met)
**Method:** Goal-backward. Every SC mapped to the concrete test(s) that prove it; those tests were RUN (not read). All workspace gates executed.

## Gate Results (all run, all green)

| Gate | Command | Result |
|------|---------|--------|
| Workspace tests | `cargo test --workspace` | **pass** (exit 0) |
| envi-geo | `cargo test -p envi-geo` | 10 unit + 1 oracle, all pass |
| envi-store | (workspace) | 19 unit, all pass |
| envi-service | `cargo test -p envi-service` | 4 unit + 7 calc + 3 jobs + 2 meta + 3 projects, all pass |
| Lint | `cargo clippy --all-targets -- -D warnings` | **clean** (exit 0) |
| Format | `cargo fmt --check` | **clean** (exit 0) |
| Engine quarantine | `cargo tree -p envi-engine -e normal --depth 1` | exactly `ndarray + num-complex + thiserror` |
| Engine untouched | `git diff crates/envi-engine crates/envi-harness` | empty |
| Zero C crates | `cargo tree --workspace \| grep -ciE 'proj-sys\|gdal'` | **0** |
| Anti-Pattern 5 | `spawn_blocking` in `envi-service/src` | **0** |
| Dedicated worker | `thread::spawn` in `envi-service/src` | **2** (>= 1) |
| conj gate | `.conj()` in `envi-engine/src/propagation/` | **0 real code** (9 hits are all comment/doc lines) |

## Goal Achievement — Observable Truths (Success Criteria)

| # | Truth (SC) | Status | Evidence |
|---|-----------|--------|----------|
| SC1 | Project CRUD + autosave/reopen-last, project-as-folder, survives restart, scene GET/PUT round-trip | ✓ VERIFIED | `contract_projects::crud_lifecycle_and_reopen_last`, `project_round_trips_across_restart` (drops AppState A, rebuilds AppState B over same dir, GETs identical FeatureCollection + reopen-last resolves), `scene_put_rejects_invalid`; `project_dir.rs` atomic `NamedTempFile::new_in`+`sync_all`+`persist` (`atomic_write_replaces_never_truncates`) |
| SC2 (adjusted D-08) | One axum binary serves API + bundle, binds localhost, refuses to start unless pure-Rust CRS self-check passes | ✓ VERIFIED | `main.rs:33-46` — self-check failure `return Err(e.into())` (non-zero exit); `DEFAULT_BIND=127.0.0.1:8080`, non-loopback logs loud warn (`:50-56`); `selfcheck.rs::crs_self_check` (Dam Square WGS84→UTM31N→WGS84, ≤1 m, zone logged) + tests `self_check_passes_on_healthy_stack`, `self_check_failure_refuses_start`; `contract_meta_static::static_bundle_served_with_spa_fallback`. GDAL/PROJ check correctly DEFERRED to Phase 8 (D-02) — not failed. |
| SC3 | Exactly one reprojection boundary: WGS84 wire, per-project auto-UTM, `LonLat`/`SceneXY` newtypes, landmark round-trip to the meter, loud degree-magnitude rejection | ✓ VERIFIED | `envi-geo` sole CRS crate; `transform.rs` radian conversion quarantined "here and ONLY here"; `landmark_round_trip_within_one_meter` (asserts ≤1 m AND ≤1e-3 m), `to_wgs84_rejects_degree_magnitude_input` (typed `GeoError::DegreeMagnitudeSceneCoord`), `oracle_utm` vs pyproj ≤1e-3 m; store reprojects only via `scene_receivers`→`ProjectCrs::to_utm` (one call site) |
| SC4 | recondition/recompute structurally split, content-hash identity, mismatched-hash → 409 against stub tensor, dense band-index spectra, 105-axis served once at meta, no client math | ✓ VERIFIED | `contract_calc` (7 tests): `recondition_rejects_mismatched_tensor_hash_with_409` (frozen `{error,expected,got,hint}` body), `recondition_with_matching_hash_returns_stub_spectra` (dense [105]), `conditioning_never_moves_identity`, `recompute_mints_identity_and_job`, **`scene_edit_invalidates_previously_valid_recondition_hash`** (stale hash 409s — proves identity re-minted on read, not cached), `recondition_unknown_calc_is_404_not_409`. Identity re-minted per request via `load_and_mint` (`calc.rs:200-216`); `tensor_hash` signature accepts NO conditioning type (D-07 structural). `meta.rs` builds axis from `envi_engine::freq::FREQ_AXIS` at runtime; `freq_axis_meta_matches_engine` asserts `centres_hz[64]==1000.0` bit-exact |
| SC5 | Job registry Queued/Running/Done/Failed/Cancelled with SSE; stub job submitted, observed live, cancelled | ✓ VERIFIED | `jobs.rs` — dedicated `std::thread::spawn` worker, `watch` channel, `CancellationToken` cooperative cancel with post-sleep re-check; `contract_jobs`: `stub_job_streams_progress_to_done`, `stub_job_cancel_yields_cancelled` (observes running over live SSE, DELETE→202, cancelled milestone on same stream, GET echoes terminal), `stub_job_failure_is_observable` |

**Score:** 5/5 truths verified (0 present-but-behaviour-unverified).

## Honest-Stub Check (critical — false green is a verification failure)

**PASS.** The compute stubs are honest and cannot be mistaken for validated physics:
- Every `CalcManifest` carries `stub: true` unconditionally in Phase 6 (`manifest.rs:45-46,119`; `calc.rs:310`).
- `ReconditionResponse` carries `stub: bool` set `true`; every canned spectrum is a deterministic all-zero `[105]` array (`calc.rs:236-248`) — obviously synthetic, cannot read as real levels.
- No handler or DTO claims real acoustic results; module headers explicitly flag "honest stubs, no false green".
- The walking-skeleton stubbing is the DECIDED scope (D-07), not a silent reduction: each contract (persistence, CRS, band-wire, recondition/recompute+409, job machine+SSE) is exercised **end-to-end** by contract tests over the real router, not merely typed.

## Requirements Coverage

| Requirement | Description | Status | Evidence |
|-------------|-------------|--------|----------|
| SVC-01 | Project-folder persistence | ✓ Satisfied | `project_dir.rs` folder layout + atomic writes; restart-survival test |
| SVC-03 | axum HTTP API serving built bundle | ✓ Satisfied | `api/mod.rs` `/api/v1` nested + `ServeDir` SPA fallback; static-bundle test |
| SVC-04 | Single binary, localhost bind, startup self-check | ✓ Satisfied | `main.rs` loopback default + refuse-to-start self-check |
| SVC-05 | Project CRUD + reopen-last | ✓ Satisfied | `api/projects.rs`; `crud_lifecycle_and_reopen_last` |
| SVC-07 | Server-side acoustics, band-index wire | ✓ Satisfied | `meta.rs` runtime axis; dense [105] band-index spectra; no client math |
| GEOX-04 | One reprojection boundary | ✓ Satisfied | `envi-geo` sole seam; single `to_utm` call site in store |

No orphaned requirements: ROADMAP maps exactly SVC-01/03/04/05/07 + GEOX-04 to Phase 6, all claimed across the four plans and all satisfied.

## Anti-Patterns Scanned

- No `TODO/FIXME/XXX/HACK/PLACEHOLDER` debt markers introduced in phase files (compute stubs are the DECIDED D-07 scope, explicitly `stub: true`-flagged, not undeclared debt).
- No `spawn_blocking` in the service (Anti-Pattern 5 honored — the job worker is a dedicated `std::thread`).
- Engine `.conj()` propagation gate is 0 in real code (the 9 grep hits are documentation lines reaffirming the rule).
- 13 code-review findings dispositioned: 11 fixed, 2 accepted-risk with rationale (06-REVIEW.md). Fixes verified present in code (e.g. HIGH-1 re-mint gate at `calc.rs:200-216`, MED-1 generic 500 body in `error.rs`, MED-2 resilient project listing).

## Security

06-SECURITY.md: 24/24 threats verified, 0 open, `status: secured`.

## Gaps

None blocking. All five success criteria are genuinely met with passing, behaviour-exercising tests. Residual risks (clean-Windows deployment untested; Phase-11 real-spectra re-mint human check; two accepted risks) are recorded in the frontmatter and are not Phase-6 blockers.

---

_Verified: 2026-07-09_
_Verifier: Claude (gsd-verifier) — goal-backward, code RUN not trusted_
