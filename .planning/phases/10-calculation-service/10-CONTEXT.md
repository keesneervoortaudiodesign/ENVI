# Phase 10: Calculation Service - Context

**Gathered:** 2026-07-11
**Status:** Ready for planning

<domain>
## Phase Boundary

A user runs a **real Nord2000 calculation end-to-end** — submit from the UI with a pre-run
cost estimate, watch progress, abort cleanly — with the complex transfer tensor streamed to a
**chunked store inside a stated memory budget**, and forests + semi-transparent screens/façades
(ENG-09/10) computed with their effects present in the results. Requirements **SVC-02, GRID-02,
WEB-07**.

**Execution model (resolved this discussion — supersedes the ROADMAP/ARCHITECTURE server-side text):**
the heavy grid **solve runs client-side in the browser as threaded WebAssembly**, on the user's own
device. The server only authenticates + serves the (cross-origin-isolated) app bundle and proxies
CORS-blocked fetches. This confirms and extends the Phase-8 pivot (08-CONTEXT D-01) from GIS decode
to the calc itself.

**In scope:**
- A browser **WASM compute crate** wrapping `envi_engine::solver::solve` (the same promoted solve the
  FORCE suite validates), driven from a **Web Worker** so the UI stays live.
- **Maximal parallelism** via **rayon-in-WASM (wasm-bindgen-rayon)**: a SharedArrayBuffer thread pool
  sized to `navigator.hardwareConcurrency`, reusing the engine's existing (caller-side) rayon loop
  over receiver chunks.
- A **progressive / staged solve** that emits results in resolution tiers so coarse results are
  visible while the rest computes: **(1)** discrete receiver-point spectra → **(2)** coarse grid
  (default 100 m) → **(3)** fine grid (default 10 m), hierarchically refined (coarse points reused,
  only gaps filled).
- An **OPFS-backed chunked `TensorSink`** (new impl of the existing engine trait) mirroring the
  flat-binary, receiver-axis, freq-contiguous chunk format, with blake3 + manifest identity.
- A **client-side job state machine** (in the worker) reusing the Phase-6 `JobStatus` vocabulary:
  submit, live progress + tier-complete events, cooperative **abort at chunk boundaries**, `Failed(reason)`.
- A **pre-run cost estimate + guardrail** (receiver count, tensor bytes, time estimate) keyed off the
  final grid spacing (SC1).
- Wiring **ENG-09/10** (forest + isolation-spectrum objects) and the **directional-phase seam**
  (pending todo, see Folded Todos) into the `SolveJob` construction site so drawn objects are never
  silently inert.
- The delivery server (`envi-service`) sends **COOP: same-origin + COEP: credentialless** and serves
  the wasm bundle (it already serves `web/dist`).

**Out of scope (later phases / dedicated work):**
- **Real authentication / login gate / sessions / accounts** — deferred to its own phase (only the
  isolation headers + bundle serving land here).
- **Results rendering** — spectral panels, isophone maps, editable color scale, difference maps
  (Phase 11). Phase 10 *computes and emits* tiered partial results; Phase 11 *renders* them.
- **Interactive fast-recalc / conditioning MAC over the cached tensor** (Phase 11) — Phase 10 produces
  and persists the tensor the MAC reads.
- **The PROJECT.md / ARCHITECTURE.md amendment pass** (the deployment-model rewrite) — recommended as a
  coordinated doc task; the binding decision is recorded here, the doc edits are a follow-up.

**Depends on (hard gates):**
- **Engine Phase 4** — the promoted `envi_engine::solver::solve` + `TensorSink`/`TensorPair` seam +
  MAC readout (all present).
- **Phase 5** — forest (SM10) + isolation-spectrum extensions computed inside the solve.
- **Phase 9** — `PropagationPathInputs` / `PathCacheKey` (the GIS→engine path seam feeding real paths).
- **Phase 6** — `JobStatus` vocabulary, blake3 tensor-identity hash + manifest, the axum bundle server.

</domain>

<decisions>
## Implementation Decisions

### Compute locus (the fork — resolved)
- **D-01:** The **heavy grid solve runs client-side as threaded WebAssembly** on the user's device.
  The server only authenticates + serves the bundle (+ CORS proxy). This confirms the Phase-8 pivot
  (08-CONTEXT D-01) for the *compute* path, not just GIS decode. The ROADMAP/ARCHITECTURE "axum job
  runner, rayon threads, on-disk chunk store, SSE" text is **superseded** for the solve path — its
  crate topology, chunk-format, memory math, and Pattern-1 "one solve path, two callers" invariant
  still hold, just relocated into the browser.
- **D-02:** A new **browser WASM compute crate** wraps `envi_engine::solver::solve` (marshalling only,
  following the existing `envi-gis-wasm` cdylib pattern). The engine's 3-dep quarantine
  (`ndarray + num-complex + thiserror`) is untouched; rayon stays caller-side.

### Parallelization
- **D-03:** **rayon-in-WASM via `wasm-bindgen-rayon`** — a SharedArrayBuffer-backed pool sized to
  `navigator.hardwareConcurrency`, reusing the engine's existing rayon parallelism verbatim (one
  parallel model native + wasm, validated by the same FORCE path). Requires a threaded/atomics wasm
  build.
- **D-04:** Cross-origin isolation via **COOP: same-origin + COEP: credentialless** (not `require-corp`)
  so the Phase-8 **direct** GIS/basemap fetches keep working without every third-party source sending
  CORP headers. `envi-service` emits these headers on the app bundle.

### Progressive-tier solve
- **D-05:** **Hierarchical refinement.** The coarse 100 m points are a strict subset of the 10 m fine
  grid (10 | 100), so coarse points are kept as-is and the fine pass computes **only the ~99% of gap
  points** — no recompute. Tier order: discrete receiver points → coarse grid → fine grid.
- **D-06:** **User sets the final (fine) spacing**, default **10 m**; the **preview tiers are
  auto-derived** as coarser multiples (e.g. ×10 → 100 m, optionally an intermediate). The cost
  estimate/guardrail (SC1) keys off the final spacing.
- **D-07:** The solver **emits tier-complete partial results** — a "tier N done" event carrying that
  tier's receiver set + stored tensor spans — so Phase 11 can render points, then the coarse map, then
  the refined map. Phase 10 owns compute + emission; Phase 11 owns rendering.

### Tensor store (client-side)
- **D-08:** **OPFS chunked, single path.** A wasm OPFS-backed `TensorSink` writes receiver-axis chunks
  (interleaved re,im f64 LE, freq-contiguous `[s][r_local][f]`) via `FileSystemSyncAccessHandle` in the
  worker. Full tensor stays off-heap; working set = workers × chunk (same bound as native, satisfying
  SC3). No dual in-memory/spill path. (`InMemorySink` remains the native/test sink.)
- **D-09:** **Persist per project, keyed by manifest hash** (scene geometry, met, receiver set, engine
  version, band axis — the blake3 identity `envi-store` already computes, SC4). Reopening the project
  or re-running an identical scene reuses the tensor; Phase-11 named weather scenarios each get their
  own hash-keyed tensor. No eviction-on-close in this phase (revisit only if OPFS quota bites).

### Job / progress / abort
- **D-10:** The **Web Worker hosting the rayon pool owns the compute job lifecycle** and posts progress
  + tier-complete events to the UI. **Reuse the Phase-6 `JobStatus` enum/wire shape** client-side so the
  UI job model is uniform. The **server SSE job machine stays only for genuine server-side async
  (ERA5/CDS)**, never the solve — it is repurposed, not deleted.
- **D-11:** **Abort is cooperative at chunk boundaries** — a SharedArrayBuffer-backed atomic cancel flag
  the rayon workers check between receiver chunks. Cancellation lands cleanly at a chunk boundary,
  already-emitted tiers (points, coarse) stay valid, OPFS handles close cleanly, and the pool is
  reusable for the next run. No `worker.terminate()`. Matches SC2 (lands `Cancelled`, stays healthy).

### Auth / delivery-server scope
- **D-12:** **Headers + bundle serving only** land here (the minimum the threaded solve needs). Real
  auth (login gate, sessions, accounts) is a **separate, deferred phase** with its own threat-model +
  secure-phase pass.

### Claude's Discretion
- Exact chunk size (receiver count per chunk) and the worker-pool sizing heuristic; the cost-estimate
  time model + the guardrail warning thresholds ("halving spacing quadruples cost"); the intermediate
  preview-tier count/spacings between points and fine; OPFS directory layout + manifest-file naming
  within the project dir; the tier-complete event payload schema (as long as it carries enough for
  Phase-11 rendering); how the atomic cancel flag + progress counters are shared across the pool; the
  threaded-wasm build toolchain details (atomics flags, bundler wiring for `wasm-bindgen-rayon`).

### Folded Todos
- **Wire directional phase into the coherent composition path (SRC-03).** `DirectivityBalloon` carries
  an optional per-band phase grid (`eval_phase`/`eval_complex`); the solver applies it via
  `SolveJob::directivity_phase_rad` to `H_coh` only (ENVI extension beyond stock Nord2000). **No
  `SolveJob` construction site populates it yet.** Phase 10 is where the coherent directional-source
  composition path lands, so **populate `directivity_phase_rad` at the WASM `SolveJob` assembly site**
  here. Full how-to in
  `.planning/phases/04-transfer-tensor-directional-sources-full-validation/deferred-items.md`
  ("Directional phase seam"). Backward-compatible: a phase-free balloon leaves `arg(H_coh)` bit-identical.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements + roadmap
- `.planning/ROADMAP.md` "Phase 10: Calculation Service" — goal + SC1–SC4 (cost estimate + guardrails,
  live progress + chunk-boundary abort, rayon-parallel receiver-chunk streaming within a memory budget,
  manifest content hashes + ENG-09/10 visible in results). **Note:** its "server-side/axum/SSE" framing
  is superseded by D-01 (client-side WASM); topology/memory-math/invariants still apply, relocated.
- `.planning/REQUIREMENTS.md` — **SVC-02** (compute-job model), **GRID-02** (receiver-grid
  computation), **WEB-07** (submit/progress/abort UI) — exact requirement wording.

### The authoritative architecture (read against the pivot)
- `.planning/research/ARCHITECTURE.md` — the tensor chunk format + receiver-axis memory math
  (§Persistence Model), **Pattern 1 "promote-the-solver / one solve path, two callers"**, the
  `TensorSink` trait shape (§Pattern 1), the recalc tiers, and Anti-Pattern 5 (long CPU solves).
  **⚠ Read against D-01/D-03/D-08:** on-disk `calc/<id>/tensor/` → **OPFS**; axum rayon job runner →
  **browser Web Worker + wasm-bindgen-rayon**; SSE progress → **worker postMessage**. Treat the format
  + memory bound + solver-promotion invariant as binding, the server-deployment as overridden.

### The pivot + prior-phase contracts this phase binds to
- `.planning/phases/08-gis-ingestion-dgm/08-CONTEXT.md` — **D-01/D-02/D-03**: the WASM client-side
  pivot, CORS-as-gatekeeper (direct fetch + login-server byte proxy fallback), OPFS per-project cache
  with the "reads only cache with network off" guarantee. Phase 10 extends this from decode to compute.
- `.planning/phases/06-service-foundation-persistence/06-CONTEXT.md` — the `JobStatus`
  Queued/Running/Done/Failed/Cancelled machine (06-04), the recondition/recompute split + 409
  tensor-hash gate, the blake3 tensor-identity hash + calc manifest (06-02). D-10 reuses this
  vocabulary client-side; the server machine is reserved for ERA5-style async.
- `.planning/phases/04-transfer-tensor-directional-sources-full-validation/deferred-items.md` —
  the **"Directional phase seam"** how-to (Folded Todos) + the `TensorSink` file-backed-store seam note.

### Existing code to build against (verify, do not break)
- `crates/envi-engine/src/solver.rs` — `pub fn solve<'a, I>(…)` + `pub struct SolveJob<'a>` (line 63):
  the single promoted solve path both the FORCE harness and the WASM crate call. Populate
  `directivity_phase_rad` here (Folded Todos).
- `crates/envi-engine/src/tensor.rs` — `pub trait TensorSink` (line 166) + `put_chunk` (receiver-axis
  chunks), `TensorPair`, `InMemorySink`/`CountingSink`, `readout_coherent`/`readout_incoherent`,
  `compose_gain` — the OPFS sink implements this trait; the readout/MAC is Phase-11's consumer.
- `crates/envi-store/src/{manifest.rs,hash.rs}` — blake3 tensor-identity + calc manifest (WASM-safe
  parts to reuse for the OPFS store identity). **Note:** `envi-store::project_dir` is `std::fs`
  (native, not WASM-safe) — factor the pure format/manifest types from the fs I/O for browser reuse.
- `crates/envi-service/src/jobs.rs` — the server `JobStatus` machine (`submit_stub_job`,
  `watch::channel` + `CancellationToken` + SSE) — repurposed for ERA5, the wire enum reused client-side.
- `crates/envi-gis-wasm/` — the repo's first WASM cdylib (wasm-bindgen pinned `=0.2.126`, no
  getrandom/uuid, ts-rs boundary DTOs into `web/src/generated/wire.ts`) — the **pattern** the new
  compute-wasm crate follows.

### Binding project contracts
- `.claude/CLAUDE.md` — engine 3-dep quarantine (byte-identical, no new engine deps), the single
  `.conj()` boundary + conj-grep-gate-zero in `propagation/`, the 105-point **band-index** framework
  (compare by band index, never nominal Hz), Playwright offline-UAT rules, English-only output, GitHub
  conventions, and the **five mandatory GSD phase-completion gates**.
- `.planning/PROJECT.md` — product vision + the two-channel `H_coh`/`P_incoh` contract. **⚠ Its
  "self-hosted localhost binary, light/no auth" deployment statement is superseded** by D-01/D-12 and
  needs the deferred amendment pass.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`envi_engine::solver::solve` + `SolveJob`** — the promoted, FORCE-validated solve; the WASM crate
  is a thin marshalling caller (Pattern 1). ENG-09/10 (forest, isolation) already compute inside it.
- **`envi_engine::tensor::TensorSink`** — the exact seam for the new OPFS-backed sink; `InMemorySink`
  stays for native/tests, `CountingSink` proves the memory bound.
- **`envi-store` blake3 hash + manifest** — the tensor-identity + manifest schema to reuse for OPFS
  store keying (factor out the WASM-safe pure types).
- **`envi-gis-wasm` cdylib pattern** — wasm-bindgen version pin, ts-rs boundary DTOs into the single
  committed `web/src/generated/wire.ts`, no-drift test — the compute-wasm crate mirrors this.
- **Phase-6 `JobStatus` enum** — reused verbatim as the client-side job wire shape.
- **Phase-9 `PropagationPathInputs` / `PathCacheKey`** — the real-world path inputs feeding `SolveJob`.

### Established Patterns
- **One solve path, two callers** (ARCHITECTURE Pattern 1) — now three callers (harness + native +
  wasm), all thin; no divergent service-side solve (Anti-Pattern 4).
- **Native-Rust-over-FFI, absolute on the WASM path** — no C in the browser; the compute crate is
  pure Rust + wasm-bindgen + wasm-bindgen-rayon.
- **One source of truth, drift made structurally impossible** — generated wire DTOs (ts-rs), reused
  `JobStatus`, reused manifest identity; no hand-mirrored TS.
- **Honest states, no false green** — `Failed(reason)`, chunk-boundary `Cancelled`, OPFS "reads only
  cache network-off" lineage from Phase 8.
- **Quality gates:** `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `cargo test`;
  Playwright offline (intercept all network). Add a threaded-wasm build + `cargo test` on the new crate.

### Integration Points
- New **compute-wasm crate** ⇄ Web Worker ⇄ `wasm-bindgen-rayon` pool ⇄ OPFS `TensorSink`.
- Worker ⇄ main-thread UI: postMessage job status + tier-complete events (reused `JobStatus` shape).
- `envi-service`: add COOP/COEP headers + wasm-bundle serving; keep the SSE job machine for ERA5 only.
- `SolveJob` assembly: fed by Phase-9 `PropagationPathInputs`; populate `directivity_phase_rad`.

</code_context>

<specifics>
## Specific Ideas

- The defining requirement of this discussion: **"client side, but make sure everything is maximally
  parallelized AND solve so that intermediate results are visible — (1) single receiver-point spectra,
  (2) coarse grid points (100×100 m), (3) finer grid points (10×10 m); make the solver loop so the
  coarse results are visible while the rest is calculated."** This directly drove D-03 (real threads,
  not a single worker), D-05/D-06/D-07 (hierarchical coarse→fine tiers with tier-complete emission),
  and the reframing of the phase from "run a job + progress bar" into "an incrementally-refining solve."
- Coarse-point reuse is exact because **10 divides 100** — no interpolation, no recompute; the coarse
  tensor spans are literally the same receivers, kept in the growing OPFS store.
- Cross-origin isolation chosen as **credentialless** specifically to protect the Phase-8 *direct*
  fetch path (basemap + AHN/Overpass) that `require-corp` would break.

</specifics>

<deferred>
## Deferred Ideas

- **PROJECT.md / ARCHITECTURE.md amendment pass** — rewrite the deployment model from "self-hosted
  localhost binary, light/no auth" to "client-side WASM compute + login/delivery server (real auth)."
  Strongly recommended as a coordinated doc task around this phase; the binding decision is recorded in
  this CONTEXT, but the docs still say server-side. (Carried from 08-CONTEXT `<deferred>`.)
- **Real authentication / login gate** — sessions, accounts, the "only runnable when logged in" gate.
  Its own phase with a dedicated threat model + secure-phase pass (D-12). Only COOP/COEP headers +
  bundle serving land in Phase 10.
- **Interactive fast-recalc / conditioning MAC + results rendering** — Phase 11 (spectra panels,
  isophone maps + editable color scale, difference maps, named weather scenarios, exports). Phase 10
  produces + persists the tensor these consume.
- **GRID-03 L_den weather-class combination** — deferred beyond Milestone 2 (unmapped).
- **OPFS quota / eviction strategy** — only revisit if per-project persistent tensors actually strain
  the OPFS quota (D-09 keeps everything for now).

### Reviewed Todos (not folded)
None — the only matching pending todo (directional-phase wiring) was folded into scope above.

</deferred>

---

*Phase: 10-Calculation Service*
*Context gathered: 2026-07-11*
