# Phase 7: Frontend Shell & Scene Editing - Context

**Gathered:** 2026-07-10
**Status:** Ready for planning

<domain>
## Phase Boundary

The first UI phase: build the **authoring surface** — draw and edit the complete
NoizCalc-style scene on a MapLibre OSM basemap (all 9 locked object kinds, including ENVI's
semi-transparent screens/buildings with isolation spectra, and forests), persist it through the
Phase-6 API, and validate it at draw time.

**Requirements:** WEB-01 (MapLibre OSM basemap), WEB-02 (place/edit directional sources +
SPL-at-reference-point calibration), WEB-03 (receivers + calculation area), WEB-04 (buildings,
walls, ground-effect zones, forests, elevation points/lines with DGM re-triangulation; last-object
property inheritance; click-to-select validation), WEB-08 (semi-transparent screen + isolation
spectrum), WEB-09 (per-façade isolation spectra), WEB-10 (isolation-spectrum editor),
SCN-01/02/03/04 (the scene-object semantics those WEB features author).

**⚠ This phase is NOT frontend-only.** Three decisions below (D-01, D-05, D-08) require backend
work in `envi-store` / `envi-service`. The planner must budget for it: new typed DTOs + `TryFrom`
into engine types, one new endpoint, and a server-side TIN. Nothing in `envi-engine` changes.

**In scope:** the Vite/React/TS frontend shell; MapLibre + Terra Draw drawing for all 9 kinds;
the isolation-spectrum editor + per-façade assignment; draw-time ground-zone topology validation;
server-side constrained-Delaunay DGM from user-drawn elevation; typed `envi-store` DTOs for
isolation spectra + forest params with a *tested* conversion into the engine's existing
`IsolationSpectrum` / `ForestCrossing`; one new interpolation endpoint; generated wire types.

**Out of scope (later phases):** source conditioning + fast tensor-MAC recalc (WEB-05, Phase 10/11),
isophone overlays (WEB-06, Phase 11), job submit/results UI (WEB-07, Phase 10), receiver spectrum
readout (WEB-11, Phase 11), weather what-if (WEB-12, Phase 9); terrain/GIS import and the C `gdal`
dependency (Phase 8); engine mapping of `ground_zone` → `GroundSegment` (Phase 9),
`elevation_*` → engine DGM (Phase 8), `calc_area` → receiver grid (Phase 10); solve-time
attachment of isolation/forest to `SolveJob` (Phase 9/10).

**Depends on:** Phase 6 (API + persistence, all contracts frozen). Phase 5 (the `IsolationSpectrum`
and `ForestCrossing` engine types this phase authors data for). Engine-independent otherwise.

</domain>

<decisions>
## Implementation Decisions

### Backend reach — "SCN/WEB objects must never be silently inert"

Discovery that framed the phase: the Phase-5 engine seams exist (`SolveJob.isolation`
`solver.rs:120`, `SolveJob.forest` `solver.rs:105`), but `envi-store`'s DTO carries **neither**
isolation spectra nor forest params, and `scene_to_engine` maps only **4 of 9** kinds
(`source`/`receiver`/`wall`/`building`). Draw a semi-transparent screen today and it *is* inert.

- **D-01:** Phase 7 closes the gap to **typed + proven convertible**. `envi-store` gains typed,
  validated DTOs for isolation spectra and forest params; they round-trip through scene GET/PUT;
  and a **tested `TryFrom`** proves conversion into the engine's existing
  `envi_engine::propagation::transmission::IsolationSpectrum` and `envi_engine::forest::ForestCrossing`.
  **`envi-engine` is NOT modified** (byte-identical, 3-dep quarantine holds). `scene_to_engine`
  stays at 4/9 kinds — the other five need subsystems that do not exist yet (`ground_zone`→Phase 9
  cut-plane extraction, `elevation_*`→Phase 8 terrain, `calc_area`→Phase 10 receiver grid).
  Isolation reaches the engine via `SolveJob.isolation` at **solve** time (Phase 9/10), which is why
  no engine change is needed now.
- **D-02:** Per-façade isolation spectra are keyed by **stable per-edge UUIDs**, never by ring
  vertex index. Each building footprint edge carries a generated UUID assigned at draw time and
  preserved across vertex moves. Inserting a vertex **splits** one edge into two, both inheriting
  the parent edge's spectrum; deleting a vertex merges. A `default_isolation` covers unassigned
  edges. *Rationale:* an index key would silently re-point a spectrum at a different façade after
  any vertex insert — a data-corruption bug no test would catch. UUID keying makes it structurally
  impossible.

  ```
  properties: {
    "kind": "building",
    "edge_ids": ["a1b2…", "c3d4…", "e5f6…", "0789…"],
    "default_isolation": { authored: {…} },
    "facade_isolation": { "c3d4…": { authored: {…} } }   ← keyed by UUID
  }
  ```

### Scene state ownership & re-hydration (SC4)

- **D-03:** A **client app store is the single source of truth** for the scene FeatureCollection,
  including all acoustic properties. **Terra Draw is a controlled view**, held in a ref: on
  `style.load` the app re-adds features from the store; TD change events write back into it.
  *Rationale:* `map.setStyle()` tears down every source and layer, and React StrictMode
  double-mounts in dev — TD's internal store is bound to the map instance lifecycle, the app store
  is not. Basemap switch, reload, and reopen then reduce to one mechanism. A 105-band spectrum is
  scene data, not geometry; it lives in the store, not in TD feature properties.
- **D-04:** **Autosave is debounced on committed edits.** Committed mutations only — a finished
  shape, a released vertex, a property change — never intermediate drag frames. Debounced
  (~750 ms), coalesced into one whole-scene `PUT /scene` (the Phase-6 API has no per-feature PATCH),
  with a **flush on tab close / navigate-away** and a visible dirty indicator. Honors Phase-6 D-06
  autosave without putting the network in the drag loop.

### Isolation-spectrum editor & façade assignment (WEB-08/09/10, SCN-01/02/03)

- **D-05:** **The server owns interpolation**, via a new endpoint
  `POST /api/v1/meta/interpolate-spectrum` taking `{ resolution, values }` and returning the
  105-point grid. One implementation, in Rust, shared by the editor's live preview and by
  `PUT /scene` validation — they cannot diverge. Honors SVC-07 literally ("no Hz-based client-side
  acoustic math"). On localhost the round-trip is ~1 ms, so the usual latency objection does not
  apply. Phase 6 froze the existing endpoints; it did not forbid new ones.
- **D-06:** **Persist the authored representation ONLY; derive `r_db[105]` on read.**
  `IsolationSpectrumDto { authored: { resolution, values } }` where `values` has 9 / 27 / 105
  entries. Authoring at 1/12 is simply `resolution: "twelfth"` — no special case, no duplication.
  Resolution switching (1/3 ⇄ 1/12) is therefore lossless, and editing an individual 1/12 band
  explicitly promotes the spectrum to `authored@twelfth` rather than silently discarding coarse
  values. *Rationale (learned the hard way in Phase 6):* storing a derived `r_db[105]` beside its
  `authored` source is the `CalcRecord.tensor_hash` shadow-cache anti-pattern — a hand-edited
  `scene.geojson` (which D-04 of Phase 6 explicitly wants to be human-editable) could carry an
  `authored` that disagrees with its `r_db`, and `r_db` is what the acoustics consume. There is no
  second copy, so there is nothing to invalidate.

### Draw-time validation & DGM scope (WEB-04, SC1/SC2)

- **D-07:** **Hard reject at draw time.** A partially crossing `ground_zone` polygon never commits:
  geometry reverts to the last valid state and a message explains the conflict, with click-to-zoom
  targeting the **existing** zone it crosses (which does exist). Containment is allowed; innermost
  wins. The scene invariant is that ground-zone topology is *always* valid. *Resolves an ambiguity
  in SC2:* its "validation messages click-to-select and zoom to the offending object" refers to the
  **persistent validation panel** (WEB-04), which covers non-geometric issues on objects that do
  exist — a wall marked semi-transparent with no spectrum, a forest with zero density. A rejected
  polygon has no object to select.
- **D-08:** **Phase 7's DGM is triangulated from user-drawn elevation only, server-side.** A
  constrained Delaunay TIN from `elevation_point` (vertices) and `elevation_line` (breaklines),
  built in Rust with **`spade`** — already in the verified engine stack per CLAUDE.md. Satisfies
  SC1's "re-triangulate the DGM" inside the phase boundary. **No terrain import, no network data
  sources, no C `gdal`** (Phase-6 D-01/D-02 deferred that to Phase 8). Phase 8 later extends the
  *same seam* by feeding imported GLO-30 / DTM samples in as additional vertices — no rewrite.

### Frontend language & the wire contract

- **D-09:** **React + Vite + TypeScript (TSX).** This **amends CLAUDE.md**, which said "JSX/React";
  the amendment is applied in this phase's commit, not left implicit. *Rationale:* the app-internal
  surface is a 9-kind discriminated union (exhaustiveness via `never` in draw handlers and property
  panels), the D-02 edge-UUID→spectrum map, and TS-first `react-map-gl` 8 / Terra Draw generics.
- **D-10:** **Wire types are GENERATED from the Rust DTOs, never hand-authored.** ~27 types cross
  the wire (13 `envi-store` DTOs + 14 `envi-service` types). Derive TS from the serde source of
  truth (`ts-rs`, or `schemars`→JSON Schema→TS), **commit the generated `.ts`**, and add a test
  asserting regeneration produces no diff — the same committed-artifact pattern as
  `tools/nord2000_oracle/` (generate at dev time, no generator needed at test time).
  *Rationale:* TypeScript checks nothing at runtime — `await res.json()` is `any`. A hand-written TS
  mirror of 27 Rust types is a second source of truth with nothing enforcing agreement: a renamed
  Rust field compiles clean and fails in the browser. Generation makes drift structurally
  impossible, the same way derive-on-read fixed the Phase-6 409 gate.
  **⚠ Research must validate** that the chosen generator faithfully renders `JobStatus`'s
  internally-tagged enum (`#[serde(tag = "state")]`, `jobs.rs:50`) with payload-carrying variants
  (`Running { progress, message }`, `Failed { reason }`) as a real TS discriminated union. If it
  cannot, fall back to a hand-written **zod** schema for that type specifically — zod validates at
  runtime, so drift fails loudly on the first request rather than silently.

### Visual design system — adopted from the sibling repo `metrao3`

**Scope note:** only the **visual appearance** is adopted. `metrao3`'s frontend *architecture* is
vanilla TypeScript with a hand-rolled DOM layer and zero runtime dependencies — that is explicitly
**NOT** adopted. ENVI stays React + Vite + TSX (D-09) because it needs `react-map-gl` 8 + Terra Draw,
and keeps generated wire types (D-10) rather than `metrao3`'s hand-written `api.ts` interfaces.

- **D-11:** ENVI's UI adopts the **metrao3 ops-console theme, dark-only**, taking the *embedded*
  variant (`crates/metrao3-web/ui/src/theme.css`) as the ancestor rather than the portal variant —
  identical tokens, but with the `Inter` / `JetBrains Mono` web-font names stripped in favour of a
  **system font stack**. ENVI serves `web/dist` offline from the binary and Phase 6 already gates on
  "zero external assets in `index.html`", so no web font may be fetched. **Do not invent tokens** —
  the token set is the contract. Adopted verbatim:
  - Layered dark surfaces (`--color-bg` `#0b0d10` → `--color-surface-3`), the text ramp, borders,
    shadows, radii, the spacing scale, the type scale, and `--row-h` (28px) / `--row-h-lg` (36px).
  - `--color-primary: #4ea8ff`. (A single-token re-hue is possible later; not now.)
  - The **severity vocabulary `ok / warn / crit / off`**, which maps directly onto the WEB-04
    validation panel: `crit` = a rejected crossing ground zone (D-07), `warn` = a wall marked
    semi-transparent with no isolation spectrum, or a forest with zero density.
  - The `.dot` / `.chip` / `.btn` / `.input` / `.panel` / `.panel-header` primitives; `.mono` with
    `font-variant-numeric: tabular-nums` for all numeric readouts — notably the 105-band spectrum table.
  - **Inline SVG icons built via DOM APIs** (`icons.ts` pattern) — never an icon font, never a CDN,
    never `innerHTML`. Values reach the DOM by `textContent` only.
- **D-12 (adaptation):** `metrao3`'s `min-height: 44px` on `.btn` / `.input` exists for a kiosk/phone
  touch target. ENVI is a mouse-driven desktop map editor with a dense property inspector, so control
  height is driven from the **existing** `--row-h` / `--row-h-lg` tokens (36px default, a 28px
  `.dense` variant for the spectrum table), while the **44px minimum is retained for primary and
  destructive actions** (Save, Delete project) where a misclick is costly. No token is added or
  changed — only which token each control consumes.
- **D-13 (adaptation):** the chrome is dark-only, so the basemap must be a **dark MapLibre vector
  style**; the basemap recedes and the drawn scene carries the colour, hued from the accent +
  severity tokens. **Research must pick a style that needs no API key and works offline** (ENVI is
  self-hosted localhost). A light OSM raster under dark chrome would create a hard luminance seam and
  would break the severity-token palette's legibility.

### Claude's Discretion
- Client state library (Zustand suggested, not mandated) and store shape.
- Panel/layout composition; how the object palette and property inspector are arranged (within D-11's
  token set and the `.panel` / `.panel-header` primitives).
- The per-kind scene-object palette: which accent/severity token hues each of the 9 kinds takes.
- Last-object property inheritance mechanics (WEB-04) — per-kind, session-scoped is the assumed default.
- Source calibration UI details (WEB-02: sound power / spectrum / SPL-at-reference-point).
- Terra Draw mode configuration and which modes back which kinds.
- Spectrum editor presentation: curve editor vs numeric table (or both); whether to ship presets
  for typical glazing/wall `R(f)` values.
- Where the `spade` TIN lives (`envi-store`, `envi-service`, or a new crate) and whether the DGM is
  served by a new endpoint or computed on demand.
- Basemap tile source and map style.
- Whether the generated-types test lives in Rust (`cargo test`) or the JS toolchain.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### The authoritative architecture (LOCKED baseline — read FIRST)
- `.planning/research/ARCHITECTURE.md` — Milestone-2 crate topology, the API boundary + endpoint
  list, the persistence model, the DTO-mirror serde quarantine (Pattern 2), the recalc-tier table
  (Pattern 3), and the Anti-Patterns (esp. #1 serde-in-engine, #5 long CPU on the tokio blocking pool).

### Requirements + roadmap
- `.planning/ROADMAP.md` "Phase 7" — goal, SC1–SC4, and the flagged **Terra Draw ⇄ react-map-gl
  lifecycle spike** (instance-in-ref, `style.load` re-hydration, StrictMode) required before feature plans.
- `.planning/REQUIREMENTS.md` §WEB (WEB-01/02/03/04/08/09/10) + §SCN (SCN-01/02/03/04).

### Binding project contracts
- `.claude/CLAUDE.md` — the toolchain (amended to TSX by D-09), the wire-types-are-generated rule
  (D-10), the engine 3-dep quarantine, the 105-point band-index framework (**compare by band index,
  never nominal Hz**), Playwright frontend UAT rules, English-only output, GitHub/commit conventions,
  and the five mandatory GSD phase-completion gates.
- `.planning/PROJECT.md` — self-hosted internal tool, light/no auth, localhost, single integrated
  app (no ArrayCalc split), Nord2000-only.
- `.planning/phases/06-service-foundation-persistence/06-CONTEXT.md` — Phase-6 locked decisions
  D-01..D-08 (pure-Rust CRS, project-as-folder, DTO mirror, recondition/recompute split, band-index
  wire, job state machine).

### The frozen contracts this phase binds to (verify, do not break)
- `crates/envi-store/src/geojson.rs` — **`KINDS` (line 38): the locked 9-kind `properties.kind`
  vocabulary**; `scene_to_engine` (maps 4/9 today); `check_wgs84`; the one reprojection call site.
- `crates/envi-store/src/dto.rs` — the 13 serde DTOs the frontend mirrors (generated, per D-10).
- `crates/envi-service/src/api/mod.rs` — the frozen endpoint table; **axum 0.8 brace path syntax
  `/{id}` (`/:id` panics at router construction)**.
- `crates/envi-service/src/jobs.rs` — **`JobStatus` (line 50): `#[serde(tag = "state")]`,
  snake_case tags `queued`/`running`/`done`/`failed`/`cancelled` — its own doc comment names this
  "the contract Phase-7 binds its EventSource handling to."**
- `crates/envi-service/src/api/meta.rs` — `GET /api/v1/meta/freq-axis`, the 105-point axis served
  once. **Never hardcode the axis client-side.**
- `crates/envi-store/src/project_dir.rs` — `ProjectStore`, atomic saves, whole-scene PUT (no PATCH).

### The engine types this phase authors data for (VERIFY-ONLY — do not modify)
- `crates/envi-engine/src/propagation/transmission.rs` (line 93) — `IsolationSpectrum`, the D-01
  `TryFrom` target.
- `crates/envi-engine/src/forest.rs` (line 204) — `ForestCrossing`, the D-01 `TryFrom` target.
- `crates/envi-engine/src/solver.rs` — `SolveJob.isolation` (line 120) / `SolveJob.forest`
  (line 105): the solve-time seams, wired in Phase 9/10, NOT this phase.
- `crates/envi-engine/src/freq.rs` — `FREQ_AXIS`, `N_BANDS`; **every 4th of the 105 points is an
  exact 1/3-octave centre** (the D-05 interpolation invariant).

### Visual design system (D-11/D-12/D-13) — external repo, READ-ONLY, never modify
- `D:\====CLAUDE\metrao3\crates\metrao3-web\ui\src\theme.css` — **the token source ENVI adopts**
  (the embedded, system-font, offline variant). Dark-only ops-console tokens: surfaces, text ramp,
  `--color-primary` `#4ea8ff`, the `ok`/`warn`/`crit`/`off` severity vocabulary, spacing, radii,
  shadows, `--row-h`, and the `.dot` / `.chip` / `.btn` / `.input` / `.panel` primitives.
  **Do not invent tokens.**
- `D:\====CLAUDE\metrao3\portal\src\theme.css` — the upstream portal variant (same tokens, but names
  `Inter` / `JetBrains Mono` first). **Not** the ancestor for ENVI: web fonts are forbidden offline.
- `D:\====CLAUDE\metrao3\crates\metrao3-web\ui\src\icons.ts` — the inline-SVG icon pattern
  (DOM-constructed, never `innerHTML`, no icon font, no CDN).
- `D:\====CLAUDE\metrao3\crates\metrao3-web\ui\src\styles.css` — layout/primitive usage reference.
- ⚠ **`metrao3`'s frontend architecture is explicitly NOT adopted** (it is vanilla TS, no framework,
  zero runtime deps, hand-written `api.ts` types). Only the visual layer crosses over. ENVI remains
  React + Vite + TSX (D-09) with generated wire types (D-10).

### Patterns to mirror
- `tools/nord2000_oracle/gen_ground_fixtures.py` + `crates/envi-harness/tests/oracle_ground.rs` —
  the **generate-at-dev-time, commit-the-artifact, test-asserts-no-drift** pattern that D-10's
  generated wire types replicate (sha256 provenance header; no generator needed at test time).
- `crates/README.md` — crate boundaries and the quarantine gates.

### Workflow reference (descriptive)
- `docs/references/dbaudio-ti386-1.6-en.md` ch. 3–4 — the NoizCalc object palette, the
  `properties.kind` vocabulary this scene mirrors, last-object property inheritance, and the
  import→model→calculate→plot loop.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **The frozen Phase-6 API** — `GET/PUT /projects/{id}/scene`, project CRUD + reopen-last,
  `GET /meta/freq-axis`, the job/SSE state machine. The frontend consumes; it does not redesign.
- **`envi-store::geojson`** — the 9-kind vocabulary, `check_wgs84` validation, unknown-kind
  preservation, and (post-Phase-6-simplify) `scene_receivers` / `scene_source_count`. The store
  owns scene-schema knowledge; the service is a thin HTTP layer. New scene validation belongs here.
- **`envi-geo`** — the single CRS seam. The wire is WGS84 GeoJSON; the frontend never reprojects.
- **Engine `IsolationSpectrum` / `ForestCrossing`** — already exist (Phase 5). D-01 only needs a
  `TryFrom` into them, not new engine types.
- **`spade`** — already in the verified engine stack (CLAUDE.md), the D-08 TIN dependency.

### Established Patterns
- **Serde quarantine:** serde lives in `envi-store`, never in `envi-engine`. New DTOs go in the store.
- **Derive-on-read over cached-and-invalidated:** Phase 6's `/simplify` deleted a write-only
  `CalcRecord.tensor_hash` shadow cache after proving it was already stale-on-met. D-06 and D-10
  apply the same principle to spectra and to wire types respectively.
- **Committed generated artifacts + a no-drift test** (oracle fixtures) — D-10's model.
- **Honest stubs, no false green** — nothing may claim a real acoustic result.
- **Quality gates:** `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `cargo test`.
  The frontend adds Playwright (devDependency only, `page.route` mocks, offline, artifacts git-ignored).

### Integration Points
- `web/` (Vite/TSX) ⇄ `envi-service` REST/GeoJSON (WGS84) + SSE — the only network seam.
  `envi-service` already serves `web/dist` via `ServeDir` with an SPA fallback; Phase 7 replaces the
  placeholder `index.html` with the real bundle.
- New: `POST /api/v1/meta/interpolate-spectrum` (D-05).
- New: typed isolation/forest DTOs in `envi-store` + `TryFrom` into engine types (D-01).
- New: server-side `spade` constrained-Delaunay TIN from user-drawn elevation (D-08).
- New: generated TS wire types committed under `web/` (D-10).

</code_context>

<specifics>
## Specific Ideas

- **The pivotal discovery** was that the roadmap's terse dependency note — *"SCN/WEB objects must
  never be silently inert"* — was describing a live gap, not a hypothetical: the engine seams exist,
  the store schema does not, and `scene_to_engine` covers 4 of 9 kinds. D-01 is the answer, cut at
  the exact line where an engine type already exists to convert into.
- **The user rejected an over-claimed type-safety argument.** The first TSX pitch offered
  `type BandSpectrum = number[] & { length: 105 }`, which does not actually type-check anything, and
  implied TypeScript would make the wire safe. It would not — `res.json()` is `any`. Re-examining
  produced D-10 (generated types), which is a genuinely stronger guarantee than the original
  proposal and connects to the same derive-on-read principle the phase-6 gates established. Prefer
  being corrected over being confident.
- Consistent user preference across Phases 6–7 for **one source of truth, drift made structurally
  impossible** rather than drift *detected*: pure-Rust CRS over FFI, flat files over SQLite,
  derive-on-read over cache-and-invalidate, edge UUIDs over indices, generated types over
  hand-written mirrors.

</specifics>

<deferred>
## Deferred Ideas

- **[ROADMAP COORDINATION] Engine mapping for the remaining 5 kinds.** `scene_to_engine` stays at
  4/9 (D-01). `ground_zone` → `GroundSegment` needs cut-plane path extraction (**Phase 9**);
  `elevation_point`/`elevation_line` → engine DGM needs terrain (**Phase 8**); `calc_area` →
  receiver grid (**Phase 10**). Each of those phases must pick its kind up.
- **[ROADMAP COORDINATION] Solve-time attachment** of `SolveJob.isolation` / `SolveJob.forest` from
  the persisted scene — **Phase 9/10**. Phase 7 proves the `TryFrom` conversion; nothing calls it
  from a solve path yet. This is the *only* remaining step before a drawn semi-transparent screen
  changes an acoustic result.
- **Terrain / GIS import** + the C `gdal` dependency + Windows provisioning + the GDAL/PROJ startup
  self-check — **Phase 8** (Phase-6 D-01/D-02, and REQUIREMENTS SVC-04's annotation).
- **WEB-05** source conditioning + interactive fast recalc (tensor MAC) + results-stale badge — Phase 10/11.
- **WEB-06** isophone fill-polygon overlays + editable colour scale — Phase 11.
- **WEB-07** calculation job submit / progress / abort / results UI + pre-run cost estimate — Phase 10.
  (Phase 6's job/SSE state machine and Phase 7's `EventSource` binding are the groundwork.)
- **WEB-11** receiver-point spectrum readout (1/12-oct expert + 1/3-oct display, dB(A)/dB(C)) — Phase 11.
- **WEB-12** weather what-if panel + difference-map view — Phase 9.
- **Directivity balloon phase seam** — `DirectivityBalloon` carries an optional per-band phase grid
  and `SolveJob::directivity_phase_rad` applies it, but **no harness/service site populates it yet**.
  Wire at the coherent directional-source composition path (Milestone 2, Phases 10–11, SRC-03).
  See `.planning/phases/04-*/deferred-items.md`. Phase 7 authors source spectra + SPL calibration
  only; balloon import is not in WEB-02.
- **SQLite persistence** — the documented upgrade path if per-feature querying ever demands it.
  The whole-scene PUT (and D-04's debounce) is the current contract.

</deferred>

---

*Phase: 7-Frontend Shell & Scene Editing*
*Context gathered: 2026-07-10*
