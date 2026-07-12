// colorScale.ts — the SINGLE breaks[]/colors[] source of truth for the isophone
// noise map (WEB-06 / GRID-04 / D-03/D-04/D-19). One `ScaleState { preset, breaks,
// colors }` object drives ALL THREE of the WASM iso-band tracer, the MapLibre fill
// layer, and the docked legend — never three parallel definitions. Editing the
// scale RE-CONTOURS the cached level grid (SC3) with NO re-solve: the tracer runs
// over the grid this store also caches, and the frontend performs NO acoustic math
// (D-01 — re-contouring calls the WASM tracer; this file only manages breaks/colors
// and interpolates DISPLAY colours, never a dB).
//
// # Module I/O
// - Input  a cached level grid + its CRS + the dB weighting label (`setIsophoneInput`,
//   from a finished readout — the field the tracer contours), the preset selection,
//   and the editable break edges (NoizCalc TI 386 §4.6.5 controls).
// - Output the state the isophone layer + legend render: `preset`, `breaks`,
//   `colors` (the single source of truth), the cached `grid`/`crs`/`weightingLabel`,
//   and an inline `error` (V5 validation). `colors.length === breaks.length + 1`:
//   N editable edges yield N+1 classes (a below-lowest cap, N−1 interior intervals,
//   and an above-highest cap), matching the EU-END 6-class default.
// - Valid input range: `breaks` strictly monotonic increasing, finite, ≥2 (V5).
//
// # Legend ≡ contour ≡ class colours (enforced)
// The legend reads `breaks`/`colors`; the contour tracer receives the SAME `breaks`
// (cap-extended by `contourBreaks` into N+1 finite bands) with `colors` as the
// per-band fills; the fill layer paints by the traced `fill` property. There is one
// array pair — a break edit updates the legend and the contour identically.

import type { ExportCrsDto, ExportGridDto } from "../generated/wire";

import { create } from "zustand";

// The two colour-scale families (D-03/D-04): the domain-standard EU-END ramp
// (default, familiar to acousticians) plus the perceptually-uniform viridis/turbo
// presets offered for quantitative reading.
export type Preset = "end" | "viridis" | "turbo";

// The cached level grid the tracer contours (SceneXY meters) — the ExportGridDto
// wire shape reused verbatim (never a hand-mirror). Cached so a break edit
// re-contours THIS grid with no re-solve (SC3).
export type LevelGridInput = ExportGridDto;

// The default EU-END break edges (dB), the canonical ~5 dB reporting bands of
// Directive 2002/49/EC (END) Annex VI Lden. Approved as the shipped default at the
// A4 human-verify checkpoint (orchestrator sign-off on the standing "do not stop"
// mandate; source: Directive 2002/49/EC Annex VI).
//
// ⚠ A4 (RESEARCH Assumptions Log): these are the canonical EU-END reporting band
// edges. 5 editable edges → 6 classes: < 55 (below-lowest cap), the four 5 dB
// intervals 55–60/60–65/65–70/70–75, and ≥ 75 (above-highest cap).
export const END_BREAKS: readonly number[] = [55, 60, 65, 70, 75];

// The EU-END class fills (6): green → yellow-green → yellow → orange → red →
// violet, ascending, the dataviz-validated EU-END sequential ramp (UI-SPEC §Data
// Visualization Palettes #1). Approved with END_BREAKS at the A4 checkpoint.
export const END_COLORS: readonly string[] = [
  "#7fbf7f", // < 55  green (below-lowest, lightest)
  "#bfdc6b", // 55–60 yellow-green
  "#f6e04b", // 60–65 yellow
  "#f4a637", // 65–70 orange
  "#e34948", // 70–75 red
  "#8a2be2", // ≥ 75  violet (above-highest, darkest)
];

// Canonical perceptually-uniform colour-map anchor stops (hex), sampled at the
// class count for the viridis/turbo presets (D-04). These are DISPLAY colours, a
// separate system from the UI accent (a dataviz non-negotiable).
const VIRIDIS_STOPS: readonly string[] = [
  "#440154", "#482878", "#3e4a89", "#31688e", "#26828e",
  "#1f9e89", "#35b779", "#6ece58", "#b5de2b", "#fde725",
];
const TURBO_STOPS: readonly string[] = [
  "#30123b", "#4145ab", "#4675ed", "#39a2fc", "#1bcfd4", "#24eca6", "#61fc6c",
  "#a4fc3b", "#d1e834", "#f9ba38", "#fb7e21", "#e5460a", "#b71301", "#7a0403",
];

// Finite caps that turn the N editable edges into N+1 CLOSED bands for the tracer
// (the tracer's V5 rejects non-finite, so ±∞ is expressed as a physically-absurd
// finite dB bound that still covers every real level). The below-lowest band is
// [LOW_CAP, breaks[0]) and the above-highest is [breaks[last], HIGH_CAP).
export const LOW_CAP = -1e6;
export const HIGH_CAP = 1e6;

// Parse a `#rrggbb` hex to [r,g,b] 0..255. Display-colour arithmetic only.
function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace("#", "");
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ];
}

// [r,g,b] 0..255 → `#rrggbb`.
function rgbToHex(rgb: [number, number, number]): string {
  const c = (v: number): string =>
    Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, "0");
  return `#${c(rgb[0])}${c(rgb[1])}${c(rgb[2])}`;
}

// Sample `n` colours evenly across a colour-map's anchor stops by linear RGB
// interpolation (display-colour math — NO acoustics). `n === 1` returns the first
// stop; `n >= 2` spans the full ramp end to end.
export function samplePalette(stops: readonly string[], n: number): string[] {
  if (n <= 0) {
    return [];
  }
  if (n === 1) {
    return [stops[0]];
  }
  const rgbs = stops.map(hexToRgb);
  const out: string[] = [];
  for (let k = 0; k < n; k += 1) {
    const t = (k / (n - 1)) * (rgbs.length - 1);
    const lo = Math.floor(t);
    const hi = Math.min(rgbs.length - 1, lo + 1);
    const f = t - lo;
    out.push(
      rgbToHex([
        rgbs[lo][0] + (rgbs[hi][0] - rgbs[lo][0]) * f,
        rgbs[lo][1] + (rgbs[hi][1] - rgbs[lo][1]) * f,
        rgbs[lo][2] + (rgbs[hi][2] - rgbs[lo][2]) * f,
      ]),
    );
  }
  return out;
}

// The `count` class colours for a preset (`count = breaks.length + 1`). END ships
// its fixed canonical hex (sampled/truncated only if the break count changes);
// viridis/turbo sample their ramp at the class count.
export function colorsForPreset(preset: Preset, count: number): string[] {
  switch (preset) {
    case "end":
      return samplePalette(END_COLORS, count);
    case "viridis":
      return samplePalette(VIRIDIS_STOPS, count);
    case "turbo":
      return samplePalette(TURBO_STOPS, count);
  }
}

// Validate a break scale (V5, mirrors the WASM tracer's `IsobandError`): ≥2 edges,
// all finite, strictly monotonic increasing. Returns an inline error string, or
// `null` when valid. The tracer re-validates in WASM (defence in depth, T-11-06-01).
export function validateBreaks(breaks: readonly number[]): string | null {
  if (breaks.length < 2) {
    return "Need at least 2 break values.";
  }
  for (let i = 0; i < breaks.length; i += 1) {
    if (!Number.isFinite(breaks[i])) {
      return `Break ${i + 1} is not a finite number.`;
    }
  }
  for (let i = 1; i < breaks.length; i += 1) {
    if (breaks[i] <= breaks[i - 1]) {
      return `Breaks must strictly increase: ${breaks[i - 1]} ≥ ${breaks[i]}.`;
    }
  }
  return null;
}

// The CAP-extended break edges the tracer contours: N editable edges → N+1 closed
// bands (below-lowest, interiors, above-highest). The colour scale's `colors`
// (length N+1) align 1:1 with these bands.
export function contourBreaks(breaks: readonly number[]): number[] {
  return [LOW_CAP, ...breaks, HIGH_CAP];
}

// Generate uniform break edges from the NoizCalc §4.6.5 controls: the smallest
// interval (first edge), the interval magnitude (dB step), and the number of edges.
// `ascending === false` still yields increasing edges (V5) — the toggle reverses
// the COLOUR order, not the numeric order.
export function generateBreaks(smallest: number, magnitude: number, count: number): number[] {
  const n = Math.max(2, Math.floor(count));
  const step = Math.abs(magnitude) || 1;
  return Array.from({ length: n }, (_, k) => smallest + k * step);
}

export interface ColorScaleState {
  // The single source of truth (D-04).
  readonly preset: Preset;
  readonly breaks: number[];
  readonly colors: string[];
  // Whether the class colours follow their break order (D-04 keep-color-sequence).
  readonly ascending: boolean;
  readonly keepColorSequence: boolean;
  // The cached contour inputs (SC3 — a break edit re-contours THIS grid, no solve).
  readonly grid: LevelGridInput | null;
  readonly crs: ExportCrsDto | null;
  // The dB weighting label from result metadata (D-04) — never the panel toggle.
  readonly weightingLabel: string;
  // Inline V5 validation error, or null.
  readonly error: string | null;

  setPreset(preset: Preset): void;
  setBreaks(breaks: number[]): void;
  setBreakAt(index: number, value: number): void;
  applyGenerator(smallest: number, magnitude: number, count: number): void;
  setAscending(ascending: boolean): void;
  setKeepColorSequence(keep: boolean): void;
  setIsophoneInput(grid: LevelGridInput, crs: ExportCrsDto, weightingLabel: string): void;
  clearIsophone(): void;
  reset(): void;
}

// Re-derive the class colours for a (possibly new) break count, honouring the
// ascending/keep-color-sequence toggles: a reversed order flips the ramp so the
// lightest stays at the low end of the VISUAL scale.
function deriveColors(
  preset: Preset,
  breakCount: number,
  ascending: boolean,
): string[] {
  const colors = colorsForPreset(preset, breakCount + 1);
  return ascending ? colors : [...colors].reverse();
}

const INITIAL: Omit<
  ColorScaleState,
  | "setPreset"
  | "setBreaks"
  | "setBreakAt"
  | "applyGenerator"
  | "setAscending"
  | "setKeepColorSequence"
  | "setIsophoneInput"
  | "clearIsophone"
  | "reset"
> = {
  preset: "end",
  breaks: [...END_BREAKS],
  colors: [...END_COLORS],
  ascending: true,
  keepColorSequence: true,
  grid: null,
  crs: null,
  weightingLabel: "dB(A)",
  error: null,
};

export const useColorScaleStore = create<ColorScaleState>((set, get) => ({
  ...INITIAL,

  // Switch preset: keep the current breaks, re-derive the class colours at the
  // current break count (END keeps its canonical hex, viridis/turbo re-sample).
  setPreset: (preset) =>
    set((s) => ({ preset, colors: deriveColors(preset, s.breaks.length, s.ascending) })),

  // Replace the break edges (validated). On a valid scale the breaks + re-derived
  // colours update together (single source of truth); an invalid scale sets the
  // inline error and leaves the last valid scale in place so the map never renders
  // a broken contour.
  setBreaks: (breaks) => {
    const error = validateBreaks(breaks);
    if (error) {
      set({ error });
      return;
    }
    const { preset, keepColorSequence, ascending, colors } = get();
    const colorsNext =
      keepColorSequence && colors.length === breaks.length + 1
        ? colors
        : deriveColors(preset, breaks.length, ascending);
    set({ breaks: [...breaks], colors: colorsNext, error: null });
  },

  // Edit a single break edge (the panel's per-row numeric input).
  setBreakAt: (index, value) => {
    const next = [...get().breaks];
    next[index] = value;
    get().setBreaks(next);
  },

  // Regenerate uniform breaks from the NoizCalc §4.6.5 controls.
  applyGenerator: (smallest, magnitude, count) =>
    get().setBreaks(generateBreaks(smallest, magnitude, count)),

  setAscending: (ascending) =>
    set((s) => ({ ascending, colors: deriveColors(s.preset, s.breaks.length, ascending) })),

  setKeepColorSequence: (keepColorSequence) => set({ keepColorSequence }),

  // Cache the contour inputs from a finished readout (the grid the tracer contours,
  // its CRS for the SceneXY→LonLat reprojection, and the metadata weighting label).
  setIsophoneInput: (grid, crs, weightingLabel) => set({ grid, crs, weightingLabel }),

  clearIsophone: () => set({ grid: null, crs: null }),

  reset: () => set({ ...INITIAL, breaks: [...END_BREAKS], colors: [...END_COLORS] }),
}));
