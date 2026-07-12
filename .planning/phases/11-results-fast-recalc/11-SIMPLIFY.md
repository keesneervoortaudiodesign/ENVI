# Phase 11 — /simplify pass (quality-only, behavior-preserving)

Applied a set of reuse/simplification cleanups to the Phase-11 diff. No observable
behavior changed; no bug hunting (that is `/gsd-code-review`). All gates green
(cargo fmt/clippy/test incl. `wire_no_drift`; web typecheck/vitest/build; Playwright).

## Applied

1. **Deduped the compute-WASM loader.** Five clients each re-implemented an identical
   `ensureGlue()` dynamic-import closure for `../generated/wasm-compute/envi_compute_wasm`.
   Extracted a single typed facade `web/src/compute/wasm.ts` (mirrors `web/src/import/wasm.ts`):
   one lazy `ensureCompute()` + typed wrappers (`planTiers`, `tensorHash`, `readoutReceivers`,
   `traceIsophones`, `estimateCost`, `exportEncode`, `exportFilename`) that localise the `as`
   casts. Rewired all consumers to the facade: `map/isophoneLayer.ts`, `store/results.ts`,
   `store/stale.ts`, `store/conditioning.ts`, `store/exportUi.ts`, plus `compute/marshalScene.ts`
   (dropped its own `ensureCompute`/`ComputeGlue`). `compute/cost.ts` was a one-fn wrapper whose
   whole body was the duplicated loader — removed it and repointed `panels/CalcPanel.tsx` at the
   facade's `estimateCost`. Same dynamic-import-for-node-isolation behavior preserved (vitest
   node graph never pulls the wasm/worker/OPFS graph).

2. **Extracted shared map fill-overlay helpers.** `map/differenceLayer.ts` duplicated
   `SCENE_LAYER_PREFIXES`, `sceneInsertBeforeId`, and the fill upsert/teardown verbatim from
   `map/isophoneLayer.ts` (differing only by ids + fill-opacity). Extracted `SCENE_LAYER_PREFIXES`,
   `sceneInsertBeforeId`, `upsertGeoJsonFillLayer(map, sourceId, layerId, opacity, data)`, and
   `removeGeoJsonFillLayer(map, sourceId, layerId)` into `map/fillOverlay.ts`; both layers use it.
   Fixes the D-18 z-order desync risk (one insert-order source of truth). Telemetry stays per-caller.
   The full IsophoneLayer/DifferenceLayer controllers + legend components were intentionally NOT
   unified (deferred, see below).

3. **Rust weather DTO conversions.** The `WeatherComponentsDto` / `SoundSpeedProfileDto` field
   mappings were hand-written across `derive_weather` + `derive_weather_friendly` in
   `crates/envi-gis-wasm/src/lib.rs`. Added `impl From<&WeatherComponents> for WeatherComponentsDto`
   and a free `profile_dto(&SoundSpeedProfile) -> SoundSpeedProfileDto` in `dto.rs`, called from the
   two genuine struct-conversion sites each. No wire-shape change (`wire_no_drift` still passes).
   - **Note:** `SoundSpeedProfileDto` lives in `envi_compute::scene_dto` and the source
     `SoundSpeedProfile` in `envi_engine` — both foreign to `envi-gis-wasm`, so the orphan rule
     forbids a `From` impl here; a free fn is the correct form. `WeatherComponentsDto` is local, so
     it uses `From`. The `raw_override` branch builds both DTOs from scalars (no source struct), so
     it stays a manual literal — nothing to dedup there.

4. **`patchSource` in `store/conditioning.ts`.** `setGain/setDelay/setMuted/setFilter` repeated the
   same `current ?? defaultConditioning()` → spread-one-field → `scheduleRecondition()` dance. Added
   a module-scoped `patchSource(get, set, sourceId, patch)` and delegated the four setters. Identical
   behavior (a synchronous `get().perSource` read equals the prior functional-updater form).

5. **Single `hexToRgb`.** Three copies (`store/colorScale.ts`, `map/weatherOverlay.ts`,
   `map/hatchPatterns.ts`'s `rgb`) collapsed into one shared `map/color.ts` `hexToRgb`, used by all
   three. Display-colour arithmetic only; inputs are program-controlled palette literals.

6. **Cheap legend memoization.** `MapLegend` (`legendClasses`) and `DifferenceLegend`
   (`buildDivergingScale` + `diffLegendClasses`) now `useMemo` keyed on their inputs (breaks/colors,
   delta) so an unrelated re-render no longer recomputes the scale/classes. Hooks moved above the
   early return to satisfy the rules-of-hooks.

## Deferred (intentionally not done — left as one-line notes)

- **Unify the five async-generation guards** (`staleGen` / `reconditionGen` / the two layer `gen`
  refs / the readout selection guard) into one primitive. Skipped: it touches just-hardened
  honest-state code (D-12/CR-01/CR-02/WR-03); the risk of a subtle regression outweighs the dedup.
- **Full IsophoneLayer/DifferenceLayer controller + legend unification.** The two React controllers
  and legend components share structure but differ in store source, scale derivation, and telemetry
  shape; only the fill-overlay primitives (item 2) were shared. A full merge is a larger refactor for
  a later pass.
- **CSV export O(K²) per-chunk batching + per-batch hash minting** in `exportUi.ts`/`results.ts`.
  This is an efficiency change with correctness surface (batching + hashing), out of scope for a
  behavior-preserving simplify pass.
