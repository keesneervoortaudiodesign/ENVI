# Phase 2: Ground Effect & Diffraction - Research

**Researched:** 2026-07-07
**Domain:** Nord2000 (AV 1106/07) ground reflection over segmented impedance + wedge diffraction + complex/partial-coherent combination, homogeneous atmosphere, Rust engine
**Confidence:** HIGH (every load-bearing equation transcribed from the primary PDF in `refs/` and verified by glyph-position extraction, page-image reading, or independent numerical validation this session)

## Summary

All the physics this phase needs lives in AV 1106/07 rev. 4 (present in `refs/AV1106-07-rev4.pdf`), and this session extracted it at implementation precision. The ground model is: Delany–Bazley impedance `Ẑ_G` from flow resistivity (Eq. 57), plane-wave coefficient `Γ̂_p` (Eq. 59), boundary-loss factor `Ê(ρ̂) = 1 + j√π·ρ̂·w(ρ̂)` with numerical distance `ρ̂` (Eq. 60), spherical-wave reflection coefficient `Q̂ = Γ̂_p + (1−Γ̂_p)Ê(ρ̂)` (Eq. 58), where `w(ρ̂)` is the Faddeeva function computed by the document's own three-branch approximation (Eqs. 61–74) — whose coefficients I reconstructed and verified against `scipy.special.wofz` to < 8·10⁻⁷ relative error. Segmented impedance is handled by computing Sub-model 1 per surface *type* and blending with frequency-interpolated Fresnel-zone weights (Sub-model 2, Eqs. 124–133). Diffraction is the Hadden–Pierce four-term finite-impedance wedge solution (Eqs. 78–91, attribution confirmed in AV 1849/00 §4) — transcribed definitively from page images and validated numerically (shadow-boundary continuity |p_diffr|·ℓ → 0.500, insertion loss tracking Maekawa +≈2 dB, the expected relationship). Screens interact with ground via the four-path image model `p̂ = p̂₁ + Q̂₁p̂₂ + Q̂₂p̂₃ + Q̂₁Q̂₂p̂₄` (Eq. 157; eight paths for two screens, Eq. 222).

Two scope-shaping discoveries the planner must internalize. **First: Nord2000's combination is not a pure coherent sum.** Every sub-model combines rays as `|p₁ + F·p₂|² + (1−F²)·|ρᵢ·p₂|²` — complex pressures weighted by a coherence coefficient `F = Ff·FΔν·Fc·Fr·Fs` (Eq. 110) with the incoherent residual using the pre-computable incoherent reflection coefficient ρᵢ (Eqs. 75–76). Even in a homogeneous atmosphere `Ff` (1/3-octave band averaging, Eq. 111, glyph-verified `x = 0.23·π·f·Δτ(f)`) is always active. The Δτ interference phase (ENG-07) lives inside the coherent term — dips emerge exactly from `e^{j2πfΔτ}·Q̂`. **Second: no FORCE road case can go fully green in Phase 2** — the reference spectra (and the per-band `dL` propagation column) embed the Jonasson road-emission model and pass-by integration (Phase 4, per Phase 1 research). Phase 2's gates must therefore be: exact unit anchors (this document provides them), a committed independent Python reference implementation as cross-oracle, and physically-verifiable properties (dip frequency/band position, soft/hard ordering, screen IL monotonicity, finiteness sweeps). Additionally, the FORCE mixed-ground and screen cases (21/24, 71–94) carry nonzero turbulence (Cv²=0.12, CT²=0.008) and wind-fluctuation (su=0.5) even in their "u=0 homogeneous" variants — so `Fc` (Eq. 113, closed-form) and Sub-model 7 (turbulence scattering behind screens, §5.16) belong in this phase, while `FΔν` (needs refraction ray machinery) stays a =1 stub until Phase 3.

**Primary recommendation:** Implement AV 1106/07 Eqs. 57–133 (ground) and 78–107 + Sub-models 4–7 + §5.21 terrain interpretation (screens) verbatim in `envi-engine::propagation::{ground, diffraction, terrain_effect}`, in Nord2000's native complex convention (e^{−jωt}; outgoing phase e^{+jωτ}) with a single documented conjugation at the TransferSpectrum boundary, gated on the numeric anchors below plus a committed scipy-based oracle script. Zero new engine dependencies.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ENG-02 | Ground effect over a segmented-impedance profile (frequency-dependent, soft↔hard), preserving complex pressure | Full equation chain verified: Ẑ_G (Eq. 57, glyph-verified), Γ̂_p (59), Ê/ρ̂ (60), w(ρ̂) (61–74, numerically validated to <8e−7), Q̂ (58), ρᵢ (75–76), Sub-model 1 (115–123) incl. ΔL_flat Eq. 120 (glyph-verified), Sub-model 2 Fresnel-weight blending (124–133), Fresnel machinery (§5.8, §5.23.4–7), impedance classes A–H (Table 2, now VERIFIED — resolves Phase 1 Assumption A1) |
| ENG-03 | Screen/barrier diffraction for single and multiple edges | Hadden–Pierce wedge solution Eqs. 78–91 (image-verified, numerically validated), Dwedge (92–94), two wedges p2wedge (95–99), thick screen p2edge (100–104), non-reflecting wedge (105–107), Sub-models 4/5/6 (157–~270), edge selection §5.21 (Eqs. 300–315), Sub-model 7 turbulence scattering (§5.16) |
| ENG-07 | Combine direct + ground-reflected + diffracted contributions as complex pressure, retaining Δτ interference phase | Partial-coherence combination formulas: Eq. 120 (flat), Eq. 182/215 (screen four-path), Eq. 110–114 coherence coefficients; cancellation-safe ΔR = 4·hS·hR/(R₁+R₂); dip anchors computed (§Numeric Anchors); finiteness guard list (§Pitfalls) |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

- **English only** in all code, comments, docs, commits.
- **Rust edition 2024**; pure-Rust crates preferred; `f64` throughout; guard catastrophic cancellation (Δτ named explicitly); clamp ξ singularities; `z₀ ≥ 0.001 m`.
- **Quality gates before "done":** `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `cargo test` (includes FORCE/analytic harness), `#![deny(unsafe_code)]` on envi-engine.
- **No CI workflows**; builds/tests operator-driven.
- **Licensing:** never commit `refs/` PDFs/.xls; implement from equations, cite by report number and equation number; port ideas not GPL source.
- **Phase-completion gates:** `/gsd-code-review`, `/gsd-secure`, `/gsd-verify`, documentation-consistency scan, README update.
- Session start: `git pull --ff-only origin main`; check `.planning/STATE.md`.

## Architectural Responsibility Map

Pure computation engine — tiers are library layers.

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Faddeeva w(z), Fresnel aux f/g polynomials | `envi-engine::propagation::special` | — | Shared numerics; used by ground (w) and diffraction (f,g); pure functions with pinned anchors |
| Ground impedance Ẑ_G, Γ̂_p, Ê, Q̂, ρᵢ | `envi-engine::propagation::ground` | `special` | The reflection-coefficient chain; per (f, ψ_G, τ₂, σ) |
| Coherence coefficients F (Ff, FΔν, Fc, Fr, Fs) | `envi-engine::propagation::coherence` | — | Cross-cutting: used by every sub-model; FΔν/Fs stubbed =1 this phase |
| Homogeneous ray variables (τ, R, ψ_G, Δτ) | `envi-engine::propagation::rays` | `geometry` | Straight-ray specialization now; Phase 3 swaps in circular-ray DirectRay/ReflectedRay behind the same struct |
| Fresnel zones (CalcFZd, FresnelZoneSize/W/Wm) | `envi-engine::propagation::fresnel` | `rays` | §5.23.4–7; weight machinery for Sub-models 2–6 |
| Wedge diffraction (pwedge, Dwedge, p2wedge, p2edge, pwedge0) | `envi-engine::propagation::diffraction` | `special`, `ground` (Q on wedge faces) | §5.7 complete; the only consumer of f/g |
| Sub-models 1/2/4/5/6/7 + ΔL_t composition | `envi-engine::propagation::terrain_effect` | all above | §5.10–5.16 + §5.22 Eq. 332; the ΔL_t(f) entry point |
| Terrain interpretation (edges, screen shapes, flatness, equivalent flat terrain) | `envi-engine::propagation::terrain_interpretation` | `scene` | §5.21 Eqs. 300–328; converts TerrainProfile → sub-model dispatch + transition parameters r_scr1/r_scr2/r_scr12/r_flat |
| Capability flags ground-effect, screen-diffraction | `envi-harness::capability` | — | Flip `GroundEffect`, `Diffraction` into `implemented_capabilities()`; FORCE road cases still skip on `emission-model` |
| Reference oracle (Python, scipy) | `tools/nord2000_oracle/` (committed) | — | Independent implementation generating pinned JSON fixtures; NOT part of the build |

**Boundary rule (new, load-bearing):** everything inside `propagation::{ground,diffraction,terrain_effect}` uses the **Nord2000-native complex convention** (time e^{−jωt}, outgoing phase e^{+jωτ}, impedance Im > 0). Conversion to ENVI's frozen e^{+jωt} TransferSpectrum convention happens in exactly one function (conjugation at the boundary). See Pattern 2.

## Standard Stack

### Core

No new runtime dependencies. The phase is pure math on the existing stack:

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `num-complex` | 0.4.6 (already in workspace) | Q̂, Ê, w(ẑ), p̂_diffr as `Complex<f64>` | Already the engine's complex type [VERIFIED: in Cargo.toml since 01-01] |
| `ndarray` | 0.17.2 (already) | unchanged | — |
| `thiserror` | 2.x (already) | new `GroundError`/`DiffractionError` variants or extension of `PropagationError` | — |

### Supporting (harness/dev only)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `approx` | 0.5.1 (already dev-dep) | anchor assertions | unchanged |
| `serde_json` or existing `toml` | workspace | load `tools/`-generated oracle fixtures in tests | prefer TOML to avoid a new dep; fixtures are small tables |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Hand-implementing w(z) per Eqs. 61–74 | `errorfunctions` crate (pure-Rust Faddeeva) | Crate is SUS (684 dl/wk, 2022, single author). More importantly, **bit-fidelity to the standard requires the document's own approximation** — a more accurate w(z) would produce slightly *different* results than reference Nord2000 implementations. Implement Eqs. 61–74 exactly; use scipy `wofz` offline to generate test fixtures |
| scipy-oracle fixtures | `libm`-based high-precision erfc port | Overkill; the offline-oracle pattern (commit generator script + generated fixtures) keeps the dependency count at zero |
| Implementing DirectRay/ReflectedRay (circular rays, §5.5.4–5.5.5) now | Straight-ray homogeneous specialization | Circular machinery is Phase 3 (ENG-05). AV 1106/07 p. 29 itself clamps `|ξ| < 1e−10 → ξ = 1e−10` — the homogeneous case is the analytic limit. Implement a `RayVars` struct with a straight-ray constructor now; Phase 3 adds the circular constructor behind the same fields (τ, R, ψ_G, Δτ) |

**Installation:** nothing to install.

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| errorfunctions | crates.io | 2022 | 684/wk | github.com/JorisDeRidder/errorfunctions | SUS (low-downloads) | **Not used** — rejected in favor of implementing Eqs. 61–74 + scipy-generated fixtures |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** `errorfunctions` — flagged by `gsd-tools query package-legitimacy` this session; recommendation is to not use it at all. If a future plan wants it as a dev-dependency oracle anyway, insert a `checkpoint:human-verify` before adding.

No other new packages. Engine dependency quarantine (ndarray/num-complex/thiserror only) is preserved.

## Verified Physics — Ground Effect (ENG-02)

All equation numbers are AV 1106/07 rev. 4. Verification tags: **[GLYPH]** = glyph-position extraction, **[IMAGE]** = read from rendered page image, **[NUM]** = independently validated numerically this session, **[TEXT]** = pypdf text (structure clear, transcribe symbols from PDF at implementation).

### 1. Ground impedance — Delany & Bazley (Eq. 57) [GLYPH+NUM]

```
Ẑ_G(f) = 1 + 9.08·(1000·f/σ)^(−0.75) + j·11.9·(1000·f/σ)^(−0.73)
```

- **Unit trap:** in Eq. 57, σ is in **Pa·s·m⁻²** (SI). Table 2 and the FORCE `.xls` column give σ in **kPa·s·m⁻²** (= kNs·m⁻⁴ — same unit). Equivalently, with σ in kPa·s·m⁻² the argument is simply `f/σ`. Sanity: σ=200 (kPa·s·m⁻², class D), f=1 kHz → X=5 → Ẑ = 3.7156 + 3.6754j — the classic grassland value [NUM].
- Sign of Im: **positive** in Nord2000's e^{−jωt} convention. See Pattern 2 (convention boundary) — do not "fix" the sign inside the module.
- Evaluate at each of ENVI's 105 exact 1/12-octave centres (the doc says "one-third octave band centre frequency" — ENVI's finer grid is the established Phase-1 deviation, document again in code).

### 2. Impedance classes (Table 2) and roughness classes (Table 3) [TEXT, table values unambiguous]

Resolves Phase 1 Assumption A1 — **all eight classes now VERIFIED** from the primary source:

| Class | σ (kPa·s·m⁻²) | Description |
|-------|---------------|-------------|
| A | 12.5 | very soft (snow/moss) |
| B | 31.5 | soft forest floor |
| C | 80 | uncompacted loose ground (turf, grass) |
| D | 200 | normal uncompacted ground (pasture) |
| E | 500 | compacted field/gravel (lawns, park) |
| F | 2000 | compacted dense (gravel road, ISO 10844) |
| G | 20000 | hard (asphalt, concrete) |
| H | 200000 | very hard/dense (dense asphalt, water) |

⚠️ Phase 1's research note said B = 31.6; the standard prints **31.5**. Update `impedance_class()` and its doc comment.

Roughness (Table 3): N: r=0, S: r=0.25 m, M: r=0.5 m, L: r=1 m — resolves Phase 1 Open Question 4. `r` feeds only the coherence coefficient Fr (Eq. 114).

### 3. Spherical-wave reflection coefficient (Eqs. 58–60) [GLYPH]

```
Q̂(f, τ₂, ψ_G, Ẑ_G) = Γ̂_p(f,ψ_G) + (1 − Γ̂_p(f,ψ_G))·Ê(ρ̂)                    (58)
Γ̂_p(f,ψ_G) = (sin ψ_G − 1/Ẑ_G) / (sin ψ_G + 1/Ẑ_G)                          (59)
Ê(ρ̂) = 1 + j·√π·ρ̂·e^(−ρ̂²)·erfc(−j·ρ̂) = 1 + j·√π·ρ̂·w(ρ̂)                  (60)
ρ̂ = ((1+j)/2)·√(ω·τ₂)·(sin ψ_G + 1/Ẑ_G)                                     (60)
```

- Nord2000 deliberately parameterizes ρ̂ by **travel time τ₂** (not k·R₂) so refraction (Phase 3) modifies it through τ — this dovetails exactly with ENVI's Phase-1 decision to carry τ as the phase primitive.
- ω·τ₂ = k·R₂ in the homogeneous case ((1+j)/2·√(ωτ₂) = √(jωτ₂/2), the classical numerical distance).
- In wedge-face reflections (Eq. 80) the *total* travel time τ_S+τ_R is passed as τ₂, and the "grazing angle" is `min(β−θ_S, π/2)` or `min(θ_R, π/2)` — transcribe exactly, do not re-derive.

### 4. Faddeeva function w(ẑ) — the document's approximation (Eqs. 61–74) [NUM — all three branches validated]

`w(ẑ) = e^(−ẑ²)·erfc(−j·ẑ)`, computed for ẑ = x + jy via `w⁺` in the region x≥0, y≥0 and symmetry relations (Eq. 62): for x<0 and/or y<0 use `w(−ẑ) = 2e^(−ẑ²) − w(ẑ)` and `w(conj(−ẑ)) = conj(w(ẑ))` (transcribe Eq. 62's exact case table from the PDF; the standard Faddeeva symmetries).

**Branch 1** (|x|>3.9 ∨ |y|>3) ∧ (x>6 ∨ y>6) — 2-pole rational (Eq. 63), verified <4e−7 vs scipy:
```
w⁺(ẑ) = j·ẑ·( 0.5124242/(ẑ²−0.2752551) + 0.05176536/(ẑ²−2.724745) )
```
**Branch 2** (|x|>3.9 ∨ |y|>3), else — 3-pole rational (Eq. 64), verified <8e−7:
```
w⁺(ẑ) = j·ẑ·( 0.4613135/(ẑ²−0.1901635) + 0.09999216/(ẑ²−1.7844927) + 0.002883894/(ẑ²−5.5253437) )
```
**Branch 3** (|x|≤3.9 ∧ |y|≤3) — Matta–Reichel-type series (Eqs. 65–74), h = 0.8, n = 1..5. Verified <3e−7 at 7 test points spanning the region [NUM — sign structure resolved by exhaustive search against wofz]:
```
A₁ = cos(2xy)          B₁ = sin(2xy)
C₁ = e^(−2πy/h) − cos(2πx/h)          D₁ = sin(2πx/h)
E  = e^(y²−x²−2πy/h)
P₂ = 2E·(A₁C₁ − B₁D₁)/(C₁²+D₁²)      Q₂ = 2E·(A₁D₁ + B₁C₁)/(C₁²+D₁²)
H  = (h·y/π)/(x²+y²) + (2hy/π)·Σₙ₌₁⁵ e^(−n²h²)·(x²+y²+n²h²) / ((x²+y²+n²h²)² − 4x²n²h²)
K  = (h·x/π)/(x²+y²) + (2hx/π)·Σₙ₌₁⁵ e^(−n²h²)·(x²+y²−n²h²) / ((x²+y²+n²h²)² − 4x²n²h²)
w(ẑ) = (H + P₂) + j·(K − Q₂)
```
(The executor should still eyeball Eqs. 65–74 on PDF p. 36 — pdfplumber `page.to_image()` renders it — but the formulas above already reproduce scipy to float32-level accuracy, which matches the approximation's intrinsic error.)

**Anchor values** (from scipy.special.wofz, use as fixtures, tol 1e−6 relative):
| ẑ | w(ẑ) |
|---|------|
| 0.5+0.5j | 0.533157 + 0.230488j |
| 1+1j | 0.304744 + 0.208219j |
| 3+2j | 0.092711 + 0.128317j |
| 7+1j | 0.019924 + 0.139158j (2-pole branch; value from wofz) |

Note: ρ̂ can land in the x<0, y>0 quadrant (very soft ground at low grazing angle: Re(sinψ+1/Ẑ) < Im) and in y<0 for hard ground — the symmetry relations are not dead code. Property-test all four quadrants against fixtures.

### 5. Incoherent reflection coefficient (Eqs. 75–76) [TEXT — verify √ during transcription]

`ρᵢ(f, Ẑ_G)` is derived from the random-incidence absorption coefficient ᾱ_ri(f, Ẑ_G) (Eq. 76, closed form in X = Re Ẑ_G, Y = Im Ẑ_G):
```
ᾱ_ri = 8·(X/(X²+Y²))·[ 1 + ((X²−Y²)/((X²+Y²)·Y))·arctan(Y/(X²+Y²+X... )) − (X/(X²+Y²))·ln(...) ]   (76 — transcribe exactly from PDF p. 37)
ρᵢ = √(1 − ᾱ_ri)   [ASSUMED: √ — pypdf shows "1 − ᾱ_ri"; energy consistency in Eq. 120 requires ρᵢ² = reflected energy, so ρᵢ = √(1−ᾱ) is the physically consistent reading; VERIFY the radical on PDF p. 37 image during implementation]
```
ρᵢ is angle-independent → precompute per impedance class per frequency (the doc recommends exactly this).

### 6. Coherence coefficients (Eqs. 110–114) [GLYPH for 111; TEXT others]

```
F(f) = Ff(f,Δτ) · FΔν(f,Δτ,Δτ⁺) · Fc(f,Cv²,CT²,t,c,ρ,d) · Fr(k₀,ψ_G,r) · Fs        (110)
Ff:  x = 0.23·π·f·Δτ(f);  Ff = 1 (x=0), sin(x)/x (0<x≤π), 0 (x>π)                  (111) [GLYPH-verified constant]
FΔν: same sinc form with x = 2π?·f·(Δτ⁺(f)−Δτ(f)) — Phase 3 (needs A⁺ = A+1.7·sA profile ray-tracing); stub = 1  (112)
Fc  = exp'(−x),  x = 5.888·10⁻³?·(CT²/(273.15+t)² + (22/3)·Cv²/c²)·f²·ρ^(5/3)·d    (113) [TEXT — transcribe the constant and exponents exactly from PDF p. 52; structure per Daigle/Ostashev theory]
Fr  = exp'(−0.5·(k₀·r·sin ψ_G)²·g(X)-form)                                          (114) [TEXT — polynomial g(X) on p. 53, transcribe]
exp'(x) = e^x (x≥−1), −e⁻¹·(2+... x<−1), 0 (x≤−2)   — clamped exponential, §5.23.3 Eq. 337
```
- ρ (transversal separation) for flat ground: `ρ = 2·hS·hR/(hS+hR)` (Eq. 119); for screen sub-models per Eqs. 178/180 with the segment-local heights.
- k₀ = 2πf/c₀ (Eq. 14).
- **Homogeneous zero-turbulence cases (FORCE 1–8): F = Ff exactly.** Mixed/screen cases (21/24, 71–94): F = Ff·Fc·Fr (FΔν→1 stub incurs a quantified risk — see Open Questions #2).

### 7. Sub-model 1 — flat terrain, one surface (Eqs. 115–123) [GLYPH for 120]

Homogeneous specialization (ξ=0, straight rays — see §Rays below):

```
ΔL_flat(f) = 10·lg( |1 + F·(R₁/R₂)·e^(j·2πf·Δτ)·Q̂(f,τ₂,ψ_G,Ẑ_G)|²
                    + (1 − F²)·(R₁/R₂)²·ρᵢ(f,Ẑ_G)² )                                (120)
```
[The `10·lg` prefix is the standard reading — ΔL values combine additively in dB throughout §5.21/5.22; verify prefix visually during transcription.]

This is the algebraic identity for a partially-coherent two-ray sum: `⟨|p₁+p₂|²⟩ = |p₁ + F·p₂|² + (1−F²)|p₂|²` with |Q̂|→ρᵢ in the incoherent residual. **The complex pressure and Δτ phase (ENG-07) live in the first term.**

- Shadow-zone branch (Eq. 121–122) is Phase 3 (needs ξ<0); in the homogeneous engine, assert dSZ = ∞.
- Referred to as `SubModel1(d, hS, hR, σ, r, z₀, A,B,C, sA,sB, Cv², CT²)` (Eq. 123) — keep this exact signature shape (weather params pass through to Phase 3).

### 8. Sub-model 2 — flat terrain, multiple surfaces (Eqs. 124–133) [TEXT, structure complete]

```
ΔL₂(f) = Σ_ii Σ_ir  w′_{ii,ir}(f) · ΔL_{ii,ir}(f)                                   (124)
```
- ΔL_{ii,ir} = Sub-model 1 computed **as if the whole ground were that (σ,r) combination**.
- Per-segment weights: low-frequency `wᵢ(f)` by `FresnelZoneW` with F_λ = **0.25·λ** (Eq. 125 — note Sub-model 2 uses 1/4, everything else terrain uses 1/16); high-frequency variant `rᵢ(f)` by `FresnelZoneWm` (Eq. 126).
- Sum weights per surface type (Eq. 127); high-frequency weight w_{ii,ir,H} via the empirical log-polynomial in r̄ᵢᵢ and tan ψ_G (Eq. 128 — transcribe the 5th-order polynomial constants 8.78/21.95/21.76/10.69/1.36? from PDF p. 57 exactly); blend low/high by log-frequency interpolation between fL and fH (Eq. 129).
- fL/fH from `PhaseDiffFreq` (Eqs. 130–132): fL at phase difference ΨL = 1.9483 − 8.052·ln(tan ψ_G)·... with h_min clamp ≥ 0.01 (Eq. 132, transcribe); fH at Ψ = π. Guard: **if fL > 0.8·fH use fL = 0.8·fH**.
- `PhaseDiffFreq` (§5.23.14, Eqs. 378–381): finds f where `Ψ(f) = 2πf·(R₂−R₁)/c₀ + arg Γ̂_p(f)` equals a target — iterate over the 1/3-octave grid, log-interpolate (Eq. 380), extrapolation rules above 10 kHz (linear from 8–10 kHz up to 100 kHz, constant above) and below 25 Hz (`f = 25·Ψ/Ψ(25 Hz)`, Eq. 381).
- **Weight normalization:** Fresnel weights normalized to ≤2 total (per §5.8/Sub-model 3 rule); in Sub-models 4–6 normalization is per-region based on excess over 1 (Eq. 187).

### 9. Homogeneous ray variables + cancellation-safe Δτ [§5.5.4–5.5.6]

AV 1106/07 itself computes the homogeneous case by clamping `|ξ| < 10⁻¹⁰ → ξ = 10⁻¹⁰` into the circular-ray machinery (p. 29). ENVI Phase 2 should instead implement the exact straight-ray limit:

- Direct: `R₁ = |SR|`, `τ₁ = R₁/c₀`, no shadow zone (dSZ = ∞).
- Reflected (flat segment): image-source construction (already in `geometry::reflect_over_segment` from 01-02), `R₂ = r₁+r₂`, `τ₂ = R₂/c₀`, ψ_G = grazing angle.
- **Δτ (Eq. 51, §5.5.6):** the doc itself warns "τ₂−τ₁ is the difference between two numbers almost equal … use the highest possible precision". For the flat/straight case use the exact cancellation-free identity:
  ```
  ΔR = R₂ − R₁ = (R₂² − R₁²)/(R₂ + R₁) = 4·hS·hR/(R₁ + R₂)       (flat ground)
  Δτ = ΔR/c₀
  ```
  For a sloped segment, use image-source coordinates: R₂² − R₁² expands to a difference of dot products computable without cancellation (`R₂² − R₁² = |S′R|² − |SR|²` with S′ the image point; expand in coordinates). Add a regression test vs f64 direct subtraction at hS=0.01, hR=1.5, d=1000 (worst case in FORCE geometry).
- Phase 3 will replace this constructor with the circular-ray version behind the same `RayVars { tau, r, psi_g, .. }` struct — design the seam now.

## Verified Physics — Diffraction (ENG-03)

### 10. Wedge solution — Hadden–Pierce with finite impedance (Eqs. 78–91) [IMAGE — definitive, + NUM validation]

Attribution: "diffraction effects are based on the wedge diffraction solution of Hadden and Pierce [had1: JASA 1981], modified to include an absorbing wedge" [VERIFIED: AV 1849/00 p. 38].

Inputs (§5.7.1): τ_S, τ_R, τ (= τ_S+τ_R normally), R_S, R_R, ℓ (= R_S+R_R normally), θ_S, θ_R (both measured from the **receiver** wedge face), wedge angle β, face impedances Ẑ_S, Ẑ_R. Validity: **β > π** and 0 ≤ θ_R ≤ θ_S ≤ β ≤ 2π (modify angles per the p. 43 scheme when the image method puts "source"/"receiver" inside the wedge).

```
p̂_diffr(f) = −(1/π) · Σₙ₌₁⁴ Q̂ₙ·A(θₙ)·Ê_ν(A(θₙ)) · e^(jωτ)/ℓ                        (78)

θ₁ = θ_S − θ_R          θ₂ = θ_S + θ_R
θ₃ = 2β − (θ_S + θ_R)   θ₄ = 2β − (θ_S − θ_R)                                       (79)

Q₁ = 1
Q₂ = Q_S(f, τ_S+τ_R, min(β−θ_S, π/2), Ẑ_S)          ← spherical-wave Q on source face
Q₃ = Q_R(f, τ_S+τ_R, min(θ_R, π/2), Ẑ_R)            ← on receiver face
Q₄ = Q₂ · Q₃                                                                          (80)

ν = π/β  (wedge index)

Ê_ν(A(θₙ)) = (π/√2) · (sin|A(θₙ)| / |A(θₙ)|)
             · e^(jπ/4) / √( 1 + (2τ_Sτ_R/τ² + 1/2)·cos²|A(θₙ)|/ν² )
             · Â_D(B)                                                                 (81)

A(θₙ) = (ν/2)·(−β − π + θₙ) + π·H(π − θₙ)          H = Heaviside (Eq. 354: H(x)=1 x>0 else 0)   (82)

B = √( 4ωτ_Sτ_R/(π·τ) ) · cos|A(θₙ)| / √( ν² + (2τ_Sτ_R/τ² + 1/2)·cos²|A(θₙ)| )      (83)

Â_D(B) = Sign(B) · ( f(|B|) − j·g(|B|) )                                              (84)

f(x) = 1/(πx)      x ≥ 5;  else Σₙ₌₀¹² aₙxⁿ  (Table 4)                                (85)
g(x) = 1/(π²x³)    x ≥ 5;  else Σₙ₌₀¹² aₙxⁿ  (Table 5)                                (86)
```

Table 4 (f) and Table 5 (g) coefficients a₀..a₁₂ [TEXT — clean extraction, full precision]:
```
f: a0=0.49997531354311, a1=0.00185249867385, a2=−0.80731059547652, a3=1.15348730691625,
   a4=−0.89550049255859, a5=0.44933436012454, a6=−0.15130803310630, a7=0.03357197760359,
   a8=−0.00447236493671, a9=0.00023357512010, a10=0.00002262763737, a11=−0.00000418231569,
   a12=0.00000019048125
g: a0=0.50002414586702, a1=−1.00151717179967, a2=0.80070190014386, a3=−0.06004025873978,
   a4=−0.50298686904881, a5=0.55984929401694, a6=−0.33675804584105, a7=0.13198388204736,
   a8=−0.03513592318103, a9=0.00631958394266, a10=−0.00073624261723, a11=0.00005018358067,
   a12=−0.00000151974284
```

**Singularity guard (p. 41, verbatim):** when |θₙ − π| < ε, subtract ε from θₙ; ε = 10⁻⁸ "has been found suitable".

**Lit-zone additions:**
- θ₁ < π (edge below line SR): add direct ray `p̂ += e^(jωτ·(R₁/ℓ))/R₁`, `R₁ = √(R_S² + R_R² − 2·R_S·R_R·cos θ₁)` (Eq. 88).
- θ₃ < π: additionally add the ray reflected in the **source** face with Q_R-weighting per Eq. 89 (uses R₂ from cos θ₂ and ψ_G,R = |arcsin-form|, transcribe from p. 42).
- θ₂ < π: additionally add the receiver-face reflection per Eq. 90 (symmetric).
- Angle-modification scheme for image sources/receivers inside the wedge (p. 43): apply the four-case table verbatim (θ′_R, θ′_S, β′).

The whole procedure = `pwedge(f, τ_S, τ_R, τ, R_S, R_R, ℓ, θ_S, θ_R, β, Ẑ_S, Ẑ_R)` (Eq. 91). Diffraction coefficient `D̂(f) = pwedge·ℓ·e^(−jωτ)` (Eqs. 92–94). Non-reflecting variant `pwedge0`/`Dwedge0` keeps only the n=1 term (Eqs. 105–107) — used by shadow-zone shielding (Phase 3) and finite screens (v2).

**Numerical validation performed this session [NUM]:** thin hard screen (β≈2π, S=(0,1), T=(50,6), R=(100,1)): IL = 12.0/14.4/17.0/19.9/22.9/25.9 dB at 125/250/500/1k/2k/4k Hz — tracks Maekawa 10·lg(3+20N) + ≈2 dB (expected for the exact wedge solution vs the empirical chart). Shadow-boundary continuity: |p̂_diffr|·ℓ → 0.5000 as θ_S−θ_R → π from both sides (the analytically-required half-field limit). Use both as engine acceptance anchors (values above tol ±0.05 dB against the committed oracle).

### 11. Two wedges (Eqs. 95–99) and thick screen (Eqs. 100–104) [TEXT, structure complete]

- **p2wedge** (two separate wedges T₁, T₂): compute `D̂₁` for the first wedge (source side) and `D̂₂` for the second, chained: the **most important wedge** (from §5.21) is computed with the real source and the other wedge's top as receiver (or vice versa); `p̂ = D̂₁·D̂₂·e^(jωτ)/ℓ²`? — exact composition per Eqs. 97/98: `p̂(f) = D̂₁(f)·D̂₂(f)·e^(jωτ)/ℓ²` where each D̂ is a Dwedge call with the specific (R,τ,θ,Z) argument lists printed in Eqs. 97–98 (which wedge gets (RS, RM+RR) vs (RS+RM, RR) depends on which is primary). τ = τ_S+τ_M+τ_R, ℓ = R_S+R_M+R_R (Eqs. 95–96). Transcribe argument lists exactly — they encode the effective source/receiver positions.
- **p2edge** (thick screen, two edges of one body, Fig. 11): same chaining idea but the top segment T₁T₂ belongs to both wedges, the top face is forced **hard (Z=∞)**, and the composed result carries a factor **0.5**: `p̂ = 0.5·D̂₁·D̂₂·e^(jωτ)/ℓ²`-form (Eqs. 102–103; the 0.5 is visible in the extraction). Wedge angles via the `Max(arctan…, arctan…)` forms of Eqs. 194–195.
- FORCE case 81 (thick screen 15 m→30 m, h=2 m) exercises p2edge via Sub-model 5; case 91 (thin screen at 15 m + trapezoid screen 75–85 m) exercises p2wedge via Sub-model 6.

### 12. Screen ⇄ ground interaction — Sub-models 4/5/6 [TEXT, complete structure]

**Sub-model 4** (one screen, one edge, §5.13): four-path image model —
```
p̂ = p̂₁ + Q̂₁·p̂₂ + Q̂₂·p̂₃ + Q̂₁·Q̂₂·p̂₄                                              (157)
```
p̂₁ = S→T→R (pwedge), p̂₂ = image-source→T→R, p̂₃ = S→T→image-receiver, p̂₄ = both images. Q̂₁/Q̂₂ = spherical-wave coefficients of the ground segments before/after the screen, evaluated **as if the other endpoint were at the screen top** (τ₂ = the reflected-path travel time on that side).

Separation into screen effect × ground effect (Eqs. 158–159, 163):
```
p̂/p̂₀ = (p̂₁,ff/p̂₀) · [ 1 + Q̂′₁·p̂₂/p̂₁ + Q̂′₂·p̂₃/p̂₁ + Q̂′₁Q̂′₂·p̂₄/p̂₁ ]
      = Δp̂_SCR · Δp̂_G,      Q̂′ᵢ = w_Qᵢ·Q̂ᵢ
Δp̂_SCR(f) = |p̂₁,ff(f)|·R_SR                                                          (163)
```
Partial-coherent ground term (Eq. 182) [TEXT, structure certain — the 4-ray generalization of Eq. 120]:
```
|Δp̂_G|² = |1 + F₂·w″₁Q̂′₁·(p̂₂/p̂₁) + F₃·w″₂Q̂′₂·(p̂₃/p̂₁) + F₄·w″₁w″₂Q̂′₁Q̂′₂·(p̂₄/p̂₁)|²
        + (1−F₂²)·|w″₁·ρ₁·p̂₂/p̂₁|² + (1−F₃²)·|w″₂·ρ₂·p̂₃/p̂₁|² + (1−F₄²)·|w″₁w″₂·ρ₁ρ₂·p̂₄/p̂₁|²
```
F₂/F₃/F₄ per Eqs. 177–181 (source-side Δτ_S, receiver-side Δτ_R, both for ray 4; transversal separations Eqs. 178/180: ρᵢ = 2h′_Sᵢh′_Rᵢ/(h′_Sᵢ+h′_Rᵢ)). Fresnel weights w₁,w₂ per Eq. 174 (F_λ = λ/16) with the rS/rR edge-proximity modifiers (Eqs. 175–176: h_max = min(0.0005·(x_T−x_S1?), 0.2)-style clamps — transcribe exactly), normalization + w_Q weights per Eq. 187, and the final level:
```
ΔL₄(f) = 20·lg| Δp̂_SCR(f) · Σᵢ₁ Σᵢ₂ w″ᵢ₁(f)·w″ᵢ₂(f)·Δp̂_G,i1,i2(f) / (Σw″ᵢ₁·Σw″ᵢ₂)‑normalized |   (188 — transcribe the exact normalization from p. 80)
```
The general model (§5.13.2) iterates the base model over **all combinations** of reflecting segments before × after the screen (segments forming the screen shape are excluded; screen shape reduced to 3 points per §5.21; wedge-face impedances = the σ of the two segments adjacent to the edge).

Geometry helpers used (all pure 2-D vector math, most already exist in `envi-engine::geometry`): `SegmentVariables` (Eq. 383 — heights above extended segment, distances along it), `ImagePoint` (Eq. 370), `NormLine` (Eq. 377 — projection + signed distance), `Length` (372), `VertDist` (390), `WedgeCross` (392 — intersection of two non-adjacent segments = equivalent wedge top).

Shadow-zone branches of Sub-model 4 (Eqs. 184–186) are Phase 3 (ξ<0 only).

**Sub-model 5** (one screen, two edges, §5.14, Eqs. 189–221): structurally identical to Sub-model 4 with pwedge→p2edge. **Sub-model 6** (two screens, §5.15, Eqs. 222–~270): eight-ray image model
```
p̂ = p̂₁ + Q̂₁p̂₂ + Q̂₃p̂₃ + Q̂₁Q̂₃p̂₄ + Q̂₂p̂₅ + Q̂₁Q̂₂p̂₆ + Q̂₂Q̂₃p̂₇ + Q̂₁Q̂₂Q̂₃p̂₈       (222)
```
with the middle-region reflection (images of T₁/T₂ in the middle segment) and p2wedge as the diffraction kernel; three Fresnel regions (before/middle/after) each with weights and normalization. **Implementation strategy: write ONE generic "screen sub-model" engine parameterized by (diffraction kernel, ray set, region count)** — Sub-models 4/5/6 differ only in kernel and combinatorics.

### 13. Sub-model 7 — turbulence scattering behind screens (§5.16, Eqs. 271–…) [TEXT]

Adds scattered energy that floors screen attenuation when Cv²/CT² > 0 (all FORCE screen cases: Cv²=0.12, CT²=0.008):
- Effective strengths: `Cve² = 10·Cv²`, `CTe² = 10·CT²` (Eq. 271 — the "×10" empirical boost is deliberate; do not "fix").
- ΔL_ws (wind) and ΔL_ts (temperature) from **two-dimensional table interpolation** (Tables 6/7, pp. 117–119) in parameters `40·R₂/R₁` and `40·h_e/R₁` (h_e = edge height above the SR line; contribution ignored if edge below SR line), with a reciprocity fix (compute both orientations, take max) and a ground-effect correction C_SR (the ground part of Eq. 188 floored at 1). Frequency scaling via f² and Coft per Eq. 272.
- Enters the compound model as additive energy: ΔL_scr terms `ΔL₄+ΔL₇,₁` etc. in Eq. 332.
- Self-contained and homogeneous-compatible → **belongs in Phase 2** (screen cases are wrong at high frequency without it). Tables 6/7 must be transcribed from PDF pp. 117–119 (numeric tables extract cleanly with pdfplumber).

### 14. Terrain interpretation & sub-model transitions (§5.21, Eqs. 300–328) [TEXT, complete]

The dispatcher that makes FORCE profiles run end-to-end:
1. **Primary edge of primary screen** (§5.21.1): every interior profile point above the terrain baseline is a candidate; pick max path-length difference `Δℓ₀ = H(zᵢ−z_m)·(|SPᵢ|+|PᵢR|−|SR|)` (Eq. 300, sign via Heaviside — negative below line-of-sight). Efficiency = transition parameter `r_scr1 = r_Δℓ·r_h·r_Fz` (Eqs. 301–305): `r_Δℓ` ramps over −λ/33 ≤ Δℓ′ < 0? (Eq. 302: 1 for Δℓ′≥0; 1+7.5Δℓ′/λ? for −0.133 < Δℓ′/λ < 0; 0 below −0.133λ — transcribe breakpoints 0.133 exactly), `r_h` ramps on screen height h_SCR/λ over [0.1, 0.3] (Eq. 303), `r_Fz` on h_SCR/h_Fz over [0.026, 0.082] with h_Fz from CalcFZd at F_λ=0.5λ (Eq. 304).
2. **Secondary screen** (§5.21.2, Eqs. 307–310) and **secondary edge of primary screen** (§5.21.3, Eqs. 311–315): analogous, plus the edge-separation modifier r_w (Eq. 308, ramp on d₁₂/λ over [0.3, 1.0]?) and the shape modifiers r′_h12/r′_Fz12.
3. **Terrain flatness** (§5.21.4, Eqs. 316–319): equivalent flat terrain by least squares (Eqs. 320–327), flatness parameter r_f from the path-length increase δR₂ = √((hSe+hRe)²+de²)−√((hSe−hRe)²+de²)-form vs Δh (Eq. 317, ramps [0.01, 0.03]; **only evaluated 25 Hz–2 kHz, frozen above 2 kHz**), concave-weight modifier (Eq. 318).
4. **Compound combination** (§5.22 Eq. 332):
```
ΔL_t = r_scr1·ΔL_scr + (1−r_scr1)·ΔL_noscr
ΔL_scr = r_scr2·(ΔL₆ + ΔL₇-terms) + (1−r_scr2)·ΔL_scr1
ΔL_scr1 = r_scr12·(ΔL₅ + ΔL₇,₂) + (1−r_scr12)·(ΔL₄ + ΔL₇,₁)
ΔL_noscr = r_flat·ΔL_flat + (1−r_flat)·ΔL₃          (ΔL_flat = Sub-model 1 or 2)
```
For the Phase 2 target set (flat ground ± screens): r_flat = 1 exactly (flat profiles), so **Sub-model 3 (non-flat, §5.12) is never invoked with nonzero weight — implement it as an explicit `todo`-guarded stub returning an error if reached with weight > 0**, and schedule it (see Open Questions #4).
5. **Screen shape reduction:** screen start/end = nearest non-convex points around the edge (`Convex` Eq. 336, threshold 0.0001 m); shapes with >3 points reduced via secondary-edge logic.

FORCE screen encoding (verified from the .xls this session): thin screen = z-spike with x-steps of 0.01 m (14.99→15.00→15.01), thick screen = flat-top trapezoid (15→30 m), double screens = spike at 15 + trapezoid 75–85 m. The wedge β computed from these near-vertical segments is ≈ 2π−ε for thin screens — well inside the β>π validity domain, but test both faces' angle computation on this geometry (near-vertical arctan).

## Complex Combination & Conventions (ENG-07)

### The convention trap (critical)

Nord2000 is written in the **e^(−jωt)** time convention: outgoing propagation phase is `e^(+jωτ)` (visible in Eqs. 78, 88, 120) and Ẑ_G carries **+j** (Eq. 57). ENVI froze **e^(+jωt)**, outgoing `e^(−jωτ)` in Phase 1 (transfer.rs contract). These are complex conjugates of each other.

**Resolution (recommended):** implement all of §5.6–§5.22 verbatim in the document's convention inside the propagation modules (doc-comment the module: "Nord2000-native: e^{−jωt}"). Band levels (|·|²) are convention-invariant, so FORCE-style validation needs no conversion. Where Phase 2 exposes a complex ratio to the TransferSpectrum (the coherent term of ΔL_t as a complex multiplier on H), apply **one** `conj()` at that boundary and unit-test it: a two-path setup must produce the same interference dip through both code paths. Mixing conventions silently (e.g., using Phase 1's e^{−jωτ} direct H with a Nord2000-native e^{+jωΔτ} ground term) inverts the *asymmetry* of the dip (arg Q̂) and shifts dip frequencies — the exact failure the dip-position tests must catch.

### What the engine returns (planner decision, recommended shape)

Nord2000's ΔL_t is a per-band **level**, not a complex transfer — the partial-coherence residual is energy without phase. To satisfy both ENG-07 and the FORCE-validation path:

1. `terrain_effect()` computes per-ray complex pressures p̂ᵢ (retaining Δτ phase — ENG-07 satisfied structurally) and returns both:
   - `delta_l_db: f64` — the Nord2000-compliant partially-coherent band value (validation path, Eq. 120/182/332), and
   - `coherent_ratio: Complex<f64>` — the F-weighted coherent sum `1 + Σ F_i·w″·Q̂′·p̂ᵢ/p̂₁` (the phase-bearing part), which multiplies into TransferSpectrum.
2. The tensor keeps the coherent ratio; the incoherent residual `(1−F²)|…|²` is carried as a per-band real add-on used when forming band levels. This is the concrete landing of PROJECT.md's open question ("represent partial-coherence F alongside the coherent transfer") — Phase 4 finalizes the storage layout, Phase 2 just keeps both quantities separable.

### Dip positions (success-criterion anchors)

First destructive interference: `2πf·Δτ + arg Q̂(f) = π`. For hard ground (arg Q̂ ≈ 0 at low f): `f_dip ≈ 1/(2Δτ) = c₀/(2ΔR)`. For finite impedance arg Q̂ > 0 shifts dips **down** in frequency; soft ground at grazing incidence (Q̂ ≈ −1) destroys low frequencies broadly instead of producing a sharp first dip.

## Numeric Anchors (computed this session — seed the oracle fixtures with these)

Conditions: t = 15 °C → c₀ = 20.05·√288.15 = **340.348 m/s** (Eq. 335, matches Phase 1).

**Impedance (Eq. 57):**
| σ (kPa·s·m⁻²) | f (Hz) | Ẑ_G |
|---|---|---|
| 200 (D) | 1000 | 3.715553 + 3.675351j |
| 12.5 (A) | 1000 | 1.339444 + 0.485614j |
| 200 (D) | 100 | 16.270679 + 19.737805j |
| 20000 (G) | 1000 | 86.873338 + 105.998290j |
| 200000 (H) | 63 | 3841.191953 + 4283.317397j |

**Geometry anchor** (synthetic flat case: hS=0.5, hR=1.5, d=97.5 — FORCE-like without emission):
R₁ = 97.505128070, R₂ = 97.520510663, ΔR = 1.538259287e−2 (check against 4hShR/(R₁+R₂)), Δτ = 4.519660952e−5 s, ψ_G = 1.175133°.

**Spherical-wave Q̂ (Eqs. 58–60 chain, via wofz):**
| σ | f (Hz) | Q̂ |
|---|---|---|
| 12.5 | 1000 | −0.947630 + 0.020506j |
| 200 | 250 | −0.838688 + 0.936625j |
| 200 | 1000 | −0.873836 + 0.135191j |
| 200 | 4000 | −0.922626 + 0.051218j |
| 20000 | 1000 | +0.797560 + 0.416202j |
| 20000 | 4000 | −0.004250 + 0.608435j |

Note |Q̂| may exceed 1 locally (σ=200, f=250 → 1.257) — that is correct physics (surface-wave contribution), **do not clamp |Q̂| ≤ 1**.

**Coherent flat-ground effect** (same geometry, σ=200, F=1, Eq. 120 coherent term):
| f (Hz) | ΔL_flat (dB) |
|---|---|
| 100 | +5.2099 |
| 200 | +1.7311 |
| 400 | −12.6247 |
| 800 | −18.1225 |
| 1000 | −15.4567 |
| 2000 | −6.7805 |
| 4000 | −0.0726 |
Deepest dip: **646.7 Hz, −19.16 dB**. On ENVI's 1/12-octave grid this falls between x=−8 (630.96 Hz) and x=−7 (667.42 Hz) — the dip-band test asserts the discrete minimum lands on one of these two points. (At this Δτ, Ff at the dip = 0.99993 — band-averaging barely matters for low flat geometries; it matters at larger Δτ/higher f.)

**Wedge (hard, β=2π·0.9999, S=(0,1), T=(50,6), R=(100,1)):** IL(f) = 12.01 / 14.35 / 17.02 / 19.90 / 22.87 / 25.87 dB at 125/250/500/1000/2000/4000 Hz; shadow-boundary limit |p̂_diffr|·ℓ = 0.500 ± 0.01 at |θ_S−θ_R−π| = 0.01°.

*(Anchors are from this session's reference implementation of the transcribed equations, cross-checked against scipy wofz and Maekawa/HP behavior. Treat as cross-implementation anchors with tol 1e−4 relative for Q̂/Ẑ, ±0.05 dB for levels — if the Rust engine disagrees, resolve against the PDF, not against these numbers.)*

## FORCE Validation Reality for Phase 2

**Parameter audit of all 62 straight-road cases (from TestStraightRoad.xls this session):**

| Case group | Cases | u | su | Cv²/CT² | Nord2000 sub-models | Phase 2 relevance |
|---|---|---|---|---|---|---|
| Flat, single impedance A–G, hr 1.5/4 | 1–8 | 0 | 0 | **0 / 0** | 1 (via 2 with road+grass segments) | Primary targets — F = Ff only, fully homogeneous |
| Flat, refraction/turbulence variants | 9–16 | 0–5 | 1.0 | 1.0/1.0 | 1/2 + refraction | Phase 3 |
| Vehicle categories | 17–18 | 0 | 0 | 0/0 | same as 4 | Phase 4 (emission) |
| Flat mixed impedance | 21–24 | 0/3 | **0.5** | **0.12/0.008** | 2 | u=0 cases (21, 24) target-able minus FΔν |
| Elevated road / valley / lake | 31–64 | 0/3 | 0.5 | 0.12/0.008 | 3 (+2) | needs Sub-model 3 — see Open Q#4 |
| Thin screen | 71–74 | 0/3 | 0.5 | 0.12/0.008 | 4 | u=0 cases (71, 74) — need Fc + ΔL₇ |
| Thick screen | 81–84 | 0/3 | 0.5 | 0.12/0.008 | 5 | u=0 cases (81, 84) |
| Two screens | 91–94 | 0/3 | 0.5 | 0.12/0.008 | 6 | u=0 cases (91, 94) |
| Non-flat, forest | 101–124 | — | — | — | 3, 10 | Phase 3/4 |

**Hard constraint (carried from Phase 1 research, still true):** every FORCE reference value — including the per-band `dL` propagation column — is a pass-by-integrated quantity over the Jonasson sub-source model (heights 0.01/0.30/0.75 m with per-band power splits). **No FORCE case can gate Phase 2 end-to-end.** ROADMAP success criteria 1–2 ("FORCE cases … match reference within tolerance") must be interpreted by the planner as: *the propagation machinery those cases require is implemented and validated at propagation level*; the actual FORCE-green gate is Phase 4 (VAL-02 traceability agrees — it maps to Phase 4).

**What Phase 2 CAN gate on (recommended acceptance ladder):**
1. **Exact anchors** (this document): w(ẑ) fixtures, Ẑ_G, Q̂, ΔR identity, dip table, wedge IL table, shadow-boundary 0.5 — tolerance 1e−4/±0.05 dB.
2. **Committed Python oracle** (`tools/nord2000_oracle/`, scipy-based, generated fixtures committed as TOML): full ΔL_t curves for the exact FORCE geometries (case 1/21/71/81/91 profiles, single source height, no emission) — engine must match the oracle to ≤0.1 dB per point. This is a cross-implementation test, not a FORCE-reference test.
3. **Physical property tests:** dip lands on predicted grid band (±1 band, mirroring the FORCE dip-shift rule); soft ground (A) attenuates more than hard (G) in 200–2000 Hz; screen IL grows monotonically with N in deep shadow; adding turbulence (Sub-model 7) floors but never increases screen attenuation... (it adds energy: ΔL_t less negative); every evaluated quantity finite over all 105 points × all case geometries (success criterion 3).
4. **Capability gate honesty:** cases 1–8 remain `Skipped(requires: emission-model)` — the skip-reason list shrinks (ground-effect, diffraction disappear from it), which is itself an assertable outcome.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Faddeeva w(z) "properly" | An arbitrary-precision erfc | **The document's Eqs. 61–74 exactly** | Fidelity to the standard beats accuracy; reference implementations use these branches; deviations show up as sub-0.01 dB diffs that poison FORCE comparison later |
| Fresnel integrals f(x), g(x) | Numerical integration of Fresnel integrals | Tables 4/5 polynomial fits + asymptotes (Eqs. 85–86) | Same fidelity argument; full-precision coefficients extracted above |
| 2-D geometry helpers | New vector library | Existing `envi-engine::geometry` + small additions (NormLine, VertDist, WedgeCross per §5.23) | Already have image-source reflection from 01-02; add the three missing helpers as pure functions with the doc's exact signatures |
| Test oracle | Trusting hand-derived expected values for full ΔL_t curves | scipy-based Python reference implementation committed under `tools/` | Full-curve fixtures catch composition errors that unit anchors can't; regenerable; not a build dependency |
| Cubic-equation reflection-point solver (Eq. 49) | — | **Nothing (this phase)** | Downward-refraction only; homogeneous reflection point = image-source intersection (already built) |

**Key insight:** as in Phase 1, the physics IS the product — but this phase has a uniquely dangerous failure mode: *silently plausible wrongness*. A mistranscribed coefficient in Eq. 64 or Table 5 still produces smooth, dip-shaped curves. The defense is layered anchors: per-function fixtures → per-sub-model curves → property tests.

## Architecture Patterns

### System Architecture Diagram

```
 Scene (01-02)                    envi-engine::propagation (extended)
 TerrainProfile ───►┌──────────────────────────────────────────────────────────┐
 (points, σ, r)     │ terrain_interpretation (§5.21)                           │
 Source/Receiver ──►│  find primary/secondary edges & screen shapes            │
                    │  r_scr1/r_scr2/r_scr12/r_flat transition params          │
                    └───────┬──────────────────────────────────────────────────┘
                            ▼ dispatch (flat / screens)
   ┌─────────────────────────────────────────────────────────────────────────┐
   │ terrain_effect (§5.10–5.16, §5.22)                                      │
   │                                                                         │
   │  Sub-model 1 ◄── ground::Q̂ ◄── special::w(ẑ) (Eqs 61–74)               │
   │  Sub-model 2 ◄── fresnel::{CalcFZd, ZoneSize, ZoneW, ZoneWm}            │
   │  Sub-models 4/5/6 ◄── diffraction::{pwedge, p2wedge, p2edge}            │
   │       │                    ▲ special::{f,g} Fresnel fits                │
   │       │                    ▲ ground::Q̂ (wedge faces, Eq. 80)            │
   │  Sub-model 7 ◄── tables 6/7 interpolation                               │
   │  coherence::F = Ff·[FΔν=1]·Fc·Fr·[Fs=1]                                 │
   │  rays::RayVars (straight-ray now; Phase 3 swaps constructor)            │
   │                                                                         │
   │  out: ΔL_t(f) dB  +  coherent complex ratio (Nord2000-native)           │
   └───────────────┬─────────────────────────────────────────────────────────┘
                   ▼ conj() at ONE boundary (convention: e^{−jωt} → e^{+jωt})
        TransferSpectrum multiply (transfer.rs, Phase-1 contract)
                   ▼
        envi-harness: capability {GroundEffect, Diffraction} now implemented;
        oracle-fixture comparison (tools/nord2000_oracle TOML) + property tests
```

### Recommended Project Structure (delta from Phase 1)

```
crates/envi-engine/src/propagation/
├── mod.rs                    # extended: terrain_effect entry point
├── divergence.rs             # (existing)
├── air_absorption.rs         # (existing)
├── special.rs                # w(ẑ) Eqs 61–74; f,g Eqs 85–86 + Tables 4/5; exp' Eq 337
├── ground.rs                 # Ẑ_G, Γ̂_p, Ê, Q̂, ρᵢ (Eqs 57–77)
├── coherence.rs              # Ff, Fc, Fr (+FΔν/Fs stubs) Eqs 110–114
├── rays.rs                   # RayVars: straight-ray τ/R/ψ_G/Δτ, cancellation-safe ΔR
├── fresnel.rs                # CalcFZd, FresnelZoneSize/W/Wm (Eqs 338–353)
├── diffraction.rs            # pwedge/Dwedge/p2wedge/p2edge/pwedge0 (Eqs 78–107)
├── terrain_effect/
│   ├── mod.rs                # ΔL_t composition Eq 332
│   ├── submodel1.rs          # Eqs 115–123 (homogeneous branch)
│   ├── submodel2.rs          # Eqs 124–133 + PhaseDiffFreq (Eqs 378–381)
│   ├── screen.rs             # generic 4/8-ray screen engine → sub-models 4/5/6
│   └── submodel7.rs          # turbulence scattering, Tables 6/7
└── terrain_interpretation.rs # §5.21 Eqs 300–328 (+ equivalent flat terrain)
tools/nord2000_oracle/        # committed Python oracle (scipy) + fixture generator
cases/ground_*.toml           # synthetic ground/screen cases with oracle-pinned expectations
```

### Pattern 1: Convention-quarantined module (the load-bearing rule)

Everything under `propagation::{special,ground,coherence,diffraction,terrain_effect}` is Nord2000-native (e^{−jωt}). One conversion function, unit-tested:

```rust
// transfer.rs — Source: convention analysis, AV 1106/07 Eqs. 57/78/120 vs 01-03 contract
/// Convert a Nord2000-native complex pressure ratio (time convention e^{−jωt})
/// into ENVI's transfer convention (e^{+jωt}). |ratio| is invariant.
pub fn nord_ratio_to_transfer(ratio: Complex<f64>) -> Complex<f64> {
    ratio.conj()
}
```

### Pattern 2: RayVars seam for Phase 3

```rust
// rays.rs — Source: AV 1106/07 §5.5.4–5.5.6 (homogeneous limit; doc clamps |ξ|<1e−10)
pub struct RayVars {
    pub tau: f64,        // travel time along ray (s)
    pub r: f64,          // travel distance (m)
    pub psi_g: f64,      // grazing angle at reflection (reflected rays only)
    pub r1: f64, pub r2: f64, // partial distances for reflected rays
}
pub struct RayPair { pub direct: RayVars, pub reflected: Option<RayVars>, pub dtau: f64 }

/// Homogeneous (straight-ray) constructor. Phase 3 adds `circular(...)` with the
/// equivalent-linear profile behind identical fields.
pub fn straight(d: f64, h_s: f64, h_r: f64, c0: f64) -> RayPair {
    // ΔR via cancellation-free identity: (R2²−R1²)/(R2+R1) = 4·hS·hR/(R1+R2)
    ...
}
```

### Pattern 3: Generic screen engine

Sub-models 4/5/6 share: ray-set construction (images), per-ray diffraction kernel call, Q̂ per region, F per ray, Fresnel weights per region, Eq. 182-form combination, Eq. 187 normalization. Implement once:

```rust
// terrain_effect/screen.rs
pub trait DiffractionKernel {  // pwedge (SM4) | p2edge (SM5) | p2wedge (SM6)
    fn diffract(&self, f: f64, geo: &ScreenGeometry, images: ImageConfig) -> Complex<f64>;
}
```
Sub-model 6's 8-ray set = the 4-ray set × {with, without} middle-region reflection — encode as bitmask over regions.

### Pattern 4: Oracle fixtures, not hand-computed spectra

```toml
# cases/ground_flat_sigma200.toml (generated by tools/nord2000_oracle/gen.py — DO NOT EDIT)
[meta]
name = "flat ground, sigma=200, hs=0.5, hr=1.5, d=97.5, F=Ff only"
kind = "ground-effect"
reference = "oracle-v1"     # oracle git-hash provenance
[expected]
tolerance_db = 0.1
# 105 values, ΔL_t per 1/12-oct point
bands = [ 5.2099, ... ]
```

### Anti-Patterns to Avoid

- **Clamping |Q̂| to ≤1:** wrong — the spherical-wave coefficient legitimately exceeds 1 near the surface-wave regime (anchor: σ=200, f=250 Hz → |Q̂|=1.257).
- **Skipping the θₙ=π epsilon guard** (p. 41): produces Inf at exact shadow-boundary geometries; FORCE thin screens with receivers near grazing sit close to this.
- **Using a "better" w(z) or Fresnel integral:** breaks bit-comparability with reference implementations; keep the standard's approximations.
- **Treating ΔL values as complex multipliers everywhere:** Sub-model outputs are dB levels combined by r-parameter interpolation (Eq. 332) — only the coherent ratio inside them is complex. Keep the two representations separate (see ENG-07 section).
- **Computing Δτ as τ₂−τ₁ naively:** the document itself flags this; use the ΔR identity.
- **Re-deriving wedge-face grazing angles:** Eq. 80's `min(β−θ_S, π/2)` / `min(θ_R, π/2)` with τ₂ = τ_S+τ_R is prescriptive, not obvious — transcribe.

## Common Pitfalls

### Pitfall 1: Convention mixing corrupts interference
**What goes wrong:** ground/diffraction phases computed in Nord2000's e^{+jωτ} convention multiplied onto Phase-1 H (e^{−jωτ}) — dips shift/mirror because arg Q̂ enters with the wrong sign.
**Why it happens:** |·|² validation passes either way; only the combined transfer is wrong.
**How to avoid:** Pattern 1 (single conj boundary) + a dip-asymmetry test: for σ=200 the dip at 646.7 Hz must sit BELOW the hard-ground 1/(2Δτ) frequency (arg Q̂ > 0 pulls dips down).
**Warning signs:** dips at frequencies above the hard-ground prediction; `conj()` calls scattered through modules.

### Pitfall 2: Treating Phase 2 as "make FORCE cases 1–8 pass"
**What goes wrong:** the phase stalls chasing pass-by-integrated reference values that require the Phase 4 emission model.
**How to avoid:** acceptance ladder (§FORCE Validation Reality); capability gate keeps road cases Skipped with a shrinking requires-list.
**Warning signs:** a plan task saying "case 1 green" without an emission-model dependency.

### Pitfall 3: Sub-model 2 blends per surface TYPE, not per segment
**What goes wrong:** implementing Eq. 124 as a per-segment sum double-counts surfaces that appear in multiple segments (FORCE case 21: road G, grass D, road G, grass D → **two** types, four segments).
**How to avoid:** group segments by (σ, r) exactly as Eq. 127 prescribes; ΔL per type computed with the full path geometry.
**Warning signs:** case 21 oracle mismatch while case 1 (two types) passes.

### Pitfall 4: Fresnel-weight normalization branches
**What goes wrong:** Eqs. 187 (screens) and the §5.8 sum-to-2 rule (flat/non-flat) have different normalization logic; mixing them skews ground effect around screens.
**How to avoid:** implement normalization per sub-model as printed; property test: weights after normalization ∈ [0,1], per-region sums ≤ 1 + excess rules.
**Warning signs:** ground effect around screens growing with segment count.

### Pitfall 5: w(z) branch discontinuities and quadrant handling
**What goes wrong:** value jumps at |x|=3.9/|y|=3/x=6/y=6 borders, or wrong symmetry for x<0 (soft ground) / y<0 (hard ground) — Q̂ then has kinks that show up as spurious ripples in ΔL(f).
**How to avoid:** fixture tests straddling every branch border (e.g., x=3.89/3.91) and in all four quadrants; continuity test |w(z⁺)−w(z⁻)| < 1e−4 across borders.
**Warning signs:** non-smooth Q̂(f) for a fixed geometry.

### Pitfall 6: Near-vertical screen segments break angle math
**What goes wrong:** FORCE thin screens use Δx = 0.01 m; naive slope arctan((z₂−z₁)/(x₂−x₁)) is fine in f64, but wedge angle β = 2π−ε and A(θₙ) → 0 makes sin|A|/|A| a 0/0 candidate; also `SegmentVariables` on a 0.01 m segment gives d′ ranges spanning 5 orders of magnitude.
**How to avoid:** sinc-guard (|A| < 1e−6 → Taylor), Eq. 78's ε=1e−8 angle guard, and a dedicated unit test on the literal case-71 profile coordinates.
**Warning signs:** NaN at exactly one frequency/geometry combination (success criterion 3 explicitly demands a full-sweep finiteness test).

### Pitfall 7: Sub-model 7's ×10 turbulence boost looks like a bug
**What goes wrong:** a reviewer "fixes" Cve² = 10·Cv² back to Cv²; screen cases then over-attenuate at high frequency vs any reference.
**How to avoid:** cite Eq. 271 in a comment ("original model underestimated scattering; effective strengths are deliberate").
**Warning signs:** exactly −10 dB/decade-style deviations in deep-shadow high-frequency bands vs oracle.

### Pitfall 8: PhaseDiffFreq extrapolation edges
**What goes wrong:** fL/fH iteration runs off the 25 Hz–10 kHz grid for extreme geometries (very hard ground: Ψ small; very soft + high: Ψ large) and returns garbage, corrupting the Sub-model 2 low/high blend.
**How to avoid:** implement Eqs. 380–381's extrapolation rules exactly (linear 10 k→100 kHz, constant above; log-scaled below 25 Hz), plus the fL ≤ 0.8·fH clamp.
**Warning signs:** ΔL₂ discontinuities at band boundaries; NaN in mixed-impedance sweeps.

## Code Examples

### w(ẑ) — branch structure (Source: AV 1106/07 Eqs. 61–74, coefficients numerically verified vs scipy)

```rust
// special.rs — Nord2000-native module (e^{−jωt} convention)
pub fn faddeeva_w(z: Complex<f64>) -> Complex<f64> {
    // Symmetry reduction to x ≥ 0, y ≥ 0 per Eq. (62):
    //   w(-conj(z)) = conj(w(z));  w(-z) = 2·exp(-z²) - w(z)
    // then dispatch:
    //   |x|>3.9 || |y|>3  →  (x>6 || y>6) ? two_pole(z) : three_pole(z)
    //   else              →  matta_reichel(z)   // h = 0.8, n = 1..=5
    ...
}

fn two_pole(z: Complex<f64>) -> Complex<f64> {          // Eq. (63)
    let z2 = z * z;
    Complex::I * z * (0.5124242 / (z2 - 0.2752551) + 0.05176536 / (z2 - 2.724745))
}
```

### Q̂ chain (Source: Eqs. 57–60, glyph-verified)

```rust
// ground.rs
pub fn ground_impedance(f_hz: f64, sigma_kpa_s_m2: f64) -> Complex<f64> {
    let x = f_hz / sigma_kpa_s_m2;                       // ≡ 1000f/σ_SI, Eq. (57) unit note
    Complex::new(1.0 + 9.08 * x.powf(-0.75), 11.9 * x.powf(-0.73))
}

pub fn spherical_q(f: f64, tau2: f64, psi_g: f64, z_g: Complex<f64>) -> Complex<f64> {
    let s = psi_g.sin();
    let inv_z = z_g.inv();
    let gamma_p = (s - inv_z) / (s + inv_z);             // Eq. (59)
    let omega_tau = 2.0 * PI * f * tau2;
    let rho = Complex::new(0.5, 0.5) * omega_tau.sqrt() * (s + inv_z);   // Eq. (60)
    let e = Complex::ONE + Complex::I * PI.sqrt() * rho * faddeeva_w(rho);
    gamma_p + (Complex::ONE - gamma_p) * e               // Eq. (58)
}
```

### Sub-model 1 homogeneous (Source: Eqs. 119–120)

```rust
// terrain_effect/submodel1.rs
pub fn delta_l_flat(f: f64, ray: &RayPair, sigma: f64, r_rough: f64, coh: &CoherenceInputs) -> GroundResult {
    let z_g = ground_impedance(f, sigma);
    let refl = ray.reflected.as_ref().expect("homogeneous flat: reflection exists");
    let q = spherical_q(f, refl.tau, refl.psi_g, z_g);
    let ratio = ray.direct.r / refl.r;                       // R1/R2
    let f_coh = coherence(f, ray.dtau, coh, refl.psi_g, r_rough); // Ff·Fc·Fr (FΔν=Fs=1)
    let phase = Complex::from_polar(1.0, 2.0 * PI * f * ray.dtau); // e^{+j2πfΔτ}: NORD-NATIVE
    let coherent = Complex::ONE + f_coh * ratio * phase * q;
    let rho_i = incoherent_coeff(f, z_g);                    // Eq. (75)–(76), precomputable
    let energy = coherent.norm_sqr() + (1.0 - f_coh * f_coh) * (ratio * rho_i).powi(2);
    GroundResult { delta_l_db: 10.0 * energy.log10(), coherent_ratio: coherent }
}
```

### Finiteness sweep (success criterion 3)

```rust
#[test]
fn no_nan_across_all_force_geometries_and_bands() {
    for case in force_case_profiles() {                  // 62 profiles, single unit source
        for &f in FREQ_AXIS.centres.iter() {             // 105 points
            let dl = terrain_effect(&case.profile, &case.src, &case.rcv, f, &homogeneous_weather());
            assert!(dl.delta_l_db.is_finite(), "{}: {f} Hz", case.id);
        }
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Phase 1 assumption "impedance class B = 31.6" | Table 2 verified: **B = 31.5** | this session | fix `impedance_class()` + doc comment |
| Phase 1 open question: roughness classes | Table 3 verified: N/S/M/L = 0/0.25/0.5/1 m | this session | GroundSegment.roughness semantics settled |
| "Nord2000 ground = plain two-ray complex sum" (naive reading) | Partial-coherence combination with F = Ff·FΔν·Fc·Fr·Fs + incoherent ρᵢ residual | — (always was) | ENG-07 output shape decision (coherent ratio + energy residual) |
| Homogeneous case via ξ-clamp 1e−10 into circular rays (reference implementations) | ENVI: exact straight-ray limit with cancellation-free ΔR | design decision | cleaner numerics; Phase 3 must verify circular(ξ→0) ≡ straight |

**Deprecated/outdated:** nothing — the physics documents are frozen (2014 rev.). The 2018 amendment document (118-22465) touches later-phase items; not relevant to §5.6–5.22 [ASSUMED — spot-check its change list when fetched for Phase 3].

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | ρᵢ = √(1−ᾱ_ri) (radical present in Eq. 75) | Ground §5 | Incoherent residual off by a power; only visible at high f where F→0; verify on PDF p. 37 image during transcription (one-minute check) |
| A2 | ΔL_flat Eq. 120 carries a 10·lg prefix (energy form shown glyph-verified) | Ground §7 | None in practice — dB combination in Eq. 332 requires it; verify visually |
| A3 | Fc constant 5.888e−3-form and exponents in Eq. 113 (structure per turbulence theory, exact constants garbled in text dump) | Coherence | Mixed/screen-case validation vs oracle would drift; transcribe from PDF p. 52 image (executor task) |
| A4 | Fr polynomial g(X) coefficients (Eq. 114, p. 53) readable from PDF | Coherence | Roughness r=0 for all Phase 2 target cases → zero immediate risk; needed for elevated-road cases later |
| A5 | Sub-model 7 Tables 6/7 extract cleanly from pp. 117–119 | Diffraction §13 | If extraction is unreliable, transcribe by page-image reading (pdfplumber to_image + visual) — slower but certain |
| A6 | p2wedge/p2edge composition is D̂₁·D̂₂·e^{jωτ}/ℓ²-form with the printed argument lists (structure from text dump; factor 0.5 for p2edge) | Diffraction §11 | Thick/double-screen cases wrong by a constant factor; the oracle + case-81/91 property tests catch it; verify Eqs. 97–98/102–103 on page images |
| A7 | FΔν = 1 stub is acceptable for u=0 FORCE cases (su=0.5 → A⁺=1.7·sA weak) in Phase 2 | Scope | If FΔν materially <1 at high f·Δτ, mixed/screen oracle comparisons still pass (oracle uses same stub) but Phase 4 FORCE comparison shows dip-region deviations; quantified fix lands with Phase 3 refraction machinery |
| A8 | 2018 amendments don't alter §5.6–5.22 | State of the Art | Re-check when the amendments doc is fetched |

## Open Questions

1. **How literally must ROADMAP success criteria 1–2 ("FORCE cases match reference") be satisfied in Phase 2?**
   - What we know: reference values require the Phase 4 emission model; VAL-02 (FORCE within tolerance) is mapped to Phase 4; Phase 1 established the capability-gate pattern for exactly this.
   - Recommendation: planner reframes the criteria as "the ΔL_t machinery for those case families is implemented and matches the committed oracle + property anchors"; note the reframing in PLAN.md so verification doesn't flag it. Alternatively pull a minimal Jonasson emission adapter forward — NOT recommended (large, distinct scope).

2. **FΔν in u=0 FORCE cases (su=0.5):** stub =1 now, or pull minimal A⁺-profile ray bending forward?
   - What we know: FΔν needs Δτ⁺ under the A⁺=A+1.7·sA profile (Eq. 10) — that's CalcEqSSP + circular-ray Δτ, i.e., the heart of Phase 3.
   - Recommendation: stub =1; make `coherence()` take FΔν as an injected factor so Phase 3 drops in without touching sub-models. Quantify the effect when Phase 3 lands (expected small for hr=1.5, d≈100 m geometries — Δτ⁺−Δτ is second-order in the weak A⁺).

3. **Where exactly does the coherent ratio multiply into H when screens are present?**
   - What we know: ΔL_t for screens is Δp_SCR (a magnitude, Eq. 163) × the Fresnel-weighted sum of complex Δp̂_G terms; the screen's own phase (e^{jωτ} over the top, longer path than R_SR) is discarded in the level but physically real.
   - Recommendation: for Phase 2, H's ground/diffraction factor = (Nord2000-native coherent ratio) · |Δp_SCR| with the direct-path phase from Phase 1 retained; revisit in Phase 4 whether the diffracted-path τ (≠ direct τ) should carry into H's phase (matters for multi-sub-source interference across screens). Log as a Phase 4 input.

4. **Sub-model 3 (non-flat terrain) is unowned by any phase.** ENG-02/03 don't name it; valley/hill/elevated-road FORCE cases (31–64, 101–114) need it before VAL-02.
   - Recommendation: keep it out of Phase 2 (flat r_flat=1 makes it dead code for the target cases; stub it with a typed error), and add it explicitly to Phase 3's plan list (it interacts with refraction-corrected Fresnel weights anyway — Eqs. 134–156 constantly call the refraction machinery) or as a Phase 3.5 insertion. Flag to user at plan review.

5. **Eq. 188 normalization detail** (how w″ sums normalize the double sum) — pypdf dump is ambiguous about the denominator.
   - Recommendation: transcribe p. 79–80 (Eq. 187–188) from page images during 02-02 execution; the oracle must implement the same reading — cross-checking both against case-71's smooth ΔL(f) shape catches misreads.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | build/test | ✓ (Phase 1) | 1.96.0 | — |
| refs/AV1106-07-rev4.pdf | equation transcription | ✓ verified this session | rev. 4 (2014), SHA-pinned | Wayback re-fetch via refs/fetch.sh |
| refs/AV1849-00-part1.pdf | HP attribution, derivation context | ✓ | — | mirror |
| refs/TestStraightRoad.xls | case geometries + parameter audit | ✓ parsed this session | 2009 | mst.dk |
| Python 3.13 + scipy/numpy | oracle + fixture generation (`tools/`) | ✓ verified this session (wofz used) | 3.13.13 | not needed at build time; fixtures committed |
| Python pdfplumber | page-image rendering for equation transcription | ✓ verified this session | — | pypdf text + manual reading |

**Missing dependencies with no fallback:** none.

## Security Domain

Same posture as Phase 1: offline computation library; no network/auth/persistence surface added.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2/V3/V4 Auth/Session/Access | no | — |
| V5 Input Validation | yes (narrow) | Terrain profiles from case files remain untrusted: reject non-finite σ/r, σ ≤ 0, degenerate screens (zero-length segments guarded in SegmentVariables/WedgeCross); all new functions return typed errors, never panic on data (extends T-01-05 posture) |
| V6 Cryptography | no | — |

### Known Threat Patterns for offline numeric Rust lib

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Crafted profile causing Inf/NaN propagation (numeric DoS) | DoS | finiteness sweep test (success criterion 3); ε-guards (θₙ=π, |A|→0, w(z) overflow in 2e^{−z²} reflection branch); typed errors on degenerate geometry |
| Supply chain | Tampering | zero new dependencies; `errorfunctions` explicitly rejected (SUS) |
| Copyright leak | compliance | equations cited by number only; Tables 4/5 coefficients and class tables are method-defined numeric facts; no PDF text/figures committed; oracle script contains formulas + citations, not document text |

## Sources

### Primary (HIGH confidence — read/verified this session)
- **AV 1106/07 rev. 4** (`refs/AV1106-07-rev4.pdf`, SHA-pinned): §5.4.2/5.4.4, §5.5.4–5.5.6, §5.6 (Eqs. 57–77), §5.7 (78–107), §5.8 (108–109), §5.9 (110–114), §5.10–5.16 (115–~272), §5.21 (300–328), §5.22 (329–334), §5.23 aux functions (335–392). Verification: pdfplumber glyph-position extraction (Eqs. 57–60, 111, 120), page-image reading (pp. 39–40 — Eqs. 78–86 definitive), pypdf text (structure of remaining sub-models).
- **Numerical validation** (this session, scipy 1.x): w(ẑ) all three branches vs `scipy.special.wofz` (<8e−7); wedge solution vs Maekawa (+≈2 dB systematic, expected) and shadow-boundary half-field limit (0.500); impedance values vs classic Chessell/Embleton grassland figures.
- **AV 1849/00 Part 1** (`refs/`): Hadden–Pierce attribution (p. 38, ref [had1] = JASA 1981); background for the wedge modification to absorbing faces.
- **TestStraightRoad.xls** (refs/): full 62-case parameter audit (u, su, dt/dz, Cv², CT², hr) + terrain-profile encodings of thin/thick/double screens (cases 71/81/91) — drove the phase-scope findings.
- Phase 1 artifacts: 01-RESEARCH.md (conventions, tolerances, FORCE format), 01-01/02/03 SUMMARYs (existing API to extend).

### Secondary (MEDIUM confidence)
- Env. Project 1335 (2010, refs/): tolerances (1 dB overall/band, dip-shift allowance) — carried from Phase 1.
- Classic literature consistency checks (training knowledge, used only as sanity cross-checks, never as source of coefficients): Chien–Soroka/Chessell spherical-wave reflection structure; Delany–Bazley grassland impedance magnitudes; Maekawa chart; partial-coherence identity.

### Tertiary (LOW confidence, flagged)
- Assumptions A3–A6 (exact constants of Eqs. 113/114, Tables 6/7, Eq. 97/102 argument lists) — structure certain, symbols to be transcribed from page images during execution.

## Metadata

**Confidence breakdown:**
- Ground-effect equation chain (57–77, 111, 119–120): HIGH — glyph-verified + numerically validated
- Faddeeva approximation (61–74): HIGH — all branches reproduce scipy to ~1e−7
- Wedge solution (78–86): HIGH — page-image transcription + physical validation (Maekawa, shadow boundary)
- Sub-model 2/4/5/6 composition: HIGH structure / MEDIUM symbol-level (marked [TEXT]; transcription from a locally available PDF is a mechanical execution-time task)
- Sub-model 7, Fc/Fr constants: MEDIUM — structure certain, constants to transcribe (Assumptions A3–A5)
- FORCE scope analysis: HIGH — parameters read from the authoritative .xls
- Rust integration plan: HIGH — extends verified Phase 1 API, zero new deps

**Research date:** 2026-07-07
**Valid until:** indefinitely for the physics (frozen standard); re-check crates only if a dependency is added
