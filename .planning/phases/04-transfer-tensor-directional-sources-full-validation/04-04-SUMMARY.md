---
phase: 04-transfer-tensor-directional-sources-full-validation
plan: 04
subsystem: engine+harness
tags: [submodel11, reflection-effect, calcfzd, facade-image-source, coordinates-loader, contour-profile, multi-class-emission, danish-lden, forest-open-q3, honest-green, oracle-fixture]

# Dependency graph
requires:
  - phase: 04-03
    provides: FORCE straight-road run_case arm + provenance==Assumed honest-green gate + Ch.6 comparator + EmissionModel flip
  - phase: 04-02
    provides: road emission model (sub-sources, pass-by, directivity balloons)
  - phase: 03-03
    provides: weather Route 1 (Met. statistics loader, energy-weighted L_den, [ASSUMED] class→(A,B))
provides:
  - Sub-model 11 (§5.20 reflection effect) kernel: L11 = 10lg(ρE) + 20lg(Srefl/SFz) + CalcFZd (Eq. 339), oracle-pinned
  - Image-source 1st/2nd-order façade reflection path builder (harness) with own-façade exclusion
  - Curved/city/yearly Coordinates-sheet loaders + contour→cut-plane TerrainProfile builder
  - Multi-lane/multi-category emission composition + source spacing + 20× city cutoff
  - Danish 12/3/9 L_den (HourScheme) — distinct from the EU 12/4/8 default (Pitfall 4)
  - Curved/city/yearly run_case arms (honest-green: Skipped on [ASSUMED] emission coefficients)
  - Capability::ReflectionEffect flipped; Open-Q3 forest decision (option b) recorded
affects: [milestone-1-acceptance, VAL-02, phase-05-forest-eng09]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "SM11 is energy-only (returns f64): a real magnitude efficiency correction on the already-complex reflection path, structurally unable to touch the phase channel (like SM7/SM10 + directivity); zero .conj() under propagation/"
    - "CalcFZd (Eq. 339) transcribed from the AV rev4 PDF page images, not the garbled pdftotext — the quartic C term is −(rS²−rR²)²"
    - "Coordinates-sheet parsing extends the label-anchored idiom with prefix section matching + multi-block (blank-separated contour) reads; never a fork of the parser"
    - "Danish L_den parameterized via HourScheme (EndDefault vs Danish 12/3/9) keeping l_den() backward-compatible"
    - "Curved/city/yearly run_case arms mirror run_force_straight_road_case: full path wired, gated Skipped on emission::coefficients::PROVENANCE == Assumed"

key-files:
  created:
    - crates/envi-engine/src/propagation/terrain_effect/submodel11.rs
    - crates/envi-harness/src/facade.rs
    - tools/nord2000_oracle/gen_submodel11_fixtures.py
    - crates/envi-harness/tests/oracle_submodel11.rs
    - crates/envi-harness/tests/fixtures/oracle/submodel11.toml
  modified:
    - crates/envi-engine/src/propagation/terrain_effect/mod.rs
    - crates/envi-harness/src/cases/xls.rs
    - crates/envi-harness/src/cases/mod.rs
    - crates/envi-harness/src/emission/mod.rs
    - crates/envi-harness/src/weather/route1.rs
    - crates/envi-harness/src/lib.rs
    - crates/envi-harness/src/capability.rs

key-decisions:
  - "OPEN-Q3 FOREST = option (b): Sub-model 10 (§5.19) is NOT pulled forward. Even with SM10 the forest cases (121-124) would still Skip on the [ASSUMED] emission coefficients, so pulling it forward buys no numeric Pass while doubling the PDF-transcription risk SM11 already carries. Cases 121-124 stay Skipped(requires: forest-scattering) — an honest, recorded accepted gap closed in Milestone-2 Phase 5 (ENG-09)."
  - "HONEST-GREEN (D-03), NON-NEGOTIABLE: the Jonasson SP 2006:12 emission coefficients are unobtainable, so curved/city/yearly overall numeric Pass is NOT achievable. The run_case arms wire the full comparison path (loaders + SM11 + multi-class emission + Danish L_den + incoherent Ch.6 readout) but stay Skipped behind the same provenance==Assumed gate as 04-03 — never a false Pass. The yearly class→(A,B) mapping stays [ASSUMED]-quarantined (03-03 posture)."
  - "SM11 is a real dB efficiency correction (Eq. 296), not a complex kernel — the honest reading of §5.20: the reflection PATH is the standard complex chain, L11 corrects its magnitude. Returns f64, phase channel untouched, zero .conj()."
  - "CalcFZd verified against the PDF page image (p. 146): C = −ℓ⁴ + 2(rS²+rR²)ℓ² − (rS²−rR²)²; the pdftotext extraction dropped the squaring."
  - "Danish L_den uses 12/3/9 (AV 1171/06 §2.2), NOT the EU 12/4/8 default (Pitfall 4); regression asserts the two schemes differ."
  - "Coordinates loaders replace the one-placeholder-per-workbook with one CaseDefinition per numeric case sheet; MAX_COORD_ROWS DoS cap + confine() path guard enforced."

requirements-completed: [VAL-02 (curved/city/yearly propagation + reflection physics wired + comparator; overall numeric Pass gated on unobtainable coefficients — accepted, honest)]

# Metrics
completed: 2026-07-09
status: complete
---

# Phase 4 Plan 4: Curved + City + Yearly FORCE (SM11 Reflection Effect + Coordinates Loaders + Multi-Class Emission + Danish L_den) Summary

**Sub-model 11 (§5.20 reflection effect) + the image-source façade-path builder, the curved/city/yearly Coordinates-sheet loaders + contour→cut-plane profile builder, multi-lane/multi-category emission, and the Danish-hours (12/3/9) L_den all land — with `Capability::ReflectionEffect` flipped and the Open-Q3 forest decision executed (option b). The full curved/city/yearly comparison path is wired, but every in-scope case stays honestly `Skipped` at run time because the Jonasson SP 2006:12 emission coefficients are unobtainable (D-03) — never a false numeric Pass. This closes VAL-02 to the honest Milestone-1 acceptance gate.**

## Accomplishments

- **Sub-model 11 (§5.20 reflection effect)** — `submodel11.rs`: the reflection-efficiency correction `L11 = 10·lg(ρE) + 20·lg(Srefl/SFz)` (Eq. 296), `ρE = 1 − α` (Eq. 297), and the `CalcFZd` Fresnel-zone half-width (Eq. 339) transcribed from the AV rev4 PDF **page images** (including the `−(rS²−rR²)²` quartic term the `pdftotext` extraction mangled). The 2 m edge-tolerance ignore rule is enforced. Energy-only (`f64`), phase channel structurally untouched, zero `.conj()`. Oracle-pinned by a committed scipy fixture (band-index, 1e-6 dB, coverage assertions).
- **Image-source façade paths** — `facade.rs`: 1st- and 2nd-order façade reflection paths built by composing `reflect_over_segment` per reflecting face, with on-segment `valid` flags and **own-façade exclusion** (the free-field SPL convention). ρE is applied at the SM11 kernel, never in geometry.
- **Coordinates-sheet loaders + contour→profile builder** — `cases/xls.rs`: `parse_curved_coordinates` (road centre line, thin/thick screens + base, terrain contour lines) and `parse_city_coordinates` (building footprints, receivers) via the label-anchored idiom (prefix section matching + multi-block contour reads); `contour_profile` interpolates a source→receiver cut-plane `TerrainProfile` from the contour lines by segment/cut intersection. Real `load_curved_road`/`load_city_street`/`load_yearly` (one case per numeric sheet) replace the three-workbook placeholder loop.
- **Multi-class emission** — `emission/mod.rs`: `TrafficClass` + `expand_traffic` compose per-category sub-sources (each at its own height, scaled by flow share — 90/10 curved, 100 % city); `source_line_offsets` for source spacing; `within_distance_cutoff` for the 20× city cutoff.
- **Danish L_den** — `weather/route1.rs`: `HourScheme` (EndDefault vs Danish 12/3/9) + `l_den_scheme`; a regression asserts the Danish split differs from the EU 12/4/8 default (Pitfall 4) and matches the hand value.
- **run_case arms + capability** — `lib.rs` `RoadGroup` dispatch (Curved/City/Yearly) wires the full comparison path but fail-softs to `Skipped` behind the `PROVENANCE == Assumed` emission gate (honest-green, mirroring 04-03). `capability.rs` adds `Capability::ReflectionEffect`, requires it for city-street, flips it, and records the Open-Q3 forest decision; shrink assertions confirm city's `reflection-effect` and curved/yearly's capability sets are now empty (forest keeps `forest-scattering`).

## Open-Q3 Forest Decision (executed: option b)

Sub-model 10 (§5.19 forest scattering) is **deliberately not implemented** this plan. Rationale: even with SM10 in hand, the forest cases (121-124) are FORCE road cases that would still `Skip` on the [ASSUMED] emission coefficients, so pulling SM10 forward buys **no numeric Pass** while doubling the PDF-transcription risk SM11 already carries. Cases 121-124 therefore stay `Skipped(requires: forest-scattering)` — an honest, recorded accepted gap to be closed in Milestone-2 Phase 5 (ENG-09). The `Capability::ForestScattering` gate + the `forest_cases_retain_forest_scattering_in_the_skip_reason` test keep this honest.

## Which cases compute vs stay Skipped (and why)

| Group | Status | Why |
|-------|--------|-----|
| curved_road (30 sheets: 8 positions × main/L/H) | Skipped | full path wired (contour cut-planes, 90/10 multi-class emission, SM1/2/3 + refraction); overall numeric Pass gated on unobtainable Jonasson coefficients (D-03) |
| city_street (4) | Skipped | full path wired (image-source 1st/2nd-order façade reflections + SM11 ρE 1.0/0.7, incoherent readout); same emission gate |
| yearly_average (4) | Skipped | full path wired (per-class runs → Danish 12/3/9 L_den; class→(A,B) [ASSUMED]); same emission gate |
| straight_road 121-124 (forest) | Skipped | `requires: forest-scattering` — Open-Q3 option (b) accepted gap |
| all other straight_road / synthetic terrain / geometry / free-field | Pass / Skipped-in-crate | unchanged from 04-03; propagation validated in-crate by oracle/anchor/property tests |

No case is a false Pass. The FORCE Skip list is honest and shrinking (`emission-model` and `reflection-effect` no longer appear as capability gaps for in-scope groups; only the unobtainable-coefficient run-time Skip and the forest gap remain).

## Deviations from Plan

### Auto-fixed / adjusted (Rules 1-3)

**1. [Rule 3 - Spec reading] SM11 kernel returns `f64`, not a complex `GroundResult`.**
- **Found during:** Task 2.
- **Issue:** the plan sketched SM11 as a complex `screen.rs`-shaped kernel, but §5.20 Eq. 296 defines `L11` as a real dB efficiency correction (`10·lg ρE + 20·lg Srefl/SFz`) applied on top of the standard complex reflection path.
- **Fix:** implemented the honest spec reading — a real correction (energy-only, phase-safe, zero `.conj()`), documented at the module head. The reflection PATH remains the standard complex chain.

**2. [Rule 3 - Parser robustness] Prefix section matching + multi-block contour reads.**
- **Found during:** Task 1 (refs-gated tests).
- **Issue:** curved/city section headers are truncated/annotated ("Road centreline coordinates", "Freq. (Hz)"), city X/Y headers sit above the label, and contour lines are blank-separated within one section.
- **Fix:** section matching by case-insensitive prefix over cols A-C; data-start clamped to the section label row; a `multi_block` mode skips internal blank rows for the terrain section. Extends the idiom, no parser fork.

### Milestone-gate reframing (honest-green override)

The plan's must-have "all in-scope curved/city/yearly FORCE cases report numeric Pass" is **not achievable** and was **not faked**: per the non-negotiable D-03 house rule the Jonasson SP 2006:12 coefficients are unobtainable, so the full comparison path is wired and the numeric-Pass entry point exists, but every FORCE road case stays `Skipped` behind the `PROVENANCE == Assumed` gate. The Milestone-1 acceptance gate is therefore met in its honest form: the whole in-scope suite is wired and validated in-crate, with a documented, shrinking Skip list and zero false Pass.

## Task Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | 8ceb75c | Coordinates loaders + contour→profile builder + multi-class emission + Danish L_den |
| 2 | 74b801d | Sub-model 11 kernel + scipy oracle + image-source façade paths (+ Open-Q3 = b) |
| 3 | 67707df | curved/city/yearly run_case arms + reflection-effect capability + shrink assertions |

## Gate Results

- `cargo test --workspace`: **346 passed, 0 failed** (includes `tensor_budget` ~74 s and the new `oracle_submodel11`, SM11 unit, facade, multi-class emission, contour, Danish-L_den, and shrink tests).
- `cargo clippy --all-targets -- -D warnings`: **clean**.
- `cargo fmt --check`: **clean**.
- `.conj()` method-call gate over `crates/envi-engine/src/propagation/` = **0** (the plan's `grep 'conj()'` also matches doc-comment mentions, as in all prior phases; the real house-rule gate is `\.conj()` = 0).
- `cargo tree -p envi-engine -e normal --depth 1`: **unchanged** (ndarray + num-complex + thiserror only) — no new engine dependency.
- `cargo run -p envi-harness -- report`: runs; shows an honest, shrinking Skip list (curved/city/yearly Skipped on [ASSUMED] emission coefficients; forest Skipped on forest-scattering) with **no false Pass**.

## TDD Gate Compliance

Plan type `tdd`. Each task landed with its tests co-located and green (the repo's established `#[cfg(test)]` + committed-oracle idiom): Task 1 loader/contour/emission/L_den tests, Task 2 SM11 unit + committed scipy oracle + facade tests, Task 3 capability shrink assertions. Commits are `feat(04-04)` per the project convention.

## Self-Check: PASSED
