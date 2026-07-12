---
phase: 11-results-fast-recalc
reviewed: 2026-07-12T00:00:00Z
depth: deep
files_reviewed: 31
files_reviewed_list:
  - web/src/App.tsx
  - web/src/map/MapCanvas.tsx
  - web/src/map/isophoneLayer.ts
  - web/src/map/differenceLayer.ts
  - web/src/map/objectStyles.ts
  - web/src/map/objectStyles.test.ts
  - web/src/map/hatchPatterns.ts
  - web/src/panels/ResultsPanel.tsx
  - web/src/panels/SpectrumPanel.tsx
  - web/src/panels/ColorScaleEditor.tsx
  - web/src/panels/ConditioningPanel.tsx
  - web/src/panels/ScenarioPanel.tsx
  - web/src/panels/ExportMenu.tsx
  - web/src/panels/CalcPanel.tsx
  - web/src/panels/WeatherPanel.tsx
  - web/src/panels/ImportPanel.tsx
  - web/src/panels/Inspector.tsx
  - web/src/panels/Palette.tsx
  - web/src/panels/ProjectBar.tsx
  - web/src/panels/FacadePanel.tsx
  - web/src/panels/fields/SourceFields.tsx
  - web/src/panels/fields/ForestFields.tsx
  - web/src/panels/fields/GroundZoneFields.tsx
  - web/src/icons.ts
  - web/src/app.css
  - web/src/theme.css
  - web/tests/e2e/results-spectrum.spec.ts
  - web/tests/e2e/isophone.spec.ts
  - web/tests/e2e/conditioning.spec.ts
  - web/tests/e2e/scenarios.spec.ts
  - web/tests/e2e/export.spec.ts
  - web/tests/e2e/objectStyling.spec.ts
  - web/tests/e2e/infoButton.spec.ts
findings:
  critical: 0
  warning: 3
  info: 6
  total: 9
status: issues_found
---

# Phase 11: Code Review Report (Web UI + Playwright UAT)

**Reviewed:** 2026-07-12
**Depth:** deep
**Files Reviewed:** 31 (plus `store/results.ts`, `store/colorScale.ts`, `help/InfoButton.tsx` traced across module boundaries)
**Status:** issues_found

## Summary

The Phase-11 results/fast-recalc UI is well-built and holds its load-bearing invariants under adversarial reading:

- **Isophones are FILL, never raster (D-02):** both `isophoneLayer.ts` and `differenceLayer.ts` hardcode `type: "fill"` and expose `layerType` telemetry; the specs assert `"fill"` across every recolour/re-contour. No heatmap layer exists.
- **Draw order (D-18):** the fills insert *before* the first scene-object layer via `sceneInsertBeforeId`, and `SceneObjectLayers` appends object layers at the top. Traced through initial mount order, async-trace timing, and basemap `style.load` rebuild — objects stay above the fills in all three paths. `objectStyling.spec.ts` proves `aboveIsophone === true`.
- **legend ≡ contour ≡ class (single source of truth):** the isophone tracer, fill, and legend all derive from one `breaks[]`/`colors[]` in `colorScale.ts`; `setBreaks` maintains `colors.length === breaks.length + 1`. The difference layer/legend both compute from one pure `buildDivergingScale(delta)`. Verified midpoint band is exactly the neutral gray (odd band count), and the brand accent is not a diverging pole.
- **Object styling (D-17/D-19):** `objectStyles.ts` is a separate display map; `useTerraDraw.ts` is untouched, and the regression-guard spec confirms draw-time behaviour is unchanged. Every hex mirrors a `--obj-*` theme token.
- **Security:** no `eval`/`innerHTML`/`dangerouslySetInnerHTML`; every dynamic string reaches the DOM as a React text child; SVG glyphs are DOM-constructed; hatch/marker rasters are program-generated from a fixed palette (no user string reaches image ids). Exports are offline blob downloads. Clean.
- **Playwright specs** genuinely drive the real bundle + real WASM offline, seed via the dev bridge, and assert observable behaviour (fill type, trace-count advance with zero network egress, instant weighting toggle with no loading state, 409 refuse leaving totals unchanged, real Blob bytes with attribution). No tautologies or self-mocking detected. Driving the vite dev server (not `web/dist`) is the accepted deviation noted in scope.

No BLOCKER-class defect was provable. The findings below are robustness / accessibility / maintainability issues.

## Warnings

### WR-01: `as ControlId` casts defeat the D-25 exhaustiveness guarantee → runtime crash on a missing catalog entry

**Files:**
`web/src/panels/ExportMenu.tsx:101`, `web/src/panels/ImportPanel.tsx:59`, `web/src/panels/Palette.tsx:80`
**Issue:** These sites construct an InfoButton id by string interpolation and cast it: `` `export.${f.id}` as ControlId ``, `` `import.layer_${layer}` as ControlId ``, `` `palette.${row.tool}` as ControlId ``. `InfoButton` immediately dereferences `const entry = catalog[controlId]` and then reads `entry.title` in `aria-label` (`InfoButton.tsx:226,294`) with no undefined guard. The `as` cast erases the compile-time proof (the `Record<ControlId, HelpEntry>` type) that the id exists in the catalog. If a new format/layer/tool string is ever added without a matching catalog entry, `entry` is `undefined` and the panel throws `TypeError: Cannot read properties of undefined (reading 'title')` at render — and there is no error boundary, so the render subtree white-screens. The `coverage.test.ts` only proves every *declared* `ControlId` has an entry; it cannot see a cast-produced id that is outside the union.
**Failure scenario:** A future dev adds a 4th export format `"png"` to `FORMATS` but forgets `export.png` in the catalog. `tsc` stays green (the cast hides it); at runtime the export menu crashes the moment it renders.
**Fix:** Drop the casts and make the ids concrete so `tsc` re-enforces coverage, e.g. give each `FORMATS`/`LAYER_KEYS`/`ROWS` entry an explicit `controlId: ControlId` field instead of interpolating, or add a defensive guard in `InfoButton` (`if (!entry) return null;`) as defence-in-depth.

### WR-02: Help popover / dock (role="dialog") have no focus management — keyboard/SR a11y gap (D-23)

**File:** `web/src/help/InfoButton.tsx:138-221, 287-324`
**Issue:** Both `HelpPopover` and `HelpDock` are `role="dialog"` portalled to `document.body`, but focus is never moved into them on open, there is no focus trap, and focus is not restored to the trigger on close. The dock is `role="dialog"` without `aria-modal` and without any focus containment. A keyboard-only or screen-reader user who activates "More" gets a dialog they cannot reach with the keyboard (Tab continues through the underlying page), and after Escape/close focus location is undefined. The prompt calls out "focus ring + a11y" as a D-23 requirement; the focus *ring* is present (`:focus-visible`), but dialog focus behaviour is missing.
**Failure scenario:** SR user opens the docked help panel; the panel content is not announced and Tab order stays in the background shell; on close, focus is lost.
**Fix:** On dock open, move focus to the dock (or its close button) via a ref + `useEffect`; trap Tab within the dock while open; on close, restore focus to `btnRef.current`. At minimum add `aria-modal="true"` and an initial-focus target.

### WR-03: Deferred `instance.once("load", …)` inside the trace resolver is uncancellable and un-generation-guarded → can re-add a stale fill layer

**Files:** `web/src/map/isophoneLayer.ts:270-274`, `web/src/map/differenceLayer.ts:274-277`
**Issue:** When a trace resolves while the style is mid-reload, the code queues `instance.once("load", () => applyIsophoneGeoJson(instance, fc))`. This deferred callback is (a) never removed by the effect cleanup, and (b) not guarded by the `gen` generation counter that protects the synchronous path. If the grid/delta is cleared (or the effect re-runs / unmounts) before that `load` fires, the queued callback still fires and re-adds a fill for data that is no longer current.
**Failure scenario:** User seeds a grid, then rapidly switches basemap and clears the isophone; the pending `once("load")` from the first trace fires after clear and re-paints a superseded fill layer, which then lingers until the next edit.
**Fix:** Capture `myGen` in the deferred closure and re-check `myGen === gen.current` (and that a valid request still exists) inside the `once("load")` handler before calling `applyIsophoneGeoJson`; or register the deferred handler through the effect so cleanup can `off` it.

## Info

### IN-01: Receiver mini-map markers are not keyboard-operable

**File:** `web/src/panels/SpectrumPanel.tsx:239-254`
**Issue:** The receiver markers are bare `<circle onClick>` SVG elements with no `role`, `tabIndex`, or keyboard handler, so they cannot be selected by keyboard. This is mitigated by the parallel `<button>` list (`spectrum-receiver-list`) that *is* accessible, so the map is a redundant affordance — acceptable, but note that the "click a marker on the map" path is mouse-only.
**Fix:** If the map is meant to be an equal input path, add `role="button"`, `tabIndex={0}`, and an `onKeyDown` (Enter/Space) to each circle; otherwise leave as the documented redundant affordance.

### IN-02: `Math.min/max(...spread)` over unbounded arrays

**Files:** `web/src/panels/SpectrumPanel.tsx:219-221` (`Math.min(...xs)` / `Math.max(...ys)`), `SpectrumPanel.tsx:308-309` (`Math.max(...allVals)` / `Math.min(...allVals)`)
**Issue:** Spreading a large array into `Math.min`/`Math.max` risks a call-stack overflow for very large inputs. Band arrays are ≤105 and the receiver picker lists only the (small) receiver-points tier today, so this is latent, not live.
**Fix:** Use a reduce-based min/max if these ever receive fine-grid-sized receiver lists.

### IN-03: WeatherPanel hour input is not clamped in the handler

**File:** `web/src/panels/WeatherPanel.tsx:145-153`
**Issue:** `min={0} max={23}` are HTML hints only; `onChange={(e) => setHour(Number(e.target.value))}` accepts any typed value (e.g. 99, or `NaN` from empty). Unless the store clamps, an out-of-range hour propagates to the Open-Meteo lookup.
**Fix:** Clamp in the handler (`setHour(Math.min(23, Math.max(0, Math.round(Number(e.target.value) || 0))))`) or in the store setter.

### IN-04: Duplicate WASM trace-client singleton in the difference layer (DRY / redundant init)

**File:** `web/src/map/differenceLayer.ts:182-186`
**Issue:** `differenceLayer.ts` imports `createWasmTraceClient` from `isophoneLayer.ts` but re-wraps it in its own module-level `traceClient` + `ensureClient`, so a second trace client is instantiated (a second `g.default()` wasm-init). Modern wasm-bindgen `init` returns the cached instance on the second call, so this is harmless today, but it is a second source of the same seam.
**Fix:** Export `ensureClient` (or the single trace client) from `isophoneLayer.ts` and reuse it in the difference layer.

### IN-05: InfoButton `aria-expanded` ignores the docked state; popover not repositioned on scroll/resize

**File:** `web/src/help/InfoButton.tsx:243, 296`
**Issue:** `aria-expanded={open}` tracks only the glance popover; after "More" (`setOpen(false); setDocked(true)`) the button reports `aria-expanded="false"` while its dock dialog is open. Separately, `popStyle` is computed once in `useLayoutEffect(…, [open])` from a `getBoundingClientRect`, so a `position:fixed` popover does not follow the trigger if the user scrolls/resizes while it is open.
**Fix:** Reflect `open || docked` in a suitable ARIA state, and reposition (or close) the popover on `scroll`/`resize` while open.

### IN-06: ColorScaleEditor "Number of intervals" generates that many break EDGES, not classes

**File:** `web/src/panels/ColorScaleEditor.tsx:55,65-71` → `store/colorScale.ts:172-176`
**Issue:** The "Number of intervals" input feeds `applyGenerator → generateBreaks(smallest, magnitude, count)`, which produces `count` *edges* → `count + 1` classes. The seeded default (`breaks.length` = 5) round-trips to the EU-END scale, so it is internally consistent, but "intervals" reading as edges vs. classes is ambiguous to a user.
**Fix:** Either relabel to "Number of break edges", or map the input to classes (`generateBreaks(..., count - 1)`), whichever matches the NoizCalc §4.6.5 wording.

---

_Reviewed: 2026-07-12_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: deep_
