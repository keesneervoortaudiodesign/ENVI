---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
current_phase: 3
current_phase_name: Meteorology & Refraction
status: executing
stopped_at: 03-02 complete; 03-03 next
last_updated: "2026-07-08T16:34:06.472Z"
last_activity: 2026-07-08
last_activity_desc: 03-02 complete (CalcEqSSPGround soft-ground fL/fH + weather Route 2 per-azimuth A + reflection before/after split)
progress:
  total_phases: 11
  completed_phases: 2
  total_plans: 11
  completed_plans: 10
  percent: 18
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-07-07)

**Core value:** A numerically faithful Nord2000 engine — validated against the FORCE road-traffic test cases — that produces correct per-band outdoor sound levels over GIS terrain.
**Current focus:** Phase 3 — Meteorology & Refraction

## Current Position

Phase: 3 (Meteorology & Refraction) — EXECUTING
Plan: 3 of 3
Status: Ready to execute
Last activity: 2026-07-08 — 03-02 complete (CalcEqSSPGround soft-ground fL/fH + weather Route 2 per-azimuth A + reflection before/after split)

Progress: [███████░░░] Phase 3 — 2/3 plans complete (03-01 refraction core, 03-02 soft-ground + weather routes)

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
| Phase 3 P1 | 95min | 3 tasks | 21 files |
| Phase 3 P02 | 34min | 3 tasks | 8 files |

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
- [Phase ?]: [Phase 3, 03-02] Eq. 26 fL middle branch = (43−3·Δc₁₀)/40 with knots at Δc₁₀=1 and 5, resolved by C⁰-continuity (pdftotext dropped the minus sign)
- [Phase ?]: [Phase 3, 03-02] calc_eq_ssp_ground takes σ (sigma_kpa) not a precomputed Ẑ_G — phase_diff_freq owns the Delany–Bazley impedance and needs σ to sweep the 1/3-oct bracket
- [Phase ?]: [Phase 3, 03-02] Route-2 A/B scaling constants are [ASSUMED] (validated by direction property tests only, no false FORCE numeric pass); B factor C/(2·(T₀+273.15)) is the exact Coft derivative; locked pending 03-03 Open-Q1
- [Phase ?]: [Phase 3, 03-02] Nord2000 nonzero-turbulence default added as a turbulence_or_nord2000_default accessor seam, NOT wired into build_terrain_inputs, so the frozen zero-turbulence terrain oracle fixtures stay untouched

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

Last session: 2026-07-08T16:33:18.660Z
Stopped at: 03-01 complete; 03-02 next
Resume file: None
