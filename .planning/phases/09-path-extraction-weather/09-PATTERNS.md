# Phase 9: Path Extraction & Weather - Pattern Map

**Mapped:** 2026-07-11
**Files analyzed:** 22 (new/modified across `envi-gis`, `envi-gis-wasm`, `envi-service`, `web/`)
**Analogs found:** 22 / 22 (all seams have an in-tree analog; only the r.profile/ERA5/Open-Meteo *fixtures* are net-new committed data)

> Phase 9 adds **zero new numerical kernels** — every acoustic contract already exists and is validated. The risk is entirely geometry-reduction correctness + plumbing discipline. So nearly every new file has a strong same-role analog to copy, and the copy is load-bearing: the conventions below (typed errors, no-silent-0.0, DoS caps, dep-quarantine, `#[must_use]`, Module-I/O doc headers, committed-fixture tests, ts-rs no-drift, offline-Playwright) are the phase's real deliverable discipline, not the math.

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/envi-gis/src/profile.rs` (NEW) | service (pure transform) | transform / file-I/O-free | `crates/envi-gis/src/terrain.rs` + `crates/envi-dgm/src/tin.rs` | exact (role) + exact (seam) |
| `crates/envi-gis/src/impedance.rs` (NEW) | service (pure transform) | transform | `crates/envi-gis/src/landcover.rs` + `impedance_table.rs` + `envi-store/src/geojson.rs` | exact |
| `crates/envi-gis/src/screening.rs` (NEW) | service (pure transform) | transform (spatial query) | `crates/envi-gis/src/landcover.rs` (geo) + `rstar` (new dep) | role-match |
| `crates/envi-gis/src/grid.rs` (NEW) | service (pure transform) | batch (lattice gen) | `crates/envi-dgm/src/tin.rs` `build_tin` | exact (seam) |
| `crates/envi-gis/src/weather.rs` (NEW) | service (pure math) | transform | `crates/envi-harness/src/weather/{mod,route3}.rs` | exact (lift, do not fork) |
| `crates/envi-gis/src/era5.rs` (NEW) | service (pure math) | transform / batch-stats | `crates/envi-harness/src/weather/route3.rs` + committed-fixture test | role-match |
| `crates/envi-gis/src/lib.rs` (MODIFY) | config / error hub | — | its own existing `GisError` enum + `mod` block | exact |
| `crates/envi-gis/Cargo.toml` (MODIFY) | config | — | own `[dependencies]` + boundary-comment | exact |
| `crates/envi-gis/tests/profile_oracle.rs` (NEW) | test (oracle) | file-I/O (fixture) | `crates/envi-gis/tests/cog_window.rs` | exact |
| `crates/envi-gis/tests/fixtures/rprofile/*` (NEW) | fixture | — | `crates/envi-gis/tests/fixtures/cog/*` | exact |
| `crates/envi-gis/tests/fixtures/{era5_synthetic,openmeteo_*}` (NEW) | fixture | — | `tests/fixtures/cog/*.toml` provenance pattern | exact |
| `crates/envi-gis-wasm/src/dto.rs` (MODIFY) | provider (wire DTOs) | request-response | its own existing DTOs (ts-rs, `deny_unknown_fields`) | exact |
| `crates/envi-gis-wasm/src/lib.rs` (MODIFY) | controller (WASM boundary) | request-response | its own existing `#[wasm_bindgen]` shims | exact |
| `crates/envi-service/src/api/era5.rs` (NEW) | controller (HTTP handler) | request-response + async-job | `crates/envi-service/src/api/proxy.rs` + `src/jobs.rs` | role-match |
| `crates/envi-service/src/api/mod.rs` (MODIFY) | route | — | own route table (`get(proxy::relay)`) | exact |
| `crates/envi-service/Cargo.toml` (MODIFY) | config | — | own `reqwest` block + `[features]` | exact |
| `crates/envi-service/tests/wire_no_drift.rs` (MODIFY) | test (no-drift) | — | itself (register new DTOs in `export_all_wire_types`) | exact |
| `web/src/import/weather.ts` (NEW) | service (fetcher) | request-response + cache | `web/src/import/fetchers.ts` + `opfs.ts` | exact |
| `web/src/panels/WeatherPanel.tsx` (NEW) | component | event-driven (UI) | `web/src/panels/ImportPanel.tsx` | exact |
| `web/src/store/weather.ts` (NEW) | store | event-driven | `web/src/store/import.ts` (zustand) | exact |
| `web/src/map/*Overlay` (weather A/B/C) (NEW/MODIFY) | component (overlay) | event-driven | `web/src/map/impedanceOverlay.ts` / `DgmOverlay.tsx` | role-match |
| `web/tests/e2e/weather-import.spec.ts` (NEW) | test (Playwright offline) | event-driven | `web/tests/e2e/import-offline-replay.spec.ts` | exact |

---

## Shared Patterns (apply to ALL new files in the named crate)

### S-1. `envi-gis` module shape — the pure-Rust WASM-safe core contract
**Source:** `crates/envi-gis/src/terrain.rs:1-52`, `landcover.rs:1-57`, `lib.rs:41-52`
**Apply to:** `profile.rs`, `impedance.rs`, `screening.rs`, `grid.rs`, `weather.rs`, `era5.rs`

Every module opens with a `# Module I/O` doc header (Inputs / Output / load-bearing Invariants), then `use` blocks that route reprojection through `envi_geo` ONLY, then `pub const` caps, then `#[must_use]` pure functions, then a `#[cfg(test)] mod tests`. Copy this exact skeleton (from `terrain.rs`):

```rust
//! <one-line purpose> (GEOX-0X).
//!
//! # Module I/O
//! - **Inputs:** …
//! - **Output:** … (never a fabricated 0.0; `Option`/typed error on absence)
//! - **Invariants (load-bearing):**
//!   1. **Bounded output** (threat T-09-…): hard-capped at `MAX_…`.
//!   2. **No silent 0.0** (D-07): absence is `None`/typed error, never a default.
//!   3. **One reprojection boundary** (GEOX-04): through `envi_geo` only.
use crate::GisError;
```

Every fallible-on-data path returns `Result<_, GisError>` or `Option<_>` — **never `unwrap()`/`panic!` on a data path** (`lib.rs:35-40` house rules). `#[must_use]` on every pure returning fn (`terrain.rs:69,101,173,226`).

### S-2. `GisError` variants — typed, `PartialEq`, foreign errors wrapped as strings
**Source:** `crates/envi-gis/src/lib.rs:66-155`
**Apply to:** every new `envi-gis` module + `lib.rs` (add variants)

The enum derives `#[derive(Debug, Error, PartialEq)]` so tests `assert_eq!`/`matches!` on variants. Struct variants carry the offending values. Any non-`PartialEq` foreign error (e.g. an `rstar`/`geo`/`envi_dgm::DgmError`/`CaseLoadError`) is captured as a **message string** so the enum stays `PartialEq` — mirror the `Tiff`/`Reproject`/`Json` variants:

```rust
/// Reprojection through [`envi_geo`] failed … captured as a message so [`GisError`] stays `PartialEq`.
#[error("reprojection error: {message}")]
Reproject { message: String },
```
New variants Phase 9 will need (name to match): a profile/hull-miss error (`OutsideHull`), a grid guardrail (`SpacingTooSmall { got, min }` / `ReceiverCapExceeded { got, limit }`), a corridor cap, a weather-fit/non-finite error (wrap `CaseLoadError` as a message), and an ERA5 field-parse error. Follow the existing `DecodeBudgetExceeded`/`TooManyImages` struct-variant style (`lib.rs:71-86`) for the DoS caps.

### S-3. DoS caps as named `pub const` (bounded work / V12)
**Source:** `terrain.rs:40` (`MAX_TERRAIN_POINTS`), `landcover.rs:82-86` (`MAX_GROUND_ZONES`, `MAX_TOTAL_VERTICES`), `envi-dgm/src/tin.rs:31` (`MAX_POINTS = 500_000`)
**Apply to:** `grid.rs` (receiver-count cap ~`1_000_000` + min-spacing `1.0 m`), `screening.rs` (corridor-candidate cap), `profile.rs` (max profile points)

Every guardrail is a documented named const, checked before allocation, returning a typed `GisError` — never an `OOM`/panic. The research tagged these `[ASSUMED]` engineering values (A1/A2); make them named consts with the rationale in the doc comment, exactly like `MAX_TERRAIN_POINTS`'s comment.

### S-4. Provenance + one-source-of-truth for σ
**Source:** `crates/envi-gis/src/provenance.rs:66-100`, `impedance_table.rs:9-20,117-123`
**Apply to:** `impedance.rs` (GEOX-02), any weather output that stamps provenance

σ (flow resistivity) lives **only** in `envi_engine::scene::impedance_class` (`scene.rs:329`) — a module resolves the **class letter** and calls the engine; it **never restates a σ literal** (`impedance_table.rs` doc + the `every_row_resolves_sigma_through_the_engine` test at `impedance_table.rs:145-169`). GEOX-02 reuses `worldcover_to_class` (`impedance_table.rs:118`) for imported land cover and `impedance_class` for A–H letters. Class **B = 31.5** is the corrected value (`scene.rs:416`) — never re-derive it.

### S-5. Committed-fixture / oracle test (no Python/GRASS/CDS at test time)
**Source:** `crates/envi-gis/tests/cog_window.rs:1-58`, `crates/envi-gis/tests/fixtures/cog/`
**Apply to:** `profile_oracle.rs` (r.profile), `era5.rs` tests (synthetic ERA5 fields), `weather.rs` tests (Open-Meteo JSON fixtures)

The oracle test loads a committed `.tif` + expected-window `.toml` (generated once offline by `tools/gis_oracle/gen_cog_fixtures.py`), decodes with the pure-Rust path, and asserts `<= tol`. **Python/GDAL are NOT needed to pass** (doc header `cog_window.rs:12`). Mirror this exactly: commit the small real-DEM extract + `r.profile` CSV under `tests/fixtures/rprofile/` with documented provenance + SHA (the single load-bearing new fixture, RESEARCH Wave 0). Pin a **documented tolerance** (TIN-barycentric vs raster-bilinear delta, A3) — do not chase bit-equality.

### S-6. Dependency quarantine — verified by `cargo tree`, not a test
**Source:** `crates/envi-gis/Cargo.toml:8-17`
**Apply to:** `crates/envi-gis/Cargo.toml` (add `rstar = "0.13"` ONLY), `crates/envi-engine` (add NOTHING)

The `envi-gis` runtime graph must stay free of any async/network/browser crate (`cargo tree -p envi-gis` shows no HTTP client, no async runtime, no `web-sys`). `rstar` is pure-Rust/WASM-safe and allowed; `reqwest`/`tokio` are **forbidden here** (Pitfall 7) and belong only in `envi-service` behind `feature=era5`. The `envi-engine` 3-dep quarantine (`ndarray + num-complex + thiserror`) is **byte-identical** — add nothing to it; all new code is `envi-gis`/`envi-service`/`web`.

### S-7. ts-rs wire DTOs — single source of truth, no-drift, no hand-authored TS
**Source:** `crates/envi-gis-wasm/src/dto.rs:1-100`, `crates/envi-service/tests/wire_no_drift.rs:88-164`
**Apply to:** `envi-gis-wasm/src/dto.rs`, `wire_no_drift.rs`

Every boundary DTO derives `#[derive(… Serialize/Deserialize, TS)]` with `#[ts(export_to = "wire.ts")]`; request DTOs are `#[serde(deny_unknown_fields)]`. A **hand-written TS mirror is forbidden** — after adding a DTO you MUST register it in `export_all_wire_types` (`wire_no_drift.rs:88`) and regenerate the committed `web/src/generated/wire.ts` via `cargo test -p envi-service --test wire_no_drift -- --ignored regenerate_committed_wire_ts`. The `wire_ts_matches_committed_source` test (`wire_no_drift.rs:191`) fails in Rust, not the browser, on drift. Bulk `Vec<u8>` bytes cross as a direct `&[u8]` param, never a serde field (`dto.rs:14-16`).

### S-8. 105-band index rule where weather meets frequency
**Source:** CLAUDE.md numerics house rule; `scene.rs` `N_BANDS`
**Apply to:** `weather.rs`, `era5.rs`, any wire DTO carrying a spectrum

Compare/index by **band index, never nominal Hz** (recurring pitfall 5). Phase-9 weather A/B/C are per-azimuth scalars feeding `SoundSpeedProfile` (not a spectrum), so the band rule mostly bites when a weather output later meets the `[105]` grid — keep engine-facing spectra dense `[105]` and band-indexed.

---

## Pattern Assignments

### `crates/envi-gis/src/profile.rs` (GEOX-01 — cut-profile extractor)

**Analogs:** `crates/envi-gis/src/terrain.rs` (sampling + no-silent-0.0 + `#[must_use]`), `crates/envi-dgm/src/tin.rs` (the Z sampler), `crates/envi-engine/src/scene.rs` (the `TerrainProfile` target).

**The Z-sampler seam** — reuse, do not re-roll (`tin.rs:76`):
```rust
pub fn interpolate_z(&self, x: f64, y: f64) -> Option<f64> // barycentric, None outside hull — NEVER silent 0.0
```
`build_tin` caps at `MAX_POINTS = 500_000` and returns `DgmError` on intersecting constraints (`tin.rs:158`) — the safety posture to wrap.

**Core sampling pattern** — the S→R walk producing strictly-ascending `(x, z)` (RESEARCH Pattern 1, derived from `terrain.rs` no-silent-0.0 discipline):
```rust
let z = tin.interpolate_z(px, py).ok_or(GisError::OutsideHull)?; // never silent 0.0
if x > last_x + 1e-9 { pts.push([x, z]); last_x = x; }           // enforce strictly-ascending x
```

**The `TerrainProfile` contract this MUST satisfy** (`scene.rs:218-265`, load-bearing):
- `TerrainProfile::new(points, segments)` rejects `EmptyProfile`, `SegmentCountMismatch` (`segments.len() != points.len()-1`), `NonFinite`, `NonAscendingX` — **strictly ascending x, N points ⇒ N−1 segments** (`scene.rs:222-251`).
- **hSv/hRv trap** (`scene.rs:288-297`): `endpoints(h_s, h_r)` puts the source above the **first** point, receiver above the **last**. The extractor emits **ground z ONLY** — never bake src/rcv acoustic height into a profile z (Pitfall 2).

**Oracle test:** copy `cog_window.rs` structure (S-5) — `tests/profile_oracle.rs` loading `tests/fixtures/rprofile/{dem extract, r.profile CSV}`.

---

### `crates/envi-gis/src/impedance.rs` (GEOX-02 — impedance segmentation)

**Analogs:** `crates/envi-gis/src/landcover.rs` (geo polygon ops + provenance emit), `impedance_table.rs` (`worldcover_to_class`), `crates/envi-store/src/geojson.rs` (the `ground_zone` kind + letter resolution), `crates/envi-engine/src/scene.rs` (`impedance_class`, `GroundSegment`).

**σ resolution — the one source of truth** (`impedance_table.rs:118`, `scene.rs:329`):
```rust
worldcover_to_class(code) -> Option<char>            // imported land cover → A..H letter (never σ here)
envi_engine::scene::impedance_class(letter) -> Option<f64>  // letter → σ (class B = 31.5)
```
Priority **drawn > imported > default** resolved **per interval** (not per point — the `GroundSegment` belongs to the span; off-by-one silently shifts every ground reflection, RESEARCH anti-pattern).

**`GroundSegment` shape** (`scene.rs:188-194`): `{ flow_resistivity: f64, roughness: f64 }`. The "row that starts the segment" convention (segment `i` carries `points[i]`'s properties) is the `build_terrain_inputs` rule cited in RESEARCH — splice a profile vertex at every polygon-boundary ∩ line crossing before building segments, keeping x strictly ascending.

**geo ops to reuse** (already an `envi-gis` dep — see `landcover.rs:48`): `geo::{Contains, LineString, Polygon, Line}` for point-in-polygon + line∩ring crossings. Do not hand-roll predicates.

---

### `crates/envi-gis/src/screening.rs` (GEOX-03 — screen edges → profile vertices)

**Analogs:** `crates/envi-gis/src/landcover.rs` (geo geometry handling), `rstar 0.13` (new dep), `crates/envi-engine/src/scene.rs` (`Building.eaves_height_m`, `Barrier`).

**THE reshape (Pitfall 1, single biggest finding):** the engine `SolveJob` has **NO `screens` field** — only `profile: &TerrainProfile` (`solver.rs:63-121`, verified: the struct is `sub_source, receiver, profile, src, rcv, atmosphere, coh, axis, weather, directivity_gain_db, directivity_phase_rad, forest, isolation`). GEOX-03 therefore **inserts each cut-plane∩wall-top crossing as an `(x, z)` vertex into the same `TerrainProfile`** with a hard (high-σ, class H = 200000) `GroundSegment`; `terrain_interpretation.rs` extracts the ≤2 diffracting edges. **Never emit a `Vec<ScreenEdge>` to the solver** (warning sign in RESEARCH Pitfall 1).

**rstar corridor query** (RESEARCH Pattern 3 example):
```rust
let tree: RTree<GeomEntry> = RTree::bulk_load(entries);       // buildings+walls+barriers, AABB-keyed
let half_w = fresnel_half_width(d, 250.0).max(20.0);          // [ASSUMED] named const, documented
for cand in tree.locate_in_envelope_intersecting(&corridor_aabb(s_xy, r_xy, half_w)) { … }
```
Height convention: injected `z = ground_z(crossing) + object_height` (`eaves_height_m` / `height_m`). A building the line passes through → **two** top vertices (thick screen = Sub-model 5). Multi-edge is capped at Nord2000's native ≤2 screens — document as a limitation, do not build an N-edge combiner (Pitfall 4). `GisError` wrapping of geometry failures follows S-2.

---

### `crates/envi-gis/src/grid.rs` (GRID-01 — building-aware receiver grid)

**Analog:** `crates/envi-dgm/src/tin.rs` `build_tin` (spade CDT, footprints as constraint edges/holes).

**Reuse the CDT + its safety posture** (`tin.rs:158-235`): `build_tin(points, breaklines)` already rejects intersecting constraints (`can_add_constraint` guard, `DgmError::IntersectingConstraint`) and caps input at `MAX_POINTS`. Build the CDT with `calc_area` boundary + footprint rings as constraints to define the valid region, then emit receivers on a **regular lattice clipped to that region** (predictable spacing for a noise map — RESEARCH Pattern 4 recommends lattice over literal CDT vertices; confirm in planning).

**Guardrails (S-3):** min spacing `1.0 m` + receiver-count cap (~`1_000_000`) as named consts returning typed `GisError` (mirror `envi_dgm::MAX_POINTS`). D-07: **no receiver inside a footprint** (geo point-in-polygon exclusion). Sample each kept point's z via `Tin::interpolate_z` (S-1 Z-sampler) and add receiver height at `SolveJob` assembly (never as a profile z).

---

### `crates/envi-gis/src/weather.rs` (METX-01 — Open-Meteo multi-level → per-azimuth A/B/C)

**Analog (LIFT, do not fork):** `crates/envi-harness/src/weather/mod.rs` + `route3.rs`.

**The exact math to reuse** (RESEARCH Don't-Hand-Roll — forking risks oracle divergence):
```rust
// crates/envi-harness/src/weather/route3.rs:199
pub fn fit_profile(heights: &[f64], c_eff: &[f64], z0: f64) -> Result<(f64,f64,f64), CaseLoadError>
// 3×3 Cramer LSQ over basis [ln(z/z0+1), z, 1]; clamps z0 >= Z0_MIN_M; typed error on singular/non-finite

// crates/envi-harness/src/weather/mod.rs:91,117
pub struct WeatherComponents { a_temp, a_wind, b, c, s_a, s_b, z0 }   // temp part once, wind projected per bearing
pub fn profile_for_bearing(base: &WeatherComponents, bearing_deg, phi_u_deg) -> WeatherProfile
//   A(bearing) = a_temp + a_wind·cos(bearing − φ_u)   (downwind A > upwind A)
```
Open-Meteo gives u(z),T(z) **directly** → **skip Monin–Obukhov reconstruction** (that's Route 3); this is the Route-2 "profile fit". `c_eff(z) = 20.05·√(T+273.15) + u·cos(Δ)` is the same formula the harness uses. The engine target is `SoundSpeedProfile { a, b, c, s_a, s_b, z0 }` (`refraction/mod.rs:47-62`).

**Where it runs:** `envi-gis` WASM over OPFS-cached JSON — **NOT** in `web/` TS (the wire contract forbids client-side acoustic math), **NOT** server-side.

**[ASSUMED] quarantine (carried from Phase 3):** the fitted A/B/C inherit the harness `[ASSUMED]` weather-route status (`weather/mod.rs:11`, `route3.rs:19`). **Never promote to a false FORCE numeric pass** — validate structurally only (downwind A > upwind A; inversion ⇒ B > 0; round-trip fit identity), exactly as the harness tests do. Subtract site elevation from geopotential height (AMSL → AGL) before fitting (Pitfall 5). Fixtures: committed Open-Meteo JSON (S-5).

**Lift location note:** `WeatherComponents`/`profile_for_bearing`/`fit_profile` are pure math depending only on `envi_engine`; lift them into a WASM-safe location (`envi-gis::weather`) — the harness comment at `weather/mod.rs:44-46` anticipates a future collapse. Do NOT duplicate the LSQ.

---

### `crates/envi-gis/src/era5.rs` (METX-02 — Obukhov + wind×stability occurrence stats)

**Analogs:** `crates/envi-harness/src/weather/route3.rs` (finiteness-checked pure derivation + typed error), committed-fixture test (S-5).

**Pure derivation only** (D-04/D-05, RESEARCH Pattern 6): from committed synthetic ERA5 fields `(iews, inss, ishf, 2t, 2d, sp, sdfor)` compute `u*`, `θ*`, `1/L` (Obukhov), then **bin each hour into a wind-speed × stability class and count** → occurrence table. **Do NOT** map classes to A/B/C or combine into L_den (deferred to GRID-03). Bin edges are `[ASSUMED]` — validate the counting against a synthetic fixture, never a numeric weather pass. Finiteness checks mirror `route3.rs:130-146` (reject non-finite → typed error, never panic — T-03-03-02).

---

### `crates/envi-gis-wasm/src/dto.rs` + `lib.rs` (WASM boundary)

**Analog:** the crate's own existing DTOs + shims (`dto.rs`, `lib.rs`).

**DTO pattern** (`dto.rs:29-41` etc.): `#[derive(Debug, Clone, Serialize/Deserialize, TS)] #[serde(deny_unknown_fields)] #[ts(export_to = "wire.ts")]`. New Phase-9 DTOs: cut-profile request/result, impedance-segmentation, screening, grid (spacing + calc_area + footprints), weather-derivation (multi-level JSON → per-azimuth A/B/C), ERA5-derivation. GeoJSON payloads cross as `#[ts(type = "unknown")] serde_json::Value` (`dto.rs:234,339`); bulk arrays as `&[u8]` params.

**Boundary shim pattern** (`lib.rs:56-85, 316-350`) — logic-free marshaller: `from_js` → call one `envi_gis` core fn → `to_js`; map `GisError` via `gis_err` (`lib.rs:83`). `to_js` MUST use `.serialize_maps_as_objects(true)` (`lib.rs:71-74`) or nested GeoJSON reads `undefined`. Register every new DTO in `wire_no_drift.rs` `export_all_wire_types` and regenerate `wire.ts` (S-7).

---

### `crates/envi-service/src/api/era5.rs` + `mod.rs` + `Cargo.toml` (flagged ERA5 endpoint)

**Analogs:** `crates/envi-service/src/api/proxy.rs` (allowlist + SSRF chokepoint), `crates/envi-service/src/jobs.rs` (async state machine).

**Async-job state machine — reuse, do not build a new one** (`jobs.rs:54-81`):
```rust
pub enum JobStatus { Queued, Running { progress, message }, Done, Failed { reason }, Cancelled }
impl JobStatus { pub fn is_terminal(&self) -> bool { … } }   // + watch::channel worker thread + SSE
```

**SSRF allowlist chokepoint — copy the shape** (`proxy.rs:48-101`): the hardcoded `SOURCES: &[(&str,&str,&str)]` (id, host, prefix), `resolve_upstream` validating **before any I/O** (`NotFound` unknown source, `BadRequest` on `..`/prefix-escape), hardcoded `https` scheme + host (caller never supplies a host), `MAX_RELAY_BYTES` cap. The CDS host is pinned exactly like `glo30`/`worldcover`.

**Feature flag (D-04, default-OFF):** the retrieval stub is `#[cfg(feature = "era5")]`; the endpoint is disabled by default. Add `[features] era5 = ["dep:reqwest"...]` to `Cargo.toml` mirroring the existing `reqwest` block (`envi-service/Cargo.toml:44-58`, already `default-features = false, rustls-tls`). Route registration in `api/mod.rs` mirrors `.route("/proxy/{source}/{*path}", get(proxy::relay))` (`mod.rs:92-94`). A `era5_flag_off` test asserts the route is absent by default. CDS key lives in server env only — never in a wire response, log, or the client bundle (V6/V10).

---

### `web/src/import/weather.ts` (Open-Meteo fetch + OPFS cache)

**Analogs:** `web/src/import/fetchers.ts` (direct-vs-proxy fetch + `ApiError` reuse), `web/src/import/opfs.ts` (per-project cache).

**Fetch pattern** (`fetchers.ts:49-60`): whole-resource plain `GET` (no `Range` → no CORS preflight), non-2xx → `throw new ApiError(status, detail)` from `../api/client` (never redeclare the error type). Date-switch (D-02): pick `api.open-meteo.com/v1/forecast` vs `archive-api.open-meteo.com/v1/archive` by requested date. Direct CORS first, login-server byte-proxy fallback via `proxyUrl` rewrite (`fetchers.ts:32-34`).

**OPFS cache (D-03, SC4)** — reuse `getTile`/`putTile` (`opfs.ts:54-85`) or an analogous `getWeather`/`putWeather` with the same `safeSeg` path-sanitization (`opfs.ts:36-40`) and fixed `projects/<uuid>/cache/weather/<key>` layout. **Cache key = `(lat, lon, timestamp)`** (D-01). What-if edits read OPFS ONLY — zero API calls (proven by Playwright, S below).

### `web/src/store/weather.ts` + `WeatherPanel.tsx` + overlay

**Analogs:** `web/src/store/import.ts` (zustand `create<State>`, `GuardrailState`), `web/src/panels/ImportPanel.tsx`, `web/src/map/impedanceOverlay.ts`.

**Panel pattern** (`ImportPanel.tsx:40-172`): every actionable control carries a `data-testid` (the E2E drives these); every string reaches the DOM as a React text child (**never `innerHTML`** — threat T-08-07-05); status chips reuse `.chip.warn`/`.chip.crit`. Date+hour picker → import → per-azimuth A/B/C readout, mirroring `LayerRow` status/progress/error/retry. Store mirrors `useImportStore` shape (`import.ts:71-115`).

### `web/tests/e2e/weather-import.spec.ts` (offline Playwright)

**Analog:** `web/tests/e2e/import-offline-replay.spec.ts`.

**Offline-first pattern** (`import-offline-replay.spec.ts:29-103`): `bootOffline(page)` returns an `unmocked` collector that MUST end `toEqual([])` (no live-network escape). Mock **both** Open-Meteo hosts + basemap + GIS with `page.route(/api\.open-meteo\.com/, …)` and `/archive-api\.open-meteo\.com/`. The SC4 proof: after a cached import, invert the network (record + `route.abort()`), re-run a what-if, and assert the weather-egress collector stays `[]` (`spec.ts:52-87`) — zero API calls on what-if. Log call-cost weight (`n_variables × n_timesteps / 100`) to a metric line.

---

## No Analog Found

None. Every seam has an in-tree analog. The only net-new artifacts with no code analog are **committed fixture data files** (their *test harness* copies `cog_window.rs`):

| Artifact | Role | Why net-new |
|----------|------|-------------|
| `tests/fixtures/rprofile/{dem, r.profile.csv}` | GRASS oracle fixture | GRASS is test-time-absent by design; generated offline once, committed with SHA/provenance |
| `tests/fixtures/era5_synthetic.*` | synthetic ERA5 fields | CDS needs a key + is queued-async; fixture avoids both |
| `tests/fixtures/openmeteo_{archive,forecast}.json` | Open-Meteo responses | offline derivation tests |

---

## Metadata

**Analog search scope:** `crates/envi-gis/{terrain,landcover,impedance_table,provenance,lib}.rs`, `crates/envi-gis-wasm/{dto,lib}.rs`, `crates/envi-dgm/src/tin.rs`, `crates/envi-engine/src/{scene,solver}.rs` + `propagation/refraction/mod.rs`, `crates/envi-harness/src/weather/{mod,route3}.rs`, `crates/envi-store/src/geojson.rs`, `crates/envi-service/src/{jobs.rs,api/proxy.rs}` + `tests/wire_no_drift.rs`, `web/src/import/{fetchers,opfs,importJob}.ts`, `web/src/panels/ImportPanel.tsx`, `web/src/store/import.ts`, `web/tests/e2e/import-offline-replay.spec.ts`.
**Files scanned:** ~24 source files across 8 crates + web.
**Pattern extraction date:** 2026-07-11
