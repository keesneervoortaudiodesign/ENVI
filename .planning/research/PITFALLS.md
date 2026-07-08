# Pitfalls Research

**Domain:** Web GIS noise-modeling UI + service (Milestone 2) on top of an existing validated Rust Nord2000 engine
**Researched:** 2026-07-08
**Confidence:** HIGH overall (project-local claims from PROJECT.md / TI 386 / OPEN-GIS-LANDSCAPE.md are HIGH; web-verified external claims tagged MEDIUM per the confidence seam — each cross-checked against official docs/issue trackers)

Scope note: Milestone 1 (engine, Phases 1–4) is not re-covered here. Every pitfall below is specific to ADDING the GIS web UI, HTTP service, open-data ingestion, and results-rendering layer around the frozen engine contract (`H_coh` complex tensor + `P_incoh` channel, 105-point 1/12-oct grid, semantic 2.5D metric scene). Phase names are functional — the M2 roadmap should bind them to concrete phase numbers (UI phases append from Phase 5).

## Critical Pitfalls

### Pitfall 1: One CRS soup — lon/lat leaks into the metric engine

**What goes wrong:**
MapLibre and Terra Draw hand you WGS84 lon/lat degrees (EPSG:4326 / web-mercator display); the engine's frozen scene contract is a projected metric CRS, Z-up, meters. If degrees ever reach engine geometry, distances are wrong by ~5 orders of magnitude (or subtly wrong by cos(lat) if someone "fixes" it with a scale factor), and every downstream acoustic result is garbage that still *renders plausibly* on the map.

**Why it happens:**
Both representations are `(f64, f64)` pairs. Nothing in the type system stops a lon/lat from being consumed as an easting/northing. The frontend, the GeoJSON wire format, and the GIS rasters all speak 4326-ish coordinates, so the "natural" flow is to defer projection — until it's deferred past the engine boundary.

**How to avoid:**
- **Single reprojection boundary**: exactly one module in the service crate converts wire GeoJSON (always lon/lat per RFC 7946) ↔ scene coordinates. Mirror the engine's single-`.conj()` discipline: one place, grep-enforceable.
- **Newtype the two spaces** (`LonLat` vs `SceneXY`) in the service crate; the engine's scene types only accept `SceneXY`.
- **Auto-UTM pinned per project**: pick the UTM zone (or national CRS) from the scene centroid at project creation, store the EPSG code in the project file, and never re-derive it silently (a scene near a zone boundary that re-derives per-request will flip zones and shift geometry by hundreds of km).
- Sanity assertion at the boundary: scene coordinates with |x| ≤ 180 and |y| ≤ 90 are almost certainly degrees — reject loudly.

**Warning signs:**
Distances of ~0.0001 "m" between drawn objects; scene bounding boxes smaller than 360×180; receiver grids that compute instantly (paths are micrometers long); results that change when the map viewport pans.

**Phase to address:**
Service foundation phase (first M2 phase — GEOX-04). The boundary module + newtypes must exist before any drawing or ingestion code is written.

---

### Pitfall 2: PROJ/GDAL axis-order ambush (lat/lon vs lon/lat)

**What goes wrong:**
Since PROJ 6 / GDAL 3, `EPSG:4326` in authority-compliant mode is **lat,lon** (north,east), while GeoJSON, MapLibre, and most code assume **lon,lat**. A transform built from the EPSG code silently swaps axes: geometry lands mirrored across the 45° line, usually in the ocean — or worse, near the site but transposed, for sites where lat≈lon.

**Why it happens:**
GDAL's historical behavior was lon,lat ("traditional GIS order"); the EPSG registry order is lat,lon; GDAL 3 defaults to authority order unless told otherwise. The Rust `proj`/`gdal` crates expose both behaviors depending on how the CRS object is constructed (`Proj::new_known_crs` honors authority order; WKT with explicit axis order doesn't).

**How to avoid:**
- Set `OAMS_TRADITIONAL_GIS_ORDER` (via `SpatialRef::set_axis_mapping_strategy`) on every `SpatialRef` used with GDAL, or construct PROJ pipelines from explicit `+proj=` strings rather than bare EPSG codes.
- One unit test that projects a known landmark (e.g. a surveyed point near the dev site) through the exact production code path and asserts easting/northing to the meter. This is the cheapest high-value test in the whole milestone.

**Warning signs:**
Imported buildings render in the wrong hemisphere; DEM sampling returns nodata everywhere; coordinates "work" for one site and break for another (lat/lon magnitudes differ).

**Phase to address:**
Service foundation phase (GEOX-04), same boundary module as Pitfall 1; landmark round-trip test in the phase's gate.

---

### Pitfall 3: GLO-30 is surface-biased (DSM) — trees and buildings baked into "ground"

**What goes wrong:**
Copernicus GLO-30 is derived from TanDEM-X radar surface returns: in forest and dense urban areas the "terrain" is meters too high (canopy/rooftop bias). Two acoustic consequences: (a) sources/receivers placed "on the ground" sit on phantom canopy, changing ground-effect geometry; (b) buildings imported from Overture are modeled as screens **on top of terrain that already contains them**, double-counting height and over-predicting screening.

**Why it happens:**
GLO-30 is the only free global tier, so it's the default; the DSM/DTM distinction is invisible in flat open test areas where the two coincide — it only bites when a real forested/urban site is modeled.

**How to avoid:**
- Prefer national LiDAR **DTM** (AHN/DGM1/RGE ALTI/…) wherever available; treat GLO-30 as the explicit fallback tier (already the locked plan — enforce it in the ingestion priority order, don't let GLO-30 shortcut it).
- When on GLO-30 fallback, surface a UI notice ("surface model — terrain may include vegetation/building height") and make manual terrain correction (elevation points/lines, Pitfall 4) easy.
- Never sample building base elevation from the DSM under the building footprint — sample the surrounding ground (footprint-boundary minimum or buffered median), the standard DSM-under-building workaround.
- Do NOT reach for FABDEM (the "corrected GLO-30") — non-commercial license, explicitly excluded (see Pitfall 12).

**Warning signs:**
Terrain profile shows a plateau exactly matching a forest polygon; buildings with near-zero effective screen height (roof ≈ "terrain"); a flat venue that the DEM renders as bumpy.

**Phase to address:**
GIS ingestion phase (DATA-01/DATA-03, GEOX-03). Building-base sampling rule is an ingestion-phase deliverable; the UI notice belongs to the map/drawing phase.

---

### Pitfall 4: The "buried subwoofer" — triangulation artifacts under sources (NoizCalc §3.2.2)

**What goes wrong:**
The triangulated ground model (DGM) built from sparse/inaccurate elevation data undulates through the venue; a source placed at 0.5 m above nominal ground ends up partially **below** the triangulated surface. The engine then computes a source under terrain — ground effect and screening are nonsense for exactly the most important paths (near-source geometry dominates ground effect).

**Why it happens:**
30 m DEM posts + Delaunay triangulation cannot represent a locally flat, graded venue. NoizCalc documents this exact failure ("SUB array partially buried by the DGM") and ships a pre-check that warns but does not stop the run.

**How to avoid:**
- Implement NoizCalc's remedy as first-class features: **elevation points/lines** (contour lines = constant z, profile lines = varying z) that locally override the DEM, and a "flatten venue" affordance (delete DEM posts inside a drawn polygon, constrain an elevation line around it — `spade` constrained Delaunay supports this directly).
- **Pre-calculation validation pass** that checks every source/receiver z against the triangulated surface and reports "object below terrain by X m" with a click-to-zoom link (NoizCalc §3.3.2's clickable error messages are the UX model). Warn, don't hard-fail, matching NoizCalc — but make the warning impossible to miss.
- Snap drawn objects' base z to the DGM at placement time and re-snap on move (NoizCalc re-references moved objects to the DGM automatically).

**Warning signs:**
Near-field levels lower than free-field expectation; ground-effect dip at wrong frequencies for near-source receivers; source z < terrain z in the scene dump.

**Phase to address:**
Map/drawing phase for elevation objects (WEB-04 family); pre-calculation validation in the grid-compute phase gate. Roadmap flag: elevation-edit objects are NOT optional polish — they are the documented workaround for a data-quality problem that WILL occur.

---

### Pitfall 5: Crossing ground-effect polygons make impedance segmentation ambiguous (NoizCalc §3.2.3)

**What goes wrong:**
Two ground-effect polygons with different impedance classes partially overlap. Along a source→receiver profile through the overlap, the impedance is undefined — NoizCalc refuses to calculate at all in this case ("crossing outlines… are ambiguous and prevent any calculation"). If ENVI instead resolves it silently (first-hit, draw-order), results become dependent on invisible ordering and are irreproducible.

**Why it happens:**
Freehand polygon drawing plus OSM/WorldCover-imported polygons practically guarantees partial overlaps. Full containment (paved lot inside a park) is fine and common; partial crossing is the poison case.

**How to avoid:**
- Adopt NoizCalc's semantics explicitly: **containment allowed (innermost wins), partial crossing rejected**.
- Validate at edit time, not only at calculation time: when a ground-effect polygon is finished/moved, run an `rstar`-indexed overlap check against its siblings; flag partial crossings on the map immediately (red outline + message), and repeat the check in the pre-calculation pass with clickable errors.
- Imported land-cover polygons should be pre-flattened into a non-overlapping coverage during ingestion (WorldCover is a raster → polygonization can guarantee this); only user-drawn overrides need the interactive check.

**Warning signs:**
Impedance segmentation along a profile depends on polygon insertion order; two runs of the same scene give different ground segments; users report "calculation blocked and I don't know why" (means the error UX is missing).

**Phase to address:**
Map/drawing phase (draw-time validation) + grid-compute phase (pre-calculation gate). The containment-resolution rule (innermost wins) must be specified in the scene model before ingestion writes ground polygons.

---

### Pitfall 6: Sharing GDAL datasets across threads in a long-running service

**What goes wrong:**
GDAL is not thread-safe per dataset: concurrent calls on one `GDALDataset` — or even on different `RasterBand`s of the same dataset — corrupt state (datasets own a file handle doing seek/read; the block cache structures are not thread-safe). In a Rust HTTP server with a worker pool, a naively shared `Dataset` produces intermittent wrong pixel reads or crashes deep in C code, on load only, never in tests. [MEDIUM — gdal.org multithreading docs + RFC 101, verified]

**Why it happens:**
The service is the first long-lived multi-threaded GDAL consumer in the project (envi-harness is a batch CLI). The Rust `gdal` crate's `Dataset` isn't `Sync`, which prevents the direct mistake — but people then wrap it in `Mutex` inside an async handler (blocking the runtime) or lazily open one global dataset per file "for performance."

**How to avoid:**
- **One `Dataset` per worker/request** (dataset opens are cheap relative to acoustics); or a per-thread dataset pool keyed by file path. On GDAL ≥ 3.10, `GDAL_OF_THREAD_SAFE` (RFC 101) gives a genuinely thread-safe read-only raster handle — usable if the pinned GDAL version supports it, but don't architect around it as the only mechanism.
- **All GDAL/PROJ calls stay in the I/O crate** behind the same thin-boundary rule as Milestone 1 (`envi-engine` remains `ndarray`+`num-complex`+`thiserror` only — add a `cargo tree` gate for the new service crate too: no `gdal`/`proj` in the engine's dependency closure).
- GDAL calls are **blocking**: in an async server (tokio/axum), route every GDAL touch through `spawn_blocking` or a dedicated rayon/OS-thread pool. Never call GDAL inline in an async handler.
- GDAL's error reporting is thread-local `CPLError` state + return codes; convert to `Result` immediately at the boundary (the `gdal` crate does this) and never let a panic unwind across the FFI boundary (wrap callback-style APIs with `catch_unwind` if any are used).

**Warning signs:**
Heisenbugs only under concurrent requests; corrupted DEM tiles in profiles; deadlocks around a `Mutex<Dataset>`; tokio runtime stalls during GIS import (blocking call on the async runtime).

**Phase to address:**
Service foundation phase (SVC-03) sets the threading + boundary rules; GIS ingestion phase implements dataset pooling. Flag for phase research: verify pinned GDAL version and whether RFC 101 thread-safe mode is available in the shipped build.

---

### Pitfall 7: `/vsicurl/` COG reads treated as fast and reliable

**What goes wrong:**
Windowed reads of Copernicus GLO-30 COGs over `/vsicurl/` work beautifully in a notebook and then, in the service: 10–60 s stalls on cold opens (large COG headers fetched in 16 KB nibbles), transient S3 errors surfacing as generic GDAL read failures, and a known cache/concurrency issue when multiple threads hit vsicurl (OSGeo/gdal#1244). A user clicking "Import GIS data" sees a hung spinner or a half-imported terrain. [MEDIUM — gdal.org virtual-file-systems docs + OSGeo issues, verified]

**Why it happens:**
Defaults are tuned for batch CLI use: 16 MB global LRU cache (`CPL_VSIL_CURL_CACHE_SIZE`), 16 KB minimum range unit, no retries, no persistence. Remote-read failure modes don't exist on the dev machine's warm cache.

**How to avoid:**
- **Local tile cache is mandatory, not optional** (DATA-04): download the AOI window once per project into a local GeoTIFF/GeoPackage; the engine and profile extractor read only local data. `/vsicurl/` is used by the *ingestion* step only, never on the calculation path.
- Tune the ingestion step: `GDAL_INGESTED_BYTES_AT_OPEN` (large COG headers in one GET), `GDAL_HTTP_MERGE_CONSECUTIVE_RANGES=YES`, `GDAL_HTTP_MAX_RETRY`/`GDAL_HTTP_RETRY_DELAY`, raise `CPL_VSIL_CURL_CACHE_SIZE`.
- Make ingestion an explicit **async job with progress + retry + clear failure states** (same job model as calculations, SVC-02), never a synchronous request handler.
- Windows note: GDAL network stack needs CA certs configured (`CURL_CA_BUNDLE`/`GDAL_CURL_CA_BUNDLE`) — a classic "works on Linux, TLS error on Windows" trap.

**Warning signs:**
Import time varies 100× between runs; sporadic "IReadBlock failed" on terrain fetch; calculation latency depends on network; the app is unusable offline even for already-imported projects.

**Phase to address:**
GIS ingestion phase (DATA-01, DATA-04). The "remote reads only at ingestion, local cache on the compute path" rule should be stated in the phase plan as an architectural invariant.

---

### Pitfall 8: GDAL build/link pain on Windows derails the service phase

**What goes wrong:**
The `gdal` crate links C GDAL. On the Windows dev machine this means: finding a GDAL build (vcpkg? conda? gisinternals? OSGeo4W?), matching the crate's supported version range, `gdal_i.lib` vs DLL name mismatches, `PROJ_LIB`/`GDAL_DATA` data-file paths not set at runtime (PROJ silently failing to find transformation grids → wrong or failed reprojection), and DLLs missing from `PATH` for the packaged service. Days can vanish here, at the very start of the milestone.

**Why it happens:**
Milestone 1 already uses `gdal`/`proj` via envi-harness — but a *deployable long-running service* raises the bar: the binary must start on a machine without a hand-configured shell environment, and `proj.db`/GDAL data must resolve from the install layout, not from an activated conda env.

**How to avoid:**
- Pin ONE provisioning route (vcpkg with a pinned baseline, or conda-forge with a lockfile) and document it in README + a bootstrap script; commit the `GDAL_HOME`/`PKG_CONFIG_PATH` recipe.
- At service startup, **self-check**: log GDAL/PROJ versions, verify `proj.db` and `GDAL_DATA` resolve, run one tiny in-memory reprojection; refuse to start (with a clear message) if the check fails — this converts a class of silent-wrong-answer bugs into an obvious boot error.
- Ship data-file paths programmatically (`proj_context_set_search_paths` / `CPLSetConfigOption("GDAL_DATA",…)` relative to the executable) rather than relying on user environment variables.

**Warning signs:**
Reprojection "succeeds" but shifts coordinates by tens of meters (missing grids); service runs from the dev shell but not from Explorer/service manager; `STATUS_DLL_NOT_FOUND` on a clean machine.

**Phase to address:**
Service foundation phase, first plan — before any feature work. Roadmap flag: give this explicit time; it is boring and reliably underestimated.

---

### Pitfall 9: Open-data licensing hygiene — Overture ODbL share-alike and missing attribution

**What goes wrong:**
Overture's **buildings** theme is ODbL (OSM-conflated, plus MS/Google building sets that are themselves ODbL): attribution is required, and derivative *databases* inherit share-alike. Since the 2025-09 release each feature carries `sources[].license`. Dropping this on import — storing bare footprints with no provenance — makes later compliance (or even just knowing what you may publish) impossible to reconstruct. Same class of problem: shipping a map UI without OSM/©OpenStreetMap contributors attribution, or Copernicus/ESA WorldCover credit lines on exported results. [MEDIUM — docs.overturemaps.org attribution + release notes, verified]

**Why it happens:**
It's an internal tool, so licensing feels academic; provenance fields are the first thing cut from an ingestion schema. But ENVI's own house rules mandate clean data hygiene precisely so nothing needs untangling later.

**How to avoid:**
- Ingestion schema keeps **per-feature provenance**: source dataset, `sources[].license`, retrieval date. It's one struct field; add it on day one.
- A static `ATTRIBUTIONS` list rendered in the map UI (MapLibre attribution control: © OpenStreetMap contributors, Overture Maps Foundation, © ESA WorldCover, Copernicus DEM © DLR/Airbus) and stamped into exported GeoTIFF/GeoJSON metadata and plot footers (GRID-05).
- Treat internal-tool status as *reduced* obligation, not zero: the share-alike question only stays moot while nothing is redistributed — provenance keeps that door open.

**Warning signs:**
Ingested features have no source field; exports carry no credits; someone asks "can we share this noise map?" and nobody can answer.

**Phase to address:**
GIS ingestion phase (schema + provenance); map UI phase (attribution control); results/export phase (metadata stamping).

---

### Pitfall 10: WorldCover class → impedance mapping errors

**What goes wrong:**
The ESA WorldCover → Nordtest impedance-class table is a small hand-written mapping, and small hand-written mappings rot: "Built-up" (class 50) mapped wholesale to hard G/H even though it includes gardens and verges; "Tree cover" mapped to forest *floor* impedance while forgetting that forests also need the separate forest-attenuation object; water (80) accidentally soft; and the already-caught project gotcha of transcription errors in the σ table itself (class B = 31.5, corrected once — the same class of error can recur here). 10 m raster resolution also smears narrow roads into surrounding grass, so paved surfaces near sources — where ground effect matters most (NoizCalc: "especially near sources and receivers") — get mis-classed soft.

**Why it happens:**
The mapping crosses two domains (remote sensing classes ↔ acoustic flow resistivity) with no natural oracle; nobody reviews a 11-row match statement.

**How to avoid:**
- Encode the mapping as a **reviewed data table** (one TOML/const table: WorldCover class → Nordtest class A–H → σ), with a unit test asserting the full table against the values documented in the data-sources research; cite the source per row.
- Default-ground semantics copied from NoizCalc: user picks **urban (hard) / rural (soft)** default; WorldCover fills in deviations; user-drawn ground-effect polygons override everything (priority: drawn > imported > default).
- Near-source hard surfaces: encourage (in UI copy and docs) drawing an explicit hard-ground polygon for stages/parking, rather than trusting 10 m raster near the source.
- Cross-check one real site's segmentation visually (colored impedance overlay on the map) before trusting batch results — an impedance-class debug layer is cheap and permanently useful.

**Warning signs:**
Ground-effect dips absent over obvious asphalt; a whole city block classed "C — loose soil"; per-band levels shift wildly when toggling default ground (means too much area rides the default).

**Phase to address:**
GIS ingestion phase (DATA-02/GEOX-02) for the table + test; map UI phase for the impedance debug overlay.

---

### Pitfall 11: Open-Meteo free-tier limits and weighted call counting

**What goes wrong:**
Free tier: non-commercial use, <10 000 calls/day, 5 000/h, 600/min — but the counting is **weighted**: a request with >10 weather variables or >2 weeks of data counts as multiple calls. ENVI's met needs (multi-level winds/temps at many pressure levels, cloud, BLH, soil) hit the >10-variables multiplier on nearly every request. A weather what-if UI that refetches per slider tick, or an L_den batch iterating hourly history, burns the budget fast; 429s then surface as "weather import broken." [MEDIUM — open-meteo.com pricing/terms, verified]

**Why it happens:**
"10k/day is plenty" intuition ignores the multiplier and ignores that UI interactivity multiplies request counts by 10–100× vs a batch script.

**How to avoid:**
- **Cache aggressively per project**: one met fetch per (site, time window), persisted with the project; the A/B/C derivation and all what-ifs run off the cached profile — the what-if feature is *defined* as manual override of already-imported data (PROJECT.md), so zero API calls per what-if. Enforce that in the design, don't rely on restraint.
- Batch variables into the fewest requests; log a computed "weighted call cost" per fetch so budget use is visible.
- Decide the commercial-tier question consciously in the weather phase: the tool is personal/internal, which fits the non-commercial wording today — record that judgment (and the tier price) in the phase decision log rather than discovering it at a 429.
- ERA5/CDS (weather classes): different failure mode — queued retrievals can take minutes to hours; must be an async job with status, never a request-response call.

**Warning signs:**
429 responses; weather import latency growing over a session; network tab shows a met request per UI interaction.

**Phase to address:**
Weather import phase (METX-01/02). The "what-ifs never call the API" rule belongs in that phase's success criteria.

---

### Pitfall 12: Non-commercial datasets creep back in (FABDEM, Meteostat)

**What goes wrong:**
FABDEM is exactly the fix for Pitfall 3 (GLO-30 with forests and buildings removed) and Meteostat is a convenient station-history API — both are CC-BY-NC and both are explicitly excluded by project policy. They tend to re-enter via a helpful tutorial, a transitive Python tool, or "just for validation."

**How to avoid:**
The exclusion is already in PROJECT.md/REQUIREMENTS.md; repeat it in the ingestion phase plan's anti-requirements, and code the DEM-source list as a closed enum (GLO-30, national LiDAR tiers) so adding a source is a reviewed code change, not a URL string.

**Warning signs:**
Any URL containing `fabdem` or `meteostat` in the codebase; a "better DEM" suggestion in a phase discussion without a license check.

**Phase to address:**
GIS ingestion + weather phases (anti-requirement in both plans).

---

### Pitfall 13: mapbox-gl-draw reflex, and Terra Draw state fighting React

**What goes wrong:**
Two frontend traps. (a) Most drawing tutorials reach for `mapbox-gl-draw`, which is broken on MapLibre GL JS v3+ (already flagged in OPEN-GIS-LANDSCAPE.md) — hours lost before rediscovering Terra Draw. (b) With Terra Draw + react-map-gl, known integration failures: "Terra Draw is not enabled" when the instance is used before `draw.start()` / map load (terra-draw#172), and **drawings disappearing on map re-render** — React re-creates or restyles the map, MapLibre drops the GeoJSON sources Terra Draw manages, then `setData` throws on undefined sources (terra-draw#197). React 18 StrictMode double-mounting doubles the pain. [MEDIUM — terra-draw issue tracker, verified]

**Why it happens:**
Terra Draw owns imperative map state; React re-renders declaratively. Any pattern that stores drawn features in React state and pushes them back into Terra Draw per render creates a feedback loop; any style change (`setStyle`, basemap switch) silently deletes Terra Draw's layers.

**How to avoid:**
- **Terra Draw instance lives in a ref**, created once in the map's `onLoad`, `draw.start()` before any mode use; never re-created on render. Prefer the `maplibre-gl-terradraw` control (watergis) added via `map.addControl` — it packages the adapter/mode wiring.
- **The scene store is the source of truth, outside React render state** (Zustand/valtio or a plain module store): Terra Draw `finish`/`change` events → update store → sync to backend. Re-hydrate Terra Draw from the store on `style.load` (basemap switches) — write that re-hydration handler in week one, because basemap switching WILL be added.
- `reuseMaps` on react-map-gl `<Map>`; treat any `setStyle` call as a "re-add all custom sources/layers" event (this applies equally to the isophone result layers, not just drawings).
- Keep react-map-gl at v8.x (the MapLibre-native line) — v8 splits `react-map-gl/maplibre` imports; mixing mapbox-flavored examples brings in token/version mismatches.

**Warning signs:**
Drawn walls vanish on basemap toggle; "Cannot read properties of undefined (reading 'setData')"; duplicated features in dev (StrictMode); drawing works until the first hot-reload.

**Phase to address:**
Map/drawing UI phase (WEB-01..04). Roadmap flag: this phase needs a small architecture spike (map lifecycle + store) before feature plans; it is the highest-churn integration in the milestone.

---

### Pitfall 14: Rendering results as a MapLibre `heatmap` instead of isophone fill polygons

**What goes wrong:**
MapLibre's `heatmap` layer is a kernel-density estimator over *point weights* — it renders "where points are dense," is zoom-dependent, and has no calibrated mapping from color to value. Used for noise levels it produces a pretty, meaningless blob: colors change with zoom, interpolation is in screen space, and no contour corresponds to any dB threshold. For a tool whose core value is a *trustworthy* calculation, this quietly destroys the result's meaning at the last mile.

**Why it happens:**
It's the path of least resistance: receiver-grid results are points, the heatmap layer eats points, demo looks great.

**How to avoid:**
Already locked, keep it locked: **server-side contouring** (GDAL `GDALContourGenerate` on the dB grid → filled isophone polygons at the color-scale class breaks) served as GeoJSON/vector tiles and rendered as `fill` layers with a fixed class↔color mapping. The renderer never interpolates values — it only paints classed polygons. Any client-side value interpolation is a spec violation, not a style choice.

**Warning signs:**
A `type: "heatmap"` layer anywhere in the style; colors that shift while zooming; a legend whose values can't be traced to contour breaks.

**Phase to address:**
Results/grid phase (GRID-04, WEB-06). Put "no client-side value interpolation; heatmap layer forbidden" in the phase's anti-requirements.

---

### Pitfall 15: Tensor memory blindness — the ~GB transfer cache eats the service

**What goes wrong:**
The frozen contract is `H[sub_source, receiver, freq]` `Complex<f64>` (16 B/element), 105 freq points. A 100k-receiver grid with 10 sub-sources = 100 000 × 10 × 105 × 16 B ≈ **1.7 GB per weather scenario per project**. A long-running service that materializes the whole tensor per job — or worse, keeps tensors of several open projects resident, or clones it for the MAC pass — OOMs on the self-hosted box, taking every user's session with it. Serializing it naively (JSON!) multiplies size ~6×.

**Why it happens:**
Milestone 1's harness handles a handful of receivers; the dense-`ndarray` mental model carries over to grids where it no longer fits. Memory failures appear only at realistic grid sizes, i.e. after "everything works."

**How to avoid:**
- **Receiver-chunked everything** (OUT-06 is already the requirement — the pitfall is honoring it end-to-end): propagation writes chunks, the MAC recalc streams chunks, persistence stores chunks (memory-mapped binary or chunked file layout, freq-contiguous as frozen), levels reduce per chunk. The full tensor never needs to be resident; the MAC output (`p[r,f]`, then per-receiver levels) is small.
- **Explicit memory budget as a job parameter**: estimated tensor size computed *before* the run from (grid points × sub-sources × 105 × 16 B) and shown to the user with the grid settings; refuse or warn above budget.
- Persist tensors as raw little-endian `Complex<f64>` chunks + a small header (or Zarr-like layout) — never a text format. Memory-map for the MAC fast path.
- One resident-tensor LRU across the service (projects evict), not per-session ownership.

**Warning signs:**
Service RSS grows per job and never shrinks; a grid-size increase turns a 2 GB process into a 10 GB one; "save project" takes minutes (text serialization).

**Phase to address:**
Grid-compute phase (GRID-02 + OUT-06 integration). The chunked tensor store format deserves its own plan; it is the load-bearing artifact of the whole fast-recalc promise.

---

### Pitfall 16: Grid-resolution cost surprise and jobs you can't cancel

**What goes wrong:**
Grid cost scales with 1/spacing²: **halving grid distance quadruples calculation time** (NoizCalc §3.3.1; its guidance: 5 m with buildings/walls … 20 m free field). A user drags spacing from 10 m to 1 m and launches a job that runs for hours — and if the job model has no cancellation or progress, the only recovery is restarting the service (losing everyone's jobs). NoizCalc ships duration statistics and an Abort button for exactly this reason.

**Why it happens:**
The compute-job model gets built for the happy path (submit → done); cancellation and progress are "polish" that never lands, because on dev-sized scenes every job finishes in seconds.

**How to avoid:**
- **Pre-run cost estimate** in the calculation dialog: receiver count, tensor size (Pitfall 15), and a time estimate extrapolated from a calibration constant (paths/sec measured on the host) — shown *before* Run.
- **Cancellation token checked at chunk boundaries** in the propagation loop (natural fit with the chunked tensor), plus per-chunk progress events (receivers done / total) streamed to the UI (SSE or polling — SSE is enough for a single-user tool).
- Sane defaults + guardrails: default 10 m; soft warning above ~50k receivers; hard confirm above the memory/time budget.
- Job state machine includes `Cancelled` and `Failed(reason)` from day one — retrofitting states into a persisted job model is painful.

**Warning signs:**
No way to know if a running job is 5% or 95% done; users kill the service to stop a job; grid settings UI has no feedback tying spacing to cost.

**Phase to address:**
Service foundation phase defines the job state machine (SVC-02); grid-compute phase implements progress/cancel checkpoints. This is a phase-ordering constraint: job model before compute integration.

---

### Pitfall 17: Fast-path confusion — what may skip propagation, and stale-tensor lies

**What goes wrong:**
Two symmetric failures around the complex-MAC fast path:
1. **Too slow:** a source filter/delay/level tweak triggers a full propagation re-run — throwing away the milestone's core architectural win (interactive reconditioning via `p[r,f] = Σ_s H[s,r,f]·G_s(f)`).
2. **Too fast (wrong):** a change that *does* alter propagation — geometry moved, terrain edited, ground polygon changed, **or the weather what-if's A/B/C edits** — is served from the cached tensor, silently returning results for the old scene. A stale-but-plausible noise map is the worst possible failure for a trust-critical tool.

The subtle case is weather: A/B/C are *propagation* inputs (refraction), so a what-if strictly invalidates `H`. "Fast weather what-if" is achievable only as **cache-per-weather-scenario** (compute H per (A,B,C) scenario, switch instantly between cached scenarios; MAC path untouched within a scenario) — not by pretending weather is a conditioning gain.

**Why it happens:**
The dependency classification (conditioning vs propagation input) lives in developers' heads, not in the API. Any endpoint that "recomputes results" without declaring which inputs changed will eventually take the wrong path.

**How to avoid:**
- **Encode the classification in the API shape**: separate endpoints/operations — `recondition` (accepts only G_s(f): filter, delay, level; always MAC) vs `recompute` (anything touching scene/terrain/ground/met; always propagation). No endpoint accepts both kinds of input.
- **Tensor cache keyed by content hash** of (scene geometry, terrain model, ground segmentation, met A/B/C, band grid, engine version). Every scene mutation bumps the hash; a MAC request whose hash ≠ cached hash is rejected with "propagation outdated," never silently served. UI shows a "results stale" badge the moment the scene diverges from the cached tensor.
- Weather what-if = named scenarios, each mapping to its own cached tensor; the UI's Beaufort/downwind/gradient controls edit a scenario, and switching scenarios is instant only if that scenario's tensor exists.
- Regression test: recondition-vs-full-recompute equivalence (`MAC(H, G)` ≡ full run with conditioned sources) on a small scene — bit-level-ish agreement is the proof the fast path is *the same physics*.

**Warning signs:**
Filter slider takes seconds (path 1); results unchanged after moving a wall (path 2); no visible staleness indicator; a single `POST /calculate` endpoint with a grab-bag body.

**Phase to address:**
Service foundation phase (API contract — this is THE central API-design decision of M2); grid/results phase for the staleness UX; weather phase for scenario-keyed caching. Roadmap flag: get this contract reviewed before any UI binds to it.

---

### Pitfall 18: Acoustic summation and weighting re-implemented (wrongly) in the UI layer

**What goes wrong:**
The engine's readout contract is exact: total level = |Σ coherent (phase intact)|² + P_incoh, per band, then weighting, then dB. The UI layer offers many tempting places to break it: JavaScript summing per-source dB magnitudes ("+3 dB per doubling" folklore) for a "quick total"; applying dB(A) weighting to an already-summed broadband number; averaging dB values spatially when downsampling the grid for display; recomputing dB(C) client-side from dB(A) plus an offset. Each yields numbers that look right within a few dB — precisely wrong enough to survive review.

**Why it happens:**
The frontend team (or future-you in the frontend) needs a number *now* and the server round-trip feels heavy; weighting tables are easy to paste from the internet (defined at nominal 1/3-oct frequencies — see Pitfall 19).

**How to avoid:**
- **Iron rule: every acoustic number displayed is computed server-side by the engine/service.** The frontend formats and colors; it never sums, weights, or averages levels. Write this into the UI phase spec as an anti-requirement and enforce it in review (grep the frontend for `10 * Math.log10` / `Math.pow(10,`).
- dB(A)/dB(C) weighting: computed in Rust with the analytic IEC 61672 response evaluated **at the exact 1/12-octave centre frequencies of the 105-point grid** (not table lookup at nominal frequencies), applied per band *before* energy summation to broadband. Both weightings computed server-side per result; the UI toggle just selects which precomputed field to display.
- Spectral readout at a receiver point (per-band panel) ships per-band levels + the correctly summed broadband values in one payload, so the UI never needs to derive one from the other.

**Warning signs:**
Any log/pow arithmetic in the JSX; dB(A)↔dB(C) toggle that responds without a data fetch (unless both fields were shipped); broadband ≠ engine-computed broadband on spot checks.

**Phase to address:**
Results phase (WEB-05/06/07 + GRID-03). The "server computes, client displays" rule goes in the first UI phase's conventions so it governs all subsequent UI work.

---

### Pitfall 19: Band-index vs nominal-frequency mismatch crossing the API

**What goes wrong:**
The engine's known internal pitfall — compare by BAND INDEX, never nominal frequency — now crosses a serialization boundary. The service exposes spectra; the frontend (or an export consumer) matches bands by nominal Hz labels ("1000 Hz"), which don't exactly equal the exact-grid centres (10^(3/10)-ratio grid, ~25.12 Hz–10 kHz). Off-by-one band joins, wrong weighting values (weighting evaluated at nominal instead of exact frequency), and export files whose frequency column disagrees with the engine's grid by fractions of a percent that break downstream exact joins.

**Why it happens:**
JSON APIs naturally serialize human-readable frequencies; nominal labels are what users expect to see; the invariant was enforced by convention inside one crate and the convention doesn't survive the hop to TypeScript.

**How to avoid:**
- The API's canonical spectrum representation is **`{ band_index, value }`** (or a dense array indexed by band), with the exact frequency grid published once at `GET /freq-grid` (band_index → exact Hz → nominal label → is_third_octave flag). Frontend uses nominal labels for *display only* and band indices for *all* data operations.
- Exports (GeoJSON/GeoPackage/CSV) carry both band index and exact frequency columns.
- A shared TypeScript type generated from (or tested against) the Rust definition, so the 105-point grid length and indexing are asserted on both sides.

**Warning signs:**
Frontend code keying objects by frequency floats; a spectrum panel with 105 bars but 1/3-octave labels misaligned; weighting curves applied at 1000 vs 997.7 Hz style discrepancies.

**Phase to address:**
Service foundation phase (API contract, alongside Pitfall 17's endpoints).

---

### Pitfall 20: Misleading contour intervals and color scales

**What goes wrong:**
The noise map's credibility lives in its legend. Failure modes: contour breaks generated at different values than the legend's class colors (GDAL contour levels drifting from the UI scale after an edit); autoscaling the color range to the data (min/max) so the same source looks "loud" or "quiet" depending on the scene extent, and threshold-relevant contours (e.g. a 45 dB(A) night limit) fall between breaks; smoothing/Bezier-styling contours until they cross each other or move levels visually by meters; rendering dB(A) results under a legend still titled dB(C).

**Why it happens:**
Contouring (server) and legend (client) are built by different code at different times; autoscale is the easy default; NoizCalc's careful treatment (user-editable interval start/size/count, unit variable `<su:>` bound to the selected weighting, non-constant intervals allowed) shows how much deliberate design this needs.

**How to avoid:**
- **Single source of truth for class breaks**: the color scale (start, interval, count, per-class colors — user-editable, per the milestone's "editable color scale" feature) lives in the project; the server contours at exactly those breaks; the legend renders exactly those breaks. Changing the scale re-contours (cheap: grid is cached, only GDALContourGenerate re-runs).
- Default scale fixed and round (e.g. 35–85 dB in 5 dB classes), not data-driven; autoscale offered as an explicit action, never the silent default.
- Legend always displays the weighting unit from the result metadata (dB(A)/dB(C)), never from UI state alone.
- If contour smoothing is offered at all, keep it presentation-only and default-off; the calculated polygons are the artifact of record (exports use unsmoothed geometry).

**Warning signs:**
Legend colors not matching polygon fills at boundaries; the same project rendering different color ranges after adding a distant receiver; exported polygons differing from displayed ones.

**Phase to address:**
Results phase (GRID-04, WEB-06).

---

### Pitfall 21: Rebuilding desktop-era NoizCalc features instead of the workflow

**What goes wrong:**
TI 386 chapter 4 is full of features that made sense for a 2019 desktop print-oriented tool and are scope traps for a web tool in 2026: PDF sheet layout with sheet sizes/north arrows/description blocks (§4.6.1–4.6.7), scanned-bitmap geo-referencing (§4.2.4), a 16-color palette editor with drag-interpolation (§4.6.6), DXF/Shapefile import (§4.2.5), right-angle drawing modes and F-key bindings. Building these early consumes the milestone while the actual workflow (import → model → calculate → read results) stays half-finished.

**Why it happens:**
TI 386 is the workflow model, and it's natural to treat the whole document as the spec rather than chapters 3's *workflow* as the spec and chapter 4 as historical detail. Some ch-4 items are load-bearing (elevation objects §3.2.2/4.3, clickable calculation messages §3.3.2, editable color scale §4.6.5) — the trap is failing to separate those from print-layout nostalgia.

**How to avoid:**
- Explicit v2 anti-requirements list derived from TI 386: no print-sheet layout engine, no bitmap geo-referencing, no palette editor beyond a class-color picker, DXF import stays FUT-01 (future), no keyboard-mode parity. Web-native equivalents suffice: browser print/PNG export of the map view; basemaps replace scanned bitmaps.
- Keep the load-bearing ch-3/4 items IN scope on purpose: elevation points/lines (Pitfall 4), pre-calc validation messages with click-to-object (Pitfalls 4/5), editable interval color scale (Pitfall 20), calculation-area object (§3.2.4 — bounds the grid and the cost, feeds Pitfall 16), property carry-over from last-entered object (§3.2 note — cheap, big modeling-speed win).

**Warning signs:**
A phase plan containing "sheet settings" or "north arrow"; DXF parsing appearing before the noise map works end-to-end.

**Phase to address:**
Milestone-2 requirements/roadmap definition (now) — encode the anti-requirements before phases are cut.

---

### Pitfall 22: GPL contamination from NoiseModelling while building the GIS pipeline

**What goes wrong:**
M2 builds exactly the parts where NoiseModelling is the architectural reference (Delaunay receiver grids, cut profiles, isosurface generation). Under deadline pressure, "port the idea" degrades into "translate the Java" — and a GPL-derived function in a non-GPL codebase is a contamination that is hard to prove absent after the fact.

**How to avoid:**
Existing policy, restated where it now becomes acute: read NoiseModelling for *interfaces and algorithms*, write implementations from the algorithm description or the underlying papers/standards; never side-by-side translate source. When a NoiseModelling behavior is used as a reference, cite it in the code comment as a *behavioral* reference ("matches NoiseModelling's receiver-grid density heuristic, reimplemented from description"). Use its output as a **cross-validation baseline** (VAL-03 pattern) — comparing numbers is always safe.

**Warning signs:**
Variable names or structure eerily matching `noisemodelling-pathfinder` classes; a commit touching grid generation shortly after someone "checked how NoiseModelling does it" with no design note in between.

**Phase to address:**
All GIS-pipeline phases (grid, contouring, profiles); one line in each phase plan's conventions.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Skip the tensor-cache content hash; invalidate "manually" | Days saved on cache plumbing | Stale-results bugs that destroy trust in the tool (Pitfall 17) | Never |
| JSON-serialize the transfer tensor | No binary format design | ~6× size, minutes-long saves, OOM on load (Pitfall 15) | Only for <1k-receiver debug dumps |
| Frontend computes "quick" dB totals | Snappy demo | Wrong numbers users screenshot and share (Pitfall 18) | Never |
| Single `/calculate` endpoint, inputs mixed | One handler | Fast-path/full-path confusion becomes unfixable API surface (Pitfall 17) | Never |
| Synchronous GIS import in the request handler | No job plumbing | Hung UI on slow COG reads; timeouts (Pitfall 7) | First spike only, behind a flag |
| Hardcode EPSG:32633 (or dev-site zone) | No auto-UTM logic | Every non-dev-site project silently wrong (Pitfall 1) | First spike only; must carry a `TODO(GEOX-04)` |
| Skip per-feature provenance on import | Simpler schema | License compliance unreconstructable (Pitfall 9) | Never — it's one field |
| Drop drawn-object re-hydration on style change | Basemap switching "works" (once) | All drawings vanish on first basemap toggle (Pitfall 13) | Never |
| No cancellation in job runner | Simpler runner | Service restarts to kill jobs; lost state (Pitfall 16) | Never past the first compute integration |
| Data-driven (min/max) color autoscale as default | No scale design | Incomparable maps, hidden threshold exceedances (Pitfall 20) | Never as default; fine as an explicit action |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| MapLibre/Terra Draw ↔ React | Drawn features in React state, re-pushed per render; instance re-created on re-render | Instance in a ref created on map load; external scene store; re-hydrate on `style.load` (terra-draw#172/#197) |
| MapLibre basemap | `setStyle` for basemap switch, losing custom sources (drawings + isophones) | Treat style change as an event that re-adds all custom sources/layers; `reuseMaps` |
| GDAL (Rust `gdal`) | One shared `Dataset` behind a `Mutex` in async handlers | Dataset per worker/request; all GDAL via `spawn_blocking`; RFC 101 thread-safe handles where GDAL ≥ 3.10 |
| PROJ / EPSG codes | Bare `EPSG:4326` transforms assuming lon,lat | `OAMS_TRADITIONAL_GIS_ORDER` everywhere; landmark round-trip test |
| Copernicus GLO-30 (`/vsicurl/`) | Remote reads on the calculation path | Ingestion-time AOI download to local cache (DATA-04); tuned retries/header-ingest; async job with progress |
| Overture (GeoParquet) | Dropping `sources[].license`; assuming permissive license (buildings are ODbL) | Keep per-feature provenance; static attribution list in UI + exports |
| ESA WorldCover | Ad-hoc class→σ match statement | Reviewed, tested mapping table; user-drawn overrides win; impedance debug overlay |
| Open-Meteo | Fetch per UI interaction; ignoring weighted call counting (>10 vars = multiple calls) | One cached fetch per (site, window); what-ifs operate on cached/overridden values only |
| ERA5 / CDS | Treating retrieval as request-response | Async job; retrievals can queue for hours |
| Building heights | Trusting `height` to exist | Explicit fallback chain: measured → `levels×3+1.5` → regional default, with per-building provenance flag and visual QA tint for defaulted heights |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Full tensor resident in RAM | RSS ≈ receivers×subs×105×16B per scenario; OOM | Chunked store + streaming MAC (OUT-06); memory budget precomputed per job | ~50k receivers × 10 subs (≈0.8 GB) on a typical self-hosted box |
| Grid spacing halved casually | Runtime ×4 per halving (NoizCalc §3.3.1) | Pre-run cost estimate; default 10 m; confirm-gate below 5 m / above receiver cap | 1 m spacing on a 1 km² area = 10⁶ receivers |
| Isophone GeoJSON with full-precision coords | Multi-MB payloads; slow map | Coordinate precision trim (~1e-6°); simplify presentation copy only; vector tiles if polygons exceed ~10 MB | City-scale maps with 1 dB intervals |
| Per-receiver React markers | DOM node per receiver; frozen UI | Single GeoJSON source + circle layer for receivers/grid | ~1k markers |
| Re-contouring by re-running propagation | Color-scale edit takes minutes | Persist the level grid; scale edits re-run only GDALContourGenerate | Always — it's a design error, not a scale limit |
| Weather what-if triggers full recompute each tick | Slider unusable | Scenario-keyed tensor cache; what-if edits a scenario, switch is instant when cached | Any interactive use |
| vsicurl on every profile extraction | Calculation latency = network latency ×paths | Local DEM cache is the only compute-path source | First multi-path calculation |

## Security Mistakes

Light/no auth is accepted for this self-hosted internal tool — these are the mistakes that still matter in that posture.

| Mistake | Risk | Prevention |
|---------|------|------------|
| Binding the service to `0.0.0.0` by default with no auth | Anyone on the LAN can read/delete projects, submit GB-scale jobs | Default bind `127.0.0.1`; require an explicit config flag (documented) to expose on LAN |
| Accepting arbitrary URLs for DEM/data sources (`/vsicurl/` SSRF) | Server fetches attacker-chosen internal URLs | Closed enum of data sources (also Pitfall 12); no user-supplied fetch URLs |
| Project names/paths concatenated into filesystem paths | Path traversal → read/write outside the projects dir | Project IDs are generated (UUID/slug); names are metadata only, never path components |
| Unbounded job submission | Trivial DoS of the single host (memory/CPU) | Job queue with concurrency 1–2, per-job memory budget (Pitfall 15), receiver-count cap |
| Trusting uploaded/imported GeoJSON geometry | Degenerate polygons (self-intersecting, 10⁶ vertices) crash contouring/triangulation | Validate + repair on ingest (`geo` validity checks, vertex caps); reject with a message, don't crash |
| Serving the cached tensor files under the web root | Accidental exposure of multi-GB internals; disk exhaustion via download loops | Tensor store outside static-serving; results exposed only via API endpoints |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Silent stale results after scene edits | User trusts an outdated noise map — worst-case failure for this tool | Results-stale badge driven by the tensor cache hash (Pitfall 17); results panel shows "computed for scene version X" |
| Validation errors as a log dump | User can't find the offending polygon | NoizCalc §3.3.2 pattern: clickable messages that select + zoom to the object |
| No cost feedback on grid settings | Accidental multi-hour jobs | Live receiver-count / memory / time estimate next to the spacing control (Pitfall 16) |
| Ground-effect defaults invisible | User doesn't realize 90% of the site rides "rural" default | Impedance overlay toggle showing effective class everywhere; default-ground picker at project creation (NoizCalc §3.1.4) |
| Buried-source warnings only in a report | Wrong near-field results shipped | Inline map badge on any object below terrain, at placement time (Pitfall 4) |
| dB(A)/dB(C) toggle ambiguity | Screenshot shows numbers with wrong implied weighting | Weighting stamped in legend title from result metadata; unit rendered in every readout (NoizCalc's `<su:>` pattern) |
| Drawing modes without property carry-over | Tedious re-entry of identical walls/buildings | Copy properties from last object of same type (NoizCalc §3.2 note) |
| Free-draw everything | Misaligned rectangular buildings | Right-angle/rectangle modes for buildings (Terra Draw has rectangle/angled modes) — but don't chase full NoizCalc key-binding parity (Pitfall 21) |

## "Looks Done But Isn't" Checklist

- [ ] **CRS boundary:** Works at the dev site — verify a southern-hemisphere AND a UTM-zone-boundary site round-trip (Pitfalls 1–2; NoizCalc's own troubleshooting §5 is literally "southern hemisphere import fails").
- [ ] **GIS import:** Imports the demo AOI — verify behavior on network failure mid-import, empty Overture tile, all-nodata DEM window, and re-import over an existing model.
- [ ] **Drawing:** Draws polygons — verify survival across basemap switch, hot-reload, StrictMode double-mount, and re-open of a saved project (Pitfall 13).
- [ ] **Ground polygons:** Renders — verify partial-overlap rejection and containment-wins resolution actually reach the impedance segmentation (Pitfall 5).
- [ ] **Buildings:** Footprints show in 3D — verify height fallback chain fires (feature with no height, no levels) and base elevation isn't sampled from DSM-under-building (Pitfall 3).
- [ ] **Calculation:** Completes on 1k receivers — verify 100k receivers stays within the memory budget, is cancellable mid-run, and reports progress (Pitfalls 15–16).
- [ ] **Fast path:** Filter change is fast — verify MAC result ≡ full recompute on the same conditioning, and that a wall move invalidates the tensor (Pitfall 17, both directions).
- [ ] **Results:** Map renders — verify legend breaks == contour breaks == color classes after a user edits the scale; verify weighting label follows result metadata (Pitfall 20).
- [ ] **Spectra API:** Returns 105 values — verify consumers key by band index, weighting evaluated at exact centres, exports carry exact + nominal frequency (Pitfalls 18–19).
- [ ] **Weather:** Imports once — verify what-if interactions issue zero API calls and the weighted call cost is logged (Pitfall 11).
- [ ] **Deploy:** Runs from `cargo run` — verify the packaged binary starts on a clean Windows machine (DLLs, `proj.db`, `GDAL_DATA`) with the startup self-check green (Pitfall 8).
- [ ] **Attribution:** Basemap credits show — verify Overture/WorldCover/Copernicus lines in the attribution control AND in exported artifacts (Pitfall 9).

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| CRS/axis-order bug shipped (1–2) | MEDIUM | Fix at boundary; bump tensor-cache hash + project schema version to force recompute of every affected project; add the landmark test that was skipped |
| DSM bias discovered on a real site (3) | LOW–MEDIUM | Re-ingest with corrected building-base sampling; user flattens venue with elevation lines (4); document GLO-30 caveat |
| Ground-overlap silently mis-resolved (5) | MEDIUM | Implement rejection + containment rule; migration pass flags existing projects with crossings for user repair |
| GDAL thread corruption in prod (6) | MEDIUM | Serialize all GDAL behind a dedicated thread as a hotfix; then per-worker datasets properly; audit past results only if reads were on the compute path |
| Tensor OOM crashes (15) | HIGH if store format must change | Introduce chunked store; version the tensor file format header from day one so migration is read-old/write-new |
| Stale-tensor wrong results found (17) | HIGH (trust) | Add content-hash invalidation; invalidate ALL cached tensors; changelog note to users; add the equivalence regression test |
| Frontend dB math discovered (18) | LOW–MEDIUM | Delete client math, ship server fields; diff screenshots to assess damage; add the grep gate to review checklist |
| GPL-tainted function found (22) | HIGH | Clean-room rewrite from the algorithm description by someone who hasn't read the Java; document the rewrite |

## Pitfall-to-Phase Mapping

Functional phase names — bind to concrete numbers when the M2 roadmap is cut (UI phases append from Phase 5). Suggested order: Service foundation → GIS ingestion → Map & drawing UI → Weather import → Grid compute & results. (Foundation must precede everything; ingestion before drawing only for the DGM/import features — drawing UI can overlap ingestion if the scene store lands first.)

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| 1. lon/lat vs metric CRS | Service foundation (GEOX-04) | Newtype boundary compiles; degree-magnitude assertion test |
| 2. PROJ axis order | Service foundation | Landmark round-trip test in CI-equivalent test suite |
| 6. GDAL thread safety | Service foundation (SVC-03) | `cargo tree` gate (no gdal in engine closure); load test with concurrent imports |
| 8. GDAL on Windows | Service foundation (first plan) | Packaged binary boots with green self-check on a clean machine |
| 16. Job model w/ cancel | Service foundation (SVC-02) | Cancel a running job; state machine has Cancelled/Failed |
| 17. Fast-path API contract | Service foundation | `recondition` vs `recompute` endpoints; MAC≡full-run equivalence test |
| 19. Band-index API | Service foundation | `/freq-grid` endpoint; TS type asserted against Rust |
| 3. DSM bias | GIS ingestion (DATA-01/03) | Building-base sampling rule tested on urban tile; GLO-30 UI caveat |
| 7. vsicurl reliability | GIS ingestion (DATA-01/04) | Compute path reads only local cache (verified by running offline) |
| 9. Overture ODbL/provenance | GIS ingestion + UI + export phases | Provenance field populated; attribution visible in UI + exports |
| 10. WorldCover mapping | GIS ingestion (DATA-02) | Mapping table unit test; impedance overlay spot-check on real site |
| 12. NC-license creep | GIS ingestion + weather (anti-reqs) | Grep for fabdem/meteostat = zero; DEM source enum closed |
| 4. Buried source / DGM | Map & drawing UI + grid-compute gate | Elevation line flattens venue; below-terrain warning fires in test scene |
| 5. Crossing ground polygons | Map & drawing UI (+ pre-calc gate) | Partial overlap rejected at draw time; containment resolves innermost |
| 13. Terra Draw / React | Map & drawing UI (spike first) | Drawings survive basemap switch + reload; StrictMode clean |
| 21. Desktop-feature creep | M2 requirements/roadmap (now) | Anti-requirements list in REQUIREMENTS v2; no ch-4 print features in phases |
| 11. Open-Meteo limits | Weather import (METX-01/02) | What-if issues zero API calls (network log); weighted cost logged |
| 15. Tensor memory | Grid compute & results (GRID-02/OUT-06) | 100k-receiver run within stated budget; RSS bounded across jobs |
| 14. Heatmap temptation | Grid compute & results (GRID-04/WEB-06) | No heatmap layer in style; contours at exact class breaks |
| 18. UI acoustic math | Grid compute & results (+ UI conventions) | Frontend grep for log/pow math = zero; server ships both weightings |
| 20. Color scale/contours | Grid compute & results | Scale edit re-contours; legend==breaks property test |
| 22. GPL contamination | All GIS-pipeline phases (conventions) | Behavioral-reference citation convention followed; review checklist item |

**Roadmap research flags:** the Map & drawing UI phase (Terra Draw/React lifecycle) and the Grid compute & results phase (chunked tensor store format) are the two phases most likely to need their own deeper phase-research; the Service foundation phase's API contract (Pitfalls 17/19) is the highest-leverage design review of the milestone.

## Sources

- Project-local (HIGH confidence): `.planning/PROJECT.md` (constraints, tensor contract, memory budget), `.planning/REQUIREMENTS.md` (v2 scope: DATA/GEOX/METX/GRID/WEB/SVC), `.planning/research/OPEN-GIS-LANDSCAPE.md` (DSM/DTM notes, license flags, frontend picks), `.claude/CLAUDE.md` (band-index rule, complex/phase contract, data hygiene), `docs/references/dbaudio-ti386-1.6-en.md` (§3.2.2 buried SUB/elevation objects, §3.2.3 crossing ground effects, §3.3.1 grid distance ×4 + abort, §3.3.2 clickable messages, §4.6.5 color scale, §5 southern-hemisphere troubleshooting).
- Web-verified this session (MEDIUM per confidence seam; each cross-checked against official docs/issues):
  - GDAL thread-safety: [GDAL multithreading docs](https://gdal.org/en/stable/user/multithreading.html), [RFC 101 raster read-only thread-safety](https://gdal.org/en/stable/development/rfc/rfc101_raster_dataset_threadsafety.html), [georust/gdal](https://github.com/georust/gdal)
  - `/vsicurl/` tuning + issues: [GDAL virtual file systems](https://gdal.org/en/stable/user/virtual_file_systems.html), [GDAL config options](https://gdal.org/en/stable/user/configoptions.html), [OSGeo/gdal#1244 (vsicurl cache/concurrency)](https://github.com/OSGeo/gdal/issues/1244), [OSGeo/gdal#8499](https://github.com/OSGeo/gdal/issues/8499)
  - Terra Draw integration: [terra-draw#172 ("not enabled" with react-map-gl)](https://github.com/JamesLMilner/terra-draw/issues/172), [terra-draw#197 (drawings disappear on re-render)](https://github.com/JamesLMilner/terra-draw/issues/197), [MapLibre terra-draw example](https://maplibre.org/maplibre-gl-js/docs/examples/draw-geometries-with-terra-draw/), [watergis/maplibre-gl-terradraw](https://github.com/watergis/maplibre-gl-terradraw)
  - Overture licensing: [Overture attribution & licensing](https://docs.overturemaps.org/attribution/), [Buildings guide](https://docs.overturemaps.org/guides/buildings/), [2025-09-24 release notes (per-feature `sources[].license`)](https://docs.overturemaps.org/blog/2025/09/24/release-notes/)
  - Open-Meteo limits: [Pricing](https://open-meteo.com/en/pricing), [Terms](https://open-meteo.com/en/terms), [open-meteo#438 (429s)](https://github.com/open-meteo/open-meteo/issues/438), [open-meteo#485 (call-limit discussion)](https://github.com/open-meteo/open-meteo/issues/485)

---
*Pitfalls research for: ENVI Milestone 2 — web GIS UI + service over the Rust Nord2000 engine*
*Researched: 2026-07-08*
