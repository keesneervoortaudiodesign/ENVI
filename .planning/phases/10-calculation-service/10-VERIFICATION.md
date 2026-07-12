---
phase: 10-calculation-service
verified: 2026-07-12T01:38:32Z
status: passed
score: 4/4 success criteria verified
behavior_unverified: 0
overrides_applied: 0
re_verification: # none — initial verification
notes: >
  Execution model is CLIENT-SIDE threaded WASM (CONTEXT D-01 supersedes the ROADMAP's
  server-side/axum/SSE framing). All SCs verified against the relocated (browser) topology;
  the chunk format, memory bound, and "one solve path, N callers" invariant still hold.
minor_observations:
  - "SC4 'engine version' in the tensor-identity hash is a schema-version prefix (envi-tensor-hash-v1 / envi-marshalled-tensor-hash-v1), not the engine crate semver. Inherited from the Phase-6 frozen hash and deliberately preserved byte-identical (10-01 must_have). Not a gap: identity is fully deterministic over all scene inputs, verifiable, and the band axis IS covered (F=105 in CalcManifest.dims + N_BANDS-validated grids). Flagged only as a literal-wording nuance."
---

# Phase 10: Calculation Service — Verification Report

**Phase Goal:** A user can run a real Nord2000 calculation end-to-end — submit from the UI with a cost estimate, watch progress, abort cleanly — with the transfer tensor streamed to a chunked store inside a stated memory budget, and forests + semi-transparent screens/façades (ENG-09/10) computed with their effects present in the results. (SVC-02, GRID-02, WEB-07)
**Verified:** 2026-07-12T01:38:32Z
**Status:** passed
**Re-verification:** No — initial verification (verifies the FIXED tree: Gate-1 code-review WR-01..06 + HI-01, the shared-memory build fix, and the security pass all landed)

## Goal Achievement

### Observable Truths (ROADMAP SC1–SC4)

| # | Success Criterion | Status | Evidence |
|---|-------------------|--------|----------|
| SC1 | UI pre-run cost estimate (receiver count, tensor bytes, time) + guardrail warns on explosive spacing | ✓ VERIFIED | `envi_compute::cost::estimate` (cost.rs:81-120) + `guardrail` (cost.rs:149-186); u64 overflow-safe; native tests pass; in-browser Test 1 asserts REAL `estimate_cost` readout |
| SC2 | Live tiered progress + clean chunk-boundary abort → Cancelled, service healthy; Failed(reason) | ✓ VERIFIED | `pool::solve_tier` + `AtomicBool` cancel; native abort test passes; **Playwright Test 2 RUNS (not skipped) and passes** — threaded solve → coarse `done` → Abort → `cancelled`, tier kept |
| SC3 | rayon-parallel receiver-chunk streaming to chunked (OPFS) store within a bounded working set | ✓ VERIFIED | `pool::solve_tier` disjoint `par_iter` ranges + OPFS `TensorSink`; `sc3_peak_resident_chunks_never_exceed_n_workers` proves peak ≤ workers×chunk, full tensor never resident |
| SC4 | blake3 content-hash identity + ENG-09/10 + directional phase visible in results | ✓ VERIFIED | `marshalled_tensor_hash` (blake3, all scene fields) + `CalcManifest.dims[F=105]`; native bit-equivalence + forest/isolation/directional-phase tests all pass; `job_assembly` populates `directivity_phase_rad` |

**Score:** 4/4 success criteria verified (0 behavior-unverified — every behavior-dependent truth has a passing behavioral test, native and/or in-browser).

### Required Artifacts

| Artifact | Provides | Status | Details |
|----------|----------|--------|---------|
| `crates/envi-compute/src/cost.rs` | SC1 cost model + Warn/Block guardrail | ✓ VERIFIED | u64-saturating byte math; "halving spacing quadruples cost"; 5 unit tests pass |
| `crates/envi-compute/src/identity.rs` | SC4 blake3 `tensor_hash` + `CalcManifest` + `chunk_receivers` | ✓ VERIFIED | Frozen pre-refactor digest preserved; re-exported by envi-store |
| `crates/envi-compute/src/tiers.rs` | Hierarchical points ⊂ coarse ⊂ fine (D-05) | ✓ VERIFIED | 33 envi-compute tests pass |
| `crates/envi-compute/src/job_assembly.rs` | SolveJob assembly + SRC-03 directional-phase seam | ✓ VERIFIED | `eval_phase → directivity_phase_rad` gated on `has_phase()` (line 128-151); 0 conj |
| `crates/envi-compute-wasm/src/scene.rs` | `PreparedScene` + `solve_prepared_range` (real solve) | ✓ VERIFIED | Bit-equal to direct engine solve; ENG-09/10 + directional phase tests pass |
| `crates/envi-compute-wasm/src/pool.rs` | Caller-side rayon sharding + cancel + SC3 bound | ✓ VERIFIED | Disjoint files, cancel-writes-nothing, SC3 high-water tests pass |
| `crates/envi-compute-wasm/src/opfs_sink.rs` | OPFS-backed `TensorSink` | ✓ VERIFIED | Byte-exact round-trip vs InMemorySink passes |
| `crates/envi-compute-wasm/src/lib.rs` | `estimate_cost`/`plan_tiers`/`prepare_solve`/`solve_chunk_range` exports | ✓ VERIFIED | Real (non-Pending) `run_chunk_range`; hash-gate before solve |
| `crates/envi-service/src/api/mod.rs` | COOP same-origin + COEP credentialless layers | ✓ VERIFIED | `SetResponseHeaderLayer` (lines 132-138); contract test passes |
| `web/src/panels/CalcPanel.tsx` | Cost readout + guardrail + Run/Abort + per-tier progress + capability banner | ✓ VERIFIED | Playwright drives all `data-testid`s (calc-estimate/run/abort/status/tier-coarse) live |
| `web/src/compute/worker.ts` | Worker job machine (initThreadPool, tier loop, cancel) | ✓ VERIFIED | Test 2 in-browser threaded solve confirms pool init + tier loop + abort |
| `web/tests/e2e/calc.spec.ts` | Offline Playwright UAT of the real threaded bundle | ✓ VERIFIED | Both tests pass in-browser (Test 2 no longer skips) |

### Key Link Verification

| From | To | Via | Status |
|------|----|----|--------|
| `envi-store/src/lib.rs` | `envi-compute::identity` | `pub use envi_compute::identity::{tensor_hash, CalcManifest, chunk_receivers}` | ✓ WIRED (wire_no_drift + envi-store compile) |
| `job_assembly.rs` | `engine directivity` | `balloon.eval_phase(dir) → SolveJob.directivity_phase_rad` | ✓ WIRED (directional-phase test) |
| `opfs_sink.rs` | `engine TensorSink` | `impl TensorSink for OpfsChunkSink` | ✓ WIRED (round-trip test) |
| `pool.rs` | `engine solver::solve` | `ranges.par_iter() → per-range engine solve` | ✓ WIRED (two-shard bit-equal test) |
| `worker.ts` | `store/calc.ts` | `postMessage(JobStatus/TierComplete) → store` | ✓ WIRED (Playwright tier progress) |
| `api/mod.rs` | browser `crossOriginIsolated` | COOP/COEP header layers | ✓ WIRED (contract test + `self.crossOriginIsolated===true` in-browser) |

### Behavioral Spot-Checks (executed by this verifier)

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Compute core native tests | `cargo test -p envi-compute` | 33 passed, 0 failed | ✓ PASS |
| WASM crate native tests (bit-equiv, ENG-09/10, SC4, SC3, abort) | `cargo test -p envi-compute-wasm` | 31 passed, 0 failed | ✓ PASS |
| Isolation headers contract | `cargo test -p envi-service --test contract_isolation_headers` | 2 passed | ✓ PASS |
| Wire no-drift (ts-rs) | `cargo test -p envi-service --test wire_no_drift` | 2 passed, 1 writer ignored | ✓ PASS |
| Calc in-browser UAT (SC1 + SC2) | `npx playwright test calc.spec.ts` | 2 passed (Test 2 ran, not skipped) | ✓ PASS |
| Full offline Playwright suite | `npx playwright test` | **21 passed, 0 skipped, 0 failed** | ✓ PASS |
| Built threaded wasm imports SHARED memory | wasm import-section decode | `flags=0x03 (SHARED) min=18 max=16384` | ✓ PASS |

### Anti-Shallow / Cross-cutting Checks

| Check | Result | Status |
|-------|--------|--------|
| No Pending/stub/todo on the real solve path | `ComputeError::Pending` removed; grep clean (2 hits are a test string + a Phase-9 geometry comment) | ✓ PASS |
| Engine byte-identical | `cargo tree` = ndarray + num-complex + thiserror; `crates/envi-engine/src` untouched in phase range | ✓ PASS |
| conj gate in `propagation/` = 0 real calls | 9 grep hits are all doc comments describing the quarantine | ✓ PASS |
| conj in `job_assembly` = 0 | 0 | ✓ PASS |
| "Real calc end-to-end in browser" backed by an in-browser test | Playwright Test 2 (genuine threaded solve) passes in headless Chromium | ✓ PASS |
| Debt markers (TODO/FIXME/XXX) in phase source | none | ✓ PASS |

### Requirements Coverage

| Requirement | Description | Status | Evidence |
|-------------|-------------|--------|----------|
| SVC-02 | Compute-job model | ✓ SATISFIED | Client-side `JobStatus` machine in worker.ts + pool.rs; abort/failed/cancelled states, Playwright-observed |
| GRID-02 | Receiver-grid computation (parallel) | ✓ SATISFIED | `pool::solve_tier` rayon-parallel disjoint chunk ranges; SC3 bound test |
| WEB-07 | Submit/progress/abort UI | ✓ SATISFIED | CalcPanel + Playwright Test 1/2 (cost, Run gate, tiered progress, single-click abort, capability banner) |

### Gate Status (mandatory CLAUDE.md phase-completion gates)

| Gate | Result |
|------|--------|
| Code review (10-REVIEW.md) | `status: fixed` — HI-01 + WR-01..06 all committed |
| Security (10-SECURITY.md) | `SECURED` — every declared threat CLOSED or accepted |
| Verification (this doc) | `passed` |

## Minor Observation (non-blocking)

**SC4 literal "engine version" in the identity hash.** The roadmap SC4 lists "engine version" as one of the content-hash inputs. The implemented `tensor_hash` / `marshalled_tensor_hash` carry a *schema*-version prefix (`envi-tensor-hash-v1`), not the engine crate's semver, and `CalcManifest` has no `engine_version` field. This is inherited from the Phase-6 frozen hash and was deliberately preserved byte-identical (10-01 must_have: "byte-identical digests to the pre-refactor implementation"). It is **not counted as a gap**: the tensor identity is fully deterministic and verifiable over every scene input (geometry, met, receiver set, sub-sources+directivity, forest, isolation), the band axis IS captured (F=105 in `CalcManifest.dims` + N_BANDS-validated grids), and `every_tensor_affecting_field_changes_identity` proves coverage. If future engine-numerics changes must invalidate cached tensors, bumping the hash version prefix (or adding the engine semver to the prefix) is a one-line follow-up — recommend tracking as a Phase-11 note.

## Deferred (out of Phase-10 scope, per CONTEXT `<deferred>` — NOT gaps)

| Item | Addressed In | Note |
|------|-------------|------|
| Results rendering (spectra, isophone maps, color scale, difference maps) | Phase 11 | Phase 10 computes + emits tiered partial results; Phase 11 renders |
| Interactive fast-recalc / conditioning MAC | Phase 11 | Phase 10 produces + persists the tensor the MAC reads |
| Real authentication / login gate / sessions | Deferred phase (D-12) | Only COOP/COEP headers + bundle serving land here |
| GRID-03 L_den weather-class combination | Beyond Milestone 2 | Unmapped |
| OPFS quota / eviction strategy | Revisit if quota bites (D-09) | Persist-everything for now |

## Human Verification Required

None. Every behavior-dependent truth (chunk-boundary abort state transition, SC3 working-set invariant, directional-phase argument change) has a passing behavioral test — native `cargo test` and, for the end-to-end browser claim, the full offline Playwright suite (21 passed) executed by this verifier.

## Gaps Summary

No gaps. All four ROADMAP success criteria are delivered in real code and proven by passing tests — native bit-equivalence for the physics (ENG-09/10 + directional phase + marshalled == direct engine solve), the SC3 high-water working-set bound, the SC1 cost/guardrail with u64 overflow-safety, and an in-browser Playwright run (Test 2 no longer skips) that exercises the genuine threaded solve → tiered progress → cooperative abort → Cancelled. The shared-memory build fix is confirmed present in the committed artifact (imported memory flag `0x03`). Engine is byte-identical (3-dep quarantine intact); the `.conj()` quarantine holds. The only nuance is a literal-wording observation on "engine version" in the identity hash, explicitly not a gap.

---

_Verified: 2026-07-12T01:38:32Z_
_Verifier: Claude (gsd-verifier)_
