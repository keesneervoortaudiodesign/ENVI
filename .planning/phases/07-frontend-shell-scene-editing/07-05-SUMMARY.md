---
phase: 07-frontend-shell-scene-editing
plan: 05
subsystem: ui
tags: [react, vite, typescript, maplibre, terra-draw, zustand, turf, playwright, vitest, css-tokens]

# Dependency graph
requires:
  - phase: 06-backend-service-shell
    provides: "envi-service ServeDir(web/dist) static-bundle serving (SVC-03) + the static_bundle_served_with_spa_fallback contract test + repo_web_dist() helper"
provides:
  - "web/ npm package — the repo's first JS toolchain (React 19 + Vite 8 + TS strict), runtime deps vs devDependency-only tooling (D-09)"
  - "web/src/theme.css — metrao3 dark ops-console tokens copied VERBATIM (D-11, system-font variant)"
  - "web/src/icons.ts — inline-SVG DOM pattern (never innerHTML) expanded to the pointer tool + 9 locked kinds"
  - "web/src/App.tsx — the four-region app shell (project bar, object palette, map slot, right rail) on the token grid"
  - "web/dist — real committed Vite bundle (zero external assets) served through envi-service"
  - "named test:unit=vitest run script (07-08 ring-diff tests depend on it)"
affects: [07-06, 07-07, 07-08, 07-09, 07-10]

# Tech tracking
tech-stack:
  added: [react@19, react-dom@19, maplibre-gl@5, react-map-gl@8, terra-draw@1, terra-draw-maplibre-gl-adapter@1, zustand@5, "@turf/turf@7", vite@8, typescript@5, "@vitejs/plugin-react@6", vitest@4, "@playwright/test@1"]
  patterns: ["runtime-deps-vs-devDependency-only split", "token-only CSS (no invented tokens, D-11)", "ref-appended inline-SVG icons (no innerHTML/dangerouslySetInnerHTML)", "stable-marker static-bundle contract assertion", "web/dist committed + git-tracked"]

key-files:
  created: [web/package.json, web/vite.config.ts, web/tsconfig.json, web/playwright.config.ts, web/index.html, web/src/main.tsx, web/src/theme.css, web/src/icons.ts, web/src/App.tsx, web/src/app.css]
  modified: [web/dist/index.html, crates/envi-service/tests/contract_meta_static.rs, .gitignore]

key-decisions:
  - "Copied metrao3 ui/src/theme.css VERBATIM (D-11) — the sole Inter/JetBrains occurrence is the header comment documenting their deliberate omission; no font-family references them (offline gate intact)"
  - "web/dist stays git-tracked and served from disk by envi-service ServeDir; contract test rewritten to stable <title>ENVI + #root markers, not hashed asset names"
  - "React icons reuse icons.ts via a ref+useEffect append (never innerHTML) so the DOM-construction discipline transfers to the React tree"
  - "Active-tool 'background tint' realised via existing --color-surface-3 + primary left-border/text since no --color-primary-bg token exists (no token invented)"

patterns-established:
  - "Runtime deps in dependencies, ALL build/test tooling in devDependencies only — nothing tooling ships in web/dist"
  - "Every web/src/*.ts(x) + config file carries a // # Module I/O header (convention crosses Rust and TS)"
  - "Incremental main.tsx: Task 1 self-contained mount, Task 2 wires <App/> + app.css — each commit typechecks/builds green"

requirements-completed: [WEB-01]

# Metrics
duration: 4min
completed: 2026-07-10
status: complete
---

# Phase 7 Plan 05: Frontend Shell & Toolchain Bootstrap Summary

**Bootstrapped the web/ React 19 + Vite 8 + TypeScript-strict toolchain, adopted the metrao3 dark ops-console theme verbatim (D-11), built the four-region app shell, and shipped a real committed web/dist bundle served through envi-service with the static-bundle contract test green against stable markers.**

## Performance

- **Duration:** ~4 min (commit span 14:08:01 → 14:12:00 +0200)
- **Started:** 2026-07-10T12:04:00Z
- **Completed:** 2026-07-10T12:12:00Z
- **Tasks:** 2
- **Files modified:** 16 (13 created, 3 modified)

## Accomplishments
- First JS toolchain in the repo: `web/package.json` with the runtime-deps-vs-devDependency-only split (D-09), a named `test:unit: vitest run` script (07-08 dependency), and pinned versions matching 07-RESEARCH.
- Vite/TS/Playwright config on metrao3 conventions (strict tsconfig verbatim + `jsx: react-jsx` + DOM libs; `base: './'`, `cssCodeSplit: false`, React plugin).
- `web/src/theme.css` copied VERBATIM from metrao3's embedded system-font variant; `icons.ts` inline-SVG pattern expanded to the pointer tool + 9 kinds (DOM-constructed, never innerHTML).
- `App.tsx` renders the four UI-SPEC regions (project bar `.topbar`, object palette `.panel`, map-canvas slot, right rail inspector+validation empty-states) with control heights from `--row-h-lg`/`--row-h` and 44px reserved for the primary Save (D-12); `app.css` layers the shell grid on token vars only.
- Real Vite bundle built and committed to `web/dist` (index.html + hashed JS/CSS), replacing the Phase-6 placeholder; zero external assets (offline gate green).
- `static_bundle_served_with_spa_fallback` updated to assert stable `<title>ENVI` + `#root` markers instead of the removed placeholder text; full `cargo test -p envi-service` green, clippy + fmt clean, `envi-engine` byte-unchanged.

## Task Commits

Each task was committed atomically:

1. **Task 1: web/ toolchain scaffold + theme + icons** - `1b7337d` (feat)
2. **Task 2: app shell (four regions) + real web/dist bundle + contract-test update** - `d6ddd69` (feat)

**Plan metadata:** this commit (docs: complete plan)

## Files Created/Modified
- `web/package.json` - First npm package; runtime deps (react/maplibre/react-map-gl/terra-draw+adapter/zustand/turf) vs devDependency-only tooling; `test:unit`=`vitest run`.
- `web/vite.config.ts` - Vite build (base `./`, es2022, cssCodeSplit false, React plugin, Module I/O header).
- `web/tsconfig.json` - metrao3 strictness verbatim + `jsx: react-jsx` + ES2022/DOM libs.
- `web/playwright.config.ts` - E2E config driving the real bundle offline (specs land later).
- `web/index.html` - Vite root, zero external assets, stable `<title>ENVI` + `#root` markers.
- `web/src/main.tsx` - StrictMode `#root` mount wiring `<App/>` + theme.css/app.css.
- `web/src/theme.css` - metrao3 dark ops-console tokens, VERBATIM (D-11).
- `web/src/icons.ts` - inline-SVG DOM glyphs for the 10 palette tools.
- `web/src/App.tsx` - the four-region app shell.
- `web/src/app.css` - shell grid + component CSS on tokens only.
- `web/dist/*` - real committed Vite bundle (replaces the placeholder).
- `crates/envi-service/tests/contract_meta_static.rs` - stable-marker static-bundle assertion.
- `.gitignore` - `web/node_modules/`, `web/test-results/`, `web/playwright-report/` (web/dist stays tracked).

## Decisions Made
- **Verbatim theme copy over the grep-0 proxy (D-11 wins).** The `grep -c "Inter\|JetBrains" theme.css` acceptance proxy literally returns 1 because the verbatim header comment (line 2) documents the deliberate *omission* of those web fonts. No `font-family` declaration references them (`--font-sans`/`--font-mono` are system-only), so the real offline gate — no web font fetched — holds. D-11 "copy VERBATIM" is the binding rule and overrides the literal proxy count. See "Deviations" for detail.
- **Stable markers, not asset names.** The contract test asserts `<title>ENVI` + `id="root"` (present in every rebuild) rather than hashed `assets/index-*.js` filenames.
- **Active-tool tint via existing surface token.** No `--color-primary-bg` token exists; the active palette row uses `--color-surface-3` + primary left-border + primary text (all existing tokens) rather than inventing a hex.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Corrected npm version pins that do not exist on the registry**
- **Found during:** Task 1 (npm install)
- **Issue:** `@types/react-dom@^19.2.7` does not exist (types packages don't track react-dom's version); `@vitejs/plugin-react` latest is 6.x (peer `vite ^8`); `vitest` latest is 4.x; bare `typescript` latest tag resolves to 7.x (would break the strict tsconfig).
- **Fix:** Pinned `@types/react@^19.2.0`, `@types/react-dom@^19.2.0`, `@vitejs/plugin-react@^6.0.0`, `vitest@^4.0.0`, and `typescript@^5.9.0` (kept on the 5.x line per 07-RESEARCH "~5.x latest"). All other runtime pins matched RESEARCH exactly.
- **Files modified:** web/package.json
- **Verification:** `npm install` clean (0 vulnerabilities); `npx tsc --noEmit` green.
- **Committed in:** 1b7337d (Task 1 commit)

**2. [Rule 3 - Blocking] Dropped playwright.config.ts from the app tsconfig include**
- **Found during:** Task 1 (`npx tsc --noEmit`)
- **Issue:** `playwright.config.ts` references the node `process` global; including it in the app tsconfig raised TS2591 (would force `@types/node` into the app graph).
- **Fix:** Matched metrao3's convention — `include: ["src", "vite.config.ts"]`; playwright.config.ts is typechecked by Playwright's own runner, not the app build.
- **Files modified:** web/tsconfig.json
- **Verification:** `npx tsc --noEmit` green.
- **Committed in:** 1b7337d (Task 1 commit)

**3. [Rule 3 - Blocking] Incremental main.tsx across the two tasks**
- **Found during:** Task 1 (main.tsx is a Task-1 file but App.tsx/app.css are Task-2 files)
- **Issue:** A main.tsx importing `./App` + `./app.css` would fail Task 1's standalone `tsc --noEmit`.
- **Fix:** Task 1 main.tsx is a self-contained StrictMode `#root` mount importing only theme.css; Task 2 rewrites it to import app.css and mount `<App/>`. Each commit typechecks/builds green.
- **Files modified:** web/src/main.tsx (Task 1 create, Task 2 update)
- **Verification:** tsc green at Task 1; `npm run build` green at Task 2.
- **Committed in:** 1b7337d + d6ddd69

---

**Total deviations:** 3 auto-fixed (all Rule 3 - blocking). No packages were substituted for non-existent names — only version *ranges* on the RESEARCH-approved packages were corrected to published values; no new package was introduced.
**Impact on plan:** All necessary to make the toolchain install/typecheck/build. No scope creep; every runtime dependency is exactly the RESEARCH set.

## Known Stubs
- **Map canvas slot** (`web/src/App.tsx`, region 3) is an intentional empty placeholder — MapLibre + Terra Draw are explicitly deferred to plan 07-06 (stated in the plan objective). Not a data stub; the shell region exists and is styled.
- **Project bar / inspector / validation** show static labels + empty-states with no store wired yet — the zustand store, API client, and live validation land in later 07-plans (07-07+). Documented in the UI-SPEC state matrix as the empty state.

## Threat Flags
None — no new network endpoint, auth path, or trust boundary introduced. `web/dist` is the intended public artifact (T-07-05-02 accepted); index.html has zero external assets (T-07-05-01 mitigated, grep-gated 0). The first `npm install` surface (T-07-05-SC) was pre-audited in RESEARCH as all-OK/Approved — no blocking-human checkpoint triggered.

## Issues Encountered
- Windows CRLF: git warns "LF will be replaced by CRLF" on staged web/ files — cosmetic (core.autocrlf), no content impact.
- The `.conj()` quarantine grep over `envi-engine/src/propagation/` shows 9 hits — all are explanatory doc-comments mentioning `.conj()`, zero real call sites; `envi-engine` is byte-unchanged this plan (VERIFY-ONLY held).

## User Setup Required
None - Node v24 / npm 11 already installed; `npm install` runs offline against the public registry with no credentials.

## Next Phase Readiness
- `web/` scaffold, theme tokens, icons, and the app shell are in place; 07-06 can wire MapLibre + Terra Draw into the map-canvas slot.
- `test:unit`=`vitest run` is present for 07-08's ring-diff unit tests.
- envi-service still serves the real bundle; the static-bundle contract test is decoupled from hashed asset names, so future rebuilds won't break it.

## Self-Check: PASSED

- All 10 created files + web/dist bundle + SUMMARY.md exist on disk.
- All three commits present in git history (1b7337d, d6ddd69, f6d94ac).
- `.planning/config.json` untouched (pre-existing unstaged modification preserved).

---
*Phase: 07-frontend-shell-scene-editing*
*Completed: 2026-07-10*
