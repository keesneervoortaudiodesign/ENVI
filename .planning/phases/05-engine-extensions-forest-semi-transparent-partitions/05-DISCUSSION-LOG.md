# Phase 5: Engine Extensions — Forest & Semi-Transparent Partitions - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-09
**Phase:** 5-Engine Extensions — Forest & Semi-Transparent Partitions
**Areas discussed:** Forest seam & geometry, Forest scattering channel, Transmission combine point, Opaque-limit regression

---

## Forest seam & geometry

| Option | Description | Selected |
|--------|-------------|----------|
| SolveJob scalar d + params | Caller supplies pre-computed through-forest length `d` + params on `SolveJob`; engine applies `A=d·a(f)`. Mirrors directivity/PropagationPath seam. | ✓ |
| List of forest segments | `SolveJob` carries a list of (entry, exit, params) spans; engine sums d. More general, heavier seam now. | |
| Engine intersects TerrainProfile | Engine computes d by intersecting a forest region on the profile. Puts geometry in the pure-math crate; no M1 geometry source to feed it. | |

**User's choice:** SolveJob scalar d + params — `SolveJob { forest: Option<ForestCrossing { d_m, density, stem_radius, kp, absorption }> }`.
**Notes:** M1 is FORCE-fed with no forest cases; real path extraction is Phase 9.

### Forest — a(f) home (follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| Engine computes a(f) | `ForestCrossing` carries raw physical params; `envi-engine` owns the `a(f)` formula. Directly oracle-testable. | ✓ |
| Harness precomputes a(f) | Harness passes `[f64; N_BANDS]`; engine only multiplies by d. Breaks the crate pattern. | |

**User's choice:** Engine computes a(f) — pure math in `envi-engine`, harness passes params only.

---

## Forest scattering channel

| Option | Description | Selected |
|--------|-------------|----------|
| Real factor on both channels | `10^(−A/20)` on `H_coh` (arg unchanged), `10^(−A/10)` on `P_incoh`. Nord2000 excess-attenuation; phase-preserving; F→1⇒P_incoh→0 bit-exact. | ✓ |
| Scatter into P_incoh | Treat scattered energy as decorrelated (remove from H_coh, add to incoherent), like SM7 turbulence. No Nord2000 spec basis; complicates bit-exact regression. | |

**User's choice:** Real factor on both channels.
**Notes:** Applied solver-side post-conj like `directivity_gain_db` (follows directly — not separately asked).

---

## Transmission combine point

| Option | Description | Selected |
|--------|-------------|----------|
| Inside propagation/ screen | Add transmitted term to the Nord2000-native `h_coh_factor` (e^{−jωt}, before the single conj). Genuine complex-pressure combination; keeps one conj boundary. | ✓ |
| Post-conj in solver | Add `H_ff·10^(−R/20)` on the ENVI post-conj side like directivity. Splits "combine as complex pressure" across the conj seam; phase-convention risk. | |

**User's choice:** Inside propagation/ screen (option 1) — **and a refinement**: rejected the plain real
`10^(−R/20)` amplitude factor. The user wants the isolation treated as a **filter whose phase follows its
amplitude in a minimum-phase way** — a minimum-phase response derived from the sound-isolation curve.
**Notes:** Decision captured as D-05 through D-09 in CONTEXT.md: complex transmission filter
`T(f) = 10^(−R/20)·e^{jφ_min}`, `φ_min = −Hilbert{ln|T|}`, built native-side ahead of the single conj.
Flagged as an ENVI extension beyond stock Nord2000 (extends ENG-10), with the phase sign/convention as a
verify item and the min-phase reconstruction as oracle-testable engine math.

---

## Opaque-limit regression

| Option | Description | Selected |
|--------|-------------|----------|
| Structural gating | `isolation: None` ⇒ exact existing opaque path; transmission + min-phase never constructed. Bit-exact; never evaluates Hilbert on −∞. Permanent regression test. | ✓ |
| Numeric with clamp | Always run transmission with R clamped ≤ ~120 dB. Perturbs opaque result ~1e−6 (not bit-exact); "opaque" becomes a magic number. | |

**User's choice:** Structural gating.
**Notes:** Sharpened by the min-phase decision — `ln|T|→−∞` as `R→∞`, so the opaque path MUST be
structurally skipped, never fed to the Hilbert transform.

---

## Claude's Discretion

- Module placement/naming within `envi-engine` (forest module vs fold into `terrain_effect`; where the
  min-phase/transmission helpers live), exact `ForestParams`/`IsolationSpectrum` field types, and the
  internal Hilbert/cepstrum algorithm — left to research/planning provided the seams, conventions, and
  regression contract in CONTEXT.md hold.

## Deferred Ideas

- ISO 9613-2 forest distance-clamp regimes (`<10→0`, `10–20→A₁₀₋₂₀`, `≥200→200·a(f)`) — ISO variant, not
  Nord2000; recorded only so the planner doesn't import it (project is Nord2000-only).
- Reflection-path transmission (semi-transparent screen modifying its reflected path by R(f)) — not needed
  in the decided scope (single straight-through transmitted path).
- Multiple/heterogeneous forest crossings on one path — single scalar `d_m` chosen; list-of-segments seam
  deferred to Phase 9 path extraction if needed.
