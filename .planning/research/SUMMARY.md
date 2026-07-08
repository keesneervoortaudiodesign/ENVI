# Project Research Summary — Milestone 2: Interactive Calculation UI

**Project:** ENVI — Nord2000 GIS Sound Propagation Model
**Domain:** Self-hosted web GIS application (Rust HTTP service + React/MapLibre frontend) wrapped around an existing validated Nord2000 acoustics engine
**Researched:** 2026-07-08
**Confidence:** HIGH

## Executive Summary

Milestone 2 turns the validated Milestone-1 engine into a NoizCalc-class interactive tool: draw a scene on an OSM/terrain map, pull open GIS + weather data, run a calculation as a background job, and read back receiver spectra and a dB(A)/dB(C) isophone noise map. The workflow template is d&b NoizCalc (TI 386 ch. 3–4) — project-as-folder, viewport GIS import onto a triangulated ground model (DGM), check-and-complete scene editing, grid calculation with progress/abort, editable color-scale contour map — rebuilt as a single integrated web app (no ArrayCalc split, Nord2000-only, no print subsystem). Every version number in the stack was verified against crates.io/npm on 2026-07-08; the frontend picks (MapLibre GL JS 5 + react-map-gl 8 + Terra Draw) and the Rust GIS crates (geo/rstar/spade/gdal/proj) were already locked and are confirmed current.

The architecture adds three new crates and one npm workspace around the untouched engine: `envi-gis` (all GDAL/PROJ FFI + data acquisition + geometry derivation), `envi-store` (serde DTO mirror + project folder + chunked tensor store), and `envi-service` (axum HTTP, job registry, recalc router, static frontend serving) — dependency direction strictly toward `envi-engine`, whose three-dependency quarantine stays byte-identical. The **single load-bearing refactor** of the milestone is promoting the Scene→level solve composition out of `envi-harness` into `envi_engine::solver` during engine Phase 4, so the FORCE suite validates exactly the code the web app runs. The milestone's flagship interactivity rests on a **three-tier recompute model** over the cached complex tensor: conditioning changes = streaming MAC (interactive), weather changes = re-solve on cached paths (per-scenario tensor caches), geometry changes = partial pipeline re-run — encoded structurally in the API as separate `recondition` vs `recompute` operations with content-hash invalidation.

The key risks are (1) **stale-tensor wrong results** — a cached-tensor answer served for a changed scene is the worst possible failure for a trust-critical tool; mitigated by hash-keyed tensor identity and the recondition/recompute API split, designed in the first service phase; (2) **coordinate-system leaks** — lon/lat degrees reaching the metric engine, prevented by one newtyped reprojection boundary mirroring the engine's single-`.conj()` discipline; (3) **tensor memory blindness** — 100k-receiver grids reach ~1.7 GB per scenario; receiver-axis chunking with streaming MAC must be honored end-to-end from day one; and (4) **integration churn** at the two FFI/imperative seams (GDAL threading + Windows provisioning; Terra Draw vs React render lifecycle), both of which get explicit early spikes. Goals 1/2/5 (CRUD, GIS import, drawing) are engine-independent and parallel-safe with engine Phases 3–4; goals 4/6/7 (what-if, spectra, noise map) hard-gate on Phases 3–4 plus the GEOX path-extraction work.

## Key Findings

### Recommended Stack

The backend is **axum 0.8 on tokio** — the 2026 Rust default, tower-native, with built-in SSE for job progress — plus **rayon** for grid parallelism and an in-process job registry (std `Mutex<HashMap>` + atomics; no job crate, no Redis, no WebSockets). Persistence is a NoizCalc-style **project folder**: human-diffable JSON (project/scene/manifest via serde) plus the tensor as **receiver-axis-chunked `.npy` files** (`ndarray-npy` 0.10 — verified exact match for the engine's ndarray 0.17 + num-complex 0.4 pins; `memmap2` for zero-copy MAC reads). No SQLite in the v2.0 baseline (rusqlite is the documented upgrade path if a catalog/history need emerges; tensors stay in files regardless). GIS ingestion uses the two accepted FFI exceptions — **gdal 0.19** (COG windowed reads of GLO-30/WorldCover via `/vsicurl/`, GeoTIFF cache, exports) and **proj 0.31** (auto-UTM) — plus **pure-Rust geoparquet 0.8** for Overture buildings (bbox-filtered S3 range reads; no DuckDB, no GDAL-Parquet driver) and **reqwest** (rustls) for Open-Meteo/ERA5. Isobands come from the **pure-Rust `contour` crate** (marching-squares filled polygons) by default, with a thin `gdal-sys::GDALContourGenerateEx` wrapper as the accepted-FFI escape hatch if quality/performance disappoints — validate against `gdal_contour` output (oracle pattern). dB(A)/dB(C) weighting is closed-form IEC 61672 evaluated at the exact 105-point grid centres, in `envi-engine::freq`.

Frontend: **Vite 8 + React 19.2**, **maplibre-gl 5.24 + react-map-gl 8.1** (import from `react-map-gl/maplibre`), **terra-draw 1.31 + terra-draw-maplibre-gl-adapter** used as core library + custom modes (not a prebuilt toolbar — ENVI's typed scene objects need property panels), **zustand** for state, **recharts** for spectra, native `fetch` + `EventSource` (no axios). Basemap: OpenFreeMap vector tiles default. Playwright for frontend UAT (devDependency, mocked `/api/*`).

**Core technologies:**
- **axum 0.8.9 + tokio 1.52 + tower-http 0.7**: HTTP service, SSE progress, static SPA serving — tokio-team maintained, ecosystem default
- **rayon 1.12 + spawn_blocking/dedicated workers**: grid batch parallelism (GRID-02) with chunk-boundary cancellation
- **ndarray-npy 0.10 + memmap2 0.9**: complex128 tensor chunks, numpy-inspectable, mmap-able for the fast-recalc MAC
- **gdal 0.19 / proj 0.31** (quarantined in `envi-gis`): DEM/WorldCover COG reads, CRS transforms, GeoTIFF/GeoPackage export
- **geoparquet 0.8 (pure Rust)**: Overture buildings via bbox-pushdown S3 reads
- **contour 0.13 (pure Rust)**: marching-squares isobands → server-side fill polygons (gdal-sys escape hatch documented)
- **maplibre-gl 5.24 / react-map-gl 8.1 / terra-draw 1.31 / zustand 5 / recharts 3 / vite 8**: locked frontend stack, versions verified current

**Do NOT add** (explicit exclusions from STACK.md): WebSockets, apalis/Redis/RabbitMQ, PostgreSQL/PostGIS, diesel/sea-orm, DuckDB, netcdf/hdf5 FFI crates, the immature `geotiff` crate, the stale `utm` crate, dashmap, mapbox-gl-draw, Leaflet/OpenLayers, deck.gl, any heatmap layer for results, Next.js/SSR, Redux, axios, GraphQL/OpenAPI codegen, auth frameworks (localhost bind; reverse proxy if ever exposed), tensors in SQLite, CI scaffolding.

### Expected Features

Feature research is organized by the milestone's 7 user goals (G1 project CRUD, G2 GIS import, G3 weather import, G4 weather what-if, G5 scene objects, G6 receiver spectra, G7 noise map), calibrated against TI 386 (primary local source) and iNoise.

**Must have (table stakes):**
- G1: Project list/create/open/save(autosave)/delete/duplicate with metadata; reopen-last
- G2: Viewport GIS import (terrain → DGM via spade CDT, buildings with height fallback chain + default height, WorldCover → ground zones), urban/rural default ground, imported objects fully editable ("check and complete"); flat-ground default DGM so drawing never blocks on import
- G4: Editable meteorology (T/RH/p, Beaufort + direction, downwind worst-case toggle, temperature gradient) — NoizCalc's Nord2000 settings verbatim
- G5: Draw + edit all object types (buildings, constant-height walls, ground zones with impedance A–H + roughness N/S/M/L dropdowns, forests, elevation points/lines with DGM re-triangulation, receivers, directional source with spectrum + SPL@reference-point calibration, calc area); property panels; **last-object property inheritance** (NoizCalc's cheap productivity rule); validation messages that click-to-select the offending object
- G6: Receiver points with per-band spectrum (1/3-oct display aggregated **by band index**), dB(A)/dB(C) totals, CSV export
- G7: Calculation settings (grid distance 5–20 m guidance, calc height, reflection order 3, weighting), job submit/progress/abort, isophone fill polygons with editable color scale + legend

**Should have (competitive differentiators):**
- G3: **Open-Meteo weather import** deriving A/B/C — NoizCalc has *nothing* here; fills the reference product's biggest gap
- G4: **Named weather scenarios** with per-scenario cached tensors, instant switching, difference maps — operationalizes TI 386's own advice
- G5/G6: **Interactive source conditioning** (gain/filter/delay) via tensor MAC — the project's flagship pillar; no competitor does this
- G5: Native multi-sub-source directional sources (removes the ArrayCalc dependency); MVP = single directional sub-source + presets
- G6: 1/12-oct expert view with coherent/incoherent split; G7: dB(A)/dB(C) toggle without recalculation; result exports (GeoTIFF/GeoJSON/PNG)

**Defer (v2.x / v3+):**
- Variable wall height along the base line (split walls instead); DXF/SHP/glTF import (FUT-01/02); L_den weather-class statistics (MET-05/GRID-03); report/print generation; multi-height/façade receivers; road-emission UI

**Anti-features (from NoizCalc ch. 4 — do NOT copy):** ArrayCalc integration, ISO 9613-2 dual parameter sets, bitmap snapshot/geo-referencing workflow, Google data sources, the print/sheet-layout subsystem, desktop F-key idioms, thread-count setting in the UI, palette editor beyond a class-color picker. Encode these as anti-requirements before phases are cut (Pitfall 21).

### Architecture Approach

Three new crates + one npm workspace around the frozen engine, single deployable binary (`envi-service`) serving the built Vite bundle. Wire format: GeoJSON in WGS84 everywhere on the network; the server reprojects to the project's pinned UTM CRS at exactly one seam. All authoritative state (scene, met, tensor, results) is server-side; the tensor never crosses the wire — the browser is a pure editor/viewer receiving ready-made fill polygons and spectra arrays. The recalc router in `envi-service` diffs requests against content hashes (geometry, met, conditioning) and routes to the cheapest valid tier: **Tier 0** color-scale re-contour, **Tier 1** conditioning MAC over mmap'd chunks (interactive), **Tier 2** weather re-solve on cached paths (per-scenario tensors), **Tier 3** partial pipeline for geometry edits (rstar corridor dirtying). The tensor identity is (geometry, met, receiver set); conditioning is a readout parameter, never baked into `H`.

**Major components:**
1. `envi-engine` (existing) — pure math; **gains in Phase 4**: `solver::compute_tensor(scene, met, paths, sink)` promoted from the harness, chunk-streaming `TensorSink` trait, and `transfer::readout` (chunk-wise MAC). This promotion is the one load-bearing refactor: both harness and service call ONE solve path, so FORCE validates the exact code users run
2. `envi-gis` (new) — the only C-linked crate: acquisition (DEM COG windows, WorldCover, Overture, Open-Meteo/ERA5, disk cache) + derivation (auto-UTM, DGM TIN, **DEM cut-profile extraction** — the flagged biggest self-build, oracle = GRASS `r.profile` —, impedance segmentation, screening edges, CDT receiver grids, contouring)
3. `envi-store` (new) — serde DTO mirror of engine scene types (keeps serde OUT of the engine; the quarantine's price), project-folder layout, chunked tensor store with manifest hashes
4. `envi-service` (new) — thin: axum API, job registry with dedicated CPU workers + SSE progress + cancellation, the recalc router, static serving
5. `web/` (new, npm) — MapLibre map shell, Terra Draw scene editing, property panels, weather what-if, job status, spectra charts, isophone overlay

**Memory model (frozen, load-bearing):** chunk the tensor along the receiver axis, freq-contiguous — a 1024-receiver chunk of 8 sub-sources is ~14 MB, so peak RSS is workers × chunk size regardless of grid size (OUT-06). Solve writes chunks, MAC streams chunks, persistence stores chunks.

### Critical Pitfalls

Top load-bearing items from the 22 catalogued (full detail in PITFALLS.md):

1. **Stale-tensor wrong results / fast-path confusion (P17)** — the central API-design decision of the milestone. Encode conditioning-vs-propagation in the API shape: separate `recondition` (only G_s(f); always MAC) vs `recompute` (scene/terrain/ground/met; always propagation) operations; tensor cache keyed by content hash; MAC requests with a mismatched hash rejected, never silently served; UI shows a results-stale badge; regression test MAC ≡ full-recompute. Weather what-if is fast only as cache-per-scenario, never by pretending met is a gain. Design this in Phase 5, before any UI binds to it.
2. **One CRS boundary + PROJ axis order (P1/P2)** — exactly one reprojection module, newtyped `LonLat` vs `SceneXY`, UTM zone pinned per project at creation, `OAMS_TRADITIONAL_GIS_ORDER` everywhere, a landmark round-trip test to the meter, degree-magnitude rejection assertion. Must exist before any drawing or ingestion code.
3. **Tensor memory blindness + uncancellable jobs (P15/P16)** — receiver-chunked everything end-to-end; pre-run cost estimate (receivers, tensor bytes, time) shown before Run; cancellation token checked at chunk boundaries; job state machine includes Cancelled/Failed from day one. Halving grid spacing quadruples cost — guardrail the spacing control.
4. **GDAL in a long-running service (P6/P7/P8)** — datasets are not thread-safe: one Dataset per worker, all GDAL via `spawn_blocking`/dedicated pools, never inline in async handlers; `/vsicurl/` only at ingestion time (compute path reads local cache only — verify by running offline); Windows provisioning (proj.db, GDAL_DATA, DLLs) gets a pinned route + startup self-check as the first plan of the service phase.
5. **Crossing ground-effect polygons (P5) + buried sources (P4)** — adopt NoizCalc semantics: containment allowed (innermost wins), partial crossing rejected at draw time AND at the pre-calc gate; elevation points/lines + "flatten venue" are NOT polish, they are the documented workaround for DEM quality (GLO-30 is a DSM — canopy/rooftop bias, P3; sample building bases from footprint-boundary ground, never DSM-under-building).
6. **Band-index contract crosses the wire (P19) + UI acoustic math (P18)** — API spectra are dense arrays keyed by band index with the exact freq grid served once at `/api/v1/meta/freq-axis`; nominal Hz labels are display-only; iron rule "every acoustic number is computed server-side" (grep the frontend for `Math.log10`/`Math.pow` in review).
7. **Terra Draw vs React lifecycle (P13)** — instance in a ref created on map load, scene store outside React render state, re-hydration on `style.load` written in week one; needs a small spike before feature plans.

## Implications for Roadmap

Milestone 2 appends phases **5–10** (engine Phases 1–4 keep their numbers; Phases 3–4 remain the execution priority). Goals 1/2/5 phases (5–7, and the geometry half of 8) are fully parallel-safe with the engine finish; Phases 9–10 hard-gate on engine Phase 4.

### Phase 5: Service Foundation + Persistence
**Rationale:** Everything hangs off the API contract, the CRS boundary, and the project store — and the highest-leverage design decisions of the milestone (recondition/recompute split, band-index axis, job state machine) live here. Also the GDAL-on-Windows provisioning pain must be absorbed first, not mid-milestone.
**Delivers:** `envi-store` (project folder, DTO mirror, scene JSON round-trip) + `envi-service` skeleton (axum, project CRUD, scene GET/PUT, static serving, job registry with Queued/Running/Done/Failed/Cancelled, SSE progress plumbing), the single CRS boundary module (newtypes + auto-UTM pinning + landmark round-trip test), `/meta/freq-axis`, GDAL/PROJ startup self-check + pinned Windows provisioning route.
**Addresses:** G1 (SVC-01/03/04, GEOX-04)
**Avoids:** Pitfalls 1, 2, 6 (rules), 8, 16 (state machine), 17 (API contract), 19
**Engine dep:** none (types only). ✅ parallel-safe with engine Phases 3–4.

### Phase 6: Frontend Shell + Scene Editing
**Rationale:** The drawing surface is the highest-churn integration (Terra Draw/React) and the largest user-facing feature block; it needs only the Phase-5 API and a flat default DGM.
**Delivers:** `web/` — MapLibre basemap (OpenFreeMap), project browser, Terra Draw object palette for all TI-386 object kinds + ENVI receivers/directional sources, property panels with last-object inheritance, snap/vertex editing, ground-overlap draw-time validation (containment-wins/crossing-rejected), clickable validation messages, autosave.
**Addresses:** G5 core, G1 UI (WEB-01..04)
**Avoids:** Pitfalls 4 (elevation objects + below-terrain badges), 5 (draw-time check), 13 (lifecycle spike first), 18 (server-computes convention set here), 21
**Engine dep:** none. ✅ parallel-safe.

### Phase 7: GIS Ingestion + DGM
**Rationale:** The NoizCalc "Import" moment — needs the Phase-5 job model and Phase-6 editability to close the check-and-complete loop, but no engine.
**Delivers:** `envi-gis::acquire` (GLO-30/LiDAR COG windows, WorldCover→σ reviewed mapping table + unit test, Overture geoparquet with height fallback chain + provenance, disk cache), viewport-import job endpoint, DGM TIN from imported terrain, imported features as ordinary editable objects, attribution control, impedance debug overlay.
**Addresses:** G2 (DATA-01..04)
**Avoids:** Pitfalls 3 (DSM bias, building-base sampling), 7 (vsicurl only at ingestion; local-cache compute path), 9 (provenance + attribution), 10 (mapping table), 12 (closed DEM-source enum)
**Engine dep:** none. ✅ parallel-safe.

### Phase 8: Path Extraction + Weather Import
**Rationale:** Real-GIS path extraction (GEOX-01..03) is the hidden prerequisite of every calculation feature and the project's flagged biggest self-build; weather import feeds the A/B/C math that lands in engine Phase 3.
**Delivers:** `envi-gis` cut-profile extractor (oracle: GRASS `r.profile`), impedance segmentation with drawn>imported>default priority, screening edges (rstar), CDT receiver grids (GRID-01); `acquire::weather` (Open-Meteo cached-per-(site,window), what-ifs never call the API) → A/B/C derivation; weather panel UI (import + manual override, single scenario).
**Addresses:** G3, G4 minimal, GEOX-01..04, GRID-01, METX-01(/02 groundwork)
**Avoids:** Pitfalls 11 (call budget), 22 (behavioral-reference-only convention for NoiseModelling)
**Engine dep:** ⚠ split — the geometry half needs only scene types (parallel-safe); the weather-derivation half needs engine **Phase 3** (MET-02/06). **Coordination flag:** the `PropagationPath` struct defined here must be agreed with Phase 4's `solver` signature early — it is the one type both sides touch.

### Phase 9: Calculation Service
**Rationale:** First phase that runs real physics end-to-end; strictly after engine Phase 4 delivers the promoted solver + tensor.
**Delivers:** chunked `.npy` tensor store + `TensorSink` impl (`envi-store`), job runner wiring `envi_engine::solver::compute_tensor` with rayon chunks, progress/cancel at chunk boundaries, pre-run cost estimate, manifest content hashes, calc submit/status/cancel endpoints (GRID-02, SVC-02 realized).
**Addresses:** G7 infrastructure
**Avoids:** Pitfalls 15 (chunk streaming honored end-to-end, budget shown pre-run), 16 (cancel/progress), 17 (hash-keyed tensor identity)
**Engine dep:** ❌ **hard gate on engine Phase 4** (solver promotion + TensorSink/readout signatures). **Coordination flag:** Phase 4 must ship the solver promotion and chunk-streaming signatures publicly, or Phase 9 forces a second refactor of freshly validated code.

### Phase 10: Results + Fast Recalc
**Rationale:** The payoff phase — spectra, noise map, and the flagship interactive conditioning — needs Phase 9's tensors.
**Delivers:** MAC readout endpoints (receiver spectra by band index, both weightings server-side), recalc router Tiers 0–3, TIN→raster resample + `contour` isobands + simplify + reproject → GeoJSON fill polygons, editable color scale (single source of truth for breaks; fixed round default, autoscale explicit-only), legend with weighting from result metadata, results-stale badge, dB(A)/dB(C) instant toggle, source-conditioning UI (gain/filter/delay), CSV/GeoTIFF/GeoJSON exports, MAC≡full-recompute equivalence test. Named weather scenarios + difference maps follow as v2.x once the single-scenario loop is proven.
**Addresses:** G6, G7, G4 full, WEB-05/06/07, GRID-04/05, OUT-03..06 exposed
**Avoids:** Pitfalls 14 (no heatmap — anti-requirement), 17 (staleness UX + equivalence test), 18, 20
**Engine dep:** ❌ Phase 4 + Phase 9.

### Phase Ordering Rationale

- **Foundation before surface:** the API contract (recondition/recompute, band-index axis, job states) and the CRS boundary are the two things that cannot be retrofitted without invalidating stored tensors and rebinding the UI — they go first (Phase 5).
- **Parallel-safe track maximized:** Phases 5–7 and half of 8 need zero engine work, matching PROJECT.md's sequencing note; the engine's Phases 3–4 stay the execution priority and nothing in the UI track blocks them.
- **The DGM is the spine:** import (7), elevation editing (6), object z-placement, and validation all hang off the triangulated ground model; a flat default DGM in Phase 6 decouples drawing from import.
- **One solve path:** the solver promotion inside engine Phase 4 (not a Milestone-2 phase) guarantees the FORCE suite validates the service's exact physics — the alternative (a service-side orchestrator) is Anti-Pattern 4 and would silently fork validated code.
- **Honest interactivity:** the tier model (MAC / cached-path re-solve / partial pipeline) is designed into Phase 5's contract, implemented in 9–10 — retrofitting caching semantics later invalidates stored tensors.

### Research Flags

Phases likely needing deeper research (`/gsd-plan-phase --research-phase <N>`):
- **Phase 6:** Terra Draw + react-map-gl lifecycle (instance-in-ref, style.load re-hydration, StrictMode) — highest-churn integration; do a small architecture spike before feature plans (Pitfall 13)
- **Phase 8:** ERA5/CDS access shape (LOW-confidence stack flag: does the CSV point-timeseries route expose u*/H/z₀, or is gridded NetCDF/GRIB unavoidable?); cut-profile algorithm design vs GRASS oracle
- **Phase 9:** chunked tensor store format details (chunk size tuning, manifest schema, mmap patterns) — the load-bearing artifact of the fast-recalc promise
- **Phase 10:** `contour` crate benchmark on 100k+-cell rasters early (gdal-sys escape hatch is the documented fallback); TIN→raster resample correctness

Phases with standard patterns (skip research-phase):
- **Phase 5:** axum CRUD + ServeDir + SSE + in-process jobs are thoroughly documented patterns; the design work is the API contract review (do that as a plan-level review, not research)
- **Phase 7:** GDAL COG reads, WorldCover mapping, and Overture geoparquet access are all pinned with verified crate versions and documented env-var tuning in STACK.md/PITFALLS.md

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Every version verified against crates.io/npm registries same-day; compatibility chains (ndarray-npy↔ndarray 0.17, tower-http↔axum 0.8, react-map-gl 8↔maplibre 5) checked from registry metadata. LOW flags: ERA5/CDS CSV fields, Vite 8 Node floor, basemap policy, contour perf |
| Features | HIGH | Primary source is the local TI 386 transcription (the explicit workflow template), cross-read against PROJECT.md/REQUIREMENTS.md; iNoise/web supplements MEDIUM |
| Architecture | HIGH | Grounded in the actual repo (harness solve composition read this session) + locked decisions; supporting-crate patterns MEDIUM (web-verified) |
| Pitfalls | HIGH | Project-local claims (TI 386 buried-SUB, crossing polygons, grid-cost ×4) HIGH; external claims (GDAL threading, vsicurl, Terra Draw issues, Overture ODbL, Open-Meteo limits) MEDIUM, each cross-checked against official docs/issue trackers |

**Overall confidence:** HIGH

### Gaps to Address

- **Divergences between research docs, resolved here (roadmapper should treat these as the decision):** (a) crate naming/topology — use ARCHITECTURE.md's `envi-gis` / `envi-store` / `envi-service` three-crate split (STACK.md's two-crate `envi-server` sketch is superseded; `envi-store` earns its keep as the serde-quarantine seam); (b) persistence — STACK.md's **no-SQLite baseline** wins (project folder: JSON + npy chunks + manifest; rusqlite is the documented upgrade path, not the Phase-5 plan); (c) contouring — STACK.md's **pure-Rust `contour` crate default** wins (house rule: native over FFI; the high-level gdal crate doesn't expose contouring), with the `gdal-sys::GDALContourGenerateEx` wrapper as the validated escape hatch.
- **ERA5/CDS route (METX-02):** whether a pure-Rust (CSV) path exists for Obukhov-class statistics — decide in Phase 8 research; never add netcdf/hdf5 FFI without that check.
- **Forest scattering coverage:** TI 386's Nord2000 forest term (`A = d·a(f)`) has no ENG-xx requirement in Milestone 1 — verify engine coverage during roadmapping; if absent, either scope it into an engine phase or ship Forest objects geometry-only with a visible "scattering pending" note (never silently inert).
- **GDAL ≥3.10 RFC-101 thread-safe handles:** verify availability in the pinned Windows GDAL build during Phase 5; don't architect around it as the only mechanism.
- **Basemap source policy** (OpenFreeMap vs alternatives): validate rendering quality + terms in Phase 6.

## Sources

### Primary (HIGH confidence)
- crates.io / npm registry APIs (2026-07-08) — all pinned versions + dependency-compatibility verification
- `docs/references/dbaudio-ti386-1.6-en.md` — TI 386 NoizCalc ch. 3–4 (workflow template, object catalog, calc settings, color scale, documented failure modes)
- Existing codebase (`envi-engine`/`envi-harness` sources read this session) — solver-promotion claim grounded in `run_case`/`build_terrain_inputs`
- `.planning/PROJECT.md`, `.planning/REQUIREMENTS.md` (v2 groups DATA/GEOX/METX/GRID/WEB/SVC/FUT), `.planning/research/OPEN-GIS-LANDSCAPE.md` — locked decisions
- docs.rs (ndarray-npy complex128 support; gdal-sys GDALContourGenerateEx presence; gdal crate contour absence)

### Secondary (MEDIUM confidence)
- GDAL official docs (multithreading, RFC 101, virtual file systems, config options) + OSGeo issue tracker (#1244, #8499)
- Terra Draw issue tracker (#172, #197) + MapLibre official Terra Draw example + watergis adapter
- Overture docs (attribution, per-feature `sources[].license`), Open-Meteo pricing/terms + issues, visgl react-map-gl v8 docs, tokio spawn_blocking guidance, geoparquet/geoarrow ecosystem docs
- DGMR iNoise product pages (competitor feature calibration)

### Tertiary (LOW confidence)
- 2026 Rust web-framework comparison articles (axum-vs-actix narrative context only, not load-bearing)
- dBmap.net (existence check for web noise-mapping precedent)

---
*Research completed: 2026-07-08*
*Ready for roadmap: yes*
