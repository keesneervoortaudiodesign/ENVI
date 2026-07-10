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
| `envi-geo` | The one pure-Rust CRS reprojection seam (GEOX-04): WGS84 ↔ project-local UTM via `proj4rs` (no C toolchain). `LonLat`/`SceneXY` newtypes. |
| `envi-store` | serde DTO mirror (keeps serde **out** of `envi-engine`) + project-as-folder flat-file persistence + the frozen tensor-identity hash (conditioning excluded, D-07). |
| `envi-dgm` | The server-side Digital Ground Model boundary (D-08): a pure-Rust constrained-Delaunay TIN (`spade`) from elevation points + breaklines. Deps: `spade`, `thiserror`. **No** `envi-engine` edge (keeps `spade` out of the engine quarantine), no C toolchain, no I/O. Backs `POST /dgm/triangulate`. |
| `envi-service` | The single deployable **axum** binary: `/api/v1` + the committed `web/dist` bundle, localhost-bound, refuse-to-start CRS self-check, job/SSE state machine, recondition/recompute split. Thin HTTP layer — no acoustics. |
| `web/` | Vite + React 19 + TSX single-page scene editor (MapLibre GL JS 5 + react-map-gl 8 + Terra Draw). Imports the generated wire-DTO mirror (D-10); Playwright-driven offline. The production build is committed at `web/dist/` and served by `envi-service`. |
| `tools/nord2000_oracle` | Committed, independent Python (`scipy.special.wofz`) reference implementation that generates the pinned fixture curves. **Not** a build dependency. |

See [`crates/README.md`](crates/README.md) for each crate's boundary rule, entry
points, the dependency-direction diagram, and the engine quarantine gates.

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

### Directional phase — beyond stock Nord2000

Stock Nord2000 road directivity is a **real** `ΔL(θ,φ,f)` summed *incoherently*.
ENVI's directivity balloons additionally carry an **optional per-band phase**
`Δφ(θ,φ,f)`, so a directional source contributes a genuinely **complex** gain
`10^{ΔL/20}·e^{+jΔφ}` (the GLL/complex-balloon datum) — which is what makes
**coherent** summation of multiple directional sub-sources (e.g. loudspeaker
arrays) correct rather than merely energetic. The phase enters the **coherent
`H_coh` channel only** (applied on the ENVI post-conj side, never in
`propagation/`); the incoherent energy channel stays magnitude-only, so the
two-channel contract is untouched. A phase-free balloon is **bit-identical** to
the magnitude-only path. Engine seam: `DirectivityBalloon::eval_phase` /
`eval_complex` → `SolveJob::directivity_phase_rad`.

## Capabilities

| Capability | Status | Phase |
|------------|--------|-------|
| Scene / path geometry | ✅ implemented | 1 |
| Free-field direct path (divergence + ISO 9613-1 air absorption) | ✅ implemented | 1 |
| **Ground effect** (segmented impedance, spherical-wave Q̂) | ✅ implemented | 2 |
| **Screen / barrier diffraction** (single / thick / double) | ✅ implemented | 2 |
| **Meteorological refraction** (wind + temperature gradients, turbulence coherence) | ✅ implemented | 3 |
| Nord2000 road emission model (Jonasson Table A.1, pass-by integration) | ✅ implemented (coeffs cited/intermediate) | 4 |
| Transfer tensor `H[s,r,f]` + MAC conditioning + directional balloons (complex phase) | ✅ implemented | 4 |
| FORCE road-traffic numeric Pass (VAL-02) | ⏳ deferred — external coefficient blocker (~2.3 dBA) | 4 |
| **Forest excess attenuation** (Sub-Model 10, Eqs. 288–291) | ✅ implemented | 5 |
| **Semi-transparent partitions** (min-phase transmission `T(f)`, opaque = `None`) | ✅ implemented | 5 |

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
  terrain (Sub-model 3, §5.12) remains a **typed hard error** — never a silent
  approximation.
- **Two-channel readout (ENG-07).** `terrain_effect()` returns the phase-live
  `h_coh_factor` and the real `p_incoh` per band; `band_levels_db_two_channel`
  forms `L = L_W + 10·lg(|H_coh|² + |H_ff|²·P_incoh)`.

### Phase 3 — meteorology & refraction

- **Equivalent-linear profile (MET-03).** The log-lin sound-speed profile
  `c(z) = A·ln(z/z₀+1) + B·z + C` collapses to an equivalent-linear curvature
  `ξ` via `CalcEqSSP`, averaging `∂c/∂z` between source and receiver heights,
  with the singular `|ξ| < 1e-6` homogeneous clamp and a cancellation-safe
  `ΔR = 4·h_S·h_R/(R₁+R₂)` travel-time difference.
- **Circular rays & shadow zone.** Direct/reflected rays over the curved
  profile (cubic reflection-point solve), the shadow-zone edge `d_SZ`, and the
  upward-refraction shielding that reuses the Phase-2 wedge kernel `pwedge0`.
  Below the ξ clamp the ray solver reproduces the straight-ray result
  **bit-for-bit** (the D-02 anchor).
- **Frequency-dependent ground (MET-04).** `CalcEqSSPGround` makes `ξ(f)`
  frequency-dependent over soft ground via `f_L`/`f_H` log-interpolation,
  evaluated by **band index** on the 105-point grid; hard ground stays
  frequency-independent.
- **Per-azimuth weather (MET-02) & routes (MET-05/06).** `A` is derived per
  source→receiver bearing (isotropic temperature part once + projected wind
  `u·cos(az−φ_u)`), an inversion (`dt/dz>0`) gives `B>0`. All three input
  routes are built — Route 1 (weather-class table → energy-weighted `L_den`),
  Route 2 (surface met → A/B), Route 3 (Monin–Obukhov reconstruction +
  hand-rolled 3×3 least-squares fit, no linear-algebra crate) — plus the
  reflection-path before/after split (`A₁/B₁`, `A₂/B₂`, ENG-06).
- **Turbulence coherence (ENG-08).** The fluctuating-refraction factor
  `F_τ` (Eq. 112, sinc with `x = 2π·f·|Δτ⁺−Δτ|`) enters through the existing
  `CoherenceInputs::f_delta_nu` seam with no call-site change; when the
  fluctuation std-devs are zero it is `1.0` **bit-exact**, so `P_incoh → 0`.
- **Honest-green scope.** The weather-route A/B/C scaling constants are
  `[ASSUMED]` (AV 1106/07 does not specify them) and validated by
  structure/direction property tests and the committed scipy oracle only — no
  false FORCE numeric pass. Wind/gradient FORCE cases stay
  `Skipped(requires: emission-model)` until the Phase-4 road-emission model.
  Segmented-ground and screened refraction are typed `NotImplemented` errors
  (deferred to Phase 4), never a silent partial result.

### Phase 5 — engine extensions (forest & semi-transparent partitions)

- **Forest excess attenuation (ENG-09).** Nord2000 **Sub-Model 10** scattering-zone
  excess attenuation `ΔL_s` (AV 1106/07 §5.19, Eqs. 288–291, Tables 8/9) from mean
  tree density, mean stem radius, average tree height, and mean absorption:
  `nQ` (Eq. 290), `T` (Eq. 289), `k_f` (Table 8), `A_e` (Table 9 tensor-product
  PCHIP), `ΔL_s = Max(1.25·k_f·T·A_e, −15)` (Eq. 291) — exactly `0` below
  `ka = 0.7`. The **−15 dB floor** and the `T`-saturation ARE Nord2000's own
  distance bounding (so the ISO 9613-2 10/20/200 m regimes are correctly out of
  scope). Applied **solver-side** as a per-band real dB factor on **both** channels
  (`10^{ΔL_s/20}` on `H_coh` with `arg` untouched, `10^{ΔL_s/10}` on `P_incoh`),
  post-conj — never a `propagation/` operator. The Eq. 288 `Fs` coherence factor is
  a **documented deferral** (see the phase `deferred-items.md`).
- **Semi-transparent partitions (ENG-10, ENVI extension).** A partition's isolation
  spectrum `R(f)` becomes a complex **minimum-phase** transmission filter
  `T(f) = 10^(−R/20)·e^{jφ_min}`, `φ_min = −H{ln|T|}` reconstructed via an
  even-mirror real-cepstrum fold over the 105-point band axis (a hand-rolled
  208-point DFT — no FFT crate). This is a documented extension **beyond stock
  Nord2000's real energy loss**: a passive partition is a minimum-phase system, so
  its transmitted phase follows its amplitude (the same discipline as the Phase-4
  directional complex phase). A flat `R` gives `φ ≡ 0`, bit-compatible with a pure
  attenuation. `T` is threaded **inside `propagation/`** (native `e^{−jωt}`,
  pre-conj, D-05) and added to the screen branch's coherent factor at the single
  `screen_channel` composition point (covering Sub-models 4/5/6) — the
  straight-through leakage relative to `p̂₀` is exactly `T(f)`, pinned end-to-end.
  It joins the **coherent channel only** — never `P_incoh` (the min-phase filter is
  deterministic, never decorrelated by `F`).
- **Opaque = `None`, bit-for-bit (D-10).** Opaque is the **structural absence** of a
  spectrum (`isolation: None`), NOT a large-`R` sentinel — the transmission term and
  the min-phase computation are never constructed on the `None` path, so the opaque
  screen result is reproduced **bit-for-bit** (a permanent committed regression,
  `opaque_regression.rs`). An isolation spectrum over flat terrain (no partition on
  the path) is a **typed error** (`IsolationWithoutScreen`), never a silent no-op.
  The `R → 0` corner is a documented **model property**, not a bug: `R ≡ 0` restores
  the direct field plus the diffracted residue (inherent to the locked additive
  composition, benign for physical partitions — never renormalized).
- **Per-façade reuse (D-11).** A building façade's `R(f)` rides the same seam: the
  engine applies whichever crossed partition's spectrum the job carries; façade
  selection and multi-partition composition are upstream Phase-7/9 concerns.

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

**FORCE road-case status (post-Phase-4).** The full road chain — emission →
tensor → ground/screen (SM1/2/3/11) → refraction → Ch.6 comparator — is wired,
and the road-emission coefficients are now **CITED** (Table A.1 from the committed
Jonasson source-modelling report, verified against the page image). The overall
numeric FORCE Pass is, however, **deferred on an external blocker**: that Table
A.1 is the report's *intermediate* DK Nord 2005 set and over-predicts the FORCE
free-field emission by a measured **~2.3 dBA** (`emission_force_delta` report-only
test), outside the Ch.6 1 dB tolerance. Per the honest-green rule the cases stay
`Skipped` with the measured-gap reason — never a false Pass — pending the
definitive Dec-2006 coefficient set. Forest cases (121–124) stay
`Skipped(requires: forest-scattering)`: the Sub-Model 10 excess-attenuation math
now exists (ENG-09, Phase 5) and is validated in-crate against a committed scipy
oracle, but the road-case `ForestCrossing` geometry extraction (rubber-band path
over the forest, Fig. 29) is a **Phase-9** upstream concern — so the FORCE forest
cases remain capability-gated, never a false Pass (D-12). The propagation physics
is validated in-crate by the oracle/anchor/property ladder above.

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

### Run the service

The `envi-service` binary is the single self-hosted deployable — one axum process
serves both the `/api/v1` JSON/GeoJSON API and the `web/dist` frontend bundle:

```sh
cargo run -p envi-service
# then browse http://127.0.0.1:8080/  (the Phase-7 MapLibre/React scene editor)
```

It binds `127.0.0.1:8080` by default and **refuses to start** unless the D-08
pure-Rust CRS self-check passes (a WGS84→UTM→WGS84 landmark round-trip within
1 m; the GDAL/PROJ self-check is deferred to Phase 8 with the C dependency).
Environment overrides:

| Var | Default | Purpose |
|-----|---------|---------|
| `ENVI_BIND` | `127.0.0.1:8080` | Listen address (a non-loopback bind logs a prominent no-auth warning). |
| `ENVI_PROJECTS_DIR` | `./projects` | The project-folder store root. |
| `ENVI_WEB_DIST` | `web/dist` | The static frontend bundle served with SPA fallback. |

### Build the frontend

The `web/` scene editor is built with Vite; the output is committed at
`web/dist/` and served by `envi-service`, so a plain `cargo run -p envi-service`
needs no Node step. To rebuild the bundle after changing `web/src`:

```sh
cd web
npm install          # first time only
npm run build        # emits the committed web/dist/ (shell + map + panels + spectrum)
```

The dark OpenFreeMap basemap (MIT, no API key) is fetched from the network **at
runtime** in the browser; the Playwright UAT, by contrast, route-intercepts the
basemap + every `/api/v1` call so `npx playwright test` runs **fully offline**.
All frontend tooling is a `devDependency` only — none of it ships in `web/dist`.

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
| 3 | Meteorology & refraction: log-lin A/B/C profile, equivalent-linear collapse (guarded ξ/Δτ), weather routes, turbulence coherence | ✅ complete |
| 4 | Dense `H[s,r,f]` transfer tensor + filter/delay recalculation, directional multi-sub-source composition, full FORCE-suite pass + NoiseModelling cross-check | ⏳ next |

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
