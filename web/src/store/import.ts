// import.ts — the client-side import state slice (D-08: import progress/status is IN-APP React state, NOT
// the Phase-6 server SSE job machine). One independent status record per layer (terrain / land cover /
// buildings) so a failed layer lands what succeeded and retries without blocking its siblings (D-07).
//
// # Module I/O
// - Input  the import orchestrator (`importJob`) driving each layer's lifecycle (start → progress →
//   done/error), the map's `ViewportTracker` writing the current WGS84 viewport, the guardrail evaluation,
//   the ImportPanel's per-layer toggles + debug-overlay toggle, and the set of sources that contributed
//   features (for the SC5 map attribution).
// - Output the per-layer `LayerState` (status + progress + toggle + last error + committed feature count +
//   the terrain surface-model flag), the guardrail state (viewport over budget), `attributedSources`
//   (drives `AttributionControl`), and the `debugOverlay` toggle (SC3 impedance overlay). The ImportPanel
//   and the map overlay/attribution read exactly these — the single source of truth for import UI.
// - Valid input range: `viewport` is WGS84 degrees or null (no map yet); layer keys are the three fixed
//   raster/vector layers.

import { create } from "zustand";

import type { BboxDto } from "../generated/wire";

// The three independently-toggleable import layers (D-06).
export type LayerKey = "terrain" | "landcover" | "buildings";
export const LAYER_KEYS: readonly LayerKey[] = ["terrain", "landcover", "buildings"];

// A single layer's lifecycle status (mirrors the dgm slice's honest success/failure recording).
export type LayerStatus = "idle" | "running" | "done" | "error";

// A structured layer failure: HTTP-ish status + a path-redacted detail (rendered as TEXT, never innerHTML).
export interface LayerError {
  readonly status: number;
  readonly detail: string;
}

// One layer's independent state (D-07: a failure here never touches another layer).
export interface LayerState {
  readonly status: LayerStatus;
  // The per-layer toggle (whether this layer participates in the next import). Default on.
  readonly enabled: boolean;
  // Client-side progress in `[0, 1]` (D-08 — not the SSE job machine's fractional progress).
  readonly progress: number;
  // A short human step label ("fetching tiles", "committing", …).
  readonly message: string;
  // The last failure (with a retry available in the panel), or null.
  readonly error: LayerError | null;
  // How many features this layer committed on its last successful import (UI feedback).
  readonly featureCount: number;
  // Terrain only: the resolved source is GLO-30, a surface model (D-05 badge).
  readonly surfaceModel: boolean;
}

// The max-area guardrail verdict (D-06): whether the current viewport is over the fetch budget.
export interface GuardrailState {
  // True when the viewport exceeds the block threshold (import is refused) — a warn-only state uses
  // `blocked: false` with a populated `detail`.
  readonly blocked: boolean;
  readonly detail: string;
}

function idleLayer(): LayerState {
  return {
    status: "idle",
    enabled: true,
    progress: 0,
    message: "",
    error: null,
    featureCount: 0,
    surfaceModel: false,
  };
}

export interface ImportState {
  // The current map viewport (WGS84), written by the map's ViewportTracker; the import target (D-06).
  readonly viewport: BboxDto | null;
  // The guardrail verdict for the current viewport, or null when within budget.
  readonly guardrail: GuardrailState | null;
  // Per-layer independent status (D-07).
  readonly layers: Readonly<Record<LayerKey, LayerState>>;
  // Registry source ids that contributed committed features (drives the SC5 AttributionControl).
  readonly attributedSources: readonly string[];
  // Whether the impedance debug overlay is shown (SC3).
  readonly debugOverlay: boolean;

  setViewport(bbox: BboxDto): void;
  setGuardrail(guardrail: GuardrailState | null): void;
  setLayerEnabled(layer: LayerKey, enabled: boolean): void;
  startLayer(layer: LayerKey): void;
  setLayerProgress(layer: LayerKey, progress: number, message: string): void;
  finishLayer(layer: LayerKey, result: { featureCount: number; surfaceModel?: boolean }): void;
  failLayer(layer: LayerKey, error: LayerError): void;
  addAttributedSources(sourceIds: readonly string[]): void;
  toggleDebugOverlay(): void;
}

// Immutably patch one layer's state (keeps the other two untouched — the D-07 independence guarantee).
function patchLayer(
  layers: Readonly<Record<LayerKey, LayerState>>,
  layer: LayerKey,
  patch: Partial<LayerState>,
): Record<LayerKey, LayerState> {
  return { ...layers, [layer]: { ...layers[layer], ...patch } };
}

export const useImportStore = create<ImportState>((set) => ({
  viewport: null,
  guardrail: null,
  layers: {
    terrain: idleLayer(),
    landcover: idleLayer(),
    buildings: idleLayer(),
  },
  attributedSources: [],
  debugOverlay: false,

  setViewport: (bbox) => set({ viewport: bbox }),
  setGuardrail: (guardrail) => set({ guardrail }),

  setLayerEnabled: (layer, enabled) =>
    set((s) => ({ layers: patchLayer(s.layers, layer, { enabled }) })),

  startLayer: (layer) =>
    set((s) => ({
      layers: patchLayer(s.layers, layer, {
        status: "running",
        progress: 0,
        message: "starting",
        error: null,
      }),
    })),

  setLayerProgress: (layer, progress, message) =>
    set((s) => ({
      layers: patchLayer(s.layers, layer, {
        progress: Math.max(0, Math.min(1, progress)),
        message,
      }),
    })),

  finishLayer: (layer, result) =>
    set((s) => ({
      layers: patchLayer(s.layers, layer, {
        status: "done",
        progress: 1,
        message: "done",
        error: null,
        featureCount: result.featureCount,
        surfaceModel: result.surfaceModel ?? s.layers[layer].surfaceModel,
      }),
    })),

  failLayer: (layer, error) =>
    set((s) => ({ layers: patchLayer(s.layers, layer, { status: "error", error }) })),

  addAttributedSources: (sourceIds) =>
    set((s) => {
      const next = new Set(s.attributedSources);
      for (const id of sourceIds) {
        next.add(id);
      }
      return { attributedSources: [...next] };
    }),

  toggleDebugOverlay: () => set((s) => ({ debugOverlay: !s.debugOverlay })),
}));
