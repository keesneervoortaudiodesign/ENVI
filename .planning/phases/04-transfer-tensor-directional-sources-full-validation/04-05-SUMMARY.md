---
phase: 04-transfer-tensor-directional-sources-full-validation
plan: 05
subsystem: testing
tags: [noisemodelling, cnossos, oracle-fixture, cross-validation, divergence, iso-9613-1, air-absorption, band-index, toml, serde]

# Dependency graph
requires:
  - phase: 04-01
    provides: Phase-4 harness/tensor scaffolding (parallel-safe wave-2 sibling)
  - phase: 01-foundation
    provides: shipped direct_path divergence (−10·lg 4πR²) + ISO 9613-1 air absorption + 105-point 1/12-octave grid
provides:
  - Committed-fixture NoiseModelling CNOSSOS oracle for ENVI's shared sub-effects (VAL-03)
  - Three minimal GeoJSON scenes (free field / thin barrier / soft ground) + JVM-safe human-run recipe
  - Band-index equality gates (divergence ≤0.1 dB, air absorption ≤0.2 dB/octave) + report-only delta tables (barrier, ground)
  - Offline (no-Java) cross-check that activates automatically once real NoiseModelling output is committed
affects: [phase-04-verification, phase-04-secure, milestone-1-acceptance, VAL-03]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Committed-fixture oracle mirroring the scipy-oracle pattern: external tool run once by an operator, numeric outputs committed as provenance-tagged TOML, zero test-time dependency on the tool"
    - "Placeholder-fixture fail-soft: [meta] placeholder=true forces an honest SKIP (never a false Pass) until the one-time operator regeneration lands"
    - "Equality-gate vs report-only delta posture: only genuinely-identical physics is gated; different-model quantities are documented, not forced to agree"

key-files:
  created:
    - tools/noisemodelling_oracle/README.md
    - tools/noisemodelling_oracle/scenes/free_field.geojson
    - tools/noisemodelling_oracle/scenes/thin_barrier.geojson
    - tools/noisemodelling_oracle/scenes/soft_ground.geojson
    - crates/envi-harness/tests/fixtures/oracle/noisemodelling.toml
    - crates/envi-harness/tests/oracle_noisemodelling.rs
  modified: []

key-decisions:
  - "Java absent + research decision 'CNOSSOS reference values: re-implementing formulas cross-checks nothing' ⇒ ship the complete harness with placeholder rows that SKIP, rather than fabricate NoiseModelling output"
  - "Compare divergence/air-absorption strictly by band index at the 8 CNOSSOS octave centres (grid indices 16,28,40,52,64,76,88,100), never by nominal frequency"
  - "8 kHz air-absorption delta (Eq. 287 band conversion vs α·d centre value) is documented against a looser bound, not forced to the 0.2 dB octave gate"
  - "Barrier + ground are report-only expected-delta tables (Hadden-Pierce vs empirical Adif; complex Q̂ vs empirical G-model) — equality would be a comparison bug"
  - "Added a live analytic identity self-check (ENVI −10·lg 4πR² ≡ CNOSSOS directive 20·lg d + 11) so the divergence comparison arithmetic is proven now, independent of the placeholder fixture"

patterns-established:
  - "oracle_noisemodelling.rs copies oracle_refraction.rs structure: Deserialize fixture + Meta with named tolerances, concat! load(), band-index comparison, coverage assertions"
  - "Provenance [meta] block: oracle name + NM version + per-scene sha256[:16] + generation date + generator recipe; a changed scene must invalidate stale numbers"

requirements-completed: [VAL-03]

# Metrics
duration: ~35min
completed: 2026-07-08
status: complete
---

# Phase 4 Plan 5: NoiseModelling CNOSSOS Cross-Validation Summary

**Committed-fixture NoiseModelling v6.0.0 (CNOSSOS-EU) oracle for VAL-03: band-index equality gates for geometrical divergence (≤0.1 dB) and ISO 9613-1 air absorption (≤0.2 dB/octave) plus report-only expected-delta tables for barrier/ground — fully offline, no Java at test time, honest SKIP on the placeholder fixture until a one-time operator regeneration lands.**

## Performance

- **Duration:** ~35 min
- **Completed:** 2026-07-08
- **Tasks:** 2
- **Files created:** 6 (+1 deferred-items log)

## Accomplishments

- **Three minimal scenes** as GeoJSON (`free_field` hard-ground G=0, `thin_barrier` 4 m screen at x=50 m, `soft_ground` G=1), 15 °C / 70 % RH FORCE atmosphere, source 0.5 m / receiver 1.5 m at 100 m (direct distance ≈ 100.005 m).
- **JVM-safe human-run recipe** (`tools/noisemodelling_oracle/README.md`): one-time Temurin JRE install, NoiseModelling v6.0.0 offline run, per-octave-band `Adiv`/`Aatm`/`Abar`/`Aground` export — with the **NEVER force-close JVM/Gradle/Maven/language-server/VS Code** rule stated first and prominently.
- **Provenance-tagged fixture** (`noisemodelling.toml`) with `[meta]` carrying oracle name, NM version, per-scene sha256, generation date, generator recipe, tolerances, and `placeholder = true`.
- **Comparison test** (`oracle_noisemodelling.rs`, 5 tests): equality gates for divergence + air absorption **by band index** at the 8 octave centres, report-only delta tables for barrier + ground, coverage assertions against truncated fixtures, and a live analytic identity self-check.

## Task Commits

1. **Task 1: oracle scenes + recipe + provenance fixture** — `8ea91cf` (test)
2. **Task 2: band-index equality gates + report-only deltas** — `bd82f6b` (test)

_TDD note: Task 2 exercises Phase-1-shipped, already-oracle-pinned engine functions (`divergence_db`, `alpha_db_per_m`, `band_attenuation_db`). No RED/GREEN engine commit split applies — the subject code pre-exists; the new artifact is the cross-validation harness. The live identity test provides a genuine pass/fail gate now._

## Sub-effect gating outcome

| Sub-effect | Posture | Status this session |
|-----------|---------|---------------------|
| Geometrical divergence | **EQUALITY** ≤0.1 dB by band index | Gate implemented + verified live; SKIP on placeholder. Live directive-identity self-check PASSES (Δ = 0.0079 dB). |
| ISO 9613-1 air absorption | **EQUALITY** ≤0.2 dB/octave (8 kHz documented) | Gate implemented + verified live; SKIP on placeholder. |
| Barrier insertion loss | **REPORT-ONLY** expected delta | Table implemented (no equality assertion); SKIP on placeholder. |
| Flat-ground excess attenuation | **REPORT-ONLY** expected delta | Table implemented (no equality assertion); SKIP on placeholder. |

## NoiseModelling fixture status: honestly gated-Skip (not generated)

Java is **not installed** on this machine (`java -version` fails) and the mandate forbids installing/faking it. Per the plan's explicit fallback and 04-RESEARCH's rejection of "re-implementing CNOSSOS formulas", the fixture ships with **clearly-marked zero placeholder rows** and `placeholder = true`; the four fixture-driven gates **SKIP** with an honest message pointing to the regeneration recipe. This is honest-green (a missing-fixture skip is acceptable; a fabricated cross-check is not). The gates activate automatically once an operator runs the recipe and sets `placeholder = false`.

**No-false-pass verification:** temporarily flipping `placeholder = false` against the zero rows made the divergence and air-absorption gates **fail loudly** (`ENVI 50.9925 dB vs CNOSSOS 0.0000 dB, Δ=50.9925 dB`), confirming there is no silent false-pass path. Fixture restored.

## Test / clippy / fmt status

- `cargo test -p envi-harness --test oracle_noisemodelling`: **5 passed** (1 live identity gate + 4 honest SKIPs).
- `cargo clippy -p envi-harness --tests -- -D warnings`: **clean** (this plan's files).
- `cargo fmt --check`: **clean**.
- `cargo clippy --all-targets -- -D warnings` (whole workspace): **fails**, but only on `crates/envi-engine/src/directivity.rs` — untracked/uncommitted in-flight work from a **concurrent sibling plan (04-02 directional sources)**, observed changing mid-execution. Out of 04-05 scope; must not be touched while a sibling executor owns it. See `deferred-items.md`.

## Decisions Made

See `key-decisions` frontmatter. Core call: honest gated-Skip harness over fabricated numbers, respecting both the "never fake" mandate and the research decision that self-reimplementing CNOSSOS cross-checks nothing.

## Deviations from Plan

### Out-of-scope discovery (logged, not fixed)

**1. [Scope boundary] Pre-existing clippy `needless_range_loop` in a concurrent sibling file**
- **Found during:** Task 2 verification (`cargo clippy --all-targets`)
- **Issue:** `crates/envi-engine/src/directivity.rs` (untracked, uncommitted work from the concurrent 04-02 directional-sources executor) fails `-D warnings`.
- **Action:** NOT fixed — outside 04-05's fixtures/tests/tools scope and actively owned by a live sibling executor (file changed between two clippy runs). Logged to `deferred-items.md` for 04-02 / the phase-completion `/gsd-code-review` gate.
- **Files modified:** none (log only).

---

**Total deviations:** 0 code auto-fixes; 1 out-of-scope item logged.
**Impact on plan:** None. This plan's own files are test/clippy/fmt-clean; the harness is complete and correct.

## Issues Encountered

- Concurrent sibling execution: `directivity.rs`/`lib.rs` uncommitted changes from a parallel plan meant the workspace-wide clippy gate is red on files I do not own. Resolved by scoping verification to this plan's crate/targets and staging only 04-05 files in each commit (never `git add -A`).

## User Setup Required

**One-time, operator-driven (optional, to activate the fixture-driven gates):** install a JRE and run NoiseModelling v6.0.0 once per `tools/noisemodelling_oracle/README.md`, then set `[meta] placeholder = false` in `noisemodelling.toml` with the exported per-octave-band values. Not required for the workspace to build/test — the tests SKIP cleanly without it.

## Next Phase Readiness

- VAL-03 harness delivered and offline; divergence identity is proven live, and the full CNOSSOS equality/report gates are wired and verified to fail-loud on mismatch.
- Blocker for a fully-green VAL-03 numeric cross-check: a one-time NoiseModelling run (Java). Until then the cross-check honestly SKIPs.
- Concurrent 04-02 directivity clippy lint should be cleared by its owning plan / the phase-completion gate before Phase 4 is marked complete.

## Self-Check: PASSED

All 6 created files present; both task commits (`8ea91cf`, `bd82f6b`) exist in history.

---
*Phase: 04-transfer-tensor-directional-sources-full-validation*
*Completed: 2026-07-08*
