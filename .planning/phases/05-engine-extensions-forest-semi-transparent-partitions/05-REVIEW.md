---
phase: 05-engine-extensions-forest-semi-transparent-partitions
reviewed: 2026-07-09T00:00:00Z
depth: standard
files_reviewed: 16
files_reviewed_list:
  - crates/envi-engine/src/forest.rs
  - crates/envi-engine/src/propagation/transmission.rs
  - crates/envi-engine/src/solver.rs
  - crates/envi-engine/src/propagation/terrain_effect/mod.rs
  - crates/envi-engine/src/lib.rs
  - crates/envi-engine/src/propagation/mod.rs
  - crates/envi-harness/src/lib.rs
  - crates/envi-harness/tests/opaque_regression.rs
  - crates/envi-harness/tests/oracle_forest.rs
  - crates/envi-harness/tests/oracle_minphase.rs
  - crates/envi-harness/tests/solve_baseline.rs
  - crates/envi-harness/tests/mac_identity.rs
  - crates/envi-harness/tests/tensor_budget.rs
  - crates/envi-harness/tests/terrain.rs
  - tools/nord2000_oracle/gen_forest_fixtures.py
  - tools/nord2000_oracle/gen_minphase_fixtures.py
findings:
  critical: 0
  warning: 1
  info: 2
  total: 3
status: issues_found
---

# Phase 5: Code Review Report

**Reviewed:** 2026-07-09
**Depth:** standard
**Files Reviewed:** 16
**Status:** issues_found

## Summary

Phase 5 adds forest Sub-Model 10 excess attenuation (`forest.rs`, engine-root,
post-conj) and a minimum-phase semi-transparent-partition transmission filter
(`propagation/transmission.rs`, native `e^{−jωt}`, pre-conj), wiring both through
`SolveJob`/`solver.rs` and `terrain_effect`.

The load-bearing invariants were verified and hold:

- **Single-conj quarantine intact.** No `.conj()` call exists anywhere in
  `propagation/` — the native phase negation in `TransmissionFilter::from_isolation`
  is written as an explicit negative sine `Complex::new(mag*c, mag*(-s))`
  (transmission.rs:236). Only doc-comment references to `.conj()` appear.
- **Dep quarantine intact.** `envi-engine/Cargo.toml` still carries only
  `ndarray + num-complex + thiserror`; the 208-point cepstral transform is
  hand-rolled O(M²) DFTs, no FFT/linalg crate introduced. `#![deny(unsafe_code)]`
  preserved.
- **Two-channel contract preserved.** Forest scales BOTH channels by a positive
  real factor (`arg(H_coh)` untouched, `P_incoh` magnitude-only); transmission
  joins the coherent channel ONLY (`base.h_coh_factor + t`), never `p_incoh`.
  The `F→1 ⇒ P_incoh→0` exact-zero survives (a real scale of `0.0` stays `0.0`).
- **Opaque limit is structural.** `isolation: None` and `forest: None` take the
  pre-extension code path (`match transmission { None => base.h_coh_factor }`,
  no `+ 0.0`); the `opaque_regression`/`solve_baseline` bit-exact fixtures pin it.
- **Numerics.** The PCHIP endpoint/interior derivatives match scipy
  `PchipInterpolator`; the even-mirror cepstral fold matches the numpy oracle;
  the `kf == 0.0`/`t == 0.0` float-equality guards compare against values that are
  exactly `0.0` by construction; forest is robust to extreme finite inputs
  (`inf.min(1.0) = 1.0`, all axes clamped, no `log10(0)`).

The code is high quality and heavily test-pinned. One robustness/threat-boundary
gap and two cosmetic items follow.

## Warnings

### WR-01: `IsolationSpectrum::new` accepts extreme finite `R`, which overflows the min-phase DFT to NaN — contradicting the stated "NaN can never poison the transfer tensor" guarantee

**File:** `crates/envi-engine/src/propagation/transmission.rs:107-114` (constructor),
`:161-193` (`min_phase_parts`), `:227-239` (`from_isolation`)

**Issue:** `IsolationSpectrum::new` validates only `value.is_finite() && value >= 0.0`
and explicitly documents accepting "any large **finite** `R`" (line 98), with the
mitigation rationale (threat T-05-02-01, mod.rs:214-226) that this keeps
`ln|T|` finite so "a NaN can never poison the transfer tensor."

That guarantee is incomplete. `ln|T|` stays finite, but the O(M²) cepstral
accumulation does not: for `R` on the order of `1e307` dB or larger (still a
finite `f64 ≥ 0` that passes the constructor), the un-normalized `idft` sum of
208 terms each ~`R·ln10/20` overflows to `±inf`, the forward DFT then mixes
`+inf`/`-inf` into `NaN`, and `phi[n]` becomes `NaN`. `from_isolation` then does
`phi[n].sin_cos()` → `(NaN, NaN)`, and although `mag = 10^(-R/20)` has underflowed
to `0.0`, `Complex::new(0.0 * NaN, 0.0 * (-NaN)) = (NaN, NaN)`. That NaN is added
into `h_coh_factor` in `screen_channel`, i.e. it reaches the transfer tensor —
exactly the outcome the constructor claims to prevent at the caller trust
boundary. (Impact is bounded: a downstream `TensorSink` rejects non-finite cells
as a typed `SinkError`, so it surfaces as an error rather than silent corruption
— but the min-phase kernel itself has no finiteness guard and the documented
invariant is not actually delivered.)

The `t10` test only exercises `flat(300.0)`; the pathological-but-finite regime
is untested.

**Fix:** Reject physically-implausible magnitudes at the constructor so the
kernel's inputs are bounded (a passive partition's `R` is at most a few hundred
dB), turning the guarantee into a real one:

```rust
// A passive partition's sound reduction index is physically ≤ a few hundred dB;
// beyond that the O(M²) cepstral accumulation can overflow to ±inf → NaN φ.
const MAX_R_DB: f64 = 1_000.0;
for (band, &value) in r_db.iter().enumerate() {
    if !value.is_finite() || value < 0.0 || value > MAX_R_DB {
        return Err(PropagationError::InvalidIsolationSpectrum { band, value });
    }
}
```

(Alternatively, add a `phi.is_finite()` assertion in `min_phase_parts` returning
a typed error — but bounding `R` at the trust boundary is the cleaner mitigation
and matches the module's own "validate at the seam" discipline.)

## Info

### IN-01: `forest_delta_l` can return a small positive `ΔL_s` (~+0.01 dB), so the repeated "ΔL_s ≤ 0 / forest can only attenuate" claim is not strictly guaranteed

**File:** `crates/envi-engine/src/forest.rs:405-406` (and the invariant statements
at `:36-41`, `:385-387`; solver.rs:246-250)

**Issue:** `A_e = table9_delta_l(...) + 20·log₁₀(8·R′)`. At `R′ = 0.0625` the
table value is exactly `6.0` and `20·log₁₀(0.5) = −6.0206`, so `A_e ≈ −0.02`;
PCHIP interpolation near that corner can leave `A_e` marginally positive, making
`ΔL_s = 1.25·k_f·T·A_e` slightly positive (up to ~`+0.0125` dB). The `f4` sweep
test explicitly tolerates this (`assert!(dls <= 0.01)`, forest.rs:524), yet the
module doc and solver comments assert an unconditional `ΔL_s ≤ 0` ("forest can
only attenuate"). The exact-zero `P_incoh` contract is unaffected (`0·factor = 0`
regardless of sign), so this is documentation-vs-behavior drift, not a bug.

**Fix:** Either clamp the result to `≤ 0` (`(1.25*kf*t*a_e).clamp(DELTA_L_FLOOR_DB, 0.0)`)
to make the "attenuate only" claim exact, or soften the comments to state the
`≤ ~0.01 dB` interpolation-corner tolerance the `f4` test already documents.

### IN-02: Incorrect inequality in a forest solver test comment

**File:** `crates/envi-engine/src/solver.rs:552`

**Issue:** The comment reads `// arg unchanged (ΔL_s ≥ 0 ⇒ mag > 0, a positive
real factor).` — `ΔL_s` is `≤ 0`, not `≥ 0`. The assertion itself is correct
(`10^(ΔL_s/20) > 0` for any finite `ΔL_s`, so `arg` is preserved); only the
parenthetical is wrong.

**Fix:** Change `ΔL_s ≥ 0` to `ΔL_s ≤ 0` (the point is only that `10^x > 0`).

---

_Reviewed: 2026-07-09_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
