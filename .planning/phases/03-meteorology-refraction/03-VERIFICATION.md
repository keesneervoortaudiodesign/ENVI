---
phase: 03-meteorology-refraction
verified: 2026-07-08T18:34:15Z
status: passed
score: 5/5 success criteria met (9/9 requirements satisfied)
behavior_unverified: 0
overrides_applied: 0
mode: mvp
evidence:
  test_suite: "cargo test --workspace — 255 passed, 0 failed, 67 ignored (capability-gated FORCE cases)"
  conj_gate: "0 real .conj() calls in propagation/ (5 hits are doc-comments)"
  dep_quarantine: "cargo tree -p envi-engine = ndarray + num-complex + thiserror (unchanged)"
accepted_deferrals:  # Intentional scope boundaries — NOT gaps
  - "Weather-route A/B/C scaling constants are [ASSUMED]/quarantined (developer checkpoint option c); validated by property/oracle only, no FORCE numeric Pass"
  - "Segmented-ground refraction and refraction-over-screen are NotImplemented typed errors (SegmentedRefractionNotImplemented) — deferred to Phase 4"
  - "calc_eq_ssp_ground is built + oracle-tested by band index but not wired into the live segmented eval path (Phase-4 wiring, WR-01)"
  - "FORCE wind/gradient numeric Pass is Phase 4 (needs the road emission model, VAL-02); cases stay Skipped(requires: emission-model) — the honest-green D-03 contract"
---

# Phase 3: Meteorology & Refraction — Verification Report

**Phase Goal:** The refraction machinery — log-lin A/B/C profile collapsed to an equivalent-linear profile with guarded numerics, frequency-dependent ground variant, reflection-path coefficient split, weather-class and similarity-theory input routes, and turbulence coherence — validated on the refraction FORCE cases.
**Verified:** 2026-07-08T18:34:15Z
**Status:** passed
**Re-verification:** No — initial verification

## Verdict

All 5 ROADMAP success criteria are MET in the shipped code, all 9 mapped requirements (ENG-05, ENG-06, ENG-08, MET-01..06) are satisfied with citing code and passing tests, and the full workspace suite is green (255 passed / 0 failed). The intentional scope boundaries the orchestrator flagged (quarantined `[ASSUMED]` weather constants, deferred segmented/over-screen refraction, un-wired `calc_eq_ssp_ground`, Phase-4 FORCE numeric) are present exactly as designed and are NOT counted as failures. No genuine gaps found.

## Goal Achievement — Observable Truths (ROADMAP Success Criteria)

| # | Truth (Success Criterion) | Status | Evidence |
|---|---------------------------|--------|----------|
| 1 | Refraction cases run; homogeneous limit reproduces Phase 2 **exactly**; no singularity blow-up; Δτ cancellation-safe in f64 | ✓ MET | D-02 bit-for-bit anchor `assert_eq!` — `rays.rs:404 circular_rays_reproduce_straight_rays_below_xi_clamp`; `terrain_effect/mod.rs:784 refraction_homogeneous_profile_is_bit_identical`; Δτ cancellation via `flat_delta_r` identity (`circular_ray.rs:302`) + `rays.rs:429 circular_dtau_finite_at_long_range`; finiteness sweep `terrain.rs finiteness_sweep_across_all_force_geometries_and_bands`. FORCE numeric deferred to Phase 4 by design (D-01/D-03). |
| 2 | `c(z)=A·ln(z/z₀+1)+B·z+C` with z₀ clamp ≥0.001 m; A per azimuth (temp once + projected wind); inversion ⇒ B>0 | ✓ MET | `profile.rs:21 sound_speed_profile` + `Z0_MIN_M`, tests `z0_below_floor_clamps_to_0_001`; `weather/mod.rs:117 profile_for_bearing` = `a_temp + a_wind·cos(bearing−φ_u)` (tests `downwind`/`upwind` A split); `route2.rs:130 b = b_scale·dtdz` with `route2.rs:229 weather_route2_inversion_gives_positive_b`. |
| 3 | CalcEqSSP collapse (∂c/∂z averaged hS..hR, Annex F closed form); CalcEqSSPGround fL/fH log-interp ⇒ frequency-dependent ξ at 1/12-oct points | ✓ MET | `eqssp.rs:60 calc_eq_ssp` (Annex F `antideriv_log`, no quadrature) — oracle `engine_calc_eq_ssp_matches_oracle_grid` (tol 1e-9); `eqssp.rs:213 calc_eq_ssp_ground` soft/hard branch, log-interp Eq.23 — oracle `engine_calc_eq_ssp_ground_matches_oracle_by_band_index`, tests `soft_ground_xi_varies_monotonically_by_band_index` / `hard_ground_is_frequency_independent`. (Live-pipeline application of the ground variant deferred to Phase 4 — see Deferrals.) |
| 4 | Reflection before/after A₁/B₁,A₂/B₂; Route 1 class table ⇒ energy-weighted L_den; Route 3 Monin–Obukhov + 3×3 LSQ | ✓ MET | `weather/mod.rs:141 ReflectionProfiles` + `route2.rs:196 route2_reflection` (tests A₁≠A₂/B₁=B₂, `reflect_over_segment` cross-check); `route1.rs:109 energy_weighted_level = 10·lg(Σ p·10^(L/10))` + `route1.rs:173 l_den`; `route3.rs:95 reconstruct_profiles` (MO) + `route3.rs:199 fit_profile` (hand-rolled 3×3 Cramer, `solve_3x3`) — oracle `engine_route3_fit_recovers_oracle_coefficients` round-trip. |
| 5 | F_τ blends coherent/partial-coherent, changing results in the expected direction on turbulent cases | ✓ MET | `coherence.rs:103 coherence_f_delta_nu` = `sinc_cutoff(2π·f·(Δτ⁺−Δτ))` (2π not 0.23π); wired via `terrain_effect/mod.rs:362` through `CoherenceInputs::f_delta_nu` (multiplied, never overwrites +j phase); Δτ⁺ from A⁺=A+1.7·sA/B⁺=B+1.7·sB (`mod.rs:315-318`); tests `f_delta_nu_zero_fluctuation_is_bit_identical` + `f_delta_nu_fluctuation_fills_the_dip`. |

**Score:** 5/5 success criteria met (0 behavior-unverified).

## Requirements Coverage

| Requirement | Description | Status | Evidence |
|-------------|-------------|--------|----------|
| ENG-05 | Refraction via equivalent-linear profile (circular-ray ξ, Δτ) with guarded numerics | ✓ SATISFIED | `eqssp.rs`, `circular_ray.rs`, `rays::circular_rays`; ξ/Δz/dSZ clamps; oracle + D-02 anchor |
| ENG-06 | Reflection paths with separate before/after coefficients (A₁/B₁, A₂/B₂) | ✓ SATISFIED | `weather::ReflectionProfiles` + `sub_path_collapses` per-leg `calc_eq_ssp` |
| ENG-08 | Fluctuating-refraction coherence F_τ (C_v², C_T²) blends coherent/partial | ✓ SATISFIED | `coherence_f_delta_nu` (Eq.112) via the f_delta_nu seam; F→1 bit-exact off-turbulence |
| MET-01 | Log-lin `c(z)` with z₀ clamp ≥0.001 m | ✓ SATISFIED | `profile.rs sound_speed_profile` + `Z0_MIN_M` |
| MET-02 | A per azimuth (wind u·cosφ), B from temperature/stability (inversion→B>0), temp once + projected wind | ✓ SATISFIED | `weather::profile_for_bearing` + `route2` |
| MET-03 | CalcEqSSP collapse averaging ∂c/∂z between hS and hR | ✓ SATISFIED | `calc_eq_ssp` (Annex F Eq.403 closed form); oracle 1e-9 |
| MET-04 | Frequency-dependent ground variant (CalcEqSSPGround) fL/fH, integrated with 1/12-oct | ✓ SATISFIED | `calc_eq_ssp_ground`; oracle by band index (live segmented wiring deferred — Phase 4) |
| MET-05 | Route 1 weather-class input → L_den energy-weighted combination | ✓ SATISFIED | `route1::energy_weighted_level` / `energy_weighted_over_classes` / `l_den` |
| MET-06 | Route 3 Monin–Obukhov reconstruction + LSQ fit A,B,C | ✓ SATISFIED | `route3::reconstruct_profiles` + `fit_profile` (3×3 normal equations) |

## Key Link Verification

| From | To | Via | Status |
|------|----|----|--------|
| `rays::circular_rays` | `rays::straight_rays` | `xi.abs() < XI_HOMOGENEOUS (1e-6)` delegation (D-02) | ✓ WIRED (`rays.rs:272`) |
| `terrain_effect::from_profile/eval` | `rays::circular_rays` | swaps straight→circular when refraction active | ✓ WIRED (`mod.rs:310,347`) |
| `terrain_effect::eval` | `coherence::coherence_f_delta_nu` | Δτ⁺ → `CoherenceInputs::f_delta_nu` (no call-site change) | ✓ WIRED (`mod.rs:362`) |
| `submodel1::eval` (shadow) | `refraction::shadow_zone::shadow_zone_shielding` | Eq.121 branch subtracts L_SZ via `pwedge0` | ✓ WIRED (`mod.rs:351`) |
| `shadow_zone` | `diffraction::pwedge0` | equivalent-wedge reuse (D-09, no new primitive) | ✓ WIRED (`shadow_zone.rs:113`) |
| `calc_eq_ssp_ground` | live eval path | (segmented refraction) | ⚠️ NOT WIRED — accepted deferral (Phase 4, WR-01); function is oracle-tested standalone |
| `weather::route1/2/3` | `run_case` pipeline | end-to-end FORCE run | ⚠️ NOT WIRED — accepted deferral (needs emission model, Phase 4); routes unit/oracle-tested |

## Behavioral / Suite Evidence

| Check | Command | Result | Status |
|-------|---------|--------|--------|
| Full workspace suite | `cargo test --workspace` | 255 passed, 0 failed, 67 ignored | ✓ PASS |
| Refraction oracles | `oracle_refraction.rs` (4 tests) | calc_eq_ssp, circular Δτ, route3 fit, calc_eq_ssp_ground-by-band | ✓ PASS |
| FORCE dynamic runner | `force_cases` | 10 passed / 67 ignored (capability-gated, never false Pass) | ✓ PASS |
| `.conj()` quarantine | grep `\.conj\(\)` in `propagation/` | 0 real calls (5 doc-comment mentions) | ✓ PASS |
| Engine dep quarantine | `cargo tree -p envi-engine --depth 1` | ndarray + num-complex + thiserror (approx dev-only) | ✓ PASS |

## Prohibitions (must-NOT checks)

| Prohibition | Status | Evidence |
|-------------|--------|----------|
| No `.conj()` in propagation/ | ✓ HELD | grep gate = 0 real calls |
| No new engine dependency | ✓ HELD | cargo tree unchanged |
| No `nalgebra`/linalg for Route 3 | ✓ HELD | hand-rolled 3×3 Cramer `solve_3x3` (`route3.rs:157`) |
| No false FORCE numeric Pass (honest-green D-03) | ✓ HELD | 67 cases Skipped; `capability.rs:230` requires-shrink test asserts missing = {EmissionModel} only |
| Compare by BAND INDEX not nominal frequency | ✓ HELD | `calc_eq_ssp_ground` oracle + `soft_ground_xi_varies_monotonically_by_band_index` iterate `FreqAxis.centres` |
| No 0.23π in F_τ (2π argument) | ✓ HELD | `coherence.rs:107` uses `TAU`; `coherence_ff` keeps 0.23π separately |

## Accepted Deferrals (intentional scope boundaries — NOT gaps)

1. **`[ASSUMED]` weather-route A/B/C constants** — AV 1106/07 does not specify the wind/temperature→A/B conversions; developer resolved the 03-03 Task-1 checkpoint as option (c): proceed quarantined, validated by direction/structure property tests + same-transcription oracle only, no false FORCE numeric Pass. Documented in `route2.rs`/`route3.rs`.
2. **Segmented-ground refraction + refraction-over-screen** — typed `PropagationError::SegmentedRefractionNotImplemented` (`terrain_effect/mod.rs:401`), surfaced honestly rather than silently returning a homogeneous result. Deferred to Phase 4.
3. **`calc_eq_ssp_ground` not wired into the live eval path** — built and oracle-tested by band index; the single-surface path uses frequency-independent `calc_eq_ssp`; the ground variant is the ready building block for the deferred segmented refraction wiring (WR-01).
4. **FORCE wind/gradient numeric acceptance** — requires the Phase-4 road emission model (VAL-02); cases stay `Skipped(requires: emission-model)`. Verified honest-green: refraction dropped from the missing set, emission-model retained.

## Anti-Patterns / Debt Markers

No blocking anti-patterns found. No unreferenced TBD/FIXME/XXX debt markers in the phase's changed files. `NotImplemented` typed errors are intentional, documented deferral gates (not stubs returning silent wrong answers). Numeric guards (ξ/Δz/dSZ clamps, z₀ floor, cubic/LSQ determinant guards) are present and correctness-critical per the CLAUDE.md numerics house rules.

## Gaps Summary

None. Every ROADMAP success criterion is delivered in shipped, tested code; the only un-wired artifacts (`calc_eq_ssp_ground` live application, end-to-end weather-route case wiring) are explicit, documented Phase-4 deferrals within this MVP phase's honest-green contract, not omissions of the phase goal.

---
_Verified: 2026-07-08T18:34:15Z_
_Verifier: Claude (gsd-verifier)_
