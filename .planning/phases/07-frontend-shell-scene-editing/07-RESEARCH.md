# Phase 7: Frontend Shell & Scene Editing - Research

**Researched:** 2026-07-10
**Domain:** React/Vite/TSX map-authoring frontend (MapLibre 5 + Terra Draw) + supporting Rust backend work (typed DTOs, interpolation endpoint, `spade` TIN, generated wire types)
**Confidence:** HIGH (gate-critical library facts version-verified against npm/crates.io/official docs this session; a small number of type-mapping edge cases tagged `[ASSUMED]`/`[UNVERIFIED]` where docs were silent)

## Summary

Phase 7 is the first UI phase and, deliberately, not frontend-only: three locked decisions (D-01 typed isolation/forest DTOs + tested `TryFrom`, D-05 server-side interpolation endpoint, D-08 server-side `spade` constrained-Delaunay TIN) require Rust work in `envi-store` / `envi-service` / a new geometry crate, while `envi-engine` stays byte-identical. The frontend is React 19 + Vite + TSX driving a MapLibre GL JS 5 basemap with Terra Draw for all 9 scene-object kinds, with a client app store as the single source of truth (D-03), debounced whole-scene autosave (D-04), a server-owned isolation-spectrum interpolation path (D-05/D-06), draw-time ground-zone topology validation (D-07), and generated TS wire types (D-10). The visual layer adopts metrao3's dark ops-console token set verbatim (D-11/D-12), and the basemap is a dark MapLibre vector style with no API key (D-13).

All four hard gates resolve cleanly. **Gate 1** (Terra Draw ⇄ react-map-gl lifecycle): the app-store-canonical pattern is achievable because Terra Draw's `change` event exposes `context.origin === "api"`, letting you ignore your own `addFeatures` writes and avoid feedback loops; re-hydration hooks on MapLibre's `style.load` event after `setStyle()`. **Gate 2** (TS generation): `ts-rs` 12 renders `JobStatus`'s internally-tagged enum as a real discriminated union via its default `serde-compat` — no zod fallback needed for `JobStatus` itself, though a couple of type mappings (`[f64; 105]`, `Uuid`) need explicit handling. **Gate 3** (`spade` TIN): `spade` 2.15.1 provides `ConstrainedDelaunayTriangulation` with `add_constraint_edges` for breaklines and `Barycentric` interpolation for Z queries; it **panics** on interior-intersecting constraints, so the endpoint must pre-validate. **Gate 4** (basemap): Protomaps (CC0 dark theme, PMTiles) is the recommendation — it is the only option that can be *genuinely* offline via a bundled PMTiles extract; OpenFreeMap's public endpoint is the zero-effort fallback but is a network dependency.

**Primary recommendation:** Build the Gate-1 lifecycle spike first (react-map-gl 8 + Terra Draw 1.31 + maplibre-gl 5 with `style.load` re-hydration and StrictMode guard) as a standalone plan before any feature plans; land the three backend seams (DTOs, interpolation endpoint, `envi-dgm` TIN) in parallel; then layer the drawing/editor/validation UI on top.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Basemap render + draw interaction | Browser (MapLibre + Terra Draw) | — | Pure client rendering; no acoustic math (SVC-07) |
| Canonical scene FeatureCollection | Browser (app store, D-03) | Backend (persistence via PUT) | Store owns geometry+props; TD is a controlled view |
| Ground-zone topology validation (draw-time) | Browser (turf.js, D-07) | — | Pure geometry, not acoustics; SVC-07 does not forbid it |
| Isolation-spectrum interpolation | API / Backend (D-05) | — | SVC-07: no Hz-based client acoustic math; one Rust impl |
| Scene persistence (whole-scene PUT) | API / Backend (Phase 6) | Disk (`envi-store` project folder) | Frozen Phase-6 contract; no per-feature PATCH |
| Constrained-Delaunay DGM | API / Backend (`spade`, D-08) | — | Server-side TIN from user-drawn elevation |
| Wire type contract | Build-time (Rust → generated `.ts`, D-10) | — | Generated from serde source of truth; committed artifact |
| Isolation/forest → engine conversion | Backend (`TryFrom`, D-01) | Engine types (verify-only) | Store owns serde; engine stays quarantined |

## Standard Stack

### Core (frontend — `web/`, new toolchain)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `vite` | 8.1.4 | Build/dev server, outputs to `web/dist` | `[VERIFIED: npm registry]` current; the default React/TS bundler |
| `react` + `react-dom` | 19.2.7 | UI framework (D-09 TSX) | `[VERIFIED: npm registry]` current stable; matches react-map-gl 8 peer range |
| `typescript` | ~5.x latest | Discriminated-union exhaustiveness (D-09) | `[CITED: typescriptlang.org]` |
| `maplibre-gl` | 5.24.0 | OSM/vector basemap (WEB-01) | `[VERIFIED: npm registry]` MapLibre GL JS 5 line |
| `react-map-gl` | 8.1.1 | React wrapper for maplibre-gl (WEB-01) | `[VERIFIED: npm registry]`; `import { Map } from 'react-map-gl/maplibre'` |
| `terra-draw` | 1.31.2 | Draw/edit all 9 kinds (WEB-02/03/04) | `[VERIFIED: npm registry]`; framework-agnostic draw engine, TS-first |
| `terra-draw-maplibre-gl-adapter` | 1.4.1 | Binds Terra Draw to a maplibre-gl map | `[VERIFIED: npm registry]`; the `TerraDrawMapLibreGLAdapter` |
| `zustand` | 5.0.14 | Canonical scene store (D-03, Claude's discretion) | `[VERIFIED: npm registry]`; minimal, no-provider, TS-native |
| `@turf/turf` | 7.3.5 | Client-side draw-time geometry predicates (D-07) | `[VERIFIED: npm registry]`; modular; use scoped `@turf/boolean-*` |

### Supporting (backend — Rust)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `spade` | 2.15.1 | Constrained-Delaunay TIN from elevation (D-08) | `[VERIFIED: crates.io]`; lives OUTSIDE `envi-engine` (quarantine) |
| `ts-rs` | 12.0.1 | Generate committed `.ts` wire types (D-10) | `[VERIFIED: crates.io]`; derive on the `envi-store`/`envi-service` DTOs |

### Dev-only
| Library | Version | Purpose | Notes |
|---------|---------|---------|-------|
| `@playwright/test` | 1.61.1 | Browser UAT of the real bundle | `[VERIFIED: npm registry]`; devDependency only; `page.route` mocks `/api/*` |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `ts-rs` | `schemars` 1.2.1 → JSON Schema → `json-schema-to-typescript` | Two-hop pipeline, extra npm dep at gen time, JSON-Schema does not model internally-tagged enums as cleanly; `ts-rs` emits TS directly. Prefer `ts-rs`. |
| `zustand` | Redux Toolkit / Jotai / `useReducer` | Zustand is the lightest fit for "one canonical FeatureCollection + maps"; RTK is heavyweight, Jotai atom-per-feature fights the whole-scene PUT model |
| Protomaps basemap | OpenFreeMap / Versatiles / CARTO raster | See Gate 4 — Protomaps is the only *genuinely offline-capable* option |
| `@turf/turf` (client validation) | Server-side validation on PUT | D-07 needs *instant* draw-time feedback; server round-trip adds latency. Keep the server PUT check too as a backstop (defense in depth) |

**Installation (frontend):**
```bash
# in web/
npm install maplibre-gl@5 react-map-gl@8 terra-draw@1 terra-draw-maplibre-gl-adapter@1 \
  zustand@5 @turf/turf@7 react@19 react-dom@19
npm install -D vite@8 typescript @playwright/test@1 @types/react @types/react-dom
```

**Installation (Rust — workspace):**
```toml
# in the crate that owns the TIN (recommend new envi-dgm), NOT envi-engine:
spade = "2.15"
# on envi-store / envi-service DTO crates:
ts-rs = "12"
```

**Version verification:** All versions above were confirmed this session via `npm view <pkg> version` and the crates.io API (`GET /api/v1/crates/<name>`). Publish freshness: `spade` 2.15.1 released 2026-07-04.

## Package Legitimacy Audit

> All packages are long-established, high-download, single-maintainer-or-org projects with public source repos. Versions verified via registry this session.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| maplibre-gl | npm | est. 2020 (MapLibre org fork of mapbox-gl) | very high | github.com/maplibre/maplibre-gl-js | OK | Approved |
| react-map-gl | npm | est. 2016 (vis.gl / Uber) | very high | github.com/visgl/react-map-gl | OK | Approved |
| terra-draw | npm | est. 2023 | growing | github.com/JamesLMilner/terra-draw | OK | Approved (single-maintainer, active) |
| terra-draw-maplibre-gl-adapter | npm | 2024 | moderate | github.com/JamesLMilner/terra-draw (monorepo) | OK | Approved — official adapter, same author |
| zustand | npm | est. 2019 (pmndrs) | very high | github.com/pmndrs/zustand | OK | Approved |
| @turf/turf | npm | est. 2014 | very high | github.com/Turfjs/turf | OK | Approved |
| @playwright/test | npm | est. 2020 (Microsoft) | very high | github.com/microsoft/playwright | OK | Approved |
| vite | npm | est. 2020 | very high | github.com/vitejs/vite | OK | Approved |
| spade | crates.io | est. 2016 | high | github.com/Stoeoef/spade | OK | Approved |
| ts-rs | crates.io | est. 2020 | high | github.com/Aleph-Alpha/ts-rs | OK | Approved |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

> Note: package names here were confirmed against official documentation (MapLibre/vis.gl/Turf/Protomaps docs, terra-draw GitHub, crates.io) AND registry lookup — not from training memory alone. The `terra-draw-maplibre-gl-adapter` name in particular was verified against both npm and the maplibre plugins page.

## Architecture Patterns

### System Architecture Diagram

```
                                  BROWSER (web/, React 19 + Vite)
  ┌───────────────────────────────────────────────────────────────────────────┐
  │  User draws/edits ──▶ Terra Draw (controlled view, held in useRef)          │
  │        │                    │  on "finish"/"select"/property-edit (COMMIT)  │
  │        │                    ▼                                               │
  │        │            Zustand app store  ◀── SINGLE SOURCE OF TRUTH (D-03)    │
  │        │            { features by id, edgeId→spectrum, selection, dirty }   │
  │        │                    │                                               │
  │        │        ┌───────────┼──────────────┬───────────────┐               │
  │        │        ▼           ▼              ▼               ▼               │
  │   draw-time   debounced   spectrum      basemap         validation         │
  │   turf.js     autosave    editor        setStyle()      panel (WEB-04)     │
  │   validate    (~750ms,    (live         ──▶ on          crit/warn dots     │
  │   (D-07)      flush on    preview)      style.load:                        │
  │        │       unload)       │          re-add features                    │
  └────────┼──────────┼──────────┼──────────────┼────────────────────────────-┘
           │          │          │              │  (all /api/* = the only network seam)
           ▼          ▼          ▼              ▼
  ┌───────────────────────────────────────────────────────────────────────────┐
  │  envi-service (axum 0.8, /api/v1)   ── serves web/dist via ServeDir + SPA   │
  │   PUT /projects/{id}/scene ─────────▶ envi-store: validate + whole-scene    │
  │   POST /meta/interpolate-spectrum ──▶ shared Rust interp (D-05) ──┐         │
  │   GET  /meta/freq-axis (Phase 6)                                  │         │
  │   [new] DGM endpoint ───────────────▶ envi-dgm: spade CDT (D-08)  │         │
  └──────────────────────────────────────────────────────────────────┼────────┘
                                                                      │
                          envi-store DTOs  ──TryFrom (tested, D-01)──▶│
                          IsolationSpectrumDto / ForestParamsDto      ▼
                          envi-engine (BYTE-IDENTICAL, verify-only): IsolationSpectrum, ForestCrossing
                     Basemap vector tiles ◀── Protomaps PMTiles (bundled offline) OR OpenFreeMap (network)
```

### Recommended Project Structure
```
web/                          # repo-root frontend package (package.json here)
├── package.json
├── vite.config.ts
├── tsconfig.json
├── playwright.config.ts
├── index.html                # replaces the Phase-6 placeholder; ZERO external assets
├── public/
│   └── basemap/              # (Gate-4 offline option) bundled PMTiles + glyphs/sprites
├── src/
│   ├── theme.css            # metrao3 tokens, copied VERBATIM (system-font variant)
│   ├── app.css              # ENVI-specific component CSS layered on tokens
│   ├── generated/
│   │   └── wire.ts          # COMMITTED, ts-rs output (D-10); no-drift test guards it
│   ├── icons.ts             # inline-SVG via DOM APIs (metrao3 pattern, never innerHTML)
│   ├── store/               # zustand: scene store, selection, dirty flag (D-03)
│   ├── map/                 # MapLibre + Terra Draw lifecycle (Gate-1 spike lands here)
│   ├── draw/                # per-kind mode config, draw handlers (never-exhaustive union)
│   ├── panels/              # object palette, property inspector, validation panel
│   ├── spectrum/            # isolation-spectrum editor (table/curve), interpolate calls
│   ├── validate/            # turf.js ground-zone topology checks (D-07)
│   └── api/                 # fetch client using generated wire.ts types
└── tests/e2e/
    ├── _mocks.ts            # page.route /api/* fixtures (metrao3 house pattern)
    └── *.spec.ts
```

### Pattern 1: App-store-canonical Terra Draw (D-03) — the Gate-1 core
**What:** The Zustand store owns the FeatureCollection; Terra Draw is a *view* re-hydrated from the store. Loop avoidance uses `context.origin === "api"`.
**When to use:** Every scene mutation.
```typescript
// Source: terra-draw guides/6.EVENTS.md (change event context.origin), verified 2026-07
// User edits → store; store → TD via addFeatures; ignore TD echoes of our own writes.
draw.on("change", (ids, type, context) => {
  if (context?.origin === "api") return;      // our own addFeatures — do NOT write back
  // user-driven create/update/delete: pull the authoritative geometry from TD snapshot
  const snapshot = draw.getSnapshot();         // GeoJSON features by id
  store.getState().applyTerraDrawChange(ids, type, snapshot);
});

// Re-hydrate the store's features INTO Terra Draw (e.g. after style.load, reload):
function rehydrate(draw, features) {
  draw.clear();
  draw.addFeatures(features);                  // fires change with origin:"api" → ignored above
}
```

### Pattern 2: `setStyle()` re-hydration on `style.load` (SC4)
**What:** `map.setStyle()` destroys every source and layer, taking Terra Draw's rendered layers with them. Re-add after the new style loads.
```typescript
// Source: MapLibre GL JS 5 docs (setStyle destroys sources/layers) + terra-draw issue #197 pattern
map.setStyle(newStyleUrl);
map.once("style.load", () => {               // fires when the NEW style is fully parsed
  // Terra Draw's adapter must re-register its layers, then re-add features from the store:
  rehydrate(draw, store.getState().terraDrawFeatures());
});
// Use "style.load" (not "styledata", which fires repeatedly) as the single re-hydration hook.
```

### Pattern 3: Instance-in-ref + StrictMode guard (React 19)
**What:** Create the Terra Draw instance once, after the map is ready, in a `useEffect`; guard against StrictMode's double-invoke.
```typescript
// Source: react-map-gl useMap() + terra-draw issue #172 (init timing), verified 2026-07
const drawRef = useRef<TerraDraw | null>(null);
const { current: mapRef } = useMap();
useEffect(() => {
  const map = mapRef?.getMap();               // underlying maplibregl.Map
  if (!map || drawRef.current) return;        // guard: StrictMode double-mount / already built
  const build = () => {
    const draw = new TerraDraw({
      adapter: new TerraDrawMapLibreGLAdapter({ map, lib: maplibregl }),
      modes: [/* polygon, point, linestring, select, ... per kind */],
    });
    draw.start();
    drawRef.current = draw;
    // wire change/finish/select handlers here
  };
  if (map.isStyleLoaded()) build(); else map.once("load", build);
  return () => { drawRef.current?.stop(); drawRef.current = null; };  // cleanup for StrictMode
}, [mapRef]);
```

### Pattern 4: Debounced committed-edit autosave (D-04)
**What:** Autosave triggers on *committed* mutations only — never intermediate drag frames.
**Precise trigger:** The Terra Draw `finish` event (fires on drag *release/commit*, actions `draw`/`dragFeature`/`dragCoordinate`/`dragCoordinateResize`) plus property-panel changes — **NOT** the `change` event (which fires continuously during drag with `type:"update"`).
```typescript
// Source: terra-draw guides/6.EVENTS.md — finish fires only on completion, verified 2026-07
draw.on("finish", (id, ctx) => { store.getState().markDirty(); scheduleAutosave(); });
// scheduleAutosave: debounce ~750ms → one whole-scene PUT /projects/{id}/scene (no PATCH).
// flush on: window "beforeunload"/"pagehide" and project close/navigate.
```

### Pattern 5: Per-edge UUID survival across vertex insert/delete (D-02) — the load-bearing recovery
**What:** Terra Draw does **NOT** tell you which edge was split or merged on a vertex insert/delete — the `change` event gives you the feature id and `type:"update"`, not the affected edge index. Per-edge UUIDs must be recovered by diffing the previous vs next polygon ring.
**Algorithm (deterministic, geometry-based):**
```
on building geometry update (change type "update"):
  prev_ring = store.buildings[id].ring          # ordered [v0..vn], with edge_ids[i] = edge (vi → vi+1)
  next_ring = snapshot.features[id].ring
  1. Match unchanged vertices by coordinate identity (exact f64 equality on the coords TD echoes).
  2. INSERT case (next has exactly one more vertex): find the inserted vertex w between matched
     vi and vi+1. Split parent edge_ids[i] into TWO new edges, BOTH inheriting the parent's spectrum
     (D-02). Generate 2 fresh UUIDs OR keep parent UUID on one half + 1 new UUID (choose one; keep parent
     on the FIRST half for stability). facade_isolation[parent] → both halves.
  3. DELETE case (next has one fewer): the two edges adjacent to the removed vertex MERGE into one.
     Keep the FIRST edge's UUID; drop the second's spectrum entry (or prefer the non-default one).
  4. MOVE case (same count, one vertex coord changed): edge_ids unchanged — endpoints keep identity.
  5. Rebuild edge_ids from next_ring; reconcile facade_isolation map by UUID.
```
**Why:** An index key would silently re-point a spectrum at the wrong façade after any insert. UUIDs keyed in the store (not in TD feature properties) make it structurally impossible. This diff runs in the store's `applyTerraDrawChange`, never in Terra Draw.

### Pattern 6: Server-owned spectrum interpolation (D-05/D-06) + exact band-index math
**What:** One Rust function, shared by the `POST /meta/interpolate-spectrum` endpoint, the read-path `r_db[105]` derivation, and PUT validation.
**Verified band-index facts** (from `envi_engine::freq`, this session):
- `N_BANDS = 105`, indices `0..=104`; band `i` ↔ `x = i − 64`, `f = 1000·G^(x/12)`, `G = 10^(3/10)`.
- **1/3-octave centres** = indices `0, 4, 8, …, 104` (stride 4) → 27 bands. (`third_octave_indices` in `meta.rs`.)
- **1/1-octave centres** = stride 12, offset 4 → indices `4, 16, 28, 40, 52, 64, 76, 88, 100` → **9 bands** (nominal 31.5, 63, 125, 250, 500, 1000, 2000, 4000, 8000). 1000 Hz is index 64.
- So `authored.values` length **9 → octave** (grid idx `4 + 12·k`), **27 → third** (grid idx `4·k`), **105 → twelfth** (all).
```rust
// resolution ∈ {octave(9), third(27), twelfth(105)} → dense [f64; 105]
// linear interpolation IN BAND INDEX (== linear in log-frequency, SCN-03) between anchors.
fn interpolate(resolution: Resolution, values: &[f64]) -> Result<[f64; 105], SpectrumError> {
    let anchors: Vec<(usize, f64)> = match resolution {
        Octave  => (0..9 ).map(|k| (4 + 12*k, values[k])).collect(),   // idx 4..100
        Third   => (0..27).map(|k| (4*k,      values[k])).collect(),   // idx 0..104
        Twelfth => return values.try_into().map(...),                  // identity
    };
    // for each band i in 0..105: linear-interp between bracketing anchors;
    // EXTRAPOLATION outside authored range (e.g. octave leaves bands 0..3 and 101..104 unspanned):
    //   flat-hold clamp to the nearest endpoint anchor value  [DECISION — see Assumptions]
    // then clamp result to IsolationSpectrum's [0.0, 1000.0] domain and reject non-finite.
}
```
**SCN-03 wording (verbatim):** *"accept 1/1-octave or 1/3-octave input and **linearly interpolate** (dB across band index = linear in log-frequency; octave/third-octave centres fall exactly on 1/12-octave band indices) to the full grid."* The stride math above makes the "fall exactly on band indices" guarantee structural.

### Pattern 7: Generated wire types + no-drift test (D-10)
```rust
// Derive on the envi-store / envi-service DTOs (both OUTSIDE the engine quarantine):
#[derive(Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../web/src/generated/wire.ts")]
#[serde(deny_unknown_fields)]
pub struct IsolationSpectrumDto { /* ... */ }
```
- Generation is invoked by `cargo test` (ts-rs exports during a test run) — keeps it in the Rust toolchain (Claude's discretion says Rust or JS; Rust matches the oracle pattern).
- Commit the generated `web/src/generated/wire.ts`.
- **No-drift test** mirrors `tools/nord2000_oracle/`: a `#[test]` regenerates to a temp path and asserts byte-equality with the committed file (or a CI-style `git diff --exit-code` on `wire.ts`). Same "generate-at-dev-time, commit-the-artifact, test-asserts-no-drift" shape as `oracle_ground.rs`.

### Anti-Patterns to Avoid
- **Storing the 105-band spectrum in Terra Draw feature properties.** A 105-band spectrum is scene data, not geometry (D-03/D-11) — it lives in the store, keyed by feature/edge UUID. TD properties are for styling only.
- **Debouncing on the `change` event.** It fires every drag frame; you would PUT on every mouse-move. Use `finish` (D-04).
- **Index-keyed façade spectra.** Silently corrupts assignment on vertex insert (D-02). UUID-keyed only.
- **Hardcoding the freq axis or doing Hz math client-side.** Violates SVC-07; the axis comes from `GET /meta/freq-axis`, interpolation from the server (D-05).
- **Hand-writing the wire types.** `res.json()` is `any`; a renamed Rust field compiles clean and fails in the browser (D-10).
- **`spade` in `envi-engine`.** Breaks the 3-dep quarantine (`cargo tree -p envi-engine` must stay `ndarray`/`num-complex`/`thiserror`). The TIN lives elsewhere.
- **`styledata` as the re-hydration hook.** It fires repeatedly; use `style.load` once.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Polygon drawing/editing with vertex handles | Custom canvas draw tooling | Terra Draw modes | Snapping, midpoints, selection, drag are solved and subtle |
| React↔maplibre binding | Manual map lifecycle in effects | react-map-gl `<Map>` + `useMap()` | Handles resize, controlled/uncontrolled viewport, ref plumbing |
| Constrained Delaunay TIN | Hand-rolled triangulation | `spade` CDT | Exact geometric predicates; hand-rolled loses to precision bugs |
| Geometry predicates (contains/overlap/disjoint) | Custom point-in-polygon | `@turf/boolean-*` | Winding/shared-edge/self-intersection edge cases are numerous |
| Rust→TS type mirror | Hand-written interfaces | `ts-rs` | Second source of truth with nothing enforcing agreement (D-10) |
| Client state | Bespoke event bus | zustand | Selectors + immutable updates + devtools for free |
| Discriminated-union JSON parsing at runtime | `as JobStatus` casts | (types are compile-time only) validate at the SSE boundary | TS erases at runtime; see Gate-2 note |

**Key insight:** Every one of ENVI's Phase-6/7 preferences — one source of truth, drift made *structurally impossible* rather than *detected* — is served by these libraries' opinionated models (generated types, UUID keys in the store, server-owned interpolation). Custom code re-introduces the second-source-of-truth this phase exists to eliminate.

## Common Pitfalls

### Pitfall 1: `setStyle()` silently deletes the drawn scene
**What goes wrong:** Switching basemap (SC4) wipes Terra Draw's rendered features; they never come back.
**Why:** MapLibre destroys all sources/layers on style swap; TD's layers go with them.
**How to avoid:** App-store-canonical (D-03) + re-hydrate on `style.load` (Pattern 2). Never treat TD's internal store as durable.
**Warning signs:** Features vanish on theme/basemap toggle; `Cannot read properties of undefined` on next click (terra-draw issue #197).

### Pitfall 2: React StrictMode double-mounts the map/draw in dev
**What goes wrong:** Two Terra Draw instances bind to one map; events double-fire; "Terra Draw is not enabled" (issue #172).
**Why:** StrictMode invokes effects twice in dev; TD init timing races the map `load`.
**How to avoid:** Instance-in-ref guard + build after `map.isStyleLoaded()`/`load` + cleanup that calls `draw.stop()` (Pattern 3).
**Warning signs:** Duplicate change events; adapter errors only in dev.

### Pitfall 3: `add_constraint_edges` panics on intersecting breaklines
**What goes wrong:** Two user-drawn `elevation_line` breaklines that cross → `spade` panics → 500 / thread abort.
**Why:** `spade` supports only *weakly intersecting* constraints (touch at endpoints only); interior intersection panics.
**How to avoid:** Pre-validate with `can_add_constraint` / `intersects_constraint`, or use the non-panicking `try_add_constraint`; return a typed 4xx with the offending segment. (See Gate 3.)
**Warning signs:** Panics on real terrain input; degenerate (collinear/duplicate) point sets.

### Pitfall 4: Comparing spectra by nominal Hz
**What goes wrong:** `f == 31.5` float compares fail; wrong band picked.
**Why:** Nominal labels (31.5) are not the exact centres (31.6228). The engine is emphatic: index by `BandIdx`, never float Hz.
**How to avoid:** All spectrum I/O is dense `[105]` by index; octave/third anchors are integer grid indices (Pattern 6). `nominal_third_octave_hz` is display-only.

### Pitfall 5: `[f64; 105]` and `Uuid` in generated TS
**What goes wrong:** `ts-rs` maps `[f64; 105]` to `Array<number>` (length erased) and needs the `uuid-impl` feature for `Uuid → string`.
**Why:** TS has no fixed-length numeric array; `ts-rs` type impls for external crates are feature-gated.
**How to avoid:** Enable `ts-rs` features `serde-compat` (default) + `uuid-impl`; accept `number[]` for band arrays (runtime length is enforced server-side by the existing `BadBandCount` check) and document it. `[ASSUMED]` on exact `Uuid` feature name — verify against ts-rs 12 Cargo features at wiring time.

### Pitfall 6: axum 0.8 path syntax
**What goes wrong:** `/:cid` panics at router construction.
**How to avoid:** The new `POST /api/v1/meta/interpolate-spectrum` uses no params; any future param route uses brace `/{id}` syntax (already the repo convention, `api/mod.rs`).

## Code Examples

### Gate 3: `spade` constrained-Delaunay TIN with Z interpolation
```rust
// Source: docs.rs/spade/2.15.1 — ConstrainedDelaunayTriangulation, verified 2026-07
use spade::{ConstrainedDelaunayTriangulation, Point2, Triangulation, HasPosition};

struct ElevVertex { pos: Point2<f64>, z: f64 }
impl HasPosition for ElevVertex { type Scalar = f64; fn position(&self) -> Point2<f64> { self.pos } }

let mut cdt: ConstrainedDelaunayTriangulation<ElevVertex> = ConstrainedDelaunayTriangulation::new();
// scattered elevation_point vertices:
for p in points { cdt.insert(ElevVertex { pos: Point2::new(p.x, p.y), z: p.z })?; }
// elevation_line breaklines (polyline; closed=false). PRE-CHECK to avoid panic on interior crossing:
for line in breaklines {
    // add_constraint_edges panics on interior intersection with an existing constraint;
    // guard each segment, or use try_add_constraint per pair.
    cdt.add_constraint_edges(line_vertices, false)?; // Result<(), InsertionError>
}
// Z query at arbitrary (x,y) via barycentric interpolation over the containing face:
let interp = cdt.barycentric();
let z = interp.interpolate(|v| v.data().z, Point2::new(qx, qy)); // Option<f64>
```
- Methods verified: `insert -> Result<FixedVertexHandle, InsertionError>`; `add_constraint(from, to) -> bool`; `add_constraint_edges(iter, closed) -> Result<(), InsertionError>`; `can_add_constraint(from, to) -> bool`; `intersects_constraint(a, b) -> bool`. Interior-intersecting constraint → **panics** (use pre-check or `try_add_constraint`). Interpolation: `Barycentric` and `NaturalNeighbor` structs.
- Degenerate inputs: 0 points → empty TIN (reject at endpoint: nothing to interpolate); 1 point / all-collinear → no triangles, `barycentric().interpolate` returns `None` outside/along the hull. **Reject** (< 3 non-collinear points) with a typed error; do not 500.

### Gate 2: `JobStatus` as a real TS discriminated union
```typescript
// Source: ts-rs wiki "Deriving the TS trait" + serde-compat, verified 2026-07.
// #[serde(tag="state", rename_all="snake_case")] on the Rust enum generates:
export type JobStatus =
  | { state: "queued" }
  | { state: "running"; progress: number; message: string }
  | { state: "done" }
  | { state: "failed"; reason: string }
  | { state: "cancelled" };
```
- `ts-rs` 12 with the default `serde-compat` feature honors `#[serde(tag=...)]` (equivalent to `#[ts(tag=...)]`) and `rename_all`, emitting the union above. Named-field variants (`Running`, `Failed`) are fully supported. The only banned shapes are tuple variants and newtype-over-non-struct — `JobStatus` has neither. **Verdict: no zod fallback needed for `JobStatus`.** (Still validate the discriminant at the `EventSource` boundary defensively, since TS erases at runtime — a `switch(status.state)` with a `never` default is the exhaustiveness guard.)

### Gate 4: dark basemap wiring (Protomaps offline / OpenFreeMap network)
```typescript
// Option A (RECOMMENDED, genuinely offline): bundled PMTiles + protomaps dark theme
// Source: docs.protomaps.com/pmtiles/maplibre + protomaps-themes-base, verified 2026-07
import { Protocol } from "pmtiles";
import layers from "protomaps-themes-base";
const protocol = new Protocol(); maplibregl.addProtocol("pmtiles", protocol.tile);
const style = {
  version: 8,
  glyphs: "/basemap/fonts/{fontstack}/{range}.pbf",   // bundled in public/basemap (offline)
  sprite: "/basemap/sprite",
  sources: { protomaps: { type: "vector", url: "pmtiles:///basemap/area.pmtiles",
                          attribution: '<a href="https://openstreetmap.org">OSM</a>' } },
  layers: layers("protomaps", "dark"),
};

// Option B (fallback, network, zero prep): OpenFreeMap public dark endpoint
const style = "https://tiles.openfreemap.org/styles/dark";   // no API key; requires network
```
- **AttributionControl:** `new maplibregl.AttributionControl({ compact: true })` or set `attribution` per source (shown above). Protomaps is CC0 (attribution requested, not required); OSM data attribution is required by CLAUDE.md regardless of style choice.

### Visual scheme wiring (D-11/D-12)
```
web/src/theme.css   ← copy metrao3 crates/metrao3-web/ui/src/theme.css VERBATIM (system-font variant)
web/src/app.css     ← ENVI component CSS: control heights from --row-h (36) / --row-h-lg,
                       .dense variant (28) for the 105-band spectrum table; 44px min-height
                       RETAINED only on .btn.primary / .btn.danger (Save, Delete project) per D-12.
```
Per-kind scene-object palette (D-11 discretion, drawn from existing tokens — do NOT invent):
| Kind | Token hue | Rationale |
|------|-----------|-----------|
| source | `--color-primary` #4ea8ff | The emitter — the accent |
| receiver | `--color-ok` #22c55e | Measurement points |
| building | `--color-text-muted` / `--color-surface-3` | Neutral structure |
| wall / screen | `--color-warn` #f59e0b when semi-transparent-without-spectrum | Ties to WEB-04 warn state |
| forest | `--color-ok` (muted) | Vegetation |
| ground_zone | `--color-off` #5a6270 base; `--color-crit` on rejected crossing (D-07) | Severity vocabulary |
| elevation_point/line | `--color-info` #4ea8ff | Terrain metadata |
| calc_area | `--color-primary` dashed outline | The domain boundary |

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| mapbox-gl-draw | Terra Draw (framework-agnostic, TS-first, adapter model) | 2023+ | Works across MapLibre/Leaflet/Google; controlled-view store model |
| react-map-gl 7 (mapbox default) | react-map-gl 8, explicit `react-map-gl/maplibre` entry | 2024–25 | First-class maplibre-gl 5 support |
| Hand-written `api.ts` interfaces (metrao3) | Generated types from serde (`ts-rs`) | — | Drift structurally impossible (D-10) |
| Hosted tiles requiring API keys (Mapbox) | Protomaps PMTiles (CC0) / OpenFreeMap (no key) | 2023+ | Self-hostable, key-free, offline-capable |

**Deprecated/outdated:**
- `mapbox-gl-draw` for new MapLibre projects — Terra Draw is the current choice.
- `/:id` axum path syntax — dead since axum 0.8.

## Validation Architecture

> `nyquist_validation` is `false` in config, so no Nyquist test-map is required. This section instead maps the phase's SC1–SC4 to concrete testable checks, as the phase task requested.

### Test Framework
| Property | Value |
|----------|-------|
| Frontend E2E | `@playwright/test` 1.61.1 (devDependency), drives the real `web/dist` bundle, mocks `/api/*` via `page.route` |
| Rust contract/unit | existing `cargo test` (workspace); `libtest-mimic` for harness (unchanged) |
| Quick run | `npx playwright test` (frontend) / `cargo test -p envi-store -p envi-service` (backend) |
| Full suite | `cargo test` + `npx playwright test` |

### SC → concrete testable check
| SC | Behavior | Test Type | Concrete check |
|----|----------|-----------|----------------|
| SC1 | Place/edit every kind + last-object inheritance + DGM re-triangulate | Playwright E2E (draw each kind, assert store + PUT body) + Rust unit (`spade` TIN from fixture points/breaklines → expected triangle count / Z sample) | E2E: draw each of 9 kinds, assert feature in store and in the mocked PUT payload; Rust: `envi-dgm` unit test asserts interpolated Z at a known point |
| SC2 | Partial crossing rejected; validation click-to-select+zoom | Playwright E2E (turf.js reject) + unit (turf predicate table) | E2E: draw a partially-crossing ground_zone, assert geometry reverts + message + zoom-to-target on the existing zone; unit: `booleanOverlap`/`booleanContains`/`booleanDisjoint` truth table |
| SC3 | Semi-transparent screen + per-façade spectra; editor 1/12 direct or 1/1·1/3 interpolated with centres on exact indices | Rust contract test (interpolation) + Playwright E2E (editor) | Rust: `interpolate(octave, 9)` → assert grid indices 4,16,…,100 equal inputs bit-for-bit and interior linear; `interpolate(third,27)` → indices 0,4,…,104; E2E: enter octave values, assert live preview matches server |
| SC4 | Scene survives basemap switch, reload, close/reopen | Playwright E2E | Draw scene → toggle basemap (assert features persist via `style.load` rehydrate) → reload page (assert GET returns same) → close/reopen project (assert reopen-last restores) |

### Backend contract tests (new this phase)
- `IsolationSpectrumDto` / forest DTO round-trip + **tested `TryFrom`** into `IsolationSpectrum` / `ForestCrossing` (D-01) — asserts conversion and rejection of out-of-range `R`/negative density.
- `POST /meta/interpolate-spectrum` axum oneshot: valid 9/27/105 → 200 with `[105]`; wrong length → 4xx; non-finite → 4xx; `R > 1000` → 4xx.
- No-drift test for generated `wire.ts` (D-10).
- `cargo tree -p envi-engine` still exactly `ndarray`/`num-complex`/`thiserror` (quarantine gate).

## Security Domain

> `security_enforcement: true`, ASVS L1. New attack surface: the interpolation endpoint, the DGM input, and DOM rendering of user values.

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V5 Input Validation | yes | `deny_unknown_fields` DTOs; length-exact `values` (9/27/105); finiteness + `[0,1000]` `R` clamp; bounded elevation-point count; reject degenerate TIN |
| V5 Injection / XSS | yes | All user values reach the DOM via `textContent` / React children only — never `dangerouslySetInnerHTML`, never `innerHTML` (D-11 icons.ts rule) |
| V4 Access Control | no (localhost single-user, PROJECT.md) | — |
| V2/V3 Auth/Session | no (light/no auth, localhost) | — |
| V6 Cryptography | no | — |

### Known Threat Patterns for this stack
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Hostile `values` array length (e.g. 10⁶ entries) on interpolate endpoint | DoS | Reject any length ∉ {9,27,105} before allocation; axum ~2 MB body cap already in place |
| Non-finite / out-of-range dB reaching the cepstrum | Tampering | Server validates finiteness + `[0, MAX_R_DB=1000]` via `IsolationSpectrum::new`; endpoint mirrors it |
| Huge elevation point/breakline set → TIN blow-up | DoS | Cap point count and breakline count per request; `spade` is O(n log n) but bound it; reject degenerate |
| Interior-intersecting breaklines → `spade` panic → thread abort | DoS | Pre-check `intersects_constraint`/`can_add_constraint`; return 4xx, never panic |
| User-authored strings (project/object names) rendered as markup | XSS (Tampering) | `textContent` / React text nodes only (D-11); the values-reach-DOM-as-text rule is a hard invariant |
| Malformed scene body | Tampering | `deny_unknown_fields` on request DTOs (existing Phase-6 posture) |

## Project Constraints (from CLAUDE.md)

- **English only** for all code, comments, UI strings, docs, commits.
- **`envi-engine` quarantine:** deps stay exactly `ndarray` + `num-complex` + `thiserror`; **serde must never enter it**; `#![deny(unsafe_code)]`. `spade`/`ts-rs` live in other crates. Byte-identical engine this phase.
- **Zero C toolchain this phase:** no `gdal`/`proj` (deferred to Phase 8). `spade` is pure Rust — fine.
- **axum 0.8 brace path syntax** `/{id}`.
- **Compare spectra by band index, never nominal Hz.**
- **Quality gates before "done":** `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `cargo test` all green.
- **Playwright:** devDependency only, never bundled into production; drives the real built bundle; mocks `/api/*` via `page.route`; artifacts (`test-results/`, `playwright-report/`) git-ignored.
- **No GitHub Actions / CI** unless explicitly asked (the no-drift and quality gates run locally via `cargo test`).
- **Five GSD phase-completion gates** run at close (code-review, simplify, secure, verify, doc-consistency).
- **`web/dist` served offline from the binary; zero external assets in `index.html`** (Phase-6 gate) — bears on Gate 4 (see honest offline verdict).
- **metrao3 is READ-ONLY, visual layer only**; do not adopt its vanilla-TS architecture or hand-written types.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Extrapolation of octave/third input outside its spanned band range uses **flat-hold clamp** to the nearest endpoint anchor | Pattern 6 | SCN-03 does not specify extrapolation; a wrong choice changes bands 0–3 / 101–104 of an octave-authored spectrum. **Confirm in discuss/planning** (alternatives: clamp vs linear-extend vs reject-if-unspanned) |
| A2 | `ts-rs` 12 `Uuid → string` needs a specific feature flag (e.g. `uuid-impl`) and `[f64;105] → number[]` (length erased) | Pitfall 5 | Wrong feature name → generation fails; verify exact `ts-rs` 12 Cargo features when wiring D-10 |
| A3 | react-map-gl 8 peerDeps admit React 19 (verified both packages current; explicit 19 test matrix not confirmed) | Standard Stack | Low — if incompatible, pin React 18.3; StrictMode guard is needed either way |
| A4 | The TIN belongs in a **new `envi-dgm` crate** (vs `envi-store`/`envi-service`) | Gate 3 recommendation | Boundary choice; `envi-store` is the fallback. Not correctness-affecting |
| A5 | On vertex insert, keeping the parent UUID on the *first* split half + one fresh UUID (vs two fresh) | Pattern 5 | Cosmetic/stability; either is correct as long as both halves inherit the parent spectrum |
| A6 | Protomaps offline path requires bundling glyphs/sprites + a viewport PMTiles extract into `public/basemap/` | Gate 4 | If a whole-region PMTiles is too large to bundle, Phase 7 accepts OpenFreeMap network fallback (see verdict) |

## Open Questions (RESOLVED)

> All three resolved during phase discussion/planning. Resolution notes added inline; carried into the 10-plan set.

1. **Fully-offline basemap vs network tile fetch (Gate 4).** — **(RESOLVED)** CONTEXT D-13a adopts **OpenFreeMap `styles/dark`** (network vector, no API key); "no API key ≠ no network" stated; a runtime tile XHR is not an `index.html` asset (Phase-6 gate stays green); Playwright `page.route`-intercepts basemap style/tiles/glyphs for offline E2E (07-06/07-10). Protomaps bundled-PMTiles remains the documented air-gapped upgrade path.
   - What we know: Protomaps can be truly offline (bundled PMTiles + glyphs/sprites, CC0). OpenFreeMap needs no key but is a network endpoint. A runtime tile fetch is an XHR by MapLibre JS, **not** an asset referenced in `index.html` — so it does **not** violate Phase-6's "zero external assets in `index.html`" gate, but it **does** break the "works with no network" claim.
   - What's unclear: whether the user wants to bundle a PMTiles extract now (real work: generate the extract for the working area) or accept a network basemap in Phase 7.
   - Recommendation: ship the **Protomaps PMTiles path** with a small bundled extract for the default working area as the offline-true default, and wire OpenFreeMap `styles/dark` as a config-selectable network fallback. Be explicit in the plan that "no API key" ≠ "no network."

2. **WEB-02 source calibration scope.** — **(RESOLVED)** Store `L_W[105]` directly (what `SourceDto`/`SubSourceDto` already carry); the optional SPL-at-reference → `L_W` back-calculation is done **server-side** (SVC-07 — no client acoustic Hz math), wired in 07-08 Task 3. Directivity-balloon import stays deferred (not in WEB-02).
   - What we know: `SourceDto`/`SubSourceDto` carry `sub_sources: [{ position:[x,y,z], spectrum: BandSpectrumDto([105]) }]`. "SPL-at-reference-point calibration" means: author a sound-power (`L_W`) spectrum, or specify an SPL at a reference distance and derive `L_W` back (free-field `L_p = L_W + 20·log10|H|`). Directivity balloon import is explicitly **deferred** (not in WEB-02).
   - What's unclear: whether calibration UI must do the SPL→`L_W` back-calculation client-side (would that be "acoustic math" barred by SVC-07?) or store the authored `L_W` directly.
   - Recommendation: store `L_W[105]` (what the DTO already carries); if SPL-at-reference entry is offered, do the free-field back-calc **server-side** (a trivial meta endpoint or fold into the existing conversion) to honor SVC-07. Confirm in planning.

3. **DGM endpoint shape (D-08, Claude's discretion).** — **(RESOLVED)** A stateless `POST /api/v1/dgm/triangulate` (points + breaklines → triangles), the TIN NOT persisted (the source elevation objects are the persisted truth), built in the new `envi-dgm` crate — 07-02 (crate) + 07-03 (endpoint). The frontend producer (debounced trigger + TIN overlay) lands in 07-07 Task 4; interior-cross rejects surface in the 07-09 validation panel.
   - Recommendation: a stateless `POST /api/v1/dgm/triangulate` taking elevation points + breaklines, returning triangle indices + optional sampled Z grid, computed on demand (no persistence of the TIN itself in Phase 7 — the source elevation objects are the persisted truth). Alternatively compute lazily inside `envi-dgm` with no HTTP surface if the frontend only needs a rendered TIN preview. Decide against `crates/README.md` boundaries during planning.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Node.js + npm | frontend build (`web/`) | ✗ (no JS toolchain in repo yet) | — | **Blocking** — first plan must scaffold `web/` and add npm; document required Node version |
| Rust + cargo | backend (spade/ts-rs) | ✓ (workspace exists) | edition 2024, rust 1.96 | — |
| Network (basemap tiles) | WEB-01 if OpenFreeMap chosen | n/a at build | — | Protomaps bundled PMTiles (offline) |
| C toolchain (gdal/proj) | NOT this phase | n/a | — | Deferred to Phase 8 |

**Missing dependencies with no fallback:** Node/npm — the repo has no JS toolchain; the first Phase-7 plan must introduce `web/package.json` + lockfile and pin a Node version. This is expected (planned) work, not a blocker to planning.

## Sources

### Primary (HIGH confidence)
- npm registry (`npm view … version`) — maplibre-gl 5.24.0, react-map-gl 8.1.1, terra-draw 1.31.2, terra-draw-maplibre-gl-adapter 1.4.1, zustand 5.0.14, @turf/turf 7.3.5, @playwright/test 1.61.1, vite 8.1.4, react 19.2.7
- crates.io API — spade 2.15.1, ts-rs 12.0.1, schemars 1.2.1
- docs.rs/spade/2.15.1 — `ConstrainedDelaunayTriangulation` method signatures + intersection panic behavior + `Barycentric`/`NaturalNeighbor`
- terra-draw `guides/6.EVENTS.md` (main) — change/finish/select/deselect signatures, `context.origin === "api"`, finish action values + release-only timing
- ts-rs wiki "Deriving the TS trait" — `#[ts(tag)]`/serde-compat discriminated unions, variant restrictions, Option/HashMap mapping
- Codebase (this session): `dto.rs`, `geojson.rs`, `jobs.rs`, `meta.rs`, `freq.rs`, `transmission.rs`, `forest.rs`, `api/mod.rs`, metrao3 `theme.css`

### Secondary (MEDIUM confidence)
- Protomaps docs (protomaps.com, docs.protomaps.com/pmtiles/maplibre) — CC0 dark theme, PMTiles offline, glyphs/sprites bundling
- OpenFreeMap (openfreemap.org, github.com/hyperknot/openfreemap) — MIT, no API key, OSM/OpenMapTiles attribution required
- terra-draw issues #197 (setStyle wipe) and #172 (react-map-gl init timing)
- react-map-gl docs (visgl.github.io/react-map-gl) — `useMap()`, maplibre entry

### Tertiary (LOW confidence)
- react-map-gl 8 exact React 19 test-matrix compat (A3) — inferred from both packages being current
- ts-rs 12 exact `Uuid`/array feature-flag names (A2) — verify at wiring time

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — every version registry-verified this session
- Gate 1 (Terra Draw lifecycle): HIGH — events/store API and origin-flag confirmed from official guide
- Gate 2 (ts-rs / JobStatus): HIGH for the union verdict; MEDIUM on `Uuid`/array feature specifics
- Gate 3 (spade TIN): HIGH — method signatures + panic behavior + interpolation confirmed from docs.rs
- Gate 4 (basemap): HIGH on licensing/no-key facts; the offline-vs-network tradeoff is a decision, not an unknown
- Band-index math: HIGH — derived directly from `freq.rs`/`meta.rs` source
- Pitfalls: HIGH

**Research date:** 2026-07-10
**Valid until:** ~2026-08-10 (30 days; frontend packages move fast — re-verify maplibre/react-map-gl/terra-draw versions if planning slips)
