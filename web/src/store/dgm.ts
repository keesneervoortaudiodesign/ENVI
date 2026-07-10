// dgm.ts — the DGM (server-side TIN) state slice (D-08, SC1). Holds the last triangulation and the last
// reject so the map overlay (07-07) and the 07-09 validation panel can read a single source of truth.
//
// # Module I/O
// - Input  the outcome of a `POST /api/v1/dgm/triangulate` from the debounced producer (`dgmTrigger`):
//   a success mesh (`setTriangulation`), a 4xx reject (`setReject`, status + path-redacted detail), or a
//   clear (`clearDgm`) when the elevation set is too small/degenerate to triangulate.
// - Output `triangulation` (the renderable TIN, or null) and `rejectReason` (the crit source the 07-09
//   ValidationPanel surfaces, or null). The TIN is NOT persisted — the elevation objects are the truth
//   (Open Question 3 RESOLVED); this slice is a derived, transient view.
// - Valid input range: `setTriangulation` clears any prior reject (success supersedes); `setReject`
//   leaves the last-good triangulation intact so the overlay does not flicker off on a transient reject.

import { create } from "zustand";

import type { DgmResp } from "../generated/wire";

// A structured triangulate reject: HTTP status + the server's path-redacted `detail` (rendered as TEXT).
export interface DgmReject {
  readonly status: number;
  readonly detail: string;
}

export interface DgmState {
  // The last successful TIN (vertices + triangles), or null when none / cleared.
  readonly triangulation: DgmResp | null;
  // The last 4xx reject (interior-crossing / degenerate breaklines), or null.
  readonly rejectReason: DgmReject | null;

  setTriangulation(tin: DgmResp): void;
  setReject(reject: DgmReject): void;
  clearDgm(): void;
}

export const useDgmStore = create<DgmState>((set) => ({
  triangulation: null,
  rejectReason: null,

  // A successful triangulation supersedes any prior reject.
  setTriangulation: (tin) => set({ triangulation: tin, rejectReason: null }),
  // A reject is recorded WITHOUT dropping the last-good mesh (the overlay keeps showing valid geometry).
  setReject: (reject) => set({ rejectReason: reject }),
  // Too few / collinear points: no request was worth firing — clear both the mesh and any stale reject.
  clearDgm: () => set({ triangulation: null, rejectReason: null }),
}));
