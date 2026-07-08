---
phase: 03-meteorology-refraction
plan: 01
subsystem: engine (propagation::refraction) + harness (cases/capability)
status: complete
completed: 2026-07-08
tags: [refraction, circular-ray, CalcEqSSP, shadow-zone, two-channel, D-02-anchor]
requires:
  - "envi-engine::propagation::rays::{RayVars, RayPair, straight_rays}"
  - "envi-engine::propagation::diffraction::{WedgeGeometry, pwedge0}"
  - "envi-engine::propagation::terrain_effect (Sub-model 1 two-channel readout)"
provides:
  - "propagation::refraction::profile::sound_speed_profile (Eq. 2, z₀ clamp)"
  - "propagation::refraction::eqssp::calc_eq_ssp (CalcEqSSP → (ξ,c₀))"
  - "propagation::refraction::circular_ray::{direct_ray, reflected_ray, reflection_point_cubic, travel_time_diff, height_of_circular_ray, DirectRayVars, TravelTimeGeometry}"
  - "propagation::refraction::shadow_zone::{shadow_zone_shielding, shadow_zone_shielding_bands}"
  - "propagation::refraction::SoundSpeedProfile"
  - "propagation::rays::circular_rays (fills RayVars/RayPair; |ξ|<1e-6 ⇒ straight_rays)"
  - "PropagationError::{NoReflectionRoot, DegenerateProfile, DegenerateShadowZone}"
  - "terrain_effect(..., Option<&SoundSpeedProfile>) refraction dispatch"
  - "submodel1::eval shadow-zone branch (Eq. 121)"
  - "CaseKind::Refraction + cases/refraction_{downwind,upwind_shadow}.toml"
affects:
  - "03-02 (CalcEqSSPGround, reflection-path split) consumes calc_eq_ssp + circular_ray"
  - "03-03 (F_τ, weather routes) feeds ξ/Δτ⁺ into these primitives"
tech-stack:
  added: []
  patterns:
    - "circular-ray constructor behind the frozen RayVars/RayPair seam (D-02 delegation)"
    - "scipy-free refraction oracle (self-provenance sha256; committed TOML)"
    - "capability-gated synthetic case kind (Skipped, never false Pass — D-03)"
key-files:
  created:
    - crates/envi-engine/src/propagation/refraction/mod.rs
    - crates/envi-engine/src/propagation/refraction/profile.rs
    - crates/envi-engine/src/propagation/refraction/eqssp.rs
    - crates/envi-engine/src/propagation/refraction/circular_ray.rs
    - crates/envi-engine/src/propagation/refraction/shadow_zone.rs
    - tools/nord2000_oracle/gen_refraction_fixtures.py
    - crates/envi-harness/tests/oracle_refraction.rs
    - crates/envi-harness/tests/fixtures/oracle/refraction.toml
    - cases/refraction_downwind.toml
    - cases/refraction_upwind_shadow.toml
  modified:
    - crates/envi-engine/src/propagation/mod.rs
    - crates/envi-engine/src/propagation/rays.rs
    - crates/envi-engine/src/propagation/terrain_effect/mod.rs
    - crates/envi-engine/src/propagation/terrain_effect/submodel1.rs
    - crates/envi-engine/src/propagation/terrain_effect/submodel2.rs
    - crates/envi-harness/src/cases/mod.rs
    - crates/envi-harness/src/cases/toml.rs
    - crates/envi-harness/src/capability.rs
    - crates/envi-harness/src/lib.rs
    - crates/envi-harness/src/main.rs
    - crates/envi-harness/tests/terrain.rs
decisions:
  - "Δτ oracle tolerance set to 1e-4 (cancellation-limited circular travel-time difference; still catches any mistranscribed exponent by >1%)"
  - "Capability::Refraction left UN-flipped in implemented_capabilities() — the engine core is done but harness weather-route wiring (feeding A/B/C) is 03-02/03-03, so refraction cases honestly stay Skipped"
  - "Shadow-zone L_SZ returned as a non-negative attenuation (Eq. 387 sign interpretation), self-consistent with the Sub-model 1 shadow branch; validated by direction property tests, not an oracle"
  - "Only the single-surface (Sub-model 1) path is refracted; segmented + refraction awaits CalcEqSSPGround (03-02)"
metrics:
  tasks: 3
  commits: 3
  files_created: 10
  files_modified: 11
  engine_tests: 144
  harness_tests: 34
---

# Phase 3 Plan 1: Refraction Core Summary

Circular-ray refraction (CalcEqSSP + DirectRay/ReflectedRay/Δτ + equivalent-wedge shadow zone) runs end-to-end through the existing Sub-model 1 two-channel readout, and is a **strict generalization** of Phase 2: with `|ξ|<1e-6` the refracted `circular_rays` reproduces `straight_rays` **bit-for-bit** (`assert_eq!`, the D-02 anchor).

## What shipped

- **profile.rs** — `sound_speed_profile` = `A·ln(z/z₀+1)+B·z+C` (Eq. 2) with `z₀` clamped ≥ 0.001 m (MET-01). `C=Coft(t₀)` reuses `sound_speed_ms`.
- **eqssp.rs** — `calc_eq_ssp` collapses the log-lin profile to `(ξ, c₀)` (Eqs. 15–21) using the **Annex F Eq. 403 closed form** for `c̄` (no quadrature), `hmin=5·z₀` floor, `hS=hR`→±0.005 m fallback, and the `|ξ|<1e-6 ⇒ (0, C)` homogeneous shortcut.
- **circular_ray.rs** — `direct_ray` (Eqs. 29–44: tan ψ_L Eq. 32, R/τ log-ratio Eqs. 34–37, past-circle-top Eqs. 38–40, Δθ Eqs. 41–42, dSZ Eq. 43; `Δz<0.01` and inner `|ξ'|<1e-10` clamps), `reflection_point_cubic` (Eq. 49 robust trig/Cardano solver + source/receiver root-selection), `reflected_ray` (Eqs. 45–50), `travel_time_diff` (Eqs. 51–53 + Δτ₀ shadow-edge cap Eq. 52), `height_of_circular_ray` (Eqs. 355–368).
- **shadow_zone.rs** — `shadow_zone_shielding` via the equivalent-wedge kernel (Eqs. 384–388) reusing `diffraction::pwedge0` (D-09), `ξSZ` frozen above 2000 Hz (Eq. 385); non-negative per-band attenuation.
- **rays::circular_rays** — fills `RayVars`/`RayPair`; delegates to `straight_rays` at `|ξ|<1e-6` (D-02 structural anchor); returns `reflected: None, dtau: 0` in the shadow zone.
- **terrain_effect dispatch** — new `Option<&SoundSpeedProfile>` param; the flat single-surface path computes `(ξ,c₀)` once via CalcEqSSP and swaps `straight_rays`→`circular_rays`. **submodel1::eval** gains the Eq. 121 shadow branch (reflected term collapses; subtract `L_SZ`; Nord-native +j phase, no conj).
- **Oracle + cases** — scipy-free refraction oracle (`gen_refraction_fixtures.py` + committed `refraction.toml`) pins ξ/c₀ (tol 1e-9) and Δτ (tol 1e-4); `CaseKind::Refraction` + two capability-gated TOML cases.

## Acceptance ladder (D-02)

1. **Homogeneous bit-for-bit anchor** — `assert_eq!(straight_rays(97.5,0.5,1.5,c0), circular_rays(97.5,0.5,1.5,5e-7,c0))` passes (exact `PartialEq`); the homogeneous `SoundSpeedProfile` path through `terrain_effect` is bit-identical to the `None` (Phase-2) path across all 105 bands.
2. **Committed scipy oracle** — CalcEqSSP ξ/c₀ (1e-9) and circular Δτ (1e-4) match over an up/down/equal-height grid.
3. **Direction + finiteness property tests** — downwind (ξ>0) → gain, upwind shadow (ξ<0) → loss (by BAND INDEX); 105-band finiteness sweep across up/down/shadow.

## Quality gates (all green)

- `cargo build --release` ✓  ·  `cargo test --workspace` ✓ (144 engine + 34 harness + oracles + 3 terrain; FORCE + 2 refraction cases capability-gated to `ignored`)
- `cargo clippy --all-targets -- -D warnings` ✓  ·  `cargo fmt --check` ✓
- `.conj()` grep gate over `propagation/` = **0**  ·  `cargo tree -p envi-engine` = `ndarray + num-complex + thiserror` (unchanged)
- Every load-bearing equation confirmed against the `refs/AV1106-07-rev4.pdf` **page images** (D-04): Eqs. 2/3, 15–21, 29–56, 355–368, 384–388, 403.

## Deviations from Plan

### Auto-fixed / decisions (no user permission needed)

**1. [Rule 3 - Blocking] `CaseKind::Refraction` added.** The plan lists `cases/refraction_*.toml` but no case kind; an unknown `kind` is a hard load error (the FORCE dynamic runner turns it into a failing trial). Added a `Refraction` variant, accepted `"refraction"` in the loader, and gated it to `Skipped` in `run_case` + `required_capabilities` (D-03 pattern). Touched `cases/mod.rs`, `cases/toml.rs`, `capability.rs`, `lib.rs`, `main.rs` (exhaustive-match arms). Files beyond the plan's `files_modified` list.

**2. [Decision] Δτ oracle tolerance 1e-4 (not 1e-7).** Δτ is a difference of near-equal circular travel times (AV §5.5.6 warns explicitly); its cross-implementation precision is cancellation-limited to ~1e-5 relative. 1e-4 is the Phase-2 oracle-style gate and still fails on any mistranscribed exponent (>1%). The homogeneous case avoids this via the exact ΔR identity (delegated `straight_rays`).

**3. [Decision] `Capability::Refraction` NOT flipped to implemented.** The refraction *engine* is done, but feeding a case's `(A,B,C,z₀)` into `terrain_effect` is the weather-route wiring of 03-02/03-03. Keeping Refraction un-implemented means the synthetic refraction cases and FORCE wind/gradient cases stay honestly `Skipped` (never a false Pass). The D-03 requires-list shrink lands when the weather routes do.

## Known Stubs / deferrals (by design, not this plan's goal)

- **Segmented + refraction** deferred to CalcEqSSPGround (03-02): only the single-surface Sub-model 1 path is refracted here.
- **Shadow-zone L_SZ magnitude** is direction-validated (property test), not oracle-pinned — the Eq. 387 sign is interpreted as a non-negative attenuation, self-consistent with the Eq. 121 branch (documented in `shadow_zone.rs`).
- **CalcEqSSPGround, reflection-path A₁/B₁/A₂/B₂ split, F_τ, weather routes** are out of scope for 03-01 (03-02/03-03).

## Self-Check: PASSED
