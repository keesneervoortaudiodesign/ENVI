# Phase 6: Service Foundation & Persistence - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-09
**Phase:** 6-Service Foundation & Persistence
**Areas discussed:** GDAL/PROJ Windows provisioning, Persistence (SQLite vs flat files), CRS-seam crate boundary, Skeleton contract depth

---

## GDAL/PROJ provisioning → CRS approach (Areas 1 + 3 merged)

The initial question presented three ways to provision the C-linked gdal+proj on Windows (vcpkg /
OSGeo4W / bundled). The user asked **"isn't there an option without C?"** — which reframed the whole
area: Phase 6 needs only coordinate **reprojection** (WGS84↔UTM), not raster I/O, and reprojection has
mature **pure-Rust** implementations. Reframed question:

| Option | Description | Selected |
|--------|-------------|----------|
| Pure-Rust CRS, defer GDAL to Phase 8 | proj4rs/geographiclib-rs for WGS84↔UTM; no C toolchain in Phase 6; envi-gis + gdal + its self-check move to Phase 8 where raster I/O needs them; honors the native-Rust house rule | ✓ |
| Provision GDAL/PROJ C now (original SC2) | vcpkg/OSGeo4W/bundled up front, create envi-gis now, keep the literal GDAL/PROJ startup self-check in Phase 6 | |

**User's choice:** Pure-Rust CRS, defer GDAL to Phase 8.
**Notes:** Decisive rationale = CLAUDE.md's "native Rust preferred over FFI wherever a mature one
exists." SC2's GDAL self-check becomes a pure-Rust CRS round-trip; GDAL Windows provisioning
(vcpkg/OSGeo4W/bundled) is a Phase-8 decision. (The three provisioning options were explained in depth
before the reframe — recorded in `<deferred>` for Phase 8.)

### CRS home (Area 3 close-out)

| Option | Description | Selected |
|--------|-------------|----------|
| Dedicated tiny crate (envi-geo) | pure-Rust crate owning LonLat/SceneXY + reprojection + auto-UTM-zone; imported by store, service, later gis; GEOX-04's one seam | ✓ |
| Module inside envi-store | crs.rs as a module in envi-store, extract later | |

**User's choice:** Dedicated `envi-geo` crate.

---

## Persistence: SQLite vs flat files (Area 2)

| Option | Description | Selected |
|--------|-------------|----------|
| Flat files (JSON/GeoJSON) | project.json + scene.geojson + calc/<id>/manifest.json; git-diffable, inspectable, copyable; fine at scene scale; imports cache separately | ✓ |
| SQLite (rusqlite) | project.db with indexed bbox cols, transactional; the architecture doc's pick; opaque binary, more ceremony | |

**User's choice:** Flat files.
**Notes:** Context that settled it — authored scenes are small (hundreds of features) and bulk imported
GIS data caches separately (Phase 8, DATA-04); the DTO mirror makes a later SQLite swap mechanical, so
flat files don't paint us into a corner. A divergence from the architecture doc's SQLite recommendation,
consciously chosen for skeleton-stage simplicity/inspectability.

---

## Skeleton contract depth (Area 4)

| Option | Description | Selected |
|--------|-------------|----------|
| Full walking-skeleton (stubbed compute) | recondition/recompute endpoints + content-hash identity + real 409 rejection (stub tensor); stub job Queued→Running→Done + live SSE + cancel; single binary serves placeholder web/dist; /meta/freq-axis served once | ✓ |
| Thin contracts (freeze shapes only) | types + routes only; no live job, SSE, or rejection yet | |

**User's choice:** Full walking-skeleton (stubbed compute).
**Notes:** Matches SC4/SC5 verbatim — the contracts are proven end-to-end (with fake compute) so Phase 7
UI and Phase 10/11 compute bind to a tested contract. The full recalc tier router (0–3) stays Phase
10/11; Phase 6 builds only the split + hash identity + rejection.

---

## Claude's Discretion

- Exact pure-Rust reprojection crate (proj4rs vs geographiclib-rs vs utm) — research picks by SC3 ≤1 m
  landmark accuracy + API fit.
- axum module/router layout + /api/v1 versioning; DTO/GeoJSON properties.kind feature schema;
  autosave/reopen-last mechanics; web/dist embedding (ServeDir vs include_dir!); SSE keep-alive details.

## Deferred Ideas

- [ROADMAP COORDINATION] GDAL/PROJ Windows provisioning + raster I/O + the GDAL startup self-check →
  Phase 8; SC2's self-check adjusted to a pure-Rust CRS round-trip in Phase 6.
- SQLite persistence — documented upgrade path (DTO mirror keeps the swap mechanical).
- Full recalc router (Tiers 0–3) — Phase 10/11.
- Real tensor/MAC + propagation behind the endpoints — Phases 9–11.
