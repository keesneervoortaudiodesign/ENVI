---
phase: 9
slug: path-extraction-weather
status: verified
threats_open: 0
asvs_level: 1
created: 2026-07-11
---

# Phase 9 — Security

> Per-phase security contract: threat register, accepted risks, and audit trail.
> Verified by gsd-security-auditor (ASVS L1, block-on high) against the shipped
> implementation — every mitigation confirmed present with file:line evidence,
> including the post-execution code-review and /simplify changes (checked for
> regressions).

---

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| imported DEM/TIN → cut-profile extractor | untrusted third-party elevation | DEM raster / TIN vertices |
| drawn/imported `ground_zone` polygons → segmentation | untrusted geometry | polygon rings + class letters |
| building/wall/barrier geometry → screening | untrusted geometry | footprints/linestrings + heights |
| `calc_area` + footprints + user spacing → receiver grid | untrusted geometry + user-controlled spacing | polygons + f64 spacing |
| Open-Meteo JSON → weather derivation | untrusted third-party met JSON | multi-level winds/temps |
| ERA5 fields → Obukhov derivation | untrusted reanalysis fields | surface flux/wind fields |
| browser JS → wasm-bindgen shims | untrusted JSON/bytes across the WASM boundary | serde DTOs + tile bytes |
| envi-service → Copernicus CDS | new outbound network endpoint (flagged off by default) | CDS request + key |
| CDS key (server env) → job/logs/wire | secret must never cross outward | API key |
| fetched/derived strings → DOM | XSS injection surface | status/error/A-B-C strings |
| cache key → OPFS path | path-traversal surface | `(lat,lon,timestamp)` key |
| test runner → network | any live escape breaks the offline/zero-egress guarantee | mocked HTTP |

---

## Threat Register

| Threat ID | Category | Component | Disposition | Mitigation (verified in code) | Status |
|-----------|----------|-----------|-------------|-------------------------------|--------|
| T-09-01-01 | Denial of Service | cut_profile sampling loop | mitigate | `MAX_PROFILE_POINTS` checked (f64) before `Vec::with_capacity`; `GisError::ProfileTooLong` — `profile.rs:102-116` | closed |
| T-09-01-02 | Tampering | outside-hull / non-finite elevation | mitigate | `interpolate_z().ok_or(GisError::OutsideHull)`; never fabricated 0.0 (D-07) — `profile.rs:124` | closed |
| T-09-01-03 | Tampering | malformed ground_zone ring | mitigate | geo predicates + `impedance_class().ok_or(UnresolvableClass)`, no panic — `impedance.rs:122-155` | closed |
| T-09-01-SC | Tampering | dependency surface | mitigate | no async/network deps in envi-gis; engine unchanged (`cargo tree`) | closed |
| T-09-02-01 | Denial of Service | rstar corridor candidate set | mitigate | `screens.len() > MAX_CORRIDOR_CANDIDATES` rejected **before** alloc + `RTree::bulk_load` (WR-02) — `screening.rs:208-213` | closed |
| T-09-02-02 | Denial of Service | receiver-grid explosion | mitigate | `SpacingTooSmall` (+NaN) + lattice count vs `MAX_RECEIVERS` before alloc — `grid.rs:82,101-115` | closed |
| T-09-02-03 | Tampering | degenerate footprint rings | mitigate | `validate_region`→`build_tin().map_err(InvalidGridRegion)`, no panic — `grid.rs:183-206` | closed |
| T-09-02-04 | Tampering | receiver inside footprint / outside hull | mitigate | AABB pre-filter (E1) still gates on `f.contains(&p)`; `interpolate_z` None→`OutsideHull` — `grid.rs:143-155` | closed |
| T-09-02-SC | Tampering | dependency surface | mitigate | only `rstar 0.13` added; no async/network; engine unchanged | closed |
| T-09-03-01 | Tampering | NaN/inf weather → poisoned A/B/C | mitigate | finiteness guards; `z0.max(Z0_MIN_M)`; CR-01 elevation `Option` rejected; WR-01 negative-AGL retain — `weather.rs` | closed |
| T-09-03-02 | Tampering | NaN/inf/degenerate ERA5 fields | mitigate | finiteness + degeneracy guards + `sdfor` reliability gate → `Era5Field` — `era5.rs:113-171,296-309` | closed |
| T-09-03-03 | Integrity | false FORCE pass from [ASSUMED] constants | mitigate | `[ASSUMED]` banners preserved; only counting/sign/round-trip tests, no numeric FORCE assertion | closed |
| T-09-03-SC | Tampering | dependency surface / async leak | mitigate | no reqwest/tokio/web-sys in envi-gis (`cargo tree`) | closed |
| T-09-04-01 | Spoofing/SSRF | ERA5 retrieval upstream | mitigate | `resolve_cds_upstream` hardcoded host, reject `..`/bad-prefix before I/O; client `redirect::Policy::none()` — `api/era5.rs:144-153`, `state.rs:85` | closed |
| T-09-04-02 | Information disclosure | CDS key / host / path | mitigate | key from `std::env::var` only; generic 500; `From<reqwest::Error>`→generic — `api/era5.rs:181,197`, `error.rs:107` | closed |
| T-09-04-03 | Denial of Service | large/queued CDS job | mitigate | Phase-6 `JobStatus` state machine + default-off endpoint; shared `MAX_RELAY_BYTES` cap inherited for the future NetCDF decode | closed |
| T-09-04-04 | Tampering | malformed JSON/bytes at WASM boundary | mitigate | input DTOs `#[serde(deny_unknown_fields)]` + typed `gis_err`; shims 1:1 logic-free — `dto.rs`, `lib.rs:98` | closed |
| T-09-04-05 | Integrity | hand-authored TS mirror drift | mitigate | all DTOs `ts_rs::TS`; `wire_no_drift` byte-equality vs committed `wire.ts` — `tests/wire_no_drift.rs` | closed |
| T-09-04-SC | Access control | endpoint enabled unintentionally | mitigate | route only under `#[cfg(feature="era5")]` (default-off); `era5_route_absent_by_default` asserts 404 — `api/mod.rs:103`, `Cargo.toml:79` | closed |
| T-09-05-01 | Tampering (XSS) | WeatherPanel string rendering | mitigate | all strings React text children; no `dangerouslySetInnerHTML`/`innerHTML` in web/src | closed |
| T-09-05-02 | Tampering | OPFS cache path traversal | mitigate | `safeSeg` whitelist + dot-run guard; fixed `projects/<uuid>/cache/weather/<key>` — `opfs.ts:36-40,102` | closed |
| T-09-05-03 | Spoofing/SSRF | Open-Meteo fetch host | mitigate | hardcoded `FORECAST_URL`/`ARCHIVE_URL` + fixed proxy source ids; WR-03 abort re-throw (no proxy bypass) — `weather.ts:28-29,148-150` | closed |
| T-09-05-04 | Integrity | client-side acoustic math drift | mitigate | A/B/C fit only in the WASM shim; TS does coordinate marshalling + `+180°` only, no acoustic arithmetic | closed |
| T-09-05-05 | Denial of Service | redundant weather fetches | mitigate | OPFS cache-then-read (zero network on hit); call-cost logged — `weather.ts:175-187` | closed |
| T-09-06-01 | Integrity | live-network escape masking a real fetch | mitigate | `bootOffline` unmocked collector `toEqual([])`; both Open-Meteo hosts mocked — `weather-import.spec.ts:135,158` | closed |
| T-09-06-02 | Integrity | SC4 regression (what-if re-fetch) | mitigate | record + `route.abort()` egress collector `[]` after cached import — `weather-import.spec.ts:101-120` | closed |
| T-09-06-03 | Access control | test needs credentials / live API | accept | mocked hosts + committed synthetic fixtures; no credentials, no live CDS/Open-Meteo (by design) | closed |

*Status: open · closed*
*Disposition: mitigate (implementation required) · accept (documented risk) · transfer (third-party)*

---

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-09-01 | T-09-06-03 | The offline Playwright suite deliberately uses `page.route` mocks of both Open-Meteo hosts + committed synthetic fixtures, so it needs no credentials and makes no live API call by design (CLAUDE.md offline-UAT rule). "Requiring credentials" is intentionally out of scope; the zero-egress collector assertions (T-09-06-01/02) are the compensating control. | Phase 9 owner | 2026-07-11 |

*Accepted risks do not resurface in future audit runs.*

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-07-11 | 27 | 27 | 0 | gsd-security-auditor (Opus, ASVS L1) |

**Notes:**
- T-09-04-03: the live CDS retrieval path (`retrieve_era5_hours`) currently issues no outbound request (IN-03b precondition-before-send), so the reachable DoS guards are the Phase-6 job state machine + the default-off endpoint; the shared `MAX_RELAY_BYTES` cap is inherited infrastructure that applies when the NetCDF decode lands. Defensible, not a gap.
- No unregistered threat flags surfaced in the SUMMARY files; the WASM boundary, flagged ERA5 endpoint, and path-cache all map to existing `T-09-*` IDs.

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-07-11
