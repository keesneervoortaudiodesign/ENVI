---
phase: 11-results-fast-recalc
plan: 07
subsystem: results-ui
tags: [web, react, wasm, conditioning, fast-recalc, web-05, svc-06, d-01, d-10, d-11, d-12, stale, 409, playwright]

# Dependency graph
requires:
  - phase: 11-results-fast-recalc
    provides: "11-03 recondition/readout_receivers MAC boundary + typed HashMismatch (client 409); 11-05 results shell + SpectrumPanel + readChunk glue; 11-06 isophone fill layer + colorScale setIsophoneInput feed"
  - phase: 10-calculation-service
    provides: "marshalled_tensor_hash identity export; the CalcPanel flat-corridor scene marshaller"
  - phase: 07-web-foundation
    provides: "the reused SpectrumEditor (WEB-10) + its server-owned interpolation seam"
provides:
  - "store/conditioning.ts — per-source ConditioningDto drive + a ~150 ms debounced recondition MAC (D-10) over the cached OPFS tensor; pushes reconditioned readouts into results + re-feeds the isophone grid; refuses a mismatched hash (SVC-06 409)"
  - "store/stale.ts — re-mints the blake3 tensor identity of the CURRENT scene and compares to the cached hash → the D-12 stale badge; conditioning never stales (D-07)"
  - "panels/ConditioningPanel.tsx — per-source Gain/Delay/Mute + the reused SpectrumEditor Filter (D-11), live no-button; surfaces the stale badge + the honest 409 reject banner"
  - "compute/marshalScene.ts — the shared flat-corridor scene marshaller + blake3 identity (extracted from CalcPanel; single source of truth for Calc submit AND the stale re-mint)"
affects: [11-08, 11-09, 11-10]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "The flagship fast-recalc: a conditioning edit debounces (~150 ms, D-10) into ONE readout_receivers MAC over the cached tensor — spectra + the isophone map update live with NO re-propagation (the calc job never leaves idle; the isophone RE-CONTOURS the cached grid, SC3)"
    - "Two honest-state guarantees: the D-12 stale badge appears the MOMENT the re-minted identity diverges (conditioning edits never re-mint → never stale, D-07); a MAC against a mismatched hash is REFUSED with the reject banner (the SVC-06 409 client realization), never silently served"
    - "D-01 grep gate: ZERO TS acoustic math in the conditioning + stale stores; the dB→complex filter, the readout law, and the identity blake3 all live in WASM. The Filter reuses the Phase-7 SpectrumEditor verbatim (D-11) — its dense [105] is materialised SERVER-side"
    - "Single marshaller: the flat-corridor PrepareSolveReq + blake3 identity live in compute/marshalScene (extracted from CalcPanel) so a scene edit hashes identically for Calc-submit AND the stale re-mint — no forked marshaller"

key-files:
  created:
    - web/src/compute/marshalScene.ts
    - web/src/store/conditioning.ts
    - web/src/store/conditioning.test.ts
    - web/src/store/stale.ts
    - web/src/store/stale.test.ts
    - web/tests/e2e/conditioning.spec.ts
  modified:
    - web/src/panels/ConditioningPanel.tsx
    - web/src/panels/CalcPanel.tsx
    - web/src/store/results.ts
    - web/src/testBridge.ts
    - web/src/app.css
    - web/package.json
    - web/dist

key-decisions:
  - "Isophone map live-update via the SAME MAC: on a successful recondition the store re-feeds colorScale.setIsophoneInput with the WASM-produced per-lattice total_dba (placed by index only — no TS dB math), so the isophone tracer re-contours the reconditioned grid (SC3). The full production fine-tier lattice feed stays the documented 11-05/06 follow-up; the fixture seeds a grid whose cells align 1:1 with the reconditioned receivers."
  - "The stale badge re-mints via the SHARED compute/marshalScene (extracted from CalcPanel) so the Calc-submit identity and the stale re-mint are byte-identical. The stale watcher keys on the manifest tensorHash (not the manifest object), so a conditioning recalc — which swaps the drive but leaves the hash — never re-runs the check (D-07 never-stale, enforced structurally)."
  - "The Filter reuses the Phase-7 SpectrumEditor verbatim (D-11): a render-nothing FilterMaterializer child watches the source's authored spectrum in the scene store and materialises its dense [105] through the SAME server interpolation the editor's curve uses (useSpectrumPreview) — the frontend authors no dB, and a dense filter drives the debounced MAC."

requirements-completed: [WEB-05, SVC-06]

# Metrics
duration: ~17 min
completed: 2026-07-12
status: complete
---

# Phase 11 Plan 07: Interactive Conditioning Fast-Recalc (WEB-05 / SVC-06) Summary

**The flagship payoff of the complex-tensor architecture: a per-source Gain(dB) + Delay(ms) + reused-SpectrumEditor Filter (D-11) drives a live DEBOUNCED (~150 ms, D-10) recondition MAC over the cached OPFS tensor — the receiver spectrum AND the isophone map update live with NO re-propagation (the calc job never leaves idle; the map re-contours the cached grid, SC3) — plus two honest-state guarantees: a results-stale badge that appears the moment the re-minted blake3 identity diverges from the cached tensor hash (D-12; conditioning edits never stale, D-07), and the SVC-06 409 refusal (a MAC against a mismatched hash is refused with an honest reject banner, never silently served), all proven by an offline Playwright UAT on the real bundle + real WASM MAC.**

## Performance

- **Duration:** ~17 min
- **Tasks:** 3 `type=auto` (+ 1 support refactor)
- **Files:** 6 created, 6 modified (incl. web/dist rebuilt)

## Accomplishments

- **Conditioning store + debounced MAC (Task 1, WEB-05 / D-10 / D-01).** `store/conditioning.ts` holds the per-source `ConditioningDto` drive (Gain/Delay/Filter/Mute) and a module-scoped ~150 ms debounce (`RECONDITION_DEBOUNCE_MS`) that coalesces a burst of edits into exactly ONE dispatch. On fire it builds the ordered `ConditioningDto[]` (the SAME wire shape, never forked) and calls the injectable `ConditioningClient` — the real client reads each covering OPFS chunk (`readChunk`) and drives the 11-03 `readout_receivers` MAC (hash-gated). On success it pushes the WASM-produced readouts into the results store (`applyConditioning`) and re-feeds the isophone grid from the reconditioned per-lattice `total_dba`; on a thrown `tensor_hash mismatch` it sets the `refuse` flag (never serves stale spectra). Zero TS acoustic math (D-01 grep gate 0).
- **Stale badge + honest 409 (Task 2, D-12 / SVC-06).** `store/stale.ts` re-mints the blake3 tensor identity of the CURRENT scene (via the WASM `tensor_hash` export, through the shared marshaller) and compares to the cached manifest hash → `isStale`. A `useStaleWatch` hook re-mints on any scene/grid edit; because it keys on the manifest `tensorHash` (not the manifest object), a conditioning recalc — which swaps the drive but leaves the hash — never re-runs it, so conditioning **never stales** (D-07, enforced structurally). `panels/ConditioningPanel.tsx` fills the 11-05 ResultsPanel slot: per-source Gain/Delay numeric inputs + a Mute toggle + a Filter that REUSES the Phase-7 `SpectrumEditor` verbatim (D-11), live with no button; it surfaces the `.chip.warn` "Out of date" badge (which never blocks edits) and the SVC-06 409 reject banner with the UI-SPEC copy.
- **Shared scene marshaller (support refactor).** Extracted the flat-corridor `PrepareSolveReq` construction + blake3 identity out of `CalcPanel` into `compute/marshalScene.ts` so the Calc submit AND the stale re-mint hash the scene identically (single source of truth, no forked marshaller). The glue is dynamic-imported so the stores stay Node-unit-testable.
- **Offline Playwright UAT (Task 3, SC2).** `web/tests/e2e/conditioning.spec.ts` drives the real bundle + real WASM MAC offline (mocked `/api/*`, a fixture tensor seeded into OPFS keyed by the real minted identity + a 3×3 isophone grid). It asserts: a Gain edit and a Filter edit (the reused SpectrumEditor) live-update the spectrum + the isophone map via the MAC with the calc job staying idle (no re-propagation) and the isophone trace count advancing (re-contour, SC3); a conditioning edit never shows the stale badge; a simulated scene edit flips the "Out of date" badge; and a MAC against the now-mismatched hash surfaces the honest 409 reject banner and updates nothing — with zero network egress.

## Task Commits

1. `1209257` refactor(11-07): extract scene marshaller into compute/marshalScene
2. `b6cadae` feat(11-07): conditioning store + debounced recondition MAC (WEB-05, D-10/D-01)
3. `cfe9b8b` feat(11-07): stale badge (re-mint identity) + 409 reject + ConditioningPanel (D-12, SVC-06)
4. `f3946b2` test(11-07): offline Playwright conditioning UAT + DEV bridge + web/dist (SC2)
5. `9bf2885` fix(11-07): dynamic-import the compute WASM glue in marshalScene

_Plan metadata commit follows this SUMMARY._

## Deviations from Plan

### Auto-fixed / structural (Rule 2/3)

**1. [Rule 3 — blocking support] Extracted the scene marshaller into `compute/marshalScene.ts`.**
- **Found during:** Task 2. The stale re-mint must hash the CURRENT scene identically to the Calc-submit path, but the marshaller lived inline in `CalcPanel`. Duplicating it would fork the identity (drift risk).
- **Fix:** Moved the flat-corridor `PrepareSolveReq` construction + blake3 identity + the pure geometry helpers into `compute/marshalScene.ts`; `CalcPanel.buildJobSpec` now wraps `buildPrepareScene`, and `useStaleWatch` reuses the SAME function. The wasm glue is dynamic-imported so importing the module in Node (the vitest graph) never pulls the browser-only worker glue.
- **Files:** `web/src/compute/marshalScene.ts` (new), `web/src/panels/CalcPanel.tsx`.
- **Verification:** web typecheck clean; the full vitest suite (49) + the conditioning e2e green.

**2. [Rule 2 — missing critical support] `results.applyConditioning` action + testBridge conditioning helpers + a `typecheck` npm script.**
- **Issue:** The reconditioned readouts must land in the results store live (a small `applyConditioning` action), and the offline UAT needs to seed a multi-source conditioning result + simulate a scene edit + read the honest-state flags (DEV-only bridge helpers). `results.ts` / `testBridge.ts` / `package.json` were not in the declared file list.
- **Fix:** Added `applyConditioning` (swap the drive + merge readouts), the `seedConditioning`/`divergeScene`/`conditioningState`/`staleState`/`calcJobState` DEV bridge helpers, and a `typecheck` script (`tsc --noEmit`) so the plan's verify commands run verbatim. All additive.
- **Files:** `web/src/store/results.ts`, `web/src/testBridge.ts`, `web/package.json`.

**3. [Scoping — documented] Isophone map live-update via the reconditioned-lattice-totals feed.**
- **Issue:** The production fine-tier lattice→`LevelGrid` readout feed is an explicitly-deferred 11-05/06 follow-up (`reconstruct_level_grid` has no WASM boundary yet). The map must still update live on conditioning.
- **Fix:** On a successful recondition the store re-feeds `colorScale.setIsophoneInput` with the WASM-produced per-receiver `total_dba` (placed into the grid by index only — no TS acoustic math), so the isophone tracer re-contours the reconditioned grid (SC3). The fixture seeds a grid whose cell count == the reconditioned receiver count (1:1). The full production lattice feed remains the documented follow-up.
- **Files:** `web/src/store/conditioning.ts`, `web/src/testBridge.ts`.

**4. [Rule 3 — build recipe] Rebuilt `web/dist` via `npm run build:web` (not the full `npm run build`).**
- **Issue:** The plan's Task-2 verify says `npm run build`, which also runs `build:wasm` + the nightly `build:wasm:compute`. This plan changed ZERO Rust — the committed compute-WASM glue is current.
- **Fix:** Rebuilt the dist with `npm run build:web` (tsc + vite build), the same approach 11-06 used, producing a fresh bundle with the conditioning surfaces. No WASM rebuild (nothing Rust changed; `cargo tree -p envi-engine` unchanged).

**Total deviations:** 4 (1 blocking extraction, 1 missing support, 1 documented scoping, 1 build-recipe). **Impact:** No engine/wire/Cargo changes (verified: my commits touched zero `.rs`/`wire.ts`/`Cargo.*`); no scope creep beyond the flagship interaction.

## Authentication Gates

None.

## Requirements

- **WEB-05** (interactive conditioning fast-recalc) — **COMPLETE**: per-source Gain/Delay/Filter drives a live debounced recondition MAC over the cached tensor; spectra + map update with no re-propagation.
- **SVC-06** (recondition/recompute honest state) — **COMPLETE**: the client-side 409 is now user-observable — a MAC against a mismatched tensor hash is refused with the honest reject banner (never silently served), and the results-stale badge appears on identity divergence (conditioning never stales). This completes the 11-03 backend realization at the UI (mirroring 11-05's WEB-11 decision).

## Verification

All commands run in `web/` on `main`:

- `npx tsc --noEmit` (`npm run typecheck`) → **clean** (exit 0).
- `npx vitest run` → **49 passed** (7 files), incl. the new `conditioning` (debounce-to-one, wire-shape, live-apply, 409 refusal) + `stale` (divergence/match, conditioning-never-stale) suites.
- `npm run build:web` (tsc --noEmit && vite build) → **built** (web/dist rebuilt + committed with the conditioning surfaces).
- `npx playwright test conditioning` → **1 passed** (offline, real bundle + real WASM MAC over the seeded OPFS tensor): live gain + filter recalc with the calc job idle + the isophone re-contour, conditioning-never-stale, the scene-edit stale badge, and the honest 409 refusal — zero network egress.
- **D-01 grep gate:** `grep -c "Math.log10\|Math.pow\|Math.exp" web/src/store/conditioning.ts web/src/store/stale.ts` → **0** (both).
- **Rust gates:** unchanged — this plan touched zero `.rs`/`wire.ts`/`Cargo.*` files (`git diff --name-only <base> HEAD` over my commits), so `cargo clippy`/`fmt`/`test`, the wire no-drift, and `cargo tree -p envi-engine` remain green from the 11-06 close-out.

## Issues Encountered

None — all acceptance gates pass. One first-run fix (the extracted marshaller statically imported the wasm glue, breaking the Node test graph) was resolved by lazy-loading the glue (commit `9bf2885`).

## Next Phase Readiness

- **Wave 4 remaining:** 11-08 (scenarios) + 11-09 (export UI). The conditioning drive + the reconditioned readouts are the input a scenario clone-then-edit (11-08) mutates; the export UI (11-09) reuses the same cached grid + readouts.
- **Open follow-up (carried):** the production fine-tier lattice→`LevelGrid` readout feed (a `reconstruct_level_grid` WASM boundary + a live `setIsophoneInput` from a finished readout) is still the natural companion — this plan re-feeds the isophone from the reconditioned receiver totals over the fixture 1:1 grid.

## Self-Check: PASSED

- Created files exist on disk: `web/src/compute/marshalScene.ts`, `web/src/store/{conditioning,conditioning.test,stale,stale.test}.ts`, `web/tests/e2e/conditioning.spec.ts`; `web/src/panels/ConditioningPanel.tsx` filled (was a stub).
- Task commits present in `git log`: `1209257`, `b6cadae`, `cfe9b8b`, `f3946b2`, `9bf2885`.
- All plan `<verification>` commands re-run green (typecheck / vitest / build:web / playwright / D-01 grep above); no Rust/wire/Cargo touched.

---
*Phase: 11-results-fast-recalc*
*Completed: 2026-07-12*
