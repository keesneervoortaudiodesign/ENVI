---
phase: 05-engine-extensions-forest-semi-transparent-partitions
verified: 2026-07-09T14:28:10Z
status: passed
score: 5/5 must-haves verified
behavior_unverified: 0
overrides_applied: 0
deferred:
  - truth: "Numeric FORCE validation of forest (cases 121–124) and semi-transparent partitions"
    addressed_in: "Phase 9 (ForestCrossing geometry extraction) + road-emission blocker (VAL-02)"
    evidence: "D-12 accepted acceptance ladder: no FORCE cases exist for these physics; validation is by analytic anchor + committed oracle + opaque-limit regression. Forest FORCE cases stay Skipped(requires: forest-scattering); no false Pass claimed."
---

# Phase 5: Engine Extensions — Forest & Semi-Transparent Partitions Verification Report

**Phase Goal:** Drawn forests actually attenuate and semi-transparent screens/façades actually transmit — the two new Nord2000-faithful acoustics exist in the engine, phase-preserving under the two-channel contract, and regression-safe against the validated opaque engine.
**Verified:** 2026-07-09T14:28:10Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth (Success Criterion) | Status | Evidence |
|---|---------------------------|--------|----------|
| 1 | Forest crossing of through-length `d` attenuated by the Nord2000 forest law (SM10 §5.19), evaluated at 1/12-octave points, matching an analytic anchor/oracle within tolerance (SC1, ENG-09) | ✓ VERIFIED | `forest.rs` implements SM10 Eqs. 288–291 + Tables 8/9 (`forest_delta_l`/`ForestBands::eval_band`). Wired into `solver.rs::solve_pair` post-conj as a two-channel magnitude factor (`10^{ΔL_s/20}` on H_coh, `10^{ΔL_s/10}` on P_incoh). Tests pass: F1 exact-zero below ka=0.7, F2 on-node hand anchor (<1e-12), F3 −15 dB floor + monotone, F4 20k-sample bounds sweep, and `oracle_forest::engine_forest_delta_l_matches_oracle_grid` (scipy PCHIP oracle, 1e-9 dB). Roadmap "A=d·a(f)" is the NoizCalc paraphrase of exactly this sub-model (per RESEARCH + verification context). |
| 2 | Semi-transparent screen contributes a straight-through transmission path (direction preserved) attenuated by the isolation spectrum, combined as complex pressure with phase intact; two-channel contract holds (F→1 ⇒ P_incoh→0 bit-exact with transmission) (SC2, ENG-10) | ✓ VERIFIED | `transmission.rs` builds native `T(f)=|T|·e^{−jφ_cep}`; threaded `SolveJob.isolation → terrain_effect(8th arg) → screen_channel` where `h_semi = h_opaque + T` (additive complex, single §5.13–5.15 point). Behavioral tests pass: `transmission_raises_deep_shadow_and_equals_t_native_where_engaged` (T7, deep-shadow rise + `h_semi−h_opaque == T_native` to 1e-9), `transmission_never_touches_the_incoherent_channel` (T8, P_incoh bit-identical Some vs None). T joins coherent channel only. |
| 3 | Opaque limit (R→∞ / isolation absent) reproduces the standard opaque-screen result bit-for-bit — permanent regression test (SC3) | ✓ VERIFIED | Structural `None` gating in `screen_channel` (`match transmission { Some(t)=>base+t, None=>base }` — a match, never `+0.0`, so negative-zero bits are preserved). `opaque_regression::opaque_screen_matches_pinned_pre_extension_baseline` passes against a committed pre-extension `opaque_baseline.toml` (to_bits of h_coh_factor re/im, p_incoh, delta_l_db for SingleEdge + ThickScreen). `solve_baseline` pins the `forest: None` / `isolation: None` whole-solver path bit-identical. |
| 4 | Building with per-façade isolation spectra applies the crossed façade's R(f) to the transmission path through that façade (engine mechanism; selection downstream per D-11) (SC4) | ✓ VERIFIED | Engine mechanism present: the identical `SolveJob.isolation: Option<&IsolationSpectrum>` seam carries whichever crossed partition's R(f) it is given; per-façade→R selection is upstream (Phase 7/9) by locked decision D-11. No separate engine code path is required — the SC2 mechanism IS the SC4 mechanism, exercised by the same T6–T9 tests. |
| 5 | ENVI extension (D-06/D-07): isolation implemented as a COMPLEX minimum-phase filter beyond stock ENG-10's real `10^(−R/20)` | ✓ VERIFIED | `transmission.rs` reconstructs `φ_min` via the even-mirror 208-pt real-cepstral fold; `TransmissionFilter::from_isolation`. Tests pass: T1 flat⇒φ≡0 (<1e-12), T2 φ linear in R, T3 first-principles D-08 causal sign/flank/antisymmetry + native negation, T4 Re(Z)=ln\|T\|, T11 MAX_R_DB cap keeps φ finite, and `oracle_minphase::engine_min_phase_matches_oracle_grid` (numpy-fft oracle, 1e-9 rad, all 105 bands, 4 spectra). Flat R ⇒ φ≡0 ⇒ bit-compatible with pure attenuation. |

**Score:** 5/5 truths verified (0 present-but-behavior-unverified)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/envi-engine/src/forest.rs` | SM10 excess attenuation (ForestCrossing, forest_delta_l, Tables 8/9, PCHIP) | ✓ VERIFIED | 668 lines. Full SM10 law + validating constructor + Fritsch–Carlson–Butland PCHIP + F1–F4/constructor tests. Cites Eqs. 289/290/291. No dB-per-metre function; no ISO 10/20/200 m regime branches. |
| `crates/envi-engine/src/solver.rs` | SolveJob.forest + SolveJob.isolation seams, post-conj two-channel forest application | ✓ VERIFIED | `forest: Option<ForestCrossing>` + `isolation: Option<&IsolationSpectrum>` on the promoted solver; `ForestBands` built once/path; applied post-conj alongside directivity (D-04). F6/F7 tests present. |
| `crates/envi-engine/src/propagation/transmission.rs` | IsolationSpectrum, min-phase cepstral kernel, native TransmissionFilter | ✓ VERIFIED | 479 lines. Validated newtype (MAX_R_DB cap), hand-rolled 208-pt DFT/IDFT pair, native negative-sine T(f), T1–T4/T10/T11 tests. |
| `crates/envi-engine/src/propagation/terrain_effect/mod.rs` | isolation threading, one-shot TransmissionFilter precompute, screen_channel add w/ structural None gating, Flat+isolation typed error | ✓ VERIFIED | 8th param `isolation`; `IsolationWithoutScreen` guard; filter built once pre-loop; per-band T by band index; structural Some/None match. |
| `crates/envi-harness/tests/fixtures/regression/solve_baseline.toml` | Pre-extension solver bit baseline | ✓ VERIFIED | Present; consumed by `solve_baseline.rs` (green). |
| `crates/envi-harness/tests/fixtures/regression/opaque_baseline.toml` | Pre-extension opaque screen bit baseline | ✓ VERIFIED | Present; consumed by `opaque_regression.rs` (green). |
| `tools/nord2000_oracle/gen_forest_fixtures.py` / `forest.toml` / `oracle_forest.rs` | scipy forest oracle + committed fixture + test | ✓ VERIFIED | All present; test green (F5). |
| `tools/nord2000_oracle/gen_minphase_fixtures.py` / `minphase.toml` / `oracle_minphase.rs` | numpy min-phase oracle + committed fixture + test | ✓ VERIFIED | All present; test green (T5). |
| `README.md` + module headers + `deferred-items.md` | Doc contract close-out (SM10 forest + min-phase extension + Fs seam) | ✓ VERIFIED | README capability rows + Phase-5 section describe both extensions; module headers carry extension notes; `deferred-items.md` records the Fs (Eq. 288) coherence seam with its `f_delta_nu` landing point. |

### Key Link Verification

| From | To | Via | Status |
|------|-----|-----|--------|
| `solver.rs` | `forest.rs` | `ForestBands::new`/`eval_band` in the band loop (post-conj) | ✓ WIRED |
| `solver.rs` | `terrain_effect/mod.rs` | `job.isolation` passed as 8th arg | ✓ WIRED |
| `terrain_effect/mod.rs` | `transmission.rs` | `TransmissionFilter::from_isolation` built once, per-band T into `screen_channel` | ✓ WIRED |
| `oracle_forest.rs` | `forest.toml` | TOML fixture load by band index | ✓ WIRED |
| `oracle_minphase.rs` | `minphase.toml` | TOML fixture load, φ per band index | ✓ WIRED |
| `opaque_regression.rs` | `opaque_baseline.toml` | to_bits equality vs committed hex | ✓ WIRED |
| `solve_baseline.rs` | `solve_baseline.toml` | to_bits equality vs committed hex | ✓ WIRED |

### Load-Bearing Invariants (project contracts)

| Invariant | Check | Result |
|-----------|-------|--------|
| Single-conj quarantine: 0 `.conj()` in `propagation/` | `grep -rn '\.conj(' … \| grep -v '//' \| wc -l` | ✓ 0 (9 raw hits, all comments) |
| `#![deny(unsafe_code)]` on engine | `lib.rs:17` | ✓ present |
| Dep quarantine (ndarray + num-complex + thiserror only) | `cargo tree -p envi-engine -e normal --depth 1` | ✓ exactly those three |
| F→1 ⇒ P_incoh→0 bit-exact with forest | `solver::forest_preserves_exact_zero_incoherent_channel` (F7) | ✓ pass |
| F→1 ⇒ P_incoh→0 bit-exact with transmission | `opaque_regression::transmission_never_touches_the_incoherent_channel` (T8) | ✓ pass |
| Opaque limit bit-for-bit | `opaque_regression` + `solve_baseline` | ✓ pass |
| No INFINITY / f64::MAX sentinel in `transmission.rs` | source grep | ✓ none (rejected ±∞ built from bit patterns) |
| No ISO 9613-2 10/20/200 m distance-clamp regime | `forest.rs` review | ✓ none — SM10 T-saturation + −15 dB floor only |

### Behavioral Spot-Checks / Test Execution

| Suite | Result | Status |
|-------|--------|--------|
| `envi-engine forest::` | 8 passed | ✓ PASS |
| `envi-engine transmission::` | 6 passed | ✓ PASS |
| `envi-engine solver::` (forest/isolation seam) | 8 passed | ✓ PASS |
| `envi-harness --test oracle_forest` (F5) | 1 passed | ✓ PASS |
| `envi-harness --test oracle_minphase` (T5) | 1 passed | ✓ PASS |
| `envi-harness --test opaque_regression` (T6/T7/T8/T9) | 4 passed, 1 ignored (generator) | ✓ PASS |
| `envi-harness --test solve_baseline` | 1 passed, 1 ignored (generator) | ✓ PASS |

### Requirements Coverage

| Requirement | Source Plan | Status | Evidence |
|-------------|-------------|--------|----------|
| ENG-09 (forest, SM10) | 05-01, 05-03 | ✓ SATISFIED | `forest.rs` + solver seam + F1–F5 ladder. REQUIREMENTS ENG-09 checkbox `[x]`, traceability "Complete". |
| ENG-10 (semi-transparent partitions, min-phase) | 05-02, 05-03 | ✓ SATISFIED (engine mechanism) | `transmission.rs` + `terrain_effect` threading + T5–T9. **Traceability flip pending** — see Doc-Consistency Follow-up. |

### Anti-Patterns Found

None blocking. No debt markers (TBD/FIXME/XXX) introduced in phase files. No stubs — all artifacts substantive and behaviorally exercised.

### Doc-Consistency Follow-up (non-blocking)

`REQUIREMENTS.md` line 67 keeps the ENG-10 checkbox `[ ]` and line 198 the traceability row `| ENG-10 | Phase 5 | Pending |`, while `ROADMAP.md` already marks Phase 5 `[x]` complete. 05-03 deliberately deferred this flip to "phase-completion verification" (now). The engine mechanism for ENG-10 is fully delivered and test-pinned, so this is a documentation-consistency artifact (project gate 5), NOT a goal-achievement gap. Action for close-out: flip ENG-10 to `[x]` / traceability "Complete". The descriptive wording of ENG-10 (min-phase extension) and ENG-09 (SM10 law) is already correct in REQUIREMENTS/README/module headers (truth about the doc *content* is verified).

### Accepted Gap (by design, D-12)

VAL/FORCE contains no forest (cases 121–124 stay `Skipped(requires: forest-scattering)`) or semi-transparent test cases. Validation of these physics is by analytic anchor + committed scipy/numpy oracle + the opaque-limit bit-exact regression — the same oracle+anchor ladder used in Phases 2–3. No false FORCE numeric Pass is claimed (FORCE inventory unchanged: 9 Pass / 102 Skipped). This is the accepted acceptance ladder for physics with no reference cases and is not a defect.

### Gaps Summary

No goal-blocking gaps. All four ROADMAP success criteria plus the D-06/D-07 ENVI complex min-phase extension are implemented, wired into the one promoted solve path, and pinned by passing behavioral tests. Every load-bearing project invariant (single-conj quarantine, two-channel F→1⇒P_incoh→0 bit-exactness, opaque-limit bit-exact regression, dep quarantine, `#![deny(unsafe_code)]`) holds. The only outstanding items are a benign ENG-10 traceability checkbox flip (doc-consistency close-out) and the by-design VAL/FORCE deferral — neither undermines goal achievement.

---

_Verified: 2026-07-09T14:28:10Z_
_Verifier: Claude (gsd-verifier)_
