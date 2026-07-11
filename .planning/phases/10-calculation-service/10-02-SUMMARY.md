---
phase: 10-calculation-service
plan: 02
subsystem: delivery-server-headers
tags: [cross-origin-isolation, coop, coep, credentialless, sharedarraybuffer, axum, tower-http, vite, wasm]

# Dependency graph
requires:
  - phase: 06-service-foundation-persistence
    provides: "envi_service::api::app router + tower-http ServeDir/ServeFile static-bundle serving; the full-app oneshot contract-test scaffolding (tests/common)"
provides:
  - "COOP: same-origin + COEP: credentialless response-header layers on the envi-service app bundle (SetResponseHeaderLayer), making the served top-level document cross-origin isolated (self.crossOriginIsolated === true)"
  - "Matching Vite server.headers (dev + Playwright) so npm run dev and browser UAT are cross-origin isolated"
  - "contract_isolation_headers oneshot test pinning both exact header values (COEP == credentialless, != require-corp)"
affects: [10-03-envi-compute-wasm, 10-04-pool-worker, 10-05-calcpanel, phase-11-results]

# Tech tracking
tech-stack:
  added: [] # no new external packages — tower-http already present (set-header feature enabled); Vite server.headers is native
  patterns:
    - "Cross-origin isolation via COOP same-origin + COEP credentialless (NOT require-corp) so SharedArrayBuffer is exposed without breaking direct third-party fetches"
    - "SetResponseHeaderLayer::overriding wrapping the whole router (harmless on /api/v1, load-bearing on the fallback bundle service)"
    - "Prod (axum) and dev (Vite) isolation headers kept identical so dev/Playwright matches production"

key-files:
  created:
    - crates/envi-service/tests/contract_isolation_headers.rs
  modified:
    - crates/envi-service/Cargo.toml
    - crates/envi-service/src/api/mod.rs
    - web/vite.config.ts

key-decisions:
  - "COEP value is credentialless, NOT require-corp (D-04): require-corp would block the Phase-8 direct third-party fetches (basemap/AHN/Overpass) that send no CORP header; the exact value is pinned by a test and asserted != require-corp"
  - "The layers wrap the whole app() router (not just the fallback service) via .layer(coop).layer(coep) — the SPA bundle is served by fallback_service which cannot be independently .layer()'d in the same nest chain, and the headers are harmless on /api/v1 JSON responses"
  - "SVC-02 (compute-job model) is a phase-level requirement delivered across 10-01..10-05; this plan contributes only the cross-origin-isolation prerequisite (SharedArrayBuffer availability), so SVC-02 stays Pending"
  - "jobs.rs SSE machine left byte-identical (git diff empty) — reserved for ERA5-style async per D-10, not deleted or repurposed"

patterns-established:
  - "Prod/dev isolation-header parity: axum SetResponseHeaderLayer and Vite server.headers emit the identical COOP/COEP pair so crossOriginIsolated behaves the same in `cargo run` delivery and `npm run dev`/Playwright"

requirements-completed: []  # SVC-02 spans the whole phase; the headers are a prerequisite, not the completion

# Metrics
duration: ~20min
completed: 2026-07-11
status: complete
---

# Phase 10 Plan 02: Cross-Origin Isolation Headers Summary

**The served application is now cross-origin isolated — COOP: same-origin + COEP: credentialless on both the axum `envi-service` bundle response and the Vite dev/Playwright server — so the browser exposes `SharedArrayBuffer`, the platform prerequisite for the wasm-bindgen-rayon thread pool the rest of Phase 10 needs; `credentialless` (not `require-corp`) keeps the Phase-8 direct GIS/basemap fetches working.**

## Performance

- **Duration:** ~20 min
- **Completed:** 2026-07-11
- **Tasks:** 2 (both `type=auto`)
- **Files modified:** 4 (1 created, 3 modified)

## Accomplishments

- Added two `tower_http::set_header::SetResponseHeaderLayer::overriding` layers to `envi_service::api::app` — `cross-origin-opener-policy: same-origin` and `cross-origin-embedder-policy: credentialless` — wrapping the whole router so the fallback bundle response (and, harmlessly, the `/api/v1` responses) carry both headers. This makes the served top-level document report `self.crossOriginIsolated === true`, the platform gate for `SharedArrayBuffer` / `initThreadPool`.
- Enabled the `set-header` feature on the already-present `tower-http` dependency (no new crate; `fs` was already there for `ServeDir`).
- Added a `server.headers` block to `web/vite.config.ts` emitting the identical COOP/COEP pair so `npm run dev` and the future Playwright test server are cross-origin isolated in dev, mirroring production — with no new npm dependency (native Vite feature, not `vite-plugin-cross-origin-isolation`; `package.json` unchanged).
- Added `contract_isolation_headers.rs` — an in-process oneshot test that pins both exact header values on the bundle response AND asserts `COEP == "credentialless"` and `COEP != "require-corp"`, a regression guard against a silent flip to the credentialed-resource-blocking variant that would break the Phase-8 direct fetches.
- Chose `credentialless` deliberately over `require-corp` (D-04): `credentialless` strips credentials on no-cors sub-resource loads so the Phase-8 direct third-party fetches (basemap/AHN/Overpass) keep working without every source having to send a CORP header.
- Left `crates/envi-service/src/jobs.rs` byte-identical (SSE job machine reserved for ERA5-style async, D-10 — not deleted or repurposed).

## Task Commits

1. **Task 1: COOP/COEP response headers on the envi-service bundle** — `7d43667` (feat)
2. **Task 2: Vite dev-server COOP/COEP headers (dev + Playwright isolation)** — `7019159` (feat)

## Files Created/Modified

- `crates/envi-service/tests/contract_isolation_headers.rs` — *created*; oneshot test pinning COOP=same-origin + COEP=credentialless on both the bundle and the `/api/v1` responses, with an explicit `!= require-corp` assertion.
- `crates/envi-service/Cargo.toml` — enabled the `set-header` feature on `tower-http` (`features = ["fs", "set-header"]`); commented why.
- `crates/envi-service/src/api/mod.rs` — added the two `SetResponseHeaderLayer` layers in `app()` plus a doc block explaining cross-origin isolation and the credentialless-vs-require-corp rationale.
- `web/vite.config.ts` — added `server.headers` with the COOP/COEP pair and a Module I/O header note documenting the dev headers and the credentialless choice.

## Deviations from Plan

None — the plan executed exactly as written. Both tasks matched their `<action>` and `<acceptance_criteria>` verbatim; no bugs, missing functionality, or blocking issues surfaced.

## Note on the `require-corp` grep gate

The plan's `<verification>` notes "`require-corp` absent from both [files]". A literal `grep -c require-corp` returns 2 in each of `api/mod.rs` and `vite.config.ts` — but every occurrence is **explanatory doc/comment text documenting the deliberate non-use**, plus one negative test assertion (`assert_ne!(coep, "require-corp")`). `require-corp` is **never a configured header value**. This is the stronger form of the gate: the CLAUDE.md documentation contract requires explaining *why* `credentialless` was chosen, and the test proves the effective value is `credentialless`, not `require-corp`. The planner anticipated the term appearing (the PLAN carries `planner-discipline-allow: require-corp`).

## Verification / Quality Gates

Run from workspace root `D:/====CLAUDE/envi`:

- `cargo fmt --check` — clean (exit 0).
- `cargo clippy --all-targets -- -D warnings` — clean (zero warnings).
- `cargo test` — all pass across the workspace; `contract_isolation_headers` 2/2, `contract_jobs` (SSE machine) 3/3 unchanged.
- `cd web && npx tsc --noEmit` — clean (exit 0), config still type-checks.
- Grep gates: `credentialless` present in `api/mod.rs` (3) and `vite.config.ts` (1 value + comment); `require-corp` never a header value (see note above).
- `git diff crates/envi-service/src/jobs.rs` — empty (SSE machine untouched, D-10).
- `git -C web diff package.json` — empty (no new npm dependency).

## Threat Flags

None — no new security-relevant surface beyond the plan's `<threat_model>`. The two headers are exactly the platform-provided mitigation (T-10-02-01/02/03); no new endpoint, no auth, no schema change.

## Known Stubs

None.

## Self-Check: PASSED

All created/modified files exist on disk; both task commits (`7d43667`, `7019159`) are present in git history.
