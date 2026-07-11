# Phase 08 — Deferred Items

Out-of-scope discoveries logged during execution (not fixed here — see the deviation SCOPE BOUNDARY).

## 08-08

### [DEFER] Re-importing an already-imported layer into a populated scene traps the WASM (`unreachable`)

- **Found during:** 08-08 Task 2 (building the DATA-04 replay).
- **Symptom:** After a viewport import succeeds, running the import AGAIN for the same viewport
  (e.g. clicking "Import (viewport)" twice, which re-fires every enabled layer) makes the
  terrain and land-cover layers fail with a WASM `unreachable` trap. The trap is in the
  `merge_features` (D-09) path when the incoming features' identities already exist in the
  existing scene (re-import of a *succeeded* layer).
- **Not a blocker for 08-08:** the plan's success criteria (a first-time import journey + the
  DATA-04 network-off proof) do not require re-importing a succeeded layer. The DATA-04 replay
  models the compute read against a reset scene (same project id ⇒ OPFS cache persists), which
  is the honest OPFS-read exercise and avoids this path. The `retryLayer` flow that a *failed*
  layer uses is unaffected — a failed layer committed nothing, so its retry merges into a scene
  without its own prior features (covered green by the Overpass-429 retry test).
- **Impact if unfixed:** a user who clicks "Import (viewport)" twice sees terrain/land cover
  error on the second click. Pre-existing in 08-07 (import E2E was deferred to 08-08), not
  introduced by this plan.
- **Suggested owner:** a follow-up fix in `crates/envi-gis/src/merge.rs` (make the D-09 merge
  idempotent/panic-free when an incoming feature identity collides with an existing imported
  one), with a WASM-boundary regression test.
