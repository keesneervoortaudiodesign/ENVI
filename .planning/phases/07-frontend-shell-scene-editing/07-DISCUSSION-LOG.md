# Phase 7: Frontend Shell & Scene Editing - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-10
**Phase:** 7-frontend-shell-scene-editing
**Areas discussed:** Backend reach of drawn objects, Scene state ownership & re-hydration, Isolation-spectrum editor & façade assignment, Draw-time validation & DGM scope, Frontend language & wire contract

---

## Backend reach of drawn objects

**Context surfaced before asking:** the engine has `SolveJob.isolation` (`solver.rs:120`) and
`SolveJob.forest` (`solver.rs:105`) from Phase 5, but `envi-store`'s DTO carries neither, and
`scene_to_engine` maps only 4 of 9 kinds. A semi-transparent screen drawn today would be inert —
exactly what the roadmap dependency note forbids.

| Option | Description | Selected |
|--------|-------------|----------|
| Typed + proven convertible | Typed validated DTOs in `envi-store`, round-tripped, with a tested `TryFrom` into the engine's existing `IsolationSpectrum` / `ForestCrossing`. No engine change, no solve-time attachment. | ✓ |
| Full 9-kind engine mapping now | Extend `scene_to_engine` to all 9 kinds — pre-empts Phases 8/9/10 and likely forces engine changes. | |
| Frontend-only, opaque passthrough | Persist isolation/forest as untyped GeoJSON properties; defer all typing to Phase 9. | |

**User's choice:** Typed + proven convertible (D-01)
**Notes:** Cut at the exact line where an engine type already exists to convert into. Isolation
reaches the engine via `SolveJob.isolation` at *solve* time, so no `envi-engine` modification is
required now — the 3-dep quarantine and byte-identical guarantee hold.

---

## Per-façade spectrum identity

**Hazard surfaced:** keying per-façade spectra by ring vertex index means any vertex insert/delete
shifts indices and silently re-points a spectrum at a different façade — a data-corruption bug no
test would catch.

| Option | Description | Selected |
|--------|-------------|----------|
| Stable per-edge UUIDs | Edge UUIDs assigned at draw time, preserved across vertex moves; insert splits (both inherit), delete merges. Spectra keyed by UUID. | ✓ |
| Edge index + default fallback | Compact, mirrors GeoJSON ring order — but silently re-points spectra on any geometry edit. | |
| Façades as separate features | Each façade its own `wall` LineString with `parent_building_id` — duplicates edge geometry, two sources of truth. | |

**User's choice:** Stable per-edge UUIDs (D-02)
**Notes:** Makes silent re-assignment structurally impossible rather than merely tested against.

---

## Scene state ownership & re-hydration

**Constraint:** SC4 requires survival across basemap switch, page reload, and project close/reopen.
`map.setStyle()` tears down all sources/layers; React StrictMode double-mounts in dev; Terra Draw's
internal store is bound to the map instance lifecycle.

| Option | Description | Selected |
|--------|-------------|----------|
| App store owns; Terra Draw is a view | Client store holds the canonical FeatureCollection; TD in a ref as a controlled rendering/editing surface, re-added on `style.load`. | ✓ |
| Terra Draw store owns; app mirrors | Fewer geometry copies, but TD's store can be dropped by `setStyle()` or StrictMode — needs manual snapshot/restore. | |
| Server is source of truth | Every mutation PUTs and refetches — puts a network RTT inside the drag loop; Phase-6 API has whole-scene PUT only. | |

**User's choice:** App store owns; Terra Draw is a view (D-03)
**Notes:** A 105-band spectrum is scene data, not geometry — it lives in the store, not in TD
feature properties. Basemap switch / reload / StrictMode collapse to one mechanism.

---

## Autosave cadence

| Option | Description | Selected |
|--------|-------------|----------|
| Debounced on committed edits | Committed mutations only (finished shape, released vertex, property change), ~750 ms debounce, coalesced whole-scene PUT, flush on close/navigate, dirty indicator. | ✓ |
| Explicit save button only | Predictable, but contradicts SVC-05 / Phase-6 D-06 which name autosave in the CRUD contract. | |
| Autosave on every committed edit, no debounce | Never more than one edit stale, but write-amplifies full-scene atomic disk writes. | |

**User's choice:** Debounced on committed edits (D-04)

---

## Isolation-spectrum editor: who interpolates

**Tension surfaced:** SVC-07 says "no Hz-based client-side acoustic math." Expanding a 1/3-octave
spectrum onto the 105-point grid is arguably exactly that. Because this is a localhost tool, a
server round-trip costs ~1 ms — the usual UX argument for client-side math does not apply.

| Option | Description | Selected |
|--------|-------------|----------|
| Server, via a meta endpoint | `POST /api/v1/meta/interpolate-spectrum`; one Rust implementation shared by editor preview and PUT validation. | ✓ |
| Server, expanded on PUT (no new endpoint) | No contract surface added, but no true interpolated curve until save+refetch — live preview reintroduces a client-side impl anyway. | |
| Client interpolates; server re-derives and rejects mismatch | Instant preview, drift caught loudly — but two implementations to keep in lockstep. | |

**User's choice:** Server, via a meta endpoint (D-05)
**Notes:** Phase 6 froze the existing endpoints; it did not forbid new ones.

---

## Isolation-spectrum editor: resolution switching

| Option | Description | Selected |
|--------|-------------|----------|
| Persist authored resolution + derived grid | Lossless 1/3 ⇄ 1/12 switching; editing a 1/12 band explicitly promotes the spectrum. | ✓ (then amended) |
| 105-grid is the only truth; downshift lossy + warned | Simplest DTO, but a 1/12 nudge is silently lost on the next save. | |
| Lock resolution at creation | Trivially lossless but rigid — a 1/3-oct datasheet spectrum can never be refined. | |

**User's choice:** Persist authored resolution + derived grid — **subsequently amended**
**Notes:** Claude flagged that persisting the derived `r_db[105]` beside its `authored` source is
the Phase-6 `CalcRecord.tensor_hash` shadow-cache anti-pattern (a hand-edited `scene.geojson` could
carry an `authored` that disagrees with its `r_db`, and `r_db` is what the acoustics consume). The
redundancy also buys nothing: `authored` at `resolution: "twelfth"` *is* the full grid.

**Amendment (D-06):** persist `authored` ONLY; derive `r_db[105]` on read. One source of truth,
nothing to invalidate, lossless switching preserved.

---

## Draw-time validation

**Ambiguity surfaced in SC2:** it says crossing polygons are "rejected at draw time" *and* that
"validation messages click-to-select and zoom to the offending object." If the polygon is rejected,
no offending object exists to select.

| Option | Description | Selected |
|--------|-------------|----------|
| Hard reject; zoom to the conflicting existing zone | Crossing shape never commits; geometry reverts; message zooms to the *existing* zone. Validation panel covers non-geometric issues on real objects. | ✓ |
| Commit but flag invalid; block calculation | Satisfies "offending object" wording literally, but the store then accepts engine-uninterpretable topology. | |
| Auto-repair by clipping | Never blocks, but silently alters deliberately-drawn geometry; SC2 says "rejected," not "repaired." | |

**User's choice:** Hard reject; zoom to the conflicting existing zone (D-07)
**Notes:** Resolves the SC2 ambiguity — the persistent validation panel (WEB-04) covers
non-geometric issues (semi-transparent wall with no spectrum, forest with zero density) whose
objects *do* exist and can be selected/zoomed.

---

## DGM scope

| Option | Description | Selected |
|--------|-------------|----------|
| Triangulate user-drawn elevation only, server-side | Constrained Delaunay via `spade` from `elevation_point` vertices + `elevation_line` breaklines. Phase 8 extends the same seam with imported samples. | ✓ |
| Draw and persist only; defer triangulation to Phase 8 | Smallest surface, but fails SC1 and leaves elevation objects inert. | |
| Triangulate and import terrain now | Requires the C `gdal` dependency, reopening the Windows provisioning decision Phase 6 deliberately deferred. | |

**User's choice:** Triangulate user-drawn elevation only, server-side (D-08)

---

## Frontend language & the wire contract

**Round 1** asked TSX vs JSX. Claude recommended TSX, illustrating it with
`type BandSpectrum = number[] & { length: 105 }`. The user chose TSX but then asked to reconsider.

**Round 2 — Claude corrected its own argument.** That type does not type-check anything (`number[]`
has `length: number`; the intersection is unconstructible from an array literal without a cast). More
importantly, the whole premise was wrong: TypeScript checks nothing at runtime — `await res.json()`
is `any`. Investigation found **~27 types** cross the wire (13 `envi-store` DTOs + 14
`envi-service` types), including `JobStatus`, an internally-tagged enum whose doc comment already
names it *"the contract Phase-7 binds its EventSource handling to."*

The real axis was therefore not TSX vs JSX, but **how the frontend learns the wire contract**.
Hand-authored TS types would be a second source of truth for 27 Rust types with nothing enforcing
agreement — the same shadow-cache anti-pattern, in a third costume.

| Option | Description | Selected |
|--------|-------------|----------|
| TSX + types generated from Rust | Derive TS from the serde DTOs, commit the generated `.ts`, test that regeneration yields no diff — mirrors the committed oracle-fixture pattern. Drift structurally impossible. | ✓ |
| TSX + zod validation at the fetch boundary | Runtime-enforced (catches what TS cannot, incl. length==105), zero backend change — but schemas remain a hand-maintained second copy. | |
| TSX + hand-written types, no validation | Fastest, full editor support — but compiles clean and breaks at runtime on any Rust rename. | |
| Plain JSX + zod, honor CLAUDE.md verbatim | Wire honest, CLAUDE.md unamended — loses 9-kind exhaustiveness and TD/react-map-gl inference. | |

**User's choice:** TSX + types generated from Rust (D-09 + D-10)
**Notes:** CLAUDE.md's "JSX/React" was amended to "TSX/React" explicitly in this phase's commit,
plus a new binding rule that wire types are generated, never hand-authored. **Research must
validate** that the generator faithfully renders `JobStatus`'s `#[serde(tag = "state")]` enum with
payload-carrying variants as a real TS discriminated union; if not, fall back to a zod schema for
that type specifically.

---

## Claude's Discretion

- Client state library (Zustand suggested, not mandated) and store shape
- Panel/layout composition; object palette and property inspector arrangement
- Last-object property inheritance mechanics (per-kind, session-scoped assumed)
- Source calibration UI details (sound power / spectrum / SPL-at-reference-point)
- Terra Draw mode configuration and which modes back which kinds
- Spectrum editor presentation: curve editor vs numeric table (or both); preset `R(f)` library
- Where the `spade` TIN lives, and whether the DGM is served by an endpoint or computed on demand
- Basemap tile source and map style
- Whether the generated-types no-drift test lives in `cargo test` or the JS toolchain

## Deferred Ideas

- Engine mapping for the remaining 5 kinds: `ground_zone` → Phase 9, `elevation_*` → Phase 8, `calc_area` → Phase 10
- Solve-time attachment of `SolveJob.isolation` / `SolveJob.forest` — Phase 9/10 (the last step before a drawn screen changes an acoustic result)
- Terrain/GIS import, C `gdal`, Windows provisioning, GDAL/PROJ startup self-check — Phase 8
- WEB-05 conditioning + fast recalc — Phase 10/11
- WEB-06 isophone overlays — Phase 11
- WEB-07 job submit/progress/abort/results UI + cost estimate — Phase 10
- WEB-11 receiver spectrum readout — Phase 11
- WEB-12 weather what-if + difference map — Phase 9
- Directivity balloon phase-seam wiring (SRC-03) — Milestone 2, Phases 10–11
- SQLite persistence — documented upgrade path, not now
