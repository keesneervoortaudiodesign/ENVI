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
- [ ] **Phase 2: Ground Effect & Diffraction** - Segmented-impedance ground reflection, single/multi-edge screens, complex-pressure combination with Δτ interference
- [ ] **Phase 3: Meteorology & Refraction** - Log-lin A/B/C profile, equivalent-linear collapse with guarded ξ/Δτ numerics, reflection-path coefficients, weather routes, turbulence coherence
- [ ] **Phase 4: Transfer Tensor, Directional Sources & Full Validation** - `H[s,r,f]` complex tensor store, directional multi-sub-source composition, filter/delay conditioning via MAC, full FORCE pass + NoiseModelling cross-validation

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

**Plans**: 5 plans

Note (planned 2026-07-07): success criteria 1-2 are satisfied at propagation level via the oracle+anchor acceptance ladder — FORCE reference spectra require the Phase 4 emission model (VAL-02 maps to Phase 4), so road cases stay capability-gated with a shrinking requires-list. Sub-model 3 (non-flat terrain, §5.12) is explicitly deferred to Phase 3 (typed-error stub; flat Phase 2 targets give r_flat = 1).

Plans:

- [x] 02-01-PLAN.md — Nord2000-native numerics core: Faddeeva w(ẑ) + Fresnel fits, Ẑ_G→Q̂ chain + ρᵢ, straight-ray Δτ (cancellation-free), coherence F with FΔν seam; committed scipy oracle (ENG-02, ENG-07)
- [ ] 02-02-PLAN.md — Flat-terrain ground effect: Fresnel-zone machinery, Sub-model 1 (dip anchors, two-channel GroundResult), Sub-model 2 segmented impedance per surface type + PhaseDiffFreq; oracle curves (ENG-02, ENG-07)
- [ ] 02-03-PLAN.md — Wedge diffraction kernels: Hadden–Pierce pwedge/Dwedge, lit-zone + angle-modification, p2wedge/p2edge/pwedge0; IL + shadow-boundary anchors (ENG-03)
- [ ] 02-04-PLAN.md — Screen⇄ground sub-models 4/5/6 (generic engine, four/eight-path complex combination) + Sub-model 7 turbulence scattering; thin-screen oracle curve (ENG-03, ENG-07)
- [ ] 02-05-PLAN.md — §5.21 terrain interpretation + Eq. 332 composition + two-channel H_coh/P_incoh transfer integration (single conj boundary), capabilities flipped, five oracle-pinned terrain cases + finiteness sweep (ENG-02, ENG-03, ENG-07)

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

**Plans**: 3 plans

Plans:

- [ ] 03-01: Log-lin profile + CalcEqSSP equivalent-linearization + guarded ray variables (ξ clamps, cancellation-safe Δτ, shadow zone)
- [ ] 03-02: CalcEqSSPGround frequency-dependent variant + reflection-path A₁/B₁/A₂/B₂ + per-azimuth A derivation
- [ ] 03-03: Weather input routes (Route 1 classes, Route 3 Monin–Obukhov fit) + F_τ turbulence coherence; refraction FORCE cases green

### Phase 4: Transfer Tensor, Directional Sources & Full Validation

**Goal**: The engine's output contract is real — a dense complex transfer tensor `H[sub_source, receiver, 1/12-oct freq]` fed by directional multi-sub-source composition, with filter/delay conditioning recomputed by cheap complex MAC — and the whole engine passes the full FORCE suite plus NoiseModelling cross-checks
**Mode:** mvp
**Depends on**: Phase 3
**Requirements**: OUT-01, OUT-02, OUT-03, OUT-04, OUT-05, OUT-06, SRC-02, SRC-03, SRC-04, VAL-02, VAL-03
**Success Criteria** (what must be TRUE):

  1. The engine produces a complex transfer value per (directional sub-source × receiver × 1/12-octave point), stored as a dense frequency-contiguous `Complex<f64>` ndarray `H[sub_source, receiver, freq]` for both single-receiver and multi-receiver cases
  2. Changing a source's conditioning — a per-frequency complex filter gain G_s(f) or a delay phase ramp e^{−j2πfτ} — recomputes receiver spectra via `p[r,f] = Σ_s H[s,r,f]·G_s(f)` with no propagation re-run, and the MAC result matches a full recompute to numerical identity
  3. A complex source composed of multiple directional sub-sources (per-band spherical directivity balloons, ΔL(θ,φ,f)) evaluates each sub-source independently into the tensor, and rotating a directivity balloon changes receiver levels in the expected direction
  4. The full FORCE road-traffic test suite passes within the standard's tolerance — the Milestone 1 acceptance gate
  5. Shared sub-effects (geometrical divergence, ISO 9613-1 air absorption, screen geometry) agree with NoiseModelling's CNOSSOS output within documented expected deltas, and a large synthetic receiver set computes with the tensor chunked/streamed inside a stated memory budget

**Plans**: 3 plans

Plans:

- [ ] 04-01: Complex tensor store (ndarray layout, chunk/stream memory budget) + conditioning MAC path (filter, delay)
- [ ] 04-02: Directional sub-sources — spherical directivity balloons, multi-sub-source composition into the tensor
- [ ] 04-03: Full FORCE suite pass + NoiseModelling cross-validation of shared sub-effects; milestone acceptance

## Requirement Coverage

| Phase | Requirements | Count |
|-------|--------------|-------|
| 1 | VAL-01, GEO-01, GEO-02, GEO-03, ENG-01, ENG-04, SRC-01 | 7 |
| 2 | ENG-02, ENG-03, ENG-07 | 3 |
| 3 | ENG-05, ENG-06, ENG-08, MET-01..06 | 9 |
| 4 | OUT-01..06, SRC-02, SRC-03, SRC-04, VAL-02, VAL-03 | 11 |

**Coverage check:** 30/30 v1 requirements mapped (ENG 8, OUT 6, MET 6, SRC 4, GEO 3, VAL 3). No orphans, no duplicates. v2 groups (DATA, GEOX, METX, GRID, WEB, SVC, FUT) deferred by design.

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. FORCE Harness, Geometry Model & Direct Path | 3/3 | Complete | 2026-07-07 |
| 2. Ground Effect & Diffraction | 1/5 | In Progress | - |
| 3. Meteorology & Refraction | 0/3 | Not started | - |
| 4. Transfer Tensor, Directional Sources & Full Validation | 0/3 | Not started | - |

---
*Roadmap created: 2026-07-07 — Milestone 1 (validated core Nord2000 engine)*
