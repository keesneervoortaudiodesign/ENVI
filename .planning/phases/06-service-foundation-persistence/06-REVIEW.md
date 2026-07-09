---
phase: 06-service-foundation-persistence
reviewed: 2026-07-09T00:00:00Z
depth: deep
files_reviewed: 22
files_reviewed_list:
  - crates/envi-geo/src/lib.rs
  - crates/envi-geo/src/crs.rs
  - crates/envi-geo/src/transform.rs
  - crates/envi-store/src/lib.rs
  - crates/envi-store/src/dto.rs
  - crates/envi-store/src/geojson.rs
  - crates/envi-store/src/hash.rs
  - crates/envi-store/src/manifest.rs
  - crates/envi-store/src/project_dir.rs
  - crates/envi-service/src/main.rs
  - crates/envi-service/src/lib.rs
  - crates/envi-service/src/selfcheck.rs
  - crates/envi-service/src/state.rs
  - crates/envi-service/src/error.rs
  - crates/envi-service/src/jobs.rs
  - crates/envi-service/src/api/mod.rs
  - crates/envi-service/src/api/meta.rs
  - crates/envi-service/src/api/projects.rs
  - crates/envi-service/src/api/scene.rs
  - crates/envi-service/src/api/jobs.rs
  - crates/envi-service/src/api/calc.rs
  - web/dist/index.html
findings:
  critical: 0
  high: 1
  medium: 2
  low: 10
  total: 13
status: fixed
fix_disposition:
  fixed: 11
  accepted_risk: 2
  fixed_at: 2026-07-09T00:00:00Z
---

# Phase 06: Code Review Report

**Reviewed:** 2026-07-09
**Depth:** deep (cross-file: engine seam, CRS boundary, store↔service call chains)
**Files Reviewed:** 22 source files across `envi-geo`, `envi-store`, `envi-service`, plus the placeholder bundle
**Status:** issues_found

## Summary

The three new crates are well-structured and the load-bearing conventions hold: the
`proj4rs` radian quarantine is airtight (`transform.rs`), the serde/engine quarantine is
respected (no serde in `envi-engine`, DTO `TryFrom` through validating constructors),
`#![deny(unsafe_code)]` is present on all three crate roots, path ids are `Path<Uuid>`
end-to-end, atomic writes use `NamedTempFile::new_in(dir)` + `sync_all` + `persist`, the
worker is a dedicated `std::thread` (not `spawn_blocking`), and the tensor hash is a
versioned, length-prefixed, `to_bits` canonical encoding with conditioning structurally
excluded. Engine and `.github/` are untouched; no C crates; no hardcoded secrets; no debug
artifacts in non-test code.

The most important finding is a seam defect in the SC4 recondition path: the 409 gate is
enforced against a **cached** `CalcRecord`, but the matched readout is built from the
**live on-disk scene**, and a scene `PUT` never invalidates calc records. This makes the
declared domain-critical mitigation (T-06-04-01 / D-07) narrower than advertised. Phase-6
data impact is nil because stub spectra are all-zero, but the frozen contract seam will
serve identity-inconsistent readouts once real spectra land (Phase 11). Two MEDIUM
robustness/info-disclosure issues and ten LOW items follow.

Verification of declared threat mitigations: T-06-01-01/02/03/04, T-06-02-01..06,
T-06-03-01/03/05/06/07, T-06-04-05/06 are present and correct in code. T-06-04-01 is
**partially** met (see HIGH-1). Accepted risks (T-06-03-02 no-auth, T-06-03-04 body limit,
T-06-04-02/03 SSE/registry growth) are genuinely documented in-code.

## High

### HIGH-1: Recondition reads out from the live scene, not the identified tensor; scene PUT never invalidates calc records

**Disposition: FIXED** (both layers). (a) `recondition` now re-mints identity from
the current scene/met/receivers per request via `load_and_mint` and gates
`req.tensor_hash` against that freshly-derived hash (`expected` in the 409 body is
the fresh hash); the canned spectra are built from the SAME scene load, so the
readout can never disagree with the accepted identity. (b) `PUT /scene` calls
`refresh_project_calc_identity` (updates cached records to the new identity) and
`DELETE /projects/{id}` evicts the project's calc records. Conditioning stays
structurally unhashed (D-07 preserved). New contract tests:
`scene_edit_invalidates_previously_valid_recondition_hash` (scene edit → prior
hash now 409s) and `recondition_unknown_calc_is_404_not_409` (unknown calc → 404,
not 409); the existing match/mismatch/recompute tests still pass.
_Requires human verification_ of the re-mint gate semantics before Phase 11 binds real spectra.

**File:** `crates/envi-service/src/api/calc.rs:219-243` (gate + readout), `crates/envi-service/src/api/scene.rs:32-39` (PUT), `crates/envi-service/src/state.rs:34-42` (CalcRecord)

**Issue:** The 409 gate compares `req.tensor_hash` against `record.tensor_hash`, which was
minted at `submit`/`recompute` and cached in `AppState::calcs`. On a **match**, the handler
then derives the receiver set from `app.store.load_scene(record.project_id)` — the *current*
on-disk scene — rather than from the identity the matched hash names. Because `put_scene`
(`save_scene`) never touches `AppState::calcs`, the following sequence passes the gate while
serving a receiver set inconsistent with the matched `tensor_hash`:

1. `POST /projects/{id}/calculations` → mints `H1`, caches `CalcRecord{tensor_hash: H1}`, returns `H1`.
2. `PUT /projects/{id}/scene` → writes a new scene (adds/moves/removes receivers). `calcs` unchanged.
3. `POST /calculations/{cid}/recondition` with `tensor_hash = H1` → `H1 == record.tensor_hash` ⇒ **no 409** ⇒ spectra built from the *edited* scene's receiver set.

The declared mitigation T-06-04-01 states the 409 gate protects against "recondition serving
stale/mismatched tensor results," and D-07 says a recondition whose hash mismatches the tensor
is rejected. As implemented, the gate only detects *client-hash ≠ last-mint*; it does not
detect that the tensor's inputs changed on disk, and the readout is taken from the live scene
regardless. Phase-6 impact is limited to inconsistency (stub spectra are all-zero so the served
*values* are unaffected), but this is the phase's headline non-retrofittable contract seam, and
the bug will produce wrong-scene readouts once Phase 11 returns real spectra.

**Fix:** Make the readout consistent with the matched identity, and/or keep records in sync:
- In `recondition`, re-mint identity from the current scene and compare against `req.tensor_hash`
  (so a live scene edit forces a 409), OR read the receiver set from the tensor artifact the
  record names rather than from `load_scene`. For Phase 6, re-deriving via `mint_identity` and
  comparing is the smallest correct change:
  ```rust
  let identity = mint_identity(&app, record.project_id)?;
  if req.tensor_hash != identity.tensor_hash {
      return Err(ApiError::Conflict { body: json!({ /* expected: identity.tensor_hash, got: req.tensor_hash, ... */ }) });
  }
  ```
- Alternatively, invalidate/evict `CalcRecord`s for a project on `save_scene`/`save_meta` so a
  stale hash can no longer match.

## Medium

### MED-1: 500 error bodies leak absolute server filesystem paths

**Disposition: FIXED.** The `Internal` arm of `From<StoreError>` now logs the full
error via `tracing::error!(error = %e, ...)` and returns a generic client-facing
`detail: "internal error"`; no `PathBuf`/internal error text reaches the 500 body.

**File:** `crates/envi-service/src/error.rs:58-95`, sourced from `crates/envi-store/src/lib.rs:60-130`

**Issue:** `StoreError::Io`, `StoreError::Json`, and `StoreError::PathEscape` all embed a
`PathBuf` in their `Display` output (`"I/O error at {path}: {source}"`, `"JSON error in {path}: …"`,
`"path escapes the store root: {path}"`). `From<StoreError>` maps these to
`ApiError::Internal { detail: e.to_string() }`, and `IntoResponse` serializes `detail` verbatim
into the JSON 500 body. Any filesystem fault (disk error, a hand-corrupted `project.json`,
a canonicalize/persist failure) therefore returns the absolute server path — and for
`PathEscape`, the canonicalized store-root layout — to the HTTP client. This is the information
disclosure that the plan's "Security Domain V5 / structured JSON, never internal errors" posture
was meant to avoid. Impact is bounded by the localhost/no-auth posture (T-06-03-02) but it is a
real internal-detail leak.

**Fix:** For the `Internal` arm, log the full `e` server-side (`tracing::error!`) and return a
generic client-facing detail (e.g. `"internal storage error"`), not `e.to_string()`. Keep
path-bearing text out of the response body:
```rust
StoreError::Io { .. } | StoreError::Json { .. } | StoreError::PathEscape { .. } => {
    tracing::error!(error = %e, "internal storage error");
    ApiError::Internal { detail: "internal storage error".into() }
}
```

### MED-2: A single malformed project.json bricks GET /projects for the whole store

**Disposition: FIXED.** `list` now matches on `load_meta`, pushing the ones that
load and `tracing::warn!`-logging (with id + error) the ones that do not, instead
of `?`-propagating the first failure. One malformed `project.json` no longer
bricks the whole listing.

**File:** `crates/envi-service/src/api/projects.rs:64-71` (list handler), `crates/envi-store/src/project_dir.rs:147-169` (store.list) + `:185-192` (load_meta)

**Issue:** `store.list()` returns every id whose dir contains a `project.json` file, then the
handler loops `app.store.load_meta(id)?` and propagates the first error with `?`. If any one
project's `project.json` is malformed JSON (`StoreError::Json`), the *entire* `GET /projects`
request fails with 500 (and, per MED-1, leaks that file's path). D-04's core selling point is
that projects are "git-diffable, human-inspectable" flat files a user may hand-edit; a single
typo in one project then makes the user unable to list *any* project — a self-inflicted but
easily-hit availability defect.

**Fix:** Make listing resilient — skip and log unreadable/malformed projects instead of failing
the whole endpoint:
```rust
for id in ids {
    match app.store.load_meta(id) {
        Ok(meta) => metas.push(meta),
        Err(e) => tracing::warn!(%id, error = %e, "skipping unreadable project in list"),
    }
}
```

## Low

### LOW-1: UTM used beyond its valid domain (±84°N / 80°S) with no rejection

**Disposition: FIXED.** Added `GeoError::LatitudeOutsideUtm` and a `[-80, 84]°`
band check (constants `UTM_LAT_MIN`/`UTM_LAT_MAX`) in both `utm_zone_for` and
`to_utm`, after the existing range check, consistent with the SC3 loud-rejection
style. Tests: `zone_selection_rejects_latitude_outside_utm_band` +
`to_utm_rejects_out_of_range_and_nonfinite` (polar case).

**File:** `crates/envi-geo/src/crs.rs:32-53` (`utm_zone_for`), `crates/envi-geo/src/transform.rs:34-54` (`to_utm`)

**Issue:** Inputs are validated only to `lat ∈ [-90, 90]`. UTM/`etmerc` is only defined to about
±84°N/80°S; beyond that the projection produces increasingly distorted eastings/northings with no
error, and can fall outside the "plausible UTM range" the rest of the code assumes. Low
probability for a Nord2000 acoustics project, but it is a silent domain-validity gap (focus item 7).

**Fix:** Reject `|lat| > 84.0` (or the chosen UTM cutoff) with a typed `GeoError` in `utm_zone_for`
/`to_utm`, or explicitly document UPS-out-of-scope and clamp/validate the latitude band.

### LOW-2: Conditioning-shape 400 masks the tensor-hash 409 in recondition

**Disposition: FIXED.** In `recondition` the 409 hash gate is now evaluated
BEFORE the conditioning `filter_band_db` length validation, so an otherwise
well-formed request with a stale hash surfaces as 409.

**File:** `crates/envi-service/src/api/calc.rs:203-230`

**Issue:** `recondition` validates `conditioning.filter_band_db` length (returns `BadRequest`/400)
*before* the 409 hash comparison. A request carrying both a bad filter length and a stale hash gets
400, not the contract's 409. Since conditioning is explicitly a readout parameter that "influences
nothing" and must not participate in identity, the identity gate should arguably be evaluated first.

**Fix:** Evaluate the hash gate (409) before conditioning-shape validation, or document that 400
takes precedence.

### LOW-3: Blocking std::fs I/O performed directly inside async handlers

**Disposition: ACCEPTED RISK** (see `## Accepted Risks`). Bounded flat-file reads
on a single-user localhost tool are not the long-CPU work D-08/Anti-Pattern-5
targets; `spawn_blocking` is explicitly prohibited by the binding rules (grep gate
must stay 0). No change made.

**File:** `crates/envi-service/src/api/calc.rs:147-183, 258-299, 315-328`; `api/projects.rs:64-146`; `api/scene.rs:22-39`

**Issue:** `submit`, `recompute`, `recondition`, and all project/scene handlers call the
synchronous `std::fs`-backed store (load_meta/load_scene/save_*/write_manifest) directly on the
tokio runtime thread. This is bounded blocking I/O (not the CPU work Anti-Pattern 5 targets) and is
tolerable for a single-user localhost tool, but it still blocks a runtime worker and is off the
architecture's "keep blocking work off the async pool" guidance.

**Fix:** Acceptable to accept-as-risk for Phase 6; if addressed, wrap store calls in
`tokio::task::spawn_blocking` (bounded I/O is exactly what that pool is for) or make the store async.

### LOW-4: `now_unix()` duplicated across three modules

**Disposition: FIXED.** Hoisted a single `pub fn now_unix()` into `envi-store`'s
`lib.rs`; `project_dir.rs`, `api/calc.rs`, and `api/projects.rs` now all call it
(local copies removed).

**File:** `crates/envi-service/src/api/calc.rs:421-426`, `crates/envi-service/src/api/projects.rs:149-154`, `crates/envi-store/src/project_dir.rs:402-407`

**Issue:** Three byte-identical `now_unix()` helpers. Code duplication; drift risk.

**Fix:** Hoist one copy (e.g. a `pub(crate)` helper in `envi-store`) and reuse.

### LOW-5: Duplicated receiver-scanning logic in calc.rs

**Disposition: FIXED.** `scene_receiver_ids` removed. `load_and_mint` returns the
reprojected `Vec<ReceiverDto>` it hashed, and `recondition` keys its spectra from
that single receiver set — one scan, one validation path.

**File:** `crates/envi-service/src/api/calc.rs:332-381` (`scene_receivers`) and `:399-418` (`scene_receiver_ids`)

**Issue:** Two near-identical scans over `receiver`-kind features with subtly different behavior
(one reprojects and can 400 on bad geometry; the other only collects ids). Divergent validation for
the same concept invites inconsistency.

**Fix:** Factor a single receiver-extraction pass that yields id + optional reprojected position.

### LOW-6: `prop_f64_array` silently drops non-numeric spectrum entries in the hash

**Disposition: FIXED.** `prop_f64_array` replaced with `prop_array` (raw JSON
elements); `write_feature` now hashes each spectrum element TOTALLY — numeric →
`n` tag + bits, non-numeric → `x` tag + canonical JSON text — so nothing is
dropped and `[1.0,"x",2.0]` can no longer collide with `[1.0,2.0]`. Test:
`spectrum_non_numeric_entries_are_hashed_not_dropped`.

**File:** `crates/envi-store/src/hash.rs:161-168`, used at `:123-129`

**Issue:** `filter_map(JsonValue::as_f64)` silently discards non-numeric array elements, so
`[1.0,"x",2.0]` hashes identically to `[1.0,2.0]`, and integer vs float coalesce via `as_f64`.
`tensor_hash` is `pub` and `mint_identity` (calc.rs:315-328) hashes the raw scene **without**
running `scene_to_engine`'s spectrum validation, so a malformed `spectrum_band_db` reaches the
hasher unvalidated. Determinism holds, but injectivity is weakened for unvalidated inputs (two
distinct malformed spectra can collide).

**Fix:** Reject non-numeric entries (or hash a presence/NaN marker per element) instead of dropping
them; or validate spectrum arrays before hashing.

### LOW-7: Tensor hash covers a hard-coded two-field met allowlist

**Disposition: FIXED.** The met contribution now destructures `MetDto { temperature_c,
humidity_pct }` WITHOUT `..`, so adding any future `MetDto` field is a compile-time
error at the hasher — forcing an explicit hashing decision rather than a silent
escape from identity. The met field set is documented in-code; the `v1` prefix
still covers a scheme bump.

**File:** `crates/envi-store/src/hash.rs:63-67`, vs `crates/envi-store/src/dto.rs:286-311` (`MetDto`)

**Issue:** The met contribution hashes only `temperature_c` and `humidity_pct`. That is exactly
`MetDto` today, but `MetDto` is `#[serde(default)]`-extensible; a future met field will silently
escape tensor identity unless someone remembers to update `hash.rs`. The `v1` version prefix
mitigates by allowing a scheme bump, but the coupling is invisible.

**Fix:** Add a comment/compile-time reminder tying the hash's met fields to `MetDto`, or hash a
canonicalized serialization of the whole `MetDto` behind the version prefix.

### LOW-8: Calc records are never purged when their project is deleted

**Disposition: FIXED** (with HIGH-1b). `DELETE /projects/{id}` now runs
`app.calcs.write().await.retain(|_, rec| rec.project_id != id)`, evicting the
deleted project's calc records.

**File:** `crates/envi-service/src/api/projects.rs:130-136` (delete) vs `crates/envi-service/src/state.rs:61` (calcs)

**Issue:** `delete` removes the project folder but leaves the project's `CalcRecord`s in the
in-memory `calcs` map. A later `recondition` on such a calc passes the hash gate then 404s at
`load_scene` (project gone). Records leak until process exit. Part of the accepted
registry-eviction risk (T-06-04-03) but worth an explicit note.

**Fix:** On project delete, remove `calcs` entries whose `project_id` matches (and their in-memory
jobs), or fold into the Phase-10 eviction policy.

### LOW-9: `guarded_dir` conflates all canonicalize failures with NotFound

**Disposition: FIXED.** `guarded_dir` now inspects `source.kind()`:
`ErrorKind::NotFound → StoreError::NotFound`, every other IO error →
`StoreError::Io { path, source }`. The existing `traversal_and_symlink_guard`
test (non-existent dir → NotFound) still passes.

**File:** `crates/envi-store/src/project_dir.rs:88-101`

**Issue:** `dir.canonicalize().map_err(|_| StoreError::NotFound { … })` reports *any* canonicalize
failure (permission denied, path too long on Windows, I/O error) as "project not found," masking
the real cause for delete/duplicate.

**Fix:** Distinguish `ErrorKind::NotFound` (→ NotFound) from other io errors (→ `StoreError::Io`).

### LOW-10: Id-less features make the tensor hash order-dependent

**Disposition: ACCEPTED RISK** (see `## Accepted Risks`). Every scene that reaches
disk passes `validate_feature_collection`, which requires a valid uuid per
feature, so id-less features cannot occur on any path that feeds the hasher in
practice. The loud-error alternative would make the frozen `pub tensor_hash`
signature fallible — churn on a D-07-frozen contract and its tests — for an
unreachable case.

**File:** `crates/envi-store/src/hash.rs:52-61, 134-138`

**Issue:** `feature_sort_key` returns `Uuid::nil()` for features lacking a parseable `id`; multiple
such features tie and `sort_by_key` (stable) preserves file order, so identity becomes order-
dependent for id-less features. Mitigated on the persist path (`validate_feature_collection` requires
a valid uuid per feature), but `tensor_hash` is `pub` and does not itself enforce that invariant.

**Fix:** Either require ids in `tensor_hash` (error on nil) or include the pre-sort index so ties are
deterministic regardless of file order.

## Accepted-risk confirmations (no action required)

- **Unbounded job/calc registry growth** — documented accepted risk (T-06-04-03); `jobs.rs` header is explicit. LOW-8 is the one concrete leak worth noting.
- **Request body size** relies on axum's ~2 MB default — documented accepted (T-06-03-04); `deny_unknown_fields` on request DTOs bounds malformed-body DoS.
- **No-auth localhost posture** — accepted (T-06-03-02); default bind is loopback and a non-loopback `ENVI_BIND` logs a prominent warning (`main.rs:50-56`).
- **`web/dist/index.html` `innerHTML`** (`:59-68`) interpolates only numeric values from the local `/meta/freq-axis` API (`n_bands`, `toFixed`-formatted centres) — no attacker-controlled string reaches the DOM sink; no XSS in the placeholder. The error path uses `textContent`. No action.
- **SPA fallback vs /api** — the nested `/api/v1` router carries its own `api_fallback` (JSON 404), so unknown API paths never fall through to the SPA HTML (`api/mod.rs:46-85`). Correct.
- **Cancel-vs-Done race** — a cancel arriving between the final post-sleep check and the `Done` send is not observed (`jobs.rs:143-168`); inherent to the stub, benign, acceptable.

## Accepted Risks

Findings whose fix is genuinely infeasible or wrong within the Phase-6 posture and
binding rules — recorded here with rationale rather than forced.

### AR-1 (LOW-3): Blocking `std::fs` I/O inside async handlers

**Rationale.** The store's flat-file reads/writes (`load_meta`, `load_scene`,
`save_*`, `write_manifest`) are small, bounded operations on an authored Nord2000
project (hundreds of features) served to a single localhost user. This is not the
hour-scale CPU work that D-08 / architecture Anti-Pattern 5 forbids on the async
runtime — that work already lives on the dedicated `std::thread` job worker. The
binding rules for this fix task explicitly prohibit introducing `spawn_blocking`
(the grep gate in `crates/envi-service/src/` must stay 0). Making the whole store
async is a large, non-Phase-6 refactor with no user-visible benefit here.
Revisit alongside the Phase-10 job semaphore if a multi-user posture ever lands.

### AR-2 (LOW-10): Id-less features make `tensor_hash` order-dependent

**Rationale.** `tensor_hash` is `pub` and does not itself enforce per-feature
uuids, so id-less features tie at `Uuid::nil()` and file order leaks into identity.
In practice this is unreachable: every scene that reaches disk is validated by
`validate_feature_collection`, which requires a valid uuid on every feature, and
the hasher is only ever fed scenes loaded from disk (via `load_and_mint`). The
two offered fixes each have a real cost: erroring on nil requires making the
D-07-**frozen** `pub tensor_hash` signature fallible (rippling to callers and its
unit tests), and a pre-sort index does not actually restore order-independence
(it only re-encodes file order). The residual risk is a theoretical collision for
inputs that cannot occur on any persisted path. Re-evaluate if a future caller
ever hashes an unvalidated `FeatureCollection`.

---

_Reviewed: 2026-07-09_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: deep_
_Fixed: 2026-07-09 — 11 fixed, 2 accepted risk (gsd-code-fixer)_
