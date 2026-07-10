# Phase 8: GIS Ingestion & DGM - Context

**Gathered:** 2026-07-10
**Status:** Ready for planning

<domain>
## Phase Boundary

The NoizCalc **"Import" moment**: users pull real-world **terrain, ground cover, and buildings**
for the current map viewport onto a triangulated ground model (**DGM TIN**), and everything imported
becomes an **ordinary editable scene object** ("check and complete"). Requirements **DATA-01..04**.

**In scope:** viewport-scoped fetch of terrain (AHN4 DTM preferred / Copernicus GLO-30 fallback),
ESA WorldCover ground cover, and OSM/Overpass buildings; materialization as editable scene objects
draped on a DGM TIN (extending the existing `envi-dgm` seam with imported elevation samples); the
WorldCover→Nordtest σ/impedance mapping table + a per-row unit test + an impedance debug overlay;
the building height fallback chain + per-feature provenance + footprint-boundary base-elevation
sampling; per-project on-disk cache (in-browser OPFS) with a "reads only cache with network off"
guarantee; data attribution UI.

**⚠ MAJOR ARCHITECTURAL PIVOT captured in this discussion (see `<decisions>` D-01 and
`<deferred>`):** the deployment model changed from Phase-6's *"self-hosted localhost axum binary,
light/no auth"* to **client-side WASM compute on the user's device, gated by a login/delivery
server** ("no installation; only runnable when logged in"). Phase 8 proceeds **under this new
assumption**; the broader PROJECT.md / ARCHITECTURE.md / Phase-6 amendments are flagged as
cross-phase follow-ups, NOT resolved here. This pivot is what makes GIS ingestion pure-Rust (GDAL
cannot cross into WASM) and moves the cache into the browser.

**Out of scope (later phases):** DEM cut-profile extraction / impedance segmentation along paths
(`ground_zone`→Phase 9), receiver-grid generation from `calc_area` (Phase 10), weather import
(Phase 9), the calculation/solve itself (Phase 10), results/contours (Phase 11), DSM→DTM flattening
of surface-model terrain, Overture buildings, national DTM sources beyond AHN, and the full
PROJECT-level re-architecture of the WASM/auth deployment model (a dedicated amendment pass).

**Depends on:** Phase 6 (CRS boundary `envi-geo`, project/store contracts — several now under revision
by the pivot), Phase 7 (imported features must be editable in the scene editor; the `envi-dgm` TIN
seam this phase feeds).

</domain>

<decisions>
## Implementation Decisions

### Ingestion & compute model — the pivot (DATA-01..04; supersedes Phase-6 D-01/D-02)
- **D-01:** **ENVI runs client-side in the browser as WebAssembly; calculations execute on the
  user's own device; a login/delivery server exists only to authenticate and serve the app bundle
  (and optionally proxy fetches).** "No installation; only runnable when logged in to the server."
  Consequence for this phase: **GIS ingestion is pure-Rust and WASM-targeted — GDAL/PROJ-C is
  ruled out entirely** (C libraries don't cross into WASM sanely). The whole Phase-6-deferred
  C-toolchain / Windows-provisioning question is therefore **moot and closed**. `envi-engine`
  (ndarray + num-complex) and `envi-dgm` (spade) already compile to `wasm32` cleanly; the new GIS
  ingestion crate (architecture's `envi-gis`) **inverts** from "everything C-linked" to "pure-Rust,
  browser-fetch + OPFS-cache, WASM-compatible."
- **D-02:** **CORS is the new gatekeeper (not `/vsicurl`).** The browser fetches COG windows /
  WorldCover / buildings / (later) weather directly. Strategy: **direct cross-origin fetch first,
  fall back to a login-server byte/range proxy** only for sources that block CORS. Maintain a
  **per-source CORS capability map**. A proxy relays bytes only (no compute), so it does not violate
  "calculations on the local machine"; it also gives a home for API-key injection / rate-limit
  smoothing (e.g. Overpass) if needed.
- **D-03:** **DATA-04 cache lives in OPFS (Origin Private File System), per project.** OPFS is the
  browser analog of the per-project disk cache: file-like, async, handles large binary blobs (DEM
  windows, tiles). The "compute reads only local cache, verified with the network off" guarantee
  means: after import, the WASM app reads terrain/landcover/buildings **exclusively from OPFS**;
  network access is touched **only at ingestion time**. (Replaces Phase-6 D-04's on-disk `cache/`
  folder for the client path.)

### Terrain source strategy (DATA-01, SC1)
- **D-04:** **AHN4 DTM (Netherlands, via PDOK — CORS-friendly COG) is the preferred source in its
  coverage area; Copernicus GLO-30 is the global fallback.** Wired behind a **pluggable source
  registry** (coverage lookup → source selection) so more national DTMs slot in later as pure data.
  AHN is real, immediate value (user is in NL) and actually exercises SC1's "DTM preferred where
  available" rather than stubbing it.
- **D-05:** **GLO-30 is a surface model (DSM) — handle by flag + badge only ("check and complete").**
  Import GLO-30 as the TIN, badge it "surface model — may include buildings/vegetation," surface that
  in the terrain/impedance overlay, and let the user correct via editing. **No DSM→DTM flattening and
  no under-footprint sample exclusion this phase** (deferred). Pairs with D-10's footprint-boundary
  base-elevation sampling, which independently avoids buildings inflating their own base heights.

### The "Import" moment & failure handling (SC1)
- **D-06:** **Import region = the current map viewport, with a max-area guardrail.** The literal
  NoizCalc "Import" / roadmap "viewport import." The guardrail warns/blocks when the viewport is too
  large, because client-side WASM must fetch every covering COG tile into browser memory — cost
  scales with area. Each layer (terrain / landcover / buildings) is **independently toggleable**.
- **D-07:** **Partial failure lands what succeeded; failed layers are retryable and non-blocking.**
  If one layer's fetch fails (e.g. Overpass down), import the layers that worked, mark the failed one
  with a clear error + retry action, show **per-layer status** in the import UI, and don't block the
  rest — you can start modelling terrain while buildings retry (check-and-complete).
- **D-08 (pivot reframe):** Import **progress is in-app client-side UI state, NOT the Phase-6
  server-side SSE job machine** (that machine was built for server compute and is undercut by the
  client-side model — see `<deferred>`). SC1's "live progress and clear failure states" is a
  client concern for the import path.

### "Check & complete" editability (SC1, SC4, SC5)
- **D-09:** **Re-import MERGES — never clobbers user edits.** Re-importing over an already-imported,
  already-edited area adds only genuinely-new features and refreshes imported-but-untouched ones;
  anything the user moved, edited, or created is preserved. Requires **per-feature identity + a
  "user-modified" flag**. (Chosen over replace-within-bbox and diff-and-choose.)
- **D-10:** **Buildings from OSM via Overpass (primary source).** JSON over HTTP, CORS-friendly,
  trivial bbox query, clean in WASM. Height/levels tags are sparser than Overture, so the **locked
  fallback chain does more work** (measured → height tag → levels×3+1.5 → user default). Overpass
  rate limits handled via the login-server proxy (D-02) if needed. Overture GeoParquet is the
  documented future upgrade (denser heights) but its in-browser parquet querying + uncertain S3/Azure
  CORS make it heavier — deferred.
- **D-11 (settled by roadmap SC4/SC5 — recorded, not re-decided):** every imported feature is an
  **ordinary editable scene object** carrying **per-feature provenance** (source + license +
  retrieval date); building base elevations sample **footprint-boundary ground, never
  DSM-under-building**; the map shows **attribution** for OSM/Overture/ESA WorldCover/Copernicus.

### Claude's Discretion
- The WorldCover→Nordtest σ/impedance **mapping table values** (SC3): researcher populates the σ per
  class from Nordtest, user reviews the table; the *mechanism* (reviewed data table + per-row unit
  test + impedance debug overlay) is fixed, the numbers are research-owned.
- Attribution UI presentation; import-panel layout and per-layer status affordances; the max-area
  guardrail threshold; OPFS keying/eviction scheme; the exact pure-Rust COG-window reader assembly
  (IFD parse → overview pick → tile-range math → range GETs → stitch) and the DEM→UTM resampler;
  the per-source CORS capability-map format; "user default" building-height value.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### The authoritative architecture (baseline — read FIRST, but note the pivot supersedes parts)
- `.planning/research/ARCHITECTURE.md` — the `envi-gis` crate design, `acquire/` (dem/landcover/
  buildings/cache) + `derive/` (crs/dgm/contour) module split, the import endpoint, and the DGM/TIN
  draping model. **⚠ Read against D-01/D-02/D-03:** the crate's "everything C-linked (gdal/proj)"
  premise is INVERTED to pure-Rust/WASM by the pivot; `/vsicurl` → browser fetch; on-disk `cache/`
  → OPFS. Treat topology/module-split as guidance, the C-toolchain assumption as overridden.
- `.planning/research/OPEN-GIS-LANDSCAPE.md` — the open-data source landscape (GLO-30, WorldCover,
  Overture/OSM), licensing, and the pure-Rust vs GDAL crate options relevant to D-01.

### Requirements + roadmap
- `.planning/ROADMAP.md` "Phase 8" — goal + success criteria SC1–SC5 (the DATA-04 network-off
  guarantee, the WorldCover-table + per-row test, the height fallback chain, footprint-boundary base
  elevation, attribution). **Note the pivot reframes SC1's "job/progress" as client-side.**
- `.planning/REQUIREMENTS.md` §DATA (DATA-01/02/03/04) — the requirement wording.

### Prior-phase context this phase binds to / revises
- `.planning/phases/06-service-foundation-persistence/06-CONTEXT.md` — Phase-6 D-01/D-02 explicitly
  **deferred GDAL/PROJ provisioning to Phase 8**; the pivot (D-01 here) **closes that as moot**
  rather than resolving it. Phase-6 D-04 (project-as-folder disk) and the job/SSE machine are
  undercut by the client-side model — see `<deferred>`.
- `.planning/phases/07-frontend-shell-scene-editing/07-CONTEXT.md` — Phase-7 D-08 built the
  **`envi-dgm` constrained-Delaunay TIN seam** (spade) from user-drawn elevation; Phase 8 **extends
  the same seam** by feeding imported GLO-30/AHN samples in as additional vertices (no rewrite).
  D-01 there notes `scene_to_engine` maps 4/9 kinds and `elevation_*`→engine DGM lands in **Phase 8**.

### Binding project contracts
- `.claude/CLAUDE.md` — **native-Rust-over-FFI house rule** (now absolute for the WASM path), the
  engine 3-dep quarantine (byte-identical, no new deps), the 105-point band-index framework
  (compare by band index, never nominal Hz), Playwright offline-UAT rules (E2E must `page.route`
  all network incl. basemap + GIS sources), English-only output, GitHub/commit conventions, and the
  five mandatory GSD phase-completion gates.
- `.planning/PROJECT.md` — the product vision (pulls terrain/ground/building/weather from public
  APIs). **⚠ Its "self-hosted, localhost, light/no auth" deployment statement is superseded by the
  pivot and needs an amendment pass (see `<deferred>`).**

### Existing code to build against (verify, do not break)
- `crates/envi-dgm/src/tin.rs` — `build_tin(points: &[[f64;3]], breaklines: &[Vec<[f64;2]>])` — the
  TIN seam imported terrain samples feed into (Phase-7 D-08). Must stay WASM-compatible.
- `crates/envi-geo/src/…` — the single pure-Rust CRS boundary (`LonLat`/`SceneXY`, UTM zone pin);
  imported GIS coordinates reproject here. WASM-compatible.
- `crates/envi-store/src/geojson.rs` — the locked 9-kind `properties.kind` vocabulary + provenance
  property carriers; imported features must conform.

### Workflow reference (descriptive)
- `docs/references/dbaudio-ti386-1.6-en.md` ch. 3–4 — the NoizCalc Import → check-and-complete →
  model loop this phase realizes.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`envi-dgm::build_tin`** (spade CDT) — the TIN this phase drapes imported terrain onto; extend by
  feeding imported elevation samples as vertices. Pure Rust, WASM-safe.
- **`envi-geo`** — the one CRS seam; imported WGS84 GIS coords reproject to project UTM here. WASM-safe.
- **`envi-store::geojson`** — the 9-kind vocabulary + `check_wgs84`; imported features become these
  editable kinds (buildings, ground_zone, elevation_*) with provenance properties.
- **The engine's WASM-compatibility** — `envi-engine` (ndarray + num-complex) and `envi-dgm` (spade)
  compile to `wasm32` cleanly, which is what makes the D-01 client-side model viable.

### Established Patterns
- **Native-Rust-over-FFI** (CLAUDE.md) — now absolute: the WASM target forbids C entirely on the
  client path. GDAL is out; the pure-Rust COG/COG-window path (assembled on the `tiff`-crate decoder
  + HTTP range requests) and the pure-Rust resampler are the way.
- **One source of truth, drift made structurally impossible** (Phases 6–7 meta-preference): OPFS as
  the *only* post-import data source (D-03), per-feature identity for merge (D-09), reviewed table +
  per-row test for impedance (SC3).
- **Honest failure states, no false green** — per-layer retryable partial import (D-07); GLO-30
  "surface model" badge (D-05); "network off" cache verification (D-03).
- **Quality gates:** `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `cargo test`;
  Playwright offline (must intercept GIS + basemap network).

### Integration Points
- New pure-Rust WASM ingestion crate (architecture's `envi-gis`, re-scoped) ⇄ browser `fetch`
  (direct + login-server proxy fallback) ⇄ OPFS cache.
- Ingestion → `envi-geo` reprojection → `envi-dgm` TIN → `envi-store` editable scene features.
- Login/delivery server: new — auth + bundle serving + CORS proxy (largely NEW scope; see deferred).

</code_context>

<specifics>
## Specific Ideas

- **The defining moment of this discussion was the deployment pivot**, driven by the user's
  requirement: *"usable directly from any browser, without ANY installation … calculations done on
  the local machine, but no install and only runnable when logged in to a server."* Confirmed as
  **WASM client-side compute + login/delivery server**. This retroactively makes the Phase-6
  "isn't there an option without C?" instinct **binding** — C is now impossible on the client path,
  not merely dispreferred — and dissolves the entire GDAL/Windows-provisioning problem that Phase 6
  had parked in Phase 8.
- **AHN specifically** — the user is in the Netherlands, so AHN4 (world-class ~0.5 m LiDAR DTM via
  PDOK, CORS-friendly COG) is the concrete first national source, not a placeholder.
- Continued Phases-6/7 preference for **simplicity, inspectability, and forgiving check-and-complete
  UX**: flag-don't-fix for surface-model terrain (D-05), merge-don't-clobber on re-import (D-09),
  land-what-succeeded on partial failure (D-07), OSM/Overpass over heavier Overture (D-10).

</specifics>

<deferred>
## Deferred Ideas

### [PROJECT-LEVEL AMENDMENT — the pivot's ripples beyond Phase 8]
The user chose to proceed with Phase 8 under the new WASM/auth assumption and **defer** the broad
re-architecture. Strongly recommend a **PROJECT.md / ARCHITECTURE.md amendment pass before Phase 10**:
- **PROJECT.md**: deployment model "self-hosted localhost binary, light/no auth" → **WASM
  client-side compute + login/delivery server (real auth)**.
- **Phase 6 revisit**: project-as-folder-on-disk persistence → **OPFS/IndexedDB and/or server-sync**
  per-user; the **server-side job/SSE state machine** is undercut (client-side compute → in-app
  progress). Both are load-bearing Phase-6 decisions now in tension.
- **Phase 10/11 revisit**: the **chunked on-disk tensor store + memory budgets** move into browser
  OPFS/memory; the **calc job runs client-side**; **Phase-11 contouring uses the pure-Rust `contour`
  crate**, not `GDALContourGenerateEx`.
- **New scope**: an **auth system** (login gate) + a **WASM build target** for engine + ingestion
  (+ COOP/COEP headers if WASM threads are wanted for 100k-receiver grids).

### [ROADMAP COORDINATION — deferred within the GIS pipeline]
- **DSM→DTM flattening / under-footprint sample exclusion** for surface-model terrain — Phase 8 does
  flag-only (D-05); acoustic correction deferred.
- **Overture GeoParquet buildings** (denser heights) — deferred behind OSM/Overpass (D-10);
  documented upgrade path.
- **National DTM sources beyond AHN** — the pluggable registry (D-04) is built now; more countries
  are pure-data additions later.
- **`ground_zone`→`GroundSegment` cut-plane extraction** (Phase 9), **`calc_area`→receiver grid**
  (Phase 10) — imported/edited here, consumed there.

</deferred>

---

*Phase: 8-GIS Ingestion & DGM*
*Context gathered: 2026-07-10*
