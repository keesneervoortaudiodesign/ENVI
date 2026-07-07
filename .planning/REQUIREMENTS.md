# Requirements: ENVI — Nord2000 GIS Sound Propagation Model

**Defined:** 2026-07-07
**Core Value:** A numerically faithful Nord2000 engine — validated against the FORCE road-traffic test cases — that produces correct per-band outdoor sound levels over GIS terrain.

> **Milestone framing.** v1 = **Milestone 1: the validated core engine** (propagation math + meteorology + directional sources + the complex-transfer-tensor output + FORCE validation), fed geometry from test-case files — no map/UI. The GIS ingestion, receiver grids, web frontend, and service are real project goals but scoped to **v2 (later milestones)**.

## v1 Requirements

### Engine — Nord2000 propagation (ENG)

- [ ] **ENG-01**: Compute direct-path attenuation (geometrical divergence) per 1/12-octave frequency point
- [ ] **ENG-02**: Compute ground effect over a segmented-impedance profile (frequency-dependent, soft↔hard), preserving complex pressure
- [ ] **ENG-03**: Compute screen/barrier diffraction for single and multiple edges
- [ ] **ENG-04**: Compute air absorption per ISO 9613-1 from temperature, humidity, pressure
- [ ] **ENG-05**: Compute refraction via the equivalent-linear sound-speed profile (circular-ray ξ, Δτ) with guarded numerics (f64, ξ singularity clamps, Δτ cancellation-safe reformulation)
- [ ] **ENG-06**: Compute reflection paths with separate before/after profile coefficients (A₁/B₁, A₂/B₂)
- [ ] **ENG-07**: Combine direct + reflected + diffracted contributions as complex pressure, retaining Δτ interference phase
- [ ] **ENG-08**: Apply the fluctuating-refraction coherence coefficient F_τ (turbulence C_v², C_T²) to blend coherent/partial-coherent contributions

### Engine output — complex transfer tensor (OUT)

- [ ] **OUT-01**: Produce, per (directional sub-source × receiver × 1/12-octave frequency point), a complex acoustic transfer value (magnitude + phase)
- [ ] **OUT-02**: Store results as a dense multi-dimensional `Complex<f64>` array `H[sub_source, receiver, freq]`, frequency-contiguous, for both single-receiver and grid cases
- [ ] **OUT-03**: Recompute receiver spectra on source-conditioning changes via complex multiply-accumulate `p[r,f] = Σ_s H[s,r,f]·G_s(f)` — no propagation re-run
- [ ] **OUT-04**: Apply per-source **filtering** (complex per-frequency gain G_s(f)) as a conditioning input
- [ ] **OUT-05**: Apply per-source **delay** (phase ramp e^{-j2πfτ}) as a conditioning input
- [ ] **OUT-06**: Chunk/stream the tensor so large receiver grids stay within a memory budget

### Meteorology — sound-speed profile (MET)

- [ ] **MET-01**: Evaluate the log-lin profile c(z) = A·ln(z/z₀+1) + B·z + C with z₀ clamped ≥ 0.001 m
- [ ] **MET-02**: Derive A per source→receiver azimuth (wind term u·cos φ) and B from temperature/stability (inversion → B>0); precompute the isotropic temperature part once, add projected wind per bearing
- [ ] **MET-03**: Collapse the log-lin profile to an equivalent-linear profile (CalcEqSSP), averaging ∂c/∂z between source height h_S and receiver height h_R
- [ ] **MET-04**: Apply the frequency-dependent ground variant (CalcEqSSPGround) with f_L/f_H log-interpolation, integrated with the 1/12-octave evaluation
- [ ] **MET-05**: Support Route 1 weather-class input — a table of (A,B) pairs with occurrence probabilities — for L_den energy-weighted combination
- [ ] **MET-06**: Support Route 3 — reconstruct u(z), T(z) from surface met via Monin–Obukhov similarity (cloud cover as stability proxy) and least-squares fit A,B,C

### Sources — directional complex sources (SRC)

- [ ] **SRC-01**: Define a point sub-source with per-1/12-octave sound power / source spectrum
- [ ] **SRC-02**: Attach a directivity function ΔL(θ, φ, f) to a sub-source
- [ ] **SRC-03**: Compose a complex source from multiple directional sub-sources evaluated independently into the transfer tensor
- [ ] **SRC-04**: Represent directivity internally as per-band spherical balloons (common denominator of CLF/SOFA/BEM)

### Geometry model (GEO)

- [ ] **GEO-01**: Represent a canonical semantic 2.5D scene (Source, Receiver, Barrier, Building, TerrainProfile) in a projected metric CRS, Z-up
- [ ] **GEO-02**: Consume a source→receiver terrain profile + ground-impedance segments + screen edges from FORCE test-case files
- [ ] **GEO-03**: Compute source→receiver azimuth and reflection-path geometry

### Validation (VAL)

- [x] **VAL-01**: Stand up a test harness that loads and runs the FORCE road-traffic test cases (built *before* propagation code)
- [ ] **VAL-02**: Engine reproduces the FORCE test-case reference results within the standard's tolerance
- [ ] **VAL-03**: Cross-validate shared sub-effects (divergence, ISO 9613-1 air absorption, screen geometry) against NoiseModelling's CNOSSOS output

## v2 Requirements

Deferred to later milestones. Tracked, not in the current roadmap.

### GIS data ingestion (DATA)

- **DATA-01**: Fetch terrain — Copernicus GLO-30 DEM (COG range reads) + national LiDAR DTM where available
- **DATA-02**: Fetch ESA WorldCover land cover and map classes → Nordtest σ / CNOSSOS G impedance
- **DATA-03**: Fetch buildings (Overture/OSM) with a height-resolution fallback chain
- **DATA-04**: Cache fetched tiles/data locally

### Real GIS geometry pipeline (GEOX)

- **GEOX-01**: Extract terrain elevation profile along a source→receiver line from a DEM raster
- **GEOX-02**: Segment ground into impedance classes along the profile from land-cover + OSM overrides
- **GEOX-03**: Derive screening edges from building/barrier geometry along the path
- **GEOX-04**: Reproject inputs to an auto-selected local metric CRS (UTM)

### Meteorology import (METX)

- **METX-01**: Import runtime meteorology from Open-Meteo (multi-level winds/temps, cloud, BLH)
- **METX-02**: Import ERA5/CDS reanalysis to derive wind×stability weather-class occurrence statistics (Obukhov length)

### Receiver grid & output (GRID)

- **GRID-01**: Generate a building-aware receiver grid via constrained Delaunay (spade)
- **GRID-02**: Batch-compute the transfer tensor over the grid, parallelized
- **GRID-03**: Combine per-weather-class results into energy-weighted L_den
- **GRID-04**: Contour results into isophone polygons (GDAL contour)
- **GRID-05**: Export results (GeoTIFF / GeoJSON / GeoPackage)

### Web frontend (WEB)

- **WEB-01**: OpenStreetMap basemap (MapLibre GL JS)
- **WEB-02**: Place/edit sources on the map (Terra Draw)
- **WEB-03**: Place/edit receivers and the receiver-grid domain
- **WEB-04**: Draw barriers/buildings
- **WEB-05**: Configure source sound power, directivity, and input conditioning (filter/delay) in the UI, with fast recalculation
- **WEB-06**: Render isophone overlays as fill polygons
- **WEB-07**: Submit a calculation job and view status/results

### Service (SVC)

- **SVC-01**: Persist projects (scene + settings + cached tensors)
- **SVC-02**: Compute-job model (submit, queue, run, fetch results)
- **SVC-03**: Rust HTTP API backend
- **SVC-04**: Single self-hosted deployable service

### Future imports & BEM (FUT)

- **FUT-01**: DXF import (Rust `dxf` crate) → canonical semantic model
- **FUT-02**: SketchUp import via glTF/COLLADA export (never `.skp`/SDK)
- **FUT-03**: Per-path-segment `PropagationCorrection` interface (architecture hook lands in v1 engine; corrections themselves v2+)
- **FUT-04**: 2.5D BEM barrier-correction tables (Bempp Rust stack: bempp-rs / kifmm)
- **FUT-05**: SOFA/AES69 directivity import

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

## Traceability

Mapped during roadmap creation (2026-07-07). See `.planning/ROADMAP.md`.

| Requirement | Phase | Status |
|-------------|-------|--------|
| ENG-01 | Phase 1 | Pending |
| ENG-02 | Phase 2 | Pending |
| ENG-03 | Phase 2 | Pending |
| ENG-04 | Phase 1 | Pending |
| ENG-05 | Phase 3 | Pending |
| ENG-06 | Phase 3 | Pending |
| ENG-07 | Phase 2 | Pending |
| ENG-08 | Phase 3 | Pending |
| OUT-01 | Phase 4 | Pending |
| OUT-02 | Phase 4 | Pending |
| OUT-03 | Phase 4 | Pending |
| OUT-04 | Phase 4 | Pending |
| OUT-05 | Phase 4 | Pending |
| OUT-06 | Phase 4 | Pending |
| MET-01 | Phase 3 | Pending |
| MET-02 | Phase 3 | Pending |
| MET-03 | Phase 3 | Pending |
| MET-04 | Phase 3 | Pending |
| MET-05 | Phase 3 | Pending |
| MET-06 | Phase 3 | Pending |
| SRC-01 | Phase 1 | Pending |
| SRC-02 | Phase 4 | Pending |
| SRC-03 | Phase 4 | Pending |
| SRC-04 | Phase 4 | Pending |
| GEO-01 | Phase 1 | Pending |
| GEO-02 | Phase 1 | Pending |
| GEO-03 | Phase 1 | Pending |
| VAL-01 | Phase 1 | Complete |
| VAL-02 | Phase 4 | Pending |
| VAL-03 | Phase 4 | Pending |

**Coverage:**

- v1 requirements: 30 total (ENG 8, OUT 6, MET 6, SRC 4, GEO 3, VAL 3)
- Mapped to phases: 30/30 ✓
- Unmapped: 0 ✓

---
*Requirements defined: 2026-07-07*
*Last updated: 2026-07-07 after roadmap creation (traceability mapped)*
