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
| **`envi-store`** | The **serde DTO mirror** (D-05) + **project-as-folder** flat-file persistence (D-04) + frozen tensor-identity hash (D-07). | Serde lives HERE, never in `envi-engine`; DTO→engine goes through the engine's validating constructors (`TryFrom`). Every write is atomic (temp-in-dir + `sync_all` + `persist`). Conditioning is structurally excluded from `tensor_hash`. | `dto::*`, `geojson::scene_to_engine`, `project_dir::ProjectStore`, `manifest::CalcManifest`, `hash::tensor_hash` |
| **`envi-service`** | The single deployable **axum** binary (SVC-03/04): `/api/v1` + the `web/dist` bundle, localhost-bound, refuse-to-start CRS self-check (D-08), job/SSE state machine, recondition/recompute split. | Thin HTTP layer — routing, jobs, and wire contracts only. Storage delegates to `envi-store`, CRS to `envi-geo`, acoustics NEVER here (SVC-07). Long CPU work runs on a dedicated `std::thread`, never tokio's blocking pool (Anti-Pattern 5, D-08). | `cargo run -p envi-service`; `api::app`, `jobs::submit_stub_job`, `selfcheck::crs_self_check` |

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
     └──────────────► envi-geo
     └──────────────► envi-engine
envi-harness ─────────────────► envi-engine
```

`envi-service` depends on `envi-store`, `envi-geo`, and `envi-engine`;
`envi-store` depends on `envi-geo` and `envi-engine`; `envi-geo` and
`envi-engine` have no intra-workspace dependencies; `envi-harness` depends on
`envi-engine` only.

## Quarantine gates (run before any change is "done")

Two gates enforce the engine's architectural invariants. Both must hold on every
change to the workspace:

**1. Engine dependency quarantine** — the engine's direct dependencies must be
exactly `ndarray`, `num-complex`, `thiserror` (serde/axum/I/O must never enter):

```sh
cargo tree -p envi-engine -e normal --depth 1
```

**2. Single-`conj` boundary** — exactly one `.conj()` exists in the whole engine
(`transfer::nord_ratio_to_transfer`), so no propagation operator silently flips
the frozen `e^{+jωt}` time convention. The grep gate over the propagation dir
returns **0**:

```sh
grep -rh '\.conj()' crates/envi-engine/src/propagation/
```

Additionally, the whole milestone ships with **zero C-linked crates** (D-01/D-02
— no `gdal`, `proj`, `proj-sys`): `cargo tree | grep -ci 'proj-sys\|gdal'`
returns 0. GDAL/PROJ provisioning is deferred to Phase 8 (GIS ingestion).
