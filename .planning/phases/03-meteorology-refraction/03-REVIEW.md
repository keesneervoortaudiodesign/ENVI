---
phase: 03-meteorology-refraction
reviewed: 2026-07-08T00:00:00Z
depth: standard
files_reviewed: 23
files_reviewed_list:
  - crates/envi-engine/src/propagation/coherence.rs
  - crates/envi-engine/src/propagation/mod.rs
  - crates/envi-engine/src/propagation/rays.rs
  - crates/envi-engine/src/propagation/refraction/circular_ray.rs
  - crates/envi-engine/src/propagation/refraction/eqssp.rs
  - crates/envi-engine/src/propagation/refraction/mod.rs
  - crates/envi-engine/src/propagation/refraction/profile.rs
  - crates/envi-engine/src/propagation/refraction/shadow_zone.rs
  - crates/envi-engine/src/propagation/terrain_effect/mod.rs
  - crates/envi-engine/src/propagation/terrain_effect/submodel1.rs
  - crates/envi-engine/src/propagation/terrain_effect/submodel2.rs
  - crates/envi-harness/src/capability.rs
  - crates/envi-harness/src/cases/mod.rs
  - crates/envi-harness/src/cases/toml.rs
  - crates/envi-harness/src/lib.rs
  - crates/envi-harness/src/main.rs
  - crates/envi-harness/src/weather/mod.rs
  - crates/envi-harness/src/weather/route1.rs
  - crates/envi-harness/src/weather/route2.rs
  - crates/envi-harness/src/weather/route3.rs
  - crates/envi-harness/tests/oracle_refraction.rs
  - crates/envi-harness/tests/terrain.rs
  - tools/nord2000_oracle/gen_refraction_fixtures.py
findings:
  critical: 1
  warning: 3
  info: 4
  total: 8
status: resolved
fixed_at: 2026-07-08
fixed: 8
accepted_risk: 0
skipped: 0
---

# Phase 3: Code Review Report

**Reviewed:** 2026-07-08
**Depth:** standard
**Files Reviewed:** 23
**Status:** resolved (all 8 findings fixed — 2026-07-08)

> **Resolution (2026-07-08):** All 8 findings fixed and committed atomically.
> `cargo test` (228 tests, workspace), `cargo clippy --all-targets -- -D warnings`,
> and `cargo fmt --check` all green. Binding invariants re-verified: zero
> `.conj()` in `propagation/` (grep gate), `cargo tree -p envi-engine` unchanged
> (ndarray + num-complex + thiserror only). No FORCE wind/gradient case flipped to
> a numeric Pass (they remain `Skipped(requires: emission-model)`).

## Summary

Adversarial review of the Phase-3 meteorology/refraction implementation (log-lin
profile, CalcEqSSP / CalcEqSSPGround collapse, circular-ray DirectRay/ReflectedRay/
Δτ, upward-refraction shadow-zone shielding, F_τ coherence, and the three weather
routes).

**The load-bearing domain invariants hold.** Verified directly:
- **Zero `.conj()` in `propagation/`** (grep confirmed — only doc-comment mentions).
- **F_τ multiplies, never overwrites, `H_coh`'s phase**: `terrain_effect::eval`
  does `f_delta_nu = coh.f_delta_nu * coherence_f_delta_nu(...)` and feeds it
  through `coherence_f`, which scales only the reflected complex term (Q̂ and
  `e^{+j2πfΔτ}` intact).
- **F→1 ⇒ P_incoh→0 bit-exact**, and **sA=sB=0 ⇒ Δτ⁺=Δτ ⇒ F_τ=1 bit-exact**
  (xi_plus==xi and c0_plus==c0 by construction, so `coherence_f_delta_nu(f,x,x)==1.0`).
- **`coherence_f_delta_nu` uses the `2π` argument** (`TAU * f * |Δτ⁺−Δτ|`), NOT
  `0.23π` (Pitfall 5 respected).
- **CalcEqSSPGround `d≤400 / hS,hR≥0.5` clamps** and the `|ξ|<1e-6` / `|ξ'|<1e-10`
  clamp separation are present and correct; the log-lin collapse and Δτ match the
  independent scipy/Python oracle to tolerance.
- **Untrusted input is guarded** with typed errors throughout (TOML/XLS loaders,
  CalcEqSSP, ray geometry), reject non-finite, path-traversal `confine`, DoS row
  caps. No panics on operator data, no hardcoded secrets, no injection surface.
- **The ASSUMED weather-route constants stay quarantined**: FORCE wind/gradient
  cases remain `Skipped(requires: emission-model)`; refraction cases dispatch to
  `Skipped` with no committed numeric reference. No false numeric Pass is created.

One reachable correctness bug (CR-01) exists in the shadow-zone onset zone, plus
three robustness/validation gaps and four documentation inconsistencies.

## Critical Issues

### CR-01: Shadow-zone onset zone (`0.95·dSZ < d < dSZ`) hard-errors on valid geometry

**File:** `crates/envi-engine/src/propagation/refraction/shadow_zone.rs:57-68`
(guard `d_sz >= d`), reached from
`crates/envi-engine/src/propagation/terrain_effect/mod.rs:338-344` and
`crates/envi-engine/src/propagation/rays.rs:273`.

**Issue:** `circular_rays` enters the "no reflected ray" shadow branch as soon as
`d > 0.95 * direct.d_sz` (rays.rs:273). `FlatChannel::eval` then calls
`shadow_zone_shielding(...)` whenever `rays.reflected.is_none()`. But
`shadow_zone_shielding` rejects `d_sz >= d` as non-physical (returns
`DegenerateShadowZone`). For any distance in the onset window
`0.95·dSZ < d < dSZ`, both conditions are simultaneously true: `reflected == None`
**and** `d_sz > d`, so `terrain_effect` returns `Err(DegenerateShadowZone)` on a
physically valid receiver placement (the shadow onset the 0.95 factor is meant to
model). The equivalent-wedge geometry itself is ill-defined there
(`d_far = d - d_sz < 0`, shadow_zone.rs:85).

This is reachable with an ordinary mild upward gradient: e.g. `ξ ≈ −7.5e-4`,
`hS=0.5, hR=1.5` gives `dSZ ≈ 100 m` (Eq. 43); at `d = 97 m` the branch fires and
the whole 105-band evaluation aborts with an error. The existing tests pass only
because their chosen geometries (`ξ=−1.5·…`, `d=97.5`) land fully past `dSZ`
(`d > dSZ`), so the onset window is never exercised.

**Fix:** Make the two thresholds consistent. Either enter the shadow branch only
at the geometric boundary (`d > direct.d_sz`) instead of `0.95·dSZ`, or have
`shadow_zone_shielding` handle the onset zone explicitly (e.g. clamp/ramp the
shielding to 0 as `d → dSZ⁺` and treat `d ≤ d_sz` as "no shielding yet, use the
non-shadow two-ray branch"). Concretely, gate the shielding call on the geometric
boundary and fall back to a coherent (reflected) evaluation in the onset window:

```rust
// rays.rs — only drop the reflected ray once past the geometric boundary
if xi < 0.0 && direct.d_sz.is_finite() && d > direct.d_sz {
    return Ok(RayPair { direct: direct_vars, reflected: None, dtau: 0.0 });
}
```

and/or in `shadow_zone_shielding`, return `Ok(0.0)` (no shielding) instead of an
error when `d_sz >= d`, since the receiver has not yet crossed the shadow edge.
Add a regression test at a geometry with `0.95·dSZ < d < dSZ`.

**Resolution:** FIXED (commit `1cc48d8`). All three thresholds reconciled on the
geometric shadow edge `dSZ`: `circular_rays` drops the reflected ray only once
`d > dSZ` (was `0.95·dSZ`), so the onset window keeps the coherent two-ray
branch; `travel_time_diff` zeroes Δτ only past `dSZ` (the Eq. 52 cap ramps Δτ→0
as `d → dSZ`); `shadow_zone_shielding` returns `Ok(0.0)` for `d ≤ dSZ` instead of
erroring, while genuinely non-physical input (non-finite / `dSZ ≤ 0`) stays a
typed error. Regression tests: `rays.rs::shadow_onset_window_retains_reflected_ray`
(the review's ξ≈−7.5e-4, hS=0.5, hR=1.5, d=97 geometry — asserts it sits in the
onset window and retains the reflected ray, and that a receiver just past `dSZ`
drops it) and `shadow_zone::pre_shadow_edge_returns_zero_shielding`.

## Warnings

### WR-01: Segmented terrain silently discards the refraction profile

**File:** `crates/envi-engine/src/propagation/terrain_effect/mod.rs:373-382`

**Issue:** `FlatChannel::eval` only applies refraction on the `single_type` path.
When the terrain has more than one surface type, it calls `submodel2(...)`, which
uses `straight_rays` internally and never consults `self.refraction`. A caller who
passes `refraction = Some(profile)` over segmented ground gets a **homogeneous**
result with no error and no warning — a silent scope reduction. This is
inconsistent with the codebase's own posture of hard-erroring on unimplemented
paths (`NonFlatTerrainNotImplemented`). Note `calc_eq_ssp_ground` (the
frequency-dependent segmented collapse) is fully implemented and oracle-tested but
is wired **nowhere** in the engine pipeline (grep: only referenced in
`tests/oracle_refraction.rs`), so the segmented-refraction path has a ready building
block it does not use.

**Fix:** Either wire `calc_eq_ssp_ground` into the segmented (`submodel2`) branch,
or return a typed error (e.g. a `SegmentedRefractionNotImplemented { f_hz }`
variant) when `refraction.is_some() && !single_type`, mirroring the
`NonFlatTerrainNotImplemented` contract, so the limitation can never be hit
silently.

**Resolution:** FIXED (commit `6252048`). Added
`PropagationError::SegmentedRefractionNotImplemented { n_types, f_hz }` and return
it from `FlatChannel::eval` whenever `refraction.is_some()` on the segmented
(Sub-model 2) branch — the silent homogeneous discard is now a hard error
mirroring `NonFlatTerrainNotImplemented`. Chose the honest-error option (not
wiring `calc_eq_ssp_ground`) deliberately, per the task constraint not to
fabricate a wiring that could flip a FORCE case to a false numeric Pass;
`calc_eq_ssp_ground` remains the documented building block for a later plan.
Regression test: `terrain_effect_tests::segmented_refraction_is_a_typed_error_not_silent`
(segmented + refraction errors; the same profile with `None` still evaluates).

### WR-02: `route2` does not validate `zu > 0` (route3 does), yielding a silent wrong-sign wind coefficient

**File:** `crates/envi-harness/src/weather/route2.rs:68,95-100`

**Issue:** `zu` (anemometer height) is only checked for finiteness via `finite_or`.
A non-physical `zu ≤ 0` from case data is not rejected. `log_zu =
(zu / z0 + 1.0).ln()`:
- `zu` slightly negative (e.g. `zu = −0.0005`, `z0 = 0.001`) → `zu/z0+1 = 0.5` →
  `log_zu = −0.693` (finite, negative) → `a_wind = u / log_zu` is finite but
  **negative**, silently flipping the wind's refraction sign.
- `zu` strongly negative → `ln` of a non-positive number = `NaN`, but the guard
  `log_zu.abs() > 1e-12` is `false` for `NaN`, so `a_wind` silently becomes `0.0`.

Either way the routine returns a plausible-looking profile from garbage input.
`route3::reconstruct_profiles` (route3.rs:116) correctly rejects `zu <= 0` with a
typed error — the two routes are inconsistent.

**Fix:** In `route2_components`, after `finite_or(params.zu_m, …)`, add
`if !(zu > 0.0) { return Err(CaseLoadError::NonFinite { context: "weather route 2".into(), what: "zu must be positive".into() }); }`
(matching route3).

**Resolution:** FIXED (commits `8875db8`, `107b2dd`). Added the typed
`CaseLoadError::NonFinite` guard rejecting `zu ≤ 0` in `route2_components`,
matching route3. Written as `zu <= 0.0` (zu is already validated finite) rather
than `!(zu > 0.0)` to satisfy `clippy::neg_cmp_op_on_partial_ord`. Regression
test: `route2::weather_route2_rejects_non_positive_zu` (0, −0.0005, −10).

### WR-03: Eq. 52 shadow-edge cap computes `R₂ − R₁` naively (catastrophic-cancellation house-rule violation)

**File:** `crates/envi-engine/src/propagation/refraction/circular_ray.rs:296-299`

**Issue:** `travel_time_diff`'s Δτ₀ cap forms
`√(d²+(hS+hR)²) − √(d²+(hS−hR)²)` by direct subtraction of two near-equal
lengths. CLAUDE.md's numerics house rule calls this out explicitly ("guard
catastrophic cancellation — the Δτ travel-time difference especially… these are
correctness-critical, not style"), and `rays.rs` already uses the cancellation-free
identity `ΔR = 4·hS·hR/(R₁+R₂)` everywhere else. At long range with low heights
(e.g. `d≥1000 m`, `hS≈0.01 m`) this naive difference loses ~4–8 significant
figures. It passes the current oracle grid only because that grid tops out at
`d=150 m` (and the Python oracle makes the *same* naive subtraction, so the
cross-check cannot catch it).

**Fix:** Reuse the identity:

```rust
let dr = 4.0 * geom.h_s * geom.h_r
    / ((d * d + (geom.h_s + geom.h_r).powi(2)).sqrt()
        + (d * d + (geom.h_s - geom.h_r).powi(2)).sqrt());
let dtau0 = (1.0 - (d / geom.d_sz).powi(2)) * dr / geom.c0;
```

**Resolution:** FIXED (commit `b39ab84`). `travel_time_diff` now forms the Eq. 52
cap via the cancellation-free identity `ΔR = 4·hS·hR/(R₁+R₂)` (as `rays.rs`
does), eliminating the naive `R₂ − R₁` subtraction. Existing
`upward_refraction_shadow_edge_zeroes_dtau` and the Δτ cancellation regression
continue to pass.

## Info

### IN-01: Stale rationale — `required_capabilities` says Refraction is gated as "unimplemented"

**File:** `crates/envi-harness/src/capability.rs:73-80`

**Issue:** The `CaseKind::Refraction` arm comment says "Gate on Refraction
(unimplemented at the case level) so these stay `Skipped`." But
`implemented_capabilities()` now includes `Capability::Refraction`
(capability.rs:124), so the capability gate does **not** fire for refraction
cases — they stay `Skipped` only because `run_case` dispatch returns
`Skipped("no committed numeric reference")` (lib.rs:71-75). The outcome is correct;
the stated mechanism is now wrong and misleading for a maintainer.

**Fix:** Update the comment to say the skip comes from dispatch (no committed
numeric reference), not from the capability gate.

**Resolution:** FIXED (commit `5e26bfe`). The `CaseKind::Refraction` comment now
states the skip comes from `run_case` dispatch ("no committed numeric
reference"), and that the required set is declared for traceability, not to force
the skip.

### IN-02: `Period` doc boundaries contradict the (correct) L_den hour weights

**File:** `crates/envi-harness/src/weather/route1.rs:64-90`

**Issue:** Doc comments state "Evening, 19:00–22:00 (4 h)" (that span is 3 h) and
"Night, 22:00–07:00 (8 h)" (that span is 9 h). The `weighting()` values (12/4/8 h,
0/+5/+10 dB) are the correct EU END numbers; only the time-boundary strings are
wrong (END evening is 19–23, night 23–07). Harmless to computation but could lead a
maintainer to "correct" the hours and break END compliance.

**Fix:** Change the boundary strings to 19:00–23:00 (evening) and 23:00–07:00
(night), or drop the explicit clock times.

**Resolution:** FIXED (commit `404a43e`). Evening is now documented 19:00–23:00
and night 23:00–07:00 (with an explicit EU END / Directive 2002/49/EC reference);
the 12/4/8 h weighting values were already correct and are unchanged.

### IN-03: Wind/temperature std-dev met fields are not validated non-negative

**File:** `crates/envi-harness/src/weather/route2.rs:70-71,109-114`

**Issue:** `su` and `sdtdz` are only checked for finiteness. A negative std-dev
(non-physical) flows through to `s_a`/`s_b` and hence the upper-refraction profile
`A⁺ = A + 1.7·sA`, producing an `A⁺ < A`. Not currently reachable through the case
pipeline (weather routes are not wired into `run_case`), but it is an untrusted-input
validation gap.

**Fix:** Reject `su < 0` / `sdtdz < 0` with a typed `CaseLoadError`, or document
that the sign is intentionally unconstrained.

**Resolution:** FIXED (commit `2d10ab5`). `route2_components` now rejects `su < 0`
and `sdtdz < 0` with a typed `CaseLoadError::NonFinite`. Regression test:
`route2::weather_route2_rejects_negative_std_devs`.

### IN-04: Inconsistent unit labels for the temperature turbulence parameter `CT²`

**Files:** `crates/envi-engine/src/propagation/coherence.rs:46`;
`crates/envi-harness/src/cases/mod.rs:224,241-242`

**Issue:** `CT²` is documented as `K²·m^{−2/3}` in `CoherenceInputs` (the physically
correct structure-parameter unit), as `K/s²` in `PropagationParams.ct2`, and as
`K²·s⁻²` on `NORD2000_DEFAULT_CT2`. Three different unit strings for the same
quantity. Values are ASSUMED so this is documentation-only, but the inconsistency
obscures whether callers are feeding the right quantity.

**Fix:** Standardize on `K²·m^{−2/3}` (and `Cv²` on `m^{4/3}·s⁻²`) across all three
doc comments.

**Resolution:** FIXED (commit `68df87a`). `PropagationParams.ct2` and
`NORD2000_DEFAULT_CT2` now read `K²·m^{−2/3}` (matching `CoherenceInputs`); the
`Cv²` labels were also normalized to `m^{4/3}·s⁻²`. Documentation only — the
(ASSUMED) values are unchanged.

---

_Reviewed: 2026-07-08_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
