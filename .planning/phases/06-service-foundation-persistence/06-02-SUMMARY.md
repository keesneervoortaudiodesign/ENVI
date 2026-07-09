---
phase: 06-service-foundation-persistence
plan: 02
subsystem: infra
tags: [serde, dto, geojson, persistence, blake3, tensor-hash, uuid, tempfile, atomic-write, crud]

# Dependency graph
requires:
  - phase: 06-service-foundation-persistence (plan 01)
    provides: "envi-geo — LonLat/SceneXY/ProjectCrs (to_utm/to_wgs84/for_location/from_zone), GeoError, the one reprojection seam (GEOX-04)"
  - phase: 01-engine-foundations
    provides: "envi-engine scene vocabulary (BandSpectrum/Scene/TerrainProfile...), freq::N_BANDS, tensor budget consts, SceneError archetype"
provides:
  - "envi-store crate — the serde DTO mirror (D-05) + project-as-folder persistence (D-04) + frozen tensor-identity hash (D-07)"
  - "StoreError typed boundary error (Archetype B, PathBuf-carrying I/O variants)"
  - "dto: BandSpectrum/Sub/Source/Receiver/Barrier/Building/GroundSegment/TerrainProfile twins + Crs/Met/Settings/ProjectMeta/Conditioning DTOs; TryFrom via engine validating constructors"
  - "geojson: 9-kind vocabulary (KINDS), validate_feature_collection, scene_to_engine (the one reprojection call site in the store)"
  - "project_dir: ProjectStore folder CRUD + reopen-last + atomic_write (new_in+sync_all+persist)"
  - "manifest: CalcManifest [S,R,105] mirroring TensorPair + chunk_receivers from engine consts + reserved tensor/pincoh dirs"
  - "hash: tensor_hash blake3 over canonical f64::to_bits LE bytes, conditioning structurally excluded"
affects: [envi-service, scene-persistence, recondition-recompute, tensor-identity, phase-07-ui, phase-09-compute]

# Tech tracking
tech-stack:
  added: ["serde 1 (derive)", "serde_json 1", "geojson 1.0", "uuid 1 (v4+serde)", "tempfile 3", "blake3 1"]
  patterns:
    - "Serde quarantine seam (D-05): engine types twinned as DTOs; DTO->engine via TryFrom through validating constructors; engine stays serde-free + byte-identical"
    - "Atomic save (Pitfall 4): NamedTempFile::new_in(dir) + sync_all + persist; every mutation routes through one atomic_write helper"
    - "Path-traversal gate (Pitfall 7): Uuid-typed ids never join raw strings; canonicalize+starts_with containment guard on destructive ops"
    - "Frozen canonical hash: version-prefixed, domain-separated, length-prefixed f64::to_bits LE bytes; NEVER serialized JSON text"
    - "Structural exclusion as a contract: conditioning is unhashable because tensor_hash's signature accepts no conditioning type (D-07)"

key-files:
  created:
    - crates/envi-store/Cargo.toml
    - crates/envi-store/src/lib.rs
    - crates/envi-store/src/dto.rs
    - crates/envi-store/src/geojson.rs
    - crates/envi-store/src/project_dir.rs
    - crates/envi-store/src/manifest.rs
    - crates/envi-store/src/hash.rs
  modified:
    - Cargo.lock

key-decisions:
  - "geojson 1.0 uses GeometryValue struct-variants { coordinates } and a newtype Position (TinyVec, Index+as_slice) — matched to those, not the tuple-variant/Vec<f64> shape the plan sketch assumed"
  - "scene.geojson persisted in WGS84 (A3); reprojection to SceneXY happens only in scene_to_engine via ProjectCrs::to_utm (the single store-side call site)"
  - "last_opened() of a deleted project returns None (existence-checked) rather than a dangling id — the documented choice the plan allowed"
  - "Source/Receiver DTOs carry a Uuid id; Source's engine twin drops it (engine Source has no id — id is store/wire identity only)"
  - "Tensor hash covers a fixed acoustic-property allowlist (heights/thickness/impedance_class/spectrum_band_db) plus geometry coords, met, and receiver-set — never conditioning"

patterns-established:
  - "envi-store is the serde quarantine seam for the whole service; envi-service (06-03) delegates onto it"
  - "atomic_write is the single write path; direct fs::write of project files is forbidden (grep gate: 0)"
  - "tensor_hash is the frozen SC4 identity primitive that 06-03's recondition 409-check binds to"

requirements-completed: [SVC-01, SVC-05, SVC-07, GEOX-04]

# Metrics
duration: 19min
completed: 2026-07-09
status: complete
---

# Phase 6 Plan 02: envi-store Persistence + DTO Mirror Summary

**The `envi-store` crate: a serde DTO mirror of the engine scene (engine stays serde-free and byte-identical), a project-as-folder flat-file store with atomic saves + full CRUD + reopen-last, and a frozen blake3 tensor-identity hash that structurally excludes conditioning (D-07).**

## Performance

- **Duration:** 19 min
- **Started:** 2026-07-09T17:12:26Z
- **Completed:** 2026-07-09T17:32:01Z
- **Tasks:** 3
- **Files modified:** 7 created + Cargo.lock

## Accomplishments
- New `envi-store` crate — the storage half of SC1 and the wire-DTO half of SC4/SVC-07; the serde quarantine seam (D-05) with the engine dependency tree still exactly `ndarray + num-complex + thiserror` and `git diff crates/envi-engine/` empty.
- Serde twins of every engine scene type with `TryFrom` conversions that route through the engine's private-field validating constructors (`BandSpectrum::from_values`, `TerrainProfile::new`); dense `[105]`-by-band-index spectra with length + finiteness rejection (SVC-07).
- The locked 9-kind GeoJSON vocabulary with unknown-kind loud rejection and unknown-to-engine kind preservation; `scene_to_engine` reprojects every coordinate through the single `envi_geo::ProjectCrs::to_utm` call site (GEOX-04).
- Project-as-folder CRUD (create/list/open/save/duplicate-excluding-calc/delete + reopen-last) with atomic `new_in + sync_all + persist` writes and a Uuid + canonicalize path-traversal guard.
- `CalcManifest` reserving the `[S,R,105]` receiver-chunked tensor layout (chunk size from the engine's own budget constants) with honest `stub=true` provenance, plus the frozen `tensor_hash` over canonical `f64::to_bits` LE bytes.

## Task Commits

Each task was committed atomically:

1. **Task 1: Scaffold envi-store + dto serde mirror with validating TryFrom** - `8cc5eaf` (feat)
2. **Task 2: geojson boundary — 9-kind vocabulary, unknown-kind preservation, one reprojection seam** - `673a39c` (feat)
3. **Task 3: project folder CRUD, atomic saves, reserved manifest layout, frozen tensor hash** - `e7f8284` (feat)

_TDD note: Tasks 1 and 3 carry `tdd="true"`. Because Rust intermediate commits with tests referencing not-yet-written types do not compile, each task was landed as a single GREEN commit whose task-level gate (`cargo test -p envi-store`) passes — atomic per-task, with the behavior tests co-located and green at commit time._

## Files Created/Modified
- `crates/envi-store/Cargo.toml` - I/O-crate manifest: envi-engine + envi-geo path deps, serde/serde_json/geojson/uuid/tempfile/blake3/thiserror; boundary-rule comment; no [lints] table.
- `crates/envi-store/src/lib.rs` - Crate header (quarantine + flat-file ethos + one reprojection seam), `#![deny(unsafe_code)]`, `StoreError` (Archetype B).
- `crates/envi-store/src/dto.rs` - Serde twins + `TryFrom` via validating constructors; `deny_unknown_fields` on frozen request DTOs, `serde(default)` on the Met/Settings/Conditioning trio; 5 behavior tests.
- `crates/envi-store/src/geojson.rs` - `KINDS` (9), `validate_feature_collection`, `scene_to_engine` (one `to_utm` call site); `geometry_positions` shared with the hash; 4 tests.
- `crates/envi-store/src/project_dir.rs` - `ProjectStore` folder CRUD + reopen-last + `atomic_write`; Uuid ids + containment guard; 4 tests.
- `crates/envi-store/src/manifest.rs` - `CalcManifest`, `chunk_receivers` from engine consts, reserved `tensor/`+`pincoh/` dirs; 3 tests.
- `crates/envi-store/src/hash.rs` - `tensor_hash` frozen canonical-byte blake3; conditioning structurally excluded; 2 tests.

## Decisions Made
- **geojson 1.0 API shape:** the enum is `GeometryValue` with struct-variants `{ coordinates }` and `Position` is a `TinyVec` newtype (Index + `as_slice`), not the tuple-`Value` / `Vec<f64>` shape the plan sketch implied. Matched to the real 1.0 API — no behavior change to the contract.
- **WGS84-on-disk, reproject-on-load (A3):** `scene.geojson` stays valid RFC 7946 GeoJSON; the only reprojection is inside `scene_to_engine`.
- **`last_opened()` existence-checked:** returns `None` (not a dangling id / error) once the recorded project is deleted — the documented choice the plan permitted.
- **`tensor_hash` acoustic-property allowlist:** heights / thickness / impedance_class / spectrum_band_db plus geometry coordinates, hashed presence-flagged; keeps identity canonical without serializing arbitrary JSON.

## Deviations from Plan

None - plan executed exactly as written. No deviation rules (bug / missing-critical / blocking) fired. The geojson-1.0 API differences (Task 2) were expected schema-fit work within the task's stated action, not an auto-fix. All task actions, verification commands, and acceptance criteria were followed.

## Issues Encountered
- **geojson 1.0 type shapes** surfaced only at compile time: `Value` -> `GeometryValue`, tuple variants -> struct variants `{ coordinates }`, and `Position` being a `TinyVec` newtype rather than `Vec<f64>`. Resolved by matching the real 1.0 API (struct-variant patterns, `Position::as_slice`/`Index`) — 4 targeted edits, no contract change.
- **clippy `-D warnings`** flagged a `sort_by` that should be `sort_by_key` and a doc list-continuation in the hash header; both fixed, workspace clippy/fmt now clean.
- The `proj4rs` GEOX-04 verification greps for zero occurrences in `crates/envi-store/src/`; an early lib.rs doc comment named `proj4rs` in prose — reworded so the gate reads exactly 0 while the boundary statement is preserved.

## User Setup Required

None - no external service configuration required. All dependencies are crates.io packages already vetted in the 06-RESEARCH Package Legitimacy Audit; no C toolchain, no credentials.

## Verification Evidence
- `cargo test -p envi-store`: 18 unit tests green (5 dto + 4 geojson + 3 manifest + 2 hash + 4 project_dir).
- Full workspace `cargo test`: green — engine + harness untouched, FORCE case skip-honest (1 ignored).
- `cargo clippy --all-targets -- -D warnings`: clean. `cargo fmt --check`: clean.
- `cargo tree -p envi-engine -e normal --depth 1`: exactly `ndarray`, `num-complex`, `thiserror` (serde did NOT enter the engine).
- `git diff --stat HEAD -- crates/envi-engine/`: empty (engine byte-identical).
- Gates: `grep -rn 'fs::write' crates/envi-store/src/` = 0 (all writes via atomic_write); `grep -rn 'proj4rs' crates/envi-store/src/` = 0 (GEOX-04 one seam); non-comment `ConditioningDto` in hash.rs = 0 (D-07 structural exclusion); `new_in`/`sync_all` present; hash carries `envi-tensor-hash-v1` + `to_le_bytes`, no JSON serialization of hash inputs; conj gate = 0.

## Frozen On-Disk Schema (for Phase 7-11 consumers)

**`projects/<uuid>/project.json`** (`ProjectMetaDto`): `id: Uuid`, `name: String`, `description: String`, `created_at_unix: u64`, `modified_at_unix: u64`, `crs: { utm_zone: u8, south: bool, label: String }`, `settings: { met: { temperature_c: f64=15.0, humidity_pct: f64=70.0 }, default_ground_class: char='D' }`.

**`projects/<uuid>/scene.geojson`**: RFC 7946 FeatureCollection in **WGS84** `[lon, lat]`. Each feature: `properties.kind` in the 9-kind vocabulary (`source, receiver, wall, building, forest, ground_zone, elevation_point, elevation_line, calc_area`), `properties.id` a Uuid string, plus per-kind properties (`height_m`, `thickness_m`, `eaves_height_m`, `impedance_class`, `spectrum_band_db[105]`, `z_m`).

**`projects/<uuid>/calc/<calc_id>/manifest.json`** (`CalcManifest`): `calc_id: Uuid`, `dims: [S, R, 105]` (mirrors `TensorPair [sub_source, receiver, freq]`), `chunk_receivers: usize` (receiver-axis chunk size), `tensor_hash: String` (64-hex), `stub: bool` (true in Phase 6), `created_at_unix: u64`. Reserved empty dirs: `tensor/` (complex H_coh), `pincoh/` (real P_incoh).

**`projects/.envi-state.json`**: `{ last_project_id: Uuid, opened_at_unix: u64 }` (reopen-last).

**tensor_hash input inventory (D-07):** version prefix `envi-tensor-hash-v1` → scene features (uuid-sorted; kind + id + geometry coordinates + acoustic properties: eaves_height_m/height_m/thickness_m/z_m/impedance_class/spectrum_band_db) → met (temperature_c, humidity_pct) → receiver-set (id-sorted; id + `[x,y,z]`). Every f64 is `to_bits().to_le_bytes()`; every sequence u64-LE length-prefixed. **Conditioning (gain/delay/filter/mute) is NOT an input and cannot be — the signature accepts no conditioning type.**

## Next Phase Readiness
- `envi-store` is import-ready for **plan 06-03 (envi-service axum binary)**: project/scene HTTP handlers delegate onto `ProjectStore`; the recondition 409-check binds to `tensor_hash`; the freq-axis DTO reads `envi_engine::freq`.
- The DTO wire shapes (dense-[105] spectra, ProjectMetaDto, ConditioningDto) are frozen contracts Phase 7 (UI) and Phases 9-11 (compute) bind to.
- No blockers.

## Self-Check: PASSED

All 7 created files exist on disk; all 3 task commits (`8cc5eaf`, `673a39c`, `e7f8284`) are present in git history.

---
*Phase: 06-service-foundation-persistence*
*Completed: 2026-07-09*
