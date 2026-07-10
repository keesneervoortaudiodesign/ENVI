---
phase: 07-frontend-shell-scene-editing
plan: 08
subsystem: ui
tags: [react, zustand, ring-diff, uuid, spectrum-editor, band-index, ts-rs, wire-types, vitest, playwright, svc-07]

# Dependency graph
requires:
  - phase: 07-frontend-shell-scene-editing
    plan: 07
    provides: "sceneStore (D-03), Palette/Inspector + per-kind fields (SourceFields/WallFields slots), api/client.ts (interpolate/freq-axis wrappers), DEV test bridge"
  - phase: 07-frontend-shell-scene-editing
    plan: 04
    provides: "web/src/generated/wire.ts — the ts-rs mirror of the Rust wire DTOs (D-10) + the wire_no_drift test"
  - phase: 07-frontend-shell-scene-editing
    plan: 03
    provides: "POST /meta/interpolate-spectrum + GET /meta/freq-axis (the server-owned interpolation seam, D-05)"
provides:
  - "web/src/store/edges.ts — ringDiff(prevRing, prevEdgeIds, nextRing): the D-02 per-edge UUID recovery (IDENTITY/MOVE/INSERT/DELETE + rebuild fallback) with reconcileFacade; initEdgeIds"
  - "web/src/store/edges.test.ts — the load-bearing Vitest unit suite (8 tests) enumerating every case by name + no-silent-repoint; web/vitest.config.ts scopes unit runs to src/ (excludes Playwright specs)"
  - "web/src/spectrum/interpolateClient.ts — debounced server preview (POST /meta/interpolate-spectrum), useFreqAxis, band-INDEX anchor math, reprojectAuthored/materializeDense"
  - "web/src/spectrum/SpectrumEditor.tsx — curve (dB vs band index) + authored table editor with server preview + explicit promote-to-twelfth (WEB-10)"
  - "web/src/panels/FacadePanel.tsx — per-edge default/override list keyed by edge UUID (WEB-09)"
  - "SourceFields SPL-at-reference → server L_W back-calc; WallFields semi-transparent info/warn treatment (WEB-02/WEB-08)"
  - "envi-store::calibrate::spl_to_lw + POST /meta/spl-to-lw (SplToLwReq/Resp wire DTOs) — server-side free-field back-calc (SVC-07)"
  - "sceneStore: setSpectrum (authored-only, D-06), open/closeSpectrumEditor, reconcileBuildingEdges wired into applyTerraDrawChange"
affects: [07-09, 07-10]

# Tech tracking
tech-stack:
  added:
    - "vitest config (web/vitest.config.ts) scoping unit tests to src/, excluding tests/e2e Playwright specs"
  patterns:
    - "Per-edge UUID ring-diff (D-02, RESEARCH Pattern 5): geometry-based recovery keyed by exact-f64 coordinate identity — a vertex insert splits keeping the parent UUID on the first half + one fresh UUID on the second (both inherit the parent spectrum); delete merges keeping the first UUID; move/identity preserve positionally; unclassifiable deltas rebuild rather than silently re-point"
    - "TDD RED→GREEN on the highest-risk logic: a throwing stub + 8 named assertions committed first (RED), implementation second (GREEN)"
    - "Server-owned interpolation (D-05): the client debounces + does band-INDEX arithmetic only; the dense [105] grid and the SPL→L_W back-calc are Rust (SVC-07) — no client acoustic Hz/log math"
    - "Authored-only persistence (D-06): the store holds { resolution, values }; r_db[105] is never a second field; coarse→1/12 is an explicit visible promotion, never silent"
    - "New wire DTOs via ts-rs + regenerated committed wire.ts + extended no-drift export list (same D-10 discipline as the existing types)"

key-files:
  created:
    - web/src/store/edges.ts
    - web/src/store/edges.test.ts
    - web/vitest.config.ts
    - web/src/spectrum/interpolateClient.ts
    - web/src/spectrum/SpectrumEditor.tsx
    - web/src/panels/FacadePanel.tsx
    - web/tests/e2e/spectrum.spec.ts
    - crates/envi-store/src/calibrate.rs
  modified:
    - web/src/store/sceneStore.ts
    - web/src/api/client.ts
    - web/src/panels/Inspector.tsx
    - web/src/panels/fields/SourceFields.tsx
    - web/src/testBridge.ts
    - web/src/App.tsx
    - web/src/app.css
    - web/src/generated/wire.ts
    - crates/envi-store/src/lib.rs
    - crates/envi-service/src/api/meta.rs
    - crates/envi-service/src/api/mod.rs
    - crates/envi-service/tests/wire_no_drift.rs
    - web/dist

key-decisions:
  - "ringDiff keeps the PARENT UUID on the FIRST split half + one fresh UUID on the second (RESEARCH Assumption A5); both halves inherit the parent spectrum. A same-vertex-count change is treated positionally (MOVE/IDENTITY); an insert/delete is a single-vertex diff located by exact-coordinate probe; any other delta falls back to a fresh rebuild that drops overrides rather than re-point them — the fail-safe direction."
  - "reconcileFacade is returned as a generic mapping instruction (a closure over the diff), so the store applies it to the subset of the shared spectra channel keyed by the building's prev edge UUIDs — edge spectra live in store.spectra keyed by edge UUID (never in TD props, D-03), and the building default lives under the building's own feature id."
  - "Resolution switching re-projects via the server (materialize dense → sample the new anchor indices), so it is non-destructive without any client interpolation; switch-to-1/12 from a coarse spectrum IS the explicit promotion event (visible .chip.info notice), satisfying D-06's 'never silently discard coarse values'."
  - "SPL→L_W is a genuinely-wired server endpoint (POST /meta/spl-to-lw over envi-store::calibrate::spl_to_lw), NOT a client calc and NOT a mock stub — free-field spherical spreading L_W=L_p+20log10(r)+10log10(4π) lives in the store beside the interpolation core, honoring SVC-07 and 'objects must never be silently inert'."

patterns-established:
  - "Pattern: the highest-risk data-integrity logic (D-02 façade re-pointing) is made structurally safe by UUID keys + a deterministic ring-diff AND guarded by dedicated named unit tests run by a real runner (Vitest), with the verify command carrying NO 2>/dev/null and NO || fallback."

requirements-completed: [WEB-02, WEB-08, WEB-09, WEB-10, SCN-01, SCN-02, SCN-03]

# Metrics
duration: 21min
completed: 2026-07-10
status: complete
---

# Phase 07 Plan 08: Acoustic Authoring Surface (SC3) Summary

**The SC3 acoustic-authoring surface is live: a building's per-façade isolation spectra are keyed by stable per-edge UUIDs and reconciled through the load-bearing D-02 ring-diff (`web/src/store/edges.ts`) — a vertex insert splits an edge keeping the parent UUID on the first half plus one fresh UUID on the second (both inheriting the parent spectrum), a delete merges keeping the first UUID, and a move/identity preserves every UUID — so a façade override can never silently re-point after an unrelated edit (8 named Vitest assertions, IDENTITY/MOVE/INSERT/DELETE + no-repoint, run with no masking); the isolation-spectrum editor (`SpectrumEditor.tsx`) plots dB vs BAND INDEX with a server-owned interpolation preview (`POST /meta/interpolate-spectrum`), lands authored anchors on the exact octave/third band indices, and PROMOTES an octave/third spectrum to authored@twelfth explicitly (visible notice, never discarding coarse values); a wall/screen carries the semi-transparent info-vs-warn treatment; and source SPL-at-reference derives L_W SERVER-side via a genuinely-wired `POST /meta/spl-to-lw` (free-field spherical spreading in `envi-store::calibrate`) — all with `envi-engine` byte-identical, `cargo test` green, and 3 fully-offline Playwright specs plus the unit suite passing.**

## Performance
- **Duration:** ~21 min (19:15Z → 19:36Z active)
- **Tasks:** 3 (Task 1 TDD: RED + GREEN commits), 5 commits total, each atomic
- **Files:** 8 created, 12 modified (incl. regenerated wire.ts + rebuilt web/dist)

## Accomplishments

### Task 1 — D-02 per-edge UUID ring-diff, TDD (commits bbed4db RED, 0eb136c GREEN)
`web/src/store/edges.ts` `ringDiff(prevRing, prevEdgeIds, nextRing)` classifies the geometry delta by exact-f64 coordinate identity (RESEARCH Pattern 5): **same vertex count** → positional edge-id preservation (IDENTITY when coords are byte-identical, else MOVE); **+1 vertex** → INSERT, located by the single-vertex removal probe, splitting the parent edge with the parent UUID on the first half + one fresh UUID on the second and copying the parent spectrum onto the second via `reconcileFacade`; **−1 vertex** → DELETE, merging the two adjacent edges keeping the first edge's UUID and dropping the second's spectrum; **any other delta** → a fail-safe `rebuild` (fresh UUIDs, overrides dropped). `initEdgeIds` mints one UUID per footprint edge. `web/src/store/edges.test.ts` (Vitest) asserts all four cases BY NAME plus the no-silent-repoint invariant (an override on edge C→D survives an insert on A→B, keeping its UUID, spectrum, and C→D segment) — 8 tests, RED-first against a throwing stub, GREEN after. `web/vitest.config.ts` scopes `npm run test:unit` to `src/**/*.test.ts` and excludes the Playwright `tests/e2e/*.spec.ts` (which import a different runner). The store's `applyTerraDrawChange` calls `reconcileBuildingEdges` (the SOLE `ringDiff` call site — grep gate holds: not in any Terra Draw callback); buildings seed `edge_ids` at draw time; `StoredSpectrum` is now `AuthoredSpectrumDto` (imported from generated/wire).

### Task 2 — isolation-spectrum editor (commit d0a0f74)
`spectrum/interpolateClient.ts`: a ~250 ms-debounced `POST /meta/interpolate-spectrum` preview (aborting superseded calls), `useFreqAxis` (the axis fetched once from `GET /meta/freq-axis`, never hardcoded), the band-INDEX anchor helpers (`anchorIndices` → octave 4+12k, third 4k, twelfth 0..104), and `reprojectAuthored`/`materializeDense`. `spectrum/SpectrumEditor.tsx`: a centered overlay with a fixed-height inline-SVG curve (105 preview points as dB vs band index 0..104, X ticks at the 1/3-oct centre indices showing nominal Hz display-only, `--color-primary` preview line, `--color-text-strong` authored anchors) + a scrolling `.mono .dense` table (band index · nominal Hz · editable dB). The 3-way `.switch` re-projects non-destructively via the server; switching a coarse spectrum to 1/12 sets `authored = { resolution: "twelfth", … }` and shows the `.chip.info` promotion notice (D-06 — coarse values promoted, not discarded). The store persists ONLY `authored`; `r_db[105]` is never a second field. Error state renders the server `detail` as text (no innerHTML). Mounted from `App.tsx` via `openSpectrumEditor`/`closeSpectrumEditor`.

### Task 3 — façade panel + semi-transparent screen + SPL calibration + E2E (commit c22516c, fmt ad18f53)
`panels/FacadePanel.tsx` (the building inspector body) lists one row per `edge_id` (short UUID `.mono`, `.chip.info "OVERRIDE"` vs `.chip.off "DEFAULT"` from the store's `spectra` channel) above a "Building default" row; each Edit opens the editor for that edge UUID (or the building id). `WallFields` drives the semi-transparent treatment state — `info` (acoustic screen, with a spectrum) vs `warn` (no spectrum) — and opens the editor for the wall id. `SourceFields` adds the optional SPL-at-reference (reference-distance input + "Derive L_W") that materializes the authored spectrum via the server then calls `POST /meta/spl-to-lw`, storing the result as authored@twelfth — **no client Hz/log arithmetic** (grep-clean). Backend: `envi-store::calibrate::spl_to_lw` (free-field `L_W = L_p + 20·log10(r) + 10·log10(4π)`, length/finiteness/positivity gated) + a thin `POST /meta/spl-to-lw` handler + `SplToLwReq`/`SplToLwResp` ts-rs DTOs (wire.ts regenerated, no-drift export list extended). `tests/e2e/spectrum.spec.ts` (fully offline) proves the screen info/warn state, the octave anchors landing on exact indices with a server preview, the explicit promote-to-twelfth, and the façade override surviving an insert elsewhere (D-02 integration) — driven via extended DEV bridge helpers.

## Deviations from Plan

### Auto-fixed / auto-added

**1. [Rule 3 — Blocking] web/vitest.config.ts created (not in the plan file list)**
- **Found during:** Task 1 — `npm run test:unit` (`vitest run`) picked up the Playwright `tests/e2e/*.spec.ts` (Vitest's default glob matches `*.spec.ts`), which import `@playwright/test` and failed the unit gate (4 files failed instead of 1).
- **Fix:** Added `vitest.config.ts` including only `src/**/*.test.ts` and excluding `tests/e2e/**`. Without it the verify command could not be green. Correct and minimal.

**2. [Rule 2 — Missing critical functionality] server-side SPL→L_W endpoint added in envi-store/envi-service (Rust, not in the web-only file list)**
- **Issue:** The plan requires SPL-at-reference to derive L_W SERVER-side (must_have + SVC-07), but no such endpoint existed. Computing it client-side violates SVC-07; a mock-only path would be "silently inert" (forbidden by the phase's core principle).
- **Fix:** Added `envi-store::calibrate::spl_to_lw` (the free-field back-calc, beside the interpolation core) + a thin `POST /meta/spl-to-lw` handler + `SplToLwReq`/`SplToLwResp` ts-rs DTOs, following the EXACT `interpolate-spectrum` pattern (thin handler, store owns the math, engine untouched). Regenerated the committed `wire.ts` and extended the no-drift export list; `cargo test`/clippy/fmt green. This is additive (non-breaking), not architectural — the same axum meta module, same DTO-mirror discipline.
- **Files:** crates/envi-store/src/{calibrate.rs,lib.rs}, crates/envi-service/src/api/{meta.rs,mod.rs}, crates/envi-service/tests/wire_no_drift.rs, web/src/generated/wire.ts, web/src/api/client.ts. **Commits:** c22516c/ad18f53.

**3. [Rule 3 — Integration] App.tsx (editor mount) + testBridge.ts (E2E observability) edited (not in the plan file list)**
- **Issue:** The centered editor overlay must mount somewhere (the shell owns overlays), and the offline E2E needs to drive façade overrides + geometry inserts (the plan permits programmatic store access). Neither file is named in the plan.
- **Fix:** `App.tsx` renders `<SpectrumEditor>` when `spectrumEditor != null`; `testBridge.ts` gained `buildingEdges`/`setSpectrum`/`spectrum`/`applyBuildingRing`/`edgeSegment` (DEV-only, statically dropped from the prod bundle). Required integration/observability, same posture as 07-07.
- **Files:** web/src/App.tsx, web/src/testBridge.ts. **Commits:** d0a0f74/c22516c.

**Total:** 3 deviations (1 blocking config, 1 required server endpoint to avoid inert-stub, 1 integration/observability). No package installs. The server endpoint is additive, not a Rule-4 architectural change.

## Verification
- `cd web && npx tsc --noEmit` — clean; `npm run build` — green (prod bundle rebuilt; `web/dist` committed).
- `npm run test:unit` — **8 passed** (ring-diff IDENTITY/MOVE/INSERT/DELETE + no-repoint + rebuild + initEdgeIds), NO `2>/dev/null`, NO `|| fallback`.
- `npx playwright test` — **7 passed** (lifecycle + draw-kinds + 2× dgm-trigger + 3× spectrum), fully offline (zero unmocked network requests asserted).
- Grep gates: `ringDiff` only in `edges.ts` + `sceneStore.applyTerraDrawChange` (not in any TD callback); `dangerouslySetInnerHTML` = 0 actual usages (comments only); no `r_db` persisted in the store; no Hz/log arithmetic in `SourceFields.tsx`.
- Rust: `git diff --quiet crates/envi-engine/` — **byte-identical**; `cargo fmt --check` clean; `cargo clippy -p envi-store -p envi-service --all-targets -- -D warnings` clean; full `cargo test` — green (incl. `wire_no_drift` byte-match, `calibrate` unit tests, `contract_meta_static` serving the rebuilt web/dist). metrao3 untouched.

## Known Stubs (documented deferrals — not silent inertness)
- **The semi-transparent screen MAP paint** (double-stroke `--color-info` / `--color-warn` line) is represented by the inspector treatment STATE (`data-treatment` info/warn chip); the actual Terra Draw line paint keyed off that state, plus the 07-09 validation-panel `warn` row, land in **07-09**. The state the paint reads is real and E2E-asserted.
- **Source L_W ↔ SourceDto.spectrum.band_db persistence** — the authored L_W lives in the store's `spectra` channel keyed by source id; mapping it into the typed `SubSourceDto` at solve time is **Phase 9/10** (the whole-scene PUT is still raw GeoJSON features, per the 07-07 deferral).
- **default_isolation / facade overrides → engine `IsolationSpectrum`** — authored here + convertible via the D-01 `TryFrom`; solve-time attachment is **Phase 9/10**.
- **Curve non-anchor click-to-edit** (the second promotion trigger in the UI-SPEC) is not shipped; the switch-to-1/12 trigger fully satisfies the explicit-promotion contract (D-06) and is the one E2E-asserted.

## Threat Coverage
| Threat ID | Mitigation shipped |
|-----------|--------------------|
| T-07-08-01 (façade spectrum re-pointing after a vertex edit) | UUID-keyed `facade` spectra + the deterministic `ringDiff` recovery with 8 dedicated unit tests incl. the explicit no-silent-repoint case; silent corruption is structurally impossible |
| T-07-08-02 (interpolate `detail` XSS) | The editor error state renders the server `detail` as a React text child only; no innerHTML (grep-clean) |
| T-07-08-03 (client-side acoustic Hz math) | Interpolation AND SPL→L_W are server-side (SVC-07); the client debounces + does band-INDEX arithmetic only — no Hz/log math in the frontend (grep-clean) |
| T-07-08-04 (105-band spectrum in TD feature props) | Spectra live in the store's `spectra` channel keyed by feature/edge UUID (D-03); only `edge_ids` (small metadata) sits in properties, never the 105-band values |

## Self-Check: PASSED
All 8 created files exist on disk; all 5 task commits (bbed4db, 0eb136c, d0a0f74, c22516c, ad18f53) are present in the history.
