---
phase: 07-frontend-shell-scene-editing
plan: 07
subsystem: ui
tags: [react, terra-draw, zustand, discriminated-union, wire-types, dgm, inheritance, palette, inspector, playwright]

# Dependency graph
requires:
  - phase: 07-frontend-shell-scene-editing
    plan: 06
    provides: "sceneStore (canonical D-03 store), useTerraDraw lifecycle hook (instance-in-ref, rebuild-on-style.load), MapCanvas, _mocks.ts offline harness"
  - phase: 07-frontend-shell-scene-editing
    plan: 04
    provides: "web/src/generated/wire.ts — the committed TS mirror of the Rust wire DTOs (D-10)"
  - phase: 07-frontend-shell-scene-editing
    plan: 03
    provides: "POST /api/v1/dgm/triangulate + /meta/interpolate-spectrum endpoints (DgmReq/DgmResp shape)"
provides:
  - "web/src/draw/kinds.ts — the 9-kind discriminated union + per-kind TD-mode/icon/hue metadata; Record<Kind,_> + assertNeverKind give the D-09 exhaustiveness guard"
  - "web/src/draw/modes.ts — kind → Terra Draw geometry mode mapping (tdModeName) + buildModes"
  - "web/src/panels/Palette.tsx — object palette (select/pan + 9 kinds), store-driven activeTool"
  - "web/src/panels/Inspector.tsx + fields/{GroundZoneFields,ForestFields,SourceFields}.tsx — per-kind property inspector (closed-enum A–H/N-S-M-L selects, forest numerics, source position + spectrum slot)"
  - "web/src/store/inheritance.ts — per-kind session-scoped last-object inheritance (lastOf/seedProps + KIND_DEFAULTS, WEB-04)"
  - "web/src/api/client.ts — typed fetch client importing request/response types from generated/wire (D-10)"
  - "web/src/dgm/dgmTrigger.ts + store/dgm.ts + map/DgmOverlay.tsx — the SC1 DGM re-triangulation producer (debounced POST /dgm/triangulate → TIN overlay; 4xx → rejectReason)"
  - "sceneStore extensions: activeTool, commitFeature/tagCreatedFeature (kind tag + inheritance), updateProperties, sceneFeatureCollection/saveScene"
affects: [07-08, 07-09, 07-10]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "9-kind discriminated union with Record<Kind,_> + assertNeverKind: a dropped case fails tsc (D-09 exhaustiveness, structural half + switch half)"
    - "closed-enum <select> for impedance A–H / roughness N/S/M/L: an out-of-vocabulary value is structurally impossible client-side (T-07-07-02)"
    - "fetch client imports GENERATED wire types (D-10); no hand-declared DTO — cannot drift"
    - "per-kind session-scoped last-object inheritance with per-field inherited/default seed chips that clear on edit (WEB-04)"
    - "debounced committed-edit producer decoupled from the raw TD change/drag path (D-04 discipline): store-subscription + 750ms debounce coalesces drag frames into ONE POST /dgm/triangulate"
    - "DEV-only window bridge (import.meta.env.DEV, statically dropped from the prod bundle) so the offline E2E drives programmatic store commits"

key-files:
  created:
    - web/src/draw/kinds.ts
    - web/src/draw/modes.ts
    - web/src/panels/Palette.tsx
    - web/src/panels/Inspector.tsx
    - web/src/panels/fields/GroundZoneFields.tsx
    - web/src/panels/fields/ForestFields.tsx
    - web/src/panels/fields/SourceFields.tsx
    - web/src/store/inheritance.ts
    - web/src/store/dgm.ts
    - web/src/api/client.ts
    - web/src/dgm/dgmTrigger.ts
    - web/src/map/DgmOverlay.tsx
    - web/src/testBridge.ts
    - web/src/vite-env.d.ts
    - web/tests/e2e/draw-kinds.spec.ts
    - web/tests/e2e/dgm-trigger.spec.ts
    - web/tests/e2e/env.d.ts
  modified:
    - web/src/store/sceneStore.ts
    - web/src/map/useTerraDraw.ts
    - web/src/map/MapCanvas.tsx
    - web/src/App.tsx
    - web/src/main.tsx
    - web/src/app.css
    - web/tests/e2e/_mocks.ts
    - web/dist

key-decisions:
  - "The palette tool lives in the canonical store (activeTool) so Palette (setActiveTool) and useTerraDraw (setMode + kind-tagging on create) share one source of truth without prop-drilling across the map/panel component trees."
  - "A drawn feature is tagged with the active kind by detecting 'not previously in the store' in the TD change handler (robust to TD's `type` string), then merging properties.kind + seeded inheritance — the same commitFeature path the DEV bridge uses for programmatic placement."
  - "ground_zone impedance/roughness are stored as class LETTERS in feature properties (impedance_class/roughness_class), matching the frozen contract-test fixtures; the class→σ number resolution is a later (Phase 9) concern. The σ shown in the select is display-only (mirrors envi_engine::scene::impedance_class; class B = 31.5)."
  - "The DGM producer is decoupled from Terra Draw's raw change/drag path: it subscribes to an elevation-set signature on the store and debounces 750ms, so drag frames coalesce into exactly one POST (proven in dgm-trigger.spec: 3 rapid commits → 1 request). The grep gate (no triangulate in useTerraDraw) holds."
  - "The whole-scene Save is wired to an explicit button (store.saveScene → putScene) against a placeholder projectId; project open/create + debounced autosave land in 07-10/07-09. This is the minimum needed to satisfy the SC1 'PUT payload' assertion, not the full autosave (D-04)."

patterns-established:
  - "Pattern: exhaustiveness via Record<Kind,_> AND a switch ending in assertNeverKind — two independent compile-time guards so removing a kind case cannot slip through (D-09)."
  - "Pattern: closed-enum select controls make out-of-vocabulary domain values structurally impossible client-side, the same 'drift made impossible' posture as generated wire types."

requirements-completed: [WEB-03]

# Metrics
duration: 21min
completed: 2026-07-10
status: complete
---

# Phase 07 Plan 07: Author Every Scene Object Kind (SC1) Summary

**All 9 locked scene kinds can now be placed from a store-driven object palette (each tool activates its Terra Draw mode and tags the finished feature with `properties.kind` into the canonical D-03 store) and edited in a per-kind property inspector — ground_zone impedance A–H + roughness N/S/M/L as closed-enum selects, forest density/stem-radius/height numerics, source sub-source position + spectrum slot — with per-field last-object inheritance chips that clear on edit; a typed fetch client imports the generated wire types (D-10, no hand-declared DTO); and the SC1 DGM producer fires a genuinely-wired debounced `POST /dgm/triangulate` that renders a TIN overlay and stores a 4xx reject — all proven by three fully-offline Playwright specs, with `envi-engine` byte-identical and `cargo test` green.**

## Performance
- **Duration:** ~21 min (16:39Z → 17:00Z)
- **Tasks:** 4, each committed atomically
- **Files:** 17 created, 8 modified (incl. rebuilt web/dist)

## Accomplishments

### Task 1 — 9-kind palette + Terra Draw modes + typed wire client (commit 31ac2ba)
`draw/kinds.ts` is the frozen 9-kind discriminated union with per-kind metadata (TD geometry mode, palette icon, kind-hue token — existing theme tokens only). Exhaustiveness is guarded twice: `KIND_META` is a `Record<Kind, KindMeta>` (a dropped kind → missing-key `tsc` error) and the Inspector switch ends in `assertNeverKind` (a dropped case → not-assignable-to-`never`). `draw/modes.ts` maps kind → TD mode; `Palette.tsx` is the store-driven tool list. `api/client.ts` imports `DgmReq/DgmResp/InterpolateReq/InterpolateResp/FreqAxisDto` from `../generated/wire` and declares **no** local DTO (grep `interface .*Dto|type .*Dto` = 0); `ApiError` carries the status + path-redacted `detail`. The store gained `activeTool`, `commitFeature`/`tagCreatedFeature` (kind tag + inheritance seeding), `updateProperties`, and `sceneFeatureCollection`/`saveScene`.

### Task 2 — property inspector + per-kind fields (commit ebe5912)
`Inspector.tsx` dispatches on `properties.kind` to a per-kind field body (empty-state when nothing selected). `GroundZoneFields` renders impedance (A–H, each option showing the class letter + display-only σ, **class B = 31.5**) and roughness (N/S/M/L) as `<select>` — an out-of-vocabulary value is structurally impossible (T-07-07-02). `ForestFields` renders density/stem-radius/height as `.mono .dense` numerics; `SourceFields` renders the sub-source position `[x,y,z]` three inputs + a disabled "Edit spectrum" slot (editor is 07-08). Each seeded field carries a `.chip.info "inherited from last {kind}"` / `.chip.off "default"` marker that clears on edit. Every value reaches the DOM as a React text child (no raw-HTML injection).

### Task 3 — last-object inheritance + draw-each-kind E2E (commit be7ee80)
`store/inheritance.ts` holds the per-kind session-scoped `lastOf`/`seedProps` + `KIND_DEFAULTS`; a new object seeds from the last committed object of its kind (fields marked inherited), and an edit updates the inheritance source so the next object inherits the edited value. `draw-kinds.spec.ts` places all 9 kinds via the palette + a DEV commit bridge, asserts each lands in the store and in the mocked whole-scene PUT payload with a valid `properties.kind`, and asserts a second ground_zone inherits the first's edited impedance ('C') with the "inherited from last ground_zone" chip that clears on edit. Fully offline.

### Task 4 — DGM re-triangulation producer (commit 1b0ba47)
`dgm/dgmTrigger.ts` subscribes to committed elevation-set changes and, **debounced 750ms** and **decoupled from the raw TD change/drag path**, assembles a `DgmReq` and calls `POST /dgm/triangulate`. Success → `dgm.setTriangulation` (rendered by `DgmOverlay` as a recessive `--color-info` TIN line layer, rebuilt on `style.load`); a 4xx → `dgm.setReject` (the 07-09 crit source, never a silent swallow); <3 non-collinear points → skip + clear. `dgm-trigger.spec.ts` proves 3 rapid commits fire exactly ONE request → TIN renders, and a 400 lands as the stored reject.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] Tightened the shared api mock glob so it no longer swallows the fetch-client module**
- **Found during:** Task 3 (draw-kinds + lifecycle specs both went blank).
- **Issue:** 07-06's `installApiMocks` used `page.route("**/api/**", …)`. The new `web/src/api/client.ts` module is served by the Vite dev server at `/src/api/client.ts`, whose path contains `/api/` — so the broad glob intercepted the ES **module** request and returned `application/json {}`, throwing "Expected a JavaScript-or-Wasm module script but the server responded with a MIME type of application/json" and blanking the app (every spec failed, including the previously-green lifecycle spec).
- **Fix:** Scoped the mock to the real backend prefix `**/api/v1/**` (the endpoints are all under `/api/v1/`), which cannot match `/src/api/client.ts`. Correct and narrower; lifecycle stays green.
- **Files:** web/tests/e2e/_mocks.ts. **Commit:** be7ee80.

**2. [Rule 3 — Integration] App.tsx / main.tsx / MapCanvas wired to the new UI (not in the plan's file list)**
- **Issue:** The plan's file list did not name `App.tsx`, `main.tsx`, or `MapCanvas.tsx`, but Task 1 "Create Palette.tsx" and Task 2 "Create Inspector.tsx" necessarily replace the inline palette/inspector in `App.tsx`; Task 4's overlay/trigger mount in `MapCanvas.tsx`; and the DEV E2E bridge installs from `main.tsx`. These are required integration edits.
- **Fix:** App uses `<Palette>` + `<Inspector>` and wires Save→`saveScene`; MapCanvas mounts `<DgmOverlay>` + `useDgmTrigger` and adds tin/reject readouts; main.tsx installs the DEV-only bridge.
- **Files:** web/src/App.tsx, web/src/main.tsx, web/src/map/MapCanvas.tsx. **Commits:** 31ac2ba/ebe5912/1b0ba47.

**3. [Rule 2 — Test observability] DEV-only commit bridge + vite-env.d.ts + a dedicated DGM spec**
- **Issue:** Terra Draw drawing is unreliable in headless WebGL, and the plan permits "programmatic store commits". A narrow DEV bridge (`window.__enviTest`) exposes the same `commitFeature` path a finished draw uses. `import.meta.env.DEV`-gating requires Vite's client ambient types (`src/vite-env.d.ts`); the guard is statically eliminated from the production bundle (verified absent from `web/dist`). Added `dgm-trigger.spec.ts` so the SC1 producer is proven genuinely wired (not silently inert), rather than only asserted in 07-10.
- **Files:** web/src/testBridge.ts, web/src/vite-env.d.ts, web/tests/e2e/{env.d.ts,dgm-trigger.spec.ts}. **Commits:** be7ee80/1b0ba47.

**4. [Rule 3 — Build coherence] inheritance.ts created in Task 1 (nominally a Task 3 file)**
- **Issue:** Task 1's `sceneStore.commitFeature` seeds properties via `seedProps`, so `store/inheritance.ts` had to exist for Task 1 to compile. It was created in Task 1 and its E2E + chip threading landed in Tasks 2–3 as planned.
- **Files:** web/src/store/inheritance.ts. **Commit:** 31ac2ba.

**Total:** 4 auto-fixed (1 blocking mock-glob collision, 2 required integration/observability, 1 build-order). No package installs, no architectural (Rule-4) changes.

## Verification
- `cd web && npx tsc --noEmit` — clean; `npm run build` — green (prod bundle excludes the DEV bridge; `grep __enviTest dist/` = none).
- `npx playwright test` — **4 passed** (lifecycle + draw-kinds + 2× dgm-trigger), fully offline (zero unmocked network requests).
- Grep gates: `grep -c "interface .*Dto\|type .*Dto" src/api/client.ts` = 0; `grep dangerouslySetInnerHTML[=:] src/` = no usage; `grep triangulate src/map/useTerraDraw.ts` = none (producer is off the raw TD path); `grep setReject src/dgm/dgmTrigger.ts` present.
- Rust: `git diff --quiet crates/envi-engine/` — byte-identical; `cargo test` — full workspace green (incl. `contract_meta_static` serving the rebuilt web/dist against stable markers); metrao3 untouched.

## Known Stubs (documented deferrals — not silent inertness)
- **"Edit spectrum" buttons** (source + wall) are disabled slots — the curve+table isolation-spectrum editor is **07-08** (WEB-10). The `api/client.ts` `interpolateSpectrum` wrapper is already in place for it.
- **Source sub-source position** is stored in feature properties separately from the map geometry; geometry↔position sync + SPL-at-reference calibration are **07-08** (WEB-02).
- **Save uses a placeholder projectId** and is button-triggered; project open/create is **07-10** and debounced autosave (D-04) is **07-09**. The whole-scene PUT path itself is real and E2E-proven.
- **DgmOverlay coordinates are `[lng, lat]` preview space**; the geodetic projection into SceneXY meters is **Phase 8** (terrain import) — the endpoint shape is unchanged.
- The **07-06 Gate-1 readouts + test buttons** in MapCanvas remain (lifecycle.spec depends on them); they are superseded when the project/basemap chrome finalises in 07-10.

## Threat Coverage
| Threat ID | Mitigation shipped |
|-----------|--------------------|
| T-07-07-01 (XSS via inspector values / ids) | All values via React text children; no dangerouslySetInnerHTML usage (grep-gated); inline DOM-built SVG icons only |
| T-07-07-02 (out-of-vocabulary impedance/roughness) | Closed-enum `<select>` for A–H and N/S/M/L — an invalid class is structurally impossible client-side; server `TryFrom` remains the backstop |
| T-07-07-03 (wire type drift) | `api/client.ts` imports the generated wire types (D-10); no hand-declared DTO to drift (grep = 0) |

## Self-Check: PASSED
