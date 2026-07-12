---
phase: 11-results-fast-recalc
verified: 2026-07-12T18:14:27Z
status: passed
score: 5/5 success criteria verified (SC1, SC2, SC3 fully; SC4, SC5 capability-verified with a single documented live-data-feed deferral)
verifier: Claude (gsd-verifier, goal-backward)
method: goal-backward — each SC traced to a source symbol + a passing test/spec that exercises the observable behavior
deferrals:
  - name: "Production live-calc → results data-feed wiring"
    affects: [SC1-livefeed, SC3-livefeed, SC4-solve, SC5-livefeed]
    what: >
      Wiring a finished multi-tier calc-worker solve into the results surface:
      (a) a `#[wasm_bindgen]` boundary over the tested `envi_compute::grid::reconstruct_level_grid`
      (fine-tier lattice → LevelGrid), (b) `calc.applyTierComplete` → live `useResultsStore.setManifest`
      + `useColorScaleStore.setIsophoneInput` from a finished readout, and
      (c) the scenario `ScenarioComputeClient.solve()` seam dispatched to the live Phase-10 calc worker
      to completion (currently a fixture readout in the offline UAT).
    status: capability-shipped-proven-against-seeded-data; live-feed is an explicit carried follow-up
    documented_in: [11-05-SUMMARY.md:142, 11-06-SUMMARY.md:153, 11-07-SUMMARY.md:104-141, 11-08-SUMMARY.md:106-140]
    evidence_of_deferral: >
      `setManifest`/`setIsophoneInput` are called only from their store definitions, `conditioning.ts`,
      and the DEV `testBridge.ts` — never from `calc.ts::applyTierComplete` (grep-confirmed).
      `reconstruct_level_grid` exists in `grid.rs:114` with unit tests but has no WASM export.
    not_a_gap_because: >
      Every SC's underlying capability (WASM/tensor math + UI + honest states) is genuinely implemented
      and proven by real-WASM-driven unit + Playwright tests; only the last-mile calc→render plumbing is
      fixture-seeded via the DEV test-bridge. Per the verification mandate, a test-bridge-seeded demo does
      not fail a criterion whose capability + honest states are real — the wiring is surfaced here so it is
      not silently lost.
---

# Phase 11: Results & Fast Recalc — Verification Report

**Phase Goal:** Turn the cached complex transfer tensor into readable, interactive results — spectral readout at receivers, dB(A)/dB(C) isophone fill maps, the flagship interactive source conditioning over the cached tensor, named weather what-if scenarios with difference maps, and exports.

**Verified:** 2026-07-12T18:14:27Z
**Status:** passed (all 5 SCs; SC4/SC5 carry one shared, documented live-data-feed follow-up)
**Re-verification:** No — initial verification.

## Method & Evidence Base

Goal-backward: each SC was traced from the observable behavior back to a concrete source symbol and a passing test/spec that exercises it. Tests executed read-only during verification:

- `cargo test -p envi-compute --lib` → **69 passed, 0 failed** (readout laws, IEC A/C anchors, isoband tracer incl. 100k-cell perf budget, tiers, DTOs).
- `cargo test -p envi-compute-wasm --lib` → **52 passed, 0 failed** — includes the load-bearing `matching_hash_recondition_equals_engine_path_bit_for_bit` (MAC ≡ engine, SC2) and `readout_all_receivers_hash_gates` (409 gate).
- `npx vitest run` (web) → **13 files, 159 passed, 0 failed** — includes `d01-no-acoustic-math.test.ts`, `colorScale`, `stale`, `conditioning`, `results`, `scenarios`, `exportUi`, `objectStyles`, help `coverage`, `InfoButton`.
- Playwright E2E specs (`results-spectrum`, `isophone`, `conditioning`, `scenarios`, `export`, `objectStyling`, `infoButton`) inspected: they drive the **real** vite-served app and **real WASM** cores, asserting the SC observables; data is DEV-bridge-seeded (the project's established offline-UAT pattern). Not re-executed here (require a WASM build + headless browser); their assertions were read and cross-checked against the passing unit layer that drives the same cores.

## Observable Truths (Success Criteria)

| #   | Success Criterion | Status | Evidence |
| --- | ----------------- | ------ | -------- |
| SC1 | Receiver spectrum readout: per-band levels (1/3-oct default ⇄ 1/12-oct by band index), instant dB(A)/dB(C), coherent/incoherent split, **zero client acoustic math** | ✓ VERIFIED | `envi-compute/readout.rs::readout_receiver` drives frozen engine laws (`compose_gain`/`readout_coherent`/`readout_incoherent`/`band_levels_db_two_channel`), never a bespoke dB/MAC loop; `weighting.rs` A/C tables at exact grid centres (IEC 61672 poles, `third_octave_anchors_match_iec_table3`, `one_khz_is_normalized_to_zero`). `SpectrumPanel.tsx` toggles 1/3⇄1/12 by band index (`displayIndices`), dB(A)⇄dB(C), split overlay. `d01-no-acoustic-math.test.ts` (passing) forbids `Math.log10/log2/pow/exp` under `store`/`panels`/`spectrum` — enforces D-01. `results-spectrum.spec.ts` asserts band-count change, weighting toggle with no reload, split totals. |
| SC2 | Interactive conditioning via tensor MAC (no re-propagation): live gain/filter/delay recalc, results-stale badge, hard 409 hash-mismatch rejection; MAC ≡ full-recompute equivalence | ✓ VERIFIED | `envi-compute-wasm/recondition.rs::gated_readout` refuses `req.tensor_hash != current` with typed `HashMismatch` **before any MAC**; `matching_hash_recondition_equals_engine_path_bit_for_bit` proves MAC≡engine to `f64::to_bits`. `stale.ts` re-mints blake3 identity with a `staleGen` monotonic guard (out-of-order false-green prevented); conditioning excluded from identity → never stales (D-07). `conditioning.spec.ts` (real WASM): gain edit bumps recalc epoch + map re-contours while **calc job stays `idle`** (no re-propagation), conditioning never stales, a simulated scene edit flips the badge, a MAC on the mismatched hash surfaces the honest 409 reject banner. |
| SC3 | Isophone **fill** polygons (no heatmap) + editable color scale; editing the scale **re-contours the cached grid without re-running propagation**; legend ≡ contour ≡ class colors; weighting label from metadata | ✓ VERIFIED | `envi-compute/isoband.rs::trace_isobands` — hand-rolled interpolated marching-squares, mean-value saddle rule, nested non-crossing bands (`saddle_grid_traces_non_crossing_bands`, 100k-cell perf test). `isophoneLayer.ts` paints a MapLibre **`fill`** layer (telemetry asserts `layerType === "fill"`, never raster). `colorScale.ts` is the single `breaks[]`/`colors[]` source; a break edit re-derives colors + re-contours **the cached grid** via the WASM tracer. `isophone.spec.ts`: preset recolour + break re-contour advance the trace count, legend row labels/colors ≡ breaks/colors, and the whole path makes **no network request** (re-contour, not re-solve). |
| SC4 | Weather what-if + named scenarios (each its own cached tensor, instant switch) + difference maps | ✓ VERIFIED (capability) — see deferral | `scenarios.ts` clone-then-edit; each scenario keyed by its OWN `tensorHash` into a per-scenario OPFS dir (`scenarios.test.ts`, security T-11-08-02). Friendly→A/B/C runs **real WASM** `derive_weather_friendly` (`envi-gis/weather.rs::components_from_friendly`, range/finiteness-gated). `envi-gis-wasm/lib.rs:767 difference_dba` computes the A−B delta in WASM; `differenceLayer.ts` renders a diverging **fill** (gray-at-0). `scenarios.spec.ts`: clone is un-computed until it solves its own identity, instant switch bumps switchEpoch with no recompute, Compare renders the diverging fill. **Deferral:** the scenario `solve()` seam is fixture-seeded in the UAT; live dispatch to the Phase-10 calc worker is the carried follow-up. |
| SC5 | Exports (GeoTIFF grid / GeoJSON isophones / CSV spectra) with band index + exact Hz + full attribution | ✓ VERIFIED (capability) — see deferral | `envi-compute/src/export/{geotiff,geojson,csv,mod}.rs` — hand-rolled encoders; `ExportMeta` stamps CRS + dB weighting label + engine/scene identity + OSM/Overture/ESA WorldCover/Copernicus attribution (D-22) onto **every** export; CSV rows are `band_index,exact_hz,…` with exact Hz from `FreqAxis::centres` (never nominal). Guards: `sanitize_export_filename` (path-traversal), `validated_zone` 1..=60, `csv_field` RFC-4180 + formula-injection (all tested). `exportUi.ts` Blob + objectURL download, **no network egress** (grep-confirmed, T-11-09-01), gated on result + not-stale. `export.spec.ts`: all three formats download offline with the metadata footer; GeoJSON parses as a FeatureCollection; CSV carries band-index + exact-Hz. **Deferral:** the level grid feeding GeoTIFF/isophone is fixture-seeded in the UAT (same live-feed follow-up). |

**Score:** 5/5 success criteria verified. SC1–SC3 fully. SC4–SC5 capability-verified with the single shared live-data-feed deferral below.

## D-01 Compliance (zero JS/TS acoustic math)

VERIFIED. All acoustic arithmetic (MAC readout, A/C weighting, band aggregation, coherent/incoherent split, contouring, export byte generation, A−B difference) executes in Rust→WASM (`envi-compute`, `envi-compute-wasm`, `envi-gis-wasm`). The TS layer marshals and renders WASM-produced values. The `d01-no-acoustic-math.test.ts` regression test (added after 11-SECURITY.md Note 1 recommended converting the review-time grep discipline into CI) passes and forbids dB-derivation `Math.*` under the results surfaces. The only `Math.log10` in `web/src` is `weatherOverlay.ts` (a σ flow-resistivity display colour transform, not a spectrum readout) — out of scope and documented.

## Requirements Coverage

| Requirement | Status | Evidence |
| ----------- | ------ | -------- |
| WEB-11 (spectrum readout) | ✓ SATISFIED | SC1 |
| WEB-05 / SVC-06 (conditioning MAC + 409) | ✓ SATISFIED | SC2 (`gated_readout`, MAC≡engine test) |
| WEB-06 / GRID-04 (isophone fill + editable scale) | ✓ SATISFIED | SC3 (`trace_isobands`, `colorScale.ts`, isophone.spec) |
| WEB-12 / METX-03 / METX-04 (scenarios + weather what-if + diff map) | ✓ SATISFIED (capability) | SC4 (per-scenario hash tensors, real WASM friendly derive + `difference_dba`); live solve() feed deferred |
| GRID-05 (exports) | ✓ SATISFIED | SC5 (GeoTIFF/GeoJSON/CSV encoders + attribution) |

No orphaned requirements: all 9 phase requirements (SVC-06, WEB-05/06/11/12, METX-03/04, GRID-04/05) map to verified capability.

## Anti-Patterns Scan

No blocking anti-patterns in the phase's changed files. The gates already ran and are recorded: `11-REVIEW-{rust,weblogic,webui}.md` (findings fixed — CSV formula-injection WR-02, UTM-zone validation WR-03, OPFS decode finiteness, filename traversal, honest 409/stale), `11-SIMPLIFY.md`, `11-SECURITY.md` (33/33 threats CLOSED, 4 accepted risks logged). No unreferenced `TBD`/`FIXME`/`XXX` debt markers surfaced. The "follow-up" / "deferred" markers in the summaries all reference the single named live-data-feed carry below (formal, documented work), not unaudited debt.

## Named Follow-Up (carried — NOT a gap)

**Production live-calc → results data-feed wiring.** The one cross-cutting item deferred across 11-05/06/07/08 and confirmed in code:

1. `envi_compute::grid::reconstruct_level_grid` (fine-tier lattice → `LevelGrid`) exists and is unit-tested (`grid.rs:114`) but has **no `#[wasm_bindgen]` boundary** yet.
2. `calc.ts::applyTierComplete` fills per-tier receiver counts but does **not** call `useResultsStore.setManifest` / `useColorScaleStore.setIsophoneInput`; those are driven only by store definitions, `conditioning.ts`, and the DEV `testBridge.ts`. So a finished live multi-tier solve does not yet populate the results manifest / isophone level grid.
3. The scenario `ScenarioComputeClient.solve()` seam is ready but, in the offline UAT, returns a fixture readout rather than dispatching the Phase-10 calc worker to completion.

Why this is a follow-up and not a gap: the SC capabilities — the WASM readout/MAC/weighting/contour/difference/export math, the panels, and the honest states (409 refusal, stale badge, never-stale conditioning, fill-not-raster, no-network re-contour) — are all genuinely implemented and proven against real WASM via the seeded fixtures. Only the last-mile calc→render plumbing (steps 1–3) is fixture-seeded. This must be wired before Phase 11 is usable end-to-end on a live-computed project; it is captured here so it is not silently lost.

## Gaps Summary

No blocking gaps. All five success criteria are delivered as genuine, tested capabilities with honest failure/stale states. The single shared live-data-feed wiring (reconstruct_level_grid WASM boundary + TierComplete→setManifest/setIsophoneInput + scenario solve() to the live worker) is an explicit, code-confirmed, consistently-documented carried follow-up, not a silent omission.

---

## Update (11-12) — production feed partially closed

The carried follow-up was partially closed after verification:

- **`reconstruct_level_grid` WASM boundary** — added in `envi-compute-wasm` (returns the existing `ExportGridDto`; `ExportGridDto` gained `Serialize`, no wire drift). Two native tests cover the typed body (`reconstruct_level_grid_typed`).
- **`applyTierComplete → setManifest` link** — `web/src/compute/resultsFeed.ts` (`buildResultsManifest` + `applyResultsFeed`) assembles the results manifest from the submitted `CalcJobSpec` + the fine `TierComplete` and pushes it into the results store (attaching the real readout/conditioning/stale clients). Wired into `CalcPanel` on the fine tier. Unit-tested (`resultsFeed.test.ts`, 3 tests) and exercised end-to-end through the REAL feed by the new `feedFromSolve` DEV bridge + `tests/e2e/results-flow.spec.ts` (a single offline session that walks feed → spectrum → info-button → isophone re-contour → object styling against real WASM, zero egress). Spectrum (SC1) + conditioning (SC2) now light up from a solve-shaped feed, not just `setManifest`.

**Two upstream blockers remain (larger than last-mile, deliberately deferred):**

1. **Phase-10 `10-03` threaded-WASM build gap** — `build:wasm:compute` ships a non-shared `WebAssembly.Memory`, so `initThreadPool` cannot start; a real threaded solve never reaches the fine tier (`calc.spec.ts` Test 2 skips honestly for this exact reason). Until fixed, the `applyResultsFeed` link is correct-but-dormant against a real Run (it is proven via the bridge/unit tests).
2. **2-D CRS-exact scene marshalling** — `marshalScene.ts` places all receivers on a 1-D corridor (`y=0`), so there is no 2-D field to reconstruct into a meaningful isophone map; the isophone/scenario *production* map feed is intentionally NOT wired to a degenerate grid (which would render a false noise map). The plan already exposes 2-D `TierReceiverDto.position`; wiring it requires engine-validated 2-D receiver-lattice geometry + per-path terrain.

_Updated: 2026-07-12_

---

_Verified: 2026-07-12T18:14:27Z_
_Verifier: Claude (gsd-verifier)_
