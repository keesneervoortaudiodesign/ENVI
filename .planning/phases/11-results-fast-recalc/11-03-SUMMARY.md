---
phase: 11-results-fast-recalc
plan: 03
subsystem: compute-wasm
tags: [wasm, recondition, tensor-mac, svc-06, hash-gate, 409, conditioning, ts-rs, d-12]

# Dependency graph
requires:
  - phase: 11-results-fast-recalc
    provides: readout_receiver / compose_gain-driven two-channel readout, ConditioningDto→Conditioning mapping surface, OPFS read_chunk (11-01)
  - phase: 10-calculation-service
    provides: marshalled_tensor_hash identity, PreparedScene, OPFS chunk byte format, the frozen server recondition 409 contract (calc.rs)
  - phase: 06-service-foundation-persistence
    provides: SVC-06 designed-ahead recondition/recompute split + 409 tensor_hash_mismatch body shape
provides:
  - Client-side SVC-06 recondition MAC boundary (envi_compute_wasm::recondition) — hash-gated, no re-propagation
  - Typed client-side 409 — ComputeError::HashMismatch { expected, got } mirroring the server body (D-12/Open Q1)
  - ReconditionReq / ReconditionResult ts-rs wire DTOs (no-drift green)
  - ConditioningDto relocated into WASM-safe envi-compute::readout (re-exported from envi-store)
affects: [11-05, 11-07]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Hash gate FIRST, before any MAC — a mismatched claimed hash is refused as a typed HashMismatch, never a silently-served stale readout (D-12 honest client 409)"
    - "Recondition drives the 11-01 readout_receiver core (compose_gain + readout_coherent) — zero bespoke MAC/dB loop at the boundary"
    - "Wire DTO relocation (Phase-10 pattern): move a shared DTO into WASM-safe envi-compute, re-export at its envi-store path so wire.ts stays byte-stable and no std::fs leaks into wasm"

key-files:
  created:
    - crates/envi-compute-wasm/src/recondition.rs
  modified:
    - crates/envi-compute/src/readout.rs
    - crates/envi-store/src/dto.rs
    - crates/envi-compute-wasm/src/dto.rs
    - crates/envi-compute-wasm/src/lib.rs
    - crates/envi-service/tests/wire_no_drift.rs
    - web/src/generated/wire.ts

key-decisions:
  - "Client-side 409 (Open Q1): the recondition boundary re-mints marshalled_tensor_hash from the CURRENT scene per request and refuses a mismatch with ComputeError::HashMismatch { expected, got } — the honest client realization of the server's frozen tensor_hash_mismatch body; the server calc.rs DTOs stay the designed-ahead contract"
  - "ConditioningDto is the per-source drive (no separate L_W on the wire): gain_db → broadband L_W, filter_band_db (dB) → complex filter 10^{dB/20}, delay_ms → delay_s, muted → exact-zero complex filter. Readout law derived from composition (default_law, Open Q2) so a no-op conditioning ≡ the plain 11-01 readout (never-stale, D-07)"
  - "ConditioningDto moved envi-store→envi-compute::readout (re-exported), mirroring the Phase-10 MetDto/ReceiverDto/scene-DTO relocation, so the wasm recondition boundary reuses it without dragging std::fs/tempfile into wasm — one wire type, never forked"

requirements-completed: []  # SVC-06 + WEB-05 backend REALIZED here (client MAC + 409), but left Pending — they become user-observable in 11-07 (UI conditioning + stale badge + 409 refusal), mirroring 11-01's WEB-11 decision.

# Metrics
duration: ~40 min
completed: 2026-07-12
status: complete
---

# Phase 11 Plan 03: Recondition MAC Boundary Summary

**The flagship client-side fast-recalc backend: a hash-gated `recondition` WASM boundary that re-mints the tensor identity, refuses a mismatched claimed hash with a typed `HashMismatch { expected, got }` (the honest client-side 409, never served stale), and on a match drives `compose_gain` + `readout_coherent` (the FORCE-validated 11-01 readout core) over the OPFS-read tensor to produce reconditioned receiver spectra with no re-propagation — proven bit-for-bit equal to the engine path, and identical to the plain readout under a no-op conditioning.**

## Performance

- **Duration:** ~40 min
- **Tasks:** 2 (Task 2 TDD: RED → GREEN)
- **Files:** 6 (1 created, 5 modified) + STATE.md/ROADMAP.md

## Accomplishments

- **Recondition/hash DTOs (ts-rs, Task 1):** `ReconditionReq { tensor_hash, per_source_conditioning: Vec<ConditioningDto>, receiver_ids }` and `ReconditionResult { spectra: Vec<Vec<f64>>, stale: bool }` added to `envi-compute-wasm::dto`, generated into the single committed `web/src/generated/wire.ts` with the no-drift test green. `ConditioningDto` was relocated from `envi-store::dto` into WASM-safe `envi_compute::readout` and re-exported at its original path — so the wasm boundary reuses the exact WEB-05 readout param without a forked shape and without dragging `std::fs`/`tempfile` into wasm (the Phase-10 relocation discipline; `wire.ts` stays byte-stable for `ConditioningDto`).
- **Typed client-side 409 surface:** `ComputeError::HashMismatch` widened from a unit variant to `{ expected, got }`, mirroring the server 409 body fields (`envi-service::api::calc`'s `tensor_hash_mismatch` `{expected, got, hint}`) so the client realization of the honest state is faithful (Open Q1 / D-12). The existing `run_chunk_range` call site and its registry-gate test were updated to the struct form.
- **Hash-gated recondition MAC (Task 2, TDD):** `crates/envi-compute-wasm/src/recondition.rs` — `recondition_receivers` re-mints/compares the tensor identity FIRST (a mismatch is refused before any MAC, producing NO spectra), then maps each `ConditioningDto` → (`L_W` = `gain_db`, complex per-band filter = `10^{dB/20}`, `delay_s` = `delay_ms/1000`), derives the readout law from source composition (`default_law`, Open Q2), and drives the 11-01 `readout_receiver` (which internally runs `compose_gain` + `readout_coherent`) over every requested receiver. Filter is validated dense `[105]` + finite (V5); a muted source is silenced with an exact-zero complex filter; every failure is a typed `ComputeError::Recondition`, never a panic (T-11-03-02). The `#[wasm_bindgen] recondition` boundary re-mints `marshalled_tensor_hash` from the passed CURRENT scene, decodes the OPFS chunk via the 11-01 `read_chunk`, and drives the typed core.
- **Acceptance gates (all green):** a matching-hash recondition equals a direct `compose_gain` + `readout_coherent` engine path bit-for-bit (`f64::to_bits`) — the MAC ≡ recompute gate; a mismatched hash returns `HashMismatch { expected, got }` and produces no spectra; a default (no-op) conditioning recondition equals the plain 11-01 readout bit-for-bit (the never-stale invariant, D-07); a non-dense `[105]` filter and a muted source are handled as typed/silent, not panics.

## Task Commits

1. **Task 1: Recondition/hash DTOs (ts-rs) + typed HashMismatch surface** — `feat(11-03)` (`a2ffeef`)
2. **Task 2: Hash-gated recondition MAC boundary** — `test(11-03)` RED (`378d600`) + `feat(11-03)` GREEN (`38a9281`) (TDD)

_Plan metadata commit follows this SUMMARY._

## Files Created/Modified

- `crates/envi-compute-wasm/src/recondition.rs` — `recondition_receivers` typed core + `to_engine` mapping + `#[wasm_bindgen] recondition` boundary + 5 native tests
- `crates/envi-compute/src/readout.rs` — `ConditioningDto` (relocated, WASM-safe, ts-rs) alongside the engine `Conditioning`
- `crates/envi-store/src/dto.rs` — `ConditioningDto` re-exported from `envi_compute::readout` (path-compatible)
- `crates/envi-compute-wasm/src/dto.rs` — `ReconditionReq` / `ReconditionResult` (ts-rs)
- `crates/envi-compute-wasm/src/lib.rs` — `ComputeError::HashMismatch { expected, got }` + `Recondition(String)`; `pub mod recondition`; `pub(crate)` marshalling helpers; `run_chunk_range` + test updated
- `crates/envi-service/tests/wire_no_drift.rs` — registered the two new DTOs
- `web/src/generated/wire.ts` — regenerated (no-drift green)

## Decisions Made

- **Client-side 409 (Open Q1 / D-12):** the recondition boundary re-mints the tensor identity from the CURRENT scene per request and refuses a mismatch with a typed `HashMismatch { expected, got }` — the honest client realization of the server's frozen `tensor_hash_mismatch` body. The server `calc.rs` recondition DTOs remain the designed-ahead contract, unchanged.
- **`ConditioningDto` as the per-source drive:** no separate `L_W` crosses the recondition wire; `gain_db` is the broadband `L_W`, `filter_band_db` (dB) the per-band complex filter, `delay_ms` the delay, `muted` an exact-zero gain. The readout law is derived from composition so a no-op conditioning reads out identically to the plain 11-01 readout — the concrete demonstration that conditioning is excluded from identity (D-07) and therefore never stales.
- **Relocate rather than depend:** `ConditioningDto` moved into `envi-compute::readout` (re-exported from `envi-store::dto`) mirroring the Phase-10 `MetDto`/`ReceiverDto`/scene-DTO moves — the wasm crate must not depend on `envi-store` (drags `std::fs`/`tempfile`). `wire.ts` for `ConditioningDto` is byte-stable (only a doc line changed).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Relocate `ConditioningDto` into WASM-safe `envi-compute`**
- **Found during:** Task 1
- **Issue:** The plan says `ReconditionReq` must "reuse the store DTO, do not fork." But `ConditioningDto` lived in `envi-store`, and `envi-compute-wasm` must not depend on `envi-store` (it would drag `std::fs`/`tempfile` into the wasm graph — the very reason Phase 10 moved `MetDto`/`ReceiverDto`/scene DTOs into `envi-compute`). Naming the type in the wasm crate was otherwise impossible without either forking it (forbidden) or breaking the wasm dependency quarantine.
- **Fix:** Moved `ConditioningDto` verbatim into `envi_compute::readout` and re-exported it at `envi_store::dto::ConditioningDto`. All existing paths (`calc.rs`, the wire no-drift import) keep resolving; `wire.ts` is byte-stable for the type.
- **Files modified:** `crates/envi-compute/src/readout.rs`, `crates/envi-store/src/dto.rs`
- **Verification:** wire no-drift test green (ConditioningDto TS unchanged); full workspace test/clippy/fmt green.
- **Committed in:** the Task 1 commit (`a2ffeef`).

**2. [Rule 2 - Missing critical] `recondition` boundary signature carries the scene + tensor bytes**
- **Found during:** Task 2
- **Issue:** The plan sketched `pub fn recondition(req: JsValue)`, but a working boundary must (a) re-mint the CURRENT identity from a scene to gate against, and (b) obtain the OPFS tensor bytes to read out. A single `req` argument cannot do either.
- **Fix:** `recondition(scene: JsValue, req: JsValue, hi_bytes: &[u8], pi_bytes: &[u8])` — re-mints `marshalled_tensor_hash(scene)` (satisfies the plan's `recondition.rs → identity.rs` key-link), decodes the chunk via the 11-01 `read_chunk`, and drives the natively-testable `recondition_receivers` core. The worker-side OPFS pre-open-read glue that supplies the bytes lands in 11-05 (as scoped by the plan).
- **Files modified:** `crates/envi-compute-wasm/src/recondition.rs`
- **Verification:** the typed core is proven by 5 native `cargo test` gates; the boundary is a thin marshalling shell over it.
- **Committed in:** the Task 2 GREEN commit (`38a9281`).

---

**Total deviations:** 2 auto-fixed (1 blocking dependency relocation, 1 missing-critical boundary signature). **Impact:** No scope creep; the engine 3-dep quarantine and `wire.ts` byte-stability are both preserved.

## Authentication Gates

None.

## Requirements

**SVC-06** and **WEB-05** are *realized at the backend* by this plan (the client-side recondition MAC + the honest 409/`HashMismatch` refusal + conditioning-never-stales), but are intentionally left **Pending** in REQUIREMENTS.md rather than marked complete here — they become **user-observable** in **11-07** (the UI conditioning controls + fast-recalc + results-stale badge + 409 refusal), which carries the same two requirement tags. This mirrors 11-01's WEB-11 decision (advance the foundation, complete at the UI plan).

## Issues Encountered

None — all five recondition acceptance gates (MAC ≡ recompute bit-exact, hash-mismatch refusal, never-stale, dense-filter validation, muted-silence) pass, and the whole-workspace build/test/clippy/fmt are green.

## Verification

All commands run at the workspace root on `main`:

- `cargo build --release` → **Finished**, exit 0.
- `cargo test` (full workspace) → **all pass**, no failures. Includes the 5 new `recondition` gates and the updated `solve_chunk_range` registry hash-gate test.
- `cargo test -p envi-service --test wire_no_drift` → **green** (regeneration produces zero diff; `ReconditionReq`/`ReconditionResult` present in `wire.ts`).
- `cargo clippy --all-targets -- -D warnings` → **clean** (whole workspace).
- `cargo fmt --check` → **clean** (exit 0).
- `cargo tree -p envi-engine` → direct deps `ndarray + num-complex + thiserror` (+ `approx` dev-dep) — **unchanged** (engine 3-dep quarantine intact, D-02).

## Next Phase Readiness

- **11-05** (spectrum panel + OPFS read glue) supplies the worker-side `createSyncAccessHandle` pre-open-read that feeds `recondition`'s `hi_bytes`/`pi_bytes`.
- **11-07** (conditioning fast-recalc UI) drives `recondition` debounced, surfaces the results-stale badge, and completes SVC-06/WEB-05 as user-observable — the `HashMismatch { expected, got }` payload is ready for the UI's 409-refusal path.

## Self-Check: PASSED

- Created file exists on disk: `crates/envi-compute-wasm/src/recondition.rs`.
- Task commits present in `git log`: `a2ffeef` (feat 11-03 DTOs), `378d600` (test 11-03 RED), `38a9281` (feat 11-03 GREEN).
- All plan `<verification>` commands re-run green (build/test/clippy/fmt/tree above); wire no-drift green.

---
*Phase: 11-results-fast-recalc*
*Completed: 2026-07-12*
