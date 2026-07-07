---
phase: 01-force-harness-geometry-model-direct-path
plan: 02
subsystem: engine
tags: [rust, nord2000, scene-model, geometry, azimuth, image-source-reflection, force-conventions, 2.5d, thiserror]

# Dependency graph
requires:
  - "01-01: envi-engine/envi-harness workspace, freq::N_BANDS, CaseDefinition/CaseKind/PropagationParams/TerrainRow, run_case dispatch, Capability enum, Outcome"
provides:
  - "envi-engine::scene — canonical semantic 2.5D vocabulary: Scene, CrsInfo, Source{sub_sources}, SubSource{position,spectrum}, BandSpectrum (len N_BANDS), Receiver, Barrier::edges, Building, TerrainProfile::new/points/segments/endpoints, GroundSegment, impedance_class(char), SceneError (projected metric CRS, Z-up)"
  - "envi-engine::geometry — azimuth_deg (clockwise from north), PathGeometry::direct (R, horizontal, azimuth; DegeneratePath guard), ReflectionGeometry + reflect_over_segment (general line reflection, grazing angle, within-segment validity), GeometryError"
  - "envi-harness::scene_build — build_scene(&CaseDefinition) with FORCE lane (x=2.5) / hSv-hRv conventions and the 97.5 m anchor; row-starts-segment impedance assignment"
  - "envi-harness — Capability::Geometry implemented; run_case Geometry arm; Outcome::FailDetail(String); [expected.geometry] TOML schema (GeometryExpected)"
affects: [01-03-direct-path, phase-02-ground-diffraction, phase-03-refraction, phase-04-tensor-emission]

# Tech tracking
tech-stack:
  added: []
  patterns: [semantic 2.5D scene (projected metric CRS Z-up), naming-discipline over units-type-system (_m/_db/_hz), typed domain errors never panic on data, hSv/hRv encoded in one function (endpoints), image-source reflection via general line reflection, capability gating (Geometry flips green), authoritative .xls over planned assumptions]

key-files:
  created:
    - crates/envi-engine/src/scene.rs
    - crates/envi-engine/src/geometry.rs
    - crates/envi-harness/src/scene_build.rs
    - cases/geometry_azimuth.toml
    - cases/geometry_reflection.toml
  modified:
    - crates/envi-engine/src/lib.rs
    - crates/envi-harness/src/lib.rs
    - crates/envi-harness/src/capability.rs
    - crates/envi-harness/src/cases/mod.rs
    - crates/envi-harness/src/cases/toml.rs
    - crates/envi-harness/src/main.rs
    - crates/envi-harness/tests/force.rs

key-decisions:
  - "Row->segment impedance assignment: each of the N-1 ground segments takes the flow resistivity/roughness of the row that STARTS it (windows(2).map(w[0])). Verified against the MIXED-impedance case 1 (road sigma=20000 at x=3.25, grass sigma=12.5 at x=5) — the plan's 'case 1 is all class A' assumption was wrong; authoritative .xls wins (Pitfall 1)"
  - "FORCE source X is the lane line at 2.5 m (NOT the first profile point at 3.25 m); receiver X is the last profile point (100 m) => horizontal distance 97.5 m, not 100 m. hSv/hRv (Z above first/last point) encoded in TerrainProfile::endpoints"
  - "Placeholder sub-source for FORCE cases: a single SubSource at the source endpoint with BandSpectrum::uniform(0.0) and placeholder height 0.0 above the first profile point; the real Nord2000 road sub-source heights (0.01/0.30/0.75 m) belong to the Phase 4 emission model"
  - "reflect_over_segment API: Some(ReflectionGeometry) with a `valid` flag for within/outside-segment (never extrapolated); None only when geometrically undefined (degenerate segment or parallel intersection)"
  - "Added Outcome::FailDetail(String) for non-spectrum (geometry) failures — the 27-band ComparisonReport does not fit geometry anchor mismatches"

requirements-completed: [GEO-01, GEO-02, GEO-03]

# Metrics
duration: 17min
completed: 2026-07-07
status: complete
---

# Phase 1 Plan 02: Semantic 2.5D Scene Model + Path Geometry Summary

**The engine's canonical vocabulary is frozen: the semantic 2.5D scene model (Source/SubSource/Receiver/Barrier/Building/TerrainProfile, projected metric CRS, Z-up), the FORCE-case-to-Scene conversion with the lane and hSv/hRv conventions (the 97.5 m anchor, not naive 100 m), and pure path geometry — azimuth + image-source reflection — validated end-to-end through the harness, flipping the first capability green.**

## Performance

- **Duration:** ~17 min
- **Completed:** 2026-07-07
- **Tasks:** 3 (all TDD: RED test commit then GREEN feat commit)

## Accomplishments

- **Semantic 2.5D scene vocabulary (GEO-01).** `envi-engine::scene` defines Scene, CrsInfo, Source/SubSource, BandSpectrum (fixed length `freq::N_BANDS` = 105), Receiver, Barrier (with `edges()`), Building, TerrainProfile and GroundSegment. `TerrainProfile::new` validates the AV 1106/07 §5.3.1 contract (non-empty, strictly ascending X, N points → N−1 segments, all-finite) with typed `SceneError` — never panics on data (threat T-01-05). `TerrainProfile::endpoints` is the single home of the hSv/hRv convention (source Z above the FIRST profile point, receiver Z above the LAST). `impedance_class` maps Nordtest A..H with A/D/G verified and B/C/E/F/H flagged ASSUMED (A1).
- **Path geometry primitives (GEO-03).** `azimuth_deg` = `atan2(dx_east, dy_north)` clockwise from north, normalized `[0,360)` (anchors 0/90/45/270). `PathGeometry::direct` returns 3-D range, horizontal distance, azimuth and guards `R → 0` with a typed `DegeneratePath` error. `reflect_over_segment` does general line reflection (handles sloped segments, not z-negation), reports the reflection point, `r1 + r2`, grazing angle, and a within-segment validity flag; hand-computed flat-ground (x=5, √116) and sloped-segment (x=5, √104, atan(2/3)) anchors pass at 1e-12.
- **FORCE → Scene conversion + the 97.5 m anchor (GEO-02).** `build_scene` applies the FORCE lane convention (source line at x=2.5 m), hSv/hRv heights via `endpoints`, and the row-starts-segment impedance rule. Case 1's horizontal source→receiver distance is **97.5 m, not 100 m** — the phase's biggest off-by-metres trap, now a hard-failing test.
- **First capability goes green.** `Capability::Geometry` is now implemented; the `run_case` Geometry arm builds the Scene, computes azimuth / direct-path / reflection, and compares to the `[expected.geometry]` anchors. `cases/geometry_azimuth.toml` and `cases/geometry_reflection.toml` **Pass** end-to-end (file → parse → Scene → engine geometry → comparison → report); the free-field case is still `Skipped [requires: free-field]`.
- **All quality gates pass:** `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `#![deny(unsafe_code)]` on envi-engine, and the engine dependency quarantine (only ndarray/num-complex/thiserror — no I/O crept in).

## Task Commits

1. **Task 1: semantic 2.5D scene types** — RED `test(01-02)` (behavior tests, todo bodies) → GREEN `feat(01-02)` (TerrainProfile::new/endpoints, impedance_class, all scene types)
2. **Task 2: path geometry primitives** — RED `test(01-02)` (azimuth/direct/reflection anchors) → GREEN `feat(01-02)` (azimuth_deg, PathGeometry::direct, reflect_over_segment)
3. **Task 3: FORCE→Scene + geometry cases green** — RED `test(01-02)` (build_scene conversion tests + geometry-expected scaffolding) → GREEN `feat(01-02)` (build_scene, TOML schema, capability wire, run_case arm, fixtures)

## Public API (for plan 01-03 and Phases 2–4)

**`envi_engine::scene`**
- `struct Scene { crs: CrsInfo, sources: Vec<Source>, receivers: Vec<Receiver>, barriers: Vec<Barrier>, buildings: Vec<Building>, terrain: Vec<TerrainProfile> }`
- `struct CrsInfo { label: String }`, `CrsInfo::local_metric()`
- `struct Source { sub_sources: Vec<SubSource> }`
- `struct SubSource { position: [f64;3], spectrum: BandSpectrum }`
- `struct BandSpectrum` (private `[f64; N_BANDS]`); `BandSpectrum::uniform(db) / from_values([f64;N_BANDS]) / as_slice() -> &[f64]`
- `struct Receiver { position: [f64;3] }`
- `struct Barrier { top_edge: Vec<[f64;3]>, thickness_m: Option<f64> }`; `Barrier::edges() -> Vec<([f64;3],[f64;3])>`
- `struct Building { footprint: Vec<[f64;2]>, eaves_height_m: f64 }`
- `struct TerrainProfile` (private points/segments); `TerrainProfile::new(Vec<[f64;2]>, Vec<GroundSegment>) -> Result<Self, SceneError>`, `.points() -> &[[f64;2]]`, `.segments() -> &[GroundSegment]`, `.endpoints(h_s, h_r) -> ([f64;2],[f64;2])`
- `struct GroundSegment { flow_resistivity: f64, roughness: f64 }`
- `fn impedance_class(char) -> Option<f64>`
- `enum SceneError { EmptyProfile, NonAscendingX{..}, SegmentCountMismatch{..}, NonFinite{..} }`

**`envi_engine::geometry`**
- `fn azimuth_deg(from: [f64;2], to: [f64;2]) -> f64` (clockwise from north, `[0,360)`)
- `struct PathGeometry { r_m, horizontal_m, azimuth_deg }`; `PathGeometry::direct(src: [f64;3], rcv: [f64;3]) -> Result<PathGeometry, GeometryError>`
- `struct ReflectionGeometry { point_x, r1_m, r2_m, grazing_angle_rad, valid }`
- `fn reflect_over_segment(s: [f64;2], r: [f64;2], seg_a: [f64;2], seg_b: [f64;2]) -> Option<ReflectionGeometry>`
- `enum GeometryError { DegeneratePath { r_m: f64 } }`

**`envi_harness`**
- `scene_build::build_scene(&CaseDefinition) -> anyhow::Result<Scene>` (FORCE lane/height conventions; synthetic positions verbatim)
- `cases::GeometryExpected { azimuth_deg, reflection_x, path_length_m, reflection_segment, tolerance }`; `SyntheticExpected.geometry: Option<GeometryExpected>`
- `capability::implemented_capabilities()` now `{ Geometry }`
- `Outcome::FailDetail(String)` — non-spectrum (geometry / scene-build) failures naming the offending quantity

## Recorded decisions (per plan `<output>`)

- **Row→segment impedance assignment:** each segment takes the flow resistivity/roughness of the row that STARTS it. This became observable on case 1 (mixed road+grass) and is verified there.
- **Placeholder sub-source for FORCE cases:** a single `SubSource` at the source endpoint, `BandSpectrum::uniform(0.0)`, placeholder height 0.0 above the first profile point. The real road sub-source heights (0.01/0.30/0.75 m) are Phase 4 (emission model).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Plan must_have "every GroundSegment.flow_resistivity == 12.5" contradicted the authoritative .xls**
- **Found during:** Task 3 GREEN (running `cargo test --workspace`; the case-1 scene test failed).
- **Issue:** The plan (and 01-RESEARCH) assumed FORCE case 1 is uniformly impedance class A. The authoritative `TestStraightRoad.xls` case-1 profile is actually MIXED: point x=3.25 carries σ=20000 (road pavement, class G), x=5 carries σ=12.5 (grass, class A), x=100 is the σ=0 terminator. So `build_scene` produces two segments — road (20000) then grass (12.5) — not "all 12.5".
- **Fix:** Corrected the case-1 test to assert the true mixed values (segment 0 = 20000, segment 1 = 12.5), and updated the `build_scene` inline comment. The `build_scene` logic (row-starts-segment) was correct and needed no change — the discovery actually makes the row→segment rule OBSERVABLE and verified, resolving the plan's flagged "re-verify on a mixed-impedance case in Phase 2" concern early. Authoritative .xls over planned assumption (01-RESEARCH Pitfall 1).
- **Files modified:** crates/envi-harness/src/scene_build.rs (test + comment)
- **Commit:** Task 3 GREEN feat commit.

**2. [Rule 3 - Blocking] `Outcome::Fail` (27-band ComparisonReport) does not fit geometry failures**
- **Found during:** Task 3 GREEN (wiring the run_case Geometry arm).
- **Issue:** `Outcome::Fail(ComparisonReport)` is spectrum-oriented; a geometry anchor mismatch (azimuth / reflection_x / path length) has no 27-band report to attach.
- **Fix:** Added `Outcome::FailDetail(String)` naming the offending quantity; handled in `main.rs` (report table) and `tests/force.rs` (trial error). The plan explicitly wanted "Fail with a small report naming the offending quantity" — this is the minimal clean realization.
- **Files modified:** crates/envi-harness/src/lib.rs, main.rs, tests/force.rs
- **Commit:** Task 3 GREEN feat commit.

## Known Stubs

- **Placeholder FORCE sub-source** (single sub-source, uniform-0 spectrum, height 0.0): intentional — the Nord2000 road emission model (sub-source heights 0.01/0.30/0.75 m, Jonasson tables) is Phase 4. Does not block the plan goal (geometry conversion + anchors).
- **Barrier merge into terrain** is not implemented: `Barrier` is a semantic object with `edges()` only; the screens-into-profile merge is Phase 2 per AV 1106/07 §5.1 (as scoped by the plan).
- **BandSpectrum L_W values** are not yet consumed by physics — plan 01-03 (direct path / SRC-01) uses them.

## Threat surface

No new network endpoints, auth paths, file access, or schema changes at trust boundaries beyond the planned CaseDefinition → scene_build boundary, which is mitigated exactly as the threat register requires (T-01-05 typed SceneError, T-01-06 degenerate/out-of-segment guards, T-01-07 the 97.5 m anchor + one-function hSv/hRv). No new dependencies (T-01-SC).

## Verification Evidence

- `cargo build --workspace` — finished, no errors
- `cargo test --workspace` — 16 (engine) + 26 (harness unit) pass; force target: 3 passed (discovery meta + 2 geometry cases), 66 ignored; 0 failed
- `cargo test -p envi-harness --test force geometry` — 2 passed (geometry_azimuth, geometry_reflection), 0 failed
- `cargo clippy --all-targets -- -D warnings` — zero warnings
- `cargo fmt --check` — clean (exit 0)
- `cargo tree -p envi-engine -e normal --depth 1` — only ndarray, num-complex, thiserror (I/O quarantine holds)
- `cargo run -p envi-harness -- report` — geometry_azimuth / geometry_reflection = Pass; freefield_100m = Skipped (requires: free-field)

## Self-Check: PASSED

All created files exist on disk; all six task commits (3 RED + 3 GREEN) are present in git history.
