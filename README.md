# ENVI — Nord2000 GIS Sound Propagation Engine

A numerically faithful, from-scratch implementation of the **Nord2000** outdoor
sound-propagation method (AV 1106/07 rev. 4) in Rust. The engine computes
per-band, phase-preserving complex transfer values over terrain; a sibling
harness validates it against the FORCE road-traffic test cases and committed
independent oracles.

The repository currently holds the **Milestone 1 core engine** — no map, no UI, no
live GIS ingestion yet (geometry comes from FORCE test-case files). **Milestone 2
— an interactive, NoizCalc-style web application around the engine — is now fully
planned** (requirements + roadmap, phases 5–11); the engine (Milestone 1 Phases
3–4) remains the current execution priority. See
[Milestone 2](#milestone-2--interactive-calculation-ui-planned) below.

## Workspace layout

| Crate | Role |
|-------|------|
| `envi-engine` | Pure-math Nord2000 core: `#![deny(unsafe_code)]`, `f64`/`Complex<f64>` only, **no I/O**. Deps: `ndarray`, `num-complex`, `thiserror`. |
| `envi-harness` | All I/O: FORCE `.xls` parsing, synthetic TOML cases, capability gating, reference comparison, reporting. |
| `tools/nord2000_oracle` | Committed, independent Python (`scipy.special.wofz`) reference implementation that generates the pinned fixture curves. **Not** a build dependency. |

## The output contract (user-mandated)

Every environmental operator is applied as a **complex, phase-preserving**
operation — nothing collapses to magnitude/energy along the chain:

- **`H_coh`** is a genuine `Complex<f64>` per 1/12-octave point with live phase:
  the Δτ interference between direct, ground-reflected and diffracted paths lives
  in the phase.
- **`P_incoh`** is a *separate* real channel: Nord2000's turbulence-decorrelated
  `(1 − F²)` energy, added **only at final level readout**, never overwriting
  phase. Total level = `10·lg(|coherent Σ|² + P_incoh)`.
- Exactly **one** `conj()` in the whole propagation codebase converts the
  Nord2000-native `e^{−jωt}` convention to ENVI's frozen `e^{+jωt}` transfer
  convention — `transfer::nord_ratio_to_transfer`. The grep gate
  `grep -rh '\.conj()' crates/envi-engine/src/propagation/` returns **0**.

## Capabilities

| Capability | Status | Phase |
|------------|--------|-------|
| Scene / path geometry | ✅ implemented | 1 |
| Free-field direct path (divergence + ISO 9613-1 air absorption) | ✅ implemented | 1 |
| **Ground effect** (segmented impedance, spherical-wave Q̂) | ✅ implemented | 2 |
| **Screen / barrier diffraction** (single / thick / double) | ✅ implemented | 2 |
| Meteorological refraction (wind + temperature gradients) | ⏳ Phase 3 | 3 |
| Nord2000 road emission model (Jonasson, pass-by integration) | ⏳ Phase 4 | 4 |

### Phase 2 — ground effect & diffraction

- **Ground effect (ENG-02).** Delany–Bazley impedance `Ẑ_G`, plane-wave `Γ̂_p`,
  spherical-wave reflection coefficient `Q̂` via the document's own Faddeeva
  `w(z)` approximation (Eqs. 57–74), and the incoherent coefficient `ρᵢ`.
  Sub-model 1 (flat, one surface type) and Sub-model 2 (segmented impedance,
  per-surface-*type* Fresnel-zone blend with `PhaseDiffFreq`).
- **Screen diffraction (ENG-03).** Hadden–Pierce finite-impedance wedge solution
  and the screen⇄ground four/eight-path image models: Sub-model 4 (one edge),
  Sub-model 5 (thick screen, two edges), Sub-model 6 (two screens), plus
  Sub-model 7 turbulence scattering (Tables 6/7) that floors the deep-shadow
  attenuation.
- **§5.21 terrain interpretation** identifies the primary/secondary edges,
  reduces the screen shape (`Convex`, Eq. 336), computes the equivalent flat
  terrain, and produces the §5.22 Eq. 332 transition parameters
  (`r_scr1`/`r_scr2`/`r_scr12`/`r_flat`) that compose the sub-models. Non-flat
  terrain (Sub-model 3, §5.12) is a **typed hard error** scheduled with Phase 3
  — never a silent approximation.
- **Two-channel readout (ENG-07).** `terrain_effect()` returns the phase-live
  `h_coh_factor` and the real `p_incoh` per band; `band_levels_db_two_channel`
  forms `L = L_W + 10·lg(|H_coh|² + |H_ff|²·P_incoh)`.

### Validation approach

Per-band FORCE reference values embed the Phase 4 emission model, so no FORCE
road case gates end-to-end before Phase 4. Phase 2 is validated at the
**propagation level** via a layered acceptance ladder:

1. **Exact unit anchors** — `Ẑ_G`, `Q̂`, `w(z)`, the ΔR identity, the ground-dip
   table, the wedge insertion-loss table.
2. **Committed scipy oracles** — five end-to-end terrain cases
   (`cases/terrain_*.toml`) cross-checked against independent 105-point
   references at **≤ 0.1 dB** (flat σ=200, mixed case-21, thin/thick/double
   screens).
3. **Property + finiteness** — the dip lands on the predicted grid band, soft
   ground attenuates more than hard, screen insertion loss grows with edge count,
   and **every evaluated quantity is finite across all 62 FORCE straight-road
   geometries × 105 bands** (ROADMAP success criterion 3).

FORCE road cases remain `Skipped(requires: emission-model)` — the skip-reason
list has *shrunk* now that ground effect and diffraction are implemented.

## Building & running

```sh
cargo build --workspace
cargo test  --workspace
cargo run   -p envi-harness -- report     # per-case outcome table
```

Regenerating the committed oracle fixtures (requires Python + scipy/numpy; the
generated TOML is committed, so this is operator-driven, not a build step):

```sh
cd tools/nord2000_oracle && python gen_case_fixtures.py
```

## Why phase-preserving? (the fast-recalc tensor)

Keeping `H_coh` complex through the whole chain is what makes interactive input
conditioning cheap. Propagation (geometry + meteorology) is expensive, but a
source *filter* is a per-frequency complex gain and a *delay* is a phase ramp
`e^{−j2πfτ}`. With the transfer cached, adjusting conditioning is a complex
multiply-accumulate `p[r,f] = Σ_s H[s,r,f]·G_s(f)` — no re-propagation. The
`TransferTensor = Array3<Complex<f64>>` indexed `[sub_source, receiver, freq]` is
the frozen forward contract for that Phase-4 recalculation path.

## Roadmap — Milestone 1 (validated core engine)

| Phase | Scope | Status |
|-------|-------|--------|
| 1 | FORCE harness, semantic 2.5D scene, complex 1/12-octave direct path (divergence + ISO 9613-1) | ✅ complete |
| 2 | Ground effect (segmented impedance, Q̂) + single/multi-edge diffraction + two-channel combination | ✅ complete |
| 3 | Meteorology & refraction: log-lin A/B/C profile, equivalent-linear collapse (guarded ξ/Δτ), weather routes, turbulence coherence | ⏳ next |
| 4 | Dense `H[s,r,f]` transfer tensor + filter/delay recalculation, directional multi-sub-source composition, full FORCE-suite pass + NoiseModelling cross-check | ⏳ |

## Milestone 2 — Interactive Calculation UI (planned)

A self-hosted web application wrapped around the engine, with the workflow modeled
on **d&b NoizCalc** (single integrated app, **Nord2000-only**). At the end of this
milestone a user can:

1. **CRUD an ENVI model** — projects created / opened / saved (autosave) / deleted / duplicated (project-as-folder, scene + settings + cached tensors).
2. **Get GIS data** — viewport import of terrain (Copernicus GLO-30 + national LiDAR DTM), ground cover (ESA WorldCover → impedance), and buildings (Overture/OSM) onto a triangulated Digital Ground Model, then check-and-complete editing.
3. **Get weather data** — Open-Meteo import deriving the per-azimuth A/B/C meteorology (ERA5/CDS groundwork).
4. **Manual weather what-if** — override wind (Beaufort + direction), downwind worst-case, temperature gradient, A/B/C; **named scenarios** with per-scenario cached tensors, instant switching, and **difference maps**.
5. **Draw scene objects** — directional sources, walls, buildings, forests, ground-effect (damping/impedance) zones, elevation points/lines, receivers, calculation area.
6. **Spectral results at points** — per-band readout (1/12-oct expert + 1/3-oct by band index), dB(A)/dB(C) totals, coherent/incoherent split, CSV export.
7. **Noise map in dB(A) & dB(C)** — server-side isophone **fill polygons** (not a heatmap) with an editable color scale + legend.

### New engine extensions (beyond stock Nord2000)

- **ENG-09 — Forest attenuation.** The Nord2000 `A = d·a(f)` term (mean tree density, mean stem radius, `kp`, mean absorption) so drawn forests actually attenuate.
- **ENG-10 — Semi-transparent partitions.** A **finite-transmission** screen/façade: the standard opaque-screen diffraction/reflection **plus** a straight-through transmission path (direction preserved) attenuated by a per-band **isolation spectrum** `R(f)` (amplitude ×`10^(−R(f)/20)`), combined as **phase-preserving complex pressure**. The opaque limit `R(f)→∞` reproduces the standard screen bit-for-bit. Buildings carry a **per-façade** `R(f)`. Isolation spectra are entered on the 1/12-octave grid, or as 1/1-/1/3-octave values linearly interpolated (in dB across band index) onto the grid.

### Planned architecture (three new crates + a web frontend)

| Piece | Role |
|-------|------|
| `envi-gis` | The single C-linked boundary: GDAL/PROJ + data acquisition (DEM COG `/vsicurl/` reads, WorldCover, Overture, Open-Meteo/ERA5) + geometry derivation (auto-UTM, DGM TIN, **DEM cut-profile** extraction, impedance segmentation, screening edges, CDT receiver grids, contouring). |
| `envi-store` | serde DTO mirror of the engine scene types (keeps serde **out** of `envi-engine`), project-folder layout, **receiver-axis-chunked** tensor store. |
| `envi-service` | axum HTTP API + job registry (SSE progress, cancellation) + the **recondition/recompute** recalc router; serves the built frontend as one deployable binary. |
| `web/` | Vite + React, **MapLibre GL JS 5 + react-map-gl 8 + Terra Draw** scene editor, property panels, weather what-if, job status, spectra charts, isophone overlay. |

`envi-engine` stays byte-identical (dependency quarantine preserved). The one
load-bearing engine refactor is promoting the Scene→level solver out of
`envi-harness` into `envi_engine::solver` (during Phase 4) so the FORCE suite
validates the exact code the web app runs.

### Roadmap — Milestone 2 (phases 5–11, appended non-destructively)

| Phase | Scope | Engine gate |
|-------|-------|-------------|
| 5 | **Engine extensions** — forest (ENG-09) + semi-transparent partitions (ENG-10), phase-preserving, opaque-limit regression | needs Phase 2 (done) |
| 6 | **Service foundation & persistence** — project store, one CRS boundary, band-index wire contract, recondition/recompute split, job state machine | parallel-safe |
| 7 | **Frontend shell & scene editing** — MapLibre/Terra Draw editor for all objects incl. semi-transparent screens/buildings + isolation-spectrum editor | parallel-safe |
| 8 | **GIS ingestion & DGM** — viewport import onto an editable DGM; offline compute path | parallel-safe |
| 9 | **Path extraction & weather** — cut-profile (GRASS `r.profile` oracle), segmentation, screening edges, CDT grids; Open-Meteo → A/B/C | weather half gates on engine **Phase 3** |
| 10 | **Calculation service** — submit/progress/abort + cost estimate; chunked, memory-bounded tensor store | hard gate engine **Phase 4** |
| 11 | **Results & fast recalc** — spectra, isophone maps, interactive MAC conditioning, named what-if scenarios + diff maps, exports | hard gate engine **Phases 3–4** |

Phases 5–8 are parallel-safe with the engine finish; the calculation/results
phases wait on the engine's transfer tensor. **Deferred beyond Milestone 2:** L_den
weather-class statistics, variable wall height, road/rail emission, DXF/SketchUp
import, 2.5D BEM barrier corrections (Bempp), SOFA directivity.

**Stack (all versions verified 2026-07-08):** axum 0.8 / tokio / rayon backend,
`ndarray-npy` + `memmap2` tensor chunks, `gdal` 0.19 / `proj` 0.31 (quarantined)
+ pure-Rust `geoparquet` / `contour`, MapLibre GL JS 5 + react-map-gl 8 +
Terra Draw + Vite 8 + React 19 frontend, Playwright for UAT. See `.planning/` for
the full project context, requirements, research (`.planning/research/`), and roadmap.

## Licensing note

The Nord2000 method is implemented **from the published equations**, cited by
report and equation number only. No copyrighted PDF text, figures, or reference
spreadsheets are committed; the numeric class/coefficient tables are
method-defined facts.
