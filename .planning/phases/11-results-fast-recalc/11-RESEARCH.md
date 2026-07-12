# Phase 11: Results & Fast Recalc - Research

**Researched:** 2026-07-12
**Domain:** Client-side WASM result readout (tensor MAC + A/C weighting), pure-Rust isophone contouring, MapLibre result overlays + object styling, weather scenarios, export byte generation, app-wide help system
**Confidence:** HIGH (integration surfaces read directly from source); MEDIUM on the two net-new algorithms (isoband tracer, A/C weighting) — both have exact reference formulas but need anchor tests

## Summary

Phase 11 is a **rendering + readout** phase, not a physics phase. Every acoustic value already exists in the Phase-10 OPFS tensor (`H_coh` + `P_incoh_abs`, `[s,r,105]`, frozen byte layout). The engine's readout laws (`readout_coherent` MAC, `readout_incoherent` energy sum, `compose_gain`) are FORCE-validated and byte-exact — Phase 11 does **not** touch them, it **drives** them from `envi-compute` / `envi-compute-wasm` over the cached tensor (D-01: all arithmetic in Rust→WASM, never JS/TS).

The work splits into six net-new capabilities, each with one clear owner tier: (1) a WASM **OPFS tensor reader** (the store is currently write-only) feeding the readout laws → receiver spectra; (2) net-new **A/C weighting tables** at the 105 exact grid centres (analytic IEC 61672-1); (3) a pure-Rust **marching-squares isoband tracer** over the fine-tier level grid → fill polygons; (4) the **conditioning MAC path** wired to the reused `SpectrumEditor` with a debounced live recalc, a blake3 **stale badge**, and a client-side realization of the **409 hash-mismatch** rejection; (5) **weather scenarios** as per-scenario hash-keyed OPFS tensors + a diverging difference map; (6) three **export encoders** (GeoTIFF/GeoJSON/CSV) in WASM. Plus two cross-cutting UI sweeps: the **NoizCalc object color+hatch styling** system and the **app-wide info-button help catalog** (D-25 flags the latter as its own plan/wave).

**Primary recommendation:** Hand-roll the isoband tracer (reuse `landcover.rs` boundary-tracing DNA, adapted for interpolated iso-bands) and the CSV/minimal-GeoTIFF encoders — the two contour crates are SUS and gdal is not in the WASM path. Add exactly one OK-verdict crate if a full GeoTIFF is wanted (`tiff` / image-tiff). Keep the spectrum chart as hand-rolled inline SVG (matches `SpectrumEditor`, zero new JS dep), styled per the dataviz method. Isolate the info-button sweep into its own wave.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01 (foundational):** ALL Phase-11 acoustic arithmetic runs in **Rust→WASM, client-side, over the OPFS tensor** — MAC readout, dB(A)/dB(C) weighting, band aggregation, coherent/incoherent split, contouring, export byte generation. The requirement language "server-side" is reinterpreted as **"in Rust/WASM, never in JS/TS."** JS/TS does zero acoustic math. The recondition/recompute split + blake3 hash gate stay the identity contract; the MAC executes in WASM.
- **D-02:** Contour **pure-Rust in WASM** — `contour` crate OR hand-rolled marching-squares (like `envi-gis/landcover.rs`). `gdal-sys GDALContourGenerateEx` is a **documented escape hatch only** if the 100k+-cell benchmark fails. Isophones render as **fill polygons**, never a heatmap layer.
- **D-03:** Default scale = **EU END-style fixed dB bands** (~5 dB steps) with the canonical noise-map palette.
- **D-04:** Editing = **presets + editable breaks** (END-standard + perceptually-uniform e.g. viridis/turbo). **Legend breaks ≡ contour breaks ≡ class colors enforced.** Editing the scale **re-contours the cached level grid without re-running propagation**. dB weighting label from result metadata.
- **D-05:** Chart primary + expandable exact-numbers table.
- **D-06:** 1/3-oct display default, toggle to full 105-point 1/12-oct expert view (aggregated by **band index**, never nominal Hz).
- **D-07:** Receiver selection via map click AND a synced receiver list (discrete receiver points primary).
- **D-08:** Coherent/incoherent split in the totals **always** + an on-demand per-band overlay of the two as separate series.
- **D-09:** A/C weighting is **net-new code** — A- and C-weighting tables **precomputed at the exact 105 1/12-octave grid centres** (WASM-side) so the dB(A)⇄dB(C) toggle is instant with no recompute.
- **D-10:** Live, **debounced MAC recalc (~100–200 ms)** on any gain/filter/delay change.
- **D-11:** Filter control **reuses the Phase-7 `SpectrumEditor`** (`web/src/spectrum/SpectrumEditor.tsx`): 1/1, 1/3, or 1/12-oct entry interpolated onto the 105-point grid → complex per-band gain `G_s(f)` for `compose_gain`.
- **D-12:** Stale badge = recompute blake3 tensor identity on scene/terrain/ground/met edit vs cached hash. **Conditioning edits never stale.** A MAC request against a mismatched hash is **rejected via the SVC-06 409 gate, never silently served**.
- **D-13:** **Clone-then-edit scenarios**, each with its own hash-keyed cached tensor; named scenarios switch instantly.
- **D-14:** Friendly inputs (T/RH/p, Beaufort wind class + direction, temp gradient, downwind worst-case toggle) drive the **Phase-9 `envi_gis::weather`** derivation → per-azimuth A/B/C; raw per-azimuth A/B/C exposed as advanced override.
- **D-15:** Downwind worst-case = **per-azimuth favourable** (downward-refraction along each source→receiver bearing independently).
- **D-16:** Difference map = **A vs B, per-receiver dB(A) delta, diverging** color scale (blue–white–red centered on 0).
- **D-17:** Full scene-object palette redesign with differentiating **color AND hatch pattern (arcering)** per object kind, following NoizCalc TI 386 §4.6.3: point = symbol/size/border; line = width/color; area = fill color + border + **separate hatch pattern**. Semi-transparent hatched fills read over the isophone fill.
- **D-18:** Objects render at **full styling on top of** the isophone fill; palette must keep noise classes + objects legible together (contrast-checked).
- **D-19:** **Validated, accessible color foundation** via the dataviz skill: perceptually-uniform **sequential** for isophones, **diverging** for difference maps, **colorblind-safe**, **light/dark theme-aware** (extend metrao3 tokens). ⚠ This restyles Phase-7 object map layers — coordinate, do not regress draw-time behavior.
- **D-20:** Export = **client-side browser download**; WASM produces bytes from the OPFS tensor/grid; nothing leaves the device.
- **D-21:** **Raster + vector + spectra**: GeoTIFF = continuous dB(A)/dB(C) level grid; GeoJSON = isophone fill polygons; CSV = spectra with **band index AND exact Hz** columns.
- **D-22:** Full metadata + attribution on every export: CRS + dB weighting label + engine/scene identity + OSM/Overture/ESA WorldCover/Copernicus attribution.
- **D-23:** **"i" info button on EVERY interactive control** via a reusable info-affordance component; app-wide (new Phase-11 controls + retrofit of all Phase-7/8/9/10 panels).
- **D-24:** Help content extensive + standards-cited (Nord2000 / NoizCalc rationale). Cite **AV 1106/07 by report number** and **TI 386**. ⚠ AV 1106/07 is copyrighted — explain in our own words, **NEVER paste the standard's text into UI help**. English-only.
- **D-25:** Help content lives as **structured data** (keyed help catalog, not JSX text) + a **coverage check** asserting every interactive control has a help entry. Large sweep — **planner should isolate into its own plan/wave**.

### Claude's Discretion

Chart library (dataviz-guided); exact debounce interval; contour break-value interpolation details; the specific hatch-pattern + symbol set per object kind; OPFS per-scenario tensor directory layout + naming; how the recondition MAC reuses vs re-streams tier spans; the exact diverging-scale midpoint/clamp for difference maps; GeoTIFF encoder choice (pure-Rust) within D-20; the info-button presentation (popover vs docked side-panel vs both) and the help-catalog file format/location within D-23–25.

### Deferred Ideas (OUT OF SCOPE)

- Real authentication / login gate (its own phase, Phase-10 D-12).
- PROJECT.md / ARCHITECTURE.md deployment-model amendment pass.
- GRID-03 L_den weather-class combination (deferred beyond Milestone 2).
- OPFS quota / eviction strategy (revisit only if it bites).
- Wholesale Phase-7 editor re-theme beyond object color/hatch.
- Parametric-EQ conditioning UI (the reused 105-point spectrum editor ships instead).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SVC-06 | `recondition` (MAC) vs `recompute` (propagation) split; tensor cache keyed by content hash; mismatched-hash MAC rejected | Server contract exists in `envi-service/src/api/calc.rs` (409 gate); WASM realization via `envi-compute-wasm` `tensor_hash` + `HashMismatch` (§Recondition MAC path). Design decision: client-side vs server-side 409 (Open Q1) |
| WEB-05 | Per-source gain/filter/delay conditioning + interactive fast recalc + stale badge | `compose_gain` + `readout_coherent` (byte-exact MAC); `ConditioningDto{gain_db,delay_ms,filter_band_db[105]}` already in `envi-store/dto.rs`; reuse `SpectrumEditor` (§Conditioning) |
| WEB-06 | Isophone fill polygons + editable color scale + legend (weighting from metadata) | Marching-squares isoband tracer (§Isophone contouring); dataviz color-scale system (§Color system); MapLibre fill layer |
| WEB-11 | Receiver spectrum: 1/12-oct expert + 1/3-oct display (band index), dB(A)/dB(C) totals + instant toggle, coherent/incoherent split, zero client acoustic math | OPFS tensor reader + `band_levels_db_two_channel` + A/C tables (§Spectrum readout, §A/C weighting) |
| WEB-12 | Weather what-if panel: import, manual override, named-scenario management, difference map | Per-scenario hash-keyed tensors + Phase-9 `envi_gis::weather` + diverging difference map (§Weather scenarios) |
| METX-03 | Manual met override (T/RH/p, Beaufort wind, downwind worst-case, temp gradient, per-azimuth A/B/C) | D-14/D-15 drive Phase-9 derivation; friendly + advanced inputs (§Weather scenarios) |
| METX-04 | Named weather scenarios + per-scenario cached tensors + instant switching + difference maps | Scenario = met-override → new `tensor_hash` → new OPFS calc dir (§Weather scenarios) |
| GRID-04 | Contour results into isophone fill polygons (pure-Rust; gdal escape hatch) | Hand-rolled isoband tracer recommended; `contour`/`contour-isobands` SUS (§Isophone contouring, §Package Legitimacy) |
| GRID-05 | Export GeoTIFF/GeoJSON + spectra CSV | WASM byte encoders; `tiff` (OK) or hand-rolled minimal GeoTIFF; hand-rolled CSV; `geojson` reuse (§Export) |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Tensor MAC readout / A-C weighting / band aggregation | `envi-engine` (frozen laws) driven by `envi-compute` | `envi-compute-wasm` boundary | D-01: acoustic math in Rust/WASM. Engine laws untouched (byte-identical quarantine); the composition/weighting/aggregation orchestration lives in `envi-compute` |
| OPFS tensor **read-back** | `envi-compute-wasm` + `web/src/compute` (JS OPFS glue) | — | The OPFS store is currently write-only; reading chunk files back is net-new plumbing at the same boundary as `opfs_sink.rs` |
| Isophone iso-band tracing | `envi-compute` (pure-Rust) | `envi-compute-wasm` export | Pure geometry over a level grid; belongs with the compute core, mirrors `envi-gis/landcover.rs` |
| Level-grid → fill-polygon reproject | `envi-compute` (SceneXY→lattice) + `envi-geo` (→LonLat) | `web/src/map` (render) | Reproject through the single CRS boundary (`envi-geo`); render as a MapLibre fill layer |
| Conditioning UI + debounce + stale badge | `web/src/panels` + `web/src/store` | `envi-compute-wasm` (MAC + hash) | UI state + debounce is a frontend concern; the MAC + identity are WASM |
| Weather scenario management | `web/src/store` (scenario registry) + OPFS layout | `envi-gis::weather` (Phase-9 A/B/C) + full solve path (Phase-10) | Each scenario is a met-override producing a new tensor via the existing solve; the UI manages named references |
| Export byte generation | `envi-compute-wasm` (encoders) | `web` (browser download) | D-20: WASM produces bytes, browser downloads; nothing leaves the device |
| Object color + hatch styling | `web/src/map` (MapLibre layers) | `web/src/theme` (tokens) | Display-time styling of scene-object layers; hatch via runtime-generated `addImage` patterns |
| Info-button help catalog | `web/src/help` (data + component) | all panels (retrofit) | Structured data + a reusable component + a coverage test; an app-wide cross-cutting sweep |

## Standard Stack

### Core (existing — verify, do not break)

| Component | Location | Purpose | Contract |
|-----------|----------|---------|----------|
| `readout_coherent(h_coh, g)` | `envi-engine/src/tensor.rs` | OUT-03 MAC `p[r,f]=Σ_s H_coh[s,r,f]·g_s(f)` | Bit-exact vs full recompute; sub-source-outermost accumulation is FROZEN |
| `readout_incoherent(h_coh, p_incoh, w)` | `envi-engine/src/tensor.rs` | Annex-A energy `e[r,f]=Σ_s w_s(f)·(\|H_coh\|²+P_incoh)` | Road/FORCE sources; two co-located = +3 dB not +6 |
| `compose_gain(l_w_db, filter, delay_s, axis)` | `envi-engine/src/tensor.rs` | `G_s(f)=10^{L_W/20}·Ĝ_filter·e^{−j2πfτ}` | Frozen composition order = bit-exact MAC. Filter must be dense `[105]` |
| `band_levels_db_two_channel(...)` | `envi-engine/src/transfer.rs` | Two-channel dB readout (coherent + incoherent) | The WEB-11 D-08 split reads out of this law |
| `ConditioningDto{gain_db, delay_ms, filter_band_db:Option<[105]>}` | `envi-store/src/dto.rs:304` | Per-source readout param (gain/delay/filter/mute) | Already the exact wire shape WEB-05 needs; excluded from tensor identity (D-07) |
| `CalcManifest{dims:[S,R,105], chunk_receivers, tensor_hash, ...}` | `envi-compute/src/identity.rs:351` | Tensor calc manifest | The reader keys off `dims` + `chunk_receivers` + hash |
| OPFS chunk byte format | `envi-compute-wasm/src/opfs_sink.rs` | `H_coh` interleaved (re,im) f64-LE 16 B/cell; `P_incoh` f64-LE 8 B/cell; `[s][r_local][f]` freq-contiguous | Frozen — the reader must decode this exact layout |
| `SpectrumEditor.tsx` | `web/src/spectrum/` | 1/1·1/3·1/12 filter entry → 105-pt interpolation | Reused verbatim as the conditioning filter (D-11) |
| `envi_gis::weather` per-azimuth A/B/C | `envi-gis/src/weather.rs` | Friendly met → per-azimuth A/B/C (Phase-9) | Drives weather overrides (D-14) |
| `vectorize_landcover` marching-squares | `envi-gis/src/landcover.rs` | Boundary-tracing DNA to adapt for iso-bands | Pattern reference (D-02) |

### Supporting (net-new code, no/minimal new deps)

| Component | Owner crate | Approach | New dep? |
|-----------|-------------|----------|----------|
| OPFS tensor **reader** | `envi-compute-wasm` + `web/src/compute/opfs.ts` | Decode frozen chunk bytes → `Array3` in worker | No |
| A/C weighting tables | `envi-compute` | Analytic IEC 61672-1 at the 105 exact centres | No |
| Iso-band tracer | `envi-compute` | Hand-rolled interpolated marching squares | No (recommended) |
| CSV spectra encoder | `envi-compute-wasm` | Hand-rolled `String` builder | No |
| GeoJSON polygon encoder | `envi-compute-wasm` | `geojson` (already in-tree in `envi-gis`) or `serde_json` | Reuse |
| GeoTIFF raster encoder | `envi-compute-wasm` | `tiff` (image-tiff, OK) + hand-written geo-keys, OR hand-rolled minimal Float32 GeoTIFF | 1 OK crate OR none |
| Spectrum chart | `web` | Inline SVG (matches `SpectrumEditor`), dataviz-styled | No (recommended) |
| Hatch fill patterns | `web/src/map` | Runtime canvas → `map.addImage` → `fill-pattern` | No |
| Info-button + help catalog | `web/src/help` | Typed keyed catalog + `<InfoButton>` + coverage test | No |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Hand-rolled iso-band tracer | `contour` / `contour-isobands` crate | Both SUS (797/16 weekly dl); `contour` already declined in Phase 8. They ARE the reference algorithm (d3-contour port, MIT) — port ideas, not the dep |
| Server-side gdal contour | `gdal-sys GDALContourGenerateEx` | Escape hatch only (D-02); gdal is NOT in the WASM path (server-only in `envi-gis`) — using it would move contouring server-side, contradicting D-01. Reserve strictly for a failed 100k-cell benchmark |
| `tiff` crate GeoTIFF | Hand-rolled minimal single-strip Float32 GeoTIFF (~200 LOC) | Zero-dep, matches project ethos; but you own the GeoKeyDirectory encoding. `tiff` is mature (1.77M dl/wk, OK) but must be verified to build for `wasm32-unknown-unknown` |
| Inline-SVG chart | uPlot / visx / Recharts | A JS chart lib is 30–500 KB and adds a dep; the panel is one chart. `SpectrumEditor` already proves inline SVG works here |

**Installation (only if a full GeoTIFF encoder is chosen):**
```bash
cargo add --package envi-compute-wasm tiff   # image-rs/image-tiff — verify wasm32 build first
```

**Version verification (run at plan time):**
```bash
cargo search tiff        # image-tiff — confirm current 0.x and wasm32 support
cargo search geojson     # already resolved in-tree via envi-gis
```

## Package Legitimacy Audit

> External-package additions target `envi-compute` / `envi-compute-wasm` (WASM), never `envi-engine` (frozen 3-dep quarantine).

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| `contour` | crates | 2019 (6 yr) | 797/wk | github.com/mthh/contour-rs | **SUS** (low-downloads) | Not used — hand-roll (already declined Phase 8). Port ideas only |
| `contour-isobands` | crates | 2023 | 16/wk | github.com/mthh/contour-isobands-rs | **SUS** (low-downloads) | Not used — algorithm reference only |
| `tiff` (image-tiff) | crates | 2018 (7 yr) | 1,773,736/wk | github.com/image-rs/image-tiff | **OK** | Approved IF a full GeoTIFF encoder is chosen; gate on a wasm32-build verify |
| `geotiff-writer` | crates | 2026-03 (new) | 158/wk | github.com/roteiro-gis/geotiff-rust | **SUS** (new + low-downloads) | REMOVED — do not use |
| `tiff-writer` | crates | 2026-03 (new) | 204/wk | (roteiro-gis) | **SUS** (new + low-downloads) | REMOVED — do not use |
| `csv` | crates | 2014 (11 yr) | 2,992,290/wk | BurntSushi/rust-csv | **OK** | Not needed — hand-roll CSV (trivial); listed as a safe fallback if wanted |
| `geojson` | crates | in-tree | — | georust/geojson | **OK** (already vetted, used by `envi-gis/landcover.rs`) | Reuse for the polygon export |

**Packages removed due to [SLOP]/[SUS] verdict:** `geotiff-writer`, `tiff-writer` (both brand-new, low-download; a hand-rolled minimal GeoTIFF or the mature `tiff` crate covers the need).
**Packages flagged as suspicious [SUS]:** `contour`, `contour-isobands` — used as **algorithm references only**, not added as deps. If the planner ever chooses to add `contour`, gate it behind a `checkpoint:human-verify` (it is declinable — the hand-rolled path is the recommendation).
**Net dependency recommendation:** **zero new crates** if GeoTIFF is hand-rolled; **one OK crate (`tiff`)** if a full-feature GeoTIFF is preferred — gated behind a wasm32-build verify checkpoint.

## Architecture Patterns

### System Architecture Diagram

```
                 Phase-10 OUTPUT (already on disk)
   OPFS: projects/<id>/calc/<tensor_hash>/{tensor,pincoh}/chunk_NNNNN.bin
   manifest: dims[S,R,105] · chunk_receivers · tensor_hash
                              │
          ┌───────────────────┴────────────────────┐
          │  NEW: OPFS tensor READER (worker + WASM) │  decode frozen bytes → Array3
          └───────────────────┬────────────────────┘
                              │  H_coh[s,r,f] + P_incoh[s,r,f]
        ┌─────────────────────┼───────────────────────────────┐
        ▼                     ▼                               ▼
  RECEIVER READOUT       ISOPHONE GRID                  CONDITIONING MAC
  (discrete r)           (fine-tier r → lattice)        (SpectrumEditor →
  band_levels_db_        readout → dB(A)/dB(C) per       ConditioningDto →
  two_channel +          node → CACHED level grid        compose_gain →
  A/C tables             │                               readout_coherent)
        │                ▼                                     │
        │          marching-squares                     debounced ~150 ms
        │          iso-band tracer                       live update
        │          (SceneXY polygons)                          │
        │                │                              stale badge = re-mint
        ▼                ▼ envi-geo → LonLat            blake3 hash vs manifest;
   SPECTRUM PANEL   MapLibre FILL layer  ◄── objects    mismatch → refuse (409
   (inline SVG +    (below full-styled   at full         semantics, client-side)
   table, dB(A)/    scene objects, D-18)  styling
   dB(C) toggle)         │                + hatch
        │                │                (addImage)
        └────────┬───────┴──────────┬──────────────────┐
                 ▼                   ▼                  ▼
          COLOR-SCALE EDITOR   DIFFERENCE MAP     EXPORT (WASM bytes)
          (breaks ≡ contour    (scenario A − B,   GeoTIFF (grid) /
          ≡ class colors;      diverging scale)   GeoJSON (polys) /
          re-contour cached                       CSV (spectra) →
          grid, NO re-solve)                      browser download
```

### Recommended Structure (net-new)

```
crates/envi-compute/src/
├── weighting.rs      # A/C weighting tables at the 105 centres (D-09)
├── readout.rs        # orchestrate tensor readout → dB(A)/dB(C) per receiver
├── isoband.rs        # hand-rolled interpolated marching-squares iso-bands (D-02)
└── export/           # geotiff.rs · geojson.rs · csv.rs byte encoders (D-21)
crates/envi-compute-wasm/src/
├── opfs_reader.rs    # decode frozen chunk bytes → Array3 (mirror of opfs_sink.rs)
├── recondition.rs    # boundary: read → compose_gain → readout_coherent → dB
└── dto.rs (+)        # ReconditionReq/Result, IsophoneReq/Result, ExportReq DTOs (ts-rs)
web/src/
├── compute/opfs.ts   # add readChunk/preopen-read glue (worker-side)
├── panels/SpectrumPanel.tsx · ColorScaleEditor.tsx · ScenarioPanel.tsx
├── map/isophoneLayer.ts · hatchPatterns.ts · objectStyles.ts (restyle Phase-7 layers)
├── store/results.ts · scenarios.ts · conditioning.ts
└── help/catalog.ts · InfoButton.tsx · coverage.test.ts   # own wave (D-25)
```

### Pattern 1: OPFS tensor read-back (net-new — the store is write-only today)

Phase 10 only WRITES chunks (`OpfsChunkSink`). Phase 11 must add a symmetric READER. The byte format is frozen and documented in `opfs_sink.rs`:
- `tensor/chunk_NNNNN.bin`: per cell `re: f64 LE` then `im: f64 LE` (16 B), in `[s][r_local][f]` logical order (freq fastest).
- `pincoh/chunk_NNNNN.bin`: per cell `f64 LE` (8 B), same order.
- `manifest`: `dims=[S,R,105]`, `chunk_receivers` (receivers per chunk), `tensor_hash`.

```rust
// Source pattern: inverse of envi-compute-wasm/src/opfs_sink.rs put_chunk (tests read_chunk)
// Decode a chunk's bytes back into [n_sub, len, N_BANDS] arrays in [s][r_local][f] order.
// Read hi in 16-B strides (re,im); pi in 8-B strides. Assemble receiver spans by chunk_index.
```
The JS worker pre-opens read handles the same way it pre-opens write handles (async `createSyncAccessHandle` hoist, worker-only — Pitfall 4). For a discrete-receiver readout you only need the chunk(s) covering that receiver's `global_index`; for the isophone grid you stream all fine-tier chunks.

### Pattern 2: The two readout laws — pick by source type (D-08)

Every receiver total = `|coherent Σ|² + P_incoh` (the two-channel contract). The tensor stores BOTH channels, so Phase 11 reads out both:
- **Coherent** part: `readout_coherent(h_coh, g)` with `g` from `compose_gain` (conditioned/loudspeaker sources) → magnitude² is the coherent energy.
- **Incoherent** part: sum `P_incoh_abs` over sub-sources with the same weights.
- **Total** dB = `10·log10(coherent_energy + incoherent_energy)`.
`band_levels_db_two_channel` (`transfer.rs:115`) already encodes this law — reuse it, do not re-derive. The D-08 split UI shows total always, and the coherent-vs-incoherent per-band overlay is the two energy channels rendered as separate series.

### Pattern 3: A/C weighting toggle with no recompute (D-09)

Precompute `A[105]` and `C[105]` **once** from the analytic formula at `FreqAxis::centres`. The dB(A) total is `10·log10(Σ_i 10^((L_i + A_i)/10))`. Toggling weighting just re-applies a different precomputed `[105]` table to the already-read band levels — no tensor read, no MAC. Compare/aggregate strictly by **band index**.

### Pattern 4: Re-contour without re-solve (SC3, D-04)

Cache the **level grid** (per-fine-tier-node dB(A) and dB(C) values) after the first readout. Editing color-scale breaks re-runs ONLY the iso-band tracer over that cached grid → new fill polygons. No tensor read, no MAC, no propagation. Only a weighting toggle re-applies the other precomputed weighting table to the cached grid (still no solve).

### Pattern 5: Stale badge + 409 as identity, not as a fresh compute (D-12)

`tensor_hash` is a blake3 digest over every tensor-affecting field (terrain, atmosphere, coherence, weather, sub-sources+directivity, receivers, forest, isolation, n_sub) — conditioning is **excluded** (`envi-compute-wasm/src/identity.rs`, `envi-store/src/hash.rs`). On any scene/terrain/ground/met edit, re-mint the hash and compare to the cached tensor's manifest hash: differ → **stale badge**. A conditioning edit changes nothing in the hash → never stales. A MAC request whose hash ≠ the tensor's hash is **refused** — the client-side realization of the server 409 (`calc.rs` already returns `{error:"tensor_hash_mismatch", expected, got, hint}`; the WASM path returns `ComputeError::HashMismatch`).

### Pattern 6: Scenario = met-override → new hash-keyed tensor (D-13/METX-04)

A named scenario clones the current met, applies overrides, and runs the **full Phase-10 solve path** (met changes tensor identity → a `recompute`, not a MAC). Its tensor lands in its own OPFS dir `calc/<scenario_tensor_hash>/`. The scenario registry stores `{name, met_overrides, tensor_hash}`. Switching = load that tensor + readout (instant, no re-solve). Difference map = readout A and B, subtract per-receiver dB(A) totals, contour the delta on the diverging scale (D-16).

### Anti-Patterns to Avoid

- **Any acoustic arithmetic in JS/TS** — violates D-01/SVC-07. Even a dB sum or a weighting must be WASM. JS only renders WASM-produced numbers.
- **Comparing/aggregating by nominal Hz** — always band index (`f == 31.5` is a bug; `freq.rs` Pitfall).
- **A heatmap raster layer for isophones** — D-02 mandates fill polygons.
- **Re-running propagation on a color-scale edit** — SC3 forbids it; re-contour the cached grid.
- **Editing `envi-engine`** — the 3-dep quarantine is byte-identical/frozen. New WASM math is a NEW impl of existing traits in `envi-compute*`, never an engine edit (the OPFS sink pattern).
- **Hand-writing a TS mirror of a Rust DTO** — all wire types are ts-rs generated with a no-drift test (project hard rule).
- **Pasting AV 1106/07 text into help** — copyrighted; explain in our own words + cite by report number (D-24).
- **`worker.terminate()` to cancel** — cooperative cancel flag at chunk boundaries (existing pattern).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| The MAC / weighted readout | A fresh dB summation | `readout_coherent`/`readout_incoherent`/`band_levels_db_two_channel` | FORCE-validated, byte-exact vs recompute; re-deriving risks last-ulp drift and the +3-vs-+6 dB phantom-interference bug |
| Gain composition | Ad-hoc filter×gain×delay | `compose_gain` | Frozen composition order is what makes MAC ≡ recompute bit-for-bit |
| Tensor identity hash | A new client hash | `envi-compute-wasm::tensor_hash` / `envi-store::hash::tensor_hash` | One identity oracle for stale badge + 409; a second hash would diverge |
| CRS reprojection (SceneXY→LonLat) | Inline proj strings | `envi-geo` (`LonLat`/`SceneXY`) | The single reprojection boundary (GEOX-04); a second boundary is a documented anti-pattern |
| Per-azimuth A/B/C from friendly met | New met math | `envi_gis::weather` (Phase-9) | Already derives the Nord2000 profile; D-14 explicitly reuses it |
| Polygon boundary tracing | From scratch | Adapt `landcover.rs` DNA | The exterior/hole classification, saddle resolution, DP-simplify, and non-crossing guarantees are already solved |
| GeoJSON serialization | Manual JSON strings | `geojson` (in-tree) | RFC-7946 winding + geometry types handled |

**Key insight:** The physics is done. The single biggest risk in this phase is **re-implementing a readout law slightly differently in the WASM orchestration layer** and silently diverging from the FORCE-validated engine. Always call the engine's laws; never paraphrase them.

## A/C Weighting — the net-new formula (D-09)

Analytic IEC 61672-1:2013 frequency-weighting transfer functions, evaluated at each exact grid centre `f = FreqAxis::centres[i]` (`1000·G^((i−64)/12)`):

```
R_A(f) = 12194² · f⁴
       / [ (f² + 20.6²) · √((f² + 107.7²)(f² + 737.9²)) · (f² + 12194²) ]
A(f) = 20·log10(R_A(f)) + 2.00    dB      # +2.00 normalizes A(1000 Hz) = 0

R_C(f) = 12194² · f²
       / [ (f² + 20.6²)(f² + 12194²) ]
C(f) = 20·log10(R_C(f)) + 0.06    dB      # +0.06 normalizes C(1000 Hz) = 0
```
`[CITED: en.wikipedia.org/wiki/A-weighting, IEC 61672-1:2013 §5.4 + Table 3]` — the constants 12194, 20.6, 107.7, 737.9 are the standard analog pole frequencies.

**Anchor test (MEDIUM→HIGH):** assert `A(1000)=0` and `C(1000)=0` exactly at band index 64; compare the 27 third-octave grid points (indices 0,4,…,104) against IEC 61672-1 Table 3 tabulated values within the standard's Class-1 design tolerance. Known checkpoints: A(100)≈−19.1 dB, A(10 000)≈−2.5 dB, C(100)≈−0.3 dB, C(10 000)≈−4.4 dB. Because these tables are compliance-adjacent, the anchor test IS the acceptance gate — do not ship the tables without it.

## Isophone Contouring — hand-rolled iso-bands (D-02, GRID-04)

**Recommendation: hand-roll an interpolated marching-squares iso-band tracer** in `envi-compute`, reusing `landcover.rs`'s ring-classification/DP-simplify/non-crossing machinery. The two `contour*` crates are SUS and were already declined in Phase 8; gdal is not in the WASM path.

**Crucial difference from `landcover.rs`:** that module traces boundaries of a **binary pixel partition** (per-class, pixel-corner loops, **no interpolation**). Isophones are **thresholded iso-bands over a continuous scalar field** — you must **linearly interpolate the crossing point** on each cell edge where the level crosses a break value, so contours are smooth, and produce **nested band polygons** between consecutive breaks (band `[b_k, b_{k+1})`). The reference algorithm is d3-contour (ported by `contour-rs`, MIT) — port the marching-squares case table + linear interpolation, keep `landcover.rs`'s exterior/hole assignment and `geo::Simplify`.

**Grid reconstruction:** the fine-tier receivers are a lattice from `envi_compute::tiers::partition` — each `TierReceiver` carries `global_index` + `position`. Arrange the fine-tier dB values into a 2D `[rows × cols]` array by lattice index, trace, get SceneXY polygons, reproject each vertex through `envi-geo` to LonLat for the MapLibre fill layer.

**Performance (100k+ cells, roadmap flag):** marching squares is O(cells × breaks) — a single pass per break over a ~316×316 grid (100k cells) × ~10 breaks ≈ 1M cell visits, trivially sub-100 ms in WASM. The escape hatch (`gdal-sys GDALContourGenerateEx`) is reserved strictly for a measured benchmark failure and would be **server-side** (a D-01 tension — note it). `[VERIFIED: crates.io — contour(SUS,797/wk), contour-isobands(SUS,16/wk)]`

## Color System (D-03/D-04/D-17/D-18/D-19) — dataviz-guided

The `dataviz` skill (loaded) supplies the accessible-palette method. Key parameters for this phase:

- **Isophones = sequential (magnitude).** dataviz default sequential hue is blue light→dark (`references/palette.md`), but D-03 wants the **EU END-standard noise-map palette** as the default (green→yellow→orange→red→violet, ~5 dB bands) because acousticians expect it. That END scale is technically a rainbow and NOT perceptually uniform — the resolution: ship END-standard as the **familiar default** AND offer **perceptually-uniform presets (viridis/turbo)** per D-04, and make color never load-bearing alone (legend + numeric break labels are the secondary encoding).
- **Difference map = diverging (polarity).** dataviz diverging pair = **blue ↔ red with a neutral gray midpoint** (exactly D-16), equal steps per arm, gray at 0 (never a hue at the midpoint). Clamp symmetric about 0.
- **Object styling = categorical (identity), fixed order, never cycled** — the 9 scene-object kinds map to the dataviz categorical slots (blue/aqua/yellow/green/violet/red/magenta/orange…). Objects sit ON TOP of the isophone fill (D-18), so **hatch patterns are the load-bearing legibility channel** (dataviz "texture fill" = the accessibility channel; 45°/135° line hatches), letting a semi-transparent hatched building read over a colored noise band.
- **Validate, don't eyeball.** Run `scripts/validate_palette.js` (in the skill base dir) on the chosen isophone ramp AND the diverging pair AND the 9-kind categorical set, against the **metrao3 dark surface** (`--surface #14171c --mode dark`) and a light surface — CVD ≥ 12, contrast, lightness band. A contrast WARN obligates visible labels or the table view.
- **Light/dark theme-aware:** extend the metrao3 tokens in `web/src/theme.css` (`--color-*`); provide dark steps validated against the dark surface, not an auto-flip.

## NoizCalc Object Styling (D-17) + Color Scale (D-04) — TI 386 model

`[CITED: docs/references/dbaudio-ti386-1.6-en.md §4.6.3, §4.6.5, §4.6.6]`

- **§4.6.3 object-type formatting:** point = symbol + size (mm plan-size or m world) + line width + border; line = width + color; **area = fill color + border + hatch pattern set separately** ("Fit to first line" aligns the pattern to the first two coords). Draw sequence: higher = drawn later (may hide lower). This is the exact D-17 model.
- **§4.6.5 grid noise map:** fill contours between contour lines per scale colors; editable color scale determined from min/max on load, then **smallest interval value + interval magnitude (dB) + number of intervals + Ascending + Keep-color-sequence**; interval sizes need not be constant (editable upper limits). This is the D-04 color-scale editor spec — mirror these controls. The scale unit follows the dB weighting (`<su:>` variable) = the D-04 "weighting label from metadata".
- **§4.6.6 palette:** 16 RGB scale colors, interpolate gradients between two colors across empty slots. Informs the preset-palette + interpolated-breaks model.
- **§4.6.5 transparent grid maps:** transparent/shade % over a background — informs D-18 overlay legibility (semi-transparent hatched objects over the fill).

**MapLibre hatch implementation:** MapLibre GL has no native hatch. Generate each hatch pattern at runtime on a `<canvas>` (diagonal line fill per kind), register via `map.addImage(id, imageData)`, and use `fill-pattern: id` on the area layers. Point symbols via `symbol` layers + generated icons; line width/color via `line` paint. ⚠ D-19: this restyles the **display** layers in `web/src/map/` — it must NOT change Terra Draw's **draw-time** styling/validation behavior (they are separate systems; coordinate, don't regress).

## Export (D-20/D-21/D-22, GRID-05)

All bytes generated in WASM from the OPFS tensor/grid; browser downloads via a Blob/`URL.createObjectURL` (nothing leaves the device).

- **GeoTIFF (raster level grid):** the continuous dB(A)/dB(C) fine-tier grid as Float32. Two options: (a) hand-roll a **minimal single-strip Float32 GeoTIFF** with `GeoKeyDirectoryTag` + `ModelPixelScaleTag` + `ModelTiepointTag` (~200 LOC, zero dep, matches the project's hand-roll ethos); (b) add the mature **`tiff`** crate (OK, 1.77M dl/wk) and write geo-keys via its generic tag encoder — **gate on a `wasm32-unknown-unknown` build verify** (image-tiff is pure Rust but confirm no incompatible deps). CRS from the project's pinned UTM.
- **GeoJSON (isophone fill polygons):** reuse the in-tree `geojson` crate; polygons already produced by the tracer in LonLat.
- **CSV (spectra):** hand-roll — columns `band_index, exact_hz, level_db` (+ dB(A)/dB(C) totals row). Exact Hz from `FreqAxis::centres`; nominal labels display-only.
- **Metadata/attribution (D-22):** CRS (EPSG from `envi-geo`), dB weighting label, engine version + scene `tensor_hash`, and the OSM/Overture/ESA WorldCover/Copernicus attribution strings (already captured in Phase-8 `Provenance`). Embed in GeoTIFF `ImageDescription`/`GDAL_METADATA`, GeoJSON `properties`/foreign members, and CSV header comments.

## Info-Button Help System (D-23/D-24/D-25)

**Recommended shape (own wave, D-25):**
- `web/src/help/catalog.ts`: a typed map `Record<ControlId, HelpEntry>` where `HelpEntry = { title, body (multi-paragraph, English), citations }`. Structured data, NOT text in JSX (D-25) — maintainable + drift-checkable.
- `web/src/help/InfoButton.tsx`: a reusable `<InfoButton controlId="...">` (icon → popover or docked side-panel; presentation is discretion). Every interactive control renders one.
- **Coverage check (test):** enumerate all interactive control ids (a central `ControlId` union or a registry) and assert each has a catalog entry — a control without help FAILS the test. This is the D-25 guarantee. Consider deriving the id set from the `ControlId` type so a new control can't compile without being enumerated.
- **Content rules (D-24):** multi-paragraph per control (what it does, every option/range/unit/default, Nord2000/NoizCalc rationale). Cite **AV 1106/07 by report number** and **TI 386** — ⚠ **never paste AV text** (copyrighted; explain in our own words). English-only.
- **Scope:** retrofit ALL Phase-7/8/9/10 panels (scene editor/palette/inspector, import, weather, calc) + the new Phase-11 controls. Large sweep — isolate into its own plan/wave so it doesn't serialize the feature work.

## Common Pitfalls

### Pitfall 1: Divergent readout in the WASM orchestration
**What goes wrong:** re-implementing the dB/weighting/MAC sum in `envi-compute` slightly differently from the engine law → results drift from the FORCE-validated baseline.
**How to avoid:** call `readout_coherent`/`readout_incoherent`/`band_levels_db_two_channel`/`compose_gain` directly; add a WASM-side test asserting MAC ≡ a full engine readout (the Phase-4 equivalence test pattern). **Warning sign:** a bespoke `10.0*x.log10()` loop in Phase-11 code.

### Pitfall 2: Reading a chunk in memory order instead of logical order
**What goes wrong:** the OPFS chunk is `[s][r_local][f]` LOGICAL order; a sliced/reused file could mislead. **How to avoid:** decode in `[s][r_local][f]` strides exactly as `opfs_sink.rs`'s `read_chunk` test does; map `r_local` back to `global_index` via the manifest `chunk_receivers`.

### Pitfall 3: Nominal-Hz comparison
**What goes wrong:** aggregating 1/12→1/3 or applying weighting by nominal frequency. **How to avoid:** band index only; 1/3-oct = every 4th grid point (`third_octave_pick`).

### Pitfall 4: OPFS sync handles on the main thread
**What goes wrong:** `createSyncAccessHandle()` throws off-worker. **How to avoid:** all tensor read I/O in the compute worker; pre-open handles async then read sync (the existing write-path hoist pattern).

### Pitfall 5: Re-solving on a scale edit
**What goes wrong:** editing color breaks triggers a recompute → violates SC3, kills interactivity. **How to avoid:** cache the level grid; re-contour only.

### Pitfall 6: Iso-bands that cross / leak at saddles
**What goes wrong:** naive marching squares produces self-crossing or leaking bands at ambiguous (saddle) cells. **How to avoid:** resolve saddles with the mean-value (cell-average) rule and reuse `landcover.rs`'s sharpest-turn stitch + non-crossing test as a property test.

### Pitfall 7: Cross-origin isolation lost in the UAT server
**What goes wrong:** SharedArrayBuffer/threaded WASM needs COOP/COEP; a Playwright static server without those headers breaks the solve. **How to avoid:** reuse the Phase-10 COOP/COEP credentialless header setup for the dev/UAT server.

### Pitfall 8: MapLibre fill-pattern regressing draw-time behavior
**What goes wrong:** restyling scene-object layers accidentally changes Terra Draw's editing/validation styling. **How to avoid:** treat display layers and draw-time styling as separate; regression-test the Phase-7 editor journeys.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Threaded WASM toolchain (nightly + `-Zbuild-std` + atomics) | recondition/isoband/export WASM build | ✓ (Phase-10) | existing `build:wasm:compute` | — |
| Cross-origin isolation (COOP/COEP) | SharedArrayBuffer tensor read | ✓ (Phase-10 `envi-service` + Vite) | existing | — |
| OPFS (`FileSystemSyncAccessHandle`) | tensor read/write | ✓ (Phase-8/10) | Chromium worker | — |
| `@playwright/test` | offline UAT | ✓ | ^1.61.1 (devDep) | — |
| `geojson` crate | polygon export | ✓ (in-tree, `envi-gis`) | — | serde_json |
| `tiff` crate (optional) | full GeoTIFF | ✗ (not added) | verify wasm32 | hand-rolled minimal GeoTIFF |
| `contour`/`contour-isobands` | — | ✗ (SUS, not used) | — | hand-rolled tracer |
| Chart JS library | spectrum panel | ✗ (none) | — | inline SVG (recommended) |

**Missing dependencies with no fallback:** none — every capability has an in-tree path.
**Missing dependencies with fallback:** GeoTIFF encoder (hand-roll or add `tiff` behind a wasm-build verify); chart lib (inline SVG).

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| "Server-side" acoustic math (SVC-07 wording) | Client-side Rust→WASM over OPFS | Phase-10 pivot (D-01) | The 409/recondition contract stays; the MAC executes in WASM, not axum |
| Server recondition endpoint returns canned zero stubs (`calc.rs`) | Real WASM MAC readout | Phase 11 | Open question: keep the server endpoint as the designed-ahead contract, or retire it (see Open Q1) |
| `contour` crate | Hand-rolled marching squares | Phase 8 (declined) | Precedent to hand-roll again for iso-bands |

**Deprecated/outdated:**
- The server-side async job stream for the solve — Phase-10 moved the solve to the compute worker; the server stream stays only for ERA5/CDS (D-10).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | A/C weighting constants (12194, 20.6, 107.7, 737.9) + offsets (+2.00, +0.06) are the IEC 61672-1 analytic form | A/C weighting | Wrong weighting = every dB(A)/dB(C) total wrong. **Mitigation:** anchor test vs IEC 61672-1 Table 3 third-octave values is mandatory before ship |
| A2 | `tiff` (image-tiff) builds for `wasm32-unknown-unknown` | Export | If not, fall back to hand-rolled GeoTIFF. **Mitigation:** wasm-build verify checkpoint before adopting |
| A3 | Fine-tier receivers form a regular reconstructable lattice (from `tiers::partition`) suitable for a 2D grid | Isophone | If tiers are irregular, grid reconstruction needs the tier plan's spacing/origin explicitly. **Verify** against `envi-compute/src/tiers.rs` at plan time |
| A4 | EU END noise-map palette break values (the "canonical" ~5 dB bands, e.g. Lden 55–75) | Color system | Default breaks look non-standard to acousticians. Confirm the exact END band edges with the user or END Annex |
| A5 | The isoband tracer sub-100 ms at 100k cells in WASM (no benchmark run this session) | Isophone | If slow, the gdal escape hatch (server-side) conflicts with D-01. **Mitigation:** the roadmap-flagged early benchmark |

## Open Questions (RESOLVED)

1. **Where is the SVC-06 409 realized — server or client?**
   - What we know: `calc.rs` has a full server-side 409 gate (currently serving canned stubs); the WASM path has `ComputeError::HashMismatch`. D-01 says the MAC runs in WASM.
   - What's unclear: whether the server recondition endpoint stays as the contract (and Phase 11 just makes it real) or is retired in favor of a purely client-side stale/refuse gate.
   - Recommendation: realize the 409 **semantics client-side** (stale badge + refuse-to-MAC on hash mismatch, the honest-state contract), and keep the server endpoint's DTOs as the frozen wire shape for the designed-ahead SVC-06. Confirm with the planner/discuss.
   - **RESOLVED: realized client-side (stale badge + refuse-to-MAC on hash mismatch); server DTOs stay the frozen wire shape — see 11-03 Task 2 / 11-07.**

2. **Coherent vs incoherent selection per source.**
   - Road sources are incoherent (Annex-A); conditioned loudspeaker arrays are coherent. The tensor stores both channels, but which readout law applies per source needs a source-type flag.
   - Recommendation: drive it off the source kind already in the scene DTO; default road → incoherent, directional/conditioned → coherent. Verify the flag exists.
   - **RESOLVED: no explicit source-type flag exists today; 11-01 Task 3 threads it by source composition with a documented default policy.**

3. **Exact EU END default break edges + palette hex.** See A4 — confirm the canonical band edges.
   - **ACCEPTED ASSUMPTION (checkpoint-gated): exact END/CNOSSOS band edges + hex carried as an explicit assumption, confirmed via a checkpoint:human-verify task in 11-06 before the default ships.**

## Security Domain

> `security_enforcement` enabled (GSD gate 3 `/gsd-secure` runs). Client-side WASM + browser download; no server acoustic compute.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V5 Input Validation | **yes** | Color-scale breaks (monotonic, finite), filter arrays dense `[105]`, met overrides in physical ranges, receiver ids valid — validate in WASM (typed errors, never panic — the existing boundary discipline) |
| V12 File/Resource | **yes** | OPFS path traversal already guarded (`safeSeg`/`assertHex` in `opfs.ts`); export filenames sanitized; downloads are Blobs, no server write |
| V6 Cryptography | no (identity only) | blake3 is an identity hash, not a security primitive — do not present it as tamper-proof |
| V2/V3/V4 Auth/Session/Access | no | Auth is deferred (Phase-10 D-12) |

### Known Threat Patterns

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Malformed tensor bytes / dims from OPFS | Tampering | The reader validates dims + finiteness like `put_chunk`; typed error, never panic/UB |
| Stale/mismatched hash served as fresh | Spoofing | Re-mint identity per request; 409/refuse (never silently serve) — honest states |
| Copyrighted AV 1106/07 text leaking into UI help | Info disclosure / licensing | D-24 rule: cite by report number, own words, never paste |
| Canvas hatch / addImage with untrusted data | — | Patterns are program-generated (no user string → image); no XSS surface |
| Export path traversal | Tampering | Sanitized filenames; Blob download only |

## Sources

### Primary (HIGH confidence)
- `crates/envi-engine/src/tensor.rs` — `readout_coherent`/`readout_incoherent`/`compose_gain`, OPFS-independent MAC laws + the frozen accumulation order
- `crates/envi-engine/src/freq.rs` — the 105-point band-index grid + `FreqAxis::centres`
- `crates/envi-compute-wasm/src/{lib.rs,opfs_sink.rs,identity.rs}` — the WASM boundary, frozen chunk byte format, tensor-identity hash + HashMismatch gate
- `crates/envi-service/src/api/calc.rs` — the recondition/recompute split + 409 `tensor_hash_mismatch` gate
- `crates/envi-gis/src/landcover.rs` — the hand-rolled marching-squares/ring-classification pattern to adapt
- `crates/envi-store/src/dto.rs` — `ConditioningDto` (the exact WEB-05 readout param)
- `web/src/compute/opfs.ts` + `web/src/store/calc.ts` — OPFS glue + client-side calc state
- `web/src/spectrum/SpectrumEditor.tsx` — the reused filter control (D-11)
- `docs/references/dbaudio-ti386-1.6-en.md` §4.6.3/§4.6.5/§4.6.6 — NoizCalc object styling + color scale
- `dataviz` skill `references/palette.md` — validated sequential/diverging/categorical palette method

### Secondary (MEDIUM confidence)
- IEC 61672-1:2013 A/C-weighting analytic formula — cross-checked MATLAB Audio Toolbox docs, MetricGate, third-octave.com, Wikipedia A-weighting (constants agree)
- `contour`/`contour-isobands` crates — algorithm reference (d3-contour port); flagged SUS, not adopted

### Tertiary (LOW confidence)
- EU END default noise-map break edges + palette hex (A4 — confirm before ship)

## Metadata

**Confidence breakdown:**
- Integration surfaces (MAC, OPFS format, DTOs, identity, 409): HIGH — read directly from committed source
- A/C weighting: MEDIUM — exact formula known, needs the mandatory IEC Table-3 anchor test
- Isophone tracer: MEDIUM — algorithm well-understood, no benchmark run this session
- Color/NoizCalc styling: HIGH — TI 386 sections read directly; dataviz method loaded
- Export encoders: MEDIUM — approach clear; `tiff` wasm32 build unverified

**Research date:** 2026-07-12
**Valid until:** 2026-08-11 (30 days — stable domain; the only moving parts are the SUS contour crates and image-tiff versions)
