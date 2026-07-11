# Phase 9: Path Extraction & Weather - Research

**Researched:** 2026-07-11
**Domain:** GIS geometry pipeline (DEM cut-profile, impedance segmentation, screening edges, CDT receiver grid) + runtime meteorology (Open-Meteo multi-level → per-azimuth A/B/C; ERA5/CDS Obukhov groundwork)
**Confidence:** HIGH (geometry/engine seams — read from source this session); MEDIUM (Open-Meteo/ERA5 external API specifics — CITED from official docs, not exercised); the weather-route A/B/C constants remain `[ASSUMED]` and quarantined from Phase 3.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Weather time = **a single representative hour**; one date+hour → one set of per-azimuth A/B/C. Cache key = `(site, timestamp)`. Named scenarios/windows/downwind-worst-case are Phase 11.
- **D-02:** Open-Meteo product **date-switched**: historical → **Archive** API (ERA5-backed); recent/near-future → **Forecast** API. Same schema/units; client picks endpoint by date.
- **D-03 (carried from Phase-8 pivot):** Weather fetch = **browser `fetch` → OPFS cache**, direct CORS first with login-server byte-proxy fallback — NOT server `reqwest`/SQLite. What-if edits read OPFS only (zero API calls, verified by network log). Supersedes ARCHITECTURE.md's reqwest+SQLite premise.
- **D-04:** ERA5/CDS = build the pure-Rust wind×stability → Obukhov → weather-class **occurrence-statistics derivation** now (tested vs committed fixture, no CDS key at test time) + an **async-job scaffold**. The actual queued CDS retrieval is a thin **FLAGGED-OFF endpoint on the login/delivery (envi-service) server** — the one place server compute is acceptable.
- **D-05:** ERA5 derivation output = **occurrence statistics only** (Obukhov length L + wind/stability class counts/frequencies). Class→A/B/C mapping + energy-weighted L_den stay deferred with **GRID-03**.
- **D-06:** Receiver grid = **user-set spacing, default 10 m, with a minimum guardrail** (guardrail value = Claude/research discretion).
- **D-07:** Building footprints are **constrained-Delaunay holes** — no receivers inside footprints; grid respects footprint edges. Facade/assessment-point receivers deferred.
- **D-08:** **Buildings + walls + barriers all screen** — `building` exterior rings at `eaves_height_m`, `wall`/`barrier` linestrings at `height_m`.
- **D-09:** 3D→2D reduction = **cut-plane ∩ footprint prism → top edges, multi-edge kept** (feed engine multi-edge diffraction; do NOT collapse to one dominant edge). **rstar corridor width = Claude/research discretion.**

### Claude's Discretion
- Cut-profile sampling step + DEM interpolation method (validate vs GRASS `r.profile`).
- rstar corridor width (GEOX-03 screening query + future geometry dirty-diff).
- Min-spacing guardrail value for the receiver grid.
- Azimuth handling for A/B/C: exact per-path azimuth (with caching) vs quantized sectors. Reuse engine Phase-3 Route-1/Route-3 math either way.
- Open-Meteo specifics: which pressure/height levels to request, model selection, unit handling.
- Per-path fan-out strategy for N receivers × M sub-sources (path-cache shape must stay compatible with Phase-11 Tier-2 weather re-solve and Tier-3 geometry dirty-diff).

### Deferred Ideas (OUT OF SCOPE)
- Named weather scenarios + manual overrides + difference maps (Beaufort/wind-dir/downwind toggle/temp gradient/A-B-C edit) → **Phase 11**.
- Full energy-weighted **L_den** weather-class combination → **GRID-03**.
- Facade/assessment-point receivers; reflection-surface geometry extraction; DSM→DTM flattening; forest **Fs** coherence factor (only the geometric forest-crossing seam is even a candidate here, and its placement is a research flag).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| GEOX-01 | Extract DEM cut-profile along S→R line from a DEM raster (oracle: GRASS `r.profile`) | §Pattern 1 (cut-profile extractor) + §Validation (committed r.profile oracle fixture, no GRASS at test time). Produces `envi_engine::scene::TerrainProfile`. |
| GEOX-02 | Segment ground into impedance classes along the profile from land cover + drawn/imported overrides (drawn > imported > default) | §Pattern 2 (impedance segmentation) — reuses `impedance_class` (A–H → σ) + Phase-8 WorldCover→σ table; `GroundSegment` per profile interval; the "row-that-starts-the-segment" convention already in `build_terrain_inputs`. |
| GEOX-03 | Derive screening edges from building/barrier/wall geometry along the path | §Pattern 3 (screening). **Key finding:** the engine has NO separate `screens` field — screen tops are inserted as `(x,z)` vertices into the `TerrainProfile`; `terrain_interpretation` extracts ≤2 diffracting edges. rstar corridor query recommended below. |
| GRID-01 | Building-aware constrained-Delaunay receiver grid (spade) + discrete receiver points | §Pattern 4 (grid) — reuse `envi-dgm` spade CDT with footprint rings as constraint edges/holes; regular lattice clipped to `calc_area` minus footprints; min-spacing guardrail. |
| METX-01 | Import Open-Meteo (multi-level winds/temps), cached per (site, window); what-if edits never call the API; log weighted call cost | §Pattern 5 (weather). Endpoints + variables CITED below; reuse Phase-3 `WeatherComponents`/`profile_for_bearing` + Route-3 `fit_profile`; OPFS cache key `(site,timestamp)`; Playwright network-log guard. |
| METX-02 | Import ERA5/CDS reanalysis → wind×stability weather-class occurrence statistics (Obukhov) — groundwork; full L_den deferred (GRID-03) | §Pattern 6 (ERA5 groundwork). Obukhov recipe CITED (ECMWF); pure-Rust derivation + async-job scaffold; flagged-off CDS endpoint on envi-service; committed fixture test. |
</phase_requirements>

## Summary

Phase 9 is overwhelmingly a **wiring + geometry-reduction** phase, not a new-physics phase. Every acoustic contract it feeds already exists and is validated: `envi_engine::solver::SolveJob` (the frozen `PropagationPath` seam), `TerrainProfile`/`GroundSegment` (the cut-plane type), `SoundSpeedProfile` + the Phase-3 `WeatherComponents`/`profile_for_bearing`/`fit_profile` A/B/C machinery, and the spade CDT (`envi-dgm`). The job is to build the *inputs* to these from the Phase-8 imported scene and from Open-Meteo — all inside the pure-Rust, WASM-safe `envi-gis` crate, obeying the Phase-8 pivot (browser fetch → OPFS; server compute only for the flagged ERA5 job).

The single most load-bearing finding: **the engine has no `screens` field.** The hypothetical `PropagationPath { screens, reflections }` in `ARCHITECTURE.md` was never built — the real `SolveJob` carries only `profile: &TerrainProfile`, and `terrain_interpretation.rs` extracts diffracting edges *from the profile points* (§5.21, ≤2 screens / one thick 2-edge screen). So GEOX-03 does not emit a separate screen list; it **inserts each cut-plane∩wall-top crossing as an `(x,z)` vertex into the `TerrainProfile` with a hard (high-σ) `GroundSegment`**, and the engine does the multi-edge classification. This reshapes the GEOX-03 task and caps "multi-edge" at Nord2000's native ≤2-screen model — a documented limitation, not a bug.

**Primary recommendation:** Add four pure-Rust modules to `envi-gis` (`profile.rs`, `impedance.rs`/segmentation, `screening.rs`, `grid.rs`) plus a `weather.rs` A/B/C-derivation module that **reuses the Phase-3 harness math** (lift `fit_profile` + `WeatherComponents`/`profile_for_bearing` into a WASM-safe location — they are pure math depending only on `envi_engine`). Keep all HTTP in the browser (Open-Meteo direct-CORS) and put the ERA5 async job as a flagged-off `envi-service` endpoint. Validate GEOX-01 against a **committed r.profile oracle fixture** (mirror Phase-8's committed-COG / scipy-oracle pattern; no GRASS at test time). Recommend an **rstar corridor half-width of `max(fixed 20 m, first-Fresnel-zone radius at 250 Hz)`** for candidate-object queries. Assemble `SolveJob`-ready geometry in Phase 9; leave the actual `solve()` fan-out + tensor store to Phase 10.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| DEM cut-profile extraction (GEOX-01) | `envi-gis` (pure-Rust WASM core) | `envi-dgm` (TIN Z-sampling) | Pure math over the imported DGM TIN + DEM raster; runs client-side WASM (Phase-8 pivot). No I/O. |
| Impedance segmentation (GEOX-02) | `envi-gis` | `envi-store` geojson (`ground_zone` read), `envi-gis::impedance_table` (WorldCover→σ) | Geometry ∩ ground_zone polygons + land-cover; emits `GroundSegment`s. Pure-Rust. |
| Screening-edge reduction (GEOX-03) | `envi-gis` (rstar + geo) | `envi-engine::scene::TerrainProfile` (target) | Cut-plane ∩ prism → profile vertices; the engine owns the diffraction classification. |
| Receiver grid (GRID-01) | `envi-gis` / `envi-dgm` (spade) | — | Constrained-Delaunay + lattice clip; pure-Rust, WASM-safe. |
| Open-Meteo fetch (METX-01 transport) | **Browser (`web/src/import`)** | envi-service byte-proxy (CORS fallback only) | Locked by D-03: browser `fetch` → OPFS. NO Rust HTTP on the client path. |
| A/B/C derivation from multi-level profile (METX-01 math) | `envi-gis` (WASM) | reuse `envi_engine` sound-speed + Phase-3 fit | Pure math; runs client-side over the OPFS-cached JSON. |
| ERA5/CDS retrieval (METX-02 transport) | **envi-service** (flagged-off) | reqwest rustls, allowlisted like Phase-8 proxy | CDS is a queued multi-minute job → cannot run client-side; the one acceptable server-compute surface. |
| ERA5 Obukhov/class-stats derivation (METX-02 math) | `envi-gis` or `envi-service` (pure-Rust) | — | Pure derivation; tested against committed fixture. |
| `SolveJob` fan-out + `solve()` + tensor store | **Phase 10** (envi-service + envi-store) | — | Explicitly out of Phase 9; Phase 9 produces the geometry/met inputs only. |

## Standard Stack

### Core (all already vetted / in-tree — no new external acoustics deps)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `envi-engine` (in-tree) | workspace | `SolveJob`, `TerrainProfile`, `GroundSegment`, `SoundSpeedProfile`, `impedance_class`, `sound_speed_ms` | The frozen contract this phase feeds; **byte-identical, no new deps** (3-dep quarantine). |
| `envi-dgm` (in-tree) | workspace | `build_tin` (spade CDT) + `Tin::interpolate_z` | Reused twice: cut-profile Z sampling and the GRID-01 receiver grid. |
| `envi-geo` (in-tree) | workspace | single pure-Rust CRS boundary (`LonLat`/`SceneXY`, proj4rs) | GEOX-04 done; Open-Meteo lon/lat and DEM coords reproject here ONLY (no 2nd proj4rs edge). |
| `spade` | 2.15.x `[VERIFIED: crates.io]` | constrained-Delaunay | Already `envi-dgm`'s dep; best-in-class CDT (OPEN-GIS-LANDSCAPE §2). |
| `geo` | 0.30 (in `envi-gis`; 0.33.1 latest) `[VERIFIED: crates.io]` | line/polygon intersection, point-in-polygon, `Line`/`LineString` clipping | Already `envi-gis` dep; georust org, mature. |

### Supporting (new crate edges to add — pre-vetted in original research)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `rstar` | 0.13.0 `[VERIFIED: crates.io]` | R*-tree spatial index for the GEOX-03 corridor query + future Tier-3 dirty-diff | Add to `envi-gis`. Same georust family as `geo`; the OPEN-GIS-LANDSCAPE-blessed in-process index. Pure-Rust, WASM-safe. |
| `reqwest` (rustls) | workspace | **ONLY** the flagged ERA5 endpoint in `envi-service` (server-side) | METX-02 transport. Allowlisted exactly like the Phase-8 byte proxy. Never on the client path. |

### Reuse, do not re-add (already present)
- `tiff`, `geojson`, `serde`/`serde_json`, `thiserror` — already `envi-gis` deps.
- `wasm-bindgen =0.2.126`, `serde-wasm-bindgen`, `ts-rs` — `envi-gis-wasm` boundary; new DTOs go through the same ts-rs no-drift generation.

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| self-built cut-profile | GRASS `r.profile` at runtime | Rejected: adds a C/Python runtime dep on the WASM client path (impossible). GRASS stays the **offline oracle** only. |
| `rstar` corridor query | brute-force O(N) scan per path | Fine for tiny scenes, but Tier-3 dirty-diff (ARCHITECTURE) needs the index anyway; add it once. |
| lift Phase-3 `fit_profile` into `envi-gis` | duplicate the LSQ fit | Reuse the **exact** algorithm (byte-identical logic) so the round-trip oracle still covers it; do not fork the math. |

**Installation (Cargo edits, no shell install):**
```toml
# crates/envi-gis/Cargo.toml  → [dependencies]
rstar = "0.13"
# crates/envi-service/Cargo.toml → [dependencies] (behind a `era5` feature flag, default-off)
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }
```

**Version verification:** `rstar 0.13.0`, `spade 2.15.1`, `geo 0.33.1` confirmed on crates.io this session (`cargo search`). `envi-gis` currently pins `geo = "0.30"` — keep it unless a 0.33 API is needed (minimize churn).

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| `rstar` | crates.io | mature (georust) | very high | github.com/georust/rstar | OK | Approved (add to envi-gis) |
| `spade` | crates.io | mature | high | github.com/Stoeoef/spade | OK | Already in `envi-dgm` |
| `geo` | crates.io | mature (georust) | very high | github.com/georust/geo | OK | Already in `envi-gis` |
| `reqwest` | crates.io | mature | very high | github.com/seanmonstar/reqwest | OK | Approved (envi-service, feature-gated) |

**Packages removed due to [SLOP] verdict:** none.
**Packages flagged as suspicious [SUS]:** none. All four are long-established, source-backed, and already sanctioned by the original OPEN-GIS-LANDSCAPE research + the Phase-8 stack. No `npm`/PyPI packages are added (Open-Meteo is called via the browser `fetch` already wired in Phase 8; no new JS dep).

## Architecture Patterns

### System Architecture Diagram

```
                          ┌─────────────────────────── web/ (browser) ───────────────────────────┐
  date+hour picker ─────► weather import: fetch(Open-Meteo Archive|Forecast)  ──direct CORS──►  Open-Meteo
        │                        │  (fallback: envi-service byte proxy)                              │
        │                        ▼                                                                   │
        │                  OPFS cache  (key = site,timestamp)  ◄── what-if reads ONLY (0 API calls) ─┘
        │                        │
        ▼                        ▼
  scene (OPFS: ground_zone, building, wall, calc_area, DGM TIN)      multi-level JSON (u,dir,T,gph per level)
        │                        │                                          │
        └────────────┬───────────┴──────────────────────────────┐          │
                     ▼ (wasm-bindgen boundary: envi-gis-wasm)    │          ▼ (envi-gis::weather, WASM)
   ┌─────────────────────────── envi-gis (pure-Rust WASM core) ──┼──────────────────────────────────┐
   │  profile.rs   : S→R line → sample DGM TIN → (x,z) points ───┼──► TerrainProfile points          │
   │  impedance.rs : profile ∩ ground_zone/landcover → σ segs ───┼──► GroundSegment[]  (drawn>imp>def)│
   │  screening.rs : rstar corridor → cut-plane ∩ prism tops ────┼──► inject (x,z) hard vertices     │
   │  grid.rs      : spade CDT (footprints=holes) + lattice clip ┼──► receiver positions [x,y,z]     │
   │  weather.rs   : c_eff(z)=20.05√(T+273.15)+u·cosΔ → fit A,B,C ┼──► WeatherComponents (per-azimuth)│
   └─────────────────────────────────────────────────────────────┼──────────────────────────────────┘
                     │                                            │
                     ▼  (Phase 9 output = the SolveJob INPUTS)    │
         TerrainProfile + src/rcv + SoundSpeedProfile(per az) + CoherenceInputs
                     │
                     ▼  ── Phase 10 ──► solve() fan-out (N rcv × M sub) → chunked tensor store

  ERA5 (METX-02 groundwork, server): envi-service [feature=era5, DEFAULT OFF]
     POST /era5/import → async job (Queued→Running→Done) → cdsapi-style queued retrieval (FLAGGED OFF)
     → pure-Rust: (iews,inss)→u*, ishf→heat flux → 1/L Obukhov → wind×stability class occurrence stats
     (class→A/B/C + L_den DEFERRED to GRID-03)
```

### Recommended Project Structure
```
crates/envi-gis/src/
├── profile.rs        # GEOX-01: S→R cut-profile extractor (samples the DGM TIN)
├── impedance.rs      # GEOX-02: impedance segmentation (drawn>imported>default)
├── screening.rs      # GEOX-03: rstar corridor + cut-plane∩prism → profile vertices
├── grid.rs           # GRID-01: spade CDT receiver grid (footprints as holes) + points
├── weather.rs        # METX-01: multi-level profile → per-azimuth WeatherComponents (A,B,C)
└── era5.rs           # METX-02: Obukhov + wind×stability class-occurrence derivation (pure)
crates/envi-gis-wasm/src/lib.rs   # + thin #[wasm_bindgen] shims + ts-rs DTOs for the above
crates/envi-service/src/api/      # + era5.rs: flagged-off async CDS retrieval endpoint
web/src/import/                    # + weather.ts (fetchers), OPFS weather cache
web/src/panels/                    # + WeatherPanel.tsx (date+hour → import → per-azimuth A/B/C)
```

### Pattern 1: DEM cut-profile extractor (GEOX-01)
**What:** Sample terrain elevation at regular intervals along the S→R line, producing the `(x, z)` points of a `TerrainProfile` where `x` is horizontal distance from the source and `z` is ground elevation.
**How (recommended):** Sample the **already-built DGM TIN** (`envi-dgm::Tin::interpolate_z`) — the scene's ground model is a TIN, so barycentric TIN interpolation is the natural, exact-on-the-mesh sampler and avoids re-deciding a raster resampling kernel. Match GRASS `r.profile` by choosing the **sampling step = DEM cell size** (`r.profile` walks the line at the raster resolution and reads the cell value; a matched step + bilinear-on-raster reproduces it). Because ENVI samples the TIN (not the raw raster), the oracle test tolerates a small documented delta (TIN barycentric vs raster bilinear) — pin the tolerance, don't chase bit-equality.
**`TerrainProfile` mapping (load-bearing — verified from `scene.rs`):**
- `points: Vec<[f64;2]>` are `(x, z)` with **strictly ascending x** (constructor rejects non-ascending/duplicate x → dedupe collinear samples).
- `x = 0` at the source ground point; increases toward the receiver.
- **Source/receiver heights are NOT profile points** — `TerrainProfile::endpoints(h_s, h_r)` places the source above the **first** point and the receiver above the **last** (the hSv/hRv trap). So the extractor emits ground `z` only; the acoustic src/rcv heights are added later.
- `N` points ⇒ `N-1` `GroundSegment`s (GEOX-02 fills these).

**Example:**
```rust
// envi-gis/src/profile.rs  (Source: derived from envi-dgm::Tin + envi-engine::scene::TerrainProfile)
pub fn cut_profile(tin: &Tin, s_xy: [f64;2], r_xy: [f64;2], step_m: f64)
    -> Result<Vec<[f64;2]>, GisError>
{
    let d = ((r_xy[0]-s_xy[0]).powi(2) + (r_xy[1]-s_xy[1]).powi(2)).sqrt();
    let n = (d / step_m).ceil().max(1.0) as usize;
    let mut pts = Vec::with_capacity(n+1);
    let mut last_x = f64::NEG_INFINITY;
    for i in 0..=n {
        let t = (i as f64) / (n as f64);
        let x = t * d;                             // horizontal distance along the line
        let px = s_xy[0] + t*(r_xy[0]-s_xy[0]);
        let py = s_xy[1] + t*(r_xy[1]-s_xy[1]);
        let z = tin.interpolate_z(px, py).ok_or(GisError::OutsideHull)?; // never silent 0.0
        if x > last_x + 1e-9 { pts.push([x, z]); last_x = x; }           // enforce strictly-ascending x
    }
    Ok(pts)
}
```

### Pattern 2: Impedance segmentation (GEOX-02)
**What:** Assign each profile interval a `GroundSegment { flow_resistivity, roughness }` by resolving **drawn > imported > default** at the interval.
**How:** For each consecutive `(x_i, x_{i+1})` interval, take a representative planar point (e.g. midpoint mapped back to `(x,y)`), then resolve in priority order: (1) is it inside a user-**drawn** `ground_zone` polygon? use its impedance class (`impedance_class(A..H)` → σ); (2) else inside an **imported** land-cover `ground_zone` (Phase-8 WorldCover→σ vectorization)? use that; (3) else the project **default** ground. Insert a profile vertex wherever a segment boundary crosses the line so segment edges land on real polygon boundaries (compute polygon-edge ∩ S→R-line crossings and splice them into the ascending-x point list before building `GroundSegment`s).
**Reuse:** `envi_engine::scene::impedance_class` (A–H → σ, class B = 31.5 corrected), `envi-gis::impedance_table` (Phase-8 WorldCover→σ, unit-tested per row), and the "row that starts the segment" convention from `build_terrain_inputs` (segment `i` carries the properties at `points[i]`, i.e. `w[0]`).
**Anti-pattern:** sampling impedance only at profile *points* rather than *intervals* — the `GroundSegment` belongs to the span, not the vertex; off-by-one here silently shifts every ground reflection.

### Pattern 3: Screening-edge reduction (GEOX-03) — the reshaped task
**What (corrected from ARCHITECTURE.md):** The engine `SolveJob` has **no `screens`/`reflections` field** — only `profile: &TerrainProfile`. `terrain_interpretation.rs` (§5.21) *derives* diffracting edges from the profile's interior points above the S–R line, classifying into Sub-model 4 (one edge), 5 (thick screen, two edges of one body), or 6 (two screens). **So GEOX-03 inserts screen tops as `(x,z)` vertices into the same `TerrainProfile`, not into a separate list.**
**How:**
1. **rstar corridor query:** index all `building`/`wall`/`barrier` geometries in an `rstar` R-tree keyed by AABB. Query the corridor around the S→R line (see corridor-width recommendation below) to get candidate objects — avoids testing every object per path.
2. **Cut-plane ∩ footprint prism (D-09):** the cut plane is the vertical plane through S→R. For a **building**, intersect the S→R *line* (plan view) with the footprint polygon exterior ring → entry/exit `x` crossings; each crossing gets a profile vertex at the building's `eaves_height_m` (the prism top). For a **wall/barrier** linestring, intersect the S→R line with each segment → one crossing at `height_m`. A building the line passes *through* yields two top vertices (entry + exit walls) = a **thick screen** the engine reads as Sub-model 5.
3. **Height convention:** the injected `z` is `ground_z(crossing) + object_height` (eaves/`height_m`), so screen tops ride on terrain. Ground under a building should be the footprint-boundary elevation (Phase-8 base-elevation rule).
4. **Impedance of screen segments:** the intervals spanning a wall/building are **hard** (high σ, e.g. class H = 200000) so the reflection over the screen face behaves; keep roughness 0.
5. **Multi-edge ceiling (document as a limitation):** Nord2000's terrain interpretation caps at **two screens / one thick screen**. "Multi-edge kept" (D-09) means *do not pre-collapse to a single edge* — insert all crossing tops and let the engine pick the primary edge (largest `Δℓ₀`) and classify; if a path crosses >2 screens, Nord2000 itself reduces to ≤2. This is standard-conformant, not a defect — flag it in the module doc and a test.

**Corridor-width recommendation (Claude's discretion → resolved):** use a half-width `w = max(20 m, r_F1)` where `r_F1 = sqrt(λ·d1·d2/(d1+d2))` is the **first-Fresnel-zone radius** evaluated at a low reference frequency (≈250 Hz, λ≈1.36 m) at the path midpoint (`d1=d2=d/2`). Rationale: objects outside the first Fresnel zone contribute negligibly to diffraction/reflection; a 20 m floor covers short paths where the Fresnel radius is tiny but a nearby wall still screens. This same corridor is the Tier-3 dirty-diff query in Phase 11, so size it once. (Confidence: MEDIUM — the *principle* is standard acoustics; the exact constants are a defensible engineering choice, not a spec value → tag `[ASSUMED]` and make it a named `const`.)

### Pattern 4: Building-aware receiver grid (GRID-01)
**What:** A regular lattice of receivers at user spacing inside `calc_area`, excluding building footprints, plus explicit discrete receiver points.
**How (recommended, simplest correct):**
1. Generate the axis-aligned lattice covering the `calc_area` bbox at `spacing_m` (default 10 m; **guardrail: reject `spacing_m < 1.0 m`** — at 1 m a 1 km² area is 10⁶ receivers × M sub-sources, already at the DoS edge; see guardrail note).
2. Keep a lattice point iff it is **inside `calc_area`** (geo point-in-polygon) **and outside every building footprint** (D-07 "no receivers inside footprints").
3. Add explicit receiver points from the scene.
4. Sample each kept point's `z` from the DGM TIN and add the receiver height (default 4 m or per project).
**Why not full CDT-mesh-vertices-as-receivers:** D-07 says footprints are constrained-Delaunay **holes** and the grid "respects footprint edges." The clean interpretation: build a spade CDT with `calc_area` boundary + footprint rings as **constraint edges** to *define the valid region*, but emit receivers on the **regular lattice clipped to that region** (predictable spacing for a noise map) rather than irregular triangle vertices. Constrained-Delaunay guarantees no receiver straddles a footprint edge. If the planner prefers literal CDT vertices (adaptive density), spade supports it, but a regular lattice is what noise maps expect and what Phase-10 chunking assumes. **Decide this explicitly in planning.**
**Guardrail (Claude's discretion → resolved):** minimum spacing **1.0 m**, and additionally a hard **receiver-count cap** (mirror `envi-dgm::MAX_POINTS`-style DoS bound, e.g. 1_000_000) returning a typed error — Phase-10 cost estimation (halving spacing quadruples cost) is the UX layer, but the geometry builder must not OOM the browser. (Confidence: MEDIUM — value is engineering judgment; make it a named `const`.)
**Reuse:** `envi-dgm::build_tin` already rejects intersecting constraints (footprint rings that cross → typed error, no panic) and caps input size — the exact safety posture GRID-01 needs.

### Pattern 5: Open-Meteo → per-azimuth A/B/C (METX-01)
**Endpoints (D-02 date-switch) `[CITED: open-meteo.com/en/docs]`:**
- Forecast (recent/near-future, incl. `past_days`): `https://api.open-meteo.com/v1/forecast`
- Archive (historical, ERA5-backed reanalysis): `https://archive-api.open-meteo.com/v1/archive`
- Same schema/units; the client selects by whether the requested date is within the forecast window.
**Variables to request `[CITED: open-meteo.com]`:** pressure-level set `temperature_{L}hPa`, `wind_speed_{L}hPa`, `wind_direction_{L}hPa`, `geopotential_height_{L}hPa` for `L ∈ {1000, 975, 950, 925, 900, 850}` (surface→~1.5 km is where the sound-relevant profile lives), **plus** height-above-ground winds `wind_speed_10m/80m/120m/180m` + `wind_direction_10m/...` + `temperature_2m`. Geopotential height is **AMSL, not AGL** — subtract the site's surface geopotential/elevation to get height above ground before fitting. Request `&hourly=...` for the single chosen hour (D-01) with `&start_date=&end_date=` (archive) or `&start_hour=&end_hour=` and pick the timestamp.
**Deriving A/B/C (reuse Phase-3 math exactly):** for each level, compute the effective sound speed `c_eff(z) = 20.05·√(T(z)+273.15) + u(z)·cos(azimuth − φ_wind)` — the **same formula already in `route3::reconstruct_profiles`** — then LSQ-fit `c_eff(z) ≈ A·ln(z/z₀+1) + B·z + C` with the **same `route3::fit_profile`** 3×3 Cramer solver. But Open-Meteo gives the profile *directly* (multi-level u, T), so **skip the Monin–Obukhov reconstruction** (that is Route-3's job when only surface met is available) — this is the **Route-2 "profile fit"** path (OPEN-GIS §4). Decompose into `WeatherComponents { a_temp, a_wind, b, c }` and use `profile_for_bearing(base, bearing, φ_wind)` for per-azimuth A. Since `A` depends on wind projection onto the path azimuth, compute the isotropic parts (`a_temp`, `b`, `c`) **once** and project `a_wind·cos(Δ)` per distinct path azimuth (exact, cheap — no need to sector-quantize; caching is by azimuth key).
**⚠ [ASSUMED] quarantine carried from Phase 3:** the fitted `A/B/C` inherit the Phase-3 `[ASSUMED]` weather-route status (03-03 Open-Q1). METX-01 must **not** promote them to a false FORCE numeric pass — validate structurally (downwind A > upwind A; inversion ⇒ B>0; round-trip fit identity), exactly as Phase 3 does.
**Where the math runs:** `envi-gis::weather` (pure-Rust, WASM) over the OPFS-cached JSON — **not** in `web/` (TS does no acoustic math, per the wire contract) and **not** server-side.
**OPFS cache + zero-API-on-what-if (SC4):** cache key = `(lat, lon, timestamp)` (D-01), stored in OPFS alongside the Phase-8 GIS cache. What-if edits read OPFS only. Enforce with the Phase-8 pattern: Playwright `page.route('**/api.open-meteo.com/**', ...)` + `archive-api` mock counts calls; a what-if journey asserts **zero** new matches.
**Call-cost logging (SC4):** Open-Meteo weights calls by variable count × time steps ("API call weight"). Log an estimated weight per fetch (e.g. `n_variables × n_timesteps / 100`) to a console/metric line so the 10k/day non-commercial budget is visible.
**CORS:** Open-Meteo is a browser-first API and serves permissive CORS (direct `fetch` from the client works); the login-server byte-proxy is the fallback for network-restricted deployments, reusing the Phase-8 allowlisted proxy. (Confidence: MEDIUM — CORS support is documented behavior/widely relied upon but the docs page did not restate the header this session; verify with one live OPTIONS/GET during implementation.)

### Pattern 6: ERA5/CDS Obukhov groundwork (METX-02)
**Why server-side (D-04):** CDS is a **queued, multi-minute async retrieval** (users report requests "queued indefinitely") and requires a `.cdsapirc` key — it cannot run client-side. So it lives as a **flagged-off** async endpoint on `envi-service` (the login/delivery server that already proxies bytes). Build the **async-job scaffold** now (Queued→Running→Done state machine — reuse the Phase-6 `jobs.rs`), the retrieval stub behind `#[cfg(feature = "era5")]` default-off, and the **pure-Rust derivation** tested against a committed fixture (no CDS key at test time).
**Obukhov recipe `[CITED: confluence.ecmwf.int/…/ERA5: How to calculate Obukhov Length]`:** from ERA5 single-level hourly fields:
- friction velocity `u* = ((iews² + inss²)/ρ)^{1/4}` from eastward/northward turbulent surface stress `iews`,`inss` (divide by air density);
- turbulent temperature scale `θ* = −ishf / (ρ·c_p·u*)` from instantaneous surface sensible heat flux `ishf`;
- `1/L = (κ·g·θ*) / (T_v·u*²)` with κ=0.4, g=9.81, virtual temperature `T_v` from `2t`,`2d`,`sp`.
- Sign convention: downward fluxes positive ⇒ daytime `ishf` typically negative ⇒ unstable (`1/L<0`); night stable (`1/L>0`).
- Quality gate: `sdfor < 50 m` marks reliable (low sub-grid orography).
**Output (D-05):** **occurrence statistics only** — bin each hour into a wind-speed × stability(1/L sign+magnitude) class and count/frequency over the retrieved period → a class-occurrence table. **Do NOT** map classes to A/B/C or combine into L_den (both deferred to GRID-03). The bin edges are `[ASSUMED]` (Nord2000 doesn't define ERA5→class) — validate the *binning/counting* against a synthetic fixture, never a numeric weather pass.

### Anti-Patterns to Avoid
- **Emitting a separate `screens: Vec<...>` for GEOX-03.** The engine has no such field — inject into `TerrainProfile`. (This is the single biggest reshape vs ARCHITECTURE.md.)
- **Putting acoustic/A-B-C math in `web/` (TypeScript).** The wire contract forbids client-side acoustic arithmetic; the fit runs in `envi-gis` WASM.
- **Adding a Rust HTTP client on the weather *client* path.** D-03: browser fetch only. Only the flagged ERA5 endpoint uses `reqwest`, server-side.
- **Adding any dep to `envi-engine`.** The 3-dep quarantine is byte-identical; all new code is in `envi-gis`/`envi-service`/`web`.
- **Promoting `[ASSUMED]` weather constants to a numeric FORCE pass.** Structural/property/round-trip tests only.
- **Sampling geopotential height as height-above-ground.** It is AMSL — subtract site elevation first.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Constrained triangulation / receiver grid | a Delaunay from scratch | `envi-dgm::build_tin` (spade) | Already panic-proof, DoS-capped, intersecting-constraint-guarded. |
| TIN Z lookup along the profile | a raster resampler | `envi-dgm::Tin::interpolate_z` | Barycentric, returns `None` outside hull (never silent 0.0). |
| Spatial candidate query for screening | O(N) per-path loop | `rstar` R*-tree | The blessed in-process index; reused for Tier-3 dirty-diff. |
| Line/polygon intersection, point-in-polygon | hand geometry | `geo` (`Line`, `LineString`, `Contains`) | Already a dep; robust predicates. |
| A/B/C LSQ fit + `c_eff(z)` | a new solver | Phase-3 `route3::fit_profile` + `reconstruct`/`profile_for_bearing` | Validated by the round-trip oracle; forking risks divergence. |
| Per-azimuth wind projection | ad-hoc trig | `weather::profile_for_bearing` + `WeatherComponents` | Encodes MET-02 (temp once, wind per bearing) correctly. |
| Impedance class → σ | inline magic numbers | `impedance_class` + `envi-gis::impedance_table` | Class B=31.5 correction + per-row-tested WorldCover table. |
| CRS reprojection | a 2nd proj4rs edge | `envi-geo` only | GEOX-04 = exactly one reprojection boundary. |
| Async job state machine (ERA5) | a new one | Phase-6 `envi-service::jobs` | Queued/Running/Done/Failed/Cancelled already exists + SSE. |
| L_den / energy-weighting (if tempted) | anything | **defer** — GRID-03 | Explicitly out of scope this phase. |

**Key insight:** Phase 9 adds essentially *zero* new numerical kernels. Every hard number (diffraction, ground, refraction, LSQ) is validated upstream; the risk is entirely in **geometry reduction correctness** (profile ascending-x, screen-into-profile, footprint holes) and **plumbing discipline** (OPFS cache, WASM boundary, quarantines).

## Runtime State Inventory

Not a rename/refactor/migration phase — greenfield modules added to existing crates. No stored data keys, live-service config, OS registrations, secret-name couplings, or build artifacts are being renamed. **None found — verified: Phase 9 only adds new files/modules and new (default-off) endpoints; it does not rename or migrate any existing identifier, cache key, or stored record.** The one new persisted artifact is the OPFS weather cache (new key namespace, no migration of existing data).

## Common Pitfalls

### Pitfall 1: Treating screens as a separate `SolveJob` field
**What goes wrong:** planning tasks around a `PropagationPath { screens, reflections }` that doesn't exist; the geometry never reaches the diffraction code.
**Why:** `ARCHITECTURE.md` sketched a hypothetical struct; the *real* `solver.rs::SolveJob` carries only `profile`. Verified this session.
**Avoid:** GEOX-03 inserts `(x,z)` vertices into `TerrainProfile`; `terrain_interpretation` does the rest.
**Warning sign:** any task output typed as `Vec<ScreenEdge>` handed to the solver.

### Pitfall 2: The hSv/hRv height trap
**What goes wrong:** adding source/receiver acoustic height as profile z-values, shifting geometry by metres.
**Why:** `TerrainProfile::endpoints(h_s,h_r)` measures `h_s` above the **first** point and `h_r` above the **last** — heights are NOT profile points.
**Avoid:** the cut-profile emits **ground z only**; heights are applied at `SolveJob` assembly.
**Warning sign:** the first/last profile z equals a src/rcv absolute height.

### Pitfall 3: Non-ascending / duplicate profile x
**What goes wrong:** `TerrainProfile::new` returns `NonAscendingX`/`EmptyProfile`, or worse a task assumes it silently sorts.
**Why:** the constructor requires **strictly ascending x** and `N-1` segments for `N` points.
**Avoid:** dedupe colinear samples and splice segment-boundary crossings while keeping x strictly increasing; count segments = points−1.

### Pitfall 4: Multi-edge means ≤2 screens
**What goes wrong:** expecting arbitrary N-edge diffraction from many buildings on a path.
**Why:** Nord2000 §5.21 classifies into Sub-model 4/5/6 = at most two screens (or one thick two-edge screen).
**Avoid:** insert all crossings, document that the engine reduces to ≤2; don't build a bespoke N-edge combiner.

### Pitfall 5: Geopotential height as AGL + compare-by-band, not Hz
**What goes wrong:** wrong profile heights (AMSL vs AGL) → wrong A/B; and the recurring nominal-Hz-vs-band-index trap when the weather output eventually meets the 105-grid.
**Avoid:** subtract site elevation from `geopotential_height`; keep all engine-facing spectra indexed by band.

### Pitfall 6: A what-if edit that silently re-fetches
**What goes wrong:** SC4 fails — a slider change triggers an Open-Meteo call.
**Why:** cache read path not wired; or cache key too specific (re-keys on view state).
**Avoid:** key strictly `(lat,lon,timestamp)`; all derivation reads OPFS; assert zero network via Playwright `page.route`.

### Pitfall 7: Leaking a Rust HTTP client onto the WASM path
**What goes wrong:** `envi-gis` picks up `reqwest`/tokio → breaks the WASM-safe, no-async invariant (Phase-8 `cargo tree` gate).
**Avoid:** `reqwest` only in `envi-service` behind `feature=era5`; `envi-gis` stays math over bytes/JSON strings.

## Code Examples

### Reuse the Phase-3 fit for Open-Meteo multi-level (METX-01)
```rust
// envi-gis/src/weather.rs  (Source: crates/envi-harness/src/weather/route3.rs fit_profile + mod.rs WeatherComponents)
// Open-Meteo gives u(z),dir(z),T(z) DIRECTLY → skip Monin–Obukhov reconstruction (that's Route 3);
// this is Route 2 "profile fit".
fn components_from_levels(levels: &[Level], phi_wind_deg: f64, z0: f64) -> WeatherComponents {
    // c_eff split: temperature part (isotropic) fit once; wind part fit as a magnitude.
    // Fit A_temp,B,C from the TEMPERATURE-only sound speed, and A_wind from the wind speed profile,
    // both via the SAME 3×3 LSQ (route3::fit_profile), so the round-trip oracle still covers it.
    let heights: Vec<f64> = levels.iter().map(|l| l.height_agl_m).collect();      // AMSL−elevation
    let c_temp: Vec<f64>  = levels.iter().map(|l| sound_speed_ms(l.t_c)).collect();
    let (a_temp, b, c)    = fit_profile(&heights, &c_temp, z0).unwrap();
    let u_along: Vec<f64> = levels.iter().map(|l| l.wind_speed).collect();         // magnitude
    let (a_wind, _, _)    = fit_profile(&heights, &u_along, z0).unwrap();
    WeatherComponents { a_temp, a_wind, b, c, s_a: 0.0, s_b: 0.0, z0 }
}
// per path azimuth (exact, cheap): profile_for_bearing(&comp, path_az_deg, phi_wind_deg)
```
*(Exact split of temperature vs wind into the log-lin basis is an implementation choice — validate by the round-trip identity + downwind>upwind property, keep `[ASSUMED]`.)*

### rstar corridor query for screening candidates (GEOX-03)
```rust
// envi-gis/src/screening.rs  (Source: rstar 0.13 AABB envelope API + geo intersection)
let tree: RTree<GeomEntry> = RTree::bulk_load(entries);        // buildings+walls+barriers, AABB-keyed
let half_w = fresnel_half_width(d, /*f_ref*/250.0).max(20.0);  // [ASSUMED] const, documented
let bbox = corridor_aabb(s_xy, r_xy, half_w);
for cand in tree.locate_in_envelope_intersecting(&bbox) {
    // cut-plane ∩ footprint prism → (x, z_top) crossings inserted into the profile point list
}
```

## State of the Art

| Old Approach (ARCHITECTURE.md, pre-pivot) | Current Approach (Phase-8/9) | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `envi-gis` = C-linked (gdal/proj) + `reqwest` + disk cache | pure-Rust, WASM-safe, browser fetch → OPFS | Phase 8 pivot (08-CONTEXT D-01/02/03) | No GDAL/PROJ; cut-profile samples the TIN not a `/vsicurl` COG; weather via browser. |
| `envi-store` SQLite weather table | OPFS-cached JSON, keyed `(site,timestamp)` | Phase 8 pivot | No server weather DB; what-if reads OPFS. |
| `PropagationPath { screens, reflections }` | `SolveJob { profile, … }` only; screens → profile vertices | engine Phase 4 (frozen) | GEOX-03 reshaped; ≤2-screen cap. |
| Route 3 (Monin–Obukhov reconstruct) for all weather | Route 2 (direct profile fit) for Open-Meteo multi-level | this phase | Open-Meteo gives the profile → skip reconstruction; ERA5 surface-only path still uses the MO recipe. |

**Deprecated/outdated for this phase:** the ARCHITECTURE.md `acquire/dem.rs` `/vsicurl` COG windowing and `contour.rs` GDAL isobands (Phase 11 uses pure-Rust `contour`); the `envi-store::db.rs` weather/rusqlite table.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | rstar corridor half-width `max(20 m, first-Fresnel@250Hz)` is adequate for candidate screening | Pattern 3 | Too narrow → misses a screening wall (under-predicts attenuation); too wide → slower query only. Tune with a property test; make it a named const. |
| A2 | Min receiver spacing 1.0 m + count cap ~1e6 is the right guardrail | Pattern 4 | Too permissive → browser OOM at Phase-10 solve; too strict → rejects a legitimate fine grid. UX cost-estimate (Phase 10) is the real gate. |
| A3 | Sampling the DGM TIN (not the raw DEM raster) still matches `r.profile` within a documented tolerance | Pattern 1 / Validation | If the delta is large, the oracle test needs a raster-bilinear sampler instead — decide before writing the oracle fixture. |
| A4 | Open-Meteo serves permissive CORS for direct browser fetch | Pattern 5 | If not, the login-server proxy is mandatory (already the fallback) — verify with one live request. |
| A5 | The temperature/wind split into the log-lin A basis is acceptable (inherits Phase-3 `[ASSUMED]`) | Pattern 5 / Code | Only a numeric-accuracy concern; quarantined from FORCE pass — structural tests bound it. |
| A6 | ERA5 wind×stability class bin edges (for occurrence stats) | Pattern 6 | `[ASSUMED]`; only the counting is tested, class→A/B/C is GRID-03. Low risk this phase. |
| A7 | Pressure levels 1000–850 hPa + 10/80/120/180 m winds capture the sound-relevant profile | Pattern 5 | Missing higher levels only matters for very tall geometry / long range; extend the level list if needed. |

**If this table drove a decision, discuss-phase already ran — these are researcher defaults the planner should keep as named constants and the reviewer should sanity-check, not silent facts.**

## Open Questions

1. **Does `SolveJob` fan-out (producing `Vec<SolveJob>` and calling `solve()`) land in Phase 9 or Phase 10?**
   - What we know: ROADMAP Phase 9 says "feed real-world PropagationPaths"; Phase 10 is "wiring the promoted engine solver + chunked tensor store." CONTEXT says Phase 9 "builds its inputs; it does not redesign the struct." No production `SolveJob` construction exists yet (only tests).
   - Recommendation: **Phase 9 produces the geometry+met inputs** (TerrainProfile with segments+screen vertices, per-azimuth `SoundSpeedProfile`/`WeatherComponents`, receiver positions, `CoherenceInputs`) and a **path-assembly function** + path-cache shape; **Phase 10 owns the fan-out loop, `solve()`, and the tensor sink.** Design the path-cache key now (`hash(geometry features ∩ corridor)` per ARCHITECTURE Tier-3).

2. **Also unresolved: was `compute_tensor(&Scene,&MetInputs,&[PropagationPath])` ever built?**
   - What we know: `solver.rs` has `SolveJob`/`solve()` but **no** `compute_tensor` and **no** `PropagationPath` type — the "promote-the-solver" refactor (ARCHITECTURE) was only partially done in Phase 4 (the streaming `solve()` exists; the `Scene`-level entry does not).
   - Recommendation: Phase 9/10 build a thin `Scene + met → Vec<SolveJob>` assembler in `envi-gis`/`envi-service`; do not retrofit the engine. Flag for Phase-10 planning.

3. **Regular lattice vs literal CDT vertices for receivers (GRID-01).**
   - What we know: D-07 says footprints are CDT holes; noise maps want predictable spacing.
   - Recommendation: regular lattice clipped to the CDT-defined valid region (see Pattern 4). Confirm in planning.

4. **Forest-crossing detection placement (`SolveJob.forest`).**
   - What we know: CONTEXT flags this; `SolveJob.forest` is a Phase-5 seam, not populated by any construction site; the Fs coherence factor stays deferred.
   - Recommendation: **detect** forest-zone crossings during path extraction (it's the same corridor/cut-plane machinery — a `forest` zone the line crosses → a `ForestCrossing` with through-length `d_m`), but treat it as **optional/last** — if it risks scope, defer the wiring to Phase 10's solve. Do not touch the Fs factor.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust workspace + `cargo` | all | ✓ | edition 2024 | — |
| `spade` | GRID-01, profile | ✓ (in `envi-dgm`) | 2.15 | — |
| `geo` | GEOX-02/03 | ✓ (in `envi-gis`) | 0.30 | — |
| `rstar` | GEOX-03 | ✗ (add) | 0.13 | O(N) scan (acceptable for tiny scenes) |
| `wasm-bindgen-cli` | WASM build | ✓ (Phase-8) | =0.2.126 (pinned) | — |
| GRASS GIS (`r.profile`) | GEOX-01 **oracle only, offline** | ✗ at test time (by design) | — | **committed oracle fixture** (mandatory; mirror Phase-8) |
| Open-Meteo (Archive+Forecast) | METX-01 | ✓ (public, no key) | — | login-server byte proxy |
| Copernicus CDS (ERA5) | METX-02 live retrieval | ✗ (flagged off; needs key) | — | committed fixture test; endpoint DEFAULT-OFF |
| Python / scipy | none at test time | ✗ (by design) | — | committed fixtures |

**Missing dependencies with no fallback:** none block Phase 9 — GRASS and CDS are deliberately test-time-absent (fixtures + flagged-off endpoint).
**Missing dependencies with fallback:** `rstar` (add via Cargo); Open-Meteo CORS (proxy fallback).

## Validation Architecture

Nyquist validation is enabled (no `workflow.nyquist_validation:false` in config). Test framework = Rust `cargo test` (in-crate + `envi-harness` integration tests) + committed oracle/fixture files + offline Playwright for the web journey.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness + `approx`; Playwright `@playwright/test` (offline, `page.route` mocks) |
| Config file | per-crate `Cargo.toml`; `web/playwright.config.ts` (Phase 7/8) |
| Quick run command | `cargo test -p envi-gis` |
| Full suite command | `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check` (+ `cd web && npx playwright test`) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| GEOX-01 | cut-profile matches `r.profile` on real DEM within tolerance (SC1) | oracle fixture | `cargo test -p envi-gis profile::oracle` | ❌ Wave 0 (commit a small real-DEM extract + `r.profile` CSV output) |
| GEOX-01 | profile is strictly-ascending x, ground-z only, hull-safe | unit/property | `cargo test -p envi-gis profile::` | ❌ Wave 0 |
| GEOX-02 | drawn > imported > default resolved per interval; segment count = points−1 | unit | `cargo test -p envi-gis impedance::` | ❌ Wave 0 |
| GEOX-03 | rstar corridor selects the right candidates; cut-plane∩prism inserts correct `(x,z)` tops; thick-building → 2 vertices; feeds a valid `TerrainProfile` | unit + property | `cargo test -p envi-gis screening::` | ❌ Wave 0 |
| GEOX-03 | end-to-end: a screened profile → `terrain_interpretation` yields the expected Sub-model 4/5/6 class | integration (envi-harness) | `cargo test -p envi-harness screening_edges` | ❌ Wave 0 |
| GRID-01 | no receiver inside a footprint; spacing honored; min-spacing/count guardrails return typed errors; discrete points included | unit + property | `cargo test -p envi-gis grid::` | ❌ Wave 0 |
| METX-01 | `c_eff(z)` + per-azimuth A: downwind A > upwind A; inversion ⇒ B>0; round-trip fit identity | property (reuse Phase-3 oracle) | `cargo test -p envi-gis weather::` | ❌ Wave 0 |
| METX-01 | date-switch picks Archive vs Forecast; OPFS cache hit; **zero API calls on what-if**; call-cost logged (SC4) | Playwright offline | `npx playwright test weather-import` | ❌ Wave 0 (mock both open-meteo hosts) |
| METX-02 | Obukhov `1/L` from fixture ERA5 fields; wind×stability class-occurrence counts; async job Queued→Done; endpoint flagged-off by default | unit + fixture + service test | `cargo test -p envi-gis era5:: && cargo test -p envi-service era5_flag_off` | ❌ Wave 0 (commit synthetic ERA5 field fixture) |

### Sampling Rate
- **Per task commit:** `cargo test -p envi-gis` (+ `-p envi-service` when the ERA5 endpoint changes).
- **Per wave merge:** `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`.
- **Phase gate:** full suite green + `cargo tree -p envi-engine` unchanged (3 deps) + `cargo tree -p envi-gis` shows no HTTP/async/browser edge + Playwright weather journey green, before `/gsd-verify-work` and the 5 completion gates.

### Wave 0 Gaps
- [ ] `crates/envi-gis/tests/fixtures/rprofile/` — a small committed real-DEM extract + its `r.profile` output CSV (generated offline once with GRASS; documented provenance + SHA like Phase-8 COG fixtures). **The single load-bearing new fixture.**
- [ ] `crates/envi-gis/tests/fixtures/era5_synthetic.*` — committed ERA5 surface-field fixture (iews,inss,ishf,2t,2d,sp,sdfor) + expected `1/L` + class counts.
- [ ] `crates/envi-gis/tests/fixtures/openmeteo_*.json` — committed Open-Meteo response fixtures (one Archive, one Forecast) for the derivation tests.
- [ ] `web/tests/weather-import.spec.ts` — offline Playwright: mock `api.open-meteo.com` + `archive-api.open-meteo.com` + basemap + GIS; assert cache-then-zero-calls + call-cost log.
- [ ] Reuse (no new file): Phase-3 `route3::fit_profile` round-trip identity covers the A/B/C fit.

## Security Domain

`security_enforcement` is enabled (default). This phase adds untrusted-input surfaces (third-party GIS geometry, Open-Meteo JSON, ERA5 fields) and one new network endpoint.

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V5 Input Validation | **yes** | Every parser (Open-Meteo JSON, ERA5 fields, footprint rings) returns typed errors, never panics on data — the established `envi-gis`/`envi-dgm` posture (finiteness checks, DoS caps). |
| V6 Cryptography | no | No secrets handled client-side; CDS key lives in server env only (never in the browser/bundle). |
| V10 Malicious / SSRF | **yes** | The ERA5 endpoint + Open-Meteo proxy must reuse the Phase-8 **allowlist** (SSRF-tested) — no arbitrary URL fetch; hosts pinned to `open-meteo.com`/CDS. |
| V12 Files/Resources | **yes** | OPFS cache writes bounded; DoS caps on receiver count, profile length, corridor candidates (mirror `envi-dgm::MAX_POINTS`). |
| V4 Access Control | partial | ERA5 endpoint is DEFAULT-OFF (feature flag) + behind the login server; no anonymous CDS retrieval. |

### Known Threat Patterns for this stack
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Malformed footprint/geometry → panic (DoS) | Denial of Service | Skip-and-report + typed errors (Phase-8 `SkipReport`); spade `can_add_constraint` guard. |
| Oversized grid / profile / candidate set (DoS) | Denial of Service | Hard caps: min spacing, receiver-count cap, MAX profile points, corridor candidate cap. |
| SSRF via weather/ERA5 fetch | Tampering / Info-disclosure | Host allowlist (reuse Phase-8 proxy SSRF tests); no user-supplied fetch URL. |
| CDS key leakage | Info-disclosure | Key in server env only; never in wire responses, logs, or the client bundle; endpoint flagged-off. |
| NaN/inf in weather profile → poisoned A/B/C | Tampering | Finiteness checks (Phase-3 route3 already rejects non-finite → typed error). |
| Silent false FORCE pass from `[ASSUMED]` constants | Integrity | Structural/property tests only; quarantine carried from Phase 3 (never a numeric pass). |

## Sources

### Primary (HIGH confidence — read from the repository this session)
- `crates/envi-engine/src/solver.rs` — `SolveJob` fields; **no `screens`/`reflections`**; the frozen chain.
- `crates/envi-engine/src/scene.rs` — `TerrainProfile` (ascending-x, N−1 segments, hSv/hRv `endpoints`), `GroundSegment`, `impedance_class`, `Building.eaves_height_m`, `Barrier`.
- `crates/envi-engine/src/propagation/terrain_interpretation.rs` — §5.21 edge extraction from profile points; Sub-model 4/5/6; ≤2-screen classification.
- `crates/envi-engine/src/propagation/refraction/mod.rs` — `SoundSpeedProfile` (a/b/c/s_a/s_b/z0).
- `crates/envi-harness/src/weather/{mod,route1,route3}.rs` — `WeatherComponents`, `profile_for_bearing`, `fit_profile` (3×3 Cramer), `reconstruct_profiles`, `energy_weighted_level`/`l_den`; the `[ASSUMED]` quarantine.
- `crates/envi-harness/src/lib.rs` — `build_terrain_inputs` (segment "row that starts" convention); SolveJob is otherwise only constructed in tests (`tests/{solve_baseline,mac_identity,tensor_budget}.rs`).
- `crates/envi-dgm/src/tin.rs` — `build_tin`, `Tin::interpolate_z`, DoS caps, intersecting-constraint guard.
- `crates/envi-gis/{Cargo.toml,buildings.rs,lib.rs}`, `crates/envi-gis-wasm/{Cargo.toml,src/lib.rs}` — the pure-Rust/WASM boundary, `eaves_height_m` key, ts-rs DTO discipline.
- `.planning/research/ARCHITECTURE.md` — derive/ module layout, recalc Tiers 0–3, path-cache; **its C-linked/reqwest/SQLite premise is superseded** (Phase-8 pivot).
- `.planning/phases/{08,03}-*/*-CONTEXT.md`, `ROADMAP.md`, `REQUIREMENTS.md` — locked decisions, SCs, requirement wording.

### Secondary (MEDIUM confidence — official docs, CITED not exercised)
- `https://open-meteo.com/en/docs` — endpoints (`api`/`archive-api`), pressure-level + height-AGL variable names, level list, licensing.
- `https://confluence.ecmwf.int/display/CKB/ERA5:+How+to+calculate+Obukhov+Length` — Obukhov recipe + ERA5 short names (iews, inss, ishf, 2t, 2d, sp, sdfor).
- `https://cds.climate.copernicus.eu/how-to-api`, `https://github.com/ecmwf/cdsapi` — CDS queued async retrieval model.
- `cargo search` — `rstar 0.13.0`, `spade 2.15.1`, `geo 0.33.1`.

### Tertiary (LOW confidence — verify during implementation)
- Open-Meteo CORS header specifics (A4) — confirm with one live request.
- TIN-vs-raster r.profile delta magnitude (A3) — measure when the oracle fixture is committed.

## Metadata

**Confidence breakdown:**
- Engine/geometry seams & task reshape: **HIGH** — read from source; the "no screens field" finding is verified.
- Standard stack / crates: **HIGH** — all in-tree or georust-blessed; versions confirmed.
- Open-Meteo endpoints/variables: **MEDIUM** — official docs, not exercised; CORS unconfirmed.
- ERA5 Obukhov recipe: **MEDIUM** — ECMWF docs; derivation is `[ASSUMED]`-bounded per D-05.
- Corridor width / grid guardrails: **MEDIUM** — engineering defaults, tagged `[ASSUMED]`, named consts.

**Research date:** 2026-07-11
**Valid until:** ~2026-08-10 for the external APIs (Open-Meteo/CDS can change variable sets/limits); the in-repo seam findings are stable until the code changes.
