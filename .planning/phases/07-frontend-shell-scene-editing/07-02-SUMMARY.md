---
phase: 07-frontend-shell-scene-editing
plan: 02
subsystem: envi-dgm (server-side digital ground model)
status: complete
tags: [tin, spade, constrained-delaunay, barycentric, breaklines, quarantine, panic-safety, dgm]
requires:
  - phase: 07-frontend-shell-scene-editing (plan 01)
    provides: "crates/* workspace convention + Archetype-A error pattern (GeoError) that DgmError mirrors"
provides:
  - envi_dgm::DgmError (TooFewPoints, IntersectingConstraint, NonFinite, TooLarge, Triangulation)
  - envi_dgm::tin::ElevVertex (HasPosition over [x,y,z])
  - envi_dgm::tin::Tin (+ interpolate_z, num_triangles, num_vertices)
  - envi_dgm::tin::build_tin(&[[f64;3]], &[Vec<[f64;2]>]) -> Result<Tin, DgmError>
  - envi_dgm::tin::MAX_POINTS / MAX_BREAKLINE_VERTICES (DoS caps)
affects:
  - Phase-7 DGM HTTP endpoint (maps DgmError -> ApiError 4xx; From<DgmError> arm still to add)
  - Phase-8 terrain import (extends the SAME seam by feeding imported samples as extra vertices)
tech-stack:
  added: ["spade 2.15 (pure-Rust constrained-Delaunay, quarantined to envi-dgm)"]
  patterns:
    - "panic guard: can_add_constraint pre-check before add_constraint (spade never panics on data)"
    - "Archetype-A pure-domain error copied from GeoError (PartialEq, struct variants carrying offending values)"
    - "boundary crate: new member via crates/* glob, zero root Cargo edit, spade never reaches envi-engine"
key-files:
  created:
    - crates/envi-dgm/Cargo.toml
    - crates/envi-dgm/src/lib.rs
    - crates/envi-dgm/src/tin.rs
  modified:
    - Cargo.lock
key-decisions:
  - "Added a 5th DgmError variant TooLarge {kind, got, limit} beyond the plan's four, to give the DoS cap (threat T-07-02-02, mandated by the Task-2 action) a dedicated typed error rather than overloading an existing variant."
  - "Breakline vertices (2-D, no Z) take their Z by barycentric interpolation over the point surface at insert time, with a nearest-vertex fallback outside the hull — never a silent 0.0."
  - "Degeneracy is detected structurally via num_inner_faces() == 0 (covers 0/1/2 points AND all-collinear in one check) rather than a separate collinearity predicate."
patterns-established:
  - "Pattern: pre-validate spade constraints (can_add_constraint) to convert a library panic into a typed 4xx-mappable error"
  - "Pattern: DoS-cap untrusted geometry counts before O(n log n) work"
requirements-completed: [WEB-04]
metrics:
  tasks_completed: 2
  files_created: 3
  files_modified: 1
  commits: 2
  duration: "~25 min"
  completed: 2026-07-10
---

# Phase 07 Plan 02: envi-dgm Constrained-Delaunay TIN Summary

**A pure-Rust `envi-dgm` crate builds a `spade` constrained-Delaunay TIN from user-drawn elevation points + breaklines with barycentric Z interpolation, turning `spade`'s interior-intersection PANIC into a typed `IntersectingConstraint` error — with `spade` fully quarantined from `envi-engine`.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-07-10
- **Completed:** 2026-07-10
- **Tasks:** 2
- **Files modified:** 4 (3 created + Cargo.lock)

## Accomplishments
- New `envi-dgm` boundary crate (D-08): `spade 2.15` + `thiserror` only, joined via the `crates/*` glob with no root `Cargo.toml` edit.
- `DgmError` (Archetype A, `PartialEq`) mirroring `GeoError` — struct variants carry the offending values.
- `build_tin` + `Tin::interpolate_z`: constrained-Delaunay from `[x,y,z]` points and `[x,y]` breakline polylines, barycentric Z query over the containing face.
- **Panic safety (the load-bearing rule):** every breakline segment is pre-checked with `can_add_constraint` before `add_constraint`; interior crossings (and self-intersections) return `IntersectingConstraint` and the process provably does NOT abort — asserted in tests.
- Degenerate (0/1/2 points, all-collinear), non-finite, and oversized input all rejected with typed errors; out-of-hull query returns `None`, never a silent `0.0`.
- 11 unit tests, all green; `spade` proven absent from the engine graph; `envi-engine` byte-identical.

## Task Commits

1. **Task 1: Scaffold envi-dgm crate + DgmError** - `96fb0b7` (feat)
2. **Task 2: Constrained-Delaunay TIN + barycentric Z (panic-proof)** - `954bfc4` (feat)

_Task 2 is `tdd="true"`; implementation + `#[cfg(test)] mod tests` live in one file per the plan's own action, so RED/GREEN were collapsed into a single `feat` commit rather than separate `test`/`feat` commits._

## Files Created/Modified
- `crates/envi-dgm/Cargo.toml` - New boundary crate manifest; `spade = "2.15"`, `thiserror = "2"`; boundary-rule comment block (D-08).
- `crates/envi-dgm/src/lib.rs` - Crate root: module-doc + Boundary statement + `#![deny(unsafe_code)]` + `pub enum DgmError`.
- `crates/envi-dgm/src/tin.rs` - `ElevVertex`, `Tin`, `build_tin`, `interpolate_z`, DoS caps, panic-guarded breakline constraints, 11 tests.
- `Cargo.lock` - New `envi-dgm` + `spade` graph pinned.

## Decisions Made
- **5th error variant `TooLarge`:** the plan listed four `DgmError` variants but its Task-2 action + threat T-07-02-02 both mandate a typed error above the DoS cap. Rather than overload `TooFewPoints`/`Triangulation`, a dedicated `TooLarge { kind, got, limit }` was added. The four required variants are all present and `PartialEq` holds. (See Deviations — Rule 2.)
- **Breakline Z:** breaklines are 2-D; each breakline vertex's Z is interpolated from the point surface (barycentric, nearest-vertex fallback outside the hull) — documented in the module header, never a silent `0.0`.
- **Degeneracy via `num_inner_faces() == 0`:** one structural check covers both "too few points" and "all-collinear" (both yield zero triangles), instead of a bespoke collinearity test.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added `DgmError::TooLarge` for the DoS cap**
- **Found during:** Task 2 (TIN builder) — cross-checked against Task 1's variant list.
- **Issue:** The plan's Task-1 action enumerates four `DgmError` variants, but Task-2's action and threat register T-07-02-02 both require "return a typed error above the cap." None of the four fit a size-limit fault cleanly.
- **Fix:** Added `TooLarge { kind: &'static str, got: usize, limit: usize }` and the `MAX_POINTS` / `MAX_BREAKLINE_VERTICES` guards at the top of `build_tin`, rejecting before any `O(n log n)` work.
- **Files modified:** crates/envi-dgm/src/lib.rs, crates/envi-dgm/src/tin.rs
- **Verification:** `oversized_point_set_is_rejected` test asserts `Err(TooLarge { kind: "points", .. })`; all four mandated variants still present and `PartialEq`.
- **Committed in:** `96fb0b7` (variant) + `954bfc4` (guards)

**2. [Rule 3 - Blocking] Placeholder `tin.rs` in the Task-1 commit**
- **Found during:** Task 1 (scaffold) — `lib.rs` declares `pub mod tin;`, so the crate cannot build without `tin.rs` existing, yet Task 1's verify is `cargo build -p envi-dgm`.
- **Issue:** Committing Task 1 with only `Cargo.toml` + `lib.rs` would leave a non-compiling tree.
- **Fix:** Included a module-header-only placeholder `tin.rs` in the Task-1 commit so the scaffold builds green; Task 2 replaced it with the full implementation.
- **Files modified:** crates/envi-dgm/src/tin.rs
- **Verification:** `cargo build -p envi-dgm` green at the Task-1 commit.
- **Committed in:** `96fb0b7`

---

**Total deviations:** 2 auto-fixed (1 missing-critical, 1 blocking).
**Impact on plan:** Both necessary — `TooLarge` closes the mandated DoS mitigation; the placeholder keeps every commit compiling. No scope creep.

## Issues Encountered
- `Tin` needed a manual `Debug` impl (used by test `assert!` messages) because `ConstrainedDelaunayTriangulation` is not `Debug`; added a lightweight `Debug` printing vertex/triangle counts.
- `spade`'s `nearest_neighbor` is a `DelaunayTriangulation`-only method (not on `ConstrainedDelaunayTriangulation`); the out-of-hull breakline-Z fallback iterates `cdt.vertices()` and picks the minimum squared distance via `total_cmp`.

## Threat Coverage
| Threat ID | Mitigation shipped |
|-----------|--------------------|
| T-07-02-01 (DoS via crossing breakline) | `can_add_constraint` pre-check → `IntersectingConstraint`, no panic (tested) |
| T-07-02-02 (DoS huge input) | `MAX_POINTS` / `MAX_BREAKLINE_VERTICES` caps → `TooLarge` (tested) |
| T-07-02-03 (non-finite / degenerate) | `NonFinite` + `TooFewPoints` rejects before insert (tested) |
| T-07-02-04 (spade reaching engine) | `envi-dgm` has no `envi-engine` dep; `cargo tree` confirms |
| T-07-02-SC (spade legitimacy) | accepted per RESEARCH audit (2.15.1 OK/Approved, pure Rust, no C deps) |

## Verification Evidence
- `cargo test -p envi-dgm` → 11 passed, 0 failed.
- `cargo clippy --all-targets -- -D warnings` (whole workspace) → clean.
- `cargo fmt --check` → clean.
- `cargo test` (whole workspace) → all suites green (exit 0).
- `cargo tree -p envi-dgm -e normal --depth 1` → `spade` + `thiserror`, no `envi-engine`.
- `cargo tree -p envi-engine -e normal --depth 1` → exactly `ndarray` + `num-complex` + `thiserror`.
- `git diff --quiet crates/envi-engine/` → exit 0 (byte-identical).
- `cargo tree | grep -ci 'proj-sys\|gdal'` → 0 (zero C toolchain).
- No `unwrap()`/`panic!` in non-test code (grep-confirmed; only a doc-comment mentions them in prose).

## Next Phase Readiness
- TIN math seam is ready. Remaining Phase-7 wiring (not this plan): a `From<DgmError> for ApiError` arm (`envi-service/src/error.rs`) mapping `TooFewPoints`/`IntersectingConstraint`/`NonFinite`/`TooLarge` → 4xx, and the DGM HTTP handler that calls `build_tin`.
- Phase-8 terrain import extends the same `build_tin` seam with imported samples as additional vertices — no re-architecture needed.

## Self-Check: PASSED

- `crates/envi-dgm/Cargo.toml` — FOUND
- `crates/envi-dgm/src/lib.rs` — FOUND
- `crates/envi-dgm/src/tin.rs` — FOUND
- Commit `96fb0b7` — FOUND
- Commit `954bfc4` — FOUND

---
*Phase: 07-frontend-shell-scene-editing*
*Completed: 2026-07-10*
