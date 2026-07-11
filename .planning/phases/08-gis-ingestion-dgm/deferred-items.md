# Phase 08 — Deferred Items

Out-of-scope discoveries logged during execution (not fixed here — see the deviation SCOPE BOUNDARY).

## 08-08

### [DEFER] Re-importing an already-imported layer into a populated scene traps the WASM (`unreachable`)

- **Found during:** 08-08 Task 2 (building the DATA-04 replay).
- **Symptom:** After a viewport import succeeds, running the import AGAIN for the same viewport
  (e.g. clicking "Import (viewport)" twice, which re-fires every enabled layer) makes the
  terrain and land-cover layers fail with a WASM `unreachable` trap. The trap is in the
  `merge_features` (D-09) path when the incoming features' identities already exist in the
  existing scene (re-import of a *succeeded* layer).
- **Not a blocker for 08-08:** the plan's success criteria (a first-time import journey + the
  DATA-04 network-off proof) do not require re-importing a succeeded layer. The DATA-04 replay
  models the compute read against a reset scene (same project id ⇒ OPFS cache persists), which
  is the honest OPFS-read exercise and avoids this path. The `retryLayer` flow that a *failed*
  layer uses is unaffected — a failed layer committed nothing, so its retry merges into a scene
  without its own prior features (covered green by the Overpass-429 retry test).
- **Impact if unfixed:** a user who clicks "Import (viewport)" twice sees terrain/land cover
  error on the second click. Pre-existing in 08-07 (import E2E was deferred to 08-08), not
  introduced by this plan.
- **Suggested owner:** a follow-up fix in `crates/envi-gis/src/merge.rs` (make the D-09 merge
  idempotent/panic-free when an incoming feature identity collides with an existing imported
  one), with a WASM-boundary regression test.

## Post-review informational (2026-07-11, code-review gate)
- **Antimeridian world-copy bbox normalization:** `evaluateGuardrail` blocks the inverted `min_lon>max_lon` representation, but a viewport panned across the antimeridian in a world-copy map can arrive as `min_lon=170,max_lon=190` (both increasing), passing the guard and enumerating non-existent `lon>180` tiles — these fetch-fail / zero-cover, never crash. Pre-existing (not a Phase-8 regression). Fix = normalize bbox longitude to [-180,180] before tile planning. Low priority (NL-focused tool).

## Simplify-gate deferred follow-ups (2026-07-11, quality only — behavior-neutral, non-blocking)
- **Efficiency E1–E4 (importJob base-elevation + tile fetch):** `sample_base_elevation` re-decodes each terrain COG window once per building (B×T redundant decodes); reuse the raster already decoded in `runTerrain` (core `base_elevation_on_raster(&Raster)` exists), retain the decoded window not raw tile bytes, reproject each ring once per CRS, and prefetch per-tile fetches concurrently (Promise.all) while decode stays serial. Import-time UI perf; deserves its own change + Playwright verify.
- **Simplification S3:** extract a generic `runRasterLayer(layer, {sourceCrs, decodeTile})` helper to dedup the ~40-line terrain/landcover raster-tile scaffolding in importJob.ts (keep runBuildings separate). Judgment call — touches D-07 per-layer independence; verify carefully.
- **Altitude A1/A2/A3:** make vertical-datum, attribution, and source-CRS registry-driven descriptor fields (add to Rust `SourceDescriptor` + `SourceDescriptorDto`, regen wire.ts + no-drift) instead of hand-authored TS keyed on source id / EPSG string (verticalDatumOf, IMPORT_ATTRIBUTIONS, sourceCrsOf). Honors the registry's 'new source = data row, no control-flow change' promise; cross-boundary schema change.
