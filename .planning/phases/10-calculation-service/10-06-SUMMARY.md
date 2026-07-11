---
phase: 10-calculation-service
plan: 06
subsystem: compute
tags: [wasm, rayon, opfs, nord2000, ts-rs, serde, directivity, forest, isolation]

# Dependency graph
requires:
  - phase: 10-calculation-service
    provides: "10-01 envi-compute core (identity, SolveCtx/assemble_jobs), 10-03 cdylib + OPFS TensorSink + threaded toolchain, 10-04 pool::solve_tier rayon driver + Web Worker job machine + calc store"
  - phase: 01-08
    provides: "envi-engine solver::solve, DirectivityBalloon (phase seam), ForestCrossing (ENG-09), IsolationSpectrum (ENG-10), the scene DTOs + interpolate core in envi-store/envi-gis-wasm"
provides:
  - "prepare_solve: marshals the whole transfer scene ONCE per submit into an owned PreparedScene keyed by tensor_hash"
  - "REAL solve_chunk_range: hash-gated, cancel-aware, rayon-sharded range solve writing one OPFS chunk file pair (no Pending stub)"
  - "PrepareSolveReq + engine-type marshalling DTOs (atmosphere/coherence/directivity/receiver/sub-source), all ts-rs-generated"
  - "WASM-safe scene DTOs (terrain/ground/isolation/forest/sound-speed) + interpolate core factored into envi-compute; re-exported at their original paths"
  - "OPFS runtime: hoisted async createSyncAccessHandle keyed by tensor_hash ahead of the synchronous solve"
affects: [11-results-readout, calc-panel-ui]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Factor-and-re-export (10-01 precedent) extended to the scene DTOs + interpolate core: one wire type, source-compatible public API, byte-stable wire.ts"
    - "Owned scene + with_local_ctx/ctx_with: no self-referential struct; borrows built inside the call"
    - "Shard-assemble over the unchanged engine solve (Pattern 1): rayon pool via pool::solve_tier, one s-major chunk per file"
    - "Hoisted async-open registry: preopenChunk (async) → synchronous openChunk lookup → closeChunk evict"

key-files:
  created:
    - crates/envi-compute/src/scene_dto.rs
    - crates/envi-compute/src/interpolate.rs
    - crates/envi-compute-wasm/src/scene.rs
  modified:
    - crates/envi-compute-wasm/src/lib.rs
    - crates/envi-compute-wasm/src/dto.rs
    - crates/envi-store/src/dto.rs
    - crates/envi-store/src/interpolate.rs
    - crates/envi-store/src/lib.rs
    - crates/envi-gis-wasm/src/dto.rs
    - crates/envi-service/tests/wire_no_drift.rs
    - web/src/generated/wire.ts
    - web/src/compute/opfs.ts
    - web/src/compute/worker.ts

key-decisions:
  - "solve_prepared_range returns the assembled (H_coh, P_incoh) tensors; lib.rs owns the OPFS write — decouples the natively-testable solve from the wasm-only sink and the pool-gated RangeSink trait"
  - "solve_prepared_shards is cfg-split: rayon pool::solve_tier sharding on native/threaded, a sequential single solve on the stable single-threaded wasm32 smoke build (bit-identical — receivers are independent)"
  - "envi-store re-exports interpolate as a thin StoreError-preserving wrapper (map_err) so server call sites keep using `?` into StoreError; SceneDtoError maps into StoreError via From"
  - "ENG-10 isolation is only valid on a screen path; the observability test uses a thin-screen cut-plane profile (the engine rejects isolation over flat terrain by design)"

patterns-established:
  - "Prepared-scene registry: a module-level static RwLock<Option<PreparedScene>> (shared across wasm-bindgen-rayon pool threads via linear memory, never thread_local)"
  - "OPFS async-open hoist: worker preopens both channel handles before the synchronous solve; Rust closes on finish, the worker releases any leaked handle on early error"

requirements-completed: [GRID-02, SVC-02]

# Metrics
duration: ~90min
completed: 2026-07-11
status: complete
---

# Phase 10 Plan 06: Close the solve_chunk_range seam Summary

**The client-side solve is now REAL: prepare_solve marshals the whole transfer scene once per submit into an owned PreparedScene, and solve_chunk_range runs the UNCHANGED engine solve rayon-sharded into OPFS — the marshalled range-solve tensor is f64::to_bits-equal to a direct envi_engine::solver::solve (with forest, isolation, and directional phase).**

## Performance

- **Duration:** ~90 min
- **Completed:** 2026-07-11
- **Tasks:** 3
- **Files modified:** 17 (3 created)

## Accomplishments
- Replaced the `ComputeError::Pending` stub in `solve_chunk_range` with a real, hash-gated, cancel-aware range solve that writes one OPFS chunk file pair per `chunk_index`.
- Added `prepare_solve` + the `PrepareSolveReq` marshalling contract (atmosphere / coherence / directivity balloon+phase / receiver / sub-source DTOs), all ts-rs-generated into the committed `wire.ts` with the no-drift test green and no duplicate `SoundSpeedProfileDto`.
- Factored the WASM-safe scene DTOs (terrain/ground/authored-isolation/forest/sound-speed) + the band-index `interpolate` core out of `envi-store`/`envi-gis-wasm` into `envi-compute`, re-exported at their original paths (source-compatible, wire.ts byte-stable).
- The load-bearing correctness gate passes: the marshalled range-solve is bit-equal to a direct engine solve — proven with a forest (ENG-09) + isolation spectrum (ENG-10) + a phase-carrying directivity balloon (SC4), and a phase-free baseline staying bit-identical; a two-shard assembly equals a single-range solve bit-for-bit; the hash-mismatch guard is a typed error.
- Wired the worker: `prepare_solve(spec.scene)` once per submit before the (unchanged) tier loop, and the hoisted async `createSyncAccessHandle` registry keyed by `tensor_hash` ahead of the synchronous solve.

## Task Commits

1. **Task 1: Scene marshalling contract — factor WASM-safe scene DTOs + PrepareSolveReq** - `ac6478a` (feat)
2. **Task 2: prepare_solve + REAL solve_chunk_range + native bit-equivalence gate** - `6da94ed` (feat)
3. **Task 3: OPFS runtime open + worker prepare_solve wiring** - `6a8b30f` (feat)

## Files Created/Modified
- `crates/envi-compute/src/scene_dto.rs` - WASM-safe terrain/ground/isolation/forest/sound-speed DTOs + `SceneDtoError`, moved from envi-store/envi-gis-wasm.
- `crates/envi-compute/src/interpolate.rs` - The band-index interpolation core, moved from envi-store (returns `SceneDtoError`).
- `crates/envi-compute-wasm/src/scene.rs` - `PreparedScene` (built via validating engine constructors) + `solve_prepared_range` (rayon shard-assemble / sequential fallback) + the native equivalence/ENG-09/10/SC4/two-shard tests.
- `crates/envi-compute-wasm/src/lib.rs` - `prepare_solve` export, the static `RwLock<Option<PreparedScene>>` registry, the real `solve_chunk_range` + `run_chunk_range` + `write_chunk_to_opfs`; removed `ComputeError::Pending`, added `Prepare`/`NotPrepared`/`HashMismatch`.
- `crates/envi-compute-wasm/src/dto.rs` - `PrepareSolveReq` + AtmosphereDto/CoherenceInputsDto/ReceiverPlacementDto/SubSourcePlacementDto/DirectionalDto/DirectivityBalloonDto/RotationDto/RangeProgressDto.
- `crates/envi-store/src/{dto,interpolate,lib}.rs` - Re-export the moved DTOs + a StoreError-preserving `interpolate` wrapper + `From<SceneDtoError> for StoreError`.
- `crates/envi-gis-wasm/src/dto.rs` + `Cargo.toml` - Re-export `SoundSpeedProfileDto` from envi-compute (new envi-compute dep edge).
- `crates/envi-service/tests/wire_no_drift.rs` + `web/src/generated/wire.ts` - Register + regenerate the 9 new wire types.
- `web/src/compute/opfs.ts` - `setActiveProject` + preopen registry (`preopenChunk`/synchronous `openChunk`/`closeChunk` evict/`releasePreopenedChunk`).
- `web/src/compute/worker.ts` - `ComputeWasm.prepare_solve` + `CalcJobSpec.scene`; the prepare_solve call + the glue OPFS hoist.

## Decisions Made
- **solve_prepared_range returns tensors, lib.rs writes OPFS.** The plan sketched `solve_prepared_range(scene, range, sink: &mut impl RangeSink)`. `RangeSink` is defined in `pool` (gated `any(not wasm32, threads)`), so referencing it would not compile on the stable single-threaded wasm32 smoke build. Returning the assembled `(H_coh, P_incoh)` and having `lib.rs` open the concrete `OpfsChunkSink` decouples the natively-testable core from the wasm-only sink and keeps the equivalence test sink-free. Same guarantee ("bit-equal to a direct engine solve"), cleaner cfg story.
- **cfg-split sharding.** `solve_prepared_shards` shards via `pool::solve_tier` on native/threaded builds (GRID-02 rayon-parallel per call, SC3 residency reused) and does one sequential `solve()` on the stable wasm32 build (no rayon there) — bit-identical because receivers are independent.
- **Isolation observability uses a screen profile.** `terrain_effect` returns `IsolationWithoutScreen` for an isolation spectrum over flat terrain (correct — a partition with no partition is contradictory). The ENG-10 test therefore uses a thin-screen cut-plane profile.

## Deviations from Plan

### Auto-fixed / adjusted

**1. [Rule 3 - Blocking] solve_prepared_range signature returns tensors instead of taking `&mut impl RangeSink`**
- **Found during:** Task 2 (scene.rs + lib.rs)
- **Issue:** The plan's `sink: &mut impl RangeSink` signature could not compile on the stable single-threaded `wasm32` build — `RangeSink` lives in the `pool` module which is `#[cfg]`-excluded there, yet `solve_chunk_range` (a `#[wasm_bindgen]` export) must still compile on that build.
- **Fix:** `solve_prepared_range` returns the assembled `(Array3<Complex<f64>>, Array3<f64>)`; `lib.rs::write_chunk_to_opfs` performs the concrete `OpfsChunkSink` `put_chunk` + `finish`. The behaviour (bit-equal to a direct engine solve, one file per chunk_index) is unchanged; the equivalence tests compare returned tensors to a direct solve.
- **Files modified:** crates/envi-compute-wasm/src/scene.rs, crates/envi-compute-wasm/src/lib.rs
- **Verification:** `scene::tests::marshalled_range_solve_is_bit_equal_to_a_direct_engine_solve` (+ ENG-09/10, SC4, two-shard) pass; stable wasm32 build compiles.
- **Committed in:** `6da94ed`

**2. [Rule 2 - Missing critical] Added RangeProgressDto wire type**
- **Found during:** Task 2 (the real solve_chunk_range needs a structured success return)
- **Issue:** `solve_chunk_range` now returns a real `RangeProgress` (must-have) crossing the wasm boundary; a structured value crossing the boundary is a wire type (D-10).
- **Fix:** Added `RangeProgressDto { chunk_index, receivers }` to `dto.rs`, registered it in `wire_no_drift`, regenerated `wire.ts`. (The worker still treats the return as `unknown`.)
- **Files modified:** crates/envi-compute-wasm/src/dto.rs, crates/envi-service/tests/wire_no_drift.rs, web/src/generated/wire.ts
- **Verification:** wire no-drift green; `solve_chunk_range_registry_hash_gate` asserts the returned progress.
- **Committed in:** `6da94ed`

**3. [Rule 3 - Blocking] calc.test.ts fixtures required the new CalcJobSpec.scene field**
- **Found during:** Task 3 (adding `scene` to CalcJobSpec)
- **Issue:** `CalcJobSpec` gained a required `scene` field; the existing 10-04 `calc.test.ts` spec fixtures did not compile under `tsc`.
- **Fix:** Added a `minimalScene()` helper and threaded `scene` into both fixtures (the store forwards the spec to a mocked submit — content is irrelevant).
- **Files modified:** web/src/store/calc.test.ts
- **Verification:** `npx tsc --noEmit` clean; `npm run test:unit` (26 tests) green.
- **Committed in:** `6a8b30f`

---

**Total deviations:** 3 (1 blocking-signature, 1 missing-critical wire type, 1 blocking test-fixture) — all necessary for correctness / compilation. No scope creep; engine byte-identical.

## Issues Encountered
- The `envi-store` interpolate/DTO error type could not move to `envi-compute` while keeping `StoreError` on the public path. Resolved with a `SceneDtoError` in `envi-compute` + a `From<SceneDtoError> for StoreError` in `envi-store` and a thin StoreError-preserving `interpolate` wrapper — mirrors the 10-01 `IdentityError` precedent. `envi-gis-wasm` gained an `envi-compute` dep to re-export the shared `SoundSpeedProfileDto`.

## Quality Gates (all green)
- `cargo clippy --all-targets -- -D warnings` — clean (exit 0)
- `cargo fmt --check` — clean (exit 0)
- `cargo test` — 56 test binaries pass, 0 failed (incl. `scene::` equivalence + ENG-09/10 + directional-phase + hash guard, and `wire_no_drift`)
  - PASS: `test scene::tests::marshalled_range_solve_is_bit_equal_to_a_direct_engine_solve ... ok`
- `cargo build -p envi-compute-wasm --target wasm32-unknown-unknown` (stable) — compiles
- `npm run build:wasm:compute` (nightly-2026-07-11 threaded) — exit 0 (real solve wired; bindings expose `prepare_solve`)
- `cargo tree -p envi-engine` — direct deps exactly ndarray + num-complex + thiserror; `git diff --stat crates/envi-engine` empty (byte-identical)
- conj gate: 0 `.conj()` in `crates/envi-engine/src/propagation` source
- no-Pending gate: 0 `Pending`/`todo!`/`unimplemented!`/`placeholder` on the real solve path (lib.rs/scene.rs)
- `cd web && npx tsc --noEmit` clean; `npm run test:unit` 26 pass; ts-rs wire no-drift green (no duplicate `SoundSpeedProfileDto`)

## Next Phase Readiness
- The client-side solve is real end-to-end (GRID-02 rayon-parallel compute, SVC-02 compute-job model delivered). Phase 11 (results readout) can read the OPFS chunk pairs written per `chunk_index` keyed by `tensor_hash`.
- Not in scope here (10-05): the CalcPanel UI that constructs a production `CalcJobSpec.scene` from the drawn scene + the offline Playwright UAT of the real threaded bundle.
- The directional-phase seam (`SolveJob::directivity_phase_rad`) is now populated from a marshalled balloon — the first production construction site beyond the job_assembly unit tests.

---
*Phase: 10-calculation-service*
*Completed: 2026-07-11*

## Self-Check: PASSED

All created files exist on disk; all three task commits are in git history.
