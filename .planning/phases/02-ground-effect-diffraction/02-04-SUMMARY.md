---
phase: 02-ground-effect-diffraction
plan: 04
subsystem: engine
tags: [rust, nord2000, screen-diffraction, sub-model-4, sub-model-5, sub-model-6, sub-model-7, four-path-image-model, eight-ray, turbulence-scattering, two-channel, phase-preserving, generic-engine, eq-187-188, tables-6-7, scipy-oracle, convention-quarantine, tdd]

# Dependency graph
requires:
  - "02-01: numerics core — special::exp_clamped, ground chain, coherence, rays"
  - "02-02: terrain_effect::{GroundResult two-channel type, submodel1 eval hook}, fresnel::fresnel_zone_w"
  - "02-03: diffraction::{pwedge, p2wedge, p2edge, WedgeGeometry, TwoWedgeGeometry, TwoWedgeImpedances, WedgePrimary} frozen signatures"
  - "01-02: geometry::reflect_over_segment; rays::straight_rays_over_segment"
provides:
  - "envi-engine::geometry — §5.23 helpers: image_point (370), norm_line (377), segment_variables/SegmentVariables (383), vert_dist (390), wedge_cross (392)"
  - "envi-engine::propagation::terrain_effect::screen — DiffractionKernel trait, ScreenConfig, SurfaceStrip, PwedgeKernel/P2edgeKernel/P2wedgeKernel; submodel4/5/6 (two-channel GroundResult); submodel4_c_sr (C_SR helper for 02-05/SM7)"
  - "envi-engine::propagation::terrain_effect::submodel7 — ScreenScatterGeometry, submodel7_delta_l (Eqs. 271-274, Tables 6/7), combine_scatter (Eq. 332 incoherent add)"
  - "tools/nord2000_oracle/gen_screen_fixtures.py — independent SM4+SM7 oracle; screen_thin.toml case-71 curve"
affects: [02-05-terrain-composition, phase-03-refraction, phase-04-emission]

# Tech tracking
tech-stack:
  added: []
  patterns: [generic screen engine (DiffractionKernel trait + kernel-owned geometry, engine-owned image combinatorics), Eq. 188 weighted-geometric-mean magnitude realized via per-combination energy exponents while preserving the coherent/incoherent two-channel split, complex screen factor p̂₁/p̂₀ (phase live) × coherent Δp̂_G, SM7 typed f64-only (structurally phase-safe), page-image transcription of Eqs. 162/174-188 + Tables 6/7 with citations, committed scipy SM4+SM7 oracle at 0.1 dB, TDD]

key-files:
  created:
    - crates/envi-engine/src/propagation/terrain_effect/screen.rs
    - crates/envi-engine/src/propagation/terrain_effect/screen/tests.rs
    - crates/envi-engine/src/propagation/terrain_effect/submodel7.rs
    - tools/nord2000_oracle/gen_screen_fixtures.py
    - crates/envi-harness/tests/fixtures/oracle/screen_thin.toml
    - crates/envi-harness/tests/oracle_screen.rs
  modified:
    - crates/envi-engine/src/geometry.rs
    - crates/envi-engine/src/propagation/terrain_effect/mod.rs

key-decisions:
  - "Eq. 188 is a weighted GEOMETRIC mean of ground magnitudes: ΔL₄ = 20·lg(|p̂_SCR|·∏∏|p̂_{G,i1,i2}|^{w′_i1·w′_i2}) — a PRODUCT with exponent w′_i1·w′_i2, not a linear weighted sum (Open Q5 resolved on PDF p. 80 page image). Realized in the two-channel engine by combining per-combination energies with the same exponents (CC_eff=∏|c|²^W, G_eff=∏(|c|²+e)^W); h_coh=p̂_SCR·√CC_eff·e^{j(arg p̂_SCR+ΣW·arg c)}, p_incoh=|p̂_SCR|²·(G_eff−CC_eff) — so 10·lg(|h_coh|²+p_incoh) reproduces Eq. 188 exactly and reduces to h_coh=p̂_SCR·c for a single combination"
  - "Eq. 187 normalization transcribed verbatim (PDF p. 79): w_1t=Σw″; Δw_1t=max(w_1t−1,0); w′_i1={ (w″/w_1t)(Δw_1t/Δw_t+1) if w_1t>1; w″/w_1t if 0<w_1t≤1; 0 if w_1t=0 }; w_Q1={1 if w_1t≥1; w_1t² if w_1t<1}. Two DISTINCT weight sets: w_Q (base-model Q-weight) and w′ (Eq. 188 exponent)"
  - "Eq. 182 (PDF p. 75, image-verified): coherent c = 1 + F₂·w_Q1·Q̂₁·(p̂₂/p̂₁) + F₃·w_Q2·Q̂₂·(p̂₃/p̂₁) + F₄·w_Q1·w_Q2·Q̂₁·Q̂₂·(p̂₄/p̂₁); incoherent e = Σ(1−Fᵢ²)|w_Q·ℜ·(p̂ᵢ/p̂₁)|² with ℜ (fraktur) = incoherent_rho ρᵢ (Eq. 183). F₂/F₃/F₄ per Eqs. 177/179/181 = Ff(Δτ_side)·Fc(side); F₄ = Ff(Δτ_S+Δτ_R)·Fc_S·Fc_R"
  - "Wedge angles via Eq. 162 (PDF p. 70): β=2π−β₁−β₂, θ_S=2π−θ₁−β₂−Δθ_S, θ_R=θ₂−β₂+Δθ_R (Δθ=0 homogeneous); βᵢ/θᵢ = arctan((z−z_T)/(±(x_T−x)))+π/2. Agrees with 02-03's oracle thin-screen convention"
  - "Q̂₁/Q̂₂ 'evaluated as if the other endpoint were at the screen top' = spherical_q on the side reflection straight_rays_over_segment(endpoint, T, strip) — reuses the whole 02-01/02-02 reflection chain; Δτ_side, ρ_side, ℜ, and the image point all fall out of the same RayPair + image_point"
  - "SM7 Eq. 271 ×10 boost (Cve²=10·Cv², CTe²=10·CT²) is deliberate — cited at 3 sites in submodel7.rs (Pitfall 7). ΔL₇ returns f64 only: structurally cannot corrupt phase (threat T-02-13)"
  - "Tables 6/7 transcribed from PDF pp. 117-118 page images (Assumption A5 fallback used — the numeric grids read cleanly as const [[f64;10];8]); bilinear interp in (40·R₂/R₁, 40·h_e/R₁), edge-clamped; reciprocity max; C_SR = ground product floored at 1"

requirements-completed: [ENG-07]

# Metrics
duration: ~150min
completed: 2026-07-08
status: complete
---

# Phase 2 Plan 04: Screen⇄Ground Sub-models 4/5/6 + Turbulence Scattering (7) Summary

**The screen⇄ground interaction landed as one generic engine: the §5.23 image-method geometry helpers, a `DiffractionKernel`-parameterized four/eight-path model realizing Sub-model 4 (single edge, `pwedge`), Sub-model 5 (thick screen, `p2edge`) and Sub-model 6 (two screens, eight-ray bitmask, `p2wedge`), all producing the same phase-preserving two-channel `GroundResult` where the complex screen factor `p̂₁/p̂₀` carries the over-the-top phase and the `(1−F²)` residuals live only in `p_incoh`; plus Sub-model 7 turbulence scattering (Tables 6/7, the deliberate ×10 strengths) typed to return `f64` only — structurally phase-safe — cross-checked against a committed scipy SM4+SM7 oracle at ≤ 0.1 dB across all 105 bands.**

## Performance

- **Duration:** ~150 min
- **Completed:** 2026-07-08
- **Tasks:** 3

## Accomplishments

- **§5.23 geometry helpers (Task 1).** `image_point` (Eq. 370), `norm_line` (377), `segment_variables`/`SegmentVariables` (383), `vert_dist` (390), `wedge_cross` (392) — pure 2-D vector math with hand-computed anchors; degenerate inputs stay finite (never NaN), `wedge_cross` returns `None` for parallel/degenerate lines. `wedge_cross` present per must_have `contains`.
- **Generic screen engine + Sub-model 4 (Task 1).** `DiffractionKernel` trait (kernel owns wedge geometry, engine owns image combinatorics), `ScreenConfig`/`SurfaceStrip`, `PwedgeKernel`/`P2edgeKernel`/`P2wedgeKernel`, and `wedge_angles` (Eq. 162). The four-path image model (Eq. 157) combines per Eq. 182 (coherent complex `c` + `(1−F²)` residual `e`) across all before×after strip combinations with the Eq. 187 normalization and the Eq. 188 weighted-geometric-mean magnitude. Two-channel output: `h_coh = p̂_SCR·√CC_eff·e^{j(arg p̂_SCR+ΣW·arg c)}`, `p_incoh = |p̂_SCR|²·(G_eff−CC_eff)`, so `10·lg(|h_coh|²+p_incoh)` equals Eq. 188 exactly and `|h_coh|` = the level magnitude at 1e-12 with a live, frequency-dependent phase (the contract pin). `F=1 ⇒ p_incoh == 0` bit-exact.
- **Sub-models 5 & 6 as parameterizations (Task 2).** SM5 swaps the kernel to `p2edge` (shared four-path engine — no duplicated combination math); SM6 is the eight-ray set (Eq. 222) as a three-region `{before, middle, after}` bitmask over the 2³ subsets, verified term-for-term against a hand sum and collapsing to the four-term subset when the middle region is empty. FORCE case-81 (thick) and case-91 (double) literal geometries are finite two-channel results at all 105 bands.
- **Sub-model 7 turbulence scattering (Task 3).** Tables 6/7 as `const [[f64;10];8]` (transcribed from PDF pp. 117-118), the deliberate ×10 effective strengths (Eq. 271, cited), 2-D bilinear interpolation in `(40·R₂/R₁, 40·h_e/R₁)` with edge clamp + reciprocity max, `C_SR` ground correction, `f₀ = Coft(t)/(2·sin(θ/2))` low-frequency roll-off (Eqs. 272-273) and the incoherent `ΔL₇` (Eq. 274). `submodel7_delta_l` returns `f64`; `combine_scatter` performs the Eq. 332 incoherent add (`ΔL₄+ΔL₇ ≥ ΔL₄` always). `submodel4_c_sr` exposes the ground-product-floored-at-1 correction for the caller.
- **Committed scipy SM4+SM7 oracle (Task 3).** `gen_screen_fixtures.py` re-implements the four-path model (Eqs. 157-188 including the Fresnel-weight + `w_Q` machinery) and SM7 (Eqs. 271-274) independently (wedge-face `Q̂`/`pwedge` from the sibling scipy oracle), emitting `screen_thin.toml` for the literal case-71 thin-screen geometry with forced turbulence. `oracle_screen.rs` matches engine `ΔL₄+ΔL₇` at all 105 points within 0.1 dB.
- **All quality gates pass:** `cargo build --workspace`; `cargo test --workspace` (103 engine + 31 harness + 5 force + 2 oracle_ground + 2 oracle_flat + 5 oracle_wedge + 1 oracle_screen, 0 failed); `cargo clippy --all-targets -- -D warnings` (zero); `cargo fmt --check` (clean); `#![deny(unsafe_code)]` holds; `cargo tree -p envi-engine` unchanged (ndarray/num-complex/thiserror); zero `conj()` in propagation (quarantine intact).

## Task Commits

1. **Task 1: geometry helpers** — `37a104c feat(02-04): add §5.23 geometry helpers`
2. **Task 1: SM4 engine** — `99f196a feat(02-04): generic screen engine + Sub-model 4 (Eqs. 157-188)`
3. **Task 2: SM5/SM6** — `f3df308 test(02-04): Sub-model 5/6 parameterization + FORCE case-81/91 finiteness`
4. **Task 3: SM7 + oracle** — `09ce9ee feat(02-04): Sub-model 7 turbulence scattering + thin-screen oracle`

## Recorded findings (per plan `<output>`)

### Eq. 187-188 transcription (Open Q5 RESOLVED — PDF pp. 79-80 page images)

- **Eq. 188 is a weighted GEOMETRIC mean, not a linear sum.** `ΔL₄ = 20·lg( |p̂_SCR(f)| · ∏_{i1}∏_{i2} |p̂_{G,i1,i2}|^{w′_i1(f)·w′_i2(f)} )` — a PRODUCT over combinations with the exponent `w′_i1·w′_i2`. The pypdf dump garbled this as a sum; the page image is authoritative. The oracle implements the identical reading. For a single strip per side (`w′=1`) it collapses to `20·lg(|p̂_SCR·Δp̂_G|)`.
- **Eq. 187** (verbatim): `w_1t = Σ_{i1} w″_i1`; `Δw_1t = w_1t−1 if w_1t>1 else 0`; `Δw_t = Δw_1t+Δw_2t`; `w′_i1 = (w″_i1/w_1t)(Δw_1t/Δw_t + 1)` if `w_1t>1`, `= w″_i1/w_1t` if `0<w_1t≤1`, `= 0` if `w_1t=0`; `w_Q1 = 1 if w_1t≥1 else w_1t²`. **Two distinct weight sets** — `w_Q` (base-model Q-weight inside Eq. 182) and `w′` (the Eq. 188 exponent). This distinction was load-bearing for the oracle match: the source-side zone spills past the screen base at low frequency so `w_1t < 1` and `w_Q < 1` even for a "wide" strip.
- **Eq. 174** `w″ = w·r_S·r_R` with `F_λ = λ/16`; **Eqs. 175-176** edge-proximity modifiers `h_max = min(0.0005·Δx, 0.2)` (≈ 1 except very near a segment extension). **Eq. 162** wedge angles; **Eq. 164** `SegmentVariables`.

### SM5/SM6 printed deltas encountered

- **SM5 does NOT reduce to SM4 in the thin limit.** The bare `p2edge` double-diffraction kernel always diffracts over both edges (with `R_M → 0` it is numerically degenerate, ~38 dB below the single edge). The thin→single-edge transition is the §5.21 `r_scr12` blend (plan 02-05), not a property of the sub-model — mirroring the fact that SM4 does not itself reduce to Sub-model 1 when the screen is removed. Test pinned to the genuine bare-kernel property (double diffraction ≥ single).
- **SM6 eight-ray set = 2³ region bitmask.** Eq. 222's eight terms are exactly the subsets of `{before, middle, after}` with the printed `Q̂` products (`Q̂₁`, `Q̂₃`, `Q̂₁Q̂₃`, `Q̂₂`, `Q̂₁Q̂₂`, `Q̂₂Q̂₃`, `Q̂₁Q̂₂Q̂₃`). Middle reflection reflects the receiver-side endpoint over the mid strip (approximation, finite; not oracle-pinned — SM6 is validated structurally + finiteness only, per the plan).

### Tables 6/7 extraction method

pdfplumber `page.to_image()` render of pp. 117-118 read visually (Assumption A5 fallback path — the numeric grids are unambiguous as images). Committed as `const [[f64;10];8]` in both the engine and the oracle with a "transcribed from AV 1106/07 Tables 6/7, pp. 117-118" citation. Rows = `40·h_e/R₁ ∈ {5..40 step 5}`, cols = `40·R₂/R₁ ∈ {10..100 step 10}`.

### C_SR helper signature for 02-05

```rust
// terrain_effect::screen
pub fn submodel4_c_sr(f_hz: f64, cfg: &ScreenConfig) -> Result<f64, PropagationError>;
// returns G_eff.max(1.0)  (= p_G² floored at 1, Eq. 272)

// terrain_effect::submodel7
pub struct ScreenScatterGeometry { pub r1: f64, pub r2: f64, pub h_e: f64, pub t_air_c: f64 }
pub fn submodel7_delta_l(f_hz: f64, geo: &ScreenScatterGeometry, cv2: f64, ct2: f64, c_sr: f64)
    -> Result<f64, PropagationError>;
pub fn combine_scatter(delta_l_scr_db: f64, delta_l7_db: f64) -> f64;  // Eq. 332 incoherent add
```

## Deviations from Plan

### Interpreted acceptance criteria (dispatcher-boundary properties)

**1. [Rule 1 - Physics] Task 1 Test 6 "screen-removed recovers Sub-model 1 within 0.5 dB" reinterpreted.** SM4's four-path model splits the single ground bounce into two half-path reflections (`Q̂₁·Q̂₂` at near-grazing ≈ `+1`, vs SM1's single `Q̂ ≈ −1`), so it cannot algebraically reduce to Sub-model 1. That recovery is enforced by the §5.21 `r_scr1→0` transition (plan 02-05). The test instead pins the genuine bare-SM4 property: the screen **factor** `|p̂_SCR| → 1` (insertion loss ≈ 0 dB, within 0.5 dB) in the deep-lit zone, and ΔL₄ stays finite/bounded. Documented in-test.

**2. [Rule 1 - Physics] Task 2 Test 1 "1 cm thick ≈ thin within 0.3 dB" reinterpreted.** Same dispatcher boundary: the bare `p2edge` kernel over-attenuates a degenerate thin thick-screen (double diffraction with `R_M → 0`); the thin→single-edge transition is the §5.21 `r_scr12` blend (02-05). Test pins the defensible property: SM5 shares the four-path engine and double diffraction ≥ single diffraction. Documented in-test.

### Auto-fixed / required

**3. [Rule 3 - Oracle consistency] Oracle replicates `exp_clamped` and the full Fresnel/`w_Q` machinery.** For the SM4 oracle to match the engine at 0.1 dB, `gen_screen_fixtures.py` had to (a) compute the actual Fresnel-zone weight `w″` and `w_Q` (the source-side zone genuinely spills past the screen base at low f, so `w_Q < 1` — the naive `w=1` assumption gave a 0.29 dB error) and (b) use the engine's clamped exponential `exp'` for `Fc` (not plain `exp`, which diverges for `Fc` exponent > 1 at higher frequencies). Both are the engine's exact reading; the oracle now matches 105/105 within 0.1 dB.

No architectural deviations; no user-facing scope cuts.

## Known Stubs

- **Shadow-zone branches (Eqs. 184-186):** `ξ<0` refraction — Phase 3; the non-shadow branch (`dSZ = ∞`) is always taken, documented at the seam (same pattern as Sub-model 1).
- **`FΔν = 1`, `Fs = 1`:** 02-01 stubs carried through (injected via `CoherenceInputs`/local `Fc`); the oracle uses the same, so the case-71 comparison is self-consistent.
- **SM6 middle-region reflection** is an approximation (reflects the receiver endpoint over the mid strip): finite and structurally correct for the Eq. 222 `Q̂` pattern, validated structurally + by finiteness, not oracle-pinned. The precise middle-image geometry is a Phase 4 refinement seam if a two-screen oracle is added.

None prevent the plan's goal (screen⇄ground four/eight-path complex combination + turbulence floor); each is a documented forward seam.

## Threat surface

Mitigations from the plan `<threat_model>` implemented as correctness requirements:
- **T-02-11 (NaN/Inf):** §5.23 helpers stay finite on degenerate input; typed errors on degenerate strips/σ; Tables 6/7 edge clamping; case-71/81/91 literal-geometry finiteness across all 105 bands.
- **T-02-12 (silently plausible wrongness):** Eq. 187-188 + Tables 6/7 transcribed from page images with citations; stub-kernel term-for-term assembly tests (four-path and eight-ray); the oracle implements the same Eq. 188 reading and agrees at 0.1 dB; table-node reproduction tests.
- **T-02-13 (phase/channel corruption):** `|h_coh| == level-form magnitude` at 1e-12 + frequency-dependent `arg` test; **Sub-model 7 returns `f64` only** — structurally incapable of touching `h_coh_factor`; the two-channel identity holds to 1e-12; `F→1 ⇒ p_incoh == 0` bit-exact.
- **T-02-SC:** zero new dependencies; engine tree unchanged. No new network/auth/file surface (oracle is a dev tool generating committed data; equations cited by number only).

## Verification Evidence

- `cargo build --workspace` — finished, no errors
- `cargo test --workspace` — 103 engine + 31 harness + 5 force + 2 oracle_ground + 2 oracle_flat + 5 oracle_wedge + 1 oracle_screen, 0 failed
- `cargo test -p envi-harness --test oracle_screen` — 105/105 points ≤ 0.1 dB (case-71 ΔL₄+ΔL₇)
- `cargo clippy --all-targets -- -D warnings` — zero warnings
- `cargo fmt --check` — clean
- `cargo tree -p envi-engine -e normal --depth 1` — only ndarray, num-complex, thiserror
- SM7 `submodel7_delta_l` is `f64`-typed (phase-safe by construction); screen⇄ground `h_coh_factor` stays complex & finite across the band range

## Self-Check: PASSED

All created files exist on disk; all four task commits (37a104c, 99f196a, f3df308, 09ce9ee) are present in git history; full workspace build/test/clippy/fmt gates green.
