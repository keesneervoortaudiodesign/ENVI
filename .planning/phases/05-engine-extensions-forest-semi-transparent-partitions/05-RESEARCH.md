# Phase 5: Engine Extensions — Forest & Semi-Transparent Partitions - Research

**Researched:** 2026-07-09
**Domain:** Nord2000 scattering-zone acoustics (Sub-Model 10) + minimum-phase transmission filters, pure-math Rust engine
**Confidence:** HIGH (forest equations verified against AV 1106/07 page images; min-phase algorithm numerically verified; composition points read from the shipped code)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Forest — seam & geometry (ENG-09)
- **D-01:** The engine receives the forest crossing via a **`SolveJob` seam**, not scene-geometry
  intersection. `SolveJob` gains `forest: Option<ForestCrossing>` where
  `ForestCrossing { d_m, density, stem_radius, kₚ, absorption }` (a single scalar through-path
  length `d_m` + the physical parameters). Mirrors the existing `directivity_gain_db` /
  `PropagationPath` seam pattern. Rationale: Milestone 1 is FORCE-fed with **no forest test cases**;
  real path/forest-length extraction is Phase 9. Keeps geometry logic out of the pure-math crate.
- **D-02:** The engine **owns the `a(f)` formula** — `ForestCrossing` carries the raw physical
  params and `envi-engine` computes `a(f)` as pure acoustics math (`fn forest_a(f, &ForestParams)`).
  Consistent with the crate split (physics = engine, I/O = harness) and directly oracle-testable.
  The harness passes params only; **no acoustics in the harness.**

#### Forest — two-channel action (ENG-09)
- **D-03:** `A = d·a(f)` applies as a **real, phase-preserving magnitude factor to both channels**:
  `10^(−A/20)` on `H_coh` (argument untouched) and `10^(−A/10)` on `P_incoh_abs`. This is Nord2000's
  treatment of forest as **excess attenuation** (not decorrelation — explicitly NOT routed into the
  incoherent channel like SM7 turbulence scattering). `F→1 ⇒ P_incoh→0` stays bit-exact.
- **D-04:** Because it is a real per-path magnitude factor, forest is applied **solver-side on the
  post-conj ENVI convention**, exactly like `directivity_gain_db` — **never inside `propagation/`**
  (the conj grep-gate stays at zero). Forest is a per-path property, structurally analogous to the
  directivity magnitude factor.

#### Semi-transparent — transmission combination point (ENG-10)
- **D-05:** The transmitted straight-through path is added **inside the `propagation/` screen
  sub-models**, in the Nord2000-native `e^{−jωt}` convention, **ahead of the single `.conj()`** at
  `transfer::nord_ratio_to_transfer`. It is genuine propagation physics combined as complex pressure
  alongside the diffracted/reflected terms (SC2). Keeps the one conj boundary intact and puts the
  opaque↔transparent behaviour in one place. Composition is **additive**: `diffracted_field +
  H_ff·T(f)` as complex pressure (the diffracted opaque field always exists; transmission adds a
  leakage path on top).

#### Semi-transparent — isolation as a MINIMUM-PHASE FILTER (ENVI extension, ENG-10+)
- **D-06:** The isolation spectrum is **not** a purely real amplitude factor. It becomes a **complex
  minimum-phase transmission filter** `T(f) = 10^(−R(f)/20) · e^{jφ_min(f)}`, where
  `φ_min(f) = −H{ ln|T(f)| }` — the minimum-phase response reconstructed from the log-magnitude via
  the Hilbert transform (equivalently the real cepstrum) over the 105-point band axis. Rationale: a
  physical passive partition **is** a minimum-phase system — its transmitted phase follows its
  amplitude and cannot be specified independently.
- **D-07:** This is an **ENVI extension beyond stock Nord2000** (which treats `R` as real energy
  loss) — same spirit as the directional complex-phase extension (Phase 4). It **extends ENG-10**
  (currently specced as real `10^(−R/20)`); REQUIREMENTS.md / SUMMARY / README / module headers get
  updated to reflect the complex min-phase transmission at phase close.
- **D-08 (verify item):** the **sign/convention of `φ_min` is load-bearing.** It must be expressed
  in the `e^{−jωt}` native convention inside `propagation/` so the single conj flips it correctly to
  ENVI's `e^{+jωt}`. Verify the phase sign against a first-principles causal-min-phase-filter check
  (same audit discipline as the w(ẑ) Faddeeva-symmetry and Δτ sign audits). Do NOT introduce a
  `.conj()` in `propagation/` to fix a sign — write conjugation explicitly if needed.
- **D-09:** the min-phase reconstruction (Hilbert / cepstrum over the grid) is **pure engine math**
  in `envi-engine`, oracle-tested against a committed scipy reference (`scipy.signal.hilbert` /
  `numpy.fft`-based min-phase), per the established oracle+anchor ladder. Note the oracle-independence
  caveat: a same-transcription oracle cross-checks implementation, not spec reading.

#### Semi-transparent — opaque-limit regression (ENG-10, SC3)
- **D-10:** **Structural gating**, not numeric clamping. A screen with **no isolation spectrum**
  (`isolation: Option<IsolationSpectrum> = None`) takes the **exact existing opaque code path** — the
  transmission term and the whole min-phase computation are **never constructed**. This (a) guarantees
  bit-identical opaque results (`R→∞` reproduces the standard screen bit-for-bit) and (b) never feeds
  `ln|T| = −∞` to the Hilbert transform (as `R→∞`, `ln|T|→−∞`). A **permanent regression test** pins
  the opaque-limit equality. "Opaque" is a distinct state (`None`), not a magic large `R` value.

#### Following directly (recorded, not separately discussed)
- **D-11:** **Per-façade building transmission (SC4)** collapses into the same ENG-10 mechanism — the
  engine applies whichever crossed partition's `R(f)` it is given; façade→`R(f)` selection is a
  downstream (Phase 7 scene / Phase 9 path) concern, not engine logic here.
- **D-12:** **Acceptance ladder (no FORCE cases exist for these physics):** analytic anchor for
  `A = d·a(f)` (linear in `d`), scipy `hilbert`/min-phase oracle for `φ_min`, the opaque-limit
  bit-exact regression, and the `F→1 ⇒ P_incoh→0` bit-exactness — the same oracle+anchor pattern used
  in Phases 2–3. No false FORCE numeric Pass is claimed; cases stay honest.

### Claude's Discretion
- Module placement/naming within `envi-engine` (a `forest` module vs folding into `terrain_effect`;
  where `min_phase`/`transmission` helpers live), exact `ForestParams`/`IsolationSpectrum` field
  types, and the internal Hilbert/cepstrum algorithm — all left to research/planning, provided the
  seams (D-01, D-05), conventions (D-04, D-05, D-08), and regression (D-10) above hold.

### Deferred Ideas (OUT OF SCOPE)
- **ISO 9613-2 forest distance-clamp regimes** (`<10 m→0`, `10–20 m→A₁₀₋₂₀`, `≥200 m→200·a(f)`) —
  the ISO variant, not Nord2000. Out of scope (project is Nord2000-only). Recorded only so the
  planner recognizes it in TI 386 and does not import it; confirm Nord2000's own `d` bounds from
  AV 1106/07.
- **Reflection-path transmission** (a semi-transparent screen also modifying its *reflected* path by
  `R(f)`) — not raised as needed; the decided scope is the single straight-through transmitted path
  added to the existing diffracted/reflected composition. Revisit only if a case demands it.
- **Multiple / heterogeneous forest crossings on one path** — D-01 chose a single scalar `d_m`; the
  list-of-segments seam was considered and deferred. Real multi-forest paths are a Phase 9
  path-extraction concern; revisit the seam then if needed.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ENG-09 | Forest scattering/attenuation along through-forest path length `d`, evaluated at 1/12-octave, applied to the transfer as per-path attenuation | §Forest findings: AV 1106/07 §5.19 Sub-Model 10 (Eqs. 288–291, Tables 8/9) verified against page images pp. 125–127; solver-side two-channel application mirrors `directivity_gain_db` (verified in `solver.rs`); anchors: kf=0 low-band zero, Rsc→0 zero, −15 dB floor, on-node hand anchor |
| ENG-10 | Semi-transparent partition: transmission path through a screen/façade attenuated by isolation `R(f)`, ray direction preserved, combined as complex pressure with phase intact; opaque limit bit-exact | §Transmission findings: composition point = `screen_channel` in `terrain_effect/mod.rs` (native side, pre-conj); transmitted term relative to `p̂₀` is exactly `T(f)`; min-phase cepstral algorithm specified and numerically verified (sign, flatness, linearity all to ~1e-15); D-10 structural gating design |

</phase_requirements>

## Summary

Both physics blocks are fully specified and implementable with **zero new dependencies**. The
critical research finding is about ENG-09: **Nord2000 does not define a per-metre forest
attenuation `a(f)` at all.** The phrase "A = d·a(f) from mean tree density, mean stem radius,
factor kp and mean absorption coefficient" (TI 386 §Forest, echoed in REQUIREMENTS ENG-09 and the
roadmap SC1) is NoizCalc's UI-level paraphrase of **AV 1106/07 §5.19 "Sub-Model 10: Scattering
Zones"** — the parameter list matches 1:1 (density n″, stem radius a, "kp" ≡ the tabulated
frequency weighting k_f, absorption α), but the actual law is
`ΔL_s = Max(1.25·k_f·T·A_e(R_sc), −15)` with `T = Min((R_sc·nQ/1.75)², 1)`, `nQ = 2·a·n″`,
`A_e = ΔL(h′,α,R′) + 20·log₁₀(8R′)` — quadratic-then-saturating in the crossing length, floored at
−15 dB, and exactly zero below `ka ≤ 0.7`. All equations and both tables were **verified against
the PDF page images (pp. 125–127)**. The locked seams (D-01..D-04) survive intact: ΔL_s is still a
real per-path, per-band dB factor applied solver-side to both channels; only the internal formula
(engine-owned per D-02) is richer than the paraphrase, and Nord2000's own bounding of `d` (the
−15 dB floor + T-saturation) answers the roadmap's SC1 question. One genuine scope discrepancy is
surfaced for the planner: Nord2000 *also* reduces the terrain-effect coherence via `Fs = 1 − k_f·T`
(Eq. 288, entering `F = Ff·FΔν·Fc·Fr·Fs`, Eq. 110) — a decorrelation mechanism D-03's rationale
explicitly excluded; recommended disposition: defer with a documented seam (see Open Questions).

For ENG-10, the composition point analysis is clean: every screen sub-model's `h_coh_factor` is a
complex pressure **ratio relative to the free-field direct `p̂₀`** (verified in `screen.rs`
`screen_factor`), and the transmitted straight-through path travels the same S→R line — so the
transmission term relative to `p̂₀` is **exactly `T(f)`**, and D-05's `diffracted + H_ff·T(f)`
reduces to `h_coh_factor + T(f)` added once in `screen_channel` (native side, pre-conj, one code
point covering SM4/5/6). The min-phase reconstruction is pinned to the **even-mirror cepstral-fold
method on the 105-point band axis** (mirror to 208 samples, real cepstrum, causal fold, DFT back):
numerically verified this session to reproduce a known minimum-phase system's phase to 6.7e-16,
with the load-bearing sign result (D-08): the cepstral fold with numpy-FFT conventions yields the
**ENVI `e^{+jωt}` (lagging) phase**, so the native `e^{−jωt}` filter is `T = |T|·e^{−jφ_cepstral}`
— written as an explicit negative-sine rotation, never `.conj()`.

**Primary recommendation:** implement forest as document-exact SM10 (Eqs. 288–291 + Tables 8/9,
PCHIP interpolation) in a crate-root `forest.rs` applied post-conj in `solve_pair`, and transmission
as a `propagation/transmission.rs` min-phase filter added structurally-gated in `screen_channel`;
validate with the adapted anchor ladder below (the "linear in d" anchor of D-12 is replaced by
SM10's exact zero/floor/on-node anchors — linear-in-d is physically wrong for Nord2000).

## Project Constraints (from CLAUDE.md)

Actionable directives that bind this phase's plans:

1. **Language/edition:** Rust 2024; `envi-engine` is pure math, `#![deny(unsafe_code)]`, deps
   quarantined to **ndarray + num-complex + thiserror only** (enforced by a `cargo tree` check).
   → No FFT crate; the min-phase DFT is hand-rolled (see Don't Hand-Roll — justified exception).
2. **Complex/phase contract:** `H_coh` complex with phase preserved through every operator;
   `P_incoh` separate real channel added only at readout; `F→1 ⇒ P_incoh→0` bit-exact; never
   collapse phase mid-chain.
3. **Time-convention quarantine:** `propagation/` is Nord2000-native `e^{−jωt}`; exactly ONE
   `.conj()` at `transfer::nord_ratio_to_transfer`; grep gate for `.conj()` in `propagation/` stays
   **zero**; write conjugation as explicit `Complex::new(re, -im)` / negative-sine when needed.
4. **Frequency framework:** 105-point 1/12-octave grid; **compare by band index, never nominal Hz**.
5. **Numerics:** f64 throughout; guard singularities/cancellation; validate inputs (finite).
6. **Verify transcriptions against AV 1106/07 page images** — done this session for Eqs. 288–291 and
   Tables 8/9 (pp. 125–127); the plan must re-verify during implementation (project Rule).
7. **Quality gates:** `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `cargo test`
   green; fail-soft FORCE gating (no false Pass); oracle fixtures committed, no Python at test time.
8. **Licensing:** never commit `refs/` PDFs; cite by report + equation number; transcribed table
   *data* in code is established practice (SM7 Tables 6/7 precedent in `submodel7.rs`).
9. **Docs contract at phase close:** module I/O headers + root README + REQUIREMENTS.md updated
   (D-07 for ENG-10's min-phase extension; **also ENG-09's wording** — see Pitfall 1).
10. **Git:** commit only when asked; conventional messages; no CI workflows.

## Architectural Responsibility Map

The "tiers" of this pure-engine phase are the engine's internal convention layers:

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Forest ΔL_s formula (SM10 Eqs. 288–291) | `envi-engine` crate root (`forest.rs`, post-conj side) | — | Real dB math, no complex convention; mirrors `directivity.rs` placement (D-02, D-04) |
| Forest factor application | `solver.rs::solve_pair` (post-conj) | — | Per-path real magnitude factor, exactly like `directivity_gain_db` (D-04) |
| Isolation spectrum type + validation | `envi-engine` (`scene.rs` or `transmission` module) | Harness constructs it (Phase 7 interpolates 1/1→1/12 upstream) | Engine consumes a validated 105-point R(f) (SCN-03 is downstream) |
| Min-phase reconstruction + native T(f) | `propagation/transmission.rs` (native `e^{−jωt}`) | — | The φ sign is native-convention-load-bearing (D-05/D-08); belongs behind the conj boundary |
| Transmission composition | `terrain_effect/mod.rs::screen_channel` (native, pre-conj) | `SolveJob`/`terrain_effect` threading | One code point covering SM4/5/6; keeps sub-models document-exact (D-05, D-10) |
| Oracle fixtures | `tools/nord2000_oracle/gen_forest_fixtures.py`, `gen_minphase_fixtures.py` | committed TOML under `crates/envi-harness/tests/fixtures/oracle/` | Established no-Python-at-test pattern (D-09/D-12) |
| Oracle/anchor tests | `crates/envi-harness/tests/oracle_forest.rs`, `oracle_minphase.rs` (+ engine unit tests) | — | Mirrors `oracle_screen.rs` etc. |

**Explicitly NOT in this phase:** scene objects (Phase 7), forest-crossing geometry / Fresnel-volume
transition Eqs. 292–294 (Phase 9 — the engine consumes a pre-computed scalar `d_m = R_sc`), calc
service wiring (Phase 10), 1/1→1/12 isolation interpolation (Phase 7 SCN-03).

## Standard Stack

### Core (unchanged — the phase adds ZERO dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| ndarray | (workspace-pinned) | tensors | Already the engine's only array dep [VERIFIED: Cargo.toml quarantine check exists] |
| num-complex | (workspace-pinned) | `Complex<f64>` | Frozen contract type |
| thiserror | (workspace-pinned) | typed errors | Existing `PropagationError` pattern |

**No installation step.** The engine dep quarantine (`ndarray + num-complex + thiserror` only,
enforced by a `cargo tree -p envi-engine` check) forbids `rustfft`/`realfft` — and none is needed:
the min-phase DFT is a 208-point one-shot per solve job (~87k flops), trivially done with a naive
O(M²) loop over `Complex<f64>`.

### Dev-tool side (oracle generation only, NOT build/test dependencies)

| Tool | Version | Purpose |
|------|---------|---------|
| Python | 3.13 [VERIFIED: `python -c` this session] | fixture generation only |
| numpy | 2.4.4 [VERIFIED: import check this session] | `np.fft` cepstral min-phase oracle |
| scipy | 1.17.1 [VERIFIED: import check + `PchipInterpolator` import this session] | PCHIP interpolation for the Table-9 forest oracle |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Naive 208-pt DFT (hand-rolled) | `rustfft` | Forbidden by the dep quarantine; naive DFT is exact-enough (O(M²)=43k complex mul) and oracle-matched to ~1e-12 |
| Cepstral fold on mirrored band axis | Resample to linear-f grid + Bode integral | Violates D-06 (locked: Hilbert over the 105-point band axis); needs out-of-band extrapolation policy; no benefit for a deterministic modeling extension |
| PCHIP (monotone cubic) for Table 9 | Catmull-Rom / natural cubic spline | Document says only "cubic interpolation" (ambiguous); PCHIP cannot overshoot the tabulated values → no spurious positive A_e; scipy has an exact reference (`PchipInterpolator`) [ASSUMED interpretation — see Assumptions A1] |

## Package Legitimacy Audit

No new packages are installed in this phase (engine dep quarantine; Python tools already present).

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| — | — | — | — | — | — | No new installs |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

## Finding 1 — The Nord2000 forest law (ENG-09): Sub-Model 10, verified from page images

**Source:** AV 1106/07 rev. 4, §5.19 "Sub-Model 10: Scattering Zones", pp. 124–129. Equations and
tables below were read from the **rendered page images** of pp. 125–127 (pdftotext mangles both
tables), per the project transcription rule. [VERIFIED: AV 1106/07 pp. 125–127 page images]

### The equations (all verified against page images)

```text
Fs   = 1 − k_f·T                                            (Eq. 288)  — coherence coefficient, see Finding 2
T    = Min( (R_sc·nQ / 1.75)², 1 )                          (Eq. 289)
nQ   = 2·a·n″                                               (Eq. 290)
ΔL₁₀ = Max( 1.25·k_f·T·A_e(R_sc), −15 )                     (Eq. 291)
       where A_e(R_sc) = ΔL(h′, α, R′) + 20·log₁₀(8·R′)
```

- `R_sc` — total sound path length **inside** the scattering zone(s), metres (= Σ R_sc,i). This is
  D-01's scalar `d_m`. When the line of sight is blocked, R_sc is measured along the rubber-band
  path over the obstacles (Figure 29) — a **Phase 9 geometry concern**, not engine logic.
- `n″` — density of trees (m⁻²); `a` — mean stem radius (m); `nQ = 2·a·n″` (m⁻¹).
- `k_f` — frequency weighting from **Table 8** as a function of `ka` (k = wave number = 2πf/c,
  a = mean stem radius), **linear interpolation** between nodes [VERIFIED: p. 125 text + table]:

  | ka | 0 | 0.7 | 1 | 1.5 | 3 | 5 | 10 | 20 |
  |----|---|-----|---|-----|---|---|----|----|
  | k_f | 0.00 | 0.00 | 0.05 | 0.20 | 0.70 | 0.82 | 0.95 | 1.00 |

  Above ka = 20, clamp k_f = 1.00 [ASSUMED boundary handling — table ends at 20 and k_f is a
  weighting ≤ 1; see Assumptions A2].
- `h′ = nQ·h` — normalised scatter obstacle height, `h` = **average tree height** (m). NOTE: `h` is
  a required input that D-01's field list omits — see the seam design below.
- `α` — absorption coefficient of the scatter obstacles, "normally in the range 0.1 – 0.4"
  [VERIFIED: p. 126 text].
- `R′ = nQ·R_sc` — normalised effective distance.
- `ΔL(h′, α, R′)` — the 3-D **Table 9** (p. 126–127), parameters (h′, α, R′), "using cubic
  interpolation" [VERIFIED: p. 126 text; the interpolation *scheme details* are not further
  specified — see Assumptions A1]. Full transcription from the page image
  (rows = R′; three h′ blocks × three α columns):

  | R′ | h′=0.01 α=0 | α=0.2 | α=0.4 | h′=0.1 α=0 | α=0.2 | α=0.4 | h′=1 α=0 | α=0.2 | α=0.4 |
  |------|-------|-------|-------|-------|-------|-------|-------|-------|-------|
  | 0.0625 | 6.0 | 6.0 | 6.0 | 6.0 | 6.0 | 6.0 | 6.0 | 6.0 | 6.0 |
  | 0.125 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
  | 0.25 | −7.5 | −7.5 | −7.5 | −6.0 | −7.0 | −7.5 | −6.0 | −7.0 | −7.5 |
  | 0.5 | −14.0 | −14.25 | −14.5 | −12.5 | −13.5 | −14.5 | −12.5 | −13.0 | −14.0 |
  | 0.75 | −18.0 | −18.8 | −19.5 | −17.3 | −18.0 | −19.0 | −16.0 | −16.8 | −17.7 |
  | 1 | −21.5 | −22.5 | −23.5 | −20.5 | −21.6 | −22.8 | −19.3 | −20.5 | −21.3 |
  | 1.5 | −26.3 | −27.5 | −29.5 | −25.5 | −27.2 | −29.0 | −24.0 | −25.5 | −26.3 |
  | 2 | −31.0 | −32.5 | −34.5 | −30.0 | −32.0 | −33.3 | −27.5 | −29.5 | −30.8 |
  | 3 | −40.0 | −42.5 | −45.5 | −37.5 | −40.5 | −42.9 | −34.2 | −36.0 | −37.8 |
  | 4 | −49.5 | −52.5 | −56.3 | −45.5 | −49.5 | −52.5 | −40.4 | −42.8 | −45.5 |
  | 6 | −67.0 | −72.5 | −78.0 | −62.0 | −67.0 | −72.0 | −52.5 | −56.2 | −60.0 |
  | 10 | −102.5 | −113.0 | −122.5 | −94.7 | −103.7 | −112.0 | −78.8 | −84.0 | −89.7 |

  Sanity cross-check on the `+20·log₁₀(8R′)` sign [VERIFIED numerically]: at R′ = 0.0625,
  A_e = 6.0 + 20·log₁₀(0.5) = −0.02 ≈ 0; at R′ = 0.125, A_e = 0.0 + 0.0 = 0 — the correction is
  ~0 where the zone is acoustically thin, then grows negative. The `+` sign is correct; a `−` sign
  would produce nonsense (attenuation at zero thickness).

### What this means vs the locked wording (READ THIS, planner)

1. **There is no `a(f)` in Nord2000.** ΔL_s is NOT linear in d. It is quadratic in `R_sc` while
   `T < 1` (T ∝ R_sc²), then follows the Table-9 + log law, and is **floored at −15 dB** ("ΔL_s has
   been limited downwards to a value of −15 dB", p. 126 [VERIFIED]). TI 386's "A = d·a(f)" and
   REQUIREMENTS ENG-09's wording are the NoizCalc paraphrase of exactly this sub-model (its
   parameter list — density, stem radius, "kp", absorption — matches Eq. 289–291's n″, a, k_f, α
   one-to-one). Implementing a literal per-metre `a(f)` is impossible: no such formula exists in the
   implement-from document.
2. **"kp" ≡ k_f (Table 8).** It is a *tabulated function of ka*, not a free user parameter
   [CITED: AV 1106/07 p. 125 Table 8; TI 386 §Forest parameter list]. Recommended reading of D-01's
   `kₚ` field: drop it as an input; the engine computes k_f from Table 8 (this is precisely D-02's
   "engine owns the formula"). Exact field types are Claude's discretion per CONTEXT.
3. **Nord2000 DOES bound d — answering the roadmap SC1 question:** via T-saturation
   (T = 1 for R_sc ≥ 1.75/nQ) and the −15 dB floor, NOT via the ISO 10/20/200 m regimes. The
   deferred-ideas note stands: do not import the ISO clamp table; the document's own Max/Min/floor
   IS the bounding.
4. **The needed inputs are:** `R_sc` (= d_m), `n″`, `a`, `α`, **and `h` (average tree height)** for
   `h′ = nQ·h`. D-01's field list lacks `h`; SCN-04 (Phase 7 forest object) already carries height,
   so adding `height_m` to `ForestCrossing` is downstream-consistent. Flag: without `h` the engine
   cannot evaluate Table 9.
5. **Low-frequency exact zero:** k_f = 0 for ka ≤ 0.7, i.e. for f ≤ 0.7·c/(2π·a) — with c = 340.348
   and a = 0.1 m that is f ≤ 379 Hz: forests do nothing below ~380 Hz for 20 cm stems. This is a
   hard analytic anchor (exactly 0 dB, no tolerance).
6. **Defaults** (mean density / stem radius / absorption): the AV document defines these as case
   inputs and gives **no defaults**; only α's "normally 0.1–0.4" range is stated. TI 386 says the
   Nord2000 defaults in NoizCalc are "a best fit to the ISO 9613-2 specification" without numbers.
   Defaults are a **Phase 7 UI concern** (SCN-04) — the engine takes explicit parameters. Do not
   invent defaults in this phase; record the open question for Phase 7 (see Open Questions Q3).

### Two-channel application (D-03/D-04, unchanged by the richer formula)

ΔL_s ≤ 0 is a per-band real level effect, additive in dB at the Eq. (1) level
(`L = L_W + ΔL_d + ΔL_a + ΔL_t + ΔL_s + ΔL_r` [VERIFIED: p. 15]). Define `A_forest(f) = −ΔL_s(f) ≥ 0`
to match D-03's sign framing; the solver applies `10^(−A/20)` to `H_coh` (arg untouched) and
`10^(−A/10)` to `P_incoh_abs`, structurally identical to `directivity_gain_db` in
`solve_pair` (`solver.rs:194–197`). `F→1 ⇒ P_incoh→0` is unaffected (a real scale of an
exactly-zero channel stays exactly zero).

## Finding 2 — Fs (Eq. 288): a Nord2000 forest effect OUTSIDE the locked scope (planner must dispose)

`Fs = 1 − k_f·T` is a **coherence reduction** that Nord2000 multiplies into the overall coherence
coefficient `F = Ff·FΔν·Fc·Fr·Fs` (Eq. 110, p. 52 [VERIFIED: text extraction]; also Eq. 118 p. 58
and the screen-model F₂/F₃/F₄, Eq. 177 p. 74 — "In case of propagation through a scattering zone
Fs is ... Otherwise Fs = 1"). It models the forest *decorrelating* the ground-reflected rays —
which shifts energy from the coherent channel into `p_incoh` inside the terrain effect.

**Tension:** D-03's rationale says forest is "excess attenuation (not decorrelation — explicitly
NOT routed into the incoherent channel)". That is true of ΔL_s itself, but Nord2000 *additionally*
applies Fs; Eq. (1)'s narrative even calls out this exact interaction ("the effect of terrain ΔLt
and ... ΔLs ... may interact ... as a decrease in coherence introduced by the latter" [VERIFIED:
p. 15]). Implementing Fs was never discussed in CONTEXT and would touch `propagation/` coherence
paths (a pre-conj change), beyond the locked application design.

**Recommended disposition (planner):** implement ONLY ΔL_s in Phase 5 (the locked scope), and
record Fs as a **documented deferred item** with a seam note: `CoherenceInputs` already carries a
caller-multiplied factor (`f_delta_nu`, applied in the SM1/SM2 paths), so a future
`Fs`-shaped multiplicative coherence input can land without structural change; the screen engine's
F₂/F₃/F₄ (`screen.rs::run_four_path`) would also need the multiply. Defer to the phase that wires
real forest paths (Phase 9/10), with a user check-in — this mirrors the Phase-4 "directional phase
seam" deferral discipline. Impact of deferring: through-forest interference dips are modeled too
deep (too coherent); no FORCE case exercises forests, so no validation pressure exists yet.

## Finding 3 — Minimum-phase reconstruction (ENG-10, D-06/D-08/D-09): algorithm, sign, oracle

### 3a. The algorithm (even-mirror cepstral fold on the band axis)

The 105-point grid is **uniform in log-frequency** (f[i] = 1000·G^((i−64)/12)), so the "105-point
band axis" of D-06 is a uniformly-sampled axis and the discrete Hilbert transform / real-cepstrum
machinery applies **directly to the band-index samples — no resampling**. Specification:

```text
Inputs:  R[n] dB, n = 0..104 (finite, ≥ 0; opaque is None and never reaches here — D-10)
1. ln_mag[n] = −R[n]·ln(10)/20                      # = ln|T(f_n)|
2. Even mirror to M = 208:   y[n] = ln_mag[n]            (n = 0..104)
                             y[n] = ln_mag[208 − n]      (n = 105..207)
   (mirror = even symmetry around BOTH endpoints n=0 and n=104; removes the
    periodic wrap discontinuity — the standard end-effect treatment; no zero-padding)
3. Real cepstrum:  c[m] = IDFT_M{y}[m]              # real & even by construction
4. Causal fold:    ĉ[0] = c[0];  ĉ[m] = 2·c[m] (1 ≤ m ≤ 103);  ĉ[104] = c[104];  ĉ[m] = 0 (m > 104)
5. Z = DFT_M{ĉ};   φ_cep[n] = Im(Z[n]),  n = 0..104
   (Re(Z[n]) reproduces ln_mag[n] — a free internal consistency check)
DFT sign convention: forward DFT uses e^{−j2πnm/M} (the numpy.fft convention) — this is
load-bearing for the sign result below and MUST be stated in the module docs.
```

`φ_cep` is exactly D-06's `−H{ln|T|}` realized as the periodic discrete Hilbert transform on the
mirrored band-axis circle. Engine implementation: naive O(M²) complex DFT with `Complex<f64>`
(M = 208 → ~43k complex multiplies, negligible; no FFT crate needed, dep quarantine intact).

**Numerically verified this session** (numpy, M = 208): for the known minimum-phase system
H = 1 + 0.5·e^{−jω}, the recipe reproduces arg H to **6.7e-16**; a constant R gives φ ≡ 0 to
3.7e-16; φ is exactly linear in R (doubling R doubles φ, error 0.0). [VERIFIED: numeric experiment
in this session]

### 3b. The sign (D-08 verify item) — first-principles derivation the verifier can check

- **ENVI convention (`e^{+jωt}`):** delay τ ⇔ phase factor `e^{−jωτ}` (frozen in `transfer.rs`).
  A causal minimum-phase filter therefore has **lagging (negative-going) phase where the magnitude
  falls**, and group delay `τ_g = −dφ/dω > 0`. One-pole check: `H(jω) = 1/(1 + jω/ω_c)` has
  `φ = −arctan(ω/ω_c) ≤ 0`. ✓
- **The cepstral recipe above produces exactly this ENVI-convention phase.** Verified numerically:
  for `H = 1 + 0.5e^{−jω}` (a causal min-phase system in the `e^{−jωn}`/z-transform convention that
  pairs with `e^{+jωt}`), φ_cep matched `arg H` including sign (φ_cep[10] = −0.1003, lagging). So:
  `φ_envi(f_n) = φ_cep[n]`.
- **Nord2000-native convention (`e^{−jωt}`):** delay τ ⇔ `e^{+jωτ}` (verified in `screen.rs`:
  `p̂₀ = e^{+jωτ_SR}/R_SR`). Native quantities are the complex conjugates of ENVI quantities, so the
  native transmission filter is
  `T_native(f_n) = 10^(−R[n]/20) · e^{−j·φ_cep[n]}` — i.e. **negate the cepstral phase** on the
  native side, written explicitly as `Complex::new(cos(φ), −sin(φ))`-style construction (or
  `sin(−φ)`), **never `.conj()`** (grep gate).
- **Round-trip check:** the single conj at `nord_ratio_to_transfer` maps
  `conj(T_native) = |T|·e^{+jφ_cep} = T_envi` — a lagging, causal, positive-group-delay filter in
  the output convention, consistent with the free-field `e^{−jωτ}` delay factor it multiplies
  alongside. For a mass-law-like *rising* R(f), `ln|T|` falls with band index ⇒ φ_envi trends
  negative ⇒ positive group delay. This is the testable causality property (see Validation).

### 3c. What the band-axis choice means (honesty note)

Operating the Hilbert transform on the log-frequency-uniform band axis (locked, D-06) yields the
minimum-phase counterpart of `ln|T|` **as a periodic function of band index**, not the analog
(linear-frequency) Bode minimum phase — the two differ by the axis warping and the band-limited
window. Consequences the planner should encode as expectations, not bugs:
- A **flat R(f) gives φ ≡ 0 exactly** (pure real attenuation) — a clean property test.
- φ is **exactly linear in R** (Hilbert linearity) — a clean property test.
- An analytic one-pole anchor (`R = 10·log₁₀(1+(f/f_c)²)` vs `φ = −arctan(f/f_c)`) will agree in
  **sign, monotonic trend, and rough magnitude at mid-band**, but NOT to tight tolerance — the
  exact numeric pin is the committed numpy oracle (D-09), plus the sign/causality property tests.
  Do not write a tight-tolerance analytic phase anchor; it will be flaky by construction.
- Edge bands (lowest/highest ~half-octave) carry the largest reconstruction end-effects even with
  mirroring; probe property tests at mid-band indices.

### 3d. Numerical guards

- `IsolationSpectrum` constructor validates: all 105 values **finite** and `≥ 0` dB (reject NaN/Inf
  with a typed error — a NaN would poison the whole tensor; R < 0 means transmission gain through a
  passive partition, unphysical). Upper bound: none needed mathematically (φ stays finite for any
  finite R; ln|T| = −R·ln10/20 is finite) — the −∞ hazard exists only for R = ∞, which is
  structurally unrepresentable per D-10 (`None` = opaque).
- `ln|T| = −∞` can NEVER reach the transform: opaque is `None` (D-10, structural), and the
  constructor rejects non-finite R.

### 3e. The committed scipy/numpy oracle (D-09)

`tools/nord2000_oracle/gen_minphase_fixtures.py`, following the established pattern (`common.py`
header discipline, sha256 provenance, committed TOML under
`crates/envi-harness/tests/fixtures/oracle/`):

```python
# numpy.fft-based real-cepstrum minimum phase — mirrors the engine spec exactly.
import numpy as np
def min_phase_envi(r_db: np.ndarray) -> np.ndarray:      # r_db: shape (105,)
    ln_mag = -r_db * np.log(10.0) / 20.0
    y = np.concatenate([ln_mag, ln_mag[-2:0:-1]])        # even mirror, M = 208
    c = np.fft.ifft(y)                                   # real cepstrum
    fold = np.zeros(len(y)); fold[0] = 1.0
    fold[1:len(y)//2] = 2.0; fold[len(y)//2] = 1.0
    z = np.fft.fft(c * fold)
    return z.imag[:105]                                  # φ_envi (e^{+jωt}, lagging)
    # native e^{−jωt} phase = −φ_envi (the engine negates on the native side)
```

Fixture inputs: at least (a) a flat R = 30 dB (expect φ ≡ 0), (b) a mass-law ramp
(e.g. R = 20 + 20·log₁₀(f/f₀)-shaped, clipped ≥ 0), (c) a measured-looking bumpy spectrum, and
(d) a narrow notch (stress case). Store R[105] + φ_envi[105]; Rust test tolerance ~1e-9 rad
(naive-DFT vs FFT round-off is ~1e-13; margin for libm differences).

**Oracle-independence caveat (mandatory statement, D-09):** the oracle implements the *same*
mirror/fold recipe as the engine — it cross-checks the implementation (DFT, fold indices, sign),
not the *choice* of recipe. The recipe's correctness as a minimum-phase reconstruction is pinned
independently by the analytic verification in 3a/3b (known min-phase system reproduced to 6.7e-16)
and by the causality property tests.

Note: `scipy.signal.hilbert` on the raw 105 samples is NOT equivalent (no mirroring, odd N,
different wraparound) — do not use it for the oracle; use the numpy recipe above so engine and
oracle share the exact sample layout.

## Finding 4 — Where the transmission term composes (ENG-10, D-05): read from the shipped code

### 4a. Reference-frame analysis (the key correctness fact)

Every screen sub-model result is normalized **relative to the free-field direct path `p̂₀`**:
`screen.rs::screen_factor` computes `p̂_SCR = p̂₁/p̂₀` with `p̂₀ = e^{+jωτ_SR}/R_SR` (Nord-native
outgoing phase, the S→R straight line), and `GroundResult::h_coh_factor` for SM4/5/6 is
`p̂_SCR · (ground combination)` — a dimensionless complex factor that later multiplies `H_ff` at the
transfer boundary (`solver.rs`: `H_coh = H_ff · nord_ratio_to_transfer(h_coh_factor)`).

The transmitted straight-through path travels **the same S→R line** (ray direction preserved,
ENG-10), so its field is `p̂₀·T(f)` and its factor **relative to `p̂₀` is exactly `T(f)`**.
D-05's `diffracted_field + H_ff·T(f)` therefore reduces, in the native factor frame, to:

```text
h_coh_factor_semi = h_coh_factor_opaque + T_native(f_band)
```

— one complex addition. No geometry, no extra kernel calls, dimensionally exact. After the single
conj this is precisely `H_ff·(conj(h_opaque) + T_envi)` = diffracted + `H_ff·T(f)`. ✓

### 4b. The composition point: `screen_channel` (recommended), not inside `run_four_path`

`terrain_effect/mod.rs::screen_channel` (lines 499–580) is where SM4/5/6 + SM7 assemble the screen
branch's `GroundResult`. Adding the transmission there — after the sub-model call, alongside the
SM7 energy add — covers all three screen classes at ONE code point, keeps `screen.rs` sub-models
document-exact, stays inside `propagation/` on the native side pre-conj (D-05's letter and spirit),
and makes D-10's structural gate a single `match`:

```rust
// screen_channel, after `base` and `sm7_energy` are computed:
let h = match transmission {
    // ENVI extension (ENG-10+): straight-through leakage T(f) relative to p̂₀,
    // native e^{−jωt}. Coherent channel ONLY (deterministic min-phase filter).
    Some(t) => base.h_coh_factor + t[band],   // t: &[Complex<f64>; N_BANDS]
    None => base.h_coh_factor,                // exact existing opaque path (D-10)
};
Ok(GroundResult::from_channels(h, base.p_incoh + sm7_energy))
```

Facts the planner needs about the surrounding machinery:

- **`P_incoh` untouched by transmission:** the min-phase T is deterministic and fully coherent; it
  adds to `h_coh_factor` only. SM7's scattered energy add and the `(1−F²)` residuals are unchanged.
  `F→1 ⇒ P_incoh→0` holds with transmission enabled (transmission never writes `p_incoh`).
- **Per-band index, not per-Hz:** `terrain_effect`'s band loop currently iterates
  `for &f in axis.centres.iter()` without an index; the transmission lookup needs the band index —
  switch to `enumerate()` (compare-by-band-index rule).
- **Precompute once per call:** φ_min needs the whole 105-point spectrum at once; construct the
  native `TransmissionFilter` (the `[Complex<f64>; N_BANDS]` table) **once before the band loop**
  (mirroring the `FlatChannel::from_profile` precompute pattern), only when isolation is `Some`.
- **Eq. 332 blend interplay (document, don't "fix"):** transmission joins the *screen branch*
  result, so in the marginal transition zone (0 < r_scr1 < 1) it is weighted by r_scr1 along with
  the rest of the screen branch. Correct: the no-screen branch already contains the full direct
  field; leakage only exists where the screen does.
- **`ScreenClass::Flat` + isolation Some:** contradictory input (no partition on the path).
  Recommend a **typed error** (`PropagationError::IsolationWithoutScreen` or similar), following
  the Pitfall-9 `WeatherScreenNotImplemented` refuse-to-be-silently-wrong precedent. The FEATURES
  note "do not silently draw forests that do nothing" applies in spirit to partitions too.
- **Weather + screen is already a typed error** (`WeatherScreenNotImplemented`), so
  isolation+weather+screen is unreachable — no new interaction to design.
- **SM6 double screens:** ONE isolation spectrum per job in this phase — it represents the total
  crossed partition stack; per-partition composition (T₁·T₂) is a Phase 9 path concern (records
  under D-11's façade-selection logic). Document in the seam's rustdoc.
- **No `.conj()` anywhere:** the native T is constructed with an explicit negative-sine phase; the
  grep gate over `propagation/` stays at zero.

### 4c. Threading the seam (signatures to extend)

| Item | Change |
|------|--------|
| `SolveJob` (`solver.rs`) | add `pub isolation: Option<&'a IsolationSpectrum>` (and `pub forest: Option<ForestCrossing>`) — on the **promoted `envi_engine::solver`** (one solve path, two callers; ROADMAP coordination flag) |
| `terrain_effect(...)` (`terrain_effect/mod.rs`) | add an `isolation: Option<&IsolationSpectrum>` parameter (8th arg, or fold `refraction`+`isolation` into a small options struct — planner's choice); builds `TransmissionFilter` once when `Some` |
| `screen_channel(...)` | add `transmission: Option<&[Complex<f64>; N_BANDS]>` + the band index |
| `IsolationSpectrum` | new engine type: 105-point R(f) dB, validating constructor (finite, ≥ 0) — natural home `scene.rs` (beside `BandSpectrum`) or the transmission module |
| `propagation/transmission.rs` (new) | `min_phase` cepstral fold + `TransmissionFilter::from_isolation(&IsolationSpectrum) -> [Complex<f64>; N_BANDS]` (native convention, sign-documented) |
| Call sites to update | `solver.rs::solve_pair`, harness `build_terrain_inputs`/`run_case` path, engine `terrain_effect` tests, harness oracle tests (`oracle_screen.rs`, `oracle_refraction.rs`, `terrain.rs`, …) — mechanical `None`/default additions |

**Opaque-limit regression leverage:** the existing committed screen-oracle fixtures
(`oracle_screen.rs` + `screen_thin.toml`) already pin the opaque numbers — if the `None` path's
bits move, those tests fail. Add on top the explicit `f64::to_bits` equality test (D-12): same
thin-screen case through `terrain_effect` with `isolation: None` vs the pre-extension entry
(keep the old-arity wrapper, or hand-assemble the old call) — assert `H_coh`/`P_incoh_abs`
bit-equality per band.

## Finding 5 — The forest solver seam (ENG-09, D-01/D-04): read from `solver.rs`

`solve_pair` (`solver.rs:161–209`) already applies per-path post-conj factors in a fixed order:
directivity magnitude (both channels), then directional phase (coherent only). Forest slots in as a
third optional block, same shape:

```rust
// SolveJob gains:
pub forest: Option<ForestCrossing>,   // Copy struct; None ⇒ bit-identical path

// ForestCrossing (recommended fields — D-01 modulo the kp finding + the h requirement):
pub struct ForestCrossing {
    pub d_m: f64,            // R_sc: through-forest path length (pre-computed upstream)
    pub density_per_m2: f64, // n″
    pub stem_radius_m: f64,  // a
    pub absorption: f64,     // α (Table 9 axis; document the 0.1–0.4 normal range)
    pub height_m: f64,       // h (average tree height) — REQUIRED for h′ = nQ·h
}

// In solve_pair's band loop (after directivity, before push):
if let Some(fc) = job.forest.as_ref() {
    // ΔL_s ≤ 0 dB from SM10 (Eqs. 288–291); c0 from coh (same speed the terrain uses).
    let dls = forest_delta_l(job.axis.centres[i], fc, job.coh.c0)?;
    hc *= 10f64.powf(dls / 20.0);   // real factor: arg(H_coh) untouched (D-03)
    pa *= 10f64.powf(dls / 10.0);   // energy factor on P_incoh_abs
}
```

Implementation notes:
- `forest_delta_l` lives in a crate-root `forest.rs` (mirrors `directivity.rs`: engine-owned pure
  math on the post-conj side, real dB — no complex convention involvement, so it does not belong in
  `propagation/`; D-04 keeps application out of `propagation/` too). It embeds Table 8 (linear
  interp) and Table 9 (PCHIP — see Assumptions A1) as consts, exactly like `submodel7.rs` embeds
  Tables 6/7 (established transcription-in-code precedent).
- Per-band cost is a table lookup + a handful of flops; no precompute needed (but a
  `[f64; N_BANDS]` precompute mirroring the balloon-eval pattern is fine if the planner prefers —
  the frequency dependence enters ONLY through k_f(ka)).
- Constructor/validation: reject non-finite or negative `d_m`, `density`, `stem_radius`,
  `height`; α clamped or validated to [0, 1] (typed error — same discipline as
  `ground_impedance`'s σ ≤ 0 rejection).
- `d_m = 0` (or density 0) ⇒ T = 0 ⇒ ΔL_s = 0 exactly — but `None` remains the structural identity
  (bit-exactness guaranteed by not touching the numbers at all).

## Architecture Patterns

### System flow with the two extensions

```text
                     SolveJob { …, forest: Option<ForestCrossing>,
                                 isolation: Option<&IsolationSpectrum> }
                                        │
        ┌───────────────────────────────┴───────────────────────────────┐
        │ solve_pair                                                     │
        │   H_ff = direct_path(...)                 (ENVI side)          │
        │   te   = terrain_effect(profile, …, isolation)                 │
        │            │  (native e^{−jωt} side, inside propagation/)      │
        │            ├─ isolation Some? → TransmissionFilter::from_iso   │
        │            │     (min-phase cepstral fold, native sign −φ_cep) │
        │            ├─ band loop: flat/SM3 channels (unchanged)         │
        │            └─ screen_channel: SM4/5/6 + SM7                    │
        │                 └─ Some(t) → h_coh_factor += t[band]  (D-05)   │
        │                    None    → EXACT existing path      (D-10)   │
        │   ── the ONE .conj(): nord_ratio_to_transfer ──                │
        │   H_coh = H_ff · conj(h_coh_factor);  P = |H_ff|²·p_incoh      │
        │   directivity gain / phase (existing, post-conj)               │
        │   forest Some? → ΔL_s = forest_delta_l(f, fc, c0)   (D-04)     │
        │        H_coh *= 10^(ΔL_s/20);  P *= 10^(ΔL_s/10)    (D-03)     │
        └────────────────────────────────────────────────────────────────┘
                                        │
                              TensorSink (unchanged)
```

### Recommended module layout

```text
crates/envi-engine/src/
├── forest.rs                        # NEW: SM10 Eqs. 288–291, Tables 8+9 consts, forest_delta_l()
│                                    #      (crate root, like directivity.rs — post-conj-side math)
├── solver.rs                        # SolveJob += forest, isolation; solve_pair applies both
├── scene.rs                         # IsolationSpectrum (validated 105-pt R(f))  [or in transmission.rs]
└── propagation/
    ├── transmission.rs              # NEW: min_phase cepstral fold + TransmissionFilter (native sign)
    └── terrain_effect/mod.rs        # terrain_effect(+isolation), screen_channel(+transmission)
```

### Pattern 1: Optional extension with a bit-identical absent path (the house pattern)

**What:** every new capability is `Option<_>`; `None` never constructs the new math and leaves
existing outputs bit-for-bit unchanged. **When:** both extensions (D-10 forest `None`, isolation
`None`). **Precedents in-tree:** `directivity_phase_rad` (phase-free balloon ⇒ `arg(H_coh)`
bit-identical), homogeneous `SoundSpeedProfile` (|ξ|<1e-6 ⇒ Phase-2 bit-identical), `f_delta_nu`
(sA=sB=0 ⇒ factor exactly 1).

### Pattern 2: Tabulated document data as consts + interpolation (SM7 precedent)

`submodel7.rs` embeds Tables 6/7 as `[[f64; 10]; 8]` consts transcribed from page images, with
edge-clamped bilinear interpolation and the transcription provenance in the rustdoc. Tables 8/9
follow the identical pattern (Table 8: 1-D linear; Table 9: 3-D, see Assumptions A1).

### Anti-patterns to avoid

- **Implementing a literal per-metre `a(f)`** from the TI 386 paraphrase — no such formula exists
  in the implement-from document; SM10 is the law (Pitfall 1).
- **Importing the ISO 9613-2 distance-clamp table** from TI 386 — explicitly deferred/out of scope.
- **A `.conj()` in `propagation/`** to fix the min-phase sign — negate the sine explicitly (D-08).
- **`R = f64::INFINITY` as "opaque"** — `None` is the opaque state (D-10); ∞ would send −∞ into
  the cepstrum.
- **Comparing bands by nominal Hz** anywhere in the new tables/filters — band index only.
- **`scipy.signal.hilbert` as the oracle** — different sample layout than the engine's
  mirror/fold; use the numpy recipe that matches the engine sample-for-sample.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Min-phase reference values | a second independent Rust implementation | committed numpy-fft oracle fixtures | Established D-09 pattern; no test-time Python |
| Table-9 3-D interpolation scheme from scratch | an ad-hoc spline | PCHIP semantics matched to `scipy.interpolate.PchipInterpolator` in the oracle | Monotone (no overshoot past tabulated values ⇒ no spurious positive A_e); scipy gives an exact cross-reference |
| Forest defaults | invented density/radius numbers | explicit parameters only; defaults deferred to Phase 7 (SCN-04) | No verified source for defaults exists in-repo (see Q3) |

**Justified hand-roll (inverted case):** the 208-point DFT for the cepstral fold IS hand-rolled
(naive O(M²) with `Complex<f64>`), because the engine dep quarantine forbids FFT crates and the
size makes O(M²) free. This mirrors the Phase-3 decision to hand-roll the 3×3 LSQ rather than pull
in nalgebra. Document the equivalence `naive DFT ≡ numpy.fft` (same sign convention) in the module
header.

## Common Pitfalls

### Pitfall 1: The "A = d·a(f)" paraphrase vs Sub-Model 10 (spec mismatch)
**What goes wrong:** planning tasks around a linear-in-d per-metre attenuation that doesn't exist;
writing D-12's "linear in d" anchor, which SM10 provably violates (T ∝ d² then saturates; −15 dB
floor).
**Why:** REQUIREMENTS ENG-09 and the roadmap inherited TI 386's UI-level paraphrase.
**How to avoid:** implement Eqs. 288–291 exactly; replace the linearity anchor with the SM10
anchors (below); update ENG-09's wording at phase close alongside D-07's ENG-10 update (same
close-out discipline).
**Warning signs:** any task text mentioning "dB per metre" or an `a(f)` return value.

### Pitfall 2: The min-phase sign flipped once (or twice)
**What goes wrong:** transmitted leakage with anti-causal (leading) phase after the conj; coherent
sums with the diffracted field interfere wrongly. Magnitudes look fine — only interference details
are wrong, so it survives casual review.
**How to avoid:** pin the DFT sign convention in writing (`e^{−j2πnm/M}` forward = numpy); engine
native phase = **−φ_cep**; test the round-trip property: after `nord_ratio_to_transfer`, a rising
R(f) must give non-increasing φ_envi trend (positive group delay) at mid-band. The analytic
verification pair (H = 1 + 0.5e^{−jω} reproduced to 6.7e-16, φ[10] = −0.1003) is the committed
sanity anchor for the recipe itself.
**Warning signs:** oracle matches but the group-delay property test fails — the fixture and engine
share a wrong sign; the property test is the independent check.

### Pitfall 3: Cepstral fold off-by-one (M/2 element)
**What goes wrong:** `ĉ[104]` (the M/2 = 104th quefrency of the 208-point sequence) doubled or
zeroed instead of kept ×1 — small phase bias everywhere, worst at band edges.
**How to avoid:** fold weights exactly `{1, 2×(1..M/2−1), 1 at M/2, 0 above}`; the flat-R ⇒ φ≡0 and
linearity properties catch most fold bugs; the oracle catches the rest.

### Pitfall 4: Table 9 out-of-range R′ and the +6 dB row
**What goes wrong:** (a) R′ > 10 needs extrapolation the document doesn't define; (b) naive cubic
interpolation near the +6.0 row can overshoot and make A_e > 0 ⇒ ΔL_s > 0 = a forest that
*amplifies*.
**How to avoid:** clamp R′ to [0.0625, 10] **consistently in both the table lookup and the
20·log₁₀(8R′) term** (clamping only one side manufactures spurious attenuation as R′→0); use
monotone (PCHIP) interpolation; property-test `ΔL_s ≤ 0 + ε` over a parameter sweep and
`ΔL_s ≥ −15` exactly. Note the floor engages early anyway (for k_f·T = 1, at R′ ≈ 3 already
1.25·A_e < −15), so the clamp's practical exposure is small. [ASSUMED boundary handling — A3]

### Pitfall 5: Forgetting `h` (tree height) in the seam
**What goes wrong:** `h′ = nQ·h` is un-evaluable; an implementer "fixes" it by hardcoding h′ = 1.
**How to avoid:** `ForestCrossing.height_m` is a required field (this research's D-01 amendment);
validation rejects h ≤ 0.

### Pitfall 6: Transmission accidentally touching `p_incoh` or SM7
**What goes wrong:** adding `|T|²` into the incoherent channel "for energy conservation" — breaks
the D-06 coherent-filter design and the `F→1 ⇒ P_incoh→0` contract.
**How to avoid:** T adds to `h_coh_factor` only; the two-channel test with `force_f = 1` (screen
test hook `submodel4_eval(..., Some(1.0))`) must give `p_incoh == 0` with transmission enabled.

### Pitfall 7: The R→0 corner of the additive leakage model
**What goes wrong:** R ≡ 0 dB (fully transparent partition) gives `h = ĥ_diff + 1` — direct field
restored PLUS the diffracted residue, i.e. slightly more than free field. This is inherent to the
locked additive composition (D-05), not a bug to "fix" with renormalization.
**How to avoid:** document as a model property (rustdoc + README); it is benign for physical
partitions (R ≥ ~10 dB ⇒ leakage ≤ 0.32, and the diffracted term dominates behind real screens).
Do NOT add ad-hoc energy normalization — that would silently violate the opaque-limit contract.

### Pitfall 8: Signature-change fallout
**What goes wrong:** `terrain_effect` gains a parameter and a dozen call sites (engine tests,
harness oracle tests, `build_terrain_inputs`) break or — worse — get a hasty `Some` default.
**How to avoid:** all existing call sites pass `None` explicitly (opaque/no-forest); grep for
`terrain_effect(` and enumerate the sites in the plan; consider keeping the old-arity wrapper as
the opaque-regression comparator.

## Code Examples

### Table 8 (k_f) — verified transcription, linear interpolation, edge-clamped

```rust
// Source: AV 1106/07 p. 125, Table 8 (verified against the page image 2026-07-09).
const KA_AXIS: [f64; 8] = [0.0, 0.7, 1.0, 1.5, 3.0, 5.0, 10.0, 20.0];
const KF_VALS: [f64; 8] = [0.00, 0.00, 0.05, 0.20, 0.70, 0.82, 0.95, 1.00];
// ka = 2π·f·a / c0;  ka ≥ 20 ⇒ k_f = 1.0 (clamp, [ASSUMED A2]);  ka ≤ 0.7 ⇒ exactly 0.
```

### SM10 core (Eqs. 289–291) — shape of `forest_delta_l`

```rust
// Source: AV 1106/07 p. 125–126, Eqs. 289/290/291 (verified against page images).
let n_q = 2.0 * fc.stem_radius_m * fc.density_per_m2;          // Eq. 290
let t = ((fc.d_m * n_q) / 1.75).powi(2).min(1.0);              // Eq. 289
let ka = 2.0 * PI * f_hz * fc.stem_radius_m / c0;
let kf = table8_kf(ka);                                        // linear interp
if kf == 0.0 || t == 0.0 { return Ok(0.0); }                   // exact zero (low f / no crossing)
let h_norm = n_q * fc.height_m;                                // h′ = nQ·h
let r_norm = (n_q * fc.d_m).clamp(0.0625, 10.0);               // R′, clamped consistently [A3]
let a_e = table9_delta_l(h_norm, fc.absorption, r_norm)        // PCHIP, [A1]
        + 20.0 * (8.0 * r_norm).log10();                       // Eq. 291 (inner)
Ok((1.25 * kf * t * a_e).max(-15.0))                           // Eq. 291 (floor)
```

### Native min-phase filter construction (`propagation/transmission.rs`)

```rust
// φ_cep = Im(DFT{fold(IDFT{ln|T| mirrored to 208})}) — the ENVI e^{+jωt} lagging phase
// (numerically verified against a known min-phase system to 6.7e-16 this session).
// NATIVE (e^{−jωt}) side flips the sign EXPLICITLY — never `.conj()` (grep gate):
let (s, c) = phi_cep[n].sin_cos();
t_native[n] = Complex::new(mag * c, mag * (-s));   // |T|·e^{−jφ_cep}
```

### Hand anchor for the forest oracle (computable without interpolation)

```text
Pick band 64 (exactly 1000 Hz), a = 3·c0/(2π·1000) = 0.16249 m  ⇒ ka = 3 (Table-8 node, k_f = 0.70)
n″, d chosen so R′ = nQ·d = 1 (Table-9 node) and nQ·d ≥ 1.75… pick nQ·d = 1 ⇒ T = (1/1.75)² = 0.32653…
h chosen so h′ = nQ·h = 0.1 (node); α = 0 (node).
A_e = −20.5 + 20·log₁₀(8) = −2.4382 dB
ΔL_s = 1.25 · 0.70 · 0.32653 · (−2.4382) = −0.6967 dB   (exact, no interpolation anywhere)
```

## Validation & Testing Strategy (D-12 ladder, adapted to the verified physics)

No FORCE cases exist for either physics — the ladder is anchors + oracles + regressions, per the
Phase 2–3 pattern. `workflow.nyquist_validation` is `false` in config; this section is the phase's
testing contract.

### Forest (ENG-09)

| Rung | Test | Kind | Where |
|------|------|------|-------|
| F1 | ka ≤ 0.7 ⇒ ΔL_s = 0 **exactly** (assert == 0.0) for all bands below the ka threshold | analytic anchor | engine unit test (`forest.rs`) |
| F2 | On-node hand anchor: ΔL_s(1 kHz, ka=3, R′=1, h′=0.1, α=0) = −0.696776… dB (derivation above), tolerance 1e-12 | analytic anchor | engine unit test |
| F3 | Floor: large d ⇒ ΔL_s = −15.0 exactly; monotone non-increasing in d until the floor | property | engine unit test |
| F4 | ΔL_s ≤ ε and ≥ −15 over a randomized parameter sweep (density, a, α, h, d); all finite | property | engine unit test |
| F5 | Committed scipy oracle: `gen_forest_fixtures.py` (PCHIP Table 9 + linear Table 8) vs engine over the 105 bands, ≥3 parameter sets; tolerance ~1e-9 (same-transcription caveat stated) | oracle | `oracle_forest.rs` + TOML fixture |
| F6 | Solver: `forest: Some` scales `H_coh` magnitude by 10^(ΔL_s/20) with `arg` unchanged (<1e-12) and `P_incoh_abs` by 10^(ΔL_s/10); `forest: None` bit-identical (`to_bits`) | contract | solver test (mirror `directivity_gain_scales_magnitude_only…`) |
| F7 | F→1 ⇒ P_incoh→0 stays bit-exact with forest enabled (zero-turbulence case: P scaled from exact 0 stays exact 0) | contract | solver test |

### Transmission / min-phase (ENG-10)

| Rung | Test | Kind | Where |
|------|------|------|-------|
| T1 | Flat R ⇒ φ ≡ 0 (< 1e-12) — pure real attenuation | property | engine unit test (`transmission.rs`) |
| T2 | Linearity: φ(2R) = 2·φ(R) exactly (assert to 1e-12) | property | engine unit test |
| T3 | Causality/sign (the D-08 verify item): rising (mass-law) R ⇒ ENVI-side φ non-increasing trend at mid-band (positive group delay after the conj); native side is the negation | property (first-principles) | engine unit test |
| T4 | `Re(Z)` reconstruction: the fold's real part reproduces ln_mag to 1e-12 (internal consistency) | property | engine unit test |
| T5 | Committed numpy oracle: `gen_minphase_fixtures.py` (recipe in Finding 3e) — R sets (flat / ramp / bumpy / notch), φ_envi per band, tolerance ~1e-9 | oracle | `oracle_minphase.rs` + TOML fixture |
| T6 | **Opaque-limit bit-exact regression (D-10):** thin-screen case via `terrain_effect` with `isolation: None` vs the pre-extension call — `h_coh_factor`, `p_incoh`, `delta_l_db` equal via `to_bits` per band; PLUS existing committed screen fixtures stay green untouched | regression (permanent) | engine test + existing `oracle_screen.rs` |
| T7 | Transmission direction: `Some(R=15 dB)` raises the deep-shadow bands vs opaque (level increases), and the added term's magnitude equals 10^(−R/20) when isolating T against a forced-zero diffracted field | property | engine test |
| T8 | Two-channel: with `force_f = Some(1.0)` (screen test hook) and transmission enabled, `p_incoh == 0.0` exactly — T never touches the incoherent channel | contract | engine test |
| T9 | `ScreenClass::Flat` + `isolation: Some` ⇒ typed error (not silent no-op) | contract | engine test |
| T10 | `IsolationSpectrum` constructor rejects NaN/Inf/negative R with typed errors | validation | engine unit test |

### Suite hygiene

- All new tests run with `cargo test` (no refs needed — these are oracle/anchor tests, not FORCE).
- FORCE forest cases (if any exist in the workbooks) remain capability-gated Skips — this phase
  does NOT flip a FORCE capability (no false Pass; road cases' skip reasons are unaffected).
- Quality gates after each plan: `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`,
  `cargo test`; plus the conj grep gate (`.conj()` count in `propagation/` == the one in
  `transfer.rs` documentation, i.e. zero in `propagation/`).

## Runtime State Inventory

Not a rename/refactor/migration phase — omitted by design. (No stored data, service config,
OS-registered state, secrets, or build artifacts are touched; the phase adds pure-math modules and
optional seam fields.) None — verified by phase scope (engine-only additive extension).

## State of the Art

| Old Approach (pre-phase) | Current Approach (this phase) | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Drawn forests would be silently inert (FEATURES.md gap) | SM10 ΔL_s engine math behind a `SolveJob` seam | Phase 5 | Phase 7 forest objects become physically meaningful |
| Screens strictly opaque | Optional min-phase transmission leakage (ENVI extension) | Phase 5 | Phase 7 semi-transparent screens/façades (SCN-01/02) become computable |
| ENG-09 spec text "A = d·a(f)" | SM10 Eqs. 288–291 (the actual Nord2000 law) | Phase 5 close-out doc update | REQUIREMENTS/README wording corrected with citation |
| ENG-10 spec text "real 10^(−R/20)" | complex min-phase T(f) (D-06/D-07) | Phase 5 close-out doc update | Documented ENVI extension, same discipline as directional phase |

**Deprecated/outdated:** the ISO 9613-2 distance-regime table (TI 386) — recognized and excluded.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Table 9's "cubic interpolation" realized as tensor-product **PCHIP** (monotone cubic) along R′, with the 3-node h′ axis interpolated on **log₁₀(h′)** (nodes are decades: 0.01/0.1/1) and α linear/pchip on its 3 nodes — the document does not specify the scheme; only 3 nodes exist in h′/α so a true cubic is impossible there [ASSUMED] | Finding 1, Don't Hand-Roll | Interpolated ΔL_s differs from other implementations by ~1 dB off-node; anchors on nodes are unaffected; oracle shares the scheme (same-transcription caveat) |
| A2 | k_f clamped to 1.00 above ka = 20 and the table's linear interpolation extended edge-clamped [ASSUMED — table ends at 20; k_f is a weighting that plateaus at 1] | Finding 1 | High-frequency/large-stem ΔL_s slightly off if the real intent differed; k_f ∈ [0.95, 1.0] bounds the error |
| A3 | R′ clamped to [0.0625, 10] consistently in table lookup AND the 20·log₁₀(8R′) term; beyond R′=10 the −15 floor dominates for k_f·T ≳ 0.19 [ASSUMED boundary handling] | Finding 1, Pitfall 4 | Very sparse forests (tiny nQ) with huge crossings could under-attenuate; exposure small, floor limits everything to −15 dB anyway |
| A4 | `log` in Eq. 291 is log₁₀ (dB context; consistent with the A_e ≈ 0 sanity check at the R′ = 0.0625/0.125 nodes) [VERIFIED numerically via the node sanity check, notation ASSUMED] | Finding 1 | If natural log, A_e values shift ~2.3×; the node sanity check (A_e≈0 at both small-R′ nodes) only works with log₁₀ — strong evidence |
| A5 | TI 386's "factor kp" ≡ AV 1106/07's k_f (Table 8) — 1:1 parameter-list match; no free "kp" input exists in the document [ASSUMED mapping, CITED sources both read this session] | Finding 1 | If NoizCalc's kp is a user scaling knob, ENVI simply doesn't expose one (document-faithful is the project's stated priority) |
| A6 | Deferring Fs (Eq. 288) is acceptable for this phase (no FORCE forest cases; decorrelation not in locked scope) [ASSUMED disposition — needs user/planner sign-off] | Finding 2, Q1 | Through-forest dips too coherent until wired; documented deferred item mitigates |

## Open Questions (RESOLVED at plan time — 2026-07-09)

> **All four resolved and reflected in the executable plans (05-01/02/03):**
> **Q1 (Fs, Eq. 288) → DEFER with a documented seam** (`forest.rs` header + `deferred-items.md`),
> per D-03's locked "excess attenuation, not decorrelation" scope, mirroring the Phase-4
> directional-phase-seam discipline. **Q2 (`terrain_effect` signature) → 8th positional
> `Option` arg** (05-03). **Q3 (default forest params) → carried to Phase 7 SCN-04.**
> **Q4 (multi-partition composition) → Phase 9 GEOX.** Dispositions accepted by the orchestrator
> for this autonomous run and verified by the plan-checker.

1. **Fs (Eq. 288) disposition — needs an explicit decision at plan time.**
   - What we know: Nord2000 multiplies `Fs = 1 − k_f·T` into every coherence coefficient when a
     path crosses a scattering zone (Eqs. 110/118/177 — verified); D-03's locked design covers
     ΔL_s only and its rationale ("not decorrelation") conflicts with implementing Fs now.
   - What's unclear: whether the user wants document-completeness (implement Fs, touching
     `propagation/` coherence paths) or locked-scope fidelity (defer).
   - Recommendation: **defer with a documented seam** (CoherenceInputs multiplicative factor;
     screen F₂/F₃/F₄ multiply), recorded in deferred-items.md + README, mirroring the
     directional-phase-seam discipline. Revisit at Phase 9 when real forest crossings exist.
2. **`terrain_effect` signature style:** 8th positional `Option<&IsolationSpectrum>` vs folding
   `refraction` + `isolation` into a `TerrainOptions` struct. Mechanical either way;
   planner's choice (options struct scales better for the eventual Fs/weather-screen work).
3. **Nord2000 default forest parameters** (n″, a, h) for Phase 7's SCN-04 UI: no verified source
   in-repo; TI 386 states defaults exist ("best fit to ISO 9613-2") without numbers. Out of scope
   for the engine (explicit params only); carry to Phase 7 research.
4. **Multi-partition transmission on one path** (e.g. SM6 with two semi-transparent screens): this
   phase takes ONE isolation spectrum per job (the total). Whether Phase 9 composes R_total
   upstream or the seam later grows per-partition spectra — revisit with GEOX (D-11 records the
   same downstream-selection principle for façades).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| cargo/rustc | build + tests | ✓ | cargo 1.96.0 | — |
| Python 3 | oracle fixture generation (dev-time only) | ✓ | 3.13 | — |
| numpy | min-phase oracle | ✓ | 2.4.4 | — |
| scipy (PchipInterpolator) | forest Table-9 oracle | ✓ | 1.17.1 | — |
| refs/AV1106-07-rev4.pdf | equation verification during implementation | ✓ (local, sha-pinned) | rev. 4 | tests fail-soft if refs absent (established) |
| pdftoppm (page-image rendering) | transcription re-verification | ✓ (winget poppler 25.07, full path needed — not on PATH) | 25.07.0 | pdftotext for text, images via the winget path used this session |

**Missing dependencies with no fallback:** none.

## Security Domain

ASVS Level 1, scoped to a pure-math library phase (no auth/session/crypto/network surface):

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | — |
| V3 Session Management | no | — |
| V4 Access Control | no | — |
| V5 Input Validation | yes | Typed-error validation on `ForestCrossing` (finite, non-negative; α∈[0,1]) and `IsolationSpectrum` (finite, ≥0) — NaN/Inf must never enter the tensor (existing `compose_gain` non-finite-rejection precedent, threat T-04-01-03) |
| V6 Cryptography | no | — |

### Known threat patterns for this phase

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| NaN/Inf poisoning of the tensor via unvalidated R(f)/forest params | Tampering (data integrity) | validating constructors, typed `PropagationError`s, finiteness property tests (F4) |
| Silent wrong physics (sign flip surviving review) | Tampering (correctness) | D-08 first-principles sign tests (T3), grep conj gate, oracle fixtures with sha256 provenance |
| Panic on degenerate input (DoS of a future service caller) | DoS | typed errors, never `unwrap` on data (house rule); `ScreenClass::Flat`+isolation typed error (T9) |
| Committing copyrighted document content | Info disclosure / licensing | tables transcribed as data with report+page citation only (SM7 precedent); refs/ stays git-ignored |

## Sources

### Primary (HIGH confidence)
- `refs/AV1106-07-rev4.pdf` pp. 124–128 — §5.19 Sub-Model 10: Eqs. 288/289/290/291, Tables 8 and 9
  **verified against rendered page images this session** (pdftoppm 150 dpi); §5.3.3 (p. 16–17)
  scattering-zone input variables (a, n″, α, h); Eq. 1 (p. 15) ΔL_s additivity; Eq. 110 (p. 52)
  F = Ff·FΔν·Fc·Fr·Fs; Eqs. 118/177 Fs entry points.
- Shipped code read this session: `crates/envi-engine/src/solver.rs` (SolveJob seam, post-conj
  application order), `transfer.rs` (conventions, the ONE conj), `propagation/terrain_effect/mod.rs`
  (Eq. 332 composition, `screen_channel`, GroundResult two-channel contract),
  `propagation/terrain_effect/screen.rs` (`screen_factor` p̂₀ reference frame, SM4/5/6 engines,
  `force_f` test hook), `submodel7.rs` (table-transcription precedent), `freq.rs` (grid law),
  `directivity.rs` (optional-extension precedent), `tools/nord2000_oracle/common.py` +
  `gen_screen_fixtures.py` (oracle pattern).
- Numeric verification this session (numpy 2.4.4): cepstral-fold recipe vs known min-phase system
  (6.7e-16), flat-R ⇒ φ≡0, exact linearity; A_e node sanity check for the +20·log₁₀(8R′) sign.

### Secondary (MEDIUM confidence)
- `docs/references/dbaudio-ti386-1.6-en.md` §Forest (L196–212) — the NoizCalc parameter framing
  ("density, stem radius, kp, absorption") and the ISO-variant distance table (excluded);
  §Building/Wall reflection-loss framing.
- `.planning/phases/05-…/05-CONTEXT.md`, `REQUIREMENTS.md`, `STATE.md` — locked scope and history.

### Tertiary (LOW confidence)
- Standard DSP literature knowledge (Oppenheim & Schafer real-cepstrum minimum-phase
  reconstruction) as background for the recipe — but the recipe itself was verified numerically
  this session, so nothing rests on recall alone.

## Metadata

**Confidence breakdown:**
- Forest physics (Eqs. 288–291, Tables 8/9): HIGH — verified against page images; only
  interpolation-scheme details and boundary clamps are [ASSUMED] (A1–A3), all node-exact anchors
  unaffected.
- Min-phase algorithm + sign: HIGH — numerically verified end-to-end this session, incl. the
  D-08 sign in both conventions.
- Composition points / signatures: HIGH — read from the shipped code, precedents identified.
- Fs disposition & defaults: MEDIUM — the finding is verified; the *disposition* needs planner/user
  sign-off (Q1, Q3).

**Research date:** 2026-07-09
**Valid until:** stable (implement-from document is frozen; codebase findings valid until Phase 5 lands)
