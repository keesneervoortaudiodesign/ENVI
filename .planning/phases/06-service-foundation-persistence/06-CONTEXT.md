# Phase 6: Service Foundation & Persistence - Context

**Gathered:** 2026-07-09
**Status:** Ready for planning

<domain>
## Phase Boundary

The first non-engine phase: stand up a self-hosted **`envi-service`** (axum) skeleton plus
**`envi-store`** persistence and a pure-Rust **`envi-geo`** CRS crate, and **lock the milestone's
non-retrofittable contracts** before any UI binds to them — project CRUD (a project is a folder),
the single CRS reprojection boundary (GEOX-04), the band-index wire format, the recondition/recompute
API split, and the job state machine.

**Requirements:** SVC-01 (project folder persistence), SVC-03 (axum HTTP API serving the built
bundle), SVC-04 (single self-hosted binary, localhost bind, startup self-check), SVC-05 (project CRUD
+ reopen-last), SVC-07 (server-side acoustics, band-index wire), GEOX-04 (one reprojection boundary).

**In scope:** the three new crates' skeletons, project-folder CRUD + scene round-trip, the pure-Rust
CRS boundary, the wire contracts (recondition/recompute split, band-index spectra, freq-axis meta
endpoint), the job state machine + SSE — all exercised end-to-end with **stubbed compute**.

**Out of scope (later phases):** real GDAL/PROJ C provisioning + raster I/O (Phase 8), the real
tensor/MAC and propagation behind the recondition/recompute endpoints (Phases 9–11), the frontend
itself (Phase 7 — Phase 6 only serves a placeholder `web/dist`), GIS import/weather (Phases 8–9).

**Depends on:** Nothing in Milestone 2 (engine types only — fully parallel-safe with the engine).
This phase creates the first non-engine crates and freezes the contracts every later Milestone-2 phase
binds to.

</domain>

<decisions>
## Implementation Decisions

### CRS boundary — pure Rust; GDAL C deferred (GEOX-04, SVC-04)
- **D-01:** Phase 6 needs **only coordinate reprojection** (WGS84 lon/lat ↔ project-local UTM
  meters), not raster/vector I/O. Reprojection is implemented in **pure Rust** — `proj4rs` or
  `geographiclib-rs` (research picks the exact crate and validates the SC3 landmark round-trip to
  ≤ 1 m). **No C toolchain, no `proj.db`/`GDAL_DATA`, no vcpkg/OSGeo4W in Phase 6.** This honors the
  CLAUDE.md house rule ("native Rust preferred over FFI wherever a mature one exists; accept
  `gdal`/`proj` only where no viable pure-Rust option exists").
- **D-02:** The **C-linked `gdal` dependency and its Windows provisioning are DEFERRED to Phase 8**
  (GIS ingestion), where raster I/O (GLO-30 COG `/vsicurl`, WorldCover, `GDALContourGenerateEx`) has
  no mature pure-Rust equivalent. `envi-gis` is **not created in Phase 6**. See the roadmap-coordination
  note in `<deferred>` — SC2's "GDAL/PROJ self-check" becomes a pure-Rust CRS self-check here.
- **D-03:** The CRS boundary lives in a new **dedicated pure-Rust crate `envi-geo`** owning the
  `LonLat` / `SceneXY` newtypes, `to_utm`/`to_wgs84`, and auto-UTM-zone selection. Imported by
  `envi-store`, `envi-service`, and later `envi-gis` (Phase 8) — one crate, one seam (GEOX-04's
  "exactly one reprojection boundary"), reused rather than re-derived downstream. Degree-magnitude
  scene coordinates are loudly rejected at this seam (SC3).

### Persistence — flat files (SVC-01, SVC-05)
- **D-04:** A project **is a folder** of **flat files**, not a SQLite database:
  `projects/<uuid>/project.json` (metadata + settings + the pinned UTM CRS) + `scene.geojson`
  (the FeatureCollection) + `calc/<calc_id>/manifest.json` (dims `[S,R,F=105]`, chunk layout, content
  hashes; `tensor/` + `pincoh/` dirs reserved for Phase 9/10). Git-diffable, human-inspectable,
  copyable/packable — the NoizCalc "project IS a directory" ethos. Justified by scale: an authored
  Nord2000 scene is small (hundreds of features), and bulk imported GIS data caches **separately** in
  `cache/` (Phase 8, DATA-04), so the authored project never grows large.
- **D-05:** The **serde DTO mirror stays in `envi-store`** (architecture Pattern 2) — engine scene
  types are never serde-derived (the three-dep engine quarantine holds byte-identical). Because the
  in-memory/wire model is the DTO, a future flat-file→SQLite swap (if per-feature querying ever
  demands it) is a mechanical storage-layer change, not a rewrite. SQLite is the documented upgrade
  path, not the Phase-6 choice.
- **D-06:** CRUD covers create / open / save (autosave) / duplicate / delete + reopen-last; a project
  survives service restart and round-trips scene GET/PUT (SC1). Autosave/reopen-last mechanics are
  Claude's discretion within this contract.

### Skeleton contract depth — full walking-skeleton, stubbed compute (SVC-07, SC4/SC5)
- **D-07:** Freeze **and exercise end-to-end** every compute-facing contract with **fake compute**:
  - **recondition/recompute split (SC4):** both endpoints exist with frozen request/response DTOs.
    **Tensor identity is keyed by content hash** (geometry + met + receiver-set); a `recondition`
    (conditioning→MAC) request whose `tensor_hash` mismatches the (stub) tensor is **actually
    rejected with 409** — contract-tested against an in-memory stub tensor. `recompute`
    (scene/terrain/ground/met→propagation) is the separate path. Conditioning is a **readout
    parameter, never hashed into tensor identity**. The full dirty-diff recalc **router (Tiers 0–3)**
    is Phase 10/11 — Phase 6 builds the **split + hash identity + rejection**, not the tier logic.
  - **band-index wire (SC4/SVC-07):** spectra cross the wire as dense `[105]` arrays **keyed by band
    index**; the 105-point 1/12-octave axis is served **once** at `GET /api/v1/meta/freq-axis`. No
    client-side acoustic math; `recondition` returns a canned spectrum by band index.
  - **job state machine (SC5):** a **synthetic stub job** genuinely runs `Queued → Running → Done`
    with **live SSE progress** and a **working cancel → Cancelled** (and `Failed(reason)`); submit /
    observe-live / cancel are demonstrable.
  - **single binary (SC2):** one axum binary binds **localhost** by default and serves a **placeholder
    `web/dist`** (the real frontend is Phase 7) — proving the single-deployable-serves-frontend
    contract. Embedding mechanism (`tower-http` ServeDir vs `include_dir!`) is Claude's discretion.
- **D-08:** **Startup self-check (SC2, adjusted):** the binary refuses to start unless a **pure-Rust
  CRS round-trip self-check** passes (reproject a known landmark WGS84→UTM→WGS84, assert ≤ 1 m; log
  the CRS/zone). The **GDAL/PROJ** version/`proj.db`/`GDAL_DATA` self-check moves to Phase 8 with the
  C dependency (D-02). Long CPU work (none real yet) uses a **dedicated worker/rayon**, never tokio's
  blocking pool (architecture Anti-Pattern 5) — the stub job establishes this shape.

### Claude's Discretion
- Exact pure-Rust reprojection crate (`proj4rs` vs `geographiclib-rs` vs `utm`) — research picks by
  the SC3 ≤1 m landmark accuracy + API fit; axum module/router layout and `/api/v1` versioning; the
  DTO / GeoJSON `properties.kind` feature-property schema (align with the architecture doc's kind
  list); autosave/reopen-last mechanics; `web/dist` embedding mechanism; SSE keep-alive details.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### The authoritative architecture (LOCKED technical baseline — read FIRST)
- `.planning/research/ARCHITECTURE.md` — the Milestone-2 crate topology, dependency graph, the API
  boundary + full endpoint list (`GET /api/v1/meta/freq-axis`, projects, scene, calculations,
  recondition/recompute), the persistence model (project-as-folder), the recalc-tier table
  (Pattern 3), the DTO-mirror serde quarantine (Pattern 2), the promote-the-solver pattern
  (Pattern 1), and the Anti-Patterns (esp. #1 serde-in-engine, #5 long CPU on spawn_blocking). This
  is the frozen baseline; every decision above either adopts it or is a **noted divergence**
  (pure-Rust CRS instead of C `proj` in Phase 6; flat files instead of SQLite).

### Requirements + roadmap
- `.planning/REQUIREMENTS.md` §SVC (SVC-01/03/04/05/07) + GEOX-04 — the requirement wording.
- `.planning/ROADMAP.md` "Phase 6" — goal + success criteria SC1–SC5 (note SC2's self-check wording
  is adjusted per D-08; surface this at planning).

### Workflow reference (descriptive)
- `docs/references/dbaudio-ti386-1.6-en.md` ch. 3–4 — the NoizCalc project-as-folder convention,
  object palette / `properties.kind` vocabulary, import→model→calculate→plot loop the API mirrors.

### Binding project contracts
- `.claude/CLAUDE.md` — the **native-Rust-over-FFI house rule** (the basis for D-01/D-02), the
  engine three-dep quarantine (must stay byte-identical — no serde in `envi-engine`), the
  105-point 1/12-octave band-index framework (compare by band index, never nominal Hz), English-only
  output, GitHub/commit conventions, and the mandatory GSD phase-completion gates.
- `.planning/PROJECT.md` — self-hosted internal tool, **light/no auth**, localhost, single
  integrated app (no ArrayCalc split), Nord2000-only.

### Existing code to build against (verify, do not modify)
- `crates/envi-engine/src/scene.rs` — the `Scene`/`Source`/`Receiver`/`Barrier`/`Building`/
  `TerrainProfile`/`GroundSegment`/`BandSpectrum` types the `envi-store` DTO mirror twins.
- `crates/envi-engine/src/freq.rs` — the 105-point `FreqAxis` served at `/meta/freq-axis`.
- `crates/envi-engine/src/tensor.rs` / `solver.rs` — the `TensorPair` / `TensorSink` / `SolveJob`
  shapes the (stubbed) recondition/recompute contracts must be forward-compatible with.
- root `Cargo.toml` — `members = ["crates/*"]` globs; `envi-geo`, `envi-store`, `envi-service` join
  automatically.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`envi-engine` scene + freq types** (`scene.rs`, `freq.rs`): the DTO mirror twins these; the
  105-point axis is served verbatim. No engine change — verify-only.
- **`TensorSink` / `SolveJob` / `TensorPair`** (`tensor.rs`, `solver.rs`): the Phase-4 seams the
  future real compute binds to; the Phase-6 stub tensor + recondition/recompute DTOs must not
  contradict their shape (content hash over geometry+met+receiver-set; conditioning as readout param).
- **Workspace globbing:** new crates need no manual workspace wiring.

### Established Patterns
- **Engine dependency quarantine** (enforced by `cargo tree -p envi-engine`): serde/geo/axum/proj
  NEVER enter the engine. `envi-geo` (pure Rust, but its own crate), `envi-store` (serde DTOs), and
  `envi-service` (axum) are all outside the quarantine.
- **Native-Rust-over-FFI** (CLAUDE.md): the decisive rule behind pure-Rust CRS in Phase 6.
- **Honest stubs, no false green:** the walking-skeleton exercises contracts with fake compute but
  must not claim real acoustic results — mirrors the engine's capability-gated honesty.

### Integration Points
- `web/` (Phase 7) ⇄ `envi-service` REST/GeoJSON (WGS84) + SSE — the only network seam; the tensor
  never crosses it.
- `envi-store` / `envi-service` → `envi-geo` reprojection (the one CRS seam).
- `envi-store` ⇄ engine types via DTO `From`/`TryFrom` (the serde quarantine seam).
- Future: `envi-service` → `envi_engine::solver`/`transfer::readout` (real compute, Phase 9–11) and
  `envi-gis` (Phase 8) — the Phase-6 contracts are the forward-compatible placeholders.

</code_context>

<specifics>
## Specific Ideas

- **"Isn't there an option without C?"** was the pivotal user question: it surfaced that Phase 6 needs
  only reprojection (not raster I/O), which has mature pure-Rust implementations — so the entire
  service skeleton builds with **zero C toolchain** and the worst Windows-setup risk is deferred to the
  phase (8) that genuinely needs GDAL. This is the phase's defining decision (D-01/D-02).
- Strong, consistent user preference for **simplicity + inspectability** in the foundational phase:
  pure-Rust CRS over FFI, flat JSON/GeoJSON over SQLite — while still demanding the **contracts be
  real and exercised end-to-end** (full walking-skeleton, D-07), not merely typed.

</specifics>

<deferred>
## Deferred Ideas

- **[ROADMAP COORDINATION] GDAL/PROJ Windows provisioning → Phase 8.** SC2's literal "GDAL/PROJ
  startup self-check" is replaced in Phase 6 by a pure-Rust CRS round-trip self-check (D-08). The C
  `gdal` dependency, its Windows provisioning decision (vcpkg / OSGeo4W / bundled — the options were
  scoped in this discussion), `proj.db`/`GDAL_DATA` resolution, and the GDAL/PROJ startup self-check
  all move to **Phase 8 (GIS Ingestion)**. Phase 8's discuss-phase must pick this up. The planner
  should note the SC2 adjustment so verification checks the pure-Rust self-check, not a GDAL one.
- **SQLite persistence** — the documented upgrade path if authored-scene per-feature querying ever
  demands it; the DTO mirror keeps the swap mechanical (D-05). Not now.
- **The full recalc router (Tiers 0–3)** — Phase 6 builds only the recondition/recompute split +
  content-hash identity + 409 rejection; the dirty-diff tier routing is Phase 10/11 (D-07).
- **Real tensor/MAC + propagation** behind the endpoints — Phases 9–11 (stubbed here).

</deferred>

---

*Phase: 6-Service Foundation & Persistence*
*Context gathered: 2026-07-09*
