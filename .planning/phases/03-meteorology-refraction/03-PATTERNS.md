# Phase 3: Meteorology & Refraction - Pattern Map

**Mapped:** 2026-07-08
**Files analyzed:** 20 (11 new, 9 modified)
**Analogs found:** 20 / 20 (every new file is a strict generalization of a Phase-1/2 shape)

> **Core insight (from RESEARCH):** refraction in Nord2000 is a *transformation of the ray inputs* to already-built sub-models, not a new propagation model. Almost every new file has a near-exact Phase-2 analog whose module shape, error handling, convention block, and test layout must be copied. The load-bearing one is `rays.rs` — the circular-ray constructor must fill `RayVars`/`RayPair` behind **identical fields** so the `|ξ|<1e-6` homogeneous shortcut reproduces `straight_rays` bit-for-bit (D-02).

---

## File Classification

### Engine — `crates/envi-engine/src/propagation/`

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `refraction/mod.rs` (NEW) | module/error-hub | — | `propagation/mod.rs` (error enum + `pub mod`) | role-match |
| `refraction/profile.rs` (NEW) | pure-numeric primitive | transform | `propagation/mod.rs::sound_speed_ms` + `ground.rs` clamp posture | role-match |
| `refraction/eqssp.rs` (NEW) | pure-numeric primitive | transform | `ground.rs` (typed-Result numeric fn) + `submodel2.rs::phase_diff_freq` (fL/fH) | exact (two analogs) |
| `refraction/circular_ray.rs` (NEW) | ray constructor | transform | `rays.rs::straight_rays` (IDENTICAL output struct) | **exact — frozen seam** |
| `refraction/shadow_zone.rs` (NEW) | numeric primitive | transform | `diffraction.rs::pwedge0`/`WedgeGeometry` (reuses the kernel) | exact |
| `rays.rs` (EXTEND) | ray constructor | transform | itself — add `circular_rays` beside `straight_rays` | exact |
| `coherence.rs` (EXTEND) | numeric primitive | transform | itself — add `coherence_f_delta_nu` beside `coherence_ff` | exact |
| `terrain_effect/submodel1.rs` (EXTEND) | sub-model wiring | transform | itself — `eval` shadow-zone branch (Eq. 121) | exact |
| `terrain_effect/mod.rs` (EXTEND) | dispatch | transform | itself — swap `straight_rays`→`circular_rays` at L268 | exact |
| `propagation/mod.rs` (EXTEND) | module/error-hub | — | itself — new `PropagationError` variants + `pub mod refraction` | exact |

### Harness — `crates/envi-harness/src/`

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `weather/mod.rs` (NEW) | I/O + profile struct | transform | `cases/mod.rs` (typed error + struct-of-inputs) | role-match |
| `weather/route1.rs` (NEW) | I/O (xls) | batch/aggregate | `cases/xls.rs` (calamine label-anchored read) | role-match |
| `weather/route2.rs` (NEW) | transform | transform | `cases/mod.rs::PropagationParams` consumer | role-match |
| `weather/route3.rs` (NEW) | transform (LSQ) | transform | (no direct analog — hand-rolled 3×3; see No Analog) | partial |
| `capability.rs` (EXTEND) | config/gate | — | itself — flip `Refraction` in `implemented_capabilities` | exact |
| `cases/mod.rs` (EXTEND) | model | — | itself — nonzero-turbulence default on `PropagationParams` | exact |

### Tools & fixtures

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `tools/nord2000_oracle/gen_refraction_fixtures.py` (NEW) | test-fixture generator | batch | `gen_ground_fixtures.py` | exact |
| `crates/envi-harness/tests/oracle_refraction.rs` (NEW) | test | request-response | `tests/oracle_ground.rs` | exact |
| `cases/refraction_*.toml` (NEW) | test data | — | `cases/terrain_flat_sigma200.toml` | exact |

---

## Pattern Assignments

### `refraction/circular_ray.rs` (ray constructor, transform) — THE frozen seam

**Analog:** `crates/envi-engine/src/propagation/rays.rs` (read in full)

This is the load-bearing D-02 file. The output structs `RayVars`/`RayPair` are **already defined** in `rays.rs` and must NOT be re-declared — `circular_rays` fills the identical fields.

**Struct to fill — DO NOT redefine** (`rays.rs` lines 29-56):
```rust
pub struct RayVars { pub tau: f64, pub r: f64, pub psi_g: f64, pub r1: f64, pub r2: f64 }
pub struct RayPair { pub direct: RayVars, pub reflected: Option<RayVars>, pub dtau: f64 }
```

**Homogeneous-shortcut delegation pattern** (RESEARCH lines 483-492 spells the target; copy the guard shape from `straight_rays` lines 73-88). The circular entry point delegates to `straight_rays` below the `1e-6` clamp so D-02 is *structural*, not a parallel reimplementation:
```rust
pub fn circular_rays(d: f64, h_s: f64, h_r: f64, xi: f64, c0: f64) -> Result<RayPair, PropagationError> {
    if xi.abs() < 1e-6 { return straight_rays(d, h_s, h_r, c0); }  // exact Phase-2 path (D-02)
    // ... DirectRay Eqs. 29–44, ReflectedRay Eqs. 45–50, cubic Eq. 49, Δτ Eqs. 51–53 ...
}
```

**Input-validation guard block to copy** (`rays.rs` lines 73-88) — the exact finite/positive checks returning `DegenerateRayGeometry { detail }`:
```rust
if !(d.is_finite() && d > 0.0) {
    return Err(PropagationError::DegenerateRayGeometry { detail: "..." });
}
let height_ok = |h: f64| h.is_finite() && h >= 0.0;
if !(height_ok(h_s) && height_ok(h_r)) { return Err(...); }
if !(c0.is_finite() && c0 > 0.0) { return Err(...); }
```

**Cancellation-free ΔR to extend** (`rays.rs` lines 93-94, 174-184) — the flat form `ΔR = 4·hS·hR/(R₁+R₂)` and the sloped-segment `(S′−S)·(S′+S−2R)/(R₂+R₁)` form. For the circular case carry the τ-difference through the `f(0)/f(z)` log-ratio (Eq. 35) — see RESEARCH Pitfall 2 (lines 515-518).

**Test layout to copy** (`rays.rs` lines 205-282): a research-geometry anchor with `assert_relative_eq!`, a cancellation regression at `hS=0.01, hR=1.5, d=1000`, and a `matches!(…, Err(DegenerateRayGeometry))` domain-error test. **Add the D-02 bit-for-bit test** (RESEARCH lines 574-583) using `assert_eq!(straight_rays(...), circular_rays(..., 5e-7, ...))` — exact `PartialEq`, not approx.

---

### `refraction/eqssp.rs` (numeric primitive, transform)

**Analog A (module shape + typed Result):** `crates/envi-engine/src/propagation/ground.rs` lines 1-49

**Convention doc-block to copy** (`ground.rs` lines 6-18) — the "Nord2000-native e^{−jωt}" note and the "Deviation from the plan interface block" note explaining why the fn returns `Result` (untrusted input → typed error, never NaN):
```rust
//! # Convention
//! Nord2000-native: time e^{−jωt}, impedance Im > 0 (do NOT flip the sign here —
//! conversion to ENVI's e^{+jωt} convention is a single conj() at the boundary).
```

**Typed-Result numeric-fn pattern** (`ground.rs` lines 40-49):
```rust
pub fn ground_impedance(f_hz: f64, sigma_kpa: f64) -> Result<Complex<f64>, PropagationError> {
    if !(sigma_kpa.is_finite() && sigma_kpa > 0.0) {
        return Err(PropagationError::InvalidFlowResistivity { sigma_kpa });
    }
    // ...
}
```
CalcEqSSP's `c̄` closed form (Annex F Eq. 403) is given ready-to-paste in RESEARCH lines 564-569. Guards: `hmin=5·z₀`, `hS=hR` → ±0.005 m fallback, `|ξ|<1e-6 ⇒ ξ=0, c̄=c₀=C`.

**Analog B (fL/fH frequency interpolation for CalcEqSSPGround):** `crates/envi-engine/src/propagation/terrain_effect/submodel2.rs` — `phase_diff_freq` at line 134 (Eqs. 378-381) is the exact `PhaseDiffFreq` aux CalcEqSSPGround reuses (Eqs. 24-27). Reuse it verbatim; only the target phase (Ψ vs 2Ψ) differs. Branch soft/hard on `σ < 1e7 Pa·s·m⁻²` (RESEARCH Pitfall 4). **Compare by band index** (D-14), evaluate at all 105 grid points.

---

### `refraction/profile.rs` (numeric primitive, transform)

**Analog:** `propagation/mod.rs::sound_speed_ms` (lines 111-114) — the `#[must_use] pub fn … -> f64` pure-primitive shape; `c(z)=A·ln(z/z₀+1)+B·z+C` (Eq. 2). Reuse `sound_speed_ms` for `C=Coft(t₀)` (Eq. 3) — **do not reintroduce 340.29**. Clamp `z₀ ≥ 0.001 m` (MET-01).

---

### `refraction/shadow_zone.rs` (numeric primitive, transform)

**Analog:** `crates/envi-engine/src/propagation/diffraction.rs` — `WedgeGeometry` (lines 33-52), `pwedge0` (line 355), `dwedge0` (line 368), and the `validate` reject-before-sqrt pattern (line 96).

ShadowZoneShielding (Eqs. 384-388) is AV-*defined* as diffraction over an equivalent wedge — **reuse `pwedge0`/`dwedge0`**, do not build a new diffraction primitive (RESEARCH "Don't Hand-Roll" line 397). Build `HeightOfCircularRay` (the wedge height `hSZ`) in `circular_ray.rs` and pass the equivalent `WedgeGeometry` into `pwedge0`. Freeze `ξSZ` above 2000 Hz (Eq. 385). **Executor must transcribe Eqs. 387-388 from the p.158-159 page images** (RESEARCH Assumption A6).

---

### `coherence.rs` (EXTEND — numeric primitive, transform)

**Analog:** itself — `coherence_ff` (lines 63-73) is the sinc template `F_τ` copies.

**F_τ helper to add** (ready-to-paste in RESEARCH lines 553-559) — same sinc form as `coherence_ff` but argument `2π·f·(Δτ⁺−Δτ)` (note **2π, NOT the 0.23π of Ff** — RESEARCH Pitfall 5):
```rust
pub fn coherence_f_delta_nu(f_hz: f64, dtau: f64, dtau_plus: f64) -> f64 {
    let x = 2.0 * std::f64::consts::PI * f_hz * (dtau_plus - dtau).abs();
    if x <= 1e-15 { 1.0 } else if x <= std::f64::consts::PI { x.sin() / x } else { 0.0 }
}
```

**Injection seam — NO call-site change** (`coherence.rs` lines 52-53 and 118): the result is passed as `CoherenceInputs::f_delta_nu` (currently `1.0`); `coherence_f` already multiplies it into the chain at line 118 (`ff * inputs.f_delta_nu * fc * fr * fs`). `sA=sB=0 ⇒ Δτ⁺=Δτ ⇒ 1.0` exactly (D-12).

**Property-test pattern to copy** (`coherence.rs` lines 169-193) — `turbulence_reduces_f_monotonically`: sweep freqs, assert `F ≤ Ff`, non-increasing, strict `<` at high f. Mirror this for F_τ direction (D-11) — **no fixed-value oracle** for the turbulence constants.

---

### `terrain_effect/submodel1.rs` (EXTEND — sub-model wiring, transform)

**Analog:** itself — the `eval` function (lines 70-127).

**Nord-native +j phase (NO conj)** to preserve in the shadow branch (lines 114-117):
```rust
// NORD-NATIVE +j sign (e^{−jωt}); no conj here (that boundary is transfer.rs).
let phase = Complex::from_polar(1.0, TAU * f_hz * rays.dtau);
let h_coh_factor = Complex::new(1.0, 0.0) + f_coh * ratio * phase * q;
```
**Two-channel readout to preserve** (lines 119-126): `p_incoh = (1.0 - f²)·(ratio·ρᵢ)²` then `GroundResult::from_channels(h_coh_factor, p_incoh)`. The shadow-zone branch (Eq. 121) collapses the reflected term and subtracts `L_SZ` from `shadow_zone.rs`; `F_τ` rides in via the unchanged `coh` input. `SubModel1Inputs` (lines 38-47) already carries weather params inside `CoherenceInputs` "so Phase 3 attaches without an API break" (line 35).

---

### `terrain_effect/mod.rs` (EXTEND — dispatch, transform)

**Analog:** itself — the `terrain_effect` dispatch and the `straight_rays` call site at line 268:
```rust
let rays = straight_rays(self.d, self.h_s, self.h_r, self.c0)?;   // → swap to circular_rays under refraction
```
Swap to `circular_rays(d, h_s, h_r, xi, c0)` when refraction is active; the `|ξ|<1e-6` shortcut keeps the homogeneous cases bit-identical. The `NonFlatTerrainNotImplemented` typed-error branch (lines 161-163) is the model for gating not-yet-done paths.

---

### `propagation/mod.rs` (EXTEND — error hub)

**Analog:** itself — the `PropagationError` enum (lines 48-107) and module declarations (lines 28-37).

Add `pub mod refraction;` beside the existing `pub mod` block (lines 28-37). Add new typed variants following the exact `#[error("…")]` + doc-comment style of `DegenerateRayGeometry` (lines 83-90): e.g. `NoReflectionRoot` (cubic empty-root, RESEARCH line 271), `DegenerateProfile`, `ShadowZone…`. Note `NonFlatTerrainNotImplemented` (lines 91-106) is documented as "scheduled with Phase 3" — reconcile if Sub-model 3 stays deferred (RESEARCH Open Q3).

---

### `capability.rs` (EXTEND — gate)

**Analog:** itself. `Capability::Refraction` already exists (line 30) and `required_capabilities` already flags it off nonzero `u`/`dt/dz` (lines 85-92). **Only two edits:**

1. Add `Capability::Refraction` to `implemented_capabilities()` (lines 108-115).
2. Update the requires-shrink assertion tests. The exact test template is `force_skip_reason_no_longer_mentions_ground_effect_or_diffraction` (lines 209-234) — clone it so a wind/gradient FORCE case's `missing` set drops `refraction` but **retains `emission-model`** (D-03), and flip `plan_02_05_implements…`'s `assert!(!…Refraction)` (line 205) to assert present.

---

### `weather/mod.rs` + `route1.rs` + `route2.rs` + `route3.rs` (NEW — harness I/O quarantine, D-15)

**Analog (typed error + struct):** `crates/envi-harness/src/cases/mod.rs` — `CaseLoadError` (lines 41-120) is the typed-error template every route module reuses; `PropagationParams` (line 196: `t0_c`, `u_ms`, `phi_deg`, `su_ms`, `dtdz`, `sdtdz`, `cv2`, `ct2`, roughness) is the **input** Route 2/3 read.

**Route 1 xls analog:** `cases/xls.rs` (calamine, label-anchored) — read `TestYearlyAverage.xls` `Met. statistics` sheet; reuse the row/sheet caps and `NonFinite`/`MissingLabel` errors. Energy-weight per class: `L_den = 10·lg(Σ p·10^(L/10))`.

**Route 3 boundary shape** (D-08 / RESEARCH lines 386-388): `fit_profile(heights, c_eff_samples, z₀) → (A,B,C)` — hand-rolled 3×3 normal equations (linear in A,B,C), **no `nalgebra`** (RESEARCH "Don't Hand-Roll" line 399; a `checkpoint:human-verify` guards any linalg-crate proposal, line 148).

> ⚠️ All three routes' A/B/C derivation constants are **`[ASSUMED]`** — AV 1106/07 does not specify them and the origin `docs/research.md` is absent (RESEARCH lines 356-358, Open Q1). Build the *machinery* now; quarantine the constants behind the planner's `checkpoint:human-verify`.

---

### `tools/nord2000_oracle/gen_refraction_fixtures.py` + `tests/oracle_refraction.rs` (NEW)

**Analog:** `tools/nord2000_oracle/gen_ground_fixtures.py` + `crates/envi-harness/tests/oracle_ground.rs` (read in full).

**Generator pattern** (`gen_ground_fixtures.py` lines 1-40): module docstring stating "regeneration is operator-driven; the TOML is committed and Python/scipy are NOT build deps", `sha256(common.py)` provenance in the fixture header, geometry-anchor constants, then a grid of points → committed TOML.

**Test pattern** (`oracle_ground.rs` lines 15-105): `#[derive(Deserialize)]` fixture structs, `load()` from `concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/oracle/…toml")`, `rel_err()` helper, per-row `assert!(err <= tol)`. Pin ξ / Δτ / CalcEqSSP / CalcEqSSPGround (D-02 rung b). **Do NOT pin F_τ** (D-11).

---

## Shared Patterns

### Nord2000-native convention (zero `.conj()` in `propagation/`)
**Source:** `crates/envi-engine/src/propagation/ground.rs` lines 6-18 (doc-block) + `submodel1.rs` lines 114-116 (the +j phase). **Apply to:** every new `refraction/*` file and the `coherence`/`submodel1` extensions.
```rust
// NORD-NATIVE +j sign (e^{−jωt}); no conj here.
let phase = Complex::from_polar(1.0, TAU * f_hz * rays.dtau);
```
The single conjugation stays at `transfer.rs::nord_ratio_to_transfer` (lines 90-91: `ratio.conj()`). Write any conjugation in `propagation/` as explicit `Complex::new(re, -im)` (D-13). Most refraction math is real anyway.

### Two-channel readout (H_coh complex + P_incoh real)
**Source:** `transfer.rs::band_levels_db_two_channel` (line 115) + `submodel1.rs` lines 119-126. **Apply to:** the shadow-zone branch and any refracted path. `F→1 ⇒ P_incoh→0` bit-exact; F_τ multiplies `H_coh`, never overwrites phase (D-12).

### Typed-error, never-panic on data
**Source:** `propagation/mod.rs` `PropagationError` (lines 48-107); harness `CaseLoadError` (`cases/mod.rs` lines 41-120). **Apply to:** all new engine fns (`Result<_, PropagationError>` with finite/positive guards) and all weather-route I/O (`Result<_, CaseLoadError>`). Cubic no-root → typed error (ASVS V5, RESEARCH line 689).

### Input-guard block (finite + positive)
**Source:** `rays.rs` lines 73-88. **Apply to:** every new numeric constructor before any sqrt/division — reject non-finite `A/B/C/z₀/u/dt-dz`, `z₀≤0` (clamp ≥0.001), `hR=hS` (±0.005 fallback, not divide-by-zero).

### Injected seam (extend interface explicitly, document the deviation)
**Source:** `coherence.rs` lines 26-31 (`d_m` deviation note) + lines 52-53 (`f_delta_nu`). **Apply to:** F_τ (fill the pre-built `f_delta_nu` seam — no call-site change) and any new `CoherenceInputs`/`SubModel1Inputs` field.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `weather/route3.rs` (LSQ 3×3 fit) | transform | transform | No least-squares / linear-solve code exists in the repo yet. Hand-roll `XᵀXβ=Xᵀy` closed-form (RESEARCH line 134/399). Nearest shape guidance: keep the `fit_profile(heights, c_eff, z₀)` boundary clean for v2 METX. |
| reflection-point cubic solver (in `circular_ray.rs`) | numeric primitive | transform | The one genuinely new numerical primitive (Eq. 49) — no existing cubic/root-finder. Cardano/trig with discriminant branch + root-selection rule (closest to source if hS<hR); homogeneous root cross-checks `geometry::reflect_over_segment` (RESEARCH lines 264-272, Pitfall 3). |

*(Both live inside files that otherwise have strong analogs; only these two algorithms are net-new. Everything else copies a Phase-1/2 shape.)*

---

## Metadata

**Analog search scope:** `crates/envi-engine/src/propagation/` (rays, coherence, ground, diffraction, fresnel, submodel1/2, terrain_effect/mod, mod), `crates/envi-engine/src/transfer.rs`, `crates/envi-harness/src/` (capability, cases/mod, cases/xls), `tools/nord2000_oracle/`, `tests/oracle_*.rs`, `cases/*.toml`.
**Files scanned:** ~18 source + 2 fixture/test.
**Pattern extraction date:** 2026-07-08
