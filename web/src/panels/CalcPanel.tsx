// CalcPanel.tsx — the Phase-10 Calculate / Run surface (WEB-07). A sibling of ImportPanel / WeatherPanel in
// the right rail: a pre-run cost estimate + a two-level guardrail, a Run gate (project + calc area + ≥1 source
// + cross-origin isolation + not blocked), progressive-tier live progress driven by the compute Web Worker,
// a cooperative single-click Abort, and an HONEST cross-origin-isolation capability-failure banner. Result
// rendering (the level-map + contour surfaces) is explicitly Phase 11 — this panel only submits + observes.
//
// # Module I/O
// - Input  the calc store (client-side job lifecycle + guardrail + capability, driven by the worker's
//   postMessage) and the scene store (open project + the drawn `calc_area` / `source` features). No props.
// - Output the panel JSX: every actionable control carries a `data-testid` (the offline Playwright UAT drives
//   these), and every dynamic string (reason, guardrail detail, counts) reaches the DOM as a React text child
//   (never a raw-HTML sink; threat T-10-05-01). Clicking Run marshals a valid flat-ground `CalcJobSpec` from the
//   drawn scene + the wasm `plan_tiers` and submits it to the worker; Abort requests a cooperative cancel.
// - Valid input range: derives entirely from store state; Run is disabled — with a visible reason — unless the
//   full gate holds. The cost estimate + tier plan come from the wasm (`estimate_cost` / `plan_tiers`, one
//   source of truth); this panel does NO acoustic or byte math itself.
//
// # Scene marshalling boundary (documented)
// The submitted `PrepareSolveReq` is a flat homogeneous down-range corridor (the 10-06 proven-valid scene
// shape) scaled to the tier plan's receiver count — a GENUINE client-side threaded solve of the unchanged
// Nord2000 engine. Deriving per-corridor terrain profiles / impedance from the drawn WGS84 polygon is Phase 11.

import { useEffect, useMemo, type ReactElement } from "react";

import init, { plan_tiers } from "../generated/wasm-compute/envi_compute_wasm";
import type {
  PlanTiersReq,
  PrepareSolveReq,
  ReceiverPlacementDto,
  SubSourcePlacementDto,
  TierKindDto,
  TierPlanResult,
} from "../generated/wire";
import type { CalcJobSpec } from "../compute/worker";
import { CalcClient } from "../compute/client";
import { estimateCost } from "../compute/cost";
import { useCalcStore, type CalcJobState } from "../store/calc";
import { useSceneStore } from "../store/sceneStore";

// The honest capability-failure copy (UI-SPEC S1) — a distinct, non-silent state, NOT a generic failure.
const CAPABILITY_MESSAGE =
  "This browser session is not cross-origin isolated, so the multi-threaded calculation cannot start. " +
  "Reload the app from its server (it sends the required COOP/COEP headers). If this keeps happening, " +
  "your browser may not support SharedArrayBuffer.";

// The hard byte budget the guardrail blocks above (the cost math + warn/block verdict live in the wasm
// `estimate_cost`; this is only the ceiling passed in). 2 GiB is a generous working ceiling.
const BUDGET_BYTES = 2 * 1024 * 1024 * 1024;

// Receiver height + the corridor's first-receiver down-range x (SceneXY meters) for the marshalled scene.
const RECEIVER_Z_M = 1.5;
const CORRIDOR_X0_M = 100;

// The three tier rows rendered in solve order (UI-SPEC §3). `points` may introduce zero receivers.
const TIER_ROWS: readonly { readonly kind: TierKindDto; readonly index: number; readonly label: string }[] = [
  { kind: "points", index: 0, label: "Receiver points" },
  { kind: "coarse", index: 1, label: "Coarse grid" },
  { kind: "fine", index: 2, label: "Fine grid" },
];

// Init the (threaded) compute wasm module once on the main thread — only `plan_tiers` (a pure fn) is called
// here; the rayon pool is the worker's job. Idempotent: wasm-bindgen caches the instance.
let computeReady: Promise<void> | null = null;
function ensureCompute(): Promise<void> {
  computeReady ??= init().then(() => undefined);
  return computeReady;
}

// A stable hex-only identity string (OPFS path segment; hex is required by the sink's path guard). Content-
// derived so an unchanged scene re-runs against the same key (D-09 idempotence, best-effort here).
function hexHash(input: string): string {
  let h = 2166136261 >>> 0;
  for (let i = 0; i < input.length; i += 1) {
    h ^= input.charCodeAt(i);
    h = Math.imul(h, 16777619) >>> 0;
  }
  return h.toString(16).padStart(8, "0");
}

// Projected polygon area (m²) via a local equirectangular shoelace around the ring's mean latitude. The ring
// is WGS84 `[lng, lat]`; a valid enough metric for the receiver-count + cost estimate (Phase-11 replaces it
// with the CRS-exact footprint). Handles an open or closed ring.
function polygonAreaM2(ring: readonly (readonly number[])[]): number {
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

// The drawn-scene inputs the panel derives its estimate + spec from: the calc-area footprint (m²), the
// source count, and the calc-area ring (null when absent).
interface SceneInputs {
  readonly areaM2: number;
  readonly sourceCount: number;
  readonly hasCalcArea: boolean;
}

function deriveSceneInputs(
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

// The chip severity word for the overall job state (mirrors ImportPanel/WeatherPanel `statusSeverity`).
function statusSeverity(state: CalcJobState): "" | "ok" | "warn" | "crit" | "off" {
  switch (state) {
    case "running":
      return "warn";
    case "done":
      return "ok";
    case "failed":
      return "crit";
    case "cancelled":
      return "off";
    default:
      return "";
  }
}

// The current 0-based tier index the worker is running, parsed from its `tier X/Y · …` progress message.
function runningTierIndex(message: string): number | null {
  const m = /tier (\d+)\/(\d+)/.exec(message);
  return m ? Number(m[1]) - 1 : null;
}

// A per-tier row's status, derived from store state so it NEVER regresses (a count, once set, stays; a done
// job is all-done; a running job shows the parsed current tier). `points` with zero receivers reads `done`
// only once the whole job finishes.
function tierStatus(
  row: { readonly kind: TierKindDto; readonly index: number },
  jobState: CalcJobState,
  message: string,
  tierCounts: Readonly<Record<TierKindDto, number>>,
): "queued" | "running" | "done" {
  if (tierCounts[row.kind] > 0 || jobState === "done") {
    return "done";
  }
  if (jobState === "running") {
    const cur = runningTierIndex(message);
    if (cur !== null) {
      if (row.index < cur) {
        return "done";
      }
      if (row.index === cur) {
        return "running";
      }
    }
  }
  return "queued";
}

function tierSeverity(status: "queued" | "running" | "done"): "" | "warn" | "ok" {
  if (status === "done") {
    return "ok";
  }
  if (status === "running") {
    return "warn";
  }
  return "";
}

// Format the wall-clock time estimate (ms) into a compact human string.
function formatTime(ms: number): string {
  if (!Number.isFinite(ms) || ms <= 0) {
    return "0 ms";
  }
  if (ms >= 1000) {
    return `${(ms / 1000).toFixed(1)} s`;
  }
  return `${Math.round(ms)} ms`;
}

// Marshal a valid flat-ground `CalcJobSpec` from the drawn scene + the wasm tier plan (see the module header
// "Scene marshalling boundary"). Returns null when the scene is incomplete.
async function buildJobSpec(spacingM: number, coarseMultiples: readonly number[]): Promise<CalcJobSpec | null> {
  const scene = useSceneStore.getState();
  const projectId = scene.projectId;
  if (!projectId) {
    return null;
  }
  const inputs = deriveSceneInputs(scene.features, (id) => scene.kindOf(id));
  if (!inputs.hasCalcArea || inputs.sourceCount < 1 || inputs.areaM2 <= 0) {
    return null;
  }

  const side = Math.max(1, Math.sqrt(inputs.areaM2));
  const planReq: PlanTiersReq = {
    fine_spacing_m: spacingM,
    lattice_origin: [0, 0],
    area_min: [0, 0],
    area_max: [side, side],
    discrete_points: [],
    coarse_multiples: [...coarseMultiples],
  };

  await ensureCompute();
  const plan = plan_tiers(planReq) as TierPlanResult;

  // The union of the tiers' receivers, keyed by global index (contiguous 0..N-1 in emission order). Each
  // global index gets a TS-minted UUID (the wasm mints none) and a valid down-range placement.
  const total = plan.tiers.reduce((n, t) => n + t.receivers.length, 0);
  const receivers: ReceiverPlacementDto[] = [];
  const receiverIds: string[] = new Array<string>(total);
  for (const tier of plan.tiers) {
    for (const r of tier.receivers) {
      receivers.push({ global_index: r.global_index, position: [CORRIDOR_X0_M + r.global_index, 0, RECEIVER_Z_M] });
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

  const tensorHash = hexHash(`${spacingM}|${Math.round(inputs.areaM2)}|${nSub}|${total}`);
  const xMax = Math.max(400, CORRIDOR_X0_M + total + 10);
  const prepareScene: PrepareSolveReq = {
    tensor_hash: tensorHash,
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

  const chunkReceivers = 32;
  return { projectId, tensorHash, planReq, scene: prepareScene, receiverIds, nSub, chunkReceivers };
}

// One per-tier row body: the label, a state-word chip (status conveyed by text, not colour alone), and the
// introduced-receiver count once the tier is done.
function TierRowBody({
  label,
  status,
  count,
}: {
  readonly label: string;
  readonly status: "queued" | "running" | "done";
  readonly count: number;
}): ReactElement {
  return (
    <>
      <span className="issue-text">{label}</span>
      <span className={`chip ${tierSeverity(status)}`}>{status}</span>
      {status === "done" ? <span className="mono">{count} receivers</span> : null}
    </>
  );
}

export function CalcPanel(): ReactElement {
  const projectId = useSceneStore((s) => s.projectId);
  const features = useSceneStore((s) => s.features);
  const kindOf = useSceneStore((s) => s.kindOf);

  const spacing = useCalcStore((s) => s.spacing_fine_m);
  const coarseMultiples = useCalcStore((s) => s.coarseMultiples);
  const setSpacing = useCalcStore((s) => s.setSpacing);
  const setCostEstimate = useCalcStore((s) => s.setCostEstimate);
  const costEstimate = useCalcStore((s) => s.costEstimate);
  const guardrail = useCalcStore((s) => s.guardrail);
  const jobState = useCalcStore((s) => s.jobState);
  const progress = useCalcStore((s) => s.progress);
  const message = useCalcStore((s) => s.message);
  const failureReason = useCalcStore((s) => s.failureReason);
  const tierCounts = useCalcStore((s) => s.tierCounts);
  const crossOriginIsolated = useCalcStore((s) => s.crossOriginIsolated);
  const abort = useCalcStore((s) => s.abort);

  const sceneInputs = useMemo(() => deriveSceneInputs(features, kindOf), [features, kindOf]);

  // On mount: attach a real compute client + forward the worker's capability / status / tier events into the
  // store, and record the main-thread cross-origin-isolation capability so Run is gated before any submit.
  useEffect(() => {
    const client = new CalcClient();
    const store = useCalcStore.getState();
    const unsubscribe = client.subscribe((msg) => {
      const s = useCalcStore.getState();
      if (msg.type === "capability") {
        s.setCapability(msg.crossOriginIsolated);
      } else if (msg.type === "status") {
        s.applyStatus(msg.status);
      } else if (msg.type === "tier") {
        s.applyTierComplete(msg.event);
      }
    });
    store.attachClient(client);
    const isolated = self.crossOriginIsolated === true && typeof SharedArrayBuffer !== "undefined";
    store.setCapability(isolated);
    return () => {
      unsubscribe();
    };
  }, []);

  // Recompute the pre-run cost estimate + guardrail from the wasm on every spacing / scene change. Only runs
  // cross-origin isolated (the threaded module needs SharedArrayBuffer to instantiate); errors are swallowed
  // so a transient estimate failure never crashes the panel.
  useEffect(() => {
    if (!crossOriginIsolated || !projectId || !sceneInputs.hasCalcArea || sceneInputs.sourceCount < 1) {
      return;
    }
    let cancelled = false;
    void estimateCost({
      area_m2: sceneInputs.areaM2,
      spacing_fine_m: spacing,
      discrete_points: 0,
      n_sub: sceneInputs.sourceCount,
      n_workers: Math.max(1, navigator.hardwareConcurrency || 4),
      budget_bytes: BUDGET_BYTES,
    })
      .then((estimate) => {
        if (!cancelled) {
          setCostEstimate(estimate);
        }
      })
      .catch(() => {
        /* transient estimate failure — the readout simply stays hidden */
      });
    return () => {
      cancelled = true;
    };
  }, [
    crossOriginIsolated,
    projectId,
    sceneInputs.areaM2,
    sceneInputs.hasCalcArea,
    sceneInputs.sourceCount,
    spacing,
    setCostEstimate,
  ]);

  const coarseSpacing = spacing * (coarseMultiples[0] ?? 10);
  const running = jobState === "running" || jobState === "queued";
  const canRun =
    !!projectId &&
    sceneInputs.hasCalcArea &&
    sceneInputs.sourceCount >= 1 &&
    !guardrail?.blocked &&
    crossOriginIsolated &&
    !running;

  // The single most relevant reason Run is disabled (never a silent block). Capability is surfaced by its own
  // banner, so it is not repeated here.
  let disabledReason: string | null = null;
  if (!canRun && projectId && crossOriginIsolated && !running) {
    if (!sceneInputs.hasCalcArea) {
      disabledReason = "Draw a calculation area to run.";
    } else if (sceneInputs.sourceCount < 1) {
      disabledReason = "Add at least one sound source to run.";
    } else if (guardrail?.blocked) {
      disabledReason = guardrail.detail;
    }
  }

  const onRun = (): void => {
    void buildJobSpec(spacing, coarseMultiples).then((spec) => {
      if (spec) {
        useCalcStore.getState().run(spec);
      }
    });
  };

  const pct = Math.round(progress * 100);

  return (
    <section className="panel" data-testid="calc-panel">
      <div className="panel-header">Calculate</div>

      {projectId ? null : (
        <div className="empty-state" data-testid="calc-no-project">
          Open a project to run a calculation.
        </div>
      )}

      {projectId && !crossOriginIsolated ? (
        <div className="form-error" role="alert" data-testid="calc-capability-error">
          {CAPABILITY_MESSAGE}
        </div>
      ) : null}

      {projectId ? (
        <>
          <label className="field-row">
            <span className="field-label">Fine grid spacing</span>
            <input
              type="number"
              className="field-input input dense"
              min={1}
              step={1}
              value={spacing}
              data-testid="calc-spacing"
              onChange={(e) => setSpacing(Number(e.target.value))}
            />
            <span className="field-unit">m</span>
          </label>

          <div className="mono" data-testid="calc-tiers">
            Tiers: receiver points → coarse {coarseSpacing} m → fine {spacing} m
          </div>

          {costEstimate ? (
            <div className="mono" data-testid="calc-estimate">
              {costEstimate.receiver_count} receivers · {(costEstimate.tensor_bytes / (1024 * 1024)).toFixed(1)} MiB
              tensor · ~{formatTime(costEstimate.time_estimate_ms)}
            </div>
          ) : null}

          {guardrail && guardrail.level !== "ok" ? (
            <div className={`chip ${guardrail.blocked ? "crit" : "warn"}`} data-testid="calc-guardrail">
              {guardrail.detail}
            </div>
          ) : null}

          <div className="btn-row">
            <button type="button" className="btn" data-testid="calc-run" disabled={!canRun} onClick={onRun}>
              Run calculation
            </button>
            {running ? (
              <button type="button" className="btn danger" data-testid="calc-abort" onClick={() => abort()}>
                Abort
              </button>
            ) : null}
            <span className={`chip ${statusSeverity(jobState)}`} data-testid="calc-status">
              {jobState === "failed" ? <span className="dot crit" aria-hidden="true" /> : null}
              {jobState}
            </span>
          </div>

          {disabledReason ? (
            <div className="issue-text" data-testid="calc-disabled-reason">
              {disabledReason}
            </div>
          ) : null}

          <div aria-live="polite">
            {jobState !== "idle" ? (
              <div className="mono" data-testid="calc-progress">
                {pct}% · {message}
              </div>
            ) : null}

            <ul className="issue-list" data-testid="calc-tiers-list">
              <li className="issue-row" data-testid="calc-tier-points">
                <TierRowBody
                  label="Receiver points"
                  status={tierStatus(TIER_ROWS[0], jobState, message, tierCounts)}
                  count={tierCounts.points}
                />
              </li>
              <li className="issue-row" data-testid="calc-tier-coarse">
                <TierRowBody
                  label={`Coarse grid (${coarseSpacing} m)`}
                  status={tierStatus(TIER_ROWS[1], jobState, message, tierCounts)}
                  count={tierCounts.coarse}
                />
              </li>
              <li className="issue-row" data-testid="calc-tier-fine">
                <TierRowBody
                  label={`Fine grid (${spacing} m)`}
                  status={tierStatus(TIER_ROWS[2], jobState, message, tierCounts)}
                  count={tierCounts.fine}
                />
              </li>
            </ul>
          </div>

          {jobState === "cancelled" ? (
            <div className="issue-text" data-testid="calc-cancelled">
              Cancelled — completed tiers kept.
            </div>
          ) : null}

          {jobState === "failed" ? (
            <div className="form-error" role="alert" data-testid="calc-failed">
              Calculation failed — {failureReason}
            </div>
          ) : null}
        </>
      ) : null}
    </section>
  );
}
