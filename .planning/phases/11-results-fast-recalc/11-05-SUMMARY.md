---
phase: 11-results-fast-recalc
plan: 05
subsystem: results-ui
tags: [web, react, wasm, readout, spectrum, web-11, d-01, playwright, opfs]

# Dependency graph
requires:
  - phase: 11-results-fast-recalc
    provides: "11-01 readout_receiver core + A/C weighting + opfs_reader read_chunk; 11-03 recondition MAC boundary"
  - phase: 10-calculation-service
    provides: "OPFS chunk byte format, TierComplete spans, compute worker, threaded compute WASM"
provides:
  - "readout_receivers WASM boundary — full ReceiverReadoutDto per receiver (band levels + coherent/incoherent split + dB(A)/dB(C) + per-channel totals)"
  - "store/results.ts — results slice (selected receiver, display/weighting/split toggles, cached WASM readouts) + main-thread readout client"
  - "ResultsPanel right-rail shell + 4 slot stubs (owned by 11-06..09) mounted once in App.tsx"
  - "SpectrumPanel — chart-primary + expandable table, dual receiver selection, instant weighting toggle, split overlay (D-01: renders WASM values, zero TS acoustic math)"
  - "compute/opfs.ts readChunk read glue + writeChunkFile seed helper"
affects: [11-06, 11-07, 11-08, 11-09]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Main-thread WASM readout: the compute module (already used main-thread for cost/plan under COOP/COEP) composes readout_receivers over async-read OPFS bytes — no worker round-trip for a level readout"
    - "Injectable ReadoutClient seam keeps store/results.ts wasm-free at module load → Node-unit-testable (mirrors calc's CalcClient)"
    - "D-01 enforced by a grep gate: zero Math.log10/Math.pow/Math.exp in results.ts + SpectrumPanel.tsx; every dB is WASM-produced; band-index aggregation, never nominal Hz"

key-files:
  created:
    - web/src/store/results.ts
    - web/src/store/results.test.ts
    - web/src/panels/ResultsPanel.tsx
    - web/src/panels/SpectrumPanel.tsx
    - web/src/panels/ColorScaleEditor.tsx
    - web/src/panels/ConditioningPanel.tsx
    - web/src/panels/ScenarioPanel.tsx
    - web/src/panels/ExportMenu.tsx
    - web/tests/e2e/results-spectrum.spec.ts
  modified:
    - crates/envi-compute/src/readout.rs
    - crates/envi-compute-wasm/src/dto.rs
    - crates/envi-compute-wasm/src/recondition.rs
    - crates/envi-service/tests/wire_no_drift.rs
    - web/src/generated/wire.ts
    - web/src/compute/opfs.ts
    - web/src/App.tsx
    - web/src/app.css
    - web/src/testBridge.ts
    - web/dist (rebuilt)

key-decisions:
  - "The plan's premise (a '11-01 WASM readout' boundary returning the full ReceiverReadout) did not exist — only recondition (band levels only). Added a readout_receivers #[wasm_bindgen] boundary + ReceiverReadoutDto generated wire type (coordinator-authorized Option A). Engine core (envi-engine) untouched; envi-compute readout extended via existing engine laws."
  - "Readout runs on the MAIN THREAD (not the calc worker): the compute WASM is already main-thread for cost/plan, and readout_receivers needs no rayon pool. OPFS reads use the async File API (main-thread capable), so no worker plumbing was added."
  - "Playwright drives the vite DEV server (already COOP/COEP → crossOriginIsolated) not vite preview/dist: the OPFS+manifest seeding needs the DEV-only testBridge, which is absent from a production dist bundle. This is the project's established e2e pattern (all specs drive the vite-served real app)."

patterns-established:
  - "ReceiverReadoutDto: WASM precomputes both weighted totals + the per-channel dB split so UI toggles are pure re-render (D-09)"
  - "1/3-oct display = the 27 third-octave BAND INDICES (0,4,…,104); 1/12-oct = all 105 — pure index selection, no acoustic aggregation in TS"

requirements-completed: [WEB-11]

# Metrics
duration: ~90 min
completed: 2026-07-12
status: complete
---

# Phase 11 Plan 05: Receiver Spectrum Panel (WEB-11) Summary

**A receiver's per-band spectrum, dB(A)/dB(C) totals, and coherent/incoherent split are read entirely from WASM (`readout_receivers` over the OPFS tensor) and rendered by a chart-primary + expandable-table React panel with instant toggles and dual (map + list) receiver selection — the frontend performs zero acoustic arithmetic (D-01), proven by an offline Playwright UAT on the real bundle with a seeded OPFS tensor decoded by the real WASM reader.**

## Performance

- **Duration:** ~90 min (incl. a nightly `-Zbuild-std` compute-WASM rebuild + a `cargo build --release`)
- **Tasks:** 3 plan tasks, executed as 5 verified layers (Rust core → WASM boundary → WASM rebuild → frontend → Playwright)
- **Files:** 9 created, 10 modified (dist rebuilt)

## Accomplishments

- **`readout_receivers` WASM boundary (new).** Surfaces the FORCE-validated `readout_receiver` core across the WASM boundary, returning the full `ReceiverReadoutDto` per receiver — band levels + the coherent/incoherent per-band split + dB(A)/dB(C) + per-channel energetic totals. Reuses the `ReconditionReq` request shape + the same hash gate; adds `ReceiverReadoutDto` + `ReadoutResult` generated wire types (ts-rs no-drift). Native tests: the split reconstructs the combined level energetically; hash-gate refuses a mismatch.
- **`ReceiverReadout` core extension.** Added `coherent_db`/`incoherent_db` (per-band) + `total_coherent_db`/`total_incoherent_db`, computed via the SAME engine laws (`band_levels_db_two_channel` with the other channel zeroed; `weighted_total_db` with a zero table) — no bespoke dB. A silent channel's `−∞` clamps to a finite `SILENCE_FLOOR_DB` for the wire.
- **Results slice + main-thread readout.** `store/results.ts` holds the selected receiver, the display/weighting/split toggles, and cached readouts; an injectable `ReadoutClient` keeps the wasm out of the Node unit-test graph. The real client reads the covering OPFS chunk (`readChunk`, async File API) and calls `readout_receivers` on the main thread (the compute WASM is already main-thread for cost/plan). Toggles mutate state only — no recompute, no worker round-trip (D-09).
- **SpectrumPanel (D-05/06/07/08/09).** Inline-SVG bar chart (series colours from UI-SPEC #4) + an on-demand coherent/incoherent overlay + an expandable band-index·Hz·dB table (tabular-nums); 1/3-oct default ⇄ 1/12-oct expert (band-index aggregation 27⇄105); instant dB(A)⇄dB(C); dual receiver selection via a mini-map marker click AND a synced list, selected surface ringed. Honest empty/loading/error states.
- **Results shell + App mount.** `ResultsPanel` hosts the five result slots; the four not-yet-built (colour scale, conditioning, scenarios, export) are render-nothing stubs OWNED by 11-06..09 so the shell is written once and never re-edited. Mounted once in `App.tsx`.
- **Offline Playwright UAT on the real bundle.** `results-spectrum.spec.ts` seeds a fixture tensor into OPFS keyed by the real wasm-minted identity, then asserts every WEB-11 observable against the REAL `readout_receivers` decode — dual selection, band-count toggle, instant weighting with no loading, the split overlay — fully offline (`/api/*` route-mocked, zero network egress).

## Task → Layer Commits

1. `c82c4fa` feat(11-05): extend ReceiverReadout with per-channel dB + split totals
2. `f22a4ba` feat(11-05): add readout_receivers WASM boundary + ReceiverReadoutDto
3. (nightly `npm run build:wasm:compute` — the generated glue is git-ignored; it feeds the committed dist)
4. `e247f45` feat(11-05): receiver spectrum panel + results shell (WEB-11)
5. `dbbb14f` test(11-05): offline Playwright spectrum-panel UAT (WEB-11)

_Plan metadata commit follows this SUMMARY._

## Deviations from Plan

### Auto-fixed / Authorized structural additions (Rule 2/3/4)

**1. [Rule 4 — surfaced, then authorized] Added the `readout_receivers` WASM boundary + wire DTO (Rust, outside the plan's declared file list).**
- **Found during:** Task 1 investigation. The plan's Task 1 said "call the 11-01 WASM readout, and return the `ReceiverReadout` (band levels + both totals + split)", but no such `#[wasm_bindgen]` export existed — only `recondition`, which returns band levels only (no totals, no split). Satisfying the must-haves under D-01 was impossible with the existing exports.
- **Action:** Returned a checkpoint; the coordinator authorized **Option A**. Added `readout_receivers` (`recondition.rs`) + `ReceiverReadoutDto`/`ReadoutResult` generated wire types (`dto.rs`, registered in `wire_no_drift.rs`, `wire.ts` regenerated), and extended `ReceiverReadout` (`envi-compute/readout.rs`). The frozen `envi-engine` quarantine was NOT touched (`cargo tree -p envi-engine` unchanged); per-channel dB + totals use existing engine laws (no bespoke dB math). Existing recondition/recompute DTOs stayed byte-frozen.
- **Files:** crates/envi-compute/src/readout.rs, crates/envi-compute-wasm/src/{dto.rs,recondition.rs}, crates/envi-service/tests/wire_no_drift.rs, web/src/generated/wire.ts.

**2. [Rule 3 — blocking] Playwright drives the vite DEV server, not vite preview/dist; new spec named `results-spectrum.spec.ts`.**
- **Issue:** (a) The coordinator asked to serve `web/dist` via vite preview, but the OPFS+manifest seeding requires the DEV-only `testBridge`, which is absent from a production dist bundle (a hard project rule) — a dist-served test cannot seed. (b) The plan's declared spec filename `spectrum.spec.ts` is already taken by the unrelated SC3 SpectrumEditor test.
- **Action:** Ran the spec against the vite DEV server (the project's established e2e pattern — all specs drive the vite-served real app; the dev server already sends COOP `same-origin` + COEP `credentialless`, so `crossOriginIsolated` holds and the real threaded compute WASM instantiates). No new preview config or header injection was needed. Named the spec `results-spectrum.spec.ts` to avoid clobbering `spectrum.spec.ts`.
- **Files:** web/tests/e2e/results-spectrum.spec.ts (new); web/playwright.config.ts + vite.config.ts unchanged.

**3. [Rule 2 — missing critical support] `writeChunkFile` (opfs.ts) + `seedResults` (testBridge) + results CSS (app.css).**
- **Issue:** The offline UAT needs a way to seed a fixture tensor into OPFS and a manifest into the store; the panel needs styling per the UI-SPEC. `app.css` was not in the plan's declared file list.
- **Action:** Added `writeChunkFile` (async OPFS write, main-thread capable — the read path's inverse) and a DEV-only `seedResults` bridge helper; added a token-only results-panel CSS block. All additive, no existing behaviour changed.
- **Files:** web/src/compute/opfs.ts, web/src/testBridge.ts, web/src/app.css.

**Total deviations:** 3 (1 architectural/authorized, 2 blocking/support). No engine-core or frozen-wire regressions.

## Verification

All commands run at the workspace root (Rust) / `web/` (frontend) on `main`:

- `cargo fmt --check` → **clean** (exit 0).
- `cargo clippy --all-targets -- -D warnings` → **clean** (whole workspace).
- `cargo test` (workspace) → **all pass** — incl. the new `envi-compute` readout split tests (`per_channel_split_reconstructs_the_combined_band_level`, `empty_incoherent_channel_floors_instead_of_neg_infinity`), the new `envi-compute-wasm` `readout_all_receivers_returns_full_split_and_totals` / `_hash_gates`, and `wire_no_drift` (18 passed, generated wire.ts matches committed).
- `cargo build --release` → **Finished** (37.55s), exit 0.
- `cd web && npm run build:wasm:compute` → **Finished** (nightly `-Zbuild-std`, exit 0); the regenerated glue exposes `readout_receivers`.
- `cd web && npx vitest run` → **30 passed** (incl. the 4 `results` store tests: cache-once, no-recompute toggles, manifest supersede, honest error).
- `cd web && npm run build:web` (tsc --noEmit && vite build) → **built** (dist rebuilt + committed).
- `cd web && npx playwright test results-spectrum` → **1 passed** (offline, real bundle, real WASM readout over seeded OPFS).
- **D-01 grep gate:** `grep -c "Math.log10\|Math.pow\|Math.exp" web/src/store/results.ts web/src/panels/SpectrumPanel.tsx` → **0** (both files).
- `cargo tree -p envi-engine` → engine 3-dep quarantine unchanged (readout extension lives in envi-compute).

## Requirements

**WEB-11** — COMPLETE. The readout foundation (11-01) is now shipped as a user-facing spectrum panel: per-band levels + instant dB(A)⇄dB(C) totals + the coherent/incoherent split, all WASM-produced, with the 1/3⇄1/12 band-index toggle and dual receiver selection, proven by an offline UAT (SC1).

## Next Phase Readiness

- The `ResultsPanel` shell + its five slots are mounted; **11-06** (isophone map / `ColorScaleEditor`), **11-07** (`ConditioningPanel`), **11-08** (`ScenarioPanel`), **11-09** (`ExportMenu`) each fill their OWN stub file without editing the shell or App.tsx.
- `readout_receivers` + `ReceiverReadoutDto` + the `readChunk` glue are the reusable readout path the isophone map + conditioning surfaces consume.
- **Open follow-up:** the results manifest here is seeded via the DEV bridge for the UAT; wiring the calc worker's `TierComplete` spans into a live `setManifest` feed (the production results path) is a natural 11-06 companion.

## Self-Check: PASSED

- Created files exist on disk: `web/src/store/results.ts`, `web/src/panels/{ResultsPanel,SpectrumPanel}.tsx`, the 4 stubs, `web/tests/e2e/results-spectrum.spec.ts`.
- Task/layer commits present in `git log` (`c82c4fa`, `f22a4ba`, `e247f45`, `dbbb14f`).
- All gate commands re-run green (fmt/clippy/test/build/vitest/vite/playwright/grep above).

---
*Phase: 11-results-fast-recalc*
*Completed: 2026-07-12*
