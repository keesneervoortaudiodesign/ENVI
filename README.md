# ENVI — Nord2000 GIS Sound Propagation Engine

A numerically faithful, from-scratch implementation of the **Nord2000** outdoor
sound-propagation method (AV 1106/07 rev. 4) in Rust. The engine computes
per-band, phase-preserving complex transfer values over terrain; a sibling
harness validates it against the FORCE road-traffic test cases and committed
independent oracles.

This repository is the **Milestone 1 core engine** — no map, no UI, no live GIS
ingestion yet (geometry comes from FORCE test-case files).

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

Beyond Milestone 1 (later milestones): live GIS ingestion (Copernicus DEM, ESA
WorldCover, Overture buildings), open weather APIs (Open-Meteo / ERA5), the
receiver-grid + isophone map output, the MapLibre/OpenStreetMap web frontend, and
future DXF/SketchUp import and 2.5D BEM barrier corrections. See `.planning/` for
the full project context and requirements.

## Licensing note

The Nord2000 method is implemented **from the published equations**, cited by
report and equation number only. No copyrighted PDF text, figures, or reference
spreadsheets are committed; the numeric class/coefficient tables are
method-defined facts.
