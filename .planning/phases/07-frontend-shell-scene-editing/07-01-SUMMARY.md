---
phase: 07-frontend-shell-scene-editing
plan: 01
subsystem: envi-store (persistence / DTO mirror)
status: complete
tags: [dto, interpolation, band-index, isolation-spectrum, forest, tryfrom, quarantine]
requires:
  - envi_engine::propagation::transmission::IsolationSpectrum (validating new(), MAX_R_DB)
  - envi_engine::forest::ForestCrossing (validating new())
  - envi_engine::freq::N_BANDS (=105)
provides:
  - envi_store::interpolate::Resolution
  - envi_store::interpolate::interpolate (shared band-index core, D-05)
  - envi_store::dto::AuthoredSpectrumDto
  - envi_store::dto::IsolationSpectrumDto (+ TryFrom into IsolationSpectrum)
  - envi_store::dto::ForestParamsDto (+ TryFrom into ForestCrossing)
affects:
  - Phase-7 POST /meta/interpolate-spectrum endpoint (consumes interpolate())
  - PUT /scene validation (consumes the same core — no divergence)
  - read-path r_db[105] derivation (D-06)
tech-stack:
  added: []
  patterns:
    - "validating TryFrom (BandSpectrumDto / TerrainProfileDto analogs)"
    - "engine constructor as sole range gate — DTO never clamps"
    - "authored-only persistence (D-06), derived-on-read"
key-files:
  created:
    - crates/envi-store/src/interpolate.rs
  modified:
    - crates/envi-store/src/dto.rs
    - crates/envi-store/src/lib.rs
decisions:
  - "interpolate() does NOT clamp to [0,1000] — the engine IsolationSpectrum::new is the sole, un-bypassed range gate, so an out-of-range authored R is rejected (never silently clamped). Resolves a self-contradiction in the plan against must-have #2 / Task-2 acceptance / threat T-07-01-02."
  - "IsolationSpectrumDto persists authored { resolution, values } ONLY; r_db[105] is derived on read (D-06 — no second persisted field, no shadow cache)."
  - "ForestParamsDto authors the Phase-7 subset only; d_m gets a 0.0 placeholder (Phase-9 solve-time geometry), absorption defaults to 0.0 when unauthored."
metrics:
  tasks_completed: 2
  files_created: 1
  files_modified: 2
  lines_added: 553
  commits: 2
  duration: "~35 min"
  completed: 2026-07-10
---

# Phase 07 Plan 01: Store-side Isolation/Forest DTOs + Shared Interpolation Core Summary

Typed, validated `IsolationSpectrumDto` + `ForestParamsDto` land in `envi-store` with a single shared band-index interpolation core (`interpolate.rs`), proving the drawn acoustic datum converts through the engine's validating constructors — with `envi-engine` byte-identical and its 3-dep quarantine intact.

## What was built

**Task 1 — `crates/envi-store/src/interpolate.rs` (commit `3b94259`)**
- `pub enum Resolution { Octave, Third, Twelfth }` (serde lowercase) + `pub fn interpolate(Resolution, &[f64]) -> Result<[f64; 105], StoreError>`.
- Anchor stride math (07-RESEARCH Pattern 6): Octave → 9 anchors at `4 + 12k` (4…100); Third → 27 at `4k` (0…104); Twelfth → identity over 105.
- Linear **in band index** between bracketing anchors; anchors copied bit-for-bit; **flat-hold** extrapolation outside the authored span (A1, documented in the module I/O header).
- Validates length (`BadBandCount`) + finiteness (`NonFinite`) before any allocation over attacker length (threat T-07-01-01).

**Task 2 — `crates/envi-store/src/dto.rs` (commit `9d9c6be`)**
- `AuthoredSpectrumDto { resolution, values }`, `IsolationSpectrumDto { authored }` — authored-only persistence (D-06).
- `ForestParamsDto { density_per_m2, stem_radius_m, height_m, absorption? }`.
- `TryFrom<&IsolationSpectrumDto> for IsolationSpectrum` (interpolate → `new()` gate) and `TryFrom<&ForestParamsDto> for ForestCrossing` (authored subset + `d_m=0.0` placeholder → `new()`), both mapping engine rejection to `StoreError::Engine`.
- All DTOs `deny_unknown_fields`; lossless JSON round-trip; inline `#[cfg(test)]` proving conversion, rejection, and round-trip.

## Verification results

- `cargo test -p envi-store` — 6 interpolate + 5 new dto tests (11 dto total) green; full `cargo test` workspace run exit 0, zero failures.
- `cargo clippy --all-targets -- -D warnings` — clean. `cargo fmt --check` — clean.
- `git diff --quiet crates/envi-engine/` — engine byte-identical (VERIFY-ONLY honored).
- `cargo tree -p envi-engine -e normal --depth 1` — exactly `ndarray`, `num-complex`, `thiserror`.
- conj gate (`grep '\.conj()' crates/envi-engine/src/propagation/`, non-comment) = 0; C-crate gate (`proj-sys`/`gdal`) = 0.
- No `r_db` field persisted anywhere (D-06 upheld).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `interpolate()` does NOT clamp output to `[0, 1000]`**
- **Found during:** Task 1.
- **Issue:** Task 1's action says "Clamp every output to `[0.0, 1000.0]`", but this directly contradicts the plan's own `must_haves.truths` #2 ("never silently clamped to a wrong value"), Task 2's acceptance ("input value 2000 → `Err(Engine)` … never a clamped-to-wrong result"), and the threat register (T-07-01-01 = length+finiteness only; T-07-01-02 = engine `new()` is the `[0,1000]` gate). Clamping in `interpolate` would make `IsolationSpectrum::new` accept a masked `1000` and defeat the tested rejection.
- **Fix:** `interpolate` validates length + finiteness only; range enforcement is left entirely to `IsolationSpectrum::new`. Documented explicitly in the module I/O header ("No range clamp here").
- **Files modified:** `crates/envi-store/src/interpolate.rs`.
- **Commit:** `3b94259`.
- **Verification:** `isolation_spectrum_dto_rejects_out_of_range_never_clamps` asserts `R=2000` → `StoreError::Engine`; `does_not_clamp_out_of_range` asserts the value passes through the core unclamped.

## Notes

- The `interpolate()` core is deliberately consumed by all three future call sites (read-path `r_db` derivation, `POST /meta/interpolate-spectrum`, `PUT /scene` validation) so they cannot diverge (D-05).
- `.planning/STATE.md` and `.planning/config.json` carry pre-existing unstaged edits from a prior session; both were left untouched per the plan's instruction.

## Self-Check: PASSED
- `crates/envi-store/src/interpolate.rs` — FOUND
- `crates/envi-store/src/dto.rs` (modified) — FOUND
- commit `3b94259` — FOUND
- commit `9d9c6be` — FOUND
