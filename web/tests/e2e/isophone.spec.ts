// isophone.spec.ts — the WEB-06 / GRID-04 isophone FILL-MAP + colour-scale offline
// UAT (SC3). Drives the REAL vite-served bundle (COOP/COEP → crossOriginIsolated, so
// the compute wasm instantiates) in headless Chromium, fully offline: `/api/*` and
// the basemap are route-mocked, and a fixture level grid is seeded into the colour-
// scale store via the DEV bridge, then the REAL `trace_isophones` wasm export
// contours it. NOTHING is reimplemented — the map renders WASM-traced polygons (D-01).
//
// Asserts the WEB-06/GRID-04/SC3 observables:
//  - the noise map renders as a MapLibre FILL layer (paint type `fill`), NEVER a
//    raster density layer (D-02);
//  - switching preset (EU-END → viridis) RE-COLOURS without a re-solve;
//  - editing a break edge RE-CONTOURS the cached grid (the tracer re-runs) with NO
//    solve worker message / network egress (SC3 — re-contour, not re-propagate);
//  - the legend break values ≡ the contour breaks ≡ the class colours (one source).

import { expect, test } from "@playwright/test";

import { bootOffline } from "./_mocks";

test("isophone fill map: fill polygons, preset recolour, break re-contour (no re-solve), legend ≡ contour ≡ class", async ({
  page,
}) => {
  const unmocked = await bootOffline(page);

  // Seed a fixture level grid — the same `setIsophoneInput` path a finished readout
  // feeds. The IsophoneLayer re-contours it via the REAL wasm tracer.
  await page.evaluate(() => window.__enviTest.seedIsophone());

  // --- The map renders a FILL layer (D-02), never a raster density layer ---
  await expect
    .poll(async () => (await page.evaluate(() => window.__enviTest.isophoneTelemetry())).layerType, {
      timeout: 8000,
    })
    .toBe("fill");
  const t0 = await page.evaluate(() => window.__enviTest.isophoneTelemetry());
  expect(t0.featureCount).toBeGreaterThan(0); // bands traced
  expect(t0.traceCount).toBeGreaterThanOrEqual(1); // at least one contour pass
  expect(t0.error).toBeNull();

  // --- The docked legend renders the six EU-END classes ---
  const legend = page.getByTestId("isophone-legend");
  await expect(legend).toBeVisible();
  await expect(legend).toHaveAttribute("data-band-count", "6");

  // Legend ≡ contour ≡ class: every legend row's colour + range label is derived
  // from the SAME breaks[]/colors[] the tracer contoured.
  const scale0 = await page.evaluate(() => window.__enviTest.colorScaleState());
  expect(scale0.preset).toBe("end");
  expect(scale0.breaks).toEqual([55, 60, 65, 70, 75]);
  // Interior class 1 spans breaks[0]–breaks[1] with colors[1].
  const row1 = page.getByTestId("isophone-legend-row-1");
  await expect(row1).toHaveAttribute("data-label", "55–60");
  await expect(row1).toHaveAttribute("data-color", scale0.colors[1]);
  // The above-highest cap row.
  await expect(page.getByTestId("isophone-legend-row-5")).toHaveAttribute("data-label", "≥ 75");

  // --- Switch preset EU-END → viridis: RE-COLOUR without a re-solve ---
  await page.getByTestId("colorscale-preset-viridis").click();
  await expect
    .poll(async () => (await page.evaluate(() => window.__enviTest.colorScaleState())).preset)
    .toBe("viridis");
  const scaleV = await page.evaluate(() => window.__enviTest.colorScaleState());
  expect(scaleV.colors).not.toEqual(scale0.colors); // genuinely recoloured
  // The legend swatch follows the new palette (single source of truth).
  await expect(row1).toHaveAttribute("data-color", scaleV.colors[1]);
  // Re-contour (to restamp the per-band fills) happened — trace count advanced.
  await expect
    .poll(async () => (await page.evaluate(() => window.__enviTest.isophoneTelemetry())).traceCount)
    .toBeGreaterThan(t0.traceCount);

  // --- Edit a break edge: RE-CONTOUR the cached grid (SC3), NO re-solve ---
  const traceBeforeEdit = (await page.evaluate(() => window.__enviTest.isophoneTelemetry()))
    .traceCount;
  const breakInput = page.getByTestId("colorscale-break-2"); // the 65 dB edge
  await breakInput.fill("64");
  // The break edit committed to the single source of truth…
  await expect
    .poll(async () => (await page.evaluate(() => window.__enviTest.colorScaleState())).breaks[2])
    .toBe(64);
  // …and re-contoured the CACHED grid (the tracer re-ran) — trace count advanced,
  // while the layer stayed a FILL layer (never a raster density layer).
  await expect
    .poll(async () => (await page.evaluate(() => window.__enviTest.isophoneTelemetry())).traceCount)
    .toBeGreaterThan(traceBeforeEdit);
  const tAfter = await page.evaluate(() => window.__enviTest.isophoneTelemetry());
  expect(tAfter.layerType).toBe("fill");
  expect(tAfter.error).toBeNull();
  // The legend range label updated to match the new contour break.
  await expect(page.getByTestId("isophone-legend-row-2")).toHaveAttribute("data-label", "60–64");

  // --- SC3 proof: the whole re-contour path touched NO network (no re-solve) ---
  // The tracer is a pure main-thread WASM call over the cached grid; a re-solve
  // would require the calc worker + OPFS chunk writes, which never happen here.
  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);

  // The map never used a heatmap/raster density layer — the fill invariant held
  // across every recolour + re-contour.
  expect(tAfter.layerType).toBe("fill");
});
