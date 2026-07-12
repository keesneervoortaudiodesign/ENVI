# Requirements: ENVI — Nord2000 GIS Sound Propagation Model

**Defined:** 2026-07-07
**Core Value:** A numerically faithful Nord2000 engine — validated against the FORCE road-traffic test cases — that produces correct per-band outdoor sound levels over GIS terrain.

> **Milestone framing.** v1 = **Milestone 1: the validated core engine** (propagation math + meteorology + directional sources + the complex-transfer-tensor output + FORCE validation), fed geometry from test-case files — no map/UI. The GIS ingestion, receiver grids, web frontend, and service are real project goals but scoped to **v2 (later milestones)**.

## v1 Requirements

### Engine — Nord2000 propagation (ENG)

- [x] **ENG-01**: Compute direct-path attenuation (geometrical divergence) per 1/12-octave frequency point
- [x] **ENG-02**: Compute ground effect over a segmented-impedance profile (frequency-dependent, soft↔hard), preserving complex pressure
- [x] **ENG-03**: Compute screen/barrier diffraction for single and multiple edges
- [x] **ENG-04**: Compute air absorption per ISO 9613-1 from temperature, humidity, pressure
- [x] **ENG-05**: Compute refraction via the equivalent-linear sound-speed profile (circular-ray ξ, Δτ) with guarded numerics (f64, ξ singularity clamps, Δτ cancellation-safe reformulation)
- [x] **ENG-06**: Compute reflection paths with separate before/after profile coefficients (A₁/B₁, A₂/B₂)
- [x] **ENG-07**: Combine direct + reflected + diffracted contributions as complex pressure, retaining Δτ interference phase
- [x] **ENG-08**: Apply the fluctuating-refraction coherence coefficient F_τ (turbulence C_v², C_T²) to blend coherent/partial-coherent contributions

### Engine output — complex transfer tensor (OUT)

- [x] **OUT-01**: Produce, per (directional sub-source × receiver × 1/12-octave frequency point), a complex acoustic transfer value (magnitude + phase)
- [x] **OUT-02**: Store results as a dense multi-dimensional `Complex<f64>` array `H[sub_source, receiver, freq]`, frequency-contiguous, for both single-receiver and grid cases
- [x] **OUT-03**: Recompute receiver spectra on source-conditioning changes via complex multiply-accumulate `p[r,f] = Σ_s H[s,r,f]·G_s(f)` — no propagation re-run
- [x] **OUT-04**: Apply per-source **filtering** (complex per-frequency gain G_s(f)) as a conditioning input
- [x] **OUT-05**: Apply per-source **delay** (phase ramp e^{-j2πfτ}) as a conditioning input
- [x] **OUT-06**: Chunk/stream the tensor so large receiver grids stay within a memory budget

### Meteorology — sound-speed profile (MET)

- [x] **MET-01**: Evaluate the log-lin profile c(z) = A·ln(z/z₀+1) + B·z + C with z₀ clamped ≥ 0.001 m
- [x] **MET-02**: Derive A per source→receiver azimuth (wind term u·cos φ) and B from temperature/stability (inversion → B>0); precompute the isotropic temperature part once, add projected wind per bearing
- [x] **MET-03**: Collapse the log-lin profile to an equivalent-linear profile (CalcEqSSP), averaging ∂c/∂z between source height h_S and receiver height h_R
- [x] **MET-04**: Apply the frequency-dependent ground variant (CalcEqSSPGround) with f_L/f_H log-interpolation, integrated with the 1/12-octave evaluation
- [x] **MET-05**: Support Route 1 weather-class input — a table of (A,B) pairs with occurrence probabilities — for L_den energy-weighted combination
- [x] **MET-06**: Support Route 3 — reconstruct u(z), T(z) from surface met via Monin–Obukhov similarity (cloud cover as stability proxy) and least-squares fit A,B,C

### Sources — directional complex sources (SRC)

- [x] **SRC-01**: Define a point sub-source with per-1/12-octave sound power / source spectrum
- [x] **SRC-02**: Attach a directivity function ΔL(θ, φ, f) to a sub-source
- [x] **SRC-03**: Compose a complex source from multiple directional sub-sources evaluated independently into the transfer tensor
- [x] **SRC-04**: Represent directivity internally as per-band spherical balloons (common denominator of CLF/SOFA/BEM)

### Geometry model (GEO)

- [x] **GEO-01**: Represent a canonical semantic 2.5D scene (Source, Receiver, Barrier, Building, TerrainProfile) in a projected metric CRS, Z-up
- [x] **GEO-02**: Consume a source→receiver terrain profile + ground-impedance segments + screen edges from FORCE test-case files
- [x] **GEO-03**: Compute source→receiver azimuth and reflection-path geometry

### Validation (VAL)

- [x] **VAL-01**: Stand up a test harness that loads and runs the FORCE road-traffic test cases (built *before* propagation code)
- [~] **VAL-02**: Engine reproduces the FORCE test-case reference results within the standard's tolerance — *chain complete, numeric Pass deferred (external blocker).* The full road chain (emission → tensor → SM1/2/3/11 → refraction → Ch.6 comparator) is wired and the emission coefficients are CITED (Table A.1); but the only publicly-available (intermediate DK Nord 2005) set over-predicts FORCE by ~2.3 dBA, so cases stay honest Skips pending the definitive Dec-2006 coefficients (see 04-VERIFICATION.md).
- [x] **VAL-03**: Cross-validate shared sub-effects (divergence, ISO 9613-1 air absorption, screen geometry) against NoiseModelling's CNOSSOS output

## Milestone 2 Requirements (v2.0 — Interactive Calculation UI)

**Scope:** the self-hosted web application around the validated engine — project CRUD, GIS + weather import, scene drawing, calculation jobs, receiver spectra, and dB(A)/dB(C) isophone maps. Workflow modeled on d&b NoizCalc (TI 386 ch. 3–4) as a **single integrated app**, **Nord2000-only**. Full differentiator set included (named weather what-if scenarios + interactive source conditioning via the tensor MAC). Two **engine extensions** — forest scattering and semi-transparent (finite-transmission) partitions — are new acoustics scoped into this milestone.

> **Engine sequencing.** ENG-09/ENG-10 are new engine physics; together with the calculation features they depend on engine Milestone-1 Phases 3–4 (meteorology + transfer tensor). The roadmap places the engine extensions in an early Milestone-2 engine phase, ahead of the calculation service. CRUD, GIS import, and scene drawing (SVC/DATA/GEOX/WEB) are engine-independent and parallel-safe with the engine finish.

### Engine extensions — new acoustics for Milestone 2 (ENG)

- [x] **ENG-09**: Compute forest excess attenuation via Nord2000 **Sub-Model 10** scattering-zone excess attenuation `ΔL_s` (AV 1106/07 §5.19, Eqs. 288–291, Tables 8/9) from mean tree density, mean stem radius, average tree height, and mean absorption — evaluated per 1/12-octave band and applied as a per-path attenuation to **both** channels (`10^{ΔL_s/20}` on `H_coh`, `10^{ΔL_s/10}` on `P_incoh`), post-conj (arg untouched). (The earlier "`A = d·a(f)` … factor `kp`" phrasing was the NoizCalc/TI 386 UI paraphrase of exactly this sub-model — `kp` ≡ the tabulated `k_f`, computed by the engine; the Eq. 288 `Fs` coherence factor is a **documented deferral** — see the phase `deferred-items.md`.)
- [x] **ENG-10**: Compute a **semi-transparent partition** — a transmission path through a screen/façade attenuated by a per-band **isolation spectrum** (transmission loss `R(f)`), with the ray **direction preserved** (straight source→receiver line), combined with the diffracted and reflected contributions as **complex pressure with phase intact** (two-channel `H_coh`/`P_incoh` contract). The isolation spectrum acts as a complex **minimum-phase** transmission filter `T(f) = 10^(−R/20)·e^{jφ_min}`, `φ_min = −H{ln|T|}` over the 105-point band axis (a documented **ENVI extension** beyond stock Nord2000's real energy loss — the same discipline as the Phase-4 directional complex phase); a flat `R` gives `φ ≡ 0`, bit-compatible with a pure attenuation. `T` joins the **coherent channel only** (never `P_incoh`). The opaque limit is the structural **`None`** state, reproducing the standard opaque screen **bit-for-bit** (a permanent committed regression). (Per-façade building transmission reuses ENG-10 with each façade's `R(f)`.)

### Scene model extensions (SCN)

- [x] **SCN-01**: Semi-transparent **screen** object — a screen carrying an assigned isolation spectrum; transmission via ENG-10, diffraction/reflection unchanged — *Phase 7 delivers the authoring (mark semi-transparent + assign spectrum via per-edge UUID); the **ENG-10 transmission path at solve time and spectrum persistence land in Phases 9–11** — authored spectra are session-only in Phase 7 (surfaced by the "Spectra session-only" UI chip)*
- [x] **SCN-02**: Semi-transparent **building** object — a 3D building where each façade (footprint edge) can be assigned its own isolation spectrum; transmission through a façade uses that façade's `R(f)` — *Phase 7 delivers per-façade authoring via stable edge-UUID assignment; **engine transmission + persistence land in Phases 9–11** (spectra session-only in Phase 7)*
- [x] **SCN-03**: **Isolation-spectrum** data type on the 105-point 1/12-octave grid; accept 1/1-octave or 1/3-octave input and **linearly interpolate** (dB across band index = linear in log-frequency; octave/third-octave centres fall exactly on 1/12-octave band indices) to the full grid
- [x] **SCN-04**: **Forest** object with editable mean tree density / mean stem radius / height (feeds ENG-09); single trees and tree lines have no effect

### GIS data ingestion (DATA)

- [x] **DATA-01**: Fetch terrain — Copernicus GLO-30 DEM + national LiDAR DTM where available (client-side whole-tile browser fetch, cached in OPFS, windowed locally in WASM — pivot per Phase-8 CONTEXT D-02/D-03)
- [x] **DATA-02**: Fetch ESA WorldCover land cover and map classes → Nordtest σ / impedance class (reviewed mapping table)
- [x] **DATA-03**: Fetch buildings (Overture GeoParquet / OSM) with a height-resolution fallback chain
- [x] **DATA-04**: Cache fetched tiles/data locally in the browser (OPFS), per project; the compute path reads only the local cache (verified with the network off) — pivot per Phase-8 CONTEXT D-03

### Real GIS geometry pipeline (GEOX)

- [x] **GEOX-01**: Extract the terrain elevation profile (DEM cut-profile) along a source→receiver line from a DEM raster (oracle: GRASS `r.profile`)
- [x] **GEOX-02**: Segment ground into impedance classes along the profile from land cover + drawn/imported overrides (priority: drawn > imported > default)
- [x] **GEOX-03**: Derive screening edges from building/barrier/wall geometry along the path
- [x] **GEOX-04**: Reproject inputs to an auto-selected local metric CRS (UTM), pinned per project, at a single reprojection boundary

### Meteorology import & what-if (METX)

- [x] **METX-01**: Import runtime meteorology from Open-Meteo (multi-level winds/temps, cloud, BLH), cached per (site, window); what-if edits never call the API
- [x] **METX-02**: Import ERA5/CDS reanalysis to derive wind×stability weather-class occurrence statistics (Obukhov length) — groundwork; full L_den statistics deferred (see GRID-03)
- [x] **METX-03**: **Manually override** meteorology for a scenario — T/RH/p, Beaufort wind class + direction, downwind worst-case toggle, temperature gradient, per-azimuth A/B/C — for what-if analysis
- [x] **METX-04**: **Named weather scenarios** with per-scenario cached tensors, instant switching, and difference maps between scenarios

### Receiver grid & output (GRID)

- [x] **GRID-01**: Generate a building-aware receiver grid via constrained Delaunay (spade); plus discrete receiver points
- [x] **GRID-02**: Batch-compute the transfer tensor over the grid, parallelized (rayon), receiver-axis chunked
- [x] **GRID-04**: Contour results into isophone fill polygons (pure-Rust `contour`; `gdal-sys` escape hatch)
- [x] **GRID-05**: Export results (GeoTIFF / GeoJSON) and spectra (CSV)

### Web frontend (WEB)

- [x] **WEB-01**: OSM/vector basemap (MapLibre GL JS 5)
- [x] **WEB-02**: Place/edit directional sources on the map (Terra Draw), with sound power / spectrum / SPL-at-reference-point calibration
- [x] **WEB-03**: Place/edit receiver points and the receiver-grid / calculation-area domain
- [x] **WEB-04**: Draw/edit buildings, walls, ground-effect (impedance A–H + roughness N/S/M/L) zones, forests, and elevation points/lines with DGM re-triangulation; last-object property inheritance; click-to-select validation messages
- [x] **WEB-05**: Configure source input conditioning (per-source gain/filter/delay) in the UI with **interactive fast recalculation** (tensor MAC) and a results-stale badge
- [x] **WEB-06**: Render isophone overlays as fill polygons with an editable color scale + legend (dB weighting from result metadata)
- [x] **WEB-07**: Submit a calculation job and view progress / abort / results; pre-run cost estimate
- [x] **WEB-08**: Draw/assign a **semi-transparent screen** and edit its isolation spectrum (SCN-01)
- [x] **WEB-09**: Assign **per-façade isolation spectra** on a semi-transparent building (SCN-02)
- [x] **WEB-10**: **Isolation-spectrum editor** — enter 1/12-octave directly, or 1/1- / 1/3-octave with linear interpolation to the 1/12-octave grid (SCN-03)
- [x] **WEB-11**: **Receiver-point spectrum readout** — per-band (1/12-oct expert + 1/3-oct display aggregated by band index), dB(A)/dB(C) totals, coherent/incoherent split, instant dB(A)⇄dB(C) toggle without recompute
- [x] **WEB-12**: Weather what-if panel — import, manual override, named-scenario management and difference-map view (METX-03/04)

### Service & persistence (SVC)

- [x] **SVC-01**: Persist projects as a project folder (scene + settings + chunked cached tensors)
- [x] **SVC-02**: Compute-job model (submit, queue, run, progress via SSE, cancel, fetch results) with a Queued/Running/Done/Failed/Cancelled state machine
- [x] **SVC-03**: Rust HTTP API backend (axum), serving the built frontend bundle
- [x] **SVC-04**: Single self-hosted deployable service (localhost bind; startup self-check) — *self-check scope adjusted by Phase-6 decision D-08: Phase 6 ships a **pure-Rust CRS landmark round-trip** self-check (zero C toolchain); the **GDAL/PROJ** version / `proj.db` / `GDAL_DATA` self-check lands in Phase 8 with the C `gdal` dependency*
- [x] **SVC-05**: Project CRUD lifecycle — create / open / save (autosave) / delete / duplicate with metadata; reopen-last
- [x] **SVC-06**: API separates **`recondition`** (conditioning-only → tensor MAC) from **`recompute`** (scene/terrain/ground/met → full propagation); tensor cache keyed by content hash; a MAC request with a mismatched hash is rejected, never silently served
- [x] **SVC-07**: All acoustic quantities are computed **server-side**; spectra cross the wire keyed by **band index** with the 1/12-octave grid served once (no Hz-based client-side acoustic math)

### Future imports & BEM (FUT — deferred beyond Milestone 2)

- **FUT-01**: DXF import (Rust `dxf` crate) → canonical semantic model
- **FUT-02**: SketchUp import via glTF/COLLADA export (never `.skp`/SDK)
- **FUT-03**: Per-path-segment `PropagationCorrection` interface (architecture hook lands in v1 engine; corrections themselves v2+)
- **FUT-04**: 2.5D BEM barrier-correction tables (Bempp Rust stack: bempp-rs / kifmm)
- **FUT-05**: SOFA/AES69 directivity import

### Deferred within v2 (v2.x / v3+ — not in Milestone 2)

- **GRID-03**: Combine per-weather-class results into energy-weighted L_den (needs METX-02 statistics; named-scenario what-if ≠ L_den statistics)
- Variable wall height along a base line (split walls instead for MVP)
- Multi-height / façade receivers; road / railway / berm emission objects
- Report / print-sheet generation, bitmap geo-referencing, palette editor (NoizCalc ch. 4 desktop features)

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| CNOSSOS-EU implementation | Nord2000 chosen for faithful refraction/inversion physics; CNOSSOS only if EU-regulatory compliance later becomes mandatory |
| Redistribute AV 1106/07 / AV 18xx documents or figures | Copyrighted — implement from equations, cite by report number |
| Multi-user SaaS / accounts / tenant isolation | Self-hosted internal tool |
| Direct `.skp` binary parsing | Proprietary + OSS-hostile, Linux-less SDK — use glTF/COLLADA export |
| FABDEM terrain, Meteostat weather | Non-commercial licenses — avoided for clean data hygiene |
| Real-time / streaming calculation | Batch compute-job model suffices |
| ArrayCalc-style companion tool / two-package split | ENVI folds source definition into the single app — no ArrayCalc dependency (NoizCalc anti-feature) |
| ISO 9613-2 dual-standard parameter sets in the UI | ENVI is Nord2000-only; carrying NoizCalc's second standard doubles the model with no benefit here |
| Print/sheet-layout subsystem, bitmap geo-referencing, palette editor | NoizCalc ch. 4 desktop-era features; a web result view + color-scale editor + image/vector export suffice |
| Google Maps data sources; heatmap result layer | Data via Copernicus/ESA/Overture/OSM; results are server-side isophone fill polygons, not a `heatmap` layer |

## Traceability

Mapped during roadmap creation (Milestone 1: 2026-07-07; Milestone 2: 2026-07-08). See `.planning/ROADMAP.md`.

| Requirement | Phase | Status |
|-------------|-------|--------|
| ENG-01 | Phase 1 | Complete (01-03) |
| ENG-02 | Phase 2 | Complete |
| ENG-03 | Phase 2 | Complete |
| ENG-04 | Phase 1 | Complete (01-03) |
| ENG-05 | Phase 3 | Complete |
| ENG-06 | Phase 3 | Complete |
| ENG-07 | Phase 2 | Complete |
| ENG-08 | Phase 3 | Complete (03-03) |
| OUT-01 | Phase 4 | Complete |
| OUT-02 | Phase 4 | Complete |
| OUT-03 | Phase 4 | Complete |
| OUT-04 | Phase 4 | Complete |
| OUT-05 | Phase 4 | Complete |
| OUT-06 | Phase 4 | Complete |
| MET-01 | Phase 3 | Complete |
| MET-02 | Phase 3 | Complete |
| MET-03 | Phase 3 | Complete |
| MET-04 | Phase 3 | Complete |
| MET-05 | Phase 3 | Complete (03-03) |
| MET-06 | Phase 3 | Complete (03-03) |
| SRC-01 | Phase 1 | Complete (01-03) |
| SRC-02 | Phase 4 | Complete |
| SRC-03 | Phase 4 | Complete |
| SRC-04 | Phase 4 | Complete |
| GEO-01 | Phase 1 | Complete |
| GEO-02 | Phase 1 | Complete |
| GEO-03 | Phase 1 | Complete |
| VAL-01 | Phase 1 | Complete |
| VAL-02 | Phase 4 | Chain complete; numeric Pass deferred (external coefficient blocker) |
| VAL-03 | Phase 4 | Complete |
| ENG-09 | Phase 5 | Complete |
| ENG-10 | Phase 5 | Complete |
| SCN-01 | Phase 7 | Complete |
| SCN-02 | Phase 7 | Complete |
| SCN-03 | Phase 7 | Complete |
| SCN-04 | Phase 7 | Complete |
| DATA-01 | Phase 8 | Complete |
| DATA-02 | Phase 8 | Complete |
| DATA-03 | Phase 8 | Complete |
| DATA-04 | Phase 8 | Complete |
| GEOX-01 | Phase 9 | Complete |
| GEOX-02 | Phase 9 | Complete |
| GEOX-03 | Phase 9 | Complete |
| GEOX-04 | Phase 6 | Complete |
| METX-01 | Phase 9 | Complete |
| METX-02 | Phase 9 | Complete |
| METX-03 | Phase 11 | Complete |
| METX-04 | Phase 11 | Complete |
| GRID-01 | Phase 9 | Complete |
| GRID-02 | Phase 10 | Complete |
| GRID-04 | Phase 11 | Complete |
| GRID-05 | Phase 11 | Complete (11-04 WASM encoders: GeoTIFF/GeoJSON/CSV + attribution; 11-09 Export… menu + Blob/objectURL download + D-22 footer, offline Playwright UAT) |
| WEB-01 | Phase 7 | Complete |
| WEB-02 | Phase 7 | Complete |
| WEB-03 | Phase 7 | Complete |
| WEB-04 | Phase 7 | Complete |
| WEB-05 | Phase 11 | Complete |
| WEB-06 | Phase 11 | Complete |
| WEB-07 | Phase 10 | Complete |
| WEB-08 | Phase 7 | Complete |
| WEB-09 | Phase 7 | Complete |
| WEB-10 | Phase 7 | Complete |
| WEB-11 | Phase 11 | Complete |
| WEB-12 | Phase 11 | Complete |
| SVC-01 | Phase 6 | Complete |
| SVC-02 | Phase 10 | Complete |
| SVC-03 | Phase 6 | Complete |
| SVC-04 | Phase 6 | Complete |
| SVC-05 | Phase 6 | Complete |
| SVC-06 | Phase 11 | Complete |
| SVC-07 | Phase 6 | Complete |

**Coverage:**

- v1 requirements: 30 total (ENG 8, OUT 6, MET 6, SRC 4, GEO 3, VAL 3)
- Mapped to phases: 30/30 ✓
- Unmapped: 0 ✓
- Milestone 2 (v2.0) requirements: 41 total (ENG 2, SCN 4, DATA 4, GEOX 4, METX 4, GRID 4, WEB 12, SVC 7)
- Mapped to phases: 41/41 ✓ (Phases 5–11; each requirement exactly one phase)
- Unmapped: 0 ✓ — GRID-03 and FUT-01..05 deferred beyond Milestone 2 by design (intentionally not mapped)

Note on SVC-06: the `recondition`/`recompute` API split and hash-keyed tensor identity are *designed* in Phase 6 (contract-tested against a stub tensor, before any UI binds to it) and *realized* end-to-end in Phase 11 where the real MAC path exists — the requirement maps to Phase 11, where it becomes user-observable.

---
*Requirements defined: 2026-07-07 (Milestone 1)*
*Last updated: 2026-07-08 — Milestone 2 (v2.0 Interactive Calculation UI) requirements defined, incl. semi-transparent partitions (ENG-10) + forest term (ENG-09)*
*Traceability updated: 2026-07-08 — Milestone 2 requirements mapped to Phases 5–11 (41/41)*
