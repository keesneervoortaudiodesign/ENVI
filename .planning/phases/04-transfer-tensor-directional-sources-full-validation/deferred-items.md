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
