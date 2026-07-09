# Phase 5: Engine Extensions — Forest & Semi-Transparent Partitions - Context

**Gathered:** 2026-07-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Two new pieces of Nord2000 physics land in the **pure-math `envi-engine`** crate, phase-preserving
under the frozen two-channel `H_coh`/`P_incoh` contract and the single-conj quarantine:

- **ENG-09 — Forest attenuation.** A propagation path crossing a forest of through-path length `d`
  is attenuated by Nord2000's `A = d·a(f)`, where `a(f)` derives from mean tree density, mean stem
  radius, factor `kₚ`, and mean absorption coefficient. Evaluated at the 105-point 1/12-octave grid,
  matched to an analytic anchor/oracle.
- **ENG-10 — Semi-transparent partitions.** A straight-through transmission path (ray direction
  preserved) through a screen/façade, attenuated by the per-band isolation spectrum `R(f)`, combined
  with the diffracted and reflected contributions as complex pressure with phase intact. The opaque
  limit `R(f)→∞` reproduces the standard opaque screen bit-for-bit. Per-façade building transmission
  reuses the same mechanism with the crossed façade's `R(f)`.

**In scope:** the two engine physics blocks + their seams into the promoted `solver`, oracle/anchor
validation, and the opaque-limit regression test.

**Out of scope (belongs to later phases):** scene objects that carry these properties (Forest,
semi-transparent Screen/Building — Phase 7, SCN-01..04), the 1/1→1/3→1/12 isolation-spectrum UI
interpolation (Phase 7, SCN-03), real path/forest-crossing geometry extraction (Phase 9, GEOX), and
wiring these into the calc service (Phase 10). The engine consumes `R(f)` **already on the 105-point
grid** and a **pre-computed crossing length `d`** — it does not do scene geometry here.

**Coordination flag (from ROADMAP):** the extended composition MUST ship inside engine Phase 4's
promoted `envi_engine::solver` — **one solve path, two callers** (harness now, service in Phase 10).
Phase 5 must land before Phase 10 can compute these objects and before Phase 7's forest /
semi-transparent UI is meaningful (objects must never be silently inert).

</domain>

<decisions>
## Implementation Decisions

### Forest — seam & geometry (ENG-09)
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

### Forest — two-channel action (ENG-09)
- **D-03:** `A = d·a(f)` applies as a **real, phase-preserving magnitude factor to both channels**:
  `10^(−A/20)` on `H_coh` (argument untouched) and `10^(−A/10)` on `P_incoh_abs`. This is Nord2000's
  treatment of forest as **excess attenuation** (not decorrelation — explicitly NOT routed into the
  incoherent channel like SM7 turbulence scattering). `F→1 ⇒ P_incoh→0` stays bit-exact.
- **D-04:** Because it is a real per-path magnitude factor, forest is applied **solver-side on the
  post-conj ENVI convention**, exactly like `directivity_gain_db` — **never inside `propagation/`**
  (the conj grep-gate stays at zero). Forest is a per-path property, structurally analogous to the
  directivity magnitude factor.

### Semi-transparent — transmission combination point (ENG-10)
- **D-05:** The transmitted straight-through path is added **inside the `propagation/` screen
  sub-models**, in the Nord2000-native `e^{−jωt}` convention, **ahead of the single `.conj()`** at
  `transfer::nord_ratio_to_transfer`. It is genuine propagation physics combined as complex pressure
  alongside the diffracted/reflected terms (SC2). Keeps the one conj boundary intact and puts the
  opaque↔transparent behaviour in one place. Composition is **additive**: `diffracted_field +
  H_ff·T(f)` as complex pressure (the diffracted opaque field always exists; transmission adds a
  leakage path on top).

### Semi-transparent — isolation as a MINIMUM-PHASE FILTER (ENVI extension, ENG-10+)
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

### Semi-transparent — opaque-limit regression (ENG-10, SC3)
- **D-10:** **Structural gating**, not numeric clamping. A screen with **no isolation spectrum**
  (`isolation: Option<IsolationSpectrum> = None`) takes the **exact existing opaque code path** — the
  transmission term and the whole min-phase computation are **never constructed**. This (a) guarantees
  bit-identical opaque results (`R→∞` reproduces the standard screen bit-for-bit) and (b) never feeds
  `ln|T| = −∞` to the Hilbert transform (as `R→∞`, `ln|T|→−∞`). A **permanent regression test** pins
  the opaque-limit equality. "Opaque" is a distinct state (`None`), not a magic large `R` value.

### Following directly (recorded, not separately discussed)
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

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Nord2000 authoritative spec (implement-from; git-ignored `refs/`, copyright — cite, never commit)
- `refs/AV1106-07-rev4.pdf` — AV 1106/07 rev.4, the Nord2000 implement-from document. Read the
  **forest-attenuation section** for the exact `a(f) = f(density, stem radius, kₚ, absorption)`
  formula and any Nord2000 distance handling, and **§5.13–5.16 screen sub-models + §5.22 Eq. 332
  composition** for where the transmission term joins the diffracted/reflected field. **Verify every
  transcribed coefficient/equation against the PDF page images** (multiple prior transcription errors
  were caught this way — class B = 31.5, the fL minus-sign, the w(ẑ) anchor).

### Nord2000 forest term + semi-transparent partition concept (reference workflow)
- `docs/references/dbaudio-ti386-1.6-en.md` §Forest (≈L196–212) — Nord2000 forest `A = d·a(f)` with
  `a(f)` from mean tree density / stem radius / `kₚ` / absorption; default Nord2000 values are a best
  fit to ISO 9613-2. **Note:** the piecewise distance table (`<10→0`, `10–20→A₁₀₋₂₀`, `20–200→d·a(f)`,
  `≥200→200·a(f)`) is the **ISO 9613-2** variant — this project is **Nord2000-only**, so the plain
  `A = d·a(f)` (roadmap SC1) is the target; still confirm against AV 1106/07 whether Nord2000 itself
  bounds `d`. TI 386 §Building/Wall for the isolation/reflection-loss framing of partitions.
- `.planning/research/FEATURES.md` (≈L156) — the forest-scattering coverage note ("do not silently
  draw forests that do nothing"), the origin of this engine phase.

### Load-bearing project contracts (must not regress)
- `.claude/CLAUDE.md` — the complex/phase contract (`H_coh` complex + separate real `P_incoh`;
  `F→1⇒P_incoh→0` bit-exact), the time-convention quarantine (single `.conj()` at
  `transfer::nord_ratio_to_transfer`; zero `.conj()` in `propagation/`), the directional-phase
  extension precedent, and the 105-point 1/12-octave frequency framework (compare by band index).
- `.planning/phases/04-transfer-tensor-directional-sources-full-validation/deferred-items.md` — the
  "Directional phase seam" note: the pattern (an optional complex-phase extension applied post-conj)
  and the discipline for reflecting an ENVI extension back into REQUIREMENTS/docs at phase close.

### Existing code the phase extends
- `crates/envi-engine/src/solver.rs` — `SolveJob` seam (add `forest`); post-conj application site for
  the real forest factor (alongside `directivity_gain_db`, D-04).
- `crates/envi-engine/src/propagation/terrain_effect/mod.rs` — `terrain_effect` / `TerrainEffect`
  two-channel composition (Eq. 332); `screen_channel` is where the transmission term is added (D-05).
- `crates/envi-engine/src/propagation/terrain_effect/screen.rs` — the generic screen⇄ground engine
  (Sub-models 4/5/6); opaque diffracted `h_coh_factor` producer that gating (D-10) branches from.
- `crates/envi-engine/src/transfer.rs` — `nord_ratio_to_transfer`, the ONE conj boundary (stays the
  only conj; the min-phase transmission is built native-side, D-05/D-08).
- `crates/envi-engine/src/directivity.rs` — the balloon eval + optional complex-phase precedent for
  how ENVI layers a phase-carrying extension.
- `crates/envi-engine/src/tensor.rs` — `TensorPair` (`H_coh` + `P_incoh_abs`), the readout law the
  two channels feed.
- `tools/nord2000_oracle/` — committed scipy oracle harness pattern for the new `a(f)` and min-phase
  fixtures (regenerated by `gen_*.py`; no Python at test time).
- `README.md` + engine module I/O headers — the documentation contract to update at phase close
  (there is currently no `crates/README.md`; the root `README.md` + module headers are authoritative).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`SolveJob` seam** (`solver.rs`): already carries optional per-path/source properties
  (`directivity_gain_db`, `directivity_phase_rad`, `weather`) applied post-conj — `forest` slots in
  the same way (D-01/D-04).
- **Two-channel `GroundResult` / `TerrainEffect`** (`terrain_effect/mod.rs`): the `h_coh_factor`
  (complex) + `p_incoh` (real) machinery the transmission term (D-05) and forest factor (D-03) plug
  into; Eq. 332 composition already blends complex channels linearly.
- **Screen sub-models** (`terrain_effect/screen.rs`): produce the opaque diffracted `h_coh_factor`
  that D-10's gating preserves untouched and D-05's transmission adds onto.
- **Committed scipy oracle harness** (`tools/nord2000_oracle/`): the established no-Python-at-test
  fixture pattern for the new `a(f)` and minimum-phase references (D-09/D-12).

### Established Patterns
- **Single-conj quarantine:** all native math in `propagation/` is `e^{−jωt}`; the only conjugation
  is `transfer::nord_ratio_to_transfer`. Transmission (D-05) and its min-phase (D-08) are built
  native-side; the forest real factor (D-04) is post-conj. Grep-gate for `.conj()` in `propagation/`
  stays at **zero**.
- **Optional phase-carrying extension:** the Phase-4 directional complex phase is the precedent for
  D-06 — an optional `e^{jφ}` that is bit-identical to the magnitude-only path when absent.
- **Honest capability gating + oracle/anchor ladder:** no false FORCE Pass; new physics validated by
  analytic anchors + committed oracles + a permanent regression (D-10/D-12).

### Integration Points
- `SolveJob.forest` → solver post-conj factor on `H_coh`/`P_incoh_abs` (D-01/D-03/D-04).
- `screen_channel` (in `terrain_effect`) → additive complex transmission `H_ff·T(f)` into the
  coherent factor, gated on `isolation: Option<_>` (D-05/D-10).
- One solve path, two callers (harness now, Phase 10 service later) — the seam additions must be on
  the promoted `envi_engine::solver`, not harness-private (ROADMAP coordination flag).

</code_context>

<specifics>
## Specific Ideas

- **Minimum-phase isolation (D-06)** is the defining "I want it like X" moment: treat the sound-
  isolation curve as a **filter** and derive its phase from its amplitude in a minimum-phase way
  (`φ_min = −Hilbert{ln|T|}`), rather than applying `R(f)` as a phase-less real attenuation. This is
  the user's explicit fidelity choice and the phase's most novel element.
- Forest as **excess attenuation on both channels** (D-03), explicitly not decorrelation — the user
  chose Nord2000-faithful behaviour over a more elaborate scattering-into-incoherent model.
- Opaque = a **distinct `None` state** with structural gating (D-10), not a large-`R` magic number —
  reflects the user's standing preference for bit-exact, structurally-guaranteed regression contracts
  (cf. `F→1⇒P_incoh→0`, MAC≡recompute).

</specifics>

<deferred>
## Deferred Ideas

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

</deferred>

---

*Phase: 5-Engine Extensions — Forest & Semi-Transparent Partitions*
*Context gathered: 2026-07-09*
