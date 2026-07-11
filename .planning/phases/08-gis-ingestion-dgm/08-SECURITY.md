---
phase: 08
phase_name: gis-ingestion-dgm
status: secured
asvs_level: 1
block_on: high
threats_total: 25
threats_closed: 25
threats_open: 0
register_authored_at_plan_time: true
audited: 2026-07-11
---

# Phase 08 — GIS Ingestion & DGM — Security Verification

**Verdict: SECURED.** All 25 plan-time threat mitigations verified present and
effective in the shipped code (verify-mitigations mode; register authored at plan
time across the eight `08-0N-PLAN.md` `<threat_model>` blocks). No HIGH (or any)
threat left open. ASVS L1, block-on high.

The new server-side attack surface is exactly one component — the allowlisted
byte relay (`crates/envi-service/src/api/proxy.rs`) — mapped to T-08-03-0x. The
new WASM crate is boundary-only with typed-error delegation. No unmapped surface
appeared. Implementation files were treated as read-only during the audit.

## Threat Register — verification

| Threat ID | Category | Disp. | Status | Evidence (file:line) |
|-----------|----------|-------|--------|----------------------|
| T-08-01-01 | Tampering (reprojection drift) | mitigate | CLOSED | `crates/envi-geo/tests/oracle_rd.rs:54-107` — proj4rs vs committed pyproj fixture ≤ 1.0 m both directions; fixture `crates/envi-geo/tests/fixtures/oracle/rd_landmarks.toml` |
| T-08-01-02 | DoS (ctor panic) | mitigate | CLOSED | `crates/envi-geo/src/crs.rs:168-179` fallible `RdNewCrs::new`→`GeoError::Proj`; test `crs.rs:359-364` |
| T-08-02-01 | DoS (decompression bomb) | mitigate | CLOSED | `crates/envi-gis/src/cog/window.rs:170-196` — `guard_image_count` + `max_decoded_px` budget (`checked_mul` overflow guard) BEFORE the read loop `:237-244`; guard ordering survived the `decode_window_generic` refactor |
| T-08-02-02 | DoS/Tampering (malicious IFD) | mitigate | CLOSED | `crates/envi-gis/src/cog/header.rs:89-103` IFD chain cap `MAX_IFD_IMAGES=64`→`TooManyImages`; degenerate dims rejected `:66-70`; typed errors, no panic |
| T-08-02-03 | Tampering (nodata/padding) | mitigate | CLOSED | `geo_tags.rs:116-127` parses `GDAL_NODATA`; `window.rs:143-144,301-302` drop nodata + non-finite to holes (never 0.0); cropped tiles `:225-227` |
| T-08-02-04 | Tampering (geotransform) | mitigate | CLOSED | `geo_tags.rs:57-104` transform from `ModelPixelScaleTag`/`ModelTiepointTag`, `MissingGeoTag` if absent; non-square via independent `sx`/`sy` |
| T-08-03-01 | InfoDisclosure/SSRF **(HIGH)** | mitigate | CLOSED | `crates/envi-service/src/api/proxy.rs:52-101` hardcoded `(id,host,prefix)` allowlist (404 unknown); prefix `starts_with` + `".."` traversal guard (400) before any I/O; URL from hardcoded `https`+host only; `state.rs:84-86` `redirect::Policy::none()`; GET-only. No user host/scheme/URL path; no cross-host bypass |
| T-08-03-02 | DoS (oversized upstream) | mitigate | CLOSED | `proxy.rs:69` `MAX_RELAY_BYTES=128 MiB` from Content-Length (`:141-148`) AND cumulative while streaming (`:165-173`); `state.rs:86` `connect_timeout(15s)` |
| T-08-03-03 | InfoDisclosure (error leak) | mitigate | CLOSED | `crates/envi-service/src/error.rs:106-122` `From<reqwest::Error>` logs full via `tracing::error!`, returns generic `Internal`; oversize/non-2xx also generic (`proxy.rs:134-147`) |
| T-08-03-04 | Supply-chain (TLS) | mitigate | CLOSED | `crates/envi-service/Cargo.toml:50-51` reqwest `default-features=false, features=["rustls-tls"]`; zero `openssl`/`native-tls` across all crates |
| T-08-04-01 | DoS (unbounded points) | mitigate | CLOSED | `crates/envi-gis/src/terrain.rs:40` `MAX_TERRAIN_POINTS=50_000` (< envi-dgm 500k); `decimate_window` stride-bounded `:76-88`; test `:307-324` |
| T-08-04-02 | Tampering (poisoned Overpass) | mitigate | CLOSED | `crates/envi-gis/src/buildings.rs:218-242` `validate_ring` + skip-and-report; `parse_leading_positive:98-105` rejects non-finite/≤0 height; malformed JSON→`GisError::Json`, no panic |
| T-08-04-03 | Tampering (impedance table) | mitigate | CLOSED | `crates/envi-gis/src/impedance_table.rs:40` `[_;11]`; test `:131-143` pins `len()==11`; `:146-169` resolves σ through `envi_engine::scene::impedance_class` (no σ literal) |
| T-08-04-04 | Tampering (base heights) | mitigate | CLOSED | `terrain.rs:174-211` footprint-boundary median; test `:407-426` excludes DSM spike under roof |
| T-08-04-SC / T-08-05-SC | Supply-chain (contour crate SUS) | mitigate | CLOSED | grep `contour` in `crates/envi-gis/Cargo.toml` → NONE; hand-rolled marching squares `landcover.rs:226-297` (checkpoint declined by orchestrator, fallback used) |
| T-08-05-01 | Tampering (landcover vectorization) | mitigate | CLOSED | `landcover.rs:102-217` partition→boundary loops, saddle resolution keeps loops touching not crossing (`:305-323`), min-area drop; tests `:433-503` assert `!is_overlaps()` pairwise |
| T-08-06-01 | Tampering (DTO drift) | mitigate | CLOSED | `crates/envi-gis-wasm/src/dto.rs` ts-rs DTOs; `crates/envi-service/tests/wire_no_drift.rs:64-81` regenerates + asserts no diff vs committed `web/src/generated/wire.ts` |
| T-08-06-02 | DoS (wasm-bindgen version drift) | mitigate | CLOSED | `crates/envi-gis-wasm/Cargo.toml:23` `wasm-bindgen = "=0.2.126"`; documented `cargo install wasm-bindgen-cli --locked --version 0.2.126` |
| T-08-06-03 | DoS (wasm runtime panic) | mitigate | CLOSED | `crates/envi-gis-wasm/src/lib.rs` deserialize→core→serialize; `GisError`→`JsValue` (no panic); `#![deny(unsafe_code)]`; no getrandom/uuid. Web side re-inits a trapped module (`web/src/import/wasm.ts`) |
| T-08-07-01 | DoS (OPFS quota) | mitigate | CLOSED | `web/src/import/opfs.ts:101-116` `estimateQuota`/`fitsQuota` via `navigator.storage.estimate()`; enforced before write `importJob.ts:195-197` |
| T-08-07-02 | Tampering (path traversal) | mitigate | CLOSED | `opfs.ts:36-50` `safeSeg` strips to `[A-Za-z0-9._-]` + neutralizes dot-runs; fixed `projects/<uuid>/cache/<source>/<tile>` layout, no raw user segment |
| T-08-07-03 | DoS (oversized viewport) | mitigate | CLOSED | `importJob.ts:120-145` `evaluateGuardrail` blocks > `GUARDRAIL_BLOCK_KM2=100` before any fetch (`:479-481`); per-tile pixel budget in WASM |
| T-08-07-04 | Tampering (scene pollution) | mitigate | CLOSED | imported features commit via whole-scene PUT → `envi_store::geojson::validate_feature_collection` (kind whitelist, uuid, WGS84); client-assigned ids `importJob.ts:172-179` |
| T-08-07-05 | XSS | mitigate | CLOSED | `web/src/import/attribution.ts` fixed constant strings; zero `dangerouslySetInnerHTML`/`innerHTML` in `web/src`; `ImportPanel.tsx` renders React text children |
| T-08-08-01 / T-08-08-02 | Tampering (offline false-green / DATA-04) | mitigate | CLOSED | `web/tests/e2e/_mocks.ts:55-69` catch-all abort + collector; `import.spec.ts`/`import-offline-replay.spec.ts` assert `toEqual([])`; replay evicts OPFS tile as negative network-off guard |

## Accepted Risks

None. All 25 threats are mitigated in code.

## Audit Trail

### Security Audit 2026-07-11

| Metric | Count |
|--------|-------|
| Threats found (register) | 25 |
| Closed | 25 |
| Open | 0 |

Verified by `gsd-security-auditor` (verify-mitigations mode, read-only). HIGH SSRF
(T-08-03-01) received focused adversarial tracing — no path to a non-allowlisted
host/scheme, off-allowlist redirect, or prefix/`..` bypass. The refactored generic
COG decoder was checked to confirm the load-bearing DoS-guard ordering survived.
