---
phase: 11
name: results-fast-recalc
asvs_level: 1
block_on: high
threats_found: 33
threats_closed: 33
threats_open: 0
accepted_risks: 4
unregistered_flags: 0
status: secured
audited: 2026-07-12
---

# Phase 11 — Security Audit (results-fast-recalc)

Retroactive verification that every threat declared in the eleven `11-*-PLAN.md`
`<threat_model>` registers is actually mitigated in the implemented code (or is a
documented accepted risk). Implementation files were treated as READ-ONLY; this
file is the only artifact written.

**Verdict: SECURED.** All 33 threats resolve to CLOSED (30 `mitigate` verified in
code + tests; 3 `accept` logged below), plus 1 out-of-scope deferred item recorded
as accepted. No `mitigate` threat was found absent. No unregistered attack surface.

## Method

- Each threat classified by disposition (`mitigate` / `accept`) before verification.
- `mitigate`: located the actual validation/guard call in the cited file (not just a
  doc comment), and confirmed a driving test where one exists.
- `accept`: recorded in the Accepted Risks log below (verification for `accept` is
  presence of the log entry).
- SUMMARY.md files carry no `## Threat Flags` section (they document per-plan threat
  coverage instead) → **no unregistered flags** surfaced during implementation.

## Threat Register

| Threat ID | Category | Disp. | Status | Evidence (file:call) |
|-----------|----------|-------|--------|----------------------|
| T-11-01-01 | Tampering | mitigate | CLOSED | `crates/envi-compute-wasm/src/opfs_reader.rs:71` `read_chunk` — byte-length gate (`LengthMismatch`) + per-cell `is_finite` gate (`NonFinite`), typed `ChunkDecodeError`, no panic; tests `read_chunk_rejects_malformed_length` / `_non_finite` |
| T-11-01-02 | Integrity | mitigate | CLOSED | `recondition.rs:140` drives engine `readout_receiver` only (no bespoke MAC); `matching_hash_recondition_equals_engine_path_bit_for_bit` asserts `f64::to_bits` equality vs `compose_gain`+`readout_coherent` |
| T-11-01-03 | Info disclosure | accept | CLOSED | Accepted Risk AR-1 (IEC weighting constants are public analytic poles, cited by report; no standard text pasted) |
| T-11-02-01 | Tampering | mitigate | CLOSED | `crates/envi-compute/src/isoband.rs:147` `validate_breaks` — `<2` / non-finite / non-monotonic → typed `IsobandError`, never panics; `BreakScale::new` gate |
| T-11-02-02 | DoS | mitigate | CLOSED | `isoband.rs:178` `trace_isobands` is O(cells×breaks); grid dims built from the tier plan receivers (`crates/envi-compute/src/grid.rs:116` from `fine_tier.receivers`, bounded by `cost.rs` estimate), not unbounded user input |
| T-11-02-03 | Integrity | mitigate | CLOSED | `isoband.rs:364` mean-value saddle rule; property tests `saddle_grid_traces_non_crossing_bands` / `peak_grid_bands_are_nested_non_crossing_and_shrink_inward` assert non-overlap |
| T-11-03-01 | Spoofing | mitigate | CLOSED | `recondition.rs:98` `gated_readout` refuses `req.tensor_hash != current` with `ComputeError::HashMismatch{expected,got}` BEFORE any MAC (client-side 409); test `mismatched_hash_returns_hashmismatch_and_no_spectra` |
| T-11-03-02 | Tampering | mitigate | CLOSED | `recondition.rs:197` `to_engine` — dense `[105]` length check + per-value `is_finite` on filter, `gain_db`/`delay_ms` finiteness → typed `Recondition` error; test `non_dense_filter_is_a_typed_error` |
| T-11-03-03 | Integrity | mitigate | CLOSED | Same bit-exact MAC≡engine test as T-11-01-02 (`matching_hash_recondition_equals_engine_path_bit_for_bit`) |
| T-11-04-01 | Info disclosure | mitigate | CLOSED | `crates/envi-compute-wasm/src/export.rs:281` `export` returns `Vec<u8>` only; no fetch/XHR/network in the export path (grep-confirmed); browser Blob-downloads |
| T-11-04-02 | Tampering | mitigate | CLOSED | `export.rs:245` `sanitize_export_filename` strips separators/`..`/NUL/control chars, collapses `..`, falls back to `export`; test `filename_is_sanitized_against_path_traversal` |
| T-11-04-03 | Tampering | mitigate | CLOSED | `export.rs:115` `run_export` — every failure a typed `ComputeError::Export`, single-strip bounded GeoTIFF; tests `missing_payload_is_a_typed_error_never_a_panic`, `out_of_range_utm_zone_...` (UTM `validated_zone` 1..=60, WR-03) |
| T-11-05-01 | Integrity | mitigate | CLOSED* | `web/src/panels/SpectrumPanel.tsx` carries no acoustic readout math (line 333-334 `Math.ceil/floor` are chart-axis ticks only); all spectra come from the WASM `readout_receivers` boundary. *See Note 1 — the "grep gate" is a review discipline, not an automated CI test. |
| T-11-05-02 | Tampering | mitigate | CLOSED | `web/src/compute/opfs.ts:100/251/296` build every OPFS path from `safeSeg(projectId)` + `assertHex(tensorHash)` (V12); `createSyncAccessHandle` is dedicated-worker-only (Pitfall 4) |
| T-11-05-03 | Availability | mitigate | CLOSED | `opfs_reader.rs` typed `ChunkDecodeError` (11-01) surfaces "result data unavailable" honest state; no panic path |
| T-11-06-01 | Tampering | mitigate | CLOSED | `web/src/store/colorScale.ts:134` inline monotonic/finite/`≥2` validation (V5) AND WASM `trace_isobands` (11-02); typed error both sides |
| T-11-06-02 | Integrity | mitigate | CLOSED | `web/src/map/isophoneLayer.ts` fill layer over the WASM tracer, never a heatmap raster (D-02); single `breaks[]/colors[]` source of truth in `colorScale.ts` |
| T-11-06-03 | Availability | mitigate | CLOSED | `colorScale.ts:5/40/178` + `isophoneLayer.ts:48` — a break edit re-contours THIS cached grid via `traceIsophones`, NO re-solve (SC3) |
| T-11-07-01 | Spoofing | mitigate | CLOSED | `web/src/store/stale.ts:57-87` re-mints blake3 identity; a `staleGen` generation counter prevents an out-of-order re-mint from writing a false-green over a newer stale verdict; WASM refuses the mismatched MAC (SVC-06) |
| T-11-07-02 | Integrity | mitigate | CLOSED | MAC runs in the WASM `recondition`/`readout_receivers` boundary; TS conditioning store performs no acoustic math (values from WASM) |
| T-11-07-03 | Tampering | mitigate | CLOSED | Gain/delay/filter validated in the 11-03 WASM boundary (`to_engine`, V5) — same evidence as T-11-03-02 |
| T-11-08-01 | Tampering | mitigate | CLOSED | `crates/envi-gis/src/weather.rs:227` `components_from_friendly` — finiteness gate + physical-range gate on temperature/gradient/wind (typed `GisError`); z0 clamped `≥ Z0_MIN_M`; Beaufort→m/s clamped to the WMO table caller-side (`scenarios.ts:49`) |
| T-11-08-02 | Integrity | mitigate | CLOSED | `web/src/store/scenarios.ts:98/238` each scenario keyed by its OWN `tensorHash` into a per-scenario OPFS `calc/<hash>/` dir; switching loads the matching cached tensor (no cross-serve) |
| T-11-08-03 | Info disclosure | accept | CLOSED | Accepted Risk AR-2 (per-scenario tensors cached locally in OPFS; quota/eviction deferred) |
| T-11-09-01 | Info disclosure | mitigate | CLOSED | `web/src/store/exportUi.ts:151-165` wraps WASM bytes in a `Blob` + `URL.createObjectURL`; no fetch/XHR/WebSocket/sendBeacon in the path (grep-confirmed) — nothing leaves the device |
| T-11-09-02 | Tampering | mitigate | CLOSED | `exportUi.ts:117/120` download name comes from the WASM `export_filename` (sanitized, T-11-04-02); objectURL, no filesystem path |
| T-11-09-03 | Availability | mitigate | CLOSED | `exportUi.ts:195/199/224/227` throws an honest `Error` on no-engine/no-chunk/no-result/no-CRS; no silent partial file |
| T-11-10-01 | Integrity | mitigate | CLOSED | Display-only restyle in `web/src/map/objectStyles.ts`, separate from `useTerraDraw.ts` (summary `git diff --stat` empty); Phase-7 draw-journey regression guard green (28-test e2e) |
| T-11-10-02 | Tampering | accept | CLOSED | Accepted Risk AR-3 (canvas hatch/marker rasters program-generated from the fixed `objectStyles` table; `objectStyles.ts:10/93/99` "no user string ever reaches image generation" — no XSS surface; no `innerHTML`/`eval`/`new Function`) |
| T-11-10-03 | Integrity | mitigate | CLOSED | `objectStyles.ts` dataviz-validated categorical palette + per-geometry hatch/symbol secondary channel + semi-transparent fills (D-18/D-19); UAT draw-order check |
| T-11-11-01 | Integrity | mitigate | CLOSED | `web/src/help/coverage.test.ts:34` iterates the `ControlId` enumeration; a control without a `catalog` entry fails the test (and is already a `tsc` error via `Record<ControlId, HelpEntry>`) — D-25 |
| T-11-11-02 | Info disclosure / licensing | mitigate | CLOSED | `web/src/help/catalog.ts:16-40` cites AV 1106/07 by report number in ENVI's own words; explicit "⚠ COPYRIGHTED — cite, never paste" discipline; spot-check found no pasted standard text/equations |
| T-11-11-03 | Integrity | mitigate | CLOSED | Additive InfoButton affordance only; full Playwright suite green (per 11-11 summary) |

## Accepted Risks

| ID | Threat | Rationale |
|----|--------|-----------|
| AR-1 | T-11-01-03 (info disclosure — IEC weighting constants) | The A/C-weighting pole frequencies are public analytic constants published in IEC 61672. They are cited by report number and implemented as numeric literals; no copyrighted standard text is reproduced. Nothing sensitive is disclosed. |
| AR-2 | T-11-08-03 (info disclosure — per-scenario tensors) | Every scenario's complex tensor is cached locally in the browser's OPFS (Phase-10 D-09 keeps everything). Data never leaves the device. Quota/eviction management is explicitly deferred (CONTEXT deferred items) — acceptable for a single-user self-hosted tool. |
| AR-3 | T-11-10-02 (tampering — canvas hatch generation) | Hatch/marker rasters are generated programmatically from the fixed 9-kind `objectStyles` table. No user-controlled string reaches image generation, and the UI uses no `innerHTML`/`eval`/`new Function`/`dangerouslySetInnerHTML` — there is no XSS surface to guard. |
| AR-4 | Real authentication / login (deferred) | Out of scope for Phase 11 per Phase-10 D-12 (WASM deployment pivot). Compute + export are fully client-side with no network egress, so no server-side authZ surface exists in this phase. Login server is a separate deferred milestone item — not a Phase-11 gap. |

## Cross-Reference: Code-Review Findings (already fixed)

The `11-REVIEW-*.md` passes found and fixed several security-relevant issues; all
are verified present in the audited tree:

- **CSV formula-injection / RFC-4180** (WR-02) — `crates/envi-compute/src/export/csv.rs:94` `csv_field` guards leading `= + - @ \t \r` with a `'` prefix and RFC-4180-quotes comma/quote/newline; test `labels_are_rfc4180_quoted_and_formula_injection_guarded`.
- **UTM-zone validation** (WR-03) — `export.rs:50` `validated_zone` enforces `1..=60` before any `as u8`, preventing `287 as u8 == 31` CRS/EPSG mismatch; test `out_of_range_utm_zone_is_a_typed_error_never_a_silent_truncation`.
- **OPFS decode finiteness** — `opfs_reader.rs` per-cell `is_finite` gate (T-11-01-01).
- **Export-filename path traversal** — `sanitize_export_filename` (T-11-04-02).
- **Honest 409 / stale states** — `gated_readout` HashMismatch + `stale.ts` generation counter (T-11-03-01 / T-11-07-01).

## Notes

1. **T-11-05-01 "grep gate" is a review discipline, not an automated test.** The
   plan described a grep gate asserting no `Math.log10`/`Math.pow` in the spectrum
   UI. No source-scanning CI test was found. The substantive integrity property
   nonetheless holds: SpectrumPanel/conditioning derive every acoustic value from
   the WASM `readout_receivers` boundary and contain no level-computation math (the
   only `Math.*` in `SpectrumPanel.tsx` is chart-axis rounding; the only
   `Math.log10` in `web/src` is `weatherOverlay.ts`, a σ colour-scale display, not a
   spectrum readout). Classified CLOSED. Recommendation (non-blocking, below the
   `block_on: high` bar): add an automated vitest that scans `web/src/panels` /
   `web/src/spectrum` for `Math.log10`/`Math.pow` so the discipline can't silently
   regress.

## Audit Trail

- Loaded all eleven `11-*-PLAN.md` `<threat_model>` registers (33 threats).
- Loaded all `11-*-SUMMARY.md` — confirmed no `## Threat Flags` section → 0 unregistered flags.
- Cross-referenced `11-REVIEW-rust.md` / `-weblogic.md` / `-webui.md` fixes.
- Verified each `mitigate` threat against the actual guard/validation call in code + its driving test.
- Verified `accept` threats are logged in Accepted Risks (AR-1..AR-3); deferred auth logged AR-4.
- Implementation files unchanged (read-only).
