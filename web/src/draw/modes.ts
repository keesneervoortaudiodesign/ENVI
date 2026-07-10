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

import {
  TerraDrawLineStringMode,
  TerraDrawPointMode,
  TerraDrawPolygonMode,
  TerraDrawSelectMode,
} from "terra-draw";

import { KIND_META, isKind, type DrawTool } from "./kinds";

// The Terra Draw mode name a palette tool activates. `select` is the built-in pointer/edit mode; each
// drawing kind resolves to its geometry mode via `KIND_META` (three kinds share point, two share
// linestring, four share polygon).
export type TdModeName = "select" | "point" | "linestring" | "polygon";

export function tdModeName(tool: DrawTool): TdModeName {
  return isKind(tool) ? KIND_META[tool].mode : "select";
}

// Construct the mode list for the `TerraDraw` constructor. One instance per geometry keeps the adapter
// footprint minimal; the kind is carried in feature properties (D-03), so no per-kind mode is needed.
// The concrete-mode union is inferred and is assignable to the constructor's `modes` parameter.
export function buildModes() {
  return [
    new TerraDrawSelectMode(),
    new TerraDrawPointMode(),
    new TerraDrawLineStringMode(),
    new TerraDrawPolygonMode(),
  ];
}
