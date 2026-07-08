# Phase 3: Meteorology & Refraction - Research

**Researched:** 2026-07-08
**Domain:** Nord2000 (AV 1106/07 rev.4) meteorological refraction — log-lin sound-speed profile, equivalent-linear collapse (CalcEqSSP/CalcEqSSPGround), circular-ray variables (ξ, Δτ), upward-refraction shadow zone, reflection-path coefficients, weather-input routes, and fluctuating-refraction/turbulence coherence — in the existing Rust `envi-engine`/`envi-harness` workspace.
**Confidence:** HIGH for the AV-1106/07 core machinery (every load-bearing equation transcribed from `refs/AV1106-07-rev4.pdf` page text this session, §5.3.2, §5.4, §5.5.1–5.5.7, §5.9, §5.10, §5.23.16, Annex F). MEDIUM/LOW for the three weather-route A/B/C *derivations*, which are **not in AV 1106/07** and whose origin research doc is **missing from the repo** — see the Assumptions Log and Open Questions.

## Summary

Everything the *engine* needs for refraction is in AV 1106/07 rev.4 and was extracted at implementation precision this session. The spine is four auxiliary functions the phase must build, each with a verbatim equation set: **CalcEqSSP** (§5.5.2, Eqs. 15–21) collapses the log-lin profile `c(z)=A·ln(z/z₀+1)+B·z+C` to an equivalent-linear profile parameterised by `(ξ, c₀)` where `ξ = (∂c/∂z)/c₀` is the relative sound-speed gradient and `∂c/∂z` is the **average gradient between hS and hR** (Eq. 18) using the closed-form `c̄` integral (Annex F Eq. 403 — no numerical integration); **CalcEqSSPGround** (§5.5.3, Eqs. 22–28) makes `(ξ(f), c₀(f))` **frequency-dependent** for soft ground by log-interpolating the gradient between `fL` (Eq. 26) and `fH` (Eq. 27) — evaluated natively at ENVI's 105 1/12-octave points; **DirectRay** (§5.5.4, Eqs. 29–44) and **ReflectedRay** (§5.5.5, Eqs. 45–50) produce the circular-ray `τ, R, ψ_G, Δα, dSZ` that fill the existing `RayVars`/`RayPair` seam; and **TravelTimeDiff** (§5.5.6, Eqs. 51–53) yields the interference `Δτ` with an upward-refraction-safe reformulation `Δτ₀` (Eq. 52). The homogeneous limit is analytically clean: `CalcEqSSP` itself returns `ξ=0, c̄=c₀=C` whenever `|ξ|<10⁻⁶` (p.24), so the D-02 bit-for-bit anchor triggers below that threshold and the Phase-2 straight-ray path is taken unchanged.

Two integration seams are already built and drop in without touching call sites. **F_τ** (fluctuating-refraction coherence, Eq. 112) is the *exact same sinc form* as `Ff` but with `x = 2π·f·(Δτ⁺(f) − Δτ(f))`, where `Δτ⁺` is the travel-time difference under the **upper-refraction profile** `A⁺=A+1.7·sA, B⁺=B+1.7·sB` (Eq. 10). It enters through the pre-built `CoherenceInputs::f_delta_nu` field (currently `1.0`); Eq. 118 confirms the full chain `F = Ff·Fτ·Fc·Fr·Fs`. The two-channel readout, the single `.conj()` boundary, and the `TerrainEffect` composition are all frozen and need no new plumbing — refraction only changes *which* `RayPair` and *which* `ξ`/`Δτ` feed the existing `submodel1`/`terrain_effect`.

The one genuinely new numerical primitive is the **downward-refraction reflection-point cubic** (Eq. 49): a cubic in `d_refl` with up to three real roots (pick closest to source if `hS<hR`, else closest to receiver); upward refraction gives a single real root. The **shadow zone** (D-09, explicit success criterion) is modelled by AV as an *equivalent wedge* (§5.23.16 `ShadowZoneShielding`, Eqs. 384–388) that **reuses the Phase-2 diffraction kernel** with a frequency-dependent equivalent height `hSZ` — so shadow-zone attenuation couples upward refraction into the already-built wedge machinery rather than being a bespoke model.

**The sharp scope warning for the planner:** the three *weather-input routes* (Route 1 classes, Route 2 FORCE surface-met→A/B, Route 3 Monin–Obukhov→LSQ A/B/C) convert meteorology into the `(A, B, C, sA, sB, z₀)` inputs — but **AV 1106/07 does not specify these conversions**. It states only "A and B are determined from weather data available at normal weather stations" and defers yearly-average classes to reference "[4]". The origin `docs/research.md` that CONTEXT.md cites for these derivations **does not exist in the repo** (verified). Route derivations are therefore MEDIUM/LOW confidence and must be treated as `[ASSUMED]` until the user supplies the Nord2000 companion reference (AV 1851/00 Part 2, or the CNOSSOS-EU meteo-class tables). This does not block the engine core (which takes A/B/C as inputs) but does gate MET-02/05/06 numeric fixtures.

**Primary recommendation:** Build a new `propagation/refraction/` submodule implementing CalcEqSSP / CalcEqSSPGround / DirectRay / ReflectedRay / TravelTimeDiff / reflection-cubic / RayCurvature / ShadowZoneShielding verbatim from the cited AV equations, wire the circular-ray constructor behind the existing `RayVars`/`RayPair` fields and `Δτ⁺` into `f_delta_nu`, keep all weather-route I/O in `envi-harness`, and gate on the Phase-2-style oracle+anchor ladder (extend `tools/nord2000_oracle/`). Zero new engine dependencies.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions (D-01 … D-16 — research HOW to satisfy, do not re-open)

**Validation gate strategy**
- **D-01:** Refraction gates for numeric green via the **same oracle+anchor ladder as Phase 2** — do NOT attempt a direct FORCE up/downwind numeric comparison in Phase 3.
- **D-02:** Three rungs: (a) **homogeneous-limit exactness anchor** — with `|ξ|` below the clamp threshold the refracted machinery reproduces the Phase-2 straight-ray results **bit-for-bit** (circular-ray constructor fills `RayVars`/`RayPair` behind identical fields); (b) **committed scipy oracle** (extend `tools/nord2000_oracle/`) pinning ξ, Δτ, CalcEqSSP collapse, and CalcEqSSPGround — regenerated by `gen_*.py`, no Python at test time; (c) **property tests for direction** — downward refraction → level gain, upward refraction → shadow-zone loss, monotonic where physics demands.
- **D-03:** FORCE up/downwind cases **stay capability-gated** — `requires:` shrinks (drop `refraction`) but retains `emission-model`, so they remain `Skipped`, never a false `Pass`, until Phase 4. The `Capability::Refraction` gate is flipped to implemented.
- **D-04:** Honor the **oracle-independence caveat** — verify transcribed ξ/Δτ/A-B/shadow-zone equations against the **AV 1106/07 rev.4 PDF page images**, not a research summary.

**Weather-input routes**
- **D-05:** Build **all three routes now** (MET-05 Route 1 + MET-06 Route 3 both in Phase 3) — no deferral.
- **D-06:** **Route 2** (FORCE-driven): surface wind `u@zu` / temperature-gradient `dt/dz` → A/B; drives up/downwind straight-road cases. Reads the harness case fields (`t0`, `u`, `φ`, `su`, `dt/dz`, `sdt/dz`, `Cv²`, `Ct²`, roughness).
- **D-07:** **Route 1** (weather classes): table of (A,B) pairs with occurrence probabilities → energy-weighted L_den-style combination; validation target = **Yearly Average** workbook (`Met. statistics` sheet).
- **D-08:** **Route 3** (Monin–Obukhov): reconstruct u(z), T(z) from surface met (cloud cover as stability proxy) and **least-squares fit A, B, C**. No weather API (v2 METX) — validate against **synthetic/derived surface-met + committed oracle**. Keep the LSQ A/B/C boundary clean for the v2 weather-import plug-in.

**Shadow zone**
- **D-09:** Implement the **full AV 1106/07 upward-refraction / shadow-zone attenuation model** — success criterion 1 requires shadow-zone cases to match. Do NOT rely solely on natural ray behavior + a documented limit.

**F_τ turbulence coherence**
- **D-10:** F_τ enters as an **additional factor in the already-built F chain** `F = Ff·FΔν·Fc·Fr·Fs` through the pre-built FΔν seam (`CoherenceInputs::f_delta_nu`, currently `1.0`) — Phase 3 drops in Δτ⁺ under the A⁺ profile without touching call sites. Feeds the two-channel readout unchanged; F_τ never overwrites phase in `H_coh`.
- **D-11:** Cv²/Ct² come from **FORCE-case fields** (`Cv2`, `Ct2`), Nord2000 defaults when absent. Validate by **property tests** (F_τ<1 under turbulence, monotonic, correct blend direction) — no fixed-value scipy oracle for the turbulence constants.

**Load-bearing conventions (do not violate)**
- **D-12:** Complex/phase contract — `H_coh: Complex<f64>` phase-preserving through the refraction operator too; turbulence-decorrelated energy stays in separate real `P_incoh`; `F→1 ⇒ P_incoh→0`.
- **D-13:** Time-convention quarantine — refraction math implemented **verbatim in Nord2000's e^{−jωt} convention inside `propagation/`**; single `.conj()` boundary stays at `transfer::nord_ratio_to_transfer` (grep gate: zero `.conj()` in `propagation/`); conjugation written as explicit `Complex::new(re, -im)`.
- **D-14:** Frequency framework — 105-point 1/12-octave grid; **compare by BAND INDEX, never nominal frequency**. CalcEqSSPGround's fL/fH interpolation evaluated natively at these points.
- **D-15:** Engine dependency quarantine — `envi-engine` stays `ndarray + num-complex + thiserror` only (`cargo tree` gate); all new I/O (oracle fixtures, weather-class tables, Route-3 met inputs) lives in `envi-harness`.
- **D-16:** Numerics house rules — `z₀` clamped ≥ 0.001 m; ξ singularity / homogeneous-atmosphere singularity clamped; Δτ via the cancellation-free `ΔR=(R₂²−R₁²)/(R₂+R₁)` identity, extended to the circular-ray case.

### Claude's Discretion
- Module layout under `propagation/` for the new refraction code (e.g. a `refraction/` submodule with profile / eq-linear-collapse / circular-ray / shadow-zone units), the exact ξ clamp threshold value, and the Route-3 LSQ solver choice — planner/researcher decide, subject to the conventions above.
- Exact synthetic surface-met fixtures and oracle-fixture granularity for Route 3.

### Deferred Ideas (OUT OF SCOPE)
- **Live weather-API ingestion** (Open-Meteo runtime, ERA5/CDS weather-class statistics) — v2 METX milestone. Phase 3 builds Route 3's fit machinery but drives it from synthetic/derived inputs.
- **Full FORCE road-traffic numeric pass** (VAL-02) — Phase 4, needs the road-emission model. Phase 3's refraction FORCE cases stay capability-gated.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ENG-05 | Refraction via equivalent-linear profile (circular-ray ξ, Δτ) with guarded numerics | Full AV chain transcribed: CalcEqSSP Eqs. 15–21, DirectRay Eqs. 29–44 (ξ/Δτ/dSZ), ReflectedRay Eqs. 45–50, TravelTimeDiff Eqs. 51–53 incl. Δτ₀ Eq. 52; clamps `z<0.01`, `|ξ|<10⁻¹⁰`, `ξ<10⁻⁶→homogeneous`; cancellation-free ΔR carried from Phase 2 rays.rs |
| ENG-06 | Reflection paths with separate before/after profile coefficients (A₁/B₁, A₂/B₂) | §5.3.4 reflection-path variables (A₁,B₁ before / A₂,B₂ after reflection point, d₁,d₂, reflector geometry); CalcEqSSP applied per sub-path (§5.5.2 "separate equivalent profiles … between source and screen, receiver and screen"); Lr reflection-efficiency correction Eq. 1 |
| ENG-08 | F_τ turbulence coherence (Cv², CT²) blending coherent/partial | F_τ = FΔν Eq. 112 (sinc in `x=2π·f·(Δτ⁺−Δτ)`, A⁺=A+1.7sA/B⁺=B+1.7sB Eq. 10) via existing `f_delta_nu` seam; Fc Eq. 113 already in `coherence.rs`; Eq. 118 confirms `F=Ff·Fτ·Fc·Fr·Fs`; two-channel readout unchanged |
| MET-01 | Log-lin profile `c(z)=A·ln(z/z₀+1)+B·z+C`, z₀≥0.001 | Eq. 2 (verbatim), C=Coft(t₀) Eq. 3, z₀ clamp 0.001 m (§5.3.2 p.16) |
| MET-02 | Derive A per azimuth (wind `u·cosφ`) + B from stability (inversion→B>0); isotropic temp once + projected wind per bearing | AV takes A/B as **input** (not derived); per-azimuth projection convention documented in `geometry.rs` (`A ∝ u·cos(az−φ_u)`, clockwise-from-north). **Derivation formulas are NOT in AV** — see Assumptions A1/A2, Open Q1 |
| MET-03 | CalcEqSSP collapse (∂c/∂z averaged between hS,hR) | Eqs. 15–21 verbatim; Eq. 18 average gradient; Eq. 19 + Annex F Eq. 403 closed-form c̄; hmin=5z₀; hS=hR fallback (±0.005 m); ξ<10⁻⁶ homogeneous shortcut |
| MET-04 | Frequency-dependent ground variant (CalcEqSSPGround) fL/fH log-interpolation at 1/12-oct | Eqs. 22–28 verbatim; Eq. 23 gradient interpolation, Eqs. 24–27 fL/fH via PhaseDiffFreq (already built §5.23.14), soft-ground trigger σ<10⁷ Pa·s·m⁻², clamps d≤400 m / hS,hR≥0.5 m |
| MET-05 | Route 1 weather-class table (A,B)+probabilities → L_den energy-weighting | TestYearlyAverage.xls `Met. statistics` sheet (harness I/O); AV refs "[4]" for classes. **Class table values NOT in AV** — Assumption A1, Open Q1 |
| MET-06 | Route 3 Monin–Obukhov reconstruct u(z),T(z) + LSQ A,B,C | Standard MO similarity + effective-c profile fit. **NOT in AV** — Assumption A2, Open Q1; validated against synthetic met + oracle |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

- **English only** in all code/comments/docs/commits.
- **Rust edition 2024**; native-Rust crates preferred; `f64` throughout; guard catastrophic cancellation (Δτ); clamp ξ singularities; `z₀ ≥ 0.001 m`.
- **Engine dep quarantine:** `envi-engine` = `ndarray + num-complex + thiserror` only, enforced by a `cargo tree -p envi-engine` check. All I/O in `envi-harness`.
- **Single `.conj()` boundary:** zero `.conj()` in `propagation/` (grep gate); the one conjugation is `transfer::nord_ratio_to_transfer`. Write conjugation as explicit `Complex::new(re, -im)`.
- **Compare by BAND INDEX**, never nominal frequency.
- **Quality gates before "done":** `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `cargo test`, `#![deny(unsafe_code)]` on `envi-engine` (`unsafe` only at FFI, none here).
- **No CI workflows**; builds operator-driven.
- **Licensing:** never commit `refs/` PDFs/.xls; cite by report + equation number; port ideas not GPL source.
- **Phase-completion gates (MANDATORY, auto-fix):** `/gsd-code-review <3> --fix`, `/gsd-secure-phase 3`, `/gsd-verify 3`, documentation-consistency scan across `.planning/`, README update.
- Session start: `git pull --ff-only origin main`; read `.planning/STATE.md`.

## Architectural Responsibility Map

Pure computation engine — "tiers" are library layers; the load-bearing boundary is engine (math, Nord2000-native) vs harness (all I/O, weather routes).

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Log-lin profile `c(z)` + `C=Coft(t₀)` | `envi-engine::propagation::refraction::profile` | — | Pure eval of Eq. 2/3; z₀ clamp; consumed by CalcEqSSP |
| Equivalent-linear collapse CalcEqSSP `(ξ,c₀)` | `refraction::eqssp` | `profile` | Eqs. 15–21 + Annex F closed-form; frequency-independent |
| Frequency-dependent CalcEqSSPGround `(ξ(f),c₀(f))` | `refraction::eqssp` | `ground` (Ẑ_G for PhaseDiffFreq), `fresnel::PhaseDiffFreq` | Eqs. 22–28; soft-ground only; native 1/12-oct |
| Circular ray variables DirectRay/ReflectedRay | `refraction::circular_ray` (fills `rays::RayVars`/`RayPair`) | `geometry` | Eqs. 29–50; the D-02 seam; cubic reflection point Eq. 49 |
| Travel-time difference Δτ, Δτ⁺, Δτ₀ | `refraction::circular_ray` → `rays::RayPair.dtau` | — | Eqs. 51–53; cancellation-free; shadow-edge Δτ₀ |
| Shadow-zone shielding (equivalent wedge) | `refraction::shadow_zone` | `diffraction` (reuses pwedge0/Dwedge), `refraction::circular_ray` (hSZ) | §5.23.16 Eqs. 384–388; couples upward refraction into wedge kernel |
| Ray curvature / ray height (scatter+reflect efficiency) | `refraction::circular_ray::ray_curvature` | — | Eqs. 54–56; only where ray *height* matters |
| F_τ (fluctuating refraction) | `coherence.rs` (existing `f_delta_nu` fed) | `refraction` (Δτ⁺) | Eq. 112; sinc(`2π·f·(Δτ⁺−Δτ)`); no call-site change |
| Sub-model 1/2 refraction wiring | `terrain_effect/{submodel1,submodel2,mod}` (existing) | `refraction` | Eqs. 115–124; swap straight `RayPair`→circular, ξ(f) into geometry |
| Reflection-path A₁/B₁/A₂/B₂ split | `envi-harness` (path setup) + `refraction::eqssp` (per sub-path) | — | §5.3.4; separate profiles before/after reflection point |
| Route 1 weather classes → L_den | `envi-harness::weather::route1` | `cases::xls` (Yearly Average) | I/O quarantine; energy-weighted class combination |
| Route 2 FORCE surface-met → A/B | `envi-harness::weather::route2` | `cases` (u/φ/dt-dz fields) | Reads parsed FORCE meteorology; produces (A,B,C) |
| Route 3 Monin–Obukhov + LSQ A/B/C | `envi-harness::weather::route3` | — | MO similarity → effective-c → LSQ fit; clean v2 boundary |
| Refraction oracle fixtures | `tools/nord2000_oracle/gen_refraction_fixtures.py` | — | scipy-committed ξ/Δτ/CalcEqSSP/CalcEqSSPGround; not a build dep |
| `Capability::Refraction` flip + FORCE requires-shrink | `envi-harness::capability` | — | Add to `implemented_capabilities()`; assert shrink |

**Boundary rule (carried, load-bearing):** all new refraction math is Nord2000-native (e^{−jωt}); it never calls `.conj()`. Weather routes are pure input-preparation (real-valued A/B/C/z₀) and live entirely in the harness.

## Standard Stack

### Core

**No new runtime dependencies.** The phase is pure math + harness I/O on the existing stack.

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `num-complex` | 0.4.6 (in workspace) | Ẑ_G in CalcEqSSPGround (PhaseDiffFreq), any complex ray-weighting | Already the engine's complex type [VERIFIED: workspace since 01-01] |
| `ndarray` | 0.17.x (in workspace) | unchanged | — |
| `thiserror` | 2.x (in workspace) | new `PropagationError` variants (shadow-zone, cubic no-root, degenerate profile) | Established typed-error posture |

### Supporting (harness / dev only)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `calamine` | (in harness) | read `TestYearlyAverage.xls` `Met. statistics` sheet for Route 1 | Route 1 class table + probabilities |
| `toml` / existing loader | workspace | load committed refraction oracle fixtures | fixtures are small tables; prefer TOML (no new dep) |
| `approx` | 0.5.x (dev-dep) | anchor/oracle assertions | unchanged |

### Route-3 LSQ solver (Claude's discretion — D-08)

The LSQ fit of `(A,B,C)` to a reconstructed effective-c profile is a **3-parameter linear least squares** (the model `c(z)=A·ln(z/z₀+1)+B·z+C` is *linear in A,B,C* once `z₀` is fixed). Recommend a **hand-rolled 3×3 normal-equations solve** (`XᵀX β = Xᵀy`, closed-form 3×3 inverse or Cramer) inside `envi-harness` — no linear-algebra crate needed, keeps the harness dep count flat. If a heavier fit is ever wanted, `nalgebra` is the standard choice, but it is **not justified** for a 3×3 system.

**Installation:** nothing to install.

## Package Legitimacy Audit

> No external packages are added in this phase. The engine dependency quarantine (`ndarray + num-complex + thiserror`) is preserved; all weather-route I/O reuses `calamine`/`toml` already vetted in Phases 1–2. Route-3 LSQ is hand-rolled (no `nalgebra`).

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| (none added) | — | — | — | — | — | — |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none
**If a future plan proposes `nalgebra`/`ndarray-linalg` for Route 3:** insert a `checkpoint:human-verify` before adding — it is unnecessary for a 3×3 system and would enlarge the harness surface.

## Verified Physics — Refraction Core (ENG-05, MET-01/03/04)

All equation numbers are AV 1106/07 rev.4; page numbers are the printed doc page. Verification tag **[PDF-TEXT: p.N]** = transcribed from the PDF page text this session (symbols/superscripts partly lost to extraction — **executor must eyeball the page image and confirm every exponent/sign before coding**, per D-04). This is the single most error-prone phase: a mistranscribed exponent still yields smooth, plausible curves.

### 1. Sound-speed profile (Eq. 2, 3) [PDF-TEXT: p.16]

```
c(z) = A·ln(z/z₀ + 1) + B·z + C          (2)
C = Coft(t₀)                             (3)   sound speed at ground from temperature
```
- `A` (m/s) = coefficient of the **logarithmic** part; `B` (s⁻¹) = coefficient of the **linear** part; `C` (m/s) = ground sound speed.
- `z₀` (roughness length) **clamped ≥ 0.001 m** to avoid ray-calc singularities (§5.3.2 explicit).
- `Coft` is the existing `sound_speed_ms(t)` primitive (Eq. 335, `c=20.05·√(t+273.15)`; the frozen Phase-1 constant — reuse it, do not reintroduce 340.29).
- Segmented terrain: `z` is the perpendicular (slant) distance to each segment.

### 2. Upper-refraction (fluctuation) profile (Eq. 10) [PDF-TEXT: p.20]

```
A⁺ = A + 1.7·sA          B⁺ = B + 1.7·sB          (10)
```
- Used only when `sA>0` or `sB>0` (fluctuating refraction). `Δτ⁺` is computed by running the whole ray chain on the `(A⁺,B⁺)` profile; `F_τ` (Eq. 112) uses `Δτ⁺−Δτ`.
- **Instantaneous levels ⇒ sA=sB=0** (§5.3.2). The FORCE straight-road cases carry `su=0.5` (⇒ nonzero sA-equivalent) on the mixed/screen groups — the same value Phase 2 noted; F_τ becomes active there.

### 3. CalcEqSSP — equivalent-linear collapse (Eqs. 15–21, Annex F Eq. 403) [PDF-TEXT: p.23–24, p.177]

```
ce(z) = c₀ + (∂c/∂z)·z                                    (15)
ce(z) = c₀·(1 + ξ·z)                                      (16)
ξ = (∂c/∂z)/c₀                                            (17)   relative gradient
∂c/∂z = (c(hR) − c(hS))/(hR − hS)                         (18)   AVERAGE gradient, hS→hR
c̄ = (1/(hR−hS))·∫[hS..hR] c(z) dz                         (19)   Annex F closed form
c₀ = c̄ − (∂c/∂z)·(hS + hR)/2                              (20)
(ξ, c₀) = CalcEqSSP(hS, hR, z₀, A, B, C)                  (21)
```
**Annex F closed form (Eq. 403 — use this, NOT numerical integration):**
```
∫ [A·ln(z/z₀+1) + B·z + C] dz
   = A·[ (z+z₀)·ln(z/z₀+1) − z ]  +  B·z²/2  +  C·z
c̄ = { A·[ (hR+z₀)ln(hR/z₀+1) − hR − (hS+z₀)ln(hS/z₀+1) + hS ]
      + B·(hR²−hS²)/2 + C·(hR−hS) } / (hR − hS)
   = A·[ (hR+z₀)ln(hR/z₀+1) − (hS+z₀)ln(hS/z₀+1) − (hR−hS) ]/(hR−hS)
      + B·(hR+hS)/2 + C
```
**Guards (all load-bearing, D-16):**
- `hS` or `hR` **not less than `hmin = 5·z₀`** when using Eqs. 18–20.
- **`hS = hR` case:** use the gradient at the height by applying Eq. 18 with modified heights `hS−0.005 m` and `hR+0.005 m`.
- **`|ξ| < 10⁻⁶` ⇒ set `ξ=0` and `c̄=c₀=C`** (the homogeneous shortcut). **This is the D-02 bit-for-bit anchor threshold** — below it, CalcEqSSP returns exactly the homogeneous profile and the straight-ray path runs unchanged. Use `10⁻⁶` as the ξ clamp threshold for the exactness anchor (Claude's-discretion value is now pinned to the document).

### 4. CalcEqSSPGround — frequency-dependent variant (Eqs. 22–28) [PDF-TEXT: p.24–25]

For a **soft** surface (`σ < 10⁷ Pa·s·m⁻²` = 10⁴ kPa·s·m⁻²; hard surfaces skip this and use plain CalcEqSSP, frequency-independent):
```
Above fH:  (ξ(f), c₀(f)) = CalcEqSSP                       (frequency-independent gradient)
Below fL:  ∂c/∂z assumed 0  ⇒ ξ=0
fL..fH:    ∂c'/∂z = (log f − log fL)/(log fH − log fL) · ∂c/∂z     (23)   log-interpolated gradient
           → feed ∂c'/∂z into Eqs. 15–20 to get ξ(f), c₀(f)
```
`fL`, `fH` from `PhaseDiffFreq` (the §5.23.14 aux already built in Phase 2) at phase differences Ψ and 2Ψ:
```
fΨ  = PhaseDiffFreq(d, hS, hR, Ẑ_G(f), c₀, Ψ)             (24)
f2Ψ = PhaseDiffFreq(d, hS, hR, Ẑ_G(f), c₀, 2Ψ)            (25)
   with d clamped ≤ 400 m and hS,hR clamped ≥ 0.5 m for Eqs. 24/25
fL  from Eq. (26)  — piecewise in Δc₁₀ = c(10)−c(0); constants ~343, 40.51 [EXECUTOR: confirm on p.26 image]
fH  = max(f2Ψ, 1.25·fL)                                    (27)
(ξ(f), c₀(f)) = CalcEqSSPGround(hS,hR,Ẑ_G(f),z₀,A,B,C)     (28)
```
- **Native 1/12-octave (D-14):** evaluate at all 105 grid points; `Ẑ_G(f)` is the existing `ground_impedance`. **Compare by band index.**
- The fL/fH machinery **mirrors Sub-model 2's PhaseDiffFreq usage** — reuse the Phase-2 `fresnel::PhaseDiffFreq` verbatim; only the target phase (Ψ vs the Sub-model-2 targets) differs. Confirm the `fL≤0.8·fH`-style clamp analog (Eq. 27 uses `max(f2Ψ,1.25·fL)`).

### 5. DirectRay — circular-ray variables (Eqs. 29–44) [PDF-TEXT: p.26–29]

Ray from lower point `L` to upper point `U` (either can be source/receiver/screen-top). `z'=z−zL`, `Δz=zU−zL`, `d`=horizontal distance.
```
ce(z') = c₀·(1 + ξ·z')                                    (29)
ξ = (∂c/∂z)/c(zL)                                          (30)
c₀ = c(zL)                                                 (31)
tan αL = (Δz·(2 + ξ·Δz)) / (… d …)                        (32)  [EXECUTOR: confirm form on p.27]
dm = d / tan αL   (horizontal dist L→circle top)          (33)
```
If `ξ>0` and `d ≤ dm`:
```
R(z) = (1/(cos αL))·… arcsin[(1+ξz')cos αL] … form        (34)  travel distance
τ(z) = (1/(ξ·c₀))·(1/2)·ln( f(0)/f(z) )                    (35)  travel time
f(0) = (1 + sin αL)/(1 − sin αL)                           (36)
f(z) = (1 + √(1−(1+ξz')²cos²αL)) / (1 − √(1−(1+ξz')²cos²αL))  (37)
```
If `d > dm` (ray passed the circle top at height `zm`):
```
cos-relation for zm via Eq. (38)
R = 2·R(zm) − R(z)                                         (39)
τ = 2·τ(zm) − τ(z)                                         (40)
```
```
Δα = arctan(Δz/d) − αL                                    (41)   change in ray angle vs straight line
if ξ<0: run Eqs. 32–41 with |ξ|; then Δα(ξ<0) = −Δα(|ξ|)   (42)
(R, τ, Δα, dSZ) = DirectRay(d, hS, hR, ξ, c₀)              (44)   dSZ=∞ if ξ≥0
```
**Numerical guards (§5.5.4 p.29, all load-bearing):**
- **`Δz < 0.01 m` ⇒ use `0.01 m`** (avoids `z→0` blow-up in Eqs. 34–37).
- **`|ξ| < 10⁻¹⁰` ⇒ use `ξ=10⁻¹⁰`** (circular formulas undefined at ξ=0 — this is the *inner* clamp; the *outer* homogeneous shortcut is CalcEqSSP's `10⁻⁶`).
- **Shadow-zone distance (ξ<0):** `dSZ = (√(−2zL/ξ) + √(−2zU/ξ))`-form (Eq. 43) [EXECUTOR: confirm radicand signs on p.29]. Assume shadow propagation when `d > 0.95·dSZ`.

### 6. ReflectedRay + reflection-point cubic (Eqs. 45–50) [PDF-TEXT: p.29–31]

The reflected ray is computed by **running DirectRay twice** — on the half-path `P→S` and `P→R` from the reflection point `P`:
```
(RS, τS, …) = DirectRay(dS, 0, hS, ξ, c₀)                  (45)
(RR, τR, …) = DirectRay(dR, 0, hR, ξ, c₀)                  (46)
R = RS + RR,  τ = τS + τR
ψ_G = αL (same for both halves; the grazing angle)
ΔαS = 2·ψ_G + arctan((zS−zR)/dS) − arctan(zS/dS)-form      (47)  [EXECUTOR confirm]
ΔαR = …                                                    (48)
(R, RS, RR, τ, ψ_G, ΔαS, ΔαR, d_refl) = ReflectedRay(...)  (50)
```
**Reflection-point cubic (Eq. 49) — the one genuinely new solver:**
```
d_refl³ + b·d_refl² + (…)·d_refl + (…) = 0     cubic in d_refl
  bS = 2·zS/ξ-form,  bR = 2·zR/ξ-form           [EXECUTOR: transcribe exact coeffs p.31]
```
- **Downward refraction (ξ>0):** up to **3 real roots** for large ξ. Pick the reflection point **closest to the source if hS<hR, else closest to the receiver**.
- **Upward refraction (ξ<0):** exactly **one real root**.
- Recommend a **robust cubic solver** (trigonometric/Cardano with discriminant branch) returning all real roots, then apply the selection rule. Add a typed error `PropagationError::NoReflectionRoot` for the (shouldn't-happen) empty-root case.
- In the **homogeneous limit** the reflection point must coincide with the Phase-2 image-source intersection (`reflect_over_segment`) — a direct D-02 cross-check.

### 7. Travel-time difference Δτ (Eqs. 51–53) [PDF-TEXT: p.30–32]

```
Δτ = τ₂ − τ₁                                              (51)
```
- The document **itself** warns: `τ₂−τ₁` is "the difference between two numbers almost equal … use the highest possible precision." Extend the Phase-2 cancellation-free identity to circular rays: compute `ΔR` from the *difference of the circular arc lengths* without subtracting the near-equal absolutes where possible; where the circular formulas force a subtraction, carry it in the τ-domain via the `f(0)/f(z)` log-ratio (Eq. 35), which is already a *difference-of-logs* form (numerically stable).
- **Upward-refraction shadow-edge fix (Eq. 52, ξ<0):** at the edge of a meteorological shadow zone `Δτ` must be exactly 0. A maximum `Δτ₀` is computed:
```
Δτ₀ = (1/C)·( √(d² + (hS+hR)²) − √((d−…dSZ…)² + …) )-form   (52)  [EXECUTOR confirm p.32]
if Δτ > Δτ₀: use Δτ₀        (ensures Δτ→0 at the shadow-zone edge)
```
- **Receiver in shadow zone** (`d > 0.95·dSZ`): `Δτ` fixed at **0** (§5.5.6).
- `TravelTimeDiff(τ₁, τ₂) → Δτ` (Eq. 53) fills the existing `RayPair.dtau`.

### 8. RayCurvature / ray height (Eqs. 54–56) [PDF-TEXT: p.31–32]

Only needed where the *height* of the ray matters (scattering-zone efficiency, reflecting-surface efficiency — Sub-model 7 already exists; this refines it under refraction):
```
RA = A·sign(…)/(…)-form,  RB = C/(2·B)-form                (54)   radii of log & linear parts
1/RA,B = 1/RA + 1/RB
ξ_ray from RA,B and start angle αS                         (55)
R_A,B, ξ_ray = RayCurvature(A,B,C,ξ,d,hS,hR)               (56)
```
- Lower priority for the Phase-3 target set (FORCE flat cases); implement if the shadow-zone `hSZ` (below) or scattering-efficiency refinement needs it. `HeightOfCircularRay` (referenced by ShadowZoneShielding) is the consumer.

## Verified Physics — Shadow Zone (D-09, success criterion 1)

### 9. Sub-model 1 shadow-zone branch (Eqs. 121–122) [PDF-TEXT: p.55]

When the receiver is in a shadow zone (`ξ<0` and `d>0.95·dSZ`), the reflected ray is **not** computed and Sub-model 1 uses Eq. 121 instead of Eq. 120:
```
Lflat(shadow) = 10·lg( |(1 − F·Q̂(f,…))·…|²  … )  −  L_SZ(f)      (121)
```
- The coherent term collapses (reflected-ray factors → 0); the dominant new term is **`L_SZ`, the shadow-zone shielding** (an *attenuation*, subtracted).
- `L_SZ(f) = ShadowZoneShielding(f, d, hS, hR, ξ, c₀, dSZ)` (Eq. 122); `ξ, c₀` are the **frequency-independent** CalcEqSSP values here.

### 10. ShadowZoneShielding = equivalent wedge (Eqs. 384–388, §5.23.16) [PDF-TEXT: p.158–159]

**This is the "full AV shadow-zone model" D-09 requires — and it reuses the Phase-2 diffraction kernel.** The shadow-zone attenuation is modelled as diffraction over an **equivalent wedge** of height `hSZ` at horizontal distance `dSZ`:
```
ξSZ(f) = ξ · (log f − log 2000)/(log 20 − log 2000)-form   (385)   frequency-dependent gradient, f<2000
hSZ(f) = HeightOfCircularRay( … dSZ, ξSZ(f) … )            (386)   ray height at dSZ = wedge height
… equivalent-wedge diffraction using pwedge0/Dwedge with hSZ …     (387–388)  [EXECUTOR: transcribe p.159]
```
- **Reuse `diffraction::pwedge0`/`Dwedge`** (the non-reflecting single-term wedge kernel already built in Phase 2, Eqs. 105–107) with the equivalent wedge geometry — do not build a new diffraction primitive.
- `HeightOfCircularRay` is a thin consumer of the DirectRay circular geometry (Eqs. 32–40) — implement it alongside `circular_ray`.
- **Executor task:** pages 158–159 continue past Eq. 386 (Eqs. 387–388 define how `hSZ` maps to the wedge attenuation); transcribe them from the page image before coding — they were beyond the clean-text extraction this session. Confirm the `2000 Hz` freeze and the `20` constant in Eq. 385.

## Verified Physics — Coherence F_τ (ENG-08)

### 11. Fluctuating-refraction coherence FΔν (Eq. 112) [PDF-TEXT: p.52]

```
FΔν(f) = { 1              x = 0
           sin(x)/x       0 < x ≤ π          x = 2π·f·(Δτ⁺(f) − Δτ(f))      (112)
           0              x > π }
```
- **Exact same sinc form as Ff** (Eq. 111, already in `coherence_ff`) but the argument is `2π·f·(Δτ⁺−Δτ)` (note: **`2π`, not the `0.23π` of Ff** — Ff's `0.23` is the 1/3-octave-averaging constant; FΔν has no such factor). `Δτ` from the average `(A,B)` profile, `Δτ⁺` from `(A⁺,B⁺)` (Eq. 10).
- **Wiring (D-10):** compute `Δτ` and `Δτ⁺` in the refraction path, evaluate `FΔν`, and pass it as `CoherenceInputs::f_delta_nu`. **No call-site change** — `coherence_f` already multiplies `f_delta_nu` into the chain. Eq. 118 confirms `F = Ff·Fτ·Fc·Fr·Fs` for Sub-model 1 under refraction.
- **F→1 ⇒ P_incoh→0 (D-12):** when `sA=sB=0`, `A⁺=A, B⁺=B ⇒ Δτ⁺=Δτ ⇒ x=0 ⇒ FΔν=1` exactly. The bit-exact homogeneous behavior is preserved.

### 12. Fc / Fr already implemented [existing coherence.rs]

- **Fc** (Eq. 113, turbulence): already in `coherence_f` with `x = 5.888e-3·(CT²/(273.15+t)² + (22/3)·Cv²/c²)·f²·ρ^(5/3)·d`. Phase 3 does **not** change it; it simply becomes active on refraction cases carrying `Cv²/CT²>0`. The page-52 text this session **confirms** the `5.888…·10⁻³` constant, the `(22/3)` factor, the `ρ^(5/3)`, and the `·d` length (resolves Phase-2 Assumption A3).
- **Fr** (Eq. 114, roughness): the g(X) polynomial (Phase-2 Assumption A4) is now visible on p.53 — a rational in `X = k₀·r·sinψ_G` with constants `0.0266860 / 0.559880 / 0.1154480 / 0.0496960 …`. Roughness is 0 for the flat FORCE targets, but transcribe when elevated-road cases (Phase 3.5/4) arrive. **Executor: confirm the exact g(X) coefficients on the p.53 image.**
- **F_τ default (D-11):** `Cv²/CT²` from FORCE fields (`Cv2`,`Ct2`); Nord2000 defaults when a case omits them (the Phase-2 `zero_turbulence()` default of 0 is the "no turbulence" fallback; a nonzero Nord2000 default belongs in the harness case-loader, documented).

## Verified Physics — Reflection-Path Coefficients (ENG-06)

### 13. Separate before/after profiles (§5.3.4, Fig. 1, Eq. 1) [PDF-TEXT: p.17–18]

For a **reflection path** (off a vertical reflector), Eq. 1 adds `Lr` (reflection-efficiency correction) and the weather profile is **split at the reflection point**:
```
A₁, B₁ = weather coefficients BEFORE the reflection point (source→reflector)   (m/s, s⁻¹)
A₂, B₂ = weather coefficients AFTER  the reflection point (reflector→receiver)
d₁ = source→reflection-point distance,  d₂ = reflection-point→receiver distance
zr,upp / zr,low / dr,l / dr,r = reflector geometry;  φr = image-source↔reflector angle;  E = effective energy reflection coeff
```
- **Mechanism:** the same propagation model runs on each sub-path with its own `CalcEqSSP((A₁,B₁,C))` and `CalcEqSSP((A₂,B₂,C))`. §5.5.2 already states separate equivalent profiles are formed "between source and the nearest screen, between receiver and the nearest screen" — the reflection-path split is the same pattern with the reflector as the mid-point.
- **Per-azimuth A (MET-02):** each sub-path has its own bearing, so `A₁` and `A₂` get **different wind projections** `A ∝ u·cos(az−φ_u)` (the `geometry::azimuth_deg` clockwise-from-north consumer). The **isotropic temperature part of A is computed once**; the **projected wind part is added per bearing** (per-sub-path). B (stability/temperature-gradient) is bearing-independent (isotropic).
- **Scope note:** the FORCE straight-road Phase-3 targets are *direct* paths (up/downwind over ground), so the A₁/B₁/A₂/B₂ *reflector* split is exercised primarily by the reflection-path plumbing and property tests, not a FORCE numeric case in this phase. Build the split as a clean interface (`before`/`after` profile pair) so Phase 4 obstacle-reflection cases consume it.

## Weather-Input Routes (MET-02/05/06) — ⚠️ NOT in AV 1106/07

**Critical finding:** AV 1106/07 takes `A, B, C, sA, sB, z₀` as **inputs** and explicitly does **not** derive them from wind/temperature. §5.3.2 p.16: *"The weather coefficients A and B are determined from weather data available at normal weather stations. When calculating the yearly average of noise levels according to [4] A and B are directly used to describe the different meteorological classes in [4]."* The origin `docs/research.md` that CONTEXT.md cites for the route formulas **is not present in the repo** (verified: only `docs/references/dbaudio-*` exists). The route derivations below are therefore **structure-only / [ASSUMED]** and must be confirmed against the Nord2000 companion reference (AV 1851/00 Part 2 "Comprehensive Outdoor Sound Propagation Model, Part 2", or CNOSSOS-EU Annex meteo tables) **before their numeric fixtures are trusted** (see Open Q1). The *engine* is unaffected — it consumes A/B/C regardless of route.

### Route 2 — FORCE surface-met → A/B (MET-02, drives up/downwind cases) [ASSUMED]

- **Inputs (already parsed):** `u` (wind @ `zu`), `φ` (wind dir re north), `dt/dz` (temp gradient), `t₀`, `z₀`.
- **Structure:** the effective sound-speed profile is `c_eff(z) = c(T(z)) + u(z)·cos(az−φ)`. Fit `A·ln(z/z₀+1)+B·z` to `c_eff(z)−C`:
  - `A` (log part) ← the **logarithmic wind/temperature term** near the ground (`u(z)` follows a log law `u(z)=(u*/κ)·ln(z/z₀+1)` in neutral conditions ⇒ `A_wind ∝ (u*/κ)·cos(az−φ)`), plus the isotropic temperature contribution.
  - `B` (linear part) ← the **temperature gradient** `dt/dz` (inversion `dt/dz>0` ⇒ downward refraction ⇒ `B>0`, matching success criterion 2).
- **[ASSUMED]** exact `u* / κ / A-scaling` constants — confirm against companion ref. The *sign* logic (inversion→B>0, downwind→A>0) is physically certain.

### Route 1 — weather classes → energy-weighted L_den (MET-05) [ASSUMED for table values]

- **Source:** `TestYearlyAverage.xls` `Met. statistics` sheet — a table of meteorological classes, each with `(A, B)` and an **occurrence probability**.
- **Combination:** run the engine per class, combine receiver levels **energy-weighted by probability**, and assemble L_den (day/evening/night periods have their own class-probability distributions).
  ```
  L_den = 10·lg( Σ_class p_class · 10^(L_class/10) )     (energy-weighted; per period then day/eve/night penalties)
  ```
- **Harness-only (D-15):** the class table + probabilities load from the .xls in `envi-harness::weather::route1`; the engine just gets each `(A,B)`.
- **[ASSUMED]** the exact class→(A,B) values and the L_den period weighting — read from the `.xls` (authoritative) and cross-check against reference "[4]".

### Route 3 — Monin–Obukhov reconstruction + LSQ A/B/C (MET-06) [ASSUMED]

- **Inputs (synthetic/derived surface met — no live API, D-08):** surface wind, temperature, and **cloud cover as a stability proxy** (Pasquill-class-style mapping to Obukhov length L).
- **Reconstruct** `u(z)` and `T(z)` via MO similarity:
  ```
  u(z) = (u*/κ)·[ ln(z/z₀) − Ψm(z/L) ]          Ψm = stability correction
  T(z) = T₀ + (T*/κ)·[ ln(z/z₀) − Ψh(z/L) ] + Γd·z    (Γd dry adiabatic lapse)
  c_eff(z) = 20.05·√(T(z)+273.15) + u(z)·cos(az−φ)
  ```
- **LSQ fit** `(A,B,C)` to `c_eff(z)` sampled over the relevant height range (§Standard Stack: linear-in-(A,B,C) ⇒ 3×3 normal equations; `z₀` fixed from surface data).
- **Clean v2 boundary (D-08, specifics):** the fit consumes reconstructed `u(z)/T(z)` arrays — **agnostic to whether they came from a FORCE fixture or a live Open-Meteo/ERA5 API**. Shape the function as `fit_profile(heights, c_eff_samples, z₀) → (A,B,C)` so v2 METX plugs in without a rewrite.
- **[ASSUMED]** the MO stability functions `Ψm/Ψh`, the cloud→L mapping, and the sampling-height set — validate against a **committed synthetic oracle** (Route-3 gen script) with the *same* transcription (oracle-independence caveat D-04 applies).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| c̄ integral (Eq. 19) | Numerical quadrature | **Annex F Eq. 403 closed form** | Exact, fast, no tolerance to tune; the integral of a log-lin has a clean antiderivative |
| fL/fH phase-difference frequencies | New root finder | **existing `fresnel::PhaseDiffFreq`** (§5.23.14, Phase 2) | Already built + tested; CalcEqSSPGround uses the same aux |
| Shadow-zone attenuation | Bespoke shadow model | **equivalent-wedge diffraction via `diffraction::pwedge0`** (§5.23.16) | AV *defines* shadow shielding AS a wedge; reuse the Phase-2 kernel |
| Homogeneous ray limit | Special-casing | **CalcEqSSP `|ξ|<10⁻⁶ ⇒ ξ=0` shortcut** → existing `straight_rays` | The document's own analytic limit; the D-02 anchor rides on it |
| Route-3 LSQ | `nalgebra`/`ndarray-linalg` | **hand-rolled 3×3 normal equations** | Model linear in (A,B,C); a 3×3 solve needs no crate |
| Reflection point (homogeneous) | Re-derive | **existing `reflect_over_segment`** (cubic only for ξ≠0) | Homogeneous root = image-source intersection; cubic Eq. 49 only for refraction |
| Faddeeva / wedge / Fresnel fits | Re-implement | **Phase-2 `special`/`diffraction`/`ground`** | Frozen, oracle-pinned; refraction only changes the ray inputs |

**Key insight:** refraction in Nord2000 is a *transformation of the ray inputs* to already-built sub-models, not a new propagation model. The whole phase is (a) four new aux functions producing `(ξ, c₀, RayPair, Δτ, Δτ⁺, L_SZ)`, and (b) feeding them into the existing `submodel1`/`terrain_effect`/`coherence` seams. The danger is *silently plausible wrongness* — a mistranscribed exponent in Eqs. 32–43 yields smooth curves that pass eyeball review; the layered oracle+anchor ladder is the only defense.

## Architecture Patterns

### System Architecture Diagram

```
 Weather inputs (harness)                    envi-engine::propagation (extended)
 ┌───────────────────────────┐
 │ Route 1: class table+prob │ (TestYearlyAverage.xls Met.statistics)
 │ Route 2: FORCE u/φ/dt-dz  │──┐
 │ Route 3: MO + LSQ fit     │  │  produce (A, B, C, sA, sB, z₀) per path/bearing
 └───────────────────────────┘  │  (per-azimuth A: isotropic temp once + projected wind)
                                 ▼
   ┌─────────────────────────────────────────────────────────────────────────┐
   │ refraction/  (NEW, Nord2000-native e^{−jωt})                            │
   │   profile        c(z)=A·ln(z/z₀+1)+B·z+C  (Eq.2, z₀≥0.001)              │
   │   eqssp          CalcEqSSP (ξ,c₀) Eqs.15–21 + Annex F  ── |ξ|<1e-6→hom.  │
   │                  CalcEqSSPGround (ξ(f),c₀(f)) Eqs.22–28 ◄─ PhaseDiffFreq │
   │   circular_ray   DirectRay Eqs.29–44, ReflectedRay Eqs.45–50            │
   │                  reflection cubic Eq.49, Δτ/Δτ⁺/Δτ₀ Eqs.51–53           │
   │                  (fills rays::RayVars/RayPair behind identical fields)   │
   │   shadow_zone    ShadowZoneShielding Eqs.384–388 ─► reuses diffraction   │
   └───────┬─────────────────────────────────────────────────────────────────┘
           │ RayPair(circular), ξ(f), Δτ, Δτ⁺
           ▼
   ┌─────────────────────────────────────────────────────────────────────────┐
   │ terrain_effect / submodel1 / submodel2  (EXISTING — swap straight→circular)│
   │   coherence::F = Ff·[FΔν=F_τ now]·Fc·Fr·Fs   (Eq.112 via f_delta_nu seam) │
   │   out: TerrainEffect { h_coh_factor, p_incoh, delta_l_db }  (unchanged)   │
   └───────────────┬─────────────────────────────────────────────────────────┘
                   ▼ conj() at ONE boundary (transfer::nord_ratio_to_transfer)
        H_coh (phase intact) + P_incoh  →  band_levels_db_two_channel
                   ▼
        harness: Capability::Refraction implemented; FORCE requires-list shrinks
        (drop 'refraction', keep 'emission-model'); oracle+anchor ladder
```

### Recommended Project Structure (delta from Phase 2 — Claude's discretion D)

```
crates/envi-engine/src/propagation/
├── rays.rs                     # EXTEND: add circular constructor filling RayVars/RayPair
├── coherence.rs                # EXTEND: compute F_τ helper (sinc, x=2π f (Δτ⁺−Δτ)); feed f_delta_nu
├── diffraction.rs              # (reused by shadow_zone — no change)
├── fresnel.rs                  # (PhaseDiffFreq reused by eqssp — no change)
├── terrain_effect/
│   ├── submodel1.rs            # EXTEND: shadow-zone branch (Eq.121), circular RayPair input
│   └── mod.rs                  # EXTEND: refraction dispatch (ξ(f) per band, up/down/shadow)
└── refraction/                 # NEW submodule
    ├── mod.rs                  # re-exports; PropagationError variants
    ├── profile.rs              # Eq.2/3 c(z), z₀ clamp
    ├── eqssp.rs                # CalcEqSSP (Eqs.15–21+AnnexF), CalcEqSSPGround (Eqs.22–28)
    ├── circular_ray.rs         # DirectRay/ReflectedRay/cubic/Δτ/Δτ⁺/Δτ₀/RayCurvature/HeightOfCircularRay
    └── shadow_zone.rs          # ShadowZoneShielding (Eqs.384–388) via pwedge0

crates/envi-harness/src/
├── weather/                    # NEW: all route I/O (I/O quarantine D-15)
│   ├── mod.rs                  # (A,B,C,sA,sB,z₀) profile struct; per-azimuth A projection
│   ├── route1.rs               # weather-class table + energy-weighted L_den
│   ├── route2.rs               # FORCE u/φ/dt-dz → A/B
│   └── route3.rs               # Monin–Obukhov reconstruct + LSQ A/B/C fit
├── capability.rs               # EXTEND: Refraction in implemented_capabilities(); assert shrink
└── cases/                      # EXTEND: nonzero-turbulence default; up/downwind case wiring

tools/nord2000_oracle/
├── gen_refraction_fixtures.py  # NEW: ξ/Δτ/CalcEqSSP/CalcEqSSPGround/shadow-zone pinned fixtures
└── (existing gen_*.py reused for the homogeneous cross-check)

cases/
└── refraction_*.toml           # NEW: oracle-pinned + property-test cases (downwind/upwind/shadow)
```

### Pattern: circular-ray constructor behind the frozen seam (D-02)

```rust
// rays.rs — Source: AV 1106/07 §5.5.4–5.5.6 (Eqs. 29–53)
/// Circular-ray variables for a refracting atmosphere. Fills the SAME RayVars/
/// RayPair fields as `straight_rays`; when |ξ|<1e-6 CalcEqSSP returns ξ=0 and
/// this must produce values bit-for-bit identical to `straight_rays` (D-02).
pub fn circular_rays(d: f64, h_s: f64, h_r: f64, xi: f64, c0: f64)
    -> Result<RayPair, PropagationError> {
    if xi.abs() < 1e-6 {                 // homogeneous shortcut → exact Phase-2 path
        return straight_rays(d, h_s, h_r, c0);
    }
    // DirectRay (Eqs. 29–44) for direct; ReflectedRay (Eqs. 45–50) via cubic Eq.49;
    // Δτ via TravelTimeDiff (Eqs. 51–53) with the Δτ₀ shadow-edge cap (Eq.52).
    // Guards: Δz<0.01→0.01; |ξ|<1e-10→1e-10; dSZ, 0.95·dSZ shadow test.
    ...
}
```

### Anti-Patterns to Avoid
- **Numerically integrating Eq. 19** — use Annex F Eq. 403 closed form.
- **Introducing `.conj()` in `refraction/`** — the grep gate is zero; write conjugation as `Complex::new(re,-im)` (there is little complex work here anyway — most refraction math is real).
- **A separate homogeneous code path that "should" match** — route the homogeneous limit through the *same* `circular_rays` entry with the `|ξ|<1e-6` shortcut delegating to `straight_rays`, so D-02 is structural, not a parallel reimplementation.
- **Trusting the extracted equation superscripts** — every exponent/sign in Eqs. 32–43, 49, 52, 385 must be confirmed on the PDF page image (D-04); the text extraction dropped superscripts.
- **Pinning F_τ to an oracle value** (D-11) — property-test direction only; the turbulence constants are AV Assumptions.
- **Deriving A/B numeric values from training knowledge and presenting them as fact** — route formulas are `[ASSUMED]`; gate their fixtures behind user confirmation of the companion reference.

## Runtime State Inventory

Not applicable — Phase 3 is a **greenfield engine feature** (new code + new tests). No rename/refactor/migration; no stored data, live-service config, OS-registered state, secrets, or build artifacts carry a renamed string. (Verified: this phase adds modules and fixtures, it does not rename existing ones.)

## Common Pitfalls

### Pitfall 1: ξ clamp confusion (two different thresholds)
**What goes wrong:** using `10⁻¹⁰` as the homogeneous-shortcut threshold, so the circular formulas run with a tiny-but-nonzero ξ and drift from the straight-ray anchor by ~1e-9 dB — the D-02 bit-for-bit test fails.
**Root cause:** AV has **two** clamps: `|ξ|<10⁻⁶ ⇒ ξ=0` (CalcEqSSP, the *homogeneous* shortcut) and `|ξ|<10⁻¹⁰ ⇒ ξ=10⁻¹⁰` (DirectRay, the *inner* division guard).
**Avoid:** delegate to `straight_rays` at the `10⁻⁶` boundary (never enter the circular formulas for near-homogeneous cases). Use `10⁻¹⁰` only inside DirectRay for genuinely-refracting cases.
**Warning signs:** D-02 anchor off by 1e-9…1e-7 dB uniformly.

### Pitfall 2: Δτ cancellation in the circular case
**What goes wrong:** `Δτ = τ₂−τ₁` computed as a naive subtraction of two near-equal circular travel times loses precision (worst at high source/receiver, long range) — exactly what the document warns about.
**Avoid:** carry the τ difference through the `f(0)/f(z)` log-ratio form (Eq. 35 is already a stable difference-of-logs); extend the Phase-2 cancellation-free `ΔR` reasoning to the arc-length difference. Add the `hS=0.01, hR=1.5, d=1000` regression from Phase 2, plus a high-source upward-refraction case near the shadow edge (where Eq. 52 `Δτ₀` must drive Δτ→0).
**Warning signs:** Δτ noise / non-monotonic interference at long range or near shadow.

### Pitfall 3: reflection cubic root selection
**What goes wrong:** picking the wrong real root of Eq. 49 (up to 3 in strong downward refraction) puts the reflection point in the wrong place → wrong ψ_G, R₂, Δτ.
**Avoid:** implement the selection rule verbatim — closest to source if `hS<hR`, else closest to receiver (downward); single root (upward). Cross-check the homogeneous root against `reflect_over_segment`.
**Warning signs:** dip frequency shifts vs the homogeneous case in the wrong direction; discontinuities as ξ crosses a root-count boundary.

### Pitfall 4: CalcEqSSPGround applied to hard ground
**What goes wrong:** running the frequency-dependent fL/fH machinery for hard surfaces (`σ≥10⁷`) — wasted work and wrong (hard ground uses the frequency-independent ξ).
**Avoid:** branch on `σ < 10⁷ Pa·s·m⁻²` (soft) exactly; hard ground uses plain CalcEqSSP.
**Warning signs:** ξ(f) varying with frequency over asphalt.

### Pitfall 5: F_τ argument constant (2π vs 0.23π)
**What goes wrong:** copying Ff's `0.23π` constant into FΔν; F_τ then under-decoheres.
**Avoid:** FΔν's `x = 2π·f·(Δτ⁺−Δτ)` — no `0.23`. Confirm on p.52.
**Warning signs:** F_τ property test shows too-weak decorrelation at high f·(Δτ⁺−Δτ).

### Pitfall 6: shadow zone reused wedge geometry
**What goes wrong:** building a bespoke shadow-attenuation curve instead of the equivalent-wedge model → fails the FORCE upward-refraction cases the ladder pins.
**Avoid:** implement ShadowZoneShielding (Eqs. 384–388) via `pwedge0` with `hSZ` from HeightOfCircularRay; freeze ξSZ above 2000 Hz (Eq. 385).
**Warning signs:** shadow-zone attenuation with no frequency structure, or discontinuity at `d=0.95·dSZ`.

### Pitfall 7: weather-route numbers presented as verified
**What goes wrong:** an executor codes A/B/C constants from training knowledge; they look plausible but are wrong because the companion reference wasn't consulted.
**Avoid:** treat all route derivation constants as `[ASSUMED]`; gate their oracle fixtures behind a `checkpoint:human-verify` and the user supplying AV 1851/00 Part 2 (or confirming the .xls-derived class table for Route 1).
**Warning signs:** Route fixtures with no cited source; L_den off by a constant offset.

## Code Examples

### F_τ helper (Source: AV 1106/07 Eq. 112, p.52)

```rust
// coherence.rs — Nord2000-native; feeds the existing f_delta_nu seam.
/// FΔν (Eq. 112): sinc coherence from fluctuating refraction.
/// `dtau` = Δτ under (A,B); `dtau_plus` = Δτ⁺ under (A⁺,B⁺)=(A+1.7sA,B+1.7sB).
#[must_use]
pub fn coherence_f_delta_nu(f_hz: f64, dtau: f64, dtau_plus: f64) -> f64 {
    let x = 2.0 * std::f64::consts::PI * f_hz * (dtau_plus - dtau).abs();
    if x <= 1e-15 { 1.0 }                       // sA=sB=0 ⇒ Δτ⁺=Δτ ⇒ 1.0 (D-12 exact)
    else if x <= std::f64::consts::PI { x.sin() / x }
    else { 0.0 }
}
```

### CalcEqSSP c̄ closed form (Source: Annex F Eq. 403, p.177)

```rust
// refraction/eqssp.rs
fn c_bar(a: f64, b: f64, c: f64, z0: f64, h_s: f64, h_r: f64) -> f64 {
    let l = |h: f64| (h + z0) * (h / z0 + 1.0).ln() - h;   // antiderivative of A·ln(z/z0+1)
    a * (l(h_r) - l(h_s)) / (h_r - h_s) + b * (h_r + h_s) / 2.0 + c
}
```

### Homogeneous-shortcut anchor test (Source: D-02, §5.5.2 p.24)

```rust
#[test]
fn circular_rays_reproduce_straight_rays_below_xi_clamp() {
    let c0 = sound_speed_ms(15.0);
    // ξ just below the 1e-6 homogeneous threshold ⇒ must equal straight_rays bit-for-bit.
    let hom = straight_rays(97.5, 0.5, 1.5, c0).unwrap();
    let circ = circular_rays(97.5, 0.5, 1.5, 5e-7, c0).unwrap();
    assert_eq!(hom, circ);      // PartialEq derive — exact, not approx
}
```

## State of the Art

| Old (Phase 2 assumption / stub) | Current (verified this session) | Impact |
|--------------------------------|--------------------------------|--------|
| FΔν = 1.0 stub (`f_delta_nu`) | FΔν = sinc(`2π·f·(Δτ⁺−Δτ)`), Eq. 112 confirmed p.52 | Drop the value into the existing seam; no call-site change |
| Fc constant "5.888e-3?" (Assumption A3) | `5.888·10⁻³`, `(22/3)`, `ρ^(5/3)`, `·d` all confirmed p.52 | Phase-2 Fc is correct as coded; no change needed |
| Fr g(X) polynomial unknown (Assumption A4) | rational in `k₀·r·sinψ_G`, constants visible p.53 | Transcribe for elevated-road cases; r=0 flat targets unaffected |
| Reflection cubic "nothing this phase" (Phase 2) | Eq. 49 cubic REQUIRED now for downward-refraction reflection point | New robust cubic solver + root-selection rule |
| Homogeneous via ξ-clamp 1e-10 (reference impls) | ENVI: `|ξ|<1e-6 ⇒ ξ=0` CalcEqSSP shortcut → exact straight-ray | D-02 bit-for-bit anchor is structural |
| "docs/research.md has the route formulas" | **research doc absent from repo**; AV does not derive A/B | Route formulas are `[ASSUMED]`; need companion ref |

**Deprecated/outdated:** none — the standard is frozen (rev.4, 2014). The 2018 amendment (118-22465) touches later items; spot-check its change list if fetched (carried Phase-2 Assumption A8).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Route 1/Route 2 A/B derivation formulas and weather-class (A,B)+probability table values (the origin `docs/research.md` is absent; AV does not specify them) | Weather Routes | HIGH for MET-05/02 fixtures — need AV 1851/00 Part 2 or the `.xls` class table + CNOSSOS meteo tables; engine core unaffected |
| A2 | Route 3 Monin–Obukhov stability functions Ψm/Ψh, cloud→Obukhov-length mapping, sampling heights | Weather Routes | MEDIUM — MET-06 numeric fixtures; validated against a same-transcription oracle only (caveat D-04) |
| A3 | Eq. 32 `tan αL` and Eq. 43 `dSZ` radicand exact forms (superscripts lost in text extraction) | Circular ray §5 | MEDIUM — confirm on p.27/p.29 images before coding; wrong form → wrong ray geometry |
| A4 | Eq. 49 cubic coefficients (`bS`, `bR`, and the `d_refl` polynomial terms) | Reflection cubic §6 | MEDIUM — confirm p.31 image; wrong coeffs → wrong reflection point |
| A5 | Eq. 52 `Δτ₀` reformulation exact form | Δτ §7 | LOW/MEDIUM — only bites high-source upward-refraction near shadow edge; confirm p.32 |
| A6 | ShadowZoneShielding Eqs. 387–388 (beyond clean extraction; hSZ→wedge-attenuation mapping) and Eq. 385 `2000 Hz` freeze / `20` constant | Shadow zone §10 | MEDIUM — the D-09 success criterion depends on it; transcribe pp.158–159 images |
| A7 | Eq. 26 `fL` piecewise constants (~343, 40.51) | CalcEqSSPGround §4 | LOW — soft-ground frequency blend; confirm p.26; oracle cross-check catches drift |
| A8 | Fr g(X) coefficients (Eq. 114 p.53) | Coherence §12 | LOW — r=0 for flat targets; needed for elevated-road (later) |
| A9 | Nord2000 default Cv²/CT² values when a FORCE case omits them (D-11) | F_τ §12 | LOW — property tests don't pin the constant; document the default in the loader |

**Empty-table check:** this table is non-empty — the weather-route derivations (A1/A2) in particular need user confirmation before their fixtures become locked decisions.

## Open Questions

1. **The origin research doc (`docs/research.md` / `nord2000_gis_research.md`) that CONTEXT.md cites for the A/B/C route derivations is not in the repo.**
   - What we know: AV 1106/07 takes A/B/C as inputs and does not derive them; the derivations are standard Nord2000 (AV 1851/00 Part 2) / CNOSSOS meteorology.
   - What's unclear: the exact scaling constants for Route 2 (u/dt-dz→A/B), the Route 1 class table (unless read straight from `TestYearlyAverage.xls`), and the Route 3 MO functions.
   - Recommendation: planner adds an early `checkpoint:human-verify` — ask the user to supply AV 1851/00 Part 2 (or confirm the `Met. statistics` sheet is authoritative for Route 1). Build the *machinery* (fit boundary, energy-weighting, projection) now with `[ASSUMED]` constants clearly quarantined; lock the constants after confirmation. The engine core (ENG-05/06/08, MET-01/03/04) proceeds regardless.

2. **Reflection-path A₁/B₁/A₂/B₂ has no FORCE numeric target in Phase 3** (the up/downwind straight-road cases are direct paths).
   - Recommendation: build the before/after profile-split interface + property tests (per-bearing A projection, energy correction Lr) now; defer the numeric obstacle-reflection case to Phase 4 where directional sources exercise it.

3. **Sub-model 3 (non-flat terrain) interaction** — Phase 2 deferred it as a typed-error stub; valley/elevated FORCE cases need it and it "constantly calls the refraction machinery."
   - Recommendation: keep Sub-model 3 out of Phase 3's *target* set (the refraction targets are flat up/downwind), but note that the refraction primitives this phase builds are its prerequisites. Flag whether Sub-model 3 becomes a Phase 3.5 insertion or folds into Phase 4 at plan review.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | build/test | ✓ (Phases 1–2) | 1.9x, edition 2024 | — |
| refs/AV1106-07-rev4.pdf | equation transcription (D-04) | ✓ verified this session | rev.4 (2014), SHA-pinned | Wayback via refs/fetch.sh |
| refs/TestStraightRoad.xls | up/downwind case geometries + met fields | ✓ (parsed Phase 1–2) | 2009 | mst.dk |
| refs/TestYearlyAverage.xls | Route 1 `Met. statistics` sheet | ✓ present (placeholder-loaded) | 2009 | mst.dk |
| Python 3.13 + scipy/numpy | oracle fixture generation (`tools/`) | ✓ (Phase 2) | 3.13 | fixtures committed; no Python at test time |
| AV 1851/00 Part 2 (companion) or CNOSSOS meteo tables | Route 2/3 derivation constants | ✗ **not in repo** | — | user supplies; `[ASSUMED]` constants until then |
| docs/research.md (origin meteo research) | route derivations per CONTEXT.md | ✗ **absent** (verified) | — | reconstruct from companion ref / user |

**Missing dependencies with no fallback:** none block the *engine* core. The route-derivation constants (A1/A2) block only MET-02/05/06 *numeric* fixtures — mitigate via the Open-Q1 checkpoint.

**Missing dependencies with fallback:** companion meteorology reference → user-supplied; route machinery built with quarantined `[ASSUMED]` constants meanwhile.

## Acceptance Ladder & Test Plan

> The Nyquist "Validation Architecture" section is intentionally omitted — `.planning/config.json` sets `workflow.nyquist_validation: false`. This leaner section is retained because D-01/D-02/D-03 make the oracle+anchor ladder the load-bearing acceptance gate for this phase; the planner needs the requirement→test mapping and Wave-0 gaps regardless of the Nyquist toggle.

**Ladder rungs (D-02):** (a) homogeneous-limit **bit-for-bit** anchor (`|ξ|<1e-6` circular ≡ straight, `PartialEq` exact); (b) committed **scipy oracle** (ξ/Δτ/CalcEqSSP/CalcEqSSPGround/shadow) at ≤ Phase-2-style tolerance; (c) **property tests** for direction (downwind→gain, upwind→shadow loss, monotonic). FORCE up/downwind cases stay **capability-gated** (D-03): drop `refraction`, keep `emission-model` — assert the requires-list shrink.

- Quick run (per task): `cargo test -p envi-engine refraction` (+ `clippy --all-targets -D warnings`, `fmt --check`).
- Full suite (per wave merge / phase gate): `cargo test` (engine + harness + `libtest-mimic` dynamic FORCE runner), plus `cargo tree -p envi-engine` unchanged and `.conj()` grep gate = 0 in `propagation/`.

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ENG-05 | `|ξ|<1e-6` circular ≡ straight bit-for-bit | unit (anchor) | `cargo test -p envi-engine circular_rays_reproduce_straight` | ❌ Wave 0 |
| ENG-05 | ξ/Δτ/CalcEqSSP match scipy oracle ≤ tol | oracle | `cargo test -p envi-harness refraction_oracle` | ❌ Wave 0 |
| ENG-05 | Δτ cancellation-safe (high-source long-range) | unit (regression) | `cargo test -p envi-engine dtau_circular_cancellation` | ❌ Wave 0 |
| ENG-05 | shadow: no blow-up, Δτ→0 at edge (Eq.52) | unit + sweep | `cargo test -p envi-engine shadow_zone` | ❌ Wave 0 |
| MET-01 | `c(z)` = Eq.2, z₀ clamp ≥0.001 | unit | `cargo test -p envi-engine profile` | ❌ Wave 0 |
| MET-02 | per-azimuth A (temp once + wind per bearing); inversion→B>0 | property | `cargo test -p envi-harness weather_route2` | ❌ Wave 0 |
| MET-03 | CalcEqSSP ∂c/∂z avg hS→hR vs oracle | oracle | `cargo test -p envi-harness calceqssp_oracle` | ❌ Wave 0 |
| MET-04 | CalcEqSSPGround fL/fH log-interp at band index | oracle | `cargo test -p envi-harness calceqsspground_oracle` | ❌ Wave 0 |
| MET-05 | Route 1 energy-weighted L_den from class table | unit | `cargo test -p envi-harness weather_route1` | ❌ Wave 0 |
| MET-06 | Route 3 MO reconstruct + LSQ A/B/C vs synthetic oracle | oracle | `cargo test -p envi-harness weather_route3` | ❌ Wave 0 |
| ENG-06 | before/after A₁B₁/A₂B₂ split; per-bearing projection | property | `cargo test -p envi-harness reflection_path_profiles` | ❌ Wave 0 |
| ENG-08 | F_τ<1 under turbulence, monotonic, correct blend dir; F_τ=1 when sA=sB=0 | property | `cargo test -p envi-engine coherence_f_delta_nu` | ❌ Wave 0 |
| (dir) | downwind→gain, upwind→shadow loss vs homogeneous | property | `cargo test -p envi-engine refraction_direction` | ❌ Wave 0 |
| (gate) | `Capability::Refraction` implemented; FORCE requires-list shrinks (drop `refraction`, keep `emission-model`) | unit | `cargo test -p envi-harness capability` | ⚠️ extend existing |
| (finiteness) | all 105 bands × all up/down/shadow geometries finite | sweep | `cargo test -p envi-engine refraction_finiteness_sweep` | ❌ Wave 0 |

### Wave 0 Gaps
- [ ] `tools/nord2000_oracle/gen_refraction_fixtures.py` — ξ, Δτ, CalcEqSSP, CalcEqSSPGround, shadow-zone (scipy; SHA-provenance)
- [ ] `cases/refraction_downwind_*.toml`, `refraction_upwind_shadow_*.toml` — oracle-pinned + property cases
- [ ] `crates/envi-engine/src/propagation/refraction/` module tree (profile/eqssp/circular_ray/shadow_zone)
- [ ] `crates/envi-harness/src/weather/` route modules + synthetic met fixtures
- [ ] extend `capability.rs` tests for the Refraction flip + FORCE requires-shrink assertion
- [ ] `checkpoint:human-verify` for the `[ASSUMED]` weather-route constants (Open Q1)

## Security Domain

> `security_enforcement` enabled (absent=enabled). Same posture as Phases 1–2: offline numeric library; no network/auth/persistence surface added. Route 3 explicitly uses **synthetic/derived** met (no live API — that's v2 METX), so no new network surface.

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | — |
| V3 Session Management | no | — |
| V4 Access Control | no | — |
| V5 Input Validation | yes (narrow) | Untrusted case/weather inputs: reject non-finite `A/B/C/z₀/u/dt-dz`, `z₀≤0` (clamp ≥0.001), `hR=hS` (use the ±0.005 fallback, not a divide-by-zero); every new fn returns a typed `PropagationError`/`CaseLoadError`, never a panic; cubic-solver no-root → typed error. Extends the T-01/T-02 posture. |
| V6 Cryptography | no | — |

### Known Threat Patterns for {offline numeric Rust engine + weather I/O}
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Crafted weather/profile causing Inf/NaN (numeric DoS) | DoS | finiteness sweep (all bands × up/down/shadow); ξ/Δz/dSZ clamps; Δτ₀ shadow-edge cap; cubic discriminant guard |
| `.xls`/TOML weather table malformed (Route 1) | Tampering/DoS | reuse Phase-1 `confine` path-guard + typed `CaseLoadError`; row/sheet caps; reject non-finite class probabilities; probabilities normalized-and-checked |
| Supply chain | Tampering | zero new dependencies; Route-3 LSQ hand-rolled (no linalg crate) |
| Copyright leak | compliance | equations cited by AV report + number only; class-table values are method-defined facts read from the (git-ignored) `.xls`; no PDF text/figures committed; oracle scripts carry formulas + citations, not document text |

## Sources

### Primary (HIGH confidence — read/verified this session)
- **AV 1106/07 rev.4** (`refs/AV1106-07-rev4.pdf`, SHA-pinned, 177 pp.): §5.2 Eq.1, §5.3.2 Eqs.2–3 (profile), §5.3.4 (reflection-path A₁/B₁/A₂/B₂), §5.4.2 Eq.10 (A⁺/B⁺), §5.4.4 Eqs.12–14, §5.5.1–5.5.7 Eqs.15–56 (CalcEqSSP/CalcEqSSPGround/DirectRay/ReflectedRay/cubic/Δτ/RayCurvature), §5.9 Eqs.110–114 (coherence, F_τ=Eq.112), §5.10 Eqs.115–123 (Sub-model 1 refraction + shadow branch Eqs.121–122), §5.23.16 Eqs.384–388 (ShadowZoneShielding), Annex F Eqs.401–403 (c̄ closed form). Verification: pypdf page-text extraction of pp.13–32, 49–55, 157–159, 175–177 this session (D-04: superscripts partly lost — flagged for page-image confirmation, Assumptions A3–A7).
- **Existing codebase seams** (read this session): `rays.rs` (RayVars/RayPair, cancellation-free ΔR), `coherence.rs` (F chain + `f_delta_nu` seam, Fc/Fr), `terrain_effect/mod.rs` + `transfer.rs` (two-channel readout, single conj boundary), `geometry.rs` (azimuth clockwise-from-north wind-projection consumer), `capability.rs` (Refraction gate), `cases/mod.rs` (parsed met fields), `tools/nord2000_oracle/gen_case_fixtures.py` (oracle pattern).

### Secondary (MEDIUM confidence)
- Phase 2 `02-RESEARCH.md` / SUMMARYs: RayVars seam design, coherence assumptions A3/A4 (now resolved), Fs/FΔν stubs, oracle+anchor ladder, tolerances.
- `refs/EnvProject1335-2010.pdf`: FORCE tolerances (1 dB band/overall) — carried.

### Tertiary (LOW confidence — flagged, need user/companion confirmation)
- Weather-route A/B/C derivation formulas (Routes 1/2/3): **not in AV 1106/07**; origin `docs/research.md` **absent from repo**. Structure from standard Nord2000/CNOSSOS meteorology (training knowledge) — `[ASSUMED]`, Assumptions A1/A2, Open Q1. Needs AV 1851/00 Part 2 or the CNOSSOS meteo-class tables.
- `refs/AV1849-00-part1.pdf`: supporting method reference (not re-read this session; Phase-2 HP attribution source).

## Metadata

**Confidence breakdown:**
- Refraction core (CalcEqSSP/CalcEqSSPGround/DirectRay/ReflectedRay/Δτ/shadow): HIGH structure / MEDIUM symbol-level — equations transcribed from PDF text; exponents/signs flagged for page-image confirmation (D-04, Assumptions A3–A7).
- F_τ (Eq.112) + Fc/Fr confirmation: HIGH — sinc form + constants read cleanly p.52–53; drops into the existing seam.
- Homogeneous-limit anchor (D-02): HIGH — `|ξ|<10⁻⁶` shortcut is document-explicit (p.24).
- Weather routes (MET-02/05/06 derivations): MEDIUM/LOW — not in AV, origin doc absent; machinery HIGH, constants `[ASSUMED]`.
- Rust integration / module plan / seams: HIGH — extends verified Phase-2 API; zero new engine deps.

**Research date:** 2026-07-08
**Valid until:** indefinitely for the physics (frozen standard rev.4); re-confirm the weather-route constants once the companion reference is supplied; re-check crates only if a dependency is proposed.
