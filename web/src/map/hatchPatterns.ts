// hatchPatterns.ts — the runtime image generators for the scene-object display layers (D-17, NoizCalc
// TI 386 §4.6.3 "area = fill + border + SEPARATE hatch pattern", + the §4.6.3 point "symbol"). MapLibre GL
// has no native hatch, so each area kind's hatch (arcering) and each point kind's marker glyph is generated
// at runtime as an `ImageData`-shaped raster (`{ width, height, data }`) and registered via
// `map.addImage(id, raster)` — the hatch as a `fill-pattern`, the marker as a symbol `icon-image`.
//
// # Module I/O
// - Input  a `Kind` (the hatch/marker is chosen from the fixed `objectStyles` table — no user string ever
//   reaches pixel generation, T-11-10-02: no XSS surface).
// - Output `hatchPattern(kind)` → the area kind's tiling hatch raster; `pointMarker(kind)` → the point
//   kind's marker-glyph raster. Both are `RasterImage` (`{ width, height, data: Uint8ClampedArray }`), which
//   is structurally an `ImageData` and is accepted directly by `map.addImage`. Generating pixels directly
//   (rather than via a DOM `<canvas>`) keeps these PURE + unit-testable in the Node vitest env (no jsdom /
//   `canvas` dependency) while staying byte-identical to what a canvas would produce.
// - Valid input range: `hatchPattern` is defined for the four AREA kinds; `pointMarker` for the three POINT
//   kinds. Calling either with a mismatched kind throws (defence in depth — the layer wiring never does).

import { objectStyles, type HatchId, type PointGlyph } from "./objectStyles";
import type { Kind } from "../draw/kinds";

// An `ImageData`-shaped raster: RGBA, row-major, `width * height * 4` bytes. `map.addImage` accepts this
// plain shape (a `StyleImageInterface`) exactly like a DOM `ImageData`.
export interface RasterImage {
  readonly width: number;
  readonly height: number;
  readonly data: Uint8ClampedArray;
}

// The hatch tile edge (px). 16 tiles densely enough for a legible pattern at map zoom without moiré.
const TILE = 16;
// The point marker canvas edge (px). 24 gives crisp glyph edges at the on-map icon size.
const MARKER = 24;

// Parse a `#rrggbb` hex into [r, g, b] (0–255). The input is always a fixed palette literal (never a user
// string) — a program-controlled value, so a strict 6-digit parse is sufficient.
function rgb(hex: string): [number, number, number] {
  const n = Number.parseInt(hex.slice(1), 16);
  return [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}

// Write one RGBA pixel into a raster buffer (no-op for out-of-bounds, so shape drawing can be naive).
function put(data: Uint8ClampedArray, w: number, h: number, x: number, y: number, r: number, g: number, b: number, a: number): void {
  if (x < 0 || y < 0 || x >= w || y >= h) {
    return;
  }
  const i = (y * w + x) * 4;
  data[i] = r;
  data[i + 1] = g;
  data[i + 2] = b;
  data[i + 3] = a;
}

// Generate an area kind's tiling hatch raster (transparent background + opaque coloured strokes in the kind
// hue). Angles are 45°/135° per the dataviz texture rule; the four hatch families are visually distinct so
// the area kinds separate by TEXTURE even where the CVD-floor hues are close (the relief rule, D-19).
export function hatchPattern(kind: Kind): RasterImage {
  const style = objectStyles[kind];
  if (style.geometry !== "area") {
    throw new Error(`hatchPattern: ${kind} is not an area kind`);
  }
  const [r, g, b] = rgb(style.color);
  const data = new Uint8ClampedArray(TILE * TILE * 4); // zero-filled ⇒ fully transparent background
  const stroke = (x: number, y: number): void => put(data, TILE, TILE, x, y, r, g, b, 235);
  const drawDiagonal45 = (): void => {
    // Lines of constant (x + y): a 45° family, one stroke every 6 px.
    for (let y = 0; y < TILE; y += 1) {
      for (let x = 0; x < TILE; x += 1) {
        if ((x + y) % 6 === 0) {
          stroke(x, y);
        }
      }
    }
  };
  const drawDiagonal135 = (): void => {
    // Lines of constant (x - y): a 135° family, sparse (every 8 px) for the low-emphasis calc-area frame.
    for (let y = 0; y < TILE; y += 1) {
      for (let x = 0; x < TILE; x += 1) {
        if (((x - y + TILE * 2) % 8) === 0) {
          stroke(x, y);
        }
      }
    }
  };
  const map: Record<HatchId, () => void> = {
    "solid-diagonal-45": drawDiagonal45,
    "sparse-135": drawDiagonal135,
    "cross-hatch": () => {
      drawDiagonal45();
      drawDiagonal135();
    },
    dotted: () => {
      // A stipple: a dot on a 4 px lattice (offset rows) — the forest texture.
      for (let y = 1; y < TILE; y += 4) {
        for (let x = 1; x < TILE; x += 4) {
          const ox = ((y / 4) & 1) === 0 ? 0 : 2;
          stroke(x + ox, y);
        }
      }
    },
  };
  map[style.hatch]();
  return { width: TILE, height: TILE, data };
}

// Generate a point kind's marker-glyph raster (the §4.6.3 "symbol"): a filled disc / hollow ring / diamond
// in the kind hue with a dark border (the point secondary channel = symbol + size + border). Rendered by a
// `symbol` layer's `icon-image`; icon-size scales it to the style's `size`.
export function pointMarker(kind: Kind): RasterImage {
  const style = objectStyles[kind];
  if (style.geometry !== "point") {
    throw new Error(`pointMarker: ${kind} is not a point kind`);
  }
  const [r, g, b] = rgb(style.color);
  const [br, bg, bb] = rgb(style.border);
  const data = new Uint8ClampedArray(MARKER * MARKER * 4);
  const c = (MARKER - 1) / 2;
  const outer = MARKER / 2 - 2; // leave a 2 px margin so the border is not clipped
  const border = style.borderWidth + 0.5;
  const inShape = (x: number, y: number, glyph: PointGlyph, radius: number): boolean => {
    const dx = x - c;
    const dy = y - c;
    switch (glyph) {
      case "disc":
      case "ring":
        return Math.hypot(dx, dy) <= radius;
      case "diamond":
        return Math.abs(dx) + Math.abs(dy) <= radius;
    }
  };
  for (let y = 0; y < MARKER; y += 1) {
    for (let x = 0; x < MARKER; x += 1) {
      const filled = inShape(x, y, style.glyph, outer);
      if (!filled) {
        continue;
      }
      const inner = inShape(x, y, style.glyph, outer - border);
      const onBorder = !inner || (style.glyph === "ring" && inShape(x, y, "ring", outer - border - 4));
      if (style.glyph === "ring") {
        // Hollow ring: draw only the annulus (hue) + the outer border; leave the centre transparent.
        if (!inner) {
          put(data, MARKER, MARKER, x, y, br, bg, bb, 255);
        } else if (!inShape(x, y, "ring", outer - border - 4)) {
          put(data, MARKER, MARKER, x, y, r, g, b, 255);
        }
        continue;
      }
      if (onBorder) {
        put(data, MARKER, MARKER, x, y, br, bg, bb, 255);
      } else {
        put(data, MARKER, MARKER, x, y, r, g, b, 255);
      }
    }
  }
  return { width: MARKER, height: MARKER, data };
}
