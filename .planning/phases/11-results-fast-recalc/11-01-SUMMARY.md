---
phase: 11-results-fast-recalc
plan: 01
subsystem: compute
tags: [wasm, readout, iec-61672-1, a-weighting, c-weighting, tensor-mac, opfs, nord2000]

# Dependency graph
requires:
  - phase: 10-calculation-service
    provides: OPFS TensorSink (frozen chunk byte format), envi-compute pure core, CalcManifest/chunk_receivers
  - phase: 04-transfer-tensor
    provides: readout_coherent / readout_incoherent / compose_gain / band_levels_db_two_channel (FORCE-validated engine laws)
provides:
  - A/C frequency-weighting tables at the 105 exact 1/12-octave grid centres (D-09)
  - OPFS tensor reader — decode frozen chunk bytes back to Array3 (inverse of opfs_sink)
  - Per-receiver two-channel readout orchestration (band levels + dB(A)/dB(C) + coherent/incoherent split)
  - MAC ≡ engine bit-exact equivalence guarantee (the load-bearing acceptance gate)
affects: [11-02, 11-03, 11-04, 11-05, 11-06, 11-07, weather-scenarios, exports]

# Tech tracking
tech-stack:
  added: [ndarray (envi-compute direct dep), num-complex (envi-compute direct dep)]
  patterns:
    - "Drive engine laws, never paraphrase them — readout uses readout_coherent/readout_incoherent/compose_gain/band_levels_db_two_channel; zero bespoke dB/MAC loops"
    - "IEC 61672-1 analytic A/C weighting precomputed once at the exact grid centres → instant dB(A)⇄dB(C) toggle"
    - "OPFS reader is the exact inverse of the sink's frozen [s][r_local][f] byte layout; typed decode error, never a panic"

key-files:
  created:
    - crates/envi-compute/src/weighting.rs
    - crates/envi-compute/src/readout.rs
    - crates/envi-compute-wasm/src/opfs_reader.rs
  modified:
    - crates/envi-compute/src/lib.rs
    - crates/envi-compute/Cargo.toml
    - crates/envi-compute-wasm/src/lib.rs

key-decisions:
  - "Readout-law policy (Open Q2): no source-type flag exists; default_law derives coherent (filter/delay present) vs incoherent (plain) from source composition, documented in the readout.rs Module I/O header"
  - "band_levels_db comes from band_levels_db_two_channel (engine law) by encoding the coherent/incoherent energies into its inputs — no bespoke 10·log10 band loop in readout.rs"
  - "ndarray + num-complex promoted to envi-compute direct deps to name the engine tensor-view types; engine 3-dep quarantine untouched (verified cargo tree)"

patterns-established:
  - "A/C weighting table + weighted_total_db aggregate strictly by band index (never nominal Hz)"
  - "Coherent channel = |Σ_s H_coh·G_s|² (readout_coherent); incoherent channel = Σ_s |G_s|²·P_incoh_abs (readout_incoherent with a zero H_coh)"

requirements-completed: []  # WEB-11 advanced here (readout foundation) but COMPLETES in 11-05 (spectrum panel UI + dB(A)⇄dB(C) toggle) — left Pending, not marked complete.

# Metrics
duration: 42 min
completed: 2026-07-12
status: complete
---

# Phase 11 Plan 01: Readout Foundation Summary

**Pure-Rust/WASM acoustic readout foundation: IEC 61672-1 A/C weighting tables at the 105 grid centres, an OPFS tensor reader that byte-exactly inverts the Phase-10 sink, and a per-receiver two-channel readout that drives the FORCE-validated engine laws — proven bit-for-bit equal to a direct engine MAC.**

## Performance

- **Duration:** ~42 min (incl. a 6m 37s release build)
- **Tasks:** 3
- **Files modified:** 6 (3 created, 3 modified) + STATE.md/ROADMAP.md

## Accomplishments

- **A/C weighting (D-09):** `a_weighting_db`/`c_weighting_db` evaluate the IEC 61672-1:2013 analytic transfer functions (poles 20.6/107.7/737.9/12194, offsets +2.00/+0.06) at the exact `FreqAxis::centres`, with an anchor test asserting `A(1000)=0`, `C(1000)=0` at band index 64 and the third-octave Table-3 checkpoints (A(100)≈−19.1, A(10k)≈−2.5, C(100)≈−0.3, C(10k)≈−4.4) within Class-1 tolerance. Precomputed once → instant dB(A)⇄dB(C) toggle.
- **OPFS tensor reader:** `read_chunk` decodes the frozen `H_coh` (16 B/cell interleaved re,im f64-LE) + `P_incoh_abs` (8 B/cell f64-LE) chunk bytes back into `[n_sub, chunk_len, N_BANDS]` `Array3`s in `[s][r_local][f]` logical order — the exact inverse of `opfs_sink::put_chunk`. Byte-length + finiteness validation with a typed `ChunkDecodeError` (never a panic; threat T-11-01-01). A native round-trip test drives the real sink and asserts index-for-index `f64::to_bits` equality on both channels, including the non-contiguous sliced-view (Pitfall 7) case.
- **Readout orchestration (D-08):** `readout_receiver` composes per-sub-source gains via `compose_gain`, then drives `readout_coherent` (OUT-03 MAC) and `readout_incoherent` (Annex-A) over the per-receiver slice, converting to per-band dB via the engine's `band_levels_db_two_channel`. Returns `ReceiverReadout { band_levels_db, coherent_energy, incoherent_energy, total_dba, total_dbc }` — the coherent/incoherent split is always present, both weighted totals always produced.
- **MAC ≡ engine gate:** the load-bearing test proves `readout_receiver`'s coherent energy equals a direct `readout_coherent` call bit-for-bit (`f64::to_bits`) over a synthetic tensor. The +3.0103 dB (not +6) test pins the Annex-A incoherent law.

## Task Commits

1. **Task 1: A/C weighting tables** — `test(11-01)` RED + `feat(11-01)` GREEN (TDD)
2. **Task 2: OPFS tensor reader** — `feat(11-01)` (auto)
3. **Task 3: Readout orchestration + MAC≡engine gate** — `test(11-01)` RED + `feat(11-01)` GREEN (TDD)

_Plan metadata commit follows this SUMMARY._

## Files Created/Modified

- `crates/envi-compute/src/weighting.rs` — A/C tables + `Weighting` selector + `weighted_total_db` (by band index)
- `crates/envi-compute/src/readout.rs` — `readout_receiver`, `ReceiverReadout`, `ReadoutLaw`, `Conditioning`, `default_law`
- `crates/envi-compute-wasm/src/opfs_reader.rs` — `read_chunk` + `ChunkDecodeError`
- `crates/envi-compute/src/lib.rs` — registered `weighting`, `readout`
- `crates/envi-compute/Cargo.toml` — `ndarray`/`num-complex` promoted to direct deps
- `crates/envi-compute-wasm/src/lib.rs` — registered `opfs_reader`

## Decisions Made

- **Open Q2 (coherent-vs-incoherent law):** resolved by deriving the law from source composition. `default_law` returns `Coherent` when any sub-source carries a conditioning filter or a non-zero delay (conditioned/directional), else `Incoherent` (road/omni, Annex-A). Documented in the `readout.rs` Module I/O header; the caller may still pass an explicit `ReadoutLaw`.
- **No bespoke dB in `readout.rs`:** `band_levels_db` is produced by the engine's `band_levels_db_two_channel` (encoding coherent energy as `|H_coh|²` and the incoherent channel as `|H_ff|²·p_incoh` with `p_incoh ≡ 1`), so there is no `10·log10` band loop in the orchestration layer — the divergence risk (RESEARCH Pitfall 1) is structurally avoided.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Promote `ndarray`/`num-complex` to envi-compute direct dependencies**
- **Found during:** Task 3 (readout orchestration)
- **Issue:** `readout.rs` names the engine tensor-view types `ArrayView3<Complex<f64>>` and `Array3`, but `ndarray` was only a transitive dep and `num-complex` was dev-only in `envi-compute` — the crate would not compile (`E0432: unresolved import ndarray`).
- **Fix:** Added `ndarray = "0.17"` + `num-complex = "0.4"` to `[dependencies]` (same pins as `envi-compute-wasm`), removed the now-redundant `num-complex` dev-dependency. Both are WASM-safe and already in the graph transitively.
- **Files modified:** crates/envi-compute/Cargo.toml
- **Verification:** `cargo tree -p envi-engine` shows the engine's direct deps unchanged (`ndarray + num-complex + thiserror`); the 3-dep quarantine is one-directional and gains nothing from this edge.
- **Committed in:** the Task 3 RED commit.

**2. [Rule 1 - Test correctness] Tightened the D-08 split sanity assertion**
- **Found during:** Task 3 GREEN
- **Issue:** the initial split/totals test asserted `|dB(A) − dB(C)| > 1.0`, which is not guaranteed for a synthetic spectrum whose energy concentrates near mid-band (where A ≈ C ≈ 0).
- **Fix:** replaced it with a stronger, deterministic assertion — the totals equal `weighted_total_db(band_levels, A/C table)` bit-for-bit (proving correct wiring to the Task-1 tables) and are not identical.
- **Files modified:** crates/envi-compute/src/readout.rs (test only)
- **Verification:** test passes; the load-bearing MAC-equivalence and +3 dB gates were unaffected.
- **Committed in:** the Task 3 GREEN commit.

---

**Total deviations:** 2 auto-fixed (1 blocking dependency, 1 test correctness)
**Impact on plan:** Both necessary; no scope creep. The engine quarantine remains byte-identical.

## Issues Encountered

None — all three acceptance gates (IEC anchor test, byte-exact OPFS round-trip, MAC ≡ engine bit-exact) pass on the first green.

## Verification

All commands run at the workspace root on `main`:

- `cargo build --release` → **Finished** (6m 37s), exit 0.
- `cargo test` (full workspace) → **all pass**, exit 0. Includes the new A/C weighting IEC-61672-1 anchor tests (5), the OPFS `read_chunk` round-trip/malformed/non-finite tests (4), and the readout MAC≡engine + Annex-A + policy tests (5).
- `cargo clippy --all-targets -- -D warnings` → **clean** (whole workspace).
- `cargo fmt --check` → **clean** (exit 0).
- `cargo tree -p envi-engine` → direct deps `ndarray + num-complex + thiserror` (+ `approx` dev-dep) — **unchanged** (engine 3-dep quarantine intact, D-02).
- Grep gates: `737.9` present in `weighting.rs` (3×); `H_BYTES_PER_CELL|P_BYTES_PER_CELL` present in `opfs_reader.rs` (16×); `readout_coherent|readout_incoherent` present in `readout.rs` (11×); no bespoke `10·log10` band loop in `readout.rs` production code.

## Requirements

**WEB-11** is *advanced* by this plan (the readout foundation: A/C tables, band-index aggregation, coherent/incoherent split, OPFS read-back) but **completes in 11-05** (the spectrum panel UI + 1/3-oct display + instant dB(A)⇄dB(C) toggle). It is intentionally left **Pending** in REQUIREMENTS.md rather than marked complete here, since the user-facing capability is not yet shipped.

## Next Phase Readiness

- Wave-2 plans **11-02** (isophone level-grid → marching-squares) and **11-03** (recondition MAC boundary + 409/HashMismatch) can now consume `readout_receiver`, the A/C tables, and `read_chunk`.
- The directivity-phase seam (`SolveJob::directivity_phase_rad`) remains not-yet-wired from the harness; a directional source can be forced through the coherent law via the explicit `ReadoutLaw` argument when that path lands.

## Self-Check: PASSED

- Created files exist on disk: `crates/envi-compute/src/weighting.rs`, `crates/envi-compute/src/readout.rs`, `crates/envi-compute-wasm/src/opfs_reader.rs`.
- Task commits present in `git log` (test/feat `(11-01)` for Tasks 1–3).
- All plan `<verification>` commands re-run green (build/test/clippy/fmt/tree above).

---
*Phase: 11-results-fast-recalc*
*Completed: 2026-07-12*
