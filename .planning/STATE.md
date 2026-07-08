---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
current_phase: 2
current_phase_name: Ground Effect & Diffraction
status: in-progress
stopped_at: Completed 02-04-PLAN.md (screen⇄ground sub-models 4/5/6 + turbulence scattering 7) — wave 3 of Phase 2
last_updated: "2026-07-08T09:23:35.912Z"
last_activity: 2026-07-08
last_activity_desc: "Completed 02-04-PLAN.md: terrain_effect::{screen, submodel7} + §5.23 geometry helpers. Generic DiffractionKernel engine realizes Sub-models 4/5/6 (four/eight-path complex image model, Eqs. 157-270) with the phase-preserving two-channel GroundResult; Sub-model 7 turbulence scattering (Tables 6/7, ×10 strengths, Eqs. 271-274) returns f64-only (phase-safe). Eq. 187-188 geometric-mean reading transcribed (Open Q5); committed scipy SM4+SM7 oracle matches case-71 ΔL₄+ΔL₇ at 105/105 ≤ 0.1 dB. ENG-07 complete. All quality gates pass."
progress:
  total_phases: 4
  completed_phases: 1
  total_plans: 5
  completed_plans: 4
  percent: 80
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-07-07)

**Core value:** A numerically faithful Nord2000 engine — validated against the FORCE road-traffic test cases — that produces correct per-band outdoor sound levels over GIS terrain.
**Current focus:** Phase 2 — Ground Effect & Diffraction

## Current Position

Phase: 2 of 4 (Ground Effect & Diffraction) — IN PROGRESS
Plan: 4 of 5 in current phase complete (02-01, 02-02, 02-03, 02-04 done; 02-05 remains)
Status: Wave 3 (screen⇄ground) complete — 02-04 delivers terrain_effect::{screen, submodel7} plus the §5.23 image-method geometry helpers. One generic DiffractionKernel engine realizes Sub-model 4 (single edge, pwedge), Sub-model 5 (thick screen, p2edge) and Sub-model 6 (two screens, eight-ray bitmask, p2wedge), all producing the phase-preserving two-channel GroundResult (complex screen factor p̂₁/p̂₀ × coherent Δp̂_G; (1−F²) residuals only in p_incoh). Sub-model 7 turbulence scattering (Tables 6/7, deliberate ×10 strengths, Eqs. 271-274) returns f64-only — structurally phase-safe. Eq. 187-188 geometric-mean reading transcribed from page images (Open Q5); a committed scipy SM4+SM7 oracle matches the case-71 ΔL₄+ΔL₇ curve at 105/105 within 0.1 dB. ENG-07 complete. Only 02-05 (§5.21 terrain interpretation + Eq. 332 composition + transfer integration) remains in Phase 2.
Last activity: 2026-07-08 — Completed 02-04-PLAN.md: terrain_effect::{screen, submodel7} + §5.23 helpers; Sub-models 4/5/6 four/eight-path complex combination; SM7 turbulence floor (f64-only, phase-safe); Eqs. 162/174-188 + Tables 6/7 transcribed; screen oracle 105/105 ≤ 0.1 dB; all quality gates pass (build/test/clippy/fmt, engine deps unchanged)

Progress: [████████░░] 80% (4/5 plans in phase 2)

## Performance Metrics

**Velocity:**

- Total plans completed: 3
- Average duration: 26min
- Total execution time: ~1.3 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1 | 3 | 77min | 26min |

**Recent Trend:**

- Last 5 plans: 25min, 17min, 35min, 55min
- Trend: —

*Updated after each plan completion*
| Phase 01 P01 | 25min | 3 tasks | 17 files |
| Phase 01 P02 | 17min | 3 tasks | 12 files |
| Phase 01 P03 | 35min | 3 tasks | 14 files |
| Phase 02 P01 | 55min | 3 tasks | 14 files |

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- Milestone 1 = validated core engine only (no map/UI/GIS ingestion); geometry fed from FORCE test-case files
- FORCE test harness (VAL-01) lands before any propagation code — Phase 1, plan 01-01
- 1/12-octave complex evaluation designed in from the first propagation phase, not retrofitted; output contract = `H[sub_source, receiver, freq]` complex tensor (Phase 4)
- Numerics guarded from the start: f64 throughout, ξ singularity clamps, cancellation-safe Δτ reformulation (Phase 3)
- [Phase 1]: Harness-before-physics enforced via capability gating: implemented_capabilities() empty, so all 66 FORCE/TOML cases report Skipped(requires: …) until later plans flip flags
- [Phase 1]: I/O quarantine: envi-engine depends only on ndarray/num-complex/thiserror; all .xls/TOML parsing lives in envi-harness
- [Phase 1, 01-02]: FORCE source line at x=2.5 m and receiver at the last profile X give horizontal distance 97.5 m (NOT 100); hSv/hRv (Z above first/last profile point) encoded solely in TerrainProfile::endpoints
- [Phase 1, 01-02]: Ground row→segment impedance rule = "row that STARTS the segment"; verified against the MIXED-impedance case 1 (road σ=20000 + grass σ=12.5) — the planned "all class A" assumption was wrong, authoritative .xls wins (Pitfall 1)
- [Phase 1, 01-02]: FORCE cases use a single placeholder SubSource (uniform-0 spectrum, height 0.0 above first point); real road sub-source heights 0.01/0.30/0.75 m are Phase 4
- [Phase 1, 01-03]: Complex transfer convention FROZEN — time e^{+jωt}, outgoing phase e^{−jωτ} with τ=R/c the carried primitive (not kR), |H|=1/√(4πR²) so L_p = L_W + 20·log10|H|; air absorption a real 10^(−ΔLₐ/20) factor; Phase 2+ effects multiply H by their complex pressure ratio
- [Phase 1, 01-03]: TransferTensor = Array3<Complex<f64>> [sub_source, receiver, freq] row-major (frequency-contiguous) — the Phase 4 forward contract; never Fortran-order
- [Phase 1, 01-03]: band_attenuation_db (Eq.287, 300 dB clamp) is the SOLE alpha·R→band converter (Pitfall 4); applied at all 105 grid points as band centres (Assumption A5, revisit Phase 4)
- [Phase 1, 01-03]: Free-field gate = strict 1e-9 dB analytic identity (harsher than FORCE 1 dB); independent dB-domain oracle (compare::analytic_freefield_reference) validates the complex roundtrip, not just the formulas
- [Phase 1, 01-03, Rule 1]: Eq.335 with coeff 20.05 gives c(15°C)=340.348 m/s (not the RESEARCH parenthetical 340.29 which uses ≈20.047); the mandated formula is the frozen phase-τ contract, so the test anchor was corrected to the formula's value
- [Phase 2, 02-01]: Nord2000-native modules quarantine the e^{−jωt} convention (special/ground/rays/coherence/terrain_effect) — the single conj() to ENVI's e^{+jωt} lands in transfer.rs in plan 02-05; no conj() in propagation modules
- [Phase 2, 02-01]: |Q̂| is NEVER clamped — |Q̂|=1.2572 at σ=200/f=250 is correct surface-wave physics, pinned by a test (anti-pattern guard)
- [Phase 2, 02-01]: Δτ via the cancellation-free identity ΔR = 4·hS·hR/(R₁+R₂) (flat) / image-point dot-product form (sloped) — the only Δτ path; hS=0.01/d=1000 regression
- [Phase 2, 02-01]: impedance_class B corrected 31.6→31.5 (Table 2 verified — resolves Phase 1 Assumption A1); all eight classes now verified
- [Phase 2, 02-01, Rule 1]: research w(7+1j) anchor (0.019924+0.139158j) is a transcription error — true wofz(7+1j)=0.011630+0.079732j (asymptote-confirmed); engine matches scipy, not the mistyped anchor
- [Phase 2, 02-01, Rule 3]: ground_impedance returns Result (σ≤0 typed error, T-02-01) and CoherenceInputs gained d_m (Fc needs distance) — both documented interface-block extensions
- [Phase 2, 02-01]: committed scipy oracle (tools/nord2000_oracle) generates ground_w_qhat.toml fixtures (sha256 provenance); Rust tests run without Python. Oracle w-tolerance 3e-6 (three-pole near-border intrinsic error ~2.5e-6, not a bug)

### Pending Todos

None yet.

### Blockers/Concerns

- ~~Must obtain AV 1106/07 and the FORCE road-traffic test-case suite before Phase 1 execution~~ RESOLVED (01-01): all 4 FORCE workbooks + AV PDFs fetched into git-ignored refs/, SHA-256 pinned in refs/refs.sha256; case "1" full-precision anchor (LAeq,24h=39.39836757521) verified. Suite stays green whether or not refs are present.
- Open question (Phase 3/4): how to represent Nord2000 partial-coherence (coefficient F_τ) alongside the coherent complex transfer — channel/representation decision pending

## Deferred Items

Items acknowledged and carried forward from previous milestone close:

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| v2 scope | DATA, GEOX, METX, GRID, WEB, SVC, FUT requirement groups | Deferred to later milestones | 2026-07-07 (roadmap) |

## Session Continuity

Last session: 2026-07-08T09:23:35.897Z
Stopped at: Completed 02-01-PLAN.md (Nord2000-native numerics core) — wave 1 of Phase 2; wave 2 (02-02, 02-03) ready to run
Resume file: None
