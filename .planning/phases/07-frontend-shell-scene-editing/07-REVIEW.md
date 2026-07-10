---
phase: 07-frontend-shell-scene-editing
reviewed: 2026-07-10T00:00:00Z
depth: deep
files_reviewed: 25
files_reviewed_list:
  - crates/envi-dgm/src/lib.rs
  - crates/envi-dgm/src/tin.rs
  - crates/envi-store/src/interpolate.rs
  - crates/envi-store/src/calibrate.rs
  - crates/envi-store/src/dto.rs
  - crates/envi-service/src/api/meta.rs
  - crates/envi-service/src/api/dgm.rs
  - crates/envi-service/src/api/mod.rs
  - crates/envi-service/src/error.rs
  - web/src/App.tsx
  - web/src/api/client.ts
  - web/src/store/edges.ts
  - web/src/store/sceneStore.ts
  - web/src/store/autosave.ts
  - web/src/store/projectActions.ts
  - web/src/store/inheritance.ts
  - web/src/store/dgm.ts
  - web/src/dgm/dgmTrigger.ts
  - web/src/map/useTerraDraw.ts
  - web/src/map/MapCanvas.tsx
  - web/src/validate/groundZone.ts
  - web/src/spectrum/SpectrumEditor.tsx
  - web/src/spectrum/interpolateClient.ts
  - web/src/testBridge.ts
  - web/src/theme.css
findings:
  critical: 0
  high: 1
  medium: 4
  low: 2
  total: 7
status: fixes_applied
fix:
  fixed_at: 2026-07-10
  fixed: 6
  accepted_risk: 1
  resolutions:
    HI-01: fixed
    ME-01: fixed
    ME-02: fixed
    ME-03: fixed
    ME-04: fixed
    LO-01: accepted_risk
    LO-02: fixed
---

# Phase 7: Code Review Report

**Reviewed:** 2026-07-10
**Depth:** deep (cross-file: store ⇄ TD lifecycle ⇄ API ⇄ engine boundary)
**Files Reviewed:** 30
**Status:** issues_found

## Summary

Phase 7 is high-quality, defensively-written code. The load-bearing subsystems each hold
up under adversarial reading:

- **`envi-dgm` panic safety (D-08)** is genuinely airtight: DoS caps *before* any `O(n log n)`
  work, finiteness rejection before insert, a `can_add_constraint` pre-check on every segment
  guarding the sole `add_constraint`, `f64 == f64` duplicate-vertex short-circuit, and 11
  tests including the two-crossing-diagonals and self-intersecting-polyline panic cases. No
  `unwrap()`/`panic!` on any data path; `#![deny(unsafe_code)]` present.
- **Endpoint validation** — `interpolate-spectrum`, `spl-to-lw`, `dgm/triangulate` — all reject
  bad input as `400` via typed `From<DgmError>` / `From<StoreError>`; every DgmError variant
  maps to a client fault, so no `5xx`/panic path exists. The Phase-6 MED-1 path-leak fix is
  intact: `Io`/`Json`/`PathEscape` are logged server-side and return a generic `"internal error"`.
- **D-06 authored-only** holds: no `r_db` is persisted anywhere (client or serde); the only `r_db`
  references are the derived server response and doc comments. **D-05 server-owned math** holds:
  no client-side Hz/log/interpolation arithmetic — only band-*index* stride math and turf geometry
  predicates (both allowed).
- **XSS**: zero `dangerouslySetInnerHTML`/`innerHTML` usages (comments only); server strings render
  as React text children throughout.
- **CLAUDE.md invariants** verified: `envi-engine` byte-identical (`git diff --quiet` clean),
  `propagation/` conj gate = 0, zero C deps in `envi-dgm`, no `.github/workflows/`, `theme.css`
  byte-identical to the metrao3 source (no invented tokens), English-only.

The findings below are concentrated in two areas the pitfall-mapper flagged: the **autosave
flush/interleave discipline (D-04)** and residual **defense-in-depth gaps in the ring-diff
same-count branch (D-02)**. None is a security hole; the highest is a bounded last-edit data-loss
window on tab close.

## High

### HI-01: Autosave flush-on-unload uses a plain `fetch` the browser aborts on page teardown

**Resolution: FIXED** (commit 786d90d) — the unload path is now `flushAutosaveOnUnload`, which issues the
whole-scene PUT with `fetch(url, { method: "PUT", body, keepalive: true })` (not `sendJson`), so the browser
completes it during teardown. `sendBeacon` was deliberately avoided (whole-scene payload can exceed its
~64 KB cap). Best-effort — the indicator is not set to "saved" on the unload path. `beforeunload`/`pagehide`
now call `flushAutosaveOnUnload`.

**File:** `web/src/store/autosave.ts:90-95` (`flushAutosave`) → `web/src/api/client.ts:82-95` (`sendJson`)
**Issue:** D-04 promises "a flush on tab close / navigate-away." `flushAutosave()` calls
`void runSave()` → `saveScene()` → `putScene()` → `fetch(..., { method, headers, body })` with
**no `{ keepalive: true }`** and no `navigator.sendBeacon`. When fired from a `beforeunload` /
`pagehide` handler, a fetch without `keepalive` is not guaranteed to complete — browsers cancel
in-flight requests as the document is torn down. So a committed edit made within the ≤750 ms
debounce window immediately before the tab closes is **silently lost**, while the dirty indicator
had shown the work as pending/saving. This is the exact D-04 guarantee the code is trying to honor,
implemented with a mechanism that does not honor it. (Confirmed: `grep keepalive|sendBeacon web/src`
→ none.)
**Fix:** Route the unload flush through a keepalive-capable path:
```ts
// autosave.ts — a dedicated unload path (keepalive body cap is 64 KB; fine for typical scenes)
function flushViaBeacon(): void {
  if (!pending) return;
  const { projectId } = useSceneStore.getState();
  const body = JSON.stringify(useSceneStore.getState().sceneFeatureCollection());
  const url = `/api/v1/projects/${encodeURIComponent(projectId ?? "current")}/scene`;
  // PUT is not sendBeacon-able; use fetch keepalive for the unload path.
  void fetch(url, { method: "PUT", headers: { "Content-Type": "application/json" }, body, keepalive: true });
  pending = false;
}
```
Wire `flushViaBeacon` (not `flushAutosave`) into the `beforeunload`/`pagehide` listeners.
For scenes above the 64 KB keepalive cap, additionally block-and-`await` the save on an explicit
close action rather than relying on the unload event.

## Medium

### ME-01: No in-flight guard on `runSave` — concurrent whole-scene PUTs can persist a stale scene

**Resolution: FIXED** (commit 786d90d) — `runSave` now holds an in-flight promise (`inFlight`) and a single
trailing-save latch (`queued`): a save requested while another is in flight only sets `queued` and returns;
once the in-flight save settles, a single trailing `runSave` re-runs with the latest store snapshot. Two
concurrent PUTs can no longer complete out of order and persist a staler scene. `pending` is cleared only on
genuine save success (so an in-flight/failed save still counts as unsaved for the unload flush).

**File:** `web/src/store/autosave.ts:61-100` (`runSave`, `saveNow`, `flushAutosave`, debounce timer)
**Issue:** `runSave()` has three independent entry points — the debounce `setTimeout`, `saveNow()`
(explicit Save), and `flushAutosave()` — with **no guard preventing overlap** and no request
sequencing. `setSaving()` is written but never read as a lock. Scenario: the debounce timer fires
`runSave` (reads snapshot S1, awaits the PUT); during the await the user clicks Save → `saveNow()`
→ a second `runSave` (reads snapshot S2 ⊇ S1). Two whole-scene PUTs are now in flight to the same
`/scene`; if the network delivers S1's PUT *after* S2's (browsers use multiple connections / HTTP2
multiplexing, so completion order is not guaranteed), the server's last write is the **staler** S1,
dropping the newer edits. The final `setSaved`/`setError` status also reflects whichever finishes
last, not whichever is newest.
**Fix:** Serialize saves — hold an in-flight promise and coalesce:
```ts
let inFlight: Promise<void> | null = null;
let queued = false;
async function runSave(): Promise<void> {
  if (inFlight) { queued = true; return; }        // coalesce: a newer snapshot will be picked up
  inFlight = (async () => {
    try { useAutosaveStore.getState().setSaving();
          await useSceneStore.getState().saveScene();
          useAutosaveStore.getState().setSaved(Date.now()); }
    catch (err) { useAutosaveStore.getState().setError(errorText(err)); }
  })();
  await inFlight; inFlight = null;
  if (queued) { queued = false; void runSave(); }  // re-run once with the latest store snapshot
}
```

### ME-02: Ring-diff MOVE branch trusts vertex-count equality — a same-count edit re-points façade spectra (D-02 gap)

**Resolution: FIXED** (commit 12dd523) — the same-count branch now counts differing vertex positions and
only takes the positional-preserve (MOVE) path when at most ONE vertex differs; a same-count insert+delete
or reorder (≥2 differing positions) falls back to `rebuild` (fresh UUIDs, overrides dropped loudly) rather
than silently re-pointing a per-edge spectrum. A regression test (`edges.test.ts`, "SAME-COUNT insert+delete
is NOT trusted as a MOVE") asserts the fail-safe drops the override; a companion test guards that a genuine
single-vertex drag still classifies as MOVE.

**File:** `web/src/store/edges.ts:89-99`
**Issue:** The whole purpose of D-02 is to make silent façade re-pointing *structurally impossible*.
The `prevN === nextN` branch returns `edgeIds: prevEdgeIds.slice()` **positionally** and
`reconcileFacade: (f) => ({...f})` without verifying the change is actually a per-vertex move. Any
same-count delta that is *not* a pure positional move — a simultaneous insert+delete in one committed
snapshot, or a vertex reorder — is classified `move` and keeps every edge UUID in its old slot, so a
per-edge override now points at a *geometrically different* façade. This is precisely the
data-corruption class D-02 exists to prevent, and it is not covered by the 8 unit tests (which only
exercise single-vertex insert/delete and a genuine move). It is low-probability under normal Terra
Draw select-mode editing (one vertex per `change`), but the store's `applyTerraDrawChange` accepts
any snapshot (incl. the `applyBuildingRing` test-bridge path and any future multi-select drag), so
the invariant is only behavioral, not structural.
**Fix:** In the same-count branch, confirm the two rings differ in at most one vertex coordinate
before trusting positional identity; otherwise fall back to `rebuild`:
```ts
if (prevN === nextN) {
  const changed = prevRing.reduce((n, v, i) => n + (sameCoord(v, nextRing[i]) ? 0 : 1), 0);
  if (changed > 1) return rebuildResult(nextRing);   // not a single-vertex move — fail safe
  // ...existing identity/move return
}
```

### ME-03: Ring-diff INSERT corrupts edge UUIDs when the inserted vertex duplicates an existing coordinate

**Resolution: FIXED** (commit 12dd523) — `ringDiff` now guards at entry with `hasDuplicateCoords` on BOTH
rings and falls back to `rebuild` when either carries a duplicate coordinate (an inserted vertex duplicating
an existing coord, or a collapsed `prevEdgeLookup`). This removes every ambiguous coordinate-identity match
before it can run. Regression tests cover the duplicate-inserted-vertex ring and a degenerate prev ring with
a duplicate (collapsed lookup).

**File:** `web/src/store/edges.ts:104-144` (INSERT loop) and `:62-74` (`pairKey`/`prevEdgeLookup`)
**Issue:** The INSERT reconciliation identifies the two split halves with `sameCoord(to, w)` /
`sameCoord(from, w)`, where `w` is the inserted vertex's coordinate. If `w` exactly equals another
ring vertex's coordinate (a degenerate but reachable footprint), **every** edge whose endpoint
matches `w` is assigned `parentId` / `secondHalfId`, overwriting unrelated edges' UUIDs and thus
their spectra. Independently, `prevEdgeLookup` builds a `Map<pairKey, edgeId>` keyed by
`"fromX,fromY|toX,toY"`; two identical directed edges (possible with duplicate vertices) collapse to
one map entry, so a MOVE/INSERT lookup silently returns the wrong prior UUID. Neither path is
guarded, and `ringOf` does not de-duplicate interior vertices.
**Fix:** Detect duplicate coordinates in `nextRing` (or a collapsed `prevEdgeLookup`, i.e.
`lookup.size < prevN`) and fall back to `rebuild` — the documented fail-safe direction — rather than
proceeding with an ambiguous coordinate match.

### ME-04: Autosave reports "Saved" while authored spectra are never persisted (misleading state)

**Resolution: FIXED (honesty)** (commit ace8cda) — fix (a) (folding authored spectra into typed feature
`properties`) was assessed OUT OF SCOPE for a review fix: the store's `spectra` channel is a generic
key→`AuthoredSpectrumDto` map whose semantics differ per kind (building `default_isolation`, per-edge façade
override, wall/screen isolation, source **L_W**), so a correct persisted shape is the Phase-9/10 typed
serialization work (`SubSourceDto.spectrum` / `IsolationSpectrumDto`) that 07-08-SUMMARY already defers — a
generic ad-hoc shape now would be a second shape Phase 9/10 must migrate, contra "one source of truth".
Fix (b) applied instead: the deferral is now honest. `ProjectBar` renders a distinct `.chip.warn`
("Spectra session-only", `data-testid="spectra-unpersisted"`) whenever the scene carries authored spectra,
so "Saved" never implies the acoustic authoring is persisted; the boundary is documented at the save seam
(`sceneStore.sceneFeatureCollection`). No false green. The serialization itself remains a documented
Phase-9/10 deferral.

**File:** `web/src/store/sceneStore.ts:494-503` (`sceneFeatureCollection`/`saveScene`), `:473-490`
(`loadScene` resets `spectra: {}`); indicator driven by `web/src/store/autosave.ts:45`
**Issue:** `saveScene()` PUTs only `Object.values(features)` (geometry + properties). The `spectra`
channel — every per-façade override, building `default_isolation`, and source `L_W` authored in the
WEB-08/09/10 editor — is **not serialized into the PUT**, and `loadScene` resets `spectra: {}` on
open/reopen/reload. So a user can author a façade isolation spectrum, watch the indicator settle to
**"Saved"**, reload, and find the spectrum gone. 07-08-SUMMARY documents spectrum→scene persistence
as a Phase-9/10 deferral, and the `spectra`-not-in-PUT design is intentional for this phase — but the
autosave UI presents an unconditional "Saved" for data it silently did not save, which is exactly the
"false green / silently inert" posture the phase's own principles forbid. This also puts SC4 ("scene
survives … reload, close/reopen") in tension for the acoustic-authoring surface.
**Fix (choose one):** (a) surface the deferral honestly — e.g. an "authored spectra are session-only
this phase" note near the editor / indicator so the user is not told spectra are saved; or (b) fold
the `spectra` map into the persisted feature `properties` (`default_isolation` under the feature id,
`facade_isolation[edgeId]`) at `sceneFeatureCollection()` time and re-hydrate them in `loadScene`,
which is the D-02/D-06 persisted shape the CONTEXT example already describes.

## Low

### LO-01: DGM producer sends WGS84 degrees into a Euclidean TIN (distorted preview)

**Resolution: ACCEPTED RISK** — `dgmTrigger.ts` is an honest, labeled stub: its module header (lines 13-16)
explicitly documents that the `[lng, lat, z]` triples are Phase-7 preview inputs and that "the geodetic
projection into SceneXY meters lands with terrain import (Phase 8) — the endpoint shape is unchanged." It
does NOT present the distorted TIN as a real elevation result. Per the fix directive, a labeled stub that
does not claim a real result is recorded as an accepted risk; Phase 8 wires the CRS-projected SceneXY
coordinates. See `## Accepted Risks` below.

**File:** `web/src/dgm/dgmTrigger.ts:29-49` (`collectElevation`)
**Issue:** `collectElevation` pushes raw `[lng, lat, z]` degrees straight into
`POST /dgm/triangulate`, which triangulates in Euclidean space treating one degree of longitude and
one degree of latitude as equal-length axes. Away from the equator this skews the TIN (a ~1.6:1
lat/lng scale at 52°N), so the overlay triangle shapes and interpolated Z are geometrically distorted.
The module header documents this as a Phase-8 projection deferral (SceneXY meters land with terrain
import), so it is an honest, labeled stub rather than a hidden defect — noted for completeness because
the on-screen TIN is a real geometric artifact, not a placeholder.
**Fix:** None required this phase; when Phase-8 projection lands, project to SceneXY before assembling
the request (the endpoint shape is unchanged).

### LO-02: Property inheritance records geometry-ish fields (`edge_ids`, `mode`) as "inherited"

**Resolution: FIXED** (commit 5cb903d) — the exclusion is centralized at the inheritance boundary:
`recordLast` (`inheritance.ts`) now strips `NON_INHERITABLE_KEYS` (`kind`, `id`, `edge_ids`, `mode`) before
storing the last-object bag, covering every caller (`updateProperties` + `tagFeature`). `updateProperties`
hands the full property bag straight to `recordLast`. No internal/geometry metadata leaks into the WEB-04
inheritance chips.

**File:** `web/src/store/sceneStore.ts:442-448` (`updateProperties` → `recordLast(kind, nonGeom)`)
**Issue:** `updateProperties` strips only `kind` and `id` before `recordLast`, so for a building the
recorded bag still carries `edge_ids` and `mode`. `seedProps` then seeds those onto the *next*
building of the kind and lists them in `inheritedFields`, producing spurious "inherited from last
building" chips and briefly seeding a stale `edge_ids` array. Correctness is preserved — `tagFeature`
overwrites `edge_ids` with fresh `initEdgeIds` and sets `mode` explicitly *after* the spread
(`sceneStore.ts:267-270`) — so this is cosmetic, not corrupting, but it leaks non-editable geometry
metadata into the WEB-04 inheritance UI.
**Fix:** Exclude geometry/structural keys from the inheritance bag, e.g.
`const { kind: _k, id: _i, edge_ids: _e, mode: _m, ...nonGeom } = props;` before `recordLast`.

---

## Accepted Risks

### LO-01: DGM TIN preview triangulates WGS84 degrees in Euclidean space (Phase-8 projection deferral)

**Accepted 2026-07-10.** `web/src/dgm/dgmTrigger.ts` feeds raw `[lng, lat, z]` degrees into the server TIN,
so the overlay is geometrically skewed away from the equator (~1.6:1 at 52°N). This is an explicitly labeled
stub, not a hidden defect: the module header documents the Phase-8 SceneXY-meters projection deferral, and
the on-screen TIN is never presented as a real elevation result. The endpoint shape is unchanged, so Phase 8
projects to SceneXY before assembling the request with no rewrite. No fix this phase; risk accepted pending
Phase-8 terrain import (the same seam that adds imported GLO-30/DTM samples).

## Fix Resolution Summary

| Finding | Severity | Resolution | Commit |
|---------|----------|-----------|--------|
| HI-01 | High | Fixed — keepalive unload flush | 786d90d |
| ME-01 | Medium | Fixed — serialized in-flight saves | 786d90d |
| ME-02 | Medium | Fixed — same-count MOVE proven single-vertex, else rebuild (+test) | 12dd523 |
| ME-03 | Medium | Fixed — duplicate-coordinate rings fall back to rebuild (+test) | 12dd523 |
| ME-04 | Medium | Fixed (honesty) — "Spectra session-only" affordance; serialization stays Phase-9/10 | ace8cda |
| LO-01 | Low | Accepted risk — labeled Phase-8 projection stub | — |
| LO-02 | Low | Fixed — internal keys stripped from inheritance bag | 5cb903d |

**Fixed: 6 · Accepted risk: 1.** All quality gates green after fixes (see below).

Gate status (post-fix, fully offline):
- `cargo fmt --check` clean · `cargo clippy --all-targets -- -D warnings` clean · `cargo test --workspace`
  green · `git diff --quiet crates/envi-engine/` byte-identical.
- `cd web && npx tsc --noEmit` clean · `npm run build` green (`web/dist` rebuilt + committed) ·
  `npm run test:unit` 12 passed (incl. 4 new ring-diff regression tests) · `npx playwright test` 14 passed
  (zero unmocked network requests).

---

## Verified clean (adversarial checks that passed)

- **`spade` panic safety (focus #2):** every insert path is `map_err`'d to `Triangulation`; the sole
  `add_constraint` is preceded by `can_add_constraint`; identical consecutive vertices short-circuit;
  DoS caps and finiteness run before any triangulation. `From<DgmError>` maps all variants to `400`.
- **Endpoint 4xx-not-5xx (focus #3):** length ∉ {9,27,105} → `BadBandCount` 400 before allocation;
  non-finite → 400; `R>1000` reaches `IsolationSpectrum::new` unclamped → 400 (07-01 clamp removal
  verified in `interpolate.rs` + `does_not_clamp_out_of_range` test); `spl-to-lw` positivity/finite
  gated; MED-1 path-leak redaction intact in `error.rs`.
- **D-06 authored-only (focus #4):** no persisted/serde `r_db`; only the derived response + docs.
- **SVC-07 no client acoustic math (focus #5):** interpolation, SPL→L_W both server calls; client does
  band-index stride math + turf geometry only.
- **XSS (focus #6):** zero `dangerouslySetInnerHTML`/`innerHTML`; SVG built as React children.
- **React effect hygiene (focus #7):** `useTerraDraw`, `useDgmTrigger`, `useAutosave`, `useFreqAxis`,
  `useSpectrumPreview`, `ZoomController`, `ResizeOnMount` each tear down subscriptions/listeners/
  timers/`AbortController`/`rAF` in cleanup.
- **SC4 rebuild-on-style.load (focus #9):** `onStyleLoad` fully `teardownDraw()`s (off handlers +
  guarded `draw.stop()`) before `buildDraw()`; StrictMode guarded by `drawRef`/`disposed`.
- **Invented tokens (focus #10):** `theme.css` byte-identical to the metrao3 source; `app.css` uses
  no raw hex; SVG layout px are documented coordinate-system constants, not tokens.
- **CLAUDE.md invariants (focus #11):** engine byte-identical, conj gate 0, no C deps, `deny(unsafe_code)`,
  no CI, English-only.
- **Honest stubs (focus #12):** TIN, spectrum preview, SPL→L_W are genuinely server-computed;
  deferrals (spectrum persistence, map paint) are documented in 07-08-SUMMARY, not faked.

---

_Reviewed: 2026-07-10_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: deep_
