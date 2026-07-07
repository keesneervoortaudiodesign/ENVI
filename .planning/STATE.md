---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
current_phase: 1
current_phase_name: FORCE Harness, Geometry Model & Direct Path
status: phase-complete
stopped_at: Completed 01-03-PLAN.md (direct path at 1/12-octave complex resolution) — Phase 1 complete
last_updated: "2026-07-07T17:45:00.000Z"
last_activity: 2026-07-07
last_activity_desc: Completed 01-03 — divergence + ISO 9613-1 air absorption + complex TransferSpectrum; ENG-01/ENG-04/SRC-01 done; FreeField capability green; walking skeleton complete
progress:
  total_phases: 4
  completed_phases: 1
  total_plans: 3
  completed_plans: 3
  percent: 25
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-07-07)

**Core value:** A numerically faithful Nord2000 engine — validated against the FORCE road-traffic test cases — that produces correct per-band outdoor sound levels over GIS terrain.
**Current focus:** Phase 1 — FORCE Harness, Geometry Model & Direct Path

## Current Position

Phase: 1 of 4 (FORCE Harness, Geometry Model & Direct Path) — COMPLETE
Plan: 3 of 3 in current phase (01-01, 01-02, 01-03 all complete)
Status: Phase 1 complete — walking skeleton stands end-to-end (case file → Scene → engine complex spectrum → dB-domain comparison → report). Next: Phase 2 (ground + diffraction)
Last activity: 2026-07-07 — Completed 01-03-PLAN.md: geometrical divergence (Eq.330) + ISO 9613-1 air absorption (Eq.286) + Nord2000 band correction (Eq.287) as complex values per 1/12-octave point; FreeField capability green; ENG-01/ENG-04/SRC-01 done; all quality gates pass

Progress: [██████████] 100% (3/3 plans in phase 1)

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

- Last 5 plans: 25min, 17min, 35min
- Trend: —

*Updated after each plan completion*
| Phase 01 P01 | 25min | 3 tasks | 17 files |
| Phase 01 P02 | 17min | 3 tasks | 12 files |
| Phase 01 P03 | 35min | 3 tasks | 14 files |

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

Last session: 2026-07-07T17:45:00.000Z
Stopped at: Completed 01-03-PLAN.md (direct path at 1/12-octave complex resolution) — Phase 1 complete
Resume file: None
