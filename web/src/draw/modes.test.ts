// modes.test.ts — the Terra Draw PAINT-OWNERSHIP contract (D-17/D-18/D-19).
//
// Terra Draw's MapLibre adapter renders the same features the canonical store holds, through its own
// `td-*` layers appended ABOVE the `map/objectStyles` display layers. At its stock defaults every TD mode
// paints in the SAME hex (`#3f97e0` for points AND lines AND polygon fills), so a committed scene object
// was covered by one identical blue and the per-kind palette underneath was invisible.
//
// The rule these tests pin: Terra Draw paints ONLY what it owns — the shape being drawn and the shape
// selected for editing. A COMMITTED object is painted at ZERO opacity, so the display layers own its
// pixels. A regression here reintroduces "every object is the same colour".

import { describe, expect, it } from "vitest";
import type { GeoJSONStoreFeatures } from "terra-draw";

import { buildModes, tdModeName } from "./modes";
import { KINDS, KIND_META } from "./kinds";

// A Terra Draw store feature in `mode`, carrying the extra flags TD sets on its own features.
function tdFeature(
  mode: "point" | "linestring" | "polygon",
  flags: Record<string, unknown> = {},
): GeoJSONStoreFeatures {
  const geometry =
    mode === "point"
      ? { type: "Point", coordinates: [4.9, 52.36] }
      : mode === "linestring"
        ? { type: "LineString", coordinates: [[4.9, 52.36], [4.91, 52.37]] }
        : {
            type: "Polygon",
            coordinates: [[[4.9, 52.36], [4.91, 52.36], [4.91, 52.37], [4.9, 52.36]]],
          };
  return {
    id: "00000000-0000-4000-8000-000000000000",
    type: "Feature",
    geometry,
    // `kind` is present exactly as the canonical store tags it — a committed object is NOT identified by
    // the absence of a kind (TD tags in-progress shapes too), but by the absence of TD's own draw flags.
    properties: { mode, kind: "building", ...flags },
  } as unknown as GeoJSONStoreFeatures;
}

// The concrete modes by TD mode name (`buildModes()` returns [select, point, linestring, polygon]).
function modesByName() {
  const [, point, linestring, polygon] = buildModes();
  return { point, linestring, polygon };
}

describe("buildModes — Terra Draw paint ownership (D-17/D-18)", () => {
  it("paints a COMMITTED object at zero opacity — the display layers own its pixels", () => {
    const { point, linestring, polygon } = modesByName();

    const committedPoint = point.styleFeature(tdFeature("point"));
    expect(committedPoint.pointOpacity).toBe(0);
    expect(committedPoint.pointOutlineOpacity).toBe(0);

    const committedLine = linestring.styleFeature(tdFeature("linestring"));
    expect(committedLine.lineStringOpacity).toBe(0);

    const committedArea = polygon.styleFeature(tdFeature("polygon"));
    expect(committedArea.polygonFillOpacity).toBe(0);
    expect(committedArea.polygonOutlineOpacity).toBe(0);
  });

  it("still paints the shape being DRAWN (the rubber band) — draw-time feedback is not lost", () => {
    const { linestring, polygon } = modesByName();

    const drawingLine = linestring.styleFeature(tdFeature("linestring", { currentlyDrawing: true }));
    expect(drawingLine.lineStringOpacity).toBeGreaterThan(0);

    const drawingArea = polygon.styleFeature(tdFeature("polygon", { currentlyDrawing: true }));
    expect(drawingArea.polygonFillOpacity).toBeGreaterThan(0);
    expect(drawingArea.polygonOutlineOpacity).toBeGreaterThan(0);
  });

  it("still paints the SELECTED shape (the edit highlight)", () => {
    const { point, linestring, polygon } = modesByName();

    expect(point.styleFeature(tdFeature("point", { selected: true })).pointOpacity).toBeGreaterThan(0);
    expect(
      linestring.styleFeature(tdFeature("linestring", { selected: true })).lineStringOpacity,
    ).toBeGreaterThan(0);
    expect(
      polygon.styleFeature(tdFeature("polygon", { selected: true })).polygonFillOpacity,
    ).toBeGreaterThan(0);
  });

  it("maps every kind to its Terra Draw geometry mode (unchanged draw-time behaviour)", () => {
    expect(tdModeName("select")).toBe("select");
    for (const kind of KINDS) {
      expect(tdModeName(kind)).toBe(KIND_META[kind].mode);
    }
    // The four modes the constructor receives: select + one per geometry.
    expect(buildModes().map((m) => m.mode)).toEqual(["select", "point", "linestring", "polygon"]);
  });
});
