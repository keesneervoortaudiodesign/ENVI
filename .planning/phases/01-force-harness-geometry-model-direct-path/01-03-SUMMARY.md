---
phase: 01-force-harness-geometry-model-direct-path
plan: 03
subsystem: engine
tags: [rust, nord2000, direct-path, iso-9613-1, air-absorption, divergence, complex-transfer, num-complex, ndarray, src-01, walking-skeleton]

# Dependency graph
requires:
  - "01-01: envi-engine/envi-harness workspace, freq::{FreqAxis, FREQ_AXIS, N_BANDS}, CaseDefinition/PropagationParams, run_case dispatch, Outcome, compare::ComparisonReport"
  - "01-02: scene::{BandSpectrum, Source, SubSource, Receiver, Scene}, geometry::PathGeometry::direct, scene_build::build_scene, Capability::Geometry"
provides:
  - "envi-engine::propagation::air_absorption — Atmosphere{t_air_c,rh_percent,pressure_kpa}::new (typed domain validation), alpha_db_per_m (ISO 9613-1 Eq.286), band_attenuation_db (Nord2000 Eq.287, 300 dB clamp), molar_h2o_percent/f_r_oxygen/f_r_nitrogen intermediates"
  - "envi-engine::propagation — sound_speed_ms (Eq.335), direct_path(&PathGeometry,&Atmosphere,&FreqAxis)->Result<TransferSpectrum>, PropagationError{InvalidTemperature,InvalidHumidity,InvalidPressure,DegenerateRange}"
  - "envi-engine::propagation::divergence — divergence_db (Eq.330), divergence_amplitude (crate-internal 1/sqrt(4piR^2))"
  - "envi-engine::transfer — TransferSpectrum=Vec<Complex<f64>>, TransferTensor=Array3<Complex<f64>> [sub_source,receiver,freq] row-major, band_levels_db (L_p = L_W + 20log10|H|, the MAC seed)"
  - "envi-harness — Capability::FreeField implemented; run_case FreeField arm; compare::analytic_freefield_reference (independent dB-domain oracle) + compare_pointwise (105-point); cases::SourceSpectrum{Unit,Uniform,Ramp} + ramp TOML schema"
affects: [phase-02-ground-diffraction, phase-03-refraction, phase-04-tensor-emission]

# Tech tracking
tech-stack:
  added: []
  patterns: [complex-transfer convention frozen (e^{+jωt}, phase −ωτ with τ=R/c the carried primitive, |H|=1/sqrt(4πR²)), three-stage transcription pinning (intermediates→published table→regression anchors), independent dB-domain test oracle (separate code path from the complex roundtrip), sole alpha·R→band converter (band_attenuation_db), row-major frequency-contiguous tensor alias, typed domain errors never panic on data]

key-files:
  created:
    - crates/envi-engine/src/propagation/mod.rs
    - crates/envi-engine/src/propagation/air_absorption.rs
    - crates/envi-engine/src/propagation/divergence.rs
    - crates/envi-engine/src/transfer.rs
    - cases/freefield_spectrum.toml
  modified:
    - crates/envi-engine/src/lib.rs
    - crates/envi-harness/src/lib.rs
    - crates/envi-harness/src/compare.rs
    - crates/envi-harness/src/capability.rs
    - crates/envi-harness/src/scene_build.rs
    - crates/envi-harness/src/cases/mod.rs
    - crates/envi-harness/src/cases/toml.rs
    - crates/envi-harness/src/cases/xls.rs
    - cases/freefield_100m.toml

key-decisions:
  - "Complex convention frozen (transfer.rs docs): time e^{+jωt}; outgoing phase e^{−jωτ} with τ=R/c the CARRIED primitive (not kR) so Phase 3's cancellation-safe Δτ slots in; |H| normalized so L_p = L_W + 20·log10|H| ⇒ free-field |H| = 1/√(4πR²); air absorption multiplies as the real factor 10^(−ΔLₐ/20); Phase 2+ effects multiply H by their complex pressure ratio"
  - "TransferTensor = Array3<Complex<f64>> shape [sub_source, receiver, freq] in default row-major order — frequency axis contiguous (PROJECT.md); never Fortran-order. Phase 4 fills it in"
  - "band_attenuation_db is the SOLE alpha·R→band-level converter (Pitfall 4); direct_path and the harness oracle both route through it; applied at all 105 grid points as band centres (Assumption A5, revisit Phase 4)"
  - "Free-field gate = strict analytic identity 1e-9 dB (01-RESEARCH Open Question 2), deliberately harsher than the FORCE 1 dB; comparison in the 105-point 1/12-octave space (27-band pick is for FORCE references)"
  - "Test oracle independence (T-01-09): analytic_freefield_reference computes L_p purely in the dB domain, reusing engine α/band-correction (independently anchored) but NOT the complex polar roundtrip — so the e2e comparison catches wiring/normalization/phase errors, not just formula errors"

requirements-completed: [ENG-01, ENG-04, SRC-01]

# Metrics
duration: 35min
completed: 2026-07-07
status: complete
---

# Phase 1 Plan 03: Direct Path at 1/12-Octave Complex Resolution Summary

**The walking skeleton is complete: geometrical divergence (Eq. 330), ISO 9613-1 air absorption with the Nord2000 Eq. 287 band correction, and a point sub-source's per-1/12-octave spectrum, assembled into a genuinely complex transfer value per 1/12-octave point — the project's load-bearing output contract — and validated end-to-end through the harness against an independent dB-domain reference at a strict 1e-9 dB analytic identity.**

## Performance

- **Duration:** ~35 min
- **Completed:** 2026-07-07
- **Tasks:** 3 (Tasks 1–2 TDD RED→GREEN; Task 3 integration wiring with unit + e2e tests)

## Accomplishments

- **ISO 9613-1 air absorption transcribed and pinned in three stages (ENG-04).** `alpha_db_per_m` is AV 1106/07 Eq. (286) verbatim; its intermediates (`molar_h2o_percent`, `f_r_oxygen`, `f_r_nitrogen`) are exposed and anchored at the FORCE conditions (h = 1.177222 %, f_rO = 36332.37 Hz, f_rN = 333.6691 Hz), the full α cross-checks the published ISO 9613-2 Table 2 values (5.0 / 22.9 / 76.6 dB/km at 1/4/8 kHz within 2 %), and six full-precision regression anchors at the exact grid centres hold to 1e-6 relative. `band_attenuation_db` is Eq. (287) with the 300 dB clamp — the sole α·R→band converter — verified at A₀ ∈ {10, 20, 100} (9.89 / 19.38918 / 81.9 dB) and monotonic across 0..300.
- **Geometrical divergence + the load-bearing complex convention (ENG-01).** `divergence_db` reproduces all four Eq. 330 anchors (−10.992099 / −30.992099 / −50.992099 / −70.992099 dB) and guards R ≤ 0 as a domain error, not a clamp. `direct_path` assembles `|H|·e^(−j·2π·f·τ)` with `τ = R/c` the carried primitive; the half-wavelength cancellation test (|sum| < 1e-10) proves the phase is live and exercised, the magnitude identity matches divergence − band-corrected absorption to 1e-9 dB, and the phase identity is `−2πfτ` at every sampled band.
- **The frozen transfer contract (transfer.rs).** `TransferSpectrum = Vec<Complex<f64>>` (len 105) and `TransferTensor = Array3<Complex<f64>>` shape `[sub_source, receiver, freq]` in row-major (frequency-contiguous) order — the Phase 4 forward contract. `band_levels_db` seeds the Phase 4 MAC: `L_p = L_W + 20·log10|H|`.
- **Free-field capability green end-to-end (SRC-01).** `Capability::FreeField` is implemented; the `run_case` FreeField arm drives Scene → `direct_path` (105 complex values) → `band_levels_db` and compares against `compare::analytic_freefield_reference` — an independent dB-domain oracle — at 1e-9 dB. Both `freefield_100m` (unit spectrum, pure transfer) and the new `freefield_spectrum` (non-uniform L_W ramp 80 + 0.1·i) report **Pass**, proving a point sub-source's per-1/12-octave spectrum rides through the complex transfer.
- **All quality gates pass:** `cargo build --workspace`, `cargo test --workspace` (all green; 65 FORCE road cases still meaningfully ignored with requires-lists), `cargo clippy --all-targets -- -D warnings` (zero warnings), `cargo fmt --check` (clean), `#![deny(unsafe_code)]` on envi-engine, and the I/O quarantine (engine deps still only ndarray/num-complex/thiserror).

## Task Commits

1. **Task 1: ISO 9613-1 air absorption + Eq. 287 band correction** — RED `0e5093d` (test) → GREEN `310c249` (feat)
2. **Task 2: divergence + complex TransferSpectrum with live phase** — RED `b663a64` (test) → GREEN `2a24a47` (feat)
3. **Task 3: free-field capability through the harness** — `6b6f59c` (feat; unit + e2e tests included)

## Complex convention as implemented (per plan `<output>`)

- **Time convention:** `e^{+jωt}`. An outgoing wave carries phase `e^{−jωτ}`.
- **Phase primitive:** `τ = R / c` with `c = sound_speed_ms(t) = 20.05·√(t+273.15)` (Eq. 335). The engine computes `2π·f·τ` and stores `τ` conceptually as the carried quantity — Phase 3's cancellation-safe `Δτ` reformulation replaces travel-time differences here; `kR` is never stored.
- **Normalization:** `|H| = 10^(−ΔLₐ/20) / √(4πR²)`, so `20·log10|H| = ΔL_d − ΔLₐ` and receiver band level falls out of `L_W` by addition: `L_p(f) = L_W(f) + 20·log10|H(f)|`.
- **Composition (Phase 2+):** effects multiply `H` by their complex pressure ratio relative to free field; `band_levels_db` computes `p = Σ_s H[s,r,f]·G_s(f)`, `|G_s| = 10^(L_W/20)`, `L = 20·log10|p|`.

## Public API (for Phases 2–4)

**`envi_engine::propagation::air_absorption`**
- `struct Atmosphere { t_air_c, rh_percent, pressure_kpa }`; `Atmosphere::new(...) -> Result<Self, PropagationError>` (typed domain validation; rejects non-finite)
- `fn alpha_db_per_m(f_hz, &Atmosphere) -> f64`, `fn band_attenuation_db(a0_db) -> f64`
- `fn molar_h2o_percent(&Atmosphere) -> f64`, `fn f_r_oxygen(&Atmosphere) -> f64`, `fn f_r_nitrogen(&Atmosphere) -> f64`

**`envi_engine::propagation`**
- `fn sound_speed_ms(t_air_c) -> f64` (Eq. 335)
- `fn direct_path(&PathGeometry, &Atmosphere, &FreqAxis) -> Result<TransferSpectrum, PropagationError>`
- `enum PropagationError { InvalidTemperature, InvalidHumidity, InvalidPressure, DegenerateRange }`

**`envi_engine::propagation::divergence`**
- `fn divergence_db(r_m) -> Result<f64, PropagationError>` (Eq. 330); `pub(crate) fn divergence_amplitude(r_m) -> Result<f64, PropagationError>`

**`envi_engine::transfer`**
- `type TransferSpectrum = Vec<Complex<f64>>` (len `N_BANDS`)
- `type TransferTensor = Array3<Complex<f64>>` — `[sub_source, receiver, freq]`, row-major
- `fn band_levels_db(&TransferSpectrum, &BandSpectrum) -> Vec<f64>` — **Phase 2 multiplies ground/diffraction complex ratios into a `TransferSpectrum`; then `band_levels_db` yields receiver levels.**

**`envi_harness`**
- `compare::analytic_freefield_reference(r_m, &Atmosphere, &BandSpectrum, &FreqAxis) -> Vec<f64>` (independent dB-domain oracle)
- `compare::compare_pointwise(got, want, tol_db, centres) -> ComparisonReport` (105-point)
- `cases::SourceSpectrum { Unit, Uniform(f64), Ramp { base_db, slope_db_per_band } }` (SRC-01); TOML `[source.spectrum]` accepts `kind = "unit" | "uniform" | "ramp"`
- `capability::implemented_capabilities()` now `{ Geometry, FreeField }`

## Numeric anchors — computed vs. reference

All anchors passed at their stated tolerances (no deviation beyond tolerance). Notably:

- Divergence: −50.992099 dB at R=100 m (1e-6), all four rows pass.
- ISO 9613-1 intermediates & regression anchors: 1e-4 / 1e-6 relative, pass.
- Published ISO 9613-2 cross-check: within 2 % table rounding.
- Free-field e2e (both cases): engine band levels match the dB-domain oracle within 1e-9 dB at all 105 points.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Sound-speed test anchor contradicted the mandated Eq. 335 formula**
- **Found during:** Task 2 GREEN (`cargo test -p envi-engine`).
- **Issue:** The plan's Test 2 asserts `sound_speed_ms(15.0) == 340.29 ± 0.01`, but Eq. 335 verbatim — `20.05·√(15 + 273.15)` — yields **340.348 m/s**. The "340.29" parenthetical (from 01-RESEARCH) corresponds to the sharper textbook coefficient ≈20.047, not Nord2000's rounded 20.05.
- **Fix:** The formula is the frozen contract every phase-τ depends on, so the formula wins: corrected the test anchor to 340.348 (the value Eq. 335 actually produces) with a documenting comment. The implementation uses `20.05` exactly as mandated.
- **Files modified:** crates/envi-engine/src/propagation/mod.rs (test)
- **Commit:** `2a24a47`

**2. [Rule 2 - Missing critical functionality] Non-finite atmosphere inputs slipped through `Atmosphere::new`**
- **Found during:** Task 1 GREEN (domain-validation test).
- **Issue:** A bare `pressure_kpa > 0.0` (and `t_air_c > −273.15`) admits `+∞`, which would poison every downstream α as NaN/∞ — a numeric-DoS path (threat T-01-08).
- **Fix:** Added explicit `is_finite()` guards on all three domains before the range checks, so NaN and ±∞ are rejected as typed errors.
- **Files modified:** crates/envi-engine/src/propagation/air_absorption.rs
- **Commit:** `310c249`

## Known Stubs

None that block the plan goal. The `TransferTensor` alias is defined but not yet populated — Phase 4 fills it in (documented as the forward contract). `Atmosphere` deliberately omits turbulence/wind fields (Phase 3). The FORCE cases still carry the placeholder single sub-source (`SourceSpectrum::Unit`) from 01-02 — the real Nord2000 road emission model is Phase 4.

## Threat surface

Mitigations from the plan's `<threat_model>` are implemented as correctness requirements: T-01-08 (typed domain errors on `Atmosphere::new`/`divergence_db`/`direct_path`; Eq. 287 clamped at 300 dB; non-finite rejected — no panic paths on data), T-01-09 (`analytic_freefield_reference` is an independent dB-domain oracle; engine formulas pinned by three-stage anchors; no float-equality assertions), T-01-10 (per-case `tolerance_db` in the committed TOML with rationale; comparator reports max deviation). T-01-SC: no new dependencies (engine deps still only ndarray/num-complex/thiserror). No new trust boundaries or network/auth/file surface introduced.

## Verification Evidence

- `cargo build --workspace` — finished, no errors
- `cargo test --workspace` — 31 (engine) + 31 (harness unit) + force target 5 passed / 65 ignored (2 free-field + 2 geometry + discovery); 0 failed
- `cargo test -p envi-harness --test force freefield` — freefield_100m + freefield_spectrum: 2 passed, 0 failed
- `cargo clippy --all-targets -- -D warnings` — zero warnings
- `cargo fmt --check` — clean (exit 0)
- `cargo tree -p envi-engine -e normal --depth 1` — only ndarray, num-complex, thiserror (I/O quarantine holds)
- `cargo run -p envi-harness -- report | grep -c Pass` — 4 (2 free-field + 2 geometry)
- `grep -c 9613 .../air_absorption.rs` — 10 (source citation present); `grep -c "sub_source, receiver, freq" .../transfer.rs` — 2 (tensor layout doc present)

## Self-Check: PASSED

All created/modified files exist on disk; the five task commits (0e5093d, 310c249, b663a64, 2a24a47, 6b6f59c) are present in git history.
