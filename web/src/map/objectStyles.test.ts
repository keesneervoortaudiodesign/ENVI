// objectStyles.test.ts — the D-17/D-19 per-kind display-styling contract (unit). Asserts every one of the
// 9 frozen kinds has a display style with the GEOMETRY-APPROPRIATE secondary channel (points → symbol +
// border, lines → width, areas → fill% + a separate hatch id), matching the 11-UI-SPEC §Palettes #3 hex +
// hatch table, and that the runtime hatch/marker generators produce non-empty `ImageData`-shaped rasters.
// Pure logic — runs in the Node vitest env (the generators are canvas-free by design).

import { describe, expect, it } from "vitest";

import { KINDS, type Kind } from "../draw/kinds";
import { END_COLORS } from "../store/colorScale";
import { hexToRgb } from "./color";
import {
  hatchImageId,
  objectStyles,
  pointIconId,
  type AreaStyle,
  type LineStyle,
  type PointStyle,
} from "./objectStyles";
import { hatchPattern, pointMarker } from "./hatchPatterns";

// The isophone fill is painted at 0.5 opacity (`fillOverlay.upsertGeoJsonFillLayer`) over the dark surface,
// so THIS — not the raw class hex — is the backdrop a scene object must read against (D-18).
const SURFACE: [number, number, number] = [0x0b, 0x0d, 0x10];
const ISOPHONE_FILL_OPACITY = 0.5;

function compositeOverSurface(hex: string, alpha: number): [number, number, number] {
  const c = hexToRgb(hex);
  return [
    alpha * c[0] + (1 - alpha) * SURFACE[0],
    alpha * c[1] + (1 - alpha) * SURFACE[1],
    alpha * c[2] + (1 - alpha) * SURFACE[2],
  ];
}

function rgbDistance(a: [number, number, number], b: [number, number, number]): number {
  return Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
}

// The 11-UI-SPEC §Palettes #3 table (the dataviz-validated categorical set): the expected colour + the
// geometry family + (for areas) the expected hatch id. The test pins the display map to this contract.
const EXPECT: Record<Kind, { color: string; geometry: "point" | "line" | "area"; hatch?: string }> = {
  source: { color: "#3987e5", geometry: "point" },
  receiver: { color: "#199e70", geometry: "point" },
  elevation_point: { color: "#c98500", geometry: "point" },
  wall: { color: "#e66767", geometry: "line" },
  elevation_line: { color: "#c98500", geometry: "line" },
  building: { color: "#9085e9", geometry: "area", hatch: "solid-diagonal-45" },
  forest: { color: "#008300", geometry: "area", hatch: "dotted" },
  ground_zone: { color: "#d95926", geometry: "area", hatch: "cross-hatch" },
  calc_area: { color: "#d55181", geometry: "area", hatch: "sparse-135" },
};

describe("objectStyles — per-kind display styling (D-17/D-19)", () => {
  it("covers all 9 frozen kinds with the validated palette", () => {
    expect(Object.keys(objectStyles).sort()).toEqual([...KINDS].sort());
    for (const kind of KINDS) {
      expect(objectStyles[kind].color).toBe(EXPECT[kind].color);
      expect(objectStyles[kind].geometry).toBe(EXPECT[kind].geometry);
    }
  });

  it("gives each kind the geometry-appropriate secondary channel", () => {
    for (const kind of KINDS) {
      const style = objectStyles[kind];
      if (style.geometry === "point") {
        // Point = symbol glyph + size + border (§4.6.3 point model).
        expect(["disc", "ring", "diamond"]).toContain(style.glyph);
        expect(style.size).toBeGreaterThan(0);
        expect(style.borderWidth).toBeGreaterThan(0);
      } else if (style.geometry === "line") {
        // Line = width + colour (§4.6.3 line model).
        expect(style.width).toBeGreaterThan(0);
      } else {
        // Area = fill% + border + a SEPARATE hatch id (§4.6.3 area model).
        expect(style.fillOpacity).toBeGreaterThan(0);
        expect(style.fillOpacity).toBeLessThanOrEqual(1);
        expect(style.borderWidth).toBeGreaterThan(0);
        expect(style.hatch).toBe(EXPECT[kind].hatch);
      }
    }
  });

  it("assigns a unique colour WITHIN each geometry family (the kinds.ts reuse principle, extended)", () => {
    const byFamily: Record<string, string[]> = { point: [], line: [], area: [] };
    for (const kind of KINDS) {
      byFamily[objectStyles[kind].geometry].push(objectStyles[kind].color);
    }
    for (const family of Object.keys(byFamily)) {
      expect(new Set(byFamily[family]).size).toBe(byFamily[family].length);
    }
  });

  it("derives program-generated (never user-string) image ids", () => {
    expect(hatchImageId("building")).toBe("envi-hatch-building");
    expect(pointIconId("source")).toBe("envi-marker-source");
  });

  it("is 8 DISTINCT hues over the 9 kinds — the only repeat is the documented cross-family one", () => {
    const hues = KINDS.map((k) => objectStyles[k].color);
    // Colour may repeat ACROSS geometry families but never within one, so 9 kinds → 8 distinct hues.
    expect(new Set(hues).size).toBe(8);
    // The single repeat is elevation_point / elevation_line (`#c98500`) — a point and a line, so they are
    // never confusable: they differ by geometry AND by their secondary channel (diamond glyph vs dash).
    const repeated = [...new Set(hues)].filter((c) => hues.filter((h) => h === c).length > 1);
    expect(repeated).toEqual(["#c98500"]);
    expect(KINDS.filter((k) => objectStyles[k].color === "#c98500").sort()).toEqual([
      "elevation_line",
      "elevation_point",
    ]);
  });

  it("gives each kind a UNIQUE secondary channel within its family (the CVD relief rule, D-19)", () => {
    // The colours sit in the palette's CVD floor band, so the non-colour channel is what actually
    // separates the kinds for a colour-blind reader — and what keeps them legible over the isophone fill,
    // where hue contrast is the only strong signal (see the readability test below). It must be unique.
    const glyphs = KINDS.filter((k) => objectStyles[k].geometry === "point").map(
      (k) => (objectStyles[k] as PointStyle).glyph,
    );
    expect(new Set(glyphs).size).toBe(glyphs.length);

    const lines = KINDS.filter((k) => objectStyles[k].geometry === "line").map((k) => {
      const s = objectStyles[k] as LineStyle;
      return `${s.width}:${s.dash ? s.dash.join(",") : "solid"}`;
    });
    expect(new Set(lines).size).toBe(lines.length);

    const hatches = KINDS.filter((k) => objectStyles[k].geometry === "area").map(
      (k) => (objectStyles[k] as AreaStyle).hatch,
    );
    expect(new Set(hatches).size).toBe(hatches.length);
  });

  it("stays readable over the isophone fill — every kind is hue-separated from every EU-END class (D-18)", () => {
    // Objects render ON TOP of the isophone fill (D-18), so each object hue must stand apart from every
    // class colour as it is ACTUALLY composited (class hex at fill-opacity 0.5 over the dark surface).
    // The separation that carries here is HUE, not luminance: the EU-END ramp is a bright sequential
    // scale, so a mid-luminance categorical hue over it is low-contrast by WCAG-1.4.11 luminance maths
    // (forest over the 60–65 yellow class is ~1.0:1) while remaining plainly distinguishable in colour —
    // which is precisely why the unique secondary channel above (hatch / glyph / dash) is mandatory.
    const backdrops = END_COLORS.map((c) => compositeOverSurface(c, ISOPHONE_FILL_OPACITY));
    for (const kind of KINDS) {
      const objectColor = hexToRgb(objectStyles[kind].color);
      for (let i = 0; i < backdrops.length; i += 1) {
        const separation = rgbDistance(objectColor, backdrops[i]);
        expect(
          separation,
          `${kind} (${objectStyles[kind].color}) is not separated from isophone class ${i}`,
        ).toBeGreaterThan(60);
      }
    }
  });
});

describe("hatchPatterns — runtime raster generators (D-17)", () => {
  const areaKinds = KINDS.filter((k): k is Kind => objectStyles[k].geometry === "area");
  const pointKinds = KINDS.filter((k): k is Kind => objectStyles[k].geometry === "point");

  it("returns a non-empty ImageData-shaped hatch raster for every area kind", () => {
    for (const kind of areaKinds) {
      const img = hatchPattern(kind);
      expect(img.width).toBeGreaterThan(0);
      expect(img.height).toBeGreaterThan(0);
      expect(img.data.length).toBe(img.width * img.height * 4);
      // Some pixels are opaque strokes (alpha > 0) — the pattern is not a blank tile.
      const painted = Array.from(img.data).filter((_, i) => i % 4 === 3 && img.data[i] > 0);
      expect(painted.length).toBeGreaterThan(0);
      // …and some pixels stay transparent (a hatch, not a solid fill).
      const transparent = Array.from(img.data).filter((_, i) => i % 4 === 3 && img.data[i] === 0);
      expect(transparent.length).toBeGreaterThan(0);
    }
  });

  it("returns a non-empty marker raster for every point kind", () => {
    for (const kind of pointKinds) {
      const img = pointMarker(kind);
      expect(img.data.length).toBe(img.width * img.height * 4);
      const painted = Array.from(img.data).filter((_, i) => i % 4 === 3 && img.data[i] > 0);
      expect(painted.length).toBeGreaterThan(0);
    }
  });

  it("rejects a mismatched geometry (defence in depth)", () => {
    expect(() => hatchPattern("source")).toThrow();
    expect(() => pointMarker("building" as Kind)).toThrow();
    // Exhaustiveness: every area style really carries a hatch (compile + runtime).
    for (const kind of areaKinds) {
      expect((objectStyles[kind] as AreaStyle).hatch).toBeTruthy();
    }
  });
});
