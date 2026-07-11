# Phase 10: Calculation Service - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-11
**Phase:** 10-calculation-service
**Areas discussed:** Compute locus (the fork), Parallelization mechanism, Progressive-tier structure, Tensor store, Job/progress/abort ownership, Auth scope

---

## Compute locus (the fork)

| Option | Description | Selected |
|--------|-------------|----------|
| Client-side WASM (browser) | Confirm the Phase-8 pivot for the heavy solve; server only auths + serves bundle | ✓ |
| Server-side axum (roadmap) | Keep the original ARCHITECTURE model; contradicts the "compute on the user's device" requirement | |
| Hybrid | Small grids client, large offloaded to server; two solve deployments | |

**User's choice:** Client-side — with two added hard requirements: **maximal parallelization** AND a **progressive solve emitting intermediate results** in resolution tiers: (1) single receiver-point spectra, (2) coarse grid 100×100 m, (3) fine grid 10×10 m; "make the solver loop so the coarse results are visible while the rest is calculated."
**Notes:** This reframed the phase from a job-with-progress-bar into an incrementally-refining solve and drove every subsequent decision.

---

## Parallelization mechanism

| Option | Description | Selected |
|--------|-------------|----------|
| rayon-in-WASM (wasm-bindgen-rayon) | SharedArrayBuffer thread pool; reuses engine rayon; needs cross-origin isolation (COEP: credentialless) | ✓ |
| Manual Web Worker pool | postMessage chunk sharding; no isolation, but a second parallelism model + serialization overhead | |
| Single worker, no threads | Off-main-thread but single-threaded; contradicts "maximally parallelized" | |

**User's choice:** rayon-in-WASM (wasm-bindgen-rayon).
**Notes:** Reuses the engine's existing caller-side rayon loop; one parallel model native + wasm; COEP credentialless chosen to preserve Phase-8 direct GIS/basemap fetches.

---

## Progressive-tier structure

| Option | Description | Selected |
|--------|-------------|----------|
| Nesting: Hierarchical (reuse coarse points) | Coarse 100 m points kept in fine tier; only ~99% gap points computed | ✓ |
| Nesting: Independent grids per tier | Each tier a full solve; recomputes coarse points | |
| Spacing: User sets final; previews auto-derived | Target (fine) spacing default 10 m; coarse tiers auto ×N | ✓ |
| Spacing: Fixed 100 m then 10 m | Hard-coded; no knob for other sites | |
| Spacing: User picks every tier | Most control, most misconfig risk | |
| Emission: Tier-complete partial results | "tier N done" event with receiver set + tensor spans | ✓ |
| Emission: Continuous chunk streaming | Smoothest but event churn, fuzzy "coarse ready" | |
| Emission: Only final result | Contradicts "coarse visible while calculating" | |

**User's choice:** Hierarchical reuse · user-set final spacing (default 10 m) with auto-derived coarse previews · tier-complete partial-result emission.
**Notes:** 10 divides 100, so coarse points are an exact subset of the fine grid — reuse is exact, no interpolation.

---

## Tensor store: OPFS vs memory

| Option | Description | Selected |
|--------|-------------|----------|
| Location: OPFS chunked (single path) | wasm OPFS TensorSink, receiver-axis chunks, blake3 manifest; off-heap; survives reload | ✓ |
| Location: In-WASM-memory with spill | Two code paths; fine grid always spills | |
| Location: In-memory only (ephemeral) | Reload loses it; breaks cached-tensor premise | |
| Lifetime: Persist per project, keyed by manifest hash | Reuse identical scenes; per-scenario tensors for Phase 11 | ✓ |
| Lifetime: Evict on close / keep last-N | Bounds disk but discards the recalc cache | |

**User's choice:** OPFS chunked single path · persist per project keyed by manifest hash.
**Notes:** 1.34 GB H_coh at 100k×8×105 exceeds reliable wasm linear-memory heap → off-heap OPFS is the natural store; working set stays workers × chunk (satisfies SC3).

---

## Job / progress / abort ownership

| Option | Description | Selected |
|--------|-------------|----------|
| Owner: Client worker; reuse JobStatus; server machine for ERA5 | Worker runs the state machine; server SSE reserved for server-async | ✓ |
| Owner: Client worker; delete server job machine | Discards the ERA5/CDS async path Phase 9 scaffolded | |
| Owner: Keep server authoritative | Network round-trip for a compute the server isn't doing | |
| Abort: Cooperative atomic flag at chunk boundaries | SharedArrayBuffer flag; partial tiers stay valid; pool reusable | ✓ |
| Abort: Terminate the worker outright | Abrupt; risks half-written OPFS chunks; rebuild pool | |

**User's choice:** Client worker owns it, reuse JobStatus vocabulary, server machine reserved for ERA5 · cooperative atomic cancel flag at chunk boundaries.

---

## Auth scope

| Option | Description | Selected |
|--------|-------------|----------|
| Headers + bundle only; defer real auth | COOP/COEP + wasm-bundle serving; auth is its own phase | ✓ |
| Include a minimal auth gate now | Broadens Phase 10 into auth infra | |
| Neither — headers elsewhere | Threaded solve can't run without isolation; not demonstrable | |

**User's choice:** Headers + bundle serving only; defer real auth to a dedicated phase.

---

## Claude's Discretion

- Chunk size + worker-pool sizing heuristic; cost-estimate time model + guardrail thresholds;
  intermediate preview-tier count/spacings; OPFS directory layout + manifest naming; tier-complete
  event payload schema; atomic cancel-flag / progress-counter sharing; threaded-wasm build toolchain
  (atomics flags, bundler wiring for wasm-bindgen-rayon).

## Deferred Ideas

- PROJECT.md / ARCHITECTURE.md deployment-model amendment pass (recommended around this phase).
- Real authentication / login gate (own phase, own threat model).
- Interactive fast-recalc MAC + results rendering (Phase 11).
- GRID-03 L_den weather-class combination (beyond Milestone 2).
- OPFS quota / eviction strategy (only if quota strains).

## Folded Todos

- Directional-phase wiring (SRC-03): populate `SolveJob::directivity_phase_rad` from
  `DirectivityBalloon::eval_phase` at the WASM `SolveJob` assembly site — this phase is where the
  coherent directional-source composition path lands.
