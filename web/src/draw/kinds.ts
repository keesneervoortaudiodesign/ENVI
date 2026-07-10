// kinds.ts — the 9-kind scene-object vocabulary (geojson.rs KINDS) as a TypeScript discriminated
// union plus per-kind palette/Terra-Draw metadata, with a `never`-exhaustive helper so a missing case
// is a COMPILE error (D-09).
//
// # Module I/O
// - Input  a `Kind` (one of the 9 frozen `properties.kind` strings) or a `DrawTool` (a Kind or the
//   pointer tool `"select"`). The metadata tokens/icons come from the approved 07-UI-SPEC palette table.
// - Output `KIND_META[kind]` — its Terra Draw geometry mode, palette icon name, kind-hue token
//   (an EXISTING theme token, never invented — D-11), and human label — plus `assertNeverKind`, the
//   exhaustiveness guard. `KIND_META` is a `Record<Kind, …>`, so removing a kind fails `tsc` (a missing
//   key), and any `switch (kind)` that reaches `assertNeverKind(kind)` fails `tsc` if a case is dropped.
// - Valid input range: `kind` ∈ KINDS (the 9 frozen strings). `assertNeverKind` is only reachable with a
//   value TypeScript has narrowed to `never`; calling it at runtime throws (defence in depth).

import type { IconName } from "../icons";

// The frozen 9-kind vocabulary — must mirror `crates/envi-store/src/geojson.rs` `KINDS` (line 38) and
// the `IconName` set in icons.ts. Ordering here is the palette display order (UI-SPEC).
export type Kind =
  | "source"
  | "receiver"
  | "wall"
  | "building"
  | "forest"
  | "ground_zone"
  | "elevation_point"
  | "elevation_line"
  | "calc_area";

// A palette tool: the pointer/select tool, or one of the 9 drawing kinds.
export type DrawTool = "select" | Kind;

// Terra Draw geometry mode a kind is drawn with. Multiple kinds share one mode (three point kinds, two
// line kinds, four polygon kinds) — the specific kind is carried in `properties.kind`, not the mode.
export type TdGeometryMode = "point" | "linestring" | "polygon";

// Per-kind palette + map metadata. Every colour is an existing theme token (UI-SPEC palette table).
export interface KindMeta {
  readonly kind: Kind;
  readonly label: string;
  readonly mode: TdGeometryMode;
  readonly icon: IconName;
  // The kind-hue token used for the palette `.dot` and (later) the map render — an EXISTING theme
  // token (D-11 "do not invent tokens"). Reused across geometry types, never within one (UI-SPEC).
  readonly hueToken: string;
}

// The 9 frozen kinds in palette order.
export const KINDS: readonly Kind[] = [
  "source",
  "receiver",
  "wall",
  "building",
  "forest",
  "ground_zone",
  "elevation_point",
  "elevation_line",
  "calc_area",
] as const;

// A `Record<Kind, KindMeta>`: TypeScript requires ALL 9 keys, so deleting a kind from `Kind` (or a row
// here) fails `tsc` — the structural half of the D-09 exhaustiveness guarantee.
export const KIND_META: Record<Kind, KindMeta> = {
  source: { kind: "source", label: "Source", mode: "point", icon: "source", hueToken: "--color-primary" },
  receiver: { kind: "receiver", label: "Receiver", mode: "point", icon: "receiver", hueToken: "--color-ok" },
  wall: { kind: "wall", label: "Wall / screen", mode: "linestring", icon: "wall", hueToken: "--color-text" },
  building: { kind: "building", label: "Building", mode: "polygon", icon: "building", hueToken: "--color-text-muted" },
  forest: { kind: "forest", label: "Forest", mode: "polygon", icon: "forest", hueToken: "--color-ok" },
  ground_zone: { kind: "ground_zone", label: "Ground zone", mode: "polygon", icon: "ground_zone", hueToken: "--color-off" },
  elevation_point: { kind: "elevation_point", label: "Elevation point", mode: "point", icon: "elevation_point", hueToken: "--color-info" },
  elevation_line: { kind: "elevation_line", label: "Elevation line", mode: "linestring", icon: "elevation_line", hueToken: "--color-info" },
  calc_area: { kind: "calc_area", label: "Calc area", mode: "polygon", icon: "calc_area", hueToken: "--color-primary" },
};

// Compile-time exhaustiveness guard (D-09): reachable only with a value narrowed to `never`. A dropped
// `case` in any `switch (kind)` that ends here becomes a `tsc` error ("not assignable to never").
export function assertNeverKind(x: never): never {
  throw new Error(`Unhandled scene kind: ${String(x)}`);
}

// Narrow a `DrawTool` to a `Kind` (false for the pointer tool). Used at the draw-commit boundary.
export function isKind(tool: DrawTool): tool is Kind {
  return tool !== "select";
}
