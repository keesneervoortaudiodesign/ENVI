// objectStyling.spec.ts — the D-17/D-18/D-19 scene-object styling offline UAT + the Terra Draw draw-time
// regression guard (Pitfall 8). Drives the REAL vite-served bundle in headless Chromium, fully offline
// (`bootOffline`: basemap + `/api/*` route-mocked, the offline guard asserts zero external egress). Nothing
// is reimplemented — the map registers the REAL objectStyles-driven fill/line/symbol layers + the REAL
// runtime hatch/marker images, observed through the DEV `objectLayerTelemetry` bridge (D-01).
//
// Asserts the D-17/D-18/D-19 observables:
//  - every scene-object kind renders through the object DISPLAY layers (fill/line/symbol) with its per-kind
//    styling — the 4 area hatch images + the 3 point marker images are registered (map.addImage);
//  - area kinds carry a hatch `fill-pattern` layer (the arcering secondary channel);
//  - the object layers sit ABOVE the isophone fill in the style draw order (D-18 — objects over the noise);
//  - CRUCIAL regression guard: a Phase-7 draw-time journey (ground_zone containment / partial-cross reject /
//    last-object inheritance) behaves IDENTICALLY — the display restyle did not touch draw-time behaviour.

import { expect, test } from "@playwright/test";

import { bootOffline } from "./_mocks";

const AREA_KINDS = ["building", "forest", "ground_zone", "calc_area"] as const;
const LINE_KINDS = ["wall", "elevation_line"] as const;
const POINT_KINDS = ["source", "receiver", "elevation_point"] as const;

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
  await page.evaluate(() => {
    const kinds = [
      "source",
      "receiver",
      "elevation_point",
      "wall",
      "elevation_line",
      "building",
      "forest",
      "ground_zone",
      "calc_area",
    ] as const;
    kinds.forEach((k, i) => window.__enviTest.commit(k, 4.9 + i * 0.002, 52.36 + i * 0.002));
  });

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
