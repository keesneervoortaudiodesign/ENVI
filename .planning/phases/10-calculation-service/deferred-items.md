# Phase 10 — Deferred Items

## [10-05] Threaded compute wasm ships a NON-shared `WebAssembly.Memory` (pool cannot start in-browser)

**Discovered:** 10-05 (first plan to load the threaded compute glue in a real browser).

**Severity:** High — blocks the actual client-side threaded solve (GRID-02 / SVC-02) from running
in any browser, despite 10-06's "solve is REAL end-to-end" claim (which was proven only by NATIVE
`cargo test` bit-equivalence, never in a browser).

**Symptom:** On Run, the compute Web Worker calls `initThreadPool(navigator.hardwareConcurrency)`,
which posts the module's `WebAssembly.Memory` to each rayon pool worker. The browser throws
`Failed to execute 'postMessage' on 'Worker': #<Memory> could not be cloned`, `initThreadPool`
never resolves, the worker's `onmessage` handler is never installed, and the job stalls at
`queued` forever (no `running`, no tier progress).

**Root cause (confirmed):** the `build:wasm:compute` artifact's memory section has limits flag
`0x0` — i.e. a **non-shared** memory. wasm-bindgen-rayon requires a **shared** memory (a
SharedArrayBuffer-backed `WebAssembly.Memory`, flag `0x03`) so it can be structured-cloned to the
pool workers under cross-origin isolation. The current recipe only sets
`-C target-feature=+atomics,+bulk-memory,+mutable-globals` — with this toolchain
(nightly-2026-07-11 + wasm-bindgen 0.2.126) that does NOT emit a shared memory.

**Attempted fix (10-05, reverted):** adding `-C link-arg=--shared-memory
-C link-arg=--max-memory=1073741824` to the `build:wasm:compute` rustflags makes the memory
shared, but wasm-bindgen's thread-prep then fails with
`failed to prepare module for threading: failed to find __heap_base`. The next step is likely to
also export `__heap_base`/`__tls_base` (e.g. `-C link-arg=--export=__heap_base`) and/or import the
memory (`--import-memory`) so wasm-bindgen's rayon thread-prep can complete. This is a
threaded-BUILD-recipe concern (10-03 territory), not the 10-05 CalcPanel, and needs iterative
rebuilds to validate — out of 10-05's file scope.

**Remediation (follow-up plan):**
1. Fix `web/package.json` `build:wasm:compute` so the emitted memory is shared AND wasm-bindgen
   thread-prep succeeds (shared-memory + max-memory link args + `__heap_base` export as needed);
   verify with: `node -e "…"` reading the wasm memory-section flag == `0x03`.
2. Re-run `cd web && npm run build:wasm:compute && npm run build && npm run test:e2e`.
   The 10-05 UAT `tests/e2e/calc.spec.ts` **Test 2** will then run fully (it currently self-skips
   with this exact reason) and assert tiered progress → coarse `done` → Abort → `cancelled`.

**Not blocking 10-05:** the CalcPanel UI, the REAL wasm `estimate_cost` pre-run readout, the Run
gate, cooperative-abort wiring, capability banner, and the offline zero-egress proof all work and
are asserted green by Test 1. Only the deep threaded solve is gated on this build fix.
