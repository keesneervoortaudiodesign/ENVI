# Phase 4: Transfer Tensor, Directional Sources & Full Validation - Pattern Map

**Mapped:** 2026-07-08
**Files analyzed:** 18 new/modified files (from 04-RESEARCH plan split 04-01…04-05)
**Analogs found:** 15 / 18 (3 genuinely new — sketches in 04-RESEARCH)

All paths relative to repo root `D:\====CLAUDE\envi`. Binding conventions verified against the current tree: engine deps = `ndarray + num-complex + thiserror` only (`crates/envi-engine/Cargo.toml:10-13`); zero `.conj()` in `propagation/` (the one boundary is `transfer.rs:90-92`); band-index comparison everywhere; frozen tensor order `[sub_source, receiver, freq]` (`transfer.rs:42-49`); typed errors, never panics.

## Two pre-built discoveries (planner: extend, don't rebuild)

1. **The Ch.6 comparator largely EXISTS.** `crates/envi-harness/src/compare.rs` already ships `compare_27_band` with the EP1335 §6 dip-shift rule (`compare.rs:156-222`, dip-shift logic 171-191), the 1 dB overall/band tolerances + 0.5 dB investigation flag (`compare.rs:18-28`), AND an anchored IEC 61672 `a_weighting_db` (`compare.rs:340-347`, anchors tested 378-382). 04-RESEARCH's "Wave 0: Ch.6 comparator + A-weighting" reduces to: LAE/LAeq,24h/LAmax conversions + wiring, not a new comparator.
2. **Segmented-refraction building block EXISTS and is oracle-pinned.** `calc_eq_ssp_ground` (§5.5.3) is implemented (`crates/envi-engine/src/propagation/refraction/eqssp.rs`) and covered by committed fixtures (`crates/envi-harness/tests/oracle_refraction.rs:137-196`, band-index rows in `tests/fixtures/oracle/refraction.toml`). 04-03's wiring task only replaces the typed error at `terrain_effect/mod.rs:393-405`.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/envi-engine/src/tensor.rs` (TensorPair store + readout laws) | model/store | batch transform | `crates/envi-engine/src/transfer.rs` | exact (it is the frozen contract's home) |
| `crates/envi-engine/src/tensor.rs` (TensorSink trait + InMemorySink) | trait seam | streaming | none — sketch in 04-RESEARCH Pattern 1 | none (see No Analog) |
| `crates/envi-engine/src/solver.rs` (SolveJob + solve loop) | service | batch request-response | `terrain_effect()` in `propagation/terrain_effect/mod.rs` + `run_freefield_case` chain in `crates/envi-harness/src/lib.rs` | role-match |
| `crates/envi-engine/src/directivity.rs` (DirectivityBalloon) | model | transform | `scene.rs` `BandSpectrum` + `freq.rs` `FreqAxis` (fixed-grid vocabulary types) | role-match |
| `crates/envi-engine/src/propagation/terrain_effect/submodel3.rs` | propagation kernel | transform | `terrain_effect/submodel2.rs` | exact |
| `crates/envi-engine/src/propagation/terrain_effect/submodel{8,10,11}.rs` | propagation kernel | transform | `terrain_effect/submodel7.rs` (energy-only) / `screen.rs` `submodel4` (complex) | role-match |
| segmented+screen refraction wiring (edit `terrain_effect/mod.rs`, `screen.rs`) | dispatch | transform | `FlatChannel` refraction dispatch, `terrain_effect/mod.rs:302-327, 344-372` | exact |
| `crates/envi-engine/src/freq.rs` A-weighting (if moved engine-side) | utility | transform | `crates/envi-harness/src/compare.rs:340-347` `a_weighting_db` (already exists) | exact (relocate/reuse) |
| `crates/envi-harness/src/emission/mod.rs` (RoadSource → SubSources) | service/builder | transform | `crates/envi-harness/src/scene_build.rs` | exact |
| `crates/envi-harness/src/emission/coefficients.rs` (Jonasson tables) | config/data | — | `scene.rs:329-341` `impedance_class` (provenance-commented constant table) | role-match |
| `crates/envi-harness/src/emission/passby.rs` (LE/LAeq/LAmax integration) | service | batch | `weather/route1.rs:173` `l_den` (energy-weighted dB sum) | partial |
| `crates/envi-harness/src/cases/xls.rs` curved/city/yearly parsers | loader | file-I/O | `cases/xls.rs:150-307` `parse_sheet` + `weather/route1.rs:230` `load_met_probabilities` | exact |
| `crates/envi-harness/src/cases/mod.rs` discovery + placeholder removal | loader | file-I/O | `cases/mod.rs:474-572` `discover` | exact |
| `crates/envi-harness/src/capability.rs` EmissionModel flip | config | — | `capability.rs:120-128` + shrink tests 230-262 | exact |
| `crates/envi-harness/src/lib.rs` FORCE `run_case` arm | controller/dispatch | request-response | `lib.rs:287-324` `run_terrain_case` | exact |
| `crates/envi-harness/src/compare.rs` LAE/LAeq/LAmax + Ch.6 wiring | comparator | transform | `compare.rs:156-222` `compare_27_band` | exact |
| `crates/envi-harness/tests/oracle_noisemodelling.rs` + fixture TOML | test | file-I/O | `tests/oracle_refraction.rs` | exact |
| `tools/noisemodelling_oracle/` + `tools/nord2000_oracle/gen_submodel*_fixtures.py` | tooling | file-I/O | `tools/nord2000_oracle/gen_ground_fixtures.py` + `common.py` | exact |
| `tests/tensor_budget.rs` / `emission_anchor.rs` (integration tests) | test | batch | `tests/oracle_refraction.rs` (structure) + `cases/xls.rs:382-425` (refs-gated skip) | role-match |
| `refs/fetch.sh` + `refs/refs.sha256` (add User's Guide AV 1171/06) | config | file-I/O | existing `ARTIFACTS` list in `refs/fetch.sh` (~lines 29-37) | exact |
| SM11 image-source façade paths (harness path builder) | geometry builder | transform | `envi_engine::geometry::reflect_over_segment` (used in `lib.rs:132-165` `run_geometry_case`) | partial |

## Pattern Assignments

### `crates/envi-engine/src/tensor.rs` — TensorPair + readout laws (04-01, OUT-01/02/03)

**Analog:** `crates/envi-engine/src/transfer.rs` (217 lines — read it whole before writing tensor.rs)

**The frozen typedef and layout test to build on** (`transfer.rs:42-49`, test at 207-217):
```rust
/// Shape is `[sub_source, receiver, freq]` in ndarray's default **row-major**
/// (C) order, so the **frequency axis is contiguous** on the last index …
pub type TransferTensor = Array3<Complex<f64>>;
// test: TransferTensor::zeros((2, 3, N_BANDS)); assert!(t.is_standard_layout(), …)
```
The tensor store is a **pair**: `TransferTensor` + `Array3<f64>` for `P_incoh` (store the absolute form `|H_ff|²·p_incoh` at fill time per 04-RESEARCH Pattern 4, so `F→1 ⇒ 0` stays bit-exact and readout needs only two stores).

**The single-source MAC seed to generalize** (`transfer.rs:61-71`) — `band_levels_db` is documented as "the single-source seed of the Phase 4 MAC (OUT-03)":
```rust
pub fn band_levels_db(h: &TransferSpectrum, spectrum: &BandSpectrum) -> Vec<f64> {
    debug_assert_eq!(h.len(), N_BANDS, "transfer spectrum must be N_BANDS long");
    h.iter().zip(spectrum.as_slice())
        .map(|(&hf, &lw_db)| {
            let g = 10f64.powf(lw_db / 20.0); // |G_s| = 10^(L_W/20), phase 0
            let p = hf * g;                    // p = H·G_s
            20.0 * p.norm().log10()
        }).collect()
}
```
Generalize to `p[r,f] = Σ_s H[s,r,f]·G_s(f)` with G_s pre-composed ONCE (`10^{L_W/20} · Ĝ_filter(f) · e^{−j2πfτ_s}`, exact `FREQ_AXIS.centres` values) so MAC ≡ recompute is `assert_eq!` on bits (04-RESEARCH Pattern 5 / Pitfall 6).

**The two-channel incoherent readout law to lift verbatim** (`transfer.rs:115-135`):
```rust
.map(|(((&hc, &hf), &pi), &lw_db)| {
    // Total energy = coherent |H_coh|² + incoherent |H_ff|²·p_incoh.
    let energy = hc.norm_sqr() + hf.norm_sqr() * pi;
    lw_db + 10.0 * energy.log10()
})
```
The tensor's Annex-A incoherent readout (roads/FORCE) is this per (s,r) with `|G_s|²` weights summed over s. Test seeds: `transfer.rs:174-204` (zero-p_incoh identity; H_coh=0 pure-incoherent anchor) — replicate both at tensor rank.

**Conditioning placement (hard rule):** G_s multiplies ENVI-convention `H_coh` AFTER `nord_ratio_to_transfer` (`transfer.rs:90-92` — THE one `.conj()`); never inside `propagation/`. The fill formula analog is `terrain_effect/mod.rs` test helper `h_coh` (lines 561-566): `hff[i] * nord_ratio_to_transfer(te.h_coh_factor[i])`.

---

### `crates/envi-engine/src/solver.rs` — SolveJob + solve loop (04-01, OUT-01/06)

**Analog:** the per-pair forward chain as invoked from the harness (`crates/envi-harness/src/lib.rs:181-235` `run_freefield_case` + `lib.rs:287-324` `run_terrain_case`) and the engine entry `terrain_effect()` (`propagation/terrain_effect/mod.rs:152-210`).

**The chain the solver runs per (s, r)** — copy the exact assembly from `terrain_effect/mod.rs` tests 555-566 + `lib.rs:214-227`:
```rust
// 1. PathGeometry::direct(src, rcv)  → direct_path(&path, &atmos, axis) → H_ff  (propagation/mod.rs:178-204)
// 2. terrain_effect(profile, src, rcv, c0, &coh, axis, weather) → { h_coh_factor, p_incoh, delta_l_db }
// 3. H_coh[s,r,f] = H_ff[f] * nord_ratio_to_transfer(h_coh_factor[f]) * 10^{ΔL_balloon/20}
// 4. P_incoh_abs[s,r,f] = |H_ff[f]|² * p_incoh[f] * 10^{ΔL_balloon/10}
```

**`SolveJob` field vocabulary** — mirror the existing signature of `terrain_effect` (`mod.rs:152-160`): `&TerrainProfile`, `[f64;3]` src/rcv, `atmos_c0: f64`, `&CoherenceInputs` (`coherence.rs:39-56`), `&FreqAxis`, `Option<&SoundSpeedProfile>`. The struct sketch is 04-RESEARCH Pattern 3; it is the Phase-9 `PropagationPath` seam — name deliberately.

**Precompute-once pattern for band-independent geometry** — copy `FlatChannel::from_profile` / `RefractionState` (`terrain_effect/mod.rs:216-338`): CalcEqSSP + `circular_rays` are computed ONCE per pair, then `eval(f, coh)` loops the 105 bands reading the stored state. The solver's per-pair loop should follow the same shape (geometry per pair, band loop innermost = frequency-contiguous writes).

**Error handling:** extend `PropagationError` (`propagation/mod.rs:50-148`) with any new solver/sink variants using `thiserror` struct-variant style with a `detail`/context field — see `DegenerateRayGeometry` (mod.rs:87-91) and the exemplary not-implemented variants `NonFlatTerrainNotImplemented` (98-107) / `SegmentedRefractionNotImplemented` (138-147), whose doc comments explain WHY hard-error-by-design.

---

### `crates/envi-engine/src/directivity.rs` — DirectivityBalloon (04-02, SRC-02/04)

**Analog (type discipline):** `crates/envi-engine/src/scene.rs:97-123` `BandSpectrum` — a fixed-length wrapper so a spectrum "can never be the wrong length on the engine's frequency grid":
```rust
pub struct BandSpectrum { values_db: [f64; N_BANDS] }   // private field + validated ctors + as_slice()
```
Give the balloon the same posture: private `Array3<f64>` grid `[az, pol, band]` with the band axis locked to `N_BANDS`, a validating constructor returning a typed error (model on `TerrainProfile::new`, `scene.rs:209-265`: check finiteness, grid monotonicity, dimensions — never panic on data), and doc-comment the angular convention up top exactly like scene.rs documents its coordinate convention (`scene.rs:1-25`).

**Rotation:** hand-rolled 3×3, no linalg crate — precedent D-08 is the Cramer solve in `crates/envi-harness/src/weather/route3.rs` (module doc `route3.rs:11` "no linalg crate (D-08)"). Engine-local, applied to the src→rcv unit vector before frame lookup.

**Band axis:** balloons are defined on `FREQ_AXIS` (`freq.rs:106`); index by `BandIdx`/usize, never by nominal Hz (`freq.rs:26-31` pitfall note). No per-balloon frequency resampling in the engine.

**Application point:** real per-band factor `10^{ΔL/20}` on `H_coh` and `10^{ΔL/10}` on `P_incoh_abs` at tensor-fill time (solver step 3/4 above) — magnitude only, phase untouched, mirroring how Sub-model 7 is "structurally incapable of corrupting the coherent phase channel" by being typed `f64` (`terrain_effect/mod.rs:33-41`).

---

### `crates/envi-engine/src/propagation/terrain_effect/submodel3.rs` — non-flat terrain (04-03)

**Analog:** `terrain_effect/submodel2.rs` — the existing segment-wise sub-model.

**Module shape to copy** (`submodel2.rs:49-88`): a plain-data geometry struct + strip slice in, `GroundResult` out, per frequency:
```rust
pub struct FlatGeometry { pub d: f64, pub h_s: f64, pub h_r: f64, pub c0: f64 }
pub struct SurfaceStrip { pub x_start: f64, pub x_end: f64, pub sigma_kpa: f64, pub roughness_r: f64 }
pub fn submodel2(f_hz: f64, strips: &[SurfaceStrip], geom: &FlatGeometry, coh: &CoherenceInputs)
    -> Result<GroundResult, PropagationError>
```
SM3 (§5.12 Eqs. 134-156: segment classification + per-segment terrain effect + Fresnel-weight sum) returns the same two-channel `GroundResult` (`terrain_effect/mod.rs:69-91` — `from_channels` derives `delta_l_db = 10·lg(|h_coh|² + p_incoh)`).

**Wiring point (the seam it replaces):** `terrain_effect/mod.rs:174-182` — the block that currently raises `NonFlatTerrainNotImplemented` when `r_flat < 1` with weight becomes the SM3 branch of the Eq. 332 blend (r-weight blending pattern at `mod.rs:191-202`: channels blend linearly, `delta_l_db` blends in dB — keep both readings).

**Verification pattern:** new scipy oracle `gen_submodel3_fixtures.py` (see tooling section) + an in-module `#[cfg(test)]` block following `terrain_effect/mod.rs:499-757` (flat-profile regression: SM3 on an actually-flat profile must reproduce SM1 bit-for-bit, mirroring `refraction_homogeneous_profile_is_bit_identical` at `mod.rs:784-805`).

### `submodel8.rs` / `submodel10.rs` / `submodel11.rs`

- **SM8/SM10 (energy-side effects):** copy `submodel7.rs` — energy-only sub-models return `f64` dB so they can never touch the phase channel (contract at `terrain_effect/mod.rs:33-41`); combine like `submodel7::combine_scatter` / `screen_channel`'s `base.p_incoh + sm7_energy` (`mod.rs:490-496`). SM10's `Fs` coherence reduction follows the `CoherenceInputs.f_delta_nu` injection pattern (`mod.rs:356-363`: incoming factor is *multiplied, never overwritten*).
- **SM11 (reflection effect, complex):** copy the `screen.rs` kernel shape — `ScreenConfig<'a>` config struct (`screen.rs:77`) + `pub fn submodel4(f_hz, cfg) -> Result<GroundResult, PropagationError>` (`screen.rs:613`). The façade-reflection *path construction* stays in the harness (see SM11 path builder below).

### Segmented-ground + screen refraction wiring (04-03, edits to existing files)

**Analog:** the already-landed Sub-model-1 refraction dispatch — copy it for the SM2 and screen channels.
- The dispatch to replicate: `terrain_effect/mod.rs:302-327` (`RefractionState` precompute: `calc_eq_ssp` → `circular_rays` once, plus the A⁺/B⁺ fluctuation profile for FΔν) and `mod.rs:344-372` (per-band eval consuming the stored state, shadow-zone branch at 350-356).
- The typed error to remove: `mod.rs:393-405` (`SegmentedRefractionNotImplemented`) — replace with a frequency-dependent `calc_eq_ssp_ground` collapse (already implemented + oracle-tested; signature visible in `tests/oracle_refraction.rs:148-160`: `calc_eq_ssp_ground(f, d, h_s, h_r, sigma_kpa, z0, a, b, c)` — note it is per-band, so it lives inside `eval`, not `from_profile`).
- Screen channel: `screen.rs:37` marks the ξ<0 shadow branches (Eqs. 184-186) as the Phase-3-deferred gap; thread `Option<&SoundSpeedProfile>` through `screen_channel` (`mod.rs:421-497`) the same way `FlatChannel` carries it. **Pitfall 9 guard:** until wired, raise a new typed error for weather+screen mirroring `SegmentedRefractionNotImplemented` (`propagation/mod.rs:130-147` shows the doc-comment style: explain the honest-green rationale in the variant docs).
- Regression tests to clone: `mod.rs:784-805` (homogeneous profile bit-identical), `mod.rs:844-875` (typed-error-not-silent), `mod.rs:974-998` (finiteness sweep over all bands × profiles).

---

### `crates/envi-harness/src/emission/mod.rs` — RoadSource → SubSources + balloons (04-02)

**Analog:** `crates/envi-harness/src/scene_build.rs` — the existing CaseDefinition → engine-types trust boundary, and the file that documents the exact placeholders 04-02 supersedes.

**The placeholders to replace (explicitly documented for Phase 4):** `scene_build.rs:45-59`:
```rust
const FORCE_LANE_X_M: f64 = 2.5;              // superseded: sub-sources sit 1 m toward the
                                              // receiver ⇒ x = 3.5 m (04-RESEARCH Pitfall 1)
const FORCE_PLACEHOLDER_SOURCE_H_M: f64 = 0.0; // superseded: 0.01 / 0.30 / 0.75 m heights
```
And the single placeholder `SubSource` construction (`scene_build.rs:121-135`): `Source { sub_sources: vec![SubSource { position, spectrum }] }` becomes the 179-point × height expansion producing `Vec<SubSource>` (engine type `scene.rs:130-136`) + balloons + per-sub-source `G_s(f)` weights.

**hSv/hRv convention:** keep going through `TerrainProfile::endpoints` (`scene.rs:288-297`) — the single place the off-by-metres trap lives; `scene_build.rs:113-119` shows the correct call shape.

**Module docs:** copy scene_build's style of documenting each convention/trap with the research citation inline (`scene_build.rs:1-16`), and the test anchoring conventions against the live workbook with the refs-gated skip (`scene_build.rs:182-233`).

### `crates/envi-harness/src/emission/coefficients.rs` — Jonasson tables

**Analog:** `crates/envi-engine/src/scene.rs:317-341` `impedance_class` — the house pattern for transcribed constant tables:
```rust
/// # Provenance
/// **All eight classes VERIFIED** against AV 1106/07 Table 2 (02-RESEARCH §2,
/// this phase — resolves Phase 1 Assumption A1). Class **B is 31.5**, not the
/// 31.6 Phase 1 assumed; corrected here.
pub fn impedance_class(class: char) -> Option<f64> { match class { 'A' => Some(12.5), … } }
```
Every coefficient carries a provenance comment (`[CITED: SP 2006:12 Table X]` or `[ASSUMED: LE−dL fit, see …]`). The `[ASSUMED]`-quarantine precedent is `cases/mod.rs:233-242` (`NORD2000_DEFAULT_CV2/CT2` — assumed values documented, validated by property tests, "never a fixed-value oracle"). The SP 2006:12 human checkpoint gates this file (04-RESEARCH Open Q1).

### `crates/envi-harness/src/emission/passby.rs` — LE / LAeq,24h / LAmax

**Analog (partial):** `weather/route1.rs:173` `l_den(day, evening, night)` — the existing energy-weighted dB combination with typed `CaseLoadError` on non-finite input. **Pitfall 4:** its `Period` hour weights must become parameterizable (Danish 12/3/9 for FORCE, not EU 12/4/8). LE integration `10·lg Σ (t_i/t₀)·10^{L_i/10}` follows the same accumulate-energy-then-log shape. A-weighting: call `compare::a_weighting_db` (`compare.rs:340-347`) at exact `FREQ_AXIS` centres — never a nominal-Hz table (the "Don't Hand-Roll" row is already satisfied by this existing function).

---

### `crates/envi-harness/src/cases/xls.rs` — curved / city / yearly parsers (04-04)

**Analog:** the straight-road parser in the same file — extend, don't fork.

**Label-anchored reading (the core idiom to reuse):** `xls.rs:111-147`:
```rust
fn find_label_row(range, col, labels: &[&str]) -> Option<u32>   // exact match — "LAE" vs "LAeq,24h"!
fn labelled_num(range, sheet, label_col, value_col, labels, fallback_row)
    -> Result<Option<f64>, CaseLoadError>  // label PRIMARY, fixed row fallback + stderr warning
```
Full sheet parse shape: `xls.rs:150-307` (`parse_sheet`) — description row, propagation block (labels col A / values col B), results block (col F/G), 27-row spectrum with row-count enforcement (243-248), terrain table with zero-padded-terminator detection (262-285), then `validate_profile` (47-77: finite + strictly-ascending-X + `MAX_PROFILE_ROWS` DoS cap). Workbook entry: `load_straight_road` (`xls.rs:319-351`) — per-sheet errors become per-case `DiscoveredCase` failures, `MAX_SHEETS` cap, `Force2009` provenance tag on every case.

**"Met. statistics" sheet is already parsed:** `weather/route1.rs:230` `load_met_probabilities(xls_path, direction_deg, period)` — the yearly-average loader composes with this, it does not reparse.

**Coordinates-sheet loaders:** new sheet-schema structs go next to `TerrainRow`/`SpectrumRow` (`cases/mod.rs:284-327`); new error variants extend `CaseLoadError` (`cases/mod.rs:49-170` — note every variant carries the sheet/path context and the caps pattern `TooManyProfileRows`/`TooManySheets`).

**Discovery wiring:** replace the placeholder block `cases/mod.rs:527-569` (the three-workbook placeholder loop) with real `load_*` calls following the straight-road arm `cases/mod.rs:508-523` (present-file check → `confine` (445-464, path-traversal guard) → loader → append; missing refs degrade to a note, never an error). Loader unit tests use the refs-gated auto-skip pattern (`xls.rs:382-425`): `if !path.is_file() { eprintln!("SKIP: … run refs/fetch.sh"); return; }`, then full-precision cell anchors at `1e-9`.

### `crates/envi-harness/src/lib.rs` — the FORCE `run_case` arm (04-03/04-04)

**Analog:** `run_terrain_case` (`lib.rs:287-324`) — the complete dispatch shape:
```rust
let te = match terrain_effect(&profile, src, rcv, coh.c0, &coh, axis, None) {
    Ok(te) => te,
    Err(PropagationError::NonFlatTerrainNotImplemented { .. }) => {
        return Outcome::Skipped("requires: non-flat-terrain (Sub-model 3, Phase 3)".to_string());
    }
    Err(e) => return Outcome::FailDetail(format!("terrain_effect failed: {e}")),
};
let report = compare::compare_pointwise(&te.delta_l_db, reference, expected.tolerance_db, &axis.centres);
if report.pass { Outcome::Pass } else { Outcome::Fail(report) }
```
Key moves to copy: (a) every fallible step maps to `Outcome::FailDetail(msg)` naming the quantity — never a panic; (b) an unimplemented-capability *typed error* maps to `Outcome::Skipped` mid-run (the honest-green invariant even after the gate passes); (c) spectrum mismatches return `Outcome::Fail(ComparisonReport)`. The FORCE arm replaces `lib.rs:76-82` ("FORCE dispatch lands in Phases 2-4") and compares via `compare_27_band(got27, want27, Some((got_overall, want_overall)), /* dip_shift for ForceStraightRoad */ true)` with the 105→27 pick `compare::pick_third_octave` (`compare.rs:320-322` — by index, every 4th point). The `dL` anchor comparison reads `SpectrumRow.dl_db` / `le_db` (`cases/mod.rs:302-313`).

### `crates/envi-harness/src/capability.rs` — the EmissionModel flip

**Analog:** exact file. Flip = add `Capability::EmissionModel` to `implemented_capabilities()` (`capability.rs:120-128`) with a plan-numbered doc comment like the Phase-3 entry (109-118). **Mandatory companion test:** the requires-shrink assertion pattern (`capability.rs:230-262`) — after the flip, assert the missing-set for representative FORCE cases is now empty (or names only the still-gated capability, e.g. `forest-scattering` if Open Q3 resolves to defer cases 121-124; that path needs a new `Capability` variant + `as_str` label, `capability.rs:17-46`).

---

### `crates/envi-harness/tests/oracle_noisemodelling.rs` + `tools/noisemodelling_oracle/` (04-05, VAL-03)

**Analog (test side):** `tests/oracle_refraction.rs` — copy its structure whole:
- `#[derive(Deserialize)]` fixture structs incl. `Meta` with named tolerances (`oracle_refraction.rs:17-79`);
- `load()` via `concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/oracle/….toml")` (81-88);
- **band-index comparison** — the fixture row carries `band_index`, the test evaluates at `axis.centres[row.band_index]` (144-160). For CNOSSOS octaves use grid indices 16, 28, 40, …, 100 (every 12th from 63.096 Hz);
- coverage assertions so a truncated fixture can't silently pass (`saw_up && saw_down`, 131-134, 192-196);
- absolute-vs-relative tolerance switch near zero (`rel_err` helper + `< 1e-12` branch, 90-92, 108-113).
Equality gates only for divergence/air-absorption; barrier/ground rows are report-only expected-delta tables (04-RESEARCH VAL-03 table).

**Analog (generator side):** `tools/nord2000_oracle/gen_ground_fixtures.py` + `common.py` — copy:
- module docstring declaring "regeneration is operator-driven; the TOML is committed; Python NOT a build dependency" (`gen_ground_fixtures.py:1-10`);
- `# generated by … — DO NOT EDIT` header + `[meta]` block with `oracle = "…"`, `provenance = "… sha256:…"` and explicit per-quantity tolerances WITH a comment justifying each (`gen_ground_fixtures.py:60-83`);
- equations cited by report+number only, no document text (`common.py:8-12` licensing note).
For NoiseModelling the "generator" is a documented human-run recipe (Java one-time; record NM version + scene hash + date in `[meta]`). **Environment rule (CRITICAL): never force-close Java/JVM/Gradle processes, and never close/kill/restart VS Code — this has crashed the editor before.** New `gen_submodel{3,8,10,11}_fixtures.py` scripts follow the same template inside `tools/nord2000_oracle/` (shared `common.py` conventions).

### Integration tests: `tensor_budget`, `emission_anchor`, MAC identity

**Analog:** test-file layout of `tests/oracle_refraction.rs` (plain `#[test]` integration file in `crates/envi-harness/tests/`); refs-gated tests use the `xls.rs:384-387` skip idiom. The budget test asserts the sink's high-water-mark byte counter (structural accounting, 04-RESEARCH Pattern 1/2 — never sample RSS, never materialize the full 100k-receiver tensor). The MAC identity test is `assert_eq!` on bits (compose G once — Pattern 5). The `LE − dL` emission anchor reads `ReferenceSpectrum.bands[i].le_db - dl_db` (`cases/mod.rs:302-327`) and compares by 27-band index.

### `refs/fetch.sh` + `refs/refs.sha256` — add the User's Guide

**Analog:** the existing `ARTIFACTS` `name|url` list (`refs/fetch.sh`, ~lines 29-37) and the pinned-hash manifest `refs/refs.sha256`. Append `Users_Guide_Nord2000_Road.pdf|https://egra.cedex.es/EGRA-ingles/I-Documentacion/National_Methods/Users_Guide_Nord2000_Road.pdf` and pin sha256 `73f465e2cd78e54d536f5c29dce139c4fb5430539c84b00d974d0bc4d7d21491` (verified live in 04-RESEARCH). SP 2006:12, if obtained without a stable URL, gets a manual-drop note + pinned hash (no URL row).

### SM11 image-source façade path builder (harness, 04-04)

**Analog (partial):** `envi_engine::geometry::reflect_over_segment` — already used end-to-end in `run_geometry_case` (`lib.rs:132-165`): mirror source over a segment, get `point_x` + `r1_m`/`r2_m` + a `valid` flag for on-segment checks. First- and second-order façade reflections compose this primitive per reflecting face; the ρE energy factor applies at the SM11 kernel, not in geometry. Case cutoff (20× shortest distance) is loader/case logic, like the road-extent handling.

## Shared Patterns

### Typed errors, never panics (every new file)
**Sources:** `scene.rs:31-66` (`SceneError`), `propagation/mod.rs:49-148` (`PropagationError`), `cases/mod.rs:48-170` (`CaseLoadError`).
Pattern: `#[derive(Debug, Error)]` + struct variants carrying the offending values and context; constructors validate at the trust boundary; "not implemented" states are dedicated variants with doc comments explaining the honest-green rationale (never a silent fallback). `debug_assert!` only for programmer contracts (e.g. `transfer.rs:62`), never data.

### The conj quarantine (tensor/solver/conditioning)
**Source:** `transfer.rs:73-92`. All Nord2000-native math stays e^{−jωt} inside `propagation/`; the ONE `.conj()` is `nord_ratio_to_transfer`. Conditioning (G_s, delay ramps), directivity factors, and all tensor arithmetic live on the ENVI-convention side (post-conj). Gate: grep `\.conj()` over `propagation/` stays 0 (any needed conjugation is written `Complex::new(re, -im)` — see `special.rs:41,275`).

### Band-index discipline (comparator, fixtures, balloons, A-weighting)
**Sources:** `freq.rs:26-31, 90-96`; `compare.rs:315-329`; `oracle_refraction.rs:144-148`. 105→27 via index pick (every 4th point); fixtures carry `band_index`, tests evaluate `axis.centres[idx]`; CNOSSOS octaves = indices 16+12k; `f == 31.5` anywhere is a bug.

### Fail-soft capability gating (all FORCE work)
**Sources:** `lib.rs:47-56` (gate before dispatch), `capability.rs` (declare vs implement + shrink tests), `tests/force.rs:55-73` (Skipped → ignored Trial with the requires-list as kind), `main.rs:60-85` (report table). New capabilities/cases must surface `Skipped(requires: …)` until numerically honest, and the flip must come with a shrink assertion.

### Committed-fixture oracle (all new numerics)
**Sources:** `tools/nord2000_oracle/gen_ground_fixtures.py` (+ `common.py`), consumed by `tests/oracle_*.rs`. Generated TOML with provenance sha256 + justified tolerances, committed; no Python/Java at test time. Standing caveat (04-RESEARCH): the scipy oracles cross-check *implementation*, not *spec reading* — the FORCE `.xls` is the only external authority.

### Refs-gated unit tests
**Source:** `xls.rs:381-425`, `scene_build.rs:182-193`. `if !path.is_file() { eprintln!("SKIP: … run refs/fetch.sh"); return; }` then full-precision cell anchors (`39.39836757521`, ε=1e-9 — never rounded appendix values).

### Bit-identical regression guards for wiring changes
**Source:** `terrain_effect/mod.rs:784-805, 881-903`. Whenever a new branch is added to an existing chain (SM3, segmented refraction, screen refraction, directivity-off, conditioning-neutral), add an `assert_eq!` (not ≈) test that the neutral configuration reproduces the previous path bit-for-bit. This is the house idiom for "new physics must not perturb frozen validation".

### Engine dep quarantine gate
**Source:** `crates/envi-engine/Cargo.toml:10-13`; verification command precedent `cargo tree -p envi-engine -e normal --depth 1` (01-VERIFICATION.md:76, 02-05/03-03 SUMMARYs). tensor.rs / solver.rs / directivity.rs add ZERO deps; every plan's gate re-runs the check.

## No Analog Found

Files with no close match in the codebase (planner should use 04-RESEARCH sketches):

| File | Role | Data Flow | Reason / fallback |
|------|------|-----------|-------------------|
| `TensorSink` trait + chunked solve loop | trait seam | streaming | No streaming/sink abstraction exists anywhere in the tree. Use 04-RESEARCH Pattern 1 sketch verbatim (`put_chunk(r_offset, ArrayView3<Complex<f64>>, ArrayView3<f64>)`); high-water-mark accounting per Pattern 2 |
| Balloon bilinear interpolation over a spherical grid | numeric kernel | transform | No interpolation code in the tree. 04-RESEARCH Pattern 6 sketch + the <0.05 dB sampling-error unit test against the analytic road directivities |
| Pass-by 1°-segment discretization + oblique-profile stretch | geometry builder | batch | No moving-source machinery exists. 04-RESEARCH Pattern 7 (perpendicular profile × 1/cosθ, exact under lateral invariance); the `LE − dL` anchor is the safety net |

## Metadata

**Analog search scope:** `crates/envi-engine/src/**` (all modules), `crates/envi-harness/src/**` (cases, capability, compare, scene_build, weather, lib, main), `crates/envi-harness/tests/**`, `tools/nord2000_oracle/**`, `refs/fetch.sh`, engine `Cargo.toml`.
**Files scanned:** 25 read in full or via targeted sections (workspace totals ~18k lines).
**Pattern extraction date:** 2026-07-08
**Upstream inputs:** `04-RESEARCH.md` (read in full; no `04-CONTEXT.md` exists — no discuss-phase ran).
**Environment rule carried forward:** VAL-03 fixture generation runs a JVM — never force-close Java/JVM/Gradle/language-server processes and never close/kill/restart VS Code.
