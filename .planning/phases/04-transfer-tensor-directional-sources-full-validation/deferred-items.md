# Deferred items — Phase 04

Out-of-scope discoveries logged during plan execution (not fixed by the owning plan).

## From 04-05 (NoiseModelling CNOSSOS cross-validation)

- **`crates/envi-engine/src/directivity.rs` — clippy `needless_range_loop`** at unit-test
  sites (`cargo clippy --all-targets -- -D warnings` fails: "the loop variable `band` is
  used to index `got`"). This file is **untracked / uncommitted in-flight work from a
  concurrent sibling plan (04-02 directional sources)** and was observed changing during
  04-05 execution. It is outside 04-05's scope (fixtures/tests/tools only) and must not be
  touched while a sibling executor owns it. 04-05's own files
  (`tests/oracle_noisemodelling.rs`, the fixture, the `tools/noisemodelling_oracle/` assets)
  are clippy-clean under `-D warnings` (`cargo clippy -p envi-harness --tests`) and
  fmt-clean. The owning plan (04-02) or the phase-completion `/gsd-code-review` gate should
  clear the directivity lint.

## Directional phase seam — wire when the coherent composition path lands

- **Feature (implemented, engine side):** `DirectivityBalloon` carries an optional
  per-band phase grid `Δφ(θ,φ,f)`; `eval_phase` / `eval_complex` return the
  directional phase / complex gain `10^{ΔL/20}·e^{+jΔφ}`. The solver applies it via
  `SolveJob::directivity_phase_rad` to the **coherent `H_coh` channel only**
  (`P_incoh` stays magnitude-only). Backward-compatible: a phase-free balloon is
  bit-identical to the magnitude-only path. This is an ENVI extension **beyond
  stock Nord2000** (real ΔL, incoherent) — see README "Directional phase" and
  `.claude/CLAUDE.md` complex/phase contract.
- **The gap (do NOT lose):** **no harness `SolveJob` construction site populates
  `directivity_phase_rad` yet.** The Nord2000 road-emission path (04-02) is
  correctly incoherent/real and must stay so. The phase field is only meaningful
  once a **coherent directional-source composition** path exists — i.e. when the
  calc/results path builds `SolveJob`s from user-drawn directional sources
  (Milestone 2, Phases 10–11; SRC-03 composition realized end-to-end).
- **When implementing that path:** evaluate the balloon with `eval_phase` (or
  `eval_complex`) at the src→rcv local direction and feed
  `SolveJob::directivity_phase_rad = Some(eval_phase(dir))` alongside
  `directivity_gain_db = Some(eval(dir))`. Add an end-to-end test that rotating a
  phased balloon changes coherent inter-sub-source interference (not just level).
- **Scope note:** SRC-02 was specified as real `ΔL(θ,φ,f)`; complex directivity is
  an accepted enhancement. Reflect it in REQUIREMENTS/SUMMARY at phase close.
