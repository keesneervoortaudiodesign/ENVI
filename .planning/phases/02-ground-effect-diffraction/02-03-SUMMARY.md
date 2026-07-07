---
phase: 02-ground-effect-diffraction
plan: 03
subsystem: engine
tags: [rust, nord2000, hadden-pierce, wedge-diffraction, pwedge, p2wedge, p2edge, complex-phase, fresnel, spherical-q, convention-quarantine, scipy-oracle]

# Dependency graph
requires:
  - "02-01: special::{fresnel_f, fresnel_g} (Â_D, Eq. 84), ground::spherical_q (wedge faces, Eq. 80), propagation::sound_speed_ms, PropagationError::DegenerateRayGeometry, diffraction.rs shell"
provides:
  - "envi-engine::propagation::diffraction — WedgeGeometry, pwedge/dwedge (Eqs. 78-94), pwedge0/dwedge0 (Eqs. 105-107), TwoWedgeGeometry/WedgePrimary/TwoWedgeImpedances, p2wedge (Eqs. 95-99), p2edge (Eqs. 100-104): the ENG-03 kernel family returning live Complex<f64> pressure"
  - "tools/nord2000_oracle/gen_wedge_fixtures.py — independent Hadden-Pierce oracle (scipy wofz on faces) + wedge_il.toml fixtures"
affects: [02-04-screens, 02-05-terrain-composition, phase-03-refraction]

# Tech tracking
tech-stack:
  added: []
  patterns: [Nord2000-native e^{-jωt} convention quarantine (zero conj() in diffraction.rs), page-image transcription with numeric anchor confirmation (Assumption A6 resolved), committed cross-implementation scipy oracle, TDD RED/GREEN per task, prescriptive Eq.80 grazing-angle transcription (no re-derivation)]

key-files:
  created:
    - crates/envi-engine/src/propagation/diffraction.rs
    - tools/nord2000_oracle/gen_wedge_fixtures.py
    - crates/envi-harness/tests/fixtures/oracle/wedge_il.toml
    - crates/envi-harness/tests/oracle_wedge.rs
  modified: []

key-decisions:
  - "Eq. 78 carries the A(θₙ) factor explicitly: p̂ = −(1/π)·Σ Q̂ₙ·A(θₙ)·Ê_ν(A(θₙ))·e^{jωτ}/ℓ — verified on PDF p. 39 image AND by the shadow-boundary 0.5 derivation (A(θ₁)=∓π/2 gives −(1/π)(π/2)=0.5)"
  - "Reused PropagationError::DegenerateRayGeometry (no new enum variant, no mod.rs edit) for wedge domain rejection — zero shared-file churn, disjoint from 02-02"
  - "p2wedge/p2edge take the raw Fig.10/11 inputs (TwoWedgeGeometry, 12 scalars) + WedgePrimary flag + face impedances; composite_geoms builds the two Dwedge argument lists per Eqs. 97/98/102/103 — the frozen 02-04 contract"
  - "Angle-modification scheme (p.43) applied BEFORE the four-term sum so ground-reflected/refracted image points inside the wedge don't trip domain validation"

requirements-completed: [ENG-03]

# Metrics
duration: ~90min
completed: 2026-07-08
status: complete
---

# Phase 2 Plan 03: Hadden–Pierce Wedge Diffraction Kernel Family Summary

**The complex, phase-preserving ENG-03 diffraction kernels — the four-term Hadden–Pierce finite-impedance wedge `pwedge`/`dwedge` (Eqs. 78–94) with its lit-zone ray additions, the p.43 angle-modification scheme, the non-reflecting `pwedge0` (Eqs. 105–107), and the `p2wedge` (Eqs. 95–99) / `p2edge` (Eqs. 100–104) composites — all returning genuine `Complex<f64>` carrying the over-the-top `e^{+jωτ}` phase, transcribed from the PDF page images (Assumption A6 resolved) and anchored to the research IL table (±0.005 dB), the shadow-boundary half-field limit (0.500), and a committed independent scipy oracle.**

## Performance

- **Duration:** ~90 min
- **Completed:** 2026-07-08
- **Tasks:** 3 (all TDD: RED test commit → GREEN feat commit)

## Accomplishments

- **`pwedge` four-term core (Task 1, Eqs. 78–86).** `wedge_sum` implements `−(1/π)·Σₙ₌₁⁴ Q̂ₙ·A(θₙ)·Ê_ν(A(θₙ))·e^{jωτ}/ℓ` with the θₙ set (Eq. 79), Ê_ν (Eq. 81), A(θₙ) with the Heaviside jump (Eq. 82), B (Eq. 83) and `Â_D = Sign·(f − j·g)` (Eq. 84) via `special::fresnel_f/g`. Face coefficients Q̂ₙ (Eq. 80) are **prescriptively** transcribed: `τ₂ = τ_S + τ_R`, grazing `min(β−θ_S, π/2)` / `min(θ_R, π/2)` via `ground::spherical_q`; Q₁=1, Q₄=Q₂·Q₃. Reproduces the hard-screen IL anchor **row-for-row within 0.005 dB** (12.01/14.35/17.02/19.90/22.87/25.87 dB). Both mandatory singular-geometry guards present: the p.41 `|θₙ−π| < 1e-8` ε-subtraction and the `|A| < 1e-6` Taylor sinc.
- **Lit-zone additions + angle-modification + `pwedge0` (Task 2, Eqs. 87–90, 105–107).** `lit_additions` adds the direct ray (θ₁<π, Eq. 88), the source-face reflection (θ₃<π, Eq. 89) and the receiver-face reflection (θ₂<π, Eq. 90) with the p.42 `ψ_G = |arcsin((R_S sinθ_S + R_R sinθ_R)/R₂)|` grazing forms. `modify_angles` applies the p.43 four-case table verbatim so image points inside the wedge are admitted. `pwedge0`/`dwedge0` keep only the n=1 term and drop the face reflections. **Shadow-boundary |p̂|·ℓ → 0.500 ± 0.01 from BOTH sides** (0.5056 shadow / 0.5077 lit at ±0.01°); deep-lit free-field recovery to 0.2 %.
- **`p2wedge`/`p2edge` composites + case-71 robustness (Task 3, Eqs. 95–104).** `composite_geoms` builds the two `dwedge` argument lists per Eq. 97/98 (p2wedge) and Eq. 102/103 (p2edge); `p2wedge = D̂₁·D̂₂·e^{jωτ}/ℓ²`, `p2edge = 0.5·D̂₁·D̂₂·e^{jωτ}/ℓ²` with **forced-hard top faces**. The literal FORCE case-71 thin screen (β = 2π−0.01, near-vertical 0.01 m faces) yields **finite complex values at all 105 grid points**. p2wedge two-screen IL ≥ single-screen IL at every frequency.
- **Independent scipy oracle (Task 3).** `gen_wedge_fixtures.py` re-implements Hadden–Pierce independently (importing only `common.py`'s `spherical_q`, which uses `scipy.special.wofz`) and emits `wedge_il.toml` (hard IL grid, 8-point shadow-boundary approach series, σ=200 finite-impedance-face wedge, thick-screen p2edge, two-wedge p2wedge) with a `common.py` sha256 provenance header. `oracle_wedge.rs` cross-checks the engine against every row (IL ±0.05 dB, complex pressures 1e-3 relative). Python/scipy are NOT build dependencies.
- **All quality gates pass:** `cargo build --workspace`; `cargo test --workspace` (81 engine + 31 harness + 2 oracle_ground + 2 oracle_flat + 5 oracle_wedge + 5 force, 0 failed); `cargo clippy --all-targets -- -D warnings` (zero); `cargo fmt --check` (clean); `#![deny(unsafe_code)]` holds; `cargo tree -p envi-engine` shows only ndarray/num-complex/thiserror (quarantine intact); **zero `conj()` in diffraction.rs** (phase-corruption threat T-02-10 mitigated).

## Recorded findings (per plan `<output>`)

### Assumption A6 RESOLVED — frozen p2wedge/p2edge argument lists (PDF pp. 45–48, image-verified)

`Dwedge(f, β, θ_S, θ_R, τ, τ_S, τ_R, ℓ, R_S, R_R, Ẑ_S, Ẑ_R)` is the sub-kernel. With `τ = τ_S+τ_M+τ_R`, `ℓ = R_S+R_M+R_R`:

**p2wedge, First primary (Eq. 97):**
- D̂₁ = Dwedge(f, β₁, θ_{1S}, θ_{1R}, **τ**, τ_S, τ_M+τ_R, **ℓ**, R_S, R_M+R_R, Ẑ_{1S}, Ẑ_{1R})
- D̂₂ = Dwedge(f, β₂, θ_{2S}, θ_{2R}, τ_M+τ_R, τ_M, τ_R, R_M+R_R, R_M, R_R, Ẑ_{2S}, Ẑ_{2R})

**p2wedge, Second primary (Eq. 98):**
- D̂₁ = Dwedge(f, β₁, θ_{1S}, θ_{1R}, τ_S+τ_M, τ_S, τ_M, R_S+R_M, R_S, R_M, Ẑ_{1S}, Ẑ_{1R})
- D̂₂ = Dwedge(f, β₂, θ_{2S}, θ_{2R}, **τ**, τ_S+τ_M, τ_R, **ℓ**, R_S+R_M, R_R, Ẑ_{2S}, Ẑ_{2R})

**p2wedge composition:** `p̂ = D̂₁·D̂₂·e^{+j2πfτ}/ℓ²` (Eq. 99).

**p2edge (Eqs. 102/103):** identical geometry mapping to Eq. 97/98, but top faces forced hard (D̂₁'s Ẑ_R = ∞, D̂₂'s Ẑ_S = ∞), and `p̂ = 0.5·D̂₁·D̂₂·e^{+j2πfτ}/ℓ²`. Only Ẑ_{1S}/Ẑ_{2R} are finite-impedance inputs (Eq. 104).

### p.42 lit-zone grazing-angle forms (Eqs. 89–90, image-verified)

- Eq. 88 (θ₁<π): R₁ = √(R_S²+R_R²−2R_SR_R cosθ₁), τ₁ = √(τ_S²+τ_R²−2τ_Sτ_R cosθ₁); add e^{jωτ₁}/R₁.
- Eq. 89 (θ₃<π): source-face reflection Q̂_R(f, τ₂, ψ_{G,R}, Ẑ_R)·e^{jωτ₂}/R₂ with R₂/τ₂ from cosθ₂ and **ψ_{G,R} = arcsin((R_S sinθ_S + R_R sinθ_R)/R₂)**.
- Eq. 90 (θ₂<π): receiver-face reflection Q̂_S(f, τ₃, ψ_{G,S}, Ẑ_S)·e^{jωτ₃}/R₃ with R₃/τ₃ from cosθ₃ and **ψ_{G,S} = arcsin((R_S sin(β−θ_S) + R_R sin(β−θ_R))/R₃)**. (The `|·|` on the arcsin is applied for numerical safety; argument clamped to [−1,1].)

### p.43 angle-modification scheme (image-verified, applied in order)

1. `0 > θ_R > β−2π`: θ′_R=0, θ′_S=θ_S−θ_R, β′=β−θ_R.
2. `θ_R ≤ β−2π`: θ′_R=0, θ′_S=2π−(β−θ_S), β′=2π.
3. `β < θ_S < 2π` (post-1/2): β′=θ_S.
4. `θ_S ≥ 2π` (post-1/2): θ′_S=2π, β′=2π.

### Final public kernel signatures for 02-04

```rust
pub struct WedgeGeometry { tau_s, tau_r, tau, r_s, r_r, l, theta_s, theta_r, beta: f64 }
pub fn pwedge(f_hz: f64, geo: &WedgeGeometry, z_s: Complex<f64>, z_r: Complex<f64>) -> Result<Complex<f64>, PropagationError>;
pub fn dwedge(f_hz: f64, geo: &WedgeGeometry, z_s: Complex<f64>, z_r: Complex<f64>) -> Result<Complex<f64>, PropagationError>;
pub fn pwedge0(f_hz: f64, geo: &WedgeGeometry) -> Result<Complex<f64>, PropagationError>;
pub fn dwedge0(f_hz: f64, geo: &WedgeGeometry) -> Result<Complex<f64>, PropagationError>;
pub struct TwoWedgeGeometry { beta1, theta_1s, theta_1r, beta2, theta_2s, theta_2r, tau_s, tau_m, tau_r, r_s, r_m, r_r: f64 }
pub enum WedgePrimary { First, Second }
pub struct TwoWedgeImpedances { z_1s, z_1r, z_2s, z_2r: Complex<f64> }
pub fn p2wedge(f_hz: f64, geo: &TwoWedgeGeometry, primary: WedgePrimary, z: &TwoWedgeImpedances) -> Result<Complex<f64>, PropagationError>;
pub fn p2edge(f_hz: f64, geo: &TwoWedgeGeometry, primary: WedgePrimary, z_1s: Complex<f64>, z_2r: Complex<f64>) -> Result<Complex<f64>, PropagationError>;
```

`θ` per wedge is measured CCW from that wedge's receiver-side face; 02-04 computes θ_S/θ_R/β from the terrain profile and passes them in (the geometry→angle conversion is 02-04's responsibility, §5.21).

## Deviations from Plan

### Auto-fixed / clarified

**1. [Rule 3 - Blocking] Reused `DegenerateRayGeometry` instead of a new error variant.** The plan's threat mitigation (T-02-08) calls for typed errors on degenerate faces. Rather than add a `PropagationError` variant (a shared-file edit to `propagation/mod.rs`, owned jointly), `pwedge`'s `validate` returns the existing `PropagationError::DegenerateRayGeometry { detail }`. Zero shared-file churn — fully disjoint from 02-02. Documented in-module.

**2. [Clarification] Deep-lit-zone test geometry retuned.** The Task-2 free-field-recovery test's first geometry (edge at y=0.2, θ₁=169°) recovered the free field only to 2.5 % — not deep enough. Retuned to S=(0,30)/T=(50,0.1)/R=(100,30) (θ₁=118°), which recovers to 0.2 %, well within the 1 % anchor. Physics unchanged; the test geometry now genuinely exercises the deep lit zone.

No architectural deviations. The IL table, shadow-boundary 0.500, and all composite argument lists match the research anchors and the independent oracle.

## Known Stubs

None. Every kernel in the ENG-03 family (`pwedge`, `dwedge`, `pwedge0`, `dwedge0`, `p2wedge`, `p2edge`) is fully implemented and anchored. The `WedgePrimary::Second` branch is implemented and exercised by `composite_geoms` (unit-tested via the composition assertion), though the oracle fixtures currently pin `First`; 02-04 will exercise `Second` when §5.21 selects the receiver-side wedge as primary.

## Threat surface

Mitigations from the plan `<threat_model>` implemented as correctness requirements:
- **T-02-08 (Inf/NaN at singular geometry):** p.41 ε=1e-8 guard, `|A|<1e-6` Taylor sinc, the literal case-71 finiteness sweep across all 105 grid points, and typed `DegenerateRayGeometry` on non-finite/non-positive/`β∉(π,2π]` inputs — no panics on data.
- **T-02-09 (silently plausible wrongness):** prescriptive Eq. 80 grazing-angle transcription (no re-derivation), IL anchor table ±0.005 dB, the analytic shadow-boundary 0.500 limit from both sides, and the independent scipy Hadden–Pierce oracle. The p2wedge/p2edge argument lists and the p2edge 0.5 factor + hard-top faces were verified on PDF page images (A6) and pinned by a composition unit test.
- **T-02-10 (phase corruption):** phase-liveness test (non-zero Im, 2πfτ advances with f); **zero `conj()` anywhere in diffraction.rs** — the module is Nord2000-native e^{−jωt} and the diffracted pressure carries live e^{+jωτ} phase.
- **T-02-SC (supply chain):** zero new dependencies; engine tree unchanged (ndarray/num-complex/thiserror).

No new network/auth/file surface. The oracle script cites equations by number only — no PDF text/figures committed (page images were rendered to the scratchpad for transcription and not committed).

## Verification Evidence

- `cargo build --workspace` — finished, no errors
- `cargo test --workspace` — 81 engine + 31 harness + 2 oracle_ground + 2 oracle_flat + 5 oracle_wedge + 5 force (65 ignored), 0 failed
- `cargo test -p envi-engine diffraction` — 13 passed (IL table, θₙ=π & sinc guards, phase liveness, Â_D continuity, shadow-boundary both sides, Dwedge normalization, deep-lit recovery, pwedge0 n=1, angle-mod, case-71 sweep, p2edge composition, p2wedge monotonicity)
- `cargo test -p envi-harness --test oracle_wedge` — 5 passed (hard IL grid, shadow series, finite-impedance faces, p2edge, p2wedge)
- `cargo clippy --all-targets -- -D warnings` — zero warnings
- `cargo fmt --check` — clean
- `cargo tree -p envi-engine -e normal --depth 1` — only ndarray, num-complex, thiserror
- IL(1000 Hz) = 19.897 dB (want 19.90 ± 0.05); shadow |p̂|·ℓ = 0.5056 / 0.5077 at ±0.01° (want 0.500 ± 0.01); case-71 = 105/105 finite; oracle provenance `common.py sha256:fecb85555466464a`

## Self-Check: PASSED

All created files exist on disk; all six task commits (3 RED + 3 GREEN) are present in git history (1d15b87, 99c99cf, a3f4a46, 462d4ae, c5810c9, 21e2ee9).
