---
phase: 07-frontend-shell-scene-editing
plan: 04
subsystem: api
tags: [ts-rs, wire-types, codegen, no-drift, discriminated-union, quarantine, d-10, typescript]

# Dependency graph
requires:
  - phase: 07-frontend-shell-scene-editing (plan 01)
    provides: "IsolationSpectrumDto / AuthoredSpectrumDto / ForestParamsDto + Resolution enum (the new isolation/forest wire DTOs)"
  - phase: 07-frontend-shell-scene-editing (plan 03)
    provides: "Interpolate{Req,Resp} + Dgm{Req,Resp} service wire DTOs"
  - phase: 06 (service skeleton)
    provides: "JobStatus / JobId, project + calc request/response DTOs, envi-store dto.rs DTO set"
provides:
  - "web/src/generated/wire.ts — the committed single-file TS mirror of all ~32 Rust wire types (D-10)"
  - "JobStatus as a real TS discriminated union keyed on `state` (no zod fallback)"
  - "crates/envi-service/tests/wire_no_drift.rs — regenerate-and-compare byte-equality no-drift guard"
  - "ts-rs 12 TS derives on every envi-store + envi-service wire DTO"
affects:
  - "Phase-7 frontend api/store layer (07-08) imports types from web/src/generated/wire.ts instead of hand-writing them"
  - "Every future wire-DTO change: the no-drift test forces regeneration in Rust, not the browser"

# Tech tracking
tech-stack:
  added:
    - "ts-rs 12 (features: uuid-impl + serde-compat default; envi-service also no-serde-warnings) — on envi-store + envi-service ONLY, never envi-engine"
  patterns:
    - "generate-at-dev-time + commit-the-artifact + test-asserts-no-drift (oracle-fixture pattern, inverted): regenerate to a TempDir, assert byte-equality with the committed file"
    - "single deterministic export_all generator in ONE test binary that sees both crates' types — avoids the #[ts(export)] cross-crate parallel-write race and cwd-relative path fragility"
    - "ts-rs serde-compat renders #[serde(tag/rename_all)] enums as real TS discriminated unions / string-literal unions"
    - "generated artifact carries a DO-NOT-EDIT provenance banner; .gitattributes eol=lf pins it byte-stable under core.autocrlf"

key-files:
  created:
    - crates/envi-service/tests/wire_no_drift.rs
    - web/src/generated/wire.ts
    - .gitattributes
  modified:
    - crates/envi-store/Cargo.toml
    - crates/envi-store/src/dto.rs
    - crates/envi-store/src/interpolate.rs
    - crates/envi-service/Cargo.toml
    - crates/envi-service/src/jobs.rs
    - crates/envi-service/src/api/meta.rs
    - crates/envi-service/src/api/dgm.rs
    - crates/envi-service/src/api/projects.rs
    - crates/envi-service/src/api/calc.rs
    - Cargo.lock

key-decisions:
  - "Chose a deterministic single-binary export_all generator over ts-rs's #[ts(export)] auto-tests. The ~32 wire types span two crates; #[ts(export)] would run per-crate test binaries racing two processes on one shared file, and resolve output paths relative to each crate's ./bindings dir (cwd-relative std::path::absolute, no `..` normalization) — neither yields a single deterministic web/src/generated/wire.ts. Every type instead carries only #[ts(export_to=\"wire.ts\")]; wire_no_drift.rs (in envi-service, which depends on envi-store so it sees BOTH type sets) drives one explicit export_all pass into a chosen dir. ts-rs merges declarations alphabetically → byte-stable output regardless of call order. Documented as a deviation (Rule 3) below."
  - "No-drift byte-equality is newline-normalized (\\r\\n→\\n) AND the committed wire.ts is pinned to LF via .gitattributes. core.autocrlf=true on this machine would otherwise give the working-tree file CRLF while ts-rs emits LF, making a raw byte compare spuriously fail on Windows. Normalization touches ONLY line endings; every meaningful content change (renamed field, new/removed variant, reordered union) still diverges and fails — proven by a temporary #[ts(rename)] smoke test that made the test FAIL, then reverted."
  - "JobStatus verified (not assumed) to render as a real discriminated union: `{ \"state\": \"queued\" } | { \"state\": \"running\", progress: number, message: string, } | { \"state\": \"done\" } | { \"state\": \"failed\", reason: string } | { \"state\": \"cancelled\" }`. ts-rs 12 serde-compat honored #[serde(tag=\"state\", rename_all=\"snake_case\")] with no zod fallback (Gate 2 confirmed). A dedicated test asserts the union shape + the running/failed payloads in the committed file."
  - "u64 timestamps (created_at_unix/modified_at_unix) render as TS `bigint`; [f64;3]/[f64;2] as fixed tuples `[number, number, number]`; Vec<f64> as `Array<number>` (length erased — accepted, enforced server-side by BadBandCount, Pitfall 5); Uuid as `string` (uuid-impl); HashMap<Uuid, T> as `{ [key in string]: T }`. All valid TS — `cd web && npx tsc --noEmit` passes with wire.ts present."
  - "JobId (`#[serde(transparent)]` newtype) triggers a benign ts-rs 'cannot parse transparent' note; ts-rs inlines the newtype to `type JobId = string` regardless. Silenced with the ts-rs no-serde-warnings feature so clippy/build stay clean; the no-drift test verifies the emitted output is correct."

patterns-established:
  - "Pattern: wire types are GENERATED from the serde source of truth and committed; a Rust-side no-drift test (regenerate → byte-equal committed) makes divergence a failing cargo test, not a runtime browser bug (D-10)."
  - "Pattern: when a generated artifact spans multiple crates, drive one explicit deterministic generator from the downstream crate's test binary rather than relying on per-crate auto-export tests."

requirements-completed: [WEB-02, WEB-08, WEB-09, WEB-10]

# Metrics
duration: 40min
completed: 2026-07-10
status: complete
---

# Phase 07 Plan 04: Generated TypeScript Wire Contract (D-10) Summary

**All ~32 Rust serde wire DTOs across `envi-store` (16 DTOs + `Resolution`) and `envi-service` (16 request/response/enum types incl. `JobStatus`) now derive `ts-rs` 12 and export into a single committed `web/src/generated/wire.ts`; a deterministic regenerate-and-compare no-drift test makes any Rust field rename fail `cargo test` (proven), `JobStatus` renders as a real TS discriminated union with no zod fallback, and `envi-engine` stays byte-identical with its 3-dep quarantine intact.**

## Performance

- **Duration:** ~40 min (incl. reading ts-rs 12 source to verify the single-file export + uuid feature mechanics)
- **Tasks:** 2/2 complete, each committed atomically

## What was built

### Task 1 — ts-rs derives on the envi-store DTOs (commit 19e8b7a)
`ts-rs = "12"` (features `uuid-impl` + default `serde-compat`) added to `envi-store`. `#[derive(..., TS)]` + `#[ts(export_to = "wire.ts")]` on all 16 DTOs (`BandSpectrum`, `SubSource`, `Source`, `Receiver`, `Barrier`, `Building`, `GroundSegment`, `TerrainProfile`, `AuthoredSpectrum`, `IsolationSpectrum`, `ForestParams`, `Crs`, `Met`, `Settings`, `ProjectMeta`, `Conditioning`) plus the `Resolution` enum. The `[f64;105]`/`Vec<f64> → number[]` length erasure is documented inline as accepted (server-side `BadBandCount` enforcement).

### Task 2 — service derives + generated file + no-drift test (commit a336270)
`ts-rs = "12"` (`uuid-impl` + `no-serde-warnings`) added to `envi-service`. `TS` derived on `JobStatus`, `JobId`, `FreqAxisDto`, `Interpolate{Req,Resp}`, `Dgm{Req,Resp}`, `OriginDto`, `{Create,Update}ProjectRequest`, `SubmitResponse`, `Recondition{Request,Response}`, `Recompute{Request,Response,Reason}`. The committed `web/src/generated/wire.ts` (557 lines, DO-NOT-EDIT banner) holds the alphabetically-merged single-file union of every type. `crates/envi-service/tests/wire_no_drift.rs` provides the generator + byte-equality guard + JobStatus-union assertion + an `--ignored regenerate_committed_wire_ts` writer.

## How the no-drift test genuinely fails on drift (verified)

The test regenerates the whole contract into a fresh `TempDir` via `export_all` and asserts (newline-normalized) byte-equality with the committed `web/src/generated/wire.ts`. To prove it is not vacuous, a temporary `#[ts(rename = "band_db_DRIFT")]` was added to `BandSpectrumDto.band_db` (changes the emitted TS, leaves Rust serde untouched so it still compiles): `wire_ts_matches_committed_source` then **FAILED** with the expected out-of-sync assertion. The rename was reverted and the test returned green. A real field rename/add/removal or any serde-attribute change that alters the TS shape therefore fails `cargo test`, in Rust, before the browser ever sees `any`.

## Deviations from Plan

### 1. [Rule 3 - Blocking/robustness] Deterministic single-binary generator instead of `#[ts(export)]` auto-tests
- **Found during:** Task 2 wiring.
- **Issue:** The plan's suggested mechanism (`#[ts(export)]` auto-tests writing during `cargo test`) cannot deterministically produce ONE combined `wire.ts` spanning two crates: the per-crate test binaries run in separate processes that would race on the shared file, and ts-rs resolves each `export_to` relative to that crate's default `./bindings` dir via cwd-relative `std::path::absolute` (which does not normalize `..`), landing files in the wrong place.
- **Fix:** Every type carries only `#[ts(export_to = "wire.ts")]` (no `export` flag → no racing auto-tests). `wire_no_drift.rs` — in `envi-service`, which depends on `envi-store` and thus sees both type sets — drives one explicit `export_all` pass into a chosen dir; ts-rs merges declarations alphabetically for byte-stable output. The committed file is (re)written by the `--ignored regenerate_committed_wire_ts` writer.
- **Files:** `crates/envi-service/tests/wire_no_drift.rs`, all derive sites.
- **Commit:** a336270.

### 2. [Rule 2 - Correctness] LF pinning + newline normalization for the byte-equality contract
- **Found during:** Task 2 (Windows `core.autocrlf=true`).
- **Issue:** ts-rs emits `\n`; with autocrlf the committed working-tree file could be `\r\n`, spuriously failing a raw byte compare.
- **Fix:** Added `.gitattributes` (`web/src/generated/wire.ts text eol=lf`) and newline-normalize both sides in the test (line-endings only — real content drift still fails).
- **Commit:** a336270.

### 3. [Rule 1 - Cleanliness] `no-serde-warnings` on envi-service's ts-rs
- ts-rs cannot parse `#[serde(transparent)]` on `JobId` and prints a benign note (it inlines the newtype to `type JobId = string` correctly anyway). Enabled the `no-serde-warnings` feature so the quality gates stay clean; the no-drift test verifies the emitted `JobId` output.

## Verification

- `cargo clippy --all-targets -- -D warnings` — clean (no ts-rs warnings; `no-serde-warnings` applied).
- `cargo fmt --check` — clean.
- `cargo test -p envi-store -p envi-service` — all green, incl. `wire_no_drift` (2 tests + 1 ignored writer) and the pre-existing 30 store / service suites.
- `cd web && npx tsc --noEmit` — passes with `wire.ts` present.
- `cargo tree -p envi-engine -e normal --depth 1` — exactly `ndarray` + `num-complex` + `thiserror`; `git diff --quiet crates/envi-engine/` — empty. ts-rs did NOT leak into the engine.

## Notes for downstream plans

- **No app consumer is wired in this plan** (07-04's frontend file scope is only `web/src/generated/wire.ts`). The current `web/src` (store/map/App from 07-05/07-06) defines no wire-shaped types, so there is no D-10 violation to fix here. The api/fetch client that imports `../generated/wire` lands in 07-08 (RESEARCH §"web/src/api/*.ts"); `tsc --noEmit` already passes with the generated file present, so that import is drop-in.
- **Directivity-phase / balloon types** are not on the wire yet (deferred per the Phase-04 note); when they land they must derive `TS` + `#[ts(export_to = "wire.ts")]` and be added to `export_all_wire_types` in the no-drift test, or the test fails — which is the intended forcing function.

## Self-Check: PASSED
