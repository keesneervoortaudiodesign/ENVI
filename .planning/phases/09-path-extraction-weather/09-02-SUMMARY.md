---
phase: 09-path-extraction-weather
plan: 02
subsystem: gis
tags: [rust, geox, grid, screening, rstar, spade, geo, terrain-profile, nord2000, wasm-safe]

# Dependency graph
requires:
  - phase: 09-path-extraction-weather
    provides: "09-01 GEOX-01/02 — envi_gis::profile::cut_profile + impedance::{segment_ground, GroundSegmentation} (the boundary-spliced (x,z)+planar cut plane this plan injects screen tops into)"
  - phase: 08-gis-ingestion-dgm
    provides: "envi-dgm build_tin (spade CDT + can_add_constraint guard) + Tin::interpolate_z (None outside hull, never silent 0.0)"
  - phase: 02-ground-effect
    provides: "envi_engine::scene TerrainProfile / GroundSegment / impedance_class (class H = 200000 hard ground; single source of truth for σ)"
provides:
  - "GEOX-03: envi_gis::screening::inject_screens — rstar corridor + cut-plane∩footprint-prism → screen tops injected as (x,z) vertices into the SAME TerrainProfile (no separate screen list); through-building → two tops (thick screen) with a hard-σ span"
  - "envi_gis::screening::ScreenObject — building (footprint + eaves_height_m) / barrier (linestring + height_m) vocabulary; CORRIDOR_MIN_HALF_WIDTH_M / CORRIDOR_REF_FREQ_HZ / MAX_CORRIDOR_CANDIDATES named [ASSUMED] consts + fresnel_half_width"
  - "GRID-01: envi_gis::grid::receiver_grid — building-aware regular lattice clipped to calc_area minus footprints (D-07), discrete points, z from the DGM TIN; MIN_SPACING_M / MAX_RECEIVERS guardrails"
  - "New GisError variants: CorridorCandidatesExceeded, SpacingTooSmall, ReceiverCapExceeded, InvalidGridRegion"
  - "rstar 0.13 added to envi-gis (pure-Rust, WASM-safe — the only new dep this phase)"
affects: [09-04-path-assembly, 10-solve-tensor, 11-results-isophones]

# Tech tracking
tech-stack:
  added: ["rstar 0.13 (envi-gis runtime dep — R*-tree corridor query, georust family, WASM-safe)"]
  patterns:
    - "Screen tops injected as (x,z) profile vertices (z = ground_z + object_height), never a separate screens Vec — the engine SolveJob has no screens field; terrain_interpretation derives ≤2 diffracting edges FROM the profile"
    - "rstar corridor pre-filter (max(20 m, first-Fresnel@250 Hz half-width)) with a MAX_CORRIDOR_CANDIDATES DoS cap before any per-candidate cut-plane∩prism work"
    - "Building-aware receiver grid: regular lattice clipped to calc_area minus footprints via geo point-in-polygon, with footprint rings validated as spade-CDT constraints (build_tin can_add_constraint guard) to reject intersecting/degenerate geometry"
    - "Guardrails as named [ASSUMED] pub consts checked before allocation → typed GisError, never OOM/panic (mirrors envi_dgm::MAX_POINTS posture)"

key-files:
  created:
    - crates/envi-gis/src/screening.rs
    - crates/envi-gis/src/grid.rs
  modified:
    - crates/envi-gis/src/lib.rs
    - crates/envi-gis/Cargo.toml
    - Cargo.lock

key-decisions:
  - "GEOX-03 injects screen tops into the TerrainProfile as (x,z) vertices (Pitfall 1 reshape) — inject_screens consumes the base GroundSegmentation and returns an augmented one, so non-screen intervals keep their GEOX-02 segment and screen spans become hard σ (class H)"
  - "A wall/barrier is a thin screen (one top vertex, no hard span); a building the line passes through is a thick screen (two tops + hard σ span). Multi-edge is preserved (all crossings inserted) but Nord2000 caps diffraction at ≤2 screens — documented, not an N-edge combiner (D-09, Pitfall 4)"
  - "GRID-01 emits a REGULAR axis-aligned lattice clipped to the CDT-defined region (09-RESEARCH Open-Q3 confirmed), not literal CDT vertices — predictable spacing for noise maps / Phase-10 chunking; build_tin is reused as a geometry-validity guard, its TIN discarded (receiver z comes from the passed-in DGM TIN)"
  - "Corridor half-width = max(CORRIDOR_MIN_HALF_WIDTH_M=20 m, first-Fresnel@250 Hz); MIN_SPACING_M=1.0 m, MAX_RECEIVERS=1_000_000, MAX_CORRIDOR_CANDIDATES=100_000 — all named [ASSUMED] consts with rationale (09-RESEARCH A1/A2)"

patterns-established:
  - "inject_screens composes AFTER segment_ground on the same (x,z)+planar frame, so GEOX-02 impedance and GEOX-03 screening share one profile and one splice epsilon (X_EPSILON_M = 1e-6)"
  - "R*-tree entries key only an AABB + index back into the caller's slice (geometry not duplicated into the tree); AABB from geo::BoundingRect"
  - "build_tin as a pure validity gate: ring vertices as z=0 points + rings as breaklines → IntersectingConstraint/degenerate → typed InvalidGridRegion, never a panic"

requirements-completed: [GEOX-03, GRID-01]

# Metrics
duration: 40min
completed: 2026-07-11
status: complete
---

# Phase 9 Plan 02: Screening edges (GEOX-03) + building-aware receiver grid (GRID-01) Summary

**The geometry half of the GIS→engine pipeline as pure-Rust, WASM-safe `envi-gis` modules: an rstar-corridor screening reducer that injects cut-plane∩footprint-prism tops as `(x,z)` vertices into the SAME `TerrainProfile` (no separate screen list — the engine has no `screens` field), and a building-aware constrained-Delaunay receiver grid that clips a regular lattice to `calc_area` minus footprints with min-spacing/count DoS guardrails.**

## Performance

- **Duration:** ~40 min
- **Completed:** 2026-07-11
- **Tasks:** 2 (both `type=auto tdd=true`)
- **Files modified:** 5 (2 created, 3 modified)

## Accomplishments
- **GEOX-03** `screening::inject_screens(&GroundSegmentation, &[ScreenObject], &Tin) -> Result<GroundSegmentation, GisError>`: builds an `rstar` R*-tree over building/wall/barrier AABBs, queries the S→R corridor at half-width `max(CORRIDOR_MIN_HALF_WIDTH_M=20 m, first-Fresnel@250 Hz)`, and (capped at `MAX_CORRIDOR_CANDIDATES`) intersects the cut plane with each candidate's footprint prism. Every crossing splices an `(x, z)` vertex at `z = ground_z(crossing) + object_height` (`eaves_height_m` for buildings, `height_m` for walls/barriers) **into the same profile** — there is no separate screen `Vec`. A building the line passes through yields **two** top vertices (thick screen) and the span between them becomes **hard** ground (class H, roughness 0); non-screen intervals inherit their GEOX-02 segment. Hull misses are `GisError::OutsideHull`, never a fabricated `0.0`.
- **GRID-01** `grid::receiver_grid(calc_area, footprints, spacing_m, discrete, tin) -> Result<Vec<[f64;3]>, GisError>`: generates the `calc_area` bbox lattice at `spacing_m` (default 10 m at the call site, D-06), keeps each point inside `calc_area` **and** outside every footprint (D-07, geo point-in-polygon), appends discrete points, and samples each z from the DGM TIN (ground only — receiver acoustic height is added at `SolveJob` assembly, the hSv/hRv trap). Footprint rings + `calc_area` boundary are validated as spade-CDT constraints via `envi_dgm::build_tin` (reusing its `can_add_constraint` guard) → `InvalidGridRegion` on intersecting/degenerate geometry, never a panic. Guardrails: `MIN_SPACING_M=1.0` → `SpacingTooSmall`; `MAX_RECEIVERS=1_000_000` checked against the bbox lattice count **before** allocation → `ReceiverCapExceeded`.
- Added `rstar = "0.13"` (pure-Rust, georust family) as the **only** new `envi-gis` dependency; the quarantine holds: `envi-engine` still exactly 3 deps, `envi-gis` gains no async/network/browser edge.

## Task Commits

Each task was committed atomically:

1. **Task 1: Screening edges → profile vertices (GEOX-03, rstar corridor)** — `03dbee0` (feat)
2. **Task 2: Building-aware receiver grid (GRID-01, spade CDT + lattice clip)** — `326d7c8` (feat)

**Plan metadata:** (this docs commit)

_Both tasks are `tdd=true`; each was implemented with its unit/property tests and committed atomically (single feat commit per task — TDD_MODE inactive this phase, matching the 09-01 precedent)._

## Files Created/Modified
- `crates/envi-gis/src/screening.rs` — GEOX-03 rstar corridor + cut-plane∩prism → profile-vertex injection (`ScreenObject`, `inject_screens`, `fresnel_half_width`, corridor consts) + 8 unit tests
- `crates/envi-gis/src/grid.rs` — GRID-01 building-aware receiver grid (`receiver_grid`, `MIN_SPACING_M`, `MAX_RECEIVERS`, `validate_region`) + 9 unit tests
- `crates/envi-gis/src/lib.rs` — `pub mod screening; pub mod grid;` + new `GisError` variants (`CorridorCandidatesExceeded`, `SpacingTooSmall`, `ReceiverCapExceeded`, `InvalidGridRegion`)
- `crates/envi-gis/Cargo.toml` — `rstar = "0.13"` dependency + boundary comment
- `Cargo.lock` — rstar 0.13 → envi-gis edge

## Decisions Made
- **`inject_screens` signature (`&GroundSegmentation → GroundSegmentation`)**: the plan sketched "return the augmented points + segments"; consuming and returning a `GroundSegmentation` lets GEOX-03 compose cleanly after GEOX-02 on the same `(x,z)`+`planar` frame — screen spans override to hard σ while every other interval keeps its impedance segment. This is the only way to satisfy "no separate screen Vec" AND "hard-σ over objects" AND "non-screen impedance preserved" observably.
- **Thin barrier vs thick building**: a wall/barrier crossing injects one top vertex and introduces no hard span (a thin screen spans no width); a through-building injects two tops (entry+exit) with a hard-σ span between them. Crossing x's are paired (`chunks_exact(2)`) into hard spans so concave footprints stay correct.
- **Regular lattice over literal CDT vertices** (09-RESEARCH Open-Q3, confirmed): `receiver_grid` clips a predictable axis-aligned lattice to the CDT-defined valid region rather than emitting irregular triangle vertices — what noise maps and Phase-10 chunking expect. `build_tin` is used purely as the intersecting/degenerate-ring validity gate; its output TIN is discarded and receiver z is sampled from the passed-in DGM TIN.
- **Guardrail values** ([ASSUMED], 09-RESEARCH A1/A2, named consts): corridor `max(20 m, first-Fresnel@250 Hz)`, `MAX_CORRIDOR_CANDIDATES=100_000`, `MIN_SPACING_M=1.0`, `MAX_RECEIVERS=1_000_000` — engineering DoS bounds, not spec values; Phase-10 cost estimation is the real UX gate.

## Deviations from Plan

None — plan executed as written. Two minor signature/refinement notes (not scope changes):

- `inject_screens` takes/returns `GroundSegmentation` rather than a bare "points + segments" pair — the same refinement 09-01 made for `segment_ground` (the spliced point list must stay in sync with segments for `TerrainProfile::new`), documented above.
- The screening `ScreenObject` unifies `wall` and `barrier` into a single `Barrier { line, height_m }` variant (both are linestrings screening at their top height, D-08); buildings are the `Building { footprint, eaves_height_m }` variant. No behavioural difference from the plan's "building/wall/barrier all screen".

All prohibitions honored: no separate screen `Vec` to the solver (injected into `TerrainProfile`); no bespoke N-edge combiner (all crossings inserted, engine caps at ≤2); no receiver generated inside a footprint (D-07); only `rstar` added, `envi-engine` untouched (3-dep quarantine byte-identical); injected screen z = `ground_z + object_height` (never collapsed to source/receiver height).

## Issues Encountered
- **Windows file lock on `envi-service.exe`** during the full `cargo test --workspace`: a user-launched server holds the running binary, so cargo cannot replace its test executable (`Access is denied (os error 5)`). Per project hygiene rules the process was **not** killed. Verified green via `cargo test --workspace --exclude envi-service` (**521 passed, 0 failed**) plus `cargo clippy --all-targets -- -D warnings` (which compiled `envi-service` clean) and `cargo fmt --check`. `envi-service` does not consume the new `screening`/`grid` modules, so it is unaffected.
- Minor clippy nit fixed inline: `!(spacing_m >= MIN_SPACING_M)` (negated partial-ord) rewritten as `!spacing_m.is_finite() || spacing_m < MIN_SPACING_M`.

## Quality Gates
- `cargo test -p envi-gis`: screening (8) + grid (9) new unit tests + all prior envi-gis tests green (80 lib + 8 + 1 integration).
- `cargo clippy --all-targets -- -D warnings`: clean (whole workspace, incl. envi-service).
- `cargo fmt --check`: clean.
- `cargo test --workspace --exclude envi-service`: **521 passed, 0 failed** (envi-service excluded only by the running-binary file lock; it compiled clean under clippy).
- `cargo tree -p envi-engine`: exactly `ndarray + num-complex + thiserror` (quarantine byte-identical).
- `cargo tree -p envi-gis`: `rstar 0.13` present; **no** HTTP client / async runtime / browser crate (grep of tokio/reqwest/hyper/async/web-sys/wasm-bindgen/futures/mio/http → none).

## Next Phase Readiness
- The four geometry links of the pipeline are now complete: GEOX-01 `cut_profile` → GEOX-02 `segment_ground` → GEOX-03 `inject_screens` all compose on one `(x,z)`+`planar` `GroundSegmentation`, and GRID-01 `receiver_grid` produces the receiver positions a calculation runs over. 09-04 path assembly can `TerrainProfile::new(seg.points, seg.segments)` from the screened segmentation and pair each `receiver_grid` position with the source to build `SolveJob` inputs.
- **Integration seam still open (09-04 / phase-close):** an end-to-end test feeding a screened profile to `terrain_interpretation` to assert the expected Sub-model 4/5/6 classification is planned for a later plan or the phase-close harness check — this plan's tests verify the injection geometry (two tops, hard span, valid `TerrainProfile`), not the engine's diffraction classification of it.
- The weather half (METX-01/02, 09-05/09-06) is independent and unaffected.

## Self-Check: PASSED

All claimed artifacts verified on disk (`crates/envi-gis/src/screening.rs`, `crates/envi-gis/src/grid.rs`) and both task commits (`03dbee0`, `326d7c8`) present in git history; the new modules + `GisError` variants compile and all 17 new tests pass.

---
*Phase: 09-path-extraction-weather*
*Completed: 2026-07-11*
