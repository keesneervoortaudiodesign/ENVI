# Roadmap: ENVI — Nord2000 GIS Sound Propagation Model

## Overview

Milestone 1 delivers the core value with nothing around it: a numerically faithful Nord2000 engine (AV 1106/07) in Rust, validated against the FORCE road-traffic test cases. The journey is risk-first: stand up the FORCE test harness and geometry model *before* any propagation code, prove the simplest path (divergence + air absorption) end-to-end at 1/12-octave complex resolution, then layer in the two hard physics blocks — ground/diffraction and meteorology/refraction (the equivalent-linear profile with guarded ξ/Δτ numerics) — each validated against its FORCE cases as it lands. The milestone closes by materializing the load-bearing output contract: the dense complex transfer tensor `H[sub_source, receiver, 1/12-oct freq]` with directional multi-sub-source composition and interactive filter/delay conditioning, plus the full FORCE pass and NoiseModelling cross-validation. No map, no UI, no live GIS ingestion — geometry comes from FORCE test-case files.

## Milestone 1: Validated Core Engine

v2 groups (DATA, GEOX, METX, GRID, WEB, SVC, FUT) are deferred to later milestones and intentionally unmapped here.

## Phases

**Phase Numbering:**

- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: FORCE Harness, Geometry Model & Direct Path** - Test harness on FORCE cases first, semantic 2.5D scene from test-case files, complex 1/12-octave direct path with air absorption
- [x] **Phase 2: Ground Effect & Diffraction** - Segmented-impedance ground reflection, single/multi-edge screens, complex-pressure combination with Δτ interference
- [x] **Phase 3: Meteorology & Refraction** - Log-lin A/B/C profile, equivalent-linear collapse with guarded ξ/Δτ numerics, reflection-path coefficients, weather routes, turbulence coherence
- [x] **Phase 4: Transfer Tensor, Directional Sources & Full Validation** - `H[s,r,f]` complex tensor store, directional multi-sub-source composition (+ complex directional phase), filter/delay conditioning via MAC, NoiseModelling cross-validation; FORCE road chain wired + coefficients cited — numeric Pass deferred on the intermediate-coefficient external blocker (VAL-02, ~2.3 dBA). All 5 completion gates closed (Phase complete 2026-07-09)

## Phase Details

### Phase 1: FORCE Harness, Geometry Model & Direct Path

**Goal**: A FORCE-driven test harness and semantic scene model exist before any propagation code, and the simplest full-stack path — geometrical divergence + ISO 9613-1 air absorption, evaluated as complex values at 1/12-octave points — runs through it and matches reference
**Mode:** mvp
**Depends on**: Nothing (first phase)
**Requirements**: VAL-01, GEO-01, GEO-02, GEO-03, ENG-01, ENG-04, SRC-01
**Success Criteria** (what must be TRUE):

  1. Running the test suite loads FORCE road-traffic test-case files, executes the engine on them, and reports per-case pass/fail against reference values with per-band deviations — the harness runs (and fails meaningfully) before propagation code exists
  2. A FORCE case's terrain profile, ground-impedance segments, and screen edges parse into the canonical semantic 2.5D scene (Source, Receiver, Barrier, TerrainProfile; projected metric CRS, Z-up), and source→receiver azimuth + reflection-path geometry computed from it match hand-computed values
  3. For a free-field configuration, the engine returns a complex transfer value per 1/12-octave frequency point (25 Hz–10 kHz, f64 throughout) whose magnitude matches spherical divergence + ISO 9613-1 air absorption within the standard's tolerance
  4. A point sub-source carries a per-1/12-octave source spectrum, and receiver band levels computed from it through the transfer values reproduce the expected free-field levels

**Plans**: 3/3 plans executed — Phase COMPLETE

Plans:

- [x] 01-01-PLAN.md — FORCE test-case loader + harness: cargo workspace, freq axis, .xls/TOML loaders, comparator, capability-gated runner (lands first; VAL-01)
- [x] 01-02-PLAN.md — Semantic 2.5D scene model + path geometry: Scene types, FORCE lane/height conventions, azimuth + image-source reflection (GEO-01/02/03)
- [x] 01-03-PLAN.md — Direct path at 1/12-octave complex resolution: divergence Eq. 330, ISO 9613-1 + Eq. 287, point sub-source spectrum through the harness (ENG-01, ENG-04, SRC-01)

### Phase 2: Ground Effect & Diffraction

**Goal**: Homogeneous-atmosphere FORCE cases with ground and screens pass — ground reflection over segmented impedance and single/multi-edge diffraction combine as complex pressure with the Δτ interference phase intact
**Mode:** mvp
**Depends on**: Phase 1
**Requirements**: ENG-02, ENG-03, ENG-07
**Success Criteria** (what must be TRUE):

  1. FORCE cases over flat soft, hard, and mixed (segmented-impedance) ground match reference within tolerance, with ground-interference dips at the correct frequencies — proving complex pressure is preserved, not band energy
  2. FORCE cases with single-edge and multiple-edge screens/barriers match reference within tolerance
  3. Direct + ground-reflected + diffracted contributions are combined as complex pressures retaining Δτ interference phase, and combined-case results stay finite and stable across the full 1/12-octave range (no NaN/blow-up at any evaluated frequency)

**Plans**: 5/5 plans executed

Note (planned 2026-07-07): success criteria 1-2 are satisfied at propagation level via the oracle+anchor acceptance ladder — FORCE reference spectra require the Phase 4 emission model (VAL-02 maps to Phase 4), so road cases stay capability-gated with a shrinking requires-list. Sub-model 3 (non-flat terrain, §5.12) is explicitly deferred to Phase 3 (typed-error stub; flat Phase 2 targets give r_flat = 1).

Plans:

- [x] 02-01-PLAN.md — Nord2000-native numerics core: Faddeeva w(ẑ) + Fresnel fits, Ẑ_G→Q̂ chain + ρᵢ, straight-ray Δτ (cancellation-free), coherence F with FΔν seam; committed scipy oracle (ENG-02, ENG-07)
- [x] 02-02-PLAN.md — Flat-terrain ground effect: Fresnel-zone machinery, Sub-model 1 (dip anchors ±0.05 dB, two-channel GroundResult), Sub-model 2 segmented impedance per surface type + PhaseDiffFreq; committed scipy flat-ground oracle 2×105-pt ≤0.1 dB (ENG-02, ENG-07)
- [x] 02-03-PLAN.md — Wedge diffraction kernels: Hadden–Pierce pwedge/Dwedge, lit-zone + angle-modification, p2wedge/p2edge/pwedge0; IL + shadow-boundary anchors (ENG-03)
- [x] 02-04-PLAN.md — Screen⇄ground sub-models 4/5/6 (generic engine, four/eight-path complex combination) + Sub-model 7 turbulence scattering; thin-screen oracle curve (ENG-03, ENG-07)
- [x] 02-05-PLAN.md — §5.21 terrain interpretation + Eq. 332 composition + two-channel H_coh/P_incoh transfer integration (single conj boundary), capabilities flipped, five oracle-pinned terrain cases + finiteness sweep (ENG-02, ENG-03, ENG-07)

### Phase 3: Meteorology & Refraction

**Goal**: The refraction machinery that makes Nord2000 worth building works — log-lin A/B/C profile collapsed to an equivalent-linear profile with guarded numerics, frequency-dependent ground variant, reflection-path coefficient split, weather-class and similarity-theory input routes, and turbulence coherence — validated on the refraction FORCE cases
**Mode:** mvp
**Depends on**: Phase 2
**Requirements**: ENG-05, ENG-06, ENG-08, MET-01, MET-02, MET-03, MET-04, MET-05, MET-06
**Success Criteria** (what must be TRUE):

  1. Refraction FORCE cases (downward refraction, upward refraction/shadow zone) match reference within tolerance, and the homogeneous limit (|ξ| below the clamp threshold) reproduces Phase 2 results exactly — no singularity blow-up, Δτ computed via a cancellation-safe reformulation in f64
  2. c(z) = A·ln(z/z₀+1) + B·z + C evaluates with z₀ clamped ≥ 0.001 m; A is derived per source→receiver azimuth (isotropic temperature part computed once, projected wind added per bearing) with inversion cases giving B > 0
  3. CalcEqSSP collapses the log-lin profile to the equivalent-linear profile (∂c/∂z averaged between h_S and h_R), and CalcEqSSPGround's f_L/f_H log-interpolation yields a frequency-dependent ξ evaluated natively at the 1/12-octave points
  4. Reflection paths compute with separate before/after profile coefficients (A₁/B₁, A₂/B₂), and a Route 1 weather-class table (A,B pairs with probabilities) produces an energy-weighted L_den-style combination; Route 3 reconstructs u(z), T(z) from surface met via Monin–Obukhov and least-squares fits A, B, C
  5. The fluctuating-refraction coherence coefficient F_τ (from C_v², C_T²) blends coherent and partially coherent contributions, changing results in the expected direction on turbulent cases

**Plans**: 3/3 plans executed

Plans:

- [x] 03-01-PLAN.md
- [x] 03-02-PLAN.md
- [x] 03-03-PLAN.md

**Wave 1**

- [x] 03-01: Log-lin profile + CalcEqSSP equivalent-linearization + guarded ray variables (ξ clamps, cancellation-safe Δτ, shadow zone)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 03-02: CalcEqSSPGround frequency-dependent variant + reflection-path A₁/B₁/A₂/B₂ + per-azimuth A derivation

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 03-03: Weather input routes (Route 1 classes → energy-weighted L_den, Route 3 Monin–Obukhov + 3×3 LSQ fit) + F_τ turbulence coherence (Eq. 112); Capability::Refraction flipped, FORCE wind/gradient requires-list shrinks to emission-model only (honest-green, no false Pass)

### Phase 4: Transfer Tensor, Directional Sources & Full Validation

**Goal**: The engine's output contract is real — a dense complex transfer tensor `H[sub_source, receiver, 1/12-oct freq]` fed by directional multi-sub-source composition, with filter/delay conditioning recomputed by cheap complex MAC — and the whole engine passes the full FORCE suite plus NoiseModelling cross-checks
**Mode:** mvp
**Depends on**: Phase 3
**Requirements**: OUT-01, OUT-02, OUT-03, OUT-04, OUT-05, OUT-06, SRC-02, SRC-03, SRC-04, VAL-02, VAL-03
**Success Criteria** (what must be TRUE):

  1. The engine produces a complex transfer value per (directional sub-source × receiver × 1/12-octave point), stored as a dense frequency-contiguous `Complex<f64>` ndarray `H[sub_source, receiver, freq]` for both single-receiver and multi-receiver cases
  2. Changing a source's conditioning — a per-frequency complex filter gain G_s(f) or a delay phase ramp e^{−j2πfτ} — recomputes receiver spectra via `p[r,f] = Σ_s H[s,r,f]·G_s(f)` with no propagation re-run, and the MAC result matches a full recompute to numerical identity
  3. A complex source composed of multiple directional sub-sources (per-band spherical directivity balloons, ΔL(θ,φ,f)) evaluates each sub-source independently into the tensor, and rotating a directivity balloon changes receiver levels in the expected direction
  4. The full FORCE road-traffic test suite passes within the standard's tolerance — the Milestone 1 acceptance gate. **Status: chain complete, numeric Pass deferred (external coefficient blocker).** The whole road chain (emission → tensor → SM1/2/3/11 → refraction → Ch.6 comparator) is wired and the emission coefficients are now CITED (Table A.1, committed report). But that public set is the report's *intermediate* DK Nord 2005 lineage — it over-predicts the FORCE free-field emission by a measured ~2.3 dBA (`emission_force_delta`), outside the 1 dB tolerance. A numeric Pass requires the definitive Dec-2006 coefficient set; until then cases stay honest Skips (D-03), never a false Pass.
  5. Shared sub-effects (geometrical divergence, ISO 9613-1 air absorption, screen geometry) agree with NoiseModelling's CNOSSOS output within documented expected deltas, and a large synthetic receiver set computes with the tensor chunked/streamed inside a stated memory budget

**Plans**: 5/5 plans complete

Plans:

- [x] 04-01-PLAN.md — Complex tensor store (TensorPair, TensorSink, InMemorySink; row-major [s,r,f]) + solver seam + conditioning MAC path (filter G_s(f) + delay e^{−j2πfτ}), bit-exact MAC≡recompute + 256 MiB budget sweep (OUT-01..06)
- [x] 04-02-PLAN.md — Directional sub-sources: per-band spherical directivity balloons + rotation + Nord2000 road emission model (0.01/0.30/0.75 m, 1 m offset, 80/20 rolling/propulsion, incoherent Annex-A) + pass-by integration + LE−dL free-field anchor (SRC-02/03/04; emission underpins VAL-02)
- [x] 04-03-PLAN.md — Straight-road FORCE pass: Sub-model 3 (§5.12) + segmented-ground refraction wiring + screen-refraction guard + SM8 Eq.279 decision + Ch.6 comparator wiring + EmissionModel flip → straight-road propagation + comparator wired; overall numeric Pass deferred (coefficients later CITED in 04-06 but intermediate, ~2.3 dBA over FORCE) → honest Skip (D-03) (VAL-02 part 1)
- [x] 04-04-PLAN.md — Curved + city + yearly: Coordinates-sheet loaders + contour→profile builder + Sub-model 11 image-source façade reflections + multi-lane/multi-category emission + Danish-hours L_den + Open-Q3 forest decision → full in-scope FORCE Pass, milestone acceptance (VAL-02 part 2)
- [x] 04-05-PLAN.md — NoiseModelling CNOSSOS cross-validation: committed offline fixtures, divergence + ISO 9613-1 air-absorption equality gates at octave band indices, barrier/ground expected-delta reports (VAL-03)

**Wave 1**

- [x] 04-01: Complex tensor store + solver + conditioning MAC

**Wave 2** *(blocked on Wave 1)*

- [x] 04-02: Directional balloons + road emission model + LE−dL anchor
- [x] 04-05: NoiseModelling cross-validation (parallel-safe — fixtures + comparison only)

**Wave 3** *(blocked on 04-02)*

- [x] 04-03: Straight-road FORCE pass (SM3 + refraction wiring + comparator + emission flip; overall numeric Pass deferred — coefficients later CITED (Table A.1) but intermediate, ~2.3 dBA over FORCE → honest Skip, D-03)

**Wave 4** *(blocked on 04-03)*

- [x] 04-04: Curved + city + yearly FORCE (SM11 reflection effect + façade image-source paths + Coordinates loaders + contour→profile builder + multi-class emission + Danish L_den; Open-Q3 forest = accepted gap; overall numeric Pass deferred — coefficients CITED but intermediate → honest Skip)

**Post-Phase-4 coefficient integration (04-06, 2026-07-09):** the road-emission coefficients were obtained (Jonasson source-modelling report, committed to `docs/references/`) and integrated — `PROVENANCE` flipped to CITED (Table A.1, verified vs page image), propulsion speed law corrected to the linear form (A.3). Measured against FORCE (`emission_force_delta`), the *intermediate* set over-predicts by ~2.3 dBA, so the numeric Pass stays deferred pending the definitive Dec-2006 set. Also shipped: complex **directional phase** on directivity balloons (beyond stock Nord2000). All five Phase-4 completion gates closed (`04-REVIEW`/`04-SECURITY`/`04-VERIFICATION` + simplify + doc-consistency).

## Milestone 2: Interactive Calculation UI

Milestone 2 wraps the validated engine in a NoizCalc-style (TI 386 ch. 3–4) self-hosted web application — draw a scene on an OSM/terrain map, import open GIS + weather data, run a calculation job, and read back receiver spectra and dB(A)/dB(C) isophone maps — as a single integrated app, Nord2000-only. Two new engine extensions (forest attenuation ENG-09, semi-transparent partitions ENG-10) land first in a dedicated engine phase so the scene objects and UI that expose them are never silently inert. This milestone is planned ahead non-destructively: engine Phases 3–4 remain the immediate execution priority. Phases 6–8 (and Phase 9's geometry half) are engine-independent and parallel-safe with the engine finish; Phase 9's weather half gates on engine Phase 3; Phases 10–11 hard-gate on engine Phase 4's promoted solver + tensor contract.

Architecture per `.planning/research/ARCHITECTURE.md`: three new crates (`envi-gis` — the only C-linked crate; `envi-store` — serde DTO mirror + project folder + chunked tensor store; `envi-service` — axum HTTP + jobs + recalc router) plus a `web/` npm workspace, around the untouched `envi-engine` quarantine. GRID-03 (L_den weather-class combination) and the FUT group (DXF/SketchUp import, BEM corrections, SOFA directivity) are deferred beyond Milestone 2 and intentionally unmapped.

## Milestone 2 Phases

- [x] **Phase 5: Engine Extensions — Forest & Semi-Transparent Partitions** - Nord2000 forest attenuation A = d·a(f) and finite-transmission partitions via per-band isolation spectra R(f), phase-preserving, with the opaque limit regression-pinned to the standard screen (completed 2026-07-09)
- [x] **Phase 6: Service Foundation & Persistence** - `envi-geo` + `envi-store` + `envi-service` skeleton: project-folder CRUD, single pure-Rust CRS boundary, band-index wire contract, recondition/recompute API split, job state machine, pure-Rust CRS startup self-check (GDAL/PROJ provisioning deferred to Phase 8 per D-01/D-02) (completed 2026-07-09)
- [x] **Phase 7: Frontend Shell & Scene Editing** - MapLibre/Terra Draw scene editor for all object kinds — including semi-transparent screens/buildings with the isolation-spectrum editor, forests, and elevation editing — with draw-time validation (completed 2026-07-10)
- [x] **Phase 8: GIS Ingestion & DGM** - Viewport import of GLO-30/LiDAR terrain, WorldCover ground cover, and Overture/OSM buildings onto a triangulated DGM; local-cache compute path; check-and-complete editability (completed 2026-07-11)
- [x] **Phase 9: Path Extraction & Weather** - DEM cut-profile extractor (GRASS oracle), impedance segmentation, screening edges, CDT receiver grids; Open-Meteo import → per-azimuth A/B/C; ERA5 groundwork (completed 2026-07-11)
- [ ] **Phase 10: Calculation Service** - Chunked tensor store + job runner wiring the promoted engine solver: submit/progress/abort with pre-run cost estimate, hash-keyed manifests, memory-bounded large grids
- [ ] **Phase 11: Results & Fast Recalc** - Receiver spectra, isophone noise maps with editable color scale, interactive source conditioning via the tensor MAC, named weather scenarios + difference maps, exports

## Milestone 2 Phase Details

### Phase 5: Engine Extensions — Forest & Semi-Transparent Partitions

**Goal**: Drawn forests actually attenuate and semi-transparent screens/façades actually transmit — the two new Nord2000-faithful acoustics exist in the engine, phase-preserving under the two-channel contract, and regression-safe against the validated opaque engine
**Depends on**: Milestone-1 Phase 2 (complete — diffraction/ground machinery and the Eq. 332 composition). Coordination flag: the extended composition must ship inside engine Phase 4's promoted `envi_engine::solver` (one solve path, two callers); Phase 5 must land before Phase 10 can compute these objects and before Phase 7's semi-transparent/forest UI is meaningful
**Requirements**: ENG-09, ENG-10
**Success Criteria** (what must be TRUE):

  1. A propagation path crossing a forest region of through-path length `d` is attenuated by Nord2000's `A = d·a(f)` (from mean tree density, mean stem radius, factor kp, mean absorption coefficient), evaluated at the 1/12-octave points and matching an analytic anchor/oracle within stated tolerance
  2. A semi-transparent screen contributes a straight-through transmission path — ray direction preserved — attenuated by `10^(−R(f)/20)`, combined with the diffracted and reflected contributions as complex pressure with phase intact; the `H_coh`/`P_incoh` two-channel contract holds (`F→1 ⇒ P_incoh→0` stays bit-exact with transmission enabled)
  3. The opaque limit `R(f)→∞` reproduces the standard opaque-screen result bit-for-bit — a permanent regression test in the harness
  4. A building with per-façade isolation spectra applies the crossed façade's `R(f)` to the transmission path through that façade

**Plans**: 3/3 plans complete

Plans:
**Wave 1**

- [x] 05-01-PLAN.md — ENG-09 forest: SM10 (Eqs. 288–291) module + SolveJob.forest seam + scipy oracle + pinned solver bit-baseline (wave 1)
- [x] 05-02-PLAN.md — ENG-10 kernel: IsolationSpectrum + hand-rolled min-phase cepstral filter (native sign pinned) + numpy oracle (wave 1, parallel)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 05-03-PLAN.md — ENG-10 integration: opaque bit-baseline, isolation threading into screen_channel (D-05/D-10), T6–T9, doc-contract close-out (wave 2)

### Phase 6: Service Foundation & Persistence

**Goal**: A self-hosted service skeleton exists with the milestone's non-retrofittable contracts locked — project persistence, one CRS boundary, the band-index wire format, the recondition/recompute API split, and the job state machine — before any UI binds to them
**Depends on**: Nothing in Milestone 2 (engine types only — fully parallel-safe with engine Phases 3–4). ~~First plan absorbs GDAL/PROJ Windows provisioning before any feature work~~ *(superseded by D-01/D-02: Phase 6 is pure-Rust; GDAL/PROJ provisioning moves to Phase 8)*
**Requirements**: SVC-01, SVC-03, SVC-04, SVC-05, SVC-07, GEOX-04
**Success Criteria** (what must be TRUE):

  1. User can create, open, save (autosave), duplicate, and delete a project with metadata and reopen-last; a project is a folder (scene JSON + settings + manifest, chunked tensor layout reserved) that survives service restart and round-trips scene GET/PUT
  2. One deployable axum binary serves the API and the built frontend bundle, binds localhost by default, and refuses to start unless the startup self-check passes — **ADJUSTED per D-08 (06-CONTEXT.md):** the self-check is a **pure-Rust CRS landmark round-trip** (WGS84→UTM→WGS84 ≤ 1 m, CRS/zone logged), not a GDAL/PROJ check; the GDAL/PROJ version/`proj.db`/`GDAL_DATA` self-check and its Windows provisioning move to Phase 8 with the C `gdal` dependency (D-01/D-02 — Phase 6 ships with zero C toolchain)
  3. Exactly one reprojection boundary exists: GeoJSON WGS84 on the wire, an auto-picked UTM CRS pinned per project at creation, newtyped `LonLat`/`SceneXY`, a landmark round-trip test accurate to the meter, and loud rejection of degree-magnitude scene coordinates
  4. The API contract structurally separates `recondition` (conditioning-only → tensor MAC) from `recompute` (scene/terrain/ground/met → propagation), with tensor identity keyed by content hash and a mismatched-hash MAC request rejected (contract-tested against a stub tensor; realized end-to-end in Phase 11) — and all spectra cross the wire as dense arrays keyed by band index with the 105-point 1/12-octave axis served once at a meta endpoint, no client-side acoustic math
  5. The job registry exposes the Queued/Running/Done/Failed/Cancelled state machine with SSE progress: a stub job can be submitted, observed live, and cancelled

**Plans**: 4/4 plans complete

**Wave 1**

- [x] 06-01-PLAN.md — `envi-geo`: pure-Rust CRS boundary crate (proj4rs, LonLat/SceneXY, UTM zone pin, landmark round-trip + pyproj oracle) — GEOX-04

**Wave 2** *(blocked on 06-01)*

- [x] 06-02-PLAN.md — `envi-store`: serde DTO mirror, GeoJSON 9-kind vocabulary, project-as-folder CRUD + atomic saves, calc manifest, blake3 tensor-identity hash — SVC-01/05/07

**Wave 3** *(blocked on 06-02)*

- [x] 06-03-PLAN.md — `envi-service` core: axum binary, D-08 pure-Rust self-check (refuse-to-start), freq-axis meta, project CRUD + scene GET/PUT over HTTP, placeholder bundle — SVC-03/04/05/07

**Wave 4** *(blocked on 06-03)*

- [x] 06-04-PLAN.md — jobs/SSE state machine + recondition/recompute split with enforced 409 hash gate + doc contract (crates/README.md, root README) — SC4/SC5

### Phase 7: Frontend Shell & Scene Editing

**Goal**: Users can draw and edit the complete NoizCalc-style scene on an OSM basemap — including ENVI's semi-transparent screens/buildings and forests — with the scene persisted through the Phase-6 API and validated at draw time
**Depends on**: Phase 6 (API + persistence). Phase 5 (semi-transparent/forest object semantics — SCN/WEB objects must never be silently inert). Engine-independent otherwise — parallel-safe with engine Phases 3–4. Research flag: Terra Draw ⇄ react-map-gl lifecycle spike (instance-in-ref, `style.load` re-hydration, StrictMode) before feature plans
**Requirements**: WEB-01, WEB-02, WEB-03, WEB-04, WEB-08, WEB-09, WEB-10, SCN-01, SCN-02, SCN-03, SCN-04
**Success Criteria** (what must be TRUE):

  1. On a MapLibre OSM/vector basemap, user can place and edit every scene object kind: directional sources (spectrum + SPL-at-reference-point calibration), receiver points, the calculation area, buildings, walls, ground-effect zones (impedance A–H + roughness N/S/M/L), forests (mean tree density / stem radius / height), and elevation points/lines that re-triangulate the DGM — with last-object property inheritance
  2. Partially crossing ground-effect polygons are rejected at draw time (containment allowed, innermost wins), and validation messages click-to-select and zoom to the offending object
  3. User can mark a screen semi-transparent and assign it an isolation spectrum, and assign per-façade isolation spectra on a building; the spectrum editor accepts direct 1/12-octave entry or 1/1- / 1/3-octave input linearly interpolated onto the 105-point grid, with octave and third-octave centres landing exactly on their band indices
  4. The drawn scene survives a basemap switch, a page reload, and a project close/re-open — Terra Draw re-hydrates from the store on `style.load`, and the persisted scene is what comes back

**Plans**: 10/10 plans complete
**Wave 1**

- [x] 07-01-PLAN.md — envi-store isolation/forest DTOs + shared band-index interpolation core + tested TryFrom (D-01/D-05/D-06)
- [x] 07-02-PLAN.md — new envi-dgm crate: spade constrained-Delaunay TIN + DgmError, panic-proof pre-checks (D-08)
- [x] 07-05-PLAN.md — web/ React+Vite+TSX scaffold + metrao3 theme + app shell + real web/dist bundle (D-09/D-11/D-12/D-13a)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 07-03-PLAN.md — envi-service endpoints: POST /meta/interpolate-spectrum + POST /dgm/triangulate + DgmError→ApiError + contract tests
- [x] 07-06-PLAN.md — Gate-1 Terra Draw ⇄ react-map-gl lifecycle spike: store-canonical, style.load rehydrate, dark basemap (D-03/D-13a)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 07-04-PLAN.md — ts-rs generated wire types (committed wire.ts) + no-drift test (D-10)

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 07-07-PLAN.md — object palette + all 9 kinds + property inspector + last-object inheritance + typed fetch client + DGM re-triangulation producer (trigger + TIN overlay)

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 07-08-PLAN.md — isolation-spectrum editor + per-façade UUID ring-diff + semi-transparent screen + SPL calibration (D-02/D-06)

**Wave 6** *(blocked on Wave 5 completion)*

- [x] 07-09-PLAN.md — draw-time ground-zone hard reject + validation panel + debounced autosave + delete-project dialog (D-04/D-07)

**Wave 7** *(blocked on Wave 6 completion)*

- [x] 07-10-PLAN.md — SC1–SC4 end-to-end E2E journeys + final committed bundle + README docs

**UI hint**: yes

### Phase 8: GIS Ingestion & DGM

**Goal**: The NoizCalc "Import" moment — users pull real-world terrain, ground cover, and buildings for the viewport onto a triangulated ground model, and everything imported is an ordinary editable object ("check and complete")
**Depends on**: Phase 6 (job model, CRS boundary, GDAL provisioning), Phase 7 (imported features editable in the scene editor). Engine-independent — parallel-safe with engine Phases 3–4
**Requirements**: DATA-01, DATA-02, DATA-03, DATA-04
**Success Criteria** (what must be TRUE):

  1. A viewport import job fetches Copernicus GLO-30 (national LiDAR DTM preferred where available, GLO-30 flagged as a surface model when used), ESA WorldCover, and Overture/OSM buildings, and materializes them as editable scene objects on a DGM TIN — with live progress and clear failure states
  2. All fetched data is cached locally per project (DATA-04) — in the browser's OPFS under the Phase-8 client-side-WASM pivot (CONTEXT D-03); the compute path reads only the local cache — verified by running with the network off — and the network (whole-tile browser fetch / byte proxy) is touched only at ingestion time
  3. The WorldCover class → Nordtest σ/impedance mapping is a reviewed data table with a unit test asserting every row, and an impedance debug overlay shows the effective ground class everywhere on the map
  4. Buildings missing height data get heights via the documented fallback chain (measured → height tag → levels×3+1.5 → user default) with per-feature provenance (source + license + retrieval date), and base elevations are sampled from footprint-boundary ground, never DSM-under-building
  5. The map shows attribution for OSM/Overture/ESA WorldCover/Copernicus data

**Plans**: 8/8 plans complete

Plans:

- [x] 08-01-PLAN.md — envi-geo RD-New (EPSG:28992) + pyproj oracle (DATA-01)
- [x] 08-02-PLAN.md — envi-gis crate + sans-I/O COG decode core + committed COG fixtures (DATA-01)
- [x] 08-03-PLAN.md — envi-service allowlisted byte proxy (GLO-30/WorldCover) + SSRF contract tests (DATA-01/02)
- [x] 08-04-PLAN.md — envi-gis feature layer: source registry + terrain decimation/base-elevation + WorldCover→σ table + Overpass buildings/height-chain + re-import merge (DATA-01/02/03)
- [x] 08-05-PLAN.md — envi-gis WorldCover→ground_zone vectorization (contour dep DECLINED — hand-rolled marching squares) (DATA-02) (completed 2026-07-11)
- [x] 08-06-PLAN.md — envi-gis-wasm cdylib bindings + Vite/wasm build + version-locked CLI + generated boundary DTOs (DATA-01/02/03)
- [x] 08-07-PLAN.md — web import path: OPFS cache + direct/proxy fetchers + per-layer state machine + ImportPanel + impedance overlay + attribution (DATA-01/02/03/04)
- [x] 08-08-PLAN.md — Playwright offline import journey + DATA-04 network-off replay (DATA-01/02/03/04)

**Wave 1** *(parallel — no shared files)*

- [x] 08-01: envi-geo RD-New
- [x] 08-02: envi-gis COG decode core + fixtures
- [x] 08-03: envi-service byte proxy

**Wave 2** *(blocked on 08-01, 08-02)*

- [x] 08-04: envi-gis feature layer (registry/terrain/impedance/buildings/merge)

**Wave 3** *(blocked on 08-04)*

- [x] 08-05: WorldCover vectorization (contour declined — hand-rolled)

**Wave 4** *(blocked on 08-04, 08-05)*

- [x] 08-06: envi-gis-wasm bindings + build wiring

**Wave 5** *(blocked on 08-03, 08-06)*

- [x] 08-07: web import path (OPFS + fetchers + ImportPanel)

**Wave 6** *(blocked on 08-07)*

- [ ] 08-08: Playwright offline + DATA-04 network-off replay

**UI hint**: yes

### Phase 9: Path Extraction & Weather

**Goal**: The GIS-to-engine geometry pipeline exists — cut profiles, impedance segmentation, screening edges, and receiver grids feed real-world `PropagationPath`s — and real weather flows in from Open-Meteo to drive the per-azimuth A/B/C meteorology
**Depends on**: Phase 8 (cached GIS data + DGM). The geometry half (GEOX-01..03, GRID-01) needs only engine scene types — parallel-safe; the weather half (METX-01/02 A/B/C derivation) gates on engine Phase 3 (MET-02/06). Coordination flag: the `PropagationPath` struct defined here must be agreed early with engine Phase 4's `solver` signature — it is the one type both sides touch
**Requirements**: GEOX-01, GEOX-02, GEOX-03, GRID-01, METX-01, METX-02
**Success Criteria** (what must be TRUE):

  1. The DEM cut-profile extracted along a source→receiver line matches the GRASS `r.profile` oracle within stated tolerance on real DEM data
  2. Ground impedance is segmented along the profile with drawn > imported > default priority, and screening edges are derived from building/wall/barrier geometry along the path (rstar corridor query)
  3. A building-aware constrained-Delaunay receiver grid generates inside the calculation area (plus discrete receiver points), respecting building footprints
  4. Weather import fetches Open-Meteo once per (site, time window), caches it with the project, and derives per-azimuth A/B/C from the multi-level profile; subsequent what-if edits issue zero API calls (verified by network log), and the weighted call cost is logged per fetch
  5. ERA5/CDS groundwork retrieves reanalysis as an async job and derives wind×stability weather-class occurrence statistics (Obukhov length) — full L_den combination stays deferred with GRID-03

**Plans**: 6/6 plans executed — Phase COMPLETE (all 5 completion gates closed 2026-07-11)

Plans:

- [x] 09-01-PLAN.md — DEM cut-profile extractor (GRASS-faithful oracle) + impedance segmentation drawn&gt;imported&gt;default (GEOX-01/02)
- [x] 09-02-PLAN.md — screening edges → TerrainProfile vertices (rstar corridor) + building-aware CDT receiver grid (GEOX-03/GRID-01)
- [x] 09-03-PLAN.md — Open-Meteo multi-level → per-azimuth A/B/C (lift Phase-3 fit) + ERA5 Obukhov/occurrence-stats derivation (METX-01/02)
- [x] 09-04-PLAN.md — WASM boundary DTOs/shims + flagged-off ERA5/CDS service endpoint + wire no-drift (METX-02)
- [x] 09-05-PLAN.md — web weather-import panel: date-switched fetch + OPFS cache + per-azimuth A/B/C + debug overlays (METX-01)
- [x] 09-06-PLAN.md — offline Playwright weather-import journey + SC4 zero-egress what-if proof (METX-01)

**Wave 1**

- [x] 09-01: cut-profile + impedance segmentation

**Wave 2** *(blocked on 09-01)*

- [x] 09-02: screening edges + receiver grid

**Wave 3** *(blocked on 09-02)*

- [x] 09-03: Open-Meteo A/B/C + ERA5 derivation

**Wave 4** *(blocked on 09-01/02/03)*

- [x] 09-04: WASM boundary + flagged ERA5 endpoint

**Wave 5** *(blocked on 09-04)*

- [x] 09-05: web weather-import panel + overlays

**Wave 6** *(blocked on 09-05)*

- [x] 09-06: offline Playwright SC4 zero-egress proof

**UI hint**: yes

### Phase 10: Calculation Service

**Goal**: A user can run a real Nord2000 calculation end-to-end — submit from the UI with a cost estimate, watch progress, abort cleanly — with the transfer tensor streamed to a chunked on-disk store inside a stated memory budget
**Depends on**: **Hard gate: engine Milestone-1 Phase 4** (promoted `envi_engine::solver` + chunk-streaming `TensorSink`/readout signatures — if Phase 4 ships these private to the harness, this phase forces a second refactor of freshly validated code). Also Phase 5 (extensions computed in the solve), Phase 9 (real `PropagationPath`s). Research flag: chunked tensor store format (chunk size, manifest schema, mmap patterns)
**Requirements**: SVC-02, GRID-02, WEB-07
**Success Criteria** (what must be TRUE):

  1. User submits a calculation from the UI and sees a pre-run cost estimate (receiver count, tensor bytes, time estimate) before Run; guardrails warn on grid spacings that explode cost (halving spacing quadruples it)
  2. A running job reports live progress and can be aborted — cancellation takes effect at chunk boundaries, the job lands in Cancelled, and the service stays healthy; failed jobs land in Failed(reason)
  3. The grid solve runs rayon-parallel over receiver-axis chunks streamed to the on-disk tensor store — a large-grid run (~100k receivers) completes within the stated memory budget, RSS bounded by workers × chunk size
  4. The calc manifest records content hashes (scene geometry, met, receiver set, engine version, band axis) so every stored tensor's identity is verifiable, and a scene containing forests and semi-transparent screens/façades computes through ENG-09/10 with their effects visible in the results

**Plans**: 2/5 plans executed

Plans:

- [x] 10-01-PLAN.md — envi-compute pure-Rust core: factored identity + cost/guardrail + hierarchical tiers + SolveJob assembly with directional-phase seam (SRC-03) [wave 1]
- [x] 10-02-PLAN.md — Cross-origin isolation: COOP/COEP credentialless headers on envi-service + Vite dev headers (D-04) [wave 1]
- [ ] 10-03-PLAN.md — envi-compute-wasm cdylib: thin boundary + ts-rs DTOs (TierComplete/JobStatus) + OPFS TensorSink + threaded-wasm build toolchain [wave 2]
- [ ] 10-04-PLAN.md — Caller-side rayon pool sharding + Web Worker job machine + client/store/opfs glue (GRID-02/SVC-02, D-03/D-10/D-11) [wave 3]
- [ ] 10-05-PLAN.md — CalcPanel UI + App mount + offline Playwright UAT of the real threaded-wasm bundle (WEB-07) [wave 4]

**UI hint**: yes

### Phase 11: Results & Fast Recalc

**Goal**: The payoff — spectral readout at receivers, dB(A)/dB(C) isophone noise maps, the flagship interactive source conditioning over the cached tensor, and named weather what-if scenarios with difference maps
**Depends on**: **Hard gates: engine Milestone-1 Phases 3–4** (refraction math + tensor/MAC). Phase 10 (cached tensors + job runner). Research flag: `contour` crate benchmark on 100k+-cell rasters early (gdal-sys `GDALContourGenerateEx` is the documented escape hatch)
**Requirements**: SVC-06, WEB-05, WEB-06, WEB-11, WEB-12, METX-03, METX-04, GRID-04, GRID-05
**Success Criteria** (what must be TRUE):

  1. A receiver point's spectrum panel shows per-band levels (1/12-octave expert view + 1/3-octave display aggregated by band index), dB(A)/dB(C) totals with an instant toggle (both weightings precomputed server-side at the exact grid centres), and the coherent/incoherent split — the frontend performs zero acoustic arithmetic
  2. Changing a source's gain/filter/delay recomputes results interactively via the streaming tensor MAC; the MAC ≡ full-recompute equivalence test passes; a MAC request against a mismatched tensor hash is rejected — never silently served — and the UI shows a results-stale badge the moment the scene diverges from the cached tensor
  3. The noise map renders as server-side isophone fill polygons (no heatmap layer) with an editable color scale where legend breaks ≡ contour breaks ≡ class colors and the weighting label comes from result metadata; editing the scale re-contours the cached level grid without re-running propagation
  4. Weather what-if works end-to-end: manual overrides (T/RH/p, Beaufort wind + direction, downwind worst-case toggle, temperature gradient, per-azimuth A/B/C) recompute as a scenario; named scenarios switch instantly via per-scenario cached tensors; difference maps render between two scenarios
  5. Results export as GeoTIFF/GeoJSON and spectra as CSV, carrying band index + exact frequency columns and data attribution metadata

**Plans**: TBD
**UI hint**: yes

## Requirement Coverage

| Phase | Requirements | Count |
|-------|--------------|-------|
| 1 | VAL-01, GEO-01, GEO-02, GEO-03, ENG-01, ENG-04, SRC-01 | 7 |
| 2 | ENG-02, ENG-03, ENG-07 | 3 |
| 3 | ENG-05, ENG-06, ENG-08, MET-01..06 | 9 |
| 4 | OUT-01..06, SRC-02, SRC-03, SRC-04, VAL-02, VAL-03 | 11 |
| 5 | ENG-09, ENG-10 | 2 |
| 6 | SVC-01, SVC-03, SVC-04, SVC-05, SVC-07, GEOX-04 | 6 |
| 7 | WEB-01..04, WEB-08, WEB-09, WEB-10, SCN-01..04 | 11 |
| 8 | DATA-01..04 | 4 |
| 9 | GEOX-01, GEOX-02, GEOX-03, GRID-01, METX-01, METX-02 | 6 |
| 10 | SVC-02, GRID-02, WEB-07 | 3 |
| 11 | SVC-06, WEB-05, WEB-06, WEB-11, WEB-12, METX-03, METX-04, GRID-04, GRID-05 | 9 |

**Coverage check:** 30/30 v1 requirements mapped (ENG 8, OUT 6, MET 6, SRC 4, GEO 3, VAL 3). No orphans, no duplicates. v2 groups (DATA, GEOX, METX, GRID, WEB, SVC, FUT) deferred by design.

**Milestone 2 coverage check:** 41/41 Milestone-2 requirements mapped (ENG 2, SCN 4, DATA 4, GEOX 4, METX 4, GRID 4, WEB 12, SVC 7). No orphans, no duplicates. GRID-03 (L_den weather-class combination) and FUT-01..05 are deferred beyond Milestone 2 by design and intentionally unmapped.

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4

Milestone 2 (5 → 6 → 7 → 8 → 9 → 10 → 11) is planned ahead: Phase 5 needs only completed engine Phase 2 (coordinate with Phase 4's solver promotion); Phases 6–8 and Phase 9's geometry half are engine-independent and parallel-safe with engine Phases 3–4; Phase 9's weather half gates on engine Phase 3; Phases 10–11 hard-gate on engine Phase 4.

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. FORCE Harness, Geometry Model & Direct Path | 3/3 | Complete | 2026-07-07 |
| 2. Ground Effect & Diffraction | 5/5 | Complete | 2026-07-08 |
| 3. Meteorology & Refraction | 3/3 | Complete | 2026-07-08 |
| 4. Transfer Tensor, Directional Sources & Full Validation | 5/5 | Complete (VAL-02 numeric Pass deferred — external coefficient blocker) | 2026-07-09 |
| 5. Engine Extensions — Forest & Semi-Transparent Partitions | 3/3 | Complete (all 5 completion gates closed) | 2026-07-09 |
| 6. Service Foundation & Persistence | 4/4 | Complete (all 5 completion gates closed) | 2026-07-09 |
| 7. Frontend Shell & Scene Editing | 10/10 | Complete (all 5 completion gates closed) | 2026-07-10 |
| 8. GIS Ingestion & DGM | 8/8 | Complete (all 5 completion gates closed) | 2026-07-11 |
| 9. Path Extraction & Weather | 6/6 | Complete (all 5 completion gates closed) | 2026-07-11 |
| 10. Calculation Service | 0/? | Not started | - |
| 11. Results & Fast Recalc | 0/? | Not started | - |

---
*Roadmap created: 2026-07-07 — Milestone 1 (validated core Nord2000 engine)*
*Milestone 2 phases appended: 2026-07-08 — Interactive Calculation UI (Phases 5–11; engine Phases 3–4 remain the execution priority)*
