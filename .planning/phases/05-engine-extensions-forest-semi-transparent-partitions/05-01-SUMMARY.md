---
phase: 05-engine-extensions-forest-semi-transparent-partitions
plan: 01
subsystem: engine
tags: [nord2000, forest, sub-model-10, pchip, scipy-oracle, solver-seam, acoustics]

# Dependency graph
requires:
  - phase: 04-transfer-tensor-directional-sources-full-validation
    provides: promoted envi_engine::solver + SolveJob seam + two-channel H_coh/P_incoh tensor
  - phase: 02-ground-diffraction-composition
    provides: terrain_effect Eq. 332 composition + single-conj quarantine
provides:
  - "envi_engine::forest — SM10 excess attenuation (Eqs. 288-291, Tables 8/9), engine-owned f64 math"
  - "ForestCrossing (validated) + forest_delta_l(f, fc, c0) -> f64 (ΔL_s ≤ 0, floored at -15 dB)"
  - "Hand-rolled scipy-equivalent tensor-product PCHIP (FC-Butland), no linalg/FFT crate"
  - "SolveJob.forest seam + post-conj two-channel application (10^{ΔL_s/20} on H_coh, 10^{ΔL_s/10} on P_incoh_abs)"
  - "Pinned bit-exact pre-extension solver baseline (None-path structural identity)"
  - "Committed scipy forest oracle (no Python at test time) + F1-F7 test ladder"
affects: [phase-07-scene-objects-SCN-04, phase-09-geometry-GEOX, phase-10-calc-service, phase-05-03-doc-closeout]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Optional extension with a bit-identical absent path (forest: None pinned by a committed to_bits fixture)"
    - "Tabulated document data as consts + hand-rolled scipy-equivalent PCHIP interpolation"
    - "Post-conj per-path real-dB factor at the crate root (directivity.rs placement precedent)"

key-files:
  created:
    - crates/envi-engine/src/forest.rs
    - crates/envi-harness/tests/solve_baseline.rs
    - crates/envi-harness/tests/fixtures/regression/solve_baseline.toml
    - tools/nord2000_oracle/gen_forest_fixtures.py
    - crates/envi-harness/tests/fixtures/oracle/forest.toml
    - crates/envi-harness/tests/oracle_forest.rs
  modified:
    - crates/envi-engine/src/lib.rs
    - crates/envi-engine/src/solver.rs
    - crates/envi-harness/tests/mac_identity.rs
    - crates/envi-harness/tests/tensor_budget.rs

key-decisions:
  - "ENG-09 implemented as the REAL Nord2000 SM10 law (Eqs. 288-291, quadratic-then-saturating, -15 dB floor, exact 0 below ka=0.7), NOT the TI 386 'A = d·a(f)' linear paraphrase (no such formula exists in AV 1106/07)"
  - "D-01 ForestCrossing interface amended (research-mandated): 'kp' DROPPED (it is Table 8's computed k_f), height_m ADDED (h' = nQ·h needs average tree height, Pitfall 5)"
  - "Fs coherence factor (Eq. 288) DEFERRED with a documented seam (module header + plan 05-03 deferred-items) — D-03's locked 'excess attenuation, not decorrelation' scope"
  - "Table 9 'cubic interpolation' realized as monotone PCHIP (scipy-equivalent), nested R'->α->log10(h'); R' clamped consistently both in the table lookup and the 20·log10(8R') term (A1/A3)"

patterns-established:
  - "Pinned pre-extension bit-baseline: capture f64 to_bits per band from the pre-seam tree, opt-in #[ignore] generator, DO-NOT-REGENERATE — proves a new Option seam's None path is byte-identical"
  - "Hand-rolled Fritsch-Carlson-Butland PCHIP matching scipy.interpolate.PchipInterpolator to 1e-9 with no new dependency"

requirements-completed: [ENG-09]

# Metrics
duration: 25min
completed: 2026-07-09
status: complete
---

# Phase 5 Plan 01: ENG-09 Forest Excess Attenuation (Sub-Model 10) Summary

**Nord2000 §5.19 Sub-Model 10 forest excess attenuation (Eqs. 288-291, Tables 8/9) as engine-owned f64 math with a hand-rolled scipy-equivalent PCHIP, wired through a `SolveJob.forest` seam as a post-conj two-channel magnitude factor, validated by a full F1-F7 anchor+oracle ladder and a pinned bit-exact None-path baseline.**

## Performance

- **Duration:** 25 min
- **Started:** 2026-07-09T12:16:55Z
- **Completed:** 2026-07-09T12:42:20Z
- **Tasks:** 3
- **Files modified:** 10 (6 created, 4 modified)

## Accomplishments
- `forest.rs` implements the verified SM10 law: `nQ` (Eq. 290), `T` (Eq. 289), `k_f` (Table 8 linear-interp, edge-clamped), `A_e = ΔL(h′,α,R′) + 20·log₁₀(8R′)` (Table 9 tensor-product PCHIP), `ΔL_s = Max(1.25·k_f·T·A_e, −15)` (Eq. 291); exactly `0.0` below `ka = 0.7` or `T = 0`.
- `ForestCrossing` with a non-finite-rejecting validating constructor (typed `ForestError`); the D-01 interface amendment (drop `kₚ`, add `height_m`) documented in the struct rustdoc.
- `SolveJob.forest` seam applies `ΔL_s` post-conj as a real factor on both channels — `arg(H_coh)` untouched, `P_incoh` energy-scaled — never inside `propagation/` (conj grep-gate stays 0).
- Hand-rolled Fritsch-Carlson-Butland PCHIP matches `scipy.interpolate.PchipInterpolator` to 1e-9 dB (F5 oracle), with no new engine dependency.
- Full F1-F7 ladder green: F1 exact-zero, F2 on-node hand anchor (formula-computed to 1e-12), F3 floor + monotonicity, F4 randomized bounds sweep, F5 scipy oracle, F6 two-channel magnitude, F7 `F→1 ⇒ P_incoh→0` bit-exact.
- Pinned pre-extension solver baseline proves the `forest: None` path is byte-identical.

## Task Commits

Each task was committed atomically:

1. **Task 1: Pin the pre-extension solver baseline** - `158442f` (test)
2. **Task 2: forest.rs — Sub-Model 10 (Eqs. 288-291, Tables 8/9)** - `8c43a6b` (feat)
3. **Task 3: SolveJob.forest seam + two-channel application + scipy oracle** - `491dcd7` (feat)

**Plan metadata:** committed with this SUMMARY (docs: complete plan)

## Files Created/Modified
- `crates/envi-engine/src/forest.rs` - SM10 math + Tables 8/9 consts + scipy-equivalent PCHIP + F1-F4/constructor tests
- `crates/envi-engine/src/lib.rs` - registered `pub mod forest;`
- `crates/envi-engine/src/solver.rs` - `SolveJob.forest` field + post-conj two-channel application in `solve_pair`; F6/F7 tests; chain-doc updated
- `crates/envi-harness/tests/solve_baseline.rs` - pinned bit-exact baseline generator + comparison test
- `crates/envi-harness/tests/fixtures/regression/solve_baseline.toml` - 105×3 hex f64 bit patterns (DO-NOT-REGENERATE)
- `tools/nord2000_oracle/gen_forest_fixtures.py` - scipy PCHIP + numpy-interp forest oracle generator
- `crates/envi-harness/tests/fixtures/oracle/forest.toml` - committed forest oracle fixture (sha256 provenance)
- `crates/envi-harness/tests/oracle_forest.rs` - F5 per-band-index oracle test with zero/attenuating/floor coverage
- `crates/envi-harness/tests/mac_identity.rs`, `tensor_budget.rs` - mechanical `forest: None` on existing SolveJob literals

## Decisions Made
- **Real SM10 law, not the linear paraphrase.** AV 1106/07 defines no per-metre `a(f)`; the roadmap/TI 386 "A = d·a(f)" is the NoizCalc paraphrase of Sub-Model 10, whose parameter list matches 1:1. Implemented Eqs. 288-291 exactly (verified against the page images in 05-RESEARCH Finding 1). This satisfies roadmap SC1 — Nord2000's own `T`-saturation + −15 dB floor is its distance bounding, so the ISO 9613-2 10/20/200 m regimes were correctly excluded.
- **D-01 interface amendment (flagged).** `kₚ` dropped as an input (it is Table 8's tabulated `k_f`, computed — precisely D-02's "engine owns the formula"); `height_m` added (`h′ = nQ·h` is un-evaluable without average tree height). Downstream-consistent with Phase-7 SCN-04.
- **Fs (Eq. 288) deferred with a seam (flagged).** Nord2000 also decorrelates via `Fs = 1 − k_f·T`; D-03's locked scope is excess attenuation only. Recorded in the `forest.rs` module header as a documented seam (`CoherenceInputs` multiplicative factor; screen F₂/F₃/F₄) to revisit when real forest crossings land (Phase 9). Plan 05-03 will add the `deferred-items.md` entry.
- **PCHIP interpolation + consistent R′ clamp (A1/A3).** Table 9's "cubic interpolation" realized as monotone PCHIP (no overshoot past tabulated values ⇒ no spurious positive `A_e`), nested R′→α→log₁₀(h′); `R′` clamped to `[0.0625, 10]` in both the lookup and the `20·log₁₀(8R′)` term.

## Deviations from Plan

None - plan executed exactly as written. The D-01 interface amendment and the Fs deferral were both pre-authorized by the plan (baked into the plan's objective and `must_haves`, sourced from 05-RESEARCH), so they are executed-as-specified rather than execution-time deviations. No deviation rules were triggered.

## Issues Encountered
- **TOML table capture (Task 1).** Emitting the top-level bit arrays *after* a `[meta]` table caused serde to capture them into `meta` (missing-field parse error). Fixed by emitting the arrays before the `[meta]` header. Resolved before commit.
- **PCHIP zero-secant sign handling.** `f64::signum` returns `±1.0` for `±0.0` (never `0`), which would break the scipy shape-preservation conditions; added an explicit three-valued `sgn` helper so the hand-rolled PCHIP matches scipy on flat table rows.

## User Setup Required

None - no external service configuration required. (Oracle regeneration is operator-driven and dev-time only; Python/numpy/scipy are not build or test dependencies — the committed TOML is used at test time.)

## Next Phase Readiness
- ENG-09 lands end-to-end; ready for Phase-7 SCN-04 (forest scene object carrying `height_m`) and Phase-9/10 path-geometry + calc wiring.
- **Carry-forward:** the Fs (Eq. 288) coherence-reduction seam — to be recorded in `deferred-items.md` by plan 05-03 and revisited at Phase 9.
- Wave-1 sibling plan 05-02 (ENG-10 min-phase kernel) is independent; plan 05-03 (ENG-10 integration + doc close-out) is Wave 2.
- Quality gates green: `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `cargo test` (212 engine + 108 harness + integration; FORCE 10 pass / 102 ignored, no capability flips); conj grep-gate 0; dep quarantine intact; `#![deny(unsafe_code)]` intact.

## Self-Check: PASSED

All 6 created files exist on disk; all 3 task commits (`158442f`, `8c43a6b`, `491dcd7`) are present in git history.

---
*Phase: 05-engine-extensions-forest-semi-transparent-partitions*
*Completed: 2026-07-09*
