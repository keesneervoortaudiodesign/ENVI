---
phase: 11-results-fast-recalc
plan: 02
subsystem: compute
tags: [wasm, isophone, marching-squares, contour, iso-bands, level-grid, geo, nord2000]

# Dependency graph
requires:
  - phase: 11-results-fast-recalc
    provides: envi-compute readout orchestration + OPFS tensor reader (the dBA vector this grid reconstructs)
  - phase: 10-calculation-service
    provides: envi-compute pure core + tiers::partition (the fine-tier lattice reconstructed here)
provides:
  - Fine-tier receiver lattice → dense index-keyed 2-D LevelGrid reconstruction (verifies RESEARCH A3)
  - Hand-rolled interpolated marching-squares iso-band tracer → nested non-crossing SceneXY FILL polygons
  - Re-contour-without-re-solve backend for SC3 (editing breaks re-runs only the tracer over the cached grid)
  - BreakScale validation (V5: monotonic/finite/≥2, typed error) — the single source of truth for legend ≡ contour ≡ class colours (D-04)
affects: [11-06]

# Tech tracking
tech-stack:
  added: [geo (envi-compute direct dep — already a workspace crate via envi-gis; no new-to-repo crate)]
  patterns:
    - "Isophones are FILL POLYGONS (nested iso-bands), never a heatmap layer (D-02)"
    - "Level grid keyed STRICTLY by lattice index (round((pos−origin)/spacing)), never nominal position"
    - "Hand-rolled interpolated marching squares (contour* crates declined SUS) — reuse landcover.rs DNA via geo: containment ring classification, Simplify, BooleanOps difference, Relate non-crossing"
    - "Bands = annular difference of strictly-nested threshold regions {v≥b_k}\\{v≥b_{k+1}} → adjacent, never overlapping"
    - "Mean-value (cell-average) saddle rule prevents self-cross/leak (Pitfall 6)"
    - "Border/hole edge crossings clipped to real nodes so all thresholds clip identically (no per-threshold sliver)"

key-files:
  created:
    - crates/envi-compute/src/grid.rs
    - crates/envi-compute/src/isoband.rs
  modified:
    - crates/envi-compute/src/lib.rs
    - crates/envi-compute/Cargo.toml
    - Cargo.lock

key-decisions:
  - "geo promoted to an envi-compute direct dependency (Rule 3): the tracer needs geo::{Simplify, BooleanOps, Contains, Relate, Orient}. geo 0.30 is already a workspace crate (envi-gis pins the same) → zero new-to-repo crates, engine 3-dep quarantine untouched (cargo tree -p envi-engine unchanged)"
  - "Bands built as annular set-difference of nested threshold regions via geo::BooleanOps (i_overlay backend), not a bespoke 81-case iso-band table — literal [b_k,b_{k+1}) geometry, adjacent (never overlapping) siblings, direction-independent"
  - "Undirected degree-2 cycle stitch + even/odd containment-depth exterior/hole classification (marching stitch has no fixed winding); regions oriented to geo convention before BooleanOps reads winding as the fill rule"
  - "No-data holes (f64::NAN) and out-of-grid padding treated as below every break; border crossings CLIPPED to the real node so nested regions share borders exactly"

requirements-completed: []  # GRID-04/WEB-06 advanced here (the pure-Rust contour CORE); both COMPLETE in 11-06 (MapLibre fill layer + colour-scale editor). Left Pending, not marked complete.

# Metrics
duration: 23 min
completed: 2026-07-12
status: complete
---

# Phase 11 Plan 02: Isophone Contouring Core (Level Grid + Iso-Band Tracer) Summary

**Pure-Rust/WASM isophone backend: reconstruct the fine-tier receiver lattice into a dense index-keyed level grid, then trace it into nested, non-crossing iso-band FILL polygons with a hand-rolled interpolated marching-squares tracer (mean-value saddle rule, annular BooleanOps difference) that re-contours on a break edit with no re-solve — the SC3 geometry the MapLibre fill layer consumes in 11-06.**

## Performance

- **Duration:** ~23 min
- **Tasks:** 2 (both `type=auto`)
- **Files:** 5 (2 created, 3 modified) — 1010 insertions

## Accomplishments

- **Level-grid reconstruction (Task 1, GRID-04 / verifies RESEARCH A3):** `grid::reconstruct_level_grid(fine_tier, dba)` maps each fine receiver's `position` back to integer `(col, row)` lattice indices via `round((pos−origin)/spacing)` and scatters its `dba[global_index]` value into a dense `[rows × cols]` row-major `Vec<f64>`. Origin/spacing are recovered from the fine tier itself (A3 holds: `tiers::partition` enumerates a single-spacing lattice from one origin). Lattice nodes the fine tier never introduced (the coarse-tier corners) and out-of-range `global_index`es stay `f64::NAN` no-data holes — no panic on an empty/degenerate lattice. Keyed **strictly by lattice index**, never nominal position.
- **Interpolated marching-squares iso-band tracer (Task 2, D-02 / WEB-06):** `isoband::trace_isobands(grid, breaks)` produces `breaks.len()−1` `IsoBand`s, band `k` covering `[breaks[k], breaks[k+1])`. Each threshold region `{v ≥ breaks[k]}` is traced by interpolated marching squares (linear edge crossings → smooth contours, not a staircase); ambiguous saddle cells are resolved by the **mean-value (cell-average) rule** so bands never self-cross or leak (Pitfall 6). Bands are the **annular set-difference** of the strictly-nested regions (`{v≥b_k} \ {v≥b_{k+1}}`), so sibling bands are **adjacent, never overlapping**. Output rings are SceneXY `[x,y]` (reprojection to LonLat is the map layer's job, 11-06).
- **Reuses the `landcover.rs` DNA via `geo`, not a copy:** containment-based ring classification (even/odd nesting depth → exterior/hole, smallest-area parent assignment), `geo::Simplify` (Douglas–Peucker), `geo::BooleanOps::difference` (annular partition), `geo::Orient` (winding for the boolean fill rule), and the `geo::Relate` non-crossing property (asserted in tests). The two `contour*` crates stay **declined** (SUS; RESEARCH Package Legitimacy) and the gdal escape hatch is **unused** — the 316×316 (~100k-cell) × 10-break trace runs in **~28 ms release** (<100 ms roadmap budget).
- **Break-scale validation (V5, threat T-11-02-01):** `BreakScale::new` / `trace_isobands` reject fewer than two breaks, a non-finite value, or a non-monotonic sequence with a typed `IsobandError` — never a panic on user input. `breaks[]` is the single source of truth for `legend ≡ contour ≡ class colours` (D-04).

## Task Commits

1. **Task 1: Level-grid reconstruction** — `5a0febc` `feat(11-02): fine-tier lattice → dense 2-D level grid reconstruction`
2. **Task 2: Iso-band tracer** — `78ff24e` `feat(11-02): interpolated marching-squares iso-band tracer (D-02)`

_Plan metadata commit follows this SUMMARY._

## Files Created/Modified

- `crates/envi-compute/src/grid.rs` — `LevelGrid` + `reconstruct_level_grid` (index-keyed reconstruction, NaN holes)
- `crates/envi-compute/src/isoband.rs` — `BreakScale`, `IsoBand`, `IsobandError`, `trace_isobands` + the hand-rolled tracer
- `crates/envi-compute/src/lib.rs` — registered `grid`, `isoband`
- `crates/envi-compute/Cargo.toml` — `geo = "0.30"` promoted to a direct dependency
- `Cargo.lock` — records the `geo` edge (+ the `ndarray` edge left uncommitted by 11-01); no new packages

## Decisions Made

- **`geo` promoted to an envi-compute direct dependency (Rule 3):** the acceptance criteria and the "reuse landcover DNA" mandate require `geo::{Simplify, BooleanOps, Contains, Relate, Orient, Area, InteriorPoint}`. `geo 0.30` is **already a workspace crate** (envi-gis pins the same version), so this adds **zero new-to-the-repo crates** and the lockfile gains only a dependency edge — no new package. `cargo tree -p envi-engine` is unchanged (the engine's `ndarray + num-complex + thiserror` 3-dep quarantine is one-directional and untouched). This mirrors 11-01's `ndarray`/`num-complex` promotion.
- **Annular bands via BooleanOps difference, not a bespoke iso-band case table:** building each band as `region(k).difference(region(k+1))` over strictly-nested threshold regions yields literal `[b_k,b_{k+1})` geometry with guaranteed non-overlap and multi-level hole handling, reusing the robust `i_overlay` backend instead of a hand-maintained 81-case table.
- **Direction-independent topology:** the marching stitch traces an undirected degree-2 segment graph (edge-id keyed so crossings from adjacent cells are bit-identical); exterior/hole is classified by even/odd containment depth, then regions are `Orient`ed to the geo convention so `BooleanOps` reads winding correctly.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Promote `geo` to an envi-compute direct dependency**
- **Found during:** Task 2 (the tracer needs `geo::Simplify`/`BooleanOps`/`Contains`/`Relate`/`Orient`, and the acceptance criteria explicitly assert `geo::Relate is_overlaps()` and `geo::Simplify`).
- **Issue:** `geo` was not an `envi-compute` dependency — the crate would not compile.
- **Fix:** Added `geo = "0.30"` to `[dependencies]` (same pin as envi-gis). Documented in the Cargo.toml comment that it is an existing workspace crate (no new-to-repo dependency) and that the engine quarantine is untouched.
- **Files modified:** `crates/envi-compute/Cargo.toml`, `Cargo.lock`
- **Verification:** `cargo tree -p envi-engine` still shows only `ndarray + num-complex + thiserror`; the Cargo.lock diff adds no new package (`geo` was already resolved for envi-gis).
- **Committed in:** the Task 2 commit (`78ff24e`).

**2. [Rule 1 - Bug] Clip border/hole edge crossings to real nodes (nested-region border sliver)**
- **Found during:** Task 2 self-test (the ramp band's right edge landed at the grid-border padding overshoot ≈10.73 instead of the expected break crossing at x=5).
- **Issue:** the exterior padding sentinel produced a slightly different border overshoot per threshold, so `{v≥b_{k+1}}` became an interior *hole* of `{v≥b_k}` instead of sharing its border — the band difference left a spurious thin sliver at the grid edge (correct area, wrong shape).
- **Fix:** when an edge straddles a pad/hole node, the crossing is **clipped to the real node's position** (not interpolated toward the sentinel), so every threshold region clips to the same grid-border / hole-edge nodes and nested regions share borders exactly. Interior crossings (both endpoints real) remain exact linear interpolations.
- **Files modified:** `crates/envi-compute/src/isoband.rs`
- **Verification:** ramp test now asserts exact interpolated crossings (x=2/5/8 within 1e-6); peak/saddle non-crossing tests pass.
- **Committed in:** the Task 2 commit (`78ff24e`).

**3. [Rule 1 - Bug] Orient region polygons before the boolean difference**
- **Found during:** Task 2 self-test (the annular difference initially returned the whole outer region — the inner region was not subtracted).
- **Issue:** the undirected marching stitch has no fixed winding; `geo::BooleanOps` (i_overlay) reads ring winding as the fill rule, so arbitrarily-wound regions produced a wrong difference.
- **Fix:** `orient(Direction::Default)` each region MultiPolygon (exterior CCW, holes CW) before differencing.
- **Files modified:** `crates/envi-compute/src/isoband.rs`
- **Verification:** band nesting/areas and non-crossing tests pass.
- **Committed in:** the Task 2 commit (`78ff24e`).

---

**Total deviations:** 3 auto-fixed (1 blocking dependency promotion, 2 tracer-correctness bugs surfaced by the plan's own acceptance tests). **Impact:** all within the plan's scope (a correct, non-crossing, re-runnable tracer); no scope creep; engine quarantine byte-identical.

## Threat Model Coverage

- **T-11-02-01 (Tampering, break input):** `validate_breaks` enforces monotonic/finite/≥2 with a typed `IsobandError`, never a panic — asserted by `invalid_breaks_return_typed_error_never_panic`.
- **T-11-02-02 (DoS, bounded work):** the trace is `O(cells × breaks)`; grid dims come from the tier plan, not unbounded user input — the 100k-cell × 10-break perf test bounds it (~28 ms release).
- **T-11-02-03 (Integrity, saddle cells):** the mean-value saddle rule + the `geo::Relate` non-crossing property test (`saddle_grid_traces_non_crossing_bands`, `peak_grid_bands_are_nested_non_crossing_and_shrink_inward`) guarantee no self-cross/leak.

## Issues Encountered

None outstanding — all three tracer acceptance gates (interpolated ramp crossings, saddle/peak non-crossing via `geo::Relate`, sub-100 ms 100k-cell benchmark) and both grid gates (index-keyed round-trip, empty-lattice safety) pass.

## Verification

All commands run at the workspace root on `main`:

- `cargo build --release` → **Finished** (14.56s), exit 0.
- `cargo test` (full workspace) → **all pass** (1 pre-existing capability-gated skip). Includes the new grid tests (index-keyed reconstruction, empty lattice, out-of-range global_index) and isoband tests (ramp band-count + exact interpolated crossings, peak nested-non-crossing-shrink, saddle non-crossing, invalid-breaks typed error, empty grid, 100k-cell perf).
- `cargo test -p envi-compute --release perf_100k` → `isoband 316x316 x10 breaks: 9 bands in 28.2 ms` (<100 ms budget; gdal escape hatch unused).
- `cargo clippy --all-targets -- -D warnings` → **clean** (whole workspace).
- `cargo fmt --check` → **clean** (exit 0).
- `cargo tree -p envi-engine` → `ndarray + num-complex + thiserror` — **unchanged** (engine 3-dep quarantine intact, D-02).
- Cargo.lock diff → **no new package**, only the `geo`/`ndarray` dependency edges on `envi-compute` (`geo` already in-tree via envi-gis).

## Requirements

**GRID-04** (contour results into isophone fill polygons) and **WEB-06** (isophone fill polygons + editable colour scale + legend) are *advanced* by this plan — it delivers the pure-Rust contour **core** (level grid + tracer + `breaks[]` source-of-truth). Both **complete in 11-06** (the MapLibre fill layer + colour-scale editor + END/viridis presets + Playwright). Left **Pending** in REQUIREMENTS.md rather than marked complete, since the user-facing capability is not yet shipped (the 11-01 precedent for WEB-11).

## Next Phase Readiness

- **11-06** (isophone fill layer + colour-scale editor) can now consume `reconstruct_level_grid` + `trace_isobands`: cache the `LevelGrid` after the first readout, and re-run only `trace_isobands` on a break edit (SC3 — no re-solve).
- The tracer emits SceneXY rings; **11-06** reprojects each vertex through `envi-geo` → LonLat for the MapLibre fill layer (the single CRS boundary).
- `breaks[]` is the single source of truth for `legend ≡ contour ≡ class colours` (D-04) — 11-06 must drive both the legend and the tracer from the same array.

## Self-Check: PASSED

- Created files exist on disk: `crates/envi-compute/src/grid.rs`, `crates/envi-compute/src/isoband.rs`.
- Task commits present in `git log`: `5a0febc` (grid), `78ff24e` (isoband).
- All plan `<verification>` commands re-run green (build/test/clippy/fmt/tree above); engine quarantine unchanged; no new crate.

---
*Phase: 11-results-fast-recalc*
*Completed: 2026-07-12*
