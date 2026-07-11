---
phase: 08-gis-ingestion-dgm
plan: 05
subsystem: gis
tags: [worldcover, landcover, vectorization, marching-squares, geojson, ground_zone, geo, impedance, wasm]

# Dependency graph
requires:
  - phase: 08-04
    provides: worldcover_to_class impedance table + DEFAULT_ROUGHNESS_CLASS (WorldCover → Nord2000 letter, σ in engine)
  - phase: 08-02
    provides: cog::Raster<T> + GeoTransform (windowed decode carrying its own geotransform)
  - phase: 08-03
    provides: provenance::Provenance D-11 stamping (plain GeoJSON properties)
  - phase: 06
    provides: 9-kind scene vocabulary incl. ground_zone + impedance_class property key
provides:
  - "envi_gis::landcover::vectorize_landcover — WorldCover Raster<u8> → editable WGS84 ground_zone polygon features"
  - "Hand-rolled marching-squares boundary tracer (per-class pixel partition → non-crossing rings with holes)"
  - "Pixel-space min-area drop + geo Douglas–Peucker simplification + bounded-work caps"
affects: [08-06, 08-07, impedance-debug-overlay, scene-merge]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Directed half-edge boundary tracing with clockwise-turn saddle resolution (non-crossing loops)"
    - "Partition vectorization: per-class binary mask → exteriors (area>0) + holes (area<0) → nested polygons"
    - "Simplify/area-filter in pixel space (CRS-independent tolerance), reproject only the final rings"

key-files:
  created:
    - crates/envi-gis/src/landcover.rs
  modified:
    - crates/envi-gis/src/lib.rs

key-decisions:
  - "SUS `contour` crate DECLINED (checkpoint pre-resolved by orchestrator): hand-rolled marching squares, zero new dependencies — only the already-present `geo` crate"
  - "Water (WC 80 → H) emitted as ground_zone features (editable, not skipped) per research Open-Q5"
  - "Default min-area 4 px² (≈400 m² at 10 m WorldCover) and DP tolerance 1.5 px; both filtered/applied in pixel space"
  - "Unknown class codes + nodata skipped, never mapped to a silent default zone (D-07)"
  - "WorldCover is EPSG:4326 → geotransform yields WGS84 directly (identity reprojection via envi_geo::LonLat, no proj string)"

patterns-established:
  - "Half-edge boundary extraction: interior-on-right unit edges stitched into closed rings; saddle → sharpest-right turn keeps loops touching, never crossing"
  - "geo::Relate DE-9IM is_overlaps() as the test-time 'no partial crossing' assertion (Phase-7 draw-rule proxy)"

requirements-completed: [DATA-02]

# Metrics
duration: 30 min
completed: 2026-07-11
status: complete
---

# Phase 8 Plan 05: WorldCover raster → ground_zone vectorization Summary

**Hand-rolled marching-squares vectorizer turning a WorldCover class raster into editable, non-crossing WGS84 `ground_zone` GeoJSON polygons carrying the reviewed Nord2000 impedance letter + provenance — with the SUS `contour` dependency declined (zero new deps).**

## Performance

- **Duration:** ~30 min
- **Started:** 2026-07-11T01:36:00Z
- **Completed:** 2026-07-11T02:06:27Z
- **Tasks:** 1 auto task (+ 1 pre-resolved checkpoint)
- **Files modified:** 2 (1 created, 1 modified)

## Accomplishments
- `vectorize_landcover(raster: &Raster<u8>, min_area_px, simplify_tol_px, provenance)` maps each contiguous same-class WorldCover region to a `ground_zone` polygon, holes preserved.
- Pure-Rust marching-squares boundary tracer over a per-class pixel partition: directed interior-on-right half-edges stitched into closed rings, with clockwise-turn saddle resolution guaranteeing adjacency/containment only — never a partial crossing (Phase-7 draw-time rule).
- Impedance class letter resolved through `worldcover_to_class` (σ stays in the engine — one source of truth); roughness defaults to class `N`; D-11 provenance stamped; no Rust-assigned `id` (TS owns UUIDs).
- `geo` Douglas–Peucker simplification + pixel-space min-area drop + bounded-work caps (`MAX_GROUND_ZONES`, `MAX_TOTAL_VERTICES`); guard-first, no panics on data.

## Task Commits

1. **Task 1: Vectorize WorldCover classes into ground_zone features** — `e87c9ea` (feat)

**Plan metadata:** _(this SUMMARY commit)_

## Files Created/Modified
- `crates/envi-gis/src/landcover.rs` — WorldCover `Raster<u8>` → per-class marching-squares rings → simplified, provenance-stamped `ground_zone` features. 9 unit tests.
- `crates/envi-gis/src/lib.rs` — registered `pub mod landcover;`.

## Decisions Made
- **`contour` crate declined (pre-resolved checkpoint).** The plan's `checkpoint:human-verify` gate on the SUS `contour` crate was resolved by the orchestrator in favor of the documented hand-rolled fallback. No new third-party dependency was added; `cargo tree -p envi-gis` confirms none of `{contour, reqwest, tokio, web-sys, hyper}` are present — the sans-I/O, WASM-safe quarantine is intact.
- **Water as zones.** WC 80 → H is emitted as an editable `ground_zone` (research Open-Q5), since the project ground default may be softer than H.
- **Defaults:** min-area `4 px²`, DP tolerance `1.5 px`, both handled in pixel space so the thresholds are CRS-independent; final rings reproject via `envi_geo::LonLat` (WorldCover is EPSG:4326 → identity, no proj string).

## Deviations from Plan

None — plan executed exactly as written, substituting the specified hand-rolled marching-squares fallback for `contour` per the pre-resolved checkpoint.

**Total deviations:** 0.
**Impact on plan:** None. The checkpoint outcome (decline `contour`) was applied as designed.

## Issues Encountered
- `geo`'s closed-ring Douglas–Peucker does not collapse a rectangle's collinear midpoints to the minimal 4 corners (a known closed-ring DP behavior). Handled by asserting the meaningful invariant — simplification strictly reduces vertex count and keeps a valid closed ring — rather than an implementation-specific exact count. No production impact (partition topology is preserved; rings stay non-crossing).

## Checkpoint Resolution
- **`checkpoint:human-verify` (T-08-05-SC, blocking-human):** RESOLVED by the orchestrator BEFORE execution — **DECLINE `contour`**, use the hand-rolled marching-squares fallback. Not re-asked. Package-legitimacy gate honored: no third-party raster-vectorization crate installed.

## Verification
- `cargo test -p envi-gis` — 45 tests pass (37 lib incl. 9 landcover + 8 integration).
- `cargo test --workspace` — all green (engine byte-identical; no regression).
- `cargo clippy -p envi-gis --all-targets -- -D warnings` — clean.
- `cargo fmt -p envi-gis --check` — clean.
- Acceptance greps: `grep contour crates/envi-gis/Cargo.toml` → none; `grep proj= crates/envi-gis/src/landcover.rs` → none; `worldcover_to_class` linkage present.
- Non-crossing assertions via `geo::Relate::is_overlaps()` on adjacency, containment, and a 3-class mosaic.

## Next Phase Readiness
- Editable `ground_zone` features are ready for the impedance debug overlay (08-07) and scene merge. The vectorizer consumes a `Raster<u8>`; wiring the WorldCover COG `u8` decode path (the class raster producer) is the remaining upstream seam (the current `cog::decode_window` handles `f32` terrain COGs).

## Self-Check: PASSED
- `crates/envi-gis/src/landcover.rs` exists on disk.
- `crates/envi-gis/src/lib.rs` registers `pub mod landcover;`.
- Task commit `e87c9ea` present in git history.
- SUMMARY.md present.

---
*Phase: 08-gis-ingestion-dgm*
*Completed: 2026-07-11*
