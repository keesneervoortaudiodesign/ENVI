// modes.ts — the Terra Draw mode set for the ENVI scene editor and the active-tool → TD-mode mapping.
//
// # Module I/O
// - Input  the shared registry of Terra Draw modes (one instance per geometry) and a `DrawTool` (the
//   palette's active tool).
// - Output `buildModes()` (the mode list handed to the `TerraDraw` constructor) and `tdModeName(tool)`
//   — the TD mode NAME (`"select" | "point" | "linestring" | "polygon"`) to `draw.setMode(...)` when the
//   palette tool changes. All 9 kinds fan into these four modes (D-09 `KIND_META.mode`); the specific
//   kind is tagged onto the finished feature as `properties.kind`, not distinguished by the TD mode.
// - Valid input range: any `DrawTool`; `"select"` maps to Terra Draw's built-in select mode.
//
// # Paint ownership — Terra Draw draws the EDIT, the display layers draw the OBJECT (D-17/D-18/D-19)
// Terra Draw's MapLibre adapter renders the SAME features the canonical store holds, through its own
// `td-point` / `td-linestring` / `td-polygon` layers, and (having no `renderBelowLayerId`) it appends those
// layers to the TOP of the style — above the `map/objectStyles` display layers. Left at their stock
// defaults, EVERY Terra Draw mode paints in the SAME hex (`#3f97e0` for `pointColor`, `lineStringColor`
// AND `polygonFillColor`/`polygonOutlineColor` alike), so every committed scene object came out one
// identical blue and the per-kind palette below it was never visible.
//
// The fix is a paint-ownership rule, not a second palette (D-19: the display map is NOT repurposed
// draw-time metadata, and draw-time styling must not mirror it either): Terra Draw paints ONLY the pixels
// it genuinely owns — the shape currently being drawn (the rubber band) and the currently-selected shape
// (the edit highlight). Every other feature is a COMMITTED scene object whose pixels belong to the
// `map/objectStyles` display layers, so Terra Draw renders it at zero opacity and stays out of the way.
// Draw-time BEHAVIOUR (modes, validation, topology, hit-testing) is untouched — only the paint is.

import {
  TerraDrawLineStringMode,
  TerraDrawPointMode,
  TerraDrawPolygonMode,
  TerraDrawSelectMode,
  type GeoJSONStoreFeatures,
} from "terra-draw";

import { KIND_META, isKind, type DrawTool } from "./kinds";

// The Terra Draw mode name a palette tool activates. `select` is the built-in pointer/edit mode; each
// drawing kind resolves to its geometry mode via `KIND_META` (three kinds share point, two share
// linestring, four share polygon).
export type TdModeName = "select" | "point" | "linestring" | "polygon";

export function tdModeName(tool: DrawTool): TdModeName {
  return isKind(tool) ? KIND_META[tool].mode : "select";
}

// Terra Draw's own feature flags (`COMMON_PROPERTIES.CURRENTLY_DRAWING` / `SELECT_PROPERTIES.SELECTED`).
// Neither constant is re-exported from the package root, so the literals are mirrored here.
const CURRENTLY_DRAWING = "currentlyDrawing";
const SELECTED = "selected";

// Terra Draw's stock draw-time feedback colour + fill opacity (the rubber band / edit highlight). This is
// DRAW-TIME chrome, deliberately ONE neutral colour for every geometry — it is not, and must never become,
// a second copy of the per-kind display palette (D-19).
const DRAW_FEEDBACK_FILL_OPACITY = 0.3;

// True while Terra Draw legitimately owns a feature's pixels: it is the shape being drawn right now, or the
// shape currently selected for editing. Everything else is a committed object the display layers render.
function isDrawTimeFeedback(feature: GeoJSONStoreFeatures): boolean {
  const props = feature.properties as Record<string, unknown> | undefined;
  return props?.[CURRENTLY_DRAWING] === true || props?.[SELECTED] === true;
}

// Opacity for a feature Terra Draw paints: fully opaque while it is draw-time feedback, fully TRANSPARENT
// once it is a committed object (the display layers own it — otherwise TD's single stock colour covers the
// per-kind palette and every object looks identical).
function feedbackOpacity(feature: GeoJSONStoreFeatures): number {
  return isDrawTimeFeedback(feature) ? 1 : 0;
}

function feedbackFillOpacity(feature: GeoJSONStoreFeatures): number {
  return isDrawTimeFeedback(feature) ? DRAW_FEEDBACK_FILL_OPACITY : 0;
}

// Construct the mode list for the `TerraDraw` constructor. One instance per geometry keeps the adapter
// footprint minimal; the kind is carried in feature properties (D-03), so no per-kind mode is needed.
// The concrete-mode union is inferred and is assignable to the constructor's `modes` parameter.
//
// Each drawing mode's styles zero the paint of every COMMITTED feature (see the paint-ownership note
// above). The mode's other styling slots — closing / snapping / coordinate / edited points — are left at
// their defaults: those are transient draw-time handles that exist only while drawing, and the display
// layers never render them.
export function buildModes() {
  return [
    new TerraDrawSelectMode(),
    new TerraDrawPointMode({
      styles: { pointOpacity: feedbackOpacity, pointOutlineOpacity: feedbackOpacity },
    }),
    new TerraDrawLineStringMode({
      styles: { lineStringOpacity: feedbackOpacity },
    }),
    new TerraDrawPolygonMode({
      styles: { fillOpacity: feedbackFillOpacity, outlineOpacity: feedbackOpacity },
    }),
  ];
}
