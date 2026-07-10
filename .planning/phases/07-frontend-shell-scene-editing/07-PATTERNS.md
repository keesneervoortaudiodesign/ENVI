# Phase 7: Frontend Shell & Scene Editing - Pattern Map

**Mapped:** 2026-07-10
**Files analyzed:** ~32 new/modified files (backend Rust + `web/` frontend)
**Analogs found:** 24 with in-repo or metrao3 analogs / 32 total (8 genuinely greenfield — React store/map/draw/panels)

> Two very different mapping modes in this phase. **Backend = real analog reuse** (a
> new crate, DTOs, an endpoint, a no-drift test — all have exact in-repo precedent from
> Phases 5/6). **Frontend = convention transfer** from the sibling repo
> `D:\====CLAUDE\metrao3` (read-only): its *build/test config* and *visual layer* transfer;
> its vanilla-TS architecture does NOT (ENVI is React). Where React has no metrao3 analog,
> that is called out, not faked.

---

## File Classification

### A. Backend (Rust — real reuse)

| New/Modified File | Role | Data Flow | Closest Analog | Match |
|-------------------|------|-----------|----------------|-------|
| `crates/envi-dgm/Cargo.toml` | config | — | `crates/envi-geo/Cargo.toml` | exact |
| `crates/envi-dgm/src/lib.rs` | crate-root / model | transform | `crates/envi-geo/src/lib.rs` | exact |
| `crates/envi-dgm/src/tin.rs` (TIN math) | service (pure) | transform | `crates/envi-geo/src/transform.rs` | role+flow |
| `DgmError` (in `envi-dgm` lib.rs) | error type | — | `GeoError` (`envi-geo/src/lib.rs:64`) | exact (Archetype A) |
| `IsolationSpectrumDto` / `ForestParamsDto` (+ `TryFrom`) in `crates/envi-store/src/dto.rs` | model/DTO | transform | `dto.rs` `BandSpectrumDto` / `TerrainProfileDto` | exact |
| interpolation fn (`envi-store`, shared) | service (pure) | transform | band-index math in `api/meta.rs` + `freq.rs` | role-match |
| `POST /meta/interpolate-spectrum` handler | controller | request-response | `api/meta.rs::freq_axis` | exact |
| `DgmError → ApiError` mapping | middleware | — | `From<StoreError> for ApiError` (`error.rs:71`) | exact |
| route registration (`api/mod.rs`) | route | — | `api_router()` (`api/mod.rs:46`) | exact |
| `contract_interpolate_spectrum.rs` | test | request-response | `tests/contract_meta_static.rs` + `tests/common/mod.rs` | exact |
| `TryFrom` unit tests (`dto.rs`) | test | — | `dto.rs` `#[cfg(test)]` | exact |
| ts-rs derives on the ~27 DTOs | model | — | `tools/nord2000_oracle/gen_*.py` (committed-artifact) | role-match |
| **wire.ts no-drift test** | test | — | `crates/envi-harness/tests/oracle_ground.rs` | role-match |

### B. Frontend (`web/` — convention transfer from metrao3, read-only)

| New/Modified File | Role | Data Flow | Closest Analog (metrao3 `crates/metrao3-web/ui/`) | Match |
|-------------------|------|-----------|---------------------------------------------------|-------|
| `web/package.json` | config | — | `ui/package.json` | convention |
| `web/vite.config.ts` | config | — | `ui/vite.config.ts` | convention |
| `web/tsconfig.json` | config | — | `ui/tsconfig.json` | convention |
| `web/playwright.config.ts` | config | — | `ui/playwright.config.ts` | convention |
| `web/index.html` | config | — | in-repo `web/dist/index.html` (placeholder) + `ui/index.html` | partial |
| `web/src/theme.css` | style | — | `ui/src/theme.css` | **copy verbatim** |
| `web/src/app.css` | style | — | `ui/src/styles.css` (usage reference only) | partial |
| `web/src/icons.ts` | utility | — | `ui/src/icons.ts` | convention |
| `web/src/generated/wire.ts` | model | — | (generated, committed) | n/a — see D-10 |
| `web/tests/e2e/_mocks.ts` | test | request-response | `ui/tests/e2e/_mocks.ts` | convention (+ basemap intercept) |
| `web/tests/e2e/*.spec.ts` | test | — | `ui/tests/e2e/connect.spec.ts` | convention |
| `web/src/api/*.ts` (fetch client) | service | request-response | `ui/src/api.ts` — **structure yes, types NO (D-10)** | partial (divergent) |
| `web/src/store/*.ts` (zustand) | store | event-driven | — none (metrao3 is store-less vanilla) | **no analog** |
| `web/src/map/*.tsx` (MapLibre+TerraDraw) | provider/hook | event-driven | `ui/src/lifecycle.ts` (partial, see §Shared) | **no analog** |
| `web/src/draw/*.ts` (per-kind modes) | utility | event-driven | — | **no analog** |
| `web/src/panels/*.tsx` (inspector/palette) | component | request-response | — | **no analog** |
| `web/src/spectrum/*.tsx` (editor) | component | request-response | — | **no analog** |
| `web/src/validate/*.ts` (turf checks) | utility | transform | — | **no analog** |
| `web/src/main.tsx` / `App.tsx` | component | — | `ui/src/main.ts` (vanilla — structure only) | weak |

---

## Pattern Assignments — Backend

### `crates/envi-dgm/Cargo.toml` (new-crate scaffold)

**Analog:** `crates/envi-geo/Cargo.toml` — the most recent new crate (Phase 6). Copy its exact shape:

```toml
[package]
name = "envi-dgm"
description = "ENVI DGM boundary — constrained-Delaunay TIN from user-drawn elevation (D-08); spade lives HERE, never in envi-engine"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

# Boundary rule (D-08, 07-RESEARCH Gate 3): pure-Rust constrained-Delaunay TIN.
# spade + thiserror ONLY at runtime — zero C toolchain, zero I/O, NO serde in the
# runtime graph, and NO dependency on envi-engine (the 3-dep quarantine must not
# be reachable through this crate). serde/approx are dev-only if the TIN gets an
# oracle test.
[dependencies]
spade = "2.15"
thiserror = "2"
```

Binding shape rules verified against `envi-geo/Cargo.toml`: **workspace inheritance** on `version`/`edition`/`rust-version`; a **one-line boundary `description`**; a **boundary-rule comment block directly above `[dependencies]`**; **no `[lints]` table**; **no `[workspace.dependencies]`** (root `Cargo.toml` has only `[workspace.package]`, lines 5-8 — deps are declared per-crate). Root `members = ["crates/*"]` (line 3) means **`envi-dgm` joins the workspace automatically** — no root edit needed. Confirmed.

### `crates/envi-dgm/src/lib.rs` (crate root)

**Analog:** `crates/envi-geo/src/lib.rs`. Copy the module-doc + guard shape:

```rust
//! # envi-dgm
//!
//! [one-paragraph what/why]
//!
//! # Boundary statement (D-08)
//! [the spade-lives-here / never-reaches-engine rule, mirroring envi-geo's GEOX-04 block]
//!
//! # House rules
//! - `f64` throughout; typed errors ([`DgmError`]), never panics on data.
#![deny(unsafe_code)]

pub mod tin;
use thiserror::Error;
```

`#![deny(unsafe_code)]` at the crate root is mandatory (both `envi-geo/lib.rs:23` and `envi-store/lib.rs:38` have it). Note `spade` is pure Rust so this holds with no FFI exception.

### `DgmError` (Archetype A — pure domain error)

**Analog:** `GeoError` (`envi-geo/src/lib.rs:64-119`). This is the **pure-domain archetype**: `#[derive(Debug, Error, PartialEq)]`, **struct variants that carry the offending values** (got/expected style), `#[error("… got {x}, expected …")]` messages, and library errors wrapped as **message strings** to keep `PartialEq` (see `GeoError::Proj { message }`). `DgmError` should mirror it exactly — the 07-RESEARCH Pitfall 3 failure modes map to variants:

```rust
#[derive(Debug, Error, PartialEq)]
pub enum DgmError {
    #[error("degenerate TIN: {got} non-collinear points, need ≥ 3")]
    TooFewPoints { got: usize },
    #[error("breaklines intersect in their interior: segment ({a:?}) crosses ({b:?})")]
    IntersectingConstraint { a: [f64; 2], b: [f64; 2] },
    #[error("non-finite coordinate: {what}")]
    NonFinite { what: String },
    // spade InsertionError captured as a message so DgmError stays PartialEq (cf. GeoError::Proj)
    #[error("triangulation error: {message}")]
    Triangulation { message: String },
}
```

**Where it converts to `ApiError`:** add a `From<DgmError> for ApiError` arm in `crates/envi-service/src/error.rs`, mirroring the `StoreError::Geo(_) => BadRequest` grouping (`error.rs:86`). `TooFewPoints` / `IntersectingConstraint` / `NonFinite` → `ApiError::BadRequest { detail: e.to_string() }` (client fault, 4xx — never a 500/panic per Pitfall 3). If the DGM endpoint routes through `envi-store`, wrap it as a new `StoreError` variant instead; if `envi-service` calls `envi-dgm` directly, add the `From<DgmError>` on `ApiError` directly. Either way, **never `panic!`/`unwrap` on `spade`** — pre-validate with `intersects_constraint`/`can_add_constraint` (RESEARCH Gate 3).

**Contrast the three archetypes (all present in-repo):**
- **Archetype A** `GeoError` — pure domain, `PartialEq`, struct variants. → `DgmError` copies this.
- **Archetype B** `StoreError` (`envi-store/src/lib.rs:71`) — the I/O-crate error: **wraps `std::io::Error` via `#[source]`, always carries the offending `PathBuf`**, **no `PartialEq`** (io::Error isn't `PartialEq`; tests use `matches!`).
- **Archetype C** `ApiError` (`envi-service/src/error.rs:20`) — the HTTP boundary: `IntoResponse`, grouped `From<StoreError>` match, and **5xx bodies redact filesystem paths** (`error.rs:95` logs full detail via `tracing::error!` but returns generic `"internal error"`).

### `IsolationSpectrumDto` / `ForestParamsDto` + `TryFrom` (in `dto.rs`)

**Analog:** `crates/envi-store/src/dto.rs` — the validating-`TryFrom` pattern is already there twice. Two sub-patterns to copy exactly:

**(1) length + finiteness → engine constructor** (copy `BandSpectrumDto`, `dto.rs:41-68`):
```rust
impl TryFrom<&BandSpectrumDto> for envi_engine::scene::BandSpectrum {
    type Error = StoreError;
    fn try_from(d: &BandSpectrumDto) -> Result<Self, StoreError> {
        if d.band_db.len() != N_BANDS { return Err(StoreError::BadBandCount { got: d.band_db.len() }); }
        for (i, v) in d.band_db.iter().enumerate() {
            if !v.is_finite() { return Err(StoreError::NonFinite { what: format!("band_db[{i}] = {v}") }); }
        }
        let arr: [f64; N_BANDS] = d.band_db.as_slice().try_into().map_err(|_| StoreError::BadBandCount { got: d.band_db.len() })?;
        Ok(Self::from_values(arr))
    }
}
```

**(2) delegate to an engine validating constructor that owns the rules** (copy `TerrainProfileDto`, `dto.rs:237-246`):
```rust
impl TryFrom<&TerrainProfileDto> for TerrainProfile {
    type Error = StoreError;
    fn try_from(d: &TerrainProfileDto) -> Result<Self, StoreError> {
        let segments = d.segments.iter().map(GroundSegment::from).collect();
        TerrainProfile::new(d.points.clone(), segments)
            .map_err(|e| StoreError::Engine { message: e.to_string() })   // engine rejection → StoreError::Engine
    }
}
```

All DTOs carry `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]` + `#[serde(deny_unknown_fields)]` (request-facing) — `dto.rs:34-35`. **Exact engine target field lists** (so the planner specifies the conversion precisely):

**`IsolationSpectrum`** (`envi-engine/src/propagation/transmission.rs:93`):
```rust
pub struct IsolationSpectrum { r_db: [f64; N_BANDS] }   // field PRIVATE
pub const MAX_R_DB: f64 = 1_000.0;                        // :109
pub fn new(r_db: [f64; N_BANDS]) -> Result<Self, PropagationError>  // :125 — rejects NaN/±∞/<0/>MAX_R_DB
pub fn as_bands(&self) -> &[f64; N_BANDS]                 // :136
```
The private field forces the `new()` path — untrusted DTO data cannot bypass the `[0, 1000]` clamp. `IsolationSpectrumDto` holds **only the authored representation** (D-06): `{ authored: { resolution, values } }` where `values.len() ∈ {9, 27, 105}`; the `TryFrom` first interpolates to dense `[105]` (the shared fn), then calls `IsolationSpectrum::new`. Its error surfaces as `StoreError::Engine { message }` (there is no `IsolationSpectrum`-specific store variant yet — add one or reuse `Engine`, matching `TerrainProfileDto`).

**`ForestCrossing`** (`envi-engine/src/forest.rs:204`) — **all fields `pub`**:
```rust
pub struct ForestCrossing {
    pub d_m: f64,                // through-forest path length (Phase-9 geometry; NOT authored in Phase 7)
    pub density_per_m2: f64,     // n″ mean tree density  — WEB-04 "zero density" warn maps here
    pub stem_radius_m: f64,      // a  mean stem radius
    pub absorption: f64,         // α  mean absorption, [0,1] (clamped to [0,0.4] at lookup)
    pub height_m: f64,           // h  mean tree height
}
pub fn new(d_m, density_per_m2, stem_radius_m, absorption, height_m) -> Result<Self, ForestError>  // :228
```
`new()` rejects: non-finite anything; `d_m`/`density` < 0; `stem_radius`/`height` ≤ 0; `absorption` ∉ `[0,1]`. **`ForestParamsDto` authors only `{ density_per_m2, stem_radius_m, height_m, absorption? }`** — `d_m` is a solve-time geometry input (Phase 9), so the Phase-7 `TryFrom` proves conversion of the *authored* subset (supply a placeholder/￼`d_m` per the tested-conversion requirement, or convert into an intermediate params struct; the tested `TryFrom` just has to prove the authored fields validate). Error → new `ForestError`-wrapping `StoreError` variant (or reuse `Engine`).

**Tests:** copy `dto.rs` `#[cfg(test)]` (`dto.rs:405`) — assert (a) valid input converts and echoes bit-for-bit (`to_bits()` comparison, `dto.rs:450`), (b) out-of-range `R > 1000` / negative density → the right typed error via `matches!`, (c) JSON round-trip lossless (`dto.rs:542`).

### `POST /meta/interpolate-spectrum` (new endpoint)

**Analog:** `crates/envi-service/src/api/meta.rs` (`GET /meta/freq-axis`). Anatomy to copy:
- serde DTO(s) in the same module, `#[derive(Debug, Clone, Serialize/Deserialize)]` (`meta.rs:27`);
- Hz/band-index values built **from `envi_engine::freq` constants, never hardcoded** (`meta.rs:14, 45`, Pitfall 9);
- an `async fn` handler returning `Json<T>` (`meta.rs:53`). For the POST, take `Json<InterpolateReq>` in and return `Result<Json<InterpolateResp>, ApiError>` (validation failures → `ApiError::BadRequest`).

The **interpolation fn itself lives in `envi-store`** (shared by the endpoint AND `PUT /scene` validation so they cannot diverge — D-05), operating in dense `[105]` by band index (RESEARCH Pattern 6). The endpoint is a thin `envi-service` wrapper — acoustics never live in the service layer (SVC-07, `crates/README.md`).

**Route registration** — add one line to `api_router()` (`api/mod.rs:46`), brace syntax, no params (Pitfall 6):
```rust
.route("/meta/interpolate-spectrum", post(meta::interpolate_spectrum))
```
`post` is already imported (`api/mod.rs:33`). Any `/:id` colon straggler panics at construction — the contract test building the full `app()` catches it.

### `contract_interpolate_spectrum.rs` (contract test)

**Analog:** `crates/envi-service/tests/contract_meta_static.rs` + `crates/envi-service/tests/common/mod.rs`. Pattern: **in-process `tower::ServiceExt::oneshot` against the full `app()` router — no socket, no credentials** (`contract_meta_static.rs:10-11, 29`). `mod common;` then use its helpers.

**`common/mod.rs` exposes (confirmed — created/kept by the Phase-6 simplify pass):**
| Helper | Signature | Use |
|--------|-----------|-----|
| `test_app()` | `-> (Router, TempDir)` | full app over a fresh `TempDir` store |
| `app_over(&Path)` | `-> Router` | app over an existing root (simulate fresh process) |
| `test_state()` | `-> (Arc<AppState>, TempDir)` | state before router (direct job submit) |
| `app_of(Arc<AppState>)` | `-> Router` | router over shared state |
| `repo_web_dist()` | `-> PathBuf` | `<workspace>/web/dist` (two levels up from crate) |
| `get(uri)` | `-> Request<Body>` | GET, empty body |
| `delete(uri)` | `-> Request<Body>` | DELETE |
| `json_req(Method, uri, &Value)` | `-> Request<Body>` | JSON body + content-type |
| `read_json(resp)` | `-> (StatusCode, Value)` | collect body; empty ⇒ `Null` |
| `source_receiver_scene(f64)` | `-> Value` | one source + one receiver near Amsterdam |
| `make_project(&Router, f64)` | `-> String` (project id) | create project + PUT scene |

Assertions per RESEARCH Validation §: valid 9/27/105 → 200 with `[105]`; wrong length → 4xx; non-finite → 4xx; `R > 1000` → 4xx. Use `TempDir` for the store root (`common/mod.rs:46`).

### ts-rs generated wire types + no-drift test (D-10)

**Analog for the mechanism:** `tools/nord2000_oracle/gen_ground_fixtures.py` + `crates/envi-harness/tests/oracle_ground.rs` — the **generate-at-dev-time, commit-the-artifact, test-asserts-no-drift** pattern.

**Provenance-header convention** (copy the shape onto the generated `wire.ts` banner):
```
# generated by tools/nord2000_oracle/gen_ground_fixtures.py — DO NOT EDIT
# Cross-implementation oracle: scipy.special.wofz + AV 1106/07 Eqs. 57-60.
[meta]
provenance = "common.py sha256:fecb85555466464a"
```
i.e. a **`generated by … — DO NOT EDIT` first line** + a **provenance line tracing the artifact to the generator**. For `wire.ts`, the `ts-rs` header should carry an equivalent `// GENERATED by cargo test (ts-rs) from envi-store/envi-service DTOs — DO NOT EDIT`.

**How the committed artifact is asserted** — `oracle_ground.rs:46-53` loads the committed file by `concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/…")` and compares engine output against it; the fixture is committed, Python is **not** needed at test time. **For D-10, invert it slightly:** the `#[test]` re-runs `ts-rs` export to a `TempDir`, then asserts **byte-equality** against the committed `web/src/generated/wire.ts` (or `git diff --exit-code web/src/generated/wire.ts`). Same "artifact committed, generator not needed to *pass*, drift fails the build" contract.

**Where the no-drift test should live:** in the crate that owns the derives. Since the ~27 DTOs span `envi-store` (13) and `envi-service` (14), put the `#[ts(export, export_to=…)]` derives on each DTO and the single no-drift `#[test]` in **`envi-service`** (it depends on `envi-store`, so it sees both type sets) — e.g. `crates/envi-service/tests/wire_no_drift.rs`. Generation runs under `cargo test` (Claude's-discretion "Rust or JS" → Rust, matching the oracle precedent). `JobStatus` (`jobs.rs:50`, `#[serde(tag="state")]`) renders as a real TS discriminated union via ts-rs 12 `serde-compat` (RESEARCH Gate 2 — no zod fallback needed).

---

## Pattern Assignments — Frontend (metrao3 convention transfer)

### `web/package.json`

**Analog:** `metrao3/crates/metrao3-web/ui/package.json`. Transfer the **conventions**, not the deps:
- `"type": "module"`, `"private": true`, `"engines": { "node": ">=20" }`;
- script names: `"dev": "vite"`, `"build": "tsc --noEmit && vite build"` (typecheck gates the build), `"preview": "vite preview"`, `"test:e2e": "playwright test"`, `"test:e2e:install": "playwright install --with-deps chromium"`;
- **devDependencies-ONLY rule for tooling** — vite/typescript/@playwright/test are dev-only; **nothing tooling ships in the bundle**. Description states it explicitly ("Build tooling is devDependencies ONLY").

**DIVERGENCE (do not copy):** metrao3 has *zero runtime deps* (vanilla TS, embedded via rust-embed). ENVI has real **dependencies** (react, react-dom, maplibre-gl, react-map-gl, terra-draw + adapter, zustand, @turf/turf — RESEARCH Standard Stack). Those go under `"dependencies"`; only the tooling stays dev-only.

### `web/vite.config.ts`

**Analog:** `ui/vite.config.ts`. Copy: `base: "./"` (relative-path bundle — works regardless of mount path), `build.target: "es2022"`, `cssCodeSplit: false`, `assetsInlineLimit: 4096`, `emptyOutDir: true`, `modulePreload: { polyfill: false }`, and the **`// # Module I/O` header on the config file** (input = `src/main.tsx` + index.html root; output = `dist/` self-contained bundle). Add the React plugin (`@vitejs/plugin-react`) — metrao3 has none. `outDir: "dist"` → ENVI's is served from `web/dist` by `envi-service`.

### `web/tsconfig.json`

**Analog:** `ui/tsconfig.json`. Copy the strictness contract verbatim: `"strict": true`, `"noUnusedLocals": true`, `"noUnusedParameters": true`, `"noFallthroughCasesInSwitch": true`, `"verbatimModuleSyntax": true`, `"isolatedModules": true`, `"moduleResolution": "bundler"`, `"types": []`. **ENVI additions:** `"jsx": "react-jsx"` and `"lib": ["ES2022","DOM","DOM.Iterable"]` (metrao3 already lists the DOM libs). `noUnusedParameters` matters for the D-09 discriminated-union `never`-exhaustiveness discipline.

### `web/playwright.config.ts` + `web/tests/e2e/_mocks.ts`

**Analog:** `ui/playwright.config.ts` + `ui/tests/e2e/_mocks.ts`. The house E2E pattern:
- **drives the REAL built bundle** via a `webServer` block (`command: "npm run dev -- --port … --strictPort"`, `reuseExistingServer: !CI`);
- `page.route("**/api/…", route => route.fulfill({ json: … }))` mocks every `/api/*` so the suite runs **fully offline, no credentials** (`_mocks.ts:108-135`);
- `@playwright/test` is a **devDependency only, never bundled**;
- an `installApiMocks(page, opts)` helper centralizes fixtures (`_mocks.ts:102`).

**ENVI-CRITICAL ADDITION (D-13a):** the basemap is a runtime network XHR (OpenFreeMap dark). Playwright must ALSO `page.route`-intercept the **basemap tile/style/glyph** requests, not just `/api/*`, or a test silently hits the network (a failed test). metrao3 has no map, so this interception is new — add it to `installApiMocks` (or a sibling `installBasemapMocks`). Mock the style JSON, `{z}/{x}/{y}` tiles, and `{fontstack}/{range}.pbf` glyphs.

### `web/src/theme.css`

**Analog:** `ui/src/theme.css` — **copy VERBATIM** (D-11), the embedded system-font variant. Its header already documents the system-font-stack rationale (`theme.css:1-4`: omit `Inter`/`JetBrains Mono` so the offline bundle never blocks on a web font). The token set IS the contract — surfaces `--color-bg #0b0d10`→`--color-surface-3`, `--color-primary #4ea8ff`, the `ok/warn/crit/off/info` severity vocabulary, `--space-0..12`, the type scale, `--row-h`/`--row-h-lg`. **Do not invent tokens** (UI-SPEC TOKEN GAPS section). Use the **`ui/src/theme.css`** ancestor, NOT `portal/src/theme.css` (which lists web-font names first).

### `web/src/icons.ts`

**Analog:** `ui/src/icons.ts`. Copy the pattern exactly: a fixed `PATHS` constant of stroke paths in a **0..16 viewBox**, `document.createElementNS(...)` DOM construction (**never `innerHTML`**), `stroke="currentColor"`, `stroke-width="1.4"`, round caps/joins, `aria-hidden="true"` (`icons.ts:17-33`). ENVI expands `IconName` to the 9-kind palette glyphs (UI-SPEC Object Palette table). The `// # Module I/O` header (`icons.ts:3-6`) is part of the convention — keep it.

### `web/src/api/*.ts` (fetch client) — **DIVERGENCE FLAG**

**Analog:** `ui/src/api.ts` — copy its *structure* (typed promise-returning fetch wrappers; an `EventSource`/SSE wrapper paired with `.close()` for the job stream; `Content-Type` handling). **DO NOT copy its type strategy:** metrao3 **hand-writes** its wire interfaces (`api.ts:30-44` `interface Peer {…}`, etc.). ENVI (D-10) **imports the generated `web/src/generated/wire.ts`** instead — the whole point is that a renamed Rust field fails the no-drift test, not the browser. So: copy the fetch/SSE plumbing shape, and **import types from `../generated/wire`**, never re-declare them. This is the "copy the wrong half" trap called out in the task — the divergence is deliberate.

### `web/index.html`

**Analogs:** the in-repo placeholder `web/dist/index.html` (source ancestor for the `<title>`/offline discipline — zero external assets) + `ui/index.html` (Vite entry structure). The new `web/index.html` is the **Vite build root** (referenced by `vite.config.ts`), replacing the Phase-6 placeholder philosophy of "no external stylesheets, fonts, or scripts". A runtime MapLibre tile XHR is NOT an `index.html` asset, so the Phase-6 "zero external assets in index.html" gate stays green (D-13a).

### React files with NO analog (store / map / draw / panels / spectrum / validate)

metrao3 is vanilla TS with a hand-rolled DOM layer and no state store, so **there is no analog** for `web/src/store/` (zustand), `web/src/map/` (react-map-gl + Terra Draw lifecycle), `web/src/draw/`, `web/src/panels/`, `web/src/spectrum/`, `web/src/validate/`. The planner should use **07-RESEARCH Patterns 1-7** (app-store-canonical Terra Draw, `setStyle` re-hydration, instance-in-ref StrictMode guard, debounced autosave, per-edge UUID ring diff, server-owned interpolation) as the source-of-truth for these, and the **UI-SPEC** for their visual composition. See "No Analog Found" below.

---

## Shared Patterns

### Module I/O header (crosses Rust AND TS)

Every new module — Rust and TS — carries a module-level I/O header. **Two dialects, same intent:**

- **Rust** (`envi-store/src/hash.rs:1-31`, `envi-geo/src/transform.rs:1-25`): a `//!` doc-comment with a title line then named `# …` sections (`# Frozen encoding`, `# Boundary statement`, `# Convention`, `# House rules`). Describes what the module owns and its invariants.
- **TS** (metrao3 `lifecycle.ts:1-11`, `icons.ts:3-6`, `vite.config.ts:3-7`): a `// <file> — <one-liner>` line then a literal **`// # Module I/O`** block with `// - Input …` / `// - Output …` bullets (and a "Valid input range" note). **metrao3's TS files use the SAME `# Module I/O` header convention as ENVI's Rust doc-headers** — replicate it on every new `web/src/*.ts(x)` file.

### Error → HTTP boundary

**Source:** `crates/envi-service/src/error.rs` (`From<StoreError> for ApiError`, `:71`).
**Apply to:** the interpolate endpoint, the DGM endpoint, any new handler. Validation faults → `BadRequest` (client), filesystem/serialization → `Internal` with a **path-redacted generic body** (`:95`). Add `DgmError` into this grouping (see DgmError section). Structured JSON body `{ "error": <code>, "detail": <value> }` always — never a bare string or HTML.

### Quarantine gates (must stay green — invoke in each PLAN's `<automated>` block)

**Source:** `crates/README.md:42-65` (precedent: the 04-01 / 06-0x PLAN `<automated>` verification blocks). Exact commands:
```sh
cargo tree -p envi-engine -e normal --depth 1     # must be EXACTLY ndarray + num-complex + thiserror
grep -rh '\.conj()' crates/envi-engine/src/propagation/     # must return 0
cargo tree | grep -ci 'proj-sys\|gdal'            # must return 0 (no C crates this phase)
```
`envi-dgm` adds `spade` — confirm `cargo tree -p envi-dgm` shows `spade` **but that `envi-dgm` does NOT depend on `envi-engine`** (so `spade` can never reach the engine graph). `spade` is pure Rust → the zero-C-crate gate stays 0 (RESEARCH: `spade` 2.15.1 has no C deps). `envi-engine` stays **byte-identical** this phase (VERIFY-ONLY).

### Test conventions

**Source:** `dto.rs` (`#[cfg(test)]` inline unit tests), `contract_meta_static.rs` + `common/mod.rs` (`tests/` integration).
- unit tests inline via `#[cfg(test)] mod tests` for pure conversion/validation logic;
- HTTP contract tests in `tests/` use `tower::ServiceExt::oneshot` against the full `app()` — **no socket binding**;
- `tempfile::TempDir` for the projects root (`common/mod.rs:46`);
- assert bit-exactness with `to_bits()` where float identity matters (`dto.rs:450`, `contract_meta_static.rs:50`).

---

## No Analog Found

Files with no close in-repo or metrao3 match (planner uses 07-RESEARCH patterns + UI-SPEC instead):

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `web/src/store/*.ts` | store | event-driven | metrao3 is store-less vanilla TS; use RESEARCH Pattern 1/5 (app-store-canonical + ring diff) |
| `web/src/map/*.tsx` | provider/hook | event-driven | No React/MapLibre/TerraDraw code in either repo; RESEARCH Patterns 1-3 (Gate-1 spike) |
| `web/src/draw/*.ts` | utility | event-driven | Per-kind Terra Draw mode config — greenfield; RESEARCH + UI-SPEC palette |
| `web/src/panels/*.tsx` | component | request-response | Inspector/palette/validation — greenfield; UI-SPEC is the contract |
| `web/src/spectrum/*.tsx` | component | request-response | Curve+table editor — greenfield; UI-SPEC editor section |
| `web/src/validate/*.ts` | utility | transform | turf.js ground-zone topology — greenfield; RESEARCH Pattern (D-07) |
| interpolation core fn | service | transform | Band-index interpolation is new logic; RESEARCH Pattern 6 gives the exact stride math |

---

## Answers to the three explicit call-outs

**(a) Analog for the generated-types no-drift test:**
`crates/envi-harness/tests/oracle_ground.rs` (paired with `tools/nord2000_oracle/gen_ground_fixtures.py`). It is the repo's canonical **commit-the-artifact + test-asserts-no-drift** pattern: a `DO NOT EDIT` + `sha256` provenance header on the committed file, generation operator-driven (not needed to *pass*), and the `#[test]` loads the committed artifact by `CARGO_MANIFEST_DIR` path and asserts a match. D-10's `wire.ts` test mirrors it, inverted to "regenerate to TempDir + assert byte-equality (or `git diff --exit-code`) against committed `web/src/generated/wire.ts`". Put the test in `crates/envi-service/tests/wire_no_drift.rs` (it sees both `envi-store` and `envi-service` DTO sets).

**(b) Does metrao3's lifecycle / leak-lint pattern transfer to React? — PARTIAL, honestly.**
React's `useEffect` cleanup **already provides teardown pairing** for anything mounted inside an effect, so the `on`/`every`/`Disposers` helpers (`lifecycle.ts`) are **redundant for React-managed listeners** — do NOT port them wholesale. **BUT** the imperative subscriptions that ENVI holds in refs — `map.on("style.load", …)`, `draw.on("change"/"finish", …)`, `maplibregl.addProtocol(…)`, `new AttributionControl(…)`, EventSource for the job SSE — are **exactly the un-React-managed subscriptions** the leak discipline exists for; each MUST be torn down in its `useEffect` return (`map.off`, `draw.stop()`, `removeProtocol`, `es.close()`), and RESEARCH Pattern 3 already shows this (`return () => { drawRef.current?.stop(); … }`). So: the **discipline transfers** (paired subscribe/teardown for imperative map/draw/SSE handles), the **`lifecycle.ts` helper module does not** (useEffect subsumes it), and the **`leak-lint.cjs` scanner transfers only partially** — its heuristic keys on same-file `addEventListener`/`removeEventListener` and `setInterval`/`clearInterval`, which do NOT match React's `map.on`/`map.off` or effect-cleanup idiom, so a verbatim copy would be largely vacuous on a React tree. Recommendation for the planner: rely on `useEffect` cleanup + a code-review checklist that every `map.on`/`draw.on`/`addProtocol`/`new EventSource` in an effect has a matching teardown in the return; adopt a *retargeted* leak-lint only if it is extended to recognize the `.on(`/`.off(` and `EventSource`/`.close()` pairs (otherwise skip it — a vacuous gate is worse than none).

**(c) The `web/dist` git-ignore question:**
Today `web/dist/index.html` is **committed / tracked** (`git check-ignore web/dist` → exit 1, not ignored), and it is **load-bearing**: `envi-service` serves it via **`ServeDir::new(web_dist).fallback(ServeFile…)`** reading from disk at runtime (`api/mod.rs:75` — this is the ServeDir choice; ENVI does NOT use `include_dir!`/rust-embed, unlike metrao3 which embeds via rust-embed). The contract test `static_bundle_served_with_spa_fallback` (`contract_meta_static.rs:86`) and the `common::repo_web_dist()` helper both require `web/dist/index.html` to exist. **If `web/dist` were git-ignored now, `cargo test` (contract tests) and a fresh-checkout binary would have nothing to serve.** Given CLAUDE.md's operator-driven builds + no CI + self-hosted-offline posture, the **built `web/dist` is the shipped artifact and should stay committed (do NOT git-ignore it)**. What breaks if it disappears: the two static-bundle contract tests fail, and the deployed binary serves 404s for `/`. Two required follow-ups for the planner: (1) the real Vite bundle **replaces** the placeholder, so the contract test's `html.contains("frontend arrives in Phase 7")` assertion (`:96, :118`) must be updated to a stable marker in the real `index.html`; (2) add `.gitignore` entries for the NEW frontend noise — **`web/node_modules/`, `web/test-results/`, `web/playwright-report/`** (the root `.gitignore` currently has none of these) — while keeping `web/dist/` tracked. Alternative (heavier, not recommended for Phase 7): switch `envi-service` to `include_dir!`/rust-embed so the bundle is compiled in and `web/dist` can be ignored — but that changes the SVC-03 serving contract and the contract tests, so defer it.

---

## Metadata

**Analog search scope:** `crates/envi-geo/`, `crates/envi-store/`, `crates/envi-service/`, `crates/envi-engine/` (target types only), `crates/envi-harness/tests/`, `tools/nord2000_oracle/`, root `Cargo.toml`/`.gitignore`; metrao3 `crates/metrao3-web/ui/` (read-only).
**Files scanned:** ~28 (in-repo) + ~10 (metrao3, read-only).
**Pattern extraction date:** 2026-07-10
