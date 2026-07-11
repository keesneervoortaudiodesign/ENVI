---
phase: 09-path-extraction-weather
reviewed: 2026-07-11T00:00:00Z
depth: standard
files_reviewed: 21
files_reviewed_list:
  - crates/envi-gis/src/profile.rs
  - crates/envi-gis/src/impedance.rs
  - crates/envi-gis/src/screening.rs
  - crates/envi-gis/src/grid.rs
  - crates/envi-gis/src/weather.rs
  - crates/envi-gis/src/era5.rs
  - crates/envi-gis/src/path.rs
  - crates/envi-gis/src/lib.rs
  - crates/envi-gis-wasm/src/dto.rs
  - crates/envi-gis-wasm/src/lib.rs
  - crates/envi-harness/src/weather/mod.rs
  - crates/envi-harness/src/weather/route3.rs
  - crates/envi-service/src/api/era5.rs
  - crates/envi-service/src/api/mod.rs
  - web/src/import/weather.ts
  - web/src/import/opfs.ts
  - web/src/import/wasm.ts
  - web/src/import/sceneDebug.ts
  - web/src/store/weather.ts
  - web/src/panels/WeatherPanel.tsx
  - web/src/map/weatherOverlay.ts
findings:
  critical: 1
  warning: 3
  info: 3
  total: 7
status: issues_found
---

# Phase 9: Code Review Report

**Reviewed:** 2026-07-11
**Depth:** standard
**Files Reviewed:** 21
**Status:** issues_found

## Summary

Phase 9 delivers the source→receiver DEM cut-profile extractor, ground-impedance
segmentation, screening-edge injection, the receiver grid, the weather-route
`(A,B,C)` fit, ERA5 occurrence statistics, the path-cache seam, their WASM
boundary, the flagged ERA5 HTTP endpoint, and the browser weather-import UI.

Overall the code is disciplined and matches its own load-bearing invariants:
typed errors instead of fabricated `0.0` in the geometry paths, DoS caps in
`profile`/`grid`, a single-source LSQ, hardcoded SSRF host + traversal reject in
the ERA5 endpoint, `safeSeg` path-traversal defense in OPFS, hardcoded
Open-Meteo hosts, text-only error rendering, and correct dependency quarantine
(no async/network/browser crate reaches `envi-gis`/`envi-gis-wasm`; `reqwest`
stays behind `envi-service` `era5`). The 105-band framework is not touched by
these files (weather works in pressure levels + azimuths), and I found no
band-index-by-Hz regression.

The findings below are one genuine silent-`0.0` violation on the untrusted
Open-Meteo path (contradicting the module's own contract and D-07), one
robustness gap that fails legitimate elevated-site weather derivations, one DoS
guard that fires later than its documentation claims, and three lower-severity
notes.

## Critical Issues

### CR-01: `OpenMeteoResponse.elevation` silently defaults to `0.0`, fabricating the AGL baseline (D-07 violation)

**File:** `crates/envi-gis/src/weather.rs:621-626` (field), `:543-552` + `:589` (use)
**Issue:** The response struct declares

```rust
#[derive(serde::Deserialize)]
struct OpenMeteoResponse {
    #[serde(default)]
    elevation: f64,
    hourly: serde_json::Map<String, serde_json::Value>,
}
```

`levels_from_openmeteo` then does `let elevation = resp.elevation;`, passes the
`is_finite` check (`0.0` is finite), and computes each pressure level's height as
`agl = gph - elevation` (line 589). If the untrusted response omits the
`elevation` key, `#[serde(default)]` silently substitutes `0.0`, so
`agl = gph` — i.e. the AGL heights become the full **AMSL geopotential** heights
(~110–1500 m). The log basis `ln(z/z₀+1)` is not translation-invariant, so the
fitted `a_wind` (and `C`) are silently wrong. This directly contradicts the
module's own docstring ("converting AMSL geopotential height to AGL by
subtracting the response `elevation`", line 546-547) and the project's
load-bearing "GIS holes must be typed `None`/errors, never fabricated `0.0`"
rule — on exactly the untrusted-Open-Meteo-bytes path the phase's threat model
calls out. Open-Meteo normally includes `elevation`, but the whole crate's
posture is that these bytes are untrusted (and may arrive via the byte proxy), so
a missing field must be a typed error, not a fabricated baseline.
**Fix:** Make the field required (drop `#[serde(default)]`) so a missing
`elevation` is a `GisError::Json` parse error, or make it `Option<f64>` and
reject `None`:

```rust
#[derive(serde::Deserialize)]
struct OpenMeteoResponse {
    elevation: Option<f64>,
    hourly: serde_json::Map<String, serde_json::Value>,
}
// ...
let elevation = resp.elevation.ok_or_else(|| GisError::Json {
    message: "open-meteo response is missing the `elevation` field".to_string(),
})?;
if !elevation.is_finite() {
    return Err(GisError::NonFinite { what: "open-meteo response elevation".to_string() });
}
```

## Warnings

### WR-01: Negative AGL heights on elevated sites fail the whole weather derivation

**File:** `crates/envi-gis/src/weather.rs:584-608` (level build), `:513-521` + `:322-327` (z≥0 guards)
**Issue:** `levels_from_openmeteo` pushes every pressure level with
`agl = gph - elevation` regardless of sign. For a site whose elevation exceeds a
low pressure level's geopotential height (routine for elevated/mountain terrain,
where 1000 hPa or 975 hPa sits below ground and Open-Meteo still returns
extrapolated data), `agl` is negative. `components_from_levels` then feeds those
heights to `fit_log_coeff` (and `fit_profile` does the same), which reject any
`z < 0.0` with `GisError::NonFinite` / `WeatherFit`. The consequence is that a
legitimate elevated-site weather import fails entirely rather than fitting on the
valid (positive-AGL) levels. This is fail-loud (no false green), but it is a
real robustness gap for a whole class of sites.
**Fix:** Drop sub-surface levels before fitting (keep the near-surface anchor,
which is always positive):

```rust
levels.retain(|l| l.height_agl_m >= 0.0);
```

after the AMSL→AGL conversion, and document that below-ground pressure levels are
discarded. Guard that at least the required number of levels remain, else return
a typed `WeatherFit`.

### WR-02: `MAX_CORRIDOR_CANDIDATES` cap fires after building the R*-tree over *all* screens, not before allocation

**File:** `crates/envi-gis/src/screening.rs:207-228`
**Issue:** The module docs (Invariant 4, threat T-09-02-01) and the code comment
say the candidate cap rejects an over-large set "before any per-candidate …
work" and mirror `grid`'s reject-before-allocation posture. But the tree is built
over the entire input first:

```rust
let entries: Vec<AabbEntry> = screens.iter().enumerate()...collect(); // all screens
let tree = RTree::bulk_load(entries);                                  // builds over all
// ...
let candidates: Vec<usize> = tree.locate_in_envelope_intersecting(corridor)...collect();
if candidates.len() > MAX_CORRIDOR_CANDIDATES { return Err(...); }     // cap only here
```

`inject_screens`' `screens` slice comes straight off the wire
(`InjectScreensReq.screens`, unbounded), so a pathological input allocates the
`entries` vector and builds the full R*-tree before the cap can reject anything.
Unlike `grid::receiver_grid` (which checks `MAX_RECEIVERS` against the lattice
count before allocating), the DoS guard here does not bound the input set — only
the post-query candidate count.
**Fix:** Bound the input up front, e.g. `if screens.len() > MAX_CORRIDOR_CANDIDATES
{ return Err(GisError::CorridorCandidatesExceeded { got: screens.len(), limit: MAX_CORRIDOR_CANDIDATES }); }`
before `bulk_load`, or cap the collected `entries` length prior to building the
tree.

### WR-03: `import.ts::fetchBody` treats an aborted request as a network failure and issues a proxy retry

**File:** `web/src/import/weather.ts:132-151`
**Issue:** On the direct fetch, any thrown value that is not an `ApiError`
(including a caller `AbortError` from `signal`) falls into the `catch` branch and
triggers a second `fetch(proxyUrlFor(...), { signal })`. Because the same
(already-aborted) `signal` is passed, the proxy fetch immediately rejects again,
so the net effect is a spurious extra request attempt and a confusing final
error on cancellation rather than a clean abort. It also means a deliberate abort
momentarily reaches the same-origin proxy.
**Fix:** Re-throw aborts before the proxy retry:

```rust
} catch (err) {
  if (err instanceof ApiError) throw err;
  if (signal?.aborted || (err as { name?: string })?.name === "AbortError") throw err;
  const res = await fetch(proxyUrlFor(base, directUrl), { method: "GET", signal });
  ...
}
```

## Info

### IN-01: ERA5 derivation result is computed then discarded

**File:** `crates/envi-service/src/api/era5.rs:224-245`
**Issue:** `run_era5_job` calls `derive_occurrence(hours)` but on success only sets
`JobStatus::Done` — the `ClassOccurrence` (`Ok(_occ)`) is dropped, and there is no
results endpoint to retrieve it. The endpoint therefore performs a validation-only
side effect. This is consistent with the D-04/D-05 "groundwork, flagged off"
framing, but as written the compute is dead (the client can never read the
statistics it triggered).
**Fix (when un-flagged):** persist the `ClassOccurrence` to the job/result store
and expose it, or note explicitly in the handler that the result is intentionally
not surfaced yet.

### IN-02: Building with an odd number of cut-plane crossings drops the unpaired hard span

**File:** `crates/envi-gis/src/screening.rs:242-253`
**Issue:** Hard-ground spans are formed with `xs.chunks_exact(2)` (entry→exit
pairs). If the source or receiver lies *inside* a footprint, `ring_crossings`
excludes the endpoints and yields an odd count, so `chunks_exact(2)` silently
drops the last crossing and the partial interior span is never tagged hard (class
H). The screen top is still injected, but the ground under it is left with its
inherited class. Rare (source/receiver inside a building is a misconfiguration),
and only the debug overlay consumes this today, but the pairing assumption is
unstated.
**Fix:** Document the endpoint-inside-footprint limitation, or handle an odd
leading/trailing crossing by treating `[x0, first]` / `[last, xn]` as hard when
the endpoint is contained.

### IN-03: `derive_era5` recomputes `obukhov` per hour, and flagged retrieval issues an outbound request before a guaranteed error

**File:** `crates/envi-gis-wasm/src/lib.rs:698-723`; `crates/envi-service/src/api/era5.rs:175-198`
**Issue:** (a) In `derive_era5`, `occurrence_stats(&hours)` already calls
`obukhov` for every hour, then `inv_l` maps `obukhov` over the same hours a second
time — a redundant per-hour recompute of the full virtual-temperature/Obukhov
chain. (b) In `retrieve_era5_hours`, even when a `CDS_API_KEY` is configured the
function issues the authenticated `app.http.get(&url)...send().await?` and then
unconditionally returns `ApiError::Internal` (NetCDF decode unimplemented), so a
successful outbound CDS request is made purely to be discarded. Both are
low-impact (the endpoint is flagged off; the derivation is correct), but the
wasted request is an avoidable outbound call.
**Fix:** (a) return the per-hour `1/L` from a single pass (have
`occurrence_stats` optionally collect `inv_l`, or fold both in one loop). (b)
skip the network round-trip until the decode is implemented (return the
"disabled/unimplemented" error before `send()`), or gate it behind an explicit
"attempt live fetch" flag.

---

_Reviewed: 2026-07-11_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
