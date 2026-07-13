// objectStyling.spec.ts — the D-17/D-18/D-19 scene-object styling offline UAT + the Terra Draw draw-time
// regression guard (Pitfall 8). Drives the REAL vite-served bundle in headless Chromium, fully offline
// (`bootOffline`: basemap + `/api/*` route-mocked, the offline guard asserts zero external egress). Nothing
// is reimplemented — the map registers the REAL objectStyles-driven fill/line/symbol layers + the REAL
// runtime hatch/marker images, observed through the DEV `objectLayerTelemetry` bridge and the DEV
// `window.__enviMap` probe (D-01).
//
// Asserts the D-17/D-18/D-19 observables:
//  - REAL RENDERED COLOUR: each of the 9 kinds renders in its OWN designed colour. This is asserted three
//    independent ways, because an earlier version of this spec asserted only the style SPEC (layer ids,
//    image ids, draw order) and so passed green while the running app painted every object in Terra Draw's
//    single stock blue (`#3f97e0`) — the TD adapter layers append ABOVE the display layers and, at their
//    defaults, paint points, lines AND polygon fills in that one hex. A spec-only test cannot see that.
//      (a) the feature MapLibre actually renders through the display layer carries `properties.kind`
//          (so the `["match", ["get","kind"], …]` paint expression cannot be silently falling through to
//          its default — the "everything is the fallback colour" failure mode);
//      (b) the layer's REAL paint expression, pulled from the live map and evaluated against that REAL
//          rendered feature, resolves to the kind's designed hex — and the 9 resolutions are not one value;
//      (c) the ACTUAL CANVAS PIXELS at each object: the strongest-chroma pixel on the object must match
//          its OWN palette entry more closely than any other kind's (a foreign layer painting over the
//          object — exactly the bug — moves this to the wrong kind);
//      (d) paint ownership: Terra Draw contributes ZERO opacity to every committed object (the display
//          layers own its pixels), read back from the baked `td-*` source-feature style properties.
//  - the 4 area hatch images + the 3 point marker images are registered (map.addImage);
//  - the object layers sit ABOVE the isophone fill in the style draw order (D-18 — objects over the noise);
//  - CRUCIAL regression guard: a Phase-7 draw-time journey (ground_zone containment / partial-cross reject /
//    last-object inheritance) behaves IDENTICALLY — the display restyle did not touch draw-time behaviour.

import { expect, test } from "@playwright/test";

import { bootOffline } from "./_mocks";

const AREA_KINDS = ["building", "forest", "ground_zone", "calc_area"] as const;
const LINE_KINDS = ["wall", "elevation_line"] as const;
const POINT_KINDS = ["source", "receiver", "elevation_point"] as const;

type Kind = (typeof AREA_KINDS | typeof LINE_KINDS | typeof POINT_KINDS)[number];

const ALL_KINDS = [...POINT_KINDS, ...LINE_KINDS, ...AREA_KINDS] as const;

const HATCH_IMAGES = AREA_KINDS.map((k) => `envi-hatch-${k}`);
const MARKER_IMAGES = POINT_KINDS.map((k) => `envi-marker-${k}`);

const OBJECT_LAYERS = [
  "envi-object-area-fill",
  "envi-object-area-hatch",
  "envi-object-area-border",
  "envi-object-area-border-dashed",
  "envi-object-line",
  "envi-object-line-dashed",
  "envi-object-point",
];

// The DESIGNED per-kind palette (11-UI-SPEC §Palettes #3 / `src/map/objectStyles.ts`). Deliberately
// mirrored here: the spec PINS the contract, so an unannounced palette edit fails this test.
const PALETTE: Record<Kind, string> = {
  source: "#3987e5",
  receiver: "#199e70",
  elevation_point: "#c98500",
  wall: "#e66767",
  elevation_line: "#c98500",
  building: "#9085e9",
  forest: "#008300",
  ground_zone: "#d95926",
  calc_area: "#d55181",
};

// The display layer that OWNS each kind's colour, and the paint/layout property that carries it. Points
// carry their hue inside the generated marker raster, so their per-kind discriminator is the icon image id.
const COLOR_SOURCE: Record<Kind, { layer: string; property: string; layout?: true }> = {
  source: { layer: "envi-object-point", property: "icon-image", layout: true },
  receiver: { layer: "envi-object-point", property: "icon-image", layout: true },
  elevation_point: { layer: "envi-object-point", property: "icon-image", layout: true },
  wall: { layer: "envi-object-line", property: "line-color" },
  elevation_line: { layer: "envi-object-line-dashed", property: "line-color" },
  building: { layer: "envi-object-area-fill", property: "fill-color" },
  forest: { layer: "envi-object-area-fill", property: "fill-color" },
  ground_zone: { layer: "envi-object-area-fill", property: "fill-color" },
  calc_area: { layer: "envi-object-area-fill", property: "fill-color" },
};

// The stub basemap background (`_mocks.STUB_DARK_STYLE`) every object pixel is composited over.
const BACKGROUND: [number, number, number] = [0x0b, 0x0d, 0x10];

// `testBridge.geometryFor`'s offset — a point sits AT the anchor, a line runs anchor → anchor+d, a polygon
// is the triangle (anchor, +d east, +d north-east). The probe aims at the CENTRE of each so the pixel box
// lands on the object, not on its corner.
const D = 0.0005;

function anchorOf(i: number): [number, number] {
  return [4.9 + i * 0.002, 52.36 + i * 0.002];
}

function probePointOf(kind: Kind, i: number): [number, number] {
  const [lng, lat] = anchorOf(i);
  if ((POINT_KINDS as readonly string[]).includes(kind)) {
    return [lng, lat];
  }
  if ((LINE_KINDS as readonly string[]).includes(kind)) {
    return [lng + D / 2, lat + D / 2]; // the line's midpoint
  }
  return [lng + (2 * D) / 3, lat + D / 3]; // the triangle's centroid
}

function hexToRgb(hex: string): [number, number, number] {
  return [
    parseInt(hex.slice(1, 3), 16),
    parseInt(hex.slice(3, 5), 16),
    parseInt(hex.slice(5, 7), 16),
  ];
}

// The DIRECTION of a colour once the near-black basemap is subtracted. Alpha-compositing a hue over an
// (almost) black backdrop scales it toward the background but PRESERVES this direction, so it is the
// robust way to ask "which palette entry is actually painted here?" across opaque glyphs/lines and
// translucent fills + hatch alike.
function chromaDirection(rgb: [number, number, number]): [number, number, number] | null {
  const v: [number, number, number] = [
    rgb[0] - BACKGROUND[0],
    rgb[1] - BACKGROUND[1],
    rgb[2] - BACKGROUND[2],
  ];
  const len = Math.hypot(v[0], v[1], v[2]);
  if (len < 24) {
    return null; // indistinguishable from the backdrop
  }
  return [v[0] / len, v[1] / len, v[2] / len];
}

// Which palette entry a sampled pixel is closest to (by chroma direction). Returns every kind sharing the
// winning hex — `elevation_point` and `elevation_line` deliberately share `#c98500` (unique WITHIN a
// geometry family, reused across families), so a tie between those two is expected, not a failure.
function nearestPaletteKinds(rgb: [number, number, number]): Kind[] {
  const dir = chromaDirection(rgb);
  if (!dir) {
    return [];
  }
  let best = "";
  let bestDist = Infinity;
  for (const kind of ALL_KINDS) {
    const pd = chromaDirection(hexToRgb(PALETTE[kind]));
    if (!pd) {
      continue;
    }
    const dist = Math.hypot(dir[0] - pd[0], dir[1] - pd[1], dir[2] - pd[2]);
    if (dist < bestDist) {
      bestDist = dist;
      best = PALETTE[kind];
    }
  }
  return ALL_KINDS.filter((k) => PALETTE[k] === best);
}

// Evaluate the `["match", ["get", <key>], k1, v1, …, fallback]` expression MapLibre is really painting
// with, against the properties of the feature MapLibre really rendered. A literal (non-expression) paint
// value passes straight through. Anything else is unsupported here on purpose: the display layers are
// specified to be `match`-on-kind, and a silent shape change should fail loudly.
function resolveExpression(expr: unknown, props: Record<string, unknown>): unknown {
  if (typeof expr === "string" || typeof expr === "number") {
    return expr;
  }
  if (!Array.isArray(expr) || expr[0] !== "match") {
    throw new Error(`unsupported paint expression: ${JSON.stringify(expr)}`);
  }
  const input = expr[1];
  if (!Array.isArray(input) || input[0] !== "get" || typeof input[1] !== "string") {
    throw new Error(`match input is not a ["get", key]: ${JSON.stringify(input)}`);
  }
  const value = props[input[1]];
  const arms = expr.slice(2, -1);
  const fallback = expr[expr.length - 1];
  for (let i = 0; i + 1 < arms.length; i += 2) {
    if (arms[i] === value) {
      return arms[i + 1];
    }
  }
  return fallback;
}

test("scene objects render at full styling (color + hatch/symbol) ON TOP of the isophone fill (D-17/D-18/D-19)", async ({
  page,
}) => {
  const unmocked = await bootOffline(page);

  // 1) Seed the isophone FILL first and wait for it to trace — so every subsequent object-layer apply
  //    recomputes the draw-order telemetry with the isophone present (deterministic above-fill assertion).
  await page.evaluate(() => window.__enviTest.seedIsophone());
  await expect
    .poll(async () => (await page.evaluate(() => window.__enviTest.isophoneTelemetry())).layerType, {
      timeout: 8000,
    })
    .toBe("fill");

  // 2) Place one feature of EACH of the 9 kinds through the same store path a finished draw commits.
  await page.evaluate((kinds) => {
    kinds.forEach((k, i) =>
      window.__enviTest.commit(k as never, 4.9 + i * 0.002, 52.36 + i * 0.002),
    );
  }, [...ALL_KINDS]);

  // 3) The object DISPLAY layers registered — fill/line/symbol layers + hatch/marker images all present.
  //    (Poll generously: under multi-worker software-WebGL contention the map effect can take a beat to
  //    settle after the batched commits.)
  await expect
    .poll(async () => (await page.evaluate(() => window.__enviTest.objectLayerTelemetry())).featureCount, {
      timeout: 15000,
    })
    .toBe(9);
  const tel = await page.evaluate(() => window.__enviTest.objectLayerTelemetry());

  // Every declared object layer exists on the style.
  for (const layer of OBJECT_LAYERS) {
    expect(tel.registeredLayers, `object layer ${layer} missing`).toContain(layer);
  }
  // The area hatch (arcering) images + the point marker glyph images are all registered (map.addImage).
  for (const img of [...HATCH_IMAGES, ...MARKER_IMAGES]) {
    expect(tel.registeredImages, `object image ${img} not registered`).toContain(img);
  }
  // The dedicated hatch fill layer exists → area kinds carry a fill-pattern (the load-bearing secondary
  // channel that lets a semi-transparent building/zone/forest read over the coloured noise fill).
  expect(tel.registeredLayers).toContain("envi-object-area-hatch");

  // 4) D-18 draw order: EVERY object layer sits ABOVE the isophone fill in the style layer order.
  expect(tel.aboveIsophone, "object layers must sit above the isophone fill (D-18)").toBe(true);
  const isoIdx = tel.layerOrder.indexOf("envi-isophone-fill");
  expect(isoIdx).toBeGreaterThanOrEqual(0);
  for (const layer of tel.registeredLayers) {
    expect(tel.layerOrder.indexOf(layer), `${layer} is not above the isophone fill`).toBeGreaterThan(isoIdx);
  }
  // The area FILL layer is drawn below the area HATCH (translucent colour under the arcering texture).
  expect(tel.layerOrder.indexOf("envi-object-area-fill")).toBeLessThan(
    tel.layerOrder.indexOf("envi-object-area-hatch"),
  );

  // Sanity: the kind lists this spec asserts on match the frozen 9 kinds (no silent drift).
  expect([...AREA_KINDS, ...LINE_KINDS, ...POINT_KINDS].sort()).toEqual(
    [
      "building",
      "calc_area",
      "elevation_line",
      "elevation_point",
      "forest",
      "ground_zone",
      "receiver",
      "source",
      "wall",
    ].sort(),
  );

  // Offline invariant: the whole styling path touched NO external network.
  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});

test("EACH of the 9 kinds RENDERS its own designed colour — not one shared fallback, not Terra Draw's stock blue (D-17)", async ({
  page,
}) => {
  const unmocked = await bootOffline(page);

  await page.evaluate((kinds) => {
    kinds.forEach((k, i) =>
      window.__enviTest.commit(k as never, 4.9 + i * 0.002, 52.36 + i * 0.002),
    );
  }, [...ALL_KINDS]);
  await expect
    .poll(async () => (await page.evaluate(() => window.__enviTest.objectLayerTelemetry())).featureCount, {
      timeout: 15000,
    })
    .toBe(9);

  const resolved: Record<string, unknown> = {};
  const sampled: Record<string, [number, number, number]> = {};

  for (let i = 0; i < ALL_KINDS.length; i += 1) {
    const kind = ALL_KINDS[i];
    const source = COLOR_SOURCE[kind];
    const centre = probePointOf(kind, i);

    // Fly to the object and let the map settle so the geojson tiles are parsed AND the frame is painted.
    await page.evaluate((c) => {
      window.__enviMap?.jumpTo({ center: c as [number, number], zoom: 18 });
    }, centre);
    await page.waitForFunction(() => window.__enviMap?.loaded() === true, undefined, { timeout: 15000 });
    await page.evaluate(
      () =>
        new Promise<void>((resolve) => {
          window.__enviMap?.once("idle", () => resolve());
          window.__enviMap?.triggerRepaint();
        }),
    );

    // --- (a) + (b) the REAL rendered feature and the REAL paint expression --------------------------
    const probe = await page.evaluate(
      ({ c, layer, property, layout }) => {
        const map = window.__enviMap;
        if (!map) {
          throw new Error("__enviMap probe missing (DEV bundle expected)");
        }
        const pt = map.project(c as [number, number]);
        const hits = map.queryRenderedFeatures(pt, { layers: [layer] });
        const expr = layout
          ? map.getLayoutProperty(layer, property)
          : map.getPaintProperty(layer, property);
        return {
          properties: (hits[0]?.properties ?? null) as Record<string, unknown> | null,
          hitLayers: hits.map((h) => h.layer.id),
          expr,
        };
      },
      { c: centre, layer: source.layer, property: source.property, layout: source.layout ?? false },
    );

    // The display layer really painted this object…
    expect(probe.hitLayers, `${kind}: not rendered by ${source.layer}`).toContain(source.layer);
    // …and the RENDERED feature carries the very property the paint expression reads. If `kind` were
    // missing, misspelled, or nested, every feature would fall through to the expression's default and
    // every object would come out one colour — this assertion makes that impossible to ship green.
    expect(probe.properties, `${kind}: no rendered feature under ${source.layer}`).not.toBeNull();
    expect(probe.properties?.["kind"], `${kind}: rendered feature has no properties.kind`).toBe(kind);

    // Evaluate the live paint expression against the live rendered feature.
    const value = resolveExpression(probe.expr, probe.properties ?? {});
    resolved[kind] = value;
    const expected = source.layout ? `envi-marker-${kind}` : PALETTE[kind];
    expect(value, `${kind}: ${source.layer}.${source.property} did not resolve to its designed value`).toBe(
      expected,
    );

    // --- (c) the ACTUAL PIXELS ---------------------------------------------------------------------
    const pixel = await page.evaluate((c) => {
      const map = window.__enviMap;
      if (!map) {
        throw new Error("__enviMap probe missing");
      }
      const canvas = map.getCanvas();
      const off = document.createElement("canvas");
      off.width = canvas.width;
      off.height = canvas.height;
      const ctx = off.getContext("2d");
      if (!ctx) {
        throw new Error("no 2d context");
      }
      ctx.drawImage(canvas, 0, 0);
      const dpr = canvas.width / canvas.clientWidth;
      const p = map.project(c as [number, number]);
      const half = 8;
      const x = Math.round(p.x * dpr) - half;
      const y = Math.round(p.y * dpr) - half;
      const img = ctx.getImageData(x, y, 2 * half + 1, 2 * half + 1);
      // The strongest-chroma pixel in the box: the object's own hue where it is most opaque (an opaque
      // glyph/line, or a near-opaque hatch stroke over a translucent fill).
      const bg = [0x0b, 0x0d, 0x10];
      let best: [number, number, number] = [bg[0], bg[1], bg[2]];
      let bestLen = -1;
      for (let k = 0; k < img.data.length; k += 4) {
        if (img.data[k + 3] === 0) {
          continue;
        }
        const rgb: [number, number, number] = [img.data[k], img.data[k + 1], img.data[k + 2]];
        const len = Math.hypot(rgb[0] - bg[0], rgb[1] - bg[1], rgb[2] - bg[2]);
        if (len > bestLen) {
          bestLen = len;
          best = rgb;
        }
      }
      return best;
    }, centre);
    sampled[kind] = pixel;

    const matched = nearestPaletteKinds(pixel);
    expect(
      matched,
      `${kind}: the pixels actually painted (rgb ${pixel.join(",")}) look like ${
        matched.length ? matched.join("/") : "the empty basemap"
      }, not ${kind} (${PALETTE[kind]}) — something is painting over the display layer`,
    ).toContain(kind);
  }

  // --- (b, continued) the 9 resolutions are NOT one value ------------------------------------------
  // 8 distinct colours over 9 kinds: elevation_point / elevation_line deliberately share `#c98500`
  // (unique WITHIN a geometry family, reused across families — they differ by glyph vs dash).
  const areaLineColors = [...AREA_KINDS, ...LINE_KINDS].map((k) => resolved[k]);
  expect(new Set(areaLineColors).size, "area/line kinds must not collapse onto one colour").toBe(6);
  expect(new Set(POINT_KINDS.map((k) => resolved[k])).size, "point kinds must not share one icon").toBe(3);

  // --- (d) PAINT OWNERSHIP: Terra Draw contributes ZERO opacity to a committed object ---------------
  // TD's adapter renders the SAME store features through `td-*` layers appended ABOVE the display layers,
  // and bakes its resolved style values into the feature properties. Left at its stock defaults it painted
  // points, lines AND polygon fills in ONE hex (#3f97e0), covering the whole palette. The display layers
  // own committed pixels; TD only paints the shape being drawn and the shape selected for editing.
  const tdPaint = await page.evaluate(() => {
    const map = window.__enviMap;
    if (!map) {
      throw new Error("__enviMap probe missing");
    }
    const grab = (src: string, keys: string[]): Record<string, unknown>[] =>
      map.querySourceFeatures(src).map((f) => {
        const out: Record<string, unknown> = {};
        for (const key of keys) {
          out[key] = f.properties?.[key];
        }
        return out;
      });
    return {
      polygon: grab("td-polygon", ["polygonFillOpacity", "polygonOutlineOpacity"]),
      linestring: grab("td-linestring", ["lineStringOpacity"]),
      point: grab("td-point", ["pointOpacity", "pointOutlineOpacity"]),
    };
  });
  const tdFeatures = [...tdPaint.polygon, ...tdPaint.linestring, ...tdPaint.point];
  expect(tdFeatures.length, "Terra Draw should be rendering the committed features").toBeGreaterThan(0);
  for (const f of tdFeatures) {
    for (const [key, opacity] of Object.entries(f)) {
      expect(
        opacity,
        `Terra Draw paints a committed object (${key}=${String(opacity)}) — its single stock colour ` +
          `covers the per-kind display palette`,
      ).toBe(0);
    }
  }

  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});

test("REGRESSION GUARD: Terra Draw draw-time behaviour is unchanged by the display restyle (Pitfall 8)", async ({
  page,
}) => {
  const unmocked = await bootOffline(page);

  // (a) ground_zone TOPOLOGY (D-07 draw-time validation) — identical to the Phase-7 journey.
  // Zone A — the reference square, commits.
  const a = await page.evaluate(() =>
    window.__enviTest.commitGroundZone([
      [5.0, 52.0],
      [5.02, 52.0],
      [5.02, 52.02],
      [5.0, 52.02],
      [5.0, 52.0],
    ]),
  );
  expect(a.outcome).toBe("ok");
  expect(a.id).not.toBeNull();

  // Zone B — fully inside A → containment ALLOWED (innermost wins), commits, no reject banner.
  const b = await page.evaluate(() =>
    window.__enviTest.commitGroundZone([
      [5.005, 52.005],
      [5.012, 52.005],
      [5.012, 52.012],
      [5.005, 52.012],
      [5.005, 52.005],
    ]),
  );
  expect(b.outcome).toBe("contained");
  expect(b.id).not.toBeNull();
  await expect(page.getByTestId("reject-banner")).toHaveCount(0);

  // Zone C — partially crosses A → HARD REJECT: reverted (never committed), banner + zoom-to-existing.
  const c = await page.evaluate(() =>
    window.__enviTest.commitGroundZone([
      [5.016, 52.016],
      [5.03, 52.016],
      [5.03, 52.03],
      [5.016, 52.03],
      [5.016, 52.016],
    ]),
  );
  expect(c.outcome).toBe("partial-cross");
  expect(c.id).toBeNull();
  expect(c.conflictId).toBe(a.id);
  await expect(page.getByTestId("reject-banner")).toBeVisible();

  // (b) LAST-OBJECT INHERITANCE (WEB-04) — a second ground_zone inherits the first's edited impedance and
  // shows the inherited chip until the field is edited. Unchanged draw-time behaviour.
  await page.getByTestId("tool-ground_zone").click();
  await page.evaluate(() => window.__enviTest.commit("ground_zone", 4.95, 52.4));
  await page.getByTestId("ground-impedance").selectOption("C");
  await expect(page.getByTestId("ground-impedance")).toHaveValue("C");
  await page.evaluate(() => window.__enviTest.commit("ground_zone", 4.96, 52.41));
  await expect(page.getByTestId("ground-impedance")).toHaveValue("C");
  await expect(page.getByTestId("inspector")).toContainText("inherited from last ground_zone");

  // Offline invariant.
  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});
