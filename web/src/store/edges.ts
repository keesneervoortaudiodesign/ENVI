// edges.ts — the per-edge UUID ring-diff recovery (D-02, RESEARCH Pattern 5): the load-bearing logic
// that keeps a building's per-façade isolation spectra pointing at the geometrically-correct segment
// across vertex insert / delete / move, even though Terra Draw never reports WHICH edge changed.
//
// # Module I/O
// - Input  the PREVIOUS footprint ring (ordered distinct vertices `[x, y]`, no closing duplicate), the
//   previous per-edge UUID list (`edgeIds[i]` is the edge `ring[i] → ring[(i+1) % n]`, so `n` vertices ⇒
//   `n` edges including the wrap edge), and the NEXT ring echoed by Terra Draw.
// - Output a `RingDiffResult`: the reconciled `edgeIds` for the next ring (SAME length as `nextRing`),
//   the classified `kind` (identity / move / insert / delete / rebuild), and a `reconcileFacade` mapping
//   instruction that rewrites a `facade` map (edge-UUID → spectrum) so a split edge's both halves inherit
//   the parent spectrum (INSERT) and a merged-away edge's entry is dropped (DELETE). This runs in the
//   STORE's `applyTerraDrawChange`, NEVER in a Terra Draw callback — TD echoes geometry, the store owns
//   the UUID↔spectrum identity (D-02/D-03).
// - Valid input range: `prevEdgeIds.length === prevRing.length`; rings are exact-`f64` coordinate lists
//   as TD echoes them (matched by exact identity, RESEARCH Pattern 5). A change that is neither a single
//   insert, a single delete, nor a same-count move falls back to `rebuild` (fresh UUIDs, overrides
//   dropped) — the only safe answer when the 1-vertex-delta assumption is violated.

// A footprint ring vertex — an exact `[x, y]` (lng/lat or SceneXY) as Terra Draw echoes it.
export type Coord = readonly [number, number];

// How the ring changed, relative to the previous one. `rebuild` is the safe fallback for a delta the
// deterministic 1-vertex diff cannot classify (drop overrides rather than silently re-point them).
export type RingDiffKind = "identity" | "move" | "insert" | "delete" | "rebuild";

// The result of reconciling `prevEdgeIds` against `nextRing`.
export interface RingDiffResult {
  readonly kind: RingDiffKind;
  // Reconciled per-edge UUIDs for `nextRing` (length === nextRing.length).
  readonly edgeIds: string[];
  // Rewrite a façade map (edge-UUID → spectrum `T`) to match the new edge set: INSERT copies the parent
  // spectrum onto the fresh second-half UUID (both halves inherit); DELETE drops the merged-away entry.
  reconcileFacade<T>(facade: Readonly<Record<string, T>>): Record<string, T>;
}

// STUB (RED): the ring-diff is specified by `edges.test.ts` first (TDD). The GREEN implementation lands
// in the next commit; until then every call fails loudly so no false green is possible.
export function initEdgeIds(_ring: readonly Coord[]): string[] {
  throw new Error("edges.initEdgeIds: not implemented");
}

export function ringDiff(
  _prevRing: readonly Coord[],
  _prevEdgeIds: readonly string[],
  _nextRing: readonly Coord[],
): RingDiffResult {
  throw new Error("edges.ringDiff: not implemented");
}
