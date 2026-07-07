---
phase: 01-force-harness-geometry-model-direct-path
verified: 2026-07-07T18:44:00Z
status: passed
score: 4/4 truths verified (roadmap success criteria); 7/7 requirements satisfied
behavior_unverified: 0
overrides_applied: 0
deferred:
  - truth: "Screen edges parse from FORCE test-case files into the semantic Barrier type"
    addressed_in: "Phase 2"
    evidence: "Phase 2 goal: 'Homogeneous-atmosphere FORCE cases with ground and screens pass ... single/multi-edge diffraction'; Phase 2 success criterion 2: 'FORCE cases with single-edge and multiple-edge screens/barriers match reference within tolerance' (ENG-03). 01-RESEARCH explicitly scopes the screens-into-terrain-profile merge (AV 1106/07 §5.1) to Phase 2; 01-02-PLAN.md documents Barrier as 'a semantic object able to LIST its screen edges' only for Phase 1."
---

# Phase 1: FORCE Harness, Geometry Model & Direct Path Verification Report

**Phase Goal:** A FORCE-driven test harness and canonical semantic 2.5D scene model exist BEFORE any propagation code, and the simplest full path — geometrical divergence + ISO 9613-1 air absorption, evaluated as COMPLEX values at 1/12-octave points — runs through the harness and matches reference.

**Requirements:** VAL-01, GEO-01, GEO-02, GEO-03, ENG-01, ENG-04, SRC-01

**Verified:** 2026-07-07T18:44:00Z
**Status:** passed
**Re-verification:** No — initial verification

All claims below were checked by actually running `cargo build/test/clippy/fmt` and `cargo run -p envi-harness -- report` in the repo (`D:\====CLAUDE\ENVI`), and by reading the source of every artifact cited, not by trusting SUMMARY.md prose.

## Goal Achievement

### Observable Truths (ROADMAP Phase 1 Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Running the test suite loads FORCE road-traffic test-case files, executes the engine on them, and reports per-case pass/fail against reference values with per-band deviations — the harness runs (and fails meaningfully) before propagation code exists | ✓ VERIFIED | `cargo test --workspace`: 31 (envi-engine) + 31 (envi-harness unit) tests pass; `force` integration target reports **5 passed, 65 ignored, 0 failed**. `cargo run -p envi-harness -- report` shows all 62 `straight_road::N` + `curved_road::workbook` + `city_street::workbook` + `yearly_average::workbook` cases as `Skipped  requires: emission-model, ground-effect[, diffraction][, refraction]` — never a false `Pass`. `crates/envi-harness/src/capability.rs` computes `required − implemented` per case; `implemented_capabilities()` returns exactly `{Geometry, FreeField}` (line 98), so FORCE road cases (which additionally require `EmissionModel`/`GroundEffect`) always gate to `Skipped` before any physics runs. `ComparisonReport` (compare.rs) carries per-band signed deviations, FORCE tolerances (1.0 dB band/overall, Env. Project 1335 §6), and a dip-shift allowance — this machinery is exercised by the 4 synthetic Pass cases and unit tests (`compare::tests::*`, 12 tests, all green). |
| 2 | A FORCE case's terrain profile, ground-impedance segments, and screen edges parse into the canonical semantic 2.5D scene (Source, Receiver, Barrier, TerrainProfile; projected metric CRS, Z-up), and source→receiver azimuth + reflection-path geometry computed from it match hand-computed values | ✓ VERIFIED (with one item deferred — see Deferred Items) | `crates/envi-engine/src/scene.rs` defines the full vocabulary: `Scene{crs,sources,receivers,barriers,buildings,terrain}`, `CrsInfo::local_metric()` (Z-up, projected metric, documented lines 3-10), `TerrainProfile::new` (typed `SceneError`, strictly-ascending-X + finite validation), `TerrainProfile::endpoints` (hSv/hRv convention). `crates/envi-harness/src/scene_build.rs::build_force_straight_road` parses the real `TestStraightRoad.xls` case 1 (ran live: `cargo test -p envi-harness force_case_1_applies_lane_and_height_conventions` → **1 passed**) and reproduces the **97.5 m** (not naive 100 m) horizontal distance anchor, plus the MIXED-impedance segments (σ=20000 road, σ=12.5 grass) — proving ground-impedance segments genuinely parse from the authoritative file, not an assumption. `crates/envi-engine/src/geometry.rs`: `azimuth_deg` (4 hand-computed anchors: 0/90/45/270°, all pass at 1e-12), `reflect_over_segment` (flat-ground `√116`/`atan(2/5)` anchor and sloped-segment `√104`/`atan(2/3)` anchor, both pass at 1e-12), `reflection_outside_the_segment_is_flagged_invalid` test. All these ran green in `cargo test --workspace`. **Screen edges are NOT yet parsed from any FORCE `.xls` into a populated `Barrier`** — `Barrier` exists as a type with `edges()` (scene.rs:158-176) but is only exercised by hand-constructed test data (`barrier_lists_its_screen_edges`, scene.rs:440), and `crates/envi-harness/src/cases/xls.rs::parse_sheet` does not read any screen/barrier columns. This is explicitly scoped to Phase 2 in `01-RESEARCH.md` line 362 ("Screens are ultimately merged into the terrain profile ... Phase 1 only needs Barrier as a semantic object + the ability to list screen edges; the merge algorithm is Phase 2") and 01-02-PLAN.md line 101. See Deferred Items below. |
| 3 | For a free-field configuration, the engine returns a complex transfer value per 1/12-octave frequency point (25 Hz–10 kHz, f64 throughout) whose magnitude matches spherical divergence + ISO 9613-1 air absorption within the standard's tolerance | ✓ VERIFIED | `crates/envi-engine/src/propagation/divergence.rs::divergence_db` reproduces all 4 Eq. 330 anchors exactly (`-10.992099`/`-30.992099`/`-50.992099`/`-70.992099` dB at R=1/10/100/1000 m, epsilon 1e-6 — ran green). `air_absorption.rs::alpha_db_per_m` pinned at FORCE conditions (h=1.177222%, f_rO=36332.37 Hz, f_rN=333.6691 Hz, `intermediates_pin_the_transcription_at_force_conditions` — ran green) and cross-checked against published ISO 9613-2 Table 2 (5.0/22.9/76.6 dB/km at 1/4/8 kHz within 2%, `published_iso_9613_2_table_2_cross_check_20c_70rh` — ran green) plus 6 regression anchors at exact grid centres (1e-6 relative, `alpha_regression_anchors_at_exact_grid_centres_force_conditions` — ran green). `propagation/mod.rs::direct_path` assembles `Complex::from_polar(amp, phase)` per of 105 points; `magnitude_identity_matches_divergence_plus_band_absorption` (ran green) confirms `20·log10|H|` = divergence − band-corrected absorption to 1e-9 dB. `freefield_100m` + `freefield_spectrum` TOML cases both report **Pass** end-to-end via `cargo run -p envi-harness -- report` (observed live). Output is genuinely complex, not a real-scalar shortcut: `output_is_genuinely_complex_nonzero_imaginary` and `half_wavelength_paths_cancel_proving_the_phase_is_live` (`|sum| < 1e-10` for two λ/2-separated phasors) both ran green — phase is live, not a placeholder. |
| 4 | A point sub-source carries a per-1/12-octave source spectrum, and receiver band levels computed from it through the transfer values reproduce the expected free-field levels | ✓ VERIFIED | `scene.rs::BandSpectrum` is a fixed-length `[f64; N_BANDS]` wrapper (`uniform`/`from_values` constructors). `cases::SourceSpectrum::{Unit,Uniform,Ramp}` (cases/toml.rs) materializes into `BandSpectrum` via `scene_build::band_spectrum`. `cases/freefield_spectrum.toml` sets a genuinely non-uniform ramp `L_W(i) = 80 + 0.1·i` dB; `transfer.rs::band_levels_db` computes `L_p = L_W + 20·log10|H|` per band. `run_freefield_case` (lib.rs) drives Scene → `direct_path` → `band_levels_db` and compares against `compare::analytic_freefield_reference` (an independently coded dB-domain oracle, not reusing the complex roundtrip) at 1e-9 dB over all 105 points — case reports **Pass** (`cargo run -p envi-harness -- report`, confirmed live). |

**Score:** 4/4 ROADMAP success criteria met (truth 2 carries one explicitly deferred sub-item, tracked below — not counted as a gap per Step 9b).

### Deferred Items

| # | Item | Addressed In | Evidence |
|---|------|-------------|----------|
| 1 | Screen edges parsed from real FORCE `.xls` data into a populated `Barrier` (currently `Barrier` is a semantic type only, exercised with synthetic test data) | Phase 2 | Phase 2 goal: "Homogeneous-atmosphere FORCE cases with ground and screens pass ... single/multi-edge diffraction"; Phase 2 success criterion 2: "FORCE cases with single-edge and multiple-edge screens/barriers match reference within tolerance" (requirement ENG-03). `01-RESEARCH.md` line 362 and `01-02-PLAN.md` line 101 both document this as an explicit Phase 1→2 scope boundary (AV 1106/07 §5.1: screens are merged into the terrain profile, an algorithm scoped to Phase 2). |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| VAL-01 | 01-01 | Stand up a test harness that loads and runs the FORCE road-traffic test cases (built before propagation code) | ✓ SATISFIED | See Truth 1. `implemented_capabilities()` empty-then-partial gating proves the harness predates physics; all 62+3 FORCE workbook cases Skip with named reasons, never false-Pass. |
| GEO-01 | 01-02 | Represent a canonical semantic 2.5D scene (Source, Receiver, Barrier, Building, TerrainProfile) in a projected metric CRS, Z-up | ✓ SATISFIED | `scene.rs` defines all five types plus `GroundSegment`/`CrsInfo`/`SceneError`; Z-up + metric CRS documented and used consistently (source/receiver `[x,y,z]`, terrain `(x,z)`). |
| GEO-02 | 01-02 | Consume a source→receiver terrain profile + ground-impedance segments + screen edges from FORCE test-case files | ⚠ SATISFIED for terrain profile + ground-impedance segments; screen-edge consumption deferred to Phase 2 (see Deferred Items) | Terrain profile + segments: `scene_build.rs::build_force_straight_road`, live-tested against `TestStraightRoad.xls` case 1 (97.5 m anchor, mixed A/G impedance). Screen edges: not parsed from any FORCE file in Phase 1 — explicitly scoped to Phase 2. |
| GEO-03 | 01-02 | Compute source→receiver azimuth and reflection-path geometry | ✓ SATISFIED | `geometry.rs::azimuth_deg` + `reflect_over_segment`, all hand-computed anchors pass (see Truth 2). |
| ENG-01 | 01-03 | Compute direct-path attenuation (geometrical divergence) per 1/12-octave frequency point | ✓ SATISFIED | `divergence.rs::divergence_db`, Eq. 330 anchors exact to 1e-6 (see Truth 3). |
| ENG-04 | 01-03 | Compute air absorption per ISO 9613-1 from temperature, humidity, pressure | ✓ SATISFIED | `air_absorption.rs`, three-stage pinning + ISO 9613-2 Table 2 cross-check (see Truth 3). |
| SRC-01 | 01-03 | Define a point sub-source with per-1/12-octave sound power / source spectrum | ✓ SATISFIED | `SubSource`/`BandSpectrum`/`SourceSpectrum::Ramp`, end-to-end `freefield_spectrum` Pass (see Truth 4). |

No orphaned requirements: all 7 requirement IDs mapped to Phase 1 in REQUIREMENTS.md are claimed by a plan (01-01/01-02/01-03) and verified above.

### Complex-Output Contract (explicitly requested cross-cutting check)

| Check | Evidence | Status |
|---|---|---|
| Output is genuinely `Complex<f64>` per (sub-source × receiver × 1/12-oct freq) | `transfer.rs::TransferSpectrum = Vec<Complex<f64>>` (len 105); `direct_path` returns it via `Complex::from_polar` | ✓ VERIFIED |
| Phase is LIVE, not a placeholder zero-imaginary shortcut | `propagation::tests::output_is_genuinely_complex_nonzero_imaginary` — asserts `im.abs() > 1e-12`; `propagation::tests::half_wavelength_paths_cancel_proving_the_phase_is_live` — two λ/2-separated unit phasors sum to `< 1e-10` magnitude. Both ran green in this session. | ✓ VERIFIED |
| `TransferTensor = Array3<Complex<f64>>` `[sub_source, receiver, freq]` alias exists as the Phase-4 forward contract | `transfer.rs` line 49: `pub type TransferTensor = Array3<Complex<f64>>;` documented row-major/frequency-contiguous; `transfer::tests::transfer_tensor_is_row_major_frequency_contiguous` asserts `t.is_standard_layout()` on a `(2,3,N_BANDS)` tensor — ran green. | ✓ VERIFIED |

### Build / Quality Gates (actually executed this session)

| Gate | Command | Result |
|---|---|---|
| Workspace tests | `cargo test --workspace` | **62 unit tests pass** (31 envi-engine + 31 envi-harness); `force` target **5 passed, 65 ignored, 0 failed**; 0 doc-tests (none written); overall 0 failed |
| Live FORCE case 1 parse | `cargo test -p envi-harness force_case_1_applies_lane_and_height_conventions` | 1 passed (real `TestStraightRoad.xls` present under `refs/`, 97.5 m anchor confirmed) |
| Clippy | `cargo clippy --all-targets -- -D warnings` | Clean, zero warnings |
| Formatting | `cargo fmt --check` | Clean, exit 0 |
| Build | `cargo build --workspace` | Finished, no errors |
| I/O quarantine | `cargo tree -p envi-engine -e normal --depth 1` | Only `ndarray`, `num-complex`, `thiserror` — engine crate carries zero file-format/I/O dependencies, confirming "harness before propagation" is an architectural property, not just a test-ordering convention |
| Harness report (manual run) | `cargo run -p envi-harness -- report` | 4 Pass (`freefield_100m`, `freefield_spectrum`, `geometry_azimuth`, `geometry_reflection`), 65 Skipped (all FORCE road/curved/city/yearly cases) with named `requires:` lists — no false Pass anywhere |

### Anti-Patterns Found

None blocking. Grep for `TBD|FIXME|XXX|TODO|HACK|PLACEHOLDER|placeholder|not yet implemented` across `crates/` turned up only documented, intentional deferrals with explicit rationale and a named consuming phase (e.g. `FORCE_PLACEHOLDER_SOURCE_H_M` — Phase 4 replaces the road-emission sub-source heights; `CaseKind::ForceCurvedRoad`/`ForceCityStreet` doc comments — "placeholder cases", loaded for real in Phases 3-4). No unreferenced debt markers. No stub `return null`/`return {}`/empty-handler patterns found in non-test code.

### Human Verification Required

None. All ROADMAP success criteria and requirements are verifiable by direct code inspection and test execution; no visual, real-time, or external-service behavior is in scope for this phase.

### Gaps Summary

No gaps found. One item (screen-edge parsing from real FORCE `.xls` data into `Barrier`) is present as a type/API but not yet exercised against real screen-bearing FORCE cases — this is explicitly and specifically deferred to Phase 2 by the project's own RESEARCH/PLAN documents and covered by Phase 2's stated success criteria (ENG-03, screens/barriers). It does not block Phase 1's goal: the "harness before propagation" invariant, the free-field complex direct path, and the geometry primitives are all genuinely implemented, tested against hand-computed and standard-published anchors, and verified live in this session (not merely claimed in SUMMARY.md).

## Final Verdict

**PASSED.** Phase 1's goal — a FORCE-driven harness and semantic scene model standing before any propagation code, plus a genuinely complex-valued divergence + ISO 9613-1 direct path validated end-to-end — is achieved in the shipped code, not just claimed in the summaries. All quality gates (build, test, clippy, fmt) are green as of this verification run. Ready to proceed to Phase 2.

---
_Verified: 2026-07-07T18:44:00Z_
_Verifier: Claude (gsd-verifier)_
