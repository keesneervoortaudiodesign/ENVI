# Phase 8: GIS Ingestion & DGM - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-10
**Phase:** 8-GIS Ingestion & DGM
**Areas discussed:** Raster ingestion & C toolchain, Terrain source strategy, Import UX & failures, Check-and-complete editability

---

## Area 1 — Raster ingestion & the C toolchain

The discussion opened on GDAL-vs-pure-Rust and the Phase-6-deferred Windows provisioning, but the
user redirected it into a **deployment-model pivot**: ENVI should be usable from any browser with no
installation, calculations on the local machine, only runnable when logged in to a server. Confirmed
as **WASM client-side compute + a login/delivery server**. This closed the C-toolchain question
(GDAL cannot cross into WASM → pure-Rust is forced, not merely preferred).

### Deployment model (pivotal clarification)
| Option | Description | Selected |
|--------|-------------|----------|
| Hosted service (server does compute) | Rust backend on a server you control; GDAL easy; adds auth/hosting | |
| Fully in-browser WASM (no backend) | Engine → WASM; forces pure-Rust; CORS + browser storage walls | |
| Hybrid (static WASM + thin proxy) | In-browser app + small stateless server for awkward bits | |
| (User's own words) | "calculations on the local machine, no install, only runnable when logged in to server" | ✓ |

**User's choice:** WASM on the user's device + login/delivery server (confirmed). Scope handling:
**proceed with Phase 8 under the new assumption**, flag PROJECT.md/architecture amendments as follow-ups.

### Area 1a — C toolchain direction (resolved by the pivot)
| Option | Description | Selected |
|--------|-------------|----------|
| Pure-Rust first (gate on research) | Zero-C attempt, GDAL fallback per-op | (subsumed) |
| GDAL as architected | Accept C toolchain + Windows provisioning | |
| Hybrid pure-Rust reads, GDAL if forced | Pure-Rust reads, GDAL only for hard ops | |

**Outcome:** moot — WASM rules out C entirely; pure-Rust is mandatory. (Original question was
rejected/reframed by the user before answering.)

**Notes:** Honest maturity assessment given: pure-Rust COG *window reads* for GLO-30/WorldCover
(float32/uint8, DEFLATE/LZW) are feasible on the `tiff`-crate decoder + assembled HTTP-range glue;
the weakest link is DEM warp/resample (no mature pure-Rust `gdalwarp` equivalent) — a research spike
should prove window-read + resample end-to-end on a real GLO-30 tile.

### Area 1b — CORS handling
| Option | Description | Selected |
|--------|-------------|----------|
| Proxy all fetches through login server | Uniform, no CORS surprises, server egress cost | |
| Direct fetch, proxy only as fallback | Direct where CORS allows, proxy blocked sources | ✓ |
| Direct only (CORS-friendly sources only) | No proxy; blocked sources unsupported | |

**User's choice:** Direct browser fetch, proxy via login server as fallback (per-source CORS map).

### Area 1c — DATA-04 cache substrate
| Option | Description | Selected |
|--------|-------------|----------|
| OPFS (Origin Private File System) | File-like, large binary blobs, network-off reads | ✓ |
| Cache API (Service Worker) | Caches HTTP responses by URL/range | |
| IndexedDB | Universal, clunkier for large binary | |

**User's choice:** OPFS, per project.

---

## Area 2 — Terrain source strategy

### Area 2a — Which terrain sources to wire
| Option | Description | Selected |
|--------|-------------|----------|
| AHN (NL) + GLO-30 global fallback | AHN4 DTM preferred in NL, GLO-30 elsewhere, pluggable | ✓ |
| GLO-30 global only; DTM seam stubbed | Only GLO-30 now, registry seam for later | |
| AHN + GLO-30 + multi-country registry now | Invest in clean multi-country registry up front | |

**User's choice:** AHN (NL) + GLO-30 fallback, pluggable registry.

### Area 2b — GLO-30 (surface model) handling
| Option | Description | Selected |
|--------|-------------|----------|
| Flag + badge only (check-and-complete) | Import as-is, badge surface model, user edits | ✓ |
| Flag + drop under-footprint samples | Exclude elevation samples inside building footprints | |
| You decide / defer to research | Let research weigh correctness vs complexity | |

**User's choice:** Flag + badge only; DSM→DTM correction deferred.

---

## Area 3 — The "Import" moment & failure handling

### Area 3a — Import region
| Option | Description | Selected |
|--------|-------------|----------|
| Current map viewport + max-area guardrail | Import what's on screen, guardrail on area/memory | ✓ |
| Explicit drawn import rectangle | User draws bbox deliberately | |
| Viewport default, calc-area refine | Viewport, or calc-area extent if drawn | |

**User's choice:** Viewport + max-area guardrail; layers independently toggleable.

### Area 3b — Partial failure
| Option | Description | Selected |
|--------|-------------|----------|
| Land what succeeded; failed layers retryable | Non-blocking, per-layer status + retry | ✓ |
| All-or-nothing (roll back) | Any failure aborts the whole import | |
| Land partial, flag scene incomplete | Import + persistent incomplete banner | |

**User's choice:** Land what succeeded; failed layers retryable, non-blocking.

**Notes:** Reframe — import progress is in-app client-side state, not the Phase-6 server SSE machine.

---

## Area 4 — "Check & complete" editability

### Area 4a — Re-import behavior
| Option | Description | Selected |
|--------|-------------|----------|
| Merge — never clobber user edits | Add new + refresh untouched; preserve edits (identity + flag) | ✓ |
| Replace within bbox (warn first) | Wipe + re-fetch in bbox; edits lost | |
| Diff & choose | Show added/changed/removed, user picks | |

**User's choice:** Merge — never clobber user edits.

### Area 4b — Buildings primary source
| Option | Description | Selected |
|--------|-------------|----------|
| OSM via Overpass (primary) | JSON, CORS-friendly, WASM-clean; sparser heights → fallback works harder | ✓ |
| Overture GeoParquet (primary) | Denser heights; heavier parquet-in-browser + uncertain CORS | |
| Overture primary, OSM fallback | Best coverage, two fetch paths + reconciliation | |

**User's choice:** OSM via Overpass (primary); Overpass rate limits via proxy if needed.

---

## Claude's Discretion
- WorldCover→Nordtest σ/impedance table *values* (mechanism fixed: reviewed table + per-row test +
  impedance debug overlay; numbers research-owned, user reviews).
- Attribution UI; import-panel layout + per-layer status; max-area guardrail threshold; OPFS
  keying/eviction; the pure-Rust COG-window reader assembly + DEM→UTM resampler; per-source CORS
  capability-map format; "user default" building-height value.

## Deferred Ideas
- **[PROJECT-LEVEL]** PROJECT.md/ARCHITECTURE.md amendment for the WASM/auth pivot; Phase-6
  persistence (project-as-folder → OPFS/sync) and job/SSE machine undercut; Phase-10/11 on-disk
  tensor store + contouring → browser/pure-Rust; new auth system + WASM build target (+ COOP/COEP).
- DSM→DTM flattening / under-footprint sample exclusion.
- Overture GeoParquet buildings (denser heights).
- National DTM sources beyond AHN (pluggable registry additions).
- `ground_zone`→`GroundSegment` (Phase 9), `calc_area`→receiver grid (Phase 10).
