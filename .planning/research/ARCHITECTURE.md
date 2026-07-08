# Architecture Research — Milestone 2: Web App + Service Around the ENVI Engine

**Domain:** Self-hosted web GIS application wrapping an existing Rust Nord2000 propagation engine
**Researched:** 2026-07-08
**Confidence:** HIGH for crate topology / integration seams (grounded in the actual repo + locked decisions); MEDIUM for individual supporting-crate picks (web-verified this session, tagged inline)

**Scope guard.** This maps how the NEW UI/service/ingestion layers integrate with the EXISTING `envi-engine` / `envi-harness` workspace. It does not redesign the engine. All engine contracts (two-channel `H_coh`/`P_incoh`, `TransferTensor = Array3<Complex<f64>>` indexed `[sub_source, receiver, freq]`, 105-point 1/12-octave grid, e^{+jωt} with the single `.conj()` boundary in `transfer::nord_ratio_to_transfer`, `envi-engine` dependency quarantine) are treated as frozen inputs.

---

## Standard Architecture

### System Overview

```
┌──────────────────────────────────────────────────────────────────────────┐
│  BROWSER — web/ (npm workspace, not cargo)                               │
│  React + MapLibre GL JS 5 + react-map-gl 8 + Terra Draw                  │
│  ┌──────────┐ ┌───────────┐ ┌────────────┐ ┌───────────┐ ┌────────────┐ │
│  │ Project  │ │ Scene     │ │ Source cfg │ │ Weather   │ │ Results    │ │
│  │ browser  │ │ editor    │ │ (spectrum, │ │ what-if   │ │ (spectra,  │ │
│  │ (CRUD)   │ │ (draw/    │ │  balloon,  │ │ panel     │ │  isophone  │ │
│  │          │ │  edit)    │ │  cond.)    │ │           │ │  overlay)  │ │
│  └────┬─────┘ └─────┬─────┘ └─────┬──────┘ └─────┬─────┘ └─────┬──────┘ │
└───────┼─────────────┼─────────────┼──────────────┼─────────────┼────────┘
        │  REST /api/v1 — GeoJSON (WGS84) in, GeoJSON + spectra JSON out   
┌───────┴─────────────┴─────────────┴──────────────┴─────────────┴────────┐
│  envi-service (NEW crate) — axum HTTP, single self-hosted binary        │
│  ┌────────────┐ ┌───────────┐ ┌─────────────┐ ┌───────────────────────┐ │
│  │ api/       │ │ jobs/     │ │ recalc/     │ │ static/ (serves built │ │
│  │ handlers + │ │ registry, │ │ dirty-diff  │ │ frontend bundle —     │ │
│  │ DTO ⇄ CRS  │ │ CPU pool, │ │ router (MAC │ │ SVC-04 one deploy-    │ │
│  │ mapping    │ │ progress  │ │ vs re-solve)│ │ able)                 │ │
│  └─────┬──────┘ └─────┬─────┘ └──────┬──────┘ └───────────────────────┘ │
└────────┼──────────────┼──────────────┼──────────────────────────────────┘
         │              │              │
┌────────┴─────┐ ┌──────┴──────────────┴────┐ ┌──────────────────────────┐
│ envi-store   │ │ envi-gis (NEW crate)     │ │ envi-engine (EXISTING,   │
│ (NEW crate)  │ │ ALL gdal/proj/geo/rstar/ │ │  quarantine unchanged:   │
│ project.db   │ │ spade + HTTP fetchers    │ │  ndarray + num-complex + │
│ (rusqlite),  │ │  acquire/: DEM, World-   │ │  thiserror only)         │
│ scene DTOs,  │ │   Cover, Overture/OSM,   │ │  scene, geometry, freq,  │
│ tensor chunk │ │   Open-Meteo, ERA5, tile │ │  propagation/*, transfer │
│ store        │ │   cache                  │ │  + Phase-4 additions:    │
│ (memmap2)    │ │  derive/: CRS (UTM), DGM │ │   solver (promoted path  │
│              │ │   TIN, cut-profile, imp. │ │   solve), tensor asm,    │
│              │ │   segmentation, screen   │ │   MAC readout            │
│              │ │   edges, receiver grid,  │ │                          │
│              │ │   GDAL contour isobands  │ │ envi-harness (EXISTING)  │
│              │ │                          │ │  keeps FORCE/TOML I/O,   │
│              │ │ depends on envi-engine   │ │  compare, capability,    │
│              │ │ (emits Scene types) —    │ │  report CLI — calls the  │
│              │ │ NEVER the reverse        │ │  promoted engine solver  │
└──────────────┘ └──────────────────────────┘ └──────────────────────────┘
         │                    │
┌────────┴────────────────────┴────────────────────────────────────────────┐
│ DISK (per-project folder, NoizCalc-style: a project IS a directory)      │
│ projects/<id>/project.db │ cache/tiles/ │ calc/<calc_id>/{tensor/,       │
│ pincoh/, grid_A.tif, grid_C.tif, contours_*.geojson, manifest.json}      │
└───────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Implementation |
|-----------|----------------|----------------|
| `envi-engine` (existing) | Pure Nord2000 math: propagation operators, scene types, freq grid, transfer tensor + MAC readout. **Gains** (Phase 4, already roadmapped): promoted path-solve orchestration + tensor assembly + conditioning readout | Rust, deps frozen at `ndarray`+`num-complex`+`thiserror`, `#![deny(unsafe_code)]` |
| `envi-harness` (existing) | FORCE/TOML validation only — case parsing, capability gating, comparison, report CLI. **Loses** the solve composition (promoted into engine); keeps everything format-facing | calamine, serde, toml, libtest-mimic (unchanged) |
| `envi-gis` (NEW) | Everything geospatial and everything C-linked: data acquisition (DEM COG windows, WorldCover, Overture/OSM buildings, Open-Meteo/ERA5) with a disk tile cache; derivation (auto-UTM CRS, DGM TIN, DEM cut-profile extraction, land-cover→impedance segmentation, building/wall screening edges, constrained-Delaunay receiver grids, GDAL contour isobands). Emits `envi-engine` scene types + `PropagationPath` inputs | `gdal`/`gdal-sys`, `proj`, `geo`, `rstar`, `spade`, `reqwest`; depends on `envi-engine` |
| `envi-store` (NEW) | Persistence: project SQLite (metadata, scene features, settings, weather, jobs), serde DTO mirror of engine scene types (keeps serde OUT of the engine), chunked tensor store on disk with a streaming read API | `rusqlite`, `serde`/`serde_json`, `geojson`, `memmap2`; depends on `envi-engine` |
| `envi-service` (NEW) | HTTP API (axum), job registry + dedicated CPU worker pool, the **recalc router** (dirty-diff → MAC vs path-reuse re-solve vs full pipeline), CRS mapping at the wire boundary, static serving of the built frontend | `axum`, `tokio`, `rayon`, `tower-http`; depends on engine + gis + store |
| `web/` (NEW, npm) | Map UI: project CRUD, Terra Draw scene editing, source spectrum/directivity/conditioning panels, weather what-if, job status, spectral readout charts, isophone fill-polygon overlay with editable color scale | React + Vite, MapLibre GL JS 5, react-map-gl 8, `maplibre-gl-terradraw` |

### Dependency graph (arrows = "depends on")

```
envi-harness ──► envi-engine ◄── envi-gis ◄──┐
                     ▲          ▲            │
                     │          │            │
                envi-store ◄────┼──── envi-service ──► envi-store
                                └──────────── (engine, gis, store)
web/  ── REST only, no cargo edge ──► envi-service
```

**Quarantine preservation:** nothing new appears in `envi-engine`'s `Cargo.toml`; the existing `cargo tree -p envi-engine` gate keeps passing verbatim. All C-linked crates (`gdal`, `gdal-sys`, `proj`) live only in `envi-gis`, behind thin safe wrappers — the sole place `unsafe` (FFI) is permitted. `envi-harness` gains no new dependencies and no dependency on the new crates; the FORCE validation path stays exactly as isolated as today.

---

## Recommended Project Structure

```
crates/
├── envi-engine/                 # EXISTING — additions land inside Phase 4
│   └── src/
│       ├── solver.rs            # NEW (Phase 4): pub compute_tensor(scene, met, paths)
│       │                        #   -> (TransferTensor /*H_coh*/, IncohTensor /*P_incoh*/)
│       │                        #   promoted from envi-harness run_*_case composition
│       └── transfer.rs          # MODIFIED (Phase 4): + readout(chunk, conditioning)
│                                #   complex MAC p[r,f] = Σ_s H[s,r,f]·G_s(f), two-channel sum
├── envi-harness/                # EXISTING — run_case arms call envi_engine::solver
├── envi-gis/                    # NEW
│   └── src/
│       ├── crs.rs               # auto-pick UTM zone, proj transforms (GEOX-04)
│       ├── acquire/             # all external data I/O + cache
│       │   ├── dem.rs           # GLO-30 /vsicurl COG windows + national LiDAR DTM (DATA-01)
│       │   ├── landcover.rs     # ESA WorldCover → Nordtest σ classes (DATA-02)
│       │   ├── buildings.rs     # Overture GeoParquet / OSM Overpass, height fallback (DATA-03)
│       │   ├── weather.rs       # Open-Meteo runtime + ERA5/CDS classes (METX-01/02)
│       │   └── cache.rs         # tile/extract disk cache (DATA-04)
│       ├── dgm.rs               # elevation points/lines → TIN (spade), object draping
│       ├── profile.rs           # DEM cut-profile extractor along S→R line (GEOX-01)
│       ├── impedance.rs         # profile impedance segmentation + zone overrides (GEOX-02)
│       ├── screening.rs         # building/wall/forest edges along path (GEOX-03, rstar)
│       ├── paths.rs             # assemble engine PropagationPath per (sub_source, receiver)
│       ├── grid.rs              # building-aware constrained Delaunay receiver grid (GRID-01)
│       └── contour.rs           # rasterize levels → GDALContourGenerateEx isobands (GRID-04)
├── envi-store/                  # NEW
│   └── src/
│       ├── dto.rs               # serde mirror of engine scene types + From/TryFrom impls
│       ├── geojson.rs           # Feature ⇄ DTO mapping (properties.kind = object type)
│       ├── db.rs                # rusqlite schema: projects, features, settings, weather, jobs
│       ├── tensor_store.rs      # chunked H_coh/P_incoh on disk, manifest, mmap streaming
│       └── project_dir.rs       # projects/<id>/ layout, open/create/delete/pack
└── envi-service/                # NEW (the deployable binary)
    └── src/
        ├── main.rs              # axum router + static frontend serving
        ├── api/                 # handlers: projects, scene, import, weather, calc, results
        ├── jobs.rs              # job registry, dedicated CPU pool, progress channels, SSE
        ├── recalc.rs            # dirty-diff router: conditioning→MAC | weather→re-solve | geometry→pipeline
        └── state.rs             # AppState: open projects, job table, config
web/                             # NEW — React frontend (Vite; not a cargo member)
    src/{map/, editor/, panels/, results/, api/}
```

### Structure Rationale

- **One new C-boundary crate (`envi-gis`), not two.** GDAL linkage is the painful build dependency; concentrating `gdal`+`proj`+fetchers in one crate gives exactly one place with FFI, one thin boundary per the house rules, and keeps `envi-service` compilable without touching C headers during pure-API work. Acquisition (`acquire/`) and derivation (`derive`-style modules) are separated inside the crate so the pure-geometry parts stay unit-testable offline.
- **`envi-store` exists so serde never enters `envi-engine`.** Engine scene types carry no serde derives (quarantine). The DTO mirror + `From` impls are boring glue, but they are the price of keeping the `cargo tree` gate byte-identical. Rejected alternative: a feature-gated `serde` dep in `envi-engine` — it would work, but it breaks the "three deps, enforced by cargo-tree" invariant that Phases 1–2 baked into CI-equivalent checks and CLAUDE.md.
- **`envi-service` is thin.** No physics, no GDAL calls beyond delegating to `envi-gis`, no SQL beyond delegating to `envi-store`. It owns exactly: HTTP, jobs, and the recalc decision.
- **`web/` outside cargo.** The frontend is built by Vite and its `dist/` is embedded/served by `envi-service` (`tower-http` ServeDir or `include_dir!`) so the deliverable stays a single self-hosted binary + a project directory (SVC-04).

---

## Integration With the Existing Workspace — New vs Modified

| Code | Status | What / Why |
|------|--------|------------|
| `envi-engine/src/solver.rs` | **MODIFIED (new module in existing crate — lands inside engine Phase 4)** | Promote the solve composition currently spread across `envi-harness::lib` (`run_terrain_case`, `build_terrain_inputs`, direct+terrain_effect+coherence assembly) into a pure engine entry point: `compute_tensor(&Scene, &MetInputs, &[PropagationPath]) -> (H_coh, P_incoh)`. It is pure math over types the engine already owns — it only lives in the harness today because the harness was the first caller. Both harness and service then call ONE solve path; the FORCE suite validates exactly the code the web app runs. **This is the single load-bearing refactor of the milestone and must be flagged into Phase 4's plan.** |
| `envi-engine/src/transfer.rs` | **MODIFIED (Phase 4, already roadmapped as OUT-03/04/05)** | Chunk-wise MAC readout: `readout(h_chunk, p_incoh_chunk, &[SourceConditioning]) -> band levels`, where `SourceConditioning = {gain, delay τ (phase ramp e^{-j2πfτ}), filter G(f) on the 105 grid}`. Signature takes an `ArrayView3` chunk, not the whole tensor, so streaming is the default calling convention (OUT-06). |
| `envi-engine/src/scene.rs` | **UNCHANGED (verify only)** | `Scene`, `Source`, `SubSource`, `Receiver`, `Barrier`, `Building`, `TerrainProfile`, `GroundSegment`, `BandSpectrum` already live in the engine — no promotion needed. Milestone 2 may need additive fields (forest attenuation zones per TI 386 §3.2.3, per-face reflection loss on walls); additions stay serde-free. |
| `envi-harness/src/lib.rs` | **MODIFIED (shrinks)** | `run_case` keeps capability gating + comparison; its solve arms delegate to `envi_engine::solver`. `build_terrain_inputs`/`scene_build` stay (they are case-format → Scene mapping, which is I/O-adjacent and correct where it is). No new deps. |
| `envi-gis`, `envi-store`, `envi-service`, `web/` | **NEW** | As mapped above. |
| Workspace `Cargo.toml` | **MODIFIED** | `members = ["crates/*"]` already globs — new crates join automatically; add workspace-level dep versions for the new stack. |

---

## The API Boundary (frontend ⇄ `envi-service`)

Wire conventions: JSON everywhere; geometry as **GeoJSON in EPSG:4326** (MapLibre/Terra Draw native); the server reprojects to the project's stored local UTM CRS via `envi-gis::crs` on write and back on read — the frontend never sees projected coordinates (GEOX-04 enforced at one seam). Scene features carry `properties.kind ∈ {source, receiver, wall, building, forest, ground_zone, elevation_point, elevation_line, calc_area}` plus kind-specific properties (heights, impedance class A–H, roughness N/S/M/L, reflection loss, per-source spectrum/balloon/conditioning refs). Stable UUID `id` per feature. dB values are plain numbers; spectra are 105-length arrays paired with the shared `freq_hz` axis (compare by band index, never nominal frequency — the axis is served once at `GET /api/v1/meta/freq-axis`).

```
Projects (SVC-01)
  GET    /api/v1/projects                     list
  POST   /api/v1/projects                     create {name, description, default_ground}
  GET    /api/v1/projects/:id                 metadata + settings + CRS
  PUT    /api/v1/projects/:id                 update metadata/settings
  DELETE /api/v1/projects/:id

Scene (WEB-02/03/04)
  GET    /api/v1/projects/:id/scene           full FeatureCollection
  PUT    /api/v1/projects/:id/scene           replace (Terra Draw edit-session commit)
  PATCH  /api/v1/projects/:id/scene/features  upsert/delete individual features
  GET    /api/v1/projects/:id/dgm             TIN summary / hillshade preview (check-your-model)

GIS import (DATA-01..04) — long-running → job
  POST   /api/v1/projects/:id/import/gis      {bbox, layers:[terrain,landcover,buildings],
                                               default_building_height} → 202 {job_id}
                                              (NoizCalc "Import" — materializes editable features + DGM)
Weather (METX-01/02 + what-if)
  POST   /api/v1/projects/:id/weather/import  {time | range} → Open-Meteo/ERA5 fetch → stored met
  GET    /api/v1/projects/:id/weather         current effective met (imported + overrides)
  PUT    /api/v1/projects/:id/weather         manual override: {beaufort, wind_dir, downwind_worst_case,
                                               temp_gradient} or direct {A, B, C} per azimuth

Calculation (SVC-02, WEB-07, GRID-02)
  POST   /api/v1/projects/:id/calculations    {mode: receivers|grid, grid:{spacing_m, height_m},
                                               weighting:[A,C], reflection_order} → 202 {calc_id, job_id}
  GET    /api/v1/jobs/:job_id                 {state: queued|running|done|failed|cancelled,
                                               progress: {paths_done, paths_total}, message}
  GET    /api/v1/jobs/:job_id/events          SSE progress stream (polling GET is the baseline)
  DELETE /api/v1/jobs/:job_id                 cancel

Results (transfer-tensor readout — the tensor NEVER crosses the wire)
  GET    /api/v1/calculations/:cid/receivers/:rid/spectrum?weighting=A
                                              {freq_hz-ref, level_db[105], level_total_db}
  GET    /api/v1/calculations/:cid/contours?weighting=A
                                              GeoJSON MultiPolygon isobands,
                                              properties {level_low_db, level_high_db}
  PUT    /api/v1/calculations/:cid/scale      {min, interval_db, n_intervals} → recontour (server-side)
  GET    /api/v1/calculations/:cid/export?format=gtiff|geojson|gpkg   (GRID-05)

Fast recalc (OUT-03/04/05 exposed) — the interactive path
  POST   /api/v1/calculations/:cid/recalc     {conditioning: {source_id: {gain_db, delay_ms,
                                               filter?, muted?}}, weather?: override}
                                              → server routes by dirty-diff (see Data Flow);
                                              conditioning-only → synchronous for point receivers,
                                              short job for full grids (MAC + recontour)
```

**Where the seam sits:** the browser holds only *view state* (draw-in-progress geometry, panel values, color scale). All authoritative state — scene, met, tensor, results — is server-side. Isophones arrive as ready-made fill polygons (locked decision: server-side GDAL contour, no client heatmap); spectra arrive as plain arrays for charting. This keeps the ~GB tensor and all physics on the Rust side and makes the frontend a pure editor/viewer.

---

## Data Flow

### Full pipeline (first calculation of a project)

```
[1 IMPORT]   OSM/Overture bldgs ─┐
             GLO-30/LiDAR DEM  ──┼─► envi-gis::acquire (HTTP, disk tile cache)
             ESA WorldCover ─────┘        │
             Open-Meteo/ERA5 ────────► met profile → A/B/C per azimuth (engine Phase-3 math)
                                          │
[2 SCENE]    importer boundary emits semantic 2.5D model (LOCKED decision #1):
             engine Scene features + DGM TIN (spade) — user edits via Terra Draw ⇄ PUT /scene
                                          │
[3 PATHS]    receiver set: explicit receivers ∪ building-aware Delaunay grid (envi-gis::grid)
             per (sub_source_s, receiver_r):
               DEM cut-profile along S→R (envi-gis::profile)
               + impedance segmentation (landcover + ground_zone overrides)
               + screening edges (buildings/walls via rstar) + reflection surfaces
               = PropagationPath { TerrainProfile, screens, refl, met-azimuth }
                                          ├──► CACHE L1: path cache, keyed by
                                          │    hash(geometry features ∩ path corridor)
[4 SOLVE]    envi_engine::solver::compute_tensor(scene, met, paths)
             — Nord2000 per path per 105 freq, rayon over (s,r) pairs, streamed out
             — PropagationCorrection hook applied per path segment (LOCKED decision #2)
                                          │
                                          ├──► CACHE L2: chunked tensor store
                                          │    calc/<cid>/tensor/ (H_coh) + pincoh/
[5 READOUT]  per chunk: MAC p[r,f] = Σ_s H[s,r,f]·G_s(f);  L(r,f) = |p|² + P_incoh
             → A/C weighting → per-receiver totals → grid raster (GeoTIFF)
                                          │
[6 CONTOUR]  envi-gis::contour: GDALContourGenerateEx isobands → GeoJSON polygons
                                          │
[7 OVERLAY]  MapLibre fill layer + editable color scale (client-side styling only)
```

### The three recompute tiers (where the short-circuit lives)

`envi-service::recalc` diffs the request against the calc manifest (hashes of: scene geometry, met inputs, conditioning) and routes to the **cheapest valid tier**:

| Change | Tier | What runs | Interactive? |
|--------|------|-----------|--------------|
| Source conditioning — filter, delay, gain, mute, spectrum swap | **Tier 1: MAC only** | Steps 5–6: stream L2 tensor chunks through `envi_engine::transfer::readout` with new `G_s(f)`, re-weight, re-contour. No propagation. | Yes — this is OUT-03; point-receiver spectra are effectively instant; a 100k grid is a chunk-parallel streaming MAC + one contour pass |
| Weather what-if — Beaufort/wind dir, downwind toggle, temp gradient, A/B/C edit | **Tier 2: re-solve on cached paths** | Steps 4–6: L1 path cache reused (NO GIS re-extraction, no DEM I/O, no re-segmentation); engine re-runs propagation with new met → new L2 tensor | "Fast recompute" per PROJECT.md — engine-bound, not GIS-bound; run as a short job with progress |
| Geometry — move/add/delete source, receiver, wall, building, zone, terrain edit | **Tier 3: partial pipeline** | Steps 3–6, but only for dirtied (s,r) pairs: an rstar query finds paths whose corridor intersects the changed features; only those re-extract + re-solve; untouched tensor chunks are kept | Background job; incremental for local edits |
| Grid spacing / calc area / new receivers | Tier 3 (new pairs only) | New paths solved, appended as new chunks | Background job |
| Color-scale edit | Tier 0 | Step 6 only (re-contour cached raster) — or pure client restyle if interval set unchanged | Instant |

The frozen contract "only geometry/meteorology changes trigger a full propagation recompute" maps exactly onto Tier 3/Tier 2; everything the NoizCalc-style workflow calls "drive the system differently" (TI 386: spectrum + SPL at reference point, filters, delays) is Tier 1 over the cached tensor. **The recalc router in `envi-service` is where this decision lives; the MAC itself lives in `envi-engine::transfer` so the harness can validate it (F→1 ⇒ P_incoh→0 bit-exactness, conditioning identities).**

### State management (frontend)

Server is the source of truth; the client keeps a normalized scene copy (fetched FeatureCollection) + optimistic Terra Draw edits committed via PATCH; job status via polling/SSE; results layers are plain fetched GeoJSON/arrays keyed by `calc_id` + conditioning hash so a what-if slider can cancel/supersede in-flight recalcs.

---

## Persistence Model

A project **is a directory** (NoizCalc convention, TI 386 §4.1 — copyable, packable, deletable as a unit):

```
projects/<uuid>/
├── project.db                  # SQLite (rusqlite): projects meta, scene features
│                               #   (GeoJSON TEXT + kind + indexed bbox cols), settings,
│                               #   weather (imported + overrides), jobs, calc manifests
├── cache/                      # envi-gis acquire cache (DATA-04)
│   ├── dem/  *.tif             # COG windows, mosaicked per import bbox
│   ├── landcover/  overture/   # extracts, already reprojected to project CRS
│   └── weather/  *.json
└── calc/<calc_id>/
    ├── manifest.json           # dims [S,R,F=105], chunk layout, hashes: geometry_hash,
    │                           #   met_hash, receiver-set, engine version, band axis
    ├── paths/                  # L1 cache: serialized PropagationPath per (s,r) block
    ├── tensor/chunk_00042.bin  # L2: H_coh — Complex<f64> as interleaved (re,im) f64 LE,
    │                           #   chunked along the RECEIVER axis (e.g. 1024 receivers
    │                           #   per chunk), layout [s][r_local][f] — freq contiguous
    │                           #   (frozen contract), read via memmap2
    ├── pincoh/chunk_00042.bin  # P_incoh — real f64, same chunking
    ├── grid_A.tif  grid_C.tif  # readout rasters per weighting (GDAL)
    └── contours_A.geojson      # last contour set (recreated on scale change)
```

**Memory budget math (why receiver-axis chunking):** `S×R×105×16 B` for `H_coh` — e.g. 8 sub-sources × 100k receivers × 105 freq × 16 B ≈ **1.34 GB** (+0.67 GB `P_incoh`). The MAC readout consumes `H[·, r_block, ·]` — all sub-sources and all frequencies for a receiver block — so chunking on the receiver axis makes every pipeline stage (solve-write, MAC-read, raster-write) a bounded streaming pass: one 1024-receiver chunk is `8×1024×105×16 B ≈ 13.8 MB`. Peak RSS = worker count × chunk size, independent of grid size (OUT-06). Writes stream chunk-by-chunk during solve, so the full tensor is never resident even at compute time.

**Storage choice:** flat binary chunks + JSON manifest + `memmap2` is recommended (zero new format risk, trivially memory-mapped, matches ndarray layouts). `zarrs` (Zarr v3, actively maintained, parallel codecs) is the upgrade path if compression or remote stores ever matter — note Zarr has no native complex dtype, so it would store the same interleaved-f64 convention. Verified this session (LOW web confidence, but corroborating the simple choice). **Anti-choice: tensors in SQLite blobs** — kills streaming, bloats the db, and SQLite gives nothing for dense numeric data.

**DB access pattern:** single-process, single-writer `rusqlite` behind a small connection wrapper called via `spawn_blocking` from axum handlers (self-hosted single-user tool; `sqlx` async machinery is not warranted). WAL mode on.

---

## Architectural Patterns

### Pattern 1: Promote-the-solver (one solve path, two callers)

**What:** The Scene→Tensor solve composition becomes `envi_engine::solver`; `envi-harness` (FORCE validation) and `envi-service` (web calc) are both thin callers.
**When:** Do it inside engine Phase 4 while the tensor assembly is being written anyway.
**Trade-offs:** + The FORCE acceptance suite validates byte-for-byte the code the web app ships — no "validated harness path vs divergent service path" drift. + The engine quarantine makes the promoted code trivially pure. − `run_case`'s arms need a mechanical rewrite (small; the harness keeps parsing/gating/compare).

```rust
// envi-engine/src/solver.rs (pure — only engine types)
pub struct PropagationPath {
    pub sub_source: usize, pub receiver: usize,
    pub profile: TerrainProfile,           // cut profile w/ impedance segments
    pub screens: Vec<ScreenEdge>, pub reflections: Vec<ReflectionSurface>,
    pub met_azimuth_deg: f64,
    // PropagationCorrection hook (locked decision #2) applies per segment here
}
pub fn compute_tensor(scene: &Scene, met: &MetInputs, paths: &[PropagationPath],
                      sink: &mut dyn TensorSink)   // sink: chunk-streaming, impl'd by envi-store
    -> Result<(), PropagationError>;
```
(`TensorSink` is a plain trait taking `(chunk_index, ArrayView3<Complex<f64>>, ArrayView3<f64>)` — the engine stays I/O-free; `envi-store` implements it over the chunk files.)

### Pattern 2: DTO mirror at the store boundary (quarantine-preserving serde)

**What:** `envi-store::dto` defines serde-derived twins of `Scene`/`Source`/… plus `From`/`TryFrom` both ways; GeoJSON mapping targets the DTOs, never engine types.
**When:** Always — it is the only way to keep `cargo tree -p envi-engine` at three deps.
**Trade-offs:** + Quarantine gate untouched; wire format can evolve (versioned DTOs) without touching physics types. − Duplicate struct definitions (mechanical; a conversion unit test per type catches drift).

### Pattern 3: Dirty-diff recalc router (content-hash tiers)

**What:** Every calc manifest stores `geometry_hash`, `met_hash`, `conditioning` (Tier-1 inputs are NOT hashed into the tensor identity — they are readout parameters). A recalc request diffs against the manifest and picks Tier 0/1/2/3 as in the Data Flow table; Tier 3 uses an rstar corridor query to dirty only intersected paths.
**When:** From the first calculation endpoint onward — retrofitting caching semantics later invalidates stored tensors.
**Trade-offs:** + Interactivity contract (conditioning = interactive MAC) is enforced structurally, not by convention. + Hash mismatch ⇒ automatic safe fallback to a deeper tier. − Corridor-dirtying needs a conservative buffer (Fresnel-zone width) to be correct; start with "any geometry change within bbox+margin dirties the pair", optimize later.

---

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| Single user, ≤10k receivers (typical) | Everything above as-is; Tier-1 recalc effectively realtime; tensor tens of MB |
| 100k-receiver grids | Already handled by design: receiver-chunk streaming (13.8 MB working set/chunk), rayon chunk-parallel MAC, contour on the raster not the vectors. First bottleneck is Tier-2/3 **solve time**, not memory → progress bar + cancellation are mandatory UX, and partial-dirty Tier 3 matters |
| Multiple concurrent calcs | Semaphore-bounded job pool (1–2 concurrent solves; solves are internally rayon-parallel already). No queue infra (Redis/apalis) — a jobs table + in-process registry suffices for a self-hosted tool |

**First bottleneck:** full-grid Tier-2 weather what-if on big grids (engine re-run). Mitigations in order: cache L1 paths (already designed), coarse-grid preview solve (compute every 4th receiver first, refine in background), then per-pair incremental refresh.
**Second bottleneck:** GIS import volume (Overture parquet, DEM mosaics) — mitigated by the bbox-scoped disk cache; imports are jobs with progress from day one.

---

## Anti-Patterns

### Anti-Pattern 1: Serde/GeoJSON/HTTP types creeping into `envi-engine`
**What people do:** "Just add `#[derive(Serialize)]` to `Scene`."
**Why it's wrong:** Breaks the enforced three-dependency quarantine and couples the physics types to wire-format churn.
**Instead:** DTO mirror in `envi-store` (Pattern 2); engine types stay wire-blind.

### Anti-Pattern 2: Re-running the pipeline for conditioning or weather changes
**What people do:** One `POST /calculate` that always does import→paths→solve→contour.
**Why it's wrong:** Destroys the milestone's core interactivity promise (OUT-03) and the whole point of the frozen tensor contract.
**Instead:** Recalc router with hash tiers (Pattern 3). The tensor identity is (geometry, met, receiver set) — conditioning is a readout parameter, never baked into `H`.

### Anti-Pattern 3: Client-side level rendering (heatmap layer / raw grid to browser)
**What people do:** Ship the level grid as JSON and style a MapLibre heatmap.
**Why it's wrong:** Explicitly locked against (OPEN-GIS-LANDSCAPE): heatmaps are density visualizations, not isophones; grids at 100k points swamp the wire; color-scale semantics (interval edges in dB) get faked.
**Instead:** Server-side GDAL isoband polygons (`GDALContourGenerateEx` via `gdal-sys` behind a safe wrapper in `envi-gis` — verified available; pure-Rust `contour` crate is the fallback if the FFI wrapper misbehaves), GeoJSON fill layers, scale edits re-contour server-side.

### Anti-Pattern 4: Duplicating the solve in the service
**What people do:** Service-side "orchestrator" that composes `direct_path` + `terrain_effect` + coherence itself, parallel to the harness's composition.
**Why it's wrong:** Two solve paths = the FORCE-validated one and the one users actually run. Any drift (operator order, the `.conj()` boundary, F→P_incoh handling) is invisible to the acceptance suite.
**Instead:** Pattern 1 — promote to `envi_engine::solver` in Phase 4; both callers thin.

### Anti-Pattern 5: Long CPU solves on tokio's blocking pool
**What people do:** `tokio::task::spawn_blocking(|| compute_tensor(...))` for hour-scale grid solves.
**Why it's wrong:** `spawn_blocking` is for bounded blocking I/O; long CPU jobs starve the pool (DB calls share it) and can't report progress cleanly.
**Instead:** Dedicated worker (std thread or rayon scope) per job, `watch`/`mpsc` progress channel bridged to SSE/polling, semaphore-bounded concurrency, cooperative cancellation token checked between path batches.

### Anti-Pattern 6: Translating NoiseModelling source for the pathfinder
**What people do:** Port NoiseModelling's `pathfinder` classes line-by-line for cut-profile/reflection search since it "already works".
**Why it's wrong:** GPLv3 → license contamination of this non-GPL codebase (hard project rule).
**Instead:** Reimplement from the *ideas* (cut-profile abstraction, corridor search) documented in research; validate the extractor against GRASS `r.profile` as the independent oracle.

---

## Suggested Build Order (Milestone-2 phases, appended from Phase 5)

Engine Phases 3 (meteorology/refraction) and 4 (tensor + solver promotion + FORCE pass) remain the execution priority; the milestone is sequenced so nothing below blocks them and the two hard dependencies are explicit.

| Order | Phase (proposed) | Contents | Depends on engine? | Parallel-safe with Phase 3/4? |
|-------|------------------|----------|--------------------|-------------------------------|
| 5 | **Service skeleton + persistence** | `envi-store` (db schema, DTO mirror, project dir) + `envi-service` (axum, project CRUD, scene GET/PUT round-trip, static serving, job registry scaffold) | Types only (already exist) | ✅ fully |
| 6 | **Frontend shell + scene editing** | `web/`: MapLibre basemap, Terra Draw object palette (all TI-386 object kinds), property panels, project browser — against Phase-5 API | No | ✅ fully |
| 7 | **GIS ingestion + DGM** | `envi-gis::acquire` (DEM/WorldCover/Overture + cache), CRS auto-UTM, DGM TIN, import job endpoint, imported-features editing (the NoizCalc "Import" moment) | No | ✅ fully |
| 8 | **Path extraction + weather import** | `envi-gis`: cut-profile (validate vs GRASS `r.profile`), impedance segmentation, screening edges, receiver grid; `acquire::weather` → A/B/C derivation | Phase 3 (met types/A-B-C math) for the weather half; cut-profile needs only scene types | ⚠️ geometry half yes; weather half after Phase 3 |
| 9 | **Calculation service** | Tensor chunk store, `TensorSink` impl, job runner wiring `envi_engine::solver`, calc submit/status/cancel, manifest hashes | **Phase 4 (solver + tensor) — hard gate** | ❌ lands after Phase 4 |
| 10 | **Results + fast recalc** | MAC readout endpoints (spectra), recalc router (Tiers 0–3), GDAL contour isobands, noise-map overlay + editable color scale, weather what-if UI, exports | Phase 4 + Phase 9 | ❌ |

**Two coordination flags for the roadmap:**
1. **Phase 4 must include the solver promotion (Pattern 1) and the chunk-streaming `TensorSink`/readout signatures** — if Phase 4 ships tensor assembly private to the harness, Phase 9 forces a second refactor of freshly validated code.
2. Phase 8's cut-profile work defines `PropagationPath` construction — agree its shape with Phase 4's `solver` signature early (it is the one struct both sides touch).

---

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|--------------------|-------|
| Copernicus GLO-30 (AWS S3 COG) | `gdal` `/vsicurl/` windowed reads, cached per bbox | DSM-biased in forest/city — prefer national LiDAR DTM where present |
| ESA WorldCover | COG windows → class→σ (Nordtest A–H; class B = 31.5) | Attribution required (CC-BY) |
| Overture / OSM buildings | GeoParquet via `geoarrow`/`parquet` bbox pushdown, or Overpass fallback — both funnel through the importer boundary | Height fallback chain: measured → `height` → levels×3+1.5 → default (TI-386-style user default) |
| Open-Meteo | JSON REST, no key; multi-level winds/temps → A/B per azimuth | ≤10k calls/day free tier — cache per project+time |
| ERA5 / CDS | Async job API → weather-class statistics (Obukhov) | Slow; import as background job, store derived classes |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| `web/` ⇄ `envi-service` | REST JSON/GeoJSON (WGS84), SSE for progress | The only network seam; tensor never crosses it |
| `envi-service` ⇄ `envi-gis` | Direct calls; long ops inside jobs | Service passes bbox/project CRS; gis returns engine scene types + paths |
| `envi-service` ⇄ `envi-engine` | `solver::compute_tensor` + `transfer::readout` | Via jobs (solve) and recalc router (readout) |
| `envi-gis` → `envi-engine` | One-way type dependency | Engine never imports gis; enforced by existing cargo-tree gate |
| `envi-store` ⇄ engine types | DTO `From`/`TryFrom` conversions | serde quarantine seam |
| `envi-harness` ⇄ `envi-engine` | `solver` calls (post-promotion) | FORCE suite validates the exact service solve path |

---

## Sources

- Existing codebase (HIGH): `crates/envi-engine/src/{scene.rs,transfer.rs,lib.rs}`, `crates/envi-harness/src/{lib.rs,capability.rs,scene_build.rs}`, workspace `Cargo.toml` — read this session; solver-promotion claim grounded in `run_case`/`build_terrain_inputs` living in the harness today.
- Locked decisions (HIGH): `.planning/PROJECT.md`, `.planning/research/OPEN-GIS-LANDSCAPE.md`, `.planning/REQUIREMENTS.md` (v2 groups DATA/GEOX/METX/GRID/WEB/SVC/FUT), `.claude/CLAUDE.md` conventions.
- Workflow reference (HIGH, descriptive): `docs/references/dbaudio-ti386-1.6-en.md` ch. 3–4 — import→model→calculate→plot loop, object palette, project-as-folder, grid-map color scale.
- [Axum vs Actix comparison, 2026](https://sharpskill.dev/en/blog/rust/rust-actix-web-vs-axum-comparison) + [framework roundup](https://aarambhdevhub.medium.com/rust-web-frameworks-in-2026-axum-vs-actix-web-vs-rocket-vs-warp-vs-salvo-which-one-should-you-2db3792c79a2) — axum 0.8.x as pragmatic default (LOW individually; well-corroborated).
- [`gdal-sys` `GDALContourGenerateEx`](https://docs.rs/gdal-sys/latest/gdal_sys/fn.GDALContourGenerateEx.html) — verified present (docs.rs, this session); high-level [`gdal` crate](https://docs.rs/gdal) does not expose it → thin unsafe wrapper in `envi-gis`.
- [`zarrs`](https://github.com/zarrs/zarrs) chunked-array option; [tokio `spawn_blocking` guidance](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html) + [axum background-task discussions](https://github.com/tokio-rs/axum/discussions/1998) — dedicated-thread pattern for long CPU jobs.
- [Overture GeoParquet access](https://docs.overturemaps.org/getting-data/) + [geoarrow ecosystem](https://cloudnativegeo.org/blog/2024/12/interview-with-kyle-barron-on-geoarrow-and-geoparquet-and-the-future-of-geospatial-data-analysis/); [rusqlite-vs-sqlx guidance](https://aarambhdevhub.medium.com/rust-orms-in-2026-diesel-vs-sqlx-vs-seaorm-vs-rusqlite-which-one-should-you-actually-use-706d0fe912f3).

---
*Architecture research for: ENVI Milestone 2 — web app + service integration with the existing Nord2000 engine*
*Researched: 2026-07-08*
