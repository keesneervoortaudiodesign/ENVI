// stale.ts — the results-stale HONEST-STATE guard (D-12). A results-stale badge must
// appear the MOMENT the re-minted blake3 tensor identity of the CURRENT scene diverges
// from the cached tensor's manifest hash — the scene changed since the result was
// computed, so the cached tensor no longer describes it. Conditioning edits are a
// READOUT parameter, structurally EXCLUDED from the tensor identity (Phase-6 D-07), so
// they never re-mint and NEVER stale (proven by the conditioning store never calling in
// here).
//
// # Module I/O
// - Input  the CURRENT marshalled scene (`PrepareSolveReq`, from the shared
//   `compute/marshalScene` — the SAME marshaller the Calc submit hashes) + the cached
//   tensor hash (the results manifest's `tensorHash`). The blake3 re-mint runs in WASM
//   (the `tensor_hash` identity export — envi-compute-wasm/identity.rs); this store
//   performs NO hashing itself.
// - Output `isStale` (the `.chip.warn` "Out of date" badge the ConditioningPanel shows;
//   it NEVER blocks edits — the state matrix "badge, never blocks edits") + the
//   injectable `IdentityClient` seam.
// - Valid input range: any `PrepareSolveReq`; a null cached hash clears the badge.

import { create } from "zustand";
import { useEffect } from "react";

import type { PrepareSolveReq } from "../generated/wire";
import { buildPrepareScene } from "../compute/marshalScene";
import { useSceneStore } from "./sceneStore";
import { useCalcStore } from "./calc";
import { useResultsStore } from "./results";

// The re-mint seam (injectable → Node-testable without wasm). The real client calls the
// `tensor_hash` WASM identity export — the SAME blake3 digest the tensor was keyed by.
export interface IdentityClient {
  remint(scene: PrepareSolveReq): Promise<string>;
}

export interface StaleState {
  // Whether the cached result is stale (the current scene identity ≠ the cached hash).
  readonly isStale: boolean;
  readonly client: IdentityClient | null;

  attachIdentityClient(client: IdentityClient): void;
  // Re-mint the CURRENT scene's identity and compare to the cached tensor hash: a
  // divergence sets `isStale` (the honest badge, D-12). Conditioning never calls this.
  checkStale(scene: PrepareSolveReq, cachedHash: string): Promise<void>;
  // Explicitly clear the badge (a fresh solve replaces the cached tensor).
  clear(): void;
  reset(): void;
}

export const useStaleStore = create<StaleState>((set, get) => ({
  isStale: false,
  client: null,

  attachIdentityClient: (client) => set({ client }),

  checkStale: async (scene, cachedHash) => {
    const client = get().client;
    if (!client) {
      return;
    }
    const current = await client.remint(scene);
    set({ isStale: current !== cachedHash });
  },

  clear: () => set({ isStale: false }),

  reset: () => set({ isStale: false }),
}));

// The REAL identity client: lazily instantiates the compute wasm module on the main
// thread (COOP/COEP holds `crossOriginIsolated`) and calls the `tensor_hash` export. A
// dynamic import keeps the wasm graph out of the Node unit-test module load.
export function createWasmIdentityClient(): IdentityClient {
  let glue: Promise<typeof import("../generated/wasm-compute/envi_compute_wasm")> | null = null;
  const ensureGlue = (): Promise<typeof import("../generated/wasm-compute/envi_compute_wasm")> => {
    glue ??= (async () => {
      const g = await import("../generated/wasm-compute/envi_compute_wasm");
      await g.default();
      return g;
    })();
    return glue;
  };
  return {
    async remint(scene) {
      const g = await ensureGlue();
      return g.tensor_hash(scene) as string;
    },
  };
}

// Watch the DRAWN scene + calc grid for an edit and re-mint the identity against the
// cached tensor hash (D-12). Fires on any committed scene edit (`commitEpoch`) or grid
// change; a divergent identity flips the stale badge. The dependency is the cached
// `tensorHash` (NOT the whole manifest), so a conditioning recalc — which swaps the
// manifest's conditioning drive but leaves `tensorHash` untouched — does NOT re-run this
// (the never-stale invariant, D-07). Mounted once by the ConditioningPanel.
export function useStaleWatch(): void {
  const commitEpoch = useSceneStore((s) => s.commitEpoch);
  const spacing = useCalcStore((s) => s.spacing_fine_m);
  const coarseMultiples = useCalcStore((s) => s.coarseMultiples);
  const tensorHash = useResultsStore((s) => s.manifest?.tensorHash ?? null);
  const checkStale = useStaleStore((s) => s.checkStale);
  const clear = useStaleStore((s) => s.clear);

  useEffect(() => {
    if (!tensorHash) {
      clear();
      return;
    }
    let cancelled = false;
    void buildPrepareScene(spacing, coarseMultiples).then((marshalled) => {
      // An incomplete scene (no calc_area / source) cannot be re-minted — leave the
      // badge as-is rather than falsely clearing it.
      if (cancelled || !marshalled) {
        return;
      }
      void checkStale(marshalled.scene, tensorHash);
    });
    return () => {
      cancelled = true;
    };
  }, [commitEpoch, spacing, coarseMultiples, tensorHash, checkStale, clear]);
}
