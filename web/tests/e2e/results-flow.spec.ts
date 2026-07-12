// results-flow.spec.ts — ONE offline session that walks the whole Phase-11 results
// workflow end-to-end against the REAL vite-served bundle + REAL compute WASM, with
// every /api/* route-mocked (zero network egress). Unlike the per-component specs,
// this is a single integration journey, and it is anchored on the PRODUCTION feed:
// `feedFromSolve` drives the real `applyResultsFeed(spec, fineEvent)` — the
// `applyTierComplete → setManifest` link the CalcPanel runs when a solve completes —
// so the spectrum panel is proven to light up from a solve-shaped feed, not a direct
// `setManifest`. It then touches the downstream surfaces (info-button help, isophone
// fill + live re-contour, scene-object styling) in the same session.
//
// Component coverage in one flow: production calc→results feed → spectrum readout
// (band-index display toggle + instant dB(A)/dB(C)) → universal info-button help →
// isophone fill layer + colour-scale re-contour (SC3) → object color/hatch styling
// above the fill (D-18). The dedicated specs (conditioning, scenarios, export) cover
// SC2/SC4/SC5 in depth and also run in the same suite.

import { expect, test } from "@playwright/test";

import { bootOffline, installMetaMocks } from "./_mocks";

test("full results workflow in one offline session, fed by the real production calc→results link", async ({
  page,
}) => {
  test.setTimeout(90_000);
  const unmocked = await bootOffline(page);
  await installMetaMocks(page);

  await expect(page.getByTestId("results-panel")).toBeVisible();

  // --- 1. PRODUCTION FEED → spectrum (SC1). Drive the REAL applyResultsFeed, not a
  // direct setManifest: seed the OPFS tensor and push a solve-shaped FINE TierComplete
  // through the same link the CalcPanel runs on solve completion. ---
  const ids: string[] = await page.evaluate(() => window.__enviTest.feedFromSolve(3));
  expect(ids).toHaveLength(3);

  // The manifest reached the store via the production feed → the empty prompt shows,
  // then selecting a receiver renders WASM-produced band levels (no reimplementation).
  await expect(page.getByTestId("spectrum-empty")).toBeVisible();
  await page.getByTestId(`spectrum-receiver-${ids[0]}`).click();
  const chart = page.getByTestId("spectrum-chart");
  await expect(chart).toBeVisible();
  await expect(chart).toHaveAttribute("data-band-count", "27"); // 1/3-oct by band index

  // Instant dB(A)⇄dB(C) toggle changes the total with NO recompute (both precomputed).
  const total = page.getByTestId("spectrum-total");
  const dba = (await total.textContent())?.trim();
  await page.getByTestId("spectrum-weighting-C").click();
  await expect(total).toHaveAttribute("data-weighting", "C");
  expect((await total.textContent())?.trim()).not.toEqual(dba);
  await expect(page.getByTestId("spectrum-loading")).toHaveCount(0);

  // 1/3 ⇄ 1/12-oct expert toggle (band-index aggregation).
  await page.getByTestId("spectrum-display-twelfth").click();
  await expect(chart).toHaveAttribute("data-band-count", "105");
  await page.getByTestId("spectrum-display-third").click();
  await expect(chart).toHaveAttribute("data-band-count", "27");

  // --- 2. Universal info-button help (D-23/25): a control's InfoButton opens its
  // glance popover, then the docked help panel, with cited content (never pasted). ---
  await page.getByTestId("info-spectrum.split").click();
  const popover = page.getByTestId("info-popover-spectrum.split");
  await expect(popover).toBeVisible();
  await expect(popover).toContainText("AV 1106/07");
  await page.getByTestId("info-more-spectrum.split").click();
  await expect(page.getByTestId("help-dock-spectrum.split")).toBeVisible();
  await page.getByTestId("help-dock-close-spectrum.split").click();

  // --- 3. Isophone noise map (SC3): a FILL layer (never a raster heatmap) traced by
  // the real WASM iso-band tracer, and a colour-scale break edit RE-CONTOURS the cached
  // grid with no re-solve (the trace count increments). ---
  await page.evaluate(() => window.__enviTest.seedIsophone());
  await expect
    .poll(async () => (await page.evaluate(() => window.__enviTest.isophoneTelemetry())).layerType, {
      timeout: 30_000,
    })
    .toBe("fill");
  await expect(page.getByTestId("isophone-legend")).toHaveAttribute("data-band-count", "6");

  await expect
    .poll(async () => (await page.evaluate(() => window.__enviTest.isophoneTelemetry())).traceCount)
    .toBeGreaterThan(0);
  const traceBefore = (await page.evaluate(() => window.__enviTest.isophoneTelemetry())).traceCount;
  await page.getByTestId("colorscale-break-2").fill("64");
  await page.getByTestId("colorscale-break-2").blur();
  await expect
    .poll(async () => (await page.evaluate(() => window.__enviTest.isophoneTelemetry())).traceCount)
    .toBeGreaterThan(traceBefore); // the edit re-contoured the SAME cached grid (SC3)

  // --- 4. Scene-object color + hatch styling (D-17/18/19): a drawn area object renders
  // with its per-kind fill + hatch ABOVE the isophone fill (D-18 draw order). ---
  await page.evaluate(() => window.__enviTest.commit("building", 4.9, 52.37));
  await expect
    .poll(async () => (await page.evaluate(() => window.__enviTest.objectLayerTelemetry())).featureCount)
    .toBeGreaterThan(0);
  const objTel = await page.evaluate(() => window.__enviTest.objectLayerTelemetry());
  expect(objTel.aboveIsophone, "object layers must sit above the isophone fill (D-18)").toBe(true);
  expect(objTel.registeredLayers).toContain("envi-object-area-hatch");

  // The entire journey — feed, spectrum, help, isophone, styling — touched zero network.
  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});
