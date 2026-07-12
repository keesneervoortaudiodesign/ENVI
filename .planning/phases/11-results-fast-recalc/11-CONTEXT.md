# Phase 11: Results & Fast Recalc - Context

**Gathered:** 2026-07-12
**Status:** Ready for planning

<domain>
## Phase Boundary

The payoff phase of Milestone 2: turn the cached complex transfer tensor into what the user
actually reads and interacts with —

- **Receiver spectrum readout** (WEB-11): per-band levels (1/12-oct expert + 1/3-oct display by
  band index), dB(A)/dB(C) totals with an instant no-recompute toggle, coherent/incoherent split.
- **Isophone noise maps** (GRID-04, WEB-06): server-computed (see D-01) isophone **fill polygons**
  (no heatmap layer) with an editable color scale where legend breaks ≡ contour breaks ≡ class colors.
- **Interactive source conditioning** (WEB-05, SVC-06): the flagship fast-recalc — per-source
  gain/filter/delay recomputed via the streaming tensor MAC (no re-propagation), a results-stale
  badge, and a hard hash-mismatch rejection.
- **Weather what-if + named scenarios** (METX-03/04, WEB-12): manual met overrides, named scenarios
  each with their own cached tensor, instant switching, and difference maps.
- **Exports** (GRID-05): GeoTIFF/GeoJSON for the grid, CSV for spectra, carrying band index + exact
  Hz + attribution.
- **Map color system + object styling** (user addition): a differentiating color **and hatch pattern
  (arcering)** per object kind, modeled on NoizCalc, on a validated accessible palette.
- **Self-explanatory UI** (user addition, app-wide): an **"i" info button on EVERY control** across the
  whole app — new Phase-11 results controls **plus a retrofit of all Phase-7/8/9/10 panels** — opening
  extensive, standards-cited help so the UI is 100% self-explanatory.

Requirements: **SVC-06, WEB-05, WEB-06, WEB-11, WEB-12, METX-03, METX-04, GRID-04, GRID-05**.

**Depends on (hard gates):**
- **Engine Milestone-1 Phases 3–4** — refraction math + the `H[s,r,f]` tensor + the `compose_gain` /
  `readout_coherent` MAC (all present, FORCE-validated).
- **Phase 10** — the client-side threaded-WASM solve + the OPFS chunked tensor store + tier-complete
  emission (Phase 10 *computes + persists*; Phase 11 *renders + reconditions*).
- **Phase 6** — the recondition/recompute API split + blake3 tensor-identity + the 409 hash gate.
- **Phase 9** — the per-azimuth A/B/C derivation (`envi_gis::weather`) for weather overrides.
- **Phase 7** — the isolation `SpectrumEditor` (reused for the conditioning filter) and the scene-object
  map layers (restyled by the color-system decision — coordinate, don't regress).

**Out of scope (deferred / other work):**
- Real authentication / login gate (its own deferred phase, per Phase-10 D-12).
- GRID-03 L_den weather-class combination (deferred beyond Milestone 2).
- The PROJECT.md / ARCHITECTURE.md deployment-model amendment pass (carried from Phase 8/10 deferred).
- A wholesale re-theme of the Phase-7 *editor interactions* beyond object color/hatch styling.

</domain>

<decisions>
## Implementation Decisions

### Compute locus — reconciling the requirements' "server-side" language (foundational)
- **D-01:** Given the **Phase-10 client-side WASM pivot** (10-CONTEXT D-01), **all Phase-11 acoustic
  arithmetic runs in Rust compiled to WASM, client-side, over the OPFS tensor** — the MAC readout,
  dB(A)/dB(C) weighting, band aggregation, coherent/incoherent split, contouring, and export byte
  generation. The requirement language "computed **server-side** / no client-side acoustic math"
  (SVC-07, WEB-11, WEB-06 "server-side isophone polygons") is reinterpreted as **"in Rust/WASM,
  never in JS/TS."** The JS/TS layer does zero acoustic math; it renders WASM-produced values. The
  recondition/recompute split + blake3 hash gate (Phase 6) stay the identity contract; the MAC itself
  executes in WASM. **Treated as settled** (extends the pivot; not re-litigated).

### Isophone map & color scale (GRID-04, WEB-06)
- **D-02:** **Contour pure-Rust in WASM.** Use the `contour` crate — or a hand-rolled marching-squares
  tracer like the Phase-8 WorldCover vectorizer (`envi-gis/landcover.rs`) — in the client-side compute
  path. `gdal-sys` `GDALContourGenerateEx` remains a **documented escape hatch only** if the roadmap's
  flagged 100k+-cell benchmark fails. Isophones render as **fill polygons**, never a heatmap layer.
- **D-03:** **Default scale = EU END-style fixed dB bands** (domain-standard ~5 dB steps) with the
  canonical noise-map palette — familiar to acousticians, comparable across runs/scenarios.
- **D-04:** **Editing = presets + editable breaks.** Pick a palette from a small preset set (END-standard
  plus perceptually-uniform, e.g. viridis/turbo) and edit the break values; class colors follow the
  palette. **Legend breaks ≡ contour breaks ≡ class colors is enforced**, and editing the scale
  **re-contours the cached level grid without re-running propagation** (SC3). The dB weighting label
  comes from result metadata.

### Spectrum readout panel (WEB-11)
- **D-05:** **Chart primary + expandable exact-numbers table** (bar/line per-band levels + totals; the
  table gives expert readout/copy).
- **D-06:** **1/3-oct display is the default**, with a toggle to the full 105-point **1/12-oct expert**
  view (aggregated by band index — never by nominal Hz).
- **D-07:** **Receiver selection via map click AND a synced receiver list** (discrete receiver points are
  the primary spectrum targets; grid cells optional).
- **D-08:** **Coherent/incoherent split in the totals always + an on-demand per-band overlay** of the two
  as separate spectrum series (keeps the default view clean, expert detail one click away).
- **D-09:** **A/C weighting is net-new code.** Build A- and C-weighting tables **precomputed at the exact
  105 1/12-octave grid centres** (WASM-side, D-01) so the **dB(A)⇄dB(C) toggle is instant with no
  recompute** — both weightings precomputed, per SC1/WEB-11.

### Interactive source conditioning (WEB-05, SVC-06)
- **D-10:** **Live, debounced MAC recalc** (~100–200 ms) on any gain/filter/delay change — spectra + map
  update live, honoring "interactive fast recalc."
- **D-11:** **Filter control reuses the Phase-7 isolation `SpectrumEditor`** (`web/src/spectrum/
  SpectrumEditor.tsx`): enter filter gain at 1/1, 1/3, or 1/12-oct, interpolated onto the 105-point grid,
  resolving to the complex per-band gain `G_s(f)` fed to `compose_gain`.
- **D-12:** **Stale badge = recompute the blake3 tensor identity** on any scene/terrain/ground/met edit and
  compare to the cached tensor's hash; badge shows when they differ. **Conditioning edits never stale**
  (gain/filter/delay are MAC readout params, excluded from identity — Phase-6 D-07). A MAC request against a
  mismatched hash is **rejected via the SVC-06 409 gate, never silently served** — realized end-to-end here.

### Weather what-if & named scenarios (METX-03/04, WEB-12)
- **D-13:** **Clone-then-edit scenarios.** A new scenario clones the current met settings; the user edits
  overrides, names it, and it computes **its own hash-keyed cached tensor** (METX-04). Named scenarios
  switch **instantly** via their per-scenario cached tensors.
- **D-14:** **Friendly inputs + advanced raw A/B/C.** Friendly overrides (T/RH/p, Beaufort wind class +
  direction, temperature gradient, downwind worst-case toggle) drive the **Phase-9 `envi_gis::weather`
  derivation** to per-azimuth A/B/C; raw per-azimuth A/B/C is exposed as an **advanced override** for experts.
- **D-15:** **Downwind worst-case = per-azimuth favourable** — assume downward-refraction (favourable)
  conditions along **each** source→receiver bearing independently: the standard Nord2000 worst-case
  noise envelope.
- **D-16:** **Difference map = A vs B, dB(A) delta, diverging.** User picks any two scenarios; the map
  renders the **per-receiver dB(A) total difference** on a **diverging color scale** (e.g. blue–white–red
  centered on 0).

### Map color system & object styling (user addition — NoizCalc model)
- **D-17:** **Full scene-object palette redesign with differentiating color AND hatch pattern (arcering)
  per object kind**, following NoizCalc's per-object-type formatting model (TI 386 §4.6.3): **point** objects
  get a distinct symbol/size/border; **line** objects a distinct width/color; **area** objects (buildings,
  ground-effect zones, forests, calculation area) get **fill color, border, and a distinct hatch pattern
  set separately**. Semi-transparent hatched fills let area objects read over the isophone fill.
- **D-18:** **Objects render at full styling on top of the isophone fill** (user choice). The palette must
  guarantee both the noise classes and the objects stay legible together (contrast-checked) — which is
  precisely why hatch patterns + a validated palette are load-bearing here.
- **D-19:** **Validated, accessible color foundation.** Use the dataviz skill's design-system method: a
  **perceptually-uniform sequential** scale for isophones, a **diverging** scale for difference maps,
  **colorblind-safe**, and **light/dark theme-aware** (extending the metrao3 tokens). **Load the `dataviz`
  skill at UI-spec/planning time.** ⚠ This restyles the Phase-7 scene-object map layers — coordinate,
  do not silently regress the editor's draw-time behavior.

### Self-explanatory UI — universal info buttons (user addition, app-wide)
- **D-23:** **An "i" info button on EVERY interactive control**, opening extensive help on that control's
  function and options, via a **reusable info-affordance component** (icon → popover/side-panel). Scope is
  **app-wide**: the new Phase-11 results controls **and a retrofit of all existing Phase-7/8/9/10 panels**
  (scene editor / palette / inspector, import, weather, calc). Goal: a **100% self-explanatory UI** — no
  control ships without help.
- **D-24:** **Help content is extensive + standards-cited.** Multi-paragraph per control: what it does,
  every option/range/unit/default explained, and the **Nord2000 / NoizCalc rationale** where relevant.
  Cite **AV 1106/07 by report number** and the **TI 386 (NoizCalc) manual** — **⚠ AV 1106/07 is
  copyrighted (CLAUDE.md licensing): explain in our own words + cite by report number; NEVER paste the
  standard's text into UI help.** English-only (project rule).
- **D-25:** **Help content lives as structured data** (a keyed help catalog, not text scattered in JSX) so
  it is maintainable and drift-checkable, with a **coverage check that asserts every interactive control has
  a help entry** (a control without help fails the check). This is a **large sweep touching prior-phase
  panels** — the planner should consider isolating it into its own plan/wave.

### Export (GRID-05)
- **D-20:** **Client-side browser download.** WASM produces the export bytes from the OPFS tensor/grid; the
  browser downloads them directly (consistent with the pivot — no tensor/results leave the device).
- **D-21:** **Both raster + vector + spectra.** GeoTIFF = the continuous dB(A)/dB(C) **level grid** (raster);
  GeoJSON = the **isophone fill polygons** (vector); CSV = **spectra** carrying **band index AND exact Hz**
  columns.
- **D-22:** **Full metadata + attribution** on every export: CRS + the dB weighting label + engine/scene
  identity + the OSM/Overture/ESA WorldCover/Copernicus data attribution (SC5).

### Claude's Discretion
- Chart library (dataviz-guided) for the spectrum panel; exact debounce interval; contour break-value
  interpolation details; the specific hatch-pattern set + symbol set per object kind; OPFS per-scenario
  tensor directory layout + naming; how the recondition MAC reuses vs re-streams tier spans; the exact
  diverging-scale midpoint/clamp for difference maps; GeoTIFF encoder choice (pure-Rust) within D-20; the
  info-button presentation (popover vs docked side-panel vs both) and the help-catalog file format/location
  within D-23–25.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements + roadmap
- `.planning/ROADMAP.md` "Phase 11: Results & Fast Recalc" — goal + SC1–SC5 (spectrum readout with zero
  client acoustic math, interactive MAC + stale badge + hash-mismatch rejection, isophone fill polygons +
  editable color scale, weather what-if + named scenarios + difference maps, exports). Its "server-side"
  framing is reinterpreted by **D-01** (client-side WASM); the fill-polygon / no-heatmap / editable-scale
  invariants still bind.
- `.planning/REQUIREMENTS.md` — **SVC-06, WEB-05, WEB-06, WEB-11, WEB-12, METX-03, METX-04, GRID-04,
  GRID-05** (exact wording). Note the SVC-06 "designed in Phase 6, realized in Phase 11" note.

### The analogous product — object styling + noise-map color scale (user-referenced)
- `docs/references/dbaudio-ti386-1.6-en.md` — the **NoizCalc** manual (TI 386). **§4.6.3 "Edit legend,
  format objects"** is the object-styling model for **D-17**: point = symbol/size/border, line =
  width/color, **area = fill color + border + hatch pattern (arcering) separately**. **§4.6.4 Grid Noise
  Map** = fill contours + editable color scale (smallest interval, interval magnitude, number of intervals,
  ascending, keep-color-sequence) for **D-03/D-04**. **§4.6.6 Color palette** (RGB, 16 scale colors,
  interpolate gradients) informs the palette system. **§4.6.5 Transparent grid maps** informs D-18 overlay
  legibility. Also a **primary source for the D-24 info-button help text** (control-level explanations).
- `refs/` — **AV 1106/07 rev.4** (git-ignored, copyrighted) is the Nord2000 implement-from document and the
  authoritative source for the **standards-cited help text** (D-24). **⚠ Cite by report number + explain in
  our own words; do NOT paste its text into UI help** (CLAUDE.md licensing / data hygiene).

### The pivot + prior-phase contracts this phase binds to
- `.planning/phases/10-calculation-service/10-CONTEXT.md` — the client-side threaded-WASM compute pivot
  (D-01/D-03), the OPFS chunked `TensorSink` (D-08), tier-complete emission (D-07), per-project hash-keyed
  tensor persistence (D-09). Phase 11 consumes what Phase 10 emits.
- `.planning/phases/06-service-foundation-persistence/06-CONTEXT.md` — the recondition/recompute split +
  the enforced 409 `tensor_hash_mismatch` gate + the blake3 tensor-identity + calc manifest; conditioning
  excluded from identity (D-07). **D-12** realizes this end-to-end.
- `.planning/phases/09-path-extraction-weather/09-CONTEXT.md` + `crates/envi-gis/src/weather.rs` — the
  per-azimuth A/B/C derivation reused for weather overrides (**D-14**).
- `.planning/research/ARCHITECTURE.md` — tensor chunk format + receiver-axis memory math + the recalc
  tiers + Pattern 1 (one solve path). ⚠ Read against the pivot: on-disk → OPFS, axum/SSE → worker.

### Existing code to build against (verify, do not break)
- `crates/envi-engine/src/tensor.rs` — `compose_gain` + `readout_coherent`/`readout_incoherent`
  (the MAC the conditioning path drives, **D-10/D-12**) + the `TensorSink`/`TensorPair` shapes.
- `crates/envi-compute/` + `crates/envi-compute-wasm/` — the client-side compute core + WASM boundary;
  the isophone contour + A/C weighting + export byte generation extend this (**D-01/D-02/D-09/D-20**).
- `crates/envi-service/src/api/calc.rs` — the `recondition`/`recompute` request/response DTOs + the 409
  hash gate (**D-12** / SVC-06).
- `web/src/spectrum/SpectrumEditor.tsx` — reused for the conditioning filter control (**D-11**).
- `web/src/store/calc.ts` + `web/src/panels/CalcPanel.tsx` — the tier-complete plumbing + the "result
  rendering is Phase 11" seam; Phase 11 adds the results panels/overlays.
- `crates/envi-gis/src/landcover.rs` — the Phase-8 hand-rolled marching-squares vectorizer, the fallback
  contour pattern for **D-02**.

### Binding project contracts
- `.claude/CLAUDE.md` — engine 3-dep quarantine (byte-identical, no new engine deps), the 105-point
  **band-index** framework (compare by band index, never nominal Hz), the two-channel `H_coh`/`P_incoh`
  contract, Playwright offline-UAT rules, English-only output, GitHub conventions, and the **five mandatory
  GSD phase-completion gates**.
- `.planning/PROJECT.md` — product vision + the two-channel contract. ⚠ Its "self-hosted localhost, light
  auth" deployment text is superseded by the Phase-10 pivot (deferred amendment pass).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`envi_engine::tensor::compose_gain` + `readout_coherent`** — the bit-exact MAC the conditioning
  fast-recalc drives; already FORCE-validated (Phase 4). No re-propagation.
- **The OPFS chunked tensor + blake3 identity** (Phase 10) — Phase 11 reads it for both the readout and
  the recondition MAC; the identity is the stale-badge oracle (D-12).
- **`web/src/spectrum/SpectrumEditor.tsx`** (Phase 7) — reused verbatim for the conditioning filter (D-11).
- **`envi_gis::weather` per-azimuth A/B/C** (Phase 9) — drives weather overrides (D-14).
- **`envi-gis/landcover.rs` marching squares** (Phase 8) — the contour fallback pattern (D-02).
- **`web/src/store/calc.ts` tier plumbing** — the results feed to render.

### Established Patterns
- **One solve path, all-WASM acoustic math** (D-01) — no JS acoustic arithmetic; the JS/TS layer renders
  WASM-produced values, keeping "band-index wire, no client-side Hz math" true in spirit.
- **Identity contract as the source of truth** — the blake3 tensor identity is reused for stale detection
  and the 409 MAC gate; conditioning is a readout param, never part of identity.
- **Generated wire DTOs (ts-rs), no hand-mirrored TS** — new result/scenario DTOs follow the committed
  `web/src/generated/wire.ts` no-drift pattern.
- **Honest states, no false green** — hash-mismatch rejected loudly; stale badge shown the moment identity
  diverges.
- **Object-type styling as data** (NoizCalc §4.6.3) — color + symbol/line/hatch per kind, defined once.

### Integration Points
- New results panels (spectrum, color-scale editor, scenario manager) ⇄ `web/src/store` ⇄ WASM readout/MAC.
- Isophone contour: WASM level grid → fill polygons → MapLibre fill layer (below full-styled objects, D-18).
- Conditioning: `SpectrumEditor` + gain/delay controls → debounced `compose_gain` MAC → live spectra + map.
- Scenario switch: select a scenario → load its OPFS hash-keyed tensor → readout; difference map = A−B.
- Export: WASM byte generation (GeoTIFF/GeoJSON/CSV) → browser download.

</code_context>

<specifics>
## Specific Ideas

- **"Make sure all objects have differentiating colors AND arcering (hatch patterns) — look at the
  NoizCalc manual."** This is the origin of D-17/D-18/D-19: model the object styling on NoizCalc's
  per-object-type formatting (TI 386 §4.6.3), where area objects carry a fill color, border, and a
  **separately selectable hatch pattern**. With objects at full styling over the isophone fill, hatching
  is what lets buildings/zones/forests read on top of a colored noise map without hiding it.
- **The flagship interaction** is the conditioning fast-recalc: adjust a source's gain/filter/delay and
  watch the whole map + spectra update live via the tensor MAC with no propagation re-run — the concrete
  payoff of the complex-tensor architecture chosen back in Phase 1/4.
- **EU END-style dB bands** as the default noise-map scale so ENVI's output looks like the printed noise
  maps acousticians already read.
- **"An 'i' info button at ALL controls that explains VERY EXTENSIVELY the function and options — the UI
  should be 100% self-explanatory."** Origin of D-23/24/25: an app-wide, standards-cited, self-documenting
  UI. The user wants a non-expert to understand every control from its own help, without external docs.

</specifics>

<deferred>
## Deferred Ideas

- **Real authentication / login gate** — sessions, accounts, the "only runnable when logged in" gate; its
  own phase with a dedicated threat model (Phase-10 D-12).
- **PROJECT.md / ARCHITECTURE.md deployment-model amendment pass** — rewrite "self-hosted localhost, light
  auth" to "client-side WASM compute + delivery/login server" (carried from Phase 8/10 deferred).
- **GRID-03 L_den weather-class combination** — deferred beyond Milestone 2 (unmapped).
- **OPFS quota / eviction strategy** — per-scenario cached tensors could strain quota; revisit only if it
  bites (Phase-10 D-09 keeps everything).
- **Wholesale Phase-7 editor re-theme beyond object color/hatch** — the color-system work restyles object
  map layers only; broader editor-interaction redesign is out of scope.
- **Parametric-EQ conditioning UI** — the shelf/peak filter alternative was considered; the reused 105-point
  spectrum editor (D-11) ships instead.

### Reviewed Todos (not folded)
None — no pending todos matched this phase (the directional-phase todo was folded and resolved in Phase 10).

</deferred>

---

*Phase: 11-Results & Fast Recalc*
*Context gathered: 2026-07-12*
