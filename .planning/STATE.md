---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
current_phase: 11
current_phase_name: results-fast-recalc
status: executing
stopped_at: Completed 11-07-PLAN.md
last_updated: "2026-07-12T14:53:14Z"
last_activity: 2026-07-12
last_activity_desc: "11-07 interactive conditioning fast-recalc (WEB-05/SVC-06) complete: per-source Gain/Delay + reused SpectrumEditor Filter (D-11) drives a ~150 ms debounced recondition MAC (D-10) over the cached tensor — spectra + isophone map update live with NO re-propagation (calc idle, map re-contours the cached grid SC3); the D-12 results-stale badge appears the moment the re-minted blake3 identity diverges (conditioning never stales, D-07); a mismatched-hash MAC is refused with the honest 409 reject banner (SVC-06), never silently served; offline Playwright UAT on the real bundle + real WASM MAC green"
progress:
  total_phases: 11
  completed_phases: 10
  total_plans: 64
  completed_plans: 60
  percent: 91
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-07-07)

**Core value:** A numerically faithful Nord2000 engine — validated against the FORCE road-traffic test cases — that produces correct per-band outdoor sound levels over GIS terrain.
**Current focus:** Phase 11 — results-fast-recalc

## Current Position

Phase: 11 (results-fast-recalc) — EXECUTING
Plan: 7 of 11 complete
Status: Executing Phase 11 — waves 1–3 complete + wave 4 in progress: 11-06 (isophone fill layer + colour-scale editor) + 11-07 (conditioning fast-recalc, WEB-05/SVC-06) complete. Wave 4 remaining: 11-09 export UI
Last activity: 2026-07-12 — 11-07 interactive conditioning fast-recalc (WEB-05/SVC-06): per-source Gain/Delay + reused SpectrumEditor Filter (D-11) drives a ~150 ms debounced recondition MAC (D-10) over the cached tensor — spectra + isophone map update live with NO re-propagation (calc idle, map re-contours the cached grid SC3); the D-12 stale badge appears on identity divergence (conditioning never stales, D-07); a mismatched-hash MAC is refused with the honest 409 reject banner (SVC-06); offline Playwright UAT on the real bundle + real WASM MAC green

Progress: [██████████] Phase 10 — 6/6 plans complete (10-01 compute core · 10-02 COOP/COEP · 10-03 wasm boundary+OPFS sink · 10-04 pool+worker · 10-05 CalcPanel+Playwright · 10-06 real solve seam closed)

## Performance Metrics

**Velocity:**

- Total plans completed: 24
- Average duration: ~28min
- Total execution time: ~6 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1 | 3 | 77min | 26min |
| 9 | 6 | - | - |
| 10 | 6 | - | - |

**Recent Trend:**

- Last 5 plans: 55min, 34min, 17min, 55min, 30min
- Trend: —

*Updated after each plan completion*
| Phase 01 P01 | 25min | 3 tasks | 17 files |
| Phase 01 P02 | 17min | 3 tasks | 12 files |
| Phase 01 P03 | 35min | 3 tasks | 14 files |
| Phase 02 P01 | 55min | 3 tasks | 14 files |
| Phase 3 P1 | 95min | 3 tasks | 21 files |
| Phase 3 P02 | 34min | 3 tasks | 8 files |
| Phase 4 P01 | 55min | 3 tasks | 6 files |
| Phase 04 P05 | 35min | 2 tasks | 6 files |
| Phase 04 P02 | 55min | 3 tasks | 12 files |
| Phase 04 P04 | 25min | 3 tasks | 12 files |
| Phase 05 P01 | 25min | 3 tasks | 10 files |
| Phase 05 P02 | 13min | 2 tasks | 5 files |
| Phase 05 P03 | 23min | 3 tasks | 13 files |
| Phase 06 P01 | 11min | 3 tasks | 8 files |
| Phase 06 P02 | 19min | 3 tasks | 7 files |
| Phase 06 P03 | 13min | 3 tasks | 13 files |
| Phase 06 P04 | 25min | 3 tasks | 12 files |
| Phase 07 P01 | 35 min | 2 tasks | 3 files |
| Phase 07 P02 | ~25 min | 2 tasks | 4 files |
| Phase 07 P05 | 4min | 2 tasks | 12 files |
| Phase 07 P03 | 13min | 3 tasks | 9 files |
| Phase 07 P06 | 16min | 3 tasks | 10 files |
| Phase 07 P04 | 40min | 2 tasks | 13 files |
| Phase 07 P07 | 21min | 4 tasks | 25 files |
| Phase 07 P08 | 21min | 3 tasks | 20 files |
| Phase 07 P09 | 42min | 4 tasks | 15 files |
| Phase 07 P10 | 17min | 3 tasks | 15 files |
| Phase 08 P01 | 30min | 2 tasks | 6 files |
| Phase 08 P02 | ~55 min | 3 tasks | 22 files |
| Phase 08 P03 | ~18 min | 2 tasks | 7 files |
| Phase 08 P04 | 28min | 3 tasks | 8 files |
| Phase 08 P06 | 70min | 2 tasks | 13 files |
| Phase 08 P07 | 165min | 3 tasks | 18 files |
| Phase 08 P08 | 150 | 2 tasks | 13 files |
| Phase 09 P01 | 45min | 2 tasks | 10 files |
| Phase 09 P02 | 40min | 2 tasks | 5 files |
| Phase 09 P03 | 40min | 2 tasks | 12 files |
| Phase 09 P04 | 55min | 3 tasks | 12 files |
| Phase 09 P05 | 50min | 2 tasks | 10 files |
| Phase 09 P06 | 12min | 1 task | 4 files |
| Phase 10 P01 | ~45min | 3 tasks | 12 files |
| Phase 10 P02 | ~20min | 2 tasks | 4 files |
| Phase 10 P03 | 95min | 3 tasks | 13 files |
| Phase 10 P04 | 40min | 3 tasks | 11 files |
| Phase 10 P06 | 90min | 3 tasks | 17 files |
| Phase 10 P05 | 150 | 2 tasks | 6 files |
| Phase 11 P03 | ~40min | 2 tasks | 6 files |
| Phase 11 P05 | 90 min | 3 tasks | 19 files |
| Phase 11 P04 | ~40min | 2 tasks | 12 files |
| Phase 11 P06 | 18 min | 3 tasks | 14 files |
| Phase 11 P07 | 17 min | 3 tasks | 12 files |

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- Milestone 1 = validated core engine only (no map/UI/GIS ingestion); geometry fed from FORCE test-case files
- FORCE test harness (VAL-01) lands before any propagation code — Phase 1, plan 01-01
- 1/12-octave complex evaluation designed in from the first propagation phase, not retrofitted; output contract = `H[sub_source, receiver, freq]` complex tensor (Phase 4)
- Numerics guarded from the start: f64 throughout, ξ singularity clamps, cancellation-safe Δτ reformulation (Phase 3)
- [Phase 1]: Harness-before-physics enforced via capability gating: implemented_capabilities() empty, so all 66 FORCE/TOML cases report Skipped(requires: …) until later plans flip flags
- [Phase 1]: I/O quarantine: envi-engine depends only on ndarray/num-complex/thiserror; all .xls/TOML parsing lives in envi-harness
- [Phase 1, 01-02]: FORCE source line at x=2.5 m and receiver at the last profile X give horizontal distance 97.5 m (NOT 100); hSv/hRv (Z above first/last profile point) encoded solely in TerrainProfile::endpoints
- [Phase 1, 01-02]: Ground row→segment impedance rule = "row that STARTS the segment"; verified against the MIXED-impedance case 1 (road σ=20000 + grass σ=12.5) — the planned "all class A" assumption was wrong, authoritative .xls wins (Pitfall 1)
- [Phase 1, 01-02]: FORCE cases use a single placeholder SubSource (uniform-0 spectrum, height 0.0 above first point); real road sub-source heights 0.01/0.30/0.75 m are Phase 4
- [Phase 1, 01-03]: Complex transfer convention FROZEN — time e^{+jωt}, outgoing phase e^{−jωτ} with τ=R/c the carried primitive (not kR), |H|=1/√(4πR²) so L_p = L_W + 20·log10|H|; air absorption a real 10^(−ΔLₐ/20) factor; Phase 2+ effects multiply H by their complex pressure ratio
- [Phase 1, 01-03]: TransferTensor = Array3<Complex<f64>> [sub_source, receiver, freq] row-major (frequency-contiguous) — the Phase 4 forward contract; never Fortran-order
- [Phase 1, 01-03]: band_attenuation_db (Eq.287, 300 dB clamp) is the SOLE alpha·R→band converter (Pitfall 4); applied at all 105 grid points as band centres (Assumption A5, revisit Phase 4)
- [Phase 1, 01-03]: Free-field gate = strict 1e-9 dB analytic identity (harsher than FORCE 1 dB); independent dB-domain oracle (compare::analytic_freefield_reference) validates the complex roundtrip, not just the formulas
- [Phase 1, 01-03, Rule 1]: Eq.335 with coeff 20.05 gives c(15°C)=340.348 m/s (not the RESEARCH parenthetical 340.29 which uses ≈20.047); the mandated formula is the frozen phase-τ contract, so the test anchor was corrected to the formula's value
- [Phase 2, 02-01]: Nord2000-native modules quarantine the e^{−jωt} convention (special/ground/rays/coherence/terrain_effect) — the single conj() to ENVI's e^{+jωt} lands in transfer.rs in plan 02-05; no conj() in propagation modules
- [Phase 2, 02-01]: |Q̂| is NEVER clamped — |Q̂|=1.2572 at σ=200/f=250 is correct surface-wave physics, pinned by a test (anti-pattern guard)
- [Phase 2, 02-01]: Δτ via the cancellation-free identity ΔR = 4·hS·hR/(R₁+R₂) (flat) / image-point dot-product form (sloped) — the only Δτ path; hS=0.01/d=1000 regression
- [Phase 2, 02-01]: impedance_class B corrected 31.6→31.5 (Table 2 verified — resolves Phase 1 Assumption A1); all eight classes now verified
- [Phase 2, 02-01, Rule 1]: research w(7+1j) anchor (0.019924+0.139158j) is a transcription error — true wofz(7+1j)=0.011630+0.079732j (asymptote-confirmed); engine matches scipy, not the mistyped anchor
- [Phase 2, 02-01, Rule 3]: ground_impedance returns Result (σ≤0 typed error, T-02-01) and CoherenceInputs gained d_m (Fc needs distance) — both documented interface-block extensions
- [Phase 2, 02-01]: committed scipy oracle (tools/nord2000_oracle) generates ground_w_qhat.toml fixtures (sha256 provenance); Rust tests run without Python. Oracle w-tolerance 3e-6 (three-pole near-border intrinsic error ~2.5e-6, not a bug)
- [Phase ?]: [Phase 3, 03-02] Eq. 26 fL middle branch = (43−3·Δc₁₀)/40 with knots at Δc₁₀=1 and 5, resolved by C⁰-continuity (pdftotext dropped the minus sign)
- [Phase ?]: [Phase 3, 03-02] calc_eq_ssp_ground takes σ (sigma_kpa) not a precomputed Ẑ_G — phase_diff_freq owns the Delany–Bazley impedance and needs σ to sweep the 1/3-oct bracket
- [Phase ?]: [Phase 3, 03-02] Route-2 A/B scaling constants are [ASSUMED] (validated by direction property tests only, no false FORCE numeric pass); B factor C/(2·(T₀+273.15)) is the exact Coft derivative; locked pending 03-03 Open-Q1
- [Phase ?]: [Phase 3, 03-02] Nord2000 nonzero-turbulence default added as a turbulence_or_nord2000_default accessor seam, NOT wired into build_terrain_inputs, so the frozen zero-turbulence terrain oracle fixtures stay untouched
- [Phase 3, 03-03] Open-Q1 checkpoint resolved as option (c): proceed with the [ASSUMED] weather-route A/B/C constants clearly quarantined — validated by structural + direction property tests and the same-transcription committed oracle ONLY; NO false FORCE numeric Pass; wind/gradient FORCE cases stay Skipped(requires: emission-model) until Phase 4
- [Phase 3, 03-03] F_τ (Eq. 112) uses the full 2π·f·|Δτ⁺−Δτ| argument (NOT 0.23π — Pitfall 5), injected through the pre-built CoherenceInputs::f_delta_nu seam with zero call-site change; sA=sB=0 ⇒ Δτ⁺=Δτ ⇒ F_τ=1 bit-exact; multiplies H_coh without overwriting the +j phase (D-12); property-tested only (no fixed oracle, D-11)
- [Phase 3, 03-03] Route 3 LSQ = hand-rolled 3×3 normal equations (Cramer + singular guard), NO nalgebra/ndarray-linalg (D-08); SoundSpeedProfile gained s_a/s_b so the engine can form the Eq. 10 A⁺/B⁺ profile; Capability::Refraction flipped ⇒ FORCE wind/gradient skip-reason shrinks to emission-model only
- [Phase 4, 04-01] Tensor is a PAIR: H_coh Array3<Complex<f64>> + P_incoh_abs Array3<f64> [sub_source, receiver, freq] row-major; P_incoh stored ABSOLUTE (|H_ff|²·p_incoh) at fill time so F→1⇒0 bit-exact and readout needs only two stores; TensorSink trait is the Phase-10 file-backed-store seam, SolveJob the Phase-9 PropagationPath seam
- [Phase 4, 04-01] MAC ≡ recompute is bit-for-bit (assert_eq! on f64::to_bits, not epsilon): compose_gain builds G_s(f)=10^{L_W/20}·filter·e^{−j2πfτ} ONCE in a frozen order, one multiply in readout_coherent (Pitfall 6); delay phase written explicitly (no .conj()); conditioning/directivity live on the ENVI post-conj side — propagation/ conj-quarantine stays at zero actual calls
- [Phase 4, 04-01] Two readout laws kept distinct: coherent MAC (OUT-03) vs incoherent Annex-A energy Σ_s w_s·(|H_coh|²+P_incoh_abs) — two identical co-located subs give +3.0103 dB, never +6 dB; 256 MiB budget proven structurally by CountingSink high-water-mark over a 100k-receiver solve (full complex tensor never resident); ZERO new engine deps
- [Phase 4, 04-01, Rule 2/3] SolveJob carries atmosphere:&Atmosphere (direct_path needs full air-absorption state; terrain_effect takes coh.c0 as in build_terrain_inputs); compose_gain returns Result (validates filter length vs N_BANDS + rejects non-finite, threat T-04-01-03) — both documented interface deviations from the plan's literal field/signature
- [Phase ?]: [Phase 5, 05-01] ENG-09 = REAL Nord2000 SM10 law (Eqs. 288-291: quadratic-then-saturating T, -15 dB floor, exact 0 below ka=0.7), NOT the TI 386 'A=d·a(f)' paraphrase — verified vs AV 1106/07 page images; SC1 satisfied by SM10's own T-saturation+floor bounding (ISO 10/20/200 m regimes excluded)
- [Phase ?]: [Phase 5, 05-01] D-01 ForestCrossing amended (research-mandated): 'kp' DROPPED (= Table 8's computed k_f, D-02), height_m ADDED (h'=nQ·h needs avg tree height, Pitfall 5); downstream-consistent with Phase-7 SCN-04
- [Phase ?]: [Phase 5, 05-01] Fs coherence factor (Eq. 288) DEFERRED with documented seam (forest.rs header + plan 05-03 deferred-items) — D-03 excess-attenuation-not-decorrelation scope; revisit Phase 9
- [Phase ?]: [Phase 5, 05-01] Table 9 interp = hand-rolled Fritsch-Carlson-Butland PCHIP matching scipy.PchipInterpolator to 1e-9 (no linalg/FFT crate), nested R'->α->log10(h'); R' clamped [0.0625,10] consistently in BOTH table lookup and 20·log10(8R') term (A3); pinned pre-extension solver bit-baseline proves forest:None byte-identical
- [Phase ?]: [Phase 5, 05-02] ENG-10+ min-phase kernel (D-06 ENVI extension): R(f)→complex T(f)=10^(−R/20)·e^{jφ_min}, φ_min via even-mirror 208-pt real-cepstrum fold on the 105-pt band axis (hand-rolled naive DFT, dep quarantine intact — no FFT crate). DFT sign PINNED numpy.fft (fwd e^{−j2πkn/M}, inv (1/M)e^{+j2πkn/M}) ⇒ fold yields ENVI lagging φ_cep, so NATIVE e^{−jωt} filter = |T|·e^{−jφ_cep} written as explicit negative sine Complex::new(mag·cosφ, mag·(−sinφ)); conj gate in propagation/ stays 0. D-08 sign pinned two ways: first-principles causality property (T3) + committed numpy oracle to 1e-9 rad on all 105 bands (T5). D-10 opaque=None structural (IsolationSpectrum rejects non-finite/negative R; no INFINITY/MAX sentinel token). Kernel-only — NOT yet reachable from any solve path (plan 05-03 threads it into screen_channel)
- [Phase ?]: [Phase 5, 05-02, Rule 1] Plan's T3 adjacency spec (φ[n+1]≤φ[n] over 30..75) was unsatisfiable — even-mirror makes φ symmetric about band 52 (φ[k]=φ[104−k]), so no rising R is monotone across mid-band (Finding 3c). Engine φ verified correct vs numpy (~1e-15); T3 reformulated to lagging-sign + rising-FLANK monotonicity (10..50) + slope-antisymmetry (φ_rise=−φ_fall) + native negation — a stronger D-08 pin, no production-code change
- [Phase 06]: envi-geo is the milestone's ONE reprojection boundary (GEOX-04) on pure-Rust proj4rs 0.1.10 — zero C toolchain (D-01/D-02); radians quarantined to transform.rs; pinned to pyproj oracle <=1e-3 m
- [Phase ?]: [Phase 06, 06-03] envi-service = the single deployable axum binary (SVC-03/04): /api/v1 + web/dist, 127.0.0.1:8080 default (warn on non-loopback ENVI_BIND), refuse-to-start on the D-08 pure-Rust CRS round-trip self-check (SC2 adjusted, GDAL deferred to Phase 8); thin handlers delegate to envi-store; freq-axis served once from envi_engine::freq (band-index wire, SVC-07); scene GET/PUT round-trips WGS84 GeoJSON and survives restart (SC1); lib+bin split so oneshot tests reach api::app
- [Phase ?]: [Phase 06, 06-04] SC5 job machine runs on a dedicated std::thread (Anti-Pattern 5/D-08, grep-gated zero spawn_blocking) with watch::channel progress + CancellationToken, bridged to SSE via WatchStream + 15s keep-alive; SC4 recondition/recompute split ENFORCES a real 409 tensor_hash_mismatch ({error,expected,got,hint} served verbatim, top-level) against the in-memory CalcRecord; D-07 conditioning-exclusion proven (same identity under any conditioning); honest all-zero [105] stubs (stub:true); tier router + registry eviction deferred to Phase 10/11
- [Phase 07]: envi-dgm TIN: added DgmError::TooLarge (5th variant) for the DoS cap mandated by threat T-07-02-02
- [Phase 07]: Breakline vertex Z interpolated from the point surface (nearest-vertex fallback outside hull) — never a silent 0.0
- [Phase 07]: 07-05: web/ frontend bootstrapped (React 19 + Vite 8 + TS strict); runtime deps vs devDependency-only tooling; metrao3 dark theme tokens copied verbatim (D-11); web/dist git-tracked and served by envi-service ServeDir; static-bundle contract test asserts stable <title>ENVI + #root markers — Toolchain bootstrap the whole frontend depends on
- [Phase 07]: interpolate-spectrum endpoint delegates to the shared envi_store::interpolate core (D-05) then the engine IsolationSpectrum::new range gate: R>1000 is a 4xx, never silently clamped
- [Phase ?]: Terra Draw re-hydration after setStyle REBUILDS a fresh instance on style.load (research's clear()+addFeatures throws against maplibre adapter v1.4.1)
- [Phase ?]: Store is canonical (D-03); TD change echoes with context.origin==='api' are ignored to break the feedback loop
- [Phase ?]: TS wire types generated from Rust serde DTOs via ts-rs (D-10)
- [Phase ?]: 07-07: palette activeTool lives in the canonical store; drawn features tagged with kind via the shared commitFeature path (WEB-04 inheritance seeded there)
- [Phase ?]: 07-07: DGM producer is debounced 750ms + decoupled from the raw TD change/drag path (SC1); closed-enum selects make out-of-vocabulary impedance/roughness impossible
- [Phase ?]: 07-10: reopen-last on boot restores the last project; getLastProject maps 404/id-less to null so the boot GET is transparent to the offline test suite
- [Phase ?]: 07-10: SC1-SC4 proven by integrated offline Playwright journeys; final web/dist committed (zero external assets); envi-dgm documented with its own quarantine gate
- [Phase 08, 08-01] RD New (EPSG:28992) added to envi-geo as a sibling source type `RdNewCrs` (to_rd/to_wgs84), NOT an overload of the UTM-specific ProjectCrs — RD is the transient AHN import source CRS, reprojected to WGS84 then handed to the project ProjectCrs (GEOX-04 single boundary preserved). proj4rs sterea + Bessel + 7-param towgs84, ZERO new deps; radians stay quarantined in transform.rs. Pinned to a committed pyproj EPSG:4326↔28992 oracle (rd_landmarks.toml, sha256 provenance, tol_m read from [meta]) at ≤1.0 m round-trip — the ~0.5 m towgs84-vs-RDNAPTRANS gap sits inside tolerance; no runtime Python. Fallible constructor wraps bad proj strings into GeoError::Proj (no panic, T-08-01-02)
- [Phase 08, 08-03]: envi-service gains its ONE new network surface — an allowlisted, bytes-only GET/Range byte relay `GET /api/v1/proxy/{source}/{*path}` (D-02, Pattern 5) for the two CORS-blocked S3 sources (GLO-30, WorldCover); every other source (PDOK AHN, Overpass) stays direct browser fetch. SSRF-proof by construction: a hardcoded `SOURCES` (id, host, path_prefix) table + a pure `resolve_upstream()` that rejects unknown source (404), prefix escape, and `..` traversal (400) BEFORE any outbound request; the shared `reqwest::Client` follows NO redirects (`Policy::none()`), has a connect timeout, and streams the body under a 128 MiB cap. Pure-Rust TLS (`reqwest` rustls-tls, default-features=false — no native-tls/openssl in the graph). MED-1: `From<reqwest::Error>` logs the full error server-side but returns a generic 500 (no host/path leak). GET-only via `get(relay)` (405 otherwise). Pinned by offline `contract_proxy.rs` (unknown-source/prefix-escape/non-GET router cases + resolve_upstream URL-builder unit cases); no test hits the network. — The single server-side surface of the client-side import pipeline; keeps "compute on the local machine" (bytes-only, no transform).
- [Phase 08]: envi-gis is the sans-I/O, WASM-safe GIS-ingestion boundary: decodes cached COG/BigTIFF over &[u8] (tiff crate), NO network/OPFS/browser deps (cargo tree gate); guard-first decode_window enforces a pre-decode max_decoded_px DoS budget from IFD dims (T-08-02-01), geotransform from ModelPixelScale/Tiepoint not nominal (T-08-02-04), nodata + non-finite dropped to Option holes never silent 0.0 (T-08-02-03). Fixtures are real GDAL 3.12.1 COGs (dev-time rasterio), Python not a test dep; envi-engine byte-identical. — Load-bearing security-critical foundation of the client-side import pipeline; sans-I/O keeps the whole core natively cargo test-able and WASM-ready.
- [Phase 08, 08-04]: envi-gis feature layer is deterministic pure data + transforms. registry = SourceDescriptor table-as-data (D-04): AHN4 DTM inside a coarse WGS84 NL coverage hull, GLO-30 DSM globally, WorldCover + Overpass; verified D-02 CORS mode per source; committed registry/ahn_index.toml kaartblad↔RD index (DO-NOT-EDIT + sha256 banner, include_str!, parsed only in tests). terrain: decimate_window grid-strides to ≤target and ≤MAX_TERRAIN_POINTS=50k (T-08-04-01), dropping nodata holes; terrain_features reprojects RD/WGS84→WGS84 through envi_geo ONLY (grep proj=0, GEOX-04) and emits editable elevation_point features WGS84-on-the-wire with NO Rust id (TS assigns crypto.randomUUID); sample_base_elevation = footprint-boundary MEDIAN, never DSM-under-roof (T-08-04-04), typed None when terrain absent (never silent 0.0, D-07). impedance_table = 11-row reviewed WorldCover→Nord2000 class table; per-row test resolves σ via envi_engine::scene::impedance_class — σ NEVER restated in envi-gis (only in a //! doc comment); roughness defaults to class N. buildings: Overpass ways+multipolygon relations → building features with the LOCKED D-10 height chain (measured reserved → height/building:height tag tolerant-parse rejecting non-finite/negative → building:levels×3+1.5 → user default), emitting eaves_height_m (the exact key building_from_feature reads, NOT a parallel height_m) + height_provenance + D-11 provenance; ring validation + skip-and-report (T-08-04-02, never fails the layer). merge = D-09 re-import keyed on (source, source_ref) with the user_modified guard (user edits survive, untouched refresh, new added, absent retained, user-created kept), deterministic order. provenance = plain GeoJSON properties (Pattern 4, zero store schema change). GisError gained Reproject + Json variants (additive; nothing outside envi-gis matches GisError). No new deps (T-08-04-SC). 28 unit + 8 integ tests green; clippy/fmt clean.
- [Phase 08, 08-05]: envi-gis/landcover.rs vectorizes a WorldCover Raster<u8> window into editable WGS84 ground_zone polygons — one per contiguous same-class region. SUS `contour` crate DECLINED at the pre-resolved human-verify checkpoint (T-08-05-SC): hand-rolled marching squares instead, ZERO new deps (only the already-present `geo`); cargo tree confirms no contour/reqwest/tokio/web-sys. Tracer = directed interior-on-right unit half-edges stitched into closed rings with sharpest-right-turn saddle resolution → a per-class partition yielding adjacency/containment ONLY, never partial crossings (T-08-05-01, Phase-7 draw rule; asserted via geo::Relate is_overlaps()). Exteriors (signed area>0) + holes (area<0) → nested Polygons; min-area drop + geo Douglas–Peucker BOTH in pixel space (CRS-independent tol); bounded-work caps MAX_GROUND_ZONES/MAX_TOTAL_VERTICES. Impedance letter via worldcover_to_class (σ stays in engine — one source of truth); roughness defaults N; D-11 provenance; no Rust id. Water (WC 80→H) emitted as zones (Open-Q5); unknown codes + nodata skipped, never a silent zone (D-07). WorldCover is EPSG:4326 → geotransform yields WGS84 directly (identity via envi_geo::LonLat, grep proj=0). 9 unit tests; full workspace green, engine byte-identical. — NOTE: consumes a Raster<u8>; the WorldCover u8-COG decode path (class-raster producer) is the remaining upstream seam (cog::decode_window currently handles f32 terrain only).
- [Phase 08]: [Phase 08, 08-06] envi-gis-wasm = the repo's first WASM crate: a thin wasm-bindgen cdylib exposing the pure envi-gis core (plan_import/decode_window/terrain_features/sample_base_elevation/map_landcover/parse_buildings/merge_features) to the browser — marshalling only, all GIS math delegated to envi_gis::. wasm-bindgen pinned '=0.2.126' with wasm-bindgen-cli 0.2.126 lockstep (Pitfall 8); no getrandom/uuid (Pitfall 9). Tile bytes cross as a direct &[u8] param (not a serde field); provenance built by resolving the registry source id -> 'static source+license. Closed the 08-05 u8-seam (cog::decode_window_u8) + added terrain::base_elevation_on_raster so the boundary stays logic-free. All 24 boundary DTOs generated via ts-rs into the SINGLE committed web/src/generated/wire.ts (envi-service dev-deps envi-gis-wasm; no-drift test green) — one source of truth for the HTTP wire AND the WASM boundary, no hand-written TS. DATA-01/02/03.
- [Phase 09]: GEOX-01 samples the DGM TIN (barycentric), not the raw raster; the r.profile oracle pins a documented TIN-vs-bilinear tolerance, not bit-equality
- [Phase 09]: GEOX-02 segment_ground returns GroundSegmentation { points, planar_xy, segments } so boundary-spliced points stay in sync with segments for a valid TerrainProfile
- [Phase ?]: GEOX-03 injects screen tops as (x,z) vertices into the TerrainProfile (no separate screens field); through-building = two tops + hard-sigma span, engine caps diffraction at <=2 screens
- [Phase ?]: GRID-01 emits a regular lattice clipped to calc_area minus footprints (build_tin used only as the intersecting-ring validity guard); receiver z from the DGM TIN, acoustic height added at SolveJob assembly
- [Phase 09, 09-03] METX-01: the Phase-3 weather LSQ (3×3 Cramer fit_profile) + WeatherComponents/WeatherProfile/profile_for_bearing/ReflectionProfiles LIFTED verbatim into WASM-safe envi_gis::weather (single source of truth); envi-harness now depends on envi-gis, re-exports the types (weather/mod.rs) and DELEGATES fit_profile (route3.rs maps GisError→CaseLoadError) so every Phase-3 call site + test is unchanged — no duplicate LSQ, engine still exactly 3 deps, no async/network edge in envi-gis
- [Phase 09, 09-03] METX-01 components_from_levels uses the validated Route-2 [ASSUMED] SEPARABLE model (a_temp=0, linear temperature gradient→B/C, neutral-log-law→a_wind), NOT a naive 3-param fit of both terms: real Open-Meteo pressure levels are sparse/≥90 m AGL so the [ln(z),z,1] basis is ill-conditioned (singular) — the separable model is well-conditioned and physically matches route2. AMSL geopotential→AGL via elevation subtract; near-surface 2m/10m anchor conditions/enriches the fit. Structural/direction/round-trip tests ONLY on committed openmeteo_{archive,forecast}.json — [ASSUMED] quarantine intact, NO false FORCE pass
- [Phase 09, 09-03] METX-02: envi_gis::era5 obukhov (1/L from iews/inss/ishf/2t/2d/sp; downward-positive ⇒ daytime unstable/night stable) + occurrence_stats (wind×stability class-occurrence table + sdfor reliability). Occurrence statistics ONLY (D-05) — NO class→A/B/C, NO L_den (deferred GRID-03). Committed era5_synthetic.toml with an INDEPENDENT-recipe 1/L oracle + exact class counts; named [ASSUMED] bin edges; new GisError::{WeatherFit, Era5Field}
- [Phase ?]: 10-02: COOP same-origin + COEP credentialless (NOT require-corp) on the envi-service bundle + Vite dev server — cross-origin isolation for SharedArrayBuffer while preserving Phase-8 direct GIS/basemap fetches
- [Phase ?]: [Phase 10, 10-03] envi-compute-wasm cdylib: thin boundary (estimate_cost/plan_tiers) over pure envi-compute core + OpfsChunkSink (NEW impl of engine TensorSink; frozen [s][r_local][f] interleaved-LE chunk format over FileSystemSyncAccessHandle, worker-only; byte-exact round-trip vs InMemorySink). Engine byte-identical.
- [Phase ?]: [Phase 10, 10-03] Threaded-wasm build scoped to ONE npm script: cargo +nightly-2026-07-11 -Zbuild-std + inline --config atomics + off-by-default 'threads' feature gating wasm-bindgen-rayon/rayon — no rust-toolchain.toml, no .cargo/config.toml (Pitfall 1). Verified end-to-end; stable gis build untouched.
- [Phase ?]: [Phase 10, 10-03] JobStatus reused verbatim from existing wire.ts (envi-service, D-10) client-side; no duplicate re-derivation. TierComplete D-07 event + cost/tier DTOs ts-rs-generated into the single committed wire.ts (no-drift green).
- [Phase ?]: 10-04: rayon native-normal + wasm-optional (threads-gated) so cargo test runs the real par_iter driver; stable wasm build stays rayon-free
- [Phase ?]: 10-04: pool::solve_tier shards disjoint ranges (one file per range at local offset 0); solve_chunk_range remains a typed seam pending a scene-context DTO
- [Phase 10, 10-01] envi-compute = the pure-Rust, WASM-safe compute core: the tensor-identity closure (tensor_hash/CalcManifest/chunk_receivers + identity DTOs + geometry_positions) factored byte-for-byte out of envi-store and re-exported (source-compatible, wire.ts byte-stable) + the SC1 cost model/Ok/Warn/Block guardrail + the hierarchical points⊂coarse⊂fine tier partition (no receiver recomputed) + the SolveJob assembly that is the FIRST site to populate SolveJob::directivity_phase_rad (SRC-03; phase-free stays bit-identical). Depends on envi-engine, adds NOTHING to its 3-dep quarantine (engine byte-identical, D-02); std::fs manifest I/O stays in envi-store
- [Phase 10, 10-06] solve_chunk_range seam CLOSED (supersedes the 10-04 typed stub): PrepareSolveReq marshals the whole transfer scene ONCE per submit into an owned PreparedScene (engine validating constructors) in a static RwLock keyed by tensor_hash; the real hash-gated, cancel-aware range-solve runs the UNCHANGED envi_engine::solver::solve rayon-sharded via pool::solve_tier into one [s][r_local][f] OPFS chunk pair — f64::to_bits-equal to a direct engine solve (forest ENG-09 + isolation ENG-10 + phase-carrying balloon SC4; two-shard == single-range; hash-mismatch typed error). WASM-safe scene DTOs factored into envi-compute, re-exported at original paths
- [Phase 10, HI-01/build-fix] OPFS tensor store keyed by the REAL blake3 marshalled tensor identity (not an ad-hoc 32-bit FNV hash) — HI-01 code-review fix. Shared-memory threaded-wasm build recipe (1099b24): build:wasm:compute emits a SHARED WebAssembly.Memory (--shared-memory --import-memory --max-memory + exported __heap_base/__wasm_init_tls/__tls_*) so wasm-bindgen-rayon initThreadPool can postMessage it to the pool workers; the in-browser threaded solve now runs (offline Playwright 21 passed)
- [Phase 11, 11-04] GRID-05 export encoders realized as three WASM byte generators (D-20/21/22): `envi_compute::export::{encode_geotiff, encode_isophone_geojson, encode_spectra_csv}` + the `envi_compute_wasm::export` `#[wasm_bindgen]` boundary → `Vec<u8>` browser-download bytes, nothing leaves the device (D-20). GeoTIFF is HAND-ROLLED (zero new dep — the `tiff`/`geotiff-writer`/`tiff-writer` crates declined per RESEARCH Package Legitimacy, same call as iso-band-vs-contour): a minimal single-strip Float32 TIFF with GeoKeyDirectory (Projected + PixelIsPoint + `ProjectedCSTypeGeoKey`=EPSG) + ModelPixelScale + ModelTiepoint + GDAL_NODATA, north-up (grid rows flipped), NaN holes preserved; round-tripped by a dev-only byte-offset reader. GeoJSON reuses the in-tree `geojson` crate (RFC-7946 MultiPolygon per band via the new `IsoBand::fill_polygons` containment classification). CSV identity = BAND INDEX + exact Hz from `FreqAxis::centres` (never nominal, Pitfall 3) + dB(A)/dB(C) totals. Every export embeds the ExportMeta footer (CRS + weighting + engine version + tensor_hash + OSM/Overture/ESA WorldCover/Copernicus attribution, D-22). SceneXY→LonLat reprojection happens ONCE in the boundary via `envi_geo::ProjectCrs::to_wgs84` (GEOX-04); `envi-geo` added to the wasm graph, builds for wasm32, engine 3-dep quarantine unchanged. Filename sanitized program-side (V12/T-11-04-02). ExportReq/ExportFormat/ExportCrsDto/ExportGridDto ts-rs-generated (no-drift green). NOTE: web/dist bundle NOT rebuilt — export UI wiring is 11-09.
- [Phase 11, 11-03] SVC-06 recondition MAC realized CLIENT-SIDE (D-01/Open Q1): `envi_compute_wasm::recondition` re-mints `marshalled_tensor_hash` from the CURRENT scene, refuses a mismatched claimed hash with a typed `ComputeError::HashMismatch { expected, got }` (mirrors the server 409 body) BEFORE any MAC — the honest client 409, never a silently-served stale readout (D-12). On a match it maps each `ConditioningDto` → (`L_W` = gain_db, complex filter = 10^{dB/20}, delay_s) and drives the FORCE-validated 11-01 `readout_receiver` (compose_gain + readout_coherent) over the OPFS-read tensor — reconditioned spectra with NO re-propagation. Law derived from composition (Open Q2): a default (no-op) conditioning reads out identically to the plain 11-01 readout, so conditioning-excluded-from-identity (D-07) means a conditioning edit never stales. `ConditioningDto` MOVED envi-store→envi-compute::readout (re-exported, wire.ts byte-stable) so the wasm boundary reuses it without dragging std::fs into wasm; `ReconditionReq`/`ReconditionResult` ts-rs-generated (no-drift green). MAC ≡ recompute bit-exact (f64::to_bits). Engine 3-dep quarantine untouched.
- [Phase 11]: trace_isophones live WASM boundary re-contours the cached grid (no re-solve, SC3); single breaks[]/colors[] source of truth drives tracer+fill+legend (legend ≡ contour ≡ class, D-02/D-04)
- [Phase 11, 11-07] WEB-05/SVC-06 conditioning fast-recalc SHIPPED (SC2): a per-source Gain(dB)+Delay(ms)+reused-SpectrumEditor Filter (D-11) drives a ~150 ms DEBOUNCED (D-10) readout_receivers MAC over the cached OPFS tensor — the receiver spectrum AND the isophone map update live with NO re-propagation (the calc job never leaves idle; the map RE-CONTOURS the reconditioned grid, SC3). D-01 held with ZERO TS acoustic math (grep gate 0): the dB→complex filter, readout law, and blake3 identity all run in WASM; the dense [105] filter is materialised SERVER-side through the reused SpectrumEditor's own interpolation. Two honest states: store/stale.ts re-mints the tensor identity of the CURRENT scene (shared compute/marshalScene marshaller, extracted from CalcPanel — single source of truth) and flips the D-12 ".chip.warn Out of date" badge on divergence; the watcher keys on the manifest tensorHash so a conditioning recalc (swaps the drive, keeps the hash) NEVER re-mints → conditioning never stales (D-07, enforced structurally). A MAC against a mismatched hash THROWS `tensor_hash mismatch` → the store sets `refuse` → the honest 409 reject banner, never a silently-served stale readout (the SVC-06 client realization). Isophone map link = re-feed setIsophoneInput with the WASM-produced per-lattice total_dba (placed by index only); the production fine-tier lattice feed stays the documented 11-05/06 follow-up. Zero Rust/wire/Cargo changes. Offline Playwright UAT on the real bundle + real WASM MAC green.

### Pending Todos

- ~~**Wire directional phase into the coherent composition path.**~~ RESOLVED
  (10-01): `envi_compute::job_assembly::assemble_jobs` is the FIRST construction
  site to populate `SolveJob::directivity_phase_rad` — `Some(balloon.eval_phase(
  dir_local))` gated on `has_phase()`, else `None` (phase-free stays bit-identical).
  Proven end-to-end (rotating a phased balloon changes the coherent sum's argument;
  a phase-free balloon leaves `arg(H_coh)` bit-identical). The full browser path
  runs this site through the wasm pool (10-03/10-04). SRC-02 shipped as real
  `ΔL(θ,φ,f)`; complex directivity is the accepted ENVI enhancement (reflect in
  REQUIREMENTS at phase close).

### Blockers/Concerns

- ~~Must obtain AV 1106/07 and the FORCE road-traffic test-case suite before Phase 1 execution~~ RESOLVED (01-01): all 4 FORCE workbooks + AV PDFs fetched into git-ignored refs/, SHA-256 pinned in refs/refs.sha256; case "1" full-precision anchor (LAeq,24h=39.39836757521) verified. Suite stays green whether or not refs are present.
- ~~Open question (Phase 3/4): how to represent Nord2000 partial-coherence (coefficient F_τ) alongside the coherent complex transfer~~ RESOLVED (03-03): F_τ (Eq. 112) multiplies the coherent H_coh_factor via the CoherenceInputs::f_delta_nu seam (phase preserved, D-12); the turbulence-decorrelated energy remains the separate real P_incoh channel; F→1 ⇒ P_incoh→0 bit-exact
- ~~Phase 10: the threaded-wasm `build:wasm:compute` artifact shipped a non-shared `WebAssembly.Memory`, so `wasm-bindgen-rayon`'s `initThreadPool` could not postMessage it to the pool workers (`#<Memory> could not be cloned`) — the in-browser threaded solve never ran, only native `cargo test` bit-equivalence had proven it~~ RESOLVED (1099b24): the build recipe now emits a SHARED memory (`--shared-memory --import-memory --max-memory` + exported `__heap_base`/`__wasm_init_tls`/`__tls_*`), the glue builds `new WebAssembly.Memory({…, shared:true})`, and the worker installs `onmessage` before the pool-init await (buffer+replay) so a same-tick submit is not dropped; offline Playwright suite 21 passed, web/dist rebuilt, engine byte-identical

## Deferred Items

Items acknowledged and carried forward from previous milestone close:

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| v2 scope | DATA, GEOX, METX, GRID, WEB, SVC, FUT requirement groups | Deferred to later milestones | 2026-07-07 (roadmap) |

## Session Continuity

Last session: 2026-07-12T14:53:14Z
Stopped at: Completed 11-07-PLAN.md — interactive conditioning fast-recalc (WEB-05/SVC-06): per-source Gain/Delay + reused SpectrumEditor Filter (D-11) drives a ~150 ms debounced recondition MAC (D-10) over the cached tensor (spectra + isophone map live, no re-propagation); D-12 stale badge on identity divergence (conditioning never stales, D-07); honest 409 reject banner on a mismatched-hash MAC (SVC-06)
Resume file: None (wave 4 continuing — 11-09 export UI menu; then wave 5 — 11-08 scenarios)
