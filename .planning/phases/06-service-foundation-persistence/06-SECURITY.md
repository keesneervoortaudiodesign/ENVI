---
phase: 06
phase_name: service-foundation-persistence
asvs_level: 1
block_on: high
audited: 2026-07-09
threats_total: 24
threats_verified: 24
threats_open: 0
status: secured
unregistered_flags: 0
accepted_risks: 4
---

# Phase 06 — Security Verification (retroactive threat-mitigation audit)

**Scope:** the three new crates `envi-geo`, `envi-store`, `envi-service` plus `web/dist/index.html`.
`envi-engine` / `envi-harness` were out of scope (untouched this phase).
**Method:** every declared threat in the four PLAN `<threat_model>` blocks was verified by reading the
CURRENT source (the tree moved twice since the plans — a code-review fix pass and a simplify pass); the
06-REVIEW conclusions were re-checked, not trusted. Each mitigation is judged Present / Insufficient /
Missing against the file:line where it actually lives.

**Verdict: SECURED.** All 24 declared threats are CLOSED (Present). No HIGH-severity mitigation is
missing or insufficient. No new attack surface appeared beyond the declared register (all three plan
SUMMARYs explicitly record "no new security surface"). The phase does not block.

---

## Threat verification table

| Threat ID | Category | Disposition | Verdict | Evidence (file:line) |
|-----------|----------|-------------|---------|----------------------|
| T-06-01-01 | Tampering (integrity) | mitigate | **Present** | `envi-geo/src/transform.rs:73-75` — degree-magnitude guard (`\|x\|<=360 && \|y\|<=90`) returns `DegreeMagnitudeSceneCoord` BEFORE any `proj4rs::transform` call; test `to_wgs84_rejects_degree_magnitude_input` |
| T-06-01-02 | DoS (NaN/Inf poisoning) | mitigate | **Present** | `envi-geo/src/crs.rs:39-59` + `transform.rs:35-50` — non-finite and out-of-range rejected before any zone math / transform; tests `zone_selection_rejects_nonfinite_and_out_of_range`, `to_utm_rejects_out_of_range_and_nonfinite` |
| T-06-01-03 | Tampering (silent corruption) | mitigate | **Present** | `envi-geo/src/transform.rs:52,78` — `to_radians`/`to_degrees` quarantined to transform.rs only; `crs.rs` has zero conversions; meter-magnitude test `utm_output_is_meter_magnitude` catches the 6-order failure |
| T-06-01-04 | Tampering (wrong math) | mitigate | **Present** | `envi-geo/tests/oracle_utm.rs` + committed pyproj fixture `tests/fixtures/oracle/utm_landmarks.toml`; `transform.rs:99-116` landmark round-trip asserts `<=1e-3 m` |
| T-06-02-01 | Tampering / info disclosure (path joins) | mitigate | **Present** | `envi-store/src/project_dir.rs:72-83` — `dir_of(id: Uuid)` / `project_dir(id: Uuid)` take `Uuid`, never `&str`; no untrusted string is ever joined; test `traversal_and_symlink_guard` |
| T-06-02-02 | Tampering / info disclosure (symlink escape) | mitigate | **Present** | `project_dir.rs:87-110` — `guarded_dir` canonicalizes root + dir, `starts_with(canon_root)` containment, `PathEscape` on failure; used by `delete`/`duplicate` |
| T-06-02-03 | Tampering (data loss / interrupted write) | mitigate | **Present** | `project_dir.rs:388-408` — `atomic_write` = `NamedTempFile::new_in(dir)` + `write_all` + `sync_all` + `persist`; ALL writes route through it; test `atomic_write_replaces_never_truncates` (no *.tmp orphan) |
| T-06-02-04 | DoS (malformed input) | mitigate | **Present** | `dto.rs` — 10× `#[serde(deny_unknown_fields)]`; `BandSpectrumDto` length + finiteness `TryFrom`; `geojson.rs:60-76` validates kind vocab + uuid + WGS84 range; typed errors, no panics |
| T-06-02-05 | Integrity (tensor identity) | mitigate | **Present** | `hash.rs:46-89` — versioned (`envi-tensor-hash-v1`), length-prefixed, `to_bits().to_le_bytes()` canonical encoding, uuid-sorted; conditioning structurally excluded (see D-07 note); tests `tensor_hash_ignores_conditioning_covers_identity_inputs`, `order_independent_over_receiver_order` |
| T-06-02-06 | Repudiation (fake results read as real) | mitigate | **Present** | `manifest.rs:45-49` — `stub: bool`, always `true` in Phase 6; `write_manifest` stamps it; test `manifest_reserves_chunk_layout` asserts `round.stub` |
| T-06-03-01 | Spoofing / info disclosure (LAN exposure) | mitigate | **Present** | `envi-service/src/main.rs:26` default `127.0.0.1:8080`; `:50-56` non-loopback `ENVI_BIND` emits a `tracing::warn!` naming the no-auth exposure |
| T-06-03-02 | Broken access control (no authn/authz) | **accept** | **Present (documented)** | Accepted per PROJECT.md localhost single-user posture; documented in-code (`main.rs:50-56` warning) AND in the Accepted Risks log below (AR-A) |
| T-06-03-03 | Tampering / info disclosure (path traversal via ids) | mitigate | **Present** | Every id-bearing handler uses `Path<Uuid>` (`api/projects.rs`, `scene.rs`, `calc.rs`, `jobs.rs`); grep for `Path<String>` returns only doc-comment negations; store-side Uuid+containment guards as defense in depth |
| T-06-03-04 | DoS (oversized bodies) | mitigate (partial accept) | **Present** | `api/mod.rs:9-14` documents the retained axum ~2 MB default body limit; strict `deny_unknown_fields` DTOs bound malformed bodies; no streaming uploads. Accepted portion logged (AR-B) |
| T-06-03-05 | Tampering (invalid scenes corrupting projects) | mitigate | **Present** | `project_dir.rs:237-244` — `save_scene` calls `validate_feature_collection` BEFORE `write_scene`; test `save_scene_rejects_invalid_scene_before_disk` proves bad input never reaches disk |
| T-06-03-06 | Info disclosure (static-path traversal via ServeDir) | mitigate | **Present** | `api/mod.rs:74-80` — traversal-safe `tower_http::services::ServeDir` (+ `ServeFile` fallback); no hand-rolled file reads in handlers |
| T-06-03-07 | DoS (startup on broken CRS → silent garbage) | mitigate | **Present** | `main.rs:33-46` — `crs_self_check()` runs FIRST; on `Err`, logs and `return Err` (non-zero exit, refuse-to-start); `selfcheck.rs` landmark round-trip `<=1 m` |
| T-06-04-01 | Integrity (recondition serving stale/mismatched tensor) | mitigate | **Present** | `api/calc.rs:200-216` — the 409 gate re-mints identity from the CURRENT scene per request (`load_and_mint`) and compares `req.tensor_hash` against the fresh hash; spectra built from the SAME load. Prior HIGH-1 (cached-record gate) is fixed. Tests `recondition_rejects_mismatched_tensor_hash_with_409`, `scene_edit_invalidates_previously_valid_recondition_hash`, `recondition_unknown_calc_is_404_not_409` |
| T-06-04-02 | DoS (SSE connection exhaustion) | **accept** | **Present (documented)** | `jobs.rs:21-27` header documents accepted risk; `api/jobs.rs:62-66` 15 s keep-alive; watch receivers are cheap. Logged AR-C |
| T-06-04-03 | DoS (unbounded job/calc registry growth) | accept (bounded) | **Present (documented)** | `jobs.rs:21-27` + `state.rs:54-61` document Phase-10 eviction; cancel is idempotent (`api/jobs.rs:72-77`); `api/projects.rs:142-152` evicts calc records on project delete (LOW-8 fix). Logged AR-C |
| T-06-04-04 | DoS (malformed recondition bodies) | mitigate | **Present** | `calc.rs:77-85` `deny_unknown_fields` on `ReconditionRequest`; `:218-231` filter length-105 validation; hash compared as an opaque string (no hex parsing of attacker input) |
| T-06-04-05 | Repudiation / spoofing (stub mistaken for real) | mitigate | **Present** | `calc.rs:127-128,245-249` — `stub: true` in every recondition response; `persist_calc:305-312` stamps `stub: true` on the manifest; tests assert both |
| T-06-04-06 | Tampering (CPU starving the async runtime) | mitigate | **Present** | `jobs.rs:121` — worker on dedicated `std::thread::spawn`; no `spawn_blocking`/`tokio::spawn`; `send_replace` never blocks on slow consumers |
| T-06-SC | Tampering (supply chain) | mitigate | **Present** | `cargo tree \| grep -ci 'proj-sys\|gdal'` = **0** (verified); dependency sets small/legitimate: envi-geo = proj4rs + thiserror (runtime); envi-store = engine/geo/serde/serde_json/geojson/uuid/tempfile/blake3/thiserror; envi-service = axum/tokio/tower-http/tracing. Applies across all four plans |

**Distinct threats:** 24 (T-06-SC counted once; it is declared identically in all four plans).
All 24 CLOSED. **threats_open = 0.**

---

## Independent ASVS-L1 spot checks (beyond the declared register)

| Area | Finding |
|------|---------|
| `#![deny(unsafe_code)]` | Present in all three new crate roots (`envi-geo/lib.rs:23`, `envi-store/lib.rs:38`, `envi-service/lib.rs:35`) + `envi-service/main.rs:11` |
| 5xx info disclosure (MED-1 regression check post-`into_response` rewrite) | **Holds.** `error.rs:95-100` — `Io`/`Json`/`PathEscape` are logged full via `tracing::error!` and returned as generic `detail: "internal error"`; the single-match `into_response` (`:44-68`) never re-derives a body from `StoreError::Display`, so no filesystem path can reach any 5xx body |
| 409 body sensitivity | `calc.rs:206-215` — the 409 echoes only `expected`/`got` tensor hashes + a static hint; no paths or internal error text |
| Tensor-identity integrity after simplify pass | **Holds.** `state.rs:38-42` — `CalcRecord { project_id }` carries NO cached hash; `calc.rs:200-205` always derives identity on read. No code path can accept a stale `tensor_hash`. Conditioning still structurally unhashable (`hash.rs` references `ConditioningDto` only in comments) |
| Input validation | WGS84 range + non-finite in `geojson.rs:389-412` (`check_wgs84`); UTM latitude band `[-80,84]` in `crs.rs:57-59`/`transform.rs:48-50`; uuid parsing at all boundaries; `deny_unknown_fields` on request DTOs |
| Secrets | None hardcoded; nothing sensitive logged (bind address + zone + paths only, server-side) |
| `web/dist/index.html` XSS | No sink from untrusted input: the `innerHTML` write (`:59-68`) interpolates only `axis.n_bands` (a server number) and `toFixed`-formatted centres from the local engine-sourced `/meta/freq-axis`; error path uses `textContent`; zero external asset loads |

---

## Unregistered flags

**None.** All three plan SUMMARYs (`06-01`, `06-03`, `06-04`; `06-02` mirrors) explicitly record
"no new security surface beyond the plan's `<threat_model>`." No new attack surface arose during
implementation that lacks a threat mapping.

---

## Accepted risks log

These are declared-`accept` (or partial-accept) dispositions, verified as genuinely documented in-code
and consistent with the PROJECT.md posture. Recording them here satisfies the disposition contract.

### AR-A (T-06-03-02) — No authentication / authorization on the API
**Rationale.** ENVI is a self-hosted, single-user, localhost-only internal tool (PROJECT.md:
"light/no auth"). The loopback bind IS the access control; ASVS V2/V3 are out of scope this milestone.
Not silently omitted: the default bind is `127.0.0.1:8080` and any non-loopback `ENVI_BIND` fires a
prominent no-auth warning (`main.rs:50-56`). Revisit if a multi-user/networked posture is ever adopted.

### AR-B (T-06-03-04) — Request body size relies on the axum ~2 MB default
**Rationale.** Authored Nord2000 scenes are small (hundreds of features); no streaming uploads exist;
`deny_unknown_fields` bounds malformed-body parsing. Documented at `api/mod.rs:9-14`. Sufficient for
the single-user localhost posture.

### AR-C (T-06-04-02 / T-06-04-03) — SSE connections & job/calc registry growth are unbounded
**Rationale.** Single-user localhost; watch receivers are cheap with a 15 s keep-alive; cancel is
idempotent and workers exit on the token check. A submission semaphore + registry eviction policy are
Phase-10 scope, documented in `jobs.rs:21-27` and `state.rs:54-61`. Concrete mitigations already added
this phase: calc records are evicted on project delete (`api/projects.rs:142-152`).

### AR-D (code-review carry-overs — not new)
Two 06-REVIEW accepted risks remain accurate against the current tree and are informational here:
- **AR-1 (LOW-3):** blocking `std::fs` in async handlers — bounded flat-file I/O on a localhost tool;
  `spawn_blocking` is forbidden by the binding rules (grep gate must stay 0). Not a declared threat.
- **AR-2 (LOW-10):** `pub tensor_hash` is order-dependent for id-less features — unreachable on any
  persisted path (`validate_feature_collection` requires a uuid per feature; the hasher is only ever
  fed disk-loaded scenes via `load_and_mint`). Residual risk is theoretical.

---

## Note for Phase 11 (forward-looking, not a Phase-6 gap)
T-06-04-01's re-mint gate is correct for the stub (all-zero spectra). When Phase 11 returns real
spectra, the readout must continue to be built from the SAME scene load the identity is minted from
(as `load_and_mint` currently guarantees) so a served receiver set can never disagree with the accepted
`tensor_hash`. This is preserved by the current structure; flagged for human re-confirmation when real
spectra bind, per the 06-REVIEW HIGH-1 note.

---

_Audited: 2026-07-09 — gsd-secure-phase. Implementation files unmodified (verification pass only)._
