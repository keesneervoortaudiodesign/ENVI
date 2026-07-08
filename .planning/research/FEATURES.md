# Feature Research

**Domain:** Interactive environmental-noise calculation UI (NoizCalc-style) wrapping the ENVI Nord2000 engine — Milestone 2
**Researched:** 2026-07-08
**Confidence:** HIGH (primary source is the local TI 386 transcription, `docs/references/dbaudio-ti386-1.6-en.md`, cross-read against `.planning/PROJECT.md`, `.planning/REQUIREMENTS.md` v2 groups, and `OPEN-GIS-LANDSCAPE.md`; web supplements MEDIUM/LOW as tagged)

> **Scope guardrails baked into every recommendation below:**
> - **Single integrated app** — no ArrayCalc split; source definition is native to ENVI.
> - **Nord2000-only** — every ISO 9613-2 parallel parameter set NoizCalc maintains is dropped.
> - **Web, not Windows desktop** — MapLibre + Terra Draw idioms replace SoundPLAN desktop idioms (F-key chords, sheet layout, bitmap initialization).
> - **Acoustics are done** (Milestone 1) — features here are UI/app/service only; engine gaps are Phase 3 (meteorology) and Phase 4 (transfer tensor + FORCE), noted per feature.

---

## NoizCalc Workflow Reference (TI 386 ch. 3–4), per goal

How the reference product actually flows, and where ENVI deviates. Goal numbers G1–G7 are used throughout this document.

### G1 — Project CRUD
**NoizCalc:** A project is an OS **folder** of files. `File > New project` creates the folder; metadata fields = title (defaults to folder name), project number, engineer, customer, free-text description. Last project auto-loads at startup; recent list; right-click popup for copy/delete/pack. (§4.1)
**ENVI:** Same lifecycle, web-shaped — a project list/landing page backed by server persistence (SVC-01), not folder management. Project = scene + settings + weather scenarios + cached results/tensors.

### G2 — GIS import
**NoizCalc:** Opening the Editor on a new project auto-opens the **Online map data interface**. User navigates/searches to the site, adjusts the **viewport**, picks a bitmap service (Google aerial/map or OSM), sets a **default building height** (used where no height data exists), clicks **Import**. The import pulls: bitmap of the viewport, **elevation data** (Google), and OSM objects — **Buildings, Ground effects, Forests**. A **Digital Ground Model (DGM)** is triangulated automatically; objects are placed onto it. The user must then **check and complete** the imported model (explicitly stated: "the automatic import does a lot of the work, but you need to check the created objects and complete the model"), using the 3D map to sanity-check heights. Default ground = **Urban** (paved/hard) or **Rural** (fields/soft) applies wherever no Ground effect polygon exists. (§3.2.1, §3.1.4, §4.3)
**ENVI:** Same viewport-import → DGM → check-and-complete loop, but the basemap is live MapLibre tiles (no bitmap snapshot), terrain comes from Copernicus GLO-30 / national LiDAR DTM (DATA-01), ground cover from ESA WorldCover mapped to Nordtest impedance classes (DATA-02), buildings from Overture/OSM with the height fallback chain (DATA-03).

### G3 — Weather data import
**NoizCalc:** **Has no weather import at all.** Meteorology is purely manual calculation settings (see G4). This whole goal is an ENVI addition (METX-01, Open-Meteo).

### G4 — Weather what-if
**NoizCalc:** Meteorology lives in the **calculation settings** dialog, not the scene: humidity / air pressure / temperature (air absorption + sound speed → wavelength → screening), and Nord2000-only extras: **Beaufort wind class** (0–12; events evacuated above 4–5), a **Downwind** toggle (hypothetical non-physical worst case — downwind in *every* source→receiver direction, "like a vortex"; TI 386 itself says this suits traffic/industry, and for open-air events specific scenarios such as "two different wind directions or no wind" are more fitting), and a **Temperature gradient** (affects distant receivers). (§3.3.1)
**ENVI:** Presents these same knobs but as editable, importable, **named weather scenarios**, with A/B/C profile coefficients derivable from Open-Meteo (Route 3 / Monin-Obukhov, MET-02/06) and exposed for expert editing.

### G5 — Scene objects
**NoizCalc:** Toolbar object types with left-click coordinate entry, a properties + coordinate-list panel, and these load-bearing UX rules (§3.2, §4.2):
- **Properties are copied from the last object of the same type entered** — enter similar objects successively.
- Right-angle drawing mode for buildings (default; F11 toggle); rectangle-by-frame entry; F2/double-click finishes; ESC deletes an unfinished object; snap-to-edge of neighbouring objects ("capture circuit"); distance indicator in status bar.
- Multi-select (Ctrl+click / frame), joint property edit, move (drag pink diamond), rotate (Ctrl+drag), duplicate, delete, divide line, insert/append/delete points; moved objects **auto re-reference to the DGM**.
- Object catalog: **Ground effects** (Nord2000: impedance class A–H + roughness class N/S/M/L), **Forest** (shape + height, default 10 m; Nord2000: mean tree density + mean stem radius, defaults fitted to ISO spec), **Building** (outline + height above ground + reflection loss 1 smooth / 2 structured façades + reflections on/off + "main building" plot flag; heights estimable from storeys), **Wall** (base line + height, height 0.0 m still screens — screening edge = base line; per-side reflection loss, RLS-90 guide values 1 / 4 / 8–11 dB(A); per-side reflection toggles; **variable height along the wall** with red-marked property-change coordinates and constant-element height jumps), **Elevation points/lines** (contour lines = one elevation; profile lines = per-vertex elevations; delete points + ring an area with an elevation line to flatten a venue; DGM re-triangulates on deselect), **Calculation area** (result extent = calc area if present, else bbox of all objects/DGM), **Stage** (= the source; placed with a click, oriented by angle, spectrum picked from an Emission library or entered per-band with bandwidth 1/3- or 1/1-octave, weighting A/B/C/D, SPL vs L_W type; calibrated to an **SPL at a reference point** ≥ 0.1 m above terrain), plus Text annotations and General lines.
- Validation: **crossing outlines of Ground effects with different values prevent calculation** (containment is fine); a pre-check warns when a source is "buried" by the DGM; calculation messages appear in a panel and **double-clicking a message opens and activates the offending object**.
**ENVI:** All of this maps onto Terra Draw modes (verified: point, linestring, polygon, rectangle, angled-rectangle, freehand + select mode with vertex drag/insert/delete, feature drag/rotate/resize, snapping toCoordinate/toLine, self-intersection validation — MEDIUM confidence, official terra-draw guides) plus React property panels. ENVI adds **explicit receiver points** (NoizCalc has none — results are grid-map only) and replaces the Stage with a **native directional source** (multi-sub-source, per-band L_W, directivity balloons — SRC-01..04).

### G6 — Spectral results at receivers
**NoizCalc:** **Does not offer receiver-point spectra.** Its only result is the grid noise map (single weighted value per grid point). Commercial peers do: iNoise has receiver objects at up to 5 heights with spectral results (MEDIUM confidence, dgmrsoftware.com). This goal is table stakes for ENVI-the-engineering-tool even though the reference product lacks it — it is the natural readout of the transfer tensor (OUT-01..03).

### G7 — Noise map
**NoizCalc:** Grid noise map computed at a **height above ground** with a user-defined **equidistant grid distance** (reasonable 5 m with buildings/walls … 20 m free field; halving distance quadruples time). Settings: **thread count** (default all cores), **highest reflection order** (both standards say **3**), **dB-weighting dB(A) or dB(C)**. Progress statistics shown during the run; **Abort calculation** button; red errors/warnings double-click to the object. Result auto-displays on the **Graphic Plot** tab: filled contour areas with a user-editable **color scale** (program suggests min/max; user sets smallest value, interval magnitude, interval count, ascending order, 16-color palette rows, non-constant intervals allowed), contour-line smoothing (filter bandwidth, exact/smooth Bezier), edge line, and transparency/shading over the background bitmap. Plus a full print/sheet-layout subsystem (sheet size, north arrow, length scale, description block, palette editor with drag-drop color interpolation, PDF/BMP/WMF export). (§3.3–3.4, §4.6)
**ENVI:** Server computes the grid via the tensor batch (GRID-02) and contours it into **isophone fill polygons** (GDAL contour, GRID-04) rendered as MapLibre fill layers with a legend + editable scale. The print/sheet subsystem is explicitly *not* copied.

---

## Feature Landscape

### Table Stakes (Users Expect These)

Missing any of these and the app is not usable as a NoizCalc-class tool. `Engine dep` = which engine phase must exist before the feature produces real output (∅ = none; UI can ship against Milestone 1 engine as-is).

| Goal | Feature | Why Expected | Complexity | Engine dep | Notes |
|---|---------|--------------|------------|-----|-------|
| G1 | Project list + create/open/save/delete, with title/description metadata | NoizCalc §4.1; any tool baseline | LOW | ∅ | Server persistence (SVC-01/03). Project = scene + settings + scenarios + result cache. |
| G1 | Reopen-last / recent projects; duplicate project | NoizCalc auto-loads last project; duplicate feeds scenario workflows | LOW | ∅ | Duplicate replaces NoizCalc's folder "copy/pack". |
| G2 | Viewport-based GIS import: pick area on map → fetch terrain + buildings + ground cover in one action | The defining NoizCalc onboarding move (§3.2.1) | HIGH | ∅ | GLO-30 COG range reads + Overture/OSM + WorldCover (DATA-01..03); server-side, cached (DATA-04); auto-pick UTM CRS (GEOX-04). |
| G2 | Triangulated DGM built from imported terrain, shown under the map; all objects placed on / re-referenced to it | "Objects are only imported onto the DGM" — the DGM is the model's spine (§4.3) | HIGH | ∅ | `spade` CDT server-side. Also provide a **flat-ground default DGM** so drawing works before/without import. |
| G2 | Default building height setting used where import has no height; height fallback chain | NoizCalc import dialog option (§3.2.1) | LOW | ∅ | Overture/OSM `height` → `levels×3m+1.5` → default (landscape doc). |
| G2 | Default ground selector: Urban (hard) / Rural (soft), applied wherever no ground zone is drawn | NoizCalc §3.1.4 — first project decision | LOW | ∅ | Maps to a default Nord2000 impedance class (urban ≈ G, rural ≈ C/D). |
| G2 | Imported objects are ordinary editable scene objects (check-and-complete loop) | TI 386 states the user *must* check and complete the import | MEDIUM | ∅ | No separate "imported" object class; provenance tag only. |
| G4 | Editable meteorology settings: temperature, humidity, air pressure; Beaufort wind class + direction; Downwind worst-case toggle; temperature gradient | Exactly NoizCalc's Nord2000 settings (§3.3.1) — goal 4 verbatim | MEDIUM | **Phase 3** (MET-01..04, ENG-05) | T/RH/p also feed ISO 9613-1 air absorption (engine has ENG-04 already). Downwind toggle = force downwind for every source→receiver azimuth. |
| G5 | Draw + edit all object types on the map: buildings (polygon), walls (line), ground zones (polygon), forests (polygon), elevation points/lines, receivers (point), sources (point + orientation), calculation area (polygon/rect) | The modeling core (§3.2) | HIGH | ∅ | Terra Draw covers every geometry mode incl. rectangle/angled-rectangle for right-angle buildings and select-mode vertex editing/snapping (verified, MEDIUM). |
| G5 | Property side panel per selected object + coordinate/height awareness | NoizCalc's property/coordinate block (§4.2.1) | MEDIUM | ∅ | Standard React state binding: Terra Draw selection event → panel; panel edits → feature properties. |
| G5 | **New objects inherit properties of the last object of the same type** | Explicit NoizCalc productivity rule (§3.2 note) | LOW | ∅ | Per-type "last properties" store. Cheap, huge modeling-speed payoff. |
| G5 | Ground-effect properties: Nord2000 impedance class **A–H** + roughness class **N/S/M/L** as dropdowns with plain-language descriptions | TI 386 tables; engine has these classes pinned (class B σ=31.5) | LOW | ∅ | Show the σ value + description ("G — hard: most normal asphalt, concrete"). Nord2000-only — no G-factor field. |
| G5 | Building properties: height above ground, reflection loss (1 smooth / 2 structured), reflections on/off | §3.2.3 Building | LOW | ∅ (reflection paths use **Phase 3** ENG-06) | Storey-count helper (floors → height estimate) is a nice LOW add. |
| G5 | Wall properties: height (0.0 m still screens), per-side reflection loss with RLS-90 presets (1 / 4 / 8–11 dB(A)), per-side reflection toggle | §3.2.3 Wall | MEDIUM | ∅ (screening exists; reflections **Phase 3**) | MVP = constant height per wall; variable height along wall is deferred (see anti-features). |
| G5 | Forest properties: height (default 10 m), mean tree density, mean stem radius (Nord2000 defaults prefilled) | §3.2.3 Forest — Nord2000 `A = d·a(f)` needs them | LOW | Scattering term must exist in engine (check: not in ENG list — flag for Phase 3/4 scope) | Single trees/lines of trees have no effect — say so in the panel help text. |
| G5 | Elevation points + elevation lines (contour = one z; profile = per-vertex z), DGM re-triangulates on edit; "flatten venue" pattern possible | §3.2.2 — fixing wrong topography is a first-session task ("buried SUB array") | HIGH | ∅ | Re-triangulation server-side; debounce on edit-commit like NoizCalc's "adapts after un-selecting". |
| G5 | Native directional source: position, orientation, per-band spectrum (enter 1/3-octave; library of presets), **SPL at a reference point** calibration, active on/off | Stage workflow minus ArrayCalc (§3.2.5); calibration note: NoizCalc "drives" the system to reproduce SPL+spectrum at the reference point | MEDIUM | ∅ for definition; **Phase 4** (SRC-02..04) for directivity/multi-sub-source evaluation | Reference point must be ≥ 0.1 m above terrain (validation). Spectrum entry: band values + weighting (A/C) + SPL-vs-L_W type, per §3.2.5 "arbitrary spectrum". |
| G5 | Scene validation + message panel: overlapping ground zones with different values block calculation; source-below-DGM warning; **click a message → select the object** | §3.2.3 note + §3.3.2 — NoizCalc's error UX is genuinely good | MEDIUM | ∅ | Run validation pre-submit server-side; return object IDs with messages. |
| G6 | Receiver points with height; per-band spectrum readout + dB(A)/dB(C) totals after calculation | Goal 6; iNoise precedent (receivers, MEDIUM conf.); the tensor's natural readout | MEDIUM | **Phase 3 + 4** (OUT-01..03) | Display at 1/3-octave by default (aggregate the 1/12-oct grid **by band index**, never nominal Hz — house rule). Table + bar chart. |
| G7 | Calculation settings: grid distance (5–20 m guidance), calculation height above ground, highest reflection order (default 3), dB weighting | §3.3.1 verbatim | LOW | Phase 3/4 for the run itself | "Save as default" per NoizCalc. Thread count: auto (server decides) — don't surface cores in a web UI. |
| G7 | Compute-job model: submit, progress %, abort, status; result auto-displays when done | §3.3 (duration statistics, Abort button); SVC-02 | MEDIUM | **Phase 4** (grid batch GRID-02) | Batch model already a project constraint (no streaming). |
| G7 | Grid noise map as **filled isophone contour polygons** on the map, dB(A) or dB(C) | Goal 7; §3.4; project decision (GDAL contour fill polygons, not heatmap) | HIGH | **Phase 4** + GRID-01..04 | Calc-area polygon (or object bbox fallback) defines extent, per §3.2.4. |
| G7 | Editable color scale: min value, interval size, interval count, palette; legend on map | §4.6.5 — NoizCalc auto-suggests scale from min/max, user edits | MEDIUM | ∅ (pure client) | Use ColorBrewer-style sequential ramps (iNoise precedent, MEDIUM conf.). Non-constant intervals = later. |

### Differentiators (Competitive Advantage)

Aligned with the Core Value + the tensor pillar. These are where ENVI beats NoizCalc, not just matches it.

| Goal | Feature | Value Proposition | Complexity | Engine dep | Notes |
|---|---------|-------------------|------------|-----|-------|
| G3 | **Open-Meteo weather import**: pick site + date/time → fetch surface + multi-level met → auto-derive Nord2000 A/B/C per source→receiver azimuth | NoizCalc has *nothing* here — users guess Beaufort classes. ENVI fills the biggest gap in the reference product (METX-01, MET-02/06 Route 3) | MEDIUM (UI) | **Phase 3** (MET-02, MET-06) | UI = date/time picker + "fetch" + a readout of derived wind vector, stability, A/B/C. Per-azimuth math is engine-side; UI shows one wind vector + derived coefficients. |
| G4 | **Named weather scenarios with side-by-side compare** (e.g. "no wind" vs "SW wind 4 Bft" vs "downwind worst case") | TI 386 itself recommends "specific scenarios (two different wind directions or no wind)" for events but offers no scenario mechanism — ENVI operationalizes its own reference's advice | MEDIUM | **Phase 3**; **Phase 4** for cached-scenario instant switching | Each scenario = met parameter set; results cached per scenario; UI toggles between computed maps instantly, and can show a difference map (iNoise "compare models" precedent). |
| G4 | Expert A/B/C profile editing (direct log-lin coefficient override) | Engineering tool honesty — consultants can pin the profile the standard route derived | LOW | **Phase 3** | Behind an "advanced" disclosure; validated ranges. |
| G5/G6 | **Interactive source conditioning: per-source gain/filter/delay sliders with near-instant result update** via the cached complex tensor (MAC, no propagation re-run) | The project's flagship pillar (OUT-03..05, WEB-05). No environmental-noise tool on the market does interactive what-if at this speed — NoizCalc recalculates everything | MEDIUM (UI) | **Phase 4** (OUT-01..06) | Honest scoping: this fast path covers **source conditioning only**. Geometry/weather edits invalidate the tensor → full re-run (make the UI show which edits are "instant" vs "requires recalculation"). |
| G5 | Native **multi-sub-source directional sources** with per-band spherical directivity balloons (CLF import later, FUT-05 SOFA after) | Removes the ArrayCalc dependency entirely — the single-app premise | HIGH | **Phase 4** (SRC-02..04) | MVP = one directional sub-source per source + canned directivity presets (omni, cardioid-ish balloon); array composition UI after validation. |
| G6 | **1/12-octave spectral resolution** readout (finer than NoizCalc/Nord2000-native 1/3-octave), with coherent/incoherent split visible in an expert view | Interference detail (comb filtering between sub-sources/ground bounce) is invisible at 1/3-oct; ENVI's phase-preserving engine makes it real | LOW (UI) | **Phase 4** | Default display stays 1/3-oct (familiar); 1/12 + `|H_coh|²` vs `P_incoh` behind a toggle. |
| G6 | Receiver spectrum export (CSV) + noise-map export (GeoJSON/GeoTIFF/PNG) | Engineering deliverables without a print subsystem (GRID-05) | LOW | Phase 4 | Replaces NoizCalc's sheet/PDF machinery at 5 % of the cost. |
| G2 | Tiered terrain quality: national LiDAR DTM where available, GLO-30 fallback; WorldCover→Nordtest σ automatic ground classes | NoizCalc imports Google elevation + whatever OSM tags as ground — ENVI's ground import is *acoustically classified* out of the box | MEDIUM | ∅ | DATA-01/02. Show provenance per zone ("WorldCover: grassland → class D"). |
| G7 | dB(A)/dB(C) **toggle without recalculation** | Weighting is a readout of the stored per-band grid results, not a new run (NoizCalc buries it in calc settings, implying re-run) | LOW | **Phase 4** (store per-band grid or both weightings server-side) | Requires keeping band-level grid results (or precomputing both weighted rasters) — memory-budget note (OUT-06). |
| G7 | Map-native result presentation: isophone polygons over live basemap with opacity control, hover readout of level at cursor | Web-map idiom beats bitmap-transparency tricks (§4.6.5's transparent/shade workarounds exist only because the desktop app composites bitmaps) | LOW | Phase 4 | MapLibre fill layer + `queryRenderedFeatures` hover. |
| G1 | Autosave with dirty-state indicator | Web-app norm; prevents the desktop "forgot to save" class of loss | LOW | ∅ | Debounced PATCH of scene state. |

### Anti-Features (Things NoizCalc Does That ENVI Should NOT Copy)

| Feature | Why NoizCalc Has It | Why ENVI Shouldn't | Alternative |
|---------|---------------------|--------------------|-------------|
| **ArrayCalc integration** (reference-point defined in a second tool, "Integrate ArrayCalc file", stage = imported loudspeaker system) | d&b sells loudspeakers; ArrayCalc is their system designer | Milestone decision: single integrated app | Native source objects with spectrum + SPL@reference-point calibration inside ENVI (table stakes G5). |
| **ISO 9613-2 standard option** + dual parameter sets (G-factor grounds, ISO forest table, "the import saves both sets") | Regulatory requests | Nord2000-only is a locked decision; dual sets double every property panel and validation path | Single Nord2000 parameter set everywhere. Record ISO/CNOSSOS as out-of-scope (already in PROJECT.md). |
| **Bitmap snapshot workflow** (import Overview.bmp/Detail.bmp, initialize scanned bitmaps with reference points, brighten/contrast, transparent/shaded map compositing) | 2019 desktop app without live tiles | MapLibre gives live, infinitely-zoomable, correctly-georeferenced basemaps for free | Live OSM/aerial raster tile basemap; layer opacity slider for results. |
| **Google Maps data source** (bitmap + elevation) | Licensed by SoundPLAN | Licensing/ToS problem for a self-hosted tool; project data tier already chosen | OSM tiles + Copernicus GLO-30/LiDAR terrain (DATA-01). |
| **Print/sheet-layout subsystem** (sheet size, description block, north arrow, length scale, 16-color palette editor with drag-drop interpolation, WMF export, "print color values") | SoundPLAN heritage; consultants deliver paper plots | Weeks of work orthogonal to the calculation value; web MVP delivers via screen + file export | PNG snapshot of the map view + GeoJSON/GeoTIFF/CSV export (GRID-05). Simple built-in legend. Revisit report generation only if actually needed. |
| **Desktop editing idioms**: F-key chords (F2 finish, F10 view toggle, F11 right-angle), Ctrl+Del, pink-diamond drag handles, "capture circuit" cursor states, wire-frame/hidden-line 3D drawing types | 90s-CAD-derived desktop UX | Web map UX has its own conventions; Terra Draw provides finish/cancel/drag/snap natively | Terra Draw select mode + on-map context toolbar; keep *the capabilities* (finish, cancel, snap, duplicate, divide line) not the key bindings. |
| **Variable wall height along the base line** (red property-change coordinates, "constant wall element" height jumps) | Full SoundPLAN wall model | Disproportionate property-panel + engine-profile complexity for MVP; rare need at event scale | MVP: constant-height walls; split a wall into segments to vary height. Revisit post-validation (the coordinate-level property model is the costly part). |
| **DXF / Shapefile / ASCII import + elevation-point filtering dialog** (§4.2.5) | Consultant data exchange | Real but not Milestone-2-goal-critical; FUT-01 already tracks DXF | Defer to v2.x (FUT-01/02). Auto-decimate imported elevation grids server-side (no user-facing tolerance dialog). |
| **Roads / railways / berms object types** (mentioned via property-change machinery) | SoundPLAN is a traffic-noise suite | ENVI Milestone 2 scope is event/industry sources; road *emission* UI is not among the 7 goals (engine's road model serves FORCE validation, not the UI) | Directional sources only. Road-source UI is a possible future milestone. |
| **Thread-count setting** in calculation dialog | Desktop machine ownership | Server decides its own parallelism; a web user shouldn't tune the host | Auto thread pool; show progress + ETA instead. |
| **"Main building" plot-layout flag** + object-type draw-sequence/legend editor (§4.6.3) | Print-plot styling | Styling-subsystem scope creep | Fixed sensible map styling per object type; per-object color later if ever. |
| **Momentary-snapshot-only results** (NoizCalc computes one condition per run, no scenario memory) | Simple mental model | Wastes ENVI's per-scenario tensor caching; goal 4 explicitly wants what-if | Named weather scenarios with cached results + difference view (differentiator G4). |

## Feature Dependencies

```
G1 Project CRUD (SVC-01/03)
    └──required by──> everything (scene, settings, results all live in a project)

Map shell (MapLibre + basemap)
    └──required by──> G5 drawing ──required by──> G7 calc area, G6 receivers, sources
    └──required by──> G2 import UX

G2 GIS import (DATA-01..04, GEOX-04)
    └──produces──> DGM (spade CDT)
                      └──required by──> G5 elevation editing, object z-referencing,
                                        source-buried-in-DGM validation
    (flat-ground default DGM lets G5 proceed without G2)

G3 Weather import (METX-01) ──feeds──> G4 scenario values
G4 What-if scenarios ──require──> ENGINE PHASE 3 (MET-01..06, ENG-05/06/08)
G4 instant scenario switching ──requires──> ENGINE PHASE 4 (cached tensors per scenario)

G5 sources/receivers/calc-area ──required by──> any calculation run
Calculation run (SVC-02 jobs, GEOX-01..03 real-GIS profiles)
    ──requires──> ENGINE PHASE 3 (meteorology) + PHASE 4 (tensor, grid batch GRID-02)
    └──produces──> G6 receiver spectra (OUT-01..03)
    └──produces──> G7 grid map (GRID-04 contouring ──> isophone polygons WEB-06)

Source conditioning UI (filter/delay/gain, WEB-05)
    ──requires──> PHASE 4 tensor (OUT-03..05)  [instant path]
G7 dB(A)/dB(C) instant toggle ──requires──> per-band grid storage (OUT-06 memory budget)
```

### Dependency Notes

- **Goals 1/2/5 are engine-independent; goals 4/6/7 (and 3's usefulness) gate on engine Phases 3–4.** This exactly matches PROJECT.md's sequencing note: CRUD, GIS import, drawing can be built in parallel with the engine finish; calculation/what-if/results features cannot run before Phase 3 (meteorology) and Phase 4 (tensor + grid batch) land.
- **The DGM is the spine:** import (G2), elevation editing (G5), object z-placement, and validation all hang off the triangulated ground model. Ship a flat default DGM first so the drawing milestone doesn't block on the import milestone.
- **"Fast recompute" has two speeds — be honest in the UI.** Source conditioning (gain/filter/delay, dB-weighting toggle) rides the cached tensor MAC and is interactive. Weather and geometry edits invalidate `H` and require a full propagation job. Weather *scenario switching* is instant only between already-computed scenarios (per-scenario tensor cache). The UI must signal which edits are which, or trust evaporates.
- **Real-GIS path extraction (GEOX-01..03) is a hidden prerequisite of every calculation feature:** Milestone 1 fed the engine FORCE-file profiles; the UI needs DEM cut-profiles, impedance segmentation, and screening-edge derivation from the drawn scene. It is the project's flagged "biggest self-build item" and belongs to the first calculation-capable phase of Milestone 2.
- **Forest scattering:** TI 386's Nord2000 forest term (`A = d·a(f)` from density/stem radius) has no ENG-xx requirement in Milestone 1's list. Verify engine coverage; if absent, either add it to a Milestone-2 engine phase or ship the Forest object as geometry-only with a visible "scattering pending" note. Do not silently draw forests that do nothing.

## MVP Definition

### Launch With (v2.0)

The slice that makes all 7 goals demonstrably work end-to-end, single scenario, single directional sub-source.

- [ ] **Project CRUD + map shell** — project list, create/open/save(autosave)/delete/duplicate; MapLibre OSM basemap (G1)
- [ ] **Scene drawing with property panels** — buildings, walls (constant height), ground zones (A–H + N/S/M/L dropdowns), forests, elevation points/lines + DGM re-triangulation, receivers, calculation area, directional source (spectrum entry + preset library + SPL@reference-point + orientation + active toggle); last-object property inheritance; snap/vertex editing; validation messages that select the offending object (G5)
- [ ] **GIS viewport import** — terrain (GLO-30, LiDAR where wired) → DGM; buildings with height fallback + default height; WorldCover → ground zones; urban/rural default ground; imported objects fully editable (G2)
- [ ] **Weather: import + manual override, single scenario** — Open-Meteo fetch (date/time), then editable T/RH/p, Beaufort + direction, downwind worst-case toggle, temperature gradient; derived A/B/C visible (G3, G4 minimal)
- [ ] **Calculation jobs** — settings (grid distance, calc height, reflection order 3, weighting), submit/progress/abort, pre-check warnings (G7 infrastructure)
- [ ] **Receiver spectra** — per-band table + bar chart (1/3-oct display), dB(A)/dB(C) totals, CSV export (G6)
- [ ] **Noise map** — isophone fill polygons, editable color scale (min/interval/count/ramp), dB(A)/dB(C) toggle, legend, opacity (G7)

**Engine gate:** the last four items produce real numbers only after engine Phases 3–4; build them against a stub/fixture job result so the UI is ready when the engine lands (PROJECT.md sequencing).

### Add After Validation (v2.x)

- [ ] **Named weather scenarios + cached-result instant switching + difference map** — trigger: MVP single-scenario loop proven; the reference doc itself recommends multi-scenario practice (G4 full)
- [ ] **Interactive source conditioning (gain/filter/delay) via tensor MAC** — trigger: Phase 4 tensor stable under the service; this is the flagship differentiator (WEB-05)
- [ ] **Multi-sub-source array composition + CLF directivity import** — trigger: single directional source validated (SRC-03/04, FUT-05)
- [ ] **1/12-oct + coherent/incoherent expert spectrum view** — trigger: receiver readout in daily use
- [ ] **GeoTIFF/GeoJSON result export; PNG map snapshot** — trigger: first real deliverable needed (GRID-05)
- [ ] **3D terrain check view** (MapLibre native terrain tilt) — trigger: DGM-correctness disputes; replaces NoizCalc's 3D-map check idiom cheaply
- [ ] **Variable wall height (segment properties)** — trigger: real project needs it

### Future Consideration (v3+)

- [ ] **DXF / Shapefile / glTF import** — deferred exchange formats (FUT-01/02); real consultant workflows only
- [ ] **L_den weather-class statistics (ERA5/CDS, MET-05, GRID-03)** — regulatory long-term assessment is a different product mode from event what-if
- [ ] **Report/print generation** — only if screen + export proves insufficient
- [ ] **Multi-height receivers / façade grids / vertical maps** — iNoise-class features; wait for demand
- [ ] **Road/rail emission UI** — separate milestone if ever

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Scene drawing + property panels (G5 core) | HIGH | HIGH | P1 |
| GIS viewport import → DGM (G2) | HIGH | HIGH | P1 |
| Project CRUD + map shell (G1) | HIGH | LOW | P1 |
| Calculation jobs + settings (G7 infra) | HIGH | MEDIUM | P1 |
| Isophone map + editable scale (G7) | HIGH | MEDIUM | P1 |
| Receiver spectra readout (G6) | HIGH | MEDIUM | P1 |
| Weather import + manual override, single scenario (G3/G4) | HIGH | MEDIUM | P1 |
| Validation/message panel with object jump | MEDIUM | MEDIUM | P1 |
| Named scenarios + compare + instant switch | HIGH | MEDIUM | P2 |
| Source conditioning via tensor MAC | HIGH | MEDIUM | P2 |
| Multi-sub-source arrays + CLF directivity | MEDIUM | HIGH | P2 |
| 1/12-oct expert spectrum view | MEDIUM | LOW | P2 |
| Result exports (GeoTIFF/GeoJSON/PNG) | MEDIUM | LOW | P2 |
| 3D terrain check view | MEDIUM | LOW | P2 |
| Variable wall height | LOW | MEDIUM | P3 |
| DXF/SHP/glTF import | MEDIUM | HIGH | P3 |
| L_den weather classes | LOW (for this tool's use) | HIGH | P3 |
| Report/print subsystem | LOW | HIGH | P3 |

**Priority key:** P1 = must have for Milestone-2 launch · P2 = should have, add when possible · P3 = future

## Competitor Feature Analysis

| Feature | NoizCalc (TI 386) | iNoise (DGMR) | ENVI Approach |
|---------|-------------------|---------------|---------------|
| Source definition | External (ArrayCalc file → Stage) + spectrum/SPL@ref in-app | Industrial point/area sources | Native directional multi-sub-source, spectrum + SPL@ref calibration in one app |
| Standards | Nord2000 + ISO 9613-2 dual | ISO 9613 + CNOSSOS | Nord2000 only, phase-preserving |
| GIS import | Google bitmap+elevation, OSM buildings/ground/forest, viewport-based | SHP/DXF/GPKG/WMS file import | Viewport-based like NoizCalc, but open data (GLO-30/LiDAR, WorldCover→σ, Overture) |
| Weather | Manual calc settings only (Beaufort, downwind, gradient) | Standard met parameters | Open-Meteo import + manual override + named scenarios (unique) |
| Receiver spectra | None (grid map only) | Receivers up to 5 heights | Receiver points with 1/3-oct display, 1/12-oct expert view, live conditioning updates |
| Results | Grid map, editable scale, Bezier-smoothed contours, print sheets | H/V/façade/difference contours, ColorBrewer | Server-contoured isophone polygons on live web map; scenario difference maps in v2.x |
| What-if speed | Full re-run every time | Full re-run ("compare models" post hoc) | Tensor MAC = interactive for source conditioning; per-scenario caches for weather (unique) |
| Platform | Windows desktop | Windows desktop | Self-hosted web app |

## Sources

- `docs/references/dbaudio-ti386-1.6-en.md` — TI 386 (1.6 EN) NoizCalc, chapters 3–4 + quick guide (**PRIMARY**, local transcription of the official d&b/SoundPLAN document; HIGH confidence for NoizCalc behavior)
- `.planning/PROJECT.md` — milestone framing, locked decisions (single app, Nord2000-only, tensor pillar, phase sequencing) (HIGH)
- `.planning/REQUIREMENTS.md` — v2 requirement groups DATA/GEOX/METX/GRID/WEB/SVC/FUT mapped throughout (HIGH)
- `.planning/research/OPEN-GIS-LANDSCAPE.md` — stack (MapLibre 5 + react-map-gl 8 + Terra Draw, spade, GDAL contour), data tiers, directivity model (HIGH, prior verified research)
- [terra-draw GitHub — guides/4.MODES.md](https://github.com/JamesLMilner/terra-draw) — built-in modes (point/linestring/polygon/rectangle/angled-rectangle/freehand/select), vertex editing, snapping, validation; MapLibre v5 adapter (MEDIUM — official docs, single-source fetch; seam provider tier LOW, upgraded on primary-source basis)
- [DGMR iNoise](https://dgmrsoftware.com/products/inoise/) — receiver/grid/contour result types, compare-models, ColorBrewer scales (MEDIUM — vendor page via web search)
- [dBmap.net](https://noisetools.net/dbmap/) — web-based noise-mapping precedent exists (LOW — existence check only)

---
*Feature research for: ENVI Milestone 2 — Interactive Calculation UI (NoizCalc-style)*
*Researched: 2026-07-08*
