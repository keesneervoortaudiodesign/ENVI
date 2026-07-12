# ENVI workspace crates

The ENVI workspace is a cargo workspace (`members = ["crates/*"]`) split into a
pure-math engine and the I/O, GIS, persistence, and service crates that wrap it.
The engine is deliberately kept in a hard dependency quarantine (three deps, no
serde, no I/O) so it stays byte-identical and independently verifiable; every
crate below states the one boundary rule that keeps that architecture intact.

## Crate table

| Crate | Role | Boundary rule | Key entry points |
|-------|------|---------------|------------------|
| **`envi-engine`** | Pure-math Nord2000 core: complex, phase-preserving 1/12-octave transfer over terrain (ground, diffraction, refraction, forest, partitions). | `#![deny(unsafe_code)]`; deps **quarantined to `ndarray` + `num-complex` + `thiserror`** — **no serde, no I/O**. Verify-only for downstream phases (must stay byte-identical). Exactly **one** `.conj()` in the whole crate (`transfer::nord_ratio_to_transfer`). | `freq::{FREQ_AXIS, N_BANDS}`, `scene::{Scene, Source, Receiver, …}`, `tensor::{TensorPair, TensorSink}`, `solver::SolveJob` |
| **`envi-harness`** | All engine validation I/O: FORCE `.xls` + synthetic TOML case loaders, capability-gated `run_case` dispatch, `libtest-mimic` dynamic runner, oracle/anchor comparison, the `report` CLI. | The only crate that reads FORCE/oracle data; fail-soft (`Skipped(requires: …)`, never a false Pass). Depends on `envi-engine` only. | `cargo run -p envi-harness -- report` |
| **`envi-geo`** | The **one** CRS reprojection seam (GEOX-04): WGS84 ↔ project-local UTM in **pure Rust** (`proj4rs`, no C toolchain). | The single reprojection boundary in the milestone; `proj4rs`'s radian convention is quarantined behind the `LonLat`/`SceneXY` newtypes. No other crate calls the projection library. | `LonLat`, `SceneXY`, `ProjectCrs::{for_location, from_zone, to_utm, to_wgs84}` |
| **`envi-store`** | The **serde DTO mirror** (D-05) + **project-as-folder** flat-file persistence (D-04) + frozen tensor-identity hash (D-07). | Serde lives HERE, never in `envi-engine`; DTO→engine goes through the engine's validating constructors (`TryFrom`). Every write is atomic (temp-in-dir + `sync_all` + `persist`). Conditioning is structurally excluded from `tensor_hash`. | `dto::*`, `geojson::{scene_to_engine, scene_receivers, scene_source_count}`, `project_dir::ProjectStore`, `manifest::CalcManifest`, `hash::tensor_hash` |
| **`envi-dgm`** | The **server-side Digital Ground Model boundary** (D-08): a pure-Rust constrained-Delaunay TIN (`spade`) from user-drawn `elevation_point` vertices + `elevation_line` breaklines, with barycentric Z. Backs `POST /dgm/triangulate` (SC1 re-triangulation). | `spade` lives **HERE and nowhere else** — this crate does **NOT** depend on `envi-engine`, so `spade` can never reach the engine's 3-dep quarantine. Pure Rust: zero C toolchain (no `gdal`/`proj`/`proj-sys`), zero I/O, no serde in the runtime graph; callers pass plain `[f64; 3]`/`[f64; 2]`. Untrusted input yields typed [`DgmError`], **never a panic** (breakline interior-crossing is pre-checked, not caught from a `spade` abort). | `tin::build_tin`, `DgmError` |
| **`envi-gis`** | The **client-side GIS-ingestion boundary** (DATA-01..04, 08-CONTEXT WASM pivot): a pure-Rust, **sans-I/O** core over cached tile bytes — COG/BigTIFF decode into windowed `f32`/`u8` rasters (IFD/geotransform parse, nodata + edge-tile safety), the AHN4/GLO-30 **source registry** + tile planning, `f32` **terrain decimation** → WGS84 `elevation_point` features + footprint-boundary **base-elevation** sampling, the reviewed **WorldCover→Nord2000 σ table** (σ resolved through the engine, never restated) + `u8` land-cover → `ground_zone` **marching-squares vectorization** (SUS `contour` crate declined), **Overpass building** parse with the height fallback chain + provenance, and **re-import merge** (D-09, `user_modified`-preserving, panic-free). **Phase 9** adds the GIS→engine **geometry pipeline**: source→receiver DEM **cut-profile** extraction (GEOX-01, committed GRASS `r.profile` oracle), **impedance segmentation** along the profile (GEOX-02, drawn>imported>default), **screening-edge injection** as `(x,z)` vertices into the `TerrainProfile` (GEOX-03, rstar corridor — no separate screen list; ≤2-edge classification left to the engine), the building-aware **CDT receiver grid** (GRID-01, footprints as holes), the **Open-Meteo → per-azimuth A/B/C** weather fit (METX-01, lifted from the engine Phase-3 3×3 LSQ — single source of truth, `[ASSUMED]` constants quarantined), **ERA5 Obukhov + wind×stability occurrence-statistics** groundwork (METX-02, occurrence-stats only — L_den deferred to GRID-03), and the pure-data **`PropagationPathInputs`** bundle + weather-invariant **`PathCacheKey`** (the Phase-10 path-assembly seam). | `#![deny(unsafe_code)]`; **no network, no OPFS, no browser DOM bindings** — a synchronous API over `&[u8]`, WASM-safe; TypeScript owns all I/O. Reprojection via `envi-geo` only (GEOX-04). Guard-first, no-panic decode: a pre-decode `max_decoded_px` DoS budget from IFD dims (T-08-02-01), typed `GisError` never a panic. **No GDAL/PROJ-C — the pivot rules C out on the client path.** | `cog::{decode_window, decode_window_u8, PixelWindow, Raster, MAX_DECODED_PX}`, `registry`, `tiles`, `terrain`, `landcover`, `buildings`, `merge`, `impedance_table`; **Phase-9 pipeline** `profile` (GEOX-01), `impedance` (GEOX-02), `screening` (GEOX-03), `grid` (GRID-01), `weather` (METX-01), `era5` (METX-02), `path` (`PropagationPathInputs`/`PathCacheKey`); `GisError` |
| **`envi-gis-wasm`** | The repo's **first WASM crate** (DATA-01/02/03): a thin **`wasm-bindgen` cdylib** exposing the pure `envi-gis` core to the browser (`plan_import`, `plan_tiles`, `decode_window`, `window_for_bbox`, `terrain_features`, `sample_base_elevation`, `reproject_ring`, `map_landcover`, `parse_buildings`, `merge_features`; **Phase-9** `extract_cut_profile`, `segment_cut_profile`, `inject_screen_edges`, `build_receiver_grid`, `derive_weather`, `derive_era5`; **Phase-11** `derive_weather_friendly` (friendly what-if knobs → per-azimuth A/B/C), `difference_dba` (per-receiver signed dB(A) A−B for scenario difference maps)). | Marshalling only — **all GIS math delegated to `envi_gis::`**; `#![deny(unsafe_code)]`, no `getrandom`/`uuid`. `wasm-bindgen` pinned `=0.2.126` (CLI lockstep). All boundary DTOs generated via **ts-rs** into the single committed `web/src/generated/wire.ts` (no-drift test; one source of truth for the HTTP wire AND the WASM boundary — no hand-written TS mirror). | `dto::*`; boundary fns above |
| **`envi-compute`** | The pure-Rust, **WASM-safe** compute core of Phase 10 (D-01/D-02): the tensor-identity closure factored byte-for-byte out of `envi-store` (`tensor_hash` + `CalcManifest` + `chunk_receivers` + the `MetDto`/`ReceiverDto` identity DTOs + `geometry_positions`, re-exported by `envi-store` for source-compatibility), the SC1 **cost model + Ok/Warn/Block guardrail**, the hierarchical **points ⊂ coarse ⊂ fine tier partition** (D-05/06, no receiver recomputed), and the **`SolveJob` assembly** that first populates `SolveJob::directivity_phase_rad` (SRC-03 directional-phase seam). **Phase 11** adds the client-side **readout + contour + export** math (all in Rust/WASM, D-01 — the JS/TS layer never does acoustic arithmetic): IEC 61672-1 **A/C weighting** tables at the 105 exact 1/12-octave centres (`weighting`), the two-channel per-receiver **`readout`** that *drives* the frozen engine laws (`readout_coherent`/`compose_gain`/`band_levels_db_two_channel`, MAC≡engine bit-exact — never re-derived), fine-tier lattice→`LevelGrid` reconstruction + the hand-rolled **interpolated marching-squares iso-band fill tracer** (`grid`/`isoband`, GRID-04, `contour` crate declined), and the **GeoTIFF/GeoJSON/CSV export** encoders (`export`, GRID-05, hand-rolled Float32 GeoTIFF — zero new dep). | `#![deny(unsafe_code)]`; **no `std::fs`, no C, no rayon** — natively `cargo test`-able and WASM-ready. Depends on `envi-engine` but adds **nothing** to its 3-dep quarantine (engine byte-identical, D-02). The `std::fs` manifest I/O (`write_manifest`/`read_manifest`) stays in `envi-store`. | `identity::{tensor_hash, CalcManifest, chunk_receivers}`, `cost::{estimate, guardrail}`, `tiers::partition`, `job_assembly::assemble_jobs`; **Phase-11** `weighting`, `readout::{readout_receiver, readout_receivers, ReceiverReadout}`, `grid::reconstruct_level_grid`, `isoband::trace_isobands`, `export::{encode_geotiff, encode_isophone_geojson, encode_spectra_csv}` |
| **`envi-compute-wasm`** | The repo's **second WASM crate** and first **threaded** one (SVC-02 / GRID-02, plan 10-03): a thin **`wasm-bindgen` cdylib** over the pure `envi-compute` core (`estimate_cost`, `plan_tiers`, `request_cancel`/`reset_cancel`, and the `solve_chunk_range` signature seam) plus the **OPFS-backed `TensorSink`** (`OpfsChunkSink` — frozen `[s][r_local][f]` interleaved-`(re,im)`-f64-LE chunk format over `FileSystemSyncAccessHandle`, worker-only; the store is **keyed by the real blake3 marshalled tensor identity**, HI-01, and truncate-on-open so a shorter re-run leaves no stale bytes) and the `wasm-bindgen-rayon` `initThreadPool` re-export (behind the off-by-default `threads` feature; rayon pool driver + the real `solve_chunk_range` land in 10-04/10-06). **Phase 11** adds the **results boundaries** over the same OPFS store: the byte-exact tensor **reader** (`opfs_reader::read_chunk`, the inverse of the sink, typed `ChunkDecodeError` + finiteness gate, never a panic), the **recondition MAC** + **client-side 409** (`recondition`/`readout_receivers` — refuse a mismatched blake3 hash with a typed `HashMismatch` *before* any MAC, D-12), the live **`trace_isophones`** re-contour (SC3, no re-solve), and the `export`/`export_filename` byte generators (path-traversal-sanitized). | Marshalling only — **all cost/tier/readout/export math delegated to `envi_compute::`**; the OPFS sink is a **NEW impl of the engine's existing `TensorSink` trait**, never an engine edit (D-02). `#![deny(unsafe_code)]`, no `getrandom`/`uuid` minting. `wasm-bindgen` pinned `=0.2.126` (CLI lockstep); DTOs — incl. the `TierComplete` D-07 payload and the **Phase-11** `ReceiverReadoutDto`/`ReconditionReq`/`ReconditionResult`/`ExportReq` — ts-rs-generated into the single committed `wire.ts` (no-drift; `JobStatus` reused from `envi-service`). The threaded (atomics + `-Zbuild-std`) build is scoped to the one `build:wasm:compute` npm script — **no repo `rust-toolchain.toml`, no atomics in `.cargo/config.toml`** (Pitfall 1) — and emits a **shared `WebAssembly.Memory`** so `initThreadPool` can hand it to the pool workers. | `dto::{TierComplete, TierPlanResult, CostEstimateResult, ReceiverReadoutDto, ReconditionReq, ExportReq, …}`, `opfs_sink::{OpfsChunkSink, ChunkHandle}`, `opfs_reader::read_chunk`; boundary fns above incl. **Phase-11** `readout_receivers`, `recondition`, `trace_isophones`, `export`, `export_filename` |
| **`envi-service`** | The single deployable **axum** binary (SVC-03/04): `/api/v1` + the `web/dist` bundle, localhost-bound, refuse-to-start CRS self-check (D-08), job/SSE state machine, recondition/recompute split, and the **allowlisted byte proxy** `GET /api/v1/proxy/{source}/{*path}` (08-03) — a bytes-only GET/Range relay for the two CORS-blocked S3 sources (GLO-30, WorldCover), SSRF-proof by a hardcoded `(host, path_prefix)` allowlist, `redirect::Policy::none()`, size cap + timeout, and host/path-free errors. **Phase 9** adds a **flagged-off** (`#[cfg(feature="era5")]`, default-off; `era5_route_absent_by_default` contract test) SSRF-allowlisted **ERA5/CDS retrieval endpoint** `POST /api/v1/era5/import` reusing the job state machine (CDS host hardcoded, key from server env only, generic 500 — no host/key leak). Serves the built **`web/`** frontend (Vite + React 19 + MapLibre/Terra Draw scene editor) as a static SPA-fallback bundle. | Thin HTTP layer — routing, jobs, wire contracts, and the byte relay only (no compute in the proxy). Storage delegates to `envi-store`, CRS to `envi-geo`, the DGM TIN to `envi-dgm`, GIS decode to `envi-gis(-wasm)`, acoustics NEVER here (SVC-07). Pure-Rust TLS (`rustls`, no native-tls/openssl). Long CPU work runs on a dedicated `std::thread`, never tokio's blocking pool (Anti-Pattern 5, D-08). | `cargo run -p envi-service`; `api::app`, `api::proxy`, `jobs::submit_stub_job`, `selfcheck::crs_self_check` |

### Phase-7 authoring endpoints (server-owned math, SVC-07)

The scene editor authors coarse spectra + elevation sets; the acoustics/geometry
math stays server-side (the client never does Hz/log/triangulation arithmetic):

| Endpoint | Crate seam | Purpose |
|----------|-----------|---------|
| `POST /meta/interpolate-spectrum` | `envi-store` (single interpolate core, D-05) | Expand an authored coarse isolation spectrum (1/1- or 1/3-octave, 9/27 anchors) onto the dense **105-point 1/12-octave band-index** grid. |
| `POST /meta/spl-to-lw` | `envi-store` (SVC-07) | Back-calculate sound power `L_W[105]` from a free-field SPL-at-reference spectrum (the free-field correction is server-side). |
| `POST /dgm/triangulate` | `envi-dgm` (`tin::build_tin`) | Constrained-Delaunay TIN from elevation points + breaklines; a typed 4xx on interior-crossing/degenerate input. |

### The `web/` frontend

`web/` is a **Vite + React 19 + TSX** single-page app (MapLibre GL JS 5 +
react-map-gl 8 + Terra Draw scene editor). It imports the Rust wire DTOs from a
**committed generated mirror** (`web/src/generated/wire.ts`, regenerated by the
`wire_no_drift` test — D-10) so a DTO rename fails the Rust test, not the
browser. Playwright drives the **real built bundle** fully offline (basemap +
`/api/v1` route-intercepted). The production `vite build` output is committed at
`web/dist/` (a load-bearing artifact — `envi-service` serves it, and the
`static_bundle_served_with_spa_fallback` contract test asserts it); all frontend
tooling is a `devDependency` only and never ships in the bundle.

> `tools/nord2000_oracle/` is a committed, independent Python (`scipy.special.wofz`)
> reference generator for the harness fixtures — **not** a workspace crate and not
> a build dependency.

## Dependency direction

Dependencies flow one way — toward the frozen engine. Nothing depends on the
service; the engine depends on nothing in the workspace.

```text
envi-service ──► envi-store ──► envi-geo
     │               │
     │               └────────► envi-engine
     ├──────────────► envi-geo
     ├──────────────► envi-engine
     └──────────────► envi-dgm            (spade TIN; NO envi-engine edge)
envi-harness ─────────────────► envi-engine
```

`envi-service` depends on `envi-store`, `envi-geo`, `envi-engine`, and
`envi-dgm`; `envi-store` depends on `envi-geo` and `envi-engine`; `envi-geo`,
`envi-engine`, and `envi-dgm` have no intra-workspace dependencies (`envi-dgm`
deliberately does NOT depend on `envi-engine`, so `spade` stays out of the
engine's quarantine); `envi-harness` depends on `envi-engine` only.

## Quarantine gates (run before any change is "done")

Three gates enforce the engine's architectural invariants. All must hold on every
change to the workspace:

**1. Engine dependency quarantine** — the engine's direct dependencies must be
exactly `ndarray`, `num-complex`, `thiserror` (serde/axum/I/O must never enter):

```sh
cargo tree -p envi-engine -e normal --depth 1
```

**2. DGM boundary quarantine** — `spade` lives only in `envi-dgm`, which must NOT
depend on `envi-engine` (so the TIN library can never reach the engine's
3-dependency quarantine). `envi-dgm`'s direct deps are exactly `spade` +
`thiserror`, and `envi-engine` is absent from its tree:

```sh
cargo tree -p envi-dgm -e normal --depth 1          # spade + thiserror only
cargo tree -p envi-dgm | grep -c envi-engine        # returns 0
```

**3. Single-`conj` boundary** — exactly one `.conj()` exists in the whole engine
(`transfer::nord_ratio_to_transfer`), so no propagation operator silently flips
the frozen `e^{+jωt}` time convention. The grep gate over the propagation dir
returns **0**:

```sh
grep -rh '\.conj()' crates/envi-engine/src/propagation/
```

Additionally, the whole milestone ships with **zero C-linked crates** (D-01/D-02
— no `gdal`, `proj`, `proj-sys`): `cargo tree | grep -ci 'proj-sys\|gdal'`
returns 0. GDAL/PROJ provisioning is deferred to Phase 8 (GIS ingestion).
