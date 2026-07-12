---
phase: 11-results-fast-recalc
plan: 11
subsystem: ui
tags: [react, tsx, help-system, info-button, accessibility, nord2000, noizcalc, playwright, vitest]

# Dependency graph
requires:
  - phase: 07-scene-authoring
    provides: metrao3 theme tokens, icons.ts glyph idiom, panel/field structure (Palette, Inspector, fields, ProjectBar, Facade)
  - phase: 08-gis-ingestion
    provides: ImportPanel controls
  - phase: 09-path-weather
    provides: WeatherPanel controls
  - phase: 10-compute
    provides: CalcPanel controls
  - phase: 11-results-fast-recalc
    provides: SpectrumPanel, ColorScaleEditor, ConditioningPanel, ScenarioPanel, ExportMenu (the new Phase-11 controls)
provides:
  - A reusable <InfoButton controlId=…/> affordance (glance popover + docked right-rail help panel)
  - A typed, structured help catalog (Record<ControlId, HelpEntry>) — data, not JSX-scattered text
  - A coverage test that FAILS if any interactive control lacks extensive, standards-cited help
  - App-wide retrofit: an InfoButton on every interactive control across ALL panels (Phase 7/8/9/10 + Phase 11)
affects: [future-ui-phases, help-content-maintenance, accessibility]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Coverage-from-type: ControlId union (runtime CONTROL_IDS array) is the single source of truth; catalog is Record<ControlId, HelpEntry> so a missing entry fails tsc, and coverage.test.ts fails if any control lacks extensive cited help"
    - "Structured help catalog as data (title/body[]/citations[]) — maintainable + drift-checkable (D-25)"
    - "Standards-cited own-words help: cite AV 1106/07 by report number + TI 386, never paste copyrighted text (D-24)"
    - "Portalled popover (position:fixed) so help renders above the map/panel stacking context"
    - "SSR/Node-safe React component (document only in effects) so Vitest node-env tests render it via react-dom/server (no jsdom)"

key-files:
  created:
    - web/src/help/controlIds.ts
    - web/src/help/catalog.ts
    - web/src/help/InfoButton.tsx
    - web/src/help/InfoButton.test.ts
    - web/src/help/coverage.test.ts
    - web/tests/e2e/infoButton.spec.ts
  modified:
    - web/src/icons.ts
    - web/src/panels/Palette.tsx
    - web/src/panels/ProjectBar.tsx
    - web/src/panels/Inspector.tsx
    - web/src/panels/FacadePanel.tsx
    - web/src/panels/ImportPanel.tsx
    - web/src/panels/WeatherPanel.tsx
    - web/src/panels/CalcPanel.tsx
    - web/src/panels/fields/SourceFields.tsx
    - web/src/panels/fields/ForestFields.tsx
    - web/src/panels/fields/GroundZoneFields.tsx
    - web/src/panels/SpectrumPanel.tsx
    - web/src/panels/ColorScaleEditor.tsx
    - web/src/panels/ConditioningPanel.tsx
    - web/src/panels/ScenarioPanel.tsx
    - web/src/panels/ExportMenu.tsx
    - web/dist/**

key-decisions:
  - "InfoButton unit test uses react-dom/server (renderToStaticMarkup) because the project deliberately has no jsdom; full DOM interaction (popover → More → docked panel) is verified by the offline Playwright spec instead"
  - "The glance popover is portalled to document.body with position:fixed (anchored to the button rect) so it sits above the MapLibre/panel stacking context and its 'More' affordance is never pointer-intercepted"
  - "Palette + export menu InfoButtons render OUTSIDE their action button (flex row) to avoid invalid nested <button> elements"
  - "Repeated/list controls (per-source conditioning, facade edges, palette kinds) get one help id per control TYPE; per-source conditioning renders one InfoButton per instance (same id)"

patterns-established:
  - "Info-Button help contract: 78-id ControlId union → typed catalog → coverage test → per-control retrofit"

requirements-completed: []

# Metrics
duration: ~45min
completed: 2026-07-12
status: complete
---

# Phase 11 Plan 11: App-wide Info-Button Help System Summary

**A reusable `<InfoButton controlId=…/>` (glance popover + docked help panel) backed by a typed, structured, standards-cited help catalog with a coverage test that fails on any gap — retrofitted onto every interactive control across all panels (Phase 7/8/9/10 + Phase 11), realizing D-23/24/25.**

## Performance

- **Duration:** ~45 min
- **Completed:** 2026-07-12
- **Tasks:** 4
- **Files modified/created:** 22 (6 created, 16 modified incl. web/dist)

## Accomplishments

- **D-23 affordance:** `InfoButton.tsx` — a 16px "i"-in-ring glyph (new `info` icon in `icons.ts`) with two depths: a portalled glance popover (title + first paragraph + citation + "More") and a docked right-rail help panel (full multi-paragraph body + all citations). Theme-aware metrao3 tokens, `:focus-visible` accent ring, `aria-*`, Esc/outside-click handling.
- **D-25 structured catalog:** `controlIds.ts` enumerates ~78 interactive controls as the single `ControlId` union (runtime `CONTROL_IDS` array + derived type); `catalog.ts` is `Record<ControlId, HelpEntry>` (a missing key fails `tsc`). `coverage.test.ts` fails if any control lacks a multi-paragraph, cited entry, asserts catalog↔ids bijection, and asserts AV 1106/07 (by report number) + TI 386 are cited.
- **D-24 content:** every entry is extensive, English-only, units-shown, band-identity-as-index+Hz, own-words prose citing Nord2000 AV 1106/07 (by report number) and NoizCalc TI 386 — never pasting the copyrighted standard (structural anti-paste guard in the coverage test, T-11-11-02).
- **Retrofit:** an InfoButton on every interactive control across Palette, ProjectBar, Inspector (+ wall/elevation/source/forest/ground fields), FacadePanel, ImportPanel, WeatherPanel, CalcPanel, SpectrumPanel, ColorScaleEditor, ConditioningPanel, ScenarioPanel, ExportMenu — additive only, no behaviour change.
- **Offline UAT:** `tests/e2e/infoButton.spec.ts` — real bundle, `page.route('**/api/*')` mocks + COOP/COEP, zero network egress: an InfoButton present on a sampled control in every panel, click → popover, More → docked panel with cited multi-paragraph help.

## Task Commits

1. **Task 1: InfoButton component + ControlId union + typed catalog** — `328fbb6` (feat)
2. **Task 2: help-coverage check + expand every entry to extensive help** — `3b3246c` (test)
3. **Task 3: retrofit Phase-7/8/9/10 panels** — `9cdb0aa` (feat)
4. **Task 4: retrofit Phase-11 panels + offline UAT + rebuild dist** — `929846b` (feat)

## Files Created/Modified

- `web/src/help/controlIds.ts` — the `ControlId` union / `CONTROL_IDS` enumeration (coverage source of truth)
- `web/src/help/catalog.ts` — `Record<ControlId, HelpEntry>` structured, standards-cited English help
- `web/src/help/InfoButton.tsx` — reusable affordance: portalled glance popover + docked help panel
- `web/src/help/InfoButton.test.ts` — SSR (react-dom/server) render + popover-content unit test
- `web/src/help/coverage.test.ts` — D-25 coverage backbone (fails on any gap; anti-paste guard)
- `web/tests/e2e/infoButton.spec.ts` — offline real-bundle Playwright UAT
- `web/src/icons.ts` — added the `info` glyph
- `web/src/panels/*` + `web/src/panels/fields/*` — InfoButton retrofit across all panels (additive)
- `web/dist/**` — rebuilt bundle

## Decisions Made

See `key-decisions` frontmatter. In short: react-dom/server unit test (no jsdom); portalled fixed-position popover (above the map stacking context); InfoButtons outside action buttons (no nested buttons); one help id per control TYPE for list controls.

## Deviations from Plan

**1. [Rule 3 - Blocking] InfoButton unit test rendered via `react-dom/server` instead of a jsdom DOM render**
- **Found during:** Task 1
- **Issue:** The plan's Task 1 acceptance ("a unit test renders `<InfoButton>` and asserts the popover + More → docked panel behavior") implied a DOM render, but the project's Vitest runs in the `node` environment with NO jsdom (a deliberate project choice documented in `vitest.config.ts`). Adding jsdom would contradict that architecture.
- **Fix:** Rendered the component + its popover with `react-dom/server` `renderToStaticMarkup` (needs no DOM) to assert the button/aria/testids and the popover's title + glance paragraph + citation + "More"; the full click→popover→"More"→docked-panel INTERACTION is asserted on the real bundle by the offline Playwright spec (the project's DOM-behaviour tier). Made `InfoButton` SSR/Node-safe (touches `document` only inside effects, portals only when open).
- **Files modified:** web/src/help/InfoButton.tsx, web/src/help/InfoButton.test.ts
- **Verification:** `vitest run help` green; `infoButton.spec.ts` asserts the interaction on the real bundle.
- **Committed in:** `328fbb6` (Task 1)

**2. [Rule 1 - Bug] Portalled the glance popover so "More" is not pointer-intercepted by the map**
- **Found during:** Task 4 (Playwright verification)
- **Issue:** An inline `position:absolute` popover in the right-rail overlapped the MapLibre map's stacking context; the popover was visible but its "More" button was intercepted by `.shell-body`, so the docked panel could not open.
- **Fix:** Portalled the popover to `document.body` with `position:fixed` anchored to the button's bounding rect (z-index 1000), and extended the outside-click detection to include the portalled popover.
- **Files modified:** web/src/help/InfoButton.tsx
- **Verification:** `infoButton.spec.ts` popover→More→dock passes; full Playwright suite green.
- **Committed in:** `929846b` (Task 4)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug).
**Impact on plan:** Both necessary for the acceptance criteria to hold on this codebase's tooling. No scope creep — every retrofit edit is additive.

## Issues Encountered

- The coverage test (Task 2) initially failed 17 single-paragraph entries (its purpose: enforcing D-24 "extensive"). Each was expanded to multi-paragraph — the test working as designed, not a defect.

## Verification

All commands run from `web/` unless noted:

- `npm run typecheck` (tsc --noEmit) — **green**
- `npx vitest run` — **158 → 158 unit tests pass** (incl. `help/coverage.test.ts` 89 assertions and `help/InfoButton.test.ts`)
- `npx vitest run coverage` — **green** (100% control coverage; a control without help fails)
- `npm run build:web` (tsc + vite build) — **green**, web/dist rebuilt
- `npx playwright test infoButton` — **2 passed** (offline, zero unmocked egress)
- `npx playwright test` (full suite) — **30 passed** (all prior specs green → no retrofit regression)
- `git diff` — **zero non-`web/` files changed**: Rust production code untouched, so `cargo clippy`/`cargo test` remain at their last green (Phase-10/11) baseline
- `cargo fmt --check` — **green**
- `cargo tree -p envi-engine` — **unchanged** (ndarray + num-complex + thiserror; the 3-dep quarantine holds)
- Wire no-drift: **holds trivially** — no Rust DTO or `web/src/generated/wire.ts` change

No verbatim AV 1106/07 text appears in the catalog (own-words prose; structural anti-paste guard + review).

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- Plan 11-11 complete; this is the LAST plan of Phase 11 (11/11).
- Phase 11 is code-complete. Per project CLAUDE.md, the five phase-completion gates (`/gsd-code-review 11`, `/simplify`, `/gsd-secure 11`, `/gsd-verify 11`, documentation-consistency scan) should be run before Phase 11 is marked fully done.

## Self-Check: PASSED

- Created files exist on disk: `web/src/help/{controlIds,catalog,InfoButton,InfoButton.test,coverage.test}.*`, `web/tests/e2e/infoButton.spec.ts`, `11-11-SUMMARY.md` — all FOUND.
- Task commits present in `git log`: `328fbb6` (Task 1), `3b3246c` (Task 2), `9cdb0aa` (Task 3), `929846b` (Task 4).
- All task `<acceptance_criteria>` re-verified: `tsc` green, `coverage.test.ts` green (100% control coverage), `infoButton.spec.ts` green (offline), full Playwright suite green (no regression), web/dist rebuilt.

---
*Phase: 11-results-fast-recalc*
*Completed: 2026-07-12*
