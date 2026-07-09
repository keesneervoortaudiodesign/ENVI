---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
current_phase: 4
current_phase_name: Transfer Tensor, Directional Sources & Full Validation
status: executing
stopped_at: Phase 5 context gathered
last_updated: "2026-07-09T12:11:37.554Z"
last_activity: 2026-07-09
last_activity_desc: "Phase 4 closed: Table A.1 integration + FORCE emission delta measured + 5 gates (REVIEW/SECURITY/VERIFICATION + simplify + doc-consistency)"
progress:
  total_phases: 11
  completed_phases: 4
  total_plans: 16
  completed_plans: 16
  percent: 36
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-07-07)

**Core value:** A numerically faithful Nord2000 engine — validated against the FORCE road-traffic test cases — that produces correct per-band outdoor sound levels over GIS terrain.
**Current focus:** Phase 4 — Transfer Tensor, Directional Sources & Full Validation

## Current Position

Phase: 4 (Transfer Tensor, Directional Sources & Full Validation) — COMPLETE
Plan: 5 of 5 complete (04-01, 04-02, 04-03, 04-04, 04-05)
Status: Ready to execute
Last activity: 2026-07-09 — Phase 4 closed: Table A.1 integration + FORCE emission delta measured + 5 gates (REVIEW/SECURITY/VERIFICATION + simplify + doc-consistency)

Progress: [██████████] Phase 4 — 5/5 plans complete (04-01 ✅, 04-02 ✅, 04-03 ✅, 04-04 ✅, 04-05 ✅)

## Performance Metrics

**Velocity:**

- Total plans completed: 12
- Average duration: ~28min
- Total execution time: ~6 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1 | 3 | 77min | 26min |

**Recent Trend:**

- Last 5 plans: 35min, 55min, 34min, 17min, 55min
- Trend: —

*Updated after each plan completion*
| Phase 01 P01 | 25min | 3 tasks | 17 files |
| Phase 01 P02 | 17min | 3 tasks | 12 files |
| Phase 01 P03 | 35min | 3 tasks | 14 files |
| Phase 02 P01 | 55min | 3 tasks | 14 files |
| Phase 3 P1 | 95min | 3 tasks | 21 files |
| Phase 3 P02 | 34min | 3 tasks | 8 files |
| Phase 4 P01 | 55min | 3 tasks | 6 files |
| Phase 04 P05 | 35min | 2 tasks | 6 files |
| Phase 04 P02 | 55min | 3 tasks | 12 files |
| Phase 04 P04 | 25min | 3 tasks | 12 files |

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
- [Phase 3, 03-03] Open-Q1 checkpoint resolved as option (c): proceed with the [ASSUMED] weather-route A/B/C constants clearly quarantined — validated by structural + direction property tests and the same-transcription committed oracle ONLY; NO false FORCE numeric Pass; wind/gradient FORCE cases stay Skipped(requires: emission-model) until Phase 4
- [Phase 3, 03-03] F_τ (Eq. 112) uses the full 2π·f·|Δτ⁺−Δτ| argument (NOT 0.23π — Pitfall 5), injected through the pre-built CoherenceInputs::f_delta_nu seam with zero call-site change; sA=sB=0 ⇒ Δτ⁺=Δτ ⇒ F_τ=1 bit-exact; multiplies H_coh without overwriting the +j phase (D-12); property-tested only (no fixed oracle, D-11)
- [Phase 3, 03-03] Route 3 LSQ = hand-rolled 3×3 normal equations (Cramer + singular guard), NO nalgebra/ndarray-linalg (D-08); SoundSpeedProfile gained s_a/s_b so the engine can form the Eq. 10 A⁺/B⁺ profile; Capability::Refraction flipped ⇒ FORCE wind/gradient skip-reason shrinks to emission-model only
- [Phase 4, 04-01] Tensor is a PAIR: H_coh Array3<Complex<f64>> + P_incoh_abs Array3<f64> [sub_source, receiver, freq] row-major; P_incoh stored ABSOLUTE (|H_ff|²·p_incoh) at fill time so F→1⇒0 bit-exact and readout needs only two stores; TensorSink trait is the Phase-10 file-backed-store seam, SolveJob the Phase-9 PropagationPath seam
- [Phase 4, 04-01] MAC ≡ recompute is bit-for-bit (assert_eq! on f64::to_bits, not epsilon): compose_gain builds G_s(f)=10^{L_W/20}·filter·e^{−j2πfτ} ONCE in a frozen order, one multiply in readout_coherent (Pitfall 6); delay phase written explicitly (no .conj()); conditioning/directivity live on the ENVI post-conj side — propagation/ conj-quarantine stays at zero actual calls
- [Phase 4, 04-01] Two readout laws kept distinct: coherent MAC (OUT-03) vs incoherent Annex-A energy Σ_s w_s·(|H_coh|²+P_incoh_abs) — two identical co-located subs give +3.0103 dB, never +6 dB; 256 MiB budget proven structurally by CountingSink high-water-mark over a 100k-receiver solve (full complex tensor never resident); ZERO new engine deps
- [Phase 4, 04-01, Rule 2/3] SolveJob carries atmosphere:&Atmosphere (direct_path needs full air-absorption state; terrain_effect takes coh.c0 as in build_terrain_inputs); compose_gain returns Result (validates filter length vs N_BANDS + rejects non-finite, threat T-04-01-03) — both documented interface deviations from the plan's literal field/signature

### Pending Todos

- **Wire directional phase into the coherent composition path (do not miss).**
  `DirectivityBalloon` now carries optional per-band phase `Δφ(θ,φ,f)`
  (`eval_phase`/`eval_complex`) and the solver applies it via
  `SolveJob::directivity_phase_rad` to `H_coh` only — an ENVI extension beyond
  stock Nord2000 (real ΔL, incoherent). **No harness `SolveJob` site populates it
  yet**; wire it when the coherent directional-source composition path lands
  (Milestone 2 Phases 10–11, SRC-03 end-to-end). Full detail + how-to in
  `.planning/phases/04-.../deferred-items.md` ("Directional phase seam").

### Blockers/Concerns

- ~~Must obtain AV 1106/07 and the FORCE road-traffic test-case suite before Phase 1 execution~~ RESOLVED (01-01): all 4 FORCE workbooks + AV PDFs fetched into git-ignored refs/, SHA-256 pinned in refs/refs.sha256; case "1" full-precision anchor (LAeq,24h=39.39836757521) verified. Suite stays green whether or not refs are present.
- ~~Open question (Phase 3/4): how to represent Nord2000 partial-coherence (coefficient F_τ) alongside the coherent complex transfer~~ RESOLVED (03-03): F_τ (Eq. 112) multiplies the coherent H_coh_factor via the CoherenceInputs::f_delta_nu seam (phase preserved, D-12); the turbulence-decorrelated energy remains the separate real P_incoh channel; F→1 ⇒ P_incoh→0 bit-exact

## Deferred Items

Items acknowledged and carried forward from previous milestone close:

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| v2 scope | DATA, GEOX, METX, GRID, WEB, SVC, FUT requirement groups | Deferred to later milestones | 2026-07-07 (roadmap) |

## Session Continuity

Last session: 2026-07-09T06:30:31.803Z
Stopped at: Phase 5 context gathered
Resume file: .planning/phases/05-engine-extensions-forest-semi-transparent-partitions/05-CONTEXT.md
