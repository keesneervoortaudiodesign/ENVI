# SECURITY — Phase 05: Engine Extensions (Forest + Semi-Transparent Partitions)

**Audit type:** retroactive threat-mitigation verification (`/gsd-secure-phase`)
**ASVS level:** 1
**Block-on:** high
**Date:** 2026-07-09
**Verdict:** SECURED — 18/18 threats resolved (15 mitigate CLOSED, 3 accept logged)

This is a pure-math Rust engine (no network / auth / PII / filesystem-input surface).
The realistic threat class is numerical robustness plus the load-bearing physics/
convention contracts: the single-`.conj()` quarantine, the two-channel
`H_coh` / `P_incoh` separation, and opaque-limit bit-exactness. Every declared
mitigation below was verified present in code (grep + read + live test), not accepted
on documentation or intent.

## Verification method by disposition

- `mitigate` → located the mitigation in the cited file and confirmed the pinning
  test is present AND passing (tests were executed, not merely read).
- `accept` → logged in the Accepted Risks section below with rationale + evidence.

## Threat Verification — Plan 05-01 (Forest Sub-Model 10)

| Threat ID | Category | Disposition | Status | Evidence |
|-----------|----------|-------------|--------|----------|
| T-05-01-01 | Tampering (data integrity) | mitigate | CLOSED | `forest.rs:228-262` validating `ForestCrossing::new` — explicit `!is_finite()` rejection + range checks, typed `ForestError` (no panic). `constructor_rejects_bad_inputs` + F4 sweep (`f4_parameter_sweep_stays_in_bounds`, 20k draws) pass. |
| T-05-01-02 | Tampering (correctness) | mitigate | CLOSED | Tables 8/9 as consts cited "AV 1106/07 pp. 125–127, verified against page images" (`forest.rs:77-142`); Eq. 291 floor `forest.rs:464`; on-node anchors F1 (`f1_below_ka_threshold_is_exactly_zero`), F2 (`f2_on_node_hand_anchor`, formula-computed not hardcoded); scipy oracle F5 `oracle_forest` passes at 1e-9 with zero/attenuating/floor coverage asserts. |
| T-05-01-03 | Tampering (contract) | mitigate | CLOSED | `forest_delta_l` / `eval_band` return `f64` (`forest.rs:451,471`, phase-incapable); applied post-conj at `solver.rs:260-264` — real factor on both channels, `arg(H_coh)` untouched. F6 (`forest_scales_magnitude_two_channels…`) + F7 (`forest_preserves_exact_zero_incoherent_channel`, `to_bits==0`) + `solve_baseline` None-path bit pin all pass. |
| T-05-01-04 | DoS (panic on degenerate input) | mitigate | CLOSED | `R'` clamped to `[0.0625,10]` consistently in BOTH the Table-9 lookup and the `20·log10(8R')` term (`forest.rs:438-439`); early exact-zero return on `T==0`/`k_f==0` (`forest.rs:454,460`); F3 floor+monotone and F4 finiteness sweep pass. |
| T-05-01-05 | Info disclosure / licensing | mitigate | CLOSED | Only transcribed table DATA present, cited by report+page (`forest.rs:1-29,77-146`); no verbatim document text. `refs/AV1106-07-rev4.pdf` is git-ignored (`git check-ignore` confirms). |
| T-05-SC | Tampering (supply chain) | accept | LOGGED | See Accepted Risks. |

## Threat Verification — Plan 05-02 (Min-Phase Kernel)

| Threat ID | Category | Disposition | Status | Evidence |
|-----------|----------|-------------|--------|----------|
| T-05-02-01 | Tampering (data integrity) | mitigate | CLOSED | `IsolationSpectrum::new` (`transmission.rs:125-132`) explicit `is_finite` + `0.0..=MAX_R_DB` per band, typed `InvalidIsolationSpectrum{band,value}`. WR-01 fix present: `MAX_R_DB = 1_000.0` (`transmission.rs:109`) bounds the O(M²) cepstral sum against overflow→NaN. T10 (rejects NaN/±∞/neg/`1e307`/`MAX+1`) + T11 (finite φ at cap) pass. |
| T-05-02-02 | Tampering (sign flip) | mitigate | CLOSED | Native negation as explicit negative sine `Complex::new(mag*c, mag*(-s))` (`transmission.rs:254`), no `.conj()`. Pinned DFT convention in header (`:33-43`). T3 first-principles causality (`t3_causal_sign_flank_antisymmetry_native_negation`) + oracle T5 `oracle_minphase` (1e-9 rad, all 105 bands, native cross-check) pass. |
| T-05-02-03 | Tampering (fold bug) | mitigate | CLOSED | Fold weights `ĉ[0]=c[0]`, `×2` for 1..103, `ĉ[104]=c[104]` (M/2 stays ×1) (`transmission.rs:195-200`). T1 flat⇒0, T2 linearity, T4 `Re(Z)==ln|T|` to 1e-12 pass; oracle full-grid compare passes. |
| T-05-02-04 | Tampering (conj/two-channel contract) | mitigate | CLOSED | Conj gate over `propagation/` = **0** (grep, filtered). Kernel outputs only `[f64;N]` phase and `TransmissionFilter{bands:[Complex;N]}` — no `P_incoh`-typed output exists. |
| T-05-02-05 | DoS (panic on adversarial R) | mitigate | CLOSED | Constructor rejects all non-finite + caps magnitude; kernel is total on validated input (no div, no log of user data beyond finite `ln|T|`). T11 confirms finite φ at the cap. |
| T-05-SC | Tampering (supply chain) | accept | LOGGED | See Accepted Risks (hand-rolled 208-pt DFT specifically to avoid an FFT crate). |

## Threat Verification — Plan 05-03 (Integration + Regression)

| Threat ID | Category | Disposition | Status | Evidence |
|-----------|----------|-------------|--------|----------|
| T-05-03-01 | Tampering (regression) | mitigate | CLOSED | Structural `match transmission { Some(t)=>base+t, None=>base }` — no `+0.0` neg-zero hazard (`terrain_effect/mod.rs:665-668`). Pre-extension bit fixture `opaque_baseline.toml` (captured before the seam) + `opaque_regression` (2 screen classes, `to_bits`) pass; existing screen-oracle suites unmoved. |
| T-05-03-02 | Tampering (P_incoh/SM7 contract) | mitigate | CLOSED | Transmission mutates only `h_coh_factor`; `p_incoh + sm7_energy` untouched (`terrain_effect/mod.rs:670-675`). T8 (opaque vs semi `p_incoh` bit-identical, non-vacuity guard) + `solve_baseline` pass. |
| T-05-03-03 | Tampering (sign/convention) | mitigate | CLOSED | T7(b) pins `(h_semi − h_opaque) == T_native` to 1e-9 at fully-engaged bands (`opaque_regression`), plus deep-shadow rise; conj gate stays 0. |
| T-05-03-04 | Spoofing (contradictory input) | mitigate | CLOSED | `isolation.is_some() && class==Flat ⇒ Err(IsolationWithoutScreen)` (`terrain_effect/mod.rs:231-232`; variant `propagation/mod.rs:239`). T9 (`IsolationWithoutScreen` matched 3× in test) passes; isolation+weather+screen documented unreachable via existing `WeatherScreenNotImplemented`. |
| T-05-03-05 | DoS (signature fallout) | mitigate | CLOSED | Compiler-enforced exhaustive update — every `SolveJob` literal carries `forest:None`/`isolation:None`, every `terrain_effect(` call gains trailing `None` (no `Default` shortcut). `cargo test --test` suites build+pass across engine+harness. |
| T-05-SC | Tampering (supply chain) | accept | LOGGED | See Accepted Risks. |

## Accepted Risks

| Risk ID | Disposition | Rationale | Evidence (verified) |
|---------|-------------|-----------|---------------------|
| T-05-SC (05-01/05-02/05-03) | accept | No new packages introduced this phase. Engine dependency quarantine (`ndarray + num-complex + thiserror`) is the design constraint; the 208-pt DFT and the Fritsch–Carlson–Butland PCHIP are hand-rolled specifically to avoid FFT/linalg crates. Python oracle tooling is dev-time only (not a build/test dependency; committed TOML fixtures used at test time). | `cargo tree -p envi-engine -e normal --depth 1` extra-dep count = **0**; `crates/envi-engine/Cargo.toml` unchanged since Phase 01 (last touch commit `256d7be`); `#![deny(unsafe_code)]` intact (`lib.rs`). |

## Cross-cutting engine invariants (re-verified this audit)

- Single-`.conj()` quarantine: `grep -rn '\.conj(' crates/envi-engine/src/propagation/ | grep -v '//'` = **0**.
- Dependency quarantine: extra deps beyond `ndarray|num-complex|thiserror` = **0**.
- `#![deny(unsafe_code)]` present on `envi-engine`.
- Both opaque/None paths pinned bit-for-bit (`solve_baseline`, `opaque_regression`) — pass.

## Unregistered Flags

None. Neither 05-01/05-02/05-03 SUMMARY declares a `## Threat Flags` section, and no new
attack surface appeared during implementation that lacks a threat mapping. The SUMMARY
"deviations" (T3 window reformulation in 05-02, T8 exact-zero→bit-identical correction in
05-03) are test-spec corrections, and the WR-01 `MAX_R_DB` cap is a *strengthening* of the
already-declared T-05-02-01 mitigation (verified present) — none introduce new surface.

## Result

**SECURED.** All 15 `mitigate` threats verified present in code and pinned by passing
tests; all 3 `accept` instances (T-05-SC) logged with verified evidence. No open threats,
no blockers, `threats_open = 0`.
