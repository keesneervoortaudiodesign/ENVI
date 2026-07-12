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
// # Activation (live, proven end-to-end)
// This fires when a real threaded solve reaches the fine tier. The threaded
// `wasm-bindgen-rayon` pool DOES start in the built bundle (the `build:wasm:compute`
// artifact emits a SHARED `WebAssembly.Memory`), so a real Run completes and this seam
// populates the spectrum + conditioning surfaces with NO fixture seed — proven by
// `tests/e2e/results-real-solve.spec.ts` (a genuine solve → `done` → spectrum renders).
// The pure assembly is additionally unit-tested (`resultsFeed.test.ts`).

import type {
  ConditioningDto,
  ReceiverPlacementDto,
  ReconditionReq,
  TierComplete,
} from "../generated/wire";
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
import { readChunk } from "./opfs";
import { readoutReceivers, reconstructLevelGrid, projectToUtm } from "./wasm";
import { useColorScaleStore, type LevelGridInput } from "../store/colorScale";
import { useSceneStore } from "../store/sceneStore";

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
  // Feed the isophone map from the SAME solved tensor (SC3), fire-and-forget so a
  // slow reconstruction never blocks the spectrum/conditioning surfaces.
  void feedIsophoneFromSolve(spec, event).catch(() => {
    /* the isophone simply stays unfed on any reconstruction error (honest empty map) */
  });
}

// The WGS84 [lng, lat] centroid of the drawn `calc_area` polygon (the anchor the
// reconstructed grid is placed at), or null when no calc area is drawn.
function calcAreaCentroid(): [number, number] | null {
  const scene = useSceneStore.getState();
  for (const id of Object.keys(scene.features)) {
    if (scene.kindOf(id) !== "calc_area") {
      continue;
    }
    const geometry = (scene.features[id] as { geometry?: { type?: string; coordinates?: unknown } })
      .geometry;
    if (geometry?.type !== "Polygon" || !Array.isArray(geometry.coordinates)) {
      continue;
    }
    const ring = geometry.coordinates[0] as [number, number][];
    if (!Array.isArray(ring) || ring.length < 3) {
      continue;
    }
    // Average the ring vertices (drop the closing duplicate if present).
    const pts =
      ring.length > 1 && ring[0][0] === ring[ring.length - 1][0] && ring[0][1] === ring[ring.length - 1][1]
        ? ring.slice(0, -1)
        : ring;
    const n = pts.length;
    const lng = pts.reduce((s, p) => s + p[0], 0) / n;
    const lat = pts.reduce((s, p) => s + p[1], 0) / n;
    return [lng, lat];
  }
  return null;
}

// Uniform break edges spanning (min, max): 5 interior edges → 6 classes. Display-scale
// math only (no acoustic derivation — D-01). Returns null for a (near-)flat field.
function autoFitBreaks(values: readonly number[]): number[] | null {
  let min = Infinity;
  let max = -Infinity;
  for (const v of values) {
    if (Number.isFinite(v)) {
      if (v < min) min = v;
      if (v > max) max = v;
    }
  }
  if (!Number.isFinite(min) || !Number.isFinite(max) || max - min < 1e-6) {
    return null;
  }
  const step = (max - min) / 6;
  return [1, 2, 3, 4, 5].map((k) => min + k * step);
}

/**
 * Reconstruct the isophone level grid (SC3) from the finished FINE tier and feed it to
 * the colour-scale store. Assembles the receiver-major dB(A) vector from the REAL WASM
 * `readout_receivers` over each fine chunk (no acoustic math in TS), reconstructs the
 * dense 2-D grid in WASM, anchors it at the drawn `calc_area` (via `project_to_utm`), and
 * auto-fits the breaks to the field's actual range so contours appear. No-ops (leaving the
 * map empty) when the grid is degenerate (e.g. a 1-D/near-flat field) — never a false map.
 *
 * Note: a solve produces PROPAGATION-TRANSFER levels (a source emission/SWL model is
 * separately deferred), so the map shows the transfer field with auto-scaled breaks; the
 * EU-END absolute default applies once absolute Lden is available.
 */
async function feedIsophoneFromSolve(spec: CalcJobSpec, event: TierComplete): Promise<void> {
  // Assemble the receiver-major dB(A) vector from the fine chunks' readouts.
  const dba = new Float64Array(spec.receiverIds.length).fill(Number.NaN);
  const conditioning: ConditioningDto[] = Array.from({ length: spec.nSub }, defaultConditioning);
  for (const span of event.spans) {
    const chunkIds = spec.receiverIds.slice(span.r_offset, span.r_offset + span.len);
    const { tensor, pincoh } = await readChunk(spec.projectId, event.tensor_hash, span.chunk_index);
    const req: ReconditionReq = {
      tensor_hash: event.tensor_hash,
      per_source_conditioning: conditioning,
      receiver_ids: [...chunkIds],
    };
    const result = await readoutReceivers(spec.scene, req, tensor, pincoh);
    result.receivers.forEach((r, i) => {
      dba[span.r_offset + i] = r.total_dba;
    });
  }

  // Reconstruct the dense 2-D grid in WASM (the tested pure reconstruction).
  const grid = (await reconstructLevelGrid(spec.planReq, dba)) as LevelGridInput;
  if (grid.rows < 2 || grid.cols < 2) {
    return; // no 2-D field — leave the isophone map empty rather than render a false one
  }

  // Anchor the grid at the drawn site: place its centre at the calc_area centroid's UTM.
  const centroid = calcAreaCentroid();
  if (!centroid) {
    return;
  }
  const { easting, northing, utmZone, south } = await projectToUtm(centroid[0], centroid[1]);
  const anchored: LevelGridInput = {
    ...grid,
    origin: [
      easting - ((grid.cols - 1) * grid.spacing_m) / 2,
      northing - ((grid.rows - 1) * grid.spacing_m) / 2,
    ],
  };

  const scale = useColorScaleStore.getState();
  const breaks = autoFitBreaks(anchored.values);
  if (breaks) {
    scale.setBreaks(breaks);
  }
  scale.setIsophoneInput(anchored, { utm_zone: utmZone, south }, "dB(A)");
}
