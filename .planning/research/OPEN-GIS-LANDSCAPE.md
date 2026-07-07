# State of the Art: Open GIS Models & Data for a Nord2000 Engine

Research synthesis (Fable 5 agents, 2026-07-07). Feeds `/gsd-new-project` requirements + roadmap. Confidence noted per claim in section detail; **High** = verified against official repo/portal/docs this session.

---

## TL;DR — recommended open stack

| Layer | Pick | Why |
|---|---|---|
| **Architecture template** | **NoiseModelling** (Univ. Gustave Eiffel, GPLv3, v6.0.0 May 2026) | Copy its module split (emission / pathfinder / propagation / db-glue) + Delaunay receiver grids + cut-profile abstraction. Port ideas, not GPL code. It implements CNOSSOS, not Nord2000 — use as structural reference + cross-validation baseline. |
| **Nord2000 engine** | **Build it — genuinely novel** | No maintained open Nord2000 engine exists. NMSIM's is a closed Windows binary; STD_CMP is a dead 2016 unlicensed prototype. |
| **Backend geometry (Rust)** | `geo` + `rstar` + `spade` (constrained Delaunay) + `gdal` (DEM/GeoPackage) + `proj` | All mature 2026. Self-build the DEM cut-profile extractor (biggest self-build item; validate vs GRASS `r.profile`). |
| **Frontend** | **MapLibre GL JS 5 + react-map-gl 8 + Terra Draw** (`maplibre-gl-terradraw`) | State-of-the-art OSM editable input. Avoid mapbox-gl-draw (broken on MapLibre) and Leaflet. Render output as server-side isophone **fill polygons** (GDAL contour), not the `heatmap` layer. |
| **Terrain (DEM)** | Tiered: national LiDAR DTM (AHN/DGM1/RGE ALTI/EA/DHM, 0.5–1 m) → **Copernicus GLO-30** COG fallback | Use DTM (bare-earth) — buildings modeled separately as screens. GLO-30 from AWS S3 COGs, GDAL `/vsicurl/` windowed reads. **Avoid FABDEM** (non-commercial license) and EEA-10 (restricted) / EU-DEM (discontinued). |
| **Ground/impedance** | **ESA WorldCover 10 m** (CC-BY, global) + CLC+ Backbone / HRL Imperviousness (Europe) + OSM overrides | Map land-cover classes → Nordtest 8-class flow resistivity σ / CNOSSOS G-factor. |
| **Buildings** | **Overture Maps** (GeoParquet, primary) + 3DBAG/LoD2/EUBUCCO for measured heights | Height fallback chain: national 3D → Overture/OSM `height` → `levels×3m+1.5` → regional default. |
| **Weather (runtime)** | **Open-Meteo** (CC-BY, no key, ≤10k/day; buy commercial tier for prod) | Single call gives 10–180 m + pressure-level winds/temps, layered cloud, BLH, soil temp — everything for coefficients A & B. |
| **Weather (L_den classes)** | **ERA5 via Copernicus CDS** | Only API exposing u*, sensible heat flux, z0 → Obukhov length (ECMWF recipe) for wind×stability weather-class statistics. |

---

## 1. Noise-modelling engines / platforms

- **NoiseModelling** — GPLv3, Java, **very active** (v6.0.0 2026-05-20). Module seam to copy: `emission` / `pathfinder` (geometry: cut-profile, Delaunay receivers, reflection/diffraction path search) / `propagation` / `jdbc`. Swap propagation for Nord2000. GPLv3 copyleft → port algorithms/architecture, do not translate source into a non-GPL product. Its v6 changelog (diffraction/curved-path fixes, Delaunay fixes) is a map of the hard geometry problems.
- **NMSIM-Python** — CC0 wrapper over a **closed Nord2000 Windows binary**; dormant (last push Apr 2024). Confirms the market gap; only its GIS pre-processing patterns are reusable.
- **opeNoise Map** (Arpa Piemonte, GPLv3, QGIS plugin) — good façade-receiver UX; scientifically light.
- **Code_TYMPAN** (EDF, GPL C++) — semi-dormant; interesting ray-tracing solver reading only.
- **STD_CMP** — dead 2016, no license, but a Python side-by-side of ISO 9613-2 / Harmonoise / **Nord2000** / CNOSSOS — read-only formula crib for validation.
- Nord2000 otherwise lives only in commercial tools (SoundPLAN, EMD windPRO/DECIBEL, FORCE, noiseLAB).

## 2. GIS backbone & Rust ecosystem

- Server-side spatial: PostGIS 3.6 / H2GIS (embedded, LGPL) / GDAL 3.12 / QGIS (4.0 LTS lands 2026) / GRASS (`r.profile` as correctness oracle).
- Rust crates (all verified mature on crates.io 2026): `geo`/`geo-types`, `rstar` (R*-tree = your in-process spatial index), `spade` (**constrained Delaunay + refinement**, best-in-class for receiver grids), `gdal` (DEM/GeoPackage, links C GDAL), `proj` (CRS transforms), `startin` (terrain TIN), `geographiclib-rs` (pure-Rust geodesics). Young/immature: `geotiff` 0.1 (fallback to `gdal`).
- **Biggest self-build gap:** no Rust terrain-profile / line-of-sight crate — write the DEM cut-profile extractor yourself. Design reference = NoiseModelling `pathfinder`.

## 3. Terrain / ground / buildings data — see TL;DR table

Key nuances: GLO-30 & SRTM are DSM-biased (high in forest/city) — prefer national LiDAR **DTM**. WorldCover 10 m is the best global ground-cover default (CC-BY, AWS S3 COGs). Nordtest→CNOSSOS impedance table (σ, G) captured in the data-sources report. Overture per-feature `sources[].license` since Sep 2025 (OSM-derived features stay ODbL → attribution + share-alike caveat).

## 4. Meteorology — see TL;DR table

Nord2000's whole met difficulty concentrates in **A (wind, direction-dependent via u·cos φ)** and **B (temperature/stability; inversion → B>0)**. Routes: (1) weather classes for L_den → ERA5/CDS; (2) profile fit → Open-Meteo multi-level; (3) surface met + Monin-Obukhov similarity → Open-Meteo (cloud cover as stability proxy) or ERA5 (direct u*, H, L). Avoid OpenWeatherMap (ODbL, no profiles) and Meteostat (CC-BY-NC).

## 5. Future capabilities (DXF / SketchUp / BEM / directional sources)

- **Canonical internal model = semantic 2.5D scene (CityJSON-aligned Rust types: Building/Barrier/TerrainTIN/Source/Receiver, projected CRS, Z-up, meters) + derived glTF mesh layer.** Importers are plugins emitting this model; no format-specific types past the boundary.
- **DXF:** Rust `dxf` crate (ixmilia, MIT); `ezdxf` as semantic reference; `dxf-parser` for in-browser preview.
- **SketchUp:** `.skp` is proprietary + SDK is OSS-hostile and Linux-less → **support via glTF/COLLADA export, never the SDK**. Isolate behind an HTTP boundary if direct `.skp` ever becomes mandatory.
- **BEM:** bet on the **Bempp Rust stack** (`bempp-rs` + `kifmm`, BSD-3, same UCL team); Bempp-cl (MIT Python) for prototyping; NumCalc (EUPL) as external validation. Hybrid pattern: **2.5D BEM** computes insertion-loss correction tables for complex barriers/objects, slotted into Nord2000 via a per-path-segment `PropagationCorrection` interface (add the hook now).
- **Complex directional sources:** model as `Source = Σ SubSource { position, L_W per band, directivity ΔL(θ,φ,f) }`, energy-summed at receiver (Nord2000 norm). Store directivity as per-band spherical balloons; ingest **CLF** (octave-band, matches Nord2000) first, **SOFA/AES69** second.

---

## Cross-cutting architecture decisions to lock now

1. Importer boundary emits the semantic 2.5D model — DXF, SketchUp/glTF, OSM all funnel through it.
2. `PropagationCorrection` interface on path segments → future BEM barrier corrections plug in without touching Nord2000 core.
3. Directional multi-sub-source model + spherical directivity balloons from day one.
4. License hygiene: port NoiseModelling *ideas* not GPL code; attribute Copernicus/ESA/OSM; avoid FABDEM (NC), EEA-10 (restricted), OpenWeatherMap/Meteostat licenses.

---

## Provenance
Full per-agent reports (with all source URLs) available in session history: (1) open GIS/noise platforms + Rust + frontend; (2) terrain/ground/building open data; (3) open weather/meteo APIs; (4) DXF/SketchUp/BEM/directional sources. Researched on Fable 5.
