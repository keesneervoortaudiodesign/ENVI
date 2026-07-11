# Phase 9: Path Extraction & Weather - Context

**Gathered:** 2026-07-11
**Status:** Ready for planning

<domain>
## Phase Boundary

The **GIS-to-engine geometry pipeline** plus **real weather**: turn the Phase-8 imported/edited
scene (DGM TIN terrain, `ground_zone` impedance, `building`/`wall`/`barrier` geometry, `calc_area`)
into real `PropagationPath`s the solver can run, and pull runtime meteorology from **Open-Meteo** to
drive the per-azimuth A/B/C refraction the engine already implements. Requirements
**GEOX-01, GEOX-02, GEOX-03, GRID-01, METX-01, METX-02**.

**In scope:**
- **GEOX-01** — DEM cut-profile extractor along a source→receiver line, producing the engine's
  `TerrainProfile` cut-plane; validated against the GRASS `r.profile` oracle on real DEM data.
- **GEOX-02** — ground-impedance segmentation along the profile with priority **drawn > imported >
  default** (locked by requirement).
- **GEOX-03** — screening-edge derivation from `building` + `wall` + `barrier` geometry along the
  path (rstar corridor query), reducing 3D objects to 2D diffraction edges on each cut-plane.
- **GRID-01** — building-aware constrained-Delaunay (spade) receiver grid inside `calc_area`, plus
  discrete receiver points.
- **METX-01** — Open-Meteo import (multi-level winds/temps), cached per (site, time window) in OPFS;
  derive per-azimuth A/B/C from the multi-level profile; what-if edits issue zero API calls.
- **METX-02** — ERA5/CDS **groundwork**: the wind×stability → Obukhov-length → weather-class
  occurrence-statistics derivation, plus an async-job scaffold for the CDS retrieval.

**Out of scope (later phases):**
- Named weather scenarios, manual overrides (Beaufort/wind-dir/downwind-worst-case/temp-gradient/
  A-B-C edit), difference maps → **Phase 11** (roadmap).
- Full energy-weighted **L_den** weather-class combination → **GRID-03** (deferred by design).
- The calc/solve itself, tensor store, cost estimation → **Phase 10**.
- Isophone contouring / results readout → **Phase 11**.
- Facade / assessment-point receivers; reflection-surface geometry extraction; DSM→DTM flattening
  (Phase-8 deferred); forest **Fs** coherence factor (Phase-5 deferred, see `<deferred>`).

**Depends on:** Phase 8 (cached GIS data + DGM TIN — the geometry half is parallel-safe, needing only
engine scene types); engine Phase 3 (Route-1/Route-3 A/B/C math — the weather half). The
`PropagationPath` coordination flag is **effectively resolved**: the seam already exists as
`envi_engine::solver::SolveJob` (engine Phase 4). This phase *builds* its inputs; it does not
redesign the struct.

</domain>

<decisions>
## Implementation Decisions

### Weather import (METX-01)
- **D-01:** **Time = a single representative hour.** The user picks one date+hour; the multi-level
  profile at that hour derives one set of per-azimuth A/B/C. Named scenarios, windows, and the
  downwind worst-case toggle are already Phase-11 scope — this phase delivers one concrete atmosphere
  per import. (SC4's "once per (site, time window)" is satisfied by caching that single-hour fetch;
  the cache key is (site, timestamp).)
- **D-02:** **Open-Meteo product is date-switched: Archive + Forecast.** Historical dates → Open-Meteo
  **Archive** API (ERA5-backed reanalysis); recent/near-future dates → **Forecast** API. Same
  schema/units; the client picks the endpoint by date. Covers both real historical modelling ("what
  was it that night") and current/near-future.
- **D-03 (from Phase-8 pivot, carried):** Weather fetch is **browser `fetch` → OPFS cache**, direct
  CORS first with login-server byte-proxy fallback — NOT a server-side `reqwest` into SQLite.
  What-if edits read OPFS only (zero API calls, verified by network log). This **supersedes**
  `ARCHITECTURE.md`'s `envi-gis` reqwest + `envi-store` SQLite weather premise, exactly as Phase 8
  did for GIS ingestion.

### ERA5/CDS groundwork (METX-02)
- **D-04:** **Server retrieval endpoint + derivation now; live fetch behind a flag.** Build the pure-
  Rust wind×stability → Obukhov → weather-class **occurrence-statistics derivation**, tested against a
  committed fixture (no CDS key needed at test time), plus an **async-job scaffold**. The actual
  queued CDS retrieval lives as a thin **retrieval endpoint on the login/delivery server** — the one
  place server compute is acceptable (it already proxies bytes; CDS is a queued multi-minute job that
  cannot run purely client-side) — **disabled/flagged this phase**.
- **D-05:** **Derivation output = occurrence statistics only** (Obukhov length L + wind/stability
  class counts/frequencies). Mapping each class to an A/B/C profile and the energy-weighted L_den
  combination stay deferred with **GRID-03**.

### Receiver grid (GRID-01)
- **D-06:** **User-set spacing, default 10 m, with a minimum guardrail.** 10 m is a typical noise-map
  resolution; the guardrail exists because client-side WASM compute scales with receiver count ×
  sub-sources. (Guardrail value is Claude/research discretion.)
- **D-07:** **Building footprints are constrained-Delaunay holes** — no receivers generated inside
  footprints; the grid respects footprint edges. Facade / assessment-point receivers are deferred.

### Screening edges (GEOX-03)
- **D-08:** **Buildings + walls + barriers all screen**, using every height-bearing kind already in
  the scene vocabulary: `building` exterior rings at `eaves_height_m`, and `wall`/`barrier`
  linestrings at `height_m`.
- **D-09:** **3D→2D reduction = cut-plane ∩ footprint prism → top edges, multi-edge kept.** Intersect
  the vertical source→receiver cut-plane with each object's footprint×height prism; every wall
  crossing yields a screen edge at its top height, feeding the engine's existing single/multi-edge
  diffraction (do not collapse to one dominant edge — the engine already does multi-edge). The rstar
  corridor width for the candidate-object query is Claude/research discretion.

### Claude's Discretion
- **Cut-profile sampling** step along the S→R line + DEM interpolation method (validate vs GRASS
  `r.profile`) — biggest self-build item; the oracle pins correctness, the method/resolution is open.
- **rstar corridor width** for both the GEOX-03 screening query and the future geometry dirty-diff.
- The **min-spacing guardrail** value for the receiver grid.
- **Azimuth handling for A/B/C**: A depends on the wind component along the *path azimuth*, so it can
  be computed exactly per distinct path azimuth (with caching) or quantized into sectors — the
  accuracy/perf trade-off is open. Reuse the engine Phase-3 Route-1/Route-3 math either way.
- Open-Meteo specifics: which pressure/height levels to request, model selection, unit handling.
- Per-path fan-out strategy for N receivers × M sub-sources (path-cache shape must stay compatible
  with the Phase-11 Tier-2 weather re-solve and Tier-3 geometry dirty-diff in `ARCHITECTURE.md`).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### The `PropagationPath` seam (the one type both engine and pipeline touch)
- `crates/envi-engine/src/solver.rs` — `SolveJob<'a>` (the `PropagationPath` coordination seam,
  already built in engine Phase 4). Fields this phase must populate: `profile: &TerrainProfile`
  (cut-plane), `src`/`rcv` `[f64;3]`, `atmosphere: &Atmosphere`, `coh: &CoherenceInputs`,
  `weather: Option<&SoundSpeedProfile>`, optional `forest`. **Build these; do not redesign the struct.**
- `crates/envi-engine/src/scene.rs` §`TerrainProfile` — the cut-plane type GEOX-01 must produce.
- `crates/envi-engine/src/propagation/refraction/mod.rs` §`SoundSpeedProfile` (`s_a`/`s_b`, Route-3
  A⁺/B⁺) — the A/B/C target that METX-01 feeds; the Route-1/Route-3 derivation math already exists.

### Architecture + module layout (read against the Phase-8 pivot — see D-03)
- `.planning/research/ARCHITECTURE.md` — the `derive/` module split for this phase:
  `profile.rs` (cut-profile GEOX-01), `screening.rs` (edges GEOX-03, rstar), `grid.rs` (receiver grid
  GRID-01), `weather.rs` (Open-Meteo + ERA5 classes METX-01/02); the `PropagationPath` corridor cache
  and the recalc-router tiers (Tier-2 weather re-solve, Tier-3 geometry dirty-diff) the path-cache
  shape must stay compatible with. **⚠ Its `envi-gis`=C-linked+reqwest and `envi-store`=SQLite premise
  is SUPERSEDED by the Phase-8 WASM pivot** (pure-Rust, browser fetch, OPFS) — treat module topology
  as guidance, the C-toolchain/SQLite/reqwest assumptions as overridden (mirror Phase-8 08-CONTEXT).
- `.planning/research/OPEN-GIS-LANDSCAPE.md` §1 (cut-profile self-build vs GRASS `r.profile`), §4
  (meteorology Routes 1/2/3; Open-Meteo CC-BY no-key multi-level; ERA5/CDS for u*/H/z0→Obukhov;
  avoid OpenWeatherMap/Meteostat by license).

### Requirements + roadmap
- `.planning/ROADMAP.md` "Phase 9: Path Extraction & Weather" — goal + SC1–SC5.
- `.planning/REQUIREMENTS.md` §GEOX (GEOX-01/02/03), §GRID (GRID-01), §METX (METX-01/02), and the
  **GRID-03** deferral note (full L_den combination).

### Prior-phase context this phase binds to
- `.planning/phases/08-gis-ingestion-dgm/08-CONTEXT.md` — the WASM pivot (D-01/02/03: client-side
  compute, CORS gatekeeper, OPFS cache) that governs the weather-fetch path here; the
  `ground_zone`/building/`wall` editable-feature model this phase consumes; the deferred
  `ground_zone`→cut-plane and `calc_area`→receiver-grid hand-offs (now realized here).
- `.planning/phases/03-meteorology-refraction/03-CONTEXT.md` + `03-RESEARCH.md` — the Route-1 class
  table, Route-3 Monin–Obukhov + 3×3 LSQ fit, and the `SoundSpeedProfile` A/B/C the weather half
  targets. **[ASSUMED] weather-route A/B/C constants are quarantined (Phase-3 03-03 Open-Q1)** —
  METX-01 wiring must not silently promote them to a false FORCE numeric pass.
- `.planning/phases/05-.../05-CONTEXT.md` — Phase-5 deferred the forest **Fs** coherence factor with
  "revisit Phase 9"; `SolveJob.forest` (Sub-Model 10 `ForestCrossing`) is the seam a path's
  forest-zone crossing would populate (see `<deferred>`).

### Binding project contracts
- `.claude/CLAUDE.md` — native-Rust-over-FFI (absolute on the WASM path), engine 3-dep quarantine
  (byte-identical, no new engine deps), the **105-point band-index** framework (compare by band index,
  never nominal Hz), Playwright offline-UAT (intercept ALL network incl. weather sources), English-
  only output, GitHub/commit conventions, and the five mandatory GSD completion gates.

### Existing code to build against (verify, do not break)
- `crates/envi-store/src/geojson.rs` — the locked 9-kind `KINDS` vocabulary (`building`, `wall`,
  `ground_zone`, `calc_area`, `forest`, `elevation_*`, …) + `scene_to_engine`; this phase consumes
  `ground_zone`/`building`/`wall`/`calc_area` and must conform to the same reprojection boundary.
- `crates/envi-dgm/src/tin.rs` — `build_tin` (spade CDT), the TIN the cut-profile samples elevation
  from; the same spade dependency backs the GRID-01 constrained-Delaunay receiver grid.
- `crates/envi-geo/src/…` — the single pure-Rust CRS reprojection boundary (GEOX-04); all imported
  GIS/weather coordinates reproject here.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`envi_engine::solver::SolveJob`** — the `PropagationPath` target; already carries every field this
  phase produces (profile, positions, atmosphere, coherence, weather, forest, directivity). Building
  its inputs is the phase.
- **Engine Phase-3 A/B/C math** (`refraction::SoundSpeedProfile`, Route-1 class table, Route-3
  Monin–Obukhov + LSQ) — the weather half feeds this, does not reimplement it.
- **`envi-dgm` spade CDT** — reused twice: elevation sampling for the cut-profile and the constrained-
  Delaunay receiver grid (building footprints as holes).
- **`envi-geo`** reprojection boundary and **`envi-store::geojson`** 9-kind vocabulary — the scene
  objects (`ground_zone`, `building`, `wall`, `calc_area`) this phase reads.
- **Phase-8 `envi-gis`** (pure-Rust, WASM-safe, OPFS-cached) — the crate/pattern the new
  `profile`/`screening`/`grid`/`weather` modules extend; browser fetch + login-server byte-proxy is
  the established weather-fetch mechanism.

### Established Patterns
- **Client-side WASM + OPFS cache, CORS gatekeeper** (Phase-8 pivot) — weather fetch obeys the same
  rules as GIS ingestion; server compute is limited to the login server (ERA5/CDS retrieval only).
- **Oracle-pinned self-build** — cut-profile validated vs GRASS `r.profile`; derivation validated vs
  a committed fixture; no runtime Python (mirrors the Phase-8 GDAL-COG and scipy-oracle patterns).
- **Honest green, no false FORCE pass** — the [ASSUMED] Phase-3 weather constants stay quarantined;
  structural/property tests only until real validation lands.
- **Quality gates:** `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `cargo test`;
  Playwright offline (must intercept weather + basemap + GIS network).

### Integration Points
- New pure-Rust WASM modules (`profile`, `screening`, `grid`, `weather`) in the GIS crate ⇄ browser
  `fetch` (Open-Meteo direct; ERA5 via login-server job) ⇄ OPFS cache ⇄ `envi-geo` reproject ⇄
  `SolveJob` construction.
- **`SolveJob.forest`**: a path's crossing of a `forest` zone would populate the Phase-5
  `ForestCrossing`; wiring forest-crossing *detection* into path extraction is a natural home here,
  but the **Fs coherence factor** itself stays deferred (see `<deferred>`). Flag for research: decide
  whether crossing-detection lands this phase or with Phase-10's solve.
- Login/delivery server gains ONE flagged surface: the ERA5/CDS async retrieval endpoint (thin,
  disabled by default).

</code_context>

<specifics>
## Specific Ideas

- The user consistently took the **lean, honest, physically-defensible** option: a single concrete
  atmosphere over speculative windows/scenarios (D-01); date-switched Archive+Forecast so real
  historical nights are modellable (D-02); ERA5 groundwork kept to derivation + a flagged scaffold
  rather than a full live pipeline (D-04/D-05); multi-edge diffraction preserved rather than collapsed
  to a dominant edge (D-09). This matches the Phases-6/7/8 preference for check-and-complete,
  no-false-green, and one-source-of-truth.
- The **`PropagationPath` = `SolveJob`** realization removes the roadmap's "agree the struct early"
  risk: the contract is frozen engine-side; Phase 9 is an input-builder, not a co-design.

</specifics>

<deferred>
## Deferred Ideas

- **Named weather scenarios + manual overrides + difference maps** (Beaufort, wind dir, downwind
  worst-case toggle, temp gradient, per-azimuth A/B/C edit) — **Phase 11** (already roadmapped).
- **Full energy-weighted L_den weather-class combination** — **GRID-03** (deferred by design; METX-02
  ships only the occurrence statistics that feed it).
- **Facade / assessment-point receivers** — beyond the calc_area grid + discrete points this phase.
- **Reflection-surface geometry extraction** (building-facade reflection paths) — not in Phase-9 SC;
  engine Phase-3 has reflection-path coefficients, but extracting reflection geometry is Phase 10/11.
- **Forest Fs coherence factor** (Phase-5 05-01 deferral, "revisit Phase 9") — remains deferred; only
  the geometric forest-crossing seam is in play here, and even that placement is a research flag.
- **DSM→DTM flattening / under-footprint sample exclusion** (Phase-8 D-05 deferral) — still deferred.
- **Overture buildings, national DTMs beyond AHN** — pluggable, pure-data additions later (Phase-8).

None of the above expanded this phase's scope — the discussion stayed within the GEOX/GRID/METX
boundary.

</deferred>

---

*Phase: 9-Path Extraction & Weather*
*Context gathered: 2026-07-11*
