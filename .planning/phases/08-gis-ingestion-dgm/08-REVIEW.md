---
phase: 08-gis-ingestion-dgm
reviewed: 2026-07-11T00:00:00Z
depth: deep
files_reviewed: 28
files_reviewed_list:
  - crates/envi-geo/src/crs.rs
  - crates/envi-geo/src/transform.rs
  - crates/envi-gis/src/buildings.rs
  - crates/envi-gis/src/cog/geo_tags.rs
  - crates/envi-gis/src/cog/header.rs
  - crates/envi-gis/src/cog/mod.rs
  - crates/envi-gis/src/cog/window.rs
  - crates/envi-gis/src/impedance_table.rs
  - crates/envi-gis/src/landcover.rs
  - crates/envi-gis/src/lib.rs
  - crates/envi-gis/src/merge.rs
  - crates/envi-gis/src/provenance.rs
  - crates/envi-gis/src/registry.rs
  - crates/envi-gis/src/terrain.rs
  - crates/envi-gis/src/tiles.rs
  - crates/envi-gis-wasm/src/dto.rs
  - crates/envi-gis-wasm/src/lib.rs
  - crates/envi-service/src/api/mod.rs
  - crates/envi-service/src/api/proxy.rs
  - crates/envi-service/src/error.rs
  - crates/envi-service/src/state.rs
  - crates/envi-service/tests/contract_proxy.rs
  - web/src/import/attribution.ts
  - web/src/import/fetchers.ts
  - web/src/import/importJob.ts
  - web/src/import/opfs.ts
  - web/src/import/wasm.ts
  - web/src/map/impedanceOverlay.ts
  - web/src/panels/ImportPanel.tsx
  - web/src/store/import.ts
  - web/src/store/sceneStore.ts
findings:
  critical: 1
  warning: 4
  info: 3
  total: 8
status: issues_found
---

# Phase 8: Code Review Report

**Reviewed:** 2026-07-11
**Depth:** deep (cross-file: fetch→WASM→merge→commit chain, proxy request path, COG decode guards)
**Files Reviewed:** 28
**Status:** issues_found

## Summary

The Phase-8 GIS-ingestion pivot is, on the whole, carefully built: the COG decode is genuinely guard-first (budget → bounds → work, all typed errors, no silent `0.0`), the geotransform is derived from IFD tags rather than nominal dims, the proxy is a hardcoded exact-host allowlist with `..`/prefix guards *before* any I/O, redirect-none client, size cap + connect timeout, and generic error hygiene (no host/path leak), the σ table stays a letter-only one-source-of-truth resolved through the engine, terrain becomes WGS84 `elevation_point` features and base elevation is typed `None` (never `0.0`) when terrain is absent, and reprojection stays behind the single `envi_geo` boundary. The proxy SSRF surface, the Overpass ring validation, and the OPFS path sanitizer all held up under adversarial tracing.

The headline defect is the **confirmed D-09 merge crash**: re-importing a terrain or land-cover layer into a populated scene deterministically traps the WASM with `unreachable`. The root cause is a cross-module contract violation — terrain/land-cover stamp a single **tile-level** `source_ref` across *every* feature they emit, so the merge's per-feature `(source, source_ref)` identity is many-to-one, and the merge `.expect()`s a 1:1 mapping. This is a Critical, easily-reproduced crash on a core workflow. Four warnings follow, the most serious being a swallowed whole-scene-save failure that can silently drop a large import.

## Critical Issues

### CR-01: Re-importing a terrain/land-cover layer panics the WASM (`unreachable`) in `merge_features`

**File:** `crates/envi-gis/src/merge.rs:59` (panic site); root cause spans `crates/envi-gis/src/terrain.rs:150`, `crates/envi-gis/src/landcover.rs:201`, `crates/envi-gis/src/provenance.rs:112`, and `web/src/import/importJob.ts:279,338`.

**Issue:**
The merge keys features by `(source, source_ref)` (`provenance::merge_key`, merge.rs:34-39, 46-48) and assumes that key is **per-feature unique** — it `.take()`s the matched incoming slot and `.expect("matched incoming present")`:

```rust
} else if let Some(i) = matched {
    // Untouched import → refresh from incoming.
    consumed[i] = true;
    out.push(incoming_slots[i].take().expect("matched incoming present")); // merge.rs:59 — PANICS
}
```

But terrain and land-cover stamp **one tile-level `source_ref` onto every feature of the tile**. In `importJob.ts` the whole tile's provenance is built once with `source_ref: tile.tile` (terrain: line 279; land-cover: line 338), and `terrain_features` / `vectorize_landcover` clone that same `Provenance` onto *every* emitted `elevation_point` / `ground_zone` (`terrain.rs:150`, `landcover.rs:201`). So all ~4000 elevation points of a tile share the identical merge key `("ahn4-dtm", "M_19FN2")`.

Reproducing scenario (guaranteed):
1. Import terrain over a viewport → scene now holds N `elevation_point` features, **all** with the same `(source, source_ref)` and `user_modified = false`.
2. Import the same viewport again. `merge()` builds `key_to_idx` where every incoming point overwrites the same key, so the key resolves to the single last index `L` (merge.rs:34-39).
3. Iterating `existing`: the **first** point matches `Some(L)`, is not user-modified, so the refresh branch takes `incoming_slots[L]` (OK). The **second** point matches `Some(L)` again → `incoming_slots[L].take()` is now `None` → `.expect(...)` **panics** → in WASM a Rust panic is an `unreachable` trap.

This matches the reported symptom exactly ("re-importing an already-succeeded layer into a populated scene traps the WASM with `unreachable`"). Land-cover has the identical defect (all `ground_zone`s of a WorldCover tile share the tile `source_ref`). Buildings are *not* affected because they stamp a genuinely per-feature `source_ref = "{type}/{id}"` (buildings.rs:130). The existing merge tests only ever use unique `source_ref`s (`way/1`, `way/2`, …), so the many-to-one case was never exercised.

Beyond the panic, the *semantics* are also wrong for these layers: even with the panic removed, N existing features collapsing onto one incoming index cannot express a correct per-feature refresh — decimated raster samples have no stable per-point identity across imports.

**Fix:** Replace the 1:1 assumption with **key-group** merge semantics (refresh replaces a key's whole non-user-modified group), and make the `take` infallible so a shared key can never panic:

```rust
use std::collections::{HashMap, HashSet};

#[must_use]
pub fn merge(existing: Vec<Feature>, incoming: Vec<Feature>) -> Vec<Feature> {
    // Which incoming keys exist at all (group membership, not a single index).
    let incoming_keys: HashSet<(String, String)> =
        incoming.iter().filter_map(merge_key).collect();

    let mut out = Vec::new();
    for e in existing {
        match merge_key(&e) {
            // User edits always survive re-import.
            Some(_) if is_user_modified(&e) => out.push(e),
            // A non-user-modified existing import whose key is refreshed this
            // round is dropped here; the whole incoming group is appended below.
            Some(k) if incoming_keys.contains(&k) => { /* replaced by incoming */ }
            // Existing import absent from this re-import → retain.
            // User-created (no identity) → always keep.
            _ => out.push(e),
        }
    }
    // Append every incoming feature that refreshes or newly adds a key. (Keyless
    // incoming, if any, are appended too.)
    out.extend(incoming);
    out
}
```

Add a regression test with multiple existing features sharing one key (the terrain/land-cover shape) plus a `user_modified` member, asserting no panic and that user edits survive. Alternatively (heavier), give each raster feature a deterministic per-pixel `source_ref` (e.g. `"{tile}#{col},{row}"`) so the invariant's stated "per-feature identity" actually holds — but note decimation stride varies with `target_points`, so the group-replace above is the lower-risk fix.

## Warnings

### WR-01: Large imports silently fail to persist — `saveScene` error is swallowed while the layer reports "done"

**File:** `web/src/import/importJob.ts:213-217`; interacts with `crates/envi-service/src/api/mod.rs:9-15` (the ~2 MB default request-body limit).

**Issue:** `commitFeatures` persists the whole merged scene via `saveScene()` inside a `try { … } catch { /* persisted on next autosave */ }` and then returns the committed count regardless of outcome, so `finishLayer(...,{featureCount})` reports success even when the PUT was rejected. With `TERRAIN_TARGET_POINTS = 4000` points *per tile* plus land-cover polygons and buildings, a multi-tile viewport easily produces tens of thousands of features. The whole-scene `PUT /projects/{id}/scene` serializes all of them, and `api/mod.rs` documents that the axum default body limit (~2 MB) is deliberately left in place ("ample for … hundreds of features"). An import of thousands of features exceeds that, the PUT returns 413/400, the error is swallowed, and — because an import is often the last action — the "autosave retries later" assumption never fires. The user sees `done, N features` but on reload the import is gone. This is a data-durability + honesty defect (the same "must not present unqualified Saved" concern the scene store already respects for session-only spectra).

**Fix:** Do not swallow a *persist* failure on the import path — surface it (e.g. `failLayer` or a distinct "committed, not saved" state) so the count is not presented as durable, and/or raise the scene-PUT body limit for the import route (a `DefaultBodyLimit` layer sized to the terrain point budget) and/or cap `TERRAIN_TARGET_POINTS × tiles` against a per-scene feature budget before committing.

### WR-02: Merge test suite never covers the many-features-per-key case (the CR-01 gap)

**File:** `crates/envi-gis/src/merge.rs:135-203`.

**Issue:** Every merge test uses distinct `source_ref`s (`way/1`, `way/2`, `way/4`, `way/9`), i.e. the buildings shape where identity is unique. The terrain/land-cover shape (N features sharing one tile `source_ref`) — the only shape that trips CR-01 — is untested, which is why the panic shipped green. A passing suite here is not evidence of correctness for the raster layers that dominate real imports.

**Fix:** Add a test constructing ≥2 existing features and ≥2 incoming features that all share one `(source, source_ref)` (with one `user_modified` among the existing), asserting `merge` does not panic, drops the stale group, appends the refreshed group, and preserves the user-modified feature. This test fails today and passes with the CR-01 fix.

### WR-03: `merge_features` cannot be retried after it traps — no wasm-instance recovery

**File:** `web/src/import/wasm.ts:50-54,142-145`; `web/src/import/importJob.ts:290,423`.

**Issue:** The wasm module is initialized exactly once (`ready ??= init()`), and `mergeFeatures` calls into it. When CR-01 traps, a Rust panic in wasm-bindgen aborts into an unrecoverable module state; because the facade caches the single initialized instance and never re-inits, the per-layer `Retry` (ImportPanel `import-retry-*`) will re-enter the same poisoned instance rather than a clean one. So the CR-01 crash is not merely a failed layer — it can wedge subsequent WASM calls (window/decode/parse) for the session. This is a robustness multiplier on CR-01; fixing CR-01 removes the trigger, but the facade should still be able to recover.

**Fix:** Primarily fix CR-01 (remove the panic). Additionally, detect a trapped module (a thrown `RuntimeError: unreachable`) and reset `ready = null` so the next call re-initializes a fresh instance, rather than caching a poisoned one for the rest of the session.

### WR-04: `median` sort relies on `partial_cmp(...).expect("values are finite")`

**File:** `crates/envi-gis/src/terrain.rs:267`.

**Issue:** `median` sorts with `a.partial_cmp(b).expect("values are finite")`, which panics if any value is `NaN`. Today it is safe only because both callers filter `z.is_finite()` before pushing (`sample_base_elevation` line 212-215; `base_elevation_on_raster` reads a raster whose non-finite samples were dropped to holes at decode). This is a latent no-panic-on-data violation guarded solely by discipline at every call site — a future caller that forgets the filter reintroduces a WASM trap on attacker-influenced elevation bytes.

**Fix:** Make it robust regardless of caller: `v.sort_by(|a, b| a.total_cmp(b))` (or filter `is_finite()` inside `median`), removing the `.expect`.

## Info

### IN-01: Antimeridian / inverted-bbox viewports yield empty or wrong tile sets

**File:** `crates/envi-gis/src/tiles.rs:329-337` (`grid_cells`), `web/src/import/importJob.ts:112-117` (`viewportAreaKm2`).

**Issue:** `grid_cells` assumes `lo <= hi`; a viewport crossing ±180° (min_lon > max_lon) produces an empty range via `take_while`, and `viewportAreaKm2` would compute a negative/absolute width. Not exploitable and unlikely for a normal map viewport, but the import would silently plan zero tiles rather than reporting the unsupported case.

**Fix:** Detect/normalize an antimeridian-crossing viewport (split into two ranges or reject with a typed "viewport crosses the antimeridian" message) rather than silently returning no tiles.

### IN-02: Proxy `..` guard is a substring match (`path.contains("..")`)

**File:** `crates/envi-service/src/api/proxy.rs:93`.

**Issue:** The traversal guard rejects any path containing the substring `..`, which also rejects legitimate filenames that merely embed `..` (none exist in the GLO-30/WorldCover schemes, so no functional impact today). Because the upstream host is hardcoded and the path must additionally `starts_with` the allowlisted prefix, there is no SSRF exposure — this is purely a note that the check is broader than "path segment `..`". No change required unless a future source uses `..` in a legitimate key.

### IN-03: `impedanceOverlay` hardcodes the project default ground class as `"D"`

**File:** `web/src/map/impedanceOverlay.ts:30,44`.

**Issue:** The "no data → project default" wash uses a literal `DEFAULT_GROUND_CLASS = "D"` because the scene store does not carry project settings this phase (documented in-file). It is a debug overlay, so a wrong default only mis-tints the wash, not the acoustics — but it is a second statement of a default that lives elsewhere and will drift if the project default changes. Track it for the phase that wires project settings into the store.

**Fix:** When project settings reach the store, read the default ground class from there instead of the literal.

---

_Reviewed: 2026-07-11_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: deep_
