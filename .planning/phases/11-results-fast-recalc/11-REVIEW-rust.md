---
phase: 11-results-fast-recalc
reviewed: 2026-07-12T00:00:00Z
depth: deep
files_reviewed: 20
files_reviewed_list:
  - crates/envi-compute/src/weighting.rs
  - crates/envi-compute/src/readout.rs
  - crates/envi-compute/src/grid.rs
  - crates/envi-compute/src/isoband.rs
  - crates/envi-compute/src/lib.rs
  - crates/envi-compute/src/export/mod.rs
  - crates/envi-compute/src/export/csv.rs
  - crates/envi-compute/src/export/geojson.rs
  - crates/envi-compute/src/export/geotiff.rs
  - crates/envi-compute/Cargo.toml
  - crates/envi-compute-wasm/src/lib.rs
  - crates/envi-compute-wasm/src/opfs_reader.rs
  - crates/envi-compute-wasm/src/recondition.rs
  - crates/envi-compute-wasm/src/export.rs
  - crates/envi-compute-wasm/src/dto.rs
  - crates/envi-compute-wasm/Cargo.toml
  - crates/envi-gis/src/weather.rs
  - crates/envi-gis-wasm/src/lib.rs
  - crates/envi-gis-wasm/src/dto.rs
  - crates/envi-service/tests/wire_no_drift.rs
  - crates/envi-store/src/dto.rs
findings:
  critical: 1
  warning: 3
  info: 4
  total: 8
status: issues_found
---

# Phase 11: Code Review Report (Rust/WASM readout + export layer)

**Reviewed:** 2026-07-12
**Depth:** deep
**Files Reviewed:** 20 (+1 store DTO consumed for the wire cross-check)
**Status:** issues_found

## Summary

The Phase-11 readout/export layer is well-built and disciplined against most project
invariants. Verified as **respected**:

- **Engine 3-dep quarantine** — no `envi-engine` edits in scope; all net-new math
  (A/C tables, iso-band tracer, GeoTIFF encoder) lives in `envi-compute`, which
  depends on the engine but adds nothing to its `ndarray + num-complex + thiserror`
  posture. `Cargo.toml` boundary comments hold.
- **Readout drives engine laws** — `readout_receiver` calls `compose_gain`,
  `readout_coherent`, `readout_incoherent`, and `band_levels_db_two_channel`; the
  MAC≡engine bit-exact test (`coherent_output_equals_direct_readout_coherent_bit_for_bit`)
  is a genuine gate. No bespoke dB/MAC re-derivation.
- **Two-channel H_coh / P_incoh contract** — the incoherent channel is always the
  magnitude-only `Σ|G|²·P_incoh` and never overwrites the coherent channel; total =
  `|coherent Σ|² + P_incoh`.
- **Compare-by-BAND-INDEX** — weighting zips `levels`/`w` by index; CSV emits
  `band_index` + exact Hz from `FreqAxis::centres`, never a nominal 31.5 Hz key.
- **Wire discipline** — `ConditioningDto` is re-exported (not re-mirrored) from
  `envi_compute::readout` to `envi_store::dto`; the no-drift test registers every new
  DTO. No hand-authored TS mirror found.
- **f64 house rules** — `z₀ ≥ Z0_MIN_M` clamps present in weather; `solve_3x3`
  singular guard is typed; OPFS decode rejects non-finite via a typed error.
- **Path-traversal** — `sanitize_export_filename` strips separators, `..`, NUL, and
  control chars and falls back to `export`; traced by hand, it is sound.

The findings below are the exceptions: one law-selection defect that silently changes
computed dB, and a cluster of robustness/hardening gaps in the readout finiteness
invariant and the export encoders.

## Critical Issues

### CR-01: Muting a sub-source silently flips the readout from incoherent to coherent summation

**File:** `crates/envi-compute-wasm/src/recondition.rs:212-216` (mute → `filter = Some(zeros)`)
combined with `crates/envi-compute/src/readout.rs:159-168` (`default_law`) and the
client entry points `recondition_receivers` (readout.rs consumer) / `readout_all_receivers`.

**Issue:** `to_engine` represents a muted source as a present all-zero complex filter:

```rust
let filter = if c.muted {
    Some(vec![Complex::new(0.0, 0.0); N_BANDS])   // "muted"
} ...
```

`default_law` then classifies the *entire* readout as `Coherent` whenever **any**
sub-source has `filter.is_some()`:

```rust
let conditioned = conditioning.iter().any(|c| c.filter.is_some() || c.delay_s != 0.0);
```

Both `recondition_receivers` and `readout_all_receivers` call `default_law(&conditioning)`
with no override, so the mute representation leaks into law selection.

**Failure scenario:** A source (or road) modelled as ≥2 plain omnidirectional
sub-sources reads out under `Incoherent` (Annex-A energy sum, two co-located = +3 dB).
The user mutes one sub-source in the UI. `to_engine` gives that sub-source a
`Some(zeros)` filter → `default_law` now returns `Coherent`. The remaining *active*
sub-sources are summed as `|Σ H_coh·G|²` (phase interference) instead of `Σ|G|²|H|²`
(energy). The reported band levels and dB(A)/dB(C) totals change to physically wrong
values for a routine UI operation, with no error and no visible cause. (The bundled
`muted_source_is_silenced` test only exercises 1 muted + 1 active source, where
single-source coherent and incoherent readouts happen to coincide, so it does not
catch this.) The same leak means an EQ (per-band filter) or delay on one sub-source
coherent-izes all co-solved sub-sources — worth confirming that mixed
road+loudspeaker tensors are never read out together under one `default_law`.

**Fix:** Do not let the mute encoding drive law selection. Either represent mute as a
first-class flag on `Conditioning` (e.g. `muted: bool`) that zeroes the gain without
setting `filter`, or have `default_law` ignore all-zero filters:

```rust
// in default_law
let conditioned = conditioning.iter().any(|c| {
    c.delay_s != 0.0
        || c.filter.as_deref().is_some_and(|f| f.iter().any(|g| *g != Complex::new(0.0, 0.0)))
});
```

and/or thread an explicit `ReadoutLaw` from the source type down the recondition path
rather than inferring it from conditioning composition.

## Warnings

### WR-01: `band_levels_db` (the primary wire output) is not floored — an all-silent receiver emits −∞

**File:** `crates/envi-compute/src/readout.rs:276-277`

**Issue:** `coherent_db` and `incoherent_db` are passed through `floor_levels(...)` to
clamp `10·log₁₀(0) = −∞` to `SILENCE_FLOOR_DB`, and the module docs state "the readout
carries only finite values on the wire." But the primary combined output
`band_levels_db` is **not** floored:

```rust
let band_levels_db =
    band_levels_db_two_channel(&h_eff, &hff_eff, &ones, &BandSpectrum::uniform(0.0));
// ^ never wrapped in floor_levels()
```

**Failure scenario:** A receiver whose coherent and incoherent energies are both zero
(all sub-sources muted, or a genuinely silent column) yields `band_levels_db[f] = −∞`.
That non-finite value flows into `ReceiverReadoutDto::band_levels_db` on the wire and
into `weighted_total_db` (→ `total_dba`/`total_dbc = −∞` when every band is silent).
`serde_wasm_bindgen` will hand the frontend `-Infinity` (not valid JSON, and a likely
render/`JSON.stringify` hazard), and the CSV export writes it as literal `NaN`,
contradicting the documented finite-on-the-wire invariant that the split channels are
careful to uphold.

**Fix:** Apply the same clamp to the combined output:

```rust
let band_levels_db = floor_levels(band_levels_db_two_channel(
    &h_eff, &hff_eff, &ones, &BandSpectrum::uniform(0.0),
));
```

and add an all-muted / all-silent receiver test asserting `band_levels_db` and both
totals stay finite.

### WR-02: CSV encoder writes receiver labels and totals without RFC-4180 quoting/escaping

**File:** `crates/envi-compute/src/export/csv.rs:37-40, 73-78` (`column_label`, header build)

**Issue:** Receiver column labels are concatenated straight into the header row with no
quoting or delimiter escaping:

```rust
out.push(',');
out.push_str(&column_label(labels, i));  // raw String, unescaped
```

`labels` is typed `&[String]` with no format constraint (the "TS-minted UUID" contract
is documentation, not enforcement).

**Failure scenario:** A label containing a comma (`"road, north"`) injects an extra
column and silently misaligns every data row from that column onward. A label with a
newline breaks the row structure entirely. A label beginning with `=`, `+`, `-`, or `@`
is a classic CSV/formula-injection payload when the downloaded file is opened in a
spreadsheet. Because the encoder is `#[must_use]` and "never panics on data," the
corruption is silent.

**Fix:** Quote fields per RFC-4180 (wrap in `"`, double internal `"`), and prefix a
leading `=`/`+`/`-`/`@`/tab with a `'` or space to neutralise formula injection. Apply
the same to any free-text field written into the `#` comment header
(`weighting_label`, `attribution`) so an embedded newline cannot break out of the
comment block.

### WR-03: `ExportCrsDto.utm_zone` is `u32` but silently truncated to `u8`, with no range check

**File:** `crates/envi-compute-wasm/src/export.rs:46-48, 131-134` and
`crates/envi-compute-wasm/src/dto.rs:512-520`

**Issue:** `ExportCrsDto.utm_zone` is `u32` (the `envi-store` `CrsDto` uses `u8`), and
nothing validates `1..=60`. `utm_epsg` uses the raw `u32` for the stamped EPSG, while
`project_crs` builds the reprojection CRS via `ProjectCrs::from_zone(crs.utm_zone as u8, ...)`:

```rust
ProjectCrs::from_zone(crs.utm_zone as u8, crs.south)   // silent u32 -> u8 wrap
```

**Failure scenario:** A malformed request with `utm_zone = 287` truncates to
`287 as u8 = 31`, so the GeoJSON is reprojected as UTM 31 while the GeoTIFF
`GeoKeyDirectory` and every footer are stamped `EPSG:32887` (an invalid/undefined
projected CRS). The download is internally inconsistent about its own CRS with no
error surfaced. GeoTIFF's `u16::try_from(epsg).unwrap_or(0)` similarly degrades a
too-large EPSG to `0` (undefined CRS) silently rather than erroring.

**Fix:** Validate `1..=60` at the boundary and reject out-of-range zones with a typed
`ComputeError::Export`, and derive the stamped EPSG from the same validated `u8` used
to build `ProjectCrs` so the metadata and the reprojection can never disagree.

## Info

### IN-01: `recondition_receivers` and `readout_all_receivers` duplicate the validate-and-loop body

**File:** `crates/envi-compute-wasm/src/recondition.rs:52-115` and `131-188`

**Issue:** The hash gate, the two count checks, the `to_engine` fan-out, `default_law`,
and the per-receiver `readout_receiver` loop are copied nearly verbatim between the two
functions; they differ only in the returned DTO shape.

**Fix:** Extract a private helper returning `Vec<ReceiverReadout>` (the full readout)
and let each public function project it into `ReconditionResult` vs `ReadoutResult`.

### IN-02: friendly raw-override hardcodes `z0.max(0.001)` instead of the `Z0_MIN_M` constant

**File:** `crates/envi-gis-wasm/src/lib.rs:714`

**Issue:** `let z0 = z0.max(0.001);` restates the roughness floor as a magic literal,
while `envi_gis::weather` clamps through `Z0_MIN_M`. If the engine constant ever
changes, this path drifts.

**Fix:** Import and use `envi_engine::propagation::refraction::profile::Z0_MIN_M` (or the
re-export) here too.

### IN-03: GeoTIFF EPSG overflow degrades to `0` (undefined CRS) instead of a typed error

**File:** `crates/envi-compute/src/export/geotiff.rs:160`

**Issue:** `let epsg = u16::try_from(meta.epsg).unwrap_or(0);` emits a valid-looking
GeoTIFF whose `ProjectedCSTypeGeoKey = 0` (undefined) for any `epsg > 65535`. A raster
that claims no CRS is a silent correctness loss for a georeferenced export.

**Fix:** Since `encode_geotiff` is infallible today, either assert/validate the EPSG
upstream (WR-03) or change the signature to return `Result` and reject an EPSG that
does not fit `u16`.

### IN-04: GeoJSON encoder swallows a serialization failure into an empty FeatureCollection

**File:** `crates/envi-compute/src/export/geojson.rs:62-63`

**Issue:** `serde_json::to_string(&fc).unwrap_or_else(|_| "…empty FeatureCollection…")`
turns any serialization error into a valid-but-empty export, silently dropping all
bands. (serde_json also encodes any non-finite reprojected coordinate as `null`,
producing structurally-invalid positions without erroring.) The failure is invisible
to the caller.

**Fix:** Propagate the serialization error as a `ComputeError::Export` rather than
substituting an empty document, and guard against non-finite coordinates before
encoding.

---

_Reviewed: 2026-07-12_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: deep_

---

## Fix pass outcome (2026-07-12)

All Critical + Warning + the four Info findings were fixed. Gates re-run green
after the changes: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test` (workspace, all pass incl. new tests + `wire_no_drift`), and
`cargo tree -p envi-engine` unchanged (`ndarray + num-complex + thiserror`). No
`envi-engine` edits; the two-channel contract, band-index comparison, and ts-rs
no-drift are preserved (no TS-exported DTO changed — `ConditioningDto`/`ExportCrsDto`
are byte-identical on the wire).

| ID | Outcome | Notes |
|----|---------|-------|
| CR-01 | **fixed** | Added a first-class `muted: bool` to the engine `Conditioning`; `default_law` now skips muted sub-sources so a mute is law-neutral (chooses coherent/incoherent by the surviving sources' composition, not by a filter vector's presence). `to_engine` threads `muted`. New ≥2-plain-sub-source regression test (one muted, with a stale delay) asserts the readout stays incoherent — the gap the bundled test missed. Still drives the engine laws (no bespoke dB/MAC). |
| WR-01 | **fixed** | Primary combined `band_levels_db` now floored through the same `floor_levels` clamp; all-silent/all-muted receiver emits `SILENCE_FLOOR_DB` and finite dB(A)/dB(C) totals. Added all-silent + muted-gain tests. |
| WR-02 | **fixed** | New `csv_field`: RFC-4180 quoting (comma/quote/CR/LF → wrap + double internal quotes) + formula-injection guard (leading `= + - @`/tab/CR → `'` prefix) applied to receiver labels; numeric columns left bare (parseable). Footer free-text (`csv_comment_lines` + `one_line`) has CR/LF collapsed so a newline can't break out of the `#` comment block / ImageDescription. Tests added. |
| WR-03 | **fixed** | `validated_zone` rejects UTM zones outside `1..=60` with a typed `ComputeError::Export` before the `as u8` cast; the stamped EPSG is derived from the same validated `u8` used to build `ProjectCrs` (metadata ⇄ reprojection can no longer disagree). Applied to all export arms + the live tracer; test added. |
| IN-01 | **fixed** | Extracted the duplicated hash-gate/validate/`to_engine`/readout loop into `gated_readout` returning `Vec<ReceiverReadout>`; both public entries project it. |
| IN-02 | **fixed** | Raw-override z₀ now clamps through `envi_engine::…::profile::Z0_MIN_M` instead of the magic `0.001`. |
| IN-03 | **fixed** | `encode_geotiff` returns `Result<_, EpsgOverflow>`; an EPSG > `u16::MAX` errors instead of silently stamping the undefined CRS 0. (WR-03 already guarantees a valid UTM EPSG upstream, so this is defense-in-depth.) Test added. |
| IN-04 | **fixed** | `encode_isophone_geojson` returns `Result<_, GeoJsonEncodeError>`; serialization failures propagate (no silent empty FeatureCollection) and non-finite ring vertices are rejected up front (no invalid `null` positions). Test added. |

_Fixed: 2026-07-12 — Claude (gsd-code-fixer)_
