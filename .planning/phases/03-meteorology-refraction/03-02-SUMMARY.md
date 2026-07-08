---
phase: 03-meteorology-refraction
plan: 02
subsystem: engine (propagation::refraction::eqssp) + harness (weather routes)
status: complete
completed: 2026-07-08
tags: [refraction, CalcEqSSPGround, weather-route, MET-02, MET-04, ENG-06, per-azimuth-A, reflection-split, ASSUMED]
requires:
  - "envi-engine::propagation::refraction::eqssp::calc_eq_ssp (03-01 frequency-independent base)"
  - "envi-engine::propagation::terrain_effect::submodel2::phase_diff_freq (Phase-2 PhaseDiffFreq aux)"
  - "envi-engine::propagation::refraction::profile::{sound_speed_profile, Z0_MIN_M}"
  - "envi-engine::propagation::sound_speed_ms (Coft, Eq. 335)"
  - "envi-engine::geometry::{azimuth_deg, reflect_over_segment}"
  - "envi-harness::cases::{PropagationParams, CaseLoadError}"
provides:
  - "propagation::refraction::eqssp::calc_eq_ssp_ground (Eqs. 22–28, soft/hard branch, per band index)"
  - "propagation::refraction::eqssp::SOFT_GROUND_SIGMA_KPA (10⁴ kPa·s·m⁻² threshold)"
  - "envi-harness::weather::{WeatherProfile, WeatherComponents, profile_for_bearing, SubPathCollapse}"
  - "envi-harness::weather::ReflectionProfiles (before/after split + sub_path_collapses)"
  - "envi-harness::weather::route2::{route2, route2_components, route2_reflection}"
  - "envi-harness::cases::PropagationParams::turbulence_or_nord2000_default (D-11 seam)"
  - "envi-harness::cases::{NORD2000_DEFAULT_CV2, NORD2000_DEFAULT_CT2}"
  - "refraction.toml oracle: [[eqssp_ground]] rows pinned by band index"
affects:
  - "03-03 (F_τ + Routes 1/3) consumes WeatherProfile/WeatherComponents + the turbulence default seam"
  - "Phase 4 obstacle-reflection cases consume ReflectionProfiles (A₁/B₁, A₂/B₂)"
tech-stack:
  added: []
  patterns:
    - "shared eq_ssp_terms/collapse_gradient helpers so CalcEqSSPGround overrides only the gradient"
    - "bearing-independent WeatherComponents decomposition: temperature-once + wind-per-bearing projection"
    - "[ASSUMED]-quarantined weather-route constants validated by direction/structure property tests only"
    - "independent scipy oracle for CalcEqSSPGround (PhaseDiffFreq reimplemented) pinned by band index"
key-files:
  created:
    - crates/envi-harness/src/weather/mod.rs
    - crates/envi-harness/src/weather/route2.rs
  modified:
    - crates/envi-engine/src/propagation/refraction/eqssp.rs
    - crates/envi-harness/src/lib.rs
    - crates/envi-harness/src/cases/mod.rs
    - crates/envi-harness/tests/oracle_refraction.rs
    - crates/envi-harness/tests/fixtures/oracle/refraction.toml
    - tools/nord2000_oracle/gen_refraction_fixtures.py
decisions:
  - "Eq. 26 fL middle branch transcribed as (43 − 3·Δc₁₀)/40 with knots at Δc₁₀ = 1 and 5 — confirmed by C⁰-continuity (factor = 1 at 1, 0.7 at 5); pdftotext dropped the minus sign, continuity resolved it"
  - "calc_eq_ssp_ground takes σ (sigma_kpa), not a precomputed Ẑ_G — phase_diff_freq owns the impedance and needs σ to sweep the 1/3-oct bracket (interface deviation from the plan's Ẑ_G argument, documented)"
  - "Route-2 A/B scaling constants marked [ASSUMED]; the B temperature-gradient factor C/(2·(T₀+273.15)) is the exact Coft derivative (not assumed); isotropic-temperature log part of A taken as 0 [ASSUMED]"
  - "geometry.rs NOT modified — the cos wind projection is a harness one-liner, no shared geometry primitive warranted (plan said 'only if warranted')"
  - "Nord2000 turbulence default added as an accessor seam (turbulence_or_nord2000_default), NOT wired into build_terrain_inputs — the frozen zero-turbulence terrain oracle fixtures stay untouched"
  - "eqssp_ground oracle tolerance 1e-6 relative (Rust↔Python transcendental last-ULP drift over the PhaseDiffFreq bracket); still catches any Eq. 23/26/27 mistranscription by >0.01%"
metrics:
  tasks: 3
  commits: 3
  files_created: 2
  files_modified: 6
  engine_tests: 149
  harness_tests: 49
  duration_min: 34
---

# Phase 3 Plan 2: Frequency-Dependent Soft Ground, Weather Routes & Reflection Split Summary

Frequency-dependent soft-ground refraction (CalcEqSSPGround), the per-azimuth weather-coefficient derivation (WeatherProfile + Route 2), and the reflection-path before/after profile split — each a strict generalization that collapses to the 03-01 single-profile result in its homogeneous/hard limit.

## What was built

**Task 1 — CalcEqSSPGround (Eqs. 22–28).** `calc_eq_ssp_ground(f, d, hS, hR, σ, z₀, A, B, C)` in `eqssp.rs`. Over soft ground (`σ < 10⁷ Pa·s·m⁻²`) the sound-speed gradient is log-interpolated between `fL` (Eq. 26) and `fH` (Eq. 27) via the existing `phase_diff_freq` aux at phases `Ψ = π` and `2Ψ = 2π` (with the Eq. 24/25 clamps `d ≤ 400 m`, `hS,hR ≥ 0.5 m`); above `fH` it is the full frequency-independent gradient, below `fL` it is zero. Over hard ground it delegates to `calc_eq_ssp` (Pitfall 4 — no fL/fH machinery, ξ(f) flat). Evaluated natively at each 1/12-octave point and **compared by band index** (D-14). `calc_eq_ssp` was refactored into shared `eq_ssp_terms` / `collapse_gradient` helpers so the ground variant reuses `c̄`/heights and overrides only the gradient — keeping the two entry points bit-identical in the homogeneous limit.

**Task 2 — `envi-harness::weather` (MET-02).** `WeatherProfile { a, b, c, s_a, s_b, z0 }` (z₀ clamped ≥ 0.001 m on construction) and a bearing-independent `WeatherComponents` decomposition; `profile_for_bearing` computes the isotropic temperature part of A once and adds the wind part `A_wind·cos(bearing − φ_u)` per bearing (the `geometry::azimuth_deg` clockwise-from-north convention). `route2` maps FORCE surface met to `(A,B,C)`: wind log coefficient `u/ln(zu/z₀+1)`, `B = (C/(2·(T₀+273.15)))·dt/dz` (inversion ⇒ B>0), `C = Coft(t₀)`. Malformed/non-finite met returns a typed `CaseLoadError`, never a panic. A `turbulence_or_nord2000_default` accessor adds the nonzero Nord2000 Cv²/CT² default (D-11) without perturbing the frozen terrain fixtures.

**Task 3 — Reflection-path split (ENG-06).** `ReflectionProfiles { before, after }` derives the two sub-path profiles from their sub-path bearings (A projected per leg, B/C/z₀ shared); `sub_path_collapses` runs `calc_eq_ssp` per leg returning `(ξ₁,c₀₁),(ξ₂,c₀₂)`; `route2_reflection` builds it from FORCE met at two bearings. Degenerate geometry returns a typed error. In the homogeneous limit both legs collapse to `(0, C)` and the reflection point is cross-checked against `reflect_over_segment`.

## Verification

- `cargo test` workspace green (engine lib 149, harness lib 49, all integration/oracle suites pass; the 67 capability-gated FORCE cases stay `Skipped`, never a false `Pass`).
- `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt --check` clean.
- `cargo tree -p envi-engine` unchanged — direct deps still `ndarray + num-complex + thiserror` only (no new dep, no linalg crate).
- `.conj()` grep gate over `propagation/` = 0 real calls (5 hits are doc-comment mentions).
- CalcEqSSPGround oracle passes **by band index**; hard-ground ξ(f) flat; soft-ground ξ(f) monotone 0→full across bands; homogeneous → (0, C) at every band.
- Direction property tests: downwind A > upwind A; inversion → B>0, lapse → B<0; A₁ ≠ A₂ across sub-path bearings, B₁ = B₂.

## Deviations from Plan

### Auto-fixed / interface adjustments (Rule 2/3 — documented)

**1. [Rule 3 - Interface] `calc_eq_ssp_ground` takes σ (`sigma_kpa`), not a precomputed `Ẑ_G`.**
- **Found during:** Task 1. The plan's artifact signature named a `z_g` (complex impedance) argument, but `phase_diff_freq` (which computes fL/fH) owns the Delany–Bazley `Ẑ_G(f)` internally and needs σ to sweep it across the 1/3-octave bracket, and the soft/hard branch is defined on σ.
- **Resolution:** the parameter is `sigma_kpa`; `Ẑ_G(f)` is derived inside exactly as Sub-model 2 does. Documented in the function's doc-comment.

**2. [Rule 1 - Transcription] Eq. 26 fL middle branch sign.**
- **Found during:** Task 1. `pdftotext` rendered the numerator as `43 3c10` (minus sign lost). Reading it as `(43 − 3·Δc₁₀)/40` with knots at Δc₁₀ = 1 and 5 is **C⁰-continuous** (factor = 1 at Δc₁₀ = 1, = 0.7 at Δc₁₀ = 5), which uniquely resolves the sign — the `(43 + 3·Δc₁₀)/40` reading is discontinuous at the upper knot. Encoded with a comment recording the continuity check.

**3. [Scope] `crates/envi-engine/src/geometry.rs` not modified.**
- The plan listed geometry.rs in `files_modified` but its action said add a helper "only if a shared projection primitive is warranted." The wind projection is a one-line `cos(bearing − φ_u)` and lives in the harness per D-15; no shared geometry primitive was warranted, so geometry.rs was left untouched.

No auth gates occurred. No architectural (Rule 4) changes.

## Known Stubs / [ASSUMED] constants

The Route-2 A/B **scaling constants are `[ASSUMED]`** (AV 1106/07 does not specify the wind/temperature → A/B conversion; the companion reference is absent from the repo — RESEARCH Weather-Routes banner, Pitfall 7). They are validated by **direction/structure property tests only** — there is **no false FORCE numeric pass**. What is physically certain and tested: downwind ⇒ larger A, inversion (`dt/dz>0`) ⇒ B>0, A₁≠A₂ / B₁=B₂. The `B` temperature-gradient factor `C/(2·(T₀+273.15))` is the exact `Coft` derivative (not assumed). The isotropic-temperature log part of A is taken as 0 `[ASSUMED]`. The Nord2000 default `Cv²/CT²` values are `[ASSUMED]` placeholders (validated by F_τ property tests in 03-03). These are **locked pending the 03-03 Open-Q1 checkpoint** — this is expected, planned scope, not an unresolved stub.

## Self-Check: PASSED

- Created files exist: `crates/envi-harness/src/weather/mod.rs`, `crates/envi-harness/src/weather/route2.rs` — FOUND.
- Modified files present with the new symbols (`calc_eq_ssp_ground`, `WeatherProfile`, `route2`, `ReflectionProfiles`) — FOUND.
- Commits FOUND: f4d2951 (Task 1), bd6363f (Task 2), 7e4c467 (Task 3).
