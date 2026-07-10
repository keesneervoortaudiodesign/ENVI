---
phase: 07
name: frontend-shell-scene-editing
verified: 2026-07-10T00:00:00Z
status: passed
criteria_total: 4
criteria_met: 4
score: 4/4 success criteria met
behavior_unverified: 0
overrides_applied: 0
verifier: gsd-verifier (goal-backward, code-run against current tree)
gates:
  cargo_test_workspace: pass
  cargo_clippy: pass
  cargo_fmt_check: pass
  tsc_noEmit: pass
  npm_build: pass
  npm_test_unit: "12 passed"
  playwright_e2e: "14 passed, fully offline"
  engine_quarantine: "ndarray+num-complex+thiserror only; git diff clean"
  dgm_boundary: "0 envi-engine dep; 0 C crates; 0 spawn_blocking"
  conj_gate: 0
  external_assets_index_html: 0
  static_bundle_contract: pass
  wire_no_drift: pass
requirements_coverage:
  WEB-01: satisfied
  WEB-02: satisfied
  WEB-03: satisfied
  WEB-04: satisfied
  WEB-08: satisfied
  WEB-09: satisfied
  WEB-10: satisfied
  SCN-01: satisfied
  SCN-02: satisfied
  SCN-03: satisfied
  SCN-04: satisfied
residuals:
  - "Authored 105-band isolation / L_W spectra are session-only this phase (not serialized into PUT /scene). Decided scope (D-01: TryFrom proven, solve-time attachment deferred to Phase 9/10); honestly surfaced via the 'Spectra session-only' warn chip — not a false green."
  - "E2E drives the store via the DEV-only window.__enviTest bridge over the Vite dev server, not real mouse-driven Terra Draw WebGL drawing nor the built web/dist bundle. The bridge calls the same store paths a finished TD edit uses; real-browser drawing UX is not automatically exercised."
  - "DGM TIN overlay triangulates raw [lng,lat,z] degrees in Euclidean space (~1.6:1 skew at 52°N) — a labeled Phase-8 projection stub (module header), never presented as a real elevation result. Accepted risk LO-01."
  - "scene_to_engine maps 4/9 kinds (source/receiver/wall/building) by decision D-01; ground_zone→Phase 9, elevation_*→Phase 8, calc_area→Phase 10. No scope reduction — this is the decided walking-skeleton cut."
---

# Phase 7: Frontend Shell & Scene Editing — Verification Report

**Phase Goal:** Users can draw and edit the complete NoizCalc-style scene on an OSM basemap —
including ENVI's semi-transparent screens/buildings and forests — with the scene persisted through
the Phase-6 API and validated at draw time.

**Verified:** 2026-07-10 · **Status:** passed (4/4) · **Re-verification:** No — initial

Every verdict below is backed by a command I ran or a file I read in the current tree, not by
SUMMARY/PLAN/REVIEW prose.

## Gate Results (all run, all green)

| Gate | Command | Result |
|------|---------|--------|
| Rust tests | `cargo test --workspace` | exit 0 — all binaries pass (envi-dgm 11, envi-store 33, envi-service unit 4 + contract_dgm 4 + contract_interpolate_spectrum 6 + contract_jobs 3 + contract_projects 3 + contract_meta_static 2 + contract_calc 7 + wire_no_drift 2) |
| Lint | `cargo clippy --all-targets -- -D warnings` | clean (Finished, no warnings) |
| Format | `cargo fmt --check` | clean (exit 0) |
| TS typecheck | `npx tsc --noEmit` | clean (exit 0) |
| Frontend build | `npm run build` | 357 modules, `web/dist` built |
| Unit tests | `npm run test:unit` | 12 passed (ring-diff D-02) |
| E2E | `npx playwright test` | 14 passed, 12.1s, **fully offline** (every spec asserts `unmocked == []`) |
| Engine quarantine | `cargo tree -p envi-engine -e normal --depth 1` | exactly `ndarray`, `num-complex`, `thiserror` |
| Engine untouched | `git diff --stat crates/envi-engine/ crates/envi-harness/` | empty |
| DGM boundary | `cargo tree -p envi-dgm \| grep -c envi-engine` | 0 |
| No C crates | `cargo tree \| grep -ci 'proj-sys\|gdal'` | 0 |
| No blocking-pool CPU | `grep -rc spawn_blocking crates/envi-service/src` | 0 |
| Conj gate | `grep -rh '.conj()' crates/envi-engine/src/propagation/ \| grep -vc '//'` | 0 |
| Offline bundle | `grep -Ec 'https?://' web/dist/index.html` | 0 |
| Static bundle contract | `cargo test -p envi-service static_bundle_served_with_spa_fallback` | 1 passed |
| Wire no-drift | `wire_ts_matches_committed_source` | passed (the `--ignored` case is the regenerator/writer, not the check — correct committed-artifact pattern) |

## Observable Truths (Success Criteria)

| # | Criterion | Verdict | Proving test(s) — would a no-op fail? |
|---|-----------|---------|----------------------------------------|
| SC1 | Place & edit every scene kind + DGM re-triangulation + last-object inheritance | **Met** | Yes |
| SC2 | Partial-cross ground-zone rejected at draw time; validation click-to-select + zoom | **Met** | Yes |
| SC3 | Semi-transparent screen + isolation spectrum; per-façade spectra; octave/third interpolated onto exact band indices | **Met** | Yes |
| SC4 | Scene survives basemap switch + reload + close/reopen; TD re-hydrates from store on `style.load` | **Met** (geometry+properties; spectra a labeled residual) | Yes |

**Score: 4/4 met.**

### SC1 — Met
`web/tests/e2e/sc1-author-every-kind.spec.ts:43` places all 9 locked kinds through the palette
(`tool-<kind>` → active-kind tag → commit) and asserts each appears in the store
(`__enviTest.state().kinds`) **and** in the coalesced `PUT /scene` payload with a valid
`properties.kind` (lines 69-73, 134-137). Ground-zone authored via closed enum selects `A–H` /
`N-S-M-L` (`ground-impedance`/`ground-roughness`, lines 79-84); forest via numeric fields; sources
carry a sound-power spectrum. Last-object inheritance is asserted on a repeated `ground_zone`
(second inherits `C`/`M`, chip "inherited from last ground_zone", then clears on edit — lines 82-91).
SC1's "re-triangulate the DGM" is proven observable: 3 non-collinear `elevation_point`s fire the
debounced `POST /dgm/triangulate` and render a TIN overlay whose triangle count goes to `1`
(`dgm-triangle-count`, line 100) — a no-op producer would leave it at 0. Corroborated by
`draw-kinds.spec.ts:40` (9 kinds + inheritance) and `dgm-trigger.spec.ts:24` (TIN renders).
Backend: `scene_to_engine` maps 4/9 kinds (source/receiver/wall/building — `geojson.rs:107-110`),
exactly the decided D-01 cut (verified NOT extended).

### SC2 — Met
`web/tests/e2e/validation.spec.ts:20` proves the D-07 hard reject: a contained zone commits
(`outcome: "contained"`), a partially-crossing zone reverts (`outcome: "partial-cross"`, `id: null`
— never committed) and names the **existing** crossed zone (`conflictId === a.id`), with the reject
banner's "Zoom to conflicting zone" targeting zone A (lines 62-69). The persistent WEB-04 validation
panel (`validation.spec.ts:77`) surfaces `warn` rows (semi-transparent wall without spectrum;
zero-density forest) and a `crit` row (mocked triangulate 4xx), and clicking a row selects + zooms
the offending object (`zoom-target`, `state().selection` — lines 118-126). Server-side TIN panic
safety behind the 4xx is real: `envi-dgm` 11 tests pass, incl. `interior_crossing_breaklines` and
`self_intersecting_breakline` (`can_add_constraint` guards the sole `add_constraint`).

### SC3 — Met
`web/tests/e2e/spectrum.spec.ts:44` authors at 1/1-octave and asserts the 9 anchors land on the
**exact** octave band indices `4,16,28,40,52,64,76,88,100` and that index `5` is absent (lines
58-61) — the D-05 invariant that octave/third centres fall on 1/12 band indices. The preview
`polyline` is the **server**-derived 105-grid (SVC-07: interpolation is a `POST
/meta/interpolate-spectrum` call, not client math — `contract_interpolate_spectrum` 6 tests pass,
`interpolate.rs` verified to not clamp so out-of-range reaches the validating constructor as 400).
Switching to 1/12 **promotes** explicitly (`spectrum-promote-notice`, `data-resolution="twelfth"`,
lines 70-75). Semi-transparent screen: `spectrum.spec.ts:20` shows `warn` without a spectrum → `info`
(acoustic screen) once assigned. Per-façade (SCN-02): `spectrum.spec.ts:80` proves the D-02 edge-UUID
key — a façade override survives a vertex insert **elsewhere** in the ring (UUID + spectrum + segment
unchanged). Ring-diff unit suite (`edges.test.ts`, 12 passed) covers IDENTITY / MOVE / INSERT / DELETE
**plus** the review-added same-count insert+delete (ME-02) and duplicate-coordinate rebuild (ME-03)
fail-safes.

### SC4 — Met (geometry+properties), with a labeled residual on spectra
`web/tests/e2e/sc4-persistence.spec.ts:85` drives all three survival paths against a captured
round-trip fixture (PUT captures the FeatureCollection; the subsequent GET returns exactly that):
(1) a basemap switch increments `rehydration-count` and Terra Draw re-populates from the store on
`style.load` (`td-feature-count > 0`), store count unchanged across two switches (no echo-loop);
(2) a page reload re-hydrates the captured scene by id (`featureIds` equal the drawn ids — a no-GET
app would return empty); (3) close (`store-feature-count → 0`) then `reopen-last` restores the same
ids. Corroborated by `lifecycle.spec.ts:16`. The Phase-6 Open/New seams work end-to-end
(`sc4-persistence.spec.ts:141`).

**Residual (honest, decided scope — not a false green):** authored 105-band isolation / L_W spectra
are **session-only** this phase — `saveScene()` PUTs geometry+properties, not the `spectra` channel,
and `loadScene` resets it. This is the D-01 decision (spectrum serialization is Phase-9/10 typed-DTO
work) and is surfaced honestly by the "Spectra session-only" `.chip.warn`
(`ProjectBar.tsx:109-118`, `data-testid="spectra-unpersisted"`), so "Saved" never implies the
acoustic authoring persisted. The SC4 E2E deliberately uses non-spectrum kinds, so what it asserts
(geometry+properties round-trip) is genuinely proven; the spectra gap is documented, not hidden.

## Requirements Coverage

| Req | Description | Status | Evidence |
|-----|-------------|--------|----------|
| WEB-01 | MapLibre OSM/vector basemap | Satisfied | `basemap.ts:18` OpenFreeMap `styles/dark`, MapLibre 5 dep, OSM `AttributionControl` |
| WEB-02 | Directional sources + SPL-at-reference calibration | Satisfied | `SourceFields.tsx`; server `calibrate.rs::spl_to_lw` (validates ref-distance finite/>0); SC1 places a source with spectrum |
| WEB-03 | Receivers + calculation area | Satisfied | SC1 places `receiver` + `calc_area` into store+PUT |
| WEB-04 | Buildings/walls/ground-zones/forests/elevation w/ DGM re-tri; inheritance; click-to-select validation | Satisfied | SC1 (all kinds + inheritance + TIN), SC2 (validation panel click-to-select+zoom) |
| WEB-08 | Semi-transparent screen + isolation spectrum | Satisfied | `spectrum.spec.ts:20` warn→info |
| WEB-09 | Per-façade isolation spectra | Satisfied | `spectrum.spec.ts:80` UUID-keyed override survives ring insert |
| WEB-10 | Isolation-spectrum editor (1/12 direct, or 1/1 / 1/3 interpolated) | Satisfied | `spectrum.spec.ts:44` anchors on band indices, server preview, promote-to-twelfth |
| SCN-01 | Semi-transparent screen object | Satisfied | `IsolationSpectrumDto` + `TryFrom` → engine `IsolationSpectrum` (`dto.rs:300`); warn/info treatment |
| SCN-02 | Per-façade building isolation | Satisfied | D-02 edge-UUID map; ring-diff no-repoint tests |
| SCN-03 | Isolation-spectrum type, octave/third linearly interpolated onto 105-grid at exact indices | Satisfied | `interpolate.rs` (server), `contract_interpolate_spectrum` (6), SC3 band-index anchors |
| SCN-04 | Forest object (density/stem radius/height) | Satisfied | `ForestParamsDto` + `TryFrom` → `ForestCrossing` (`dto.rs:338`); `ForestFields.tsx`; SC1 forest via numeric fields |

**11/11 requirements satisfied.** No orphans (all Phase-7 REQ-IDs appear in plan frontmatter).

## Honest-Stub Audit (no fabricated acoustic result)

- **Spectra persistence** — labeled `.chip.warn` "Spectra session-only"; "Saved" does not imply
  spectra saved. No false green. ✓
- **DGM TIN overlay** — module header (`dgmTrigger.ts:13-16`) documents the degrees-as-meters Phase-8
  projection deferral; never presented as a real elevation result. Accepted risk LO-01. ✓
- **Spectrum preview / SPL→L_W** — genuinely server-computed (`/meta/interpolate-spectrum`,
  `spl_to_lw`), not faked; SVC-07 "no client-side acoustic math" holds (client does band-index stride
  + turf geometry only). ✓
- **testBridge** — `import.meta.env.DEV`-gated; Vite statically drops it from `web/dist` (`main.tsx:24`),
  so it does not ship. ✓

## Anti-Patterns / Findings

No blocker anti-patterns. Code review (`07-REVIEW.md`) found 7 issues (0 critical); 6 fixed, 1
accepted risk (LO-01 TIN projection). Security (`07-SECURITY.md`) 39/39 threats closed, 0 open,
3 accepted risks. Both re-verified against the current tree.

## Gaps

None blocking. The residuals listed in frontmatter are decided-scope deferrals (Phase 8/9/10),
each honestly surfaced in the UI or documented at the code seam — consistent with the phase's
walking-skeleton cut (D-01) and its "objects must never be silently inert / no false green" principle.

---

_Verified: 2026-07-10_
_Verifier: Claude (gsd-verifier) — goal-backward, executed against the current tree_
