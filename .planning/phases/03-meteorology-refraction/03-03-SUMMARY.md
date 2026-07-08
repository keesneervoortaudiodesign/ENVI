---
phase: 03-meteorology-refraction
plan: 03
subsystem: engine (propagation::coherence + terrain_effect refraction) + harness (weather Routes 1/3, capability)
status: complete
completed: 2026-07-08
tags: [refraction, coherence, F_tau, Eq-112, MET-05, MET-06, ENG-08, route1, route3, monin-obukhov, LSQ, capability-flip, ASSUMED, honest-green]
requires:
  - "envi-engine::propagation::coherence::{CoherenceInputs, coherence_ff, coherence_f} (Phase-2 seam; f_delta_nu was 1.0)"
  - "envi-engine::propagation::refraction::SoundSpeedProfile (03-01) + eqssp::calc_eq_ssp"
  - "envi-engine::propagation::rays::circular_rays (03-01 Δτ)"
  - "envi-engine::propagation::terrain_effect::terrain_effect refraction dispatch (03-01)"
  - "envi-engine::propagation::refraction::profile::Z0_MIN_M + sound_speed_ms (Coft)"
  - "envi-harness::cases::{CaseLoadError, PropagationParams} + the calamine .xls pattern (cases/xls.rs)"
  - "tools/nord2000_oracle/gen_refraction_fixtures.py (03-01/03-02 oracle generator)"
provides:
  - "propagation::coherence::coherence_f_delta_nu(f, dtau, dtau_plus) — Eq. 112 sinc, x=2π·f·|Δτ⁺−Δτ|"
  - "SoundSpeedProfile::{s_a, s_b} fields (Eq. 10 A⁺=A+1.7·sA / B⁺=B+1.7·sB fluctuation std-devs)"
  - "terrain_effect Δτ⁺ wiring: RefractionState.dtau_plus → FΔν injected via CoherenceInputs::f_delta_nu (no call-site change)"
  - "envi-harness::weather::route1::{energy_weighted_level, energy_weighted_over_classes, l_den, load_met_probabilities, MetClass, Period, N_MET_CLASSES}"
  - "envi-harness::weather::route3::{reconstruct_profiles, fit_profile, SurfaceMet, default_heights} (MO + hand-rolled 3×3 LSQ)"
  - "envi-harness::capability::implemented_capabilities now contains Capability::Refraction"
  - "refraction.toml oracle: [[route3_fit]] round-trip rows (recover A/B/C exactly)"
affects:
  - "Phase 4 emission model: FORCE wind/gradient cases now Skipped on emission-model ONLY (refraction dropped) — the last capability gate before the road source"
  - "Phase 4 fast-recalc tensor consumes the same phase-preserving H_coh_factor (FΔν multiplies, never overwrites phase)"
  - "Milestone-2 METX (Phase 9 weather half) reuses route3::fit_profile / reconstruct_profiles for the A/B/C derivation"
tech-stack:
  added: []
  patterns:
    - "FΔν injected through the pre-built CoherenceInputs::f_delta_nu seam with ZERO call-site change to coherence_f (D-10)"
    - "Δτ⁺ precomputed once per terrain_effect call (frequency-independent geometry) in RefractionState; sA=sB=0 ⇒ Δτ⁺=Δτ bit-exact ⇒ FΔν=1"
    - "hand-rolled 3×3 normal-equations LSQ (Cramer + determinant guard) — no nalgebra/ndarray-linalg (D-08)"
    - "energy-weighting validated as a method-defined identity; [ASSUMED] class→(A,B) never numerically pinned (D-04)"
    - "round-trip identity oracle for the LSQ fit (synthetic exact profile → recover A/B/C)"
    - "label-anchored calamine read of the Met. statistics sheet; fail-soft typed errors when refs/ absent"
key-files:
  created:
    - crates/envi-harness/src/weather/route1.rs
    - crates/envi-harness/src/weather/route3.rs
  modified:
    - crates/envi-engine/src/propagation/coherence.rs
    - crates/envi-engine/src/propagation/refraction/mod.rs
    - crates/envi-engine/src/propagation/terrain_effect/mod.rs
    - crates/envi-harness/src/weather/mod.rs
    - crates/envi-harness/src/capability.rs
    - crates/envi-harness/src/lib.rs
    - crates/envi-harness/tests/oracle_refraction.rs
    - crates/envi-harness/tests/fixtures/oracle/refraction.toml
    - tools/nord2000_oracle/gen_refraction_fixtures.py
decisions:
  - "Task-1 checkpoint resolved by the developer as option (c): proceed with the [ASSUMED] weather-route A/B/C constants clearly quarantined, validated by structural + direction property tests and the same-transcription committed oracle ONLY — NO false FORCE numeric Pass. Wind/gradient FORCE cases stay Skipped(requires: emission-model) until Phase 4."
  - "SoundSpeedProfile extended with s_a/s_b (engine refraction/mod.rs — NOT in the plan file list; necessary Rule-3 seam so the engine can form the Eq. 10 A⁺/B⁺ profile and compute Δτ⁺). homogeneous() sets them 0."
  - "FΔν multiplies the incoming CoherenceInputs::f_delta_nu (default 1.0) rather than overwriting it — general and bit-identical on the non-fluctuating path; never touches the +j phase of h_coh_factor (D-12)."
  - "F_τ is property-tested only (bit-exact 1.0 off-turbulence, monotone + π cutoff, 2π-not-0.23π sinc pin, dip-filling direction) — no fixed-value oracle (turbulence std-devs are AV Assumptions, D-11)."
  - "Route 1 committed tests validate the energy-weighting identity + L_den penalties on synthetic data; the Met. statistics .xls loader is exercised only fail-soft (structural distribution check when refs/ present)."
  - "Route 3 fit_profile solved by hand-rolled 3×3 Cramer with a determinant/scale singular guard (typed error) — no linalg crate; cargo tree -p envi-engine unchanged."
  - "lib.rs run_case Refraction arm updated (not in the plan file list): after the capability flip these reference-free (bands=none) synthetic cases would reach the arm — message corrected to an honest no-numeric-reference Skip, never a false Pass."
metrics:
  tasks: 2 (Task 1 checkpoint pre-resolved by the developer)
  commits: 3
  files_created: 2
  files_modified: 9
  engine_tests: 155
  harness_tests: 68
  duration_min: 17
---

# Phase 3 Plan 3: F_τ Turbulence Coherence + Weather Routes 1/3 + Refraction Capability Flip Summary

The slice that closes Phase 3: the fluctuating-refraction coherence factor `FΔν` (Eq. 112) wired through the pre-built coherence seam, the two remaining weather-input routes (Route 1 weather-class → energy-weighted `L_den`, Route 3 Monin–Obukhov + 3×3 LSQ), and the `Capability::Refraction` flip that shrinks the FORCE wind/gradient skip-reason to `emission-model` only — honestly green, no false numeric Pass.

## What was built

**Task 1 — checkpoint (pre-resolved).** The blocking-human checkpoint locking the source of truth for the `[ASSUMED]` weather-route A/B/C constants was resolved by the developer as **option (c)**: proceed with the constants clearly quarantined, validated by structural + direction property tests and the same-transcription committed oracle only, with NO false FORCE numeric Pass. Execution skipped straight to Tasks 2–3 under that posture.

**Task 2 — F_τ fluctuating-refraction coherence (Eq. 112).** `coherence::coherence_f_delta_nu(f, dtau, dtau_plus)` — the sinc with the **full `2π` argument** `x = 2π·f·|Δτ⁺−Δτ|` (NOT the `0.23π` band-averaging factor of `coherence_ff`, Pitfall 5): `1.0` for `x ≤ 1e-15`, `sin(x)/x` for `x ≤ π`, `0` beyond. `SoundSpeedProfile` gained `s_a`/`s_b` (Eq. 10 std-devs). In the `terrain_effect` single-surface refraction path, `RefractionState` precomputes `Δτ⁺` by running `circular_rays` on the upper-refraction profile `A⁺=A+1.7·sA`, `B⁺=B+1.7·sB` (`ξ⁺` via `calc_eq_ssp`), and the per-band eval multiplies `FΔν` into `CoherenceInputs::f_delta_nu` — **no call-site change to `coherence_f`** (the D-10 seam) and **never overwriting** the `+j` phase of `h_coh_factor` (D-12). When `sA=sB=0` the plus-profile equals the mean profile ⇒ `Δτ⁺=Δτ` bit-for-bit ⇒ `FΔν=1` ⇒ `P_incoh→0` exactly.

**Task 3a — Route 1 (MET-05).** `weather::route1`: `energy_weighted_level(levels, probs) = 10·lg(Σ pᵢ·10^{Lᵢ/10})` (probabilities normalized, non-finite/zero-sum rejected typed) + `l_den(day, eve, night)` with the END `+0/+5/+10 dB` penalties and 12/4/8-hour weights; `MetClass`/`energy_weighted_over_classes` pair each class's `[ASSUMED]` `(A,B)` with its probability; `load_met_probabilities` is a label-anchored calamine read of the `TestYearlyAverage.xls` `Met. statistics` sheet (three `Direction` blocks → M1–M25, per-direction linear interpolation), fail-soft when `refs/` is absent.

**Task 3b — Route 3 (MET-06).** `weather::route3`: `reconstruct_profiles(met, heights)` builds `u(z)=(u*/κ)[ln(z/z₀+1)−Ψ_m(z/L)]`, `T(z)=t₀+(dt/dz)·z`, `c_eff(z)=20.05·√(T+273.15)+u(z)·cos(az−φ)` via Monin–Obukhov (Businger–Dyer `Ψ_m`, all `[ASSUMED]`); `fit_profile(heights, c_eff, z0)` fits the log-lin model by a **hand-rolled 3×3 normal-equations** solve (`XᵀX β = Xᵀy`, Cramer + determinant/scale singular guard, typed error) — **no nalgebra/ndarray-linalg** (D-08).

**Task 3c — capability flip + oracle.** `Capability::Refraction` added to `implemented_capabilities()`; a new requires-shrink assertion proves a downwind/inversion FORCE case's `missing` set equals exactly `{EmissionModel}` (refraction dropped, emission-model retained). Route-3 round-trip fixtures (`[[route3_fit]]`) added to `gen_refraction_fixtures.py` + `oracle_refraction.rs` (recover A/B/C to 1e-6).

## Verification

- `cargo test` workspace green — engine lib **155**, harness lib **68**, all integration/oracle suites pass; the **67 capability-gated FORCE cases stay `Skipped`**, never a false `Pass`.
- `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt --check` clean.
- `cargo tree -p envi-engine` **unchanged** — direct deps still `ndarray + num-complex + thiserror` only (no new dep, no linalg crate).
- `.conj()` grep gate over `propagation/` = **0** real calls (5 hits are doc-comment mentions; `coherence_f_delta_nu` is real-valued).
- **Honest-green D-03 confirmed by the dynamic runner**: the ONLY skip kind across all FORCE road cases is `[requires: emission-model]`; **zero** FORCE cases still list `refraction` (verified with a grep count of 0). The two synthetic refraction TOML cases (`bands="none"`) fail-soft to `Skipped` with an accurate no-numeric-reference message.
- F_τ: `FΔν=1` bit-exact when `sA=sB=0`; monotone non-increasing with `π` cutoff; the `2π` argument pinned via `sin(1)/1` at `x=1`; fluctuation fills the deepest coherent null (direction) — property tests, no fixed oracle.
- Route 3 `fit_profile` round-trips known `(A,B,C)` to 1e-6 (unit + committed oracle, up/down profiles); collinear/insufficient heights → typed singular error. Route 1 energy-weighting + `L_den` penalty identities validated on synthetic data; the live `Met. statistics` distribution sums to ≈1 per period.

## Deviations from Plan

### Auto-added / interface adjustments (Rule 2/3 — documented)

**1. [Rule 3 - Seam] `SoundSpeedProfile` gained `s_a`/`s_b` (engine `refraction/mod.rs`, not in the plan's Task-2 file list).**
- The engine needs the fluctuation std-devs to form the Eq. 10 `A⁺=A+1.7·sA`, `B⁺=B+1.7·sB` profile and compute `Δτ⁺`. Added the two fields (`homogeneous()` sets them 0); the harness `WeatherProfile` already carried `s_a`/`s_b` from 03-02, so this closes the engine-side mirror. Only two struct literals existed (both engine test helpers) — updated.

**2. [Rule 1 - Accuracy] `lib.rs` `run_case` Refraction arm message (not in the plan's Task-3 file list).**
- After the `Capability::Refraction` flip, the reference-free (`bands="none"`) synthetic refraction TOML cases pass the capability gate and reach this arm, which carried a now-stale `"plans 03-02/03-03"` message. Corrected to an honest `Skipped` naming the absence of a committed numeric reference (the `[weather]` table is not parsed into `CaseDefinition`, so full end-to-end wiring of these cases is out of scope and there is no reference to compare) — stays `Skipped`, never a false Pass.

**3. [Rule 2] `energy_weighted_over_classes` convenience added** so `MetClass` is load-bearing (it pairs each class level with its probability) rather than a documentary-only type.

No auth gates occurred. No architectural (Rule 4) changes. The `nonzero-turbulence Nord2000 default on PropagationParams` named in the Task-2 action already existed as `turbulence_or_nord2000_default` (03-01/03-02) — no change was needed and none was made, so `cases/mod.rs` was not modified.

## Known Stubs / [ASSUMED] constants

Per the Task-1 checkpoint outcome (option c), the following are `[ASSUMED]` and **deliberately never pinned to a FORCE numeric Pass** (D-03/D-04):
- **Route 1 class → `(A, B)` mapping** — the `Met. statistics` sheet supplies only occurrence probabilities; the per-class profile coefficients are `[ASSUMED]`. Only the **energy-weighting combination** (a method-defined identity) is committed-tested.
- **Route 3 Monin–Obukhov stability constants** (`Ψ_m` Businger–Dyer `β=5`, the `1/L` stability proxy) — validated by direction/structure property tests only; the LSQ fit is validated by the round-trip identity.
- **F_τ turbulence std-devs / `Cv²`/`CT²` defaults** — validated by F_τ direction/monotonicity property tests, no fixed-value oracle.

These are expected, planned quarantine, not unresolved stubs. They resolve numerically only when the Phase-4 road-emission model lands (the wind/gradient FORCE cases then drop their last `emission-model` gate) or when a companion reference (AV 1851/00 Part 2 / CNOSSOS-EU meteo tables) is supplied.

## Self-Check: PASSED

- Created files exist: `crates/envi-harness/src/weather/route1.rs`, `crates/envi-harness/src/weather/route3.rs` — FOUND.
- Modified files present with the new symbols (`coherence_f_delta_nu`, `SoundSpeedProfile::s_a/s_b`, `RefractionState.dtau_plus`, `route1::energy_weighted_level`, `route3::fit_profile`, `Capability::Refraction` in `implemented_capabilities`, `[[route3_fit]]` oracle rows) — FOUND.
- Commits FOUND: 8b83fde (Task 2 — F_τ), 30023cf (Task 3 — Routes 1/3 + capability flip).
