# Phase 5: Engine Extensions — Forest & Semi-Transparent Partitions - Pattern Map

**Mapped:** 2026-07-09
**Files analyzed:** 13 new/modified files
**Analogs found:** 12 / 13 (the Hilbert/cepstrum min-phase kernel has no codebase analog — see "No Analog Found")

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/envi-engine/src/forest.rs` (NEW; placement discretionary, top-level recommended) | pure-math module (per-band attenuation from physical params) | transform (params → `[f64; N_BANDS]` dB) | `crates/envi-engine/src/propagation/air_absorption.rs` | exact |
| `crates/envi-engine/src/propagation/transmission.rs` (NEW; name discretionary — `min_phase`/`transmission`) | pure-math module (complex min-phase filter, Nord-native side) | transform (`R(f)` → `T(f): Complex` per band) | `crates/envi-engine/src/propagation/special.rs` | role-match |
| `crates/envi-engine/src/solver.rs` (MOD) | solver seam (`SolveJob.forest` + post-conj application) | request-response (job → tensor pair) | itself — `directivity_gain_db`/`directivity_phase_rad` pattern | exact |
| `crates/envi-engine/src/propagation/terrain_effect/screen.rs` (MOD) | screen sub-model engine (transmission add point) | transform (geometry → two-channel `GroundResult`) | itself — `run_four_path`/`run_eight_path` coherent producers | exact |
| `crates/envi-engine/src/propagation/terrain_effect/mod.rs` (MOD) | composition/dispatch (`screen_channel`, optional-input threading) | transform (profile → `TerrainEffect`) | itself — the `refraction: Option<&SoundSpeedProfile>` threading pattern | exact |
| `crates/envi-engine/src/propagation/mod.rs` (MOD) | module registration + typed error variants | config | itself — `PropagationError` struct-variant pattern | exact |
| `crates/envi-engine/src/lib.rs` (MOD, if `forest` is top-level) | crate root module list | config | itself (`pub mod directivity;`) | exact |
| `tools/nord2000_oracle/gen_forest_fixtures.py` (NEW) | oracle fixture generator | batch (formula → committed TOML) | `tools/nord2000_oracle/gen_submodel11_fixtures.py` | exact |
| `tools/nord2000_oracle/gen_minphase_fixtures.py` (NEW) | oracle fixture generator (scipy hilbert/min-phase) | batch | `gen_submodel11_fixtures.py` + `common.py` (scipy usage) | exact |
| `crates/envi-harness/tests/fixtures/oracle/{forest,minphase}.toml` (NEW, committed) | test data | — | `crates/envi-harness/tests/fixtures/oracle/submodel11.toml` | exact |
| `crates/envi-harness/tests/oracle_forest.rs` + `oracle_minphase.rs` (NEW) | oracle-comparison tests | batch (TOML → per-band assert) | `crates/envi-harness/tests/oracle_submodel11.rs` | exact |
| opaque-limit + `F→1` regression tests (NEW; in `screen.rs`/`solver.rs` `#[cfg(test)]` or a harness test) | regression test (bit-exact) | test | `terrain_effect/mod.rs` test `refraction_homogeneous_profile_is_bit_identical` + `solver.rs` `.to_bits()` asserts | exact |
| `crates/envi-harness/tests/{mac_identity.rs:53, tensor_budget.rs:56}` (MOD) | existing `SolveJob` construction sites | — | mechanical: add `forest: None` (plus 8 sites in `solver.rs` tests) | exact |

Also at phase close (doc contract): root `README.md` + module I/O headers of every touched module; `REQUIREMENTS.md` ENG-10 wording update for the min-phase extension (D-07).
Likely **untouched**: `crates/envi-harness/src/capability.rs` — `Capability::ForestScattering` is Sub-model 10 *scattering* (FORCE cases 121–124), a distinct accepted gap; ENG-09 excess attenuation does not close it.

---

## Pattern Assignments

### 1. `crates/envi-engine/src/forest.rs` — `forest_a(f)` + `ForestCrossing` (ENG-09, D-01/D-02)

**Placement precedent:** post-conj per-path/source extension modules live at **crate top level**, not in `propagation/` — `directivity.rs` is the precedent (its module doc, `directivity.rs:20-21`: "The slice is what a `SolveJob`'s `directivity_gain_db` carries — a **real magnitude** factor `10^{ΔL/20}` on `H_coh`, never a `propagation/` operator"). Register in `lib.rs` alongside `pub mod directivity;` (`lib.rs:19-26`).

**Math-shape analog:** `propagation/air_absorption.rs` — a per-band real attenuation computed from a validated physical-parameter struct.

**Validating-constructor pattern** (`air_absorption.rs:46-84`) — `ForestCrossing::new` should mirror `Atmosphere::new` exactly (typed error, explicit non-finite rejection, never panic on data):
```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Atmosphere {
    pub t_air_c: f64,
    pub rh_percent: f64,
    pub pressure_kpa: f64,
}

impl Atmosphere {
    pub fn new(t_air_c: f64, rh_percent: f64, pressure_kpa: f64) -> Result<Self, PropagationError> {
        // Reject non-finite explicitly (NaN and ±∞): a bare `x > bound` lets
        // +∞ through, which would poison every downstream α as NaN/∞.
        if !t_air_c.is_finite() || t_air_c <= -273.15 {
            return Err(PropagationError::InvalidTemperature { t_air_c });
        }
        ...
    }
}
```

**Pure per-band formula pattern** (`air_absorption.rs:149-163`) — `forest_a(f, &params)` should look like `alpha_db_per_m(f_hz, atmos)`:
```rust
/// Evaluate at the EXACT grid centre frequency, never the nominal label — α
/// grows as f², so nominal-vs-exact is a systematic error (01-RESEARCH Pitfall 3).
#[must_use]
pub fn alpha_db_per_m(f_hz: f64, atmos: &Atmosphere) -> f64 { ... }
```
Note the doc-comment discipline: the AV 1106/07 equation number in the header, the formula reproduced in a ` ```text ` block, and intermediates exposed as `pub fn` for stage-by-stage anchor tests (`air_absorption.rs:97-135`, "Three-stage transcription pinning" `air_absorption.rs:18-23`).

**Additive-real-dB precedent:** `terrain_effect/submodel7.rs:188-228` — an effect returning real `f64` dB is *structurally* incapable of touching phase (module doc `submodel7.rs:7-9`). Forest is the same type discipline: `a(f)` and `A = d·a(f)` are `f64`, never `Complex`.

**Anchor-test pattern** (`air_absorption.rs:228-245`) — assert at exact grid centres via `axis.third_octave_pick(i)` / `axis.centres[i]`, never nominal Hz; plus the D-12 analytic anchor "linear in d" mirrors `effective_strength_is_ten_times_base` (`submodel7.rs:315-325`, a factor-doubling ⇒ +10·lg 2 property test).

---

### 2. `crates/envi-engine/src/solver.rs` — `SolveJob.forest` + post-conj application (D-01/D-03/D-04)

**Analog: itself.** Copy the `directivity_gain_db` pattern verbatim.

**Field pattern** (`solver.rs:69-79`) — add `forest: Option<ForestCrossing>` (or `Option<&'a ForestCrossing>`; note `SolveJob` is `#[derive(Debug, Clone, Copy)]`, so a small `Copy` params struct can go by value like `src: [f64; 3]` does):
```rust
/// Optional pre-evaluated per-band directivity gain ΔL (dB), applied as a
/// real magnitude factor `10^{ΔL/20}` to `H_coh` and `10^{ΔL/10}` to
/// `P_incoh_abs`. The harness evaluates a balloon (`DirectivityBalloon::eval`)
/// into this slice; the solver only applies the factor.
pub directivity_gain_db: Option<[f64; N_BANDS]>,
```

**Application pattern** (`solver.rs:181-207`, in `solve_pair`) — the forest factor slots in exactly like the directivity magnitude block; `A_i = d_m · forest_a(f_i)` then `−A` in dB terms:
```rust
// Optional directivity magnitude: scales both channels (10^{ΔL/20} on the
// coherent transfer, 10^{ΔL/10} on the incoherent energy).
if let Some(dir) = job.directivity_gain_db.as_ref() {
    hc *= 10f64.powf(dir[i] / 20.0);
    pa *= 10f64.powf(dir[i] / 10.0);
}
```
D-03 is literally this shape with `ΔL = −A`: `hc *= 10^{−A/20}` (real factor — `arg(H_coh)` untouched), `pa *= 10^{−A/10}`. NEVER route forest into `terrain_effect` — it is a post-conj solver-side factor (D-04), like directivity, and the `propagation/` conj grep-gate stays 0.

Design choice for the planner: either precompute `[A_i; N_BANDS]` per job (needs `axis` — available as `job.axis`) or evaluate `forest_a(job.axis.centres[i], ...)` inline per band inside the existing loop (`solver.rs:181`).

**Solver test patterns to copy** (`solver.rs:341-445`):
- `directivity_gain_scales_magnitude_only_leaving_phase_unchanged` — build a `base` job, then `SolveJob { forest: Some(...), ..base }` (struct-update syntax, `solver.rs:365-368`), assert `arg` unchanged, `|H|` scaled by `10^{−A/20}`, `P` by `10^{−A/10}`.
- Bit-exact channel isolation via `.to_bits()` (`solver.rs:439-443`):
```rust
assert_eq!(
    with_phase.tensor().p_incoh_abs[[0, 0, f]].to_bits(),
    plain.tensor().p_incoh_abs[[0, 0, f]].to_bits(),
    "band {f}: directional phase must not touch P_incoh"
);
```
- `forest: None` regression: a `None` job must be bit-identical to today's output (same discipline as "A balloon without phase leaves `arg(H_coh)` untouched, bit-for-bit", `solver.rs:24`).

**Mechanical fallout:** every `SolveJob { ... }` literal gains `forest: None` — 8 sites in `solver.rs` tests (lines 256, 308, 348, 365, 399, 417, 454, 480) + `crates/envi-harness/tests/mac_identity.rs:53` + `crates/envi-harness/tests/tensor_budget.rs:56`.

---

### 3. `crates/envi-engine/src/propagation/transmission.rs` — min-phase filter `T(f)` (ENG-10+, D-06/D-08/D-09)

**Convention analog:** `propagation/special.rs` — the model for a pure-numerics module inside `propagation/` (Nord-native `e^{−jωt}`, header states the convention up front, `special.rs:6-17`):
```rust
//! # Convention (load-bearing)
//!
//! Nord2000-native module family: time convention **e^{−jωt}**, outgoing phase
//! **e^{+jωτ}**, impedance **Im > 0** (AV 1106/07). Conversion to ENVI's
//! e^{+jωt} transfer convention happens in exactly ONE function at the
//! `TransferSpectrum` boundary (plan 02-05) — **never here**.
```

**Explicit-conjugation style (D-08 sign work):** if any conjugation-like symmetry appears in the Hilbert/cepstrum math, write it as `Complex::new(re, -im)` with the quarantine comment — copy `special.rs:36-45`:
```rust
// Eq. 62: reflect across the imaginary axis into quadrant 1.
// w(ẑ) = conj(w(−conj(ẑ))), with −conj(ẑ) = (−x, y), x < 0 ⇒ Re > 0.
// Conjugation is written explicitly (negate Im) rather than `.conj()`
// — this is Faddeeva symmetry math, NOT a convention conversion, and
// the sole convention-boundary conjugation lives in `transfer.rs`
// (the propagation-module `conj` quarantine, threat T-02-15).
let refl = Complex::new(-z.re, z.im); // −conj(ẑ)
let w = w_plus(refl);
Complex::new(w.re, -w.im) // conj(w)
```
The single real conj boundary stays `transfer.rs:90-92`:
```rust
#[must_use]
pub fn nord_ratio_to_transfer(ratio: Complex<f64>) -> Complex<f64> {
    ratio.conj()
}
```

**Optional-phase extension precedent:** `directivity.rs:262-288` (`new_with_phase`) + `directivity.rs:415-425` (`eval_complex`) — magnitude bit-identical when the phase datum is absent, phase built as explicit `(cos, sin)`:
```rust
let mag = 10f64.powf(mag_db[band] / 20.0);
// ENVI e^{+jωt}: explicit (cos, sin), never `.conj()`.
*slot = Complex::new(mag * phase[band].cos(), mag * phase[band].sin());
```
(In `transmission.rs` the same construction is used but on the **native** side, with the D-08-verified sign of `φ_min` for `e^{−jωt}` — the conj at `transfer.rs` then flips it to ENVI convention automatically.)

**Isolation-spectrum type:** the per-band real spectrum on the 105-point grid is `[f64; N_BANDS]` — the exact type of `directivity_gain_db: Option<[f64; N_BANDS]>` (`solver.rs:73`). `N_BANDS = 105` from `freq.rs:36`; index by band, never by nominal Hz (`freq.rs:26-31`). A validated newtype `IsolationSpectrum` should follow the `DirectivityBalloon::new` validation pattern (finite everywhere, fixed length; typed error enum with `thiserror`, `directivity.rs:77-149`).

**No-new-deps constraint (structural, from `lib.rs` + CLAUDE.md):** `envi-engine` deps are quarantined to `ndarray + num-complex + thiserror` — **no FFT crate**. The Hilbert/cepstrum kernel must be hand-rolled (a direct DFT over the fixed 105-point axis is O(N²)≈11k ops, fine). Precedent for deliberately hand-rolling instead of adding a crate: `directivity.rs:493-497`:
```rust
/// A hand-rolled 3×3 rotation matrix (SRC-03), engine-local — **no linalg
/// crate** (D-08 precedent, mirroring the Cramer solve in `weather::route3`).
```

---

### 4. `crates/envi-engine/src/propagation/terrain_effect/screen.rs` — transmission add point (D-05/D-10)

**Analog: itself.** Two producer sites make the opaque coherent factor; the transmission term `+ T(f)` (the straight-through path normalized by the free-field direct `p̂₀` — note the ray direction is preserved, so the normalized transmitted term is `T(f)` itself, no extra geometry factor) joins the coherent complex sum there, Nord-native, ahead of the conj.

**Input seam:** `ScreenConfig` (`screen.rs:77-99`) gains `isolation: Option<&IsolationSpectrum>` (or per-band `Option<Complex<f64>>` pre-evaluated `T` at this `f` — planner's choice; `ScreenConfig` is `Copy` with borrowed slices, so a borrowed option fits the existing style):
```rust
#[derive(Debug, Clone, Copy)]
pub struct ScreenConfig<'a> {
    pub source: [f64; 2],
    pub receiver: [f64; 2],
    pub screen: &'a [[f64; 2]],
    ...
    pub z_face_source: Complex<f64>,
    pub z_face_receiver: Complex<f64>,
    pub coh: &'a CoherenceInputs,
}
```

**Coherent producer — four-path engine** (`screen.rs:584-592`, end of `run_four_path`):
```rust
let cc_eff = cc_eff_ln.exp();
let g_eff = g_eff_ln.exp();
let scr_mag = p_scr.norm();
let scr_arg = p_scr.arg();
// ĥ = p̂_SCR · √CC_eff · e^{j(arg p̂_SCR + Σ W·arg c)} — |ĥ|² = |p̂_SCR|²·CC_eff.
let h_coh = Complex::from_polar(scr_mag * cc_eff.sqrt(), scr_arg + phase_acc);
// p_incoh = |p̂_SCR|²·(G_eff − CC_eff) ≥ 0 (weighted GM is monotone).
let p_incoh = scr_mag * scr_mag * (g_eff - cc_eff).max(0.0);
Ok((GroundResult::from_channels(h_coh, p_incoh), g_eff))
```
**Eight-path engine** (`screen.rs:846-849`):
```rust
// ĥ = p̂_SCR · coherent (both complex, phase live); p_incoh = |p̂_SCR|²·incoh.
let h_coh = p_scr * coherent;
let p_incoh = p_scr.norm_sqr() * incoh;
Ok(GroundResult::from_channels(h_coh, p_incoh))
```
D-05 additive composition: `h_coh + T(f)` as complex pressure (`diffracted_field + H_ff·T(f)` in normalized-ratio form). Where exactly the ground-reflection weighting of the transmitted path applies (bare `T` vs `T·Δp̂_G`) is a spec question for the planner against AV 1106/07 §5.22 Eq. 332 — the *mechanical* pattern is: mutate the coherent complex channel at these two return sites, `P_incoh` untouched.

**D-10 structural gating** — the `None` branch must be the byte-identical existing code path; the pattern is early construction-avoidance, exactly like the refraction dispatch (`terrain_effect/mod.rs:398, 427-438`):
```rust
if let Some(rs) = self.refraction {
    // ... circular-ray path
} else {
    let rays = straight_rays(self.d, self.h_s, self.h_r, self.c0)?;
    submodel1::eval(f, &rays, ..., None, None)
}
```
i.e. `if let Some(iso) = cfg.isolation { h_coh += t_of(iso) }` — with `None`, no `T`, no `ln|T|`, no Hilbert call is ever constructed.

**Free-field normalization reference** (`screen.rs:449-463`, `screen_factor`) — how a path is expressed relative to `p̂₀`:
```rust
let p1 = kernel.diffract(f_hz, source, receiver)?;
let r_sr = dist(source, receiver);
let tau_sr = r_sr / c0;
// p̂₀ = e^{+jωτ_SR}/R_SR (Nord-native free-field direct).
let p0 = Complex::from_polar(1.0 / r_sr, TAU * f_hz * tau_sr);
Ok(p1 / p0)
```

---

### 5. `crates/envi-engine/src/propagation/terrain_effect/mod.rs` — threading + `screen_channel` (D-05/D-10)

**Analog: itself.** The optional per-path input threading pattern is `refraction: Option<&SoundSpeedProfile>` — an `Option` parameter on `terrain_effect` (`mod.rs:158-166`) flowing to the consumer, with a typed-error guard when a combination is unimplemented:
```rust
pub fn terrain_effect(
    profile: &TerrainProfile,
    src: [f64; 3],
    rcv: [f64; 3],
    atmos_c0: f64,
    coh: &CoherenceInputs,
    axis: &FreqAxis,
    refraction: Option<&SoundSpeedProfile>,
) -> Result<TerrainEffect, PropagationError> {
```
`isolation: Option<&IsolationSpectrum>` threads the same way: `solve_pair` (`solver.rs:169-177`) → `terrain_effect(...)` → `screen_channel` → `ScreenConfig`. All three existing `terrain_effect` call sites in `solver.rs` (169) and its tests then pass `None`.

**`screen_channel` assembly point** (`mod.rs:535-579`) — where `ScreenConfig` is built and SM7 energy is added; the isolation option drops into the `ScreenConfig` literal (`mod.rs:535-545`) and the two-channel reassembly at the end shows the contract:
```rust
// Sub-model 7 is energy-only (f64) — it can never touch the phase channel.
Ok(GroundResult::from_channels(
    base.h_coh_factor,
    base.p_incoh + sm7_energy,
))
```

**Two-channel shapes (the frozen contract these plug into)** — `GroundResult` (`mod.rs:71-94`) and `TerrainEffect` (`mod.rs:106-114`):
```rust
pub struct GroundResult {
    /// The Nord2000 band value `10·lg(|h_coh_factor|² + p_incoh)`, dB.
    pub delta_l_db: f64,
    /// The F-weighted coherent complex sum, phase LIVE (multiplies into `H_coh`).
    pub h_coh_factor: Complex<f64>,
    /// The `(1−F²)·|ρᵢ·p̂ᵢ/p̂₀|²` turbulence-decorrelated residual — real,
    /// non-negative, added at readout only. Exactly `0` when `F → 1`.
    pub p_incoh: f64,
}

impl GroundResult {
    #[must_use]
    pub fn from_channels(h_coh_factor: Complex<f64>, p_incoh: f64) -> Self {
        let energy = h_coh_factor.norm_sqr() + p_incoh;
        Self { delta_l_db: 10.0 * energy.log10(), h_coh_factor, p_incoh }
    }
}
```
Transmission goes into `h_coh_factor` (coherent, native side); forest never appears here at all (solver-side).

**Opaque-limit regression pattern** — copy `refraction_homogeneous_profile_is_bit_identical` (`mod.rs:867-888`): run the same case with `isolation: None` vs the pre-change baseline (or opaque-vs-`Some` structural equality), asserting `assert_eq!` (f64 `==`, i.e. bit-for-bit for non-NaN) per band:
```rust
for i in 0..N_BANDS {
    assert_eq!(
        base.delta_l_db[i], hom.delta_l_db[i],
        "band {i}: homogeneous refraction must match the Phase-2 path bit-for-bit"
    );
}
```

---

### 6. `crates/envi-engine/src/propagation/mod.rs` — error variants + module registration

New modules append to the list at `mod.rs:28-37` (`pub mod transmission;`). New failure modes (e.g. invalid forest params, malformed isolation spectrum) follow the `PropagationError` struct-variant pattern (`mod.rs:50-96`):
```rust
#[error("invalid flow resistivity: σ = {sigma_kpa} kPa·s·m⁻² must be positive and finite")]
InvalidFlowResistivity { sigma_kpa: f64 },
#[error("degenerate ray geometry: {detail}")]
DegenerateRayGeometry { detail: &'static str },
```
(Alternatively, a self-contained module error enum via `thiserror` is fine — `DirectivityError` in `directivity.rs:77-149` is that precedent for a top-level module like `forest.rs`.)

---

### 7. `tools/nord2000_oracle/gen_forest_fixtures.py` + `gen_minphase_fixtures.py` (D-09/D-12)

**Analog:** `gen_submodel11_fixtures.py` — copy its whole structure:
- Module docstring: what it writes, "Regeneration is operator-driven", "Python/scipy are NOT build dependencies", the oracle-independence caveat, "Equations cited by report + number only (licensing rule)" (`gen_submodel11_fixtures.py:1-18`).
- Reimplement the formula in plain Python (`:34-73`); for min-phase use `scipy.signal.hilbert` / `numpy.fft` as the **independent** kernel (the analog of `common.py:22` using `scipy.special.wofz` as the independent Faddeeva).
- Named configs list + sparse band indices (`:78-96`): `BAND_INDICES = list(range(0, 105, 8))`.
- Output writing with provenance + tolerance (`:99-134`):
```python
provenance = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
out = Path(__file__).resolve().parents[2] / (
    "crates/envi-harness/tests/fixtures/oracle/submodel11.toml"
)
lines = [
    "# generated by tools/nord2000_oracle/gen_submodel11_fixtures.py — DO NOT EDIT",
    ...
    "[meta]",
    f'provenance = "gen_submodel11_fixtures.py sha256:{provenance}"',
    f"c0 = {c0}",
    "tol_abs_db = 1e-6",
]
...
lines.append(f"band_index = {bands}")
lines.append("l11_db = [" + ", ".join(f"{v:.10f}" for v in vals) + "]")
```
- The frequency axis comes from `flat_models.freq_axis()` (the Python mirror of `FreqAxis`), `c0` from `common.C0_M_S` — reuse both.
- For the min-phase oracle, emit **complex** values as parallel `re = [...]` / `im = [...]` arrays (or `t_mag`/`t_phase_rad`), still keyed by `band_index`.

---

### 8. `crates/envi-harness/tests/oracle_forest.rs` / `oracle_minphase.rs`

**Analog:** `oracle_submodel11.rs` — copy end to end:
- Serde structs mirroring the TOML (`oracle_submodel11.rs:19-43`), `load()` via `concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/oracle/....toml")` (`:45-52`).
- Per-row assertion **by band index** (`:84-95`):
```rust
for (k, &bi) in case.band_index.iter().enumerate() {
    let f = axis.centres[bi];
    let got = submodel11(f, &cfg)
        .unwrap_or_else(|e| panic!("{} band {bi}: submodel11 failed: {e}", case.name));
    let want = case.l11_db[k];
    assert!((got - want).abs() <= fx.meta.tol_abs_db, ...);
}
```
- Coverage assertions so a truncated fixture cannot pass silently (`:105-112`): minimum row count + "saw both regimes" flags (for forest: a zero-`d` row and an attenuating row; for min-phase: a flat-`R` row where `φ_min ≡ 0` and a sloped-`R` row with live phase).

---

## Shared Patterns

### Two-channel factor application (forest, D-03)
**Source:** `crates/envi-engine/src/solver.rs:192-197`
**Apply to:** the forest block in `solve_pair`
```rust
hc *= 10f64.powf(dir[i] / 20.0);   // amplitude on H_coh (arg untouched)
pa *= 10f64.powf(dir[i] / 10.0);   // energy on P_incoh_abs
```

### Single-conj quarantine + explicit conjugation
**Source:** `crates/envi-engine/src/transfer.rs:90-92` (the ONE `.conj()`); `crates/envi-engine/src/propagation/special.rs:36-45` (explicit `Complex::new(re, -im)` style)
**Apply to:** `transmission.rs` (native-side `T(f)`), any D-08 sign work. The grep gate `\.conj()` over `propagation/` must stay **zero** after this phase.

### Typed errors, never panics on data
**Source:** `propagation/mod.rs:50+` (`PropagationError`), `air_absorption.rs:67-84` (`Atmosphere::new`), `directivity.rs:77-149` (`DirectivityError`)
**Apply to:** `ForestCrossing`/`IsolationSpectrum` constructors, degenerate `d_m`/non-finite `R(f)` handling.

### Bit-exact structural regression (`None` ⇒ identical)
**Source:** `terrain_effect/mod.rs:867-888` (`assert_eq!` per band on `None`-vs-homogeneous), `solver.rs:439-443` (`.to_bits()` channel isolation)
**Apply to:** opaque-limit test (D-10), `forest: None` no-op test, `F→1 ⇒ P_incoh→0` preservation.

### Band-index discipline
**Source:** `freq.rs:26-31, 36, 90-96`; `oracle_submodel11.rs:85` (`let f = axis.centres[bi];`)
**Apply to:** everything — `[f64; N_BANDS]` spectra, oracle fixtures keyed by `band_index`, anchors at `axis.third_octave_pick(i)`. `f == 31.5` anywhere is a bug.

### Module I/O header contract
**Source:** every module shown (e.g. `submodel7.rs:1-27`, `screen.rs:1-38`): `//!` header = AV 1106/07 section/equation numbers + convention statement + the channel contract the module honors + deliberate deviations flagged "do not fix".
**Apply to:** `forest.rs`, `transmission.rs`, and the headers of all modified modules; root `README.md` at phase close (D-07 doc reflection).

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| Hilbert/cepstrum min-phase kernel (inside `transmission.rs`) | numeric algorithm | transform (105-pt log-magnitude → phase) | No FFT/Hilbert/cepstrum code exists anywhere in the workspace, and the engine dep quarantine (`ndarray + num-complex + thiserror` only) forbids `rustfft`. Must be hand-rolled (direct DFT over the fixed grid is trivial at N=105). Style precedents: `special.rs` (hand-rolled document numerics), `directivity.rs:493-497` `Rotation3` ("no linalg crate" hand-roll precedent). The **numeric truth** comes from the committed scipy oracle (D-09), and the D-08 sign audit follows the w(ẑ)/Δτ audit discipline — a first-principles causal-filter check in a unit test, not just the oracle. |

Also without an analog in-code (but with a defined workflow): the AV 1106/07 forest-attenuation `a(f)` formula itself — read from `refs/AV1106-07-rev4.pdf` and verified against page images (per-project rule; multiple transcription errors were caught this way). `docs/references/dbaudio-ti386-1.6-en.md` §Forest carries the ISO-variant distance table which must **not** be imported (deferred item).

## Metadata

**Analog search scope:** `crates/envi-engine/src/**`, `crates/envi-harness/tests/**`, `tools/nord2000_oracle/**`
**Files scanned:** 33 engine sources, 14 harness tests, 10 oracle scripts; 11 read in full
**Pattern extraction date:** 2026-07-09
