---
phase: 04-transfer-tensor-directional-sources-full-validation
plan: 01
subsystem: engine
tags: [tensor, ndarray, complex-transfer, mac, conditioning, streaming, memory-budget]

# Dependency graph
requires:
  - phase: 01-foundation
    provides: "TransferTensor = Array3<Complex<f64>> frozen typedef, direct_path, two-channel readout law, FREQ_AXIS"
  - phase: 02-ground-screens
    provides: "terrain_effect two-channel GroundResult (h_coh_factor + p_incoh), the single conj boundary nord_ratio_to_transfer"
  - phase: 03-meteorology-refraction
    provides: "SoundSpeedProfile + refraction wiring consumed via terrain_effect's weather arg"
provides:
  - "tensor.rs: TensorPair store, TensorSink trait, InMemorySink, CountingSink, both readout laws, compose_gain conditioning"
  - "solver.rs: SolveJob (Phase-9 PropagationPath seam) + chunked solve() streaming loop"
  - "Public engine API for OUT-01..06 (tensor/MAC/filter/delay/streaming budget)"
  - "New PropagationError variants (DegenerateJobGeometry, DegenerateChunkSize, SubSourceOutOfRange, Sink) + SinkError type"
affects: [04-02-directional-sources-emission, 04-03-straight-road-force, 04-04-curved-city-yearly, 09-service, 10-grid-service]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Streaming sink trait (put_chunk) as the Phase-10 file-backed-store seam"
    - "High-water-mark byte accounting as a structural memory-budget proof (never RSS sampling)"
    - "Single-composition conditioning gain (compose G once, multiply once) ⇒ bit-exact MAC ≡ recompute"
    - "Reusable single-chunk working buffer bounds the solver's resident set"

key-files:
  created:
    - crates/envi-engine/src/tensor.rs
    - crates/envi-engine/src/solver.rs
    - crates/envi-harness/tests/mac_identity.rs
    - crates/envi-harness/tests/tensor_budget.rs
  modified:
    - crates/envi-engine/src/lib.rs
    - crates/envi-engine/src/propagation/mod.rs

key-decisions:
  - "SolveJob carries `atmosphere: &Atmosphere` (not a bare atmos_c0): direct_path needs the full air-absorption state for H_ff; terrain_effect consumes coh.c0, exactly as the harness build_terrain_inputs does. Documented interface deviation from the plan's literal `atmos_c0` field."
  - "compose_gain returns Result<Vec<Complex<f64>>, SinkError> (not a bare Vec): honors threat T-04-01-03 (validate filter length against N_BANDS, reject non-finite) — never panic on operator input."
  - "P_incoh stored in ABSOLUTE form |H_ff|²·p_incoh at fill time (04-RESEARCH Pattern 4), so readout needs only two stores and F→1 ⇒ 0 stays bit-exact."
  - "MAC≡recompute proven by folding the SAME composed G into the tensor then reading out with unit gain — one composition path, assert_eq! on raw f64 bits."
  - "jobs must arrive receiver-major; solve() groups by chunk index and flushes the used receiver span, keeping the working set to one chunk."
  - "The 256 MiB budget test uses CountingSink (no full-tensor backing; folds Σ_s per chunk into a compact real readout) so the 504 MB complex tensor is never resident."

patterns-established:
  - "TensorSink trait: streaming seam keeping the engine I/O-free while letting M2 add a file-backed sink"
  - "Two readout laws kept strictly distinct: coherent MAC (OUT-03) vs incoherent Annex-A energy sum (roads/FORCE)"
  - "Conditioning + directivity live on the ENVI-convention (post-conj) side; propagation/ conj-quarantine stays at zero actual calls"

requirements-completed: [OUT-01, OUT-02, OUT-03, OUT-04, OUT-05, OUT-06]

# Metrics
duration: 55min
completed: 2026-07-08
status: complete
---

# Phase 4 Plan 01: Transfer Tensor, Solver & MAC Conditioning Summary

**The frozen `Array3<Complex<f64>>` forward contract is now a real, streamable artifact: a paired coherent/incoherent tensor, a chunked solver that fills it within a stated 256 MiB budget, and interactive conditioning whose coherent MAC equals a full recompute bit-for-bit.**

## Performance

- **Duration:** ~55 min
- **Started:** 2026-07-08 (post Phase 3 close)
- **Completed:** 2026-07-08
- **Tasks:** 3 of 3
- **Files modified/created:** 6

## Accomplishments

- **OUT-01/02 (tensor):** `TensorPair` — row-major, frequency-contiguous `H_coh: Array3<Complex<f64>>` + absolute `P_incoh_abs: Array3<f64>`, filled for single- and multi-receiver solves.
- **OUT-03 (MAC):** `readout_coherent` `p[r,f] = Σ_s H_coh[s,r,f]·G_s(f)` recomputes receiver spectra from conditioning alone, with NO propagation re-run — proven **bit-for-bit** equal to a full recompute (`assert_eq!` on `f64::to_bits`, not epsilon).
- **OUT-04/05 (filter + delay):** `compose_gain` folds `10^{L_W/20}·filter(f)·e^{−j2πfτ}` into ONE complex gain per band, one frozen multiplication order; a real 0.5 filter drops the level exactly −6.0206 dB and a τ>0 delay carries the −2πfτ phase.
- **OUT-06 (streaming budget):** `solve()` streams receiver-axis chunks through `TensorSink`; a 100 000-receiver × 3-sub-source synthetic solve stays under 256 MiB, proven by a high-water-mark byte counter — the full complex tensor is never resident.
- Incoherent Annex-A readout (`readout_incoherent`) exists alongside the coherent MAC and is kept strictly distinct: two identical co-located sub-sources give **+3.0103 dB**, never +6 dB.

## Task Commits

Each task followed the plan's TDD RED→GREEN cycle:

1. **Task 1: TensorPair + TensorSink + InMemorySink + readout laws**
   - `61bcd7f` (test — RED) → `41ad980` (feat — GREEN)
2. **Task 2: SolveJob seam + chunked solve() loop**
   - `198fbb7` (test — RED) → `7d2fd07` (feat — GREEN)
3. **Task 3: Conditioning composition + bit-exact MAC + 256 MiB budget**
   - `3ab08e4` (test — RED) → `b6064d7` (feat — GREEN)

## Files Created/Modified

- `crates/envi-engine/src/tensor.rs` (created) — `TensorPair`, `TensorSink` trait, `InMemorySink` (+HWM), `CountingSink`, `readout_coherent`/`readout_incoherent`, `compose_gain`, `SinkError`, `DEFAULT_TENSOR_BUDGET_BYTES`, `BYTES_PER_CELL_PAIR`.
- `crates/envi-engine/src/solver.rs` (created) — `SolveJob<'a>` seam + `solve()` chunked receiver-axis loop and the private `solve_pair` frozen chain.
- `crates/envi-engine/src/lib.rs` (modified) — registered `pub mod solver; pub mod tensor;`.
- `crates/envi-engine/src/propagation/mod.rs` (modified) — added `PropagationError` variants: `DegenerateJobGeometry`, `DegenerateChunkSize`, `SubSourceOutOfRange`, `Sink(#[from] SinkError)`.
- `crates/envi-harness/tests/mac_identity.rs` (created) — MAC-vs-recompute bit-identity integration test.
- `crates/envi-harness/tests/tensor_budget.rs` (created) — 100k-receiver high-water-mark budget sweep.

## Verification

- `cargo test -p envi-engine` — **172 passed** (includes 10 tensor + 5 solver unit tests).
- `cargo test --workspace` — all green: harness lib 70, force runner 10 pass / 67 fail-soft Skipped (unchanged), mac_identity 1, tensor_budget 1 (~79 s), all oracle suites unchanged.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo fmt --check` — clean.
- `cargo tree -p envi-engine -e normal --depth 1` — **unchanged** (ndarray + num-complex + thiserror only; ZERO new deps).
- Conjugation quarantine: zero actual `.conj()` call sites in `crates/envi-engine/src/propagation/` (the 10 grep hits are doc-comments; the sole real conjugation stays in `transfer.rs::nord_ratio_to_transfer`).

## MAC ≡ recompute bit-exact proof

`mac_identity.rs` builds a 2-sub × 3-receiver tensor via `solve()`, composes a per-sub gain `G_s(f) = 10^{L_W/20}·filter(f)·e^{−j2πfτ_s}` ONCE, then compares two paths:
- **A (MAC):** `readout_coherent(H, [G_s])`.
- **B (recompute):** fold the SAME `G_s` into the tensor (`H_cond = H·G_s`), then read out with unit gain.

Every receiver × band asserts `p_mac.re.to_bits() == p_rec.re.to_bits()` and the same on `.im` — bit-for-bit, not epsilon. This holds because there is exactly one composition order and one multiplication (Pitfall 6).

## Memory-budget behavior

`chunk_receivers = floor(256·1024·1024 / (3·105·24)) = 35 507`. Over 100 000 receivers that is 3 chunks; the largest chunk footprint is `3·35 507·105·24 = 268 432 920 B = 255.996 MiB ≤ 256 MiB`. The solver reuses a single `[n_sub, chunk_receivers, N_BANDS]` working buffer, and `CountingSink` folds `Σ_s (|H_coh|² + P_incoh_abs)` per chunk into a compact `[n_rcv, N_BANDS]` real readout — the 504 MB complex tensor is never allocated. The proof is structural byte accounting (`high_water_bytes()`), not an RSS sample.

## Deviations from Plan

### Auto-added critical functionality

**1. [Rule 2 — Threat mitigation] `compose_gain` returns `Result`, not a bare `Vec`**
- **Found during:** Task 3.
- **Issue:** The plan's `<action>` pseudo-signature returned `Vec<Complex<f64>>`, but the plan's own threat register (T-04-01-03) requires `compose_gain` to validate slice lengths against `N_BANDS` with typed errors and to reject non-finite input — impossible with a bare `Vec` without panicking.
- **Fix:** Return `Result<Vec<Complex<f64>>, SinkError>`; validate filter length (`FilterLengthMismatch`) and reject non-finite `delay_s`/filter (`NonFinite`).
- **Files:** `crates/envi-engine/src/tensor.rs`. **Commit:** `b6064d7`.

**2. [Rule 3 — interface adjustment] `SolveJob` carries `atmosphere: &Atmosphere` instead of a bare `atmos_c0: f64`**
- **Found during:** Task 2.
- **Issue:** The plan listed `atmos_c0: f64` in the SolveJob vocabulary, but `direct_path` (needed to build `H_ff`) requires the full `Atmosphere` (temperature, RH, pressure) for air absorption; `atmos_c0` alone cannot produce `H_ff`.
- **Fix:** Carry `atmosphere: &Atmosphere`; pass `coh.c0` to `terrain_effect` (identical to the harness's `build_terrain_inputs`), so the two remain consistent. Documented in the SolveJob doc-comment. This mirrors the 04-RESEARCH Pattern 3 sketch, which itself lists `atmosphere: &Atmosphere`.
- **Files:** `crates/envi-engine/src/solver.rs`. **Commit:** `7d2fd07`.

## Known Stubs

None. All delivered functions are fully wired and tested; no placeholder data paths.

## TDD Gate Compliance

Each task has a `test(04-01)` (RED) commit followed by a `feat(04-01)` (GREEN) commit; RED states were confirmed failing (`todo!()` / unimplemented) before implementation. No REFACTOR commits were needed.

## Notes / Forward context

- The 256 MiB budget test runs a real 300k-pair solve and takes ~79 s in debug (the physics is recomputed per pair by design — synthetic, identical geometry). It is a normal (non-ignored) test so the phase gate exercises it.
- `SolveJob` is deliberately named/shaped as the Phase-9 `PropagationPath` coordination seam; `TensorSink` is the Phase-10 file-backed-store seam.
- T-04-01-04 (stale/mismatched tensor identity for the MAC) is accepted for this plan — hash-keyed tensor identity is a Phase-11/SVC-06 concern; the MAC-identity test proves numerical equivalence only.

## Self-Check: PASSED

All created files exist on disk (tensor.rs, solver.rs, mac_identity.rs, tensor_budget.rs, this SUMMARY); all GREEN commits (`41ad980`, `7d2fd07`, `b6064d7`) are present in git history.
