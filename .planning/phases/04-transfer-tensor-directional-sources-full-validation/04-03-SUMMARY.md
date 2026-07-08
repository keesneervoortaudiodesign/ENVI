---
phase: 04-transfer-tensor-directional-sources-full-validation
plan: 03
subsystem: engine+harness
tags: [submodel3, non-flat-terrain, segmented-refraction, screen-guard, sm8-eq279, ch6-comparator, laeq-24h, emission-model, honest-green, oracle-fixture]

# Dependency graph
requires:
  - phase: 04-01
    provides: tensor + solver + incoherent Annex-A readout
  - phase: 04-02
    provides: road emission model (sub-sources, pass-by, directivity, LE−dL anchor)
  - phase: 03-03
    provides: calc_eq_ssp_ground per-band collapse (oracle-pinned) + refraction capability
provides:
  - Sub-model 3 (§5.12 non-flat terrain) two-channel GroundResult behind the former NonFlatTerrainNotImplemented seam, bit-identical to SM1 on flat profiles
  - Segmented-ground refraction wired (SM2 consumes calc_eq_ssp_ground per band)
  - Screen+weather typed guard (never a silently-unrefracted screen — Pitfall 9)
  - SM8 Eq.279 ray-count decision (accepted-gap: no in-scope FORCE geometry demands N>1)
  - Ch.6 comparator wiring: LAE / LAeq,24h / LAmax conversions over exact 1/3-oct centres
  - Capability::EmissionModel flipped + Capability::ForestScattering added (honest forest Skip)
  - FORCE straight-road run_case arm (honest-green: Skipped on [ASSUMED] emission coefficients)
affects: [phase-04-04, milestone-1-acceptance, VAL-02]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Non-flat terrain (SM3) mirrors submodel2 module shape: plain-data geometry struct in, two-channel GroundResult out, per frequency"
    - "Provenance gate for honest-green: run_force_straight_road_case Skips while emission::coefficients::PROVENANCE == Assumed — the numeric-Pass entry point is wired but gated on verified coefficients"
    - "Capability shrink test pattern extended: in-scope straight-road cases now have an EMPTY missing-capability set; forest cases retain forest-scattering"

key-files:
  created:
    - crates/envi-engine/src/propagation/terrain_effect/submodel3.rs
    - tools/nord2000_oracle/gen_submodel3_fixtures.py
    - crates/envi-harness/tests/oracle_submodel3.rs
    - crates/envi-harness/tests/fixtures/oracle/submodel3.toml
  modified:
    - crates/envi-engine/src/propagation/terrain_effect/mod.rs
    - crates/envi-engine/src/propagation/screen.rs
    - crates/envi-engine/src/propagation/mod.rs
    - crates/envi-harness/src/compare.rs
    - crates/envi-harness/src/capability.rs
    - crates/envi-harness/src/lib.rs

key-decisions:
  - "SM3 reproduces SM1 bit-for-bit on an actually-flat profile (assert_eq!) — new physics never perturbs frozen Phase 1-3 validation"
  - "Segmented-ground refraction reuses the already-oracle-pinned calc_eq_ssp_ground collapse per band inside eval (no new oracle needed; property + bit-identical-homogeneous regression only)"
  - "Screen+weather raises a typed error rather than silently computing an unrefracted screen (Pitfall 9) — deferred screen-refraction stays honest"
  - "SM8 (Eq.279) accepted-gap: no in-scope FORCE downwind geometry demands N>1, so SM8 is left unimplemented with a recorded note (Assumption A6)"
  - "HONEST-GREEN (D-03): the Jonasson SP 2006:12 rolling/propulsion emission coefficients are unobtainable, so the FORCE straight-road overall LAeq,24h numeric Pass is NOT achievable. The run arm stays Skipped with a shrunken reason (capability gate passes; the provenance==Assumed gate holds), never a false Pass. Propagation (SM1/2/3 + refraction) + the Ch.6 comparator are validated in-crate by oracle/anchor/property tests."

requirements-completed: [VAL-02 (straight-road propagation + comparator; overall numeric Pass gated on unobtainable coefficients)]

# Metrics
completed: 2026-07-09
status: complete
---

# Phase 4 Plan 3: Straight-Road FORCE Pass (SM3 + Refraction Wiring + Ch.6 Comparator + Emission Flip) Summary

**Sub-model 3 non-flat terrain, segmented-ground refraction wiring, the screen+weather guard, the SM8 Eq.279 decision, and the Ch.6 straight-road comparator + LAE/LAeq,24h/LAmax conversions land — with `Capability::EmissionModel` flipped. The in-scope straight-road FORCE cases clear the capability gate but stay honestly `Skipped` at run time because the Jonasson SP 2006:12 emission coefficients are unobtainable (D-03) — never a false numeric Pass.**

## Accomplishments

- **Sub-model 3 (§5.12 non-flat terrain)** — `submodel3.rs` computes a two-channel `GroundResult` behind the former `NonFlatTerrainNotImplemented` seam; bit-identical to SM1 on a flat profile, oracle-pinned (concave/convex/transition) against a committed scipy fixture, finite across all 105 bands on previously-erroring profiles.
- **Segmented-ground refraction** — the `SegmentedRefractionNotImplemented` seam is replaced by the already-implemented, oracle-tested `calc_eq_ssp_ground` per-band collapse inside `eval`; homogeneous limit reproduces the Phase-2 SM2 result bit-for-bit.
- **Screen+weather guard** — a screen case with a `SoundSpeedProfile` returns a refracted result or a typed error, never a silently-unrefracted screen (Pitfall 9).
- **SM8 Eq.279 decision** — evaluated over the FORCE downwind geometries; no case demands N>1, so SM8 stays unimplemented as a recorded accepted-gap (Assumption A6).
- **Ch.6 comparator wiring** — `l_ae`, `l_aeq_24h`, `l_amax` over the exact 1/3-octave centres (never nominal Hz), reusing the existing dip-shift `compare_27_band` and `a_weighting_db`. Case-1 anchor: `LAeq,24h = LAE + 10·lg N − 10·lg 86400`.
- **Capabilities** — `EmissionModel` flipped; `ForestScattering` added so forest cases (121–124) keep an honest `Skipped(requires: forest-scattering)`. Shrink assertions updated: in-scope straight-road cases now have an empty missing-capability set.

## Task Commits

1. **Task 1: Sub-model 3 + scipy oracle + flat bit-identical regression** — `4576303`
2. **Task 2: segmented-ground refraction + screen guard + SM8 Eq.279 decision** — `80ef854`
3. **Task 3: Ch.6 comparator + EmissionModel flip + straight-road run arm** — `066f373`

## Honest-green: FORCE numeric Pass is blocked on unobtainable coefficients

The road emission model is fully wired (04-02/04-03: sub-source split, pass-by integration, directivity, the full SM1/2/3 + refraction chain), and the free-field `LE − dL` shape is anchored. But the **absolute** rolling/propulsion sound-power coefficients (Jonasson **SP 2006:12**) could not be obtained (paywalled/copyrighted). An overall `LAeq,24h`/`LAE`/`LAmax` numeric Pass therefore depends on unobtainable data, so `run_force_straight_road_case` stays `Skipped` behind a `PROVENANCE == Assumed` gate — the capability gate passes, the reason list has shrunk, but no false Pass is emitted (D-03). The propagation physics it exercises is validated in-crate by the oracle/anchor/property tests.

## Test / clippy / fmt status

- `cargo test --workspace`: green (engine 195, harness 90 + all oracle/integration suites incl. `oracle_submodel3`).
- `cargo clippy --all-targets -- -D warnings`: clean. `cargo fmt --check`: clean.
- conj grep over `crates/envi-engine/src/propagation/`: 0. `cargo tree -p envi-engine`: unchanged.

## Deviations from Plan

- **Numeric Pass → honest Skip (D-03).** The plan's success criterion "in-scope straight-road cases report numeric Pass" is superseded by the honest-green directive in the same plan ("NEVER fake a numeric Pass"): the coefficients are unobtainable, so the cases stay `Skipped` with a shrunken reason. The comparator, conversions, and run arm are all wired and unit-tested; the numeric-Pass entry point is reached only once verified coefficients are in hand.

## Next Phase Readiness

- 04-04 (curved/city/yearly + SM11 + forest decision) can build on the comparator, emission, and readout wired here. All four FORCE workbooks are present in `refs/`; Python+scipy are available for the SM11 oracle.
- Milestone-1 acceptance (VAL-02 full FORCE numeric Pass) remains gated on the SP 2006:12 coefficients — an external, non-code blocker.

---
*Phase: 04-transfer-tensor-directional-sources-full-validation*
*Completed: 2026-07-09*
