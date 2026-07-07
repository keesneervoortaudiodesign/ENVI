# Walking Skeleton — ENVI (Nord2000 Core Engine)

**Phase:** 1
**Generated:** 2026-07-07

> Adapted for a pure computation engine: no web UI, no database, no HTTP in Milestone 1. The "full stack" is: case file → loader → semantic scene → engine physics → complex spectrum → reference comparison → human-readable report. "Deployment" is a runnable `cargo test` plus a minimal CLI entrypoint.

## Capability Proven End-to-End

A developer runs `cargo test` (or `cargo run -p envi-harness -- report`) and sees FORCE test-case files load into the canonical scene model, the engine compute a free-field direct-path **complex** spectrum at all 105 1/12-octave points, and the harness report per-case Pass/Fail/Skipped against an analytic reference — with every not-yet-implementable FORCE road case ignored under a named list of missing capabilities.

## Architectural Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Language / numerics | Rust (stable 1.96), f64 throughout, typed errors, no panics on data | Mandated by PROJECT.md; Nord2000 numerics (Phase 3 ξ/Δτ) demand careful f64 handling |
| Workspace layout | Cargo workspace, `crates/envi-engine` (pure math, zero I/O deps) + `crates/envi-harness` (all file parsing, comparison, test execution) | The I/O quarantine makes "harness before propagation code" an architectural property; Phase 3–4 modules (`met/`, `rays/`) and v2 crates (GIS ingestion, web service) attach as siblings without moving code |
| Numeric stack | ndarray 0.17.2 + num-complex 0.4.6 + thiserror 2; approx 0.5 (dev) | Verified compatible on crates.io (ndarray declares num-complex ^0.4, approx ^0.5); all crates passed the package legitimacy audit |
| Case I/O | calamine 0.36 reads the FORCE `.xls` (BIFF) directly, label-anchored; serde+toml for synthetic cases; libtest-mimic dynamic test runner (`harness = false`) | The .xls cells are the only full-precision authoritative reference (report appendix is rounded/superseded); one Trial per case gives per-case pass/fail, filtering, CI output |
| Frequency framework | 105-point 1/12-octave grid, `f = 1000·G^(x/12)`, G = 10^0.3, x = −64..=40; every 4th point is an exact 1/3-octave centre (27-band FORCE comparison = index pick) | Deliberate, documented deviation from IEC 61260-1 even-b midbands: this is an evaluation grid, not a filter bank; nominal labels never used as float keys |
| Complex transfer contract | `H(f)` per (sub-source × receiver × freq); time convention e^{+jωt}, phase e^{−jωτ} with **τ = R/c as the carried primitive**; normalization `L_p = L_W + 20·log10\|H\|`; Phase-2+ effects multiply in as complex pressure ratios re free field | This is the load-bearing output contract (PROJECT.md); τ-as-primitive is what lets Phase 3's cancellation-safe Δτ slot in; `TransferTensor = Array3<Complex<f64>>` `[s,r,f]` row-major (freq-contiguous) is defined now so Phase 4 is a fill-in, not a refactor |
| Validation posture | Capability gating: each case declares required capabilities; unimplemented → Skipped(requires: …); Phase 1 numeric gates are analytic/ISO anchors (1e-9 dB identities, ±2 % published tables), NOT the FORCE 1 dB tolerance | FORCE road cases need the Jonasson emission model + ground effect (Phase 2/4); pretending otherwise stalls the phase (RESEARCH Pitfall 2) |
| Reference data hygiene | `refs/` git-ignored; `refs/fetch.sh` downloads the 4 FORCE workbooks + AV reports with SHA-256 pinning; `reference_version` (force-2009 / force-2010 / analytic) travels on every case | Copyrighted material never enters git; the 2010-revised spreadsheets can be swapped in later as a pure data change |
| Auth / DB / deployment | N/A — offline library + test harness; "run command" is `cargo test --workspace` and `cargo run -p envi-harness -- report` | Milestone 1 scope; SVC/WEB groups are v2 |

## Stack Touched in Phase 1

- [ ] Project scaffold — cargo workspace, two crates, pinned deps, CI-able `cargo test` (plan 01-01)
- [ ] Entry points — libtest-mimic test target `force` + CLI report bin (plan 01-01)
- [ ] One real read — FORCE `.xls` worksheets + synthetic TOML cases → `CaseDefinition` (plan 01-01)
- [ ] Canonical model — semantic 2.5D `Scene` (Source/SubSource/Receiver/Barrier/Building/TerrainProfile), projected metric CRS, Z-up; FORCE lane/height conventions (plan 01-02)
- [ ] Engine computation — divergence (Eq. 330) + ISO 9613-1 air absorption (Eq. 286/287) as `Complex<f64>` per 1/12-octave point (plan 01-03)
- [ ] One real write — per-case comparison report (per-band deviations, Pass/Fail/Skipped table) via `cargo test` and the CLI (plans 01-01 → 01-03)

## Out of Scope (Deferred to Later Slices)

- Ground effect, screen/diffraction, complex-pressure combination (Phase 2)
- Meteorology profiles, equivalent-linearization, ξ/Δτ guarded numerics, weather routes, turbulence coherence (Phase 3)
- Road emission model (Jonasson tables, 179 source points, pass-by integration), directivity balloons, the dense `H[s,r,f]` tensor store + MAC conditioning, full FORCE pass, NoiseModelling cross-validation (Phase 4)
- Any GIS ingestion, CRS transforms (proj/gdal), receiver grids, web UI/service (v2 milestones)
- Curved Road / City Street / Yearly Average workbook layouts (loaded as Skipped placeholders only)

## Subsequent Slice Plan

Each later phase multiplies new physics into the Phase 1 complex transfer and flips harness capabilities on — no structural change:

- Phase 2: ground reflection + diffraction as complex pressure ratios → ground/screen FORCE cases green
- Phase 3: refraction machinery (equivalent-linear profile, guarded ξ/Δτ, weather routes, F_τ) → refraction FORCE cases green
- Phase 4: `H[s,r,f]` tensor + directional multi-sub-source composition + MAC conditioning → full FORCE suite + NoiseModelling cross-checks
