---
phase: 02-ground-effect-diffraction
reviewed: 2026-07-08T00:00:00Z
depth: standard
files_reviewed: 27
files_reviewed_list:
  - crates/envi-engine/src/propagation/special.rs
  - crates/envi-engine/src/propagation/ground.rs
  - crates/envi-engine/src/propagation/diffraction.rs
  - crates/envi-engine/src/propagation/fresnel.rs
  - crates/envi-engine/src/propagation/rays.rs
  - crates/envi-engine/src/propagation/coherence.rs
  - crates/envi-engine/src/propagation/mod.rs
  - crates/envi-engine/src/propagation/terrain_effect/mod.rs
  - crates/envi-engine/src/propagation/terrain_effect/submodel1.rs
  - crates/envi-engine/src/propagation/terrain_effect/submodel2.rs
  - crates/envi-engine/src/propagation/terrain_effect/submodel7.rs
  - crates/envi-engine/src/propagation/terrain_effect/screen.rs
  - crates/envi-engine/src/propagation/terrain_effect/screen/tests.rs
  - crates/envi-engine/src/propagation/terrain_interpretation.rs
  - crates/envi-engine/src/transfer.rs
  - crates/envi-engine/src/geometry.rs
  - crates/envi-engine/src/scene.rs
  - crates/envi-harness/src/capability.rs
  - crates/envi-harness/src/cases/mod.rs
  - crates/envi-harness/src/cases/toml.rs
  - crates/envi-harness/src/scene_build.rs
  - crates/envi-harness/src/lib.rs
  - crates/envi-harness/src/main.rs
  - crates/envi-harness/tests/oracle_flat.rs
  - crates/envi-harness/tests/oracle_ground.rs
  - crates/envi-harness/tests/oracle_screen.rs
  - crates/envi-harness/tests/oracle_wedge.rs
  - crates/envi-harness/tests/terrain.rs
  - tools/nord2000_oracle/common.py
  - tools/nord2000_oracle/flat_models.py
  - tools/nord2000_oracle/gen_ground_fixtures.py
  - tools/nord2000_oracle/gen_flat_fixtures.py
  - tools/nord2000_oracle/gen_wedge_fixtures.py
  - tools/nord2000_oracle/gen_screen_fixtures.py
  - tools/nord2000_oracle/gen_case_fixtures.py
findings:
  critical: 1
  warning: 5
  info: 3
  total: 9
status: issues_found
---

# Phase 02: Code Review Report

**Reviewed:** 2026-07-08T00:00:00Z
**Depth:** standard
**Files Reviewed:** 35
**Status:** issues_found

## Summary

Reviewed the Nord2000 ground-effect + diffraction implementation (Sub-models 1/2/4/5/6/7,
the wedge/Fresnel/Faddeeva numerics core, the §5.21 terrain interpretation dispatcher, the
phase-coherence and convention-quarantine contracts, and the Rust↔Python oracle harness).

The phase-coherence contract (complex `h_coh_factor` / real `p_incoh`, single `.conj()`
boundary in `transfer.rs`) is implemented consistently everywhere it was checked, and the
convention quarantine holds — no stray `.conj()` in `propagation/`. The single-surface and
single-screen numerics (Sub-models 1, 2, 4, and the wedge/Fresnel/Faddeeva core) are backed
by real cross-implementation oracles (scipy `wofz`) and match to the documented tolerances.

The most significant finding is in the **Sub-model 6 (two-screen) eight-ray engine**: the
"middle region" reflection is built from the same endpoint pair as the "after region"
reflection (receiver ↔ second edge), which has no physically coherent interpretation as a
reflection occurring *between* the two screens, and is not exercised by any committed test
with a middle-region ground type that differs from the after-region type — so the bug (if
it is one) is invisible to the current test suite, including the independently-written
Python oracle, which was written to mirror the exact same convention. Two further Warnings
cover a related double-screen simplification (hardcoded source-side σ for the whole middle
strip) and a broader note on oracle independence: several Python oracles reimplement the
*same* equation transcriptions and the *same* documented simplifications as the engine,
so they cannot catch a shared misreading of the source document — worth flagging per the
review's oracle-trust mandate even though the code comments already partially disclose it.

A latent numerics-robustness gap was found in `faddeeva_w`'s lower-half-plane overflow
guard: it attempts to clamp non-finite output, but `f64::clamp` is a no-op on `NaN`, so a
`NaN` produced by the `Complex::exp` polar form (`inf * -0.0`) is not actually fixed. The
practical trigger (`Re(ẑ) == 0.0` bit-exact with `|Im(ẑ)| ≳ 26.6`) is not reachable from the
current `Q̂` call sites in normal operation, so this is a Warning, not a Critical, but it
does violate the function's own documented "never NaN" invariant.

## Critical Issues

### CR-01: Sub-model 6 "middle region" reflects the wrong endpoint pair — untested for heterogeneous double-screen ground

**File:** `crates/envi-engine/src/propagation/terrain_effect/screen.rs:743-820` (see also
`crates/envi-engine/src/propagation/terrain_effect/screen/tests.rs:483-519` and
`tools/nord2000_oracle/gen_case_fixtures.py:151-194`, `tools/nord2000_oracle/gen_case_fixtures.py:379-392`)

**Issue:** In `run_eight_path` (Sub-model 6, two screens), the three region reflections are
built as:

```rust
let before = cfg.before.first().map(|st| side_vars(f_hz, s, t1, st, cfg.coh, false)).transpose()?;
let after  = cfg.after.first().map(|st| side_vars(f_hz, r, t2, st, cfg.coh, true)).transpose()?;
let middle = cfg.middle.first().map(|st| side_vars(f_hz, r, t2, st, cfg.coh, true)).transpose()?;
```

`middle` uses the **identical** endpoint pair as `after` — `(receiver, T₂)` — differing only
in which `SurfaceStrip` (and therefore which σ) is passed in. There is no physical reading
under which a ground reflection occurring *between* the two screens (T₁↔T₂) is modeled by
reflecting the receiver off T₂: any real bounce in that span belongs to the S→T₁→T₂ leg, not
the T₂→R leg. Because `straight_rays_over_segment` reflects across the **infinite line**
through the segment (not the finite extent — the segment's finite extent only feeds the
unused `valid` flag), the "middle" ray currently differs from "after" *only* in the ground
impedance σ substituted in — i.e. it silently re-uses the after-region ray geometry with the
middle-region's acoustic properties, rather than modeling a distinct T₁–T₂ bounce.

This is not caught by any committed test: every Sub-model 6 test (`eight_ray_assembly_matches_hand_sum`,
`empty_middle_collapses_to_four_terms`, `force_thick_and_double_screens_finite_all_bands` in
`screen/tests.rs`, and the case-91 double-screen oracle fixture generated by
`gen_case_fixtures.py`) uses the **same σ** for the before/middle/after strips, so the bug
(if the endpoints are indeed wrong) is numerically invisible — `middle` and `after` compute
bit-identical `SideVars` in every test that exercises this path. The Python oracle
(`gen_case_fixtures.py::_eight_path`, `gen_screen_fixtures.py::_reflect_flat`) mirrors the
exact same `(r, t2, middle_seg)` convention, so it cannot catch this either (see WR-03) — a
genuinely independent double-screen reference with heterogeneous ground would be needed to
settle this. Given real double-screen FORCE-style scenes plausibly have different ground
types before/between/after two barriers, this can silently produce a wrong `ΔL_t` for a
production case that nothing in the test suite would flag.

**Fix:** Verify against AV 1106/07 Eq. 222's region definitions. If the middle-region bounce
is meant to modify the T₁→T₂ leg (as the "before"/"after" analogy to their own legs would
suggest), it should use the edges as endpoints, not the receiver:

```rust
let middle = cfg
    .middle
    .first()
    .map(|st| side_vars(f_hz, t1, t2, st, cfg.coh, /* side */ false_or_true))
    .transpose()?;
```

and the corresponding image point should be substituted into the *middle* leg of the ray
(`s→t1'→t2→r` or `s→t1→t2'→r`), not into the receiver-side leg as `rp`. At minimum, add a
test with `middle` strip σ different from `before`/`after` σ and pin the expected ΔL_t
against a genuinely independent reference (not the same-convention Python port), so any
future correction (or confirmation this is intentional) is verifiable.

## Warnings

### WR-01: `faddeeva_w`'s overflow guard is a no-op for NaN — violates the documented "never NaN" contract

**File:** `crates/envi-engine/src/propagation/special.rs:46-60`

**Issue:** The lower-half-plane branch guards against overflow:

```rust
let neg_z2 = -(z * z);
let mut two_exp = 2.0 * neg_z2.exp();
if !two_exp.re.is_finite() || !two_exp.im.is_finite() {
    two_exp = Complex::new(
        two_exp.re.clamp(-f64::MAX, f64::MAX),
        two_exp.im.clamp(-f64::MAX, f64::MAX),
    );
}
```

`f64::clamp` is specified to be a no-op when `self` is `NaN` (comparisons against `NaN` are
always false, so neither branch of the clamp fires and the original `NaN` passes through
unchanged). For `ẑ = 0 - iy` with `Re(ẑ) == 0.0` exactly and `|y| ≳ 26.6` (so
`exp(y²)` overflows to `+inf`), `Complex::exp`'s polar form computes
`Complex::new(inf * cos(-0.0), inf * sin(-0.0)) = Complex::new(inf, inf * -0.0)`, and
`inf * -0.0 = NaN` under IEEE-754. The `is_finite()` check correctly detects this (both
`inf` and `NaN` are non-finite), but the "fix" only clamps the real part; the imaginary part
stays `NaN` and propagates into the caller (`faddeeva_w(-z)`'s subtraction), silently
poisoning `Q̂` for any downstream call that produces a purely-imaginary `ẑ` in this magnitude
range.

This exact geometry (`Re(ẑ)==0.0` bit-exact) is not reached by the current `spherical_q`
call sites under normal FORCE-style inputs (reaching `Re(rho)==0` bit-exact requires an
exact transcendental coincidence), so this is not observed to corrupt Phase 2 target cases —
but it is a real, demonstrable defect in a function whose docstring and design intent
(guard against exactly this) says otherwise, and it is one unit test away from proving
wrong.

**Fix:** Handle `NaN` explicitly rather than relying on `clamp`:

```rust
if !two_exp.re.is_finite() || !two_exp.im.is_finite() {
    let fix = |v: f64| if v.is_nan() { 0.0 } else { v.clamp(-f64::MAX, f64::MAX) };
    two_exp = Complex::new(fix(two_exp.re), fix(two_exp.im));
}
```
(or derive the correct sign/magnitude analytically for the `im == 0` case rather than
defaulting to `0.0`). Add a regression test at `faddeeva_w(Complex::new(0.0, -27.0))` (or
similar) asserting a finite result.

### WR-02: Double-screen "middle" reflecting strip hardcodes the source-side σ, ignoring the actual ground between the two screens

**File:** `crates/envi-engine/src/propagation/terrain_interpretation.rs:547-571`

**Issue:** `region_strips` builds the middle-region reflecting strip for a double screen as:

```rust
let middle = vec![StripSpec {
    seg_a: [t1x, 0.0],
    seg_b: [midx.max(t1x) + 0.0, 0.0],
    sigma_kpa: sigma_src,       // = segments[0].flow_resistivity — the FIRST profile segment
    roughness_r: rough_src,
}];
```

`sigma_src`/`rough_src` are taken from `segments[0]` — the very first ground segment of the
*entire* profile (i.e. whatever is under the source) — regardless of what ground segment(s)
actually span the region between the two screens. For any double-screen profile where the
ground between the barriers differs from the ground at the source (a realistic case — e.g.
grass at the source, pavement between two barriers), the middle-region reflection coefficient
will silently use the wrong σ. This is compounded by CR-01 above (the same region's endpoints
may also be wrong), but is a distinct, independently fixable defect.

**Fix:** Derive the middle strip's σ/roughness from the actual profile segment(s) spanning
`[t1x, t2x]` (via the existing `strips_of` helper, analogous to the flat-ground `before`),
rather than reusing `sigma_src`.

### WR-03: Cross-implementation oracles are not fully independent of the engine's equation transcription or its documented simplifications

**File:** `tools/nord2000_oracle/flat_models.py`, `tools/nord2000_oracle/gen_wedge_fixtures.py`,
`tools/nord2000_oracle/gen_screen_fixtures.py`, `tools/nord2000_oracle/gen_case_fixtures.py`

**Issue:** The Python oracles are billed as "independent" cross-implementation references
(and they genuinely are independent for the Faddeeva function `w(z)`, via
`scipy.special.wofz`, which is the highest-value part of the cross-check). However, the
surrounding equations (Sub-model 1/2 Eq. 115-133, the wedge four-term sum Eq. 78-91, the
Sub-model 4 four-path combination Eq. 157-188, Table 6/7 transcription, and the Eq. 332
top-level blend) are **transcribed identically** — same Horner-form Fresnel-integral
coefficients (`F_COEFFS`/`G_COEFFS`, byte-for-byte the same numbers as
`special.rs`), same Eq. 187/188 weight-normalization logic, same Table 6/7 grids. The
docstring of `gen_screen_fixtures.py` says this explicitly: *"Both implementations use the
SAME Eq. 187-188 reading ... and the SAME Table 6/7 transcription."* In addition,
`gen_case_fixtures.py`'s thick/double-screen four-/eight-path composition
(`_four_path`/`_eight_path`) reuses the engine's own documented "single representative top"
simplification for Sub-model 5 (`screen_top` always returning `T₁`, see `screen.rs:599-605`)
and the same middle-region convention flagged in CR-01 — so a shared misreading of any of
these equations, or the accuracy impact of the documented simplifications, is invisible to
every oracle-backed test, including `terrain_screen_thick_case81` and
`terrain_screens_double_case91` (`crates/envi-harness/tests/terrain.rs`). This does not
invalidate the oracle's value for catching *branch/numerics* transcription errors (the
stated purpose), but the review's oracle-trust mandate calls for flagging this explicitly:
these tests are a genuine floating-point/branch cross-check, not an independent physics
verification, for everything except the Faddeeva evaluation itself.

**Fix:** No code change required for Phase 2 (this is a documentation/scope note). Consider
adding a plan/backlog item to source at least one Sub-model 5/6 fixture from a source
genuinely external to this codebase (e.g. a published Nord2000 validation report figure) so
the "single representative top" and middle-region conventions get an independent check
before Phase 3/4 builds further on them.

### WR-04: Turbulence-decorrelation `Fc` is duplicated between `coherence.rs` and `screen.rs`

**File:** `crates/envi-engine/src/propagation/terrain_effect/screen.rs:283-294` vs.
`crates/envi-engine/src/propagation/coherence.rs:82-103`

**Issue:** `screen.rs::fc` re-implements the exact turbulence-decorrelation formula (Eq. 113)
that `coherence.rs::coherence_f` already computes inline (the `fc` branch), including the
`5.888e-3` constant and the `22.0/3.0` factor. The module doc even explains why a *local*
copy exists ("so the screen engine can form F₄ = Ff·Fc_S·Fc_R ... without an API change to
`coherence`"), but this means the Eq. 113 constants now live in two places. If Assumption
A3's exact constants are ever corrected (the module doc for `coherence.rs` already flags
them as a transcription-uncertain assumption), only one of the two copies is likely to be
updated, silently reintroducing a Fc mismatch between the flat-ground and screen paths.

**Fix:** Extract the turbulence-decorrelation core (everything except the `rho`/`d`
arguments) into a single `pub(crate)` helper in `coherence.rs` that both `coherence_f` and
`screen.rs::fc` call, e.g. `pub(crate) fn fc_core(f_hz, cv2, ct2, t_air_c, c0, rho, d) -> f64`.

### WR-05: Dead closure `s_of` in `run_eight_path`

**File:** `crates/envi-engine/src/propagation/terrain_effect/screen.rs:759-765, 795`

**Issue:**

```rust
let s_of = |b: usize, v: &Option<SideVars>| -> [f64; 2] {
    if b != 0 { v.map(|x| x.image).unwrap_or(s) } else { s }
};
...
let _ = s_of; // (kept for documentation of the reflection convention)
```

`s_of` is defined but never called — `sp`/`rp` below are computed by separate inline logic
(`let sp = if use_before { before.unwrap().image } else { s };` etc.). The closure is
discarded via `let _ = s_of;` purely to silence the unused-variable lint, per the comment.
This is dead code masquerading as documentation: if the real `sp`/`rp` logic below ever
diverges from what `s_of` describes, nothing will catch the drift, because `s_of` is never
exercised.

**Fix:** Either delete `s_of` entirely (put the "reflection convention" explanation in a
plain comment), or actually use it to compute `sp` (and an equivalent for `rp`) so the
"documentation" is enforced by the compiler/tests rather than aspirational.

## Info

### IN-01: `scene_build::build_scene` doesn't branch on `CaseKind::Terrain`

**File:** `crates/envi-harness/src/scene_build.rs:69-78`

**Issue:** `build_scene`'s match only handles `ForceStraightRoad` and `FreeField | Geometry`;
`CaseKind::Terrain` falls into the generic `other => Err(...)` arm. This is currently
harmless because `run_terrain_case` (in `lib.rs`) calls `build_terrain_inputs` directly and
never routes terrain cases through `build_scene` — but it's a latent trap: a future
refactor that unifies case dispatch through `build_scene` (a natural simplification) would
silently break terrain cases with a generic "not implemented" error instead of a clear
signal.

**Fix:** Add an explicit `CaseKind::Terrain => Err(anyhow!("terrain cases build scenes via build_terrain_inputs, not build_scene"))` arm (or a comment) so the omission reads as a decision, not an oversight.

### IN-02: `incoherent_rho`'s plain `atan` is only correct because `Re(Ẑ_G) > 1` always holds

**File:** `crates/envi-engine/src/propagation/ground.rs:104-106` vs.
`tools/nord2000_oracle/gen_screen_fixtures.py:120-124` (`atan2`)

**Issue:** `ground.rs` and `flat_models.py` compute `(y / (1.0 + x)).atan()`, while
`gen_screen_fixtures.py::_rho_i` uses the quadrant-correct `atan2(y, 1.0 + x)`. These only
agree because `1 + x > 0` always holds for `Ẑ_G` from `ground_impedance` (`Re(Ẑ_G) = 1 +
9.08·X^{-0.75} > 1` for any positive `X`). This invariant is not asserted or documented at
the `incoherent_rho` call site, so a future caller passing a hand-constructed `Ẑ_G` with
`Re < -1` (e.g. in a unit test or a future refraction-adjusted impedance) would silently get
the wrong branch with plain `atan`.

**Fix:** Either use `y.atan2(1.0 + x)` to make the formula robust regardless of caller, or
add a debug assertion / comment noting the `Re(Ẑ_G) > -1` invariant it currently relies on.

### IN-03: `normalize_weights` returns a value duplicated inside its own return type

**File:** `crates/envi-engine/src/propagation/terrain_effect/screen.rs:404-416`

**Issue:** `normalize_weights` returns `(f64, SideWeightsRaw)` where the first tuple element
(`w_t`) is also stored as `SideWeightsRaw::w_t`. Every call site discards the first element
(`let (_, src_raw) = normalize_weights(...)`), making the tuple wrapper redundant.

**Fix:** Change the signature to return `SideWeightsRaw` directly and drop the tuple.

---

_Reviewed: 2026-07-08T00:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
