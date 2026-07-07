---
phase: 02-ground-effect-diffraction
plan: 02
subsystem: engine
tags: [rust, nord2000, fresnel-zones, sub-model-1, sub-model-2, ground-dip, two-channel, phase-preserving, partial-coherence, phase-diff-freq, segmented-impedance, scipy-oracle, convention-quarantine, tdd]

# Dependency graph
requires:
  - "02-01: propagation::ground (ground_impedance→Result, spherical_q, incoherent_rho), propagation::rays (RayPair, straight_rays), propagation::coherence (CoherenceInputs, coherence_f/coherence_ff), propagation::special (exp_clamped)"
  - "01-02: scene::impedance_class; 01-03: propagation::sound_speed_ms, PropagationError"
provides:
  - "envi-engine::propagation::fresnel — calc_fz_d (Eqs.338-339), fresnel_zone_size (340-344), fresnel_zone_w (345-351), fresnel_zone_wm (352-353); F_λ·λ is a caller parameter"
  - "envi-engine::propagation::terrain_effect::GroundResult — the two-channel type {delta_l_db, h_coh_factor: Complex<f64>, p_incoh: f64} + from_channels()"
  - "envi-engine::propagation::terrain_effect::submodel1 — SubModel1Inputs + submodel1() (Eq.120); pub(crate) eval() with F-override hook (Sub-model 2 + test reuse)"
  - "envi-engine::propagation::terrain_effect::submodel2 — FlatGeometry, SurfaceStrip, submodel2() (Eqs.124-133); pub(crate) group_types/phase_diff_freq/type_weights"
  - "tools/nord2000_oracle — flat_models.py (independent scipy Sub-model 1/2) + gen_flat_fixtures.py + flat_sigma200.toml / flat_mixed_case21.toml (105-pt ΔL_t curves)"
affects: [02-04-screens, 02-05-terrain-composition, phase-03-refraction]

# Tech tracking
tech-stack:
  added: []
  patterns: [two-channel phase-preserving GroundResult (complex h_coh + real p_incoh, never collapsed), per-TYPE (not per-segment) Fresnel blend, weight normalization to Σ=1 for single-type↔Sub-model-1 consistency, PhaseDiffFreq log-interpolation with 25Hz/100kHz extrapolation edges, committed scipy flat-ground oracle at 0.1 dB, TDD RED/GREEN per task, convention quarantine (no conj in propagation)]

key-files:
  created:
    - crates/envi-engine/src/propagation/terrain_effect/submodel1.rs
    - crates/envi-engine/src/propagation/terrain_effect/submodel2.rs
    - tools/nord2000_oracle/flat_models.py
    - tools/nord2000_oracle/gen_flat_fixtures.py
    - crates/envi-harness/tests/fixtures/oracle/flat_sigma200.toml
    - crates/envi-harness/tests/fixtures/oracle/flat_mixed_case21.toml
    - crates/envi-harness/tests/oracle_flat.rs
  modified:
    - crates/envi-engine/src/propagation/fresnel.rs
    - crates/envi-engine/src/propagation/terrain_effect/mod.rs

key-decisions:
  - "Eq. 128 high-frequency polynomial last coefficient is 3.1, NOT the research-guessed 1.36 — confirmed on PDF p. 57 image: r\"=8.78r̄⁵−21.95r̄⁴+21.76r̄³−10.69r̄²+3.1r̄, which gives P(0)=0, P(1)=1 exactly (an S-curve)"
  - "Eq. 132 Δα_L = π − (1.9483·ln(h_min) + 18.052)·tan ψ_G with h_min = max(min(hS,hR),0.01); the constant is 18.052 not the research-noted 8.052 (PDF p. 57 image)"
  - "Eq. 120 carries a 10·lg prefix (research Assumption A2 RESOLVED on PDF p. 54); coherent term is 1 + F·(R1/R2)·e^{+j2πfΔτ}·Q̂ (Nord-native +j, no conj)"
  - "Eq. 76 ρᵢ matches the existing 02-01 incoherent_rho verbatim (PDF p. 37 image): ᾱ_ri=8(X/m)[1−(X/m)ln((1+X)²+Y²)+((X²−Y²)/(Ym))arctan(Y/(1+X))], ρᵢ=√(1−ᾱ) — Assumption A1 confirmed, no change"
  - "Sub-model 2 two-channel blend: h_coh_factor sums complex-linearly with w′, p_incoh sums with w′² (squared weights); delta_l_db = 10·lg(|Σw′h|²+Σw′²p) — the phase-preserving reading of Eq. 124 (document defines only the level), oracle implements the same"
  - "Per-type weights normalized to Σ=1: required so a single surface type reduces Sub-model 2 to Sub-model 1 exactly at every frequency (§5.8 sum rule); a no-op when the profile already spans the Fresnel zone"
  - "FresnelZoneWm (Eq. 353) implemented as the symmetric 0.5(w_S+w_R) each-side overlap; FresnelZoneW (Eq. 351) as the single zone-coverage fraction — both bounded [0,1] by construction"

requirements-completed: []

# Metrics
duration: 18min
completed: 2026-07-07
status: complete
---

# Phase 2 Plan 02: Flat-Terrain Ground Effect (Sub-models 1 & 2) Summary

**The flat-terrain ground effect landed on the 02-01 numerics core: the Fresnel-zone weight machinery (Eqs. 338–353), Sub-model 1's partially-coherent two-ray sum reproducing the research ΔL dip table to ±0.05 dB, Sub-model 2's per-surface-TYPE segmented-impedance blend with PhaseDiffFreq, and — load-bearing — the two-channel `GroundResult` (`h_coh_factor: Complex<f64>` phase-live + `p_incoh: f64` turbulence-decorrelated energy) that keeps complex pressure separate from incoherent energy all the way through, cross-checked against a committed scipy oracle at ≤ 0.1 dB.**

## Performance

- **Duration:** ~18 min
- **Completed:** 2026-07-07
- **Tasks:** 3 (all TDD: RED test commit → GREEN feat commit)

## Accomplishments

- **fresnel.rs — Fresnel-zone machinery (Task 1).** `calc_fz_d` (Eq. 339 ellipse quadratic), `fresnel_zone_size` (Eqs. 340–344, with `a₁`=CalcFZd at `π−ψ_G`, `a₂` at `ψ_G`, `b` at `π/2`), `fresnel_zone_w` (Eqs. 345–351, zone-coverage fraction) and `fresnel_zone_wm` (Eqs. 352–353, symmetric each-side 0.5(w_S+w_R)). `F_λ·λ` is a caller-supplied parameter (Sub-model 2 passes 0.25·λ; screens/terrain will pass λ/16, 0.5·λ) — never hard-coded. Property tests pin weight bounds ∈ [0,1], zone shrink-with-frequency (and smaller `F_λ`), hS==hR zone centring + S↔R reciprocity, and typed errors on degenerate geometry (F_λ=0, zero distance) — the transposition-catching properties transcription is prone to. All equations transcribed from PDF page images (§5.23.4–7).
- **submodel1.rs + GroundResult two-channel type (Task 2).** `GroundResult { delta_l_db, h_coh_factor: Complex<f64>, p_incoh: f64 }` with module docs spelling out the user-locked contract. `submodel1` (Eq. 120) reproduces the full research ΔL anchor table (+5.2099 @100 Hz … −0.0726 @4 kHz) within ±0.05 dB; the deepest dip lands on grid point 630.96 Hz at −19.12 dB (research 646.7 Hz/−19.16); the dip sits below the computed hard-ground `1/(2Δτ)` (arg Q̂ > 0 pulls it down — the convention pin); the two-channel identity `10·lg(|h_coh|²+p_incoh)==delta_l_db` holds to 1e-12 at every grid point; forcing `F=1` gives `p_incoh == 0.0` bit-exact; soft (class A) attenuates more than hard (class G) over 200–2000 Hz; all outputs finite over σ∈{12.5,200,20000,200000}×geometry×extreme case.
- **submodel2.rs — segmented impedance per surface TYPE (Task 3).** `submodel2` (Eqs. 124–133) groups strips by `(σ, r)` TYPE (Pitfall 3): a `[G,D,G,D]` four-strip profile yields exactly two per-type Sub-model 1 evaluations. Low-frequency `FresnelZoneW` weights + high-frequency `FresnelZoneWm` blended via the Eq. 128 log-polynomial and Eq. 129 log-frequency interpolation between `fL`/`fH` from `phase_diff_freq`. A single-type "segmented" profile collapses to Sub-model 1 at 1e-9 dB (no double-counting). The SM2 two-channel identity holds to 1e-12.
- **PhaseDiffFreq (Eqs. 378–381).** `Ψ(f) = 2πf·ΔR/c₀ + arg Γ̂_p(f,ψ_G,Ẑ_G,min)` bracketed on the 1/3-octave grid and log-interpolated (Eq. 380), with both extrapolation edges exact (linear 8–10 kHz → 100 kHz cap above; `f=25·Ψ/Ψ(25 Hz)` below 25 Hz) and the `fL ≤ 0.8·fH` clamp. Never NaN across the extreme-geometry sweep; monotone (larger target → higher f).
- **Committed scipy flat-ground oracle (Task 3).** `tools/nord2000_oracle/flat_models.py` is an independent `scipy.special.wofz`-based Sub-model 1/2 (equation citations only), generating `flat_sigma200.toml` (uniform σ=200, zero turbulence) and `flat_mixed_case21.toml` (FORCE case-21 road σ=20000 / grass σ=200 alternating, `Cv²=0.12`, `CT²=0.008`, FΔν=1). `envi-harness::tests/oracle_flat.rs` cross-checks engine Sub-model 1/2 vs both 105-point curves within 0.1 dB. Python/scipy are NOT build dependencies.
- **All quality gates pass:** `cargo build --workspace`, `cargo test --workspace` (68 engine + 31 harness unit + 5 force + 2 oracle_ground + 2 oracle_flat, 0 failed), `cargo clippy --all-targets -- -D warnings` (zero), `cargo fmt --check` (clean), `#![deny(unsafe_code)]` holds, `cargo tree -p envi-engine` unchanged (ndarray/num-complex/thiserror only).

## Task Commits

1. **Task 1: fresnel.rs** — RED `test(02-02)` (bounds/shrink/reciprocity/degenerate) → GREEN `feat(02-02)` (Eqs. 338–353) → `style(02-02)` (rustfmt)
2. **Task 2: GroundResult + submodel1.rs** — RED `test(02-02)` (anchor table, dip band, asymmetry, two-channel identity, F=1⇒0, soft<hard, finiteness) → GREEN `feat(02-02)` (Eq. 120)
3. **Task 3: submodel2.rs + oracle** — RED `test(02-02)` (per-TYPE grouping, single-type collapse, PhaseDiffFreq guards, weight bounds, SM2 identity) → GREEN `feat(02-02)` (Eqs. 124–133 + flat_models.py oracle + fixtures + harness test)

## Recorded findings (per plan `<output>`)

- **Eq. 120 prefix (Assumption A2 RESOLVED):** the `10·lg` prefix is confirmed on the PDF p. 54 image; the coherent term is `1 + F·(R₁/R₂)·e^{+j2πfΔτ}·Q̂` (Nord-native `+j`, no conj). Noted in the `submodel1.rs` module doc.
- **Eq. 128 constants (Assumption transcription):** `r"_ii = 8.78·r̄⁵ − 21.95·r̄⁴ + 21.76·r̄³ − 10.69·r̄² + 3.1·r̄` — the last coefficient is **3.1**, not the research-guessed 1.36. Verified on PDF p. 57: the coefficients sum to `8.78−21.95+21.76−10.69+3.1 = 1.0`, giving `P(0)=0`, `P(1)=1` (a normalized S-curve on [0,1]). The `r_h` grazing-angle ramp is `log(200·tanψ_G)/log(8)` on `0.005 < tanψ_G < 0.04` (continuous at both breakpoints), and `w_H = (r_{ii,ir} − r'_{ii,ir})·r_h + r'_{ii,ir}`.
- **Eq. 132 Ψ_L:** `Δα_L = π − (1.9483·ln(h_min) + 18.052)·tan ψ_G`, `h_min = max(min(hS,hR), 0.01)`. The constant is **18.052** (research noted 8.052 — corrected from the page image). `fL = PhaseDiffFreq(…, Δα_L)`, `fH = PhaseDiffFreq(…, π)`, clamp `fL ≤ 0.8·fH`.
- **PhaseDiffFreq edge rules (Eqs. 379–381):** `Ψ(f) = 2πf·ΔR/c₀ + arg Γ̂_p` with `ΔR = √(d²+(hS+hR)²) − √(d²+(hS−hR)²)`, `ψ_G = arcsin((hS+hR)/R₂)`, uses the **plane-wave** Γ̂_p (not spherical Q̂) at `Ẑ_G,min` (softest ground). Below 25 Hz: `f = 25·Ψ/Ψ(25 Hz)`. Above 10 kHz: linear extrapolation from the 8 kHz / 10 kHz values, capped at 100 kHz.
- **Two-channel blend rule for Sub-model 2 (for 02-04/02-05 reuse):** the document defines only the level `ΔL₂ = Σ w′·ΔL`. The phase-preserving reading (user contract) blends `h_coh_factor` complex-linearly with `w′` and `p_incoh` with the squared weights `w′²`, then `delta_l_db = 10·lg(|Σ w′·h|² + Σ w′²·p)`. Per-type `w′` are normalized to `Σ=1` so a single surface type reduces Sub-model 2 to Sub-model 1 exactly (and the SM2 two-channel identity holds to 1e-12). The oracle implements the identical blend — screens (02-04) and the Eq. 332 composition (02-05) reuse this exact two-channel shape.
- **Eq. 76 ρᵢ (Assumption A1 confirmed):** the PDF p. 37 image matches the 02-01 `incoherent_rho` verbatim — `ᾱ_ri = 8(X/m)[1 − (X/m)·ln((1+X)²+Y²) + ((X²−Y²)/(Ym))·arctan(Y/(1+X))]`, `ρᵢ = √(1−ᾱ_ri)`. No change needed.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Transcription] Eq. 128 last coefficient 1.36 → 3.1**
- **Found during:** Task 3 (transcribing Eq. 128 from PDF p. 57).
- **Issue:** 02-RESEARCH §8 guessed the 5th-order polynomial constants as "8.78/21.95/21.76/10.69/1.36". The PDF page image shows **3.1**, not 1.36. The correct set sums to exactly 1.0 (P(1)=1, an S-curve), which the research value violates.
- **Fix:** Used 3.1 in both the engine (`type_weights`) and the oracle (`flat_models._poly`).
- **Files:** submodel2.rs, tools/nord2000_oracle/flat_models.py.

**2. [Rule 1 - Transcription] Eq. 132 constant 8.052 → 18.052**
- **Found during:** Task 3 (transcribing Eq. 132 from PDF p. 57).
- **Issue:** 02-RESEARCH §8 noted `ΨL = 1.9483 − 8.052·ln(tan ψ_G)`; the page image reads `Δα_L = π − (1.9483·ln(h_min) + 18.052)·tan ψ_G` (constant 18.052, `ln(h_min)` not `ln(tan ψ_G)`, `tan ψ_G` as the outer multiplier).
- **Fix:** Transcribed the page-image form exactly.
- **Files:** submodel2.rs, flat_models.py.

**3. [Rule 3 - Consistency] Per-type weight normalization to Σ=1**
- **Issue:** Eq. 124 as printed has no normalization divisor, but a single-type flat terrain MUST reduce to Sub-model 1 (the standard's own consistency requirement), and at low frequency the Fresnel zone can spill past the profile so raw weights sum to < 1.
- **Fix:** Normalize per-type `w′` to `Σ=1` (a no-op when the profile spans the zone). Guarantees the single-type↔Sub-model-1 identity at 1e-9 dB and the SM2 two-channel identity; documented in `submodel2.rs` and mirrored in the oracle.

### Interface note

`FlatGeometry { d, h_s, h_r, c0 }` was introduced as the Sub-model 2 geometry carrier (the interface block referenced `&FlatGeometry` without defining it). `submodel1::eval()` is `pub(crate)` with an `Option<f64>` F-override — the test hook for `F=1` and the per-type reuse path for Sub-model 2.

## Known Stubs

- **`terrain_effect/submodel2.rs` roughness path:** `Fr` (roughness coherence) rides through `CoherenceInputs`; all Phase 2 target strips have `r=0` so `Fr=1`. The per-type roughness override in `submodel1::eval` is live for when non-zero roughness arrives.
- **Shadow-zone branch (Eqs. 121–122):** `dSZ = ∞` in the homogeneous engine; the `ShadowZoneShielding` term is a documented Phase 3 seam (needs ξ < 0 refraction). Not a scope cut — the researcher-scoped homogeneous specialization.
- **`FΔν = 1`** (injected via `CoherenceInputs::f_delta_nu`) and **`Fs = 1`** remain 02-01 stubs; both implementations (engine + oracle) use FΔν=1 so the case-21 comparison is self-consistent.

None prevent the plan's goal (flat-terrain ground effect + the two-channel shape); each is a documented forward seam.

## Threat surface

Mitigations from the plan `<threat_model>` implemented as correctness requirements: **T-02-05** (PhaseDiffFreq Eq. 380/381 extrapolation + `fL ≤ 0.8·fH` clamp; typed `DegenerateRayGeometry` on zero-length/degenerate strips and `F_λ=0`; extreme-geometry finiteness sweep — no NaN), **T-02-06** (page-image transcription with equation-citation comments; the per-TYPE grouping counting test; the single-type degeneracy identity vs Sub-model 1; full-curve oracle cross-check at 0.1 dB), **T-02-07** (the `10·lg(|h_coh|²+p_incoh)==delta_l_db` identity test at 1e-12 for both sub-models; `F→1 ⇒ p_incoh==0` bit-exact). **T-02-SC** holds: zero new dependencies. No new network/auth/file surface (oracle is a dev tool generating committed data).

## Verification Evidence

- `cargo build --workspace` — finished, no errors
- `cargo test --workspace` — 68 engine + 31 harness + 5 force + 2 oracle_ground + 2 oracle_flat, 0 failed
- `cargo test -p envi-harness --test oracle_flat` — 2 passed (σ=200 Sub-model 1 + case-21 Sub-model 2, 2×105 points ≤ 0.1 dB)
- `cargo clippy --all-targets -- -D warnings` — zero warnings
- `cargo fmt --check` — clean
- `cargo tree -p envi-engine -e normal --depth 1` — only ndarray, num-complex, thiserror
- ΔL anchors: +5.2099@100 / −18.1225@800 / −0.0726@4000 within ±0.05 dB; deepest dip grid point 630.96 Hz = −19.12 dB; F=1 ⇒ p_incoh = 0.0

## Self-Check: PASSED

All created files exist on disk; all seven task commits (3 RED + 3 GREEN + 1 style) are present in git history; full workspace test/clippy/fmt gates green.
