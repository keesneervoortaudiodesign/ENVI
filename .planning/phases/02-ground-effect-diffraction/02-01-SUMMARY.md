---
phase: 02-ground-effect-diffraction
plan: 01
subsystem: engine
tags: [rust, nord2000, faddeeva, wofz, fresnel, ground-impedance, spherical-q, coherence, rays, cancellation-free, scipy-oracle, convention-quarantine]

# Dependency graph
requires:
  - "01-02: geometry::reflect_over_segment (image-source reflection, grazing angle, r1+r2), scene::impedance_class"
  - "01-03: propagation::sound_speed_ms (Eq.335, c(15°C)=340.348), PropagationError, TransferSpectrum"
provides:
  - "envi-engine::propagation::special — faddeeva_w (Eqs.61-74, three branches + Eq.62 symmetry), fresnel_f/fresnel_g (Eqs.85-86 + Tables 4/5), exp_clamped (Eq.337)"
  - "envi-engine::propagation::ground — ground_impedance (Eq.57, Result<Complex,_>), gamma_p (Eq.59), spherical_q (Eqs.58+60, |Q̂| unclamped), incoherent_rho (Eqs.75-76, Paris)"
  - "envi-engine::propagation::rays — RayVars/RayPair, straight_rays + straight_rays_over_segment (cancellation-free ΔR, Phase-3 circular-ray seam)"
  - "envi-engine::propagation::coherence — CoherenceInputs (with injected f_delta_nu + d_m), coherence_ff (Eq.111), coherence_f (Eq.110 = Ff·FΔν·Fc·Fr·Fs)"
  - "envi-engine::propagation module shells fresnel/diffraction/terrain_effect registered (wave-2 plans own disjoint files)"
  - "tools/nord2000_oracle — committed scipy wofz + Q̂ chain oracle (common.py, gen_ground_fixtures.py) + ground_w_qhat.toml fixtures"
  - "PropagationError extended: InvalidFlowResistivity, DegenerateRayGeometry"
affects: [02-02-flat-ground, 02-03-diffraction, 02-04-screens, 02-05-terrain-composition, phase-03-refraction]

# Tech tracking
tech-stack:
  added: []
  patterns: [convention-quarantined module family (Nord2000-native e^{−jωt}, single conj boundary in 02-05), committed cross-implementation scipy oracle with sha256 provenance, complex-norm relative error for complex anchors, cancellation-free ΔR identity, injected coherence seams (FΔν/Fs stubs) not silent scope reduction, TDD RED/GREEN per task]

key-files:
  created:
    - crates/envi-engine/src/propagation/special.rs
    - crates/envi-engine/src/propagation/ground.rs
    - crates/envi-engine/src/propagation/rays.rs
    - crates/envi-engine/src/propagation/coherence.rs
    - crates/envi-engine/src/propagation/fresnel.rs
    - crates/envi-engine/src/propagation/diffraction.rs
    - crates/envi-engine/src/propagation/terrain_effect/mod.rs
    - tools/nord2000_oracle/common.py
    - tools/nord2000_oracle/gen_ground_fixtures.py
    - crates/envi-harness/tests/oracle_ground.rs
    - crates/envi-harness/tests/fixtures/oracle/ground_w_qhat.toml
  modified:
    - crates/envi-engine/src/propagation/mod.rs
    - crates/envi-engine/src/scene.rs
    - crates/envi-harness/Cargo.toml

key-decisions:
  - "faddeeva_w reduces to Q1 via Eq.62 (w(conj(-z))=conj(w(z)); w(-z)=2e^{-z²}-w(z)) then dispatches two-pole/three-pole/Matta-Reichel; reproduces scipy wofz to <2.6e-6 across the plane (worst case at three-pole near-border)"
  - "ground_impedance returns Result<Complex,_> (NOT the interface block's bare Complex) — σ≤0/non-finite is a typed error at the untrusted-terrain boundary (threat T-02-01, a definition-of-done must_have)"
  - "CoherenceInputs carries an extra d_m (propagation distance) beyond the interface block — Fc Eq.113 is ∝ρ^{5/3}·d; geometry stays caller-injected"
  - "|Q̂| is never clamped: anchor σ=200/f=250 gives |Q̂|=1.2572 (surface-wave regime), pinned by a test"
  - "impedance_class B corrected 31.6→31.5 (Table 2 verified; resolves Phase 1 Assumption A1); all eight classes now verified"

requirements-completed: []

# Metrics
duration: 55min
completed: 2026-07-07
status: complete
---

# Phase 2 Plan 01: Nord2000-Native Numerics Core Summary

**The complex numerics foundation the rest of Phase 2 stands on — the document's own Faddeeva w(ẑ) (Eqs. 61-74), the Fresnel-integral fits f/g, the Ẑ_G→Γ̂_p→Ê→Q̂ ground reflection chain plus incoherent ρᵢ, straight-ray variables with a cancellation-free ΔR, and the coherence coefficient F with an injected FΔν seam — all in Nord2000's quarantined e^{−jωt} convention, anchored to research values and cross-checked against a committed scipy oracle.**

## Performance

- **Duration:** ~55 min
- **Completed:** 2026-07-07
- **Tasks:** 3 (all TDD: RED test commit → GREEN feat commit)

## Accomplishments

- **special.rs — Faddeeva w(ẑ), Fresnel f/g, exp' (Task 1).** `faddeeva_w` implements Eq. 62 quadrant reduction + the two-pole (63) / three-pole (64) / Matta–Reichel series (65-74) branches, reproducing `scipy.special.wofz` to `<2.6e-6` relative everywhere (`<3.2e-7` at the four anchors), continuous across every branch border (`|jump| < 5e-7`), correct in all four quadrants, with an overflow guard on the `2·exp(−ẑ²)` reflection. `fresnel_f`/`fresnel_g` are the full-precision Table 4/5 Horner fits with `x≥5` asymptotes; `exp_clamped` is the Eq. 337 three-branch clamp. Module-level convention doc declares the e^{−jωt} quarantine rule.
- **ground.rs — the reflection-coefficient chain (Task 2).** `ground_impedance` (Delany–Bazley Eq. 57, kPa unit) matches all five Ẑ_G anchors with `Im > 0`; `spherical_q` (Eqs. 58-60 via `special::faddeeva_w`) matches all six research Q̂ anchors within 1e-4 including the hard-ground `+0.797560+0.416202j`; `|Q̂|=1.257` at σ=200/250 Hz is preserved (no clamp); `incoherent_rho` is the Paris random-incidence ρᵢ, bounded in (0,1) for every class A-H. `scene::impedance_class` B corrected 31.6→31.5.
- **Committed scipy oracle + fixture grid (Task 2).** `tools/nord2000_oracle/{common.py, gen_ground_fixtures.py}` is an independent `scipy.special.wofz`-based implementation of the Q̂ chain (equation-number citations only, no PDF text). It generated `ground_w_qhat.toml` (30 w-points across all four quadrants + every branch border, 24 Q̂-points over σ∈{12.5..200000}×f∈{63,250,1000,4000}), committed with a `common.py` sha256 provenance field. `envi-harness::tests/oracle_ground.rs` cross-checks the engine against every fixture point (w within 3e-6, Q̂ within 1e-4). Python/scipy are NOT build dependencies.
- **rays.rs + coherence.rs — the ΔR seam and F composition (Task 3).** `straight_rays` reproduces the geometry anchor (R₁/R₂/ΔR/Δτ/ψ_G to 1e-8) via the cancellation-free `ΔR = 4·hS·hR/(R₁+R₂)`, survives the hS=0.01/d=1000 worst case (identity vs naive within 2.4e-14), and `straight_rays_over_segment` handles sloped segments through the image-point dot-product form with no NaN. `coherence_f = Ff·FΔν·Fc·Fr·Fs` with `Ff` glyph-anchored (0.99993 at the dip), `Fc`/`Fr` structurally transcribed, `FΔν` injected (=1) and `Fs` a documented stub.
- **Wave-2 unblocked.** `fresnel.rs`, `diffraction.rs`, `terrain_effect/mod.rs` shells are registered in `propagation/mod.rs` so plans 02-02/02-03/02-04 own disjoint file sets — no `mod.rs` merge conflicts.
- **All quality gates pass:** `cargo build --workspace`, `cargo test --workspace` (51 engine + 31 harness + 5 force + 2 oracle, 0 failed), `cargo clippy --all-targets -- -D warnings` (zero), `cargo fmt --check` (clean), `#![deny(unsafe_code)]` holds, and `cargo tree -p envi-engine -e normal --depth 1` shows only ndarray/num-complex/thiserror.

## Task Commits

1. **Task 1: special.rs + module shells** — RED `test(02-01)` (anchors, quadrants, borders, fits, clamp; shells) → GREEN `feat(02-01)` (w branches, Fresnel Horner fits, exp' clamp)
2. **Task 2: ground.rs + scipy oracle** — RED `test(02-01)` (Ẑ_G/Q̂/ρᵢ anchors, σ-error, scene B=31.5, oracle fixtures) → GREEN `feat(02-01)` (Delany-Bazley chain, Paris ρᵢ, class B fix, oracle tolerance)
3. **Task 3: rays.rs + coherence.rs** — RED `test(02-01)` (ΔR anchors, cancellation regression, F composition) → GREEN `feat(02-01)` (cancellation-free ΔR, Fc/Fr/Fs + injected FΔν)

## Recorded findings (per plan `<output>`)

- **ρᵢ radical (Assumption A1):** implemented as `ρᵢ = √(1 − ᾱ_ri)` with `ᾱ_ri` the canonical Paris random-incidence absorption coefficient in X=Re Ẑ_G, Y=Im Ẑ_G. The PDF page image was not accessible at execution; the standard Paris closed form was used and verified to reduce correctly to the real-impedance limit `8/X·[1 + 1/(1+X) − (2/X)·ln(1+X)]` as Y→0, and to give `ᾱ_ri ∈ (0,1)` for every class A-H at the FORCE bands. **Confirmation against PDF p. 37 still outstanding** — flagged for the phase security/verify gate.
- **Fc/Fr constants (Assumptions A3/A4):** the `Fc` constant `5.888e-3` and structure (`∝(CT²/(273.15+t)² + (22/3)·Cv²/c²)·f²·ρ^{5/3}·d`) are used as stated in 02-RESEARCH; the `Fr` `g(X)` polynomial (A4) was unavailable and set to `g=1`. **Zero impact on Phase 2 targets** (FORCE 1-8 have Cv²=CT²=0 and roughness r=0 ⇒ Fc=Fr=1 exactly); turbulence cases are gated by property tests only.
- **Eq. 62 case table:** the document's symmetries reduce to the standard Faddeeva relations `w(conj(−ẑ))=conj(w(ẑ))` and `w(−ẑ)=2·exp(−ẑ²)−w(ẑ)`; no deviation from standard Faddeeva symmetry was found.
- **Oracle provenance hash:** `common.py sha256:fecb85555466464a` (recorded in the fixture `[meta].provenance`).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Research w(7+1j) anchor is a transcription error**
- **Found during:** Task 1 (computing full-precision anchors).
- **Issue:** 02-RESEARCH §4 prints `w(7+1j) = 0.019924+0.139158j`. The true `scipy.special.wofz(7+1j)` is `0.011629963+0.079732056j` (confirmed by the asymptote `w(z)→i/(√π·z)`). The research value is wrong.
- **Fix:** Used the correct scipy value in the anchor test (the document approximation reproduces scipy, so the engine must match scipy, not the mistyped anchor — consistent with the research note "resolve against the PDF, not against these numbers").
- **Files:** crates/envi-engine/src/propagation/special.rs
- **Commit:** Task 1 RED/GREEN.

**2. [Rule 1 - Tolerance] Oracle w-grid tolerance raised 2e-6 → 3e-6**
- **Found during:** Task 2 GREEN (oracle w-grid).
- **Issue:** The document's own three-pole branch differs from `wofz` by up to ~2.5e-6 relative just past the |y|=3 border (e.g. 1+3.01j) — larger than 02-RESEARCH's "~8e-7" (interior) estimate and the plan's suggested 2e-6.
- **Fix:** Set the fixture `w_tol_rel = 3e-6` with a documenting comment. This is the approximation's INTRINSIC error, not an engine bug (the Rust w(z) faithfully reproduces the document formula; standalone Python confirmed the same 2.5e-6 vs scipy).
- **Files:** tools/nord2000_oracle/gen_ground_fixtures.py, the regenerated fixture.
- **Commit:** Task 2 GREEN.

**3. [Rule 3 - Blocking] Two interface-block signatures extended for correctness**
- **`ground_impedance` returns `Result<Complex<f64>,_>`** (not bare `Complex`): required to satisfy the T-02-01 typed-error-on-invalid-σ must_have. Downstream plans call it with `?`.
- **`CoherenceInputs` gained `d_m`**: the Fc turbulence integral (Eq. 113) needs the propagation distance, which the interface block omitted. Geometry stays caller-injected.
- **Files:** ground.rs, coherence.rs (both documented in-module under "Deviation from the plan interface block").

## Known Stubs

- **`fresnel.rs` / `diffraction.rs` / `terrain_effect/mod.rs`** — doc-only shells, intentionally (implemented in plans 02-02/02-03/02-04). Registered now so wave-2 plans never conflict on `mod.rs`. Does not block this plan's goal (the numerics core is complete and self-contained).
- **`coherence_f` Fs = 1.0** — documented scattering-zone stub (not active for Phase 2 target cases).
- **`coherence_f` FΔν** — injected via `CoherenceInputs::f_delta_nu = 1.0`; Phase 3 refraction drops in the real factor without touching call sites.
- **`Fr` g(X) polynomial = 1** — Assumption A4 unavailable; inert for all Phase 2 targets (roughness r=0).

None of these stubs prevent the plan's goal (the Nord2000-native numerics core) from being achieved; each is a documented forward seam, not silent scope reduction.

## Threat surface

Mitigations from the plan `<threat_model>` implemented as correctness requirements: **T-02-01** (overflow guard on `2·exp(−ẑ²)`; branch-border + four-quadrant fixture tests; typed `InvalidFlowResistivity` on σ≤0/non-finite and `DegenerateRayGeometry` on bad geometry — no panics on data), **T-02-02** (cancellation-free ΔR is the only Δτ path; hS=0.01/d=1000 regression), **T-02-03** (full-precision Table 4/5 coefficients pinned by tests; committed scipy oracle as independent cross-implementation), **T-02-04** (oracle cites equations by number only; no PDF text/figures committed). **T-02-SC** holds: zero new engine dependencies (`errorfunctions` not used); num-complex added only as a harness dev-dep (already a workspace dep via the engine). No new network/auth/file surface.

## Verification Evidence

- `cargo build --workspace` — finished, no errors
- `cargo test --workspace` — 51 engine + 31 harness unit + 5 force (65 ignored) + 2 oracle, 0 failed
- `cargo test -p envi-harness --test oracle_ground` — 2 passed (w-grid 30 pts / 4 quadrants, Q̂-grid 24 pts)
- `cargo clippy --all-targets -- -D warnings` — zero warnings
- `cargo fmt --check` — clean
- `cargo tree -p envi-engine -e normal --depth 1` — only ndarray, num-complex, thiserror
- w(0.5+0.5j)=0.533157+0.230488j within 1e-6; Q̂(20000,1000)=+0.797560+0.416202j within 1e-4; |Q̂|(200,250)=1.2572; ΔR=1.538259287e-2 within 1e-8; Ff(646.7)=0.99993

## Self-Check: PASSED

All created files exist on disk; all six task commits (3 RED + 3 GREEN) are present in git history.
