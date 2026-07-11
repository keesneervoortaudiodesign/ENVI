# Phase 10: Calculation Service - Pattern Map

**Mapped:** 2026-07-11
**Files analyzed:** 20 new/modified (7 Rust core, 4 Rust wasm-boundary, 6 web TS/TSX, 1 store, 2 config)
**Analogs found:** 18 / 20 (2 partial — OPFS sync-sink + wasm thread pool have no in-repo analog)

This phase is a *plumbing* phase. Almost every new file has a strong existing analog in the
repo; the two genuinely-new shapes (an OPFS `FileSystemSyncAccessHandle` sink, and a
`wasm-bindgen-rayon` pool driver) are called out under **No Analog Found** with the nearest
partial references. The single most load-bearing invariant across the whole phase: **new code
must never touch `envi-engine`** — the 3-dep quarantine and FORCE-validated `solve()` stay
byte-identical; parallelism, the wasm boundary, and the OPFS sink all live in the NEW crates.

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/envi-compute/src/lib.rs` | crate-root / config | — | `crates/envi-gis/src/lib.rs` (pure core crate root) | role-match |
| `crates/envi-compute/src/job_assembly.rs` | service (assembly) | transform | `crates/envi-engine/src/solver.rs` `SolveJob` construction (tests) | exact |
| `crates/envi-compute/src/identity.rs` | model / util | transform | `crates/envi-store/src/{hash.rs,manifest.rs}` (factor-out) | exact |
| `crates/envi-compute/src/tiers.rs` | service | transform | `crates/envi-gis/src/grid.rs` (receiver-grid partition) [ASSUMED] | role-match |
| `crates/envi-compute/src/cost.rs` | util (pure fn) | transform | `crates/envi-store/src/manifest.rs` `chunk_receivers` (byte-math pure fn) | role-match |
| `crates/envi-compute/Cargo.toml` | config | — | `crates/envi-gis/Cargo.toml` (pure-Rust core) | role-match |
| `crates/envi-compute-wasm/src/lib.rs` | controller (wasm boundary) | request-response | `crates/envi-gis-wasm/src/lib.rs` | exact |
| `crates/envi-compute-wasm/src/dto.rs` | model (wire DTOs) | transform | `crates/envi-gis-wasm/src/dto.rs` | exact |
| `crates/envi-compute-wasm/src/pool.rs` | service (rayon driver) | batch | `crates/envi-engine/src/solver.rs` `solve()` loop (caller side) | role-match |
| `crates/envi-compute-wasm/src/opfs_sink.rs` | provider (I/O sink) | file-I/O / streaming | `crates/envi-engine/src/tensor.rs` `InMemorySink`/`CountingSink` (trait impl) | role-match |
| `crates/envi-compute-wasm/Cargo.toml` | config | — | `crates/envi-gis-wasm/Cargo.toml` | exact |
| `crates/envi-store/src/{manifest.rs,hash.rs}` (MODIFIED) | model (re-export) | — | itself (self-refactor: keep fs, re-export moved types) | exact |
| `crates/envi-service/src/api/mod.rs` (MODIFIED) | config (headers) | request-response | itself `app()` (`ServeDir` mount) + `jobs.rs` reuse | exact |
| `web/src/compute/worker.ts` | worker (job machine) | event-driven / pub-sub | `crates/envi-service/src/jobs.rs` `run_stub_job` (state machine) + `web/src/import/importJob.ts` | role-match |
| `web/src/compute/client.ts` | service (main-thread API) | request-response | `web/src/import/wasm.ts` (typed facade + lazy init) | role-match |
| `web/src/compute/opfs.ts` | provider (OPFS glue) | file-I/O | `web/src/import/opfs.ts` | role-match |
| `web/src/compute/cost.ts` | util | transform | `web/src/import/wasm.ts` (thin wasm-fn wrapper) | role-match |
| `web/src/store/calc.ts` | store (zustand slice) | event-driven | `web/src/store/import.ts` | exact |
| `web/src/panels/CalcPanel.tsx` | component | request-response | `web/src/panels/ImportPanel.tsx` | exact |
| `web/vite.config.ts` (MODIFIED) | config (dev headers) | — | itself (add `server.headers`) | exact |

---

## Pattern Assignments

### `crates/envi-compute-wasm/src/lib.rs` (controller, request-response)

**Analog:** `crates/envi-gis-wasm/src/lib.rs` — copy the thin-boundary discipline verbatim.

**Crate-doc + deny + module layout** (analog lib.rs:1-30): the boundary crate carries the same
"Boundary ONLY — no logic here" doc header, `#![deny(unsafe_code)]`, `pub mod dto;`, and the
"No `getrandom`/`uuid` (ids minted in TS via `crypto.randomUUID()`)" note. Replicate that header
for the compute crate, adding the `initThreadPool` re-export note.

**Marshalling helper trio** (analog lib.rs:71-100) — copy `from_js` / `to_js` / `js_err`
verbatim; they are the only glue and must not diverge:
```rust
fn from_js<T: DeserializeOwned>(v: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(v).map_err(|e| js_err(&e.to_string()))
}
fn to_js<T: Serialize>(v: &T) -> Result<JsValue, JsValue> {
    let serializer = serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true);
    v.serialize(&serializer).map_err(|e| js_err(&e.to_string()))
}
fn js_err(msg: &str) -> JsValue { JsError::new(msg).into() }
```

**Per-function boundary shape** (analog lib.rs:330-351, `plan_import`) — each `#[wasm_bindgen]`
export does exactly three things: `from_js` → call one `envi_compute::` core fn → `to_js`. The
compute crate's exports (`estimate_cost`, `plan_tiers`, `solve_chunk_range`, `request_cancel`)
follow this exact skeleton. Errors map through a typed `ComputeError`→`JsValue` helper mirroring
`gis_err` (analog lib.rs:98-100).

**wasm-trap non-panicking contract:** the core is `#![deny(unsafe_code)]` and returns typed
errors, never panics on data (mirrors the `GisError` boundary). The engine `solve()` already
returns `Result<_, PropagationError>` (solver.rs:139).

---

### `crates/envi-compute-wasm/Cargo.toml` (config)

**Analog:** `crates/envi-gis-wasm/Cargo.toml` — mirror the pin + `crate-type` exactly.

**`[lib]` crate-type** (analog Cargo.toml:18-19) — `crate-type = ["cdylib", "rlib"]`. The `rlib`
is required so the native `cargo test` (no-drift DTO test) sees the boundary; the wasm bundle
uses only `cdylib`.

**wasm-bindgen exact pin** (analog Cargo.toml:37-46):
```toml
wasm-bindgen = "=0.2.126"      # EXACT pin; wasm-bindgen-cli must match (Pitfall 8)
js-sys = "0.3"
serde-wasm-bindgen = "0.6.5"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
ts-rs = { version = "12", features = ["no-serde-warnings"] }
```
**Add for Phase 10** (per RESEARCH §Standard Stack): `wasm-bindgen-rayon = "1.3.0"`,
`rayon = "1.10"`, `envi-compute = { path = "../envi-compute" }`, `envi-engine = { path = "../envi-engine" }`,
and `web-sys` with OPFS features **only if** using `web-sys` over JS glue (Open Question Q3
recommends JS glue → then `web-sys` may not be needed here). **Do NOT** add `getrandom`/`uuid`
(analog Cargo.toml:14-17 comment). Verify `cargo tree -p envi-compute-wasm -i wasm-bindgen`
resolves to `0.2.126` (RESEARCH A1).

---

### `crates/envi-compute-wasm/src/dto.rs` (model, wire DTOs)

**Analog:** `crates/envi-gis-wasm/src/dto.rs` (ts-rs `#[derive(TS)]`, `#[ts(export_to = "wire.ts")]`,
`#[serde(deny_unknown_fields)]` on request DTOs). Every boundary DTO is ts-rs-generated into the
single committed `web/src/generated/wire.ts` with a no-drift test — a hand-authored TS mirror is
forbidden (RESEARCH Anti-Patterns; CLAUDE.md D-10). The `TierComplete` payload (RESEARCH
§Progressive tier pattern) is a new DTO here, NOT hand-written in TS.

---

### `crates/envi-compute/src/job_assembly.rs` (service, transform) — directional-phase seam lands here

**Analog:** `crates/envi-engine/src/solver.rs` `SolveJob` construction — the canonical field set is
solver.rs:62-121, and the harness/test construction sites (solver.rs:316-330, 366-388) show every
field populated including the two directivity fields already threaded as `None`.

**`SolveJob` full field list to populate** (solver.rs:63-121): `sub_source`, `receiver`,
`profile`, `src`, `rcv`, `atmosphere`, `coh`, `axis`, `weather`, `directivity_gain_db`,
`directivity_phase_rad`, `forest`, `isolation`. ENG-09/10 enter via `forest` (solver.rs:105) and
`isolation` (solver.rs:120) — drawn forest/façade objects feed these so they are never silently
inert (CONTEXT scope).

**Directional-phase wiring (SRC-03 Folded Todo)** — the balloon eval seam. `DirectivityBalloon`
exposes (directivity.rs:361, 374, 394):
```rust
pub fn has_phase(&self) -> bool                       // phase_grid_rad.is_some()
pub fn eval(&self, dir_local: [f64; 3]) -> [f64; N_BANDS]        // ΔL dB  → directivity_gain_db
pub fn eval_phase(&self, dir_local: [f64; 3]) -> [f64; N_BANDS]  // Δφ rad → directivity_phase_rad
```
Populate at the assembly site (RESEARCH §Directional-phase seam):
```rust
let gain  = balloon.eval(dir_local);        // Some(gain) → directivity_gain_db
let phase = balloon.eval_phase(dir_local);  // NEW wiring
let job = SolveJob {
    /* ...profile, src, rcv, atmosphere, coh, axis, weather... */
    directivity_gain_db: Some(gain),
    directivity_phase_rad: if balloon.has_phase() { Some(phase) } else { None },
    forest, isolation, ..
};
```
`eval_phase` returns `[0.0; 105]` for a phase-free balloon (directivity.rs:395-397), so gating on
`has_phase()` keeps the road/incoherent path bit-identical. The solver applies `e^{+jΔφ}` to
`H_coh` only via explicit `(cos, sin)`, never `.conj()` (solver.rs:250-253) — do NOT replicate any
conjugation on the assembly side. **Add the rotate-a-phased-balloon end-to-end test** the solver
tests model at solver.rs:457-513 (`directivity_phase_rotates_coherent_channel_only`).

**Receiver-major ordering contract:** `solve()` requires jobs in non-decreasing receiver order
(solver.rs:123-130); the assembly must yield jobs receiver-major within each chunk range.

---

### `crates/envi-compute/src/identity.rs` (model/util) — factored from envi-store

**Analog:** `crates/envi-store/src/hash.rs` + `manifest.rs` — lift the pure parts, leave the
`std::fs` I/O behind.

**LIFT verbatim into `envi-compute::identity`** (all WASM-safe — `blake3`/`serde`/`geojson`/`uuid`
only):
- `tensor_hash(scene, met, receivers) -> String` (hash.rs:46-89) + every private helper
  (`write_feature`, `feature_sort_key`, `put_len`/`put_f64`/`put_str`, hash.rs:93-207). The frozen
  encoding header (hash.rs:1-31) — `b"envi-tensor-hash-v1"`, u64-LE length prefixes,
  `to_bits().to_le_bytes()` per f64, uuid-sorted features — moves with it. Note the D-07
  structural exclusion: no conditioning argument.
- `CalcManifest` struct (manifest.rs:32-49) + `chunk_receivers(n_sub, n_receivers)`
  (manifest.rs:55-60) — the latter derives from `envi_engine::tensor::{BYTES_PER_CELL_PAIR,
  DEFAULT_TENSOR_BUDGET_BYTES}` (manifest.rs:26,57-58); **never re-derive** the chunk-size math.

**LEAVE in `envi-store`** (they are `std::fs`, native-only): `write_manifest` (manifest.rs:67-82),
`read_manifest` (manifest.rs:88-98), and the `project_dir::atomic_write` dependency
(manifest.rs:29). **Re-export** the moved types from `envi-store` so its public API stays
source-compatible (`pub use envi_compute::identity::{CalcManifest, chunk_receivers};` and
`tensor_hash`) — downstream `envi-service` recompiles unchanged.

**Verify** `blake3` + `geojson` compile for `wasm32-unknown-unknown` at plan time (RESEARCH A2).

---

### `crates/envi-compute-wasm/src/opfs_sink.rs` (provider, file-I/O) — implements the engine trait

**Analog (trait + reference impls):** `crates/envi-engine/src/tensor.rs` — the `TensorSink` trait
(tensor.rs:166-181) and `InMemorySink`/`CountingSink` (tensor.rs:185-284, 432+) as the shape to
mirror. There is no OPFS analog in-repo (see No Analog Found).

**Trait to implement** (tensor.rs:175-180):
```rust
fn put_chunk(
    &mut self,
    r_offset: usize,
    h_coh: ArrayView3<'_, Complex<f64>>,
    p_incoh_abs: ArrayView3<'_, f64>,
) -> Result<(), SinkError>;
```

**Validation gates to copy from `InMemorySink`** (tensor.rs:228-268) — before touching the OPFS
handle, replicate the same dimension/finite checks (`ChannelShapeMismatch`, `BandCountMismatch`,
`SubCountMismatch`, `ChunkOutOfBounds`, `NonFinite`); the sink NEVER panics on caller data
(threat T-04-01-02). `SinkError` variants are enumerated at tensor.rs:62-131.

**Byte serialization** (RESEARCH §OPFS pattern + Pitfall 7) — frozen `[s][r_local][f]`
freq-contiguous layout, `H_coh` interleaved `(re, im)` f64-LE (16 B/cell), `P_incoh` f64-LE
(8 B/cell) in a parallel file. Iterate the view in logical order or `.as_standard_layout()` first
(the chunk view may be a non-contiguous slice — tensor.rs `TensorPair` is row-major but the sliced
view is not guaranteed contiguous). Sketch (RESEARCH Code Examples):
```rust
let mut hbuf = Vec::with_capacity(h.len() * 16);
for z in h.iter() { hbuf.extend_from_slice(&z.re.to_le_bytes());
                    hbuf.extend_from_slice(&z.im.to_le_bytes()); }
```

**Add** a round-trip test: write chunk → read back → equals the `InMemorySink` result
index-for-index (Pitfall 7). `BYTES_PER_CELL_PAIR = 24` and `DEFAULT_TENSOR_BUDGET_BYTES = 256 MiB`
are the frozen constants (tensor.rs:48,54) the chunk-size + memory-bound math uses.

**Worker-only constraint** (RESEARCH Pitfall 4): `createSyncAccessHandle()` throws outside a
dedicated worker; all sink I/O runs inside the compute worker's rayon tasks. Disjoint chunk ranges
→ disjoint files → one exclusive-lock handle per task (Pitfall 5).

---

### `crates/envi-compute-wasm/src/pool.rs` (service, batch) — caller-side rayon sharding

**Analog:** `crates/envi-engine/src/solver.rs` `solve()` loop (solver.rs:139-198) — the compute
crate calls this UNCHANGED, once per disjoint receiver-chunk range. The engine has **no internal
rayon** (verified — `grep rayon crates/envi-engine/src` is empty); GRID-02 parallelism is a
caller-side obligation.

**Driver sketch** (RESEARCH §Caller-side rayon sharding):
```rust
ranges.par_iter().try_for_each(|range| {
    if cancel.load(Ordering::Relaxed) { return Err(ComputeError::Cancelled); }
    let jobs = assemble_jobs(range, ctx);                    // job_assembly.rs
    let mut sink = OpfsChunkSink::open(range.chunk_index, ctx)?;
    envi_engine::solver::solve(jobs, ctx.n_sub, ctx.chunk_receivers, &mut sink)?;
    sink.flush_close()?;
    Ok(())
})
```
Disjoint ranges ⇒ disjoint chunk indices ⇒ disjoint files ⇒ no shared-mutable sink, no locking
(Anti-Pattern: shared-mutable `TensorSink`). Cancel granularity is per-chunk-range = D-11 "abort
at chunk boundaries". A `static AtomicBool` (or `Arc<AtomicBool>`) in wasm shared linear memory is
visible to all pool threads; flip it via `#[wasm_bindgen] pub fn request_cancel()`
(RESEARCH §Cooperative abort).

---

### `web/src/panels/CalcPanel.tsx` (component, request-response)

**Analog:** `web/src/panels/ImportPanel.tsx` — the closest structural sibling (per UI-SPEC).

**Panel skeleton** (analog ImportPanel.tsx:111-119): `<section className="panel"
data-testid="calc-panel">` + `<div className="panel-header">Calculate</div>` + an `.empty-state`
when `projectId` is null. Mounted in `App.tsx` `aside.right-rail` (App.tsx:59-64) between
`WeatherPanel` and `ValidationPanel` (UI-SPEC AC#2).

**Status-severity chip helper** (analog ImportPanel.tsx:29-38) — reuse verbatim shape; map the
frozen `JobStatus` states (`running`→`warn`, `done`→`ok`, `failed`/`cancelled`→`crit`/`off`):
```tsx
function statusSeverity(status): "" | "warn" | "crit" {
  if (status === "error") return "crit";
  if (status === "running") return "warn";
  return "";
}
```

**Progress + count row idiom** (analog ImportPanel.tsx:58-73) — the `{pct}% {message}` `.mono`
readout and the per-tier `done` chip + `{n} receivers` count copy this exact `LayerRow` pattern;
the per-tier `.issue-list`/`.issue-row` rows mirror it. `pct = Math.round(progress * 100)`
(ImportPanel.tsx:44).

**Run button + `canRun` gate** (analog ImportPanel.tsx:108-109) — `const canRun = !!projectId &&
hasCalcArea && hasSource && !guardrail?.blocked && crossOriginIsolated && state !== "running"`.
Disabled `<button className="btn" data-testid="calc-run" disabled={!canRun}>`.

**Strings as React text children only** (analog ImportPanel.tsx:81-84) — every dynamic string
(`reason`, guardrail detail, counts) reaches the DOM as a text child, never `innerHTML` (threat
T-08-07-05). Full copy contract + `data-testid` list + state matrix are in **10-UI-SPEC.md**
(the authoritative visual contract) — CalcPanel introduces no new CSS token/class.

---

### `web/src/store/calc.ts` (store, event-driven)

**Analog:** `web/src/store/import.ts` — copy the zustand slice shape (import.ts:17-58).

**Pattern:** `import { create } from "zustand";` (import.ts:17), a typed state interface with
`readonly` fields, honest lifecycle status enum (`idle|running|done|error` → here the frozen
`JobStatus` states), a guardrail sub-state (import.ts:52-57 `GuardrailState { blocked, detail }`),
and per-slice actions. **Crucially** (import.ts:1-3 header): the client compute job status is
**in-app React/zustand state driven by the Web Worker `postMessage`**, NOT the server SSE machine —
exactly as the import slice is client-side and NOT the Phase-6 SSE job machine (D-10 mirrors
import D-08).

---

### `web/src/compute/worker.ts` (worker, event-driven / pub-sub)

**Analog (state machine):** `crates/envi-service/src/jobs.rs` `run_stub_job` (jobs.rs:139-172) —
the `Running{progress,message}` → `Done`/`Failed{reason}`/`Cancelled` progression, with a cancel
check between steps. The worker reproduces this loop client-side, per tier/chunk-range, posting
each transition via `postMessage` instead of `watch::Sender::send_replace`.

**Analog (worker init + lazy wasm):** `web/src/import/wasm.ts` `ensureWasm` (wasm.ts:64-68) — the
one-shot idempotent init. Extend it: `await init(); await initThreadPool(navigator.hardwareConcurrency);`
ONCE before any solve (RESEARCH Pitfall 2 — pool not ready until the promise resolves). Assert
`self.crossOriginIsolated === true` at startup and surface the capability-failure state honestly
(Pitfall 3, UI-SPEC S1).

**Cancel:** a `SharedArrayBuffer`-backed `Int32Array` cancel flag the rayon tasks read between
chunk ranges; abort via the flag, NEVER `worker.terminate()` (D-11).

---

### `web/src/compute/client.ts` + `web/src/compute/cost.ts` (service/util)

**Analog:** `web/src/import/wasm.ts` — the typed-facade pattern (wasm.ts:87-95 `call<Req,Res>`
single audited cast site). `client.ts` is the main-thread `submit`/`cancel`/`subscribe` API over
`postMessage`; `cost.ts` is a thin wrapper calling the wasm `estimate_cost` fn (prefer the wasm fn
over a TS mirror — one source of truth, RESEARCH Open Question 3).

---

### `web/src/compute/opfs.ts` (provider, file-I/O)

**Analog:** `web/src/import/opfs.ts` — reuse the dir-layout + safety helpers directly.

**`safeSeg` path-traversal guard** (analog opfs.ts:36-40) — the OPFS dir/chunk names derive ONLY
from the hex manifest hash + integer chunk index; the `safeSeg` regex `[^A-Za-z0-9._-] → _` is the
V12 defence (threat T-08-07-02). **Key difference:** import's opfs.ts uses the async main-thread
API (`createWritable`, opfs.ts:14-15 explicitly notes sync handles are worker-only); the compute
sink's *sync* `createSyncAccessHandle` I/O lives in the worker (Rust side or this glue). This
`opfs.ts` provides the worker-side layout helpers + the JS glue (`open/write/flush/close`) the
Rust sink calls (RESEARCH Open Question Q3 recommends JS glue over `web-sys`).

**Quota check** (analog opfs.ts:124-139) — reuse `estimateQuota` + `fitsQuota` for the cost
guardrail's hard OPFS block (`navigator.storage.estimate()`, SC1 / RESEARCH §Cost model).

---

### `crates/envi-service/src/api/mod.rs` (MODIFIED — config headers)

**Analog:** itself — the `app()` bundle mount (api/mod.rs:113-118) is where COOP/COEP layers wrap
the `ServeDir`.

**Current mount** (api/mod.rs:113-118):
```rust
let serve_dir = ServeDir::new(web_dist).fallback(ServeFile::new(web_dist.join("index.html")));
Router::new().nest("/api/v1", api_router()).fallback_service(serve_dir).with_state(state)
```

**Add** (RESEARCH Code Examples — `tower-http` `SetResponseHeaderLayer`, already a dep via
`ServeDir`; confirm the `set-header` feature):
```rust
let coop = SetResponseHeaderLayer::overriding(
    HeaderName::from_static("cross-origin-opener-policy"),
    HeaderValue::from_static("same-origin"));
let coep = SetResponseHeaderLayer::overriding(
    HeaderName::from_static("cross-origin-embedder-policy"),
    HeaderValue::from_static("credentialless"));   // NOT require-corp (D-04, Pitfall 3)
Router::new().nest("/api/v1", api_router())
    .fallback_service(serve_dir).layer(coop).layer(coep).with_state(state)
```
The SSE job machine (`jobs.rs`) is **repurposed for ERA5, not deleted** (D-10) — no change to the
job routes (api/mod.rs:94-95). `JobStatus` (jobs.rs:54-73) is reused as the client wire shape.

---

### `web/vite.config.ts` (MODIFIED — dev headers)

**Analog:** itself — add `server.headers` (no new npm dependency; Vite native). RESEARCH Code
Examples:
```ts
server: { headers: {
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Embedder-Policy": "credentialless",
}},
```
The existing `base: "./"`, `assetsInclude` for `src/generated/wasm/*.wasm` (vite.config.ts:37), and
build config are untouched; add a parallel `assetsInclude`/glob for the new
`src/generated/wasm-compute/` artifact and a `.gitignore` entry (git-ignored like the gis one).

---

## Shared Patterns

### Thin wasm boundary (marshalling only)
**Source:** `crates/envi-gis-wasm/src/lib.rs:71-100, 330-351`
**Apply to:** `envi-compute-wasm/src/lib.rs`, every `#[wasm_bindgen]` export.
Each export = `from_js` → one core call → `to_js`; no logic in the boundary. `#![deny(unsafe_code)]`,
no `getrandom`/`uuid`, typed-error → `JsValue` (never a panic on data).

### Generated wire DTOs, drift structurally impossible
**Source:** `crates/envi-gis-wasm/src/dto.rs` (ts-rs `#[ts(export_to = "wire.ts")]`) +
`crates/envi-service/src/jobs.rs:41-54` (`JobStatus` `#[derive(TS)]`).
**Apply to:** `envi-compute-wasm/src/dto.rs`, the `TierComplete` payload, the reused `JobStatus`.
All boundary DTOs generate into the single committed `web/src/generated/wire.ts` with a no-drift
test; a hand-written TS mirror of any Rust DTO is forbidden (CLAUDE.md D-10).

### Non-panicking typed-error boundary (V5)
**Source:** `crates/envi-engine/src/tensor.rs:62-131` (`SinkError`) + `solver.rs:139-146`
(`Result<_, PropagationError>`) + `envi-gis-wasm/src/lib.rs:98-100` (`gis_err`).
**Apply to:** the OPFS sink (copy `InMemorySink`'s validation gates, tensor.rs:228-268), the wasm
boundary (`ComputeError`→`JsValue`), and all marshalling. Never `unwrap`/panic on caller data.

### OPFS hex-only path keys (V12 path-traversal)
**Source:** `web/src/import/opfs.ts:36-50` (`safeSeg` + fixed-segment `cacheDir`).
**Apply to:** `web/src/compute/opfs.ts` and the Rust sink dir layout — chunk/dir names are the hex
manifest hash + integer chunk index only; reject any non-hex/non-numeric component.

### Client-side job status is app state, not the SSE machine
**Source:** `web/src/store/import.ts:1-3, 38-49` (import D-08 precedent).
**Apply to:** `web/src/store/calc.ts` + `web/src/compute/worker.ts` — worker `postMessage`
drives the store; the server SSE machine stays reserved for ERA5 (D-10).

### Frozen band-index framework (105 points, never nominal Hz)
**Source:** `envi-engine/src/freq.rs` `N_BANDS`, used throughout tensor.rs/solver.rs.
**Apply to:** tier merge, receiver de-dup, and the `TierComplete` spans — index `[f=0..105]`;
receiver identity is the UUID/position, never a frequency (Pitfall 6; CLAUDE.md house rule).

---

## No Analog Found

Two shapes have no close in-repo match; the executor uses RESEARCH patterns + the partial
references below.

| File | Role | Data Flow | Reason / nearest reference |
|------|------|-----------|----------------------------|
| `crates/envi-compute-wasm/src/opfs_sink.rs` | provider | file-I/O | No `FileSystemSyncAccessHandle` sink exists — `web/src/import/opfs.ts` is main-thread ASYNC (`createWritable`) and explicitly avoids sync handles (opfs.ts:14-15). The `TensorSink` trait + `InMemorySink` validation (tensor.rs:166-284) give the *trait shape*; the sync-handle byte I/O is new (RESEARCH §OPFS pattern + Pitfall 4/5/7). |
| `crates/envi-compute-wasm/src/pool.rs` (`wasm-bindgen-rayon` + `initThreadPool` + SAB cancel) | service | batch | No threaded-wasm pool exists in the repo (`envi-gis-wasm` is single-threaded stable). The `solve()` loop (solver.rs:139-198) is the callee; the pool driver + `initThreadPool` + SharedArrayBuffer atomics are net-new (RESEARCH §Caller-side rayon sharding, §Cooperative abort; nightly `-Zbuild-std` toolchain, Pitfall 1). |

Partial-only (role-match, no exact data-flow twin): `crates/envi-compute/src/tiers.rs` (hierarchical
receiver partition — the coarse⊂fine lattice logic is new physics-free geometry; `envi-gis/src/grid.rs`
receiver-grid generation [ASSUMED, not read this session] is the nearest) and `crates/envi-compute/src/cost.rs`
(byte/receiver arithmetic — `manifest.rs:55-60 chunk_receivers` is the nearest pure byte-math fn).

---

## Metadata

**Analog search scope:** `crates/envi-engine/src/{solver.rs,tensor.rs,directivity.rs}`,
`crates/envi-store/src/{manifest.rs,hash.rs}`, `crates/envi-service/src/{jobs.rs,api/mod.rs}`,
`crates/envi-gis-wasm/{Cargo.toml,src/lib.rs}`, `web/src/{import/opfs.ts,import/wasm.ts,
store/import.ts,panels/ImportPanel.tsx,App.tsx}`, `web/vite.config.ts`, workspace `Cargo.toml`.
**Files scanned:** 15 read in full/part this session.
**Pattern extraction date:** 2026-07-11
