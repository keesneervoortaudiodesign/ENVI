---
status: investigating
trigger: "gis import for all except buildings throws error, test fully aotomatend in-browser playwright with default opening location (amsterdam centre); imported buildings are all the same height; map is dark, must be color; object all heve the same color, we designed alle different colors like in noizcalc; the drawn objects don't yet feed the solver (marshalScene sends a flat homogeneous corridor), so a wall you draw won't change the numbers."
created: 2026-07-13
updated: 2026-07-13
---

# Debug: GIS import + render defects (Amsterdam centre)

## Symptoms (user-reported)

1. GIS import throws an error on every layer **except** buildings.
2. Imported buildings all have the **same height**.
3. Basemap is **dark**; it must be **colour** (user chose OpenFreeMap **Liberty**).
4. Scene objects all render the **same colour**; they were designed with distinct
   per-kind colours (NoizCalc-style).
5. Drawn objects **do not feed the solver** — `marshalScene` emits a flat homogeneous
   corridor (`weather: null`, `forest: null`, `isolation: null`, no screens), so a
   drawn wall cannot change the numbers. (User scope decision: wire **everything,
   including weather** — which additionally requires engine work, see below.)

## Reproduction (automated, in-browser, real network)

`web/tests/e2e/_diag-import.spec.ts` (temporary) drives the **dev** bundle (the prod
bundle strips the `__enviTest` DEV bridge) with `/api/v1` proxied to a real
`envi-service`, at the app's own default view — Amsterdam centre
(`MapCanvas initialViewState = 4.9041, 52.3676, zoom 12`), bbox
`4.894,52.363 → 4.914,52.373`.

Enabling this required a **new dev-only Vite `/api` proxy** (`vite.config.ts`):
without it the dev server has no backend, so `/api/v1/proxy/**` (the byte relay for
the CORS-blocked GLO-30 + WorldCover S3 sources) 404s. That gap is why no offline
test ever exercised the real proxy path.

## Evidence (confirmed, from the live run)

```
terrain    error  { status: 0,   detail: "No terrain tiles cover this viewport." }
landcover  error  { status: 0,   detail: "tiff decode error: unsupported error:
                                  Photometric interpretation RGBPalette with bits
                                  per sample [8] is unsupported" }
buildings  error  { status: 504, detail: "<html> ... Gateway Timeout" }

network:   200  /api/v1/proxy/worldcover/.../ESA_WorldCover_10m_2021_v200_N51E003_Map.tif
           504  https://overpass-api.de/api/interpreter
           (NO request to PDOK/AHN at all)
buildings in store: 1504
```

## Root causes

### RC-1 — terrain: tile planning, not fetching (CONFIRMED)
**No network request is issued at all** for terrain. The failure is upstream of the
fetch, in `envi_gis` source-registry / tile planning: for an Amsterdam viewport the
AHN kaartblad index resolves no tile, and GLO-30 (global coverage) does not take over
as a fallback. Amsterdam is unambiguously inside NL, so either the NL coverage hull
test or the `registry/ahn_index.toml` kaartblad lookup is wrong — and the GLO-30
fallback path is not reached.

### RC-2 — landcover: real WorldCover COG is a PALETTE TIFF (CONFIRMED)
The tile fetches fine (HTTP 200 through the byte proxy) and then **fails to decode**:
the real ESA WorldCover product is `PhotometricInterpretation = RGBPalette` (8-bit,
colour-mapped class raster), which `envi_gis::cog::decode_window_u8` (the `tiff` crate
path) rejects as unsupported.

**Why every offline test passed anyway:** the committed fixture
`tests/e2e/fixtures/worldcover_fixture.tif` is evidently NOT palette-encoded, so the
fixture decodes and the real tile does not. This is the fixture-vs-reality blind spot
the live-smoke spec was built to catch — it just was not covering the COG decode path.

### RC-1b — terrain: the REAL tile is 330 MB and is fetched WHOLE (CONFIRMED)
Once RC-1 was fixed, terrain correctly plans kaartblad `M_25GN1` — and then the import
hangs, because the fetcher issues a **plain whole-tile GET**:
`.../dtm_05m/M_25GN1.tif` → `Content-Length: 346,218,632` (**330 MB**) for a ~1 km²
window. ESA WorldCover is 54 MB for the same reason.
**Both sources support HTTP Range** (verified: PDOK → `206 Partial Content`,
`Content-Range: bytes 0-1023/346218632`; WorldCover → `206` + `Accept-Ranges: bytes`),
and `envi-service`'s byte proxy already relays Range. Cloud-Optimised GeoTIFF exists so a
client can read the header then fetch ONLY the overlapping tiles — ENVI decodes COGs but
never range-reads them. The tiny committed fixtures hid this completely.
**Fix in flight:** two-pass windowed COG read (header → tile byte-range plan → range
fetch → decode), keeping `envi-gis` sans-I/O (TS owns fetch+OPFS).

### RC-4b — building heights: THE DATA IS CORRECT; the UI never shows it (CONFIRMED)
Not an import bug. Measured live in-browser at Amsterdam: the scene store holds **4718
buildings with 244 DISTINCT `eaves_height_m`**, all three fallback tiers firing
(`height_provenance`: height_tag 3680, levels 95, default 943). `envi-gis` was verified
against the real Overpass response (3214 elements → 235 distinct heights, 0 skips).
The real defect: **the building Inspector has no height field at all**, and buildings
render as flat 2-D polygons with no extrusion — so every building looks identical and the
value is invisible/uneditable. Also found: **walls carry no height property whatsoever**,
so a drawn wall could never screen anything (a prerequisite for the solver wiring).
**Fix in flight:** Inspector height + provenance display, editable; wall `height_m`.

### RC-3 — buildings: transient Overpass 504 (NOT the user's bug)
Overpass returned a gateway timeout on this run, yet 1504 buildings still reached the
store — consistent with the user's report that buildings are the layer that *works*.
This is upstream flakiness, not a defect. **Separate real issue:** the buildings that
land all carry the same height (symptom 2) — under investigation.

### RC-4 — object colours: Terra Draw paints OVER the display layers (CONFIRMED, FIXED)
Not a colour-definition bug at all. The `objectStyles` display layers were rendering the
correct per-kind colours all along — Terra Draw was **re-rendering the same store
features on top of them in its single stock blue** (`#3f97e0`):
- `web/src/draw/modes.ts` built TD with stock modes and **no `styles`** → every mode
  defaults to the same hex.
- `web/src/map/useTerraDraw.ts` constructs the MapLibre adapter with **no
  `renderBelowLayerId`** → the `td-*` layers `addLayer()` on TOP of the style.
- `SceneOverlay` is the last child of `<Map>`, so TD's `load` handler runs after the
  display layers' → TD always ends up above.
Points/lines were 100% TD blue (TD's r=6 circle and 4 px line fully cover the 16 px glyph
and 3 px line); polygons got a TD blue outline + tint.
**Fix:** TD now paints only the pixels it OWNS — the shape being drawn and the shape
selected for editing; a committed object is rendered at zero opacity so the display
layers own it. No second palette, no draw-time behaviour change.

**The test was vacuous.** `objectStyling.spec.ts` asserted the style SPEC (layer ids,
registered images, draw order vs the isophone fill) and never asserted a single COLOUR —
so it passed over a visibly-broken app. Its own telemetry even contained the evidence
(`layerOrder` showed the `td-*` layers sitting above the object layers) and the test
simply never looked. Now rewritten to assert the RENDERED result: `queryRenderedFeatures`
+ the live paint expression evaluated against a real feature + **actual canvas pixels** +
a TD paint-ownership check. Verified to FAIL on the pre-fix code.

### RC-5 — basemap dark → Liberty (FIXED)
`basemap.ts` `styles/dark` → `styles/liberty` (same provider, MIT, keyless). Verified by
screenshot at Amsterdam centre. The offline mock regex still matches the new URL.

### RC-6 — solver wiring (NOT a defect — a multi-phase feature programme)
`marshalScene.ts` never reads the scene: it fabricates a source, a synthetic receiver
lattice, a 2-point flat profile (hardcoded σ=200) and nulls for weather/forest/isolation.
The `PrepareSolveReq` DTO **already carries** terrain/weather/forest/isolation and
`PreparedScene` already threads them into the engine (native tests prove the effects are
live). The structural gap is that the wire carries ONE profile per tensor while the
physics needs ONE PROFILE PER PATH. The Phase-9 extractors that do this (`cut_profile`,
`segment_ground`, `inject_screens`, `receiver_grid`, per-azimuth A/B/C) exist, are
wasm-exported, and are **dangling** — reachable only from a debug overlay.
Blockers found: walls carry **no height property**; real terrain will trip
`ConvexSegmentNotImplemented` (§5.12 convex wedge unimplemented); weather+screens
hard-errors (`WeatherScreenNotImplemented` — the shadow-zone branches recur across
Sub-models 4/5/6, ~10 equations, plus the screen path is straight-ray only).
`SegmentedRefractionNotImplemented` confirmed **dead** (never constructed) — safe to delete.
Full staged plan captured; user scope decision = implement everything including weather.

## Current Focus

hypothesis: all six root causes identified; RC-4/RC-5 fixed.
next_action: land the envi-gis fixes (RC-1/RC-2/building heights), then run the solver
             wiring (RC-6) as planned GSD phases — it is a feature programme, not a fix.
