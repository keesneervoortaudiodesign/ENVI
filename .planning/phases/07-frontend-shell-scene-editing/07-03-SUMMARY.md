---
phase: 07-frontend-shell-scene-editing
plan: 03
subsystem: api
tags: [axum, endpoint, interpolation, tin, dgm, band-index, contract-test, error-mapping, quarantine]

# Dependency graph
requires:
  - phase: 07-frontend-shell-scene-editing (plan 01)
    provides: "envi_store::interpolate::{Resolution, interpolate} shared band-index core (D-05) + StoreError grouping"
  - phase: 07-frontend-shell-scene-editing (plan 02)
    provides: "envi_dgm::tin::build_tin + DgmError (TooFewPoints, IntersectingConstraint, NonFinite, TooLarge, Triangulation)"
provides:
  - "POST /api/v1/meta/interpolate-spectrum endpoint (meta::interpolate_spectrum + InterpolateReq/InterpolateResp)"
  - "POST /api/v1/dgm/triangulate endpoint (dgm::triangulate + DgmReq/DgmResp)"
  - "From<DgmError> for ApiError (every variant -> BadRequest 4xx)"
  - "envi_dgm::tin::Tin::vertices()/triangles() read-only mesh accessors"
affects:
  - "Phase-7 frontend editor live-preview (consumes /meta/interpolate-spectrum)"
  - "Phase-7 frontend DGM render (consumes /dgm/triangulate mesh)"
  - "Phase-8 terrain import (extends the same build_tin seam)"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "thin HTTP handler delegates to a store/dgm core; NO acoustics or geometry math in envi-service (SVC-07)"
    - "typed domain error -> ApiError 4xx grouping (From<DgmError> mirrors From<StoreError>)"
    - "range/validation gate owned by the engine constructor (IsolationSpectrum::new), not the service"
    - "offline oneshot contract test over the full app() router (tower::ServiceExt::oneshot, no socket)"

key-files:
  created:
    - crates/envi-service/src/api/dgm.rs
    - crates/envi-service/tests/contract_interpolate_spectrum.rs
    - crates/envi-service/tests/contract_dgm.rs
  modified:
    - crates/envi-service/src/api/meta.rs
    - crates/envi-service/src/api/mod.rs
    - crates/envi-service/src/error.rs
    - crates/envi-service/Cargo.toml
    - crates/envi-dgm/src/tin.rs
    - Cargo.lock

key-decisions:
  - "interpolate_spectrum calls the shared envi_store::interpolate core (D-05, satisfies the interpolate\\( key-link) AND passes the dense grid through the engine IsolationSpectrum::new [0, MAX_R_DB] range gate, so R>1000 is a 4xx and never a silently-clamped 200. The range gate is a Rule-2 add: the plan's Task-1 action only mapped StoreError from interpolate (which cannot reject a range fault), yet must-have #1 + Task-3 require R>1000 -> 4xx."
  - "DgmResp returns a self-contained renderable mesh { vertices: Vec<[f64;3]>, triangles: Vec<[usize;3]> }; this required adding read-only Tin::vertices()/triangles() accessors to envi-dgm (Rule-3 blocking: the wave-1 Tin exposed only counts, not the mesh the response contract needs)."
  - "The non-finite contract case is refused at the transport boundary: JSON has no NaN/inf literal and serde_json rejects an overflow literal (1e400) as NumberOutOfRange -> axum 400. Asserted as a 4xx client error that is never HTML/500; the store finiteness gate remains defense-in-depth for internal call paths."
  - "DgmReq.breaklines is #[serde(default)] so a points-only body is valid (contract-tested)."

patterns-established:
  - "Pattern: engine/store validating constructor is the sole range gate; the HTTP handler surfaces its rejection as a 4xx, never re-implements or clamps."
  - "Pattern: map a wave-1 pure-domain error (DgmError) into the HTTP boundary via a dedicated From arm grouping every variant to BadRequest."

requirements-completed: [SCN-03, WEB-04, WEB-10]

# Metrics
duration: 13min
completed: 2026-07-10
status: complete
---

# Phase 07 Plan 03: Interpolate-Spectrum + DGM-Triangulate Endpoints Summary

**Two thin `envi-service` endpoints — `POST /meta/interpolate-spectrum` (delegates to the shared `envi_store::interpolate` core, D-05) and `POST /dgm/triangulate` (delegates to `envi_dgm::build_tin`, D-08) — each mapping every typed store/dgm fault to a structured 4xx, proven by 10 offline oneshot contract tests, with `envi-engine` byte-identical and its 3-dep quarantine intact.**

## Performance

- **Duration:** ~13 min
- **Started:** 2026-07-10T15:07:39Z
- **Completed:** 2026-07-10T15:20:29Z
- **Tasks:** 3
- **Files modified:** 9 (3 created + 6 modified)

## Accomplishments
- `POST /api/v1/meta/interpolate-spectrum`: expands an authored octave(9)/third(27)/twelfth(105) spectrum onto the dense `[105]` band-index grid via the **single** shared interpolation core (no second impl — SVC-07), then the engine `IsolationSpectrum::new` range gate. Wrong length, out-of-range `R>1000`, and non-finite each surface as a structured 4xx — the out-of-range value is **rejected, never silently clamped** (the load-bearing 07-01 property).
- `POST /api/v1/dgm/triangulate`: builds a constrained-Delaunay TIN from untrusted points + breaklines and returns a self-contained renderable mesh. **Interior-crossing breaklines return a 400, never a panic/500** (Pitfall 3) — contract-proven by the request completing with a 400.
- `From<DgmError> for ApiError`: every `DgmError` variant maps to `BadRequest` (4xx); the `Display` text carries only offending coordinates/counts, so no filesystem paths leak (MED-1 posture preserved).
- 10 new offline oneshot contract tests (6 interpolate + 4 dgm), all green; full workspace `cargo test` green (0 failures); clippy/fmt clean; engine byte-identical; 3-dep quarantine + zero-C-toolchain gates hold.

## Task Commits

Each task was committed atomically:

1. **Task 1: POST /meta/interpolate-spectrum (D-05) + route** — `30648ab` (feat)
2. **Task 2: POST /dgm/triangulate (D-08) + DgmError->ApiError + route** — `1c3aad8` (feat)
3. **Task 3: Contract tests for both endpoints (oneshot, offline)** — `4115e4d` (test)

## Files Created/Modified
- `crates/envi-service/src/api/meta.rs` — added `InterpolateReq`/`InterpolateResp` + `interpolate_spectrum` handler (delegates to `envi_store::interpolate` + engine range gate).
- `crates/envi-service/src/api/dgm.rs` — new module: `DgmReq`/`DgmResp` + `triangulate` handler (delegates to `envi_dgm::tin::build_tin`).
- `crates/envi-service/src/api/mod.rs` — `pub mod dgm;` + two brace-syntax route registrations.
- `crates/envi-service/src/error.rs` — `From<DgmError> for ApiError` (all variants -> 400).
- `crates/envi-service/Cargo.toml` — added `envi-dgm` path dependency.
- `crates/envi-dgm/src/tin.rs` — added read-only `Tin::vertices()`/`triangles()` mesh accessors.
- `crates/envi-service/tests/contract_interpolate_spectrum.rs` — 6 oneshot contract tests.
- `crates/envi-service/tests/contract_dgm.rs` — 4 oneshot contract tests.
- `Cargo.lock` — `envi-dgm`/`spade` now in the `envi-service` graph.

## Decisions Made
- **Range gate in the handler (delegated to the engine).** `interpolate` deliberately does not clamp (07-01), so `interpolate_spectrum` runs the dense grid through `IsolationSpectrum::new` — the sole `[0, MAX_R_DB]` authority — to satisfy must-have #1's "R>1000 -> 4xx, never clamped". This keeps the range rule where it belongs (the engine) while the interpolation math stays in the store; no acoustic math is inlined in the service.
- **Self-contained mesh response.** `DgmResp` returns `{ vertices, triangles }` so the frontend can render the TIN without a second round-trip; required minimal read-only accessors on `envi-dgm`'s `Tin`.
- **Non-finite handled at the transport boundary.** JSON cannot carry NaN/±∞; serde_json rejects `1e400` as `NumberOutOfRange` (axum 400). The contract asserts a 4xx client error that is never HTML/500.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added the engine `IsolationSpectrum::new` range gate to `interpolate_spectrum`**
- **Found during:** Task 1.
- **Issue:** Task-1's action said to call `envi_store::interpolate::interpolate` and "map `StoreError` … so length/finite/range faults become 4xx". But `interpolate` (per 07-01) validates length + finiteness ONLY and never returns a range error — so relying on it alone could NOT satisfy must-have #1 ("R>1000 … returns a structured 4xx") or Task-3's `R>1000 -> 4xx` acceptance. A pure `interpolate` call would 200 on an out-of-range value.
- **Fix:** After the shared `interpolate` call, pass the dense grid through `IsolationSpectrum::new` (the sole `[0, MAX_R_DB]` gate); its rejection maps to `ApiError::BadRequest`. Range enforcement thus stays in the engine, interpolation stays in the store, and no math is inlined in the service (SVC-07 upheld).
- **Files modified:** crates/envi-service/src/api/meta.rs
- **Verification:** `out_of_range_r_is_rejected_never_clamped` asserts `R=2000 -> 400` with a structured `bad_request` body.
- **Committed in:** `30648ab` (Task 1 commit).

**2. [Rule 3 - Blocking] Added read-only `Tin::vertices()`/`triangles()` accessors to `envi-dgm`**
- **Found during:** Task 2.
- **Issue:** `DgmResp` (plan action) requires `triangles: Vec<[usize;3]>`, and Task-3 asserts "200 with a non-empty `triangles`". The wave-1 `Tin` (07-02) exposed only `num_triangles`/`num_vertices`/`interpolate_z` — its `cdt` is private, so the handler could not extract the mesh.
- **Fix:** Added two read-only accessors on `Tin` (`vertices()` -> `[x,y,z]` in vertex-index order; `triangles()` -> vertex-index triples over `inner_faces()`), using `spade`'s `VertexHandle::index()`. No behavior change to `build_tin`; `envi-engine` untouched.
- **Files modified:** crates/envi-dgm/src/tin.rs
- **Verification:** `valid_square_returns_non_empty_triangulation` asserts a non-empty `triangles` with every index `< vertices.len()`; the 11 existing `envi-dgm` unit tests still pass.
- **Committed in:** `1c3aad8` (Task 2 commit).

---

**Total deviations:** 2 auto-fixed (1 missing-critical, 1 blocking).
**Impact on plan:** Both necessary — deviation #1 makes the mandated `R>1000 -> 4xx` behavior real (the plan's own must-have), and #2 supplies the mesh the response contract requires. No scope creep; both stay within the plan's named files (meta.rs / the DgmResp shape / envi-dgm's Tin).

## Issues Encountered
- serde_json rejects `1e400` as `NumberOutOfRange` (confirmed in the registry source), so a non-finite `values` number never reaches the store finiteness gate over HTTP — it is refused earlier as a 400. The contract test asserts the boundary property (4xx, never HTML/500) rather than the internal `NonFinite` code path, which remains covered by 07-01's unit tests and is defense-in-depth for internal callers.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Both backend seams are now reachable over HTTP with structured error contracts; the Phase-7 frontend editor can call `/meta/interpolate-spectrum` for live spectrum preview and `/dgm/triangulate` for DGM rendering.
- Phase-8 terrain import extends the same `build_tin` seam (imported samples as extra vertices) — the endpoint shape needs no change.

## Threat Coverage
| Threat ID | Mitigation shipped |
|-----------|--------------------|
| T-07-03-01 (DoS via `values` length) | store `interpolate` rejects length ∉ {9,27,105} before allocation -> 400 (contract-tested) |
| T-07-03-02 (non-finite / R>1000) | serde boundary refuses non-finite (400); engine `IsolationSpectrum::new` rejects R>1000 unclamped -> 400 (contract-tested) |
| T-07-03-03 (DGM point/breakline size) | `build_tin` `MAX_POINTS`/`MAX_BREAKLINE_VERTICES` caps -> `TooLarge` -> 400 |
| T-07-03-04 (interior-crossing breaklines) | `can_add_constraint` pre-check -> `IntersectingConstraint` -> 400, no panic (contract-tested) |
| T-07-03-05 (error-body disclosure) | validation faults -> `BadRequest{detail}` (safe coords/counts text); no filesystem paths in `DgmError`/store 4xx `Display` |

## Self-Check: PASSED

- `crates/envi-service/src/api/dgm.rs` — FOUND
- `crates/envi-service/tests/contract_interpolate_spectrum.rs` — FOUND
- `crates/envi-service/tests/contract_dgm.rs` — FOUND
- commit `30648ab` — FOUND
- commit `1c3aad8` — FOUND
- commit `4115e4d` — FOUND

---
*Phase: 07-frontend-shell-scene-editing*
*Completed: 2026-07-10*
