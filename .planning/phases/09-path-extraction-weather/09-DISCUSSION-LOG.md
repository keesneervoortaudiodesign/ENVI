# Phase 9: Path Extraction & Weather - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-11
**Phase:** 9-path-extraction-weather
**Areas discussed:** Weather import UX, ERA5 groundwork scope, Receiver grid defaults, Screening-edge rules

---

## Weather import — time model (METX-01)

| Option | Description | Selected |
|--------|-------------|----------|
| Single representative hour | One date+hour → one set of per-azimuth A/B/C; windows/scenarios/worst-case are Phase 11 | ✓ |
| Window, auto-reduced | User picks a window; reduced to a representative profile now | |
| Window, store all hours | Fetch/cache every hour; defer reduction to Phase 11 | |

**User's choice:** Single representative hour
**Notes:** Keeps this phase to one concrete atmosphere; cache key = (site, timestamp). SC4's "once per (site, time window)" satisfied by caching the single-hour fetch.

## Weather import — source product (METX-01)

| Option | Description | Selected |
|--------|-------------|----------|
| Date-switched: Archive + Forecast | Historical → Open-Meteo Archive (ERA5-backed); recent/future → Forecast; same schema | ✓ |
| Forecast API only | ~last 3 months + 16-day forecast; can't model arbitrary historical night | |
| Archive API only | ERA5 reanalysis only; ~5-day lag, no near-future | |

**User's choice:** Date-switched: archive + forecast
**Notes:** Covers real historical modelling and current/near-future with one date-driven endpoint switch.

## ERA5/CDS groundwork — where the job runs + deliverable (METX-02)

| Option | Description | Selected |
|--------|-------------|----------|
| Server retrieval endpoint + derivation now; live fetch behind a flag | Pure-Rust Obukhov→class-stats derivation + fixture + async-job scaffold; thin CDS endpoint on login server, flagged off | ✓ |
| Full live CDS fetch end-to-end | Real key, real queued retrieval, live stats this phase | |
| Derivation only, no job at all | Derivation math + fixture; no CDS/network wiring | |

**User's choice:** Server retrieval endpoint + derivation now; live fetch behind a flag
**Notes:** CDS is a queued multi-minute job that can't run purely client-side → the login/delivery server is the one acceptable place for it; no CDS key needed to pass CI.

## ERA5/CDS groundwork — derivation output (METX-02)

| Option | Description | Selected |
|--------|-------------|----------|
| Occurrence stats only (Obukhov L + wind classes) | Wind×stability class occurrence counts/frequencies; L_den combination stays with GRID-03 | ✓ |
| Stats + wire into A/B/C per class | Also map each class to A/B/C now | |

**User's choice:** Occurrence stats only (Obukhov L + wind classes)
**Notes:** Full energy-weighted L_den combination stays deferred with GRID-03 per roadmap.

## Receiver grid — spacing control (GRID-01)

| Option | Description | Selected |
|--------|-------------|----------|
| User-set, default 10 m, min guardrail | 10 m typical noise-map resolution; guardrail because WASM cost scales with receivers × sub-sources | ✓ |
| User-set, default 5 m | Finer, ~4× receivers/compute | |
| Fixed spacing, no control | Hard-code one spacing; expose control later | |

**User's choice:** User-set, default 10 m, min guardrail
**Notes:** Guardrail value left to Claude/research.

## Receiver grid — building handling (GRID-01)

| Option | Description | Selected |
|--------|-------------|----------|
| Exclude interior (footprints as CDT holes) | No receivers inside footprints; grid respects footprint edges; facade points deferred | ✓ |
| Exclude interior + auto facade points | Also auto-place assessment points off facades | |

**User's choice:** Exclude interior (footprints as CDT holes)

## Screening edges — which objects screen (GEOX-03)

| Option | Description | Selected |
|--------|-------------|----------|
| Buildings + walls + barriers, all height-carrying | building rings @ eaves_height_m + wall/barrier lines @ height_m | ✓ |
| Walls + barriers only | Buildings block LOS on map but inject no diffraction edges | |

**User's choice:** Buildings + walls + barriers, all height-carrying

## Screening edges — 3D→2D reduction (GEOX-03)

| Option | Description | Selected |
|--------|-------------|----------|
| Cut-plane ∩ footprint prism → top edges, multi-edge kept | Each wall crossing → screen edge at top height; feeds engine multi-edge diffraction; rstar corridor width = discretion | ✓ |
| Single dominant edge only | Reduce each path to one highest/nearest edge | |

**User's choice:** Cut-plane ∩ footprint prism → top edges, multi-edge kept
**Notes:** Engine already does multi-edge — don't under-use it.

---

## Claude's Discretion

- Cut-profile sampling step + DEM interpolation method (oracle-pinned to GRASS `r.profile`).
- rstar corridor width (screening query + future geometry dirty-diff).
- Receiver-grid min-spacing guardrail value.
- Azimuth handling for A/B/C (exact per-path azimuth vs. quantized sectors — accuracy/perf).
- Open-Meteo level selection, model, unit handling.
- Per-path fan-out strategy (N receivers × M sub-sources; path-cache shape must stay compatible with Phase-11 Tier-2/Tier-3 recalc).

## Deferred Ideas

- Named weather scenarios / manual overrides / difference maps → Phase 11.
- Full L_den weather-class combination → GRID-03.
- Facade / assessment-point receivers → later.
- Reflection-surface geometry extraction → Phase 10/11.
- Forest Fs coherence factor (Phase-5 deferral) → remains deferred; only geometric forest-crossing seam in play, placement is a research flag.
- DSM→DTM flattening (Phase-8 deferral) → still deferred.
- Overture buildings / national DTMs beyond AHN → pure-data additions later.
