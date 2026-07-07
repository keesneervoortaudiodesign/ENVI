# ENVI — Nord2000 GIS Sound Propagation Model

## What This Is

ENVI is a web-based environmental noise-propagation tool that implements the **full Nord2000** acoustic method from scratch, computing outdoor sound levels over real-world terrain. A Rust backend does the heavy acoustics and GIS math; a JSX frontend lets you position complex directional sound sources and receivers graphically on an OpenStreetMap basemap. It pulls terrain, ground-type, building and open-weather data from public APIs to drive Nord2000's meteorology-dependent refraction model. It is a self-hosted, internal engineering tool — the first maintained open Nord2000 engine (all existing open tools implement CNOSSOS or wrap a closed binary).

## Core Value

A **numerically faithful Nord2000 engine** — validated against the FORCE road-traffic test cases — that produces correct per-band outdoor sound levels over GIS terrain. If the map, the imports, and the UI all failed, a trustworthy propagation calculation must still stand.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Full Nord2000 propagation engine in Rust (direct path, ground effect, screen/diffraction, air absorption), validated against the FORCE test cases
- [ ] Nord2000 meteorology model: log-lin sound-speed profile (A/B/C coefficients), equivalent-linear profile, frequency-dependent ground variant, guarded ray variables (ξ, Δτ)
- [ ] Complex sound sources built from combined directional point sub-sources (per-band L_W + directivity), energy-summed at receivers
- [ ] GIS geometry pipeline: DEM cut-profile extraction along source→receiver lines, ground-impedance segmentation, building/barrier screening
- [ ] Open geospatial data ingestion — terrain (Copernicus GLO-30 + national LiDAR DTM), ground (ESA WorldCover), buildings (Overture/OSM)
- [ ] Meteorology import from open weather APIs (Open-Meteo runtime; ERA5/CDS for weather-class L_den statistics), deriving A/B/C per source→receiver azimuth
- [ ] Web frontend: graphical positioning of sources/receivers/barriers on an OpenStreetMap basemap
- [ ] Receiver-grid computation and isophone (noise contour) map output
- [ ] Self-hosted web service: project persistence + a compute-job model for batch calculations

### Out of Scope

- Redistributing the AV 1106/07 / AV 18xx Nord2000 documents or their text/figures — copyrighted; implement from the equations, cite by report number
- CNOSSOS-EU implementation — Nord2000 is the deliberate choice for faithful refraction/inversion physics; CNOSSOS only if EU-regulatory compliance later becomes a hard requirement
- Multi-user SaaS / accounts / tenant isolation — this is a self-hosted internal tool
- Direct `.skp` (SketchUp binary) parsing — proprietary + OSS-hostile SDK; ingest via glTF/COLLADA export instead
- FABDEM terrain and Meteostat weather — non-commercial licenses avoided even for an internal tool to keep data hygiene clean
- Real-time / streaming calculation — batch compute-job model is sufficient

## Context

- **Origin research:** `c:\Users\keesn\Downloads\nord2000_gis_research.md` (dropped into `docs/research.md`) — canonical references (AV 1106/07 is the implement-from document; FORCE test cases are the validation suite), meteorology comparison vs CNOSSOS, and the A/B/C log-lin profile implementation spec.
- **Landscape survey:** `.planning/research/OPEN-GIS-LANDSCAPE.md` (Fable 5, 2026-07-07) — verified state of open GIS/noise platforms, Rust geo ecosystem, open terrain/ground/building/weather data, and DXF/SketchUp/BEM tooling.
- **Key market fact:** No maintained open-source Nord2000 *engine* exists. NoiseModelling (GPLv3) implements CNOSSOS and is the architectural template (module split: emission / pathfinder / propagation / db-glue) and a cross-validation baseline for shared sub-effects. NMSIM's Nord2000 is a closed Windows binary.
- **Hardest self-build items:** (1) the DEM cut-profile / line-of-sight extractor — no Rust crate exists (design ref = NoiseModelling `pathfinder`, correctness oracle = GRASS `r.profile`); (2) the equivalent-linearization of the sound-speed profile (AV 2005/99) with guarded numerics (f64, Δτ catastrophic-cancellation guard, ξ singularity clamps).

## Constraints

- **Tech stack**: Rust backend (f64 throughout, careful cancellation handling, strong typing for per-band arrays) — mandated by the physics and the user. Frontend in JSX/React.
- **Tech stack**: Rust geo crates — `geo` + `rstar` + `spade` (constrained Delaunay receiver grids) + `gdal` (DEM/GeoPackage) + `proj` (CRS). Frontend — MapLibre GL JS 5 + react-map-gl 8 + Terra Draw; output as server-side isophone fill polygons (GDAL contour), not a heatmap layer.
- **Licensing**: Personal/internal tool. Do not redistribute copyrighted Nord2000 documents. Port NoiseModelling *ideas*, not GPL source. Honor Copernicus/ESA/OSM attribution; avoid non-commercial data (FABDEM, Meteostat).
- **Data**: Global region → Copernicus GLO-30 + ESA WorldCover + Overture as the universal tier, national LiDAR DTM where available. Compute in a local metric CRS per site (auto-pick UTM).
- **Deployment**: Self-hosted web service — project persistence + compute-job model; light/no auth.
- **Validation**: The FORCE road-traffic test suite is the acceptance gate for the engine — stand up the test harness on it *before* writing propagation code.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Implement AV 1106/07 from scratch (not wrap NMSIM, not use CNOSSOS) | User wants a full, modifiable Nord2000 engine; no open one exists; enables future BEM | — Pending |
| Rust backend + JSX frontend | Physics needs f64/careful numerics + strong typing; user-specified | — Pending |
| Milestone 1 = validated core engine passing FORCE cases, no map/UI yet | Risk-first: de-risk the hardest (acoustics numerics) before GIS/UI polish | — Pending |
| Canonical internal geometry = semantic 2.5D scene (CityJSON-aligned) + derived glTF mesh | Nord2000 needs vertical cross-sections from footprints+heights, not triangle soup; keeps DXF/SketchUp/BEM futures open | — Pending |
| Add a per-path-segment `PropagationCorrection` hook + directional multi-sub-source model early | Lets future 2.5D BEM barrier corrections and directivity slot in without touching the core | — Pending |
| Open-Meteo (runtime) + ERA5/CDS (weather classes) for meteorology | Only free/open sources exposing the multi-level winds / stability / u* needed for A & B | — Pending |
| Global data tier: Copernicus GLO-30 + ESA WorldCover + Overture | Chosen "Global" market; national LiDAR layered where available | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-07-07 after initialization*
