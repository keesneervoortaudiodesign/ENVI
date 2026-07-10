# Phase 8: GIS Ingestion & DGM - Research

**Researched:** 2026-07-10
**Domain:** Browser-side (WASM) GIS ingestion — COG terrain windows, land-cover rasters, OSM buildings — onto a constrained-Delaunay DGM, with OPFS caching under the client-side-compute pivot
**Confidence:** HIGH on source endpoints/CORS (live-probed this session), HIGH on crate stack, MEDIUM on the σ-mapping values (research-owned, user reviews per CONTEXT)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Ingestion & compute model — the pivot (DATA-01..04; supersedes Phase-6 D-01/D-02)**
- **D-01:** **ENVI runs client-side in the browser as WebAssembly; calculations execute on the
  user's own device; a login/delivery server exists only to authenticate and serve the app bundle
  (and optionally proxy fetches).** "No installation; only runnable when logged in to the server."
  Consequence for this phase: **GIS ingestion is pure-Rust and WASM-targeted — GDAL/PROJ-C is
  ruled out entirely** (C libraries don't cross into WASM sanely). The whole Phase-6-deferred
  C-toolchain / Windows-provisioning question is therefore **moot and closed**. `envi-engine`
  (ndarray + num-complex) and `envi-dgm` (spade) already compile to `wasm32` cleanly; the new GIS
  ingestion crate (architecture's `envi-gis`) **inverts** from "everything C-linked" to "pure-Rust,
  browser-fetch + OPFS-cache, WASM-compatible."
- **D-02:** **CORS is the new gatekeeper (not `/vsicurl`).** The browser fetches COG windows /
  WorldCover / buildings / (later) weather directly. Strategy: **direct cross-origin fetch first,
  fall back to a login-server byte/range proxy** only for sources that block CORS. Maintain a
  **per-source CORS capability map**. A proxy relays bytes only (no compute), so it does not violate
  "calculations on the local machine"; it also gives a home for API-key injection / rate-limit
  smoothing (e.g. Overpass) if needed.
- **D-03:** **DATA-04 cache lives in OPFS (Origin Private File System), per project.** OPFS is the
  browser analog of the per-project disk cache: file-like, async, handles large binary blobs (DEM
  windows, tiles). The "compute reads only local cache, verified with the network off" guarantee
  means: after import, the WASM app reads terrain/landcover/buildings **exclusively from OPFS**;
  network access is touched **only at ingestion time**. (Replaces Phase-6 D-04's on-disk `cache/`
  folder for the client path.)

**Terrain source strategy (DATA-01, SC1)**
- **D-04:** **AHN4 DTM (Netherlands, via PDOK — CORS-friendly COG) is the preferred source in its
  coverage area; Copernicus GLO-30 is the global fallback.** Wired behind a **pluggable source
  registry** (coverage lookup → source selection) so more national DTMs slot in later as pure data.
- **D-05:** **GLO-30 is a surface model (DSM) — handle by flag + badge only ("check and complete").**
  Import GLO-30 as the TIN, badge it "surface model — may include buildings/vegetation," surface that
  in the terrain/impedance overlay, and let the user correct via editing. **No DSM→DTM flattening and
  no under-footprint sample exclusion this phase** (deferred). Pairs with D-10's footprint-boundary
  base-elevation sampling.

**The "Import" moment & failure handling (SC1)**
- **D-06:** **Import region = the current map viewport, with a max-area guardrail.** Client-side WASM
  must fetch every covering COG tile into browser memory — cost scales with area. Each layer
  (terrain / landcover / buildings) is **independently toggleable**.
- **D-07:** **Partial failure lands what succeeded; failed layers are retryable and non-blocking.**
  Per-layer status in the import UI; you can start modelling terrain while buildings retry.
- **D-08 (pivot reframe):** Import **progress is in-app client-side UI state, NOT the Phase-6
  server-side SSE job machine.** SC1's "live progress and clear failure states" is a client concern
  for the import path.

**"Check & complete" editability (SC1, SC4, SC5)**
- **D-09:** **Re-import MERGES — never clobbers user edits.** Adds only genuinely-new features,
  refreshes imported-but-untouched ones; anything the user moved, edited, or created is preserved.
  Requires **per-feature identity + a "user-modified" flag**.
- **D-10:** **Buildings from OSM via Overpass (primary source).** JSON over HTTP, CORS-friendly,
  trivial bbox query, clean in WASM. Locked fallback chain: measured → height tag → levels×3+1.5 →
  user default. Overpass rate limits handled via the login-server proxy (D-02) if needed. Overture
  GeoParquet is the documented future upgrade — deferred.
- **D-11 (settled by roadmap SC4/SC5):** every imported feature is an **ordinary editable scene
  object** carrying **per-feature provenance** (source + license + retrieval date); building base
  elevations sample **footprint-boundary ground, never DSM-under-building**; the map shows
  **attribution** for OSM/Overture/ESA WorldCover/Copernicus.

### Claude's Discretion
- The WorldCover→Nordtest σ/impedance **mapping table values** (SC3): researcher populates the σ per
  class from Nordtest, user reviews the table; the *mechanism* (reviewed data table + per-row unit
  test + impedance debug overlay) is fixed, the numbers are research-owned.
- Attribution UI presentation; import-panel layout and per-layer status affordances; the max-area
  guardrail threshold; OPFS keying/eviction scheme; the exact pure-Rust COG-window reader assembly
  (IFD parse → overview pick → tile-range math → range GETs → stitch) and the DEM→UTM resampler;
  the per-source CORS capability-map format; "user default" building-height value.

### Deferred Ideas (OUT OF SCOPE)
- **PROJECT-LEVEL AMENDMENT:** the pivot's ripples beyond Phase 8 (PROJECT.md deployment statement,
  Phase-6 persistence/job-machine revisit, Phase 10/11 tensor-store/contour moves, the auth system +
  full WASM build target with COOP/COEP) — a dedicated amendment pass **before Phase 10**, not here.
- DSM→DTM flattening / under-footprint sample exclusion (flag-only per D-05).
- Overture GeoParquet buildings (documented upgrade path).
- National DTM sources beyond AHN (registry built now; more countries = pure data later).
- `ground_zone`→`GroundSegment` cut-plane extraction (Phase 9), `calc_area`→receiver grid (Phase 10).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DATA-01 | Fetch terrain — Copernicus GLO-30 DEM (COG windowed reads) + national LiDAR DTM where available | Per-source capability map (§CORS), verified PDOK AHN4 + GLO-30 endpoints/tile schemes, whole-tile-then-window read strategy on the `tiff` crate (BigTIFF-capable), EPSG:28992 reprojection via existing `proj4rs` (sterea + towgs84 verified supported) |
| DATA-02 | Fetch ESA WorldCover land cover and map classes → Nordtest σ / impedance class (reviewed mapping table) | Verified WorldCover S3 tile scheme (proxy required — no CORS); full 11-class → Nord2000 A–H σ table proposed with per-row rationale (§WorldCover mapping); vectorization options for editable `ground_zone` features |
| DATA-03 | Fetch buildings (Overture GeoParquet / OSM) with a height-resolution fallback chain | Overpass verified CORS-open; query shape, tag parsing (`height`, `building:levels`), fallback-chain math + provenance fields, footprint-boundary base-elevation algorithm (§Buildings) |
| DATA-04 | Cache fetched tiles/data locally; the compute path reads only the local cache | OPFS layout + Rust/TS split (I/O in TS, math in WASM), whole-tile cache keying, network-off verification strategy in Playwright (§Patterns, §Validation) |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

- **Native Rust over FFI** — now absolute on the client path (wasm32 forbids C). No `gdal`, no `proj` C.
- **Engine 3-dep quarantine** — `envi-engine` stays byte-identical: `ndarray` + `num-complex` + `thiserror` only; nothing in this phase touches the engine. `cargo tree -p envi-engine` gate must keep passing.
- **105-point band-index framework** — not directly exercised here, but any impedance data must resolve through `envi_engine::scene::impedance_class` semantics (class letters A–H, **B = 31.5**), never re-derived tables.
- **Quality gates:** `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `cargo test` green; `#![deny(unsafe_code)]` on pure crates.
- **Playwright offline UAT** — E2E drives the real built bundle; `page.route(...)` must intercept **all** network: `/api/*`, basemap tiles/style/glyphs, **and every GIS source added this phase** (PDOK, S3 proxies, Overpass). Artifacts git-ignored; devDependency only.
- **Frontend:** TypeScript/React (TSX), wire types **generated** from Rust serde DTOs (ts-rs, committed, no-drift test) — any new import DTOs follow D-10 of Phase 7.
- **English-only output**; conventional commits with the `Co-Authored-By: Claude Opus 4.8` trailer; commit/push only when asked.
- **Licensing/data hygiene:** attribute Copernicus / ESA / OSM; **avoid FABDEM and Meteostat** (NC licenses); never commit the copyrighted Nord2000 documents; port GPL ideas, never GPL code.
- **Never kill VS Code / Java processes**; target only owned child PIDs.
- **Five mandatory GSD completion gates** at phase end (code-review → simplify → secure → verify → doc-consistency scan), all findings fixed or accepted.

## Summary

Phase 8 is the first phase built under the client-side-WASM pivot, and the research resolves its three genuinely open technical fronts. **First, CORS (D-02): live HTTP probes this session produced a verified per-source capability map.** PDOK AHN serves per-kaartblad BigTIFF COGs with `Access-Control-Allow-Origin: *` and working 206 range GETs — but its preflight rejects the `Range` header, so *browser* range reads are only safe where the browser skips the preflight (Chromium safelists simple `bytes=N-M` ranges; Firefox may not). Overpass is fully CORS-open. The Copernicus GLO-30 and ESA WorldCover S3 buckets honor range GETs but send **no CORS headers at all** — direct browser fetch is impossible; they must go through a byte-relay proxy on the existing `envi-service`. **Second, the read path:** because AHN tiles are small (≈3–30 MB compressed) and GLO-30/WorldCover tiles are 17/54 MB, the *simplest correct* design is **whole-tile ingest** — plain GETs (no Range header → no preflight anywhere), whole tiles cached in OPFS, and all windowing/decoding done locally by the pure-Rust `tiff` crate (BigTIFF + tiled + deflate decode verified). Range-windowed remote reads become a documented later optimization via the proxy. **Third, the WASM shape:** keep the new `envi-gis` crate **sans-I/O** — no network, no OPFS, no `web-sys` in the core. TypeScript owns fetch + OPFS (native browser APIs, trivially mocked in Playwright); Rust owns parsing, reprojection (proj4rs verifiably supports `sterea`+`towgs84`, so EPSG:28992 needs **zero new dependencies** in `envi-geo`), windowing, resampling, the σ table, and the height chain. This makes the whole ingestion core natively `cargo test`-able against committed GDAL-generated fixture COGs — the same committed-oracle pattern as `tools/nord2000_oracle/`.

The phase should build the **thinnest slice of the pivot** that satisfies DATA-01..04: one new pure crate (`envi-gis`) + one thin `cdylib` bindings crate, a wasm-bindgen build step wired into the existing Vite frontend, an allowlisted `/api/v1/proxy/*` byte relay on `envi-service`, and OPFS caching from TS. **No auth/login system this phase** — the existing localhost `envi-service` already *is* the delivery server (it serves `web/dist`); the login gate belongs to the pre-Phase-10 amendment pass. Scene persistence, the 9-kind vocabulary, the server-side `/dgm/triangulate` seam, and Phase-7 editing all stay exactly as they are — imported features are ordinary GeoJSON features flowing through the existing `PUT /scene`.

**Primary recommendation:** whole-tile ingest → OPFS → sans-I/O `envi-gis` WASM core for window/decimate/map/merge → existing scene + DGM seams; proxy only GLO-30 + WorldCover; no auth, no server-side ingestion, no C.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Source selection (coverage registry) | Browser WASM core (`envi-gis`) | — | Pure data lookup (bbox → source); must be testable natively |
| Network fetch of tiles / Overpass | Browser TS orchestrator | API proxy (`envi-service`) for CORS-blocked sources | `fetch` is a browser primitive; proxy relays bytes only (D-02) |
| CORS byte/range proxy | API / Backend (`envi-service`) | — | Only tier that can add CORS headers; allowlisted hosts, no compute |
| OPFS per-project cache (DATA-04) | Browser TS orchestrator | — | OPFS is a browser API; keep out of WASM (sync handles are worker-only) |
| COG parse / window decode / resample | Browser WASM core (`envi-gis`) | — | CPU-bound pure math; `tiff` crate; natively testable |
| WGS84 ↔ RD New ↔ project UTM | Browser WASM core via `envi-geo` | — | GEOX-04: the ONE reprojection boundary gains RD New; proj4rs is wasm-clean |
| WorldCover class → σ table + vectorize | Browser WASM core (`envi-gis`) | — | Reviewed data table + per-row test live next to the code that applies it |
| Overpass JSON → building features + height chain | Browser WASM core (`envi-gis`) | — | Deterministic mapping, provenance stamping; unit-testable |
| Import progress / per-layer status / guardrail UI | Browser React state | — | D-08: client UI state, NOT the Phase-6 SSE job machine |
| Scene persistence of imported features | API / Backend (existing `PUT /scene`) | — | Phase-6 contract unchanged; features are ordinary GeoJSON |
| DGM TIN build | API / Backend (existing `POST /dgm/triangulate` → `envi-dgm`) | Browser WASM (future, pre-Phase-10 amendment) | Thinnest slice: reuse the Phase-7 server seam; `envi-dgm` is already wasm-clean when the move comes |
| Impedance debug overlay rendering | Browser (MapLibre layer) | — | Client styling of imported `ground_zone` features / cached raster |
| Attribution display (SC5) | Browser (MapLibre AttributionControl) | — | Map-tier concern |

## Scope Recommendation — the thinnest pivot slice (planner must resolve)

The context defers the broad WASM/auth re-architecture to a pre-Phase-10 amendment pass. Research
conclusion on what Phase 8 itself must and must not build:

**IN (required by DATA-01..04 + locked decisions):**
1. `envi-gis` pure crate + `envi-gis-wasm` cdylib bindings; wasm32 build wired into the Vite build (`web/`).
2. TS import orchestrator: fetch (direct or via proxy) → OPFS write → WASM calls → feature commit via existing scene path.
3. `envi-service` gains **only** the allowlisted byte/range proxy routes (GLO-30, WorldCover; Overpass optional fallback). No other server change.
4. envi-geo gains EPSG:28992 (RD New) alongside UTM — still the single reprojection boundary.

**OUT (defer to the amendment pass — recommend explicitly recording in the plan):**
- Login/auth of any kind. The current localhost binary already serves the bundle; "only runnable when logged in" is not a DATA requirement.
- Moving scene persistence, DGM triangulation, or the job machine client-side. Phase 8 uses the Phase-6/7 server seams unchanged.
- WASM threads / COOP+COEP headers (needed only for Phase-10 grid solves).
- Compiling `envi-engine` to wasm (nothing in DATA-01..04 computes acoustics).

**Consequence to flag in the plan:** the project cache (OPFS) is per-browser-origin while the scene
is server-persisted — opening a project in a different browser finds scene but empty cache. Phase-8
UX: detect cache miss and offer re-import (deterministic; provenance carries the source). Record as
accepted asymmetry until the amendment pass.

## Standard Stack

### Core (Rust, workspace)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `tiff` (image-rs) | 0.11.3 | BigTIFF + tiled-TIFF decode (COG windows) | Pure Rust, BigTIFF since 0.6, tile-by-index since 0.7.3, deflate/LZW/packbits decode; 1.7M dl/wk `[VERIFIED: crates.io + changelog]` |
| `proj4rs` | 0.1.10 (already in `envi-geo`) | EPSG:28992 sterea + towgs84 → WGS84 → project UTM | Already the pinned CRS dep; `sterea`, `towgs84`, wasm32 all supported `[VERIFIED: projections.md + README]` — **zero new deps for RD New** |
| `spade` | 2.15 (already in `envi-dgm`) | DGM TIN (extended seam, not rewritten) | Phase-7 D-08 seam; pure Rust, wasm-clean `[VERIFIED: Cargo.toml read]` |
| `geojson`, `serde`/`serde_json` | workspace versions | Feature construction, Overpass JSON parse | Already in `envi-store`; serde quarantine applies only to `envi-engine` |
| `thiserror` | 2 | Typed errors, never panic on data | House pattern (envi-geo/envi-dgm) |
| `wasm-bindgen` + `js-sys` | 0.2.126 | WASM bindings crate only (`envi-gis-wasm`) | The standard; 8.3M dl/wk `[VERIFIED: crates.io]` |
| `serde-wasm-bindgen` | 0.6.5 | JsValue ⇄ serde DTO at the WASM boundary | Standard serde adapter `[VERIFIED: crates.io]` |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `geo` | 0.30+ | Polygon simplification (vectorized landcover), point-in-polygon, densify | georust staple, 282k dl/wk `[VERIFIED: legitimacy check]` |
| `contour` (contour-rs) | 0.13+ | Marching-squares isobands to vectorize WorldCover class masks | **[WARNING: flagged SUS — 800 dl/wk; real repo mthh/contour-rs since 2019, already the documented Phase-11 pick. Planner must gate install behind a checkpoint:human-verify.]** Alternative: hand-rolled marching squares (~200 lines) if the checkpoint declines |
| `wasm-bindgen-futures` | 0.4.x | Only if any async crosses the boundary | Prefer NOT to need it — sans-I/O core keeps the boundary synchronous |
| `flate2`/`weezl` | (transitive) | Deflate/LZW codecs | Pulled by `tiff`; do not depend directly |

### Frontend (npm — no new runtime deps)

`fetch`, OPFS (`navigator.storage.getDirectory()`), and `crypto.randomUUID()` are browser natives —
**no new npm runtime dependencies**. Build tooling: `wasm-bindgen-cli` (must match the
`wasm-bindgen` crate version **exactly** — see Pitfall 8) or `wasm-pack`. Playwright already present.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `tiff` + own window logic | `async-tiff` (developmentseed) | Purpose-built for COG, but async + `object_store`-coupled — wrong shape for a sans-I/O sync core fed from OPFS buffers `[ASSUMED]` |
| `tiff` + own window logic | `cloudtiff`, `georust/geotiff` | Younger/thinner; `georust/geotiff` is itself built on `tiff`; neither adds enough over direct `tiff` use `[ASSUMED]` |
| Whole-tile GET | HTTP Range windows from browser | Range is preflight-risky against PDOK (see Pitfall 2) and irrelevant behind the proxy; whole-tile is simpler and caches better — Range via proxy is the documented optimization |
| `contour` crate | hand-rolled marching squares | ~200 lines, well-specified; choose if the SUS checkpoint declines the dep |
| Overpass main instance | kumi.systems / other mirrors | Same API; keep endpoint configurable, main instance default |

**Installation:**
```bash
# workspace: new crates envi-gis (pure) + envi-gis-wasm (cdylib) join via crates/* glob
cargo add tiff --package envi-gis            # 0.11.3
cargo add geo geojson serde serde_json thiserror --package envi-gis
# bindings crate only:
cargo add wasm-bindgen js-sys serde-wasm-bindgen --package envi-gis-wasm
# build tooling (Wave 0):
cargo install wasm-bindgen-cli --locked --version <exact wasm-bindgen crate version>
```

**Version verification (performed this session):**
```bash
cargo search tiff          # -> tiff = "0.11.3"        [VERIFIED: crates.io]
cargo search wasm-bindgen  # -> wasm-bindgen = "0.2.126" [VERIFIED: crates.io]
cargo search proj4rs       # -> proj4rs = "0.1.10"     [VERIFIED: crates.io] (already pinned)
cargo search serde-wasm-bindgen # -> 0.6.5             [VERIFIED: crates.io]
```

## Package Legitimacy Audit

Run via `gsd-tools query package-legitimacy check --ecosystem crates` this session.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| tiff | crates | 2018 | 1.77M/wk | github.com/image-rs/image-tiff | OK | Approved |
| wasm-bindgen | crates | 2018 | 8.3M/wk | github.com/wasm-bindgen/wasm-bindgen | OK | Approved |
| web-sys | crates | 2018 | 5.6M/wk | (wasm-bindgen monorepo) | OK | Approved (avoid in core; TS owns browser APIs) |
| js-sys | crates | 2018 | 7.8M/wk | (wasm-bindgen monorepo) | OK | Approved |
| wasm-bindgen-futures | crates | 2018 | 4.9M/wk | (wasm-bindgen monorepo) | OK | Approved (only if needed) |
| proj4rs | crates | 2023 | 25k/wk | github.com/3liz/proj4rs | OK | Approved (already in workspace) |
| serde-wasm-bindgen | crates | — | — | cloudflare/serde-wasm-bindgen | OK | Approved |
| geo | crates | 2015 | 282k/wk | github.com/georust/geo | OK | Approved |
| geo-types | crates | 2018 | 344k/wk | github.com/georust/geo | OK | Approved |
| contour | crates | 2019 | 800/wk | github.com/mthh/contour-rs | **SUS** | Flagged — planner inserts `checkpoint:human-verify` before install; hand-rolled marching squares is the fallback |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** `contour` (low download count only; repo is real and it is already ARCHITECTURE.md's Phase-11 pick — the checkpoint is a formality but mandatory)

## Per-Source CORS Capability Map (D-02 deliverable — live-probed 2026-07-10)

All probes performed with `curl` sending `Origin:` (and preflight `OPTIONS`) this session.

| Source | Endpoint | ACAO on GET | Range GET | Preflight allows `Range`? | Verdict | License |
|--------|----------|-------------|-----------|---------------------------|---------|---------|
| **PDOK AHN4 DTM 0.5 m** | `service.pdok.nl/rws/actueel-hoogtebestand-nederland/atom/downloads/dtm_05m/M_*.tif` | `*` ✅ | 206 ✅ | **NO** — OPTIONS→403, `Allow-Headers: Content-Type` only | **DIRECT, whole-tile GET** (no Range header → no preflight). Range GETs work on Chromium only (safelisted simple ranges) — do not rely on them | **CC0** (rights URL in ATOM feed) `[VERIFIED: live probe + ATOM feed]` |
| **Copernicus GLO-30** | `copernicus-dem-30m.s3.amazonaws.com/Copernicus_DSM_COG_10_<N/S>lat_00_<E/W>lon_00_DEM/<same>.tif` | **absent** ❌ | 206 ✅ | n/a | **PROXY REQUIRED** | Free with mandatory credit line `[VERIFIED: live probe; license CITED: registry.opendata.aws/copernicus-dem]` |
| **ESA WorldCover 2021 v200** | `esa-worldcover.s3.eu-central-1.amazonaws.com/v200/2021/map/ESA_WorldCover_10m_2021_v200_<N/S>lat<E/W>lon_Map.tif` (3°×3° tiles) | **absent** ❌ (OPTIONS→403) | 206 ✅ | n/a | **PROXY REQUIRED** | CC-BY 4.0 `[VERIFIED: live probe; license CITED: esa-worldcover.org/en/data-access]` |
| **OSM Overpass** | `overpass-api.de/api/interpreter` | `*` ✅ (+ `Access-Control-Max-Age: 600`) | n/a (POST/GET JSON) | n/a | **DIRECT** | ODbL (attribution required) `[VERIFIED: live probe]` |
| **OpenFreeMap basemap** (Phase 7) | runtime tiles | works today | n/a | n/a | unchanged | MIT style / OSM data |

Key facts behind the verdicts:
- The GLO-30 example tile `Copernicus_DSM_COG_10_N52_00_E004_00_DEM.tif` is 17,037,271 bytes; 1° tiles; float32, DEFLATE + predictor 3, 1024-px internal tiles, average-resampled overviews `[VERIFIED: live probe + CITED: copernicus-dem-30m readme.html]`.
- The AHN DTM tile `M_01GN2.tif` begins `49 49 2B 00` — **BigTIFF** — and is 3,211,812 bytes for a full 5×6.25 km kaartblad `[VERIFIED: live probe]`. The ATOM feed `dtm_05m.xml` carries a `georss:polygon` per tile — the tile index for the source registry can be generated from it at dev time and committed.
- `Range` is CORS-safelisted for **simple single byte ranges** (`bytes=N-M`) in Chromium (shipped) and WebKit trunk; Firefox historically preflights (Bugzilla 1733981 / 1762794) `[CITED: github.com/whatwg/fetch/issues/1310, MDN CORS-safelisted request header]`. Since PDOK's preflight rejects `Range`, browser range reads against PDOK are Chromium-only — hence the whole-tile recommendation.
- Proxy consequence: behind the same-origin proxy there is no CORS at all, so range windows through the proxy are always safe; that is where the range-read optimization lives if ever needed.

## Architecture Patterns

### System Architecture Diagram

```
                 ┌────────────────────────── BROWSER ──────────────────────────┐
                 │                                                              │
 Viewport bbox   │  React import panel (per-layer toggles, progress, retry,    │
 (WGS84)  ──────▶│  guardrail warning)          [D-06/D-07/D-08: client state] │
                 │        │                                                    │
                 │        ▼                                                    │
                 │  TS import orchestrator (web/src/import/)                   │
                 │   1. registry.plan(bbox)  ──────────────┐                   │
                 │   2. for each needed tile:              │ (WASM call)       │
                 │        cache hit? ──read OPFS──┐        ▼                   │
                 │        miss:  fetch(direct)    │   ┌─────────────────────┐  │
                 │           or  fetch(/api/proxy)│   │ envi-gis-wasm       │  │
                 │        write OPFS ─────────────┤   │ (cdylib, thin)      │  │
                 │   3. tile bytes ───────────────┴──▶│  envi-gis (pure):   │  │
                 │   4. features/grid ◀───────────────│  COG parse+window   │  │
                 │        │                           │  resample/decimate  │  │
                 │        │                           │  WorldCover→σ table │  │
                 │        │                           │  Overpass→buildings │  │
                 │        │                           │  height chain,      │  │
                 │        │                           │  merge (D-09)       │  │
                 │        │                           │  via envi-geo CRS   │  │
                 │        ▼                           └─────────────────────┘  │
                 │  scene store (Phase-7) ── commitFeature (provenance props)  │
                 └────────┼──────────────────────────────┼────────────────────┘
                          │ PUT /scene (existing)        │ POST /dgm/triangulate (existing)
                          ▼                              ▼
                 ┌──────────────────── envi-service (axum, unchanged core) ────┐
                 │  + NEW: /api/v1/proxy/{glo30|worldcover}/{path}             │
                 │    allowlisted host+prefix, GET+Range relay, size cap,      │
                 │    timeout — bytes only, no compute            [D-02]       │
                 └───────────┬─────────────────────────────────────────────────┘
                             │ (server-side fetch, no CORS constraints)
              ┌──────────────┼──────────────────┐
              ▼              ▼                  ▼
   PDOK AHN COGs      GLO-30 S3 (no CORS)  WorldCover S3 (no CORS)      Overpass API
   (direct from       (proxied)            (proxied)                    (direct from
    browser, CC0)                                                        browser, ODbL)
```

Trace of the primary use case (SC1): viewport bbox → registry picks AHN4 (NL) or GLO-30 → covering
tiles fetched (direct/proxy) → whole tiles land in OPFS (`DATA-04`: only ingestion touches the
network) → WASM decodes a window from cached bytes, decimates to elevation samples, maps landcover
classes, builds building features with heights + provenance → TS commits features through the
existing Phase-7 scene path → server `envi-dgm` re-triangulates the DGM → user edits everything.

### Recommended Project Structure

```
crates/
├── envi-gis/                    # NEW — pure Rust, sans-I/O, #![deny(unsafe_code)]
│   └── src/
│       ├── lib.rs               # crate doc: I/O-free contract, wasm32 target note
│       ├── registry.rs          # source registry: coverage polygons → SourceId (D-04)
│       ├── cog/
│       │   ├── header.rs        # BigTIFF/TIFF IFD parse (via tiff crate Decoder over &[u8])
│       │   ├── window.rs        # bbox → overview pick → tile indices → stitch → Raster<f32>
│       │   └── geo_tags.rs      # ModelPixelScale/Tiepoint → geotransform; nodata tag
│       ├── terrain.rs           # window → decimated elevation grid (guardrail-aware)
│       ├── landcover.rs         # WorldCover class raster → σ class + vectorize to zones (SC3)
│       ├── impedance_table.rs   # THE reviewed WorldCover→Nordtest table + per-row test
│       ├── buildings.rs         # Overpass JSON → footprints, height chain, base elevation (SC4)
│       ├── merge.rs             # re-import merge: per-feature identity + user_modified (D-09)
│       └── provenance.rs        # source/license/retrieved_at property stamping (D-11)
├── envi-gis-wasm/               # NEW — cdylib; wasm-bindgen boundary ONLY (no logic)
│   └── src/lib.rs               # plan_import(), decode_window(), map_landcover(), ...
web/
├── src/import/                  # NEW — TS orchestrator
│   ├── opfs.ts                  # per-project cache: projects/<uuid>/cache/<source>/<tile>
│   ├── fetchers.ts              # direct vs proxy routing per capability map
│   ├── importJob.ts             # per-layer state machine (D-07/D-08), guardrail
│   └── attribution.ts           # SC5 strings → MapLibre AttributionControl
crates/envi-service/src/api/proxy.rs   # NEW — allowlisted byte relay
crates/envi-geo/src/crs.rs             # MODIFIED — + RD New (EPSG:28992) named CRS
tools/gis_oracle/                # NEW — gen_cog_fixtures.py (GDAL/rasterio at dev time),
                                 #       committed tiny COGs + expected-window TOMLs
```

### Pattern 1: Sans-I/O core ("plan / fulfill")
**What:** `envi-gis` never fetches, never touches OPFS, never imports `web-sys`. Its API is
synchronous over byte slices: `plan_import(bbox) -> TilesNeeded`, then
`decode_window(&tile_bytes, bbox, budget) -> Raster`. TS performs all I/O between the two calls.
**When to use:** everywhere in this phase.
**Why:** (a) the OPFS sync-handle worker constraint and wasm async landmines vanish; (b) the whole
core runs under native `cargo test` against committed fixtures — the same testing honesty as the
engine; (c) Playwright mocks plain `fetch` calls, nothing exotic.
```rust
// envi-gis/src/cog/window.rs — shape, not final signature
pub fn decode_window(
    tile_file: &[u8],            // whole cached COG (from OPFS via TS)
    window: BboxRd,              // source-CRS window
    max_decoded_px: usize,       // memory guardrail (D-06)
) -> Result<Raster<f32>, GisError>;
```

### Pattern 2: Whole-tile ingest, local windowing
**What:** ingestion fetches **entire source tiles** with plain GETs (no `Range` header), caches them
in OPFS, and every window/overview read afterwards is local.
**Why:** kills the PDOK Range-preflight landmine, makes DATA-04's "network only at ingestion" literal
(one fetch per tile ever), and tile sizes are small (AHN ≈3–30 MB, GLO-30 17 MB, WorldCover 54 MB).
**Upgrade path (documented, not built):** range-windowed reads through the proxy for very large
sources.

### Pattern 3: Source registry as data (D-04)
**What:** `SourceDescriptor { id, kind: Dtm|Dsm|Landcover|Buildings, coverage: MultiPolygon,
crs, tile_scheme, endpoint_template, cors: Direct|Proxy, license, attribution }` — a static table.
AHN4 coverage = NL boundary polygon (from the ATOM feed's `georss` bbox); GLO-30 coverage = global.
AHN kaartblad index (tile name ↔ RD bbox) is **generated at dev time from the ATOM feed and
committed** (same committed-artifact pattern as oracle fixtures) — no runtime dependency on the feed.
**Why:** "more national DTMs slot in later as pure data" is only true if the registry is data.

### Pattern 4: Provenance + merge identity as plain GeoJSON properties (D-09/D-11)
**What:** every imported feature carries
`properties: { id: <uuid>, kind, source: "ahn4-dtm"|"glo30"|"worldcover"|"osm-overpass",
source_ref: "<tile or osm way id>", license: "CC0"|"CC-BY-4.0"|"ODbL", retrieved_at: ISO8601,
imported: true, user_modified: false, height_provenance?: "height_tag"|"levels"|"default", ... }`.
The Phase-6 store already **preserves unknown additional properties** (verified in
`geojson.rs`) — no store schema change is needed for provenance. Re-import merge rule: match on
`(source, source_ref)`; if `user_modified` → keep user's version; if absent → add; if present and
untouched → refresh geometry/props. Phase-7's `commitFeature` path sets `user_modified: true` on any
edit — one flag, one place.
**Anti-pattern avoided:** a parallel "import ledger" store that can drift from the scene.

### Pattern 5: Allowlisted byte relay (D-02)
**What:** `GET /api/v1/proxy/{source}/{*path}` where `{source}` selects a hardcoded
`(scheme, host, path_prefix)`; the handler forwards `GET` (+ optional `Range`), streams the body,
enforces a response size cap and timeout, and never accepts a full user-supplied URL.
**Why:** SSRF-proof by construction; bytes-only preserves "compute on the local machine".

### Anti-Patterns to Avoid
- **`web-sys`/network/OPFS calls inside `envi-gis`:** re-creates the async/worker landmines and makes the core untestable natively. TS owns I/O.
- **A second reprojection site:** RD New goes into `envi-geo` (GEOX-04's single boundary), never inline proj strings in `envi-gis`.
- **Hand-written TS mirrors of import DTOs:** Phase-7 D-10 applies — new wire/boundary types are ts-rs-generated and committed with the no-drift test. This includes the WASM boundary types (generate from the same Rust DTOs).
- **Storing decoded rasters in OPFS:** cache the *compressed source tiles* (provenance-pure, small); decode on demand. A decoded-raster cache is a second source of truth with an invalidation problem.
- **Trusting `nominal` tile geometry:** always read width/height/geotransform from the IFD (GLO-30 tiles above 50°N are *not* 3600 px wide — see Pitfall 5).
- **Blocking import on one failed layer:** D-07 — per-layer independent state machines.

## WorldCover → Nordtest σ / Impedance Mapping (SC3 — research-owned values, user reviews)

Nord2000's eight impedance classes with nominal flow resistivity σ (kPa·s/m²), as pinned in the
engine and confirmed against the Nord2000 Road User's Guide `[VERIFIED: forcetechnology.com
Nord2000 Road User's Guide table + engine impedance_class]`:
A = 12.5 (snow/moss-like) · B = 31.5 (soft forest floor) · C = 80 (uncompacted loose ground: turf,
grass, loose soil) · D = 200 (normal uncompacted: forest floors, pasture) · E = 500 (compacted field,
park lawns, gravel) · F = 2000 (compacted dense: gravel road, parking) · G = 20000 (asphalt) ·
H = 200000 (dense asphalt, concrete, **water**).

ESA WorldCover v200 class values `[CITED: WorldCover_PUM_V2.0.pdf, esa-worldcover.s3.eu-central-1.amazonaws.com/v200/2021/docs/]`:

| WC code | WorldCover class | → Nord2000 class | σ (kPa·s/m²) | Rationale | Row confidence |
|---------|------------------|------------------|--------------|-----------|----------------|
| 10 | Tree cover | **B** | 31.5 | "Soft forest floor" is B's literal descriptor | MEDIUM |
| 20 | Shrubland | **C** | 80 | Uncompacted loose vegetated ground | MEDIUM |
| 30 | Grassland | **D** | 200 | WorldCover grassland ⊇ pasture — D's "pasture field"; C (80, "turf/grass") is the softer defensible alternative — **user decides** | LOW–MEDIUM |
| 40 | Cropland | **D** | 200 | Tilled/normal uncompacted ground | MEDIUM |
| 50 | Built-up | **G** | 20000 | Predominantly sealed surfaces; conservative for noise (hard = louder) | MEDIUM |
| 60 | Bare / sparse vegetation | **E** | 500 | Compacted bare field/gravel character | LOW |
| 70 | Snow and ice | **A** | 12.5 | A's literal descriptor (snow) | HIGH |
| 80 | Permanent water bodies | **H** | 200000 | Water is acoustically hard — H's descriptor names water | HIGH |
| 90 | Herbaceous wetland | **B** | 31.5 | Saturated soft vegetated ground; between A and C | LOW |
| 95 | Mangroves | **B** | 31.5 | Wet forest floor analog | LOW |
| 100 | Moss and lichen | **A** | 12.5 | A's literal descriptor (moss-like) | HIGH |

Mechanism (locked by CONTEXT): this table lives in `envi-gis::impedance_table` as a const array; a
unit test asserts **every row** maps to a valid engine class letter and the exact σ the engine pins
for that letter (import the letters, never re-state σ in two places — resolve σ through
`envi_engine::scene::impedance_class` in the test); the impedance **debug overlay** styles imported
`ground_zone` features (plus a "no data → project default" wash) by class letter. Roughness: imported
zones default to class **N** (roughness is not derivable from WorldCover) — surface in the table
review. `[ASSUMED: per-row assignments — the review IS the mechanism for firming these]`

**Vectorization for editability:** per-class binary mask over the imported window → marching-squares
isobands (`contour` crate, gated, or hand-rolled) → `geo` simplification (Douglas-Peucker, ~1–2 px
tolerance) → drop polygons under a min-area threshold (discretion) → `ground_zone` features with
class + provenance. Adjacent same-boundary polygons don't violate Phase-7's crossing rejection
(adjacency/containment are legal; crossings are not produced by marching squares on a partition).

## Buildings: heights, base elevation, Overpass (SC4)

**Overpass query** (single request per import, direct from browser — CORS verified):
```
[out:json][timeout:25];
( way["building"]({south},{west},{north},{east});
  relation["building"]["type"="multipolygon"]({south},{west},{north},{east}); );
out body geom;
```
`out geom` inlines coordinates (no second lookup). Handle multipolygon relations (outer/inner rings).
Rate limiting: per-IP slots, HTTP 429 on denial, `/api/status` shows slot state
`[CITED: dev.overpass-api.de/overpass-doc/en/preface/commons.html]`. One import = one query; retry
with backoff on 429; the proxy fallback exists if limits ever bite (D-02).

**Height fallback chain (locked order, D-10) — implementation facts:**
1. **measured** — no measured-height source this phase (3DBAG/AHN-derived heights are the deferred upgrade); branch exists but is empty. Record `height_provenance: "measured"` reserved.
2. **`height` tag** — parse meters; tolerate `"12"`, `"12 m"`, `"12.5m"`; reject non-finite/negative; `building:height` as synonym. Coverage is sparse: <5% of buildings globally carry `height` `[ASSUMED: literature figure — nature.com OSM completeness studies]`.
3. **`building:levels` × 3 + 1.5** — the locked formula (≈3 m/storey + roof allowance; OSM convention is ~3 m/level `[CITED: wiki.openstreetmap.org/wiki/Key:building:levels]`). Parse decimal levels; ignore `roof:levels` this phase.
4. **user default** — import-panel setting (discretion; suggest 6 m ≈ 2 storeys NL rowhouse). `height_provenance: "default"`.

Every building gets `height_m`, `height_provenance`, plus D-11 provenance props. The Phase-6
`building_from_feature` expects `eaves_height_m` — imported buildings must emit the properties the
existing 9-kind schema already consumes (verify exact property names against `geojson.rs` at
planning; do not invent a parallel height property).

**Base elevation (locked: footprint-boundary ground, never DSM-under-building):** densify the
exterior ring to ≤ N m vertex spacing (suggest 2–5 m), sample the terrain source at each boundary
point, take the **median** (robust against DSM spikes from adjacent trees/vehicles; min is the
conservative alternative — discretion). Store as `base_elevation_m` + `base_elevation_source`.
Works identically for AHN DTM (where under-footprint interpolation would also be fine) and GLO-30
DSM (where under-footprint reads the roof — the rule exists for exactly this case).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| TIFF/BigTIFF decode (IFDs, codecs, predictors) | own TIFF parser | `tiff` crate | BigTIFF offsets, deflate + float predictor-3, LZW quirks — years of edge cases; the crate decodes tiles by index `[VERIFIED: changelog]` |
| RD New ↔ WGS84 | own sterea math | `proj4rs` (already pinned) | sterea + Bessel + towgs84 Helmert verified supported; hand math would silently disagree with pyproj oracle |
| Delaunay TIN | any new triangulator | existing `envi-dgm::build_tin` | Phase-7 seam, panic-guarded, DoS-capped — extend by feeding vertices |
| Marching squares (if dep declined) | ad-hoc region tracing | `contour` crate (gated) | isoband topology (saddles, holes) is fiddly; hand-roll only as the documented ~200-line fallback with tests |
| Polygon simplification | own Douglas-Peucker | `geo::Simplify` | Established, tested |
| UUIDs in WASM | `uuid` + getrandom wiring | `crypto.randomUUID()` in TS | Avoids the wasm getrandom feature dance entirely (Pitfall 9) |
| CORS/security of fetches | permissive proxy | allowlisted relay (Pattern 5) | SSRF-proof by construction |

**Key insight:** in this domain the expensive mistakes are *format* edge cases (BigTIFF, predictors,
tile padding at raster edges, nodata) and *web-platform* edge cases (preflights, worker-only APIs).
Both are avoided by buying the format work (`tiff`, `proj4rs`) and by moving the web-platform work
into TS where it is native.

## Common Pitfalls

### Pitfall 1: AHN COGs are BigTIFF
**What goes wrong:** a reader assuming classic TIFF magic (`II*\0`) fails instantly — the probed AHN tile begins `II+\0` (BigTIFF, 64-bit offsets) `[VERIFIED: live byte probe]`.
**How to avoid:** `tiff` crate handles BigTIFF since 0.6.0; add a fixture that IS BigTIFF so the test suite can't pass without it.
**Warning signs:** "invalid TIFF header" only on AHN tiles, not on GLO-30.

### Pitfall 2: The `Range` preflight trap
**What goes wrong:** browser `fetch` with a JS-set `Range` header triggers a CORS preflight unless the value is a simple single range on Chromium; PDOK's `OPTIONS` returns 403 and its `Access-Control-Allow-Headers` omits `Range` `[VERIFIED: live probe]` — so a "COG windowed read" straight from the browser works in dev (Chromium) and fails for Firefox users.
**How to avoid:** whole-tile plain GETs (Pattern 2) — no Range header exists to preflight. Range reads only through the same-origin proxy.
**Warning signs:** import works in Playwright/Chromium, fails in Firefox with an opaque CORS error.

### Pitfall 3: Cross-origin response *header* visibility
**What goes wrong:** even when ACAO passes, JS cannot read `Content-Range`/`Content-Length` response headers unless `Access-Control-Expose-Headers` lists them — total-size probes silently return null.
**How to avoid:** whole-tile GETs don't need them; the committed tile index carries sizes if needed.

### Pitfall 4: Decoded-window memory blow-up
**What goes wrong:** 0.5 m AHN over a 10×10 km viewport = 4×10⁸ px ≈ 1.6 GB f32 — a browser-tab kill. The 3.2 MB compressed tile is deceptive (flat NL terrain compresses ~150:1).
**How to avoid:** D-06 guardrail computed from *decoded* pixel count, not area alone: `max_decoded_px` budget in `decode_window` (suggest ≤ 64–128 M px ≈ 256–512 MB) and pick a COG overview level for the display TIN when over budget; full-res stays in cache for Phase 9's cut profiles.
**Warning signs:** import fine on a street, tab crash on a municipality.

### Pitfall 5: GLO-30 tiles are not square above 50°N
**What goes wrong:** GLO-30 reduces longitudinal resolution with latitude (1× at 0–50°, coarser bands above) `[CITED: copernicus-dem-30m readme.html]` — NL (50.7–53.5°N) tiles have fewer columns than 3600; hardcoded grid math misplaces every sample.
**How to avoid:** always derive geotransform from `ModelPixelScale`/`ModelTiepoint` IFD tags; never assume pixel counts.

### Pitfall 6: Vertical datum mismatch (NAP vs EGM2008)
**What goes wrong:** AHN heights are NAP; GLO-30 heights are EGM2008 geoid `[CITED: Copernicus DEM Product Handbook i5.0]`. Mixing sources in one project (or re-importing the same area from the other source) steps all elevations by decimeters.
**How to avoid:** Phase 8 rule: **one terrain source per import region**, record `vertical_datum` in provenance, and badge mixed-datum projects. No datum transformation this phase (relative geometry dominates propagation; flag, don't fix — consistent with D-05).

### Pitfall 7: Nodata and edge tiles
**What goes wrong:** AHN kaartblad edges and water bodies carry nodata (float sentinel, `GDAL_NODATA` ASCII tag); feeding sentinels into the TIN creates 3.4e38-meter mountains; COG edge tiles are padded to full tile size with garbage beyond the image bounds.
**How to avoid:** parse the nodata tag, drop nodata samples before decimation (the TIN interpolates holes), and clamp tile reads to `ImageWidth/Length`.
**Warning signs:** absurd Z range in the DGM summary; spikes at coastlines.

### Pitfall 8: `wasm-bindgen` crate ↔ CLI version lockstep
**What goes wrong:** `wasm-bindgen-cli` version must **exactly** match the `wasm-bindgen` crate version or the build fails with a versioned schema error — classic CI/dev drift.
**How to avoid:** pin both; install the CLI with `--locked --version` in a Wave-0 task; document in web/README.

### Pitfall 9: `getrandom`/uuid on wasm32
**What goes wrong:** any transitive `getrandom` without the `wasm_js` backend fails to compile or panics on wasm32-unknown-unknown.
**How to avoid:** generate feature UUIDs in TS (`crypto.randomUUID()`); keep randomness out of the Rust core entirely; if a dep pulls getrandom, enable its js/wasm feature explicitly.

### Pitfall 10: OPFS API split (sync = worker-only)
**What goes wrong:** `FileSystemSyncAccessHandle` (fast sync reads) is **only available in dedicated workers** `[CITED: MDN createSyncAccessHandle]`; calling it on the main thread throws.
**How to avoid:** use the async main-thread API (`getDirectory()` → `getFileHandle` → `createWritable`/`getFile().arrayBuffer()`) from TS — ingestion is not latency-critical. Workers + sync handles are a Phase-10 concern if ever.

### Pitfall 11: Playwright must now mock four more origins
**What goes wrong:** the CLAUDE.md offline rule — any test that lets a PDOK/S3/Overpass request escape is a failed test, and `page.route` patterns scoped to `/api/*` won't catch them.
**How to avoid:** a shared `mockGisSources(page)` helper routing `**service.pdok.nl/**`, `**/api/v1/proxy/**`, `**overpass-api.de/**` (plus the existing basemap mocks) onto committed fixture files; serve 206 slices from the fixture buffer when a Range header is present (needed once range-via-proxy exists). Add a catch-all `page.route('**', abort)` guard after the allowlist to prove nothing escapes.

### Pitfall 12: Overpass geometry quirks
**What goes wrong:** unclosed ways, self-intersecting footprints, multipolygon relations with role-less members — imported as-is they violate scene validation or produce degenerate buildings.
**How to avoid:** validate rings (close, ≥4 positions, finite), skip-and-report invalid features in the per-layer status (D-07 honesty) rather than failing the layer.

## Code Examples

### RD New in `envi-geo` (proj4rs — no new deps)
```rust
// envi-geo/src/crs.rs — EPSG:28992 proj string (towgs84 7-param, ~0.5 m vs RDNAPTRANS)
// Source: epsg.io/28992 + proj4rs README towgs84 example [CITED]
const RD_NEW: &str = "+proj=sterea +lat_0=52.15616055555555 +lon_0=5.38763888888889 \
 +k=0.9999079 +x_0=155000 +y_0=463000 +ellps=bessel \
 +towgs84=565.417,50.3319,465.552,-0.398957,0.343988,-1.8774,4.0725 +units=m +no_defs";
// proj4rs sharp edge (already quarantined in transform.rs): longlat is RADIANS.
// Validate against a pyproj oracle fixture (existing pattern), tolerance <= 1.0 m.
```

### Windowed decode from a cached whole tile (`tiff` crate)
```rust
// envi-gis/src/cog/window.rs — Source: docs.rs/tiff decoder API [CITED]
use std::io::Cursor;
use tiff::decoder::{Decoder, DecodingResult};

pub fn read_tile_f32(tile_file: &[u8], chunk_index: u32) -> Result<Vec<f32>, GisError> {
    let mut dec = Decoder::new(Cursor::new(tile_file))?;   // handles TIFF + BigTIFF
    // navigate IFD chain for overview selection: dec.more_images() / dec.next_image()
    match dec.read_chunk(chunk_index)? {                    // one internal COG tile
        DecodingResult::F32(v) => Ok(v),
        other => Err(GisError::UnexpectedSampleFormat { got: format!("{other:?}") }),
    }
}
// Window = union of covering chunks, cropped by ImageWidth/Length, nodata dropped.
```

### OPFS cache write/read (TS orchestrator — main thread, async API)
```ts
// web/src/import/opfs.ts — Source: MDN File System API [CITED]
async function cacheDir(projectId: string, source: string): Promise<FileSystemDirectoryHandle> {
  let dir = await navigator.storage.getDirectory();
  for (const seg of ["projects", projectId, "cache", source]) {
    dir = await dir.getDirectoryHandle(seg, { create: true });
  }
  return dir;
}
export async function putTile(projectId: string, source: string, name: string, bytes: ArrayBuffer) {
  const fh = await (await cacheDir(projectId, source)).getFileHandle(name, { create: true });
  const w = await fh.createWritable();          // async API: main-thread safe
  await w.write(bytes); await w.close();
}
export async function getTile(projectId: string, source: string, name: string) {
  try {
    const fh = await (await cacheDir(projectId, source)).getFileHandle(name);
    return await (await fh.getFile()).arrayBuffer();
  } catch { return null; }                       // cache miss -> fetch path
}
```

### Playwright: offline GIS mocking incl. 206 slices
```ts
// web/e2e/helpers/mockGis.ts
import { readFileSync } from "node:fs";
export async function mockGisSources(page: Page) {
  const ahn = readFileSync("e2e/fixtures/ahn_dtm_fixture.tif");
  await page.route("**service.pdok.nl/**/dtm_05m/*.tif", async (route) => {
    const range = route.request().headers()["range"];       // e.g. "bytes=0-1023"
    if (range) {
      const [, s, e] = range.match(/bytes=(\d+)-(\d+)?/)!;
      const start = +s, end = e ? +e : ahn.length - 1;
      return route.fulfill({ status: 206, contentType: "image/tiff",
        headers: { "content-range": `bytes ${start}-${end}/${ahn.length}` },
        body: ahn.subarray(start, end + 1) });
    }
    return route.fulfill({ status: 200, contentType: "image/tiff", body: ahn });
  });
  await page.route("**overpass-api.de/api/interpreter*", (r) =>
    r.fulfill({ json: JSON.parse(readFileSync("e2e/fixtures/overpass_buildings.json", "utf8")) }));
  // proxy-backed sources are same-origin: mock /api/v1/proxy/** the same way
}
```

### Proxy relay (axum, allowlisted)
```rust
// envi-service/src/api/proxy.rs — bytes only, no compute (D-02)
const SOURCES: &[(&str, &str, &str)] = &[
    ("glo30",      "copernicus-dem-30m.s3.amazonaws.com",           "/Copernicus_DSM_COG_"),
    ("worldcover", "esa-worldcover.s3.eu-central-1.amazonaws.com",  "/v200/2021/map/"),
];
// GET /api/v1/proxy/{source}/{*path}: reject unknown source; require path.starts_with(prefix);
// forward GET (+ Range verbatim); stream body; cap at e.g. 128 MiB; 15 s connect timeout.
// NOTE: envi-service needs an HTTP client dep for this (reqwest with rustls, or hyper client) —
// server-side only, never in the wasm path. Planner picks; keep TLS pure-Rust (rustls).
```

## State of the Art

| Old Approach (ARCHITECTURE.md, pre-pivot) | Current Approach (this phase) | When Changed | Impact |
|--------------------------|-------------------------------|--------------|--------|
| `envi-gis` = "everything C-linked": gdal/proj/`/vsicurl` server-side | Pure-Rust sans-I/O core + TS fetch/OPFS in browser | 08-CONTEXT pivot (2026-07-10) | Zero C toolchain ever; Phase-6 "GDAL provisioning in Phase 8" closed as moot |
| On-disk `projects/<id>/cache/` server folder | OPFS per project in browser | pivot D-03 | Network-off guarantee becomes a Playwright-verifiable client property |
| Import as server job + SSE progress | Client-side per-layer state (D-08) | pivot | Phase-6 job machine untouched, unused by import |
| Overture GeoParquet primary for buildings | OSM Overpass primary; Overture deferred | 08-CONTEXT D-10 | No parquet/arrow stack in the browser this phase |
| `GDALContourGenerateEx` for isobands (Phase 11) | pure-Rust `contour` crate (also reusable here for landcover vectorization) | pivot deferred list | One vectorization dep serves Phases 8 and 11 |

**Deprecated/outdated notes:** AHN4 is the current complete national release; AHN5 acquisition is
underway but PDOK's 0.5 m raster ATOM service (probed) states "Het huidige AHN is versie 4" —
AHN4 remains correct for D-04 `[VERIFIED: ATOM feed subtitle]`. `geotiff` 0.1-era immaturity noted in
OPEN-GIS-LANDSCAPE still holds; the `tiff` crate is the load-bearing decoder.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `wasm32-unknown-unknown` target | WASM build | ✓ | installed | — |
| rustc / cargo | workspace | ✓ | 1.96.0 | — |
| node / npm | web build | ✓ | 24.15.0 / 11.12.1 | — |
| Playwright (@playwright/test, Chromium) | E2E | ✓ | ^1.61.1 | — |
| `wasm-bindgen-cli` | WASM bindings build | ✗ | — | Wave-0: `cargo install wasm-bindgen-cli --locked --version <match crate>` (or wasm-pack) |
| Python 3 + numpy + pyproj | dev-time oracles (RD New fixtures) | ✓ | 3.13.13 / 2.4.4 / 3.7.2 | — |
| rasterio or GDAL python | dev-time COG fixture generation | ✗ | — | `pip install rasterio` (wheels bundle GDAL on Windows); alternative: generate fixtures with `tiff` crate's encoder + verify values with pyproj/numpy only |
| Internet (PDOK/S3/Overpass) | dev-time fixture capture + manual UAT only | ✓ (probed) | — | Tests never require it (fail-soft/committed fixtures) |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** `wasm-bindgen-cli`, `rasterio` — both single-command dev-time installs (Wave 0).

## Validation Architecture (advisory — `nyquist_validation: false` in config; included because oracles exist)

| Oracle | What it validates | How |
|--------|-------------------|-----|
| GDAL-generated fixture COGs + expected-window TOMLs | the whole `envi-gis` COG path (BigTIFF, tiled, deflate, predictor-3, nodata, overviews, edge tiles) | `tools/gis_oracle/gen_cog_fixtures.py` (rasterio, dev-time) writes tiny synthetic COGs + per-window expected values; committed with sha256 header; `cargo test` compares — the `tools/nord2000_oracle` pattern verbatim |
| Real AHN micro-window (CC0 → committable) | end-to-end against ground truth | commit a ≤50 KB real AHN crop + its known NAP elevations (AHN viewer / pyproj-verified); assert sampled Z within tolerance |
| pyproj fixtures for EPSG:28992 | envi-geo RD New transform | extend the existing pyproj oracle (landmark: e.g. Onze Lieve Vrouwetoren Amersfoort ≈ RD (155000, 463000)); ≤ 1 m round-trip |
| Per-row impedance test (SC3, mandated) | WorldCover→σ table | every row resolves via `envi_engine::scene::impedance_class`; count(11) asserted |
| Height-chain unit tests | DATA-03 fallback order + provenance | one test per branch incl. tag-parsing edge cases ("12 m", levels="2.5") |
| Merge property tests | D-09 | re-import over {untouched, moved, deleted, user-created} feature sets; user edits always survive |
| Playwright offline journeys | SC1/SC2/SC5 | import from fixtures → features editable → **network-off replay** (route-abort everything, reload, assert terrain/landcover/buildings render from OPFS) → attribution visible |

Suggested commands: per-task `cargo test -p envi-gis`; phase gate `cargo test` (workspace) + `npm run test:e2e`.

## Security Domain

`security_enforcement: true`, ASVS L1. No auth surface is added this phase (deferred by scope
recommendation) — the new attack surfaces are the proxy and untrusted GIS bytes.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no (deferred to amendment pass) | — record as accepted scope |
| V4 Access Control | yes (proxy) | hardcoded source allowlist; no user-supplied URLs |
| V5 Input Validation | yes | typed rejection of malformed COG/Overpass/GeoJSON; finiteness checks (existing envi-dgm/envi-geo style) |
| V10 SSRF | yes (proxy) | Pattern 5: `(host, path_prefix)` allowlist, GET-only, no redirect following (or same-host redirects only), response size cap, timeout |
| V12 File handling | yes (OPFS) | project-uuid-keyed paths only (no user-controlled path segments); quota check via `navigator.storage.estimate()` with honest failure UI |
| V6 Cryptography | no new | — |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Decompression bomb in fetched TIFF (tiny file → huge decode) | DoS | `max_decoded_px` budget enforced BEFORE decode (from IFD dims, not from decoded output); per-tile decoded-size sanity vs `TileWidth×TileLength×bpp` |
| Malicious IFD (cyclic IFD chain, absurd tile counts/offsets) | DoS/Tampering | `tiff` crate handles most; cap IFD count and tile count; typed errors, never panic (house rule) |
| SSRF via proxy | Information disclosure | allowlist (V10 above); never accept full URLs; deny non-GET |
| Oversized/poisoned Overpass response | DoS | response size cap on the fetch; per-feature validation with skip-and-report |
| OPFS quota exhaustion | DoS (self) | estimate-before-write; eviction scheme (discretion) with per-project accounting |
| Scene pollution via imported properties | Tampering | imported features pass the same `validate_feature_collection` as drawn ones (kind whitelist, uuid, WGS84 checks) — already enforced server-side |
| Supply chain (new crates) | Tampering | legitimacy audit above; `contour` gated behind human-verify |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Per-row WorldCover→σ assignments (esp. 30 grassland → D vs C; 60, 90, 95) | Mapping table | Wrong default ground effect until user review — mechanism (reviewed table) exists precisely to catch this |
| A2 | PDOK AHN COGs contain overview levels (COG-standard) | Patterns/terrain | If absent, decimation falls back to stride-sampling full-res tiles (slower, same result) — code the fallback |
| A3 | `tiff` crate decodes DEFLATE + floating-point predictor 3 (GLO-30) correctly on wasm32 | Stack | Verify with fixture in Wave 0/1; fallback: predictor-3 undo is ~30 lines if a gap surfaces |
| A4 | OSM `height` coverage <5% globally / `building:levels` sparser in NL (BAG import has geometry, not heights) | Buildings | Only affects how often the default branch fires — no design impact |
| A5 | Exact Copernicus GLO-30 attribution string wording | Attribution | Verify against Product Handbook §license before shipping the SC5 string |
| A6 | `async-tiff`/`cloudtiff`/`georust-geotiff` rejected without deep trial | Alternatives | Low — `tiff` is strictly more mature; revisit only if `tiff` hits a codec gap |
| A7 | Overpass main instance stability/rate limits acceptable for single-user tool | Buildings | Mirror endpoints + proxy fallback already designed (D-02) |
| A8 | `contour` crate is wasm32-clean (pure Rust) | Landcover | Checked at the human-verify checkpoint; hand-rolled fallback documented |

## Open Questions

1. **Grassland row (WC 30): C (80) or D (200)?**
   - What we know: descriptors overlap ("turf, grass" = C; "pasture field" = D).
   - Recommendation: default D=200; the SC3 review is the decision point. Softer C over-predicts ground dip benefit — D is the safer acoustic default.
2. **Cache-miss on another browser/machine (scene server-side, cache OPFS-side).**
   - Recommendation: detect at project open (provenance says imported; OPFS says empty) → banner + one-click re-import. Record as accepted asymmetry until the amendment pass.
3. **DGM TIN placement this phase: keep server `/dgm/triangulate` (recommended) or move client-side?**
   - Recommendation: keep server-side — thinnest slice; `envi-dgm` is wasm-clean whenever the amendment pass moves it. Decimated elevation features flow through the existing debounced producer.
4. **Elevation-sample feature volume:** decimated `elevation_point` features must stay well under envi-dgm's 500k cap and scene-PUT practicality.
   - Recommendation: target 2–10k points per import (grid spacing auto-picked from viewport size; user-adjustable); full-res raster stays in OPFS for Phase 9.
5. **Landcover vectorization detail:** min-area threshold, simplification tolerance, whether water (WC 80) becomes zones or is skipped (water = H = often the H default anyway).
6. **Guardrail numbers:** max viewport area per source resolution + `max_decoded_px`. Recommend deriving both from a single decoded-bytes budget (≈256 MB).
7. **envi-service HTTP client for the proxy:** reqwest/rustls vs raw hyper — planner's pick; TLS must stay pure-Rust.

## Sources

### Primary (HIGH confidence — live-probed or read this session)
- Live HTTP probes (curl, this session): PDOK AHN ATOM feed + DTM tile (CORS `*`, 206, BigTIFF magic, CC0 rights URL, EPSG:28992); GLO-30 S3 tile (206, **no** ACAO, 17 MB, naming); WorldCover S3 tile (206, **no** ACAO, OPTIONS 403, naming); Overpass (`ACAO: *`).
- Codebase reads: `crates/envi-dgm/src/tin.rs` (build_tin seam, MAX_POINTS, panic guards), `crates/envi-geo/src/lib.rs` + Cargo.toml (proj4rs 0.1.10, radians quarantine), `crates/envi-store/src/geojson.rs` (9 kinds, unknown-props preserved), `crates/envi-service/src/api/mod.rs` (`/dgm/triangulate`, route table), `web/package.json` (Playwright/Vite/TS).
- `cargo search` + `gsd-tools package-legitimacy` (crate versions/verdicts).
- Local environment probes (wasm32 target, node, python/pyproj, missing wasm-bindgen-cli/rasterio).

### Secondary (MEDIUM confidence — official docs fetched/cited)
- [image-tiff CHANGES.md](https://github.com/image-rs/image-tiff/blob/master/CHANGES.md) — BigTIFF since 0.6, tiles since 0.7.3, codecs, 0.11.3.
- [proj4rs repo + projections.md](https://github.com/3liz/proj4rs) — sterea/stere/tmerc/etmerc supported, towgs84, wasm32 target.
- [copernicus-dem-30m readme](https://copernicus-dem-30m.s3.amazonaws.com/readme.html) — COG layout, DEFLATE predictor 3, 1024-px tiles, DSM, latitude-dependent spacing; [Copernicus DEM Product Handbook i5.0](https://dataspace.copernicus.eu/sites/default/files/media/files/2024-06/geo1988-copernicusdem-spe-002_producthandbook_i5.0.pdf) — EGM2008 (EPSG:3855) vertical datum.
- [ESA WorldCover data access](https://esa-worldcover.org/en/data-access) + [PUM v2.0](https://esa-worldcover.s3.eu-central-1.amazonaws.com/v200/2021/docs/WorldCover_PUM_V2.0.pdf) — v200 tiles, EPSG:4326, CC-BY 4.0, 11 classes.
- [Nord2000 Road User's Guide (FORCE)](https://forcetechnology.com/-/media/force-technology-media/pdf-files/projects/nord2000/nord2000-users-guide-road.pdf) — impedance class table A–H.
- [Overpass commons doc](https://dev.overpass-api.de/overpass-doc/en/preface/commons.html) — slots/429/status.
- [whatwg/fetch #1310](https://github.com/whatwg/fetch/issues/1310) + [MDN CORS-safelisted request header](https://developer.mozilla.org/en-US/docs/Glossary/CORS-safelisted_request_header) — Range safelisting status (Chromium shipped; Firefox bugzilla 1733981/1762794 open).
- [MDN createSyncAccessHandle](https://developer.mozilla.org/en-US/docs/Web/API/FileSystemFileHandle/createSyncAccessHandle) — worker-only constraint; [Playwright Route docs](https://playwright.dev/docs/api/class-route) — fulfill with Buffer/status/headers.
- [OSM wiki Key:building:levels](https://wiki.openstreetmap.org/wiki/Key:building:levels) — ~3 m/level convention.

### Tertiary (LOW confidence — search-only, marked [ASSUMED] where used)
- OSM height-tag coverage figures (Nature Communications completeness study, OSMBuildings LoD post); reqwest-wasm DeepWiki summary; `async-tiff`/`cloudtiff` characterization; AHN5 status.

## Metadata

**Confidence breakdown:**
- Per-source CORS/endpoints: HIGH — live-probed, not searched.
- Crate stack (tiff/proj4rs/wasm-bindgen): HIGH — versions + capabilities verified on registry/repos; A3 (predictor-3 on wasm) needs a Wave-0/1 fixture.
- Architecture (sans-I/O split, whole-tile ingest): HIGH internal consistency — derived from verified constraints (preflight behavior, OPFS API split, tile sizes).
- σ mapping values: LOW–MEDIUM per row by design — the user review IS the mechanism (SC3).
- Building heights/Overpass: MEDIUM — conventions cited, coverage figures assumed.

**Research date:** 2026-07-10
**Valid until:** ~2026-08-10 for crate versions; the CORS probes are point-in-time — re-probe cheaply at execution if an import fails (bucket policies can change without notice).
