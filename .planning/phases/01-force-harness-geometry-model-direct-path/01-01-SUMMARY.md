---
phase: 01-force-harness-geometry-model-direct-path
plan: 01
subsystem: testing
tags: [rust, cargo-workspace, calamine, libtest-mimic, ndarray, num-complex, thiserror, serde, nord2000, force, frequency-axis]

# Dependency graph
requires: []
provides:
  - "Two-crate cargo workspace: envi-engine (pure math, zero I/O deps) + envi-harness (all file parsing / comparison / test execution)"
  - "envi-engine::freq — 105-point 1/12-octave FreqAxis (f = 1000·G^(x/12)), N_BANDS=105, N_THIRD_OCT=27, third_octave_pick(i)=centres[i*4], NOMINAL_THIRD_OCT labels, LazyLock singleton"
  - "envi-harness::cases — CaseDefinition shared representation, CaseKind, ReferenceVersion, PropagationParams, TerrainRow, ReferenceSpectrum, discover() with path-traversal guard, TOML + FORCE .xls loaders"
  - "envi-harness::compare — compare_spectrum, compare_27_band, ComparisonReport, BandDeviation, BandVerdict, dip-shift allowance, FORCE tolerance constants, a_weighting_db"
  - "envi-harness::capability — Capability enum, required_capabilities(), implemented_capabilities() (empty in this plan)"
  - "run_case(&CaseDefinition) -> Outcome {Pass, Fail(ComparisonReport), Skipped(String)} + libtest-mimic force test target + report CLI"
affects: [01-02-scene-geometry, 01-03-direct-path, phase-02-ground-diffraction, phase-03-refraction, phase-04-tensor-emission]

# Tech tracking
tech-stack:
  added: [ndarray 0.17, num-complex 0.4, thiserror 2, calamine 0.36, serde 1, toml 1, anyhow, libtest-mimic 0.8, approx 0.5]
  patterns: [I/O quarantine (engine has zero file-format deps), capability gating (required − implemented = Skip reason), label-anchored .xls parsing, index-based band mapping (never float-equality on nominal frequencies), reference_version provenance on every case, refs/ gitignored with SHA-256 manifest]

key-files:
  created: [Cargo.toml, .gitignore, refs/fetch.sh, refs/refs.sha256, cases/freefield_100m.toml, crates/envi-engine/Cargo.toml, crates/envi-engine/src/lib.rs, crates/envi-engine/src/freq.rs, crates/envi-harness/Cargo.toml, crates/envi-harness/src/lib.rs, crates/envi-harness/src/main.rs, crates/envi-harness/src/cases/mod.rs, crates/envi-harness/src/cases/toml.rs, crates/envi-harness/src/cases/xls.rs, crates/envi-harness/src/compare.rs, crates/envi-harness/src/capability.rs, crates/envi-harness/tests/force.rs]
  modified: []

key-decisions:
  - "1/12-octave grid uses the x/12 rule (not IEC even-b midbands) so every 4th point is an exact 1/3-octave centre — FORCE comparison is an index pick, not aggregation"
  - "envi-engine carries only ndarray/num-complex/thiserror; all parsing lives in envi-harness — makes 'harness before propagation' an architectural property"
  - "Capability gate: implemented_capabilities() is empty, so every case (incl. free-field TOML and 62 FORCE road cases) reports Skipped(requires: …) — the harness fails meaningfully before any physics exists"
  - "FORCE .xls read at full float precision (LAeq,24h = 39.39836757521) via label-anchored calamine parsing, never the rounded report-appendix values"

patterns-established:
  - "I/O quarantine: engine crate has zero I/O deps; harness owns calamine/serde"
  - "Capability gating: run_case computes required − implemented and returns a named Skip reason; later phases flip flags, no harness rewrite"
  - "Index-based 27-band mapping through third_octave_pick; nominal Hz labels are display-only"
  - "Untrusted-input posture: no unwrap on cell/file data, typed CaseLoadError, caps (≤200 sheets, ≤10k profile rows, exactly 27 spectrum rows), NaN/Inf + non-ascending-X rejection, path-traversal confinement"

requirements-completed: [VAL-01]

# Metrics
duration: 25min
completed: 2026-07-07
status: complete
---

# Phase 1 Plan 01: FORCE Harness Spine Summary

**A two-crate Rust workspace whose FORCE-driven test harness loads real FORCE `.xls` cases + synthetic TOML fixtures and reports per-case outcomes — every road case Skipped with a named missing-capability list — before any propagation physics exists.**

## Performance

- **Duration:** ~25 min (this session: Task 3 GREEN completion + full quality-gate greening)
- **Completed:** 2026-07-07
- **Tasks:** 3 (all TDD; Tasks 1–2 committed in prior sessions, Task 3 completed this session)
- **Files modified this session:** 8 (compare.rs, capability.rs, main.rs, tests/force.rs + fmt normalization of freq.rs and the three loader files)

## Accomplishments

- **Walking-skeleton spine stands and fails meaningfully.** `cargo test --workspace` is green: 28 unit tests + the `harness::discovery` meta-test pass, and all 66 FORCE/TOML cases report as **ignored** with their requires-list visible (e.g. `[requires: emission-model, ground-effect]`). No propagation code exists yet — exactly ROADMAP success criterion 1.
- **Comparator with FORCE tolerances + dip-shift allowance.** `compare_27_band` produces per-band signed deviations against a 27-band reference, encodes the 1 dB overall / 1 dB per-band tolerance (EP1335 §6) and the ±1-band ground-dip warning downgrade (default ON for straight-road kinds).
- **Capability gate wired end-to-end.** `required_capabilities` derives emission-model + ground-effect for road cases, plus diffraction (screen descriptions) and refraction (nonzero wind `u` / temperature gradient `dt/dz`, or up/downwind description); `implemented_capabilities` is empty, so `run_case` gates everything to Skipped.
- **CLI report** (`cargo run -p envi-harness -- report`) prints the fixed-width per-case table (id | kind | reference | outcome | detail) — the walking skeleton's human-readable write end.
- **All quality gates pass:** `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `#![deny(unsafe_code)]` on envi-engine, engine dependency quarantine (only ndarray/num-complex/thiserror).

## Task Commits

1. **Task 1: workspace + 1/12-octave freq axis + failing e2e harness (RED)** — `256d7be` (test), `c5ede3c` (feat)
2. **Task 2: TOML + FORCE .xls loaders + real discovery** — `cbe40e4` (test), `f67907e` (feat)
3. **Task 3: comparator + capability gate + meaningful-failure runner (GREEN)** — `4d771d6` (test), `3c4eb86` (feat, this session)

_TDD plan: each task has a RED test commit followed by a GREEN feat commit._

## Public API (for plans 01-02 / 01-03 to wire into)

**`envi_engine::freq`**
- `N_BANDS: usize = 105`, `N_THIRD_OCT: usize = 27`, `G: f64` (10^0.3), `NOMINAL_THIRD_OCT: [f64; 27]`
- `FreqAxis { centres: [f64; 105] }`, `FreqAxis::new()`, `FreqAxis::third_octave_pick(third_idx) -> f64` (= `centres[third_idx*4]`), plus a `LazyLock<FreqAxis>` singleton

**`envi_harness::cases`**
- `struct CaseDefinition { id, name, kind: CaseKind, reference_version: ReferenceVersion, description, source_position: Option<[f64;3]>, receiver_position: Option<[f64;3]>, propagation: PropagationParams, terrain_profile: Vec<TerrainRow>, reference_spectrum: Option<ReferenceSpectrum>, expected: Option<SyntheticExpected> }`
- `enum CaseKind { FreeField, Geometry, ForceStraightRoad, ForceCurvedRoad, ForceCityStreet, ForceYearlyAverage }`
- `enum ReferenceVersion { Analytic, Force2009, Force2010 }` (`as_str()`)
- `PropagationParams` (all worksheet fields `Option<f64>`; `rh_percent` default 70.0, `pressure_kpa` default 101.325)
- `TerrainRow { x_m, z_m, flow_resistivity_kns_m4, roughness_m }`, `ReferenceSpectrum { bands: Vec<SpectrumRow>, laeq_24h_db, lae_db, lamax_db }`, `SyntheticExpected { tolerance_db, bands }`
- `discover(refs_dir, cases_dir) -> Discovery { cases: Vec<DiscoveredCase>, notes: Vec<String> }`, `DiscoveredCase { id, case: Result<CaseDefinition, CaseLoadError> }`
- `toml::load_toml_case(&Path)`, `xls::load_straight_road(&Path)`, typed `CaseLoadError`

**`envi_harness::capability`**
- `enum Capability { FreeField, Geometry, EmissionModel, GroundEffect, Diffraction, Refraction, … }` (`as_str()`)
- `required_capabilities(&CaseDefinition) -> BTreeSet<Capability>`
- `implemented_capabilities() -> BTreeSet<Capability>` (**empty in this plan** — plans 01-02/01-03 extend it)

**`envi_harness::compare`**
- `compare_spectrum(got, want, tol_db) -> (Vec<BandDeviation>, bool)`
- `compare_27_band(got, want, overall: Option<(f64,f64)>, dip_shift_rule: bool) -> ComparisonReport`
- `ComparisonReport { deviations, verdicts, max_abs_dev_db, overall_dev_db, tol_band_db, tol_overall_db, warnings, pass }`, `render_table()`
- `BandDeviation`, `BandVerdict {Ok, DipShiftWarning, Fail}`, `a_weighting_db(f)`
- consts `FORCE_TOL_OVERALL_DB=1.0`, `FORCE_TOL_BAND_DB=1.0`, `FORCE_INVESTIGATE_DB=0.5`

**`envi_harness` (crate root)**
- `run_case(&CaseDefinition) -> Outcome`
- `enum Outcome { Pass, Fail(ComparisonReport), Skipped(String) }`

## Reference data (refs/)

**Fetched successfully this session.** All four FORCE workbooks (TestStraightRoad / TestCurvedRoad / TestCityStreet / TestYearlyAverage .xls) plus AV 1106-07, AV 1849-00 Part 1, and Env. Project 1335 PDFs are present under the git-ignored `refs/`, pinned in the committed `refs/refs.sha256` manifest. Only `refs/fetch.sh` and `refs/refs.sha256` are tracked by git — no copyrighted `.xls`/PDF is staged (T-01-04 verified).

## .xls layout notes

No deviation from the RESEARCH-verified layout was encountered. `load_straight_road` parses all 62 straight-road worksheets; case "1" yields `LAeq,24h = 39.39836757521` (full precision, abs 1e-9), terrain profile first X = 3.25 m, strictly ascending — all confirmed by the passing `straight_road_loads_62_cases_with_full_precision_anchor` test (runs because refs are present; auto-ignores when absent). Label-anchored parsing (col A/F label cells) is primary; fixed row positions are the fallback, per plan.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Float-equality assertion in a committed RED test**
- **Found during:** Task 3 GREEN (running `cargo test --workspace`)
- **Issue:** `report_carries_max_deviation_and_overall` asserted `assert_relative_eq!(report.max_abs_dev_db, 0.9)` with the default f64 epsilon. `max_abs_dev_db` is computed as `30.0 - 29.1 = 0.8999999999999986`, whose relative error (~1.5e-15) exceeds `f64::EPSILON`, so the test failed.
- **Fix:** Added `epsilon = 1e-12`, matching the adjacent `overall_dev_db` assertion on line 300 (same test).
- **Files modified:** crates/envi-harness/src/compare.rs (test module)
- **Commit:** `3c4eb86`

**2. [Rule 3 - Blocking] `cargo fmt --check` failures from prior commits**
- **Found during:** Task 3 quality-gate run
- **Issue:** freq.rs and the three case-loader files carried unformatted long lines from earlier commits; `cargo fmt --check` (a wave definition-of-done gate) was red.
- **Fix:** Ran `cargo fmt`; whitespace-only normalization. Committed with the Task 3 GREEN change.
- **Commit:** `3c4eb86`

## Known Stubs

None that block the plan goal. `implemented_capabilities()` returns an empty set and every `run_case` dispatch arm returns `Skipped` **by design** — this plan's explicit contract is that no physics exists yet, so every case must gate to a named missing-capability skip. Plan 01-02 adds the Geometry dispatch/capability; plan 01-03 adds FreeField. The `main.rs` non-`report` path and unused-for-now fields (e.g. `expected`, `reference_spectrum`) are consumed by 01-02/01-03.

## Verification Evidence

- `cargo build --workspace` — finished, no errors
- `cargo test --workspace` — 4 (engine) + 24 (harness unit) + 1 discovery pass; 66 FORCE/TOML cases ignored with requires-lists; 0 failed
- `cargo clippy --all-targets -- -D warnings` — finished, zero warnings
- `cargo fmt --check` — clean (exit 0)
- `cargo tree -p envi-engine -e normal --depth 1` — only ndarray, num-complex, thiserror (I/O quarantine holds)
- `git ls-files | grep -iE '\.(xls|pdf)$'` — no FORCE `.xls`/AV PDF tracked (a pre-existing d&b TI-386 reference PDF under docs/references/ predates this plan and is out of scope)
- `cargo run -p envi-harness -- report` — prints per-case table; `grep -c Skipped` = 66

## Self-Check: PASSED

All listed files exist on disk and all six task commits (256d7be, c5ede3c, cbe40e4, f67907e, 4d771d6, 3c4eb86) are present in git history.
