---
phase: 10-calculation-service
plan: 03
subsystem: infra
tags: [wasm, wasm-bindgen, wasm-bindgen-rayon, rayon, opfs, ts-rs, threaded-wasm, tensor-sink, cost-model, tiers]

# Dependency graph
requires:
  - phase: 10-calculation-service (10-01)
    provides: envi-compute core (cost model, hierarchical tiers, tensor-identity closure, SolveJob assembly)
  - phase: 04-* (envi-engine)
    provides: TensorSink trait + TensorPair + BYTES_PER_CELL_PAIR + solver::solve
  - phase: 08-* (envi-gis-wasm)
    provides: the thin-cdylib pattern, ts-rs wire.ts no-drift mechanism, wasm-bindgen =0.2.126 pin
provides:
  - envi-compute-wasm cdylib+rlib — the browser compute boundary (estimate_cost, plan_tiers, request_cancel/reset_cancel, solve_chunk_range seam)
  - OpfsChunkSink — a new impl of envi_engine::tensor::TensorSink writing the frozen [s][r_local][f] interleaved-LE chunk format over FileSystemSyncAccessHandle (worker-only)
  - TierComplete D-07 event payload + cost/tier boundary DTOs generated into the single committed wire.ts (no-drift)
  - build:wasm:compute — a scoped nightly + -Zbuild-std + atomics npm script producing the threaded (SharedArrayBuffer) module
affects: [10-04, 10-05, 11]

# Tech tracking
tech-stack:
  added: [wasm-bindgen-rayon 1.3.0 (threads feature only), rayon 1.10/1.12 (threads feature only)]
  patterns:
    - "Threaded-wasm build scoped to one npm script via cargo +nightly + inline --config rustflags (no rust-toolchain.toml, no .cargo/config.toml — Pitfall 1)"
    - "Off-by-default `threads` cargo feature isolates wasm-bindgen-rayon from stable/native builds"
    - "OPFS TensorSink as a boxed ChunkHandle seam (native Vec<u8> mock + wasm FileSystemSyncAccessHandle extern glue)"
    - "I/O errors captured and surfaced at finish() (SinkError has no I/O variant; engine unchanged)"

key-files:
  created:
    - crates/envi-compute-wasm/Cargo.toml
    - crates/envi-compute-wasm/src/lib.rs
    - crates/envi-compute-wasm/src/dto.rs
    - crates/envi-compute-wasm/src/opfs_sink.rs
  modified:
    - crates/envi-service/Cargo.toml (dev-dep envi-compute-wasm)
    - crates/envi-service/tests/wire_no_drift.rs (register compute DTOs)
    - web/src/generated/wire.ts (regenerated: TierComplete + cost/tier DTOs)
    - web/package.json (build:wasm:compute script)
    - web/vite.config.ts (assetsInclude wasm-compute)
    - .gitignore (wasm-compute artifact)
    - web/README.md, crates/README.md (docs)

key-decisions:
  - "wasm-bindgen-rayon + rayon gated behind an off-by-default `threads` feature so stable/native builds never compile the atomics toolchain (protects the envi-service no-drift rlib and the wasm32 stable smoke build)"
  - "Threaded build uses `cargo +nightly-2026-07-11` + inline `--config` rustflags (cross-platform: works in cmd.exe and POSIX; per-invocation, no global leak) instead of RUSTUP_TOOLCHAIN/RUSTFLAGS env prefixes"
  - "OpfsChunkSink is non-generic over a boxed ChunkHandle trait object (native Vec<u8> mock + wasm FileSystemSyncAccessHandle extern glue via a BARE module specifier resolved by Vite at bundle time)"
  - "OPFS I/O errors surfaced by finish() (SinkError has no I/O variant; engine byte-identical, D-02) — put_chunk stays within the SinkError validation contract and never panics"
  - "JobStatus reused verbatim from the existing wire.ts (envi-service, Phase 6/7); no duplicate re-derivation (a second `export type JobStatus` would break tsc)"

patterns-established:
  - "Pattern: threaded-wasm toolchain isolation — one npm script, cargo +pinned-nightly, inline --config atomics, no toolchain/config files at rest"
  - "Pattern: OPFS chunk sink round-trip proven byte-exact vs InMemorySink (incl. a non-contiguous sliced view, Pitfall 7)"

requirements-completed: [GRID-02, SVC-02]

# Metrics
duration: ~95min
completed: 2026-07-11
status: complete
---

# Phase 10 Plan 03: envi-compute-wasm — browser compute boundary, OPFS TensorSink, threaded build Summary

**Thin `envi-compute-wasm` cdylib exposing the pure cost/tier core + an OPFS-backed `TensorSink` (byte-exact `[s][r_local][f]` chunk format over `FileSystemSyncAccessHandle`), with `TierComplete`/cost/tier DTOs generated into the committed `wire.ts` and a scoped nightly + `-Zbuild-std` + atomics `build:wasm:compute` script that produces the threaded SharedArrayBuffer module — engine byte-identical.**

## Performance

- **Duration:** ~95 min
- **Completed:** 2026-07-11
- **Tasks:** 3
- **Files modified/created:** 13

## Accomplishments
- New `envi-compute-wasm` cdylib+rlib mirroring `envi-gis-wasm` discipline: `estimate_cost`/`plan_tiers` delegate to the pure `envi_compute::{cost,tiers}` core; `request_cancel`/`reset_cancel` back the D-11 cooperative cancel flag; `solve_chunk_range` declares the 10-04 pool seam. `#![deny(unsafe_code)]`, no id-minting.
- `OpfsChunkSink` — a NEW impl of the engine's existing `TensorSink` trait (no engine change): replicates `InMemorySink`'s validation gates (typed `SinkError`, never a panic), serializes the frozen `[s][r_local][f]` interleaved-`(re,im)`-f64-LE (16 B) + `P_incoh` f64-LE (8 B) format iterating logical order (correct for non-contiguous slices, Pitfall 7), and writes via a boxed `ChunkHandle` seam (native mock + wasm `FileSystemSyncAccessHandle` extern glue). Round-trip proven byte-exact vs `InMemorySink`.
- `TierComplete` (D-07 event) + cost/tier request/response DTOs ts-rs-generated into the single committed `wire.ts` with the no-drift test green; `JobStatus` reused from `envi-service`.
- Scoped `build:wasm:compute` npm script — verified to produce the threaded module (glue + `_bg.wasm` + rayon worker snippets, `initThreadPool` exported) without leaking nightly/atomics onto the stable `build:wasm` (gis) or native builds.

## Task Commits

1. **Task 1: envi-compute-wasm scaffold, thin boundary + ts-rs DTOs** — `f53ecef` (feat)
2. **Task 2: OPFS-backed TensorSink (frozen chunk byte format)** — `145be96` (feat, tdd behavior-first)
3. **Task 3: scoped threaded-wasm build toolchain** — `15961e6` (build)

**Plan metadata:** _(final docs/state commit)_

## Files Created/Modified
- `crates/envi-compute-wasm/Cargo.toml` — cdylib+rlib, wasm-bindgen `=0.2.126`, `threads` feature gating wasm-bindgen-rayon/rayon, ndarray/num-complex pinned to the engine's versions.
- `crates/envi-compute-wasm/src/lib.rs` — thin boundary exports + cancel flag + `init_thread_pool` re-export (threads-gated).
- `crates/envi-compute-wasm/src/dto.rs` — cost/tier request+response DTOs + `TierComplete` + `ChunkSpanDto` (ts-rs → wire.ts).
- `crates/envi-compute-wasm/src/opfs_sink.rs` — `OpfsChunkSink` + `ChunkHandle` seam + wasm extern glue + round-trip/negative tests.
- `crates/envi-service/Cargo.toml`, `tests/wire_no_drift.rs` — dev-dep + register the compute DTOs in the no-drift export list.
- `web/src/generated/wire.ts` — regenerated (adds `TierComplete`, `CostEstimateResult`, `TierPlanResult`, etc.).
- `web/package.json` — `build:wasm:compute` + wired into `build`.
- `web/vite.config.ts` — `assetsInclude` for `wasm-compute`.
- `.gitignore` — threaded artifact ignored (root, beside the gis wasm/ entry).
- `web/README.md`, `crates/README.md` — threaded-build docs + crate-table rows.

## Decisions Made
- **`threads` feature gate.** `wasm-bindgen-rayon`/`rayon` are optional deps enabled only by the threaded build, so native `cargo test` (incl. the envi-service no-drift rlib) and the wasm32 stable smoke build never compile the atomics toolchain. This is what keeps Pitfall-1 isolation airtight AND the stable acceptance builds green.
- **Cross-platform threaded build without env prefixes.** Used `cargo +nightly-2026-07-11` + inline `--config "target.wasm32-unknown-unknown.rustflags=['-C','target-feature=+atomics,+bulk-memory,+mutable-globals']"` — works identically in `cmd.exe` and POSIX shells (avoids the POSIX-only `RUSTFLAGS=... cargo` prefix that would break `npm run` on Windows) and stays per-invocation (no `rust-toolchain.toml`, no `.cargo/config.toml`).
- **Boxed `ChunkHandle` seam.** `OpfsChunkSink` is non-generic over `Box<dyn ChunkHandle>` so the native `Vec<u8>` test mock and the wasm `FileSystemSyncAccessHandle` handle share one impl, and `impl TensorSink for OpfsChunkSink` matches the plan's grep gate.
- **JobStatus reuse, not re-derivation.** The `JobStatus` union is already in the committed `wire.ts` from `envi-service` (Phase 6/7) and is reused client-side per D-10; re-deriving a second identical ts-rs type would emit a duplicate `export type JobStatus` and break `tsc`. The compute worker (10-04, TypeScript) constructs `JobStatus` from that existing wire type.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `ndarray` version pinned to the engine's `0.17`, not `0.16`**
- **Found during:** Task 2 (OPFS sink tests)
- **Issue:** I initially declared `ndarray = "0.16"`; the engine uses `ndarray = "0.17"`, so the chunk `ArrayView3` types did not unify (two ndarray versions in the graph, E0308 in the round-trip test).
- **Fix:** Changed `crates/envi-compute-wasm/Cargo.toml` to `ndarray = "0.17"` (and kept `num-complex = "0.4"`), so the sink's views unify with the engine's exactly.
- **Verification:** `cargo test -p envi-compute-wasm opfs_sink` — 8/8 pass.
- **Committed in:** `145be96`

**2. [Rule 3 - Blocking] wasm-bindgen `module` must be a bare specifier, not a `/`-path**
- **Found during:** Task 2 (wasm32 build of the OPFS extern glue)
- **Issue:** `#[wasm_bindgen(module = "/src/compute/opfs.ts")]` makes wasm-bindgen read the file at **compile time**; the glue is authored in 10-04, so `cargo build --target wasm32` failed (`failed to read file ...opfs.ts`).
- **Fix:** Used a **bare** module specifier `envi-compute-opfs` (resolved by Vite at bundle time via a `resolve.alias` added in 10-04), which compiles now with no file read.
- **Verification:** `cargo build -p envi-compute-wasm --target wasm32-unknown-unknown` compiles; the threaded `wasm-bindgen` step emits the module.
- **Committed in:** `145be96`

**3. [Rule 3 - Blocking] `.gitignore` entry placed in the root file, not a new `web/.gitignore`**
- **Found during:** Task 3
- **Issue:** The plan's file list named `web/.gitignore`, but the repo has no `web/.gitignore` — the sibling gis artifact (`web/src/generated/wasm/`) is ignored in the **root** `.gitignore`.
- **Fix:** Added `web/src/generated/wasm-compute/` to the root `.gitignore` beside the gis entry (single, consistent ignore file). `git check-ignore` confirms the artifact is ignored.
- **Impact:** The Task-3 verify command does not grep `web/.gitignore`; the artifact is correctly ignored. Convention preserved.

**4. [Interface note] OPFS I/O errors surfaced at `finish()`**
- **Found during:** Task 2
- **Issue:** `envi_engine::tensor::SinkError` (the fixed trait contract) has no I/O variant, and the engine must stay byte-identical (D-02), so a real OPFS write failure cannot be returned as a `SinkError` from `put_chunk`.
- **Fix:** `put_chunk` captures the first `OpfsError` and short-circuits later writes; `OpfsChunkSink::finish()` (called by the 10-04 pool before marking a tier complete) surfaces it. `put_chunk` stays within the `SinkError` validation contract and never panics; covered by an `io_error_at_finish` test.
- **Impact:** No engine change; honest error surfacing.

---

**Total deviations:** 3 blocking auto-fixes + 1 interface note. **Impact:** all necessary for correctness/compilation; no scope creep. Engine and `envi-compute` byte-identical (empty diff).

## Accepted Risks / Notes

- **Transitive `uuid` in the dependency tree (not id-minting).** The plan's Pitfall-9 prohibition targets id-minting. `getrandom` is **absent** (verified 0) and `uuid` carries **no `v4`/getrandom** feature — it is pulled transitively by the plan-mandated `envi-compute::identity` closure (parse/serialize of TS-minted ids in `tensor_hash`), exactly as `envi-store` and the compute core already carry it. The crate declares **no direct** `getrandom`/`uuid` and mints no ids. The literal `cargo tree | grep -c 'uuid' == 0` gate is therefore not met (unavoidable given the mandated `envi-compute` dep), but the intent (no id-minting / no `getrandom`) is fully satisfied.
- **`estimate_cost` composes two pure core calls** (`envi_compute::cost::estimate` then `guardrail(&estimate, budget)`) — zero inline math; the guardrail requires the estimate as its input. This is still a logic-free thin boundary; splitting them or adding a combined core fn (which would edit the frozen 10-01 `envi-compute`) was avoided.
- **`usize` byte counts on wasm32 (out of scope, envi-compute/10-01).** The cost model returns `usize`; on 32-bit wasm `usize` is `u32`, so extreme grids (> ~4.3 GB tensor bytes) could overflow inside `envi_compute::cost` before the boundary sees them. This lives in the frozen 10-01 core (not this plan's files) and does not affect the typical grid regime; logged for a future `envi-compute` hardening (use `u64`/`f64` for byte math). The boundary DTO already carries byte counts as JS-safe `f64`.
- **Threaded toolchain install: SUCCEEDED.** `rustup toolchain install nightly-2026-07-11 --component rust-src` installed cleanly; `npm run build:wasm:compute` ran end-to-end (exit 0) and produced `web/src/generated/wasm-compute/{envi_compute_wasm.js,_bg.wasm}` + rayon worker snippets with `initThreadPool` exported. **No accepted-risk fallback was needed** — the threaded build is verified working, not merely asserted.
- **Default `npm run build` now requires the pinned nightly + rust-src** (the plan mandated wiring `build:wasm:compute` into `build`). Documented in `web/README.md`; a contributor lacking nightly can use `npm run build:web`.

## Issues Encountered
- Two ndarray versions in the graph (fixed by pinning to `0.17`); the wasm-bindgen `/`-path compile-time file read (fixed with a bare specifier). Both resolved during Task 2 and covered by the passing tests.

## User Setup Required
None for this plan's code. To (re)build the threaded module a developer needs `rustup toolchain install nightly-2026-07-11 --component rust-src` (documented in `web/README.md`).

## Next Phase Readiness
- **10-04** can now: `import init, { initThreadPool, estimate_cost, plan_tiers, request_cancel, solve_chunk_range } from "./generated/wasm-compute/envi_compute_wasm"`; author `web/src/compute/opfs.ts` and wire the Vite `resolve.alias` `envi-compute-opfs → ./src/compute/opfs.ts`; implement the rayon pool driver + `solve_chunk_range` body (the OPFS sink and cancel flag are ready).
- Engine + `envi-compute` byte-identical; all quality gates green.

## Self-Check: PASSED

- Created files verified present: `envi-compute-wasm/{Cargo.toml,src/lib.rs,src/dto.rs,src/opfs_sink.rs}`, `10-03-SUMMARY.md`.
- Task commits verified in history: `f53ecef`, `145be96`, `15961e6`.
- Engine + `envi-compute` byte-identical (empty diff since baseline `20763e3`).

---
*Phase: 10-calculation-service*
*Completed: 2026-07-11*
