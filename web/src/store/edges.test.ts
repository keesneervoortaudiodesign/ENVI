// edges.test.ts — the LOAD-BEARING unit tests for the D-02 per-edge UUID ring-diff (RESEARCH Pattern 5).
//
// # Why this file exists (no-false-green house rule)
// A per-façade isolation spectrum keyed by an edge UUID silently re-points at the WRONG façade after any
// vertex insert unless the ring-diff recovers edge identity correctly — a data-corruption bug NO
// integration test catches (the geometry still looks right; only the acoustic assignment is wrong). These
// tests enumerate and assert, BY NAME, the four reconciliation cases the store depends on, plus the
// no-silent-repoint invariant. Run by Vitest: `npm run test:unit` (verify: `npx tsc --noEmit &&
// npm run test:unit`, with NO `2>/dev/null` and NO `|| fallback`).

import { describe, expect, it } from "vitest";

import { initEdgeIds, ringDiff, type Coord } from "./edges";

// A fixed unit square, distinct vertices in ring order (no closing duplicate): A→B→C→D→(wrap)→A.
const A: Coord = [0, 0];
const B: Coord = [1, 0];
const C: Coord = [1, 1];
const D: Coord = [0, 1];
const SQUARE: readonly Coord[] = [A, B, C, D];

// Stable, human-readable edge ids so failures are legible (real ids are crypto.randomUUID()).
const EDGE_AB = "edge-AB";
const EDGE_BC = "edge-BC";
const EDGE_CD = "edge-CD";
const EDGE_DA = "edge-DA";
const SQUARE_IDS = [EDGE_AB, EDGE_BC, EDGE_CD, EDGE_DA];

describe("initEdgeIds", () => {
  it("mints one fresh UUID per ring edge (n vertices ⇒ n edges, incl. the wrap edge)", () => {
    const ids = initEdgeIds(SQUARE);
    expect(ids).toHaveLength(SQUARE.length);
    expect(new Set(ids).size).toBe(SQUARE.length); // all distinct
    // real UUID shape (defence: not a placeholder / index)
    for (const id of ids) {
      expect(id).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);
    }
  });
});

describe("ringDiff — IDENTITY (unchanged ring)", () => {
  it("preserves EVERY edge UUID byte-identical and leaves the façade map untouched", () => {
    const facade = { [EDGE_CD]: { resolution: "octave", values: [1, 2, 3] } } as const;
    const result = ringDiff(SQUARE, SQUARE_IDS, [A, B, C, D]);

    expect(result.kind).toBe("identity");
    // byte-identical, position-for-position — the cheapest guard against a diff that "recovers" by
    // regenerating all UUIDs.
    expect(result.edgeIds).toEqual(SQUARE_IDS);
    expect(result.edgeIds).toHaveLength(4);
    // façade untouched
    expect(result.reconcileFacade(facade)).toEqual(facade);
  });
});

describe("ringDiff — MOVE (one vertex dragged, same vertex count)", () => {
  it("preserves all edge_ids and the façade map (endpoints keep identity)", () => {
    const movedC: Coord = [1.5, 1.2]; // C dragged
    const facade = { [EDGE_BC]: { resolution: "third", values: [9] } } as const;
    const result = ringDiff(SQUARE, SQUARE_IDS, [A, B, movedC, D]);

    expect(result.kind).toBe("move");
    expect(result.edgeIds).toEqual(SQUARE_IDS); // unchanged
    expect(result.reconcileFacade(facade)).toEqual(facade);
  });
});

describe("ringDiff — INSERT (one vertex added, edge splits into two)", () => {
  it("keeps the parent UUID on the FIRST half + one fresh UUID on the second; both inherit the spectrum", () => {
    // Insert W on edge B→C (parent EDGE_BC): next ring A, B, W, C, D.
    const W: Coord = [1, 0.5];
    const facadeSpectrum = { resolution: "octave", values: [10, 20] } as const;
    const facade = { [EDGE_BC]: facadeSpectrum };

    const result = ringDiff(SQUARE, SQUARE_IDS, [A, B, W, C, D]);

    expect(result.kind).toBe("insert");
    expect(result.edgeIds).toHaveLength(5);

    // The parent UUID survives on the first half (B→W).
    expect(result.edgeIds).toContain(EDGE_BC);
    // Exactly one fresh UUID was minted (the second half, W→C); the other three unchanged edges survive.
    const known = new Set(SQUARE_IDS);
    const fresh = result.edgeIds.filter((id) => !known.has(id));
    expect(fresh).toHaveLength(1);
    const secondHalfId = fresh[0];
    expect(secondHalfId).toMatch(/^[0-9a-f]{8}-/); // a real UUID, not an index/placeholder
    // The three edges NOT incident to the insertion are byte-identical.
    expect(result.edgeIds).toContain(EDGE_AB);
    expect(result.edgeIds).toContain(EDGE_CD);
    expect(result.edgeIds).toContain(EDGE_DA);

    // BOTH halves inherit the parent spectrum: parent id keeps its entry, the fresh id gets a copy.
    const reconciled = result.reconcileFacade(facade);
    expect(reconciled[EDGE_BC]).toEqual(facadeSpectrum);
    expect(reconciled[secondHalfId]).toEqual(facadeSpectrum);
  });

  it("copies nothing when the split edge had no override (both halves inherit the building default)", () => {
    const W: Coord = [1, 0.5];
    const result = ringDiff(SQUARE, SQUARE_IDS, [A, B, W, C, D]);
    // No facade entry for the parent → nothing to copy; the map stays empty.
    expect(result.reconcileFacade({})).toEqual({});
  });
});

describe("ringDiff — DELETE (one vertex removed, two edges merge)", () => {
  it("merges keeping the FIRST edge's UUID and drops the second edge's spectrum entry", () => {
    // Remove C: edges B→C (EDGE_BC) and C→D (EDGE_CD) merge into B→D. Keep the first (EDGE_BC).
    const facade = {
      [EDGE_BC]: { resolution: "octave", values: [1] },
      [EDGE_CD]: { resolution: "octave", values: [2] },
    } as const;

    const result = ringDiff(SQUARE, SQUARE_IDS, [A, B, D]);

    expect(result.kind).toBe("delete");
    expect(result.edgeIds).toHaveLength(3);
    // The first edge's UUID is kept for the merged edge; the second is gone.
    expect(result.edgeIds).toContain(EDGE_BC);
    expect(result.edgeIds).not.toContain(EDGE_CD);
    // The two untouched edges survive.
    expect(result.edgeIds).toContain(EDGE_AB);
    expect(result.edgeIds).toContain(EDGE_DA);

    const reconciled = result.reconcileFacade(facade);
    expect(reconciled[EDGE_BC]).toEqual({ resolution: "octave", values: [1] }); // first kept
    expect(reconciled[EDGE_CD]).toBeUndefined(); // second dropped
  });
});

describe("ringDiff — NO SILENT RE-POINTING (the whole reason D-02 exists)", () => {
  it("a façade override on edge C→D still maps to the C→D segment after an insert ELSEWHERE in the ring", () => {
    const overrideSpectrum = { resolution: "twelfth", values: [42] } as const;
    const facade = { [EDGE_CD]: overrideSpectrum };

    // Insert W on the UNRELATED edge A→B (next ring A, W, B, C, D) — nowhere near C→D.
    const W: Coord = [0.5, 0];
    const next: readonly Coord[] = [A, W, B, C, D];
    const result = ringDiff(SQUARE, SQUARE_IDS, next);

    expect(result.kind).toBe("insert");
    // EDGE_CD survives untouched…
    expect(result.edgeIds).toContain(EDGE_CD);
    // …and its position in the new ring is still the geometric C→D segment.
    const cdIndex = result.edgeIds.indexOf(EDGE_CD);
    const from = next[cdIndex];
    const to = next[(cdIndex + 1) % next.length];
    expect(from).toEqual(C);
    expect(to).toEqual(D);
    // …and its override spectrum is unchanged (NOT re-pointed to the newly-split edge).
    expect(result.reconcileFacade(facade)[EDGE_CD]).toEqual(overrideSpectrum);
  });
});

describe("ringDiff — REBUILD fallback (unclassifiable multi-vertex delta)", () => {
  it("mints fresh UUIDs and drops overrides rather than silently re-pointing", () => {
    // Two vertices added at once (not a single TD insert) → cannot safely reconcile.
    const result = ringDiff(SQUARE, SQUARE_IDS, [A, B, C, D, [2, 2], [3, 3]]);
    expect(result.kind).toBe("rebuild");
    expect(result.edgeIds).toHaveLength(6);
    expect(result.edgeIds.some((id) => SQUARE_IDS.includes(id))).toBe(false); // all fresh
    expect(result.reconcileFacade({ [EDGE_CD]: { resolution: "octave", values: [1] } })).toEqual({});
  });
});

describe("ringDiff — SAME-COUNT insert+delete is NOT trusted as a MOVE (ME-02, the D-02 gap)", () => {
  it("rebuilds (drops overrides) rather than positionally re-pointing when >1 vertex differs", () => {
    // A same-vertex-count delta that is NOT a pure move: delete B and insert E ⇒ [A, C, E, D]. Two positions
    // differ (B→C, C→E), so a positional-preserve would silently re-point EDGE_CD onto a different segment.
    const E: Coord = [2, 2];
    const overrideSpectrum = { resolution: "twelfth", values: [42] } as const;
    const facade = { [EDGE_CD]: overrideSpectrum };

    const result = ringDiff(SQUARE, SQUARE_IDS, [A, C, E, D]);

    // Fail-safe: this is unclassifiable as a single-vertex move → rebuild, NOT "move".
    expect(result.kind).toBe("rebuild");
    expect(result.edgeIds).toHaveLength(4);
    expect(result.edgeIds.some((id) => SQUARE_IDS.includes(id))).toBe(false); // all fresh — no positional keep
    // The override is DROPPED loudly (façade reverts to the building default) — never silently re-mapped to a
    // now-geometrically-different edge.
    expect(result.reconcileFacade(facade)[EDGE_CD]).toBeUndefined();
    expect(result.reconcileFacade(facade)).toEqual({});
  });

  it("still treats a genuine single-vertex move (exactly one position differs) as MOVE", () => {
    // Guard the guard: a real drag of one vertex must NOT regress into a rebuild.
    const movedC: Coord = [1.5, 1.2];
    const facade = { [EDGE_BC]: { resolution: "third", values: [9] } } as const;
    const result = ringDiff(SQUARE, SQUARE_IDS, [A, B, movedC, D]);
    expect(result.kind).toBe("move");
    expect(result.edgeIds).toEqual(SQUARE_IDS);
    expect(result.reconcileFacade(facade)).toEqual(facade);
  });
});

describe("ringDiff — DUPLICATE coordinates fall back to REBUILD (ME-03)", () => {
  it("rebuilds when an inserted vertex duplicates an existing ring coordinate", () => {
    // Insert a vertex whose coordinate exactly equals B: next ring [A, B, B, C, D]. The coordinate-identity
    // probes (split match, prevEdgeLookup) are ambiguous here, so the only safe answer is a fresh rebuild.
    const overrideSpectrum = { resolution: "octave", values: [10, 20] } as const;
    const facade = { [EDGE_BC]: overrideSpectrum };

    const result = ringDiff(SQUARE, SQUARE_IDS, [A, B, B, C, D]);

    expect(result.kind).toBe("rebuild");
    expect(result.edgeIds).toHaveLength(5);
    expect(result.edgeIds.some((id) => SQUARE_IDS.includes(id))).toBe(false); // all fresh
    // No ambiguous re-point: the override is dropped, not mis-assigned to a collapsed lookup entry.
    expect(result.reconcileFacade(facade)).toEqual({});
  });

  it("rebuilds when the PREVIOUS ring already carried a duplicate coordinate (collapsed lookup)", () => {
    // A degenerate prev footprint with B duplicated collapses prevEdgeLookup — do not trust any recovery.
    const dupPrev: readonly Coord[] = [A, B, B, C];
    const dupIds = ["e-0", "e-1", "e-2", "e-3"];
    const result = ringDiff(dupPrev, dupIds, [A, B, C]);
    expect(result.kind).toBe("rebuild");
    expect(result.reconcileFacade({ "e-1": { resolution: "octave", values: [1] } })).toEqual({});
  });
});
