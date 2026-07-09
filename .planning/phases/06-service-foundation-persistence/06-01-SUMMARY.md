---
phase: 06-service-foundation-persistence
plan: 01
subsystem: infra
tags: [crs, proj4rs, utm, reprojection, geo, oracle-fixture, pyproj]

# Dependency graph
requires:
  - phase: 01-engine-foundations
    provides: "SceneError/CrsInfo error-enum + crate-scaffold conventions (envi-engine analogs)"
provides:
  - "envi-geo crate — the milestone's ONE reprojection boundary (GEOX-04)"
  - "LonLat/SceneXY newtypes + GeoError typed errors (the only wire/scene coordinate types)"
  - "ProjectCrs::{for_location, from_zone, to_utm, to_wgs84, label, proj_string} — auto-UTM-zone + transforms"
  - "utm_zone_for plain zone formula (shared with the pyproj oracle generator)"
  - "committed pyproj oracle fixture + tools/crs_oracle/gen_utm.py (house oracle pattern extended to CRS)"
affects: [envi-store, envi-service, envi-gis, scene-persistence, startup-self-check]

# Tech tracking
tech-stack:
  added: ["proj4rs 0.1.10 (pure-Rust PROJ.4/etmerc)", "pyproj 3.7.2 (dev-tool only, oracle generation)"]
  patterns:
    - "One reprojection seam (GEOX-04): proj4rs quarantined to envi-geo; no other crate calls it"
    - "Radian quarantine: degree<->radian conversion lives in transform.rs and ONLY there"
    - "SC3 loud rejection: degree-magnitude scene coords rejected before any transform"
    - "Cross-implementation oracle fixture (pyproj C PROJ vs proj4rs) — committed, no Python at test time"

key-files:
  created:
    - crates/envi-geo/Cargo.toml
    - crates/envi-geo/src/lib.rs
    - crates/envi-geo/src/crs.rs
    - crates/envi-geo/src/transform.rs
    - crates/envi-geo/tests/oracle_utm.rs
    - crates/envi-geo/tests/fixtures/oracle/utm_landmarks.toml
    - tools/crs_oracle/gen_utm.py
  modified:
    - Cargo.lock

key-decisions:
  - "proj4rs 0.1.10 as the pure-Rust CRS crate (zero C toolchain, D-01/D-02); geographiclib-rs/utm rejected"
  - "Norway/Svalbard UTM grid exceptions deliberately skipped (cartographic, not accuracy — plain formula stays <=3deg from central meridian)"
  - "Proj/wgs84 fields kept pub(crate) so transform.rs (sibling module) drives them without a public radian surface"
  - "ProjectCrs Debug hand-implemented (proj4rs Proj is not Debug) — surfaces zone/south/label only"

patterns-established:
  - "envi-geo is the single import point for reprojection across envi-store/envi-service/envi-gis"
  - "GeoError follows the SceneError archetype: Debug+Error+PartialEq, struct variants carrying offending values"
  - "Oracle generator records sha256-of-self provenance; fixture is committed data, Rust test needs no Python"

requirements-completed: [GEOX-04]

# Metrics
duration: 11min
completed: 2026-07-09
status: complete
---

# Phase 6 Plan 01: envi-geo CRS Boundary Summary

**Pure-Rust WGS84<->UTM reprojection seam on proj4rs 0.1.10 — auto-UTM-zone selection, radian-quarantined transforms, SC3 degree-magnitude rejection, pinned to pyproj ground truth to <=1e-3 m.**

## Performance

- **Duration:** 11 min
- **Started:** 2026-07-09T16:56:11Z
- **Completed:** 2026-07-09T17:07:06Z
- **Tasks:** 3
- **Files modified:** 7 created + Cargo.lock

## Accomplishments
- New `envi-geo` crate — the milestone's exactly-one reprojection boundary (GEOX-04), pure Rust, zero C toolchain (D-01/D-02 honored).
- `LonLat`/`SceneXY` newtypes + typed `GeoError`; `ProjectCrs` auto-picks the UTM zone + hemisphere (Dam Square -> zone 31N, Sydney -> 56S) and transforms both directions.
- proj4rs's radians convention fully quarantined inside `transform.rs`; the public API speaks only degrees and meters.
- SC3 loud rejection of degree-magnitude scene coordinates and typed rejection of non-finite/out-of-range inputs (never panics on data).
- pyproj oracle fixture (10 world-spread landmarks, both hemispheres) pins proj4rs `etmerc` to the C PROJ reference to `<=1e-3 m`, discharging the accuracy assumption empirically — with no Python needed at test time.

## Task Commits

Each task was committed atomically:

1. **Task 1: Scaffold envi-geo — newtypes, GeoError, UTM zone selection** - `b798086` (feat)
2. **Task 2: transform.rs — to_utm/to_wgs84 with radian quarantine + SC3 guards** - `324d956` (feat)
3. **Task 3: pyproj oracle — generator, committed fixture, cross-impl test** - `0de2786` (test)

## Files Created/Modified
- `crates/envi-geo/Cargo.toml` - Crate manifest; proj4rs 0.1.10 + thiserror runtime only, serde/toml/approx dev-only, boundary-rule comment, no [lints] table.
- `crates/envi-geo/src/lib.rs` - GEOX-04 boundary header, `#![deny(unsafe_code)]`, `LonLat`/`SceneXY` newtypes, `GeoError` (PartialEq), re-exports.
- `crates/envi-geo/src/crs.rs` - `utm_zone_for` plain zone formula + `ProjectCrs::{for_location, from_zone, label, proj_string}`; Norway/Svalbard deviation documented; hand-rolled Debug.
- `crates/envi-geo/src/transform.rs` - `to_utm`/`to_wgs84` driving proj4rs; radian conversion quarantined here; degree-magnitude + range + non-finite guards; five behavior tests.
- `crates/envi-geo/tests/oracle_utm.rs` - Cross-implementation oracle test asserting proj4rs vs pyproj `<=1e-3 m`, zone agreement, round-trip `<=1e-6 deg`, hemisphere + count truncation guards.
- `crates/envi-geo/tests/fixtures/oracle/utm_landmarks.toml` - Committed pyproj ground-truth fixture (10 landmarks, `tol_m=1e-3`, sha256 provenance).
- `tools/crs_oracle/gen_utm.py` - Operator-driven pyproj oracle generator, sha256-of-self provenance, mirrors `tools/nord2000_oracle/gen_ground_fixtures.py`.

## Decisions Made
- **proj4rs 0.1.10** chosen per research (only mature pure-Rust `utm`/`etmerc` option); satisfies D-01's zero-C-toolchain requirement.
- **Norway/Svalbard zone exceptions skipped** — cartographic conventions, not accuracy requirements; documented in the `crs.rs` `# Deviation` header.
- **`proj`/`wgs84` fields are `pub(crate)`** (not private) so the sibling `transform.rs` module can drive them while keeping radians off the public API — a necessary visibility choice because the plan splits struct (crs.rs) from transforms (transform.rs).
- **`ProjectCrs: Debug` hand-implemented** because `proj4rs::proj::Proj` is not `Debug`; surfaces `utm_zone`/`south`/`label` only.

## Deviations from Plan

None - plan executed exactly as written. All three tasks' actions, verification commands, and acceptance criteria were followed as specified; no auto-fix rules (bugs/missing-critical/blocking) were triggered.

## Issues Encountered
- **pyproj not installed** on the dev machine. Task 3's action explicitly anticipates this ("if pyproj is missing, install it ... dev-machine tool only, never a build dep"). Installed `pyproj 3.7.2` via `pip install pyproj`, ran the generator once, and committed the resulting TOML. pyproj is not a build/test dependency — the Rust oracle test reads only the committed fixture.
- Intermediate note: after Task 1, `proj`/`wgs84` fields produced a transient `dead_code` warning (they are first read by Task 2's transform.rs). Resolved as soon as Task 2 landed; Task 1's gate (`build`/`test`/`tree`) passed regardless, and the final workspace `clippy --all-targets -- -D warnings` is clean.

## Verification Evidence
- `cargo test -p envi-geo`: 9 unit tests + 1 oracle test green (landmark round-trip verified `<=1e-3 m`; oracle agreement `<=1e-3 m` on all 10 landmarks).
- Full workspace `cargo test`: green (FORCE harness untouched, skip-honest).
- `cargo clippy --all-targets -- -D warnings`: clean. `cargo fmt --check`: clean.
- `cargo tree -p envi-geo -e normal --depth 1`: exactly `proj4rs` + `thiserror` (serde/toml stayed dev-only).
- `cargo tree -p envi-engine -e normal --depth 1`: exactly `ndarray`, `num-complex`, `thiserror` (quarantine holds).
- `cargo tree | grep -ci 'proj-sys\|gdal'`: 0 (zero C-linked crates, D-01/D-02).
- `git diff --stat HEAD -- crates/envi-engine/`: empty (engine byte-identical).
- Conj quarantine (`grep '.conj(' propagation/`): 0 (no propagation code touched).

## Threat Flags

No new security surface beyond the plan's `<threat_model>`. All four data-boundary threats (T-06-01-01 degree-magnitude tampering, T-06-01-02 NaN/Inf DoS, T-06-01-03 radians silent corruption, T-06-01-04 etmerc accuracy) are mitigated and unit-/oracle-tested as planned.

## Next Phase Readiness
- `envi-geo` is import-ready for **plan 06-02 (`envi-store`)** — the scene DTO persistence layer reprojects WGS84 GeoJSON <-> project UTM through `ProjectCrs::{to_utm, to_wgs84}` at exactly this seam.
- The D-08 startup self-check (plan 06-03) consumes the same `for_location`/`to_utm`/`to_wgs84` pair verified here.
- No blockers.

## Self-Check: PASSED

All 7 created files exist on disk; all 3 task commits (`b798086`, `324d956`, `0de2786`) are present in git history.

---
*Phase: 06-service-foundation-persistence*
*Completed: 2026-07-09*
