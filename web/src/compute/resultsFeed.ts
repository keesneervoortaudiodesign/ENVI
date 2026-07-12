// resultsFeed.ts — the PRODUCTION seam that turns a finished client-side solve into
// the results surfaces (SC1 spectrum + SC2 conditioning). When the compute worker
// flushes the FINE tier (its `TierComplete`), `applyResultsFeed` assembles the
// `ResultsManifest` from the submitted `CalcJobSpec` + that event and pushes it into
// the results store (and attaches the real readout/conditioning/stale clients) — the
// exact wiring the DEV `testBridge.seedConditioning` performs, but from a real solve
// instead of a fixture. This is the missing `applyTierComplete → setManifest` link
// the goal-backward verification flagged.
//
// # Module I/O
// - Input  the submitted `CalcJobSpec` (its `scene` + `receiverIds` + identity) and
//   the FINE `TierComplete` (its `spans` → chunk/rLocal map). No acoustic math here:
//   the manifest is pure bookkeeping; every dB is produced later by the WASM readout.
// - Output a `setManifest` into `useResultsStore` + the attached real clients, so the
//   spectrum panel and conditioning fast-recalc run against the real cached tensor.
//
// # Activation caveat (documented, honest)
// This fires only when a real threaded solve reaches the fine tier. In the current
// build the `wasm-bindgen-rayon` pool cannot start (the `build:wasm:compute` artifact
// ships a NON-shared `WebAssembly.Memory` — the Phase-10 `10-03` threaded-build gap;
// see `calc.spec.ts` Test 2, which skips honestly), so no tier completes and this
// seam stays dormant until that build gap is fixed. The assembly is unit-tested
// (`resultsFeed.test.ts`) so it is correct-by-construction the moment the pool runs.

import type { ConditioningDto, ReceiverPlacementDto, TierComplete } from "../generated/wire";
import type { CalcJobSpec } from "./worker";
import {
  useResultsStore,
  createWasmReadoutClient,
  type ChunkSpanRef,
  type ReceiverRef,
  type ResultsManifest,
} from "../store/results";
import {
  createWasmConditioningClient,
  useConditioningStore,
} from "../store/conditioning";
import { createWasmIdentityClient, useStaleStore } from "../store/stale";

// The default (un-conditioned) per-source drive: unity gain, no delay/filter, live.
function defaultConditioning(): ConditioningDto {
  return { gain_db: 0, delay_ms: 0, filter_band_db: null, muted: false };
}

/**
 * Assemble the results manifest from the submitted job spec and the FINE tier's
 * completion event — PURE (no store/wasm side effects), so it is unit-testable in
 * Node. Each fine span `[r_offset, r_offset+len)` maps global indices to their
 * chunk column (`rLocal = i`); the receiver id + SceneXY position come from the
 * spec (`receiverIds[g]`, `scene.receivers[g].position`).
 */
export function buildResultsManifest(spec: CalcJobSpec, fineEvent: TierComplete): ResultsManifest {
  const posByGlobal = new Map<number, readonly [number, number, number]>();
  for (const r of spec.scene.receivers as readonly ReceiverPlacementDto[]) {
    posByGlobal.set(r.global_index, r.position as readonly [number, number, number]);
  }

  const receivers: ReceiverRef[] = [];
  const spans: ChunkSpanRef[] = [];
  for (const span of fineEvent.spans) {
    const spanIds: string[] = [];
    for (let i = 0; i < span.len; i += 1) {
      const g = span.r_offset + i;
      const id = spec.receiverIds[g] ?? "";
      const p = posByGlobal.get(g);
      receivers.push({
        id,
        globalIndex: g,
        position: p ? [p[0], p[1]] : [0, 0],
        chunkIndex: span.chunk_index,
        rLocal: i,
      });
      spanIds.push(id);
    }
    spans.push({ chunkIndex: span.chunk_index, receiverIds: spanIds });
  }

  return {
    projectId: spec.projectId,
    tensorHash: fineEvent.tensor_hash,
    scene: spec.scene,
    perSourceConditioning: Array.from({ length: spec.nSub }, defaultConditioning),
    receivers,
    spans,
  };
}

/**
 * Push a finished solve's FINE tier into the results store and attach the real
 * readout/conditioning/stale clients, so the spectrum panel + conditioning
 * fast-recalc run against the real cached OPFS tensor. Idempotent with the panels'
 * guarded client attach. No-ops for a non-fine tier or an empty fine tier.
 */
export function applyResultsFeed(spec: CalcJobSpec, event: TierComplete): void {
  if (event.tier !== "fine" || event.spans.length === 0) {
    return;
  }
  const manifest = buildResultsManifest(spec, event);
  const results = useResultsStore.getState();
  results.setManifest(manifest);
  results.attachReadoutClient(createWasmReadoutClient());
  const conditioning = useConditioningStore.getState();
  conditioning.attachConditioningClient(createWasmConditioningClient());
  conditioning.seedFromManifest(manifest.perSourceConditioning);
  useStaleStore.getState().attachIdentityClient(createWasmIdentityClient());
}
