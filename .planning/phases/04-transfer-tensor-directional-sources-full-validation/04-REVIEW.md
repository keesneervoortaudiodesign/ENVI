---
phase: 04-transfer-tensor-directional-sources-full-validation
reviewed: 2026-07-09T00:00:00Z
depth: standard
files_reviewed: 24
files_reviewed_list:
  - crates/envi-engine/src/directivity.rs
  - crates/envi-engine/src/lib.rs
  - crates/envi-engine/src/propagation/mod.rs
  - crates/envi-engine/src/propagation/terrain_effect/mod.rs
  - crates/envi-engine/src/propagation/terrain_effect/submodel11.rs
  - crates/envi-engine/src/propagation/terrain_effect/submodel2.rs
  - crates/envi-engine/src/propagation/terrain_effect/submodel3.rs
  - crates/envi-engine/src/propagation/terrain_effect/submodel8.rs
  - crates/envi-engine/src/solver.rs
  - crates/envi-engine/src/tensor.rs
  - crates/envi-harness/src/capability.rs
  - crates/envi-harness/src/cases/mod.rs
  - crates/envi-harness/src/cases/xls.rs
  - crates/envi-harness/src/compare.rs
  - crates/envi-harness/src/emission/coefficients.rs
  - crates/envi-harness/src/emission/mod.rs
  - crates/envi-harness/src/emission/passby.rs
  - crates/envi-harness/src/facade.rs
  - crates/envi-harness/src/lib.rs
  - crates/envi-harness/src/scene_build.rs
  - crates/envi-harness/src/weather/route1.rs
  - crates/envi-harness/tests/emission_force_delta.rs
  - crates/envi-harness/tests/oracle_submodel11.rs
  - crates/envi-harness/tests/oracle_submodel3.rs
findings:
  critical: 0
  warning: 3
  info: 4
  total: 7
status: issues_found
---

# Phase 4: Code Review Report

**Reviewed:** 2026-07-09
**Depth:** standard
**Files Reviewed:** 24
**Status:** issues_found

## Summary

The Phase-4 transfer-tensor / directional-source / full-validation code is mature
and defensively written. The complex/phase contract, the `.conj()` quarantine,
the two-channel readout laws, and the honest-green Skip posture are all consistent
with the project conventions (and were treated as ground truth, not flagged). The
untrusted `.xls` boundary is well guarded: `MAX_SHEETS` / `MAX_PROFILE_ROWS` /
`MAX_COORD_ROWS` / 27-row spectrum caps, `require_finite` NaN screening, strictly-
ascending X validation, and a canonicalizing `confine` path-traversal guard. Every
numeric energy/log reduction I checked clamps with `.max(f64::MIN_POSITIVE)` before
`log10`, and the ±90° pass-by / `1/cos θ` singularities are capped.

I found **no BLOCKER-class defects** — no injection, data-loss, panic-on-untrusted-
input, or clearly-incorrect shipping computation. The findings below are contract
gaps and robustness/consistency issues: three WARNINGs (a documented-but-unenforced
validation contract, a doc/impl mismatch in the contour profile builder, and one
production `.expect()` that contradicts the "never panic on data" posture) and four
INFO items.

## Warnings

### WR-01: `readout_incoherent` documents non-finite rejection it does not perform

**File:** `crates/envi-engine/src/tensor.rs:330-361`
**Issue:** The doc comment states the function errors "if the channels disagree in
shape, `w` has the wrong sub-source or band count, **or any input is non-finite**."
The implementation only validates `w` (via `validate_real_gain`) and the channel
shape. It never checks `h_coh` / `p_incoh_abs` for finiteness, so a NaN/∞ cell in
the tensor flows through `hf.norm_sqr() + *pf` into the energy sum and silently
produces a NaN band level rather than a typed `SinkError::NonFinite`. Today the
tensor is finiteness-validated at fill time (`InMemorySink`/`CountingSink`), so the
risk is low, but `readout_incoherent` is `pub` and can be called on any
`ArrayView3`, and its own contract promises the guard. (`readout_coherent`'s doc,
by contrast, only claims to validate `g`, and is accurate.)
**Fix:** Either add the missing guard before the accumulation loop, e.g.
```rust
if h_coh.iter().any(|z| !z.re.is_finite() || !z.im.is_finite())
    || p_incoh_abs.iter().any(|v| !v.is_finite())
{
    return Err(SinkError::NonFinite { what: "readout tensor channel" });
}
```
or narrow the doc comment to say only `w` is validated (matching `readout_coherent`).

### WR-02: `contour_profile` never prepends the `x = 0` source-foot endpoint its doc promises

**File:** `crates/envi-harness/src/cases/xls.rs:565-607`
**Issue:** The doc states: "Samples are sorted by distance (deduplicated), a `x = 0`
endpoint at the source foot is prepended if missing …". The implementation collects
only the contour/cut intersection samples and dedup-sorts them; there is no `x = 0`
prepend. When the first contour crossing sits at `t > 0`, the returned
`TerrainProfile` begins at that offset rather than at the source foot, so the
along-cut geometry is shifted relative to the documented contract. Current impact
is low (curved cases have no live `build_scene` arm — `scene_build::build_scene`
returns an error for `ForceCurvedRoad`, and the group runner Skips), but this is a
real logic defect that will surface when the curved path is numerically wired.
**Fix:** Either implement the prepend the doc describes:
```rust
if points.first().is_none_or(|p| p[0].abs() > 1e-6) {
    points.insert(0, [0.0, points.first().map_or(0.0, |p| p[1])]);
}
```
(choosing a defensible source-foot elevation), or remove the "prepended if missing"
sentence so the contract matches behavior.

### WR-03: production `.expect()` on `interp.screens` contradicts the "never panic on data" posture

**File:** `crates/envi-engine/src/propagation/terrain_effect/mod.rs:524`
**Issue:** `let (s1, s2) = interp.screens.expect("double screen carries two shapes");`
panics if a `TerrainInterpretation` ever reports `class == DoubleScreen` with
`screens == None`. The module family repeatedly advertises "typed errors, never
panics on data-dependent paths," and `interp` is derived from caller-controlled
terrain (`interpret_terrain(profile, …)`). The invariant is presently upheld by
`interpret_terrain`, but coupling engine correctness to an unchecked cross-function
invariant via `.expect()` is exactly the panic surface the crate's error taxonomy
is meant to avoid.
**Fix:** Return a typed `PropagationError` instead of panicking, e.g. surface a
`DegenerateRayGeometry`/dedicated variant when `class == DoubleScreen && screens.is_none()`,
so a malformed interpretation degrades to a Skipped/typed-error rather than a crash.

## Info

### IN-01: length checks are `debug_assert_eq!` only — release builds silently truncate via `zip`

**File:** `crates/envi-harness/src/compare.rs:129, 271`
**Issue:** `compare_spectrum` and `compare_pointwise` guard equal lengths with
`debug_assert_eq!`, then compare with `got.iter().zip(want)`. In a release build a
mismatched-length input compares only the shorter prefix and can report `pass` while
ignoring the missing tail. All current callers pass equal 27/105 arrays, so this is
latent.
**Fix:** Promote to a runtime check returning an empty/failed report (or a `Result`)
on unequal lengths.

### IN-02: inconsistent zero-vehicle handling between the two `LAeq,24h` helpers

**File:** `crates/envi-harness/src/compare.rs:376-378` vs `crates/envi-harness/src/emission/passby.rs:303-317`
**Issue:** `passby::laeq_24h_from_lae` returns a typed `PassbyError::NonFinite` for
`n_vehicles == 0`, while `compare::l_aeq_24h` silently applies
`n_vehicles.max(f64::MIN_POSITIVE)` and returns a large negative number for `n = 0`.
Two code paths, two behaviors for the same degenerate input.
**Fix:** Pick one contract (prefer the typed-error form) and route the other through it.

### IN-03: row-scan caps are applied inconsistently across the `.xls` parsers

**File:** `crates/envi-harness/src/cases/xls.rs:389` vs `:220, :661-700`
**Issue:** `read_coord_section` bounds its scan with
`last_row(range).min(MAX_COORD_ROWS as u32)`, but the label/spectrum scans in
`parse_sheet` and `parse_case_sheet_spectrum` iterate raw `0..=last_row(range)`.
The parsed vectors are still capped (spectrum breaks/errors at 27), so this is a
scan-cost inconsistency rather than an allocation-DoS, and calamine has already
materialized the sheet — but the cap discipline is uneven.
**Fix:** Apply the same `MAX_*`-clamped upper bound to all sheet scans for consistency.

### IN-04: `DirectivityBalloon::eval` returns unity gain (0 dB) on a degenerate direction

**File:** `crates/envi-engine/src/directivity.rs:374-383, 651-663`
**Issue:** A zero-length or non-finite `dir_local` returns `[0.0; N_BANDS]`
(ΔL = 0 dB ⇒ ×1.0 gain). This is the documented, deliberately non-panicking
fallback (avoids a NaN into the tensor), but it silently substitutes the *neutral*
directivity for the *actual* pattern rather than signaling the caller bug — a caller
passing a coincident source/receiver gets an omnidirectional result with no
diagnostic.
**Fix:** Acceptable as-is given the documented contract; optionally add a
`debug_assert!` on `dir` finiteness/length so the caller bug is caught in tests
without changing the safe production fallback.

---

_Reviewed: 2026-07-09_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
