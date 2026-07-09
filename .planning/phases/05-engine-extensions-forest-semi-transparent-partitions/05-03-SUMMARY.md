---
phase: 05-engine-extensions-forest-semi-transparent-partitions
plan: 03
subsystem: engine
tags: [nord2000, semi-transparent, isolation, minimum-phase, screen, opaque-regression, two-channel, envi-extension, doc-closeout]

# Dependency graph
requires:
  - phase: 05-engine-extensions-forest-semi-transparent-partitions
    provides: "05-02 min-phase kernel (IsolationSpectrum, TransmissionFilter::from_isolation, native T(f)=|T|·e^{−jφ_cep})"
  - phase: 05-engine-extensions-forest-semi-transparent-partitions
    provides: "05-01 forest seam (SolveJob.forest) + the pinned pre-extension solver baseline pattern"
  - phase: 04-transfer-tensor-directional-sources-full-validation
    provides: "promoted envi_engine::solver + SolveJob seam + two-channel H_coh/P_incoh tensor"
  - phase: 02-ground-diffraction-composition
    provides: "terrain_effect Eq. 332 composition + screen_channel + single-conj quarantine"
provides:
  - "SolveJob.isolation: Option<&IsolationSpectrum> — the semi-transparent partition seam on the one solve path (D-05)"
  - "terrain_effect 8th param isolation + IsolationWithoutScreen guard + one-shot TransmissionFilter precompute (Q2)"
  - "screen_channel 4th param transmission: Option<Complex> — the single §5.13–5.15 composition point, structural None gating (D-10)"
  - "PropagationError::IsolationWithoutScreen (contradictory-input typed error)"
  - "Permanent bit-exact opaque-limit regression (opaque_regression.rs + opaque_baseline.toml, T6) + T7/T8/T9"
  - "Phase-5 documentation contract closed (REQUIREMENTS ENG-09/ENG-10, README, module headers) + deferred-items.md (Fs seam)"
affects: [phase-07-scene-objects-SCN-01/02/03, phase-09-geometry-GEOX, phase-10-calc-service]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Optional propagation/-side seam threaded through a positional param, bit-identical on the None path (refraction:Option<_> precedent)"
    - "Structural Some/None match at the assembly point — NOT a `+ 0.0` (negative-zero bit hazard) — as the bit-exactness guarantee"
    - "Pre-extension bit-exact regression captured BEFORE the seam mutates the producer, pinning the opaque limit (solve_baseline precedent)"

key-files:
  created:
    - crates/envi-harness/tests/opaque_regression.rs
    - crates/envi-harness/tests/fixtures/regression/opaque_baseline.toml
    - .planning/phases/05-engine-extensions-forest-semi-transparent-partitions/deferred-items.md
  modified:
    - crates/envi-engine/src/propagation/mod.rs
    - crates/envi-engine/src/propagation/terrain_effect/mod.rs
    - crates/envi-engine/src/solver.rs
    - crates/envi-harness/src/lib.rs
    - crates/envi-harness/tests/terrain.rs
    - crates/envi-harness/tests/mac_identity.rs
    - crates/envi-harness/tests/tensor_budget.rs
    - crates/envi-harness/tests/solve_baseline.rs
    - README.md
    - .planning/REQUIREMENTS.md

key-decisions:
  - "Composition point = screen_channel (single §5.13–5.15 assembly for SM4/5/6): h_semi = h_opaque + T, one complex add relative to p̂₀, OUTSIDE the F-weighting (T is deterministic & fully coherent, never decorrelated, never in p_incoh)"
  - "Q2 signature disposition: isolation is terrain_effect's 8th POSITIONAL param (+ #[allow(clippy::too_many_arguments)]) — minimal mechanical churn on a bit-exactness-critical change; options-struct refactor deferred to when Fs lands"
  - "D-10 opaque = structural None: screen_channel's None arm returns base.h_coh_factor EXACTLY (match, not `+0.0` which flips negative-zero bits); pinned by a pre-extension bit-exact fixture captured in Task 1 before Task 2 touched terrain_effect"
  - "R→0 documented model property (Pitfall 7): R≡0 restores the direct field PLUS the diffracted residue — inherent to the additive composition, benign, NOT renormalized"

requirements-advanced: [ENG-10]

# Metrics
duration: 23min
completed: 2026-07-09
status: complete
---

# Phase 5 Plan 03: ENG-10 Integration + Opaque Regression + Doc Close-out Summary

**The plan-05-02 min-phase transmission kernel is threaded into the one solve path (SolveJob.isolation → terrain_effect → screen_channel, native-side/pre-conj per D-05) so semi-transparent screens/façades add the straight-through `T(f)` to the coherent screen factor as complex pressure; the opaque limit is pinned bit-for-bit by a permanent pre-extension regression (D-10 structural None), the two-channel contract is test-locked (T never touches p_incoh), and the Phase-5 documentation contract is closed with the Fs deferral written down.**

## Performance
- **Duration:** ~23 min
- **Started:** 2026-07-09T13:12:43Z
- **Completed:** 2026-07-09T13:35:51Z
- **Tasks:** 3
- **Files:** 13 (3 created, 10 modified)

## Accomplishments
- **Opaque-limit regression (T6, D-10/SC3).** `opaque_regression.rs` + `opaque_baseline.toml` capture `terrain_effect`'s 105-band `h_coh_factor` (re/im), `p_incoh`, and `delta_l_db` as `f64` to_bits for two screen classes (SingleEdge case71, ThickScreen case81), generated from the PRE-extension tree (Task 1, before Task 2 touched `terrain_effect`) — so the opaque limit is unfakeable.
- **The seam threaded (D-05, pre-conj, native-side).** `SolveJob.isolation: Option<&IsolationSpectrum>` → `solve_pair` passes it as `terrain_effect`'s 8th arg → `TransmissionFilter::from_isolation` built ONCE before the band loop → per-band native `T` passed into `screen_channel` by BAND INDEX → structural `Some(t) ⇒ base+t` / `None ⇒ base` at the coherent factor.
- **`IsolationWithoutScreen` typed error** (`propagation/mod.rs`) refuses a partition spectrum over flat terrain (contradictory input, threat T-05-03-04).
- **Full T6–T9 ladder green:** T6 opaque bit-exact; T7 deep-shadow rise + `(h_semi − h_opaque) == T_native` to 1e-9 at fully-engaged bands (re-pinning the native sign end-to-end); T8 `p_incoh` bit-identical under transmission; T9 typed error on flat+isolation.
- **Mechanical fallout** (compiler-enforced, no defaults): trailing `None` at every `terrain_effect` call site and `isolation: None` on every `SolveJob` literal (~30 sites across engine + harness).
- **Documentation contract closed (D-07):** REQUIREMENTS ENG-09 rewritten to the real SM10 law (Eqs. 288–291) and ENG-10 extended with the min-phase filter; README Phase-5 section + two capability rows; `terrain_effect` module + function headers; `deferred-items.md` records the Fs (Eq. 288) seam, the D-01 amendment, Q3/Q4 carry-forwards, and the Phase-4 directional-phase pointer.
- **Green bar:** `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `cargo test --workspace` (217 engine + 108 harness + all integration suites, both bit-pins, both oracles) all green; conj gate 0; dep quarantine 0; `#![deny(unsafe_code)]` intact.

## Task Commits
1. **Task 1: pin pre-extension opaque screen baseline (T6 groundwork)** — `53c8ece` (test)
2. **Task 2: thread isolation SolveJob→terrain_effect→screen_channel + T6–T9** — `d9a49fa` (feat)
3. **Task 3: phase close-out — REQUIREMENTS/README/deferred-items** — `d4c26bc` (docs)

**Plan metadata:** committed with this SUMMARY (docs: complete plan).

## Composition-point decision (per plan `<output>` request)
- **The single point is `screen_channel`**, after the SM4/5/6 sub-model call: `h_semi = h_opaque + T_native[band]`, one complex addition. It is load-bearing twice: it covers all three screen classes at one code point, and it keeps `T` OUTSIDE the `F`-coherence weighting — the min-phase filter is deterministic and fully coherent, so it must never be decorrelated by `F` nor leak into `p_incoh`.
- **Marginal-zone weighting (documented, not "fixed"):** in the Eq. 332 outer blend the whole screen branch — including `T` — is weighted by `r_scr1`. So `diff = h_semi − h_opaque = r_scr1·T` (leakage only exists where the screen does). At full engagement (`r_scr1 = 1`) `diff == T`, which T7(b) pins to 1e-9.
- **Q2 signature disposition:** `isolation` is the 8th positional parameter (not an options struct) with `#[allow(clippy::too_many_arguments)]` — minimal churn on a bit-exactness-critical change; the options-struct refactor can absorb `refraction`+`isolation` when the Fs seam lands.
- **R→0 model property:** `R ≡ 0` restores the direct field PLUS the diffracted residue (inherent to the additive composition) — a documented benign property, never renormalized (Pitfall 7).
- **FORCE report inventory (pre = post):** `9 Pass / 102 Skipped`, unchanged by this plan — every FORCE path carries `isolation: None`, and the T6 + `solve_baseline` bit-exact regressions prove the `None` path is byte-identical (no capability flips, no false Pass, D-12).

## Deviations from Plan

### Auto-fixed / spec corrections

**1. [Rule 1 — Bug in the plan's T8 test spec] `p_incoh == 0.0 bits` is unachievable for a screen; pinned the exact stronger contract instead.**
- **Found during:** Task 2 (writing T8).
- **Issue:** The plan specified T8 as "zero-turbulence screen case with isolation Some asserts `p_incoh.to_bits() == 0.0` per band". A screen's `p_incoh` is **never** exact-zero: Sub-model 7 floors the shadow at `NO_SCATTER_DB = −300 dB`, so `sm7_energy = 10^(−30) ≈ 1e-30` is added to `p_incoh` for every screen band regardless of turbulence (and isolation cannot be combined with a flat/non-screen geometry — that is exactly the T9 error). So the literal assertion cannot pass.
- **Fix:** Implemented the exact, achievable, and STRONGER pin of the same contract — `p_incoh` is **bit-for-bit identical** between the opaque (`None`) and semi (`Some`) runs, with a non-vacuity guard that the coherent channel genuinely moves. This directly proves "transmission never touches the incoherent channel" (the `source:` note in the plan's own prohibition table: "the only screen_channel mutation is on the h side of from_channels"). Same class of plan-test-spec correction as plan 05-02's T3.
- **Files:** `crates/envi-harness/tests/opaque_regression.rs`.
- **Commit:** `d9a49fa`.

### Notes (not deviations)
- The Q2 8-arg disposition, the D-10 structural-None gating, the R→0 documented property, and the ENG-09/ENG-10 requirement rewordings were all pre-authorized in the plan objective/`must_haves` — executed as specified.
- Acceptance grep `grep -c 'TransmissionFilter::from_isolation' terrain_effect/mod.rs` returns **2**, not 1: one is the single code invocation (built once, pre-loop, line ~223), the other is a rustdoc cross-reference (line ~155). "Built once, pre-loop" is satisfied — exactly one construction in code.

## Documentation contract
- **REQUIREMENTS.md** wording updated for both extensions (checkbox/traceability flip left to phase verification, per the plan). **ENG-10 checkbox stays `[ ]` in this plan** — flipped at phase-completion verification.
- **README.md** Phase-5 section + capability rows + refined forest FORCE status.
- **Module headers** carry their extension notes: `forest.rs`/`transmission.rs` (05-01/05-02), `terrain_effect/mod.rs` (module + `terrain_effect`/`screen_channel` fn docs, this plan), `solver.rs` chain doc + `SolveJob.isolation` rustdoc (this plan).
- **deferred-items.md** records the Fs (Eq. 288) coherence seam with its `CoherenceInputs::f_delta_nu` landing point, the D-01 `ForestCrossing` amendment, Q3 default-params, Q4/D-11 single-spectrum, and the Phase-4 directional-phase pointer.

## Authentication gates
None — no external service configuration. Oracle/fixture regeneration is operator-driven and dev-time only.

## Next Phase Readiness
- ENG-10 lands end-to-end on the promoted solver: semi-transparent screens/façades compute with phase intact, the opaque path is bit-frozen, and the two-channel contract is test-pinned. The per-façade mechanism (D-11) is engine-complete; façade→`R(f)` selection and multi-partition composition are Phase-7/9 upstream (recorded in deferred-items).
- **Carry-forwards for Phase 7/9/10:** the Fs coherence seam (with a user check-in), Nord2000 default forest parameters (SCN-04 research), the `ForestCrossing` road-case geometry wiring, and the still-open Phase-4 directional-phase harness population.
- Quality gates green; conj grep-gate 0; dep quarantine intact; `#![deny(unsafe_code)]` intact; FORCE inventory unchanged.

## Self-Check: PASSED

All 3 created files exist on disk (`opaque_regression.rs`, `opaque_baseline.toml`, `deferred-items.md`) plus this SUMMARY; all 3 task commits (`53c8ece`, `d9a49fa`, `d4c26bc`) are present in git history.

---
*Phase: 05-engine-extensions-forest-semi-transparent-partitions*
*Completed: 2026-07-09*
