// testBridge.ts — a DEV-ONLY window bridge that lets the offline Playwright suite drive programmatic
// scene commits (the plan permits "programmatic store commits" for the draw-each-kind E2E).
//
// # Module I/O
// - Input  none at import; `installTestBridge()` attaches `window.__enviTest` with commit/save/read
//   helpers backed by the canonical store (D-03). Terra Draw drawing is unreliable in headless WebGL, so
//   the E2E commits geometry through the same store path a finished draw uses (`commitFeature`).
// - Output `window.__enviTest.{commitActive, commit, state, save}` — placing a feature of the active (or
//   an explicit) kind, a store snapshot (feature-id → kind, selection, inherited-field lists), and the
//   whole-scene PUT. Installed ONLY under `import.meta.env.DEV`; the production `vite build` bundle (what
//   ships in web/dist and the contract test serves) never contains it.
// - Valid input range: WGS84 `lng`/`lat`; `kind` ∈ the 9 frozen KINDS.

import type { GeoJSONStoreFeatures } from "terra-draw";

import { KIND_META, isKind, type Kind } from "./draw/kinds";
import { useSceneStore } from "./store/sceneStore";
import { reopenLast } from "./store/projectActions";
import { useImportStore, type LayerKey, type LayerStatus } from "./store/import";
import { runImport, retryLayer } from "./import/importJob";
import { getTile, removeTile } from "./import/opfs";
import { writeChunkFile } from "./compute/opfs";
import { useResultsStore, createWasmReadoutClient } from "./store/results";
import { applyResultsFeed } from "./compute/resultsFeed";
import type { CalcJobSpec } from "./compute/worker";
import type { TierComplete } from "./generated/wire";
import {
  createWasmConditioningClient,
  useConditioningStore,
  type ConditioningState,
} from "./store/conditioning";
import { createWasmIdentityClient, useStaleStore } from "./store/stale";
import { useCalcStore, type CalcJobState } from "./store/calc";
import { useColorScaleStore, type LevelGridInput } from "./store/colorScale";
import { isophoneTelemetry } from "./map/isophoneLayer";
import { differenceTelemetry } from "./map/differenceLayer";
import { objectLayerTelemetry } from "./map/objectStyles";
import {
  createWasmDifferenceClient,
  useDifferenceStore,
} from "./store/difference";
import {
  beaufortToMs,
  useScenarioStore,
  type ScenarioComputeClient,
  type ScenarioSolveResult,
} from "./store/scenarios";
import type { ConditioningDto, ExportCrsDto } from "./generated/wire";
import type { GroundZoneOutcome } from "./validate/groundZone";
import type { AuthoredSpectrumDto, BboxDto, PrepareSolveReq } from "./generated/wire";

const RESULTS_N_BANDS = 105;

// A minimal valid PrepareSolveReq (deny_unknown_fields) whose ONLY use here is the
// tensor-identity hash + the readout hash gate — the readout does NOT re-solve it.
function buildResultsScene(receiverCount: number): PrepareSolveReq {
  return {
    tensor_hash: "",
    n_sub: 1,
    terrain: { points: [[2.5, 0], [400, 0]], segments: [{ flow_resistivity: 200, roughness: 0 }] },
    atmosphere: { temperature_c: 15, humidity_pct: 70, pressure_kpa: 101.325 },
    coherence: { cv2: 0, ct2: 0, t_air_c: 15, c0: 340.348, roughness_r: 0, f_delta_nu: 1, d_m: 97.5 },
    sub_sources: [{ position: [2.5, 0, 0.5] }],
    receivers: Array.from({ length: receiverCount }, (_, i) => ({
      global_index: i,
      position: [100 + 10 * i, 0, 1.5],
    })),
  } as unknown as PrepareSolveReq;
}

// Deterministic fixture tensor bytes in the frozen `[s][r][f]` freq-fastest layout
// the OPFS sink writes (16 B/cell interleaved re,im f64-LE H_coh; 8 B/cell f64-LE
// P_incoh). All values finite + P > 0 so the readout's coherent/incoherent split
// is live. This is the SAME byte contract `read_chunk` (opfs_reader.rs) decodes.
function buildResultsFixtureBytes(receiverCount: number): {
  tensor: Uint8Array;
  pincoh: Uint8Array;
} {
  const cells = receiverCount * RESULTS_N_BANDS;
  const tensor = new Uint8Array(cells * 16);
  const pincoh = new Uint8Array(cells * 8);
  const tv = new DataView(tensor.buffer);
  const pv = new DataView(pincoh.buffer);
  let hi = 0;
  let pi = 0;
  for (let r = 0; r < receiverCount; r += 1) {
    for (let f = 0; f < RESULTS_N_BANDS; f += 1) {
      tv.setFloat64(hi, 0.02 * (r + 1) * (1 + 0.001 * f), true);
      tv.setFloat64(hi + 8, (0.01 * (f + 1)) / RESULTS_N_BANDS, true);
      hi += 16;
      pv.setFloat64(pi, 1e-6 * (r + 1) * (f + 1), true);
      pi += 8;
    }
  }
  return { tensor, pincoh };
}

// A multi-source conditioning scene (11-07 UAT): `nSub` sub-sources + `receiverCount`
// receivers over the flat corridor. The ONLY use here is the tensor identity + the MAC
// hash gate — the recondition does NOT re-solve it.
function buildConditioningScene(nSub: number, receiverCount: number): PrepareSolveReq {
  return {
    tensor_hash: "",
    n_sub: nSub,
    terrain: { points: [[2.5, 0], [400, 0]], segments: [{ flow_resistivity: 200, roughness: 0 }] },
    atmosphere: { temperature_c: 15, humidity_pct: 70, pressure_kpa: 101.325 },
    coherence: { cv2: 0, ct2: 0, t_air_c: 15, c0: 340.348, roughness_r: 0, f_delta_nu: 1, d_m: 97.5 },
    sub_sources: Array.from({ length: nSub }, (_, i) => ({ position: [2.5, 0, 0.5 + 0.3 * i] })),
    receivers: Array.from({ length: receiverCount }, (_, i) => ({
      global_index: i,
      position: [100 + 10 * i, 0, 1.5],
    })),
  } as unknown as PrepareSolveReq;
}

// Deterministic fixture tensor bytes for `nSub` sub-sources × `receiverCount` receivers in
// the frozen `[s][r][f]` freq-fastest layout `read_chunk` decodes (16 B/cell interleaved
// re,im f64-LE H_coh; 8 B/cell f64-LE P_incoh). All finite + P > 0 so the two-channel
// readout is live and a gain change genuinely moves the reconditioned totals.
function buildConditioningFixtureBytes(
  nSub: number,
  receiverCount: number,
): { tensor: Uint8Array; pincoh: Uint8Array } {
  const cells = nSub * receiverCount * RESULTS_N_BANDS;
  const tensor = new Uint8Array(cells * 16);
  const pincoh = new Uint8Array(cells * 8);
  const tv = new DataView(tensor.buffer);
  const pv = new DataView(pincoh.buffer);
  let hi = 0;
  let pi = 0;
  for (let s = 0; s < nSub; s += 1) {
    for (let r = 0; r < receiverCount; r += 1) {
      for (let f = 0; f < RESULTS_N_BANDS; f += 1) {
        tv.setFloat64(hi, 0.02 * (s + 1) * (r + 1) * (1 + 0.001 * f), true);
        tv.setFloat64(hi + 8, (0.01 * (f + 1)) / RESULTS_N_BANDS, true);
        hi += 16;
        pv.setFloat64(pi, 1e-6 * (s + 1) * (r + 1) * (f + 1), true);
        pi += 8;
      }
    }
  }
  return { tensor, pincoh };
}

// The square lattice side the seeded scenarios' fixture readouts map onto (a 24×24
// grid near the UTM 31N central meridian, so the SceneXY→LonLat reprojection lands in
// the Netherlands — the SAME lattice the isophone UAT seeds).
const SCENARIO_SIDE = 24;

// A seeded scenario compute client for the 11-08 offline UAT: the friendly A/B/C
// DERIVATION runs the REAL WASM (`derive_weather_friendly`, so the friendly routing is
// genuinely exercised offline), and the full-solve is a deterministic FIXTURE readout
// (a per-scenario tensor hash + a spatial dB(A) ramp whose amplitude scales with the
// scenario's warmth) — the SAME fixture-seeding the results/conditioning UATs use for
// the OPFS tensor, so a Compare produces a genuinely diverging A − B field.
async function seededScenarioClient(): Promise<ScenarioComputeClient> {
  const { deriveWeatherFriendly } = await import("./import/wasm");
  return {
    async derive(met, azimuths) {
      return deriveWeatherFriendly({
        temperature_c: met.temperature_c,
        temp_gradient_c_per_m: met.tempGradientCPerM,
        wind_speed_ms: beaufortToMs(met.beaufort),
        wind_from_deg: met.windFromDeg,
        z0: met.z0,
        downwind_worst_case: met.downwindWorstCase,
        path_azimuths_deg: [...azimuths],
        raw_override:
          met.mode === "advanced" && met.raw
            ? { a: met.raw.a, b: met.raw.b, c: met.raw.c, z0: met.raw.z0 }
            : null,
      });
    },
    async solve(scenario): Promise<ScenarioSolveResult> {
      const side = SCENARIO_SIDE;
      const n = side * side;
      // The fixture readout: a spatial dB(A) ramp whose amplitude scales with the
      // scenario's warmth (the base scenario, 15 °C, is flat 65 dB(A); a warmer
      // scenario ramps ±amp about 65). Two different scenarios thus differ spatially,
      // so their A − B delta spans negative→positive (a genuinely diverging map).
      const amp = scenario.met.temperature_c - 15;
      const totalsDba = Array.from(
        { length: n },
        (_, i) => 65 + amp * (i / (n - 1) - 0.5) * 2,
      );
      const grid = {
        rows: side,
        cols: side,
        origin: [500_000, 5_800_000] as [number, number],
        spacing_m: 10,
        values: totalsDba,
      };
      const crs = { utm_zone: 31, south: false };
      // The per-scenario tensor identity (the OPFS calc/<hash>/ key): distinct met ⇒
      // distinct hash, so clone-then-edit yields its own cached tensor.
      const hash =
        `scenario-${scenario.met.temperature_c}-${scenario.met.beaufort}-` +
        `${scenario.met.windFromDeg}-${scenario.met.downwindWorstCase}`;
      return { tensorHash: hash, totalsDba, grid, crs };
    },
  };
}

// A JSON-safe per-layer import status snapshot (the 08-08 offline E2E asserts on these).
export interface ImportLayerSnapshot {
  readonly status: LayerStatus;
  readonly featureCount: number;
  readonly surfaceModel: boolean;
  readonly error: { readonly status: number; readonly detail: string } | null;
}

// Build a minimal valid geometry for a kind's Terra Draw geometry mode, offset around [lng, lat].
function geometryFor(kind: Kind, lng: number, lat: number): GeoJSONStoreFeatures["geometry"] {
  const d = 0.0005;
  switch (KIND_META[kind].mode) {
    case "point":
      return { type: "Point", coordinates: [lng, lat] };
    case "linestring":
      return { type: "LineString", coordinates: [[lng, lat], [lng + d, lat + d]] };
    case "polygon":
      return {
        type: "Polygon",
        coordinates: [[[lng, lat], [lng + d, lat], [lng + d, lat + d], [lng, lat]]],
      };
  }
}

function commit(kind: Kind, lng: number, lat: number): string {
  const id = crypto.randomUUID();
  const feature = {
    id,
    type: "Feature",
    geometry: geometryFor(kind, lng, lat),
    properties: {},
  } as unknown as GeoJSONStoreFeatures;
  useSceneStore.getState().commitFeature(kind, feature);
  return id;
}

export interface EnviTestBridge {
  // Commit a feature of the CURRENTLY active palette kind (proves palette selection → kind tag).
  commitActive(lng: number, lat: number): string;
  // Commit a feature of an explicit kind.
  commit(kind: Kind, lng: number, lat: number): string;
  // A JSON-safe snapshot of the store for assertions.
  state(): {
    kinds: Record<string, string | null>;
    selection: string | null;
    inherited: Record<string, readonly string[]>;
  };
  // The per-edge UUIDs (D-02) of a building feature, in ring order.
  buildingEdges(id: string): string[];
  // Set an authored isolation spectrum for a feature/edge key (a façade override or a wall/screen spectrum).
  setSpectrum(key: string, authored: AuthoredSpectrumDto): void;
  // The authored spectrum stored under a key, or null.
  spectrum(key: string): AuthoredSpectrumDto | null;
  // Apply a building geometry update (a new footprint ring) through the same store path a Terra Draw edit
  // uses, so the D-02 ring-diff reconciles edge_ids + façade spectra. `ring` is a CLOSED ring `[x, y][]`.
  applyBuildingRing(id: string, ring: [number, number][]): void;
  // The current [x, y] endpoints of the edge whose UUID is `edgeId` on building `id` (for re-point checks).
  edgeSegment(id: string, edgeId: string): { from: [number, number]; to: [number, number] } | null;
  // Draw a ground_zone from a CLOSED ring `[x, y][]` through the SAME draw-time classification path a
  // finished Terra Draw polygon takes (D-07): the geometry is upserted then classified. Returns the
  // outcome, the id (present only when committed), and the crossed zone's id on a partial-cross reject.
  commitGroundZone(ring: [number, number][]): {
    outcome: GroundZoneOutcome;
    id: string | null;
    conflictId: string | null;
  };
  // Merge a non-geometric property patch into a feature (a committed inspector edit path).
  update(id: string, patch: Record<string, unknown>): void;
  // Open a project (id + display name) — the Delete-project dialog compares the typed name to this.
  openProject(id: string, name: string): void;
  // Close the current project (route to the empty/no-project state) — the SC4 close-before-reopen step.
  closeProject(): void;
  // Reopen the last-opened project (GET /projects/last → scene) — the SC4 reopen-last step. Resolves to
  // whether a project was restored.
  reopenLast(): Promise<boolean>;
  // The canonical feature ids currently in the store (SC4 round-trip fidelity checks).
  featureIds(): string[];
  // Trigger the whole-scene PUT.
  save(): Promise<void>;

  // --- GIS import (08-08 offline E2E). Drives the REAL import orchestrator (D-06/D-07), not a stub. ---
  // Run a viewport import for the currently-open project over the given WGS84 bbox (every enabled layer).
  runImport(bbox: BboxDto): void;
  // Enable/disable a layer for the next import/retry (D-06 per-layer toggles).
  setImportLayerEnabled(layer: LayerKey, enabled: boolean): void;
  // Retry a single failed layer without touching its siblings (D-07).
  retryImportLayer(layer: LayerKey): void;
  // A JSON-safe snapshot of every layer's import status (assertions).
  importState(): {
    layers: Record<LayerKey, ImportLayerSnapshot>;
    attributedSources: string[];
    debugOverlay: boolean;
  };
  // Toggle the SC3 impedance debug overlay.
  toggleImpedanceOverlay(): void;
  // The scene load epoch — bumped on every import commit (loadImportedScene). A re-run signal for the
  // DATA-04 replay that is independent of the D-09 idempotent-merge feature count.
  sceneEpoch(): number;
  // Whether a source tile is cached in this project's OPFS (DATA-04 replay asserts an OPFS hit).
  cachedTile(source: string, tile: string): Promise<boolean>;
  // Evict a cached source tile — the DATA-04 negative guard (removed entry ⇒ the compute read fails).
  evictTile(source: string, tile: string): Promise<void>;

  // --- Results (WEB-11 spectrum-panel offline UAT, 11-05) ---
  // Seed a deterministic fixture tensor into OPFS (one chunk) + set the results
  // manifest keyed by the REAL wasm-minted tensor identity, so the panel's readout
  // hash gate matches. Returns the receiver UUIDs in order. Opens a test project if
  // none is open.
  seedResults(receiverCount: number): Promise<string[]>;

  // Drive the PRODUCTION calc→results feed (11-VERIFICATION follow-up): seed the OPFS
  // tensor, then push a solve-shaped FINE `TierComplete` through the REAL
  // `applyResultsFeed(spec, event)` — the same `applyTierComplete → setManifest` link
  // the CalcPanel runs when a real solve completes — rather than calling `setManifest`
  // directly. Proves the production feed path lights up the spectrum panel. Returns ids.
  feedFromSolve(receiverCount: number): Promise<string[]>;

  // --- Isophone map (WEB-06/GRID-04 offline UAT, 11-06) ---
  // Seed a deterministic level grid + CRS + weighting into the colour-scale store —
  // the SAME `setIsophoneInput` a finished readout feeds. The IsophoneLayer then
  // re-contours it via the REAL WASM tracer (no re-solve). A ramp spanning ~40–90 dB
  // so all six EU-END classes appear.
  seedIsophone(): void;
  // The isophone telemetry (trace count / rendered feature count / layer paint type)
  // for the SC3 re-contour + fill-not-raster assertions.
  isophoneTelemetry(): ReturnType<typeof isophoneTelemetry>;
  // The current colour-scale source of truth (preset + breaks + colors) so the UAT
  // can assert legend ≡ contour ≡ class colours.
  colorScaleState(): { preset: string; breaks: number[]; colors: string[] };

  // --- Scene-object display styling (D-17/D-18/D-19 offline UAT, 11-10) ---
  // The object-layer telemetry (registered fill/line/symbol layer ids + hatch/marker
  // image ids + full style layer order + whether every object layer sits ABOVE the
  // isophone fill). Proves the D-18 draw order + hatch registration without
  // reimplementing the app.
  objectLayerTelemetry(): ReturnType<typeof objectLayerTelemetry>;

  // --- Conditioning fast-recalc (WEB-05/SVC-06 offline UAT, 11-07) ---
  // Seed a `nSub`-source × `receiverCount`-receiver results manifest keyed by the REAL
  // wasm-minted tensor identity (OPFS chunk + manifest) AND a square isophone grid whose
  // cell count == `receiverCount` (so a conditioning recalc re-feeds the map 1:1). Attaches
  // the real recondition / readout / identity clients. `receiverCount` must be a perfect
  // square. Returns the source ids (sub-source index strings) + the receiver UUIDs.
  seedConditioning(nSub: number, receiverCount: number): Promise<{ sourceIds: string[]; receiverIds: string[] }>;
  // Simulate a SCENE edit since the solve: replace the manifest's scene with a mutated one
  // (a moved receiver) — its re-minted identity now diverges from the cached tensor hash, so
  // the stale badge flips (WASM re-mint) AND a subsequent MAC is refused (SVC-06 409). The
  // cached tensor key is left intact (the honest "result no longer matches the scene" state).
  divergeScene(): Promise<void>;
  // A JSON-safe snapshot of the conditioning drive + honest-state flags (assertions).
  conditioningState(): {
    perSource: Record<string, ConditioningDto>;
    order: string[];
    refuse: boolean;
    pending: boolean;
    recalcEpoch: number;
  };
  // The results-stale badge state (D-12).
  staleState(): { isStale: boolean };
  // The calc job lifecycle state — asserted `idle` to prove a conditioning recalc runs the
  // tensor MAC with NO propagation re-run (no solve-worker submit).
  calcJobState(): CalcJobState;

  // --- Weather scenarios + difference map (WEB-12/METX-03/04 offline UAT, 11-08) ---
  // Reset the scenario registry and attach the seeded compute client (REAL WASM friendly
  // derivation + a fixture full-solve readout) and the REAL WASM difference client, so the
  // panel's New/Compute/Switch/Compare flow runs offline against real WASM. Opens a test
  // project if none is open.
  seedScenarios(): Promise<void>;
  // A JSON-safe snapshot of the scenario registry (the panel drives clone/compute/switch).
  scenarioState(): {
    scenarios: { id: string; name: string; computed: boolean }[];
    activeId: string | null;
    compareA: string | null;
    compareB: string | null;
    computeEpoch: number;
    switchEpoch: number;
  };
  // A JSON-safe snapshot of the difference-map state (asserted on Compare).
  differenceState(): { hasDelta: boolean; epoch: number; labelA: string; labelB: string };
  // The difference-layer telemetry (trace/feature counts, paint type, neutral-midpoint
  // colour) for the diverging-map assertions (gray-at-0, fill-not-raster).
  differenceTelemetry(): ReturnType<typeof differenceTelemetry>;
}

export function installTestBridge(): void {
  const bridge: EnviTestBridge = {
    commitActive(lng, lat) {
      const tool = useSceneStore.getState().activeTool;
      if (!isKind(tool)) {
        throw new Error("commitActive: the active tool is the pointer, not a drawing kind");
      }
      return commit(tool, lng, lat);
    },
    commit,
    state() {
      const s = useSceneStore.getState();
      const kinds: Record<string, string | null> = {};
      for (const id of Object.keys(s.features)) {
        kinds[id] = s.kindOf(id);
      }
      return { kinds, selection: s.selection, inherited: { ...s.inheritedFields } };
    },
    buildingEdges(id) {
      const raw = useSceneStore.getState().features[id]?.properties?.["edge_ids"];
      return Array.isArray(raw) ? (raw.filter((x) => typeof x === "string") as string[]) : [];
    },
    setSpectrum(key, authored) {
      useSceneStore.getState().setSpectrum(key, authored);
    },
    spectrum(key) {
      return (useSceneStore.getState().spectra[key] ?? null) as AuthoredSpectrumDto | null;
    },
    applyBuildingRing(id, ring) {
      const prev = useSceneStore.getState().features[id];
      const feature = {
        id,
        type: "Feature",
        geometry: { type: "Polygon", coordinates: [ring] },
        properties: { ...(prev?.properties ?? {}) },
      } as unknown as GeoJSONStoreFeatures;
      useSceneStore.getState().applyTerraDrawChange([id], "update", [feature]);
    },
    edgeSegment(id, edgeId) {
      const s = useSceneStore.getState();
      const edges = this.buildingEdges(id);
      const pos = edges.indexOf(edgeId);
      if (pos < 0) {
        return null;
      }
      const geometry = s.features[id]?.geometry;
      if (!geometry || geometry.type !== "Polygon") {
        return null;
      }
      const outer = geometry.coordinates[0];
      const closed =
        outer.length > 1 &&
        outer[0][0] === outer[outer.length - 1][0] &&
        outer[0][1] === outer[outer.length - 1][1];
      const verts = closed ? outer.slice(0, outer.length - 1) : outer.slice();
      const from = verts[pos];
      const to = verts[(pos + 1) % verts.length];
      return { from: [from[0], from[1]], to: [to[0], to[1]] };
    },
    commitGroundZone(ring) {
      const id = crypto.randomUUID();
      const feature = {
        id,
        type: "Feature",
        geometry: { type: "Polygon", coordinates: [ring] },
        properties: {},
      } as unknown as GeoJSONStoreFeatures;
      // Upsert the raw geometry (the draw path), then run the D-07 draw-time classification.
      useSceneStore.getState().applyTerraDrawChange([id], "create", [feature]);
      const outcome = useSceneStore.getState().commitGroundZoneCandidate(id);
      const conflictId = useSceneStore.getState().groundReject?.conflictId ?? null;
      return { outcome, id: outcome === "partial-cross" ? null : id, conflictId };
    },
    update(id, patch) {
      useSceneStore.getState().updateProperties(id, patch);
    },
    openProject(id, name) {
      useSceneStore.getState().setProject(id, name);
    },
    closeProject() {
      useSceneStore.getState().resetProject();
    },
    reopenLast() {
      return reopenLast();
    },
    featureIds() {
      return Object.keys(useSceneStore.getState().features);
    },
    save() {
      return useSceneStore.getState().saveScene();
    },
    runImport(bbox) {
      const projectId = useSceneStore.getState().projectId;
      if (!projectId) {
        throw new Error("runImport: no project is open");
      }
      runImport(projectId, bbox);
    },
    setImportLayerEnabled(layer, enabled) {
      useImportStore.getState().setLayerEnabled(layer, enabled);
    },
    retryImportLayer(layer) {
      retryLayer(layer);
    },
    importState() {
      const s = useImportStore.getState();
      const layers = {} as Record<LayerKey, ImportLayerSnapshot>;
      for (const layer of Object.keys(s.layers) as LayerKey[]) {
        const l = s.layers[layer];
        layers[layer] = {
          status: l.status,
          featureCount: l.featureCount,
          surfaceModel: l.surfaceModel,
          error: l.error ? { status: l.error.status, detail: l.error.detail } : null,
        };
      }
      return { layers, attributedSources: [...s.attributedSources], debugOverlay: s.debugOverlay };
    },
    toggleImpedanceOverlay() {
      useImportStore.getState().toggleDebugOverlay();
    },
    sceneEpoch() {
      return useSceneStore.getState().loadEpoch;
    },
    async cachedTile(source, tile) {
      const projectId = useSceneStore.getState().projectId;
      if (!projectId) {
        return false;
      }
      return (await getTile(projectId, source, tile)) !== null;
    },
    async evictTile(source, tile) {
      const projectId = useSceneStore.getState().projectId;
      if (projectId) {
        await removeTile(projectId, source, tile);
      }
    },
    async seedResults(receiverCount) {
      let projectId = useSceneStore.getState().projectId;
      if (!projectId) {
        projectId = "results-uat-project";
        useSceneStore.getState().setProject(projectId, "Results UAT");
      }
      const scene = buildResultsScene(receiverCount);
      // Mint the tensor identity with the REAL wasm (crossOriginIsolated holds — the
      // dev server sends COOP/COEP), so the readout's re-mint gate matches exactly.
      const glue = await import("./generated/wasm-compute/envi_compute_wasm");
      await glue.default();
      const tensorHash = glue.tensor_hash(scene);
      const { tensor, pincoh } = buildResultsFixtureBytes(receiverCount);
      await writeChunkFile(projectId, tensorHash, "tensor", 0, tensor);
      await writeChunkFile(projectId, tensorHash, "pincoh", 0, pincoh);
      const ids = Array.from({ length: receiverCount }, () => crypto.randomUUID());
      useResultsStore.getState().setManifest({
        projectId,
        tensorHash,
        scene,
        perSourceConditioning: [{ gain_db: 80, delay_ms: 0, filter_band_db: null, muted: false }],
        receivers: ids.map((id, i) => ({
          id,
          globalIndex: i,
          position: [100 + 10 * i, 0],
          chunkIndex: 0,
          rLocal: i,
        })),
        spans: [{ chunkIndex: 0, receiverIds: ids }],
      });
      return ids;
    },
    async feedFromSolve(receiverCount) {
      let projectId = useSceneStore.getState().projectId;
      if (!projectId) {
        projectId = "feed-uat-project";
        useSceneStore.getState().setProject(projectId, "Feed UAT");
      }
      const scene = buildResultsScene(receiverCount);
      const glue = await import("./generated/wasm-compute/envi_compute_wasm");
      await glue.default();
      const tensorHash = glue.tensor_hash(scene);
      const { tensor, pincoh } = buildResultsFixtureBytes(receiverCount);
      await writeChunkFile(projectId, tensorHash, "tensor", 0, tensor);
      await writeChunkFile(projectId, tensorHash, "pincoh", 0, pincoh);
      const ids = Array.from({ length: receiverCount }, () => crypto.randomUUID());
      // A solve-shaped job spec + a single-chunk FINE tier over all receivers.
      const spec: CalcJobSpec = {
        projectId,
        tensorHash,
        planReq: {
          fine_spacing_m: 10,
          lattice_origin: [0, 0],
          area_min: [0, 0],
          area_max: [0, 0],
          discrete_points: [],
          coarse_multiples: [],
        },
        scene,
        receiverIds: ids,
        nSub: 1,
        chunkReceivers: receiverCount,
      };
      const event: TierComplete = {
        kind: "tier_complete",
        tier: "fine",
        tier_index: 0,
        spacing_m: 10,
        tensor_hash: tensorHash,
        receiver_ids: ids,
        spans: [
          {
            chunk_index: 0,
            r_offset: 0,
            len: receiverCount,
            tensor_file: `calc/${tensorHash}/tensor/0`,
            pincoh_file: `calc/${tensorHash}/pincoh/0`,
          },
        ],
      };
      // The REAL production feed (not a direct setManifest).
      applyResultsFeed(spec, event);
      return ids;
    },
    seedIsophone() {
      // A 24×24 lattice near the UTM 31N central meridian (so the SceneXY→LonLat
      // reprojection lands in the Netherlands), values ramping ~40–90 dB so the
      // full EU-END class scheme (< 55 … ≥ 75) traces.
      const rows = 24;
      const cols = 24;
      const values: number[] = [];
      for (let r = 0; r < rows; r += 1) {
        for (let c = 0; c < cols; c += 1) {
          values.push(40 + (c / (cols - 1)) * 50);
        }
      }
      const grid: LevelGridInput = {
        rows,
        cols,
        origin: [500_000, 5_800_000],
        spacing_m: 10,
        values,
      };
      const crs: ExportCrsDto = { utm_zone: 31, south: false };
      useColorScaleStore.getState().setIsophoneInput(grid, crs, "dB(A)");
    },
    isophoneTelemetry() {
      return isophoneTelemetry();
    },
    colorScaleState() {
      const s = useColorScaleStore.getState();
      return { preset: s.preset, breaks: [...s.breaks], colors: [...s.colors] };
    },
    objectLayerTelemetry() {
      return objectLayerTelemetry();
    },
    async seedConditioning(nSub, receiverCount) {
      let projectId = useSceneStore.getState().projectId;
      if (!projectId) {
        projectId = "conditioning-uat-project";
        useSceneStore.getState().setProject(projectId, "Conditioning UAT");
      }
      const scene = buildConditioningScene(nSub, receiverCount);
      // Mint the identity with the REAL wasm (crossOriginIsolated holds — dev COOP/COEP),
      // so both the readout hash gate AND the stale re-mint compare exactly.
      const glue = await import("./generated/wasm-compute/envi_compute_wasm");
      await glue.default();
      const tensorHash = glue.tensor_hash(scene);
      const { tensor, pincoh } = buildConditioningFixtureBytes(nSub, receiverCount);
      await writeChunkFile(projectId, tensorHash, "tensor", 0, tensor);
      await writeChunkFile(projectId, tensorHash, "pincoh", 0, pincoh);
      const ids = Array.from({ length: receiverCount }, () => crypto.randomUUID());
      const perSourceConditioning: ConditioningDto[] = Array.from({ length: nSub }, () => ({
        gain_db: 0,
        delay_ms: 0,
        filter_band_db: null,
        muted: false,
      }));
      useResultsStore.getState().setManifest({
        projectId,
        tensorHash,
        scene,
        perSourceConditioning,
        receivers: ids.map((id, i) => ({
          id,
          globalIndex: i,
          position: [100 + 10 * i, 0],
          chunkIndex: 0,
          rLocal: i,
        })),
        spans: [{ chunkIndex: 0, receiverIds: ids }],
      });
      // Attach the real clients (idempotent with the panels' guarded attach) so a
      // bridge-driven diverge/recalc never races the React mount effects.
      useResultsStore.getState().attachReadoutClient(createWasmReadoutClient());
      useConditioningStore.getState().attachConditioningClient(createWasmConditioningClient());
      useConditioningStore.getState().seedFromManifest(perSourceConditioning);
      useStaleStore.getState().attachIdentityClient(createWasmIdentityClient());
      // A square isophone grid whose cell count == receiverCount → the recondition recalc
      // re-feeds it 1:1 from the WASM-reconditioned lattice totals (map updates live).
      const side = Math.round(Math.sqrt(receiverCount));
      const grid: LevelGridInput = {
        rows: side,
        cols: side,
        origin: [500_000, 5_800_000],
        spacing_m: 10,
        values: Array.from({ length: side * side }, () => 50),
      };
      useColorScaleStore.getState().setIsophoneInput(grid, { utm_zone: 31, south: false }, "dB(A)");
      return { sourceIds: Array.from({ length: nSub }, (_, i) => String(i)), receiverIds: ids };
    },
    async divergeScene() {
      const m = useResultsStore.getState().manifest;
      if (!m) {
        return;
      }
      const mutated = JSON.parse(JSON.stringify(m.scene)) as PrepareSolveReq;
      // Move a receiver 5 m down-range — a genuine scene edit that re-mints a NEW identity.
      mutated.receivers[0].position[0] += 5;
      // The manifest scene is now the CURRENT (edited) scene; the cached tensor is still
      // keyed by the OLD hash, so a subsequent MAC against it is refused (SVC-06 409).
      useResultsStore.setState({ manifest: { ...m, scene: mutated } });
      if (!useStaleStore.getState().client) {
        useStaleStore.getState().attachIdentityClient(createWasmIdentityClient());
      }
      await useStaleStore.getState().checkStale(mutated, m.tensorHash);
    },
    conditioningState() {
      const s: ConditioningState = useConditioningStore.getState();
      return {
        perSource: { ...s.perSource },
        order: [...s.order],
        refuse: s.refuse,
        pending: s.pending,
        recalcEpoch: s.recalcEpoch,
      };
    },
    staleState() {
      return { isStale: useStaleStore.getState().isStale };
    },
    calcJobState() {
      return useCalcStore.getState().jobState;
    },
    async seedScenarios() {
      let projectId = useSceneStore.getState().projectId;
      if (!projectId) {
        projectId = "scenario-uat-project";
        useSceneStore.getState().setProject(projectId, "Scenario UAT");
      }
      useScenarioStore.getState().reset();
      const client = await seededScenarioClient();
      useScenarioStore.getState().attachClient(client);
      useDifferenceStore.getState().attachClient(createWasmDifferenceClient());
    },
    scenarioState() {
      const s = useScenarioStore.getState();
      return {
        scenarios: s.scenarios.map((sc) => ({
          id: sc.id,
          name: sc.name,
          computed: sc.computed,
        })),
        activeId: s.activeId,
        compareA: s.compareA,
        compareB: s.compareB,
        computeEpoch: s.computeEpoch,
        switchEpoch: s.switchEpoch,
      };
    },
    differenceState() {
      const s = useDifferenceStore.getState();
      return {
        hasDelta: s.delta !== null,
        epoch: s.epoch,
        labelA: s.labelA,
        labelB: s.labelB,
      };
    },
    differenceTelemetry() {
      return differenceTelemetry();
    },
  };
  (window as unknown as { __enviTest?: EnviTestBridge }).__enviTest = bridge;
}
