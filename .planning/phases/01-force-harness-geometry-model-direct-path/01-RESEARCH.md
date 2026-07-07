# Phase 1: FORCE Harness, Geometry Model & Direct Path - Research

**Researched:** 2026-07-07
**Domain:** Nord2000 outdoor sound propagation (Rust engine) — test harness, semantic scene model, free-field direct path at 1/12-octave complex resolution
**Confidence:** HIGH (primary sources obtained and numerically cross-verified this session)

## Summary

The single most important finding of this session: **the primary sources are obtainable right now.** AV 1106/07 rev. 4 (the implement-from document, 177 pages) was downloaded via the Wayback Machine (FORCE's site redesign broke the original URL), and the FORCE/DELTA road-traffic test-case Excel workbooks (2009 corrected edition) download directly from the Danish EPA (mst.dk) and parse cleanly — 62 straight-road cases with **full-precision** (unrounded) reference spectra, propagation parameters, and terrain profiles in a fixed per-worksheet cell layout. The blocker recorded in STATE.md ("must obtain AV 1106/07 and the FORCE test suite") is resolved except for one caveat: a 2010 revision of the test values exists (Environmental Project 1335; files named `*_20100610.xls`) whose spreadsheets are not online — the 2009 set is the actionable baseline and the deltas are documented.

The physics for this phase is now fully specified and verified: the Nord2000 compound model is `L_R = L_W + ΔL_d + ΔL_a + ΔL_t + ΔL_s + ΔL_r` per band (AV 1106/07 Eq. 1/329); the direct-path divergence is `ΔL_d = −10·log10(4πR²)` (Eq. 330); air absorption is ISO 9613-1 pure-tone α (Eq. 286 — full equation set below, transcription verified against published ISO 9613-2 Table 2 values) followed by Nord2000's band correction `ΔL_a = −A₀·(1.0053255 − 0.00122622·A₀)^1.6` (Eq. 287 / AV 1849/00 Eq. 3, resolved by glyph-position extraction and confirmed by two independent numerical cross-checks). The sound speed is `c = 20.05·√(t + 273.15)` (Eq. 335). For the 1/12-octave axis, use the IEC 61260-1 base-10 ratio with the `x/12` rule: 105 points from 25.119 Hz to 10 kHz where **every 4th point lands exactly on a 1/3-octave exact centre** — making FORCE band comparison a direct index pick, no aggregation needed.

One scope-shaping fact the planner must internalize: the FORCE road cases embed the **road source model** (Jonasson SP 2006:12 emission tables, 2–3 sub-sources per vehicle at 0.01/0.30/0.75 or 3.5 m, 179 source points, pass-by integration). Phase 1 cannot — and per the roadmap should not — make any FORCE road case go green. The Phase 1 harness must therefore load real FORCE cases, execute what exists, and report **meaningful failures** (success criterion 1), while the Phase 1 *numeric gate* is unit-level: divergence identities and ISO 9613-1 reference anchors (computed below).

**Primary recommendation:** Build a two-crate cargo workspace (`envi-engine` pure math, `envi-harness` I/O + case running via `calamine` + `libtest-mimic`), read the FORCE `.xls` files directly (fixed cell layout verified), define the complex transfer convention `H(f) = 10^(ΔL/20) · e^(−jkR) / √(4π R²)` now, and gate Phase 1 on the anchor values in this document.

## Phase Requirements

<phase_requirements>

| ID | Description | Research Support |
|----|-------------|------------------|
| VAL-01 | Test harness that loads and runs FORCE road-traffic test cases, built before propagation code | FORCE suite format fully reverse-engineered (§FORCE Test Suite): 4 workbooks, per-sheet cell layout, tolerances (1 dB overall / 1 dB per band / dip-shift rule), download URLs verified working; harness pattern via `calamine` + `libtest-mimic` |
| GEO-01 | Canonical semantic 2.5D scene (Source, Receiver, Barrier, Building, TerrainProfile), projected metric CRS, Z-up | Rust type design in §Architecture Patterns; AV 1106/07 §5.3.1 terrain-profile contract (points + per-segment σ, r; hSv/hRv conventions) |
| GEO-02 | Consume terrain profile + impedance segments + screen edges from FORCE case files | Exact spreadsheet columns verified (X, Z, flow resistivity kNs·m⁻⁴, roughness); impedance classes A–H mapped; screens encoded as terrain-profile segments per AV 1106/07 §5.1 |
| GEO-03 | Source→receiver azimuth and reflection-path geometry | atan2 azimuth in projected CRS; image-source reflection geometry; wind-projection consumer (A·cos φ) documented for Phase 3 forward-compat |
| ENG-01 | Direct-path attenuation (geometrical divergence) per 1/12-oct point | ΔL_d = −10·log10(4πR²) verified (Eq. 330); anchors: d=100 m → −50.992099 dB |
| ENG-04 | ISO 9613-1 air absorption from T, RH, p | Full equation set + Nord2000 band correction verified; reference table at 15 °C/70 % (FORCE conditions) computed |
| SRC-01 | Point sub-source with per-1/12-octave source spectrum | `SubSource { position, spectrum: BandSpectrum }` design; L_W → complex amplitude convention; forward-compatible with Nord2000 road sub-source heights |

</phase_requirements>

## Architectural Responsibility Map

This is a pure computation engine — tiers are library layers, not web tiers.

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Frequency axis (1/12-oct grid, band mapping) | `envi-engine::freq` | — | Load-bearing shared vocabulary; every module indexes it |
| Scene model (Source/Receiver/Barrier/TerrainProfile) | `envi-engine::scene` | — | Canonical semantic types; importers emit these, physics consumes them |
| Path geometry (azimuth, distances, reflection geometry) | `envi-engine::geometry` | `scene` (inputs) | Pure geometry over scene types; no physics |
| Direct-path propagation (divergence, air absorption, complex transfer) | `envi-engine::propagation` | `freq`, `geometry` | The physics; pure functions, f64, no I/O |
| FORCE case loading (.xls / TOML) | `envi-harness::cases` | `calamine`, `serde` | I/O quarantined outside the engine; engine never sees file formats |
| Reference comparison + tolerance reporting | `envi-harness::compare` | `freq` (band mapping) | Comparator is harness logic, reusable across phases |
| Test execution / per-case pass-fail | `envi-harness` bin + `libtest-mimic` | — | One dynamic test per FORCE case; CLI report for humans |

**Boundary rule:** `envi-engine` has zero I/O dependencies (no calamine, no serde requirement in core types beyond optional derive). `envi-harness` owns all file parsing. This is the seam that makes "harness before propagation code" a real architectural property rather than a sequencing note.

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `ndarray` | 0.17.2 | Dense N-d arrays; will hold `H[s,r,f]` | The Rust numeric-array standard; 99 M downloads; declares `num-complex ^0.4`, `approx ^0.5` as deps — stack is internally consistent [VERIFIED: crates.io API 2026-07-07] |
| `num-complex` | 0.4.6 | `Complex<f64>` transfer values | The only mainstream complex type; ndarray-compatible; 5.1 M/wk [VERIFIED: npm registry equivalent — crates.io + package-legitimacy OK] |
| `thiserror` | 2.0.18 | Error types for engine + loader | De-facto standard [VERIFIED: crates.io] |

### Supporting (harness crate only)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `calamine` | 0.36.0 | Read FORCE `.xls` (legacy BIFF) directly | Pure Rust, no COM/Excel needed; **confirmed the actual TestStraightRoad.xls parses** (via the same BIFF format xlrd read this session) [VERIFIED: crates.io + local parse of real file] |
| `serde` + `toml` | 1.0.228 / 1.x | Synthetic/unit case files (free-field cases, hand-computed geometry cases) | Phase 1 free-field cases aren't FORCE road cases — define them in TOML [VERIFIED: crates.io] |
| `approx` | 0.5.1 | `assert_relative_eq!` / `assert_abs_diff_eq!` in unit tests | Float comparison standard; ndarray integrates with it [VERIFIED: crates.io] |
| `libtest-mimic` | 0.8.2 | Dynamic test generation: one libtest-style test per FORCE case worksheet | Standard for data-driven test harnesses (`harness = false`); 292 k/wk since 2018 [VERIFIED: crates.io + package-legitimacy OK] |
| `anyhow` | current | Harness-side error context | Harness/bin only, never in engine API [VERIFIED: crates.io] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `calamine` (read .xls directly) | Hand-transcribe cases to TOML/JSON | Transcription of 80 cases × 27 bands × full precision is error-prone; calamine reads the authoritative bytes. Keep TOML for *synthetic* cases only |
| `libtest-mimic` | Plain `#[test]` with a loop over cases | A loop reports one failure per run; libtest-mimic gives per-case pass/fail lines, filtering (`cargo test -- straight_road_04`), and CI-friendly output |
| `ndarray` in Phase 1 | Plain `Vec<Complex<f64>>` per path | Phase 1's per-path spectrum is 1-D; `Vec` suffices for the path result, but define the `H[s,r,f]` type alias on ndarray NOW so Phase 4 is a fill-in, not a refactor |
| Rust geo stack (`geo`/`gdal`/`proj`) | — (defer) | **Not needed in Phase 1.** FORCE profiles are already local metric 2-D sections. Pulling gdal in now adds a C build dependency for zero benefit. The scene types just document the CRS convention |

**Installation:**
```bash
# workspace root
cargo new --lib crates/envi-engine
cargo new crates/envi-harness
# engine
cargo add -p envi-engine ndarray@0.17 num-complex@0.4 thiserror@2
cargo add -p envi-engine --dev approx@0.5
# harness
cargo add -p envi-harness calamine@0.36 serde@1 --features serde/derive
cargo add -p envi-harness toml anyhow libtest-mimic
```

**Version verification:** all versions above checked against crates.io on 2026-07-07 (`crates.io/api/v1/crates/<name>` + `cargo search`). ndarray 0.17.2's own manifest requires `num-complex ^0.4` and `approx ^0.5` — the recommended pins cannot conflict. [VERIFIED: crates.io API]

## Package Legitimacy Audit

Ran `gsd-tools query package-legitimacy check --ecosystem crates` on all recommendations.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| ndarray | crates.io | since 2015 | 99.2 M total | github.com/rust-ndarray/ndarray | OK* | Approved |
| num-complex | crates.io | 10 yrs | 5.1 M/wk | github.com/rust-num/num-complex | OK | Approved |
| approx | crates.io | 10 yrs | 2.0 M/wk | github.com/brendanzab/approx | OK | Approved |
| serde | crates.io | 11 yrs | 17.6 M/wk | github.com/serde-rs/serde | OK | Approved |
| toml | crates.io | 11 yrs | 12.8 M/wk | github.com/toml-rs/toml | OK | Approved |
| calamine | crates.io | 9 yrs | 232 k/wk | github.com/tafia/calamine | OK | Approved |
| thiserror | crates.io | 6 yrs | 22 M/wk | github.com/dtolnay/thiserror | OK | Approved |
| anyhow | crates.io | 6 yrs | 13 M/wk | github.com/dtolnay/anyhow | OK | Approved |
| libtest-mimic | crates.io | 8 yrs | 292 k/wk | github.com/LukasKalbertodt/libtest-mimic | OK | Approved |

\* The seam returned SUS for ndarray due to a null-signal API glitch; manually re-verified against the crates.io API directly (0.17.2, 99 M downloads, rust-ndarray org, updated 2026-01-10) → treated as OK. No Rust crate here has install scripts (crates.io has no postinstall mechanism; `build.rs` exists only in C-linking crates, none of which are recommended for this phase).

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none (ndarray glitch manually cleared)

## FORCE Test Suite (VAL-01) — Verified Format & Sourcing

### What exists and where — all links verified working this session

| Artifact | What | URL | Status |
|----------|------|-----|--------|
| AV 1106/07 rev. 4 (13 Jan 2014) | **The implement-from method spec**, 177 pp | `http://web.archive.org/web/20240221070539/https://forcetechnology.com/-/media/force-technology-media/pdf-files/projects/nord2000/nord2000-nordtestproposal-rev4.pdf` | Downloaded + text-extracted this session [VERIFIED] |
| Env. Project 1276 (2009), "Corrected test cases" | Report + **the 4 .xls workbooks** | `https://www2.mst.dk/udgiv/publications/2009/978-87-7052-938-9/html/default_eng.htm` → `.../TestStraightRoad/TestStraightRoad.xls` (same pattern for TestCurvedRoad, TestCityStreet, TestYearlyAverage) | .xls downloaded + parsed this session [VERIFIED] |
| Env. Project 1335 (2010), "Revised test cases" | Report describing the **2010-revised** values (`*_20100610.xls`) | `https://mst.dk/media/ecyi5sso/revised_test_cases_for_updated_version_of_nord2000.pdf` | PDF downloaded; the revised .xls files were NOT found online [VERIFIED report / xls missing] |
| AV 1849/00 Part 1 (background physics, homogeneous atmosphere) | Source of the air-absorption band-correction derivation | `http://www.magasbakony.hu/Val/Nord2000_homogeneous_atmosphere_Part_1.pdf` (mirror); also archived at forcetechnology.com via Wayback | Downloaded this session [VERIFIED] |
| AV 1851/00 Part 2, AV 1117/06 validation, User's Guide, 2018 amendments (118-22465) | Background / later phases | All archived under `web.archive.org/web/*/forcetechnology.com/-/media/force-technology-media/pdf-files/projects/nord2000/*` | Wayback CDX confirmed 200s [VERIFIED] |

**Licensing handling:** these documents are freely downloadable but copyrighted. Do NOT commit the PDFs or .xls files to git. Plan a `just fetch-refs` / shell script that curls them into a git-ignored `refs/` directory with SHA-256 checks, so the repo stores provenance, not content.

### Suite structure (from Env. Project 1335/1276, verified against the actual .xls)

| Group | Cases | Content | Phase relevance |
|-------|-------|---------|-----------------|
| Straight Road | 62 (12 terrain groups: flat 1–18, mixed impedance 21–24, elevated road 31–44, valley 51–64, thin/thick/double screens 71–94, non-flat 101–114, forest 121–124) | Constant perpendicular terrain profile; homogeneous + up/downwind variants; hr 1.5/4 m | Groups drive Phases 2–3; harness loads all now |
| Curved Road | 10 (8 positions; 4-1/4-2, 5-1/5-2 screen variants) | 2-lane curved road, 3 wind conditions, thin+thick screens, coordinates in a `Coordinates` sheet | Phase 4 (needs 3-D pathfinding + emission) |
| City Street | 4 | 1st+2nd order façade reflections, ρE = 1.0 and 0.7, receiver at façade | Phase 4 |
| Yearly Average | 4 | L_den via weather-class statistics (`Met. statistics` sheet) | Phase 3/4 (Route 1) |

### Per-worksheet cell layout (verified by parsing TestStraightRoad.xls, 62 sheets named "1"…"62")

Fixed grid, 40 rows × 9 cols:
- **Row 0:** title; **row 2:** description string (e.g., `Flat terrain, d=100 m, hr=1.5 m, impedance A, homogeneous atm.`)
- **Traffic block** (col A/B, rows 6–8): `Nvc` (veh per T), `v` (km/h), `T` (hours)
- **Propagation block** (col A/B, rows 12–21): `hr` (m), `t0` (°C), `z0` (m), `zu` (m), `u` (m/s), `φ` (°), `su` (m/s), `dt/dz` (°/m), `sdt/dz` (°/m), `Cv2` (m^(4/3)/s²), `Ct2` (K/s²). RH = 70 % globally (stated in report text, not on sheet)
- **Terrain profile** (cols A–D from row ~25): `X` (m from road centre line), `Z` (m), `Flow res.` (kNs·m⁻⁴), `Roughn.` (m); zero-padded rows terminate the profile
- **Results** (cols F–I): `LAeq,24h`, `LAE`, `LAmax` (overall, A-weighted) + 27-row spectrum table `Freq (25…10000 Hz) | Leq,24h | LE | dL` — **stored at full float precision** (e.g. LAeq = 39.39836757521), not the rounded values printed in the report appendix
- `dL` = propagation effect = LE − free-field LE (free field = divergence only, air absorption and ground excluded) — informational, not part of the pass criterion

**Semantics needed by the loader:** vehicles drive in the middle of the nearest 5 m lane → source line at x = 2.5 m from road centre; profile x starts at 3.25 m; source heights come from the road source model (0.01 / 0.30 / 0.75 m — not on the sheet); hSv is height above the FIRST profile point, hRv above the LAST (AV 1106/07 §5.3.1); receiver at horizontal distance = last profile X.

### Acceptance tolerances (Env. Project 1335 §6 — the "standard's tolerance" for VAL-02 and this phase's success criterion 3)

- Overall A-weighted levels (LAeq,24h, LAE, LAmax): **≤ 1 dB** deviation, with > 0.5 dB flagged for investigation
- Per 1/3-octave band: **≤ 1 dB**; exceedances must be investigated
- Ground-dip exception: a shift of the interference dip by one 1/3-octave band is acceptable (large band deviations near dips)
- Road-length caveat: groups 1 & 4 assume an infinite road; groups 2 & 3 include 40× the shortest receiver distance

**Harness design consequence:** the comparator must report per-band signed deviations, max |deviation|, overall-level deviation, and support three case outcomes: `Pass`, `Fail(report)`, and `Skipped(reason)` — where reason for all road cases in Phase 1 is `requires: emission-model, ground-effect, …` derived from a per-case capability list. This makes "the harness runs and fails meaningfully before propagation code exists" (success criterion 1) concrete: every case loads, is inspected for required capabilities, and reports what's missing; free-field synthetic cases actually execute.

### Recommended harness input schema

Two input sources, one internal representation:

1. **FORCE `.xls` loader** (calamine): reads the verified layout above → `CaseDefinition`. Robust to the 2010 revision (same layout per report).
2. **TOML synthetic cases** (`cases/*.toml`): for Phase 1 free-field gates and hand-computed geometry checks; identical `CaseDefinition` shape plus explicit expected values with per-case tolerance:

```toml
# cases/freefield_100m.toml
[meta]
name = "free-field 100 m, 15C 70% RH"
kind = "free-field"          # engine capability tag
reference = "analytic"        # vs "force-2009" / "force-2010"

[source]
position = [0.0, 0.0, 0.5]    # metric CRS, Z-up
spectrum = { kind = "unit" }  # L_W = 0 dB in every band (transfer test)

[receiver]
position = [100.0, 0.0, 1.5]

[atmosphere]
t_air_c = 15.0
rh_percent = 70.0
pressure_kpa = 101.325

[expected]
tolerance_db = 1e-9           # analytic identity for divergence
bands = "analytic:divergence+iso9613"
```

## Architecture Patterns

### System Architecture Diagram

```
                      ┌────────────────────────────────────────────────┐
                      │                envi-harness (bin + tests)      │
  FORCE .xls ──────►  │  cases::xls (calamine)  ─┐                     │
  (refs/, gitignored) │                          ├─► CaseDefinition    │
  cases/*.toml ─────► │  cases::toml (serde)    ─┘        │            │
                      └───────────────────────────────────┼────────────┘
                                                          ▼
                      ┌────────────────────────────────────────────────┐
                      │                envi-engine (lib, no I/O)       │
                      │  scene:    Scene{Source,Receiver,Barrier,      │
                      │            Building,TerrainProfile}            │
                      │      │                                         │
                      │      ▼                                         │
                      │  geometry: PathGeometry{R, azimuth,            │
                      │            reflection geometry}                │
                      │      │                                         │
                      │      ▼                                         │
                      │  propagation::direct(path, atmos, &FREQ_AXIS)  │
                      │      = divergence ∘ air_absorption ∘ phase     │
                      │      │                                         │
                      │      ▼                                         │
                      │  TransferSpectrum: [Complex<f64>; 105]         │
                      │  (per sub-source × receiver; seed of H[s,r,f]) │
                      └───────────────────────────────────┼────────────┘
                                                          ▼
                      ┌────────────────────────────────────────────────┐
                      │  harness::compare                              │
                      │  band levels ← |H|², A-weighting, band pick    │
                      │  vs reference → per-band deviation table       │
                      │  → Pass / Fail(report) / Skipped(capabilities) │
                      └────────────────────────────────────────────────┘
```

### Recommended Project Structure

```
envi/
├── Cargo.toml                  # [workspace] members = ["crates/*"]
├── refs/                       # git-ignored: fetched PDFs + .xls (script + checksums committed)
│   └── fetch.sh
├── cases/                      # synthetic TOML cases (committed — our own content)
├── crates/
│   ├── envi-engine/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── freq.rs         # FreqAxis: 105-pt 1/12-oct grid, band indices, A-weighting
│   │       ├── scene.rs        # Source, SubSource, Receiver, Barrier, Building, TerrainProfile
│   │       ├── geometry.rs     # PathGeometry: distance, azimuth, image-source reflection
│   │       ├── propagation/
│   │       │   ├── mod.rs      # compose sub-effects into TransferSpectrum
│   │       │   ├── divergence.rs
│   │       │   └── air_absorption.rs   # ISO 9613-1 + Eq.287 band correction
│   │       └── transfer.rs     # TransferSpectrum, future H[s,r,f] alias on ndarray
│   └── envi-harness/
│       ├── src/
│       │   ├── lib.rs
│       │   ├── cases/{mod,xls,toml}.rs   # → CaseDefinition
│       │   ├── compare.rs                # deviations, tolerances, report table
│       │   └── capability.rs             # per-case required-capabilities → Skip reasons
│       └── tests/
│           └── force.rs        # libtest-mimic: one test per worksheet per workbook
└── docs/
    └── research.md             # origin research (already planned)
```

Mirrors the origin-research §5 layout (`met/`, `rays/`, `grid/` slot in as siblings of `propagation/` in Phases 3–4). Workspace-from-day-1 so v2 crates (gdal ingestion, web service) attach without moving code.

### Pattern 1: Fixed frequency axis as a shared, precomputed value

**What:** One `FreqAxis` owning the 105 exact centre frequencies, constructed once (`std::sync::LazyLock`), passed by reference. Band values indexed by a newtype `BandIdx(usize)`, never by raw float comparison.
**When to use:** everywhere — the axis is the engine's vocabulary.

```rust
// Source: IEC 61260-1:2014 midband rule (odd-b form), G = 10^(3/10)
pub const N_BANDS: usize = 105;           // x = −64 ..= 40
pub const G: f64 = 1.9952623149688795;    // 10f64.powf(0.3)

pub struct FreqAxis {
    pub centres: [f64; N_BANDS],          // 1000 * G^(x/12), x=-64..=40
}
impl FreqAxis {
    pub fn new() -> Self {
        let mut centres = [0.0; N_BANDS];
        for (i, x) in (-64i32..=40).enumerate() {
            centres[i] = 1000.0 * G.powf(f64::from(x) / 12.0);
        }
        Self { centres }
    }
    /// Every 4th point is an exact 1/3-octave centre: 27 bands, 25.119 Hz … 10 kHz.
    /// third_octave_index 0 → nominal 25 Hz (exact 25.1189), 26 → 10 kHz (exact 10000.0).
    pub fn third_octave_pick(&self, third_idx: usize) -> f64 {
        debug_assert!(third_idx < 27);
        self.centres[third_idx * 4]
    }
}
```

Key facts (computed + cross-checked this session):
- IEC 61260-1:2014 defines `G = 10^(3/10)` (preferred; base-2 permitted) and midband `f_m = f_r·G^(x/b)` for odd `b`, `f_m = f_r·G^((2x+1)/(2b))` for even `b`, `f_r = 1000 Hz` [CITED: IEC 61260-1:2014 via iTeh preview / NI standards page].
- **Deliberate deviation, document it:** `b = 12` is even, so strict IEC 1/12-octave midbands are half-step offset and NEVER hit 1000 Hz. ENVI instead uses the `x/12` rule so the grid **contains all 27 exact 1/3-octave centres** (x ≡ 0 mod 4). This is what makes FORCE comparison an index pick and Nord2000's "evaluate band level at the centre frequency" semantics carry over. This is a frequency-*point* evaluation grid, not a bank of IEC filter passbands — the standard constrains filters, not evaluation grids.
- Grid: f₋₆₄ = 25.118864 Hz, f₀ = 1000 Hz, f₄₀ = 10000.000 Hz. Nominal↔exact mapping: nominal 25/31.5/40/…/10000 label the exact centres 25.1189/31.6228/39.8107/…/10000.

### Pattern 2: Complex transfer convention (the load-bearing contract)

**What:** Define once, in `transfer.rs` doc comments, and never violate:

- Time convention `e^{+jωt}`; an outgoing wave carries phase `e^{−jkR}`, `k = 2πf/c`, `c = Coft(t_air) = 20.05·√(t_air + 273.15)` [VERIFIED: AV 1106/07 Eq. 335].
- Per (sub-source, receiver, frequency point): `H = A·e^{−jkR}` with amplitude normalized so that **band SPL falls out of L_W by addition**: `L_p(f) = L_W(f) + 20·log10|H(f)|`, i.e. for the free field `|H| = 1/√(4πR²)` ⇒ `20·log10|H| = ΔL_d = −10·log10(4πR²)` — exactly Eq. (330). Air absorption multiplies in as the real factor `10^(ΔL_a/20)`.
- Phase 2+ effects (ground, diffraction) multiply H by their complex pressure ratio relative to free field (Nord2000 computes exactly this ratio, free-field reference `p₀ = 1/r_SR` — AV 1106/07 §5.13 text). The Phase 1 H is therefore the correct seed: later effects are complex multiplications, and Δτ interference appears in the phase automatically.
- Receiver band level from a source spectrum: amplitude `|G_s(f)| = 10^{L_W(f)/20}` (phase 0 unless conditioned), `p(f) = Σ_s H[s,r,f]·G_s(f)`, `L(f) = 20·log10|p(f)|`. This is precisely the Phase 4 MAC contract (OUT-03) — designed in now.

```rust
// Source: AV 1106/07 Eq. (329)/(330)/(335); convention e^{+jωt}
use num_complex::Complex;

pub type TransferSpectrum = Vec<Complex<f64>>; // len == N_BANDS; Phase 4: Array3<Complex<f64>> [s,r,f]

pub fn direct_path(r: f64, atmos: &Atmosphere, axis: &FreqAxis) -> TransferSpectrum {
    let c = 20.05 * (atmos.t_air_c + 273.15).sqrt();        // Coft, Eq. (335)
    let amp_div = 1.0 / (4.0 * std::f64::consts::PI * r * r).sqrt();
    axis.centres.iter().map(|&f| {
        let alpha = iso9613_alpha(f, atmos);                 // dB/m, Eq. (286)
        let a0 = alpha * r;                                  // pure-tone path attenuation, dB
        let dla = band_corrected_attenuation_db(a0);         // Eq. (287): ≥ 0 dB, subtractive
        let amp = amp_div * 10f64.powf(-dla / 20.0);
        let k = 2.0 * std::f64::consts::PI * f / c;
        Complex::from_polar(amp, -k * r)
    }).collect()
}
```

**ndarray layout note (Phase 4 forward-compat):** `Array3<Complex<f64>>` with shape `[s, r, f]` in default (C/row-major) order is frequency-contiguous on the last axis — exactly the PROJECT.md constraint. Nothing to configure; just don't use `.f()` (Fortran order) constructors. [VERIFIED: ndarray docs semantics — default is row-major]

### Pattern 3: Scene types — semantic 2.5D, minimal now, CityJSON-aligned in naming only

```rust
/// All coordinates: projected metric CRS (site-local; v2 auto-UTM), meters, Z-up.
/// Phase 1 sources: FORCE case files only (already local metric).
pub struct Scene {
    pub crs: CrsInfo,               // descriptive tag now ("local-metric"); proj integration is v2
    pub sources: Vec<Source>,
    pub receivers: Vec<Receiver>,
    pub barriers: Vec<Barrier>,     // vertical screens: polyline + top height (thin), or footprint+height (thick)
    pub buildings: Vec<Building>,   // footprint polygon + eaves height (2.5D)
    pub terrain: Vec<TerrainProfile>,
}

pub struct Source { pub sub_sources: Vec<SubSource> }          // SRC-01: composition-ready
pub struct SubSource {
    pub position: [f64; 3],
    pub spectrum: BandSpectrum,     // L_W (dB re 1 pW) per 1/12-oct point, len N_BANDS
    // directivity: Phase 4 (SRC-02); PropagationCorrection hook: field reserved, Phase 2+
}
pub struct Receiver { pub position: [f64; 3] }

/// AV 1106/07 §5.3.1: profile = points (x,z) ascending + per-SEGMENT sigma & roughness.
/// x is distance along the vertical cut plane source→receiver. N points → N−1 segments.
pub struct TerrainProfile {
    pub points: Vec<[f64; 2]>,          // (x, z)
    pub segments: Vec<GroundSegment>,   // len = points.len() − 1
}
pub struct GroundSegment {
    pub flow_resistivity: f64,          // kNs·m⁻⁴ (Nordtest sigma; class table below)
    pub roughness: f64,                 // m (class N = 0)
}
```

- **hSv/hRv convention** [VERIFIED: AV 1106/07 §5.3.1]: source height is measured above the FIRST profile point, receiver height above the LAST. Encode as doc-comment + debug_assert in the profile-to-path constructor, because it is an off-by-metres trap.
- Screens are ultimately merged into the terrain profile ("screens and other man-made structures have been made a part of the terrain profile" — AV 1106/07 §5.1). Phase 1 only needs `Barrier` as a semantic object + the ability to list screen edges; the merge algorithm is Phase 2.
- Impedance classes (Nordtest, for translating case descriptions like "impedance A"): A = 12.5, B = 31.6, C = 80, D = 200, E = 500, F = 2000, G = 20000, H = 200000 kNs·m⁻⁴. A, D, G directly corroborated this session (case 1 grass = 12.5 "impedance A"; report: ground class D = 200, road class G = 20000) [VERIFIED for A/D/G: Env. Project 1335 + xls]; B/C/E/F/H from the standard Nordtest table [ASSUMED — confirm against AV 1106/07 Table when transcribing, or the User's Guide].

### Pattern 4: Azimuth & reflection geometry (GEO-03)

- Azimuth: `az = atan2(dx_east, dy_north)` in the projected CRS (degrees clockwise from north to match FORCE wind convention `φ_u re north`; wind projection for Phase 3: `A ∝ u·cos(az − φ_u)`). Hand-computable test values: source (0,0), receiver (100, 100) → 45°.
- Reflection-path geometry (vertical plane): image-source method — reflect S in the line containing a ground/obstacle segment, reflection point = intersection of image-S→R with the segment; valid iff the intersection lies within the segment. Also compute grazing angle ψ_G for Phase 2. For obstacle (vertical reflector) paths, AV 1106/07 treats the path as an unfolded S→O→R with `Lr` correction — Phase 1 only needs the geometric primitives (image point, path length r₁+r₂, incidence angle), which are pure 2-D vector math. Test with hand-computed cases (e.g., S=(0,0,2), R=(10,0,2) over flat ground → reflection at x=5, path length √(10²+4²)).

### Anti-Patterns to Avoid

- **Parsing spreadsheets inside the engine crate:** keeps calamine/serde out of the numeric core; the seam is `CaseDefinition → Scene + Atmosphere + Expected`.
- **Nominal frequencies as floats as keys:** 31.5 vs 31.622776… — always index by `BandIdx`, render nominal labels only for display.
- **Retrofitting complex phase later:** compute `e^{−jkR}` from day 1 even though Phase 1 magnitudes don't need it; Phase 2's Δτ interference validates against dips only if the phase convention is already exercised and tested (e.g., unit test: two paths with Δr = λ/2 cancel).
- **Building a generic "units" or "quantity" type system:** f64 + naming discipline (`_db`, `_m`, `_hz` suffixes) is the Nord2000-implementation norm; a units crate adds friction across every equation transcription.
- **Copying reference values from the report appendix:** the appendix prints superseded rounded values ("values are from an earlier version of the test data"); ONLY the .xls cells are authoritative, and they carry full precision.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Legacy .xls (BIFF8) parsing | A cell-record parser | `calamine` | BIFF is a gnarly OLE2 compound format; calamine is pure Rust and battle-tested |
| Float test assertions | `(a - b).abs() < eps` scattered | `approx` (`assert_relative_eq!`, `assert_abs_diff_eq!` with explicit `epsilon`/`max_relative`) | Consistent semantics; ndarray implements the traits |
| Data-driven test executor | Custom main() with println pass/fail | `libtest-mimic` | Gets `cargo test` UX (filtering, parallelism, JSON output) for dynamically-discovered FORCE cases |
| Complex arithmetic | Struct with re/im and hand-written ops | `num_complex::Complex<f64>` | `from_polar`, `arg`, `norm` are exactly the needed ops; interoperates with ndarray |
| TOML/serde plumbing | Hand parser for case files | `serde` + `toml` | Obvious |
| CRS transforms | Anything | Nothing (this phase) | Inputs are already site-local metric; `proj` arrives with GIS ingestion in v2 |

**Key insight:** everything else in this phase — divergence, ISO 9613-1, A-weighting, the frequency grid, image-source geometry — IS the product and must be hand-written from the equations. There is no acoustics crate to lean on (that absence is the project's reason to exist). The "don't hand-roll" surface here is deliberately small: quarantine it in the harness crate.

## Verified Physics — the exact formulas the planner should specify

### 1. Compound model & direct path [VERIFIED: AV 1106/07 Eq. (1)/(329), (330)]

```
L_R(f) = L_W(f) + ΔL_d + ΔL_a(f) + ΔL_t(f) + ΔL_s(f) + ΔL_r(f)     per band
ΔL_d   = −10·log10(4π R²)          R = straight S→R distance (homogeneous atmosphere)
```
Phase 1 implements ΔL_d and ΔL_a; ΔL_t = ΔL_s = ΔL_r = 0 (free field).

**Anchors** (analytic, use `tolerance ≤ 1e-9 dB`):
| R | ΔL_d |
|---|------|
| 1 m | −10.992099 dB |
| 10 m | −30.992099 dB |
| 100 m | −50.992099 dB |
| 1000 m | −70.992099 dB |

### 2. Sound speed [VERIFIED: AV 1106/07 Eq. (335)]

```
c = Coft(t) = 20.05 · sqrt(t + 273.15)      t in °C, c in m/s      (15 °C → 340.29 m/s)
```

### 3. ISO 9613-1 pure-tone atmospheric absorption [VERIFIED: constants match AV 1106/07 Eq. (286); transcription reproduces published ISO 9613-2 Table 2 values within table rounding]

Inputs: frequency `f` (Hz), temperature `T = 273.15 + t_air` (K), relative humidity `RH` (%), pressure ratio `p_rel = p_a / p_r`, `p_r = 101.325 kPa`, `T0 = 293.15 K`, `T01 = 273.16 K` (triple point).

```
C        = −6.8346·(T01/T)^1.261 + 4.6151
h        = RH · 10^C / p_rel                                  (molar concentration of water vapour, %)
f_rO     = p_rel · ( 24 + 4.04e4·h·(0.02 + h)/(0.391 + h) )                    (O2 relaxation, Hz)
f_rN     = p_rel · (T/T0)^(−1/2) · ( 9 + 280·h·exp(−4.170·((T/T0)^(−1/3) − 1)) ) (N2 relaxation, Hz)

α(f)     = 8.686 · f² · [ 1.84e-11 · (1/p_rel) · (T/T0)^(1/2)
             + (T/T0)^(−5/2) · ( 0.01275·e^(−2239.1/T) / (f_rO + f²/f_rO)
                               + 0.1068 ·e^(−3352.0/T) / (f_rN + f²/f_rN) ) ]   (dB/m)
```

**Reference anchors computed from this transcription** (self-consistency gates for unit tests):
- Published cross-check (ISO 9613-2 Table 2, 20 °C / 70 % / 101.325 kPa): 1 kHz → 5.0, 4 kHz → 22.9, 8 kHz → 76.6 dB/km. This transcription gives 4.98 / 23.09 / 77.63 dB/km — within the table's rounding (published values are rounded to the displayed digits). Encode with tolerance ±2 %.
- FORCE conditions (15 °C, 70 %, 1 atm) — intermediate values to pin in tests: `h = 1.177222 %`, `f_rO = 36332.37 Hz`, `f_rN = 333.6691 Hz`; α at exact 1/3-oct centres: 25.119 Hz → 1.709912e-5, 100 Hz → 2.512361e-4, 398.107 Hz → 1.921459e-3, 1000 Hz → 4.079240e-3, 3981.07 Hz → 2.638566e-2, 10000 Hz → 1.435243e-1 dB/m. (Derived values — pin as regression anchors, tolerance 1e-12 relative, and cross-check the three published anchors above independently.)
- Validity: ISO 9613-1 specifies 50 Hz–10 kHz (AV 1849/00 states this range); Nord2000 applies it from 25 Hz — α is negligible there (~0.02 dB/km), extrapolation is standard practice. Document it.

### 4. Nord2000 band correction to air absorption [VERIFIED: AV 1849/00 Eq. (3) — resolved by glyph-position extraction from the PDF; AV 1106/07 Eq. (287) identical; cross-checked numerically at two points against independently published behavior]

Pure-tone α overestimates 1/3-octave band attenuation at high `α·R` (filter passband effect; Joppa/Sutherland method, empirically adjusted):

```
A_0  = α(f_0) · R                          pure-tone attenuation over path at exact band centre, dB
ΔL_a = −A_0 · (1.0053255 − 0.00122622·A_0)^1.6
```

Checks that pinned the operator structure: A_0 = 10 → |ΔL_a| = 9.89 (0.11 dB less than pure tone); A_0 = 20 → 19.389 (0.61 dB less — matches the documented "0.6 dB at 20 dB"); A_0 = 100 → 81.9 (18.1 dB less — matches AV 1849/00 Figure 2's 0–20 dB difference axis). Guard: expression is monotonic-safe for A_0 < ~410 dB; clamp A_0 (e.g. at 300 dB, beyond audibility) to avoid the polynomial turning over at extreme ranges. Apply the same formula at all 105 grid points (each 1/12-point is treated as a band centre; deviation from strict 1/3-octave usage is inherent to the finer-grid design decision — document it).

### 5. A-weighting (harness-side, for overall-level comparison) [CITED: IEC 61672-1 standard formula]

```
R_A(f) = 12194²·f⁴ / ( (f²+20.6²) · sqrt((f²+107.7²)·(f²+737.9²)) · (f²+12194²) )
A(f)   = 20·log10(R_A(f)) + 2.00     (A(1000) ≈ 0.0001 dB ✓; A(100) = −19.145; A(8000) = −1.147)
```
Needed by the comparator to form LAeq-style totals from band spectra (Phase 4 gate; implement in harness now, it is 10 lines).

### 6. Numerical guards applicable to this phase (origin research §4.6 forward-look)

- f64 everywhere; deny `f32` in engine code via review convention (add `#![deny(clippy::cast_possible_truncation)]` posture; clippy lint `clippy::float_arithmetic` is too blunt — skip).
- Phase from geometry: compute `k·R` as `2π·f·(R/c)` keeping `R/c` (travel time) as the primitive — Phase 3's Δτ cancellation-safe reformulation will replace travel-time differences; establishing τ as the carried quantity NOW means Phase 2's two-path interference already uses `exp(−jω·τ)` and Phase 3 slots in.
- `4πR²` for R ≥ ~0.1 m is benign; guard R → 0 (source/receiver coincident) with a domain error, not a clamp.
- No catastrophic cancellation exists in the free-field path itself — the traps begin in Phase 2 (Δτ) — but the *pattern* (prefer formulations over subtraction of near-equal numbers) should be written into the propagation module docs now.

## Common Pitfalls

### Pitfall 1: Validating against the report appendix instead of the spreadsheets
**What goes wrong:** The Env. Project 1335 appendix prints old, rounded values; a harness seeded from them fails against correct code.
**Why it happens:** The PDF is easier to read than BIFF .xls.
**How to avoid:** Loader reads .xls cells (full float precision verified, e.g. `39.39836757521`); PDF used only for semantics.
**Warning signs:** Reference values with exactly 2 decimals in fixture files.

### Pitfall 2: Treating the FORCE road cases as a Phase 1 numeric gate
**What goes wrong:** Free-field code can never reproduce LAeq,24h (needs Jonasson emission tables, 179 source points, pass-by integration, ground effect) — the phase stalls chasing an impossible target.
**Why it happens:** "Validated against FORCE" reads like "run case 1 now."
**How to avoid:** Capability tags per case; Phase 1 gates = analytic anchors + ISO 9613-2 published-table cross-checks + hand-computed geometry; FORCE cases load and report `Skipped(requires: …)`.
**Warning signs:** A plan task that says "make straight-road case N pass" in Phase 1.

### Pitfall 3: Nominal vs exact centre frequencies
**What goes wrong:** Using 31.5, 63, 315… as computation frequencies gives sub-0.1 dB but systematic errors in α (which grows as f²) and misaligns the 1/12 grid so no point equals a 1/3 centre.
**How to avoid:** Exact centres `1000·G^(x/12)` everywhere; nominal labels only in display/reporting; spreadsheet's `Freq.` column (25, 31.5, …) maps by band INDEX, not by float equality.
**Warning signs:** `f == 31.5` anywhere; interpolation to "find" the 1/3-oct value on the 1/12 grid.

### Pitfall 4: Skipping the band correction on air absorption (or applying it twice)
**What goes wrong:** Using raw `α·R` is fine below ~10 dB of path attenuation but overestimates by 0.6 dB at 20 dB and ~18 dB at 100 dB — high-frequency long-range bands blow the 1 dB tolerance in later phases; conversely applying Eq. 287 on top of already-band-averaged data double-corrects.
**How to avoid:** One function `band_attenuation_db(alpha_db_per_m, r)` used by everyone; unit tests at A_0 ∈ {10, 20, 100} against the anchors above.
**Warning signs:** `alpha * r` appearing outside that function.

### Pitfall 5: hSv/hRv and profile-orientation mistakes
**What goes wrong:** Source height applied above z=0 instead of above the first profile point; receiver x taken as "distance from source" when FORCE X is "distance from road centre line" (source sits at x = 2.5 m, not x = 0).
**How to avoid:** Constructor takes the profile + heights and produces absolute (x,z) for S and R; TOML/hand-computed geometry cases assert absolute coordinates; document the FORCE lane convention in the xls loader.
**Warning signs:** d = 100 m case producing R = 100.0 exactly (should be 97.5 m horizontal from source line to receiver — verify against case 1's dL at high frequency where ground effect is small).

### Pitfall 6: Baking the 2009 reference values in as immutable truth
**What goes wrong:** The 2010 revision changed some results (that's why it exists); when the 20100610 files are obtained, silently mixed references corrupt the pass/fail history.
**How to avoid:** `reference_version` field ("force-2009" / "force-2010") in `CaseDefinition` and in every report line; fetch script pins SHA-256 of the downloaded .xls.
**Warning signs:** Reference files in the repo without provenance metadata.

### Pitfall 7: Committing copyrighted material
**What goes wrong:** AV 1106/07 PDF or FORCE .xls land in git; repo can never be safely published; violates the PROJECT.md licensing constraint.
**How to avoid:** `refs/` in `.gitignore` from the first commit; fetch script + checksums are committed, artifacts are not. The .xls reference VALUES extracted into fixture files are method-defined numbers (facts), fine to commit in small case-fixture form with citation — but prefer loading from the git-ignored .xls to keep the boundary crisp.

## Code Examples

### FORCE .xls loading (calamine, layout verified against the real file)

```rust
// Source: cell layout verified by parsing TestStraightRoad.xls (2009-02-16, Birger Plovsing)
use calamine::{open_workbook, Reader, Xls, Data};

pub fn load_straight_road(path: &Path) -> anyhow::Result<Vec<CaseDefinition>> {
    let mut wb: Xls<_> = open_workbook(path)?;
    let sheet_names = wb.sheet_names().to_owned();       // "1" … "62"
    sheet_names.iter().map(|name| {
        let r = wb.worksheet_range(name)?;
        let num = |row: u32, col: u32| -> f64 {
            match r.get_value((row, col)) { Some(Data::Float(v)) => *v, _ => f64::NAN }
        };
        // propagation block col A(0)/B(1): hr row12, t0 row13, z0 row14, zu row15,
        // u row16, phi row17, su row18, dt/dz row19, sdt/dz row20, Cv2 row21, Ct2 row22 (0-based; verify at load with label cells)
        // results: LAeq F6/G6, LAE F7/G7, LAmax F8/G8; spectrum rows 13..39 cols F..I
        // terrain: rows 25.. cols A..D until zero-padding
        // IMPORTANT: read label cells (col A / col F strings) and match by label, not row index,
        // to survive the 2010-revision layout drift.
        todo!()
    }).collect()
}
```

(Cell coordinates above are indicative from this session's dump; the loader MUST anchor on label strings — `"hr"`, `"t0"`, `"Freq."`, `"X"` — as the robust key, with row positions as a fallback.)

### Per-band tolerance comparison + report row

```rust
// Source: tolerance rules from Env. Project 1335 §6
pub struct BandDeviation { pub band: usize, pub nominal_hz: f64, pub got_db: f64, pub want_db: f64, pub dev_db: f64 }

pub fn compare_spectrum(got: &[f64], want: &[f64], tol_db: f64) -> (Vec<BandDeviation>, bool) {
    let devs: Vec<_> = got.iter().zip(want).enumerate()
        .map(|(i, (&g, &w))| BandDeviation { band: i, nominal_hz: NOMINAL_THIRD_OCT[i], got_db: g, want_db: w, dev_db: g - w })
        .collect();
    let pass = devs.iter().all(|d| d.dev_db.abs() <= tol_db);
    (devs, pass)
}
```

### Dynamic FORCE tests (libtest-mimic)

```rust
// Source: libtest-mimic 0.8 README pattern
// envi-harness/tests/force.rs   ([[test]] name="force", harness=false in Cargo.toml)
use libtest_mimic::{Arguments, Trial};

fn main() -> std::process::ExitCode {
    let args = Arguments::from_args();
    let mut trials = Vec::new();
    for case in envi_harness::cases::discover("refs/") {           // xls + cases/*.toml
        trials.push(Trial::test(case.id.clone(), move || {
            match envi_harness::run_case(&case) {
                Outcome::Pass => Ok(()),
                Outcome::Skipped(why) => Err(format!("SKIP (expected): {why}").into()), // or Trial::with_ignored_flag
                Outcome::Fail(report) => Err(report.render_table().into()),
            }
        }).with_ignored_flag(case.requires_unbuilt_capabilities()));
    }
    libtest_mimic::run(&args, trials).exit_code()
}
```

`with_ignored_flag(true)` gives exactly the "fails meaningfully before propagation code exists" behavior: `cargo test` shows every FORCE case as `ignored: requires emission-model,ground-effect`, and free-field TOML cases run for real. Flip capabilities on as later phases land — no harness rewrite.

### Unit anchors (approx)

```rust
// Source: anchors computed & cross-verified in research session 2026-07-07
use approx::assert_relative_eq;

#[test]
fn divergence_100m() {
    assert_relative_eq!(divergence_db(100.0), -50.992099, epsilon = 1e-6);
}
#[test]
fn iso9613_published_anchors_20c_70rh() {   // ISO 9613-2 Table 2 (rounded published values)
    assert_relative_eq!(alpha_db_per_km(1000.0, 20.0, 70.0), 5.0, max_relative = 0.02);
    assert_relative_eq!(alpha_db_per_km(4000.0, 20.0, 70.0), 22.9, max_relative = 0.02);
    assert_relative_eq!(alpha_db_per_km(8000.0, 20.0, 70.0), 76.6, max_relative = 0.02);
}
#[test]
fn relaxation_frequencies_force_conditions() {  // 15 °C, 70 % RH, 1 atm
    assert_relative_eq!(f_r_oxygen(15.0, 70.0, 1.0), 36332.37, max_relative = 1e-4);
    assert_relative_eq!(f_r_nitrogen(15.0, 70.0, 1.0), 333.6691, max_relative = 1e-4);
}
#[test]
fn band_correction_20db() {
    assert_relative_eq!(band_attenuation_db(20.0), 19.38918, epsilon = 1e-4);
}
#[test]
fn half_wavelength_paths_cancel() {   // proves complex phase is real from day 1
    let f = 1000.0; let c = 20.05_f64 * (15.0 + 273.15).sqrt();
    let r1 = 100.0; let r2 = r1 + c / f / 2.0;
    let h = unit_amp_phasor(r1, f, c) + unit_amp_phasor(r2, f, c);
    assert!(h.norm() < 1e-10);
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| FORCE PDFs at forcetechnology.com/-/media/... | Site rebuilt (Next.js), old media URLs 404 | between 2024 and 2026 | Fetch via Wayback Machine snapshots (all key reports archived, verified 200) |
| Env. Project 1022/2005 original test cases | 1276/2009 corrected → **1335/2010 revised** (`*_20100610.xls`) | 2009, 2010 | 2009 .xls online at mst.dk; 2010 .xls must be requested from FORCE — track `reference_version` |
| ndarray 0.15/0.16 (older tutorials) | ndarray 0.17.2 (num-complex 0.4, approx 0.5) | 0.17 line current in 2026 | Use 0.17 APIs; older blog snippets may not compile |
| calamine pre-0.30 API (`open_workbook_auto`, `Range<DataType>`) | 0.36: `Data` enum, `worksheet_range` returning `Result` | ongoing | Follow current docs.rs, not old examples |

**Deprecated/outdated:** nothing else relevant; the physics documents are frozen (2014 rev. of AV 1106/07 + 2018 amendments doc for later phases).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Nordtest impedance classes B=31.6, C=80, E=500, F=2000, H=200000 kNs·m⁻⁴ (A=12.5, D=200, G=20000 are verified) | Scene types | Low for Phase 1 (impedance unused in free field); confirm from AV 1106/07 §5.3/User's Guide before Phase 2 |
| A2 | A-weighting formula constants (IEC 61672-1) from training knowledge; self-check A(1000)=0 passes | Physics §5 | Very low — formula is ubiquitous; harness totals would be uniformly off and caught by case comparison |
| A3 | Road source line at x = 2.5 m (middle of nearest 5 m lane) and sub-source heights 0.01/0.30/0.75 m apply to the straight-road cases | FORCE semantics | Only affects Phase 4 full-case runs; heights verified in AAS2011 paper, lane geometry from Env. Project 1335 text — the exact x offset should be re-verified when implementing emission |
| A4 | calamine 0.36 reads this specific BIFF file (verified via xlrd, same format family; calamine xls support is mature) | Stack | Low; fallback = one-time conversion of .xls→.csv via Python script committed to tools/ |
| A5 | Applying the Eq. 287 band correction at 1/12-octave points (rather than 1/3) is acceptable for the finer grid | Physics §4 | Correction is derived for 1/3-oct filters; at 1/12 resolution the true band correction is smaller. Deviation only matters at extreme α·R; flag for a Phase 4 decision (could scale the correction or apply per-1/3 aggregation) |

## Open Questions

1. **Where to get the 2010-revised spreadsheets (`TestStraightRoad_20100610.xls` etc.)?**
   - What we know: Env. Project 1335 (2010) documents them; not found on mst.dk, forcetechnology.com (old site archived, no .xls ever archived), or the open web.
   - What's unclear: whether FORCE still distributes them on request (they historically supplied software/test data to implementers).
   - Recommendation: proceed on the 2009 set now (deltas are algorithm-level refinements, mostly relevant to screens/refraction cases); user emails FORCE for the 2010 set in parallel; harness carries `reference_version` so swapping is a data change.

2. **What exactly is "the standard's tolerance" for the Phase 1 free-field gate (success criterion 3)?**
   - What we know: the FORCE tolerance (1 dB overall / 1 dB per band) applies to full cases; free field has no published per-case reference.
   - Recommendation: planner should set: analytic divergence identity ≤ 1e-9 dB; ISO 9613-1 vs published ISO 9613-2 Table 2 anchors ≤ 2 % (table rounding); internal regression anchors (this doc) ≤ 1e-12 relative. That is stricter than the FORCE 1 dB and unambiguous.

3. **1/12-octave evaluation is an ENVI extension — how to report against Nord2000's native 1/3-octave?**
   - What we know: grid contains all exact 1/3 centres (index pick, every 4th point); Nord2000 band level = value at centre frequency, so the pick IS the band value under Nord2000 semantics.
   - Recommendation: comparator always reports in 27-band 1/3-oct space via the pick; keep the full 105-point spectrum as the engine artifact. No aggregation function needed in Phase 1 (decide energy-averaging vs pick only if Phase 4 cross-validation demands it).

4. **Roughness column semantics (`Roughn.` in m; classes N/M).**
   - What we know: class N = 0 (all Phase-1-relevant cases); report mentions "roughness class N/M" for elevated-road groups.
   - Recommendation: store as `f64` per segment now; resolve the class↔value table (AV 1106/07 §5.3.1/terrain roughness section) during Phase 2 planning.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain (rustc/cargo) | everything | ✓ | 1.96.0 (2026-05-25) | — |
| git | commits/tags | ✓ | 2.54.0.windows.1 | — |
| Internet → mst.dk / web.archive.org | refs fetch script | ✓ (downloads verified this session) | — | artifacts already in session scratchpad; re-download any time |
| Python 3 + pypdf/xlrd | optional tooling (one-off inspection/conversion) | ✓ | 3.13.13 | not required by the build |
| GDAL/PROJ C libs | — (v2 only) | not probed | — | explicitly NOT a Phase 1 dependency |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** none blocking. Note: the 2010-revised .xls files are a *data* gap (Open Question 1), not an environment gap.

## Security Domain

Phase 1 is an offline computation library + test harness: no network, no auth, no persistence, no untrusted users. ASVS applicability is minimal and honest:

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | — |
| V3 Session Management | no | — |
| V4 Access Control | no | — |
| V5 Input Validation | yes (narrow) | Treat case files as untrusted input: calamine is pure Rust (memory-safe); loader must reject NaN/Inf/negative-where-invalid values, enforce ascending profile x, bound array sizes; engine constructors validate domains (R > 0, RH ∈ [0,100], T > −273.15) and return typed errors, never panic on data |
| V6 Cryptography | no (checksums only) | SHA-256 pinning of fetched reference artifacts (integrity, not security-critical) |

### Known Threat Patterns for offline Rust lib

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Malicious/corrupt spreadsheet causing panic/OOM in loader | DoS (Tampering) | calamine + explicit size/row caps; loader fuzzable later; `Result` everywhere, no `unwrap` on cell data |
| Supply-chain (crate) compromise | Tampering | All 9 crates legitimacy-checked (table above); `cargo update` discipline; optionally add `cargo-deny` in CI (advisories + license) |
| Committing copyrighted refs | (compliance, not STRIDE) | `.gitignore refs/` from first commit; fetch script pattern (Pitfall 7) |

## Sources

### Primary (HIGH confidence — documents obtained and read this session)
- AV 1106/07 rev. 4 (Plovsing, DELTA/FORCE, 2014) via Wayback `web/20240221070539` — Eq. (1)/(329) compound model, Eq. (330) divergence, Eq. (286)/(287) air absorption, Eq. (335) Coft, §5.3.1 terrain input contract, §5.1 screens-into-profile
- AV 1849/00 Part 1 (mirror: magasbakony.hu) — air-absorption band correction Eq. (3), glyph-position-resolved; ISO 9613-1 usage notes
- Env. Project 1335/2010 "Revised test cases…" (mst.dk PDF) — suite structure, groups, tolerances (§6), parameter semantics
- `TestStraightRoad.xls` (mst.dk, Env. Project 1276/2009) — downloaded, parsed; cell layout, full-precision references, 62 sheets
- Numerical cross-verification: ISO 9613-1 transcription vs ISO 9613-2 Table 2 published values (≤ rounding); band-correction vs two documented behavior points
- crates.io API + `cargo search` — all crate versions; `gsd-tools package-legitimacy` — all OK

### Secondary (MEDIUM confidence)
- "Traffic Noise Prediction with Nord2000 — An Update" (AAS 2011, acoustics.asn.au p110.pdf, downloaded) — road source model: sub-source heights, 80/20 power splits, Jonasson 2006 tables
- Env. Project 1276/2009 HTML index (mst.dk) — .xls download URLs
- IEC 61260-1:2014 midband rules — via iTeh preview PDF + NI standards documentation (consistent)

### Tertiary (LOW confidence, flagged)
- Nordtest impedance class table beyond A/D/G (Assumption A1)
- JASA 2023 validation paper (pubs.aip.org, abstract only — paywalled; not needed for Phase 1)

## Metadata

**Confidence breakdown:**
- FORCE suite format & sourcing: HIGH — actual files downloaded and parsed
- Direct-path & air-absorption physics: HIGH — primary-source equations with independent numerical cross-checks
- Frequency framework: HIGH — IEC rule cited, grid computed, alignment property proven numerically
- Rust stack: HIGH — versions and compatibility verified against crates.io manifests
- Scene-model design details (impedance class table completeness, roughness classes): MEDIUM — Phase 2 concerns, flagged in Assumptions
- Road source model details: MEDIUM — verified from a conference paper, primary tables (SP 2006:12) not yet fetched (Phase 4 need)

**Research date:** 2026-07-07
**Valid until:** ~2026-08-07 for crate versions (30 days); the physics sources are frozen standards — indefinitely valid; re-check mst.dk link availability at execution time (files also retrievable via Wayback)
