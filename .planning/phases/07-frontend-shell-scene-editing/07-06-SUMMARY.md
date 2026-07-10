---
phase: 07-frontend-shell-scene-editing
plan: 06
subsystem: ui
tags: [react, maplibre, terra-draw, zustand, react-map-gl, playwright, lifecycle-spike, gate-1]

# Dependency graph
requires:
  - phase: 07-frontend-shell-scene-editing
    plan: 05
    provides: "web/ React 19 + Vite toolchain, theme.css tokens, icons.ts, the four-region App shell (map-canvas slot), playwright.config webServer, committed web/dist"
provides:
  - "web/src/store/sceneStore.ts — the canonical Zustand scene store (D-03): features by id, applyTerraDrawChange (upsert/delete from a TD snapshot), terraDrawFeatures() for re-hydration, selection, dirty; isolation spectra keyed in the store, never in TD feature properties"
  - "web/src/map/useTerraDraw.ts — the Gate-1 lifecycle hook: ONE instance-in-ref TerraDraw under a StrictMode guard, change/finish wiring, origin==='api' loop-break, and rebuild-on-style.load re-hydration after setStyle"
  - "web/src/map/basemap.ts — OpenFreeMap styles/dark (D-13a, no API key) + inline dark fallback + attachOsmAttribution helper"
  - "web/src/map/MapCanvas.tsx — react-map-gl <Map> mounting the basemap + Terra Draw into the shell slot"
  - "web/tests/e2e/_mocks.ts — installOffline stack (offline guard + basemap + /api interception) reused by all later specs"
  - "the empirically-verified Gate-1 lifecycle answer (single instance, style.load rebuild, no feedback loop) that de-risks 07-07..07-10 drawing/editor plans"
affects: [07-07, 07-08, 07-09, 07-10]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "app-store-canonical Terra Draw (D-03): store is truth, TD is a controlled view; change echoes with context.origin==='api' are ignored"
    - "rebuild-on-style.load (CORRECTED SC4 recovery): setStyle destroys the adapter's sources, so a FRESH TerraDraw instance is built on style.load rather than clear()+addFeatures() on the stale adapter"
    - "instance-in-ref + StrictMode guard + effect-cleanup teardown for every imperative map/draw/AttributionControl subscription"
    - "fully-offline Playwright: page.route offline-guard aborts+records any external request; basemap style/tiles/glyphs intercepted"
    - "SwiftShader WebGL launch flags so MapLibre renders in headless Chromium"

key-files:
  created:
    - web/src/store/sceneStore.ts
    - web/src/map/basemap.ts
    - web/src/map/useTerraDraw.ts
    - web/src/map/MapCanvas.tsx
    - web/tests/e2e/_mocks.ts
    - web/tests/e2e/lifecycle.spec.ts
  modified:
    - web/src/App.tsx
    - web/src/app.css
    - web/playwright.config.ts
    - web/dist

key-decisions:
  - "Rebuild a fresh TerraDraw instance on style.load (not clear()+addFeatures) — research Pattern 2 throws 'setData of undefined' against the real maplibre adapter v1.4.1"
  - "Adapter constructed as new TerraDrawMapLibreGLAdapter({ map }) — the installed v1.4.1 has NO lib param (research said { map, lib })"
  - "Basemap switch target is an inline dark fallback style so the switch is offline and dark-only (D-13), while the primary basemap stays the OpenFreeMap network dark style (D-13a) that the E2E intercepts"
  - "Gate-1 observability (store/TD/instance/rehydration counters) surfaced to the DOM so the offline E2E can assert lifecycle facts without pixel inspection; these temporary readouts are superseded by real drawing UI in 07-07"

requirements-completed: []
requirements-advanced: [WEB-01]

# Metrics
duration: 16min
completed: 2026-07-10
status: complete
---

# Phase 7 Plan 06: Terra Draw ⇄ react-map-gl Lifecycle Spike (Gate-1) Summary

**Landed the flagged Gate-1 lifecycle spike: a MapLibre dark-vector basemap under a store-canonical Terra Draw (D-03) with a StrictMode single-instance guard and — after correcting research's re-hydration recipe against the real adapter — a rebuild-on-`style.load` recovery that keeps the scene alive across `map.setStyle()`, all proven by a fully-offline Playwright spec.**

## Performance
- **Duration:** ~16 min (commit span 17:34 → 17:50 +0200)
- **Tasks:** 3
- **Files:** 6 created, 4 modified (incl. rebuilt web/dist)

## Accomplishments
- **Canonical store (D-03).** `sceneStore.ts` holds `features` by id, `applyTerraDrawChange(ids, type, snapshot)` (upsert every id present in the TD snapshot, delete every id absent), `terraDrawFeatures()` for re-hydration, plus `selection`/`dirty`. A 105-band isolation spectrum is scene data — the store reserves a `spectra` channel keyed by feature/edge id; `grep "spectr" web/src` matches only the store module (never TD properties).
- **Instance-in-ref lifecycle (Gate-1 core).** `useTerraDraw.ts` builds exactly ONE TerraDraw after map load, guarded by `drawRef.current` against StrictMode's dev double-mount, tears everything down in the effect cleanup (`draw.stop()`, `map.off`, AttributionControl removed). The `change` handler writes user edits into the store and early-returns on `context.origin === "api"` (the store's own `addFeatures` echoes — no feedback loop). `finish` is the committed-edit trigger (D-04; autosave scheduling deferred to 07-09).
- **Basemap (D-13/D-13a).** `basemap.ts` exports the OpenFreeMap `styles/dark` URL (MIT, no API key, network-fetched), an inline dark fallback used to exercise the switch offline, and `attachOsmAttribution` (MapLibre `AttributionControl` mentioning OpenStreetMap — data hygiene). `MapCanvas.tsx` mounts the react-map-gl `<Map>` into the App shell's map slot with `attributionControl={false}` so our OSM control is the single deterministic one.
- **SC4 re-hydration, corrected.** After `map.setStyle()` the scene is rebuilt from the store on the single `style.load` hook (never `styledata`; `grep -rc styledata web/src/map` = 0).
- **Offline E2E.** `lifecycle.spec.ts` drives the real bundle and asserts: dark basemap renders, OSM attribution present, exactly ONE live TerraDraw under StrictMode, a seeded feature survives TWO basemap switches (store stays 1 → no echo-loop; TD re-hydrated to 1), and ZERO unmocked network requests (the offline guard aborts+records anything leaving localhost). Green in 2.7 s.

## Task Commits
1. **Task 1: canonical store + dark basemap config** — `4b05b31` (feat)
2. **Task 2: instance-in-ref Terra Draw lifecycle + MapCanvas** — `4c88d0a` (feat)
3. **Task 3: offline lifecycle E2E + rebuild-on-style.load correction** — `ba703ac` (test)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Gate-1 research correction] `style.load` re-hydration must REBUILD the instance, not `clear()+addFeatures()`**
- **Found during:** Task 3 (E2E — the feature vanished after a basemap switch; `td-feature-count` → 0).
- **Issue:** Research Pattern 2 / SC4 core prescribed `draw.clear(); draw.addFeatures(store.terraDrawFeatures())` on `style.load`. Against the installed `terra-draw-maplibre-gl-adapter@1.4.1` this throws `TypeError: Cannot read properties of undefined (reading 'setData')`: `map.setStyle()` destroys the adapter's GeoJSON source, and the adapter's `render()` calls `getSource(id).setData(...)` assuming the source still exists — it does not self-reattach. A `stop()/start()` fallback threw identically.
- **Fix:** On `style.load` (only when an instance already exists — i.e. a real re-style, not the initial load), tear the instance down and build a FRESH `TerraDraw` whose new adapter registers its source/layers onto the new style, then re-add the canonical features from the store. Net live-instance count stays exactly 1.
- **Why it matters for later plans:** every plan that switches basemap/theme or reloads (07-09/07-10 SC4 paths) must rebuild, not re-add. This is the load-bearing correction the plan asked to record prominently.
- **Files:** web/src/map/useTerraDraw.ts. **Commit:** ba703ac.

**2. [Rule 3 — Blocking] Adapter constructor has no `lib` param in v1.4.1**
- **Issue:** Research/RESEARCH.md showed `new TerraDrawMapLibreGLAdapter({ map, lib: maplibregl })`. The installed adapter's constructor is `{ map, renderBelowLayerId?, prefixId? } & BaseAdapterConfig` — **no `lib`**; passing it would be an excess-property error / dead config.
- **Fix:** `new TerraDrawMapLibreGLAdapter({ map: instance })`. **Files:** web/src/map/useTerraDraw.ts. **Commit:** 4c88d0a.

**3. [Rule 3 — Blocking] Headless Chromium needs SwiftShader WebGL for MapLibre**
- **Issue:** MapLibre GL requires WebGL2; headless Chromium (137+) emits `CONTEXT_LOST_WEBGL` and blocks the software fallback, so the map never fired `load` (`map-ready` stuck on "no").
- **Fix:** Added `launchOptions.args` (`--enable-unsafe-swiftshader`, `--use-gl=angle`, `--use-angle=swiftshader`, `--ignore-gpu-blocklist`) to `playwright.config.ts`. Test-only; no effect on the shipped bundle. **Files:** web/playwright.config.ts. **Commit:** ba703ac.

**4. [Rule 3 — Blocking] `.map-slot` re-styled to host a filling map**
- **Issue:** The 07-05 `.map-slot` flex-centered a text placeholder; a react-map-gl `<Map>` needs a positioned, non-centering container or it renders at 0px.
- **Fix:** `.map-slot` → `position: relative; overflow: hidden` (no flex centering); added `.map-overlay/.map-controls/.map-readout` styles on existing tokens only (no new token, D-11). **Files:** web/src/app.css. **Commit:** 4c88d0a.

**5. [Rule 2 — Observability for the required assertions] Gate-1 DOM readouts**
- **Issue:** SC4 "features remain visible" and "exactly one instance" are not DOM-observable by default — `draw.getSnapshot()` reflects TD's INTERNAL store, which survives `setStyle` even when rendering is broken, so it cannot prove a re-render.
- **Fix:** Added lifecycle diagnostics to the store (`drawInstancesLive/Built`, `rehydrations`) and surfaced store/TD/instance/rehydration counts as `data-testid` readouts the E2E asserts on. Because the recovery now REBUILDS the instance, `td-feature-count` and `rehydrations` genuinely prove re-hydration ran. Temporary — removed when the real drawing/inspector UI lands in 07-07. **Files:** web/src/store/sceneStore.ts, web/src/map/MapCanvas.tsx. **Commits:** 4b05b31/4c88d0a/ba703ac.

**Total:** 5 auto-fixed (1 research correction, 4 blocking/observability). No package installs, no architectural (Rule-4) changes.

## Verification
- `web/`: `npx tsc --noEmit` clean; `npm run build` green; `npx playwright test lifecycle` → 1 passed, fully offline (no request to `tiles.openfreemap.org`).
- `grep -rc styledata web/src/map/` = 0 (style.load is the single re-hydration hook).
- `grep "spectr" web/src` → store module only (no spectrum in TD properties).
- Rust untouched: `git diff --quiet crates/envi-engine/` → byte-identical; `cargo test -p envi-service --test contract_meta_static` → 2 passed (static bundle serves the new web/dist against stable `<title>ENVI`/`#root` markers).

## Known Stubs
- **Temporary Gate-1 controls + readouts** (`MapCanvas.tsx`: "Switch basemap" / "Add test point" buttons and the store/TD/instance/rehydration `data-testid` readouts) exist only to exercise and assert the lifecycle. They are the spike's scaffolding and are replaced by real per-kind drawing + inspector UI in 07-07+.
- **`finish` handler** marks dirty only — debounced whole-scene autosave (D-04) is intentionally deferred to 07-09.
- **`spectra` store channel / `select`** are reserved skeleton — the isolation-spectrum editor and per-edge UUID diffing land in later 07-plans (07-07/07-09/07-10). These are documented deferrals, not silent inertness.

## Threat Flags
None new. The only runtime network surface is the accepted OpenFreeMap basemap XHR (T-07-06-01, D-13a) — a MapLibre XHR, not an index.html asset, so the Phase-6 "zero external assets" gate stays green. Attribution/label strings reach the DOM via React text / MapLibre's own attribution (a fixed OSM constant, never a user string — T-07-06-02). Every map/draw/AttributionControl subscription is torn down in the effect cleanup (T-07-06-03). The E2E aborts+fails on any un-mocked request (T-07-06-04).

## Next Phase Readiness
- The Gate-1 lifecycle is proven and de-risked: 07-07 can add per-kind Terra Draw modes and the object palette wiring on top of `useTerraDraw`/`sceneStore` without re-solving setStyle/StrictMode.
- Later SC4 paths (07-09 autosave/reload, 07-10 basemap/theme) MUST use the rebuild-on-`style.load` recovery documented here, not clear()+addFeatures().
- `installOffline` in `_mocks.ts` is the shared offline harness for every subsequent spec.

## Self-Check: PASSED
- All 6 created files + the SUMMARY exist on disk.
- All three task commits present in git history (4b05b31, 4c88d0a, ba703ac).
- `.planning/config.json` untouched (pre-existing unstaged modification preserved).
