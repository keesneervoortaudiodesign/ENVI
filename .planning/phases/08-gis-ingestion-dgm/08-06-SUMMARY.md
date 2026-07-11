---
phase: 08-gis-ingestion-dgm
plan: 06
subsystem: infra
tags: [wasm, wasm-bindgen, ts-rs, vite, envi-gis, gis-ingestion, boundary-dto]

# Dependency graph
requires:
  - phase: 08-04
    provides: envi-gis feature layer (registry, terrain_features, sample_base_elevation, buildings, merge, impedance table)
  - phase: 08-05
    provides: envi-gis landcover vectorization (WorldCover Raster<u8> → ground_zone polygons)
provides:
  - "envi-gis-wasm cdylib: the first WASM crate — thin wasm-bindgen boundary exposing the pure envi-gis core to the browser (7 boundary fns, marshalling-only)"
  - "cog::decode_window_u8 (WorldCover u8 class-raster decode) — closes the 08-05 u8-seam so map_landcover turns tile bytes into ground_zone features"
  - "terrain::base_elevation_on_raster — raster-backed footprint-boundary median sampler (boundary needs no closure)"
  - "Version-locked wasm-bindgen (=0.2.126) + wasm-bindgen-cli 0.2.126; Vite build:wasm wiring; web/README.md build docs"
  - "WASM boundary DTOs generated into the single committed web/src/generated/wire.ts via ts-rs, no-drift-tested (D-10)"
affects: [gis-ingestion, wasm, frontend-import-ui, milestone-2]

# Tech tracking
tech-stack:
  added: [wasm-bindgen 0.2.126, wasm-bindgen-cli 0.2.126, js-sys 0.3, serde-wasm-bindgen 0.6.5]
  patterns:
    - "WASM boundary = thin cdylib over the pure sans-I/O core (mirror of envi-service's thin-HTTP-handler rule): marshalling only, all GIS math delegated to envi_gis::"
    - "Tile bytes cross as a direct &[u8] wasm-bindgen parameter (efficient typed-array), params/results as serde-wasm-bindgen JsValue DTOs"
    - "WASM boundary DTOs ride the SAME ts-rs generate-and-commit no-drift mechanism as the HTTP wire — one committed wire.ts, one source of truth"
    - "wasm-bindgen crate ↔ CLI version lockstep pinned with `=` + `--locked --version` (Pitfall 8)"

key-files:
  created:
    - crates/envi-gis-wasm/Cargo.toml
    - crates/envi-gis-wasm/src/lib.rs
    - crates/envi-gis-wasm/src/dto.rs
    - web/README.md
  modified:
    - crates/envi-gis/src/cog/window.rs
    - crates/envi-gis/src/cog/mod.rs
    - crates/envi-gis/src/terrain.rs
    - crates/envi-service/Cargo.toml
    - crates/envi-service/tests/wire_no_drift.rs
    - web/src/generated/wire.ts
    - web/vite.config.ts
    - web/package.json
    - .gitignore

key-decisions:
  - "Single wire.ts (Option A): envi-service dev-depends on envi-gis-wasm (rlib) and registers the boundary DTOs in the existing export_all_wire_types — one committed artifact for BOTH the HTTP wire and the WASM boundary, not a parallel wasm.ts"
  - "Tile bytes are a direct &[u8] parameter, never a serde DTO field (avoids the slow Array<number> path); only params/results marshal via serde-wasm-bindgen"
  - "Provenance built at the boundary by resolving the registry source id → 'static source+license (never a leaked runtime &'static str; license never restated)"
  - "GeoJSON feature-collection payloads are typed `unknown` on the wire (validated by envi-store on PUT) rather than minting a bespoke GeoJSON TS type this plan"
  - "ts-rs is a normal dep of envi-gis-wasm; it compiles for wasm32 (the fs export path is never invoked on the wasm side)"

patterns-established:
  - "First WASM crate in the repo — the boundary-only cdylib template (delegates to a sans-I/O core, no logic, no getrandom/uuid)"
  - "u8 vs f32 COG decode: parallel decode_window/decode_window_u8 producers feeding terrain (f32) and landcover (u8) consumers"

requirements-completed: [DATA-01, DATA-02, DATA-03]

# Metrics
duration: 70min
completed: 2026-07-11
status: complete
---

# Phase 8 Plan 6: envi-gis-wasm WASM Boundary + Version-Locked Build + Generated DTOs Summary

**A logic-free `envi-gis-wasm` cdylib exposes the pure GIS-ingestion core to the browser over a version-locked `wasm-bindgen` (=0.2.126) build, with all 7 boundary functions and 24 boundary DTOs generated from Rust and no-drift-tested into the single committed `wire.ts` — no hand-written TypeScript.**

## Performance

- **Duration:** ~70 min
- **Completed:** 2026-07-11T02:49:23Z
- **Tasks:** 2
- **Files modified:** 13 (4 created, 9 modified)

## Accomplishments
- Created the repo's **first WASM crate**, `envi-gis-wasm`: a thin `cdylib`+`rlib` wasm-bindgen boundary exposing `plan_import`, `decode_window`, `terrain_features`, `sample_base_elevation`, `map_landcover`, `parse_buildings`, `merge_features` — each delegating to exactly one `envi_gis::` core path, no GIS math of its own (`#![deny(unsafe_code)]`).
- **Closed the 08-05 u8-seam:** added `cog::decode_window_u8` so `map_landcover` actually decodes a WorldCover `u8` class COG and vectorizes it into `ground_zone` features (the decoder previously handled only `f32` terrain).
- **Kept the boundary logic-free** for base elevation by adding `terrain::base_elevation_on_raster` (a raster-backed footprint-boundary median sampler) in the core, so the boundary marshals bytes→raster→core with no closure crossing wasm.
- **Version lockstep:** pinned `wasm-bindgen = "=0.2.126"`, installed `wasm-bindgen-cli --locked --version 0.2.126`; the wasm32 build + `wasm-bindgen --target web` pipeline runs clean, emitting all 7 boundary fns. No `getrandom`/`uuid` in the graph (Pitfall 9).
- **Vite wiring:** `npm run build:wasm` (cargo wasm32 + wasm-bindgen → git-ignored `src/generated/wasm/`) chained into `build`; `web/README.md` documents the build and the mandatory crate↔CLI version match (0.2.126).
- **Generated wire types (D-10):** all 24 boundary DTOs derive `ts_rs::TS`, are registered in `export_all_wire_types`, and generated into the single committed `web/src/generated/wire.ts`; the `wire_no_drift` test passes with no diff. Request DTOs are `#[serde(deny_unknown_fields)]`.

## Task Commits

1. **Task 1: envi-gis-wasm cdylib boundary + Wave-0 CLI pin + Vite build wiring** — `e2040cf` (feat)
2. **Task 2: WASM boundary DTOs generated + committed + no-drift test** — `2c8eef2` (feat)

## Files Created/Modified
- `crates/envi-gis-wasm/Cargo.toml` — cdylib+rlib crate; `wasm-bindgen = "=0.2.126"`, js-sys, serde-wasm-bindgen, envi-gis, ts-rs.
- `crates/envi-gis-wasm/src/lib.rs` — 7 `#[wasm_bindgen]` boundary fns + marshalling helpers (serde-wasm-bindgen, registry-resolved provenance, GeoJSON FeatureCollection wrap/unwrap).
- `crates/envi-gis-wasm/src/dto.rs` — 24 serde + ts-rs boundary DTOs (`deny_unknown_fields` on requests; feature payloads typed `unknown`).
- `crates/envi-gis/src/cog/window.rs` + `cog/mod.rs` — `decode_window_u8` (u8 class-raster decode, same guard-first ordering as the f32 path) + re-export.
- `crates/envi-gis/src/terrain.rs` — `base_elevation_on_raster` (raster-backed boundary-median sampler) + 2 tests.
- `crates/envi-service/Cargo.toml` — dev-dependency on `envi-gis-wasm` (for the shared no-drift test).
- `crates/envi-service/tests/wire_no_drift.rs` — registered the 24 boundary DTOs in `export_all_wire_types`.
- `web/src/generated/wire.ts` — regenerated to include the WASM boundary types (committed).
- `web/vite.config.ts` — `assetsInclude` glob for the wasm output + documented pipeline (no `node:*` imports).
- `web/package.json` — `build:wasm` + `build:web` scripts; `build` chains the wasm step.
- `web/README.md` — build + version-lockstep docs (pinned 0.2.126).
- `.gitignore` — ignore the regenerated `web/src/generated/wasm/` build artifact.

## Decisions Made
- **Single wire.ts (Option A over a parallel wasm.ts):** the plan allowed either; routing the WASM DTOs through the existing `export_all_wire_types` + one committed `wire.ts` keeps a single source of truth for both the HTTP wire and the WASM boundary, matching D-10. Cost: `envi-service` gains a dev-only rlib dep on the wasm crate (native compile of wasm-bindgen is fine; nothing reaches the running service).
- **Tile bytes as `&[u8]` param, not a DTO field** — efficient wasm-bindgen typed-array marshalling; serde DTOs stay bytes-free.
- **Boundary provenance via registry lookup** — the `'static` source id + license come from `envi_gis::registry`, never restated; runtime strings (`source_ref`, `retrieved_at`) stay owned.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2/3 - Missing Critical + Blocking] Added `cog::decode_window_u8` (WorldCover u8 decode seam)**
- **Found during:** Task 1 (wiring `map_landcover`)
- **Issue:** `vectorize_landcover` consumes a `Raster<u8>`, but `cog::decode_window` decodes only `f32` terrain COGs (the seam flagged at 08-05 close). Without a u8 producer, `map_landcover` would be a dangling boundary fn with no way to turn WorldCover tile bytes into `ground_zone` features.
- **Fix:** Added `decode_window_u8` (parallel to `decode_window`, identical guard-first ordering, `DecodingResult::U8` branch, nodata→hole) + `cog` re-export. The f32 path is untouched (zero regression).
- **Files modified:** crates/envi-gis/src/cog/window.rs, crates/envi-gis/src/cog/mod.rs
- **Verification:** `cargo build -p envi-gis-wasm --target wasm32-unknown-unknown` compiles the `map_landcover` path; envi-gis suite green (39+8 tests).
- **Committed in:** `e2040cf`

**2. [Rule 2 - Missing Critical] Added `terrain::base_elevation_on_raster` (keeps the boundary logic-free)**
- **Found during:** Task 1 (wiring `sample_base_elevation`)
- **Issue:** `terrain::sample_base_elevation` takes a `Fn(f64,f64)->Option<f64>` closure — not marshallable across wasm. A naive fix would put the nearest-pixel sampling logic in the boundary, violating "boundary logic-free".
- **Fix:** Added `base_elevation_on_raster(ring, max_spacing, &Raster<f32>)` in the core (nearest-pixel inverse-geotransform sampler over the raster) so the boundary decodes + calls it. Geometry stays in the pure core.
- **Files modified:** crates/envi-gis/src/terrain.rs (+2 unit tests)
- **Verification:** 2 new tests (`base_elevation_on_raster_reads_the_boundary_from_a_decoded_window`, `..._is_none_when_ring_is_outside_the_window`) pass.
- **Committed in:** `e2040cf`

**3. [Rule 3 - Blocking] `registry::source` keys on `&'static str`; resolved runtime ids via `registry()` scan**
- **Found during:** Task 1 (building provenance from a runtime `source_id`)
- **Issue:** `registry::source(&'static str)` cannot accept an owned string; `E0521` (borrow escapes to `'static`).
- **Fix:** Scanned `registry::registry().iter().find(|d| d.id == p.source_id.as_str())` in the boundary helper (returns `&'static SourceDescriptor`). No envi-gis API change.
- **Files modified:** crates/envi-gis-wasm/src/lib.rs
- **Verification:** wasm32 build compiles clean.
- **Committed in:** `e2040cf`

**4. [Rule 1 - Bug] `vite.config.ts` `node:url` import broke `tsc --noEmit`**
- **Found during:** Task 2 (frontend typecheck against the augmented wire.ts)
- **Issue:** The initial `@wasm` alias used `fileURLToPath` from `node:url`, but the web tsconfig has `types: []` / no `@types/node` and typechecks `vite.config.ts` → `TS2307: Cannot find module 'node:url'`.
- **Fix:** Reworked the wasm reference to a plain `assetsInclude` glob (no `node:*` import); the future ingestion UI imports the glue via a direct relative path. Avoids pulling in `@types/node` (out of scope for a boundary plan).
- **Files modified:** web/vite.config.ts
- **Verification:** `npx tsc --noEmit` → exit 0.
- **Committed in:** `2c8eef2`

---

**Total deviations:** 4 auto-fixed (1 missing-critical+blocking u8 seam, 1 missing-critical sampler, 1 blocking registry lookup, 1 bug). **Impact:** All necessary for a working, logic-free boundary that typechecks; two are core-side additions (correctly placed in envi-gis, not the boundary). No scope creep — the u8 seam was explicitly flagged for reconciliation.

## Issues Encountered
- **`target/debug/envi-service.exe` lock** (a running service holds it): the native workspace relink for the no-drift regeneration and the full clippy/test gates hit `Access is denied (os error 5)`. Resolved per the env note by running those steps with `CARGO_TARGET_DIR=target/verify`, then removing `target/verify`. The wasm32 build and envi-gis library tests were unaffected. No process was killed.

## Quality Gates
- `cargo build -p envi-gis-wasm --target wasm32-unknown-unknown` — **succeeds**.
- `wasm-bindgen --version` = **0.2.126** (matches the `=0.2.126` crate pin); `npm run build:wasm` pipeline emits all 7 boundary fns.
- `cargo test -p envi-service --test wire_no_drift` — **passes** (no drift).
- `cargo clippy --all-targets -- -D warnings` — **clean** (workspace).
- `cargo fmt --check` — **clean** (workspace).
- `cargo test` — **all green** (workspace, incl. envi-gis 39+8, envi-service 108+wire_no_drift; engine byte-identical).
- `npx tsc --noEmit` (web) — **exit 0**.
- `cargo tree -p envi-gis-wasm` — **no getrandom/uuid**.

## User Setup Required
None — no external service configuration required. (Operator note: a full `npm run build` now requires the `wasm32-unknown-unknown` target and `wasm-bindgen-cli 0.2.126`, per web/README.md; `npm run build:web` skips the wasm step.)

## Next Phase Readiness
- The WASM boundary is ready for the TS ingestion orchestrator (fetch + OPFS + `crypto.randomUUID()`) to call it — that browser-side wiring is the remaining Wave-5 work (08-07+), not built here.
- `directivity_phase_rad` seam and the FORCE road-emission integration remain the standing Milestone-1/2 pending items (unaffected by this plan; engine byte-identical).

---
*Phase: 08-gis-ingestion-dgm*
*Completed: 2026-07-11*

## Self-Check: PASSED
- Created files verified on disk: `crates/envi-gis-wasm/{Cargo.toml,src/lib.rs,src/dto.rs}`, `web/README.md`, `08-06-SUMMARY.md`.
- Task commits verified in git history: `e2040cf` (Task 1), `2c8eef2` (Task 2).
- All plan `<verification>` commands re-run green (wasm32 build, no-drift, clippy, fmt, cargo test, tsc).
