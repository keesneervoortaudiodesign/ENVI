---
phase: 08-gis-ingestion-dgm
plan: 03
subsystem: api
tags: [envi-service, axum, reqwest, rustls, proxy, ssrf, byte-relay, cors, s3, glo30, worldcover]

# Dependency graph
requires:
  - phase: 06-*
    provides: envi-service axum 0.8 thin-HTTP layer — AppState, api_router(), ApiError -> structured JSON envelope, full-app contract-test harness
  - phase: 07-*
    provides: guard-first typed-error boundary pattern (reject-before-work), reused as reject-before-network in the relay
provides:
  - GET /api/v1/proxy/{source}/{*path} allowlisted, bytes-only GET/Range byte relay for the two CORS-blocked GIS sources (GLO-30, WorldCover)
  - Hardcoded SOURCES (id, host, path_prefix) allowlist — SSRF-proof by construction (no user URLs, GET-only, no cross-host redirect)
  - resolve_upstream(source, path) pure SSRF chokepoint (allowlist + prefix + traversal guard -> hardcoded https URL), unit-testable offline
  - Shared reqwest::Client on AppState (rustls TLS, redirect::Policy::none(), connect timeout) — 128 MiB streamed size cap
  - From<reqwest::Error> for ApiError (MED-1: log full server-side, generic 500 client body)
  - crates/envi-service/tests/contract_proxy.rs — offline allow + SSRF-rejection contract
affects: [08-04, 08-07, 08-landcover, terrain-import, envi-service-wasm-deploy]

# Tech tracking
tech-stack:
  added: [reqwest 0.12 (rustls-tls + stream, default-features=false — pure-Rust TLS, no C-linked backend)]
  patterns:
    - "Allowlisted byte relay (Pattern 5): {source} indexes a hardcoded (host, prefix) table; upstream URL built from hardcoded scheme+host+validated path; never a user-supplied URL"
    - "Reject-before-network: pure resolve_upstream() runs the allowlist/prefix/traversal guards and returns typed ApiError BEFORE any outbound request (mirrors tin::build_tin guard-first ordering)"
    - "GET-only via axum get(handler): any other method is a 405 at the router, never reaching the handler"
    - "Streamed relay under a cumulative size cap (Content-Length pre-check + per-chunk running total), no in-memory buffering, no byte transform"
    - "MED-1 error hygiene at the reqwest boundary: tracing::error! the full error (may embed the URL) but return generic Internal { detail }"

key-files:
  created:
    - crates/envi-service/src/api/proxy.rs
    - crates/envi-service/tests/contract_proxy.rs
  modified:
    - crates/envi-service/Cargo.toml
    - crates/envi-service/src/state.rs
    - crates/envi-service/src/api/mod.rs
    - crates/envi-service/src/error.rs
    - Cargo.lock

key-decisions:
  - "Reused ApiError directly (NotFound / BadRequest / Internal) rather than a new ProxyError enum — the three relay faults map cleanly onto existing variants and keep one error boundary."
  - "Client redirect policy is Policy::none() (no redirects at all), stricter than 'same-host only' — the two S3 sources serve 200/206 directly, so zero-redirect is the tightest SSRF stance."
  - "Non-GET is enforced structurally by registering get(relay) only (405 from axum method routing, empty body), not by a handler-level method check — matches the plan's route pattern and the GET-only prohibition; the contract test asserts 405."
  - "resolve_upstream is a pub pure function so the happy-path URL construction is proven offline (no S3 fetch) at the unit level, per the plan's 'unit-level check of the URL builder OR local stub' guidance."
  - "Size cap enforced twice: an early Content-Length pre-check plus a cumulative per-chunk guard in the streamed body (defends an upstream that omits or lies about Content-Length)."

patterns-established:
  - "Allowlisted server-side byte relay for CORS-blocked sources — the single new network surface; future CORS-blocked GIS sources add a row to SOURCES, nothing else"
  - "Pure, offline-testable SSRF chokepoint (resolve_upstream) separate from the async I/O handler"

requirements-completed: [DATA-01, DATA-02]

# Metrics
duration: 18 min
completed: 2026-07-11
status: complete
---

# Phase 8 Plan 3: envi-service allowlisted byte proxy Summary

**Bytes-only, rustls-backed GET/Range relay `GET /api/v1/proxy/{source}/{*path}` for the two CORS-blocked GIS sources (Copernicus GLO-30 S3, ESA WorldCover S3), SSRF-proof by a hardcoded (host, prefix) allowlist, pinned by offline contract tests.**

## Performance

- **Duration:** ~18 min
- **Started:** 2026-07-11T00:58:00Z
- **Completed:** 2026-07-11T01:16:00Z
- **Tasks:** 2
- **Files modified:** 7 (2 created, 5 modified)

## Accomplishments
- Added the phase's ONLY new server-side surface: an allowlisted, bytes-only GET/Range byte relay so the browser can reach the two CORS-blocked S3 sources; every other source (PDOK AHN, Overpass) stays direct-fetch and never touches the relay (D-02).
- SSRF-proof by construction: a hardcoded `SOURCES` `(id, host, path_prefix)` table, a pure `resolve_upstream()` guard that rejects unknown source (404), prefix escape, and `..` traversal (400) BEFORE any outbound request, and a client that follows NO redirects — the relay can only ever reach an allowlisted `(host, prefix)`.
- Pure-Rust TLS: `reqwest` with `rustls-tls` + `default-features = false` and `stream`; `cargo tree` confirms no `native-tls`/`openssl-sys` entered the dependency graph.
- DoS + hygiene guards: connect timeout, a 128 MiB streamed size cap (Content-Length pre-check + cumulative per-chunk), and a `From<reqwest::Error>` boundary that logs the full error server-side but returns a generic `500` (MED-1 — no host/path leak).
- Offline contract suite (`contract_proxy.rs`): full-app router cases for unknown-source / prefix-escape / non-GET, plus unit cases on the URL builder — no test hits the network.

## Task Commits

Each task was committed atomically:

1. **Task 1: Allowlisted byte-relay handler + rustls HTTP client + route** - `8db1c6e` (feat)
2. **Task 2: contract_proxy.rs — allow + SSRF-rejection contract tests** - `e537c56` (test)

**Plan metadata:** committed with this SUMMARY (docs: complete plan)

## Files Created/Modified
- `crates/envi-service/src/api/proxy.rs` - The byte relay: `SOURCES` allowlist, pure `resolve_upstream()` SSRF chokepoint, `relay` handler (GET/Range passthrough, streamed body, 128 MiB cap).
- `crates/envi-service/tests/contract_proxy.rs` - Offline allow + SSRF-rejection contract (3 full-router + 4 unit tests).
- `crates/envi-service/Cargo.toml` - Added `reqwest` (rustls-tls, stream, default-features=false).
- `crates/envi-service/src/state.rs` - `AppState.http` shared `reqwest::Client` built once at startup (redirect none, connect timeout).
- `crates/envi-service/src/api/mod.rs` - `pub mod proxy;` + route `/proxy/{source}/{*path}` (axum 0.8 brace + wildcard, GET-only).
- `crates/envi-service/src/error.rs` - `From<reqwest::Error> for ApiError` (MED-1 generic 500).
- `Cargo.lock` - reqwest/rustls dependency graph.

## Decisions Made
- Reused `ApiError` (no new `ProxyError`) — three relay faults map onto NotFound/BadRequest/Internal.
- `redirect::Policy::none()` (zero redirects) as the tightest SSRF stance; the S3 sources answer 200/206 directly.
- Non-GET enforced structurally by `get(relay)` (405, empty body) rather than a handler method check; the contract test asserts 405.
- `resolve_upstream` is a `pub` pure function so happy-path URL construction is verified offline at the unit level (no S3 fetch).
- Size cap enforced twice (Content-Length pre-check + cumulative per-chunk) to defend an upstream that omits/lies about Content-Length.

## Deviations from Plan

None - plan executed exactly as written. All acceptance criteria, the plan-level verification, and the four `<threat_model>` mitigations (T-08-03-01 allowlist/no-redirect, T-08-03-02 size-cap+timeout, T-08-03-03 MED-1 generic errors, T-08-03-04 rustls/no-openssl) were satisfied without unplanned work.

## Issues Encountered
- The running `envi-service.exe` holds a Windows lock on `target/debug/envi-service.exe`, so `cargo build -p envi-service` (and integration-test builds, which relink the crate binary) fail the final artifact swap with "Access is denied (os error 5)". Per CLAUDE.md the process was NOT killed. Verification was performed non-destructively by building/testing into an isolated `CARGO_TARGET_DIR=target/verify` (a different, unlocked path): the binary linked successfully there (confirming Task 1's `cargo build` acceptance — router constructs without panic) and the full `cargo test -p envi-service` + `cargo clippy --all-targets -- -D warnings` ran green. The isolated dir was removed after. The main-tree `target/debug/envi-service.exe` will relink normally the next time the user restarts the service.

## Quality Gates
- `cargo clippy -p envi-service --all-targets -- -D warnings` — clean (main tree, check-mode; and in the isolated dir with the new test).
- `cargo fmt --check` — clean.
- `cargo test -p envi-service` — green (existing lib/contract suites + `contract_proxy` 7/7), run in the isolated target dir; `wire_no_drift` still green (the relay adds no wire DTO, as designed).
- `grep -rn "native-tls\|openssl" crates/envi-service/Cargo.toml` — zero matches; `cargo tree` shows neither `native-tls` nor `openssl-sys` in the graph.

## User Setup Required
None - no external service configuration required (the relay targets public S3 sources; no credentials).

## Next Phase Readiness
- The single new server surface for Phase 8 is in place and SSRF-hardened. 08-04 (terrain import) and the landcover/buildings plans can fetch GLO-30 / WorldCover tiles through `/api/v1/proxy/{source}/{*path}` (same-origin, mockable in Playwright by routing `/api/v1/proxy/**`).
- No blockers. Note for the operator: restart the running `envi-service.exe` to pick up the new relay route in the live binary.

## Self-Check: PASSED
- `crates/envi-service/src/api/proxy.rs` — FOUND on disk.
- `crates/envi-service/tests/contract_proxy.rs` — FOUND on disk.
- Commit `8db1c6e` (feat, Task 1) — FOUND in git log.
- Commit `e537c56` (test, Task 2) — FOUND in git log.
- All plan `<acceptance_criteria>` and `<verification>` re-run green (7/7 contract tests, clippy, fmt, no-openssl/native-tls).

---
*Phase: 08-gis-ingestion-dgm*
*Completed: 2026-07-11*
