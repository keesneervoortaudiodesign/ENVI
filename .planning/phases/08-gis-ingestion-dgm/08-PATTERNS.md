# Phase 8: GIS Ingestion & DGM - Pattern Map

**Mapped:** 2026-07-10
**Files analyzed:** 21 new/modified files (from 08-CONTEXT + 08-RESEARCH "Recommended Project Structure")
**Analogs found:** 18 / 21 (3 files have no codebase analog — see "No Analog Found")

The codebase has unusually strong, deliberately-repeated conventions (boundary crates, typed no-panic errors, committed-oracle fixtures, generated wire types, offline Playwright). Nearly every new Phase-8 file has a direct in-repo template. **Planner: cite the analog file + line range in each plan action.**

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/envi-gis/Cargo.toml` | config | — | `crates/envi-dgm/Cargo.toml` + `crates/envi-geo/Cargo.toml` | exact |
| `crates/envi-gis/src/lib.rs` (crate doc + `GisError`) | model (error enum + boundary doc) | — | `crates/envi-dgm/src/lib.rs` | exact |
| `crates/envi-gis/src/cog/{header,window,geo_tags}.rs` | service (parser/decoder) | transform (bytes → raster) | `crates/envi-dgm/src/tin.rs` (validate-first, DoS caps, typed errors) | role-match |
| `crates/envi-gis/src/terrain.rs` | service | transform | `crates/envi-dgm/src/tin.rs` | role-match |
| `crates/envi-gis/src/landcover.rs` | service | transform | `crates/envi-dgm/src/tin.rs` + `envi-store/src/geojson.rs` (feature construction) | role-match |
| `crates/envi-gis/src/impedance_table.rs` | model (reviewed data table + per-row test) | — | `crates/envi-engine/src/scene.rs:317-341` (`impedance_class`) + test at `:410-422` | exact |
| `crates/envi-gis/src/registry.rs` | model (source registry as data) | — | `crates/envi-engine/src/scene.rs` (`impedance_class` table-as-code) | role-match |
| `crates/envi-gis/src/buildings.rs` | service (untrusted-JSON parse + height chain) | transform | `crates/envi-store/src/geojson.rs` (`opt_f64`/`req_f64`/`building_from_feature`) | exact |
| `crates/envi-gis/src/merge.rs` | service (D-09 merge) | transform | `crates/envi-store/src/geojson.rs` (property carriers) | partial |
| `crates/envi-gis/src/provenance.rs` | model (property stamping) | — | `crates/envi-store/src/geojson.rs` (unknown-props-preserved contract) | role-match |
| `crates/envi-gis-wasm/src/lib.rs` | boundary (cdylib bindings) | request-response (JsValue ⇄ DTO) | — none (first WASM crate) | no analog |
| `crates/envi-service/src/api/proxy.rs` | controller (byte relay) | streaming request-response | `crates/envi-service/src/api/dgm.rs` + `api/mod.rs` + `error.rs` | role-match |
| `crates/envi-geo/src/crs.rs` (MOD: + RD New) | service (CRS) | transform | itself (`WGS84_PROJ` const + `from_zone`) | exact |
| `crates/envi-geo/tests/oracle_rd.rs` (or extend `oracle_utm.rs`) | test (oracle) | file-I/O (committed fixture) | `crates/envi-geo/tests/oracle_utm.rs` | exact |
| `tools/gis_oracle/gen_cog_fixtures.py` + committed fixtures | config/test-data generator | batch | `tools/crs_oracle/gen_utm.py` + fixture header | exact |
| `web/src/import/fetchers.ts` | service (typed fetch, direct-vs-proxy) | request-response | `web/src/api/client.ts` | exact |
| `web/src/import/opfs.ts` | service (browser cache) | file-I/O | — none (first OPFS use); style from `client.ts` headers | no analog |
| `web/src/import/importJob.ts` (+ store slice) | hook/orchestrator (per-layer state machine) | event-driven | `web/src/dgm/dgmTrigger.ts` + `web/src/store/dgm.ts` | exact |
| `web/src/import/attribution.ts` | utility | — | trivial; header style from `client.ts` | partial |
| `web/src/panels/ImportPanel.tsx` | component | request-response (store-derived UI) | `web/src/panels/ValidationPanel.tsx` | exact |
| `web/tests/e2e/` GIS mocks + import specs | test | event-driven | `web/tests/e2e/_mocks.ts` + `dgm-trigger.spec.ts` | exact |
| New wire DTOs (proxy/import/WASM boundary types) | model (serde+ts-rs) | request-response | `api/dgm.rs` `DgmReq`/`DgmResp` + `tests/wire_no_drift.rs` | exact |

## Pattern Assignments

### `crates/envi-gis/Cargo.toml` (config)

**Analog:** `crates/envi-dgm/Cargo.toml` (whole file, 15 lines) — copy the boundary-rule comment convention verbatim in spirit:

```toml
[package]
name = "envi-dgm"
description = "ENVI DGM boundary — constrained-Delaunay TIN from user-drawn elevation (D-08); spade lives HERE, never in envi-engine"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

# Boundary rule (D-08, 07-RESEARCH Gate 3): pure-Rust constrained-Delaunay TIN.
# spade + thiserror ONLY at runtime — zero C toolchain, zero I/O, NO serde in the
# runtime graph, and NO dependency on envi-engine ...
[dependencies]
spade = "2.15"
thiserror = "2"
```

For `envi-gis`: same shape, deps `tiff`, `geo`, `geojson`, `serde`/`serde_json`, `thiserror` (research §Standard Stack), `envi-geo` + `envi-engine` (for `impedance_class` letters only — verify the engine 3-dep quarantine is one-directional: engine must not gain deps; envi-gis *depending on* envi-engine is legal, mirroring `envi-store`'s existing `envi_engine::scene` import at `geojson.rs:27`). Workspace membership is automatic via the `crates/*` glob (`Cargo.toml:3`). Dev-only deps (`approx`, `toml`) follow `envi-geo/Cargo.toml:15-18` (`[dev-dependencies]` with an explanatory comment).

---

### `crates/envi-gis/src/lib.rs` (error enum + boundary doc)

**Analog:** `crates/envi-dgm/src/lib.rs` (whole file, 92 lines)

**Crate-doc pattern** (lines 1-31): `//! # envi-gis`, a "# Boundary statement" section naming what lives HERE and nowhere else, a load-bearing safety section, "# House rules", then `#![deny(unsafe_code)]` and `pub mod` list. For envi-gis the boundary statement is the sans-I/O contract: *no network, no OPFS, no `web-sys`; synchronous API over byte slices; TS owns all I/O* (research Pattern 1).

**Error-enum pattern** (`envi-dgm/src/lib.rs:47-92`) — copy structurally:

```rust
/// `PartialEq` is derived so tests can `assert_eq!` / `matches!` on variants;
/// `spade` insertion errors are wrapped as message strings so the equality
/// holds (mirroring `envi_geo::GeoError::Proj`).
#[derive(Debug, Error, PartialEq)]
pub enum DgmError {
    #[error("degenerate TIN: {got} distinct point(s) yield no triangle (need >= 3 non-collinear)")]
    TooFewPoints { got: usize },
    ...
    /// The point or breakline-vertex count exceeded the documented DoS bound
    #[error("input too large: {got} {kind}, limit is {limit}")]
    TooLarge { kind: &'static str, got: usize, limit: usize },
    /// `spade` failed ... captured as a message so `DgmError` stays `PartialEq`.
    #[error("triangulation error: {message}")]
    Triangulation { message: String },
}
```

`GisError` variants follow the same rules: struct variants carrying offending values (got/expected style), `PartialEq` derived, foreign errors (`tiff::TiffError`) wrapped as `{ message: String }` exactly like `DgmError::Triangulation` / `GeoError::Proj { message }` (`envi-geo/src/lib.rs:112-118`). Every doc comment names the threat/requirement it discharges.

---

### `crates/envi-gis/src/cog/window.rs`, `terrain.rs`, `landcover.rs` (parsers/transforms over untrusted bytes)

**Analog:** `crates/envi-dgm/src/tin.rs`

**Module I/O header pattern** (`tin.rs:1-21`) — mandatory on every new module (doc-consistency gate 5 enforces it):

```rust
//! Constrained-Delaunay TIN builder + barycentric Z interpolation.
//!
//! # Module I/O
//! - **Inputs:** scattered `elevation_point` vertices `[x, y, z]` (meters) ...
//! - **Output:** a queryable [`Tin`] ...
//! - **Invariant (load-bearing):** this module NEVER panics on data. ...
```

**Guard-ordering pattern — the core pattern to replicate** (`tin.rs:158-211`): DoS caps FIRST (before any O(n) work), finiteness SECOND, then the real work with every foreign error mapped to a typed variant:

```rust
pub const MAX_POINTS: usize = 500_000;   // documented DoS bound as a pub const (tin.rs:31)

pub fn build_tin(points: &[[f64; 3]], breaklines: &[Vec<[f64; 2]>]) -> Result<Tin, DgmError> {
    // --- DoS caps (threat T-07-02-02): reject before any O(n log n) work. ---
    if points.len() > MAX_POINTS {
        return Err(DgmError::TooLarge { kind: "points", got: points.len(), limit: MAX_POINTS });
    }
    ...
    // --- Finiteness (threat T-07-02-03): reject NaN/inf before insert. ---
    for p in points {
        if !p.iter().all(|c| c.is_finite()) {
            return Err(DgmError::NonFinite { what: format!("point {p:?}") });
        }
    }
    ...
    cdt.insert(...).map_err(|e| DgmError::Triangulation { message: e.to_string() })?;
```

For `decode_window` this maps directly onto the research security controls: `max_decoded_px` enforced from IFD dims BEFORE decode (= the `MAX_POINTS` pre-work guard), nodata/edge clamping, `tiff` errors wrapped. Export the budget as a `pub const` with a threat-ID doc comment.

**"Never a silent 0.0 / never a false value" pattern** (`tin.rs:70-83`): fallible lookups return `Option`, with a test asserting the None case (`tin.rs:281-286` `outside_hull_returns_none_not_zero`). Apply to nodata samples and out-of-coverage window queries.

**Inline `#[cfg(test)]` unit-test style** (`tin.rs:245-392`): one small named fixture helper (`unit_square_z_eq_y`), one test per error variant using `assert!(matches!(...), "got {result:?}")`, and a dedicated test proving the no-panic property (`interior_crossing_breaklines_are_rejected_without_panic`, `tin.rs:344-357`). Every `GisError` variant gets the same treatment (BigTIFF magic, oversized IFD, nodata, edge tile).

---

### `crates/envi-gis/src/impedance_table.rs` (the SC3 reviewed table + per-row test)

**Analog:** `crates/envi-engine/src/scene.rs:317-341` (the pinned σ table) + its test at `scene.rs:410-422`

```rust
/// Nordtest ground-impedance class → flow resistivity σ (kNs·m⁻⁴).
/// # Provenance
/// **All eight classes VERIFIED** against AV 1106/07 Table 2 ... Class **B is 31.5** ...
#[must_use]
pub fn impedance_class(class: char) -> Option<f64> {
    match class {
        'A' => Some(12.5),
        'B' => Some(31.5),
        ...
        _ => None,
    }
}
```

The Phase-8 table is a `const` array of `(u8 /* WC code */, char /* Nord2000 class */, &'static str /* rationale */)` rows keyed to the research §WorldCover table. **Load-bearing rule from CONTEXT/RESEARCH:** never restate σ — the per-row test resolves σ through `envi_engine::scene::impedance_class(letter)` and asserts `is_some()` + the exact engine value, plus `assert_eq!(TABLE.len(), 11)`. Provenance doc-comment style (report + verification note per row confidence) copies `scene.rs:321-325`.

---

### `crates/envi-gis/src/registry.rs` (source registry as data, D-04)

**Analog:** same table-as-code pattern as `impedance_table.rs` above (`scene.rs:329` style), plus the **committed-artifact rule**: the AHN kaartblad index (tile name ↔ RD bbox, generated at dev time from the PDOK ATOM feed) is committed with a provenance header exactly like `crates/envi-geo/tests/fixtures/oracle/utm_landmarks.toml:1-8`:

```
# generated by tools/crs_oracle/gen_utm.py — DO NOT EDIT
# Cross-implementation CRS oracle: pyproj (C PROJ) vs proj4rs (pure Rust).
[meta]
provenance = "gen_utm.py sha256:a3c34d0b3b293b46"
```

`SourceDescriptor` fields per research Pattern 3 (`id, kind, coverage, crs, tile_scheme, endpoint_template, cors: Direct|Proxy, license, attribution`).

---

### `crates/envi-gis/src/buildings.rs` (Overpass parse + height chain + provenance)

**Analog:** `crates/envi-store/src/geojson.rs`

**Property-reading helpers** (`geojson.rs:229-244`) — copy these two helpers verbatim as the house pattern for optional/required numeric props:

```rust
/// Read an optional finite f64 property.
fn opt_f64(feature: &geojson::Feature, key: &str) -> Option<f64> {
    feature.properties.as_ref().and_then(|p| p.get(key)).and_then(JsonValue::as_f64)
}
/// Read a required finite f64 property, else [`StoreError::MissingProperty`].
fn req_f64(feature: &geojson::Feature, kind: &str, key: &str) -> Result<f64, StoreError> { ... }
```

**Ring handling** (`geojson.rs:302-331` `building_from_feature`): exterior-ring extraction with typed error on missing ring, and the RFC-7946 closing-vertex drop:

```rust
let exterior = rings.first().ok_or_else(|| StoreError::GeoJson {
    message: "building polygon has no exterior ring".to_string(),
})?;
...
// RFC 7946 rings repeat the first vertex as the last; drop the closing duplicate
if footprint.len() > 1 && footprint.first() == footprint.last() {
    footprint.pop();
}
```

**CRITICAL property-name contract** (research flag): imported buildings must emit **`eaves_height_m`** — the exact key `building_from_feature` reads at `geojson.rs:316` (`req_f64(feature, "building", "eaves_height_m")`). Do not invent a parallel `height_m` property for buildings; `height_m` is the source/receiver/wall key (`geojson.rs:254,272,289`).

**Skip-and-report (D-07/Pitfall 12):** invalid Overpass features are skipped with a per-feature report, mirroring how `scene_to_engine` skips non-mappable kinds without mutating input (`geojson.rs:111` `_ => {}` + doc "skip, never mutate").

---

### `crates/envi-gis/src/merge.rs` + `provenance.rs` (D-09/D-11)

**Analog (partial):** `crates/envi-store/src/geojson.rs`

The load-bearing enabler is already verified in the analog: **unknown additional properties are preserved** by `validate_feature_collection` (`geojson.rs:50-57` doc: "Unknown additional properties are permitted; unknown kind strings are not") — so `source`, `source_ref`, `license`, `retrieved_at`, `imported`, `user_modified`, `height_provenance`, `vertical_datum` ride through the existing store with **zero schema change**. Provenance stamping = building a `serde_json::Map` of these keys (research Pattern 4 lists the exact key set); merge rule matches on `(source, source_ref)` with the `user_modified` guard. Feature `id` must be a UUID string — the store gate is `feature_uuid` (`geojson.rs:207-227`); UUIDs are generated in **TS** via `crypto.randomUUID()` (research Don't-Hand-Roll: avoids the wasm getrandom dance). The `user_modified: true` flag is set at the Phase-7 commit path — `commitFeature` / `updateProperties` in `web/src/store/sceneStore.ts` (`sceneStore.ts:95,323`): one flag, one place.

---

### `crates/envi-service/src/api/proxy.rs` (controller, streaming byte relay)

**Analog:** `crates/envi-service/src/api/dgm.rs` (handler shape) + `api/mod.rs` (registration) + `error.rs` (status mapping)

**Handler module pattern** (`dgm.rs:1-27`): module doc names the endpoint, the "thin wrapper / NO logic here (SVC-07)" statement, and the panic-safety note. The handler itself (`dgm.rs:73-79`):

```rust
pub async fn triangulate(Json(req): Json<DgmReq>) -> Result<Json<DgmResp>, ApiError> {
    let tin = build_tin(&req.points, &req.breaklines)?;
    Ok(Json(DgmResp { vertices: tin.vertices(), triangles: tin.triangles() }))
}
```

For the proxy, the extractor is `Path((source, path)): Path<(String, String)>` with the wildcard route; the allowlist is a hardcoded `const SOURCES: &[(&str, &str, &str)]` (research Code Example, `proxy.rs` sketch) — reject unknown `source` and non-prefix paths with `ApiError::BadRequest`/`NotFound` BEFORE any network I/O (same guard-first ordering as `build_tin`).

**Route registration** (`api/mod.rs:47-75`): add to `api_router()` using **axum 0.8 brace syntax** — the module doc at `mod.rs:3-8` is load-bearing: *"the 0.7 colon syntax `/:id` panics at router construction"*. Wildcard: `.route("/proxy/{source}/{*path}", get(proxy::relay))`. Add `pub mod proxy;` at `mod.rs:23-28`.

**Error mapping** (`error.rs:106-127`): a `From<ProxyError> for ApiError` (or direct `ApiError` construction) following the `From<DgmError>` impl — every request-fault variant maps to `BadRequest { detail: e.to_string() }`, with the MED-1 rule from `error.rs:60-66, 93-101`: upstream/internal failures log the full error via `tracing::error!` and return a **generic** detail (never leak upstream URLs/hosts beyond the allowlisted names).

**New dep note:** envi-service needs an HTTP client (research Open Question 7 — reqwest/rustls or hyper; TLS pure-Rust). Body-size cap + timeout are the proxy's own guards (axum's ~2 MB default body limit noted at `mod.rs:9-14` applies to *requests*, not the relayed response).

**Contract test analog:** `crates/envi-service/tests/contract_dgm.rs` (sibling files `contract_*.rs` all build the FULL `app()` router per `mod.rs:5-8` so route-syntax stragglers panic the suite). A new `contract_proxy.rs` copies that harness; SSRF cases (unknown source, path escape, non-GET) are its per-variant tests.

---

### `crates/envi-geo/src/crs.rs` MODIFIED (+ RD New EPSG:28992)

**Analog:** the file itself.

**Named-CRS const pattern** (`crs.rs:21-23`):

```rust
/// The WGS84 geographic projection string (proj4rs longlat, radians on the wire
/// — converted in [`transform`](crate::transform) and ONLY there).
const WGS84_PROJ: &str = "+proj=longlat +ellps=WGS84 +datum=WGS84 +no_defs";
```

Add `RD_NEW` the same way (exact string in research §Code Examples — sterea + bessel + 7-param towgs84). **Projection construction + error wrap** copies `from_zone` (`crs.rs:89-103`): `Proj::from_proj_string(...).map_err(proj_err)?` with the shared `proj_err` helper (`crs.rs:143-147`). The radians quarantine stays in `transform.rs` — the crate doc (`lib.rs:15-17`) makes this a hard rule: public API speaks degrees/meters only; no new module may touch radians.

**Oracle extension:** `tests/oracle_utm.rs:39-49` fixture-loading shape (committed TOML via `concat!(env!("CARGO_MANIFEST_DIR"), ...)`, `tol_m` read from `[meta]`, never hardcoded) + `tools/crs_oracle/gen_utm.py` (pyproj Transformer, LANDMARKS list, sha256 provenance header). For RD New: a `gen_rd.py` sibling (or extend `gen_utm.py`) with the Amersfoort landmark (RD origin ≈ (155000, 463000)), tolerance ≤ 1.0 m per research (towgs84 7-param ≈ 0.5 m vs RDNAPTRANS).

---

### `web/src/import/fetchers.ts` (typed fetch, direct-vs-proxy routing)

**Analog:** `web/src/api/client.ts`

**Module header discipline** (`client.ts:1-13`): `// # Module I/O` comment block with Input/Output/Valid-input-range — required on every new TS module.

**Error class + safe-detail extraction** (`client.ts:39-74`) — reuse `ApiError` and `errorText` by import (do NOT redeclare; `client.ts` owns them per its own comment at `:52`):

```ts
export class ApiError extends Error {
  readonly status: number;
  readonly detail: string;
  ...
}
export function errorText(err: unknown, fallback = "Request failed."): string { ... }
```

**Fetch-wrapper shape** (`client.ts:76-90` `getJson`/`deleteResource`): every wrapper takes an optional `AbortSignal`, throws `ApiError(status, detail)` on non-2xx. The proxy path is same-origin (`/api/v1/proxy/{source}/{path}`), so it composes on `BASE = "/api/v1"` (`client.ts:28`); direct PDOK/Overpass fetches are new absolute-URL cases — binary responses use `res.arrayBuffer()` instead of the JSON path, but keep the same non-2xx → `ApiError` contract. The per-source Direct|Proxy decision reads the registry capability map (wire-typed, see Shared Patterns).

**No hand-written wire types** (`client.ts:4-7`, load-bearing): request/response DTO types are imported from `../generated/wire` only.

---

### `web/src/import/importJob.ts` + import store slice (per-layer state machine, D-07/D-08)

**Analog:** `web/src/dgm/dgmTrigger.ts` (whole file, 162 lines) + `web/src/store/dgm.ts`

**Orchestrator conventions to copy** (`dgmTrigger.ts:98-162`):
- Outcome written to a dedicated zustand slice, success and failure both (`dgm.setTriangulation` / `dgm.setReject`) — "never a silent swallow" (`dgmTrigger.ts:112-126`).
- `AbortController` supersession: `controller?.abort(); controller = new AbortController()` and every `.then` guarded by `if (!signal.aborted)` (`dgmTrigger.ts:109-117`).
- Aborted ≠ error: `if (signal.aborted) return; // superseded — not an error` (`dgmTrigger.ts:119-121`).
- `ApiError` instances land in the store as `{ status, detail }` (`dgmTrigger.ts:122-124`) — the ImportPanel per-layer status rows read exactly this, the way `ValidationPanel` reads `rejectReason`.
- Doomed-request skip: check preconditions (viewport guardrail!) before fetching, and clear state instead of firing (`dgmTrigger.ts:104-108`).
- Effect cleanup tears down subscription + timer + controller (`dgmTrigger.ts:154-161`).

Per-layer independence (D-07) = one such state machine instance per layer (terrain/landcover/buildings), each with its own status field in the import slice; a failed layer stores its error and exposes a retry action, never blocking siblings.

---

### `web/src/panels/ImportPanel.tsx` (component)

**Analog:** `web/src/panels/ValidationPanel.tsx` (whole file, 123 lines)

Copy structurally: header comment with Module I/O; store-selector reads (`ValidationPanel.tsx:38-41`); `useMemo` derivation of rows from store slices (`:46-88`); the render skeleton:

```tsx
<section className="panel" data-testid="validation">
  <div className="panel-header">Validation</div>
  {rows.length === 0 ? (
    <div className="empty-state">No issues — the scene is valid.</div>
  ) : (
    <ul className="issue-list" data-testid="issue-list"> ... </ul>
  )}
</section>
```

Conventions that E2E depends on: `data-testid` on the section and on each actionable row (`data-testid={`issue-${row.severity}`}`, `:102`); all text reaches the DOM as React text children — **never innerHTML** (`:14`, security posture); status chips via `className={`chip ${severity}`}` (`:110`). ImportPanel adds: per-layer toggle + status row + retry button, the guardrail warning, the GLO-30 "surface model" badge (D-05), and attribution strings (SC5 — MapLibre `AttributionControl` wiring lives in `attribution.ts`).

---

### `web/tests/e2e/` — `mockGis` helper + import specs

**Analog:** `web/tests/e2e/_mocks.ts` + `web/tests/e2e/dgm-trigger.spec.ts`

**Ordering rule — load-bearing** (`_mocks.ts:7-9, 50-52`): register the offline guard FIRST, specific mocks AFTER ("Playwright matches the most-recently-registered route first, so the specific mocks win"). The guard (`_mocks.ts:52-66`) allows only localhost/data/blob, aborts + records everything else:

```ts
export function installOfflineGuard(page: Page, onUnmocked: (url: string) => void): Promise<void> {
  return page.route("**/*", (route: Route) => {
    const url = route.request().url();
    if (url.startsWith("http://localhost") || ... ) return route.continue();
    onUnmocked(url);
    return route.abort();
  });
}
```

**⚠ Direction from the guard's design:** PDOK and Overpass are cross-origin and thus already aborted by the existing guard — a new `installGisMocks(page)` (fixture-serving routes for `**service.pdok.nl/**`, `**overpass-api.de/**`, `**/api/v1/proxy/**`) must be registered AFTER `installOffline`, exactly like `installMetaMocks` / `installTriangulateMock` (`_mocks.ts:105-145`). The 206-Range-slice mock shape is in research §Code Examples (`mockGis.ts` sketch); binary fixture via `route.fulfill({ contentType: "image/tiff", body: buffer })`.

**Boot + spec pattern** (`_mocks.ts:83-88` `bootOffline`; `dgm-trigger.spec.ts:24-46`): `const unmocked = await bootOffline(page)`; post-boot route registrations with call counters; drive via `window.__enviTest` bridge; end every test with the offline-clean assertion:

```ts
expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
```

The DATA-04 **network-off replay** test is the same machinery inverted: import from fixtures, then re-register an abort-everything route (or rely on the guard with the GIS mocks removed), reload, and assert terrain/landcover/buildings render from OPFS only.

**Deterministic-oracle mock style** (`_mocks.ts:128-145` `installTriangulateMock`): mocks model the real contract's failure branch too (breaklines ≥ 2 → 400 with `{ detail }`), so both spec branches exercise the wire. GIS mocks should do the same (e.g. Overpass 429 branch for the retry test).

---

### New wire DTOs (proxy/import boundary types)

**Analog:** `crates/envi-service/src/api/dgm.rs:37-61` + `crates/envi-service/tests/wire_no_drift.rs`

DTO derive pattern (`dgm.rs:37-47`):

```rust
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]        // request-facing: typo'd key = loud 4xx
#[ts(export_to = "wire.ts")]
pub struct DgmReq { ... }
```

**Registration is mandatory:** every new TS-deriving type must be added to `export_all_wire_types` in `wire_no_drift.rs:74-113` ("a new wire DTO must be added here"), then regenerate the committed artifact:

```
cargo test -p envi-service --test wire_no_drift -- --ignored regenerate_committed_wire_ts
```

(`wire_no_drift.rs:193-206`). The Phase-7 D-10 rule extends to the **WASM boundary**: `envi-gis-wasm`'s JsValue-crossing DTOs are generated from the same Rust source (research anti-pattern list) — the planner must route their TS types through this same generated-and-committed mechanism (either the existing `wire.ts` pipeline if the DTOs live in a ts-rs-visible crate, or a parallel committed artifact with its own no-drift test copying `wire_no_drift.rs:118-157`).

---

### `tools/gis_oracle/gen_cog_fixtures.py` + committed COG fixtures + `envi-gis` fixture tests

**Analog:** `tools/crs_oracle/gen_utm.py` (docstring + provenance pattern, lines 1-16) + `crates/envi-geo/tests/oracle_utm.rs` (consumer) + `crates/envi-geo/tests/fixtures/oracle/utm_landmarks.toml` (header)

Generator docstring contract (`gen_utm.py:1-16`): states what it writes, that regeneration is operator-driven, that **Python is NOT a build/test dependency**, and that the fixture header records a generator sha256. Consumer contract (`oracle_utm.rs:39-56`): fixture loaded from `CARGO_MANIFEST_DIR`-relative path with a loud `expect` if missing; a truncation guard (`fx.case.len() >= 10`); tolerance read from `[meta]`; coverage assertions at the end (`:102-105` both-hemispheres check → for COGs: "fixture set must include one BigTIFF, one predictor-3, one nodata-edge case" — Pitfall 1 requires a BigTIFF fixture so the suite *cannot* pass without BigTIFF support). Expected-window values live in per-fixture TOMLs, same `[meta] tol` + `[[case]]` layout as `utm_landmarks.toml`.

---

## Shared Patterns

### Typed no-panic error boundary (all new Rust code)
**Source:** `crates/envi-dgm/src/lib.rs:47-92` + `crates/envi-geo/src/lib.rs:64-119`
**Apply to:** `envi-gis` (all modules), `proxy.rs`
`thiserror` struct-variant enums, `PartialEq` derived, foreign errors wrapped as `{ message: String }`, offending values carried in fields, NEVER `unwrap()`/`panic!` on a data path. Guard ordering: caps → finiteness → work.

### HTTP status mapping
**Source:** `crates/envi-service/src/error.rs:72-127`
**Apply to:** `proxy.rs`, any new endpoint
Client-fault variants → `ApiError::BadRequest { detail: e.to_string() }` (400); missing → `NotFound`; internal/upstream → `tracing::error!(error = %e, ...)` + generic `Internal { detail: "internal error" }` (MED-1: never leak paths/URLs in 500 bodies). Response envelope is always `{ "error": code, "detail": ... }` JSON (`error.rs:45-70`).

### Module I/O documentation contract
**Source:** `crates/envi-dgm/src/tin.rs:1-21` (Rust), `web/src/api/client.ts:1-13` (TS)
**Apply to:** every new file. Inputs / Outputs / Valid-input-range / load-bearing invariants, naming the decision or threat IDs (D-xx, T-xx). CLAUDE.md gate 5 checks this plus `crates/README.md` + root `README.md` updates for the new crates/endpoints.

### Committed-artifact + no-drift (the house one-source-of-truth mechanism)
**Source:** `tools/crs_oracle/gen_utm.py` + `crates/envi-geo/tests/oracle_utm.rs` (fixture direction); `crates/envi-service/tests/wire_no_drift.rs` (regenerate-and-compare direction)
**Apply to:** COG fixtures, AHN kaartblad tile index, RD-New pyproj fixtures, all new wire/WASM-boundary TS types. Generator never needed at test time; sha256/banner provenance header; drift fails `cargo test`.

### Offline Playwright stack
**Source:** `web/tests/e2e/_mocks.ts:52-88` (guard-first ordering, `bootOffline`, unmocked-collector) + `dgm-trigger.spec.ts:44-45` (the `toEqual([])` closing assertion)
**Apply to:** every new import spec. New GIS origins (PDOK, Overpass, `/api/v1/proxy/**`) get fixture routes registered after boot; each spec ends by asserting the collector is empty.

### Wire types generated, never hand-authored
**Source:** `crates/envi-service/src/api/dgm.rs:37-47` + `tests/wire_no_drift.rs:74-113` + `web/src/api/client.ts:15-26`
**Apply to:** every type crossing HTTP or the WASM boundary. `#[derive(TS)] #[ts(export_to = "wire.ts")]`, `deny_unknown_fields` on request DTOs, register in `export_all_wire_types`, import from `../generated/wire` in TS.

### The one-reprojection-boundary rule (GEOX-04)
**Source:** `crates/envi-geo/src/lib.rs:7-17` + `crates/envi-store/src/geojson.rs:78-93` (the one call site style)
**Apply to:** `envi-gis`. RD New goes INTO `envi-geo`; `envi-gis` calls `envi_geo` types only — no inline proj strings, no second `proj4rs` dependency edge (a `cargo tree`-style check comment in Cargo.toml mirrors the existing gates).

## No Analog Found

Files with no close match in the codebase (planner should use 08-RESEARCH patterns/code-examples instead):

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `crates/envi-gis-wasm/src/lib.rs` | cdylib bindings | JsValue ⇄ DTO | First WASM crate in the repo. Use research §Standard Stack (`wasm-bindgen` 0.2.126 + `serde-wasm-bindgen` 0.6.5), keep it logic-free ("boundary ONLY"), pin CLI version in lockstep (Pitfall 8), document the build step in `web/README` |
| `web/src/import/opfs.ts` | browser cache I/O | file-I/O | First OPFS use. Full code sketch in 08-RESEARCH §Code Examples (`putTile`/`getTile`, async main-thread API — Pitfall 10 forbids sync handles outside workers). Adopt `client.ts` module-header style |
| Vite/wasm build wiring (`web/vite.config.ts` MOD + Wave-0 `wasm-bindgen-cli` install) | config | — | No prior wasm build step. Research §Environment Availability lists the install commands; keep `wasm-bindgen` crate ↔ CLI versions identical |

## Metadata

**Analog search scope:** `crates/envi-dgm`, `crates/envi-geo` (+ tests/fixtures), `crates/envi-store/src/geojson.rs`, `crates/envi-engine/src/scene.rs`, `crates/envi-service/src/{api,error.rs,tests}`, `web/src/{api,dgm,panels,store,generated}`, `web/tests/e2e`, `tools/crs_oracle`, workspace `Cargo.toml`
**Files scanned:** 18 read in full or targeted-range; directory listings across 6 trees
**Pattern extraction date:** 2026-07-10
