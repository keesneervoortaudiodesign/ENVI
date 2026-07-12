// objectStyles.test.ts — the D-17/D-19 per-kind display-styling contract (unit). Asserts every one of the
// 9 frozen kinds has a display style with the GEOMETRY-APPROPRIATE secondary channel (points → symbol +
// border, lines → width, areas → fill% + a separate hatch id), matching the 11-UI-SPEC §Palettes #3 hex +
// hatch table, and that the runtime hatch/marker generators produce non-empty `ImageData`-shaped rasters.
// Pure logic — runs in the Node vitest env (the generators are canvas-free by design).

import { describe, expect, it } from "vitest";

import { KINDS, type Kind } from "../draw/kinds";
import { hatchImageId, objectStyles, pointIconId, type AreaStyle } from "./objectStyles";
import { hatchPattern, pointMarker } from "./hatchPatterns";

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
