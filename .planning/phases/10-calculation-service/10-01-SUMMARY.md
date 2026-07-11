---
phase: 10-calculation-service
plan: 01
subsystem: compute-core
tags: [wasm, blake3, tensor-identity, cost-model, directivity, solve-job, rust]

# Dependency graph
requires:
  - phase: 04-transfer-tensor-directional-sources-full-validation
    provides: "envi_engine::solver::solve + SolveJob (directivity_gain_db/_phase_rad, forest, isolation), TensorSink/InMemorySink, BYTES_PER_CELL_PAIR/DEFAULT_TENSOR_BUDGET_BYTES, DirectivityBalloon::{eval, eval_phase, has_phase}"
  - phase: 06-service-foundation-persistence
    provides: "envi_store::hash::tensor_hash + manifest::{CalcManifest, chunk_receivers} + dto::{MetDto, ReceiverDto} + geojson::geometry_positions (the closure factored out here)"
provides:
  - "envi-compute pure-Rust core crate (WASM-safe: no std::fs, no C at runtime, no rayon)"
  - "envi_compute::identity — factored tensor_hash + CalcManifest + chunk_receivers + MetDto/ReceiverDto + geometry_positions (frozen encoding preserved, digest-pinned)"
  - "envi_compute::cost — estimate() + guardrail() (SC1 receiver/byte/time model + Ok/Warn/Block)"
  - "envi_compute::tiers — partition() hierarchical points ⊂ coarse ⊂ fine (D-05/D-06)"
  - "envi_compute::job_assembly — assemble_jobs() wiring SolveJob::directivity_phase_rad (SRC-03, first population site) + ENG-09/10 forest/isolation threading"
affects: [10-02-headers, 10-03-envi-compute-wasm, 10-04-pool-worker, 10-05-calcpanel, phase-11-results]

# Tech tracking
tech-stack:
  added: [] # no new external packages — all deps (blake3/geojson/uuid/ts-rs/serde) already in the workspace
  patterns:
    - "Pure-Rust WASM-safe core factored below the std::fs boundary (mirrors envi-gis); envi-store re-exports for source compatibility"
    - "Frozen-encoding regression: a fixed-input digest pins tensor_hash byte-for-byte across the move"
    - "Directional-phase seam wired at the SolveJob assembly site, gated on has_phase(), no conjugation in the assembly"

key-files:
  created:
    - crates/envi-compute/Cargo.toml
    - crates/envi-compute/src/lib.rs
    - crates/envi-compute/src/identity.rs
    - crates/envi-compute/src/cost.rs
    - crates/envi-compute/src/tiers.rs
    - crates/envi-compute/src/job_assembly.rs
  modified:
    - crates/envi-store/src/hash.rs
    - crates/envi-store/src/manifest.rs
    - crates/envi-store/src/dto.rs
    - crates/envi-store/src/geojson.rs
    - crates/envi-store/src/lib.rs
    - crates/envi-store/Cargo.toml

key-decisions:
  - "envi-compute uuid is serde-only (NOT v4): the identity closure parses/compares/serializes ids but never generates them (Pitfall 9); v4 pulls getrandom and breaks the wasm32 build — a deviation from the plan's literal 'uuid(v4+serde)' forced by the wasm-safe acceptance criterion"
  - "cost::estimate takes n_workers (5th arg) beyond the plan's 4-arg signature: both the SC3 working-set bound and the time extrapolation genuinely need it; t_pair is a documented tunable const (DEFAULT_T_PAIR_MS)"
  - "The TryFrom<&ReceiverDto> for Receiver impl moved to envi-compute (orphan rule) and returns IdentityError; a From<IdentityError> for StoreError in envi-store keeps re-exported call sites source-compatible"
  - "geometry_positions is re-exported pub from envi_store::geojson (was pub(crate)); a minor, plan-requested surface increase, no external consumer relied on its prior visibility"
  - "GRID-02/WEB-07 are phase-level requirements delivered across 10-02..10-05 (wasm crate, pool/worker, UI); this plan contributes the core but does NOT complete them — left Pending to keep REQUIREMENTS.md honest"

patterns-established:
  - "Factor-below-the-fs-boundary: pure format/hash/math into a WASM-safe core, native I/O stays in the owning crate, re-export for source compatibility"
  - "has_phase()-gated directional phase: Some(eval_phase) iff the balloon carries phase, else None → arg(H_coh) bit-identical"

requirements-completed: []  # GRID-02/WEB-07 span the whole phase; not completed by 10-01 alone (see key-decisions)

# Metrics
duration: ~45min
completed: 2026-07-11
status: complete
---

# Phase 10 Plan 01: envi-compute Pure-Rust Core Summary

**The WASM-safe compute core of Phase 10 — factored tensor identity, cost/guardrail, hierarchical tiers, and the SolveJob assembly that first wires the directional-phase seam (SRC-03) — landed as a pure-Rust crate with the engine byte-identical.**

## Performance

- **Duration:** ~45 min
- **Completed:** 2026-07-11
- **Tasks:** 3 (Task 3 via TDD RED→GREEN)
- **Files modified:** 12 (6 created, 6 modified)

## Accomplishments

- Created the `envi-compute` pure-Rust core crate that compiles for `wasm32-unknown-unknown` (no `std::fs`, no C at runtime, no `rayon`), so the exact FORCE-validated `envi_engine::solver::solve` path will run unchanged in the browser.
- Factored the tensor-identity closure (`tensor_hash` + `CalcManifest` + `chunk_receivers` + `MetDto`/`ReceiverDto` + `geometry_positions`) out of `envi-store` into `envi_compute::identity` **byte-for-byte** — a regression test pins the frozen pre-refactor digest (`dcb2485e…ee2c`); `envi-store` re-exports every moved item so its public API is unchanged.
- Added `envi_compute::cost` (SC1 receiver/tensor-byte/working-set/time estimate + `Ok/Warn/Block` guardrail that always states the exact "halving the spacing quadruples the cost" relation) and `envi_compute::tiers` (the hierarchical `points ⊂ coarse ⊂ fine` partition where coarse is a strict subset of fine and the fine tier lists only the gap points — D-05).
- Wired the **directional-phase seam SRC-03** at the SolveJob assembly site — the first construction site ever to populate `SolveJob::directivity_phase_rad` (from `DirectivityBalloon::eval_phase`, gated on `has_phase()`), with ENG-09/10 forest/isolation threaded so drawn objects are never silently inert.

## Task Commits

1. **Task 1: Scaffold envi-compute + factor the WASM-safe identity closure** — `83a89c9` (feat)
2. **Task 2: Cost model + guardrail (SC1) and hierarchical tier partition (D-05/D-06)** — `edd5289` (feat)
3. **Task 3: SolveJob assembly + directional-phase seam (SRC-03)** — `4f61dd8` (test, RED) → `b9f5efe` (feat, GREEN)

## Files Created/Modified

- `crates/envi-compute/Cargo.toml` — pure-core crate manifest; WASM-safe deps only (uuid serde-only, no v4).
- `crates/envi-compute/src/lib.rs` — crate root, `#![deny(unsafe_code)]`, module map.
- `crates/envi-compute/src/identity.rs` — factored `tensor_hash` (frozen encoding) + `CalcManifest` + `chunk_receivers` + `MetDto`/`ReceiverDto` + `geometry_positions` + `IdentityError`.
- `crates/envi-compute/src/cost.rs` — `estimate()`/`guardrail()` pure byte/receiver arithmetic (SC1).
- `crates/envi-compute/src/tiers.rs` — `partition()` hierarchical subset-preserving tier plan (D-05).
- `crates/envi-compute/src/job_assembly.rs` — `assemble_jobs()` + `SolveCtx`; wires `directivity_phase_rad` (SRC-03) and ENG-09/10.
- `crates/envi-store/src/hash.rs` — now a thin `pub use envi_compute::identity::tensor_hash;` re-export.
- `crates/envi-store/src/manifest.rs` — re-exports `CalcManifest`/`chunk_receivers`; keeps the `std::fs` write/read.
- `crates/envi-store/src/dto.rs` — re-exports `MetDto`/`ReceiverDto`; `Receiver` import dropped.
- `crates/envi-store/src/geojson.rs` — re-exports `geometry_positions`; local copy removed.
- `crates/envi-store/src/lib.rs` — `From<IdentityError> for StoreError`; blake3 no longer used directly.
- `crates/envi-store/Cargo.toml` — adds `envi-compute` path dep; drops the now-transitive `blake3`.

## Deviations from Plan

### Auto-fixed / auto-adjusted (Rules 1-3)

**1. [Rule 3 - Blocking] `uuid` `v4` feature breaks the wasm32 build**
- **Found during:** Task 1 (`cargo build -p envi-compute --target wasm32-unknown-unknown`).
- **Issue:** The plan specified `uuid (v4+serde)`; on `wasm32-unknown-unknown` the `v4` feature demands a randomness source (`getrandom`) and fails to compile — directly contradicting the plan's own "compiles for wasm32" must-have.
- **Fix:** Enabled only `serde` on `uuid`. The identity closure parses/compares/serializes uuids but never generates them (10-RESEARCH Pitfall 9 — ids are minted in TS), so `v4` was never needed.
- **Files:** `crates/envi-compute/Cargo.toml`.
- **Commit:** `83a89c9`.

**2. [Rule 3 - Blocking] Orphan rule on the moved `ReceiverDto` conversion**
- **Found during:** Task 1.
- **Issue:** Moving `ReceiverDto` to `envi-compute` made `impl TryFrom<&ReceiverDto> for Receiver` an orphan in `envi-store` (both types foreign).
- **Fix:** Moved the impl into `envi-compute` returning a new `IdentityError`; added `From<IdentityError> for StoreError` in `envi-store` so `geometry_positions`' `?` and any re-exported call site stay source-compatible. The impl is unused by any call site (verified).
- **Files:** `crates/envi-compute/src/identity.rs`, `crates/envi-store/src/lib.rs`.
- **Commit:** `83a89c9`.

### Interface adjustment

**3. [Interface] `cost::estimate` gained an `n_workers` parameter**
- The plan's `estimate(area_m2, spacing_fine_m, discrete_points, n_sub)` cannot compute the working-set bound or the time extrapolation (both formulas in 10-RESEARCH require the worker count) without it. Added `n_workers` as a 5th argument rather than hide a dishonest default; `DEFAULT_T_PAIR_MS` is a documented tunable const. Commit `edd5289`.

### Scope note

**4. GRID-02 / WEB-07 not marked complete.** The plan frontmatter lists them, but they are phase-level requirements delivered across plans 10-02..10-05 (COOP/COEP headers, the `envi-compute-wasm` cdylib + OPFS sink, the rayon pool + Web Worker, the CalcPanel UI). 10-01 delivers only the core; `REQUIREMENTS.md` traceability is left `Pending` to stay honest. The ROADMAP `10-01-PLAN.md` checkbox is ticked.

## Quality Gates (all green)

- `cargo build -p envi-compute --target wasm32-unknown-unknown` — succeeds (WASM-safe).
- `cargo tree -p envi-engine` — exactly `ndarray + num-complex + thiserror`; `git diff --stat crates/envi-engine` empty (engine byte-identical, D-02).
- `cargo tree -p envi-compute` — no `tempfile`, no runtime C-linked crate, no `rayon`, no `getrandom` (the `cc` seen is blake3's build-time SIMD dep; the wasm build proves blake3 needs no C there).
- `cargo test` (whole workspace) — all pass, including the frozen-digest regression, the cost 4×/guardrail tests, the tier subset/disjoint/contiguous-index tests, and the directional-phase rotation + bit-identical baseline tests.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo fmt --check` — clean.
- `grep -c conj crates/envi-compute/src/job_assembly.rs` — 0 (no conjugation in the assembly); `grep eval_phase` ≥ 1, `grep directivity_phase_rad` ≥ 1.

## Known Stubs

None. Every module is fully implemented and unit-proven. (The wasm cdylib, OPFS sink, pool, worker, and UI are separate downstream plans 10-02..10-05, not stubs of this plan.)

## Self-Check: PASSED

- All six created files exist on disk; all six modified files present.
- Commits `83a89c9`, `edd5289`, `4f61dd8`, `b9f5efe` all in `git log`.
