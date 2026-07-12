---
phase: 11-results-fast-recalc
reviewed: 2026-07-12T00:00:00Z
depth: deep
files_reviewed: 23
files_reviewed_list:
  - web/src/store/results.ts
  - web/src/store/results.test.ts
  - web/src/store/colorScale.ts
  - web/src/store/colorScale.test.ts
  - web/src/store/conditioning.ts
  - web/src/store/conditioning.test.ts
  - web/src/store/stale.ts
  - web/src/store/stale.test.ts
  - web/src/store/difference.ts
  - web/src/store/scenarios.ts
  - web/src/store/scenarios.test.ts
  - web/src/store/exportUi.ts
  - web/src/store/exportUi.test.ts
  - web/src/compute/marshalScene.ts
  - web/src/compute/opfs.ts
  - web/src/help/catalog.ts
  - web/src/help/controlIds.ts
  - web/src/help/coverage.test.ts
  - web/src/help/InfoButton.tsx
  - web/src/help/InfoButton.test.ts
  - web/src/generated/wire.ts
  - web/src/testBridge.ts
  - web/src/import/wasm.ts
findings:
  critical: 2
  warning: 4
  info: 2
  total: 8
status: issues_found
---

# Phase 11: Code Review Report (web state/logic + help + WASM glue)

**Reviewed:** 2026-07-12
**Depth:** deep
**Files Reviewed:** 23
**Status:** issues_found

## Summary

Reviewed the TypeScript state/logic layer of the acoustics results UI at deep depth,
tracing async call chains across the zustand stores, the WASM client seams, the OPFS
glue, and the help catalog. The invariants the layer is built around hold on the
**happy path**: D-01 is respected (no `Math.log/pow/exp` dB arithmetic anywhere — the
only `Math.sqrt` is a lattice-side length in `marshalScene`, geometry not acoustics);
band aggregation is documented as index-based; `wire.ts` is a genuinely generated
artifact with a no-drift test; `testBridge` is correctly gated behind
`import.meta.env.DEV` at its `main.tsx` call site so it never ships to prod; and the
help coverage test is non-vacuous (it enforces `CONTROL_IDS.length >= 50` and a
catalog↔ids bijection, so it cannot pass with an empty set or an uncovered control).

The defects are concentrated in **async ordering**. Three of the stores fire
long-lived async work (WASM MAC over OPFS reads, blake3 re-mint, full solve) from
rapidly-repeatable user actions but apply the results with **no generation/epoch
guard**. Under realistic timing this produces exactly the failure the phase mandate
forbids: a stale readout served as current, and — worse — a `false-green` stale badge
that can ungate a stale export. Two of those are classified Critical. The remaining
findings are a wire-type drift risk (an untyped inline mirror of a generated DTO), an
in-flight recalc that a reset/new-solve does not abort, and a silent partial CSV export.

## Critical Issues

### CR-01: Conditioning fast-recalc has no in-flight generation guard — an out-of-order MAC serves a stale spectrum/map as current

**File:** `web/src/store/conditioning.ts:190-238` (`scheduleRecondition` + `runRecondition`)
**Issue:**
The debounce coalesces edits *within* one 150 ms window, but it does **not** serialize
dispatches. Once the timer fires, `runRecondition` starts an `await client.recondition(...)`
whose real client (`createWasmConditioningClient`) awaits an OPFS `readChunk` per span —
genuinely async and, for a large tensor, slower than 150 ms. A user who keeps adjusting a
gain/delay slider will pause >150 ms while the first MAC is still in flight, scheduling a
**second** `runRecondition`. The two dispatches now race, and the store applies whichever
resolves last with no ordering check:

```ts
const { readouts } = await client.recondition({ manifest, perSourceConditioning });
useResultsStore.getState().applyConditioning(perSourceConditioning, readouts);
feedIsophoneFromReadouts(manifest, readouts);
set({ pending: false, refuse: false, recalcEpoch: get().recalcEpoch + 1 });
```

**Failure scenario:** Dispatch A (gain = 6) fires, then dispatch B (gain = 9) fires while
A is still reading OPFS. A resolves *after* B. The results store ends up showing A's
readouts (gain = 6) and `manifest.perSourceConditioning = [gain 6]`, while
`conditioning.perSource` shows gain = 9. The spectrum panel and isophone map now display a
value that does not match the current drive — a silently-served stale result — and
`recalcEpoch` (the "updated live" honest-state signal) is double-bumped out of order. It
self-heals only on the next in-order completion.

**Fix:** Stamp each dispatch with a monotonic id and drop late results:

```ts
let reconditionGen = 0; // module scope, reset in reset()
async function runRecondition(get, set) {
  const gen = ++reconditionGen;
  // ...
  const { readouts } = await client.recondition({ manifest, perSourceConditioning });
  if (gen !== reconditionGen) return;            // a newer dispatch supersedes this one
  if (useResultsStore.getState().manifest !== manifest) return; // manifest swapped under us
  useResultsStore.getState().applyConditioning(perSourceConditioning, readouts);
  feedIsophoneFromReadouts(manifest, readouts);
  set({ pending: false, refuse: false, recalcEpoch: get().recalcEpoch + 1 });
}
```
(The same `gen` check also fixes the `pending`/`refuse` flags being cleared by a
superseded dispatch.)

### CR-02: Stale-badge re-mint has no generation guard — an out-of-order remint can show `false-green` and ungate a stale export

**File:** `web/src/store/stale.ts:55-62` (`checkStale`) and `web/src/store/stale.ts:104-121` (`useStaleWatch`)
**Issue:**
`useStaleWatch` re-runs on every committed scene edit and calls the async
`buildPrepareScene(...)` then the async `checkStale(...)`. The effect's `cancelled` flag
only guards the gap *between* `buildPrepareScene` resolving and `checkStale` being called
— it does **not** cancel the in-flight `client.remint(scene)` inside `checkStale`, which
unconditionally does `set({ isStale: current !== cachedHash })`. Two rapid edits can leave
two remints in flight; the **last to resolve wins**, regardless of which scene is current.

**Failure scenario:** Edit A returns the scene to a state matching the cached tensor
(should be *not* stale); edit B then diverges the scene (should be *stale*). If remint B
resolves first (`isStale = true`) and remint A resolves last (`isStale = false`), the badge
ends **green while the scene is divergent** — a false-green. Because the ExportMenu gates
the export affordance on this badge (and `catalog.ts` "export.open" documents "Export is
disabled … while the result is out of date"), a false-green here lets the user export a map
that no longer matches the scene — the exact honest-state failure D-12 exists to prevent.

**Fix:** Guard `checkStale` with a generation counter (or thread the effect's `cancelled`
into it), so only the newest request may write `isStale`:

```ts
let staleGen = 0;
checkStale: async (scene, cachedHash) => {
  const client = get().client;
  if (!client) return;
  const gen = ++staleGen;
  const current = await client.remint(scene);
  if (gen !== staleGen) return;      // superseded by a newer scene edit
  set({ isStale: current !== cachedHash });
},
```

## Warnings

### WR-01: `computeScenario` marks a scenario `computed: true` even if its met changed during the solve — false-green cached scenario

**File:** `web/src/store/scenarios.ts:227-253`
**Issue:**
`computeScenario` captures `scenario` (its `met`) at the start, then awaits `derive` and
`solve`. On success it maps over the **current** scenario list and sets
`{ derived, solve, computed: true }` for that id — with no check that the met is still the
one that was solved. `setMet` (line 212) correctly sets `computed: false`, but a `setMet`
that lands *during* the in-flight solve is overwritten back to `computed: true` by the
completing solve, whose `solve`/`derived` are for the **old** met.

**Failure scenario:** User clicks Compute (met v1), then edits temperature (met v2,
`computed:false`); the v1 solve resolves and flips the scenario to `computed: true` with a
cached tensor/totals for v1 while `met` is v2. The list shows the scenario as computed/
current, and a Compare (difference map) will subtract a tensor that does not match the
displayed met. Likely mitigated if the panel disables met edits while `computingId === id`,
but the store must not depend on that.

**Fix:** Before the success `set`, re-read the scenario and bail if its met changed:

```ts
const still = get().scenarios.find((s) => s.id === id);
if (!still || still.met !== scenario.met) return; // met edited mid-solve — discard
```
(or stamp a per-scenario met generation and compare.)

### WR-02: Recondition/readout WASM request is built as an untyped inline object, not the generated `ReconditionReq` DTO — silent wire drift

**File:** `web/src/store/conditioning.ts:265-269`, `web/src/store/results.ts:239-243`
**Issue:**
Both readout sites hand-assemble the request as an inline object literal:

```ts
const request = {
  tensor_hash: manifest.tensorHash,
  per_source_conditioning: perSourceConditioning,
  receiver_ids: span.receiverIds,
};
g.readout_receivers(manifest.scene, request, tensor, pincoh) as ReadoutResult;
```

A generated DTO for exactly this shape already exists — `ReconditionReq`
(`generated/wire.ts:1537`, fields `tensor_hash` / `per_source_conditioning` /
`receiver_ids`) — but neither site annotates against it. This is the "hand-written TS
mirror of a Rust DTO" the project decision D-10 forbids: because the literal is untyped,
a Rust-side field rename would compile clean and fail only at the `deny_unknown_fields`
WASM boundary in the browser, defeating the generated-types contract.

**Fix:** Import and annotate the request so drift is a `tsc` error:

```ts
import type { ReconditionReq } from "../generated/wire";
const request: ReconditionReq = {
  tensor_hash: manifest.tensorHash,
  per_source_conditioning: [...perSourceConditioning],
  receiver_ids: [...span.receiverIds],
};
```

### WR-03: `reset()` / a new-solve manifest swap does not abort an in-flight recondition — resurrected epoch and cross-tensor readout merge

**File:** `web/src/store/conditioning.ts:201-207` (`reset`), `web/src/store/conditioning.ts:214-238` (`runRecondition`)
**Issue:**
`reset()` clears the debounce *timer* but cannot stop a `runRecondition` that has already
fired and is awaiting the MAC. When that in-flight call resolves it still runs
`applyConditioning(...)` and `set({ ..., recalcEpoch: get().recalcEpoch + 1, refuse: false })`
against whatever state now exists. Two concrete bad outcomes: (a) after `reset()`, epoch is
resurrected 0→1 and stale readouts are re-applied; (b) if a fresh solve called
`setManifest` (new `tensorHash`) while the MAC was in flight, `applyConditioning` merges the
**old-tensor** reconditioned readouts and overwrites `manifest.perSourceConditioning` on the
**new** manifest — stale spectra keyed against a new tensor identity.

**Fix:** The generation guard from CR-01 covers this — reset should bump the generation
(`reconditionGen++`) so any in-flight dispatch is discarded on completion, and
`runRecondition` should verify `useResultsStore.getState().manifest === manifest` before
applying.

### WR-04: `collectReadouts` silently drops receivers, so a partial CSV is presented as a complete export

**File:** `web/src/store/exportUi.ts:197-228`
**Issue:**
The CSV collector reuses cached readouts and fetches uncached ones through the readout
client, but a receiver is **silently skipped** when the client is absent (`!client`) or no
covering span is found (`if (span)` with no `else`). The header comment claims the CSV
"covers EVERY receiver", yet if the `ReadoutClient` was never attached, *every* uncached
receiver is dropped and `downloadExport` still resolves "successfully" — writing a CSV with
missing (or zero) receiver columns and no error.

**Failure scenario:** Export CSV is invoked before any receiver was viewed and before the
readout client is attached → an empty/partial spectra file is saved with a valid filename
and footer, implying a complete export.

**Fix:** Fail loudly on an incomplete collection rather than emitting a partial file:

```ts
if (!readout) {
  if (!client) throw new Error("Cannot export spectra: readout engine unavailable.");
  // span-missing is a manifest inconsistency — surface it, don't drop silently
  throw new Error(`No chunk span for receiver ${r.id}; export aborted.`);
}
```

## Info

### IN-01: `ReadoutResult.stale` is never asserted (missed defense-in-depth on the honest-state gate)

**File:** `web/src/store/results.ts:244-249`, `web/src/store/conditioning.ts:273-279`
**Issue:** `ReadoutResult` carries a `stale: boolean` (`wire.ts:1382`, documented "always
false — a mismatched hash is refused, never served stale"). Both consumers cast to
`ReadoutResult` and read `result.receivers` without ever checking `result.stale`. The
invariant is currently upheld by the WASM throwing on mismatch, so this is not a live bug,
but an explicit `if (result.stale) throw …` would make the honest-state contract enforced
on the client too, so a future engine regression that flags rather than throws cannot leak
a stale readout.
**Fix:** Add a cheap assertion after each `readout_receivers` call: `if (result.stale) throw new Error("refused: stale readout")`.

### IN-02: `selectReceiver` can leave `loadingReceiverId` pointing at a previous in-flight fetch when a cached receiver is selected

**File:** `web/src/store/results.ts:162-179`
**Issue:** When a receiver whose readout is already cached is selected, the function
returns early (`readouts[id]` truthy) without touching `loadingReceiverId`. If a fetch for a
*different* receiver is still in flight, the loading indicator stays attached to that other
id until it resolves, so a brief mislabeled spinner can appear on the newly-selected (but
already-loaded) receiver. Cosmetic only — no data correctness impact.
**Fix:** Not required; if desired, clear `loadingReceiverId` when selecting an already-cached
receiver, or key the spinner strictly off `selectedReceiverId === loadingReceiverId`.

---

_Reviewed: 2026-07-12_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: deep_
