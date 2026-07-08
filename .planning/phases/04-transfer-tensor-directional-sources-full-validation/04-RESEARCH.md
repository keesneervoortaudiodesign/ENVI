# Phase 4: Transfer Tensor, Directional Sources & Full Validation - Research

**Researched:** 2026-07-08
**Domain:** Complex transfer-tensor output contract, directional multi-sub-source composition, Nord2000 road emission model, FORCE full-suite validation, NoiseModelling cross-validation
**Confidence:** HIGH (tensor/MAC/directivity architecture, FORCE gap inventory) / MEDIUM (emission-model specifics — the authoritative coefficient report is not freely downloadable; see Open Questions)

## Summary

Phase 4 turns the engine's frozen forward contract into a real artifact: `TransferTensor = Array3<Complex<f64>>` indexed `[sub_source, receiver, freq]` (already typedef'd in `crates/envi-engine/src/transfer.rs`), fed by a promoted `envi_engine::solver` that evaluates each directional sub-source independently through the existing `direct_path` + `terrain_effect` chain, streams receiver-axis chunks through a `TensorSink` trait, and supports two readout laws: the **coherent complex MAC** `p[r,f] = Σ_s H[s,r,f]·G_s(f)` (OUT-03, the conditioning use case) and the **incoherent Nord2000 energy sum** (AV 1106/07 Annex A — mandatory for road sources and FORCE comparison). The tensor design is low-risk: memory math is trivial (2 520 B per sub-source×receiver for the H_coh + P_incoh pair), chunking over the receiver axis is natural because every receiver already gets its own cut-plane profile, and the MAC-equals-recompute identity can be made **bit-exact by construction** if conditioning is composed into a single complex gain per band before one multiplication.

The heavy half of the phase is VAL-02. Codebase archaeology shows the "full FORCE suite" needs substantially more than the emission model: **Sub-model 3 (non-flat terrain, §5.12) is a typed hard error** covering 24 of the 62 straight-road cases; **segmented ground + refraction is a typed error** (`calc_eq_ssp_ground` exists, oracle-tested, but not wired into `terrain_effect`); **the screen channel consumes no refraction at all** (wind variants of screen groups); **Sub-model 8** (multiple ground reflections under strong downwind), **Sub-model 10** (forest scattering — cases 121-124, currently scoped to Milestone-2 ENG-09), and **Sub-model 11** (reflection effect — city-street façades, curved-road embankment) are entirely absent; and the curved-road/city-street/yearly-average workbooks are loader placeholders. The emission model itself is well characterized from freely-verifiable sources (sub-sources at 0.01/0.30/0.75 m + optional 3.5 m, 1 m horizontal offset toward the receiver, 80/20 rolling/propulsion energy splits, per-1/3-octave coefficient tables, frequency-dependent vertical directivity on all sources + horn-effect horizontal directivity on the lowest) — but the **coefficient tables live in Jonasson SP Rapport 2006:12, which is not freely downloadable** (verified across four search paths). That is the phase's single acquisition blocker and needs a human checkpoint. A strong mitigation exists: the FORCE `.xls` `dL` column gives per-band `LE − LE_freefield`, so `LE − dL` yields the emission+pass-by-integration free-field spectrum per case — an authoritative end-to-end anchor for the emission pipeline that is independent of the propagation model.

VAL-03 (NoiseModelling cross-validation) should follow the project's committed-fixture oracle pattern: run NoiseModelling v6 (Java, GPLv3 — run the binary, commit its **outputs**, never port its code) once against 2-3 minimal scenes, commit per-band results as TOML fixtures with provenance, and gate only the legitimately comparable quantities — geometric divergence (identical formula, `20·lg d + 11`) and ISO 9613-1 air absorption (same underlying standard) at the 8 octave centres 63 Hz–8 kHz, which fall exactly on the 1/12-octave grid by band index. Screen insertion loss and ground effect are **documented-expected-delta reports, not pass/fail gates** — Nord2000's Hadden-Pierce wedge and complex-impedance ground are different models from CNOSSOS-EU's empirical formulas, and equality would actually be suspicious. Note: **Java is not installed on this machine** — fixture generation needs a one-time JRE install (and the CRITICAL environment rule applies: never force-close Java processes).

**Primary recommendation:** Expand the phase from 3 to 5 plans (see Plan Split). Build 04-01 (tensor + solver promotion + MAC) and 04-02 (balloons + emission model, with the SP 2006:12 human checkpoint and the `LE − dL` anchor) as the foundation; land the straight-road FORCE pass (04-03: SM3 + refraction wiring + comparator) before the 3D groups (04-04: curved/city/yearly + SM11), and keep VAL-03 as a small fixture-oracle plan (04-05).

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| OUT-01 | Complex transfer value per (directional sub-source × receiver × 1/12-oct point) | Solver architecture (§Architecture Patterns 1-2); existing `TransferSpectrum`/`terrain_effect` chain produces exactly this per pair |
| OUT-02 | Dense `Complex<f64>` `H[sub_source, receiver, freq]`, frequency-contiguous, single + multi receiver | `TransferTensor` typedef already frozen row-major (transfer.rs:49); layout test exists; extend with paired `P_incoh: Array3<f64>` |
| OUT-03 | Recompute spectra via MAC `p[r,f] = Σ_s H[s,r,f]·G_s(f)`, no propagation re-run | Pattern 4: coherent readout; bit-exact identity via single-composition rule (Pitfall 7) |
| OUT-04 | Per-source filtering: complex per-frequency gain G_s(f) | Conditioning composed into one `Complex<f64>` per band on the ENVI-convention side (Pattern 5) |
| OUT-05 | Per-source delay: phase ramp e^{−j2πfτ} | Same conditioning composition; sign consistent with frozen e^{+jωt}/e^{−jωτ} convention (Pattern 5) |
| OUT-06 | Chunk/stream tensor within a memory budget | `TensorSink` trait + receiver-axis chunking; memory math + 256 MiB budget recommendation (§Tensor Store) |
| SRC-02 | Directivity ΔL(θ,φ,f) attached to a sub-source | `DirectivityBalloon` per-band spherical grid + bilinear interpolation + rotation matrix (Pattern 6) |
| SRC-03 | Multiple directional sub-sources evaluated independently into the tensor | Solver iterates (s,r) pairs; incoherent composition per AV 1106/07 Annex A for readout (Pattern 4) |
| SRC-04 | Directivity internally as per-band spherical balloons | Balloon struct as the single representation; road analytic directivities sampled onto balloons at construction (Pattern 6) |
| SRC-04/road | Road source model (3 sub-source heights, Jonasson) | §Emission Model: heights/splits/directivities/corrections cited; coefficient acquisition ladder |
| VAL-02 | Full FORCE suite passes within the standard's tolerance | §FORCE Gap Inventory: complete case-group → capability map, tolerances from Env. Project 1335 Ch. 6, comparator requirements |
| VAL-03 | Cross-validate shared sub-effects vs NoiseModelling CNOSSOS | §NoiseModelling Cross-Validation: committed-fixture oracle, comparable quantities + expected-delta posture, offline/no-credential |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

These are binding on every recommendation below (all were verified against the current tree):

- **Engine dep quarantine:** `envi-engine` = `ndarray + num-complex + thiserror` ONLY (`cargo tree -p envi-engine` gate). All I/O in `envi-harness`. ⇒ the tensor store, solver, sink trait, and directivity balloons must be pure math in the engine; `.xls` loaders, emission-coefficient files, and NoiseModelling fixtures live in the harness.
- **Time-convention quarantine:** e^{+jωt} frozen; Nord2000-native math e^{−jωt} inside `propagation/`; **exactly one `.conj()` at `transfer::nord_ratio_to_transfer`**; grep gate `\.conj()` over `propagation/` = 0. ⇒ conditioning (G_s, delay ramps) applies on the ENVI-convention side (transfer/tensor layer), never inside `propagation/`.
- **Two-channel contract:** `H_coh` complex with live phase; `P_incoh` real, readout-only; `L = L_W + 10·lg(|H_coh|² + |H_ff|²·P_incoh)`; `F→1 ⇒ P_incoh→0` bit-exact. ⇒ the tensor is a **pair** of stores.
- **Frequency framework:** 105-point 1/12-octave grid; compare by **band index**, never nominal Hz. FORCE 27-band comparison = `third_octave_pick` (every 4th point).
- **f64 throughout; guarded numerics** (Δτ cancellation, ξ clamps, z₀ ≥ 0.001 m).
- **refs/ hygiene:** FORCE .xls + AV PDFs git-ignored, SHA-256-pinned in `refs/refs.sha256`, fetched by `refs/fetch.sh`; never commit; verify transcriptions against PDF page images; the authoritative `.xls`/PDF wins over any summary.
- **Fail-soft tests:** `Skipped(requires: …)` — never a false Pass. FORCE flips to numeric Pass only when honestly computable.
- **Quality gates:** `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `cargo test`, `#![deny(unsafe_code)]` in the engine.
- **No CI workflows; commit/push only when asked; English everywhere; no GPL source translation (NoiseModelling = ideas + executable oracle only).**
- **CRITICAL environment rule:** never close/kill VS Code or Java/JVM/language-server processes (relevant to VAL-03's NoiseModelling runs).

**Note:** No `04-CONTEXT.md` exists (no discuss-phase ran). Constraints derive from ROADMAP.md Phase-4 success criteria, REQUIREMENTS.md, CLAUDE.md, and the frozen Phase 1-3 contracts.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Tensor store + `TensorSink` trait + readout laws (OUT-01/02/03/06) | `envi-engine` (`tensor.rs` / `solver.rs`) | — | Pure ndarray math; Milestone-2 Phase 10 hard-gates on these being **public engine API**, not harness-private (ROADMAP Phase-10 coordination flag) |
| Conditioning (filter G_s(f), delay ramp) (OUT-04/05) | `envi-engine` (tensor/readout layer) | harness (test drivers) | Complex multiply on the ENVI-convention side; no propagation involvement |
| Directivity balloons + rotation (SRC-02/04) | `envi-engine` (`directivity.rs`) | harness (loads/constructs road balloons) | Pure math (grid + interpolation + 3×3 rotation); no I/O |
| Road emission model (spectra, corrections, sub-source expansion) | `envi-harness` (`emission/`) | — | Coefficient tables are **data** (transcribed constants); it is a *source* model, not propagation; keeps engine quarantine clean. Its output is plain `SubSource`s + balloons + `G_s(f)` |
| Pass-by integration (LE/LAeq,24h/LAmax), A-weighting | harness (`emission/passby.rs`, `compare`) — A-weight formula may sit in `envi-engine::freq` | — | Integration weights are case/harness concerns; A-weighting is a closed IEC 61672 formula (pure math, reused by Milestone-2 WEB-11 server-side) |
| Sub-models 3/8/10, segmented+screen refraction wiring | `envi-engine::propagation` | — | Core AV 1106/07 propagation math behind the existing typed-error seams |
| Sub-model 11 (reflection effect) + 3D path building (curved/city) | engine (SM11 kernel) + harness (image-source path construction from Coordinates sheets) | — | The kernel is §5.20 math; building reflection paths from workbook geometry is case I/O |
| FORCE loaders (curved/city/yearly), comparator (Ch. 6 rules) | `envi-harness` (`cases/`, `compare.rs`) | — | Established calamine label-anchored pattern (xls.rs) |
| NoiseModelling fixtures (VAL-03) | `tools/noisemodelling_oracle/` (generation) + harness tests (consumption) | — | Mirrors the scipy-oracle pattern: no Java at test time |

## Standard Stack

### Core (no additions — the phase ships on the existing stack)

| Library | Version (workspace) | Purpose | Why Standard |
|---------|--------------------|---------|--------------|
| ndarray | already pinned in Cargo.toml | `Array3<Complex<f64>>` tensor + `Array3<f64>` P_incoh store | Frozen forward contract; row-major default = frequency-contiguous [VERIFIED: transfer.rs layout test] |
| num-complex | already pinned | `Complex<f64>` MAC arithmetic | Existing engine dep |
| thiserror | already pinned | Typed errors for new solver/sink failure modes | Existing engine dep |
| calamine | already pinned (harness) | Curved/city/yearly `.xls` loaders (Coordinates + Met. statistics sheets) | Proven label-anchored pattern in `cases/xls.rs` |
| serde/toml (harness) | already pinned | NoiseModelling fixtures, emission-coefficient data files | Existing oracle-fixture pattern |
| libtest-mimic (harness) | already pinned | Dynamic FORCE case runner | Existing |

**Installation:** none — **recommend ZERO new dependencies for Phase 4.** Specifically:
- **No `rayon` yet.** OUT-06 is a *memory* budget, not a speed target; sequential receiver-chunk iteration satisfies it. Rayon-parallel grid solves are Milestone-2 Phase 10 (GRID-02) and belong in the service/harness tier anyway (the engine quarantine forbids rayon in `envi-engine`).
- **No linalg crate** (03-03 precedent D-08: hand-rolled 3×3). Balloon rotation is a hand-rolled 3×3 matrix.
- **No SOFA/CLF parser crates** — SRC-04 needs only the internal balloon representation this milestone; file-format import is FUT-05.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `TensorSink` trait + in-memory chunks | memmap2-backed file store now | Milestone-2 Phase 10 territory; adds a dep + I/O to the wrong crate; the trait seam gives Phase 10 its extension point without paying now |
| Balloon grid + bilinear interpolation | Spherical harmonics coefficients | SH is elegant but wrong for MVP: CLF/SOFA/BEM ingest (the SRC-04 rationale) is grid-natural, error analysis is simpler, and Nord2000 road directivities are smooth enough that a grid is exact-enough (Pitfall 8) |
| Harness-side emission model | Engine-side `emission` module | Violates the spirit of the quarantine (source data ≠ propagation math) and drags coefficient-table data into the pure crate; NoiseModelling's emission/propagation module split is the proven template |

## Package Legitimacy Audit

**No new packages are recommended for this phase.** All work lands on dependencies already in the workspace lockfile (ndarray, num-complex, thiserror, calamine, serde, toml, libtest-mimic, approx). If the planner nonetheless adds a package (e.g., `rayon` against this research's recommendation), it must run the package-legitimacy gate (`cargo search` + registry age/downloads/repo check) before the install task.

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

## Architecture Patterns

### System Architecture Diagram

```
                    ┌─────────────────────────── envi-harness (I/O tier) ───────────────────────────┐
FORCE .xls ──► cases/loaders ──► CaseDefinition ──► emission/ (Jonasson model)                      │
(straight/curved/                                    │  L_W,rolling(f,v,cat) + L_W,propulsion(f,v)   │
 city/yearly +                                       │  corrections: surface(DAC12), temp(15°C)      │
 Coordinates,                                        ▼                                               │
 Met. statistics)                    sub-source expansion: per source point (179 @ 1°)              │
                                     × heights {0.01, 0.30 | 0.75 m} × balloons (horn/vertical)     │
                                                     │                                               │
                                     per-pair geometry builder: oblique cut-plane profile           │
                                     (perpendicular profile stretched 1/cosθ), src/rcv, coh, met    │
                                                     ▼                                               │
┌──────────────────────────── envi-engine (pure-math tier) ─────────────────────────────┐          │
│  solver::solve(jobs, &mut sink):                                                       │          │
│    for each receiver chunk:                                                            │          │
│      for each (sub_source s, receiver r):                                              │          │
│        direct_path → H_ff(f)  ──┐                                                      │          │
│        terrain_effect → h_coh_factor(f), p_incoh(f)   (SM1/2/[3]/4-7 + refraction)     │          │
│        H_coh[s,r,f] = H_ff · nord_ratio_to_transfer(h_coh_factor)   ← THE one conj     │          │
│                     · 10^{ΔL_balloon(dir_sr, f)/20}     (real directivity factor)      │          │
│      sink.put_chunk(H_coh chunk, P_incoh chunk)   ← TensorSink trait (Phase-10 seam)   │          │
│                                                                                        │          │
│  readout (two laws):                                                                   │          │
│   coherent MAC  : p[r,f] = Σ_s H_coh[s,r,f]·G_s(f)          (OUT-03, conditioning)     │          │
│   incoherent    : e[r,f] = Σ_s |G_s|²·(|H_coh|² + |H_ff|²·P_incoh)   (Annex A, roads)  │          │
└────────────────────────────────────────────────────────────────────────────────────────┘          │
                                                     │                                               │
                        pass-by integration: LE(f)=10lg Σ_i (t_i/t₀)·10^{L_i/10};                   │
                        LAeq,24h = LAE + 10lg N − 10lg 86400;  LAmax = max_i LA(i)                   │
                                                     ▼                                               │
                        compare: Ch.6 comparator (1 dB overall, 1 dB/band, dip-shift rule)          │
                        + NoiseModelling fixture checks (divergence/air-abs equality gates)          │
└────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Recommended Module Layout

```
crates/envi-engine/src/
├── tensor.rs            # TensorPair store (H_coh Array3<Complex>, P_incoh Array3<f64>),
│                        #   TensorSink trait, InMemorySink, chunk math, readout laws
├── solver.rs            # SolveJob (the Phase-9/10 PropagationPath seam), solve() loop
├── directivity.rs       # DirectivityBalloon (per-band spherical ΔL grid), rotation, eval
├── freq.rs              # + a_weight_db(f) (IEC 61672 closed form; reused by M2 WEB-11)
└── propagation/
    ├── submodel3.rs     # §5.12 non-flat terrain (behind the existing typed-error seam)
    ├── submodel8.rs     # §5.17 multiple ground reflections (if FORCE needs it — see A6)
    ├── submodel10.rs    # §5.19 forest scattering (scope decision — see Open Q3)
    └── submodel11.rs    # §5.20 reflection effect kernel (city street / curved road)
crates/envi-harness/src/
├── emission/
│   ├── mod.rs           # RoadSource: category, speed, surface, temperature → SubSources+balloons
│   ├── coefficients.rs  # Jonasson tables (transcribed data; provenance comments per value)
│   └── passby.rs        # source-point discretization, time weights, LE/LAeq/LAmax
├── cases/
│   ├── xls.rs           # + curved/city/yearly sheet parsers (Coordinates, Met. statistics)
│   └── …
├── weighting.rs         # (or engine freq.rs) A/C-weight application at band centres
└── compare.rs           # + Ch.6 comparator: overall-level gate, per-band gate, dip-shift rule
tools/noisemodelling_oracle/   # scene defs + run instructions + fixture generator (Java, one-time)
```

### Pattern 1: `TensorSink` trait — the Phase-10 seam (OUT-06)

**What:** The solver never allocates the full tensor; it fills fixed-size receiver-axis chunks and hands them to a sink.
**Why:** ROADMAP Phase 10 explicitly warns that shipping the solver/sink private to the harness forces a second refactor of freshly validated code. A trait keeps the engine free of I/O while letting `envi-store` (M2) implement a file-backed sink later.

```rust
// envi-engine (design sketch — pure math, no I/O)
pub trait TensorSink {
    /// Receive one receiver-axis chunk: h_coh[s, r_local, f], p_incoh[s, r_local, f],
    /// where r_local spans [r_offset, r_offset + chunk_len).
    fn put_chunk(
        &mut self,
        r_offset: usize,
        h_coh: ndarray::ArrayView3<'_, Complex<f64>>,
        p_incoh: ndarray::ArrayView3<'_, f64>,
    ) -> Result<(), SinkError>;
}
```

Chunk sizing: `chunk_receivers = floor(budget_bytes / (n_sub × N_BANDS × 24))` (24 B = 16 B `Complex<f64>` + 8 B `f64`). The in-memory sink tracks a **high-water-mark byte counter** so the memory-budget test asserts accounting structurally rather than sampling RSS (portable, deterministic).

### Pattern 2: Memory footprint (OUT-06) — the actual numbers

- Per (sub_source, receiver): 105 × 16 B = **1 680 B** (`H_coh`) + 105 × 8 B = **840 B** (`P_incoh`) = **2 520 B**.
- FORCE straight-road case: 179 source points × 2 heights = 358 sub-sources × 1 receiver ≈ **0.9 MB** — trivial, no chunking needed.
- Success-criterion-5 synthetic set: 100 000 receivers × 3 sub-sources ≈ **756 MB** total ⇒ must stream. With a **256 MiB budget** (recommended stated budget) and 4 096-receiver chunks × 3 subs = **31 MB/chunk**, working set stays far under budget; the full tensor is *never* resident.
- Readout for the large set must also be chunked (fold `Σ_s` per chunk) — don't materialize `p[r,f]` for 100k receivers either (100k × 105 × 16 B = 168 MB is fine, but do it chunked anyway for the pattern's sake).

### Pattern 3: `SolveJob` — geometry stays outside the engine

The engine cannot build road geometry (no GIS/case logic in the quarantine). The harness constructs, per (sub_source, receiver): the cut-plane `TerrainProfile`, positions, `CoherenceInputs`, and `Option<SoundSpeedProfile>`; the solver just runs the frozen chain. **This struct is the Phase-9 `PropagationPath` coordination seam** (ROADMAP flag) — name and shape it deliberately:

```rust
pub struct SolveJob<'a> {
    pub sub_source: usize,          // tensor row
    pub receiver: usize,            // tensor column
    pub profile: &'a TerrainProfile,
    pub src: [f64; 3], pub rcv: [f64; 3],
    pub atmosphere: &'a Atmosphere,
    pub coh: CoherenceInputs,
    pub weather: Option<&'a SoundSpeedProfile>,
    pub directivity: Option<(&'a DirectivityBalloon, Rotation3)>, // evaluated at dir(src→rcv)
}
```

### Pattern 4: Two readout laws — coherent MAC vs Annex A incoherent sum

**What:** AV 1106/07 Annex A (p. 165) is explicit: composite/moving sources are sums of **incoherent** point sources — "the sound pressure levels of the sub-sources are added incoherently." The FORCE references embed that law. OUT-03's coherent MAC is the *conditioning* contract (correlated sub-sources, the M2 loudspeaker use case). Both must exist on the same tensor:

```rust
// Coherent (OUT-03): p[r,f] = Σ_s H_coh[s,r,f] · G_s(f)          → L = 20·lg|p|  (+ incoh channel)
// Incoherent (Annex A / FORCE): e[r,f] = Σ_s |G_s(f)|²·(|H_coh[s,r,f]|² + |H_ff[s,r,f]|²·P_incoh[s,r,f])
//                                                                 → L = 10·lg e
```

**When to use:** FORCE/road readout = incoherent, always. Coherent MAC = OUT-03 tests + M2 conditioning.
Note the incoherent law needs `|H_ff|` per (s,r) — carry it either as a third small store or (cheaper) fold `|H_ff|²·P_incoh` into the stored `P_incoh` at fill time so the stored incoherent channel is already "absolute" (recommended: store `P_incoh_abs[s,r,f] = |H_ff|²·p_incoh`; then `F→1 ⇒ 0` is preserved bit-exact and the readout needs only two stores).

### Pattern 5: Conditioning composition — bit-exact MAC ≡ recompute (OUT-03/04/05)

**What:** Filter gain and delay compose into **one** complex gain per band *before* any multiplication:
`G_s(f) = 10^{L_W(f)/20} · Ĝ_filter(f) · e^{−j2πfτ_s}` — with `f` the exact `FREQ_AXIS.centres` value.
**Why bit-exact matters:** success criterion 2 says "matches a full recompute to numerical identity." f64 multiplication is deterministic; if MAC and "recompute with conditioning folded into the source" both evaluate `H · G_composed` with the same composition order, results are **bit-identical** (`assert_eq!` on bits), not merely ≈. If instead one path computes `(H·G₁)·G₂` and the other `H·(G₁·G₂)`, they differ in the last ulp and the test weakens to an epsilon. Freeze the order: **compose G first, multiply once.**
**Sign check:** delay = `e^{−j2πfτ}` is consistent with the frozen convention (outgoing phase e^{−jωτ}); a delayed source composes exactly like extra path travel time. Conditioning applies to ENVI-convention `H_coh` (post-conj side) — it never enters `propagation/` (grep gate stays 0).

### Pattern 6: Directivity balloons (SRC-02/03/04)

**What:** One representation for everything (CLF/SOFA/BEM common denominator, per REQUIREMENTS SRC-04):

```rust
pub struct DirectivityBalloon {
    /// ΔL dB, indexed [azimuth, polar, band]; equal-angle grid, e.g. 5° (73 × 37 × 105).
    grid: ndarray::Array3<f64>,
    // + angular resolutions; poles stored as single duplicated rows
}
// eval: direction (unit vec, source-local frame) → (az, pol) → bilinear interp → ΔL(f) per band
// rotation: hand-rolled 3×3 matrix applied to the src→rcv unit vector BEFORE frame lookup
```

- Applied as a **real** per-band factor `10^{ΔL/20}` multiplying `H_coh` (and scaling `P_incoh_abs` by `10^{ΔL/10}`) at tensor-fill time. Phase untouched — directivity is a magnitude pattern.
- Road analytic directivities (horn effect, vertical car-body screening, heavy-vehicle horizontal) are **sampled onto balloons at construction**; a unit test asserts sampling+interpolation error < 0.05 dB against the analytic form (they are smooth low-order trig functions — a 5° grid is far inside that budget).
- Success criterion 3's rotation test: balloon with a known lobe; evaluate at a fixed receiver; rotate the lobe away; assert the level drops (direction + magnitude sign, by band index).
- 105-band grid: balloons are defined on the same `FREQ_AXIS` (no per-balloon frequency resampling in the engine; the harness resamples imported data at construction — M2/FUT concern).

### Pattern 7: Pass-by geometry — the oblique-profile stretch

For the straight road, terrain is constant parallel to the road, so the cut-plane profile from a source point at immission angle θ to the receiver is the **perpendicular profile with horizontal distances stretched by the oblique factor** (heights, impedances, roughness, screen heights unchanged): if the perpendicular span is `d⊥` and the oblique horizontal span is `d⊥/cos θ`, scale every profile `x` by `1/cos θ` about the road-side origin. This follows geometrically from lateral invariance (it is not an approximation). Segment time weight: `t_i = ℓ_i / v`, `ℓ_i = d⊥·(tan(θ_i+½°) − tan(θ_i−½°))` along the source line. [ASSUMED: this is the standard Nord2000-road implementation practice; the geometric derivation is exact for laterally-invariant terrain — verify the type-case convention against the `.xls` `dL` anchor, which will expose any discretization mismatch immediately.]

### Anti-Patterns to Avoid

- **Coherently summing road sub-sources for FORCE.** Creates phantom interference between the 0.01 m and 0.30 m sources and between source points; Annex A mandates incoherent addition. FORCE would fail with frequency-comb artifacts. (Also the answer to the 02-05 "Δp_SCR phase decision" handoff: for road/FORCE the cross-sub-source phase never materializes energetically — keep the screen factor's phase as built; revisit coherent cross-source interference semantics only for M2 correlated sources.)
- **Applying conditioning inside `propagation/`.** Breaks the conj quarantine and the recompute split. Conditioning is a transfer-layer multiply.
- **Storing `kR` or resampling frequencies per balloon.** τ is the carried primitive; the 105-point axis is the shared vocabulary.
- **A second tensor fill path for "recompute."** The MAC-identity test must reuse the same fill code with conditioning folded in — two implementations would drift.
- **Materializing the full 100k-receiver tensor to test the budget.** The sink accounting IS the test.

## FORCE Full-Suite Gap Inventory (VAL-02)

### What the suite actually contains

Source: Env. Project 1335 (2010), `refs/EnvProject1335-2010.pdf` Ch. 1-6 [CITED: pages 13-25]; workbook structure verified against `cases/xls.rs` and the live refs.

| Group | Workbook | Cases | Traffic / geometry facts (Ch. 2-5) | Extra capabilities needed |
|-------|----------|-------|-------------------------------------|---------------------------|
| Straight road 1 (flat) | TestStraightRoad | 1-18 | cat-1 @ 80 km/h (17 = cat-2, 18 = cat-3), DAC 12 surface, axle width 1.5 m (2.5 m cat 2/3), 179 source points @ 1° covering ±89°, 10 000 veh/24 h, RH 70 % | emission + pass-by integration (+ heavy-vehicle coefficients for 17/18) |
| 2 (flat mixed) | 〃 | 21-24 | per-group pattern: downwind / no wind / upwind / no-wind h_r = 4 m | + **segmented-ground refraction wiring** (typed error today) |
| 3-4 (elevated road, N/M roughness) | 〃 | 31-34, 41-44 | non-flat profiles | + **Sub-model 3** (§5.12; typed error today) |
| 5-6 (valley, valley+lake) | 〃 | 51-54, 61-64 | non-flat, mixed impedance (lake = H) | + SM3 + segmented refraction |
| 7-9 (thin/thick/double screen) | 〃 | 71-74, 81-84, 91-94 | screens on flat ground; wind variants | + **screen-channel refraction** (screen channel consumes no `SoundSpeedProfile` today) |
| 10-11 (non-flat valley/hill) | 〃 | 101-104, 111-114 | non-flat without screens | + SM3 |
| 12 (forest) | 〃 | 121-124 | scattering zone x = 10-80 m, nQ = 0.075 m⁻¹, mean stem radius 0.1 m, kp = 1.25 (Table 2) | + **Sub-model 10** (§5.19) — currently scoped to M2 ENG-09 (see Open Q3) |
| Curved road | TestCurvedRoad | 10 cases (8 positions; 4-1/4-2, 5-1/5-2) | 2 lanes both trafficked, 90 % cat-1 @ 90 km/h + 10 % cat-2/3 @ 70 km/h, source spacing 10/25/50 m, thin screen (ρE = 1), thick screen as building AND as grass embankment (slope 0.5), terrain from contour lines in the "Coordinates" sheet, u = 0 and ±3 m/s @ 315° | + loaders (Coordinates sheet → per-pair profiles), multi-lane/multi-category emission, embankment-in-profile handling |
| City street | TestCityStreet | 4 positions | 12 m flat-roof buildings, façade receivers ("free-field" SPL: exclude own-façade reflection), **1st AND 2nd order reflections** from other façades, ρE = 1 and 0.7, cat-1 @ 50 km/h, source spacing 2 m/20 m, source cutoff at 20× shortest distance | + **Sub-model 11** (§5.20 reflection effect) + image-source path construction |
| Yearly average | TestYearlyAverage | 4 orientations | = straight-road case 4 terrain/traffic; Met. statistics sheet (M1-M25 classes, loaded in 03-03); vehicles 7 889/791/1 320 day/eve/night; L_den | + per-weather-class propagation runs + class→(A,B) mapping (03-03 [ASSUMED] quarantine) + **Danish L_den hours** (Pitfall 4) |

### Verified engine gaps (grep/read against the current tree)

1. **Sub-model 3** — `PropagationError::NonFlatTerrainNotImplemented` raised whenever `(1−r_flat)` carries weight (`terrain_effect/mod.rs:174-179`). §5.12 is segment-wise classification (concave/convex/transition) + per-segment terrain effect + Fresnel-zone-weight summation, Eqs. 134-156 (AV pp. 59-67). Interacts with refraction (refraction-corrected heights via `FresnelZoneW`). **24 straight-road cases blocked.**
2. **Segmented ground + refraction** — `PropagationError::SegmentedRefractionNotImplemented` (`terrain_effect/mod.rs:401`); `calc_eq_ssp_ground` (§5.5.3) exists and is oracle-tested (`refraction/eqssp.rs:213`) but Sub-model 2 still uses `straight_rays`. Blocks all wind/gradient variants over mixed impedance.
3. **Screen channel refraction** — `screen.rs` has zero refraction plumbing (only a "Phase 3" comment at line 37); the ξ<0 shadow branches for screens (Eqs. 184-186) and refraction-corrected screen geometry are unimplemented. Blocks wind variants of groups 7-9.
4. **Sub-model 8 (multiple ground reflections, §5.17, AV p. 120)** — absent. Fires when the ray count `N` (Eq. 279, grows with `d`, `A`, and decreasing `h_max`) exceeds the fixed sub-model ray count under strong downward refraction. FORCE distances (≲ 200 m) with u = 3 m/s *may* keep N ≤ 1 — **evaluate Eq. 279 over all FORCE downwind geometries first; implement only if any case demands it** (see Assumption A6).
5. **Sub-model 10 (forest, §5.19, AV pp. 124-128)** — absent; `Fs` coherence reduction (Eqs. 288-290, kf/ka Table 8) + ΔLs scattering attenuation. Needed only for cases 121-124. Scope tension: this *is* ENG-09 (currently Milestone-2 Phase 5). See Open Q3 — a scope decision the planner must surface.
6. **Sub-model 11 (reflection effect, §5.20, AV p. 129)** + lateral/3D reflection-path construction — absent. Needed for city street (1st+2nd order façade reflections with ρE = 0.7/1) and possibly curved-road screen faces. Finite-screen lateral diffraction (Annex E) is *informative* — check whether any curved-road case needs it before implementing.

### Harness gaps

- Curved/city/yearly **loaders** are placeholders (`CaseKind::ForceCurvedRoad/CityStreet/YearlyAverage` → `Skipped`); the curved/city workbooks carry a "Coordinates" sheet (road centre line, barriers, calc points, terrain contour lines) and per-vehicle-class extra sheets; yearly carries "Met. statistics" (already parsed by `weather::route1::load_met_probabilities`).
- **Emission model + pass-by integration** (next section).
- **A/C-weighting** at exact band centres (IEC 61672 closed form) — needed for LAeq,24h/LAE/LAmax.
- **Ch. 6 comparator** (below).
- `run_case` FORCE arms + `Capability::EmissionModel` flip (keep fail-soft until numerically honest).

### The acceptance tolerance (Env. Project 1335 Ch. 6, p. 25) [CITED]

- **Overall A-weighted levels (LAeq,24h, LAE, LAmax): ≤ 1 dB** deviation on every case (deviations > 0.5 dB "should give rise to increased awareness").
- **Per-1/3-octave-band: ≤ 1 dB** generally; deviations > 1 dB in a band "shall be investigated".
- **Ground-dip exception:** near interference dips, a dip shift of **one third-octave** is acceptable — the comparator needs a dip-shift rule: when a band fails, check whether shifting the computed spectrum ±4 grid indices (±1/3 octave on the 1/12 grid) around a detected dip brings it within tolerance, and report it as a dip-shift pass with annotation rather than hard failure. This is a **new comparator capability** — the current `compare_pointwise` is strict per-band.
- **Road-length caveat:** groups 1 & 4 assume an infinite road; groups 2-3 use a 40× shortest-distance section — mismatched extent shows up as spectrum sensitivity (document the implemented extent per group).

### The `dL` validation asset

Each straight-road sheet carries per-band `dL` = propagation effect = `LE − LE_freefield` (free-field disregards ground, screens, AND air absorption) [CITED: EP 1335 p. 15]. Therefore:
- `LE − dL` = **free-field pass-by spectrum** = emission model + directivity + pass-by integration + divergence only. This anchors the whole emission pipeline **before** any hard propagation lands (04-02 can gate on it).
- Conversely `dL` itself validates propagation+integration largely independent of absolute emission level. Use both directions in the acceptance ladder.

## Emission Model (Nord2000 Road / Jonasson)

### Verified structure [CITED: Kragh, "Traffic Noise Prediction with Nord2000 — An Update", Danish Road Institute / vejdirektoratet.dk §2.1; User's Guide Nord2000 Road AV 1171/06 §3.1.1]

- A vehicle = **two (or three) point sub-sources** at heights **0.01 m, 0.30 m, 0.75 m** (+ **3.5 m** for heavy vehicles with high exhaust). Light (cat 1): 0.01 + 0.30 m. Heavy (cat 2/3): 0.01 + 0.75 m.
- **Horizontal position: 1 m from the vehicle centre line, toward the receiver** ("average position of the nearest wheels"). ⇒ FORCE straight-road source line is at x = 2.5 + 1.0 = **3.5 m** from the road centre, NOT the 2.5 m Phase-1 placeholder (Pitfall 1).
- Sound power per **1/3-octave band 25 Hz-10 kHz** from coefficient tables, tyre/road (rolling) and propulsion computed **separately**, then distributed: **rolling: 80 % to the low source, 20 % to the high; propulsion: 80 % to the high source, 20 % to the low** (energy split).
- **Directivities:** every sub-source has a frequency-dependent **vertical** directivity (car-body screening); the 0.01 m source additionally has a frequency-dependent **horizontal** directivity (tyre horn effect); the heavy 0.75 m propulsion source has a frequency-**independent** horizontal directivity. These map exactly onto SRC-02/04 balloons.
- **Corrections:** road surface (reference = average of DAC 11 / SMA 11, > 2 years old, Denmark; FORCE uses **DAC 12** ⇒ aggregate-size correction from AV 1171/06 §3.1.7 Figure 4-series), air temperature (reference 20 °C; FORCE t₀ = 15 °C ⇒ rolling-noise correction, K ≈ 0.1 dB/°C for DAC cat-1, halved for cat 2/3 [CITED: KCB Användarhandledning §; verify against SP 2006:12]), wet-surface and studded-tyre corrections (not exercised by FORCE — defaults dry/unstudded), acceleration/deceleration (not exercised — constant speed).
- FORCE straight-road traffic: cat-1 @ 80 km/h (cases 17/18: cat-2/cat-3), 10 000 vehicles/24 h; curved road: 90 % cat-1 @ 90 + 10 % cat-2/3 @ 70 km/h; city street: cat-1 @ 50 km/h [CITED: EP 1335 Ch. 2-4].

### Where the coefficients live — the acquisition problem

The per-band coefficient tables (a_R(f), b_R(f) rolling; a_P(f), b_P(f) propulsion; per category) are in **H. Jonasson, "Acoustic Source Modelling of Nordic Road Vehicles", SP Rapport 2006:12, Borås 2006** — reference [1] of EP 1335 and [4] of AV 1171/06. **Not freely downloadable** (verified: DiVA has only the 2004 Harmonoise deliverable; no mirror on forcetechnology.com, mst.dk, or the web archive). Acquisition ladder for the planner:

1. **Human checkpoint (recommended, blocking for 04-02):** user obtains SP 2006:12 (RISE publication service / library / research request via ResearchGate author copy). Add to `refs/fetch.sh` + `refs.sha256` only if a stable URL exists; otherwise document a manual drop into `refs/` with a pinned hash.
2. **Fallback A — Swedish KCB tables with de-adjustment [HIGH RISK, ASSUMED]:** kunskapscentrumbuller.se "Användarhandledning Nord2000 version 1.0" Tabell 16 reproduces the coefficient tables **but with 2015 Swedish adaptation terms folded in** — NOT the DK-reference values the FORCE spectra embed. Only usable if the 2015 adjustment terms (their ref [17]) can be obtained and reversed. Do not silently use (Pitfall 9).
3. **Fallback B — partial reverse-derivation from the `.xls`:** `LE − dL` gives the integrated free-field spectrum per case; with the cited heights/splits/geometry, the **combined** effective L_W(f) at 80 km/h cat-1/DAC-12 can be fitted per band. This validates/pins case-1-like configurations but cannot uniquely recover per-sub-source splits, speed slopes (b coefficients), or directivities — sufficient for the flat cat-1 cases only if checkpoint 1 fails; mark everything derived this way [ASSUMED] with the fit provenance.
4. The **directivity functions**: Nord2000 adopted/adapted the Harmonoise source structure [CITED: AV 1171/06 Foreword]. Harmonoise/IMAGINE deliverables (public) contain candidate function forms, but Nord2000's were "adapted and fitted to available Nordic source data" — treat Harmonoise forms as [ASSUMED] until checked against SP 2006:12.

Also **add the freely-downloadable User's Guide to `refs/fetch.sh`**: `https://egra.cedex.es/EGRA-ingles/I-Documentacion/National_Methods/Users_Guide_Nord2000_Road.pdf` (verified live; sha256 `73f465e2cd78e54d536f5c29dce139c4fb5430539c84b00d974d0bc4d7d21491`; 51 pages, AV 1171/06).

### Pass-by integration (straight road) [CITED: EP 1335 Ch. 2 for the discretization; formulas are standard energy integration]

- 179 source points at constant **1° immission-angle steps**, θ ∈ [−89°, +89°], each representing the road segment subtended by 1°.
- Per band: `L_E(f) = 10·lg Σ_i (t_i/t₀)·10^{L_i(f)/10}`, `t₀ = 1 s`, `t_i = ℓ_i/v` (v = 22.2̄ m/s at 80 km/h), `ℓ_i = d⊥·[tan(θ_i+0.5°) − tan(θ_i−0.5°)]`.
- `LAeq,24h = LAE + 10·lg(N) − 10·lg(86 400)` with N = 10 000 (= LAE − 9.37 dB). Sanity: case 1 LAeq,24h = 39.398… ⇒ LAE ≈ 48.8 dB — consistent with the sheet's separate LAE row (cheap cross-check in the loader tests).
- `LAmax` = max over source points of the A-weighted instantaneous level (all sub-sources energy-summed) [ASSUMED: no time-constant smearing — A2; verify against the .xls values, which will make any discrepancy obvious].
- The **tensor is the natural home**: sub_source axis = (source point × height); LE = incoherent readout with weights `t_i·10^{L_W/10}`; LAmax = per-source-point partial readout. This makes 04-01's tensor load-bearing for 04-03 rather than parallel plumbing.

## NoiseModelling Cross-Validation (VAL-03)

### Tool facts

**NoiseModelling v6.0.0** (Université Gustave Eiffel, GPLv3, May 2026 — [VERIFIED: OPEN-GIS-LANDSCAPE.md provenance]) implements CNOSSOS-EU (Directive 2015/996). Java 11+; runs fully offline from local input (GeoJSON/CSV/H2GIS) via Groovy WPS scripts or the standalone distribution. **Java is not installed on this machine** (verified `java -version` fails) — one-time JRE install (e.g., Temurin via winget) is required for fixture generation, or generate fixtures on another machine. **CRITICAL: never force-close the JVM/Gradle processes** (project environment rule).

### Method: committed-fixture oracle (mirrors the scipy-oracle pattern — no Java at test time)

1. `tools/noisemodelling_oracle/`: 2-3 minimal scenes as data + a documented run recipe (README): (a) free field, point source, flat **hard** ground (G=0), receiver at 100 m, no barrier; (b) same with a thin barrier (known height/position); (c) soft ground (G=1) flat case for the report-only ground comparison. Standard atmosphere 15 °C / 70 % RH to match FORCE parameters.
2. Run NoiseModelling once; export per-octave-band receiver levels and path attenuation terms (Adiv, Aatm, Abar, Aground are separately reported by CNOSSOS implementations); commit as `crates/envi-harness/tests/fixtures/oracle/noisemodelling.toml` with provenance (NM version, scene hash, date, generator = human-run recipe).
3. Rust tests compare by **band index**: CNOSSOS is octave-band 63 Hz-8 kHz; all 8 octave centres fall exactly on the 1/12-octave grid (indices 16, 28, 40, …, 100 — every 12th point from 63.096 Hz). Never compare at nominal frequencies.

### Comparable quantities and the delta posture

| Quantity | Comparable? | Gate | Expected delta & why |
|----------|------------|------|----------------------|
| Geometric divergence | YES — identical physics | equality ≤ 0.1 dB | Nord2000 ΔL_d = −10·lg(4πR²) ≡ CNOSSOS Adiv = 20·lg d + 11 (same formula, 10·lg 4π = 10.99) |
| Air absorption | YES — same underlying standard | equality ≤ 0.2 dB per octave point | Both use ISO 9613-1 α; ENVI's Eq. 287 band conversion vs CNOSSOS's αd/1000 may differ slightly at 8 kHz (band-averaging vs centre value) — if > 0.2 dB, document, don't force |
| Screen path-difference geometry (δ) | YES — pure geometry | equality ≤ 1 cm on δ | Same 2D cut-plane construction |
| Barrier insertion loss | REPORT-ONLY | none (documented table) | Nord2000 Hadden-Pierce wedge vs CNOSSOS Δdif = 10·lg(3 + (40/λ)C″δ): expect 0-6 dB differences, largest at high fc and grazing incidence — equality would indicate a bug in the comparison, not success |
| Flat-ground excess attenuation | REPORT-ONLY | none | Complex spherical-wave Q̂ interference (dips) vs CNOSSOS empirical G-model (no dips in octave bands) — fundamentally different; document magnitude + trend agreement only |

**License posture:** running the GPL binary and committing its numeric **outputs** is clean; porting/translating its source is forbidden (CLAUDE.md). **Offline/no-credentials:** all inputs local, no API keys, no network at test time (fixtures committed).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| `.xls` parsing for the 3 remaining workbooks | new parser | existing calamine label-anchored pattern (`cases/xls.rs`) | survives layout drift; DoS guards already built |
| Tensor container | custom strided buffer | `ndarray::Array3` (+ views for chunks) | frozen contract; layout test exists |
| Rotation math | quaternion crate | hand-rolled 3×3 matrix (engine-local) | one function; D-08 precedent (no linalg crates) |
| CNOSSOS reference values | re-implementing CNOSSOS formulas | NoiseModelling executable oracle → committed fixtures | VAL-03 names NoiseModelling; re-implementation cross-checks nothing |
| ISO 9613-1 / Faddeeva / wedge kernels | anything new | Phase 1-2 modules as-is | already oracle-pinned |
| A-weighting | table lookup by nominal Hz | IEC 61672 closed-form Ra(f) at exact band centres | float-keying nominal frequencies is the project's own Pitfall 3 |

**Key insight:** everything numerically hard in this phase (Faddeeva, wedges, Δτ, ξ) already exists and is oracle-pinned — Phase 4's risk is **integration and data provenance**, not new numerics (except SM3/SM8/SM10/SM11 transcription, which follows the established verify-against-page-images protocol).

## Common Pitfalls

### Pitfall 1: The 1 m source offset — Phase 1's x = 2.5 m placeholder is wrong for emission
**What goes wrong:** Phase 1 froze "FORCE source line at x = 2.5 m ⇒ horizontal distance 97.5 m". AV 1171/06 §3.1.1 puts all sub-sources **1 m from the vehicle centre line toward the receiver** ⇒ x = 3.5 m, d⊥ = 96.5 m. ~0.09 dB divergence shift plus ground-geometry changes on every case.
**How to avoid:** rebuild source positions in the emission expansion (04-02); the Phase-1 decision record must be superseded explicitly. The `LE − dL` anchor will catch it if missed.
**Warning signs:** a uniform fraction-of-a-dB bias across all flat cases.

### Pitfall 2: 2009 vs 2010 reference values, and method-revision drift
**What goes wrong:** the pinned workbooks came from the **2009 publication URLs** (Env. Project 1276) and are tagged `Force2009`; EP 1335 (2010) documents *revised* workbooks (`*_20100610.xls`, not found online — verified). Some case values changed between revisions. Additionally, `refs/AV1106-07-rev4.pdf` is the **2014** revision and DELTA published **2018 amendments** (archived at forcetechnology) — both post-date the 2010 reference computation.
**How to avoid:** (a) verify which revision the pinned `.xls` actually contains by cross-checking a handful of values against EP 1335's appendix example sheets (page images); (b) gate against what's in hand, record provenance per case (`ReferenceVersion` already plumbs this); (c) do NOT implement the 2018 amendments — the references pre-date them.
**Warning signs:** a small set of cases failing by a consistent offset while their group siblings pass.

### Pitfall 3: Coherent summation of incoherent sub-sources
**What goes wrong:** using the OUT-03 coherent MAC for the FORCE readout adds phantom interference between sub-sources/source points (AV Annex A mandates incoherent addition). Spectra grow comb artifacts; totals swing by several dB.
**How to avoid:** Pattern 4's two readout laws; FORCE dispatch uses only the incoherent law. Test: two identical co-located sub-sources ⇒ +3 dB (incoherent), not +6 dB.

### Pitfall 4: L_den day/evening/night hours — Danish 12/3/9, not EU-default 12/4/8
**What goes wrong:** `route1::l_den` (03-03) uses 12/4/8-hour weights; Denmark defines day 07-19 (12 h), evening 19-22 (**3 h**), night 22-07 (**9 h**) [CITED: AV 1171/06 §2.2 Table 1]. TestYearlyAverage (7 889/791/1 320 vehicles) will mismatch.
**How to avoid:** parameterize period durations in `l_den`; use the Danish split for FORCE.

### Pitfall 5: Strict per-band comparison fails legitimately near ground dips
**What goes wrong:** Ch. 6 explicitly allows a one-third-octave dip shift; a strict 1 dB per-band gate produces false failures exactly where the physics is most sensitive.
**How to avoid:** extend the comparator with the dip-shift rule (detect local minimum, allow ±4 grid-index shift, annotate as dip-shift pass). Keep the overall-level 1 dB gate strict.

### Pitfall 6: MAC "identity" that's only approximate
**What goes wrong:** composing conditioning in different orders on the MAC path vs the recompute path leaves last-ulp differences and forces an epsilon test, weakening success criterion 2.
**How to avoid:** Pattern 5 — one composed `G_s(f)`, one multiplication, `assert_eq!` on bits.

### Pitfall 7: Emission coefficients from the wrong lineage
**What goes wrong:** the easiest-to-find coefficient tables (Swedish KCB guide) embed 2015 Swedish adaptation terms; CNOSSOS or Harmonoise tables are different models. All fail FORCE by design.
**How to avoid:** only SP 2006:12 values (or `.xls`-derived fits marked [ASSUMED]); every transcribed constant carries a provenance comment; the `LE − dL` anchor gates the pipeline before propagation is in the loop.

### Pitfall 8: Balloon sampling degrading FORCE totals
**What goes wrong:** coarse balloon grids + interpolation error shave tenths of dB near grazing angles (θ → ±89°), where the horn-effect directivity moves fastest and the time weights `tan θ` blow up.
**How to avoid:** 5° grid (or finer near-horizontal), unit test sampling error < 0.05 dB vs the analytic form across the exact evaluation directions used by the 179-point integration.

### Pitfall 9: Wind screen cases silently computed with unrefracted screens
**What goes wrong:** flipping `Capability::EmissionModel` makes wind screen cases (72/74/82/84/92/94…) runnable, but the screen channel ignores the weather profile today — they'd produce homogeneous-screen numbers and fail (or worse, pass by luck).
**How to avoid:** wire screen refraction (or raise a typed error for weather+screen until wired — mirroring `SegmentedRefractionNotImplemented`) so the honest-green invariant survives intermediate states.

### Pitfall 10: Road-extent truncation sensitivity
**What goes wrong:** groups 1/4 assume an infinite road; ±89° at d⊥ ≈ 96.5 m already spans ±5.5 km of road. Truncating at the terrain-profile extent or mishandling the last segments biases low frequencies.
**How to avoid:** implement the exact ±89° 1°-segment discretization (the reference's own discretization); document extent per group; the `dL` anchor localizes errors here.

## Code Examples

### Chunked solve + budget accounting (engine, sketch)

```rust
// Source: design synthesis from transfer.rs contract + ROADMAP Phase-10 seam
pub fn solve<'a>(
    jobs: impl Iterator<Item = SolveJob<'a>>,   // harness-built geometry, receiver-major order
    n_sub: usize, chunk_receivers: usize,
    axis: &FreqAxis, sink: &mut dyn TensorSink,
) -> Result<(), PropagationError> {
    // fill Array3 chunks [n_sub, chunk_receivers, N_BANDS]; flush to sink per chunk;
    // high-water mark = n_sub * chunk_receivers * N_BANDS * 24 bytes  ≤ budget
    …
}
```

### The two readouts (engine, sketch)

```rust
// Coherent MAC (OUT-03): bit-exact vs recompute when G is pre-composed (Pattern 5)
pub fn mac_coherent(h: ArrayView3<Complex<f64>>, g: &[Vec<Complex<f64>>]) -> Array2<Complex<f64>> {
    // p[r,f] = Σ_s h[s,r,f] * g[s][f]   — iterate s outermost, f contiguous innermost
}

// Incoherent (Annex A / FORCE): weights carry |G_s|² (emission energy × pass-by time weight)
pub fn readout_incoherent(h: ArrayView3<Complex<f64>>, p_incoh_abs: ArrayView3<f64>,
                          w: &[Vec<f64>]) -> Array2<f64> {
    // e[r,f] = Σ_s w[s][f] * (h[s,r,f].norm_sqr() + p_incoh_abs[s,r,f])
}
```

### Ch.6 conversions (harness)

```rust
// LAeq,24h from LAE:  l_aeq_24h = l_ae + 10.0*(N as f64).log10() - 10.0*86_400f64.log10();
// A-weighting at exact centres (IEC 61672 Ra(f)), applied per band index — never by nominal Hz.
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Nord96 Nordic method | Nord2000 Road (Jonasson source + AV 1106/07 propagation) | 2006 official | The model being implemented |
| EP 1022/2005 test cases | EP 1276 (2009, corrected) → **EP 1335 (2010, revised)** `*_20100610.xls` | 2010-06-10 | Reference-version provenance matters (Pitfall 2) |
| AV 1106/07 (2007) | rev. 2010 → **rev4 (2014)** in refs/ → 2018 amendments (do NOT implement) | 2014 / 2018 | FORCE 2010 refs pre-date rev4/2018 deltas — watch for method drift on failures |
| NoiseModelling v4/v5 | **v6.0.0** (May 2026) | 2026-05 | Use v6 for VAL-03 fixtures; record version in provenance |

**Deprecated/outdated:** Harmonoise/IMAGINE coefficient tables for this purpose (different fitted data); KCB 2015-adjusted Swedish tables (wrong lineage for FORCE).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Rolling/propulsion speed law is `L_W = a + b·lg(v/v_ref)` with v_ref = 70 km/h | Emission | Wrong speeds for 50/70/90 km/h groups; verify against SP 2006:12 the moment it's in hand |
| A2 | LAmax = max over the 179 source positions, no Fast-time-constant smearing | Pass-by | LAmax off by tenths; the .xls LAmax row will falsify immediately |
| A3 | Pinned `.xls` files (2009 URLs) contain the values EP 1335 documents | FORCE inventory | Gate against a stale reference; verify per Pitfall 2(a) |
| A4 | The 3.5 m high-exhaust source is NOT active in FORCE cases 17/18 | Emission | Heavy-vehicle cases biased; check SP 2006:12 defaults + case description |
| A5 | Oblique-path profile = perpendicular profile stretched 1/cos θ (exact for lateral invariance) | Pattern 7 | Integration-level bias; `dL` anchor catches it |
| A6 | Sub-model 8 contributes nothing at FORCE geometries (N ≤ 1 in Eq. 279 for d ≲ 200 m, u = 3 m/s) | Gap inventory | Downwind cases fail by up to ~1 dB at low f; **evaluate Eq. 279 over all downwind cases as a 04-03 task before deciding to skip SM8** |
| A7 | Nord2000 directivity functions ≈ Harmonoise forms (adapted) | Emission | Grazing-angle integration bias; resolve with SP 2006:12 |
| A8 | DAC 12 surface correction is the AV 1171/06 §3.1.7 aggregate-size curve value at D = 12 | Emission | Constant offset ~±0.5 dB on all cases; transcribe from the PDF figure (page images) |
| A9 | Yearly-average class→(A,B) mapping comes from Eurasto VTT-R-02530-06 (EP 1335 ref [6]) | Yearly average | 03-03's [ASSUMED] quarantine persists; silo.tips hosts an unofficial copy — verify contents before trusting |

## Open Questions

1. **SP Rapport 2006:12 acquisition (BLOCKER for full VAL-02).**
   - What we know: not freely downloadable (four search paths verified); User's Guide + Kragh paper give structure but not the coefficient tables.
   - What's unclear: whether the user can obtain it (RISE, library, author request).
   - Recommendation: **blocking human checkpoint at the start of 04-02**, with Fallback B (`LE − dL` fit, [ASSUMED]-quarantined, flat-cat-1 cases only) as the degraded path — mirroring the 03-03 option-(c) precedent (honest Skips, no false Pass).
2. **Which workbook revision is pinned?** Verify 2009-vs-2010 content (Pitfall 2a) and hunt the `*_20100610.xls` set once more from EP 1335's landing page; if found, add to fetch.sh as `Force2010`.
3. **Forest cases 121-124 (Sub-model 10) — scope decision.** VAL-02 says "full FORCE suite"; ENG-09 (the same physics) is scoped to Milestone-2 Phase 5. Options: (a) pull the SM10 propagation kernel forward into 04-03/04-04 (it is self-contained: Fs Eqs. 288-290 + ΔLs; the scene already carries the Table-2 parameters), or (b) leave 121-124 capability-gated (`requires: forest-scattering`) and define the Milestone-1 gate as 58/62 + curved + city + yearly, closing the last 4 in Phase 5. **Needs user sign-off — surface in plan-phase discussion.** Recommendation: (a) if SP 2006:12 arrives early (the phase's critical path is data, not this kernel); otherwise (b) with an explicit accepted-gap note.
4. **Curved-road/city-street effort.** The Coordinates-sheet → per-source-point profile construction (contour interpolation) plus SM11 image-source paths is the largest single harness work item; confirm the user wants it inside Milestone 1 (VAL-02 as written says yes).
5. **Java for VAL-03.** Install Temurin JRE locally (winget) or generate NoiseModelling fixtures elsewhere; either way commit fixtures so tests never need Java. (Environment rule: never kill JVM processes.)
6. **Δp_SCR cross-source phase semantics** (02-05 handoff): resolved for Phase 4 as "keep as-built; incoherent road readout makes it moot" (Anti-Patterns) — confirm no M2 requirement pulls it earlier.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | everything | ✓ | rustc/cargo 1.96.0 | — |
| Python + scipy/numpy | oracle regeneration (tools/nord2000_oracle) | ✓ | 3.13 / scipy 1.17.1 / numpy 2.4.4 | committed fixtures (no Python at test time) |
| refs/ artifacts (4 .xls + 3 PDFs) | FORCE cases, equation verification | ✓ | SHA-256-pinned | fail-soft Skips |
| pdftotext / pdftoppm (poppler) | PDF equation/page-image verification | ✓ | mingw64 + winget poppler 25.07 | — |
| **Java (JRE 11+)** | NoiseModelling fixture generation (VAL-03) | **✗** | — | install Temurin once, or generate fixtures on another machine; fixtures committed |
| **SP Rapport 2006:12** | emission coefficients (VAL-02) | **✗ (not online)** | — | human acquisition checkpoint; Fallback B (`LE−dL` fit, [ASSUMED]) |
| User's Guide AV 1171/06 | emission structure/corrections | ✓ (verified live URL) | sha256 73f465e2… | add to fetch.sh |
| VTT-R-02530-06 (weather classes) | yearly-average class→(A,B) | partial (unofficial mirror) | — | keep 03-03 [ASSUMED] quarantine if unobtainable |
| NoiseModelling v6.0.0 dist | VAL-03 | not yet downloaded | v6.0.0 (2026-05) | offline zip; one-time use |

**Missing with no fallback:** none strictly blocking the phase start (04-01 needs nothing external).
**Missing with fallback:** SP 2006:12 (degraded [ASSUMED] path), Java (fixture generation elsewhere / one-time install).

## Validation Architecture

> Included at orchestrator request (config `nyquist_validation: false` notwithstanding) — this phase IS the milestone's validation gate.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (rustc 1.96) + libtest-mimic dynamic FORCE runner (existing) |
| Config file | none needed (workspace tests) |
| Quick run command | `cargo test -p envi-engine` |
| Full suite command | `cargo test --workspace` then `cargo run -p envi-harness -- report` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| OUT-01/02 | tensor filled per (s,r,f), row-major, freq-contiguous | unit | `cargo test -p envi-engine tensor` | ❌ Wave 0 (layout test seed exists in transfer.rs) |
| OUT-03/04/05 | MAC ≡ recompute bit-exact; filter+delay conditioning | unit | `cargo test -p envi-engine mac_identity` | ❌ Wave 0 |
| OUT-06 | 100k-receiver synthetic sweep, high-water mark ≤ 256 MiB | integration | `cargo test -p envi-harness --test tensor_budget` | ❌ Wave 0 |
| SRC-02/03/04 | balloon sampling error; rotation direction test; incoherent +3 dB composition | unit | `cargo test -p envi-engine directivity` | ❌ Wave 0 |
| emission | `LE − dL` free-field anchor per flat case | integration (refs-gated, fail-soft) | `cargo test -p envi-harness --test emission_anchor` | ❌ Wave 0 |
| VAL-02 | Ch.6 comparator per case group; capability flip; Skip-shrink assertions | dynamic runner | `cargo run -p envi-harness -- report` | partial (runner exists; comparator extension needed) |
| VAL-03 | divergence/air-abs equality vs committed NM fixtures; delta report | integration | `cargo test -p envi-harness --test oracle_noisemodelling` | ❌ Wave 0 |
| SM3/8/10/11 | scipy-oracle fixtures per new sub-model (established pattern) | oracle | `cargo test -p envi-harness --test oracle_*` | ❌ per plan |

### Sampling Rate
- **Per task commit:** `cargo test -p envi-engine` + clippy/fmt.
- **Per plan merge:** `cargo test --workspace` + `cargo run -p envi-harness -- report` (Skip-reason audit: never a false Pass).
- **Phase gate:** full suite green, FORCE report shows Pass for every in-scope case, conj-grep = 0, `cargo tree -p envi-engine` unchanged.

### Wave 0 Gaps
- Ch.6 comparator (overall gate + per-band gate + dip-shift rule) — pre-req for any FORCE flip.
- A-weighting utility + LAE/LAeq/LAmax conversions with unit anchors.
- Oracle generators: `gen_submodel3_fixtures.py` (+ SM8/SM10/SM11 as scoped) following `tools/nord2000_oracle` conventions (sha256 provenance, no Python at test time). Note the standing oracle-independence caveat: scipy oracles cross-check *implementation*, not the *spec reading* — the FORCE `.xls` is the only true external authority.

## Recommended Plan Split

The roadmap's 3 plans understate VAL-02 (six missing sub-model capabilities + three loaders + the emission model). Recommend **5 plans** (planner may fold 04-05 into 04-03/04-04 if small):

| Plan | Scope | Requirements | Key gate |
|------|-------|--------------|----------|
| **04-01 Tensor + solver + MAC** | `tensor.rs` (TensorPair, TensorSink, InMemorySink, both readout laws), `solver.rs` (SolveJob seam), conditioning composition, memory-budget sweep, MAC bit-identity | OUT-01..06 | budget test + bit-exact MAC; `cargo tree` unchanged |
| **04-02 Directional sources + emission model** | `directivity.rs` balloons + rotation; **human checkpoint: SP 2006:12**; `emission/` (coefficients + corrections + sub-source expansion + balloons for horn/vertical/heavy-horizontal); pass-by integration; `LE − dL` free-field anchor green on flat cat-1 cases; User's Guide added to fetch.sh | SRC-02/03/04 (+ emission for VAL-02) | free-field anchor ≤ ~0.3 dB/band on case-1 family before any propagation coupling |
| **04-03 Straight-road FORCE pass** | SM3 (§5.12) + segmented-ground refraction wiring + screen-channel refraction (+ SM8 Eq. 279 evaluation → implement iff needed); Ch.6 comparator + A-weighting; `Capability::EmissionModel` flip; 62-case gate (58 if forest deferred per Open Q3) | VAL-02 (core) | `report` shows numeric Pass for all in-scope straight-road cases at Ch.6 tolerance |
| **04-04 Curved + city + yearly** | Coordinates-sheet loaders + contour→profile builder; SM11 + image-source façade reflections (1st/2nd order, ρE 0.7/1); multi-lane/multi-category emission; yearly-average per-class runs + Danish L_den hours (Pitfall 4); forest SM10 here iff pulled forward | VAL-02 (completion) | all four workbooks Pass; milestone acceptance |
| **04-05 NoiseModelling cross-validation** | JRE checkpoint; scenes + run recipe + committed fixtures; equality gates (divergence, air absorption at octave indices) + expected-delta report (barrier, ground) | VAL-03 | fixture tests green offline, deltas documented |

**Emission model placement rationale:** it pairs with 04-02, not 04-03 — the road source is the *first concrete instance* of the directional multi-sub-source model (SRC-02/03/04), and the `LE − dL` anchor lets emission go green without waiting on SM3/refraction wiring. 04-03/04-04 then only debug propagation, never emission and propagation simultaneously.

**Sequencing:** 04-01 → 04-02 → 04-03 → 04-04, with 04-05 parallel-safe any time after 04-01 (it touches only fixtures + comparison tests).

## Sources

### Primary (HIGH confidence)
- `refs/AV1106-07-rev4.pdf` (AV 1106/07, rev. 2014, 177 pp.) — §5.12 (SM3, pp. 59-67), §5.17 (SM8, p. 120), §5.19 (SM10, pp. 124-128), §5.20 (SM11, p. 129), §5.22 (compound, p. 143), Annex A (incoherent composite sources, p. 165), Annex E (lateral diffraction, p. 170) — text extracted this session via pdftotext (page-image verification per house rule remains for transcription tasks)
- `refs/EnvProject1335-2010.pdf` (Env. Project 1335, 2010, 35 pp.) — Ch. 2-5 (case definitions, traffic parameters, 179-point/1° discretization, forest Table 2), Ch. 6 (acceptable deviation), refs list (SP 2006:12 = [1], VTT-R-02530-06 = [6])
- User's Guide Nord2000 Road (AV 1171/06, 51 pp.) — downloaded + verified this session (sha256 73f465e2…): §3.1.1 (sub-source heights + 1 m offset), §2.2 Table 1 (Danish day/eve/night), §3.1.4-3.1.7 (categories, speeds, surface corrections) — https://egra.cedex.es/EGRA-ingles/I-Documentacion/National_Methods/Users_Guide_Nord2000_Road.pdf
- ENVI codebase (transfer.rs, terrain_effect/, capability.rs, cases/xls.rs, eqssp.rs, phase 01-03 SUMMARYs) — gap inventory greps quoted with file:line

### Secondary (MEDIUM confidence)
- Kragh, "Traffic Noise Prediction with Nord2000 — An Update", Danish Road Institute — https://www.vejdirektoratet.dk/api/drupal/sites/default/files/publications/traffic_noise_prediction_with_nord2000.pdf (source-model structure: 80/20 splits, directivity kinds, temperature) — downloaded + verified this session
- KCB "Användarhandledning Nord2000 version 1.0" — https://kunskapscentrumbuller.se/documents/slutversioner/... (Tabell 16 coefficient lineage warning; temperature coefficients) — downloaded this session
- `.planning/research/OPEN-GIS-LANDSCAPE.md` (NoiseModelling v6.0.0 facts, GPL posture)
- forcetechnology web-archive CDX listing (2018 amendments, AV 1117/06 validation report availability)

### Tertiary (LOW confidence / negative results)
- SP Rapport 2006:12 availability: NOT freely downloadable (DiVA, mst.dk, forcetechnology, generic search — all negative this session)
- `*_20100610.xls` revised workbooks: not located online this session
- VTT-R-02530-06: unofficial mirror only (silo.tips)

## Metadata

**Confidence breakdown:**
- Tensor/MAC/solver architecture: HIGH — builds on frozen, tested contracts in the tree
- FORCE gap inventory: HIGH — every gap verified by grep/read against the current tree + the case definitions read from the authoritative PDFs in refs/
- Emission model structure: MEDIUM-HIGH — heights/splits/directivity kinds cited from two independent official-lineage documents; coefficient values NOT in hand
- Emission coefficient acquisition: LOW — the authoritative report is offline; the fallback ladder is designed but degraded
- NoiseModelling method: MEDIUM — tool verified current (v6.0.0), Java absence verified; fixture pattern proven in-project

**Research date:** 2026-07-08
**Valid until:** ~2026-08-07 (stable domain; re-check only NoiseModelling version and any SP 2006:12 acquisition outcome)
