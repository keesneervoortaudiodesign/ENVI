---
phase: 07-frontend-shell-scene-editing
plan: 09
subsystem: ui
tags: [react, zustand, turf, terra-draw, autosave, validation, delete-dialog, playwright, d-07, d-04]

# Dependency graph
requires:
  - phase: 07-frontend-shell-scene-editing
    plan: 07
    provides: "sceneStore (D-03), Palette/Inspector, dgm slice rejectReason (crit source), api/client.ts, DEV test bridge, commitFeature/tagCreatedFeature/updateProperties"
  - phase: 07-frontend-shell-scene-editing
    plan: 08
    provides: "semi-transparent wall/screen treatment state (info/warn), setSpectrum, façade edge UUID ring-diff"
  - phase: 07-frontend-shell-scene-editing
    plan: 03
    provides: "POST /dgm/triangulate (4xx interior-crossing reject) + PUT /projects/{id}/scene persistence"
provides:
  - "web/src/validate/groundZone.ts — classifyGroundZone (turf booleanOverlap/booleanContains): ok/contained(allowed)/partial-cross(reject) draw-time topology (D-07)"
  - "web/src/store/sceneStore.ts extensions: commitGroundZoneCandidate (hard-reject revert + transient groundReject), commitEpoch (committed-edit counter OFF the change/drag path), zoomRequest + zoomToFeature/selectAndZoom, setProject/resetProject, projectName"
  - "web/src/panels/RejectBanner.tsx — transient map-anchored crit banner + zoom-to-conflicting-zone (Surface B)"
  - "web/src/panels/ValidationPanel.tsx — persistent click-to-select+zoom panel (Surface A): warn (wall no-spectrum, forest zero-density) + crit (dgm rejectReason) rows"
  - "web/src/store/autosave.ts — scheduleAutosave (750ms debounce) keyed off commitEpoch, coalesced whole-scene PUT, beforeunload/pagehide flush, saveNow; useAutosaveStore indicator state (D-04)"
  - "web/src/panels/ProjectBar.tsx — .conn dirty/saving/saved/failed indicator + ⋯ overflow menu"
  - "web/src/panels/DeleteProjectDialog.tsx — typed-name confirmation gate (danger disabled until exact match); api/client deleteProject (DELETE /projects/{id})"
  - "web/src/map/MapCanvas.tsx ZoomController — store-canonical zoom-to-fit (fitBounds off the panel/banner components)"
affects: [07-10]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Draw-time hard reject (D-07): a partial-cross ground_zone is reverted in the store commit path (never enters features) and raised as a transient groundReject; the Terra Draw path additionally removeFeatures() the reverted id. Containment (turf booleanContains either direction) is allowed, innermost wins."
    - "commitEpoch discipline (D-04): a committed-edit counter bumped ONLY by committed mutations (tag/commit feature, noteCommit on TD finish, property change, spectrum edit, accepted ground_zone) and NEVER by applyTerraDrawChange — so autosave subscribes to commitEpoch (not the dirty flag) and can never fire per drag frame. Same 'decoupled-from-the-raw-change-path' posture as 07-07's dgmTrigger."
    - "Store-canonical zoom (zoomRequest + a ZoomController inside <Map>): the validation panel + reject banner set a zoom target on the store; only the map component touches fitBounds — the panels never hold a map ref."
    - "Two DISTINCT validation surfaces kept structurally apart: Surface B (transient RejectBanner, never a panel row) vs Surface A (persistent ValidationPanel of non-geometric issues on objects that exist)."
    - "Typed-name destructive gate: the danger button is disabled until the typed text === the store's canonical project name (client-side compare, no server call); focus opens on Cancel, never the danger button."

key-files:
  created:
    - web/src/validate/groundZone.ts
    - web/src/panels/RejectBanner.tsx
    - web/src/panels/ValidationPanel.tsx
    - web/src/store/autosave.ts
    - web/src/panels/ProjectBar.tsx
    - web/src/panels/DeleteProjectDialog.tsx
    - web/tests/e2e/validation.spec.ts
  modified:
    - web/src/store/sceneStore.ts
    - web/src/map/MapCanvas.tsx
    - web/src/map/useTerraDraw.ts
    - web/src/api/client.ts
    - web/src/App.tsx
    - web/src/app.css
    - web/src/testBridge.ts
    - web/dist

key-decisions:
  - "classifyGroundZone builds turf Feature<Polygon> via turf's polygon() (throws → null, skipped defensively) and returns the FIRST partial-cross's conflictId; a containment relationship in either direction (booleanContains) is `contained` (allowed). booleanOverlap is false for containment/equal polygons, so innermost-wins never mis-rejects."
  - "Autosave keys off a NEW commitEpoch counter rather than the pre-existing dirty flag, because dirty is set by the raw applyTerraDrawChange (change/drag) path — subscribing to dirty would PUT per drag frame, violating D-04. TD `finish` now calls noteCommit (bumps epoch), so a released vertex drag on an existing feature still autosaves exactly once."
  - "The reject banner's 'Zoom to conflicting zone' targets the EXISTING crossed zone (the rejected candidate has no object to select) via the same store zoomRequest the validation-panel rows use; a ZoomController inside <Map> is the single fitBounds call site (a DOM `zoom-target` readout makes the store-canonical zoom E2E-observable without pixel inspection)."
  - "Delete uses a new client deleteProject (DELETE /projects/{id}); on success the store resetProject() clears the scene + project identity to route to the empty/no-project state. The server `detail` on a failed delete renders as a React text child only."

patterns-established:
  - "Pattern: a destructive-edit invariant (always-valid ground-zone topology) is enforced at the draw-commit boundary in the store, not by post-hoc validation — a partial cross can never enter the canonical FeatureCollection."

requirements-completed: [WEB-01, WEB-04]

# Metrics
duration: 42min
completed: 2026-07-10
status: complete
---

# Phase 07 Plan 09: Scene Integrity & Persistence (SC2 + SC4) Summary

**Ground-zone topology is now always valid via a draw-time HARD REJECT (D-07): a partially-crossing ground_zone is classified by turf (`booleanOverlap`/`booleanContains`), reverted in the store commit path so it never enters the canonical FeatureCollection, and surfaced as a TRANSIENT map-anchored crit banner whose "Zoom to conflicting zone" targets the EXISTING crossed zone — while containment commits (innermost wins); non-geometric issues on objects that exist surface in a SEPARATE persistent click-to-select validation panel (a semi-transparent wall without a spectrum → warn, a zero-density forest → warn, an interior-crossing elevation breakline → crit, sourced from 07-07's dgm `rejectReason`), each row selecting + zoom-to-fitting its object; committed edits autosave through a 750 ms debounce keyed off a NEW `commitEpoch` counter that is structurally OFF the Terra Draw change/drag path (never a per-frame PUT), coalesced into one whole-scene `PUT /scene` with a flush-on-unload and a dirty/saving/saved project-bar indicator; and project deletion is guarded by a typed-name confirmation whose danger button stays disabled until the name matches exactly — all proven by a fully-offline Playwright spec (4 tests) plus the whole 11-test suite green, `envi-engine` byte-identical, and `cargo test` green.**

## Performance
- **Duration:** ~42 min
- **Tasks:** 4, each committed atomically
- **Files:** 7 created, 8 modified (incl. rebuilt web/dist)

## Accomplishments

### Task 1 — Draw-time ground-zone hard reject + transient banner (commit c8026ba)
`validate/groundZone.ts` `classifyGroundZone(candidate, existingZones)` uses turf `booleanOverlap` (partial cross → reject, carrying the FIRST crossed zone's id) and `booleanContains` (either direction → `contained`, allowed) — otherwise `ok`; a non-polygon candidate or a degenerate ring classifies safely (`ok`/skip) rather than throwing. The store's `commitGroundZoneCandidate(id)` runs this on the just-upserted (still-untagged) candidate: a `partial-cross` DELETES the candidate geometry from `features` (revert to last-valid — it never commits) and raises the transient `groundReject { conflictId, nonce }`; `ok`/`contained` tags + selects + clears any stale reject and bumps `commitEpoch`. `useTerraDraw` routes a newly-drawn ground_zone through this path and `removeFeatures()` the reverted id from Terra Draw's own view. `RejectBanner.tsx` is the map-anchored top-center crit banner (auto-dismiss ~6 s, timer reset per nonce, torn down on unmount) with a "Zoom to conflicting zone" action → `zoomToFeature(conflictId)`; a `ZoomController` inside `<Map>` is the sole `fitBounds` call site (store-canonical zoom).

### Task 2 — Persistent validation panel (commit 187411f)
`ValidationPanel.tsx` (Surface A) derives its rows live from the store (no spinner): a `wall` with `semi_transparent === true` and no `spectra[id]` → `.chip.warn`; a `forest` with `density_per_m2 === 0` → `.chip.warn`; the dgm slice's `rejectReason` (07-07's `dgmTrigger` producer, `POST /dgm/triangulate` 4xx) → one `.chip.crit` row with the `.dot.crit` pulse whose text is the stored `detail`. Each row click calls `selectAndZoom(targetId)` — selecting the object (which opens the inspector) and setting the zoomRequest the `ZoomController` fits. The transient reject banner is NEVER a row here. All descriptions are React text children.

### Task 3 — Debounced autosave + dirty indicator + Delete dialog (commit b049cb0)
`store/autosave.ts` `scheduleAutosave()` debounces 750 ms and coalesces into one `sceneStore.saveScene()` (whole-scene PUT, no PATCH); `useAutosave()` subscribes to `commitEpoch` (the committed-edit counter — NOT the dirty flag, so it is structurally off the change/drag path, D-04) and registers `beforeunload`/`pagehide` flush (torn down on unmount). `useAutosaveStore` exposes `status` (idle/dirty/saving/saved/error) + `savedAt` for the indicator; `saveNow()` backs the explicit Save. `ProjectBar.tsx` renders the `.conn`-pattern indicator (Dirty=`--color-warn` "Unsaved" · Saving=`--color-primary` · Saved=`--color-ok` "Saved · hh:mm" mono clock · failed=`--color-crit`) as a TEXT label with no `.dot` pulse (dirty-overload mitigation), plus the `⋯` overflow menu → "Delete project…". `DeleteProjectDialog.tsx` is the typed-name modal: `.section-title` + `.chip.crit "IRREVERSIBLE"`, body naming the on-disk deletion (project name via a text child), a name-match `.input` gating the 44px `.btn.danger` (disabled until exact match), Cancel (default focus, Esc cancels), states idle/deleting/error(server `detail` as text)/success. `api/client.ts` gains `deleteProject` (`DELETE /projects/{id}`).

### Task 4 — Validation + persistence E2E (commit 1f5f4a7)
`tests/e2e/validation.spec.ts` (fully offline): (1) a contained ground_zone commits (`contained`) and a partial cross reverts (`partial-cross`, no id) + banners + zooms to the EXISTING zone A (`zoom-target` === A.id) — and asserts the banner text is NOT a validation-panel row; (2) a semi-transparent wall without a spectrum + a zero-density forest each produce a `.chip.warn` row, and three elevation points + two crossing elevation_lines drive the mocked triangulate 400 into exactly one `.chip.crit` row, with warn/crit row clicks each selecting + zooming the offending object; (3) one committed edit fires EXACTLY ONE coalesced PUT after the debounce with the indicator transitioning `dirty → saved`; (4) the delete danger button enables only on an exact typed-name match, focus opens on Cancel, and a mocked DELETE success routes the bar to "No project". `testBridge.ts` gains `commitGroundZone` / `update` / `openProject` helpers.

## Deviations from Plan

### Auto-fixed / auto-added

**1. [Rule 3 — Integration] Files edited beyond the plan's file list (required wiring)**
- **Issue:** The plan's file list named the new components + `sceneStore.ts` + `ProjectBar.tsx`, but not the integration seams they need: `web/src/map/MapCanvas.tsx` (the `ZoomController` + `zoom-target` readout), `web/src/map/useTerraDraw.ts` (routing a drawn ground_zone through `commitGroundZoneCandidate` + `finish`→`noteCommit`), `web/src/App.tsx` (mounting `<ProjectBar>`/`<ValidationPanel>`/`<RejectBanner>` + `useAutosave()`), `web/src/api/client.ts` (`deleteProject`), and `web/src/testBridge.ts` (E2E helpers).
- **Fix:** Made the minimal integration edits. `App.tsx` now composes the new panels and mounts autosave; `useTerraDraw` enforces the D-07 reject at the draw-commit boundary; `MapCanvas` owns the single `fitBounds`.
- **Files:** web/src/map/MapCanvas.tsx, web/src/map/useTerraDraw.ts, web/src/App.tsx, web/src/api/client.ts, web/src/testBridge.ts. **Commits:** c8026ba/187411f/b049cb0/1f5f4a7.

**2. [Rule 2 — Missing critical functionality] `commitEpoch` committed-edit counter + `deleteProject` client**
- **Issue:** D-04 requires autosave to fire on committed edits but NOT on drag frames. The pre-existing `dirty` flag is set by the raw `applyTerraDrawChange` (change/drag) path, so keying autosave off it would PUT per frame. A `commitEpoch` counter (bumped only by committed mutations) makes the discipline structural. Separately, the Delete dialog needs a real DELETE endpoint client (none existed).
- **Fix:** Added `commitEpoch` (+ `noteCommit`, called from TD `finish`) to the store and subscribed autosave to it; added `deleteProject` (`DELETE /projects/{id}`) to the client. Both are additive, not architectural.
- **Files:** web/src/store/sceneStore.ts, web/src/store/autosave.ts, web/src/api/client.ts. **Commits:** c8026ba/b049cb0.

**3. [Rule 2 — Test observability] Store-canonical zoom + a `zoom-target` DOM readout**
- **Issue:** "click-to-select + zoom" and "zoom to conflicting zone" are not DOM-observable if the panels call `fitBounds` directly (and the panels/banner live outside `<Map>`, with no map ref). A `zoomRequest` in the store + a `ZoomController` inside `<Map>` decouples them; a `zoom-target` readout lets the offline E2E assert the zoom target without pixel inspection (the same posture as the 07-06 Gate-1 readouts).
- **Files:** web/src/store/sceneStore.ts, web/src/map/MapCanvas.tsx. **Commit:** c8026ba.

**Total:** 3 deviations (integration wiring, 2 additive store/client mechanisms, test observability). No package installs (`@turf/turf` landed in 07-05). No architectural (Rule-4) changes.

## Verification
- `cd web && npx tsc --noEmit` — clean; `npm run build` — green (prod `web/dist` rebuilt + committed).
- `npm run test:unit` — **8 passed** (the 07-08 ring-diff suite, unaffected).
- `npx playwright test` — **11 passed** (lifecycle + draw-kinds + 2× dgm-trigger + 3× spectrum + 4× validation), fully offline (every spec asserts zero unmocked external requests).
- Grep gates: `scheduleAutosave` is called ONLY from the autosave commit-epoch subscription — never in `useTerraDraw` (grep-clean, off the change/drag path); `dangerouslySetInnerHTML` = 0 usages (a comment in icons.ts only); `booleanOverlap` present in `validate/groundZone.ts`.
- Rust: `git diff --quiet crates/envi-engine/` — **byte-identical**; full `cargo test` — green (incl. `contract_meta_static` serving the rebuilt `web/dist`). metrao3 untouched.

## Known Stubs (documented deferrals — not silent inertness)
- **Open / New project buttons** in `ProjectBar` are placeholders — real project open/create + `projectName` population is **07-10**. The delete dialog + tests set identity via the `setProject` seam (`openProject` bridge helper); the delete PUT/DELETE paths themselves are real and E2E-proven.
- **Flush-on-unload is a best-effort `fetch`** (not `sendBeacon`/`keepalive`) — acceptable for a localhost single-user tool; a keepalive upgrade is a later hardening if a remote deployment ever lands. The listener registration + teardown are asserted by the plan's grep gate.
- **Building per-EDGE semi-transparent warn** (a footprint edge marked semi-transparent without a spectrum → per-edge warn stroke, UI-SPEC) is not surfaced — the store carries no per-edge `semi_transparent` flag yet; the panel covers the `wall` kind. Façade OVERRIDE presence (07-08) is unaffected. Deferred to when per-edge screen state lands.
- **The 07-06 Gate-1 map controls + readouts** (switch-basemap / add-test-point + the store/TD/instance/zoom/dgm readouts) remain in `MapCanvas` — superseded when the project/basemap chrome finalises in **07-10**.

## Threat Coverage
| Threat ID | Mitigation shipped |
|-----------|--------------------|
| T-07-09-01 (invalid ground-zone topology reaching the store) | Draw-time hard reject in `commitGroundZoneCandidate` — a partial cross reverts (deletes the candidate) and never enters `features`; topology invariant always valid. Server PUT validation remains the backstop. |
| T-07-09-02 (PUT-per-drag-frame flooding) | Autosave keys off `commitEpoch` (committed edits only), 750 ms debounce, coalesced whole-scene PUT — structurally off the `applyTerraDrawChange` change/drag path (E2E: one committed edit → exactly one PUT). |
| T-07-09-03 (XSS via save/delete `detail`, project name) | All strings via React text children (indicator label, dialog body/name, `form-error` detail); no `dangerouslySetInnerHTML` (grep-clean); the delete gate compares the name client-side. |
| T-07-09-04 (accidental irreversible delete) | Typed-name confirmation; danger button disabled until an exact match; focus opens on Cancel, never the danger button; double-submit blocked while deleting. |

## Self-Check: PASSED
