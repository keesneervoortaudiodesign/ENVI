---
phase: 02-ground-effect-diffraction
verified: 2026-07-08T10:18:22Z
status: passed
score: 8/8 must-haves verified
behavior_unverified: 0
overrides_applied: 0
re_verification:
  previous_status: none
  previous_score: none
  gaps_closed: []
  gaps_remaining: []
  regressions: []
deferred:
  - truth: "FORCE road-traffic cases (1-8, 21/24, 71-94) reproduce reference spectra end-to-end"
    addressed_in: "Phase 4"
    evidence: "ROADMAP Phase 4 requirements list VAL-02 ('Engine reproduces the FORCE test-case reference results within the standard's tolerance'); 02-RESEARCH 'FORCE Validation Reality' documents that every FORCE reference value embeds the Jonasson road-emission/pass-by model, which is Phase 4 (SRC-02/03/04, OUT-01..06). This is explicitly called out in the ROADMAP Phase 2 note ('planned 2026-07-07') and in 02-05-PLAN's scope note — not a silent gap."
  - truth: "Sub-model 3 (non-flat terrain, §5.12) computes a real ΔL for elevated-road/valley/forest profiles"
    addressed_in: "Phase 3"
    evidence: "ROADMAP Phase 2 note: 'Sub-model 3 (non-flat terrain, §5.12) is explicitly deferred to Phase 3 (typed-error stub; flat Phase 2 targets give r_flat = 1)'. Confirmed as PropagationError::NonFlatTerrainNotImplemented, exercised by exactly 25 of the 62 FORCE straight-road profiles in the finiteness sweep, never producing NaN."
  - truth: "FΔν (fluctuating-refraction Ff/FΔν split) is computed from actual A⁺ ray tracing rather than stubbed at 1"
    addressed_in: "Phase 3"
    evidence: "coherence.rs doc: 'Phase 2 callers pass 1.0; Phase 3 drops in Δτ⁺ under the A⁺ profile with no call-site change'. ROADMAP Phase 3 requirements include ENG-05/ENG-08 (refraction, turbulence coherence F_τ)."
---

# Phase 2: Ground Effect & Diffraction Verification Report

**Phase Goal:** Homogeneous-atmosphere ground + screen behavior: ground reflection over segmented impedance and single/multi-edge diffraction combine as complex pressure with the Δτ interference phase intact; combined results stay finite/stable across the full 1/12-octave range.
**Verified:** 2026-07-08T10:18:22Z
**Status:** passed
**Re-verification:** No — initial verification

## Method

Ran the shipped code directly (not just read SUMMARY claims):

```
cargo test --workspace                              → 173 passed, 0 failed, 65 ignored (capability-gated)
cargo clippy --all-targets -- -D warnings            → clean, 0 warnings
cargo fmt --check                                    → clean
cargo run -p envi-harness -- report                  → 9 Pass rows (4 Phase-1 + 5 terrain); 62 FORCE road cases Skipped
cargo tree -p envi-engine -e normal --depth 1         → ndarray, num-complex, thiserror only (dep quarantine holds)
```

Then read the actual source for every requirement, the two-channel contract, and the conj() quarantine, and cross-checked every numeric anchor named in the task against a real, running assertion in the test suite (not just a comment).

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Flat soft/hard/mixed (segmented-impedance) ground gives correct dip frequencies with complex pressure preserved (not band energy) | ✓ VERIFIED | `submodel1.rs::delta_l_reproduces_research_anchor_table` matches the 7-point anchor table (100 Hz–4 kHz) to ±0.05 dB; `submodel1.rs::deepest_dip_lands_on_predicted_grid_band` finds the dip at grid point 630.96/667.42 Hz (continuous anchor 646.7 Hz) with depth −19.16 dB ± 0.3; `submodel2.rs` implements segmented-impedance blending (Sub-model 2, Eqs. 124-133) exercised by `oracle_flat.rs::engine_submodel2_matches_flat_mixed_case21_oracle` (Pass) and the `terrain_mixed_case21` e2e case (Pass at 0.1 dB) |
| 2 | Single-edge and multiple-edge screens behave correctly | ✓ VERIFIED | `diffraction.rs` implements Hadden–Pierce `pwedge`/`p2wedge`/`p2edge`/`Dwedge` (Eqs. 78-107); `oracle_wedge.rs` (5 tests) matches an independent scipy `wofz`-based oracle for hard-screen IL, shadow boundary, finite-impedance faces, two-wedge (p2wedge) and thick-screen (p2edge) composition; `terrain_effect/screen.rs` Sub-models 4/5/6 (single-edge/thick/double) selected by `ScreenClass`; the 5 committed e2e cases include thin (case-71), thick (case-81) and double (case-91) screens, all Pass at 0.1 dB against an independently-ported oracle |
| 3 | Direct + ground-reflected + diffracted combine as complex pressures retaining Δτ interference phase, finite/stable across the full 1/12-octave range | ✓ VERIFIED | `terrain_effect/mod.rs::phase_is_live_through_ground_and_screen` proves `H_coh` has `|Im|>0` at ≥90% of bands and `arg(H_coh)≠arg(H_ff)` for both flat and screen geometries; `terrain_effect/mod.rs::dip_lands_on_the_same_grid_point_both_paths` proves the dip lands on the same grid point natively and through the transfer path; `finiteness_sweep_across_all_force_geometries_and_bands` (harness) evaluates ALL 62 FORCE straight-road profiles × 105 bands — 37 flat-evaluated (finite) + 25 non-flat (typed error, never NaN), confirmed by direct run: `finiteness sweep: 37 flat-evaluated + 25 non-flat (typed error)` |

**Score:** 3/3 roadmap success criteria verified (all VERIFIED, none PRESENT_BEHAVIOR_UNVERIFIED)

### Critical Contract: Two-Channel H_coh / P_incoh (user-mandated)

| # | Contract item | Status | Evidence |
|---|----------------|--------|----------|
| 4 | Ground Q̂, wedge diffraction, screen⇄ground composites are Complex<f64>, phase live | ✓ VERIFIED | `ground.rs::spherical_q` returns `Complex<f64>`, never clamps `\|Q̂\|` (test `surface_wave_regime_is_not_clamped`, `\|Q̂\|=1.257` at σ=200/250Hz); `diffraction.rs::pwedge`/`p2wedge`/`p2edge` return `Complex<f64>`; `terrain_effect::GroundResult::h_coh_factor: Complex<f64>` is the composed output of every sub-model (1/2/4/5/6) |
| 5 | `GroundResult`/terrain-effect carries `h_coh_factor: Complex<f64>` + `p_incoh: f64` as separate fields; total level = \|coherent Σ\|² + P_incoh | ✓ VERIFIED | `terrain_effect/mod.rs:66-74` — `struct GroundResult { delta_l_db: f64, h_coh_factor: Complex<f64>, p_incoh: f64 }`; `GroundResult::from_channels` derives `delta_l_db = 10·lg(\|h_coh_factor\|² + p_incoh)`; `TerrainEffect` (the per-band-axis type) carries the same three fields as parallel `Vec`s; `transfer.rs::band_levels_db_two_channel` implements `L = L_W + 10·lg(\|H_coh\|² + \|H_ff\|²·p_incoh)`, tested exactly (`two_channel_adds_incoherent_energy_at_free_field_magnitude`) |
| 6 | F→1 ⇒ P_incoh→0 (tested) | ✓ VERIFIED | `submodel1.rs::two_channel_identity_and_f_one_zeroes_incoherent` line 285-286: `let g1 = eval(1000.0, &rays, 200.0, 0.0, &coh, Some(1.0)).unwrap(); assert_eq!(g1.p_incoh, 0.0, ...)` — bit-exact, not approximate. `eval()`'s `force_f: Option<f64>` hook exists specifically for this test |
| 7 | Exactly ONE conj() call in the engine, in transfer.rs; propagation/ matches are comments only | ✓ VERIFIED | `grep -rn '\.conj()' crates/envi-engine/src/propagation/` → 2 hits, BOTH inside `//` comment lines in `special.rs` (documenting that Faddeeva symmetry math uses explicit `Complex::new(re,-im)` instead, "the propagation-module conj quarantine"); filtered grep (excluding comment lines) → 0 hits. `transfer.rs` has exactly one production `.conj()` call: `pub fn nord_ratio_to_transfer(ratio: Complex<f64>) -> Complex<f64> { ratio.conj() }` (line 91); the only other `.conj()` occurrence in transfer.rs is inside a `#[cfg(test)]` assertion comparing against the production call, not a second engine call site |

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/envi-engine/src/propagation/terrain_interpretation.rs` | §5.21 edge/screen ID, r_scr1/r_scr2/r_scr12/r_flat, equivalent flat terrain | ✓ VERIFIED | 400+ lines; `ScreenClass`, `TransitionParams`, `interpret_terrain()`; ramp breakpoints (0.133λ, [0.1,0.3], [0.026,0.082]) present with citations; `CONVEX_EPS=0.0001`, `FLATNESS_FREEZE_HZ=2000.0` constants match research |
| `crates/envi-engine/src/propagation/terrain_effect/mod.rs` | `terrain_effect()` entry point, Eq. 332 composition, two-channel `TerrainEffect` | ✓ VERIFIED | `terrain_effect()` dispatches flat/screen branches, composes via `r_scr1` blend, returns `TerrainEffect{h_coh_factor, p_incoh, delta_l_db}` |
| `crates/envi-engine/src/transfer.rs` | `nord_ratio_to_transfer` (single conj boundary) + `band_levels_db_two_channel` readout | ✓ VERIFIED | Both functions present, doc-commented, unit-tested |
| `cases/terrain_screen_thin_case71.toml` (+ 4 siblings) | oracle-pinned end-to-end screen case | ✓ VERIFIED | All 5 files present, `provenance = "oracle sha256:..."`, `tolerance_db = 0.1`, generated by `tools/nord2000_oracle/gen_case_fixtures.py` |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| `terrain_effect/mod.rs` | `terrain_interpretation.rs` | dispatch on `r_scr1`/`r_scr2`/`r_scr12`/`r_flat` | ✓ WIRED | `terrain_effect()` calls `interpret_terrain()` then `interp.transition_params(f)` per band |
| `transfer.rs` | `terrain_effect/mod.rs` | `H_coh[f] = H_ff[f]·nord_ratio_to_transfer(h_coh_factor[f])` | ✓ WIRED | Exercised in `terrain_effect_tests::h_coh()` helper and in `flat_channel_reproduces_submodel1_through_the_transfer_path` (1e-9 dB identity) |
| `envi-harness::lib.rs` (`run_case`) | `propagation::terrain_effect` | `Scene → terrain_effect → band_levels_db_two_channel → comparison` | ✓ WIRED | `crates/envi-harness/tests/terrain.rs::five_terrain_cases_pass_against_the_oracle` drives the real `run_case()` path (file → Scene → engine → readout → comparison), not a shortcut — confirmed by direct `cargo test` run: `test five_terrain_cases_pass_against_the_oracle ... ok` |

### Behavioral Spot-Checks (executed directly, not from SUMMARY)

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Full workspace test suite | `cargo test --workspace` | 173 passed, 0 failed, 65 ignored (all `[requires: emission-model(, refraction)]`) | ✓ PASS |
| Clippy strict | `cargo clippy --all-targets -- -D warnings` | 0 warnings | ✓ PASS |
| Format | `cargo fmt --check` | clean, exit 0 | ✓ PASS |
| Harness report | `cargo run -p envi-harness -- report` | 9 Pass rows (4 free-field/geometry + 5 terrain); 62 FORCE road rows Skipped `requires: emission-model[, refraction]` | ✓ PASS |
| conj() quarantine | `grep -rn '\.conj()' propagation/` vs `grep -c conj transfer.rs` | 2 hits in propagation/, both comment-only; 1 production call in transfer.rs | ✓ PASS |
| Finiteness sweep | `cargo test -p envi-harness --test terrain finiteness_sweep -- --nocapture` | `finiteness sweep: 37 flat-evaluated + 25 non-flat (typed error)` = 62/62 FORCE straight-road profiles covered, 0 NaN | ✓ PASS |
| Dependency quarantine | `cargo tree -p envi-engine -e normal --depth 1` | ndarray, num-complex, thiserror only | ✓ PASS |
| `#![deny(unsafe_code)]` | `grep` in `crates/envi-engine/src/lib.rs` | present (line 17) | ✓ PASS |

### Requirements Coverage

| Requirement | Description | Status | Evidence |
|-------------|-------------|--------|----------|
| ENG-02 | Ground effect over segmented-impedance profile, preserving complex pressure | ✓ SATISFIED | `ground.rs` (Ẑ_G/Γ̂_p/Ê/Q̂/ρᵢ), `submodel1.rs`/`submodel2.rs`, all anchor tests pass, `\|Q̂\|` never clamped |
| ENG-03 | Screen/barrier diffraction, single and multiple edges | ✓ SATISFIED | `diffraction.rs` (Hadden-Pierce), `terrain_effect/screen.rs` (SM4/5/6), 5 oracle-pinned e2e cases incl. thin/thick/double screens |
| ENG-07 | Combine direct+reflected+diffracted as complex pressure, retaining Δτ phase | ✓ SATISFIED | Two-channel `GroundResult`/`TerrainEffect`, single conj boundary at `transfer.rs`, phase-liveness test, dip-through-both-paths test |

No orphaned requirements — REQUIREMENTS.md maps exactly ENG-02/03/07 to Phase 2, all three covered.

### Anti-Patterns Found

None. Searched all files touched by plan 02-05 (`terrain_interpretation.rs`, `terrain_effect/mod.rs`, `special.rs`, `transfer.rs`, harness capability/cases/lib/main/scene_build) for `TBD`/`FIXME`/`XXX`/`TODO`/`HACK`/`PLACEHOLDER`/"coming soon"/"not yet implemented" — zero matches. No stub returns (`return null`/empty-array patterns) found in the engine propagation modules; every sub-model performs real arithmetic on real inputs.

### Documented Deferrals (verified explicit, not silent)

1. **FORCE road-case reference-spectrum matching** — ROADMAP Phase 2 explicitly reframes success criteria 1-2 with a dated note ("planned 2026-07-07") explaining that FORCE reference spectra require the Phase 4 Jonasson emission model (VAL-02 traceability already maps to Phase 4 in REQUIREMENTS.md, predating this phase). The harness still runs all 62 straight-road cases and reports `Skipped requires: emission-model[, refraction]` for each — not silently dropped, not marked Pass.
2. **Sub-model 3 (non-flat terrain, §5.12)** — explicit typed error `PropagationError::NonFlatTerrainNotImplemented`, documented in ROADMAP Phase 2 note and 02-05-PLAN scope note, exercised by 25/62 FORCE profiles in the finiteness sweep (confirmed: never NaN, always the typed error).
3. **FΔν = 1 stub** — documented in `coherence.rs` module doc and 02-05-SUMMARY "Phase 3 handoff notes"; not hidden, and the oracle fixtures use the same stubbed value so cross-implementation comparisons stay self-consistent (no false precision).
4. **Fractional r_scr2/r_scr12 channel blend** — documented as a Phase 3 revisit seam in `terrain_effect/mod.rs` doc comment; confirmed every Phase 2 target profile only exercises `r ∈ {0,1}` for these two parameters (only `r_scr1` transitions fractionally, and the linear-channel blend is proven correct there via the dip-identity tests).

### Administrative Note (non-blocking)

`.planning/ROADMAP.md`'s "Plans" line under Phase 2 still reads "4/5 plans executed" and the Progress table still shows "4/5 / In Progress" for Phase 2, and the `- [ ] 02-05-PLAN.md` bullet under Phase 2's Plans list is unchecked — even though the phase-level checkbox (`- [x] **Phase 2...**`) and 02-05-SUMMARY.md (`status: complete`) both confirm the phase is done. The most recent commit (`f679765`) updated the phase checkbox but missed the plan-count line, progress table, and the individual plan bullet. This is a documentation-bookkeeping gap only — it does not affect any shipped code or test — but should be fixed before starting Phase 3 so the roadmap accurately reflects state.

## Gaps Summary

No gaps against the phase goal or ENG-02/03/07. All three ROADMAP success criteria are independently verified by running (not reading about) the test suite, the harness report, and dedicated grep/inspection of the conj() quarantine and two-channel contract. The one item noted above is a stale-roadmap-bookkeeping issue, not a code or goal gap, and is recorded as a WARNING-level administrative note rather than a gap.

## Verdict

**PASSED.** Phase 2's goal — homogeneous-atmosphere ground and screen diffraction combining as complex pressure with Δτ phase intact, finite/stable across the full 1/12-octave range — is genuinely delivered in the shipped code, verified by running `cargo test --workspace` (173/173 pass), `cargo clippy -D warnings` (clean), `cargo fmt --check` (clean), `cargo run -p envi-harness -- report` (9 Pass, 62 honestly Skipped), and direct source inspection of the two-channel `H_coh`/`P_incoh` contract and the single-conj() quarantine (both hold exactly as mandated). ENG-02, ENG-03, ENG-07 are satisfied. Deferrals (FORCE end-to-end gating, Sub-model 3, FΔν seam) are explicit and documented, not silent scope cuts. Recommend fixing the stale ROADMAP plan-count/progress-table entries before proceeding to Phase 3, but this does not block phase acceptance.

---
*Verified: 2026-07-08T10:18:22Z*
*Verifier: Claude (gsd-verifier)*
