---
phase: 11-results-fast-recalc
plan: 09
subsystem: results-export-ui
tags: [web, react, wasm, export, geotiff, geojson, csv, grid-05, d-20, d-22, attribution, playwright, blob-download]
status: complete

# Dependency graph
requires:
  - phase: 11-results-fast-recalc
    provides: "11-04 envi-compute-wasm::export (_export/export_filename) GeoTIFF/GeoJSON/CSV byte encoders + ExportReq wire types; 11-05 results store + ReadoutClient (readout_receivers); 11-06 colorScale store (cached grid/crs/breaks/colors/weightingLabel)"
  - phase: 10-calculation-service
    provides: "main-thread compute WASM module (COOP/COEP crossOriginIsolated), OPFS chunk read glue, threaded compute bundle"
provides:
  - "web/src/store/exportUi.ts — downloadExport(format): assembles ExportReq (CRS + weighting + engine/scene identity + open-data attribution footer, D-22), invokes the 11-04 WASM export encoder, wraps bytes in a Blob + objectURL, triggers a browser download (D-20 nothing leaves the device)"
  - "buildExportReq (pure, per-format payload selection) + browserDownloadSink (Blob/objectURL) + createWasmExportClient (main-thread _export/export_filename seam) + collectReadouts (CSV gathers every receiver via the results ReadoutClient)"
  - "web/src/panels/ExportMenu.tsx — the Export… .menu (GeoTIFF/GeoJSON/CSV) filling the 11-05 stub, disabled-until-result + disabled-when-stale + Generating… busy + inline encode-error states"
  - "web/tests/e2e/export.spec.ts — offline Playwright UAT: all 3 formats download non-empty bytes with attribution, GeoJSON parses as FeatureCollection, CSV has band-index+exact-Hz columns, menu gates on result + stale, zero network egress"
  - "web/dist rebuilt so the export path is live in the committed bundle"
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Export runs MAIN-THREAD via an injectable ExportClient seam (mirrors the readout/trace/identity clients) — the calc worker owns only the rayon-pool SOLVE; pure byte-generation WASM boundaries are main-thread with a dynamic-import factory keeping the wasm graph out of the Node unit test"
    - "downloadExport splits into a pure buildExportReq (unit-testable footer + payload assembly) + a client.encode + a DownloadSink.save (Blob/objectURL); both client and sink are injectable so the Node unit test asserts the dispatched ExportReq without wasm or the DOM"
    - "wasm-bindgen renames the `export` boundary to `_export` (reserved JS word); the glue exposes _export + export_filename, called on the main-thread compute module"
    - "CSV export gathers EVERY receiver's spectrum through the results ReadoutClient (WASM readout_receivers over the OPFS chunk), reusing the exact ReadoutRequest shape — complete CSV, zero TS acoustic math (D-01)"
    - "Deferred URL.revokeObjectURL (setTimeout 0) so Chromium's download manager reads the Blob before the object URL is freed"

key-files:
  created:
    - web/src/store/exportUi.ts
    - web/src/store/exportUi.test.ts
    - web/tests/e2e/export.spec.ts
  modified:
    - web/src/panels/ExportMenu.tsx
    - web/src/app.css
    - web/dist (rebuilt)

key-decisions:
  - "Export dispatched on the MAIN THREAD via an injectable ExportClient seam, NOT via a new worker.ts message (the plan named worker.ts). The established codebase pattern for pure byte-generation WASM boundaries — readout_receivers, trace_isophones, tensor_hash — is main-thread through a client seam; the compute worker owns the rayon-pool SOLVE job lifecycle exclusively. worker.ts left untouched; the must_haves artifacts (ExportMenu.tsx + exportUi.ts) and the key_link (exportUi→export.rs) are fully satisfied."
  - "The attribution footer uses a canonical DATA_ATTRIBUTION constant (© OpenStreetMap / Overture / ESA WorldCover / Copernicus — the UI-SPEC footer copy) so every export always carries open-data attribution (D-22), independent of which import sources a session used."
  - "engine_version stamped as the constant `envi 0.1.0` (matching the engine-side ExportMeta convention); scene identity = the results manifest tensor_hash."
  - "The whole Export menu gates on manifest present AND a cached grid+CRS (colorScale) — a result exists; all three formats enable/disable together, matching the UI-SPEC state matrix (disabled until a result exists / disabled while stale)."
  - "CSV export fetches every receiver's readout (not only the cached/selected ones) via the results ReadoutClient, so the exported spectra cover all receivers."

patterns-established:
  - "buildExportReq(format, ctx): pure per-format ExportReq assembly — footer on every format (D-22); grid for raster, breaks+fills for vector (cap-extended contourBreaks, matching the live isophone layer), receivers for CSV"
  - "browserDownloadSink(): Blob + URL.createObjectURL + hidden <a download> click + deferred revoke — the client-side download primitive (D-20, no server request)"

requirements-completed: [GRID-05]

# Metrics
duration: ~30 min
tasks: 3
files_created: 3
files_modified: 3
completed: 2026-07-12
---

# Phase 11 Plan 09: Results Export Menu (GeoTIFF / GeoJSON / CSV) Summary

**The results panel's "Export…" menu now downloads the continuous level grid as a GeoTIFF, the isophone fill polygons as a GeoJSON, and the receiver spectra as a band-index + exact-Hz CSV — every byte generated in WASM by the 11-04 encoders and saved via a Blob/objectURL so nothing leaves the device (D-20), every file carrying the full CRS + dB-weighting + engine/scene-identity + open-data-attribution footer (D-22), proven by an offline Playwright UAT on the real bundle + real WASM that captures each of the three downloads and inspects its bytes.**

## What was built

**Task 1 — export dispatch + Blob download (commit f432c87):**
- `web/src/store/exportUi.ts` — `downloadExport(format)`: gathers the export context from the client stores (the cached level grid + CRS + colour scale from `colorScale`, the tensor identity + per-receiver readouts from `results`), assembles the `ExportReq` with the full attribution footer (D-22), invokes the 11-04 WASM `_export` encoder, wraps the returned bytes in a `Blob`, and triggers a browser download via `URL.createObjectURL` + a hidden `<a download>`.
- Injectable `ExportClient` (main-thread `_export`/`export_filename` seam, dynamic-import factory) + `DownloadSink` (Blob/objectURL) so the Node unit test drives it with fakes; `buildExportReq` is pure (per-format payload selection). CSV gathers every receiver's spectrum through the results `ReadoutClient` (WASM `readout_receivers` over the OPFS chunk).
- `web/src/store/exportUi.test.ts` — asserts the footer on all three formats, the per-format payload selection, and the dispatch + sink save.

**Task 2 — Export menu UI (commit 920fb5c):**
- `web/src/panels/ExportMenu.tsx` fills the 11-05 stub: an "Export…" primary CTA opening a `.menu` of GeoTIFF (raster level grid) / GeoJSON (isophone polygons) / CSV (spectra: band index + exact Hz), each calling `downloadExport(format)`. Honest states (UI-SPEC state matrix): disabled until a result exists, disabled while stale, a "Generating…" busy label, and an inline encode-error message on failure.
- `web/src/app.css` — a small export-menu-item layout block (tokens only). `web/dist` rebuilt (full `npm run build`) so the export path is live in the committed bundle.

**Task 3 — offline Playwright UAT (commit 14eb1cc):**
- `web/tests/e2e/export.spec.ts` — on the real vite bundle, fully offline (COOP/COEP + `/api/*` mocks, fixture tensor via `seedResults` + level grid via `seedIsophone`), captures each format's download event and inspects its bytes: GeoTIFF is a non-empty little-endian TIFF whose ImageDescription carries `EPSG:32631` + `OpenStreetMap`; GeoJSON parses as a valid `FeatureCollection` with the attribution footer; CSV has `band_index,exact_hz` columns + a receiver column per seeded UUID + the `# Attribution:` footer. The menu is disabled with no result and after a scene edit (`divergeScene` → stale). Zero network egress.
- Hardened `exportUi.ts`: deferred `URL.revokeObjectURL` so Chromium's download manager reads the Blob before the object URL is freed.

## Verification

- `cd web && npm run typecheck` — clean.
- `cd web && npm run test:unit` — **60 passed** (9 files), incl. the new 4 `exportUi` tests (footer on all 3 formats, per-format payload, dispatch + sink save, filename base).
- `cd web && npm run build` — full build green (envi-gis-wasm + threaded compute-wasm rebuilt via nightly `-Zbuild-std`, tsc `--noEmit`, vite build); `web/dist` rebuilt and committed. The bundle contains `Export…`/`GeoTIFF`/`GeoJSON`.
- `cd web && npm run test:e2e -- export` — **1 passed** (offline, real bundle + real WASM export encoders; all three downloads captured with non-empty bytes + attribution; menu result/stale gating; unmocked network egress = empty).
- Task-1 grep gates: `grep -c createObjectURL web/src/store/exportUi.ts` = **2** (≥1); `grep -c "Math.log10\|Math.pow\|Math.exp" web/src/store/exportUi.ts` = **0** (D-01, no TS acoustic math).
- Wire no-drift: **unaffected** — `wire.ts` and all Rust DTOs unchanged (`git diff` vs base = no Rust/wire changes).
- `cargo tree -p envi-engine --depth 1` — **unchanged** (ndarray + num-complex + thiserror; the 3-dep quarantine intact).
- `cargo fmt --check` — clean. No Rust files changed this plan, so clippy/test are unaffected (green at the phase-10 close-out base).

## Deviations from Plan

**1. [Rule 3 — architecture consistency] Export dispatched MAIN-THREAD via a client seam, not a new `worker.ts` message.**
- **Found during:** Task 1 (read_first of `worker.ts`/`client.ts`). The plan's Task 1 action named `web/src/compute/worker.ts` for an "export message". The compute worker owns the rayon-pool SOLVE job lifecycle (submit/cancel/tier-complete) exclusively; every pure byte-generation WASM boundary in this codebase (`readout_receivers`, `trace_isophones`, `tensor_hash`) runs on the main thread through an injectable client seam.
- **Fix:** Implemented `createWasmExportClient` (main-thread `_export`/`export_filename`) mirroring the results/trace/identity clients. `worker.ts` left untouched. The must_haves artifacts (`ExportMenu.tsx` + `exportUi.ts`) and the key_link (`exportUi.ts` → `export.rs`, "invokes the WASM export boundary", pattern `export`) are satisfied.
- **Verification:** unit + e2e green; the export encoder runs in WASM on the crossOriginIsolated main-thread module.

**2. [Rule 1 — bug] Deferred `URL.revokeObjectURL`.**
- **Found during:** Task 3 (the UAT captured a truncated/empty GeoTIFF). A synchronous `revokeObjectURL` right after `anchor.click()` freed the object URL before Chromium's download manager had read the Blob.
- **Fix:** `setTimeout(() => URL.revokeObjectURL(url), 0)` in `browserDownloadSink`.
- **Files:** `web/src/store/exportUi.ts`. **Commit:** 14eb1cc.

**3. [Rule 2 — completeness] CSV exports every receiver, not only the cached/selected ones.**
- **Found during:** Task 1. The results store caches a readout only when the user selects a receiver; a CSV of only-viewed receivers would be incomplete.
- **Fix:** `collectReadouts` gathers each manifest receiver's spectrum, reusing the existing results `ReadoutClient` (WASM `readout_receivers` over the OPFS chunk) for any uncached one — a complete CSV, zero TS acoustic math (D-01).
- **Files:** `web/src/store/exportUi.ts`.

**Total deviations:** 3 (1 architecture-consistency, 1 bug, 1 completeness). No engine-core, wire-contract, or Rust changes. The plan's `files_modified` entry `web/src/compute/worker.ts` was intentionally NOT touched (deviation 1).

## Threat mitigations (from the plan threat_model)

- **T-11-09-01 (info disclosure):** all bytes generated in WASM; the download rides a `Blob` + `URL.createObjectURL` only — no server request, no filesystem path. The offline UAT asserts zero network egress (D-20).
- **T-11-09-02 (filename tampering):** the download filename is program-derived (`exportBase` → the WASM `export_filename`/`sanitize_export_filename`, V12) — an object URL, never a filesystem path.
- **T-11-09-03 (encode failure):** `downloadExport` failures surface as the honest inline encode-error state in `ExportMenu`; no silent partial file.

## Known Stubs

None. All three formats download real, self-describing bytes produced by the 11-04 WASM encoders; the menu enables only when a result exists and disables while stale.

## Self-Check: PASSED

- Created files exist on disk: `web/src/store/exportUi.ts`, `web/src/store/exportUi.test.ts`, `web/tests/e2e/export.spec.ts`.
- Task commits present in `git log`: f432c87 (Task 1), 920fb5c (Task 2), 14eb1cc (Task 3).
- Acceptance criteria re-run green: typecheck + unit (60 passed), full build (dist rebuilt, bundle carries the export path), export e2e (1 passed offline), grep gates (createObjectURL=2, acoustic-math=0), wire no-drift unaffected, engine 3-dep quarantine unchanged.

---
*Phase: 11-results-fast-recalc*
*Completed: 2026-07-12*
