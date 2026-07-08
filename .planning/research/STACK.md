# Stack Research — Milestone 2: Interactive Calculation UI + Service

**Domain:** Self-hosted web app (Rust HTTP service + React/MapLibre frontend) wrapped around the existing `envi-engine` Nord2000 acoustics engine
**Researched:** 2026-07-08
**Confidence:** HIGH for all version numbers (verified directly against crates.io / npm registry APIs on 2026-07-08); MEDIUM for architecture/pattern recommendations (multi-source web research, cross-checked against official docs); LOW items flagged inline.

> **Scope guard.** The engine stack (ndarray + num-complex, the two-crate workspace, the frozen `H[sub_source, receiver, freq]` complex tensor contract) and the locked high-level picks (MapLibre GL JS 5 + react-map-gl 8 + Terra Draw; Rust HTTP backend; geo/rstar/spade/gdal/proj; server-side isophone fill polygons) are **decided** — this document selects the concrete libraries/versions inside those locks and how they bolt onto the existing workspace. Nothing here relitigates OPEN-GIS-LANDSCAPE.md.

---

## Workspace integration (how the new stack hosts the existing crates)

Two **new** workspace members; the existing crates are untouched:

```
crates/
  envi-engine/    (existing — pure math; deps stay ndarray + num-complex + thiserror ONLY)
  envi-harness/   (existing — FORCE/TOML validation harness; test-only, not a service dep)
  envi-gis/       (NEW — all GIS I/O: gdal, proj, geoparquet, reqwest, spade grids,
                   cut-profile extraction, contouring. The gdal/proj FFI quarantine
                   lives HERE, behind thin safe wrappers.)
  envi-server/    (NEW — axum HTTP service: API, job manager, project persistence,
                   static frontend serving. Depends on envi-gis + envi-engine.)
frontend/         (NEW — Vite + React app; built bundle served by envi-server)
```

- Dependency direction: `envi-server → envi-gis → envi-engine`. The `cargo tree -p envi-engine` quarantine check keeps holding — neither new crate is a dependency *of* the engine.
- `envi-harness` stays the validation harness; `envi-gis` gets its own oracle-style tests (e.g. cut-profile vs GRASS `r.profile`, contour vs `gdal_contour`), mirroring the Milestone-1 oracle pattern.
- One deployable: `cargo build --release -p envi-server` produces the single self-hosted binary (SVC-03/04).

---

## Recommended Stack

### 1. HTTP backend — axum on tokio

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **axum** | 0.8.9 | HTTP framework | The 2026 Rust default: tokio-team maintained, tower-native middleware, built-in SSE (`axum::response::sse`), typed extractors. actix-web's only edge is ~10–15% raw throughput under extreme load — irrelevant for a single-user self-hosted tool; axum's tower/tokio ecosystem fit and simpler model win. |
| **tokio** | 1.52.3 | Async runtime | The runtime axum requires (≥1.44); `rt-multi-thread`, `macros`, `signal`, `sync`, `fs` features. |
| **tower** | 0.5.x | Service/middleware trait | Comes with axum; protocol-agnostic middleware. |
| **tower-http** | 0.7.0 | HTTP middleware | `ServeDir` (frontend bundle + SPA fallback), `trace`, `compression-gzip` (GeoJSON responses compress ~10×). Verified: 0.7.0 targets http 1 + tower 0.5 → compatible with axum 0.8. |
| **serde / serde_json** | 1 / 1.0.150 | API DTOs + persistence | Universal; scene/settings/job JSON. |
| **tracing / tracing-subscriber** | 0.1 / 0.3 | Structured logging | The tokio-ecosystem standard; job/ingest diagnostics. |
| **uuid** | 1.23.4 | Job + project IDs | `v4` feature; stable IDs for the job registry and API routes. |

**Integration notes:**
- `AppState { projects: ProjectStore, jobs: JobManager }` in an `Arc`, injected via axum `State`.
- API namespace `/api/*`; everything else falls through to `ServeDir("frontend/dist")` with `index.html` fallback (SPA routing).
- Light/no auth per project constraints — bind to localhost by default; if ever exposed, put a reverse proxy in front rather than adding an auth framework.

### 2. Compute-job model — in-process, rayon compute, SSE progress

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **rayon** | 1.12.0 | Batch grid parallelism (GRID-02) | `par_iter` over receivers/paths is the natural shape of the propagation batch; work-stealing pool sized to cores (NoizCalc exposes exactly this "number of threads" knob). |
| **tokio-stream** | 0.1 | `WatchStream` for SSE | Adapts a `tokio::sync::watch` progress channel into the `Stream` axum's `Sse` response consumes. |
| *(std)* `Mutex<HashMap<Uuid, Job>>` | — | Job registry | A handful of concurrent jobs at most — no job crate, no DashMap needed. |
| *(std)* `AtomicUsize` / `AtomicBool` | — | Progress counter / cancel flag | Incremented from rayon worker threads (receivers done / total); cancel checked between receiver chunks (rayon can't be preempted mid-task). |

**The pattern (submit / queue / run / fetch):**
1. `POST /api/projects/{id}/jobs` validates the scene, allocates a `Uuid`, inserts `Job { status: Queued, progress: watch::Sender }` in the registry, returns `202 { job_id }` immediately.
2. The job body runs via `tokio::task::spawn_blocking` (tokio's documented bridge for CPU-bound work), which drives rayon `par_iter` over receiver chunks; each chunk writes its tensor slab and bumps the atomic counter; a small side task publishes `JobProgress { done, total, phase }` into the `watch` channel.
3. `GET /api/jobs/{id}` — poll snapshot; `GET /api/jobs/{id}/events` — **SSE** stream of progress (axum built-in `Sse` + `WatchStream`). SSE over WebSocket: progress is strictly server→client, SSE is plain HTTP, auto-reconnects natively (`EventSource`), and needs zero extra dependencies. The UI never pushes over the socket.
4. `DELETE /api/jobs/{id}` sets the cancel `AtomicBool`; the compute loop exits at the next chunk boundary. Results land in the project folder (see §3); `GET /api/jobs/{id}/result` returns the isophone GeoJSON / receiver spectra.
5. **Fast recalc is NOT a job**: source-conditioning changes (filter/delay) hit a synchronous `POST /api/projects/{id}/recalc` that does the complex MAC over the cached tensor (`p[r,f] = Σ_s H[s,r,f]·G_s(f)`) and re-contours — sub-second, returns results inline. Only geometry/meteorology changes spawn a full propagation job.

**Why no job crate:** apalis/faktory/Redis-backed queues exist for distributed multi-worker fleets. This is one process on one machine with one user (SVC-04); an in-process registry is less code than any crate's configuration, and job state doesn't need to survive restarts (re-run the calculation).

### 3. Project persistence — NoizCalc-style project folder + npy tensor chunks

NoizCalc's model ("a project consists of several files stored jointly in one project folder", TI 386 §4.1) is exactly right for GB-scale artifacts, and it's the recommendation:

```
projects/MyVenue.envi/
  project.json        — metadata, calculation settings, weather inputs, color scale
  scene.json          — semantic 2.5D scene (sources, receivers, walls, buildings,
                        ground zones, elevation lines, calc area) — geo via GeoJSON geometry
  gis/                — cached DEM/WorldCover windows (GeoTIFF), buildings.geojson  (DATA-04)
  met/                — cached Open-Meteo / ERA5 responses (JSON/CSV)
  tensors/
    manifest.json     — dims, chunking, freq grid id, scene hash it was computed from
    h_coh.r0000.npy   — complex128 chunks, split along the receiver axis
    p_incoh.r0000.npy — real f64 incoherent-power channel (same chunking)
  results/
    grid_dba.npy, isophones_dba.geojson, receiver_spectra.json
```

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **ndarray-npy** | 0.10.0 | Tensor chunk serialization | Verified: requires `ndarray ^0.17.1` and supports `Complex<f64>` (numpy `complex128`) via the default `num-complex-0_4` feature — an **exact match** for the engine's pinned `ndarray 0.17` + `num-complex 0.4`. `.npy` is a dumb, mmap-able, numpy-inspectable format (the scipy oracle tooling can read it directly). |
| **memmap2** | 0.9.11 | Zero-copy chunk reads | Fast-recalc MACs read tensor chunks without deserializing; npy's fixed header + contiguous body maps cleanly. |
| **serde_json** | 1.0.150 | scene/project/manifest JSON | Human-diffable project files; trivially versioned with a `"format_version"` field. |

**Integration notes:**
- **Chunk along the receiver axis, keep freq contiguous** — this preserves the frozen tensor layout (`[sub_source, receiver, freq]`, freq-contiguous) and directly satisfies OUT-06's chunk/stream memory budget: a 100k-receiver grid never has to be resident at once, and the recalc MAC streams chunk-by-chunk.
- `manifest.json` carries a **scene hash** — tensor cache invalidation is "geometry/met changed ⇒ hash differs ⇒ tensor stale ⇒ full job required", which is precisely the fast-recalc contract.
- **No SQLite in the v2.0 baseline.** Project list = scan the projects directory (NoizCalc does the same). rusqlite 0.40.1 (`bundled`) is the future pick *if* a job-history/catalog need appears — but do not put tensors in it; multi-GB blobs in SQLite are an anti-pattern.
- **GeoPackage is an export format only** (GRID-05, via `gdal`), never the project store.

### 4. GIS ingestion (`envi-gis`)

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **gdal** | 0.19.0 (+ gdal-sys 0.12) | COG windowed reads: Copernicus GLO-30 + ESA WorldCover via `/vsicurl/`; GeoTIFF cache; GeoPackage/GeoTIFF export | The accepted FFI exception. Both datasets are public COGs on AWS S3 — `Dataset::open("/vsicurl/https://…")` + windowed `read_as` fetches only the needed tiles (DATA-01/02). Set `GDAL_DISABLE_READDIR_ON_OPEN=EMPTY_DIR` and enable `CPL_VSIL_CURL` caching. |
| **proj** | 0.31.0 | CRS transforms (GEOX-04) | The accepted FFI exception #2 (PROJ ≥9). Auto-UTM is a 5-line formula — zone = `floor((lon+180)/6)+1`, EPSG `32600+zone` (N) / `32700+zone` (S) — feeding `Proj::new_known_crs("EPSG:4326", "EPSG:326xx")`. No extra crate. |
| **geoparquet** | 0.8.0 | Overture buildings (DATA-03) | **Pure Rust** GeoParquet reader (georust; arrow/parquet ^58 underneath) with bbox-filtered reads against GeoParquet 1.1 — Overture ships bbox columns, so a site-extent query range-reads only the relevant row groups from S3. Avoids requiring a GDAL build with the Parquet driver (unreliable on Windows) and avoids DuckDB entirely. |
| **object_store** | (version pinned by parquet ^58) | Anonymous S3 range reads for Overture | Used through geoparquet/parquet's async reader; let geoparquet's dependency graph drive the exact version. |
| **reqwest** | 0.13.4 | Open-Meteo (METX-01) + ERA5/CDS (METX-02) + Overpass fallback HTTP clients | The standard async HTTP client; use `rustls-tls` + `json` features (no OpenSSL linkage). Open-Meteo is keyless JSON. |
| **csv** | 1.4.0 | ERA5 point-timeseries parsing | CDS's ERA5 point time-series route delivers CSV — parse with `csv`, **avoiding NetCDF/GRIB FFI crates entirely**. ⚠ LOW confidence flag: verify during the METX phase that the CDS timeseries endpoint exposes the u*/H/z₀ fields needed for Obukhov-length weather classes; if only the gridded NetCDF route has them, prefer GRIB via a pure-Rust decoder or a tiny preprocessing script over adding `netcdf` FFI. |
| **geo** | 0.33.1 | 2D geometry ops (impedance-zone overlay, path clipping, polygon simplify) | Already locked; current version verified. |
| **rstar** | 0.13.0 | Spatial index (buildings/zones along a path, GEOX-02/03) | Already locked; the in-process R*-tree. |
| **spade** | 2.15.1 | DGM triangulation + constrained-Delaunay receiver grids (GRID-01) | Already locked; CDT + refinement also covers the NoizCalc-style elevation-point thinning (insert-until-error-below-tolerance against a max-deviation threshold, TI 386 §4.2.5). |
| **geojson** | 1.0.0 | GeoJSON de/serialization (scene geometry, isophones, Overpass) | 1.0 (Mar 2026) — georust's serde-based GeoJSON ↔ geo-types bridge. |

**Integration notes:**
- The **DEM cut-profile extractor** (GEOX-01, the known biggest self-build) lives in `envi-gis`: sample the DEM (or the spade DGM TIN) along the source→receiver line, emit the segmented profile struct `envi-engine` already consumes from FORCE cases. Same input contract = the engine doesn't change.
- All ingested data is cached under the project folder (`gis/`, `met/`) — DATA-04 — so a saved project recalculates offline.
- Keep every gdal/proj call inside `envi-gis`; expose only geo-types/ndarray types across the crate boundary (the thin-FFI-boundary house rule).

### 5. Server-side isophone pipeline

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **contour** | 0.13.1 | Marching-squares **isobands** → filled polygons (GRID-04) | Pure-Rust d3-contour port producing filled level-band MultiPolygons directly. The locked decision is *server-side fill polygons*; `contour` delivers exactly that natively (house rule: native Rust over FFI). The high-level `gdal` crate does **not** expose `GDALContourGenerate` — using GDAL here would mean hand-rolling an unsafe `gdal-sys` wrapper for no gain. |
| **geo** (Simplify) | 0.33.1 | Polygon simplification / smoothing | Douglas-Peucker on the band polygons ≈ NoizCalc's "filter bandwidth" contour smoothing knob. |
| *(self, ~100 lines)* TIN → raster resample | — | Barycentric interpolation of receiver values onto a regular grid | The CDT receiver grid is scattered; `contour` wants a regular grid. Linear interpolation over the spade triangles is exact w.r.t. the TIN and trivial to implement. |
| *(in envi-engine)* dB(A)/dB(C) weighting | — | Level weighting | IEC 61672 A/C closed-form curves evaluated on the 105-point 1/12-octave grid (by **band index**, per house rule) — pure math, zero dependencies, belongs in `envi-engine::freq`. |

**Pipeline:** receiver spectra (from tensor + `P_incoh`) → per-receiver L<sub>A</sub>/L<sub>C</sub> → TIN→raster resample → `contour` isobands at the user's color-scale interval edges → `geo` simplify → `proj` reproject UTM→WGS84 → `geojson` FeatureCollection with `{level_min, level_max}` per feature → MapLibre `fill` layer with a data-driven color expression. The **color scale stays client-side** (editable intervals/colors, NoizCalc §4.6.5 style) — recontouring only happens when interval *edges* change.

**Validation:** cross-check band polygons against `gdal_contour -p` output on the same raster (oracle pattern, like FORCE/scipy in Milestone 1). If `contour`'s output quality or performance ever disappoints, the fallback is a thin `gdal-sys::GDALContourGenerateEx` wrapper — an accepted-FFI escape hatch, not the default.

### 6. Frontend

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **vite** | 8.1.3 | Build/dev tooling | Current stable major (8.1, verified); instant dev server, `server.proxy` forwards `/api` to the axum port during development. |
| **@vitejs/plugin-react** | 6.0.3 | React JSX transform | Standard pairing with Vite 8. |
| **react / react-dom** | 19.2.7 | UI framework | Locked (JSX/React); 19.2 is current stable. |
| **maplibre-gl** | 5.24.0 | Map renderer | Locked at major 5; 5.24 is the current release. |
| **react-map-gl** | 8.1.1 | React bindings | Locked at major 8. **Import from `react-map-gl/maplibre`** (v8 split the codebase; the maplibre endpoint requires maplibre-gl ≥4, so 5.24 is fine). The identical code is independently published as `@vis.gl/react-maplibre` 8.1.1 — a drop-in rename if visgl ever sunsets the combined package. |
| **terra-draw** | 1.31.2 | Drawing engine (WEB-02/03/04) | Locked. Use the **core library + adapter directly** (not a prebuilt toolbar): ENVI's scene objects (directional sources, walls with heights, impedance zones, elevation lines, calc area) are *typed* objects needing custom modes, per-type styling, and property panels — a generic draw toolbar doesn't model that. |
| **terra-draw-maplibre-gl-adapter** | 1.4.1 | Terra Draw ↔ MapLibre glue | The official adapter package (Terra Draw v1 split adapters out of core). |
| **zustand** | 5.0.14 | State management | Small, unopinionated store for scene graph, selection, job status, color scale; no Redux ceremony; plays well with frequent map-driven updates. |
| **recharts** | 3.9.2 | Per-band spectrum readout (WEB-05) | Declarative React charting; a 105-point per-band line/bar spectrum with A/C-weighted overlays is well within its comfort zone. (If spectra ever need 60 fps live updates, swap the plot component for uPlot 1.6 — isolated behind one component.) |
| *(native)* `fetch` + `EventSource` | — | API + SSE client | Zero dependencies; `EventSource` handles SSE reconnect natively. No axios. |

**Basemap & terrain tiles:**
- Basemap: **OpenFreeMap** vector tiles (free, keyless, no usage caps) as the default style; raw `tile.openstreetmap.org` raster is acceptable for an internal low-volume tool but is policy-capped — don't ship it as the default. (MEDIUM confidence — validate tile-source choice during the UI phase.)
- Optional 3D terrain/hillshade for visual checking (the *computational* DGM truth is server-side): AWS Terrain Tiles (Terrarium `raster-dem`, free S3) via `map.setTerrain`.

**Serving the bundle:** `npm run build` → `frontend/dist` → tower-http `ServeDir` with `index.html` fallback. Optionally embed via **rust-embed 8.12.0** for a true single-file deployable; start with `ServeDir` (simpler, no rebuild-on-frontend-change), add rust-embed when packaging matters.

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| **@playwright/test** 1.61.1 | Frontend UAT (mandated) | devDependency only; drives the real built bundle; mock `/api/*` per-test via `page.route(...)`; artifacts git-ignored. |
| **cargo clippy / fmt / test** | Quality gates | Unchanged; new crates join the workspace gates. `#![deny(unsafe_code)]` on `envi-server`; `envi-gis` allows `unsafe` only if a `gdal-sys` escape hatch is ever needed. |
| GDAL system library 3.12 | Runtime dep of `envi-gis` | Windows dev: OSGeo4W or conda-forge or vcpkg; document `GDAL_HOME`/`PKG_CONFIG_PATH` setup in README. This is the one non-cargo install the project has. |

---

## Installation

```bash
# Backend (crates/envi-server)
cargo add axum@0.8 tokio@1 --features tokio/rt-multi-thread,tokio/macros,tokio/signal,tokio/sync,tokio/fs
cargo add tower@0.5 tower-http@0.7 --features tower-http/fs,tower-http/trace,tower-http/compression-gzip
cargo add serde@1 --features derive
cargo add serde_json@1 uuid@1 --features uuid/v4
cargo add tracing@0.1 tracing-subscriber@0.3 tokio-stream@0.1 rayon@1

# GIS crate (crates/envi-gis)
cargo add gdal@0.19 proj@0.31 geo@0.33 rstar@0.13 spade@2 geojson@1
cargo add geoparquet@0.8            # brings arrow/parquet ^58 + object_store transitively
cargo add reqwest@0.13 --features rustls-tls,json
cargo add csv@1 ndarray-npy@0.10 memmap2@0.9 contour@0.13

# Frontend (frontend/)
npm create vite@latest frontend -- --template react
npm install maplibre-gl@5 react-map-gl@8 terra-draw@1 terra-draw-maplibre-gl-adapter@1 zustand@5 recharts@3
npm install -D @playwright/test
```

---

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| axum 0.8 | actix-web 4.12 | Only for extreme-throughput services (~10–15% more req/s); its middleware is framework-specific vs tower's ecosystem. Not this project. |
| In-process job registry (std Mutex + rayon) | apalis / Redis-backed queue | Multi-worker distributed fleets or jobs that must survive restarts. A single-process self-hosted tool needs neither. |
| Project folder + per-chunk `.npy` | **zarrs 0.23** (Zarr v3) | If tensor chunks later need compression codecs, cloud object-store backends, or concurrent multi-writer access. zarrs is excellent but a large dependency tree for what a manifest + npy chunks already deliver. Revisit if OUT-06 memory budgeting outgrows flat files. |
| Project folder (no DB) | rusqlite 0.40.1 (`bundled`) | If a cross-project catalog, job history, or search emerges. SQLite-only + sync fits (wrap in `spawn_blocking`); sqlx 0.8.6 only if async/compile-checked queries become important. Tensors stay in files regardless. |
| geoparquet (pure Rust) | DuckDB preprocessing / `overturemaps` CLI | One-off manual extracts. In-service ingestion should not shell out or embed an analytics engine. |
| contour crate (pure Rust isobands) | `gdal-sys::GDALContourGenerateEx` wrapper | If contour quality/perf on large grids disappoints — accepted-FFI escape hatch, validated against the same oracle. |
| recharts | uPlot 1.6.32 | High-frequency live spectra (animation at 60 fps); uPlot is faster but imperative. |
| react-map-gl 8 (`/maplibre` entry) | @vis.gl/react-maplibre 8.1.1 | Same code, maplibre-only package name. Adopt if the combined package stops tracking maplibre releases. |
| ServeDir static serving | rust-embed 8.12 | When single-binary distribution matters more than dev-loop convenience. |

---

## What NOT to Use (explicit exclusions)

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| WebSockets for job progress | Bidirectional machinery for a unidirectional stream; more state, no `EventSource` auto-reconnect | axum SSE + `EventSource` |
| apalis / faktory / Redis / RabbitMQ | Distributed job infra for a one-process tool | In-process registry + rayon |
| PostgreSQL / PostGIS | A database server contradicts "single self-hosted deployable"; rstar/geo cover in-process spatial queries | Project folder (+ rusqlite later if needed) |
| diesel / sea-orm | No relational schema exists to manage | serde JSON files |
| DuckDB | Embedded analytics engine just to read Overture | geoparquet crate |
| netcdf / hdf5 crates | Heavy C FFI outside the accepted gdal/proj exceptions | CDS CSV timeseries route (verify in phase research) |
| geotiff crate (0.1) | Immature (known from landscape research) | gdal COG reads |
| utm crate | Stale (2022); auto-UTM is a 5-line formula | Inline formula + proj |
| dashmap | Unneeded concurrency dep at this job count | std `Mutex<HashMap>` |
| mapbox-gl-draw | Broken on MapLibre (known from landscape research) | Terra Draw |
| Leaflet / OpenLayers | Locked out — MapLibre GL is decided | maplibre-gl 5 |
| deck.gl | GPU overlay framework; server-side fill polygons render as plain MapLibre fill layers | MapLibre `fill` layer |
| Heatmap layer for results | Explicitly rejected in the locked stack | Server-side isophone fill polygons |
| Next.js / Remix / SSR | The frontend is a static SPA served by axum; SSR adds a Node runtime to a Rust deployable | Vite SPA + ServeDir |
| Redux / redux-toolkit | Ceremony; store needs are simple | zustand 5 |
| axios | `fetch` is sufficient and native | fetch + EventSource |
| GraphQL / utoipa OpenAPI codegen | Tiny single-consumer API; schema machinery is bloat | Hand-written REST + serde DTOs |
| Auth frameworks (e.g. axum-login) | "Light/no auth" constraint; internal tool | Localhost bind; reverse proxy if ever exposed |
| Tensors in SQLite | GB blobs in a B-tree page store; kills mmap fast-recalc | Per-chunk `.npy` + manifest |
| GitHub Actions / CI scaffolding | Repo rule: no CI unless explicitly requested | Operator-run cargo/npm/Playwright |

---

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| axum 0.8.9 | tokio ≥1.44, tower 0.5, http 1, hyper 1 | Verified from crates.io dependency metadata. |
| tower-http 0.7.0 | http 1 + tower 0.5 → axum 0.8 | Verified from crates.io dependency metadata. |
| ndarray-npy 0.10.0 | **ndarray ^0.17.1 + num-complex 0.4** | Verified (docs.rs): exact match for the engine's existing pins (`ndarray = "0.17"`, `num-complex = "0.4"`); complex128 via default `num-complex-0_4` feature. |
| geoparquet 0.8.0 | arrow/parquet **^58**, geo-types ^0.7.16, geoarrow-array 0.8 | Verified. Let geoparquet pin arrow/parquet (58, not the newest 59.1) and its matching object_store; geo 0.33 shares geo-types 0.7 ✓. |
| gdal crate 0.19.0 / gdal-sys 0.12 | System GDAL (3.12 current) | Requires GDAL C library installed; Windows via OSGeo4W/conda/vcpkg. |
| proj 0.31.0 | System PROJ 9.x | Second accepted FFI boundary. |
| react-map-gl 8.1.1 (`/maplibre`) | maplibre-gl ≥4 → 5.24.0 ✓ | Verified from visgl docs. |
| terra-draw 1.31.2 + terra-draw-maplibre-gl-adapter 1.4.1 | maplibre-gl 5.x | maplibre.org's official example runs Terra Draw on MapLibre 5.24. |
| vite 8.1.3 | Node.js ≥ 20.19 / 22.12 (Vite 7 baseline; confirm exact Vite 8 floor at setup) | React 19.2 + @vitejs/plugin-react 6 ✓. |
| Rust workspace | edition 2024, rust-version 1.96 | All recommended crates build on stable 2026 toolchains. |

---

## Open items for phase-level research (LOW confidence flags)

1. **ERA5/CDS access shape (METX-02):** whether the CDS point-timeseries (CSV) route exposes u*, sensible heat flux and z₀, or whether the gridded route (NetCDF/GRIB) is unavoidable — determines if a pure-Rust path exists. Decide in the meteorology-import phase.
2. **Basemap source policy:** OpenFreeMap vs MapTiler-with-key vs OSM raster for the default style — validate rendering quality + terms during the first UI phase.
3. **Vite 8 exact Node floor** — check at frontend scaffold time.
4. **`contour` crate performance on 100k+-cell rasters** — benchmark early in the isophone phase; the `gdal-sys` escape hatch is the documented fallback.

## Sources

- **crates.io registry API** (2026-07-08) — exact latest stable versions for all Rust crates listed (gdal 0.19.0, proj 0.31.0, geo 0.33.1, rstar 0.13.0, spade 2.15.1, contour 0.13.1, ndarray-npy 0.10.0, axum 0.8.9, tokio 1.52.3, tower-http 0.7.0, reqwest 0.13.4, rusqlite 0.40.1, geojson 1.0.0, geoparquet 0.8.0, parquet 59.1.0, rayon 1.12.0, memmap2 0.9.11, zarrs 0.23.13, rust-embed 8.12.0, uuid 1.23.4, csv 1.4.0, serde_json 1.0.150) + dependency-graph compatibility checks — HIGH (primary registry).
- **npm registry API** (2026-07-08) — maplibre-gl 5.24.0, react-map-gl 8.1.1, @vis.gl/react-maplibre 8.1.1, terra-draw 1.31.2, terra-draw-maplibre-gl-adapter 1.4.1, @watergis/maplibre-gl-terradraw 1.14.3, vite 8.1.3, react 19.2.7, zustand 5.0.14, recharts 3.9.2, uplot 1.6.32, @playwright/test 1.61.1, @vitejs/plugin-react 6.0.3 — HIGH (primary registry).
- docs.rs/ndarray-npy — ndarray ^0.17.1 + complex128 support verification — HIGH.
- github.com/georust/gdal + docs.rs/gdal-sys — contour generation not in high-level API — MEDIUM.
- visgl.github.io/react-map-gl + github.com/visgl/react-maplibre — v8 maplibre split / @vis.gl/react-maplibre relationship — MEDIUM.
- maplibre.org/maplibre-gl-js/docs (Terra Draw example), terradraw.water-gis.com — Terra Draw on MapLibre 5 — MEDIUM.
- tokio docs (`spawn_blocking`), tokio-rs/axum discussion #1998, users.rust-lang.org task-pool threads — background-job pattern — MEDIUM (cross-checked).
- 2026 framework comparisons (rustify.rs, sharpskill.dev, reintech.io, Medium) — axum-vs-actix landscape — LOW (narrative web sources; used for context only, not load-bearing).
- zarrs.dev / docs.rs/zarrs — Zarr v3 alternative — MEDIUM.
- geoparquet.org, docs.rs/geoparquet, developmentseed.org/lonboard (Overture GeoParquet spatial reads) — MEDIUM.
- `docs/references/dbaudio-ti386-1.6-en.md` (project-local) — NoizCalc project-folder model, DGM workflow, calculation settings, color-scale UX — the workflow template this stack serves.

---
*Stack research for: ENVI Milestone 2 — Interactive Calculation UI + service layer*
*Researched: 2026-07-08 (Fable 5); all registry versions verified same-day*
