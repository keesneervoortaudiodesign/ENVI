// objectStyles.ts — the per-kind DISPLAY styling for the MapLibre scene-object layers (D-17/D-18/D-19,
// NoizCalc TI 386 §4.6.3). This is a SEPARATE display-styling map, NEVER a repurposing of the draw-time
// `KIND_META` in `draw/kinds.ts`: display layers (this file, `web/src/map/`) and Terra Draw's draw-time
// styling/validation are two independent systems (D-19 / Pitfall 8). Objects render at FULL styling ON TOP
// of the 11-06 isophone fill (D-18) and stay legible over it via the dataviz-validated categorical palette
// + a geometry-specific SECONDARY channel (symbol / width / hatch — the load-bearing accessibility encoding
// for the CVD floor band, dataviz relief rule).
//
// # Module I/O
// - Input  a `Kind` (one of the 9 frozen `properties.kind` strings). No user string ever reaches image
//   generation (T-11-10-02: colours/hatch are program-generated from this fixed table — no XSS surface).
// - Output `objectStyles[kind]` — a `KindDisplayStyle` (a discriminated union on `geometry`): points carry
//   colour + glyph + size + border; lines carry colour + width (+ optional dash); areas carry fill colour +
//   fill-opacity + border + a SEPARATE hatch id. Plus the MapLibre image-id helpers (`hatchImageId`,
//   `pointIconId`) and a module-level telemetry singleton (`objectLayerTelemetry`) the offline UAT reads.
// - Valid input range: `kind` ∈ the 9 frozen kinds. Every hex here MIRRORS a `--obj-*` token in
//   `web/src/theme.css` (added this phase); the literal hex is the source of truth for MapLibre paint
//   because MapLibre GL cannot consume CSS custom properties (same pattern as the isophone `colorScale`
//   hex). The 8-slot categorical set is the dataviz-validated dark palette from 11-UI-SPEC §Palettes #3.
//
// # Paint ownership (load-bearing — see `draw/modes.ts`)
// These display layers own the pixels of a COMMITTED scene object. Terra Draw's MapLibre adapter renders
// the very same store features through its own `td-*` layers, which it appends ABOVE these — and at TD's
// stock defaults every mode paints in ONE hex (`#3f97e0` for points, lines AND polygon fills alike), so a
// stock Terra Draw silently covers this entire palette and every object renders identically blue. The
// modes in `draw/modes.ts` therefore paint a committed feature at ZERO opacity and keep their colour for
// the shape being drawn / selected only. Do not re-enable TD's committed-feature paint (`objectStyling`
// e2e asserts the rendered pixels + TD's zero opacity, and `draw/modes.test.ts` pins the rule).

import type { Kind } from "../draw/kinds";

// The geometry family a kind renders as (points/lines/areas — NoizCalc §4.6.3 formatting classes). A kind's
// display geometry mirrors its Terra Draw mode but is declared independently so the display map never reads
// draw-time metadata (D-19 separation).
export type ObjGeometry = "point" | "line" | "area";

// The runtime-generated hatch (arcering) pattern id for an area kind (angles from the dataviz texture rule
// 45°/135°). Realized as an `ImageData`-shaped raster by `hatchPatterns.ts` → `map.addImage`.
export type HatchId = "solid-diagonal-45" | "dotted" | "cross-hatch" | "sparse-135";

// The runtime-generated point marker glyph for a point kind (§4.6.3 "symbol"). Realized as an icon raster
// by `hatchPatterns.ts` → `map.addImage`, rendered by a `symbol` layer's `icon-image`.
export type PointGlyph = "disc" | "ring" | "diamond";

// Point display style: colour identity + a symbol glyph + a plan-size + a border (§4.6.3 point model).
export interface PointStyle {
  readonly geometry: "point";
  readonly color: string;
  readonly glyph: PointGlyph;
  // The marker footprint in device px (the generated icon is drawn at this size; `size` = the symbol size).
  readonly size: number;
  readonly border: string;
  readonly borderWidth: number;
}

// Line display style: colour + width (§4.6.3 line model). An optional dash array is the line's secondary
// differentiator (e.g. elevation lines are dashed to separate them from a solid wall/screen).
export interface LineStyle {
  readonly geometry: "line";
  readonly color: string;
  readonly width: number;
  readonly dash: readonly number[] | null;
}

// Area display style: a semi-transparent fill colour + a border + a SEPARATE hatch pattern (§4.6.3 area
// model). The semi-transparent fill (§4.6.5) + the hatch let a building/zone/forest read over the isophone
// fill (D-18). An optional dashed border frames the low-fill calc area.
export interface AreaStyle {
  readonly geometry: "area";
  readonly color: string;
  readonly fillOpacity: number;
  readonly border: string;
  readonly borderWidth: number;
  readonly borderDash: readonly number[] | null;
  readonly hatch: HatchId;
}

export type KindDisplayStyle = PointStyle | LineStyle | AreaStyle;

// The 8-slot dataviz-validated dark categorical palette (11-UI-SPEC §Palettes #3, validated against surface
// `#14171c` on 2026-07-12: Lightness PASS · Chroma PASS · CVD WARN in the 8–12 floor band — legal ONLY
// because hatch/symbol + legend are the mandated secondary encoding · Contrast PASS). Colour may repeat
// ACROSS geometry types but is unique WITHIN a geometry type (the `kinds.ts` reuse principle, extended).
//
// Each hex MIRRORS a `--obj-*` token in `theme.css`; the literal is the MapLibre-paint source of truth.
export const objectStyles: Record<Kind, KindDisplayStyle> = {
  // Points (unique hues within the point family): source blue, receiver aqua, elevation_point yellow.
  source: { geometry: "point", color: "#3987e5", glyph: "disc", size: 16, border: "#0b0d10", borderWidth: 2 },
  receiver: { geometry: "point", color: "#199e70", glyph: "ring", size: 16, border: "#0b0d10", borderWidth: 2 },
  elevation_point: { geometry: "point", color: "#c98500", glyph: "diamond", size: 15, border: "#0b0d10", borderWidth: 2 },
  // Lines (unique within the line family): wall a 3px solid red, elevation_line a 2px dashed yellow.
  wall: { geometry: "line", color: "#e66767", width: 3, dash: null },
  elevation_line: { geometry: "line", color: "#c98500", width: 2, dash: [2, 2] },
  // Areas (unique within the area family): each a distinct hue + fill-opacity + a distinct hatch.
  building: { geometry: "area", color: "#9085e9", fillOpacity: 0.45, border: "#9085e9", borderWidth: 1, borderDash: null, hatch: "solid-diagonal-45" },
  forest: { geometry: "area", color: "#008300", fillOpacity: 0.35, border: "#008300", borderWidth: 1, borderDash: null, hatch: "dotted" },
  ground_zone: { geometry: "area", color: "#d95926", fillOpacity: 0.30, border: "#d95926", borderWidth: 1, borderDash: null, hatch: "cross-hatch" },
  calc_area: { geometry: "area", color: "#d55181", fillOpacity: 0.15, border: "#d55181", borderWidth: 1, borderDash: [3, 2], hatch: "sparse-135" },
};

// The MapLibre image id under which an area kind's hatch raster is registered (`map.addImage`) and
// referenced (`fill-pattern`). A fixed, program-derived id — never a user string (T-11-10-02).
export function hatchImageId(kind: Kind): string {
  return `envi-hatch-${kind}`;
}

// The MapLibre image id under which a point kind's marker-glyph raster is registered + referenced
// (`icon-image`). A fixed, program-derived id — never a user string (T-11-10-02).
export function pointIconId(kind: Kind): string {
  return `envi-marker-${kind}`;
}

// --- Telemetry (offline UAT observability, mirrors isophoneLayer.TELEMETRY) -------------------------------

// A JSON-safe snapshot the object-styling UAT reads to prove the display layers exist, carry hatch on the
// area kinds, and sit ABOVE the isophone fill (D-18 draw order) — without reimplementing the app (D-01).
export interface ObjectLayerTelemetry {
  // The ids of the object display layers currently registered on the style, in style order.
  registeredLayers: string[];
  // The MapLibre image ids registered for the hatch patterns + point markers (via `map.addImage`).
  registeredImages: string[];
  // The number of scene features fed to the object source (0 when the scene is empty).
  featureCount: number;
  // The full style layer-id order (bottom→top). The UAT asserts every object layer index exceeds the
  // isophone-fill index — objects on top of the noise fill (D-18).
  layerOrder: string[];
  // True when every registered object layer sits ABOVE the isophone fill layer in `layerOrder`. Null when
  // the isophone layer is absent (nothing to compare against yet).
  aboveIsophone: boolean | null;
}

const TELEMETRY: ObjectLayerTelemetry = {
  registeredLayers: [],
  registeredImages: [],
  featureCount: 0,
  layerOrder: [],
  aboveIsophone: null,
};

// Overwrite the telemetry snapshot (called by the imperative layer controller after each apply). Kept here
// (module-level, browser-shared) so `testBridge` can read it the same way it reads `isophoneTelemetry`.
export function setObjectLayerTelemetry(next: ObjectLayerTelemetry): void {
  TELEMETRY.registeredLayers = next.registeredLayers;
  TELEMETRY.registeredImages = next.registeredImages;
  TELEMETRY.featureCount = next.featureCount;
  TELEMETRY.layerOrder = next.layerOrder;
  TELEMETRY.aboveIsophone = next.aboveIsophone;
}

// A snapshot of the object-layer telemetry for the offline UAT / the DEV test bridge.
export function objectLayerTelemetry(): ObjectLayerTelemetry {
  return {
    registeredLayers: [...TELEMETRY.registeredLayers],
    registeredImages: [...TELEMETRY.registeredImages],
    featureCount: TELEMETRY.featureCount,
    layerOrder: [...TELEMETRY.layerOrder],
    aboveIsophone: TELEMETRY.aboveIsophone,
  };
}
