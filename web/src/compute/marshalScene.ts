// marshalScene.ts — the single source of truth that marshals the DRAWN scene into a
// `PrepareSolveReq` (the flat homogeneous down-range corridor of the 10-06 proven-valid
// shape) + the wasm-minted tensor identity. Extracted verbatim from `CalcPanel` so that
// BOTH the Calc submit path AND the 11-07 results-stale re-mint hash the scene the same
// way — a scene edit changes the identity here, and the stale badge compares that fresh
// identity against the cached tensor's manifest hash (D-12). One marshaller, never forked.
//
// # Module I/O
// - Input  the canonical scene store (drawn `calc_area` footprint + `source` features) and
//   the fine-grid spacing + coarse multiples (from the calc store). No acoustic math here —
//   `plan_tiers` (receiver lattice) and `tensor_hash` (blake3 identity) are BOTH wasm; this
//   module only assembles the request DTO from drawn inputs.
// - Output `buildPrepareScene(...)` → the marshalled `PrepareSolveReq` (with its true blake3
//   `tensor_hash` set), the TS-minted receiver UUIDs, `n_sub`, the `plan_tiers` request, and
//   the projected area — or null when the scene is incomplete (no project-independent gate:
//   needs a `calc_area` + ≥1 `source` + a positive area). The pure geometry/derivation
//   helpers (`deriveSceneInputs`, `plannedReceiverCount`) are re-exported for the cost panel.
// - Valid input range: a Polygon `calc_area` ring (WGS84) + ≥1 `source`.

import type {
  PlanTiersReq,
  PrepareSolveReq,
  ReceiverPlacementDto,
  SubSourcePlacementDto,
  TierPlanResult,
} from "../generated/wire";
import { useSceneStore } from "../store/sceneStore";

// Receiver height + the corridor's first-receiver down-range x (SceneXY meters).
export const RECEIVER_Z_M = 1.5;
export const CORRIDOR_X0_M = 100;

// Init the (threaded) compute wasm module once on the main thread — only pure fns
// (`plan_tiers`, `tensor_hash`) are called here; the rayon pool is the worker's job.
// The glue is a DYNAMIC import inside the factory so importing this module in Node (the
// vitest unit graph for store/stale.ts) never pulls the browser-only wasm/worker graph.
// Idempotent: wasm-bindgen caches the instance.
type ComputeGlue = typeof import("../generated/wasm-compute/envi_compute_wasm");
let gluePromise: Promise<ComputeGlue> | null = null;
export function ensureCompute(): Promise<ComputeGlue> {
  gluePromise ??= (async () => {
    const g = await import("../generated/wasm-compute/envi_compute_wasm");
    await g.default();
    return g;
  })();
  return gluePromise;
}

// Projected polygon area (m²) via a local equirectangular shoelace around the ring's mean
// latitude. The ring is WGS84 `[lng, lat]`; a valid enough metric for the receiver-count +
// cost estimate (Phase-11 replaces it with the CRS-exact footprint). Handles open/closed rings.
export function polygonAreaM2(ring: readonly (readonly number[])[]): number {
  if (ring.length < 3) {
    return 0;
  }
  const latMean = ring.reduce((s, p) => s + p[1], 0) / ring.length;
  const mPerDegLat = 110540;
  const mPerDegLon = 111320 * Math.cos((latMean * Math.PI) / 180);
  let acc = 0;
  for (let i = 0; i < ring.length; i += 1) {
    const a = ring[i];
    const b = ring[(i + 1) % ring.length];
    acc += a[0] * mPerDegLon * (b[1] * mPerDegLat) - b[0] * mPerDegLon * (a[1] * mPerDegLat);
  }
  return Math.abs(acc) / 2;
}

// The drawn-scene inputs the panel derives its estimate + spec from.
export interface SceneInputs {
  readonly areaM2: number;
  readonly sourceCount: number;
  readonly hasCalcArea: boolean;
}

export function deriveSceneInputs(
  features: Readonly<Record<string, unknown>>,
  kindOf: (id: string) => string | null,
): SceneInputs {
  let areaM2 = 0;
  let hasCalcArea = false;
  let sourceCount = 0;
  for (const id of Object.keys(features)) {
    const kind = kindOf(id);
    if (kind === "source") {
      sourceCount += 1;
    } else if (kind === "calc_area") {
      const geometry = (features[id] as { geometry?: { type?: string; coordinates?: unknown } }).geometry;
      if (geometry?.type === "Polygon" && Array.isArray(geometry.coordinates)) {
        const ring = geometry.coordinates[0];
        if (Array.isArray(ring)) {
          hasCalcArea = true;
          areaM2 = Math.max(areaM2, polygonAreaM2(ring as number[][]));
        }
      }
    }
  }
  return { areaM2, sourceCount, hasCalcArea };
}

// The square-lattice `plan_tiers` request BOTH the pre-run cost estimate and the submitted
// job derive from — a single source for the receiver set (WR-04). Side = sqrt(area).
export function buildPlanReq(
  spacingM: number,
  coarseMultiples: readonly number[],
  areaM2: number,
): PlanTiersReq {
  const side = Math.max(1, Math.sqrt(areaM2));
  return {
    fine_spacing_m: spacingM,
    lattice_origin: [0, 0],
    area_min: [0, 0],
    area_max: [side, side],
    discrete_points: [],
    coarse_multiples: [...coarseMultiples],
  };
}

// The receiver count the ACTUAL job will solve — the union of the tier plan's tiers
// (coarse ⊂ fine). Used by the cost estimate so its numbers match the solved grid (WR-04).
export async function plannedReceiverCount(
  spacingM: number,
  coarseMultiples: readonly number[],
  areaM2: number,
): Promise<number> {
  const g = await ensureCompute();
  const plan = g.plan_tiers(buildPlanReq(spacingM, coarseMultiples, areaM2)) as TierPlanResult;
  return plan.tiers.reduce((n, t) => n + t.receivers.length, 0);
}

// The marshalled scene + its identity + receiver ids.
export interface MarshalledScene {
  readonly scene: PrepareSolveReq;
  readonly receiverIds: string[];
  readonly nSub: number;
  readonly planReq: PlanTiersReq;
  readonly areaM2: number;
}

// Marshal a valid flat-ground scene from the drawn scene + the wasm tier plan (see the module
// header "Scene marshalling boundary"). Returns null when the scene is incomplete. The returned
// `scene.tensor_hash` is the TRUE blake3 identity (the wasm hasher excludes the field itself),
// so a caller can compare it against a cached tensor hash to detect divergence (D-12 stale).
export async function buildPrepareScene(
  spacingM: number,
  coarseMultiples: readonly number[],
): Promise<MarshalledScene | null> {
  const scene = useSceneStore.getState();
  const inputs = deriveSceneInputs(scene.features, (id) => scene.kindOf(id));
  if (!inputs.hasCalcArea || inputs.sourceCount < 1 || inputs.areaM2 <= 0) {
    return null;
  }

  const planReq = buildPlanReq(spacingM, coarseMultiples, inputs.areaM2);

  const g = await ensureCompute();
  const plan = g.plan_tiers(planReq) as TierPlanResult;

  // The union of the tiers' receivers, keyed by global index (contiguous 0..N-1 in emission
  // order). Each global index gets a TS-minted UUID (the wasm mints none) and a placement.
  const total = plan.tiers.reduce((n, t) => n + t.receivers.length, 0);
  const receivers: ReceiverPlacementDto[] = [];
  const receiverIds: string[] = new Array<string>(total);
  for (const tier of plan.tiers) {
    for (const r of tier.receivers) {
      receivers.push({
        global_index: r.global_index,
        position: [CORRIDOR_X0_M + r.global_index, 0, RECEIVER_Z_M],
      });
      receiverIds[r.global_index] = crypto.randomUUID();
    }
  }
  for (let i = 0; i < total; i += 1) {
    receiverIds[i] ??= crypto.randomUUID();
  }

  const nSub = inputs.sourceCount;
  const subSources: SubSourcePlacementDto[] = Array.from(
    { length: nSub },
    (_v, i): SubSourcePlacementDto => ({ position: [2.5, 0, 0.5 + 0.3 * i], directivity: null }),
  );

  const xMax = Math.max(400, CORRIDOR_X0_M + total + 10);
  const prepareScene: PrepareSolveReq = {
    // Placeholder — replaced below by the TRUE blake3 tensor identity. The Rust hasher
    // EXCLUDES this field, so the placeholder does not affect the digest.
    tensor_hash: "",
    n_sub: nSub,
    terrain: {
      points: [
        [2.5, 0],
        [xMax, 0],
      ],
      segments: [{ flow_resistivity: 200, roughness: 0 }],
    },
    atmosphere: { temperature_c: 15, humidity_pct: 70, pressure_kpa: 101.325 },
    coherence: { cv2: 0, ct2: 0, t_air_c: 15, c0: 340.348, roughness_r: 0, f_delta_nu: 1, d_m: 97.5 },
    weather: null,
    sub_sources: subSources,
    receivers,
    forest: null,
    forest_path_length_m: null,
    isolation: null,
  };

  // Derive the OPFS/manifest key from the REAL tensor identity (HI-01 / D-09).
  const tensorHash = g.tensor_hash(prepareScene);
  prepareScene.tensor_hash = tensorHash;

  return { scene: prepareScene, receiverIds, nSub, planReq, areaM2: inputs.areaM2 };
}
