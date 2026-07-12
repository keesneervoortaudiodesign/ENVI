---
phase: 10-calculation-service
reviewed: 2026-07-12T00:00:00Z
depth: deep
files_reviewed: 41
files_reviewed_list:
  - crates/envi-compute/Cargo.toml
  - crates/envi-compute/src/cost.rs
  - crates/envi-compute/src/identity.rs
  - crates/envi-compute/src/interpolate.rs
  - crates/envi-compute/src/job_assembly.rs
  - crates/envi-compute/src/lib.rs
  - crates/envi-compute/src/scene_dto.rs
  - crates/envi-compute/src/tiers.rs
  - crates/envi-compute-wasm/Cargo.toml
  - crates/envi-compute-wasm/src/dto.rs
  - crates/envi-compute-wasm/src/lib.rs
  - crates/envi-compute-wasm/src/opfs_sink.rs
  - crates/envi-compute-wasm/src/pool.rs
  - crates/envi-compute-wasm/src/scene.rs
  - crates/envi-gis-wasm/Cargo.toml
  - crates/envi-gis-wasm/src/dto.rs
  - crates/envi-service/Cargo.toml
  - crates/envi-service/src/api/mod.rs
  - crates/envi-service/tests/contract_isolation_headers.rs
  - crates/envi-service/tests/wire_no_drift.rs
  - crates/envi-store/Cargo.toml
  - crates/envi-store/src/dto.rs
  - crates/envi-store/src/geojson.rs
  - crates/envi-store/src/hash.rs
  - crates/envi-store/src/interpolate.rs
  - crates/envi-store/src/lib.rs
  - crates/envi-store/src/manifest.rs
  - crates/README.md
  - web/package.json
  - web/README.md
  - web/vite.config.ts
  - web/src/App.tsx
  - web/src/compute/client.ts
  - web/src/compute/cost.ts
  - web/src/compute/opfs.ts
  - web/src/compute/worker.ts
  - web/src/compute/worker.test.ts
  - web/src/panels/CalcPanel.tsx
  - web/src/store/calc.ts
  - web/src/store/calc.test.ts
  - web/tests/e2e/calc.spec.ts
findings:
  critical: 0
  warning: 6
  info: 4
  total: 10
status: fixed
fix_pass:
  fixed_at: 2026-07-12
  fixed: [HI-01, WR-01, WR-02, WR-03, WR-04, WR-05, WR-06]
  accepted: [IN-01, IN-02, IN-03, IN-04]
  commits:
    HI-01: ab38609 (+ style 78bd45b)
    WR-01: 59823ae
    WR-02: 8e205a1
    WR-03: ec2a270
    WR-04: 5f7803b
    WR-05: c91b4a4
    WR-06: 6a9c56e
    dist: 718d30e
  gates: cargo fmt/clippy/test green; envi-engine byte-identical (empty diff, deps
    = ndarray+num-complex+thiserror); wasm32 stable build ok; build:wasm:compute
    (nightly-2026-07-11) ok; tsc clean; wire no-drift green; npm test:unit 26/26;
    playwright 21/21 (calc.spec Test 1 + Test 2 pass).
---

# Phase 10: Code Review Report

**Reviewed:** 2026-07-12
**Depth:** deep (cross-file, call-chain, invariant verification)
**Files Reviewed:** 41
**Status:** issues_found

## Summary

Phase 10 is high-quality, heavily-tested code. The engine quarantine holds (envi-engine
byte-identical, `.conj()` grep gate 0, OPFS sink is a fresh `TensorSink` impl), the one-solve-path
invariant is respected (`pool::solve_tier` and the sequential fallback both call the unchanged
`envi_engine::solver::solve`), the band-index rule is honoured everywhere, the wire DTOs are all
`ts_rs`-generated with a no-drift test and no hand-authored TS mirror, and the OPFS path guard
(`safeSeg` + `assertHex`) correctly rejects traversal. The `static RwLock<Option<PreparedScene>>`
concurrency is sound: it is read-locked during the sharded solve, the rayon workers never touch the
lock, and a panic under a read guard does not poison it. The marshalled range-solve is proven
bit-equal to a direct engine solve.

No Critical defects were found. The findings below are a **High-severity identity-contract violation
in the UI marshalling** (an ad-hoc 32-bit hash used where the blake3 `tensor_hash` is mandated),
two **Medium** robustness/numeric-safety gaps, and several Low/Info items. Nothing here is a security
vulnerability in the classic sense (the whole surface is same-origin, client-side, worker-driven), but
the identity finding is a real data-integrity risk on the D-09 tensor-reuse path.

I did **not** re-flag the disclosed accepted risks (non-shared `WebAssembly.Memory` blocking the
in-browser threaded solve; the `usize` cost-overflow note) as *new* — but WR-02 sharpens the overflow
note with its exact trigger condition, and IN-01 records that the end-to-end threaded solve remains
unverified in a real browser.

---

## High

### HI-01: CalcPanel keys the OPFS tensor store with an ad-hoc 32-bit FNV hash, not the mandated blake3 `tensor_hash`

**File:** `web/src/panels/CalcPanel.tsx:71-78` (the `hexHash` helper) and `:248` (its use as `tensorHash`)

**Issue:** The whole Phase-10 identity design (D-09, and the `envi_compute::identity::tensor_hash`
closure that 10-01 factored out specifically to be WASM-safe) mandates that the OPFS chunk store and
manifest be keyed by the **blake3 tensor-identity hash over geometry + met + receivers**. CalcPanel
instead invents its own key:

```ts
function hexHash(input: string): string {
  let h = 2166136261 >>> 0;               // 32-bit FNV-1a
  ...
  return h.toString(16).padStart(8, "0"); // 8 hex chars = 32 bits
}
...
const tensorHash = hexHash(`${spacingM}|${Math.round(inputs.areaM2)}|${nSub}|${total}`);
```

This is a **32-bit** hash over only four scalars, and it never routes through
`envi_compute::identity::tensor_hash` at all. It is used as the OPFS directory key
(`projects/<id>/calc/<hash>/…`) and as the `PrepareSolveReq.tensor_hash` / `SolveChunkRangeReq.tensor_hash`
that the `HashMismatch` gate compares.

**Failure scenario:** D-09 says an identical re-run *reuses* the hash-keyed tensor. Two genuinely
different calc setups whose `(spacing, round(area_m²), n_sub, total)` tuples collide under the 32-bit
FNV map to the **same** `calc/<hash>/` directory. Run B (`create:true`) overwrites Run A's chunk
files; a later D-09 "reuse" of A then reads B's tensor — silently wrong acoustic results attributed
to scene A. Birthday collisions are plausible within a single project's edit history (~77k distinct
grids for a 50% collision chance on 32 bits). The `HashMismatch` guard does **not** catch this: it
only checks that the client passed the same (colliding) hash to `prepare_solve` and
`solve_chunk_range`.

It is currently *masked* only because the marshalled scene is a flat corridor fully determined by
those four scalars, so today a collision maps two scenes that happen to produce byte-identical
tensors → the *bytes* are the same. The moment Phase 11 derives real per-corridor terrain/impedance
from the drawn polygon (explicitly the documented next step, module header lines 18-21), the key
stops covering the scene geometry and the collision becomes an active mis-serve. This also violates
the "one source of truth / drift made structurally impossible" invariant the phase is built around.

**Fix:** Compute the OPFS/manifest key from the real identity closure — expose
`envi_compute::identity::tensor_hash` (or a thin wasm wrapper over it) and call it from CalcPanel over
the actual marshalled scene + receivers, instead of `hexHash`. At minimum, if a placeholder key is
retained for the flat-corridor stage, widen it to the full blake3 digest and include every field that
affects the tensor (receiver positions, sub-source positions, terrain, met) so it cannot under-key.

**Resolution (2026-07-12) — FIXED (commit ab38609).** Took the "full blake3 digest" path
(`envi_compute::identity::tensor_hash` hashes a GeoJSON `FeatureCollection`, which the flat-corridor
`PrepareSolveReq` is not — forcing it would under-key terrain/atmosphere/coherence). New
`envi-compute-wasm::identity::marshalled_tensor_hash` computes a blake3 digest over EVERY
tensor-affecting field of the marshalled scene (terrain, atmosphere, coherence, weather, sub-sources +
directivity, receiver positions, forest, isolation, `n_sub`; the `tensor_hash` field itself excluded),
using the same frozen byte-encoding discipline as `envi_compute::identity` under its own
`envi-marshalled-tensor-hash-v1` version tag. Exposed as a new `#[wasm_bindgen] tensor_hash(req)` boundary
export; CalcPanel now derives the OPFS key from it and the FNV `hexHash` is deleted. Two distinct scenes
with the same `(spacing, area, n_sub, total)` tuple now get distinct 64-hex keys (proven by
`colliding_scalar_tuple_scenes_get_distinct_hashes`). No new wire DTO (returns a plain string), so
wire_no_drift stays green.

---

## Warnings

### WR-01: `solve_chunk_range` can panic (slice out-of-bounds) despite its documented "never panics on data" contract

**File:** `crates/envi-compute-wasm/src/scene.rs:454-480` (shard slicing) via `local_receivers`
(`:310-325`); boundary contract asserted in `crates/envi-compute-wasm/src/lib.rs:281` and the
module/threat docs (T-10-03-02 / T-10-06-03/04).

**Issue:** `solve_prepared_shards` shards on the request's `len` but slices the *actual* selected
receivers:

```rust
let chunk_receivers = scene.local_receivers(r_offset, len); // may return k < len
...
while lo < len { ... shard_len based on `len` ... }          // shards cover 0..len
...
chunk_receivers[s.r_offset..s.r_offset + s.len]              // panics if k < len
```

`local_receivers` returns only the receivers whose `global_index` falls in `[r_offset, r_offset+len)`.
If the prepared scene's receivers do not densely cover that global range, `k < len` and the shard
slice `chunk_receivers[..]` indexes out of bounds → panic. In wasm (`panic = abort`) this traps the
whole module, bricking the compute worker until reload — directly contradicting the boundary's stated
"never panics on data" guarantee.

**Failure scenario:** A `SolveChunkRangeReq { r_offset, len }` whose `len` exceeds the prepared
receiver coverage (e.g. `prepare_solve` with 2 receivers, then `solve_chunk_range` with `len: 5`).
The `HashMismatch` gate passes (hash matches), so the request reaches the shard slice and panics.
Reachability is **low** in normal operation — the worker always derives `r_offset`/`len` from the same
deterministic `plan_tiers`, so tier-internal indices are contiguous and `k == len` — but the boundary
is a `pub #[wasm_bindgen]` entry point that explicitly promises not to panic on data.

**Fix:** Validate the range against the prepared scene before sharding: return
`ComputeError::Prepare`/a typed error when `local_receivers(r_offset, len).len() != len` (or clamp the
shard plan to `chunk_receivers.len()`), so a malformed range is a typed `JsValue` error rather than a
wasm trap.

**Resolution (2026-07-12) — FIXED (commit 59823ae).** Added `ComputeError::Range { r_offset, len,
covered }` and `PreparedScene::local_receiver_count` (a cheap filter, no allocation).
`solve_prepared_range` validates `covered == len` BEFORE any sharding/slicing and returns the typed
`Range` error on a malformed request — a `JsValue` error, never a wasm trap. Test
`range_exceeding_receiver_coverage_is_a_typed_error_not_a_panic` covers both the over-long and the
past-the-end sparse case.

### WR-02: `envi_compute::cost` byte math overflows `usize` on wasm32 and can silently defeat the SC1 `Block` guardrail once a real quota is passed

**File:** `crates/envi-compute/src/cost.rs:93-94` (`tensor_bytes = receiver_count * per_receiver`),
guardrail comparison at `:141`.

**Issue:** On `wasm32`, `usize` is 32-bit. `tensor_bytes = receiver_count * per_receiver` (and
`working_set_bytes` at `:97`) are `usize` multiplications with no widening. In a wasm release build
(overflow checks off) an extreme grid wraps `tensor_bytes` to a small value; the guardrail then
compares the wrapped value against `budget_bytes` (`:141`) and returns `Ok`/`Warn` instead of `Block`,
and the DTO reports a wrong (tiny) tensor size to the UI.

This is **currently masked**: CalcPanel passes a fixed `BUDGET_BYTES = 2 GiB` (`CalcPanel.tsx:48`),
which is below `u32::MAX` (~4.29 GiB), so `Block` always fires at ~2 GiB *before* `receiver_count`
grows enough to overflow `tensor_bytes`. **The trap is latent:** `web/src/compute/opfs.ts` already
exports `estimateQuota`/`fitsQuota` for exactly the "use the real OPFS quota as the budget" path
(D-09/SC1). Desktop OPFS quotas are routinely > 2 GiB. The first time the real quota is wired in as
`budget_bytes` on wasm32, a large multi-source grid (e.g. ~426k receivers at `n_sub = 4`) overflows
`tensor_bytes` and the hard `Block` — the exact runaway-grid stop SC1 exists to provide — is silently
bypassed, with the actual solve then free to exhaust OPFS. This refines (not contradicts) the 10-03
accepted-risk note, which framed it as not affecting "the typical grid regime."

**Fix:** Do the byte arithmetic in `u64`/`f64` inside `cost::estimate` (the boundary DTO already
carries these as `f64`), or use `checked_mul` and saturate to `usize::MAX` so an overflowing grid is
forced to `Block`, never wrapped under budget. This is a small change to the frozen core, justified by
the guardrail being safety-critical.

**Resolution (2026-07-12) — FIXED (commit 8e205a1).** `CostEstimate::{tensor_bytes, working_set_bytes}`
are now `u64`, computed via `saturating_mul` so an overflowing grid saturates to `u64::MAX` and always
trips `Block`; `guardrail` takes `budget_bytes: u64` and `WARN_TENSOR_BYTES` is `u64`. The boundary DTO
still carries bytes as JS-safe `f64` (`est.tensor_bytes as f64`). New test
`large_grid_byte_math_does_not_overflow_u32_and_still_blocks` proves a >u32::MAX-byte grid (500k
receivers × 10 080 B ≈ 5.04 GB) still Blocks against the 2 GiB budget.

### WR-03: OPFS chunk files are reopened `create:true` and written from offset 0 but never truncated — stale trailing bytes on reuse

**File:** `web/src/compute/opfs.ts:110-121` (`openChunkHandle`, `create:true`, no `truncate`) and
`crates/envi-compute-wasm/src/opfs_sink.rs:157-234` (`put_chunk` writes at advancing offsets, no
`truncate`).

**Issue:** The sink writes each chunk at advancing byte offsets and closes; it never calls the
handle's `truncate()`. On the D-09 reuse path a chunk file is reopened with `create:true`. If a later
run writes *fewer* bytes into `chunk_<idx>.bin` than a prior run left there (a shorter span for the
same `chunk_index`), the stale trailing bytes from the previous run remain in the file.

**Failure scenario:** Today this is masked — within one `tensor_hash` the chunk layout is deterministic
and `chunkReceivers` is a constant `32` (`CalcPanel.tsx:270`), so re-runs write byte-identical files.
It becomes reachable if `chunkReceivers` ever becomes device- or concurrency-dependent while the
identity key does not include it (and note WR-04/HI-01: the key already omits it), producing a
differently-chunked file over a stale longer one. Harm is limited because Phase-11 reads are
span-bounded (the `TierComplete` span carries `len`), so trailing garbage is not read — but the store
is left internally inconsistent.

**Fix:** Have the sink `truncate` each chunk file to its written length on `finish` (add a `truncate`
to the `ChunkHandle` seam and call `handle.truncate(size)` before `close`), or fold `chunkReceivers`
into the identity key so a re-layout always lands in a fresh directory.

**Resolution (2026-07-12) — FIXED (commit ec2a270).** `openChunkHandle` now calls `handle.truncate(0)`
immediately after `createSyncAccessHandle()`, BEFORE any write. Because the sink writes each chunk
contiguously from offset 0 (`h_off`/`p_off` start at 0), truncating on open guarantees a reopened chunk
file holds exactly the current run's bytes — a shorter re-run can leave no stale trailing bytes.

### WR-04: The cost-estimate receiver count and the actual solved grid diverge (area vs. square-side rounding)

**File:** `web/src/panels/CalcPanel.tsx:346-352` (estimate uses `area_m2`) vs. `:214-225`
(plan uses `area_max: [side, side]`, `side = sqrt(area_m2)`).

**Issue:** The pre-run estimate calls `estimate_cost` with `area_m2 = sceneInputs.areaM2`, giving
`receiver_count = floor(area / spacing²)`. The actual job builds a square lattice of side
`sqrt(area)`, whose node count is `(floor(side/spacing)+1)²`. The two differ by the lattice boundary
term (e.g. 100 m side at 3 m spacing: estimate 1111 vs. actual ~1156, +4%). The user is shown a
receiver count / tensor size / time that is not the count actually computed.

**Failure scenario:** Not a correctness bug in the solve, but the "pre-run estimate + guardrail" (SC1)
is the user's decision surface; a consistently-low estimate can let a grid through the guardrail that
the real lattice pushes over. Minor and bounded.

**Fix:** Derive the estimate from the same `plan_tiers` receiver count the job will use (the panel
already calls `plan_tiers` in `buildJobSpec`), or feed `estimate_cost` the actual `side²` area so the
two agree.

**Resolution (2026-07-12) — FIXED (commit 5f7803b).** Reconciled to one shared derivation (the
guardrail is safety-critical, so reconciled rather than documented). Extracted `buildPlanReq` — the
single square-lattice `plan_tiers` request both `buildJobSpec` and the estimate use. The estimate effect
now counts the real tier plan's receivers (`plannedReceiverCount`) and feeds that exact count to
`estimate_cost` as `discrete_points` with `area_m2: 0`, so the cost model's `receiver_count` equals the
solved grid — no `floor(area/spacing²)` vs `(floor(side/spacing)+1)²` boundary drift. All byte/time/
guardrail math stays in the Rust cost model (one source of truth); only the receiver count is now the
true plan count. (Note: `feed side² area` alone would NOT fix the boundary term, so the plan-count path
was taken.)

### WR-05: CalcPanel creates a new `CalcClient` (and its dedicated Worker) on every mount without tearing down the previous one

**File:** `web/src/panels/CalcPanel.tsx:317-336` (mount effect).

**Issue:** The mount effect does `new CalcClient()`, subscribes, and `attachClient(client)`, but the
cleanup only calls `unsubscribe()`. `CalcClient` owns a lazily-spawned dedicated `Worker`
(`client.ts:30-40`) and exposes no `terminate`/`dispose`, and the store's `attachClient` simply
overwrites the reference (`calc.ts:124`). On any remount (HMR, route change, conditional render) the
prior worker (and its rayon pool / SharedArrayBuffer memory) is orphaned, not closed.

**Failure scenario:** Repeated panel mounts leak dedicated workers and their thread pools; over a long
session this accumulates worker threads. Low severity (the panel likely mounts once in the SPA), but
it is a genuine resource leak with no disposal path.

**Fix:** Give `CalcClient` a `dispose()` that closes the worker (a clean `close()`, not the D-11-banned
mid-solve `terminate()` of a running job), call it in the effect cleanup, and have `attachClient`
dispose any client it replaces.

**Resolution (2026-07-12) — FIXED (commit c91b4a4).** Added `CalcClient.dispose()` which terminates the
dedicated worker and drops subscribers (idempotent). This is TEARDOWN of the whole client on
unmount/replace — distinct from the D-11 rule that a *running* solve is aborted cooperatively via
`cancel()` (a main-thread `Worker` exposes only `terminate()`; there is no `close()`). The store's
`CalcClient` interface gains `dispose()`, `attachClient` disposes any client it replaces, and
CalcPanel's effect cleanup calls `client.dispose()`. calc.test.ts mocks updated for the new method.

### WR-06: `run_chunk_range` reports `receivers: req.len` without confirming that many receivers were actually solved

**File:** `crates/envi-compute-wasm/src/lib.rs:298-301` and `crates/envi-compute-wasm/src/pool.rs:141-144`.

**Issue:** Both the single-chunk path and `solve_tier` build `RangeProgress`/`RangeProgressDto` from
the *requested* `range.len`/`req.len`, not from the number of receivers the solve actually wrote. In
the sequential wasm fallback (`scene.rs:520-539`), when `local_receivers` selects `k < len`, the sink
is `InMemorySink::new(n_sub, len)` and only `k` columns are written (the rest stay zero), yet progress
still reports `len`. Combined with WR-01, an inconsistent range is reported as fully solved.

**Failure scenario:** Same low-reachability envelope as WR-01 (worker always passes contiguous
tier-internal ranges). It matters as a defence-in-depth honesty gap: progress/`receivers` counts are
taken on faith rather than measured, so a partial/zero-filled chunk would be reported as complete.

**Fix:** Report the actually-written receiver count (e.g. `local_receivers(...).len()` /
`chunk_receivers.len()`) and treat `k != len` as an error (see WR-01) rather than silently
zero-filling.

**Resolution (2026-07-12) — FIXED (commit 6a9c56e).** `run_chunk_range` now reports
`local_receiver_count(r_offset, len)` (measured from the prepared scene) rather than trusting `req.len`.
Combined with WR-01 (which turns a partial range into a typed `Range` error before this point), the
count is measured, not assumed. The internal per-shard `pool.rs` `RangeProgress.receivers` needs no
change: WR-01's coverage gate makes each shard's `len` exact (the shards partition a fully-covered
`[0, len)`).

---

## Info

### IN-01: The end-to-end client-side threaded solve is not verified in a real browser (disclosed accepted risk)

**File:** `web/tests/e2e/calc.spec.ts:143-149` (Test 2 self-skips).

The `build:wasm:compute` artifact ships a non-shared `WebAssembly.Memory`, so `initThreadPool` cannot
start the rayon pool in-browser; Test 2 (the actual tiered solve → abort → `cancelled` flow) always
skips in this environment. This is honestly disclosed (10-05-SUMMARY, `deferred-items.md`) and the
skip is real, not faked — so it is **not** re-flagged as a defect. Recorded here only because GRID-02
/ SVC-02 / WEB-07 are marked requirements-completed while the deep threaded solve remains
browser-unverified; the native `cargo test` bit-equivalence + the offline Test 1 are what actually
back those claims. Close the 10-03 shared-memory build gap to lift the skip.

**Resolution (2026-07-12) — ALREADY RESOLVED (not part of this fix pass).** The shared-memory build was
fixed (commit 1099b24); Test 2 in `calc.spec.ts` now runs and passes. The full offline Playwright suite
is 21/21 in this fix pass's verification. No action taken here.

### IN-02: No `manifest.json` is persisted client-side for the OPFS tensor store (D-09 manifest identity)

**File:** `web/src/compute/opfs.ts` (writes only `tensor/`+`pincoh/` chunk files); `CalcManifest`
lives in `crates/envi-compute/src/identity.rs:349-366` but is only written server-side
(`envi-store::manifest::write_manifest`).

D-09 keys persistence "per project, by manifest hash." The client OPFS store writes chunk files but no
`manifest.json` (dims, `chunk_receivers`, `tensor_hash`, `created_at`). Phase 11 currently reads spans
from the `TierComplete` event stream, so this is not yet a blocker, but the persisted store has no
self-describing manifest for a cold reopen. Consider writing the `CalcManifest` into
`calc/<hash>/manifest.json` alongside the chunks so the reuse/reopen path (D-09) is self-contained.

### IN-03: `CalcManifest.stub` still documents "Phase 6 always writes true" after Phase 10 made compute real

**File:** `crates/envi-compute/src/identity.rs:362-363`.

The `stub: bool` field's doc comment ("`true` while compute is stubbed (Phase 6 always)") is now stale:
Phase 10 delivers a real solve. The client OPFS path writes no manifest (IN-02), so no false-green is
emitted today, but if/when the client persists a `CalcManifest` it must set `stub: false` for genuine
tensors. Update the doc and ensure the honest-provenance flag is set correctly at the new write site.

### IN-04: `PREPARED.read()` lock-poison is mapped to `NotPrepared`, a misleading error

**File:** `crates/envi-compute-wasm/src/lib.rs:286`.

`PREPARED.read().map_err(|_| ComputeError::NotPrepared)?` reports a poisoned lock as "no prepared
scene," which is misleading for diagnostics (poison implies a prior panic while holding the lock, a
different condition). Not a correctness bug — read guards do not poison on panic in std, so this arm
is effectively unreachable — but a distinct `ComputeError` (or documenting the arm as unreachable)
would be clearer.

---

## Invariants verified clean

- **Engine quarantine:** `git`-level byte-identity is claimed and consistent with the code — this
  phase adds only new crates/impls; the OPFS sink (`opfs_sink.rs`) is a fresh `TensorSink` impl, not
  an engine edit. No `.conj()` appears in `propagation/` (unchanged). `job_assembly.rs` performs no
  conjugation (the `e^{+jΔφ}` boundary stays in the engine solver).
- **One solve path (Pattern 1):** `pool::solve_tier` (`pool.rs:138`) and the sequential fallback
  (`scene.rs:535`) both call the unchanged `envi_engine::solver::solve`; no divergent solve exists.
  The marshalled range-solve is proven `f64::to_bits`-equal to a direct solve (scene.rs tests).
- **Band-index rule:** `interpolate.rs` and all spectrum handling compare by band index; anchors at
  `4+12k` / `4k` / identity are correct; no nominal-Hz comparison found.
- **Wire types generated:** every DTO derives `ts_rs::TS` into `wire.ts` with the no-drift test;
  `JobStatus` is reused, not re-derived; no hand-authored TS mirror of a Rust DTO.
- **Concurrency/soundness of `static RwLock<Option<PreparedScene>>`:** write-locked once per submit,
  read-locked per range; rayon workers capture the read-borrowed `&PreparedScene` and never touch the
  lock; JS single-threaded orchestration prevents a write interleaving an in-flight solve; a panic
  under a read guard does not poison the lock. No data race.
- **Cancel between chunks:** honoured — `run_chunk_range` checks `is_cancel_requested()` at entry
  (lib.rs:283), and `solve_tier` re-checks the shared `CANCEL` flag at every shard boundary
  (pool.rs:130), returning `Cancelled` before opening any file. The worker flips the flag cooperatively
  and never calls `worker.terminate()`.
- **OPFS path safety:** `assertHex` (opfs.ts:67) rejects any non-`[0-9a-f]` hash and `safeSeg`
  (opfs.ts:58) strips `/`,`\`,`..`,NUL; only the UUID + hex hash + two fixed literals reach
  `getDirectoryHandle`. No traversal/injection.
- **Threaded-build isolation:** `wasm-bindgen-rayon`/`rayon` are gated behind the off-by-default
  `threads` feature and target-`cfg` tables, so the stable `envi-gis-wasm` build never compiles the
  atomics toolchain.
- **Honest states:** `Failed(reason)` / `Cancelled` are real terminal states; the earlier
  `ComputeError::Pending` stub was removed in 10-06; the capability-failure path is a distinct honest
  banner, not a generic failure. The e2e suite skips honestly rather than faking a solve.

---

---

## Fix-pass note on the remaining Info items (2026-07-12)

- **IN-01** — ALREADY RESOLVED (see inline; shared-memory build fixed in 1099b24, Test 2 passes).
- **IN-02** (no client-side `manifest.json`), **IN-03** (`CalcManifest.stub` stale doc), **IN-04**
  (`PREPARED.read()` poison → `NotPrepared`) — ACCEPTED / deferred. These are Info-severity and were not
  in this fix pass's scope (which targeted HI-01 + the six Warnings). None is a defect today: the client
  OPFS path writes no `CalcManifest` yet (IN-02/IN-03 emit no false-green), and a `std` read guard does
  not poison on panic so the IN-04 arm is effectively unreachable. They remain tracked for the Phase-11
  reuse/reopen path (IN-02/03) and a future diagnostics cleanup (IN-04).

_Reviewed: 2026-07-12_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: deep_
_Fix pass: 2026-07-12 — Claude (gsd-code-fixer): HI-01 + WR-01..06 fixed; IN-01..04 accepted; all gates green._
