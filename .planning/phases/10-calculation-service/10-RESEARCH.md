# Phase 10: Calculation Service - Research

**Researched:** 2026-07-11
**Domain:** Client-side threaded WebAssembly compute (wasm-bindgen-rayon), OPFS chunked tensor store, Web Worker job machine, progressive tier solve, cross-origin isolation
**Confidence:** HIGH for repo seams / engine contracts (grounded in the actual source read this session); MEDIUM for the threaded-wasm toolchain specifics (web-verified this session, tagged inline); LOW only where flagged as Open Questions.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** The heavy grid solve runs **client-side as threaded WebAssembly** on the user's device. The server only authenticates + serves the (cross-origin-isolated) bundle (+ CORS proxy). The ROADMAP/ARCHITECTURE "axum job runner, rayon threads, on-disk chunk store, SSE" text is **superseded** for the solve path — crate topology, chunk-format, memory math, and Pattern-1 "one solve path, two callers" invariant still hold, relocated into the browser.
- **D-02:** A new **browser WASM compute crate** wraps `envi_engine::solver::solve` (marshalling only, following the existing `envi-gis-wasm` cdylib pattern). Engine's 3-dep quarantine (`ndarray + num-complex + thiserror`) untouched; rayon stays caller-side.
- **D-03:** **rayon-in-WASM via `wasm-bindgen-rayon`** — a SharedArrayBuffer-backed pool sized to `navigator.hardwareConcurrency`, reusing the engine's parallelism. Requires a threaded/atomics wasm build.
- **D-04:** Cross-origin isolation via **COOP: same-origin + COEP: credentialless** (NOT `require-corp`) so Phase-8 direct GIS/basemap fetches keep working. `envi-service` emits these headers on the app bundle.
- **D-05:** **Hierarchical refinement.** Coarse 100 m points are a strict subset of the 10 m fine grid (10 | 100), so coarse points are kept and the fine pass computes only the ~99% gap points — no recompute. Tier order: discrete receiver points → coarse grid → fine grid.
- **D-06:** **User sets the final (fine) spacing**, default **10 m**; preview tiers are auto-derived as coarser multiples (×10 → 100 m, optionally an intermediate). Cost estimate/guardrail (SC1) keys off the final spacing.
- **D-07:** The solver **emits tier-complete partial results** — a "tier N done" event carrying that tier's receiver set + stored tensor spans — so Phase 11 can render points, then coarse map, then refined map. Phase 10 owns compute + emission; Phase 11 owns rendering.
- **D-08:** **OPFS chunked, single path.** A wasm OPFS-backed `TensorSink` writes receiver-axis chunks (interleaved re,im f64 LE, freq-contiguous `[s][r_local][f]`) via `FileSystemSyncAccessHandle` in the worker. Full tensor stays off-heap; working set = workers × chunk. No dual in-memory/spill path. (`InMemorySink` remains the native/test sink.)
- **D-09:** **Persist per project, keyed by manifest hash** (the blake3 identity `envi-store` already computes). Reopening/re-running an identical scene reuses the tensor; Phase-11 named weather scenarios each get their own hash-keyed tensor. No eviction-on-close this phase.
- **D-10:** The **Web Worker hosting the rayon pool owns the compute job lifecycle** and posts progress + tier-complete events. **Reuse the Phase-6 `JobStatus` enum/wire shape** client-side. The server SSE job machine stays only for genuine server-side async (ERA5/CDS), never the solve.
- **D-11:** **Abort is cooperative at chunk boundaries** — a SharedArrayBuffer-backed atomic cancel flag rayon workers check between receiver chunks. No `worker.terminate()`. Already-emitted tiers stay valid, OPFS handles close cleanly, pool reusable.
- **D-12:** **Headers + bundle serving only** land here. Real auth (login gate, sessions, accounts) is a separate, deferred phase.

### Claude's Discretion
- Exact chunk size (receiver count per chunk) and the worker-pool sizing heuristic.
- The cost-estimate time model + guardrail warning thresholds ("halving spacing quadruples cost").
- The intermediate preview-tier count/spacings between points and fine.
- OPFS directory layout + manifest-file naming within the project dir.
- The tier-complete event payload schema (must carry enough for Phase-11 rendering).
- How the atomic cancel flag + progress counters are shared across the pool.
- The threaded-wasm build toolchain details (atomics flags, bundler wiring for `wasm-bindgen-rayon`).

### Folded Todos (in scope this phase)
- **Wire directional phase into the coherent composition path (SRC-03).** Populate `SolveJob::directivity_phase_rad` from `DirectivityBalloon::eval_phase` at the WASM `SolveJob` assembly site. Backward-compatible: a phase-free balloon leaves `arg(H_coh)` bit-identical.

### Deferred Ideas (OUT OF SCOPE)
- PROJECT.md / ARCHITECTURE.md deployment-model amendment pass (recommended as a coordinated doc task around this phase; binding decision recorded in CONTEXT, doc edits are a follow-up).
- Real authentication / login gate (own phase, own threat model).
- Interactive fast-recalc MAC + results rendering (Phase 11): spectra panels, isophone maps, editable color scale, difference maps. Phase 10 *computes and emits* tiered partials; Phase 11 *renders* them.
- GRID-03 L_den weather-class combination (beyond Milestone 2).
- OPFS quota / eviction strategy (only revisit if quota strains).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| **SVC-02** | Compute-job model (submit, queue, run, progress, cancel, fetch results) with a Queued/Running/Done/Failed/Cancelled state machine | Reuse the `JobStatus` enum (`envi-service/src/jobs.rs`) as the client-side wire shape (§Standard Stack, §Pattern: Worker job machine). Progress/tier events via `postMessage`, not SSE (D-10). Cooperative abort via SharedArrayBuffer atomic flag (§Pattern: Cooperative abort). |
| **GRID-02** | Batch-compute the transfer tensor over the grid, parallelized (rayon), receiver-axis chunked | The engine's `solve()` streams receiver-axis chunks into a `TensorSink` (`envi-engine/src/solver.rs:139`). **Engine has NO internal rayon** (verified) — parallelism is caller-side: shard the receiver axis into disjoint chunk ranges, run one `solve()` per range on the wasm-bindgen-rayon pool, each writing its own OPFS chunk file (§Pattern: Caller-side rayon sharding). |
| **WEB-07** | Submit a calculation job and view progress / abort / results; pre-run cost estimate | Submit/progress/abort UI driven by the worker job machine; pre-run cost estimate from grid spacing + area (§Cost Estimate Model). "Results" here = tiered partials emitted + persisted; rendering is Phase 11. |
</phase_requirements>

## Summary

Phase 10 relocates the validated Nord2000 grid solve into the browser as **threaded WebAssembly**. The engine's promoted `solve()` path (Phase 4) is already shaped for this: it takes an iterator of `SolveJob`s in receiver-major order and streams paired `(H_coh, P_incoh)` receiver-axis chunks into a `TensorSink` trait, with a single reusable working buffer that bounds the resident set to one chunk (OUT-06). Nothing in the engine changes.

The new work is entirely a **thin boundary + orchestration layer** in the browser, mirroring the existing `envi-gis-wasm` cdylib pattern: a new `envi-compute-wasm` crate marshals inputs, assembles `SolveJob`s in Rust, drives a `wasm-bindgen-rayon` thread pool over disjoint receiver-chunk ranges, and streams each chunk through a new **OPFS-backed `TensorSink`** (`FileSystemSyncAccessHandle`, worker-only). A Web Worker hosts the pool and owns the job lifecycle, reusing the Phase-6 `JobStatus` vocabulary and posting progress + tier-complete events to the UI. `envi-service` gains two response headers (`COOP: same-origin`, `COEP: credentialless`) so the page is `crossOriginIsolated` and `SharedArrayBuffer` is available.

Three findings dominate the plan. **(1) The engine has no internal rayon** — `solve()` is sequential over its job iterator; GRID-02's "parallelized (rayon)" is satisfied *caller-side* by sharding the receiver axis and running independent `solve()` calls on the pool, each owning disjoint chunk files (no shared-mutable sink, no engine change). **(2) The threaded build needs nightly Rust + `-Zbuild-std` + `RUSTFLAGS=-C target-feature=+atomics,+bulk-memory,+mutable-globals`**, which is a *different* toolchain from the existing stable `envi-gis-wasm` build — the two must be kept in separate build invocations and the atomics RUSTFLAGS must NOT leak into the stable build. **(3) `envi-store`'s manifest/hash types are entangled with `std::fs`** (`write_manifest`, `atomic_write`, `project_dir`) — the pure format/hash/`chunk_receivers` logic must be factored into a WASM-safe module (no `std::fs`, no `blake3` file I/O) that both native `envi-store` and the browser compute crate can share.

**Primary recommendation:** Build a new pure-Rust `envi-compute` core crate (WASM-safe: no `std::fs`, no C, no rayon) holding the `SolveJob` assembly, tier partitioning, cost model, and manifest/hash types factored out of `envi-store`; wrap it in a thin `envi-compute-wasm` cdylib that adds the `wasm-bindgen-rayon` pool + OPFS sink; drive it from a Web Worker that reuses `JobStatus`. Pin a specific nightly toolchain for the threaded build only.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Nord2000 grid solve (per (s,r) chain) | Browser / Client (WASM) | — | D-01: compute runs on the user's device; engine `solve()` is pure math, WASM-safe |
| Parallelism over receiver chunks | Browser / Client (wasm-bindgen-rayon pool) | — | D-03: SharedArrayBuffer thread pool; engine has no internal rayon (caller-side) |
| Tensor persistence (chunked) | Browser / Client (OPFS) | — | D-08: `FileSystemSyncAccessHandle` in the worker; full tensor off-heap |
| Job lifecycle / progress / abort | Browser / Client (Web Worker) | — | D-10: worker owns the state machine; UI is a `postMessage` consumer |
| Cost estimate + guardrail | Browser / Client (main thread pre-run) | WASM core (pure fn) | SC1: cheap arithmetic on grid spacing + area; the formula lives in the shared core |
| Cross-origin isolation headers | Frontend Server (axum `envi-service`) | — | D-04: only the delivery server can set COOP/COEP on the bundle response |
| Bundle serving | Frontend Server (axum `ServeDir`) | — | Already serves `web/dist` (`envi-service/src/api/mod.rs:114`) |
| SSE job machine (ERA5/CDS only) | Frontend Server (axum) | — | D-10: server async reserved for genuine server-side async, never the solve |
| Directivity balloon eval (gain + phase) | WASM core (Rust) | — | `SolveJob` holds engine types; balloon `eval`/`eval_phase` must run in Rust at the assembly site |

## Standard Stack

### Core (new dependencies)
| Library | Version | Ecosystem | Purpose | Why Standard |
|---------|---------|-----------|---------|--------------|
| `wasm-bindgen-rayon` | `1.3.0` | crates.io | Bridges rayon's global thread pool onto Web Workers backed by SharedArrayBuffer; emits `initThreadPool(n)` JS glue | The de-facto (and effectively only) mature adapter for rayon-in-browser; authored by a wasm-bindgen maintainer (RReverser); ~280k downloads on 1.3.0 [CITED: crates.io/crates/wasm-bindgen-rayon] |
| `rayon` | `1.10` (workspace-pin) | crates.io | Data-parallel iterator over receiver-chunk ranges (caller-side only) | Standard Rust data-parallelism; the same crate a native parallel driver would use [ASSUMED — verify exact latest at plan time] |
| `wasm-bindgen` | `=0.2.126` | crates.io | Boundary glue (already the repo pin) | **Reuse the existing exact pin** (`envi-gis-wasm/Cargo.toml:37`); wasm-bindgen-rayon 1.x depends on `wasm-bindgen = "0.2"` (caret), so `0.2.126` satisfies it [CITED: github.com/RReverser/wasm-bindgen-rayon] |
| `js-sys` / `web-sys` | `0.3` | crates.io | OPFS access from Rust (`web-sys` features: `FileSystemFileHandle`, `FileSystemSyncAccessHandle`, `Navigator`, `StorageManager`, `FileSystemDirectoryHandle`) OR thin JS glue | `web-sys` already available transitively; OPFS sync-access types are behind opt-in features |

### Core (existing — reuse verbatim, do NOT rebuild)
| Asset | Location | Purpose |
|-------|----------|---------|
| `envi_engine::solver::solve` + `SolveJob` | `crates/envi-engine/src/solver.rs:63,139` | The single promoted, FORCE-validated solve path. The compute crate is a thin caller (Pattern 1). ENG-09/10 (forest, isolation) already compute inside it via `SolveJob.forest` / `SolveJob.isolation`. |
| `envi_engine::tensor::TensorSink` | `crates/envi-engine/src/tensor.rs:166` | The exact trait the OPFS sink implements: `put_chunk(r_offset, ArrayView3<Complex<f64>>, ArrayView3<f64>)`. `InMemorySink` stays native/test; `CountingSink` proves the memory bound. |
| `envi_engine::tensor::{BYTES_PER_CELL_PAIR, DEFAULT_TENSOR_BUDGET_BYTES}` | `crates/envi-engine/src/tensor.rs:48,54` | `24` bytes/cell (16 complex + 8 real); 256 MiB default budget. The chunk-size formula uses these — never re-derive. |
| `envi_engine::directivity::DirectivityBalloon` | `crates/envi-engine/src/directivity.rs:158` | `eval(dir) -> [f64;105]` (gain ΔL) and `eval_phase(dir) -> [f64;105]` (Δφ rad) — exactly the shapes `SolveJob.directivity_gain_db` / `directivity_phase_rad` expect. |
| `JobStatus` enum | `crates/envi-service/src/jobs.rs:54` | Queued/Running{progress,message}/Done/Failed{reason}/Cancelled, internally tagged `snake_case`. Reuse as the client-side wire shape (D-10). |
| `envi-store` manifest + hash | `crates/envi-store/src/{manifest.rs,hash.rs}` | `CalcManifest`, `chunk_receivers(n_sub,n_rcv)`, `tensor_hash(scene, met, receivers)`. **Factor the pure parts out of `std::fs` for WASM reuse** (see §Refactor). |
| `envi-gis-wasm` cdylib pattern | `crates/envi-gis-wasm/` | The template: thin boundary, `serde-wasm-bindgen` marshalling, ts-rs boundary DTOs → committed `web/src/generated/wire.ts`, no `getrandom`/`uuid`, version-pinned wasm-bindgen. |
| Phase-9 grid/path shims | `crates/envi-gis-wasm/src/lib.rs` (`build_receiver_grid`, `extract_cut_profile`, `segment_cut_profile`, `inject_screen_edges`) | Produce the `PropagationPath` inputs (profiles, screens, receiver positions) the `SolveJob` assembly consumes. |

### Supporting (JS/build side)
| Tool | Version | Purpose | Notes |
|------|---------|---------|-------|
| Vite `server.headers` config | (existing Vite 8) | Set COOP/COEP on the **dev** server so `crossOriginIsolated` is true during `npm run dev` and Playwright | No new npm dependency needed — Vite supports `server.headers` natively; avoid `vite-plugin-cross-origin-isolation` |
| nightly Rust toolchain | pin one, e.g. `nightly-2026-01-15` | Required for `-Zbuild-std` (threaded std) | **Threaded build only** — do NOT add a workspace-wide `rust-toolchain.toml` (would force nightly on the stable engine/gis builds) |
| `wasm-bindgen-cli` | `=0.2.126` | Already required (lockstep with the crate) | Same CLI works for the threaded module; the atomics come from RUSTFLAGS + build-std, not the CLI |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `wasm-bindgen-rayon` | Manual `Worker` pool + `postMessage` sharding | A second parallelism model + serialization overhead + no shared engine code; rejected in discussion (CONTEXT §Parallelization) |
| `web-sys` OPFS types | Thin hand-written JS glue calling OPFS, invoked over `wasm-bindgen` | JS glue is simpler if `web-sys`'s sync-access-handle features are unstable/gated; both are viable — see Open Question Q3 |
| nightly + `-Zbuild-std` | Wait for stable threaded std | Not available; nightly is mandatory today for wasm threads [CITED: rustwasm.github.io/docs/wasm-bindgen/examples/raytrace.html] |
| COEP `credentialless` | COEP `require-corp` | `require-corp` breaks Phase-8 direct third-party fetches (basemap/AHN/Overpass) that don't send CORP headers (D-04) |

**Installation (Rust side, new crates):**
```bash
# workspace Cargo.toml gains, in the NEW crates only (engine untouched):
#   envi-compute      -> rayon (native driver + tests), envi-engine, envi-geo(? no) — pure Rust
#   envi-compute-wasm -> wasm-bindgen=0.2.126, wasm-bindgen-rayon=1.3.0, js-sys, web-sys, serde-wasm-bindgen, ts-rs
cargo add --package envi-compute-wasm wasm-bindgen-rayon@1.3.0
```

**Threaded build command (the new npm script — separate from `build:wasm`):**
```bash
# build:wasm:compute (nightly + atomics + build-std, scoped to THIS command only)
RUSTUP_TOOLCHAIN=nightly-2026-01-15 \
RUSTFLAGS='-C target-feature=+atomics,+bulk-memory,+mutable-globals' \
cargo build -p envi-compute-wasm --release --target wasm32-unknown-unknown \
  -Z build-std=std,panic_abort
wasm-bindgen --target web --out-dir web/src/generated/wasm-compute \
  --out-name envi_compute_wasm \
  target/wasm32-unknown-unknown/release/envi_compute_wasm.wasm
```

**Version verification (run at plan/execute time):**
```bash
cargo search wasm-bindgen-rayon        # confirm 1.3.0 is still current
cargo search rayon                     # confirm the exact latest 1.x
# Confirm wasm-bindgen-rayon 1.3.0's wasm-bindgen requirement is satisfied by =0.2.126:
cargo tree -p envi-compute-wasm -i wasm-bindgen   # after adding the dep
```

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| `wasm-bindgen-rayon` | crates.io | 1.3.0 released Dec 2024; crate ~4 yrs | 1.3.0: ~280k; total ~460k | github.com/RReverser/wasm-bindgen-rayon | OK | Approved — authored by a wasm-bindgen maintainer; verify at plan time via `cargo add` + `cargo tree` |
| `rayon` | crates.io | mature (>8 yrs) | tens of millions | github.com/rayon-rs/rayon | OK | Approved (already a well-known standard) |
| `wasm-bindgen` `=0.2.126` | crates.io | existing repo pin | ecosystem-standard | github.com/rustwasm/wasm-bindgen | OK | Reused verbatim |
| `web-sys` / `js-sys` `0.3` | crates.io | existing (transitive) | ecosystem-standard | rustwasm | OK | Approved (enable OPFS features) |

**Packages removed due to [SLOP] verdict:** none.
**Packages flagged as suspicious [SUS]:** none.

> Provenance note: `wasm-bindgen-rayon` was surfaced via WebSearch/training and confirmed on crates.io this session; it is authored by the wasm-bindgen maintainer and is the ecosystem-standard adapter, so it is treated as `OK`. Still, the planner should keep the `cargo add` + `cargo tree` verification step as a task so the exact resolved `wasm-bindgen` version is confirmed compatible with the `=0.2.126` pin. **No new npm packages are introduced** (COOP/COEP dev headers use Vite's built-in `server.headers`).

## Architecture Patterns

### System Architecture Diagram

```
                        ┌─────────────────────────────────────────────┐
   User clicks "Run" ─► │  MAIN THREAD (React UI)                      │
                        │  - pre-run COST ESTIMATE (pure fn)           │
                        │  - Submit → postMessage(job spec) to Worker  │
                        │  - renders JobStatus + tier-complete events  │◄────┐
                        └───────────────┬─────────────────────────────┘     │
                                        │ postMessage(spec)                  │ postMessage
                                        ▼                                    │ (Running{progress},
                        ┌─────────────────────────────────────────────┐     │  TierComplete, Done,
                        │  DEDICATED WEB WORKER (owns job lifecycle)   │     │  Failed, Cancelled)
                        │  JobStatus state machine (reused wire shape) │─────┘
                        │  SharedArrayBuffer: [cancel flag | progress] │
                        │                                              │
                        │  init: wasm module + initThreadPool(         │
                        │        navigator.hardwareConcurrency)        │
                        │                                              │
                        │  for tier in [points, coarse, fine-gaps]:    │
                        │    partition receivers → chunk ranges        │
                        │    rayon: par_iter(chunk ranges)  ───────────┼──┐ each rayon task:
                        │      (checks cancel flag between chunks)      │  │  - assemble SolveJob[] (Rust,
                        │    emit TierComplete{receiver set, spans}     │  │    balloon eval + eval_phase)
                        └───────────────┬───────────────────────────────┘  │  - solve(jobs, n_sub, chunk, sink)
                                        │                                    │  - sink = OPFS chunk file
                                        ▼                                    │
                        ┌─────────────────────────────────────────────┐    │
                        │  OPFS (Origin Private File System)           │◄───┘
                        │  projects/<id>/calc/<tensor_hash>/           │
                        │    manifest.json                             │
                        │    tensor/chunk_<idx>.bin  (re,im f64 LE)    │  ← FileSystemSyncAccessHandle
                        │    pincoh/chunk_<idx>.bin  (f64 LE)          │    (worker-only, sync write/flush/close)
                        └─────────────────────────────────────────────┘

     axum envi-service: serves the bundle with  COOP: same-origin + COEP: credentialless
     (makes the page crossOriginIsolated ⇒ SharedArrayBuffer available). Also: CORS byte proxy (Phase-8).
```

### Recommended Project Structure
```
crates/
├── envi-compute/              # NEW pure-Rust core (WASM-safe: no std::fs, no C, no rayon-in-lib)
│   └── src/
│       ├── lib.rs             # #![deny(unsafe_code)]
│       ├── job_assembly.rs    # PropagationPathInputs + sources → Vec<SolveJob>
│       │                      #   (balloon eval + eval_phase → directivity_gain_db/_rad)
│       ├── tiers.rs           # hierarchical receiver partition (points ⊂ coarse ⊂ fine)
│       ├── cost.rs            # cost estimate + guardrail thresholds (pure fn, SC1)
│       └── identity.rs        # manifest + tensor_hash + chunk_receivers, factored from envi-store
├── envi-compute-wasm/         # NEW thin cdylib (mirrors envi-gis-wasm)
│   └── src/
│       ├── lib.rs             # #[wasm_bindgen] boundary; initThreadPool re-export
│       ├── dto.rs             # ts-rs boundary DTOs → web/src/generated/wire.ts
│       ├── pool.rs            # rayon sharding over chunk ranges + cancel-flag checks
│       └── opfs_sink.rs       # TensorSink impl over FileSystemSyncAccessHandle
└── envi-store/                # MODIFIED: re-export identity types from envi-compute; keep fs I/O here
web/
└── src/
    ├── compute/
    │   ├── worker.ts          # dedicated Worker: JobStatus machine, initThreadPool, tier loop
    │   ├── client.ts          # main-thread API: submit / cancel / subscribe (postMessage)
    │   ├── cost.ts            # thin wrapper calling the wasm cost fn (or mirrors — prefer wasm)
    │   └── opfs.ts            # OPFS dir layout helpers (reuse Phase-8 web/src/import/opfs.ts)
    ├── generated/wasm-compute/ # git-ignored build artifact (threaded module + glue)
    └── panels/
        └── CalcPanel.tsx      # submit + cost estimate + progress + abort UI (WEB-07)
```

### Pattern: Caller-side rayon sharding (GRID-02 — the engine is NOT internally parallel)
**What:** The engine's `solve(jobs, n_sub, chunk_receivers, &mut sink)` is **sequential** — one reusable working buffer, receiver-major order, single `&mut dyn TensorSink`. Verified: `grep rayon crates/envi-engine/src` returns nothing. GRID-02's "parallelized (rayon)" is a *caller-side* obligation.
**When:** In the compute crate's pool driver, per tier.
**How:** Partition the tier's receiver axis into disjoint chunk ranges (each an integer number of `chunk_receivers`-sized chunks). `rayon`'s `par_iter` over the ranges; each task calls `solve()` on its own sub-iterator of jobs (receivers in that range only) with its **own OPFS sink writing its own chunk files**. Because ranges are disjoint → chunk indices are disjoint → files are disjoint → **no shared-mutable sink, no locking**. Merge is implicit (all files land in the same `calc/<hash>/tensor/` dir). This keeps `solve()` and the engine byte-identical.
```rust
// envi-compute-wasm/src/pool.rs (sketch — pure orchestration, no new physics)
use rayon::prelude::*;
fn solve_tier(ranges: &[ChunkRange], ctx: &SolveCtx, cancel: &AtomicBool) -> Result<(), ComputeError> {
    ranges.par_iter().try_for_each(|range| {
        if cancel.load(Ordering::Relaxed) { return Err(ComputeError::Cancelled); }
        let jobs = assemble_jobs(range, ctx);          // Rust: balloon eval + eval_phase here
        let mut sink = OpfsChunkSink::open(range.chunk_index, ctx)?; // its own file(s)
        envi_engine::solver::solve(jobs, ctx.n_sub, ctx.chunk_receivers, &mut sink)?;
        sink.flush_close()?;                           // FileSystemSyncAccessHandle.flush()+close()
        Ok(())
    })
}
```
**Trade-off:** + zero engine change, exact FORCE-validated code runs; + trivial parallel correctness (disjoint files). − the cancel check granularity is per-chunk-range, matching D-11 "abort at chunk boundaries".

### Pattern: Progressive hierarchical tier solve (D-05/06/07)
**What:** Emit results in resolution tiers; coarse points are a strict subset of fine (10 | 100), reused, not recomputed.
**How:** One global receiver axis holds ALL receivers (discrete ∪ coarse-lattice ∪ fine-lattice), each tagged with the coarsest tier it belongs to. Because `spacing_coarse = k · spacing_fine` with integer `k` and a shared lattice origin, a fine grid point at lattice index `(i,j)` is *also* a coarse point iff `i % k == 0 && j % k == 0`. Assign global receiver indices so each tier's set is contiguous-ish (or carry an explicit index list per tier). Solve order: (1) discrete points, (2) coarse-lattice points, (3) fine points NOT already in coarse. After each tier's chunk files are flushed, post a `TierComplete` event.
**tier-complete event payload (recommended schema):**
```ts
type TierComplete = {
  kind: "tier_complete";
  tier: "points" | "coarse" | "fine";
  tier_index: number;                 // 0,1,2
  spacing_m: number | null;           // null for discrete points
  tensor_hash: string;                // ties spans to the manifest identity (D-09)
  receiver_ids: string[];             // stable receiver UUIDs in this tier (Phase-11 lookup)
  spans: Array<{                      // where this tier's data lives in OPFS
    chunk_index: number;
    r_offset: number; len: number;    // receiver-axis span
    tensor_file: string;              // "tensor/chunk_00042.bin"
    pincoh_file: string;              // "pincoh/chunk_00042.bin"
  }>;
};
```
This carries everything Phase 11 needs to read the chunk files and render points → coarse map → refined map. **Phase 10 emits; Phase 11 renders.**

### Pattern: OPFS chunked TensorSink (D-08)
**What:** A `TensorSink` impl that writes each receiver-axis chunk to an OPFS file via `FileSystemSyncAccessHandle`.
**Constraints (verified):** `createSyncAccessHandle()` is **only available in a dedicated Web Worker** — it deliberately cannot be called on the main thread [CITED: developer.mozilla.org FileSystemFileHandle/createSyncAccessHandle]. It takes an **exclusive lock** on the file until `close()`. `write(buffer, {at})` returns bytes written; `read(buffer, {at})`; `flush()` commits to disk (optional but call it before `close()` for durability); `getSize()`; `truncate()`. Despite the "Sync" name, `createSyncAccessHandle()` itself returns a Promise; the read/write/flush/close on the returned handle are synchronous.
**Chunk byte format (frozen from ARCHITECTURE §Persistence Model):** per chunk file, layout `[s][r_local][f]` (freq-contiguous, row-major), `H_coh` as **interleaved (re, im) f64 little-endian** (16 B/cell), `P_incoh` as f64 LE (8 B/cell) in a parallel `pincoh/` file. This maps cleanly from the engine's `ArrayView3<Complex<f64>>` / `ArrayView3<f64>` chunk views (already row-major / standard layout — `tensor.rs` test `tensor_pair_is_row_major_frequency_contiguous`). In `put_chunk`, serialize the view to a `Vec<u8>` (or write directly from the ndarray's contiguous slice when standard-layout) and `write()` it to the handle.
**Rust reaches OPFS via** either `web-sys` (`FileSystemSyncAccessHandle`, feature-gated) OR a thin JS glue module imported over `wasm-bindgen` (see Q3). The handle lives in the worker; each rayon task opens its own handle for its own chunk file(s).

### Pattern: Web Worker job machine + reused JobStatus (D-10, SVC-02)
**What:** The dedicated worker runs the Queued→Running→Done/Failed/Cancelled machine and posts status via `postMessage`. The wire shape is the **reused** `JobStatus` enum (serialized identically to `envi-service/src/jobs.rs`), so the UI job model is uniform across server jobs (ERA5) and client jobs (solve).
**Why not SSE:** the solve is client-side; there is no server round-trip. The server SSE machine stays only for genuine server async (ERA5/CDS). The `JobStatus` *shape* is shared; the *transport* differs (postMessage vs SSE).
**Progress:** a `Running { progress, message }` posted per completed chunk-range (or throttled). Progress fraction = chunks_done / chunks_total across the whole calc (weight tiers by receiver count).

### Pattern: Cooperative abort at chunk boundaries (D-11)
**What:** A `SharedArrayBuffer`-backed `Int32Array` holds a cancel flag (and optionally a shared progress counter). Rayon tasks `Atomics.load` / `AtomicBool::load` the flag between chunk ranges; on set, they stop before the next `solve()` call, flush+close any open handle, and the tier loop returns `Cancelled`. **No `worker.terminate()`** — already-emitted tiers stay valid, OPFS handles close cleanly, and the pool is reusable for the next run. Matches SC2.
**Sharing across the pool:** the `SharedArrayBuffer` is created in the worker and its atomics view is captured by the Rust side (passed as a pointer into wasm shared memory, or read via a `js_sys::Int32Array` over the SAB). Because wasm-bindgen-rayon's memory IS a SharedArrayBuffer, an `AtomicBool` in wasm linear memory is already visible to all pool threads — the simplest design is a single `static AtomicBool` (or an `Arc<AtomicBool>`) in the wasm module, flipped by a `#[wasm_bindgen] pub fn request_cancel()` the worker calls.

### Pattern: Directional-phase seam wiring (SRC-03, Folded Todo)
**Where it lands:** the `SolveJob` assembly site in `envi-compute::job_assembly` — this phase is where the coherent directional-source composition path first exists. For each directional sub-source, at the src→rcv local direction `dir`:
```rust
// envi-compute/src/job_assembly.rs (the assembly site the deferred-items.md points to)
let gain = balloon.eval(dir_local);          // [f64; 105]  → directivity_gain_db
let phase = balloon.eval_phase(dir_local);   // [f64; 105]  → directivity_phase_rad (NEW wiring)
let job = SolveJob {
    /* ...profile, src, rcv, atmosphere, coh, axis, weather... */
    directivity_gain_db: Some(gain),
    directivity_phase_rad: if balloon.has_phase() { Some(phase) } else { None },
    forest, isolation, ..
};
```
`eval_phase` returns `[0.0; 105]` for a phase-free (magnitude-only) balloon, so gating on `has_phase()` keeps the road/incoherent path bit-identical (a phase-free balloon leaves `arg(H_coh)` untouched — solver already asserts this). **Add an end-to-end test:** rotating a phased balloon changes coherent inter-sub-source interference (not just level) — per deferred-items.md.

### Anti-Patterns to Avoid
- **Adding rayon or `#[wasm_bindgen]` to `envi-engine`.** Breaks the 3-dep quarantine and the FORCE-validated purity. Parallelism and the boundary live in the new crates only.
- **A shared-mutable `TensorSink` across rayon threads.** Unnecessary and forces locking. Use disjoint chunk ranges → disjoint files → per-task sinks.
- **Global `RUSTFLAGS=+atomics` via `.cargo/config.toml` or a workspace `rust-toolchain.toml`.** Would force the atomics/nightly build onto the stable `envi-gis-wasm` and the native engine/harness builds, breaking them. Scope nightly + RUSTFLAGS to the single `build:wasm:compute` command.
- **Calling `createSyncAccessHandle()` on the main thread.** It throws — it is worker-only. All OPFS sync I/O happens inside the compute worker.
- **A hand-authored TS mirror of a Rust DTO.** Forbidden (CLAUDE.md D-10). Boundary DTOs are ts-rs-generated into `web/src/generated/wire.ts` with the no-drift test, exactly like `envi-gis-wasm`.
- **`getrandom`/`uuid` in the wasm crate.** The existing gis-wasm crate mints ids in TS via `crypto.randomUUID()`. The compute crate follows suit (Pitfall 9).
- **COEP `require-corp`.** Breaks Phase-8 direct fetches; use `credentialless` (D-04).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Rayon-on-Web-Workers thread pool | A manual `Worker[]` + `postMessage` chunk dispatcher | `wasm-bindgen-rayon` | Handles worker spawning, SharedArrayBuffer wiring, `initThreadPool`, and pool teardown; reuses the engine's rayon code unchanged |
| The Nord2000 solve | Any browser-side re-composition of direct+ground+diffraction | `envi_engine::solver::solve` | Pattern 1: one solve path, FORCE-validated; Anti-Pattern 4 forbids a divergent solve |
| Tensor chunk streaming | A bespoke in-memory-then-spill buffer | The existing `TensorSink` trait + the receiver-axis chunk contract | Already bounds RSS to one chunk (OUT-06); `CountingSink` proves it |
| Tensor identity hash | A new hash over JSON | `envi-store::hash::tensor_hash` (factored to WASM-safe) | Frozen blake3 byte encoding (D-07); conditioning structurally excluded |
| Chunk-size math | A guessed receiver-per-chunk constant | `envi-store::manifest::chunk_receivers` (factored) | Derives from `DEFAULT_TENSOR_BUDGET_BYTES` / `BYTES_PER_CELL_PAIR` — never re-derive |
| Cross-origin-isolation dev headers | A custom dev-server middleware or a plugin | Vite `server.headers` config | Native Vite feature; no dependency |
| Directivity gain/phase interpolation | Any per-band interpolation in TS | `DirectivityBalloon::eval` / `eval_phase` | Engine-side, phase-preserving, already tested |

**Key insight:** Almost everything physics- and tensor-shaped already exists in the engine and is FORCE-validated. Phase 10 is a *plumbing* phase — a thin Rust boundary, a pool driver, an OPFS sink, a worker state machine, and two HTTP headers. The single most valuable discipline is to keep all new code out of `envi-engine` and make the browser call the exact validated `solve()`.

## Common Pitfalls

### Pitfall 1: The atomics RUSTFLAGS leaking into the stable builds
**What goes wrong:** Setting `+atomics,+bulk-memory` globally (e.g. `.cargo/config.toml [build] rustflags`) makes the existing stable `envi-gis-wasm` build and even native `cargo build`/`cargo test` try to use atomics-enabled std, which fails without `-Zbuild-std`/nightly.
**Why:** Cargo applies `[build]` RUSTFLAGS to every target.
**How to avoid:** Scope the flags to the single threaded-build command via env vars on the `build:wasm:compute` npm script (`RUSTFLAGS=... RUSTUP_TOOLCHAIN=nightly-… cargo build -Z build-std …`). Never add a workspace `rust-toolchain.toml`. Keep `build:wasm` (gis, stable) exactly as-is.
**Warning signs:** `cargo test` at the workspace root suddenly failing with "requires -Zbuild-std" or "atomics" errors after the phase; the stable gis-wasm bundle failing to build.

### Pitfall 2: `initThreadPool` not awaited before the first parallel call
**What goes wrong:** Calling into the pool before `await initThreadPool(n)` resolves → panic or single-threaded fallback.
**Why:** wasm-bindgen-rayon spawns the workers asynchronously; the pool is not ready until the promise resolves [CITED: docs.rs/wasm-bindgen-rayon].
**How to avoid:** In the worker init sequence: `await init(); await initThreadPool(navigator.hardwareConcurrency);` — once, before any `solve` call. Guard against double-init (one-shot pool).
**Warning signs:** intermittent "cannot spawn" panics; solves running on one core.

### Pitfall 3: Page not actually cross-origin isolated ⇒ no SharedArrayBuffer
**What goes wrong:** `SharedArrayBuffer` is `undefined` / `initThreadPool` fails because the response didn't carry both COOP and COEP, or an embedded resource violated COEP.
**Why:** Both `COOP: same-origin` AND `COEP: credentialless` must be present on the top-level document response, and `self.crossOriginIsolated` must be `true` [CITED: web.dev/articles/coop-coep].
**How to avoid:** axum sets both headers on the bundle response (prod) AND Vite `server.headers` sets them (dev + Playwright). Assert `self.crossOriginIsolated === true` at worker startup and surface a clear error if false. Verify Phase-8 direct fetches still succeed under `credentialless` (they should — credentialless strips credentials on no-cors, no CORP required).
**Warning signs:** `crossOriginIsolated` false in console; `SharedArrayBuffer is not defined`; basemap/import fetches failing only after headers land (would indicate `require-corp` was used by mistake).

### Pitfall 4: `createSyncAccessHandle` called outside a dedicated worker
**What goes wrong:** Throws `InvalidStateError` / not-a-function on the main thread or in a shared/service worker.
**Why:** Sync access handles are dedicated-worker-only by spec.
**How to avoid:** All OPFS sink I/O lives in the compute worker (or its rayon sub-workers). The main thread only issues `postMessage`.
**Warning signs:** OPFS writes throwing only when triggered from a UI callback path.

### Pitfall 5: Exclusive-lock contention on OPFS files
**What goes wrong:** Two tasks try to open a sync access handle on the *same* file → the second blocks/throws (exclusive lock until close).
**Why:** `createSyncAccessHandle` takes an exclusive lock.
**How to avoid:** The disjoint-chunk-range design guarantees one file per task. Ensure each rayon task's chunk index is unique; close the handle (`flush()` then `close()`) before the task returns so re-runs/reopens don't collide.
**Warning signs:** "already locked" / `NoModificationAllowedError`; hangs when re-running an identical scene (D-09 cache reopen).

### Pitfall 6: Comparing/aggregating tiers by frequency instead of band index
**What goes wrong:** Any tier-merge or receiver de-dup that keys on nominal Hz drifts against the 105-point 1/12-octave grid.
**Why:** CLAUDE.md house rule — compare by BAND INDEX, never nominal frequency.
**How to avoid:** All tensor spans are `[s][r_local][f=0..105]` by index; receiver identity is the UUID/position, never a frequency. The manifest `dims[2]` is always `N_BANDS = 105`.
**Warning signs:** off-by-one band shifts in Phase-11 readout; oracle mismatches at band edges.

### Pitfall 7: Non-standard-layout chunk views serialized wrong
**What goes wrong:** Writing an ndarray view assuming contiguous memory when it is a non-standard-layout slice → garbled bytes.
**Why:** `put_chunk` receives `ArrayView3` slices; a sub-view may not be contiguous.
**How to avoid:** In the OPFS sink, iterate in `[s][r_local][f]` order explicitly (or `.to_owned()`/`.as_standard_layout()` before grabbing the byte slice), matching the frozen `[s][r_local][f]` freq-contiguous format. Add a round-trip test (write chunk → read back → equals the `InMemorySink` result index-for-index).
**Warning signs:** Phase-11 readout garbage; a chunk round-trip test failing only for chunked (not single-chunk) solves.

### Pitfall 8: wasm-bindgen crate ↔ CLI version drift on the new module
**What goes wrong:** The threaded module is generated by a `wasm-bindgen-cli` version different from the `=0.2.126` crate pin → ABI mismatch.
**Why:** Same lockstep rule as `envi-gis-wasm` (web/README.md §Version lockstep).
**How to avoid:** Use the same pinned `wasm-bindgen-cli --version 0.2.126` for both modules; confirm `wasm-bindgen-rayon 1.3.0` resolves `wasm-bindgen` to `0.2.126` (`cargo tree -i wasm-bindgen`).
**Warning signs:** "invalid raw pointer" / import-mismatch panics at module init.

## Runtime State Inventory

> Phase 10 is greenfield compute plumbing, not a rename/refactor. The one migration-adjacent item is factoring shared types out of `envi-store`; recorded below for completeness.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | OPFS `calc/<hash>/` tensor chunks are NEW (no prior tensor data exists to migrate — Phase 6 only *reserved* empty `tensor/`+`pincoh/` dirs). Phase-8 OPFS GIS cache is separate and untouched. | None (new store); reuse Phase-8 `web/src/import/opfs.ts` helpers for the dir layout |
| Live service config | axum bundle-serving config gains 2 response headers; no external service config | Add COOP/COEP layer in `envi-service` |
| OS-registered state | None | None |
| Secrets/env vars | Threaded build needs `RUSTUP_TOOLCHAIN` + `RUSTFLAGS` set on one npm script (build-time env, not runtime secrets) | Document in web/README.md |
| Build artifacts | NEW `web/src/generated/wasm-compute/` (git-ignored, like the gis one); `envi-store` re-exports change its public surface (downstream `envi-service` recompiles) | Add to `.gitignore`; update `crates/README.md` I/O headers |

**Refactor — factor WASM-safe identity types out of `envi-store` (§in-scope):** `envi-store::manifest::{CalcManifest, chunk_receivers}` and `envi-store::hash::tensor_hash` are pure/serde types entangled with `std::fs` at the *module* level (`write_manifest`/`read_manifest` use `std::fs`, `project_dir::atomic_write`). Move the pure struct + `chunk_receivers` + `tensor_hash` (which already only needs `blake3`, `serde_json`, `geojson`, `uuid` — all WASM-safe) into the new `envi-compute::identity`; keep `write_manifest`/`read_manifest`/`atomic_write` (the `std::fs` I/O) in `envi-store`, re-exporting the moved types so `envi-store`'s public API is source-compatible. Verify `blake3` and `geojson` compile for `wasm32-unknown-unknown` (both are pure Rust — HIGH confidence they do; confirm at plan time).

## Cost Estimate Model (SC1)

**Receiver count** for a calc area of `A` m² at final spacing `s_fine` plus `D` discrete points:
```
N_fine    ≈ A / s_fine²                (grid points inside the calc area, building footprints subtracted)
N_receivers = D + N_fine               (coarse points are a SUBSET of fine — not additional)
```
Note tiers do NOT add receivers: coarse ⊂ fine (D-05). The estimate keys off `s_fine` (D-06).

**Tensor bytes** (using the engine's frozen constant `BYTES_PER_CELL_PAIR = 24`):
```
bytes = n_sub · N_receivers · 105 · 24        (H_coh 16 B + P_incoh 8 B per cell)
# e.g. 8 sub-sources × 100k receivers × 105 × 24 ≈ 2.0 GB on OPFS (H_coh 1.34 GB + P_incoh 0.67 GB)
```
This is the OPFS on-disk footprint (no compression). RAM working set is bounded separately: `workers × chunk_bytes` where `chunk_bytes = n_sub · chunk_receivers · 105 · 24` (SC3).

**Time estimate (device-adaptive, recommended):** a fixed per-pair constant is not portable across devices. Recommend a **calibration probe**: at submit time, time a small solve of `K` (s,r) pairs (e.g. K=256) on one worker → derive `t_pair` ms → extrapolate:
```
t_est ≈ (n_sub · N_receivers · t_pair) / n_workers
```
Fall back to a conservative built-in `t_pair` default if the probe is skipped. Report a range, not a point estimate. [ASSUMED — the exact `t_pair` default and probe size are Claude's discretion; calibrate during execution.]

**Guardrail ("halving spacing quadruples cost"):** since `N_fine ∝ 1/s²`, halving `s_fine` → 4× receivers → ~4× bytes and ~4× time. Surface, when the user edits spacing, the multiplicative delta vs the current setting. Recommend WARN thresholds (Claude's discretion — starting points):
- `tensor_bytes > 2 GB` (OPFS quota pressure — real quota is browser/disk-dependent; treat as a soft warn) → "large; may exceed browser storage."
- `t_est > 5 min` → "long run; consider a coarser final spacing."
- `N_receivers > 100k` → "very fine grid."
Block only on a hard OPFS quota check (`navigator.storage.estimate()`), never on the soft thresholds.

## Code Examples

### Reused JobStatus wire shape (client-side, from envi-service/src/jobs.rs:54)
```rust
// The SAME enum shape the worker posts and the UI consumes (D-10). JSON:
//   {"state":"running","progress":0.5,"message":"tier 2 · chunk 40/512"}
//   {"state":"done"} | {"state":"failed","reason":"…"} | {"state":"cancelled"}
// The client worker mirrors this shape; the ts-rs-generated wire.ts already has it.
```

### OPFS sink write path (worker-only; sketch)
```rust
// envi-compute-wasm/src/opfs_sink.rs — impl envi_engine::tensor::TensorSink
fn put_chunk(&mut self, r_offset: usize,
             h: ArrayView3<Complex<f64>>, p: ArrayView3<f64>) -> Result<(), SinkError> {
    // Serialize [s][r_local][f] freq-contiguous: interleaved (re,im) f64 LE for H, f64 LE for P.
    let mut hbuf = Vec::with_capacity(h.len() * 16);
    for z in h.iter() { hbuf.extend_from_slice(&z.re.to_le_bytes());
                        hbuf.extend_from_slice(&z.im.to_le_bytes()); }
    let mut pbuf = Vec::with_capacity(p.len() * 8);
    for v in p.iter() { pbuf.extend_from_slice(&v.to_le_bytes()); }
    self.tensor_handle.write_at(&hbuf, self.h_offset)?;   // FileSystemSyncAccessHandle.write
    self.pincoh_handle.write_at(&pbuf, self.p_offset)?;
    // (offsets advance; iteration order matches the frozen [s][r_local][f] layout)
    Ok(())
}
```
Note `h.iter()` on an `ArrayView3` yields elements in logical order; guard non-standard layout per Pitfall 7 (or `h.as_standard_layout()` first).

### axum COOP/COEP header layer (envi-service)
```rust
// crates/envi-service/src/api/mod.rs — wrap the bundle service (D-04)
use tower_http::set_header::SetResponseHeaderLayer;
use axum::http::{HeaderValue, header::HeaderName};
let coop = SetResponseHeaderLayer::overriding(
    HeaderName::from_static("cross-origin-opener-policy"),
    HeaderValue::from_static("same-origin"));
let coep = SetResponseHeaderLayer::overriding(
    HeaderName::from_static("cross-origin-embedder-policy"),
    HeaderValue::from_static("credentialless"));
Router::new().nest("/api/v1", api_router())
    .fallback_service(serve_dir)
    .layer(coop).layer(coep)          // on the bundle (and safe on API responses too)
    .with_state(state)
```
`tower-http` is already a dependency (`ServeDir`); confirm the `set-header` feature is enabled.

### Vite dev-server headers (dev + Playwright cross-origin isolation)
```ts
// web/vite.config.ts — add so `npm run dev` and Playwright get crossOriginIsolated
export default defineConfig({
  base: "./",
  plugins: [react()],
  server: { headers: {
    "Cross-Origin-Opener-Policy": "same-origin",
    "Cross-Origin-Embedder-Policy": "credentialless",
  }},
  // ...existing build config unchanged
});
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Server-side axum job runner + on-disk chunk store + SSE (ROADMAP/ARCHITECTURE) | Client-side threaded WASM + OPFS + worker postMessage (D-01) | Phase-8/10 discussion (2026-07) | The whole solve relocates to the browser; server is delivery + auth + CORS proxy only |
| COEP `require-corp` (original cross-origin-isolation recipe) | COEP `credentialless` | Chrome 96+/Firefox; standard by 2024 | Preserves direct third-party fetches without CORP headers (D-04) |
| Single-thread WASM or manual worker pools | `wasm-bindgen-rayon` + `initThreadPool` | mature since ~2021 | Reuses native rayon code in-browser |

**Deprecated/outdated:**
- The ROADMAP Phase-10 SC text ("on-disk tensor store", "SSE", "service stays healthy") — read as OPFS / postMessage / worker-pool-stays-healthy per CONTEXT D-01/D-08/D-10/D-11.
- `SharedArrayBuffer` without cross-origin isolation (the pre-Spectre era) — now requires COOP+COEP everywhere except where a Chrome enterprise policy/flag is set.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `wasm-bindgen-rayon 1.3.0` resolves `wasm-bindgen = "0.2"` and is compatible with the `=0.2.126` pin | Standard Stack | If it caps below 0.2.126, the pin must move (ripples to gis-wasm lockstep) — verify with `cargo tree -i wasm-bindgen` first |
| A2 | `blake3`, `geojson`, `serde_json`, `uuid` all compile for `wasm32-unknown-unknown` | Refactor | If not, `tensor_hash` can't run in-browser; would need a JS blake3 or a server hash call |
| A3 | `web-sys` exposes `FileSystemSyncAccessHandle` under a stable feature | OPFS pattern / Q3 | If gated/unstable, use JS glue for OPFS instead of `web-sys` (both viable) |
| A4 | The per-pair time constant is calibratable at submit time cheaply | Cost model | A bad estimate is only a UX wart (SC1 asks for "a time estimate", not a guaranteed bound) |
| A5 | Default `nightly-2026-01-15` (placeholder) supports `-Zbuild-std` for `wasm32-unknown-unknown` | Toolchain | Pick a known-good nightly at plan time; any recent nightly works |
| A6 | `rayon`'s global pool inside wasm is the pool `initThreadPool` sets up (one pool, not per-call) | Caller-side sharding | If mis-wired, tasks run single-threaded; validated by a worker-count assertion in a smoke test |
| A7 | OPFS quota is large enough for multi-GB tensors on target devices | Cost model / D-09 | Quota varies; the guardrail + `navigator.storage.estimate()` mitigate; eviction is deferred (CONTEXT) |

**If this table is emptied by verification at plan time, note it in the plan.** A1, A2, A3 are the load-bearing ones — each has a concrete verification command/fallback above.

## Open Questions

1. **`web-sys` OPFS types vs JS glue (Q3).**
   - What we know: OPFS sync access handles work only in a dedicated worker; both `web-sys` (feature-gated types) and a thin JS module invoked over wasm-bindgen can reach them.
   - What's unclear: whether the `web-sys` `FileSystemSyncAccessHandle` binding is ergonomic/stable enough vs a ~30-line JS glue that the Rust sink calls.
   - Recommendation: **Default to a thin JS glue** (`web/src/compute/opfs.ts`) exposing `open(path) → handle`, `write(handle, bytes, at)`, `flush`, `close`, and have the Rust sink call it via `wasm-bindgen` extern. It sidesteps `web-sys` feature churn and keeps the sink testable with a JS mock. Revisit `web-sys` if the glue proves chatty.

2. **Intermediate preview tier (points → 100 m → 10 m, or add an in-between?).**
   - What we know: D-06 allows an optional intermediate tier; coarse must divide fine.
   - What's unclear: whether a middle tier (e.g. 30 m — but 30 ∤ 10 breaks exact subset; use 50 m only if 50|10 fails → it doesn't; valid subset multiples of 10 are 20/30/40/50…, and coarse must ALSO be a multiple that fine subdivides — any `k·10` works as a coarse tier and its points are a subset of the 10 m grid).
   - Recommendation: **Start with two tiers after points: 100 m then 10 m** (k=10). Keep the tier list data-driven (`[k1, k2, …]`) so an intermediate (e.g. 50 m) is a one-line config, but don't over-build. Any `k·s_fine` coarse spacing keeps exact-subset reuse.

3. **Where the cost formula lives (Rust core vs TS).**
   - What we know: SVC-07 forbids client-side *acoustic* math, but the cost estimate is not acoustic — it's receiver-count/byte arithmetic.
   - Recommendation: put the formula in `envi-compute::cost` (pure Rust) and expose it over the boundary so there is one source of truth; a trivial TS mirror is acceptable ONLY for the instant slider feedback, but prefer calling the wasm fn. (Not a SVC-07 violation either way.)

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust nightly + `rust-src` | threaded `-Zbuild-std` | ✗ (verify) | — | `rustup toolchain install nightly && rustup component add rust-src --toolchain nightly` |
| `wasm32-unknown-unknown` target | wasm build | ✓ (used by gis-wasm) | — | `rustup target add wasm32-unknown-unknown` |
| `wasm-bindgen-cli` `=0.2.126` | boundary glue | ✓ (gis-wasm requires it) | 0.2.126 | `cargo install wasm-bindgen-cli --locked --version 0.2.126` |
| Chromium with crossOriginIsolated | Playwright threaded-WASM UAT | ✓ (Playwright chromium) | — | Vite `server.headers` provides COOP/COEP; assert `crossOriginIsolated` |
| OPFS + `FileSystemSyncAccessHandle` | tensor store | ✓ (Chromium/Playwright) | — | Feature-detect; error clearly if absent (Safari lacks credentialless anyway) |

**Missing dependencies with no fallback:** none (all have install/feature-detect paths).
**Missing dependencies with fallback:** nightly toolchain (install command above) — the only genuinely new build prerequisite.

## Security Domain

> `security_enforcement: true`, ASVS level 1. This phase runs untrusted-scale compute on the user's own device and writes to the user's own OPFS; the classic server threat surface is small (only 2 headers + existing bundle serving), but the WASM/worker/OPFS surface has its own concerns.

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V1 Architecture | yes | Cross-origin isolation is a security boundary — COOP/COEP correctly scoped; document the trust model (compute is client-side, no server-side execution of user scene) |
| V5 Input Validation | yes | The wasm boundary validates all marshalled inputs into typed errors (mirror `envi-gis-wasm`'s `from_js` + `GisError`); the engine's `SinkError`/`PropagationError` already reject non-finite/degenerate data — never panic on data |
| V6 Cryptography | partial | `blake3` tensor identity is integrity-only (not a security hash); no secrets — do not hand-roll, reuse the frozen encoding |
| V12 Files/Resources | yes | OPFS path keys derive from the manifest hash (hex) — never from unsanitized user strings → no path traversal in the OPFS dir layout; enforce hex-only chunk/dir names |
| V14 Configuration | yes | COOP/COEP headers set on the delivery server; verify they don't accidentally break Phase-8 fetches (credentialless, not require-corp) |

### Known Threat Patterns for client-side WASM + OPFS
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| SharedArrayBuffer/Spectre timing side-channel | Information disclosure | Cross-origin isolation (COOP+COEP) is exactly the mitigation the platform requires; enforced by the headers |
| Malformed marshalled input → panic/UB in wasm | Denial of service | Typed-error boundary (`from_js`→typed error), `#![deny(unsafe_code)]` on the core; engine already non-panicking on data |
| OPFS path injection via user-controlled names | Tampering | Key files by hex manifest hash + integer chunk index only; reject any non-hex/non-numeric component |
| Denial via runaway grid (OOM/quota) | Denial of service | Cost guardrail + `navigator.storage.estimate()` hard check + cooperative abort; RSS bounded by workers×chunk (SC3) |
| Unbounded job growth in the worker registry | Denial of service | Client-side single job at a time; supersede/cancel in-flight before starting a new solve (mirrors Phase-11 what-if supersede) |
| COEP misconfig re-enabling credentialed cross-origin loads | Information disclosure | `credentialless` strips credentials on no-cors; keep it (not `unsafe-none`); assert `crossOriginIsolated` |

**Note for the `/gsd-secure` gate:** the threat model here is mostly platform-provided (isolation headers) + the existing non-panicking boundary discipline; the net-new mitigations to verify are (a) hex-only OPFS keys, (b) the cost/quota guardrail, (c) `crossOriginIsolated` assertion, (d) credentialless (not require-corp).

## Sources

### Primary (HIGH confidence — read this session)
- `crates/envi-engine/src/solver.rs` — `solve()` is sequential (no rayon), `SolveJob` fields incl. `directivity_phase_rad`, chunk-streaming loop.
- `crates/envi-engine/src/tensor.rs` — `TensorSink` trait, `BYTES_PER_CELL_PAIR=24`, `DEFAULT_TENSOR_BUDGET_BYTES`, row-major freq-contiguous layout.
- `crates/envi-engine/src/directivity.rs` — `DirectivityBalloon::{eval, eval_phase, has_phase}` signatures.
- `crates/envi-store/src/{manifest.rs,hash.rs,lib.rs}` — `CalcManifest`, `chunk_receivers`, `tensor_hash`, the `std::fs` entanglement to factor out.
- `crates/envi-gis-wasm/{Cargo.toml,src/lib.rs}` — the cdylib boundary pattern, `=0.2.126` pin, ts-rs DTOs, no getrandom/uuid, Phase-9 grid/path shims.
- `crates/envi-service/src/{jobs.rs,api/mod.rs}` — `JobStatus` enum, `ServeDir` bundle serving, tower-http usage.
- `web/{package.json,vite.config.ts}`, `.planning/{ROADMAP.md,REQUIREMENTS.md,research/ARCHITECTURE.md}`, `10-CONTEXT.md`, `04-.../deferred-items.md`.

### Secondary (MEDIUM confidence — web-verified this session)
- [wasm-bindgen-rayon (docs.rs)](https://docs.rs/wasm-bindgen-rayon) + [crates.io](https://crates.io/crates/wasm-bindgen-rayon) + [GitHub RReverser/wasm-bindgen-rayon](https://github.com/RReverser/wasm-bindgen-rayon) — 1.3.0, initThreadPool, RUSTFLAGS/build-std/nightly requirement.
- [The wasm-bindgen Guide — Parallel Raytracing](https://rustwasm.github.io/docs/wasm-bindgen/examples/raytrace.html) — atomics/bulk-memory + `-Z build-std` recipe.
- [MDN — createSyncAccessHandle()](https://developer.mozilla.org/en-US/docs/Web/API/FileSystemFileHandle/createSyncAccessHandle) + [MDN OPFS](https://developer.mozilla.org/en-US/docs/Web/API/File_System_API/Origin_private_file_system) + [web.dev OPFS](https://web.dev/articles/origin-private-file-system) — worker-only, exclusive lock, write/flush/close.
- [MDN COEP](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Cross-Origin-Embedder-Policy) + [Chrome for Developers — COEP: credentialless](https://developer.chrome.com/blog/coep-credentialless-origin-trial) + [web.dev — coop-coep](https://web.dev/articles/coop-coep) — credentialless vs require-corp, crossOriginIsolated, no Safari support for credentialless.
- [web.dev — cross-origin isolation guide](https://web.dev/articles/cross-origin-isolation-guide) — verifying `self.crossOriginIsolated`, dev-server header caveats.

### Tertiary (LOW confidence — flagged as assumptions/open questions)
- Exact `rayon` latest version, nightly date, `web-sys` OPFS feature stability, per-pair time constant — all tagged `[ASSUMED]`, each with a verify-at-plan-time command or fallback.

## Metadata

**Confidence breakdown:**
- Repo seams / engine contracts / reuse plan: HIGH — grounded in the actual source read this session.
- Threaded-wasm toolchain (nightly, build-std, RUSTFLAGS, initThreadPool): MEDIUM — well-corroborated across official wasm-bindgen docs + the crate, but exact nightly date + wasm-bindgen-rayon↔0.2.126 resolution must be verified with `cargo tree` at plan time.
- OPFS sync-access + COOP/COEP: MEDIUM-HIGH — consistent across MDN/web.dev/Chrome docs.
- Cost time model + guardrail thresholds: LOW — Claude's discretion; calibrate during execution.

**Research date:** 2026-07-11
**Valid until:** 2026-08-10 (30 days; the wasm-bindgen-rayon / OPFS / COEP landscape is stable, but re-verify crate versions if planning slips)

## RESEARCH COMPLETE
