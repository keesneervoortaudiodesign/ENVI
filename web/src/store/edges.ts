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

// Exact-`f64` coordinate identity (RESEARCH Pattern 5: match vertices by the coords TD echoes).
function sameCoord(a: Coord, b: Coord): boolean {
  return a[0] === b[0] && a[1] === b[1];
}

// Whole-ring coordinate equality (used to locate the single inserted / deleted vertex).
function ringEquals(a: readonly Coord[], b: readonly Coord[]): boolean {
  if (a.length !== b.length) {
    return false;
  }
  for (let i = 0; i < a.length; i++) {
    if (!sameCoord(a[i], b[i])) {
      return false;
    }
  }
  return true;
}

// The ring with vertex `i` removed (order preserved) — the probe used to find an insert/delete position.
function withoutAt(ring: readonly Coord[], i: number): Coord[] {
  return ring.filter((_, k) => k !== i);
}

// A directed edge's coordinate-pair key, used to map an UNCHANGED next-edge back to its prev UUID. Edges
// are directed in ring order, so `(from → to)` is unambiguous for a footprint with distinct vertices.
function pairKey(from: Coord, to: Coord): string {
  return `${from[0]},${from[1]}|${to[0]},${to[1]}`;
}

// Build the `pairKey → edgeId` lookup for every prev edge `prevRing[i] → prevRing[(i+1) % n]`.
function prevEdgeLookup(prevRing: readonly Coord[], prevEdgeIds: readonly string[]): Map<string, string> {
  const n = prevRing.length;
  const lookup = new Map<string, string>();
  for (let i = 0; i < n; i++) {
    lookup.set(pairKey(prevRing[i], prevRing[(i + 1) % n]), prevEdgeIds[i]);
  }
  return lookup;
}

// One fresh UUID per ring edge — the initial assignment when a building is first drawn.
export function initEdgeIds(ring: readonly Coord[]): string[] {
  return ring.map(() => crypto.randomUUID());
}

// True when two ring vertices share an exact coordinate. Duplicate coordinates make the exact-`f64`
// matching the diff relies on (INSERT/DELETE probes, `prevEdgeLookup`) AMBIGUOUS: a `pairKey` collapses so
// two distinct directed edges map to one UUID, and a `sameCoord(to, w)` split probe matches more than one
// edge — either of which would silently re-point a façade spectrum. When present, the only safe answer is
// `rebuild` (ME-03).
function hasDuplicateCoords(ring: readonly Coord[]): boolean {
  const seen = new Set<string>();
  for (const c of ring) {
    const key = `${c[0]},${c[1]}`;
    if (seen.has(key)) {
      return true;
    }
    seen.add(key);
  }
  return false;
}

// The fail-safe result: mint fresh UUIDs for every next edge and DROP all overrides (a façade reverts to
// the building default — visible and loud) rather than risk silently re-pointing a spectrum at the wrong
// façade. This is the only safe answer whenever per-edge identity cannot be PROVEN (the exact class of
// data-corruption D-02 exists to prevent).
function rebuildResult(nextRing: readonly Coord[]): RingDiffResult {
  return {
    kind: "rebuild",
    edgeIds: nextRing.map(() => crypto.randomUUID()),
    reconcileFacade: () => ({}),
  };
}

export function ringDiff(
  prevRing: readonly Coord[],
  prevEdgeIds: readonly string[],
  nextRing: readonly Coord[],
): RingDiffResult {
  const prevN = prevRing.length;
  const nextN = nextRing.length;

  // GUARD (ME-03): a ring carrying duplicate coordinates makes every coordinate-identity match ambiguous
  // (`prevEdgeLookup` keys collapse; the INSERT/DELETE split probes match >1 edge). Rebuild rather than
  // proceed with an ambiguous match that could re-point a spectrum. Checking both rings covers a collapsed
  // prev lookup AND a next ring whose inserted vertex duplicates an existing coordinate.
  if (hasDuplicateCoords(prevRing) || hasDuplicateCoords(nextRing)) {
    return rebuildResult(nextRing);
  }

  // IDENTITY / MOVE — same vertex count. Edge identity is positional: the edge between vertex `i` and
  // `i+1` keeps its UUID even as endpoints move (D-02 MOVE), so the façade map is untouched. IDENTITY is
  // the strict sub-case where every coordinate is also unchanged (byte-identical edge ids either way).
  if (prevN === nextN) {
    const identical = ringEquals(prevRing, nextRing);
    if (!identical) {
      // Positional preservation is only SOUND when the same-count change is provably a single-vertex MOVE
      // — i.e. at most one vertex position differs. A same-count insert+delete or a vertex reorder differs
      // in ≥2 positions and is NOT a move: keeping edge UUIDs positionally would re-point a per-edge
      // override at a geometrically different façade (the D-02 gap, ME-02). Fail safe to `rebuild` — drop
      // overrides loudly (façade reverts to the building default, visibly) rather than silently re-point.
      let changed = 0;
      for (let i = 0; i < prevN; i++) {
        if (!sameCoord(prevRing[i], nextRing[i])) {
          changed++;
        }
      }
      if (changed > 1) {
        return rebuildResult(nextRing);
      }
    }
    return {
      kind: identical ? "identity" : "move",
      edgeIds: prevEdgeIds.slice(),
      reconcileFacade: (facade) => ({ ...facade }),
    };
  }

  // INSERT — exactly one vertex added. Find the new vertex position `j` (removing it recovers prevRing);
  // its parent edge is `prev[(j-1) → j]`. The parent UUID stays on the FIRST half; the second half gets a
  // fresh UUID (Assumption A5). Both halves inherit the parent's spectrum.
  if (nextN === prevN + 1) {
    let j = -1;
    for (let k = 0; k < nextN; k++) {
      if (ringEquals(withoutAt(nextRing, k), prevRing)) {
        j = k;
        break;
      }
    }
    if (j >= 0) {
      const parentIndex = (j - 1 + prevN) % prevN; // prev edge prev[parentIndex] → prev[parentIndex+1]
      const parentId = prevEdgeIds[parentIndex];
      const secondHalfId = crypto.randomUUID();
      const w = nextRing[j];
      const lookup = prevEdgeLookup(prevRing, prevEdgeIds);
      const edgeIds: string[] = [];
      for (let k = 0; k < nextN; k++) {
        const from = nextRing[k];
        const to = nextRing[(k + 1) % nextN];
        if (sameCoord(to, w)) {
          edgeIds.push(parentId); // first half: prev[parentIndex] → w
        } else if (sameCoord(from, w)) {
          edgeIds.push(secondHalfId); // second half: w → prev[parentIndex+1]
        } else {
          edgeIds.push(lookup.get(pairKey(from, to)) ?? crypto.randomUUID());
        }
      }
      return {
        kind: "insert",
        edgeIds,
        // Both halves inherit the parent spectrum: the parent id keeps its entry (first half), the fresh
        // second-half id gets a copy. If the parent had no override, neither half does (both inherit the
        // building default) — nothing to copy.
        reconcileFacade: <T,>(facade: Readonly<Record<string, T>>): Record<string, T> => {
          const next: Record<string, T> = { ...facade };
          if (Object.prototype.hasOwnProperty.call(facade, parentId)) {
            next[secondHalfId] = facade[parentId];
          }
          return next;
        },
      };
    }
  }

  // DELETE — exactly one vertex removed. The two edges adjacent to the removed vertex merge into one; keep
  // the FIRST edge's UUID (the merged edge), drop the second edge's spectrum entry (RESEARCH Pattern 5).
  if (nextN === prevN - 1) {
    let r = -1;
    for (let i = 0; i < prevN; i++) {
      if (ringEquals(withoutAt(prevRing, i), nextRing)) {
        r = i;
        break;
      }
    }
    if (r >= 0) {
      const firstIndex = (r - 1 + prevN) % prevN; // edge prev[r-1] → prev[r]
      const mergedId = prevEdgeIds[firstIndex]; // kept for the merged edge
      const droppedId = prevEdgeIds[r]; // edge prev[r] → prev[r+1], merged away
      const mergedFrom = prevRing[firstIndex]; // prev[r-1]
      const mergedTo = prevRing[(r + 1) % prevN]; // prev[r+1]
      const lookup = prevEdgeLookup(prevRing, prevEdgeIds);
      const edgeIds: string[] = [];
      for (let k = 0; k < nextN; k++) {
        const from = nextRing[k];
        const to = nextRing[(k + 1) % nextN];
        if (sameCoord(from, mergedFrom) && sameCoord(to, mergedTo)) {
          edgeIds.push(mergedId); // the merged edge prev[r-1] → prev[r+1]
        } else {
          edgeIds.push(lookup.get(pairKey(from, to)) ?? crypto.randomUUID());
        }
      }
      return {
        kind: "delete",
        edgeIds,
        reconcileFacade: <T,>(facade: Readonly<Record<string, T>>): Record<string, T> => {
          const next: Record<string, T> = { ...facade };
          delete next[droppedId]; // the merged-away second edge's spectrum entry
          return next;
        },
      };
    }
  }

  // REBUILD — a delta that is neither a single insert, a single delete, nor a same-count move. There is no
  // safe way to recover per-edge identity, so mint fresh UUIDs and drop all overrides rather than silently
  // re-point them at the wrong façade (the exact failure D-02 exists to prevent).
  return rebuildResult(nextRing);
}
