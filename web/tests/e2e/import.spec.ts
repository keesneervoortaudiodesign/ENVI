// import.spec.ts — the offline GIS import journey (SC1/SC3/SC5, DATA-01..03), fully offline (D-13a).
//
// # Module I/O
// - Input  the REAL Vite-served bundle with the whole network intercepted: the offline guard + basemap +
//   generic `/api/*` mocks (bootOffline), then `installGisMocks` serving the committed fixtures for every
//   GIS source (PDOK AHN, the `/api/v1/proxy/**` byte relay for GLO-30/WorldCover, Overpass) and the
//   `POST /dgm/triangulate` oracle. The import is driven through the DEV `window.__enviTest` bridge — the
//   SAME orchestrator (`runImport`) the ImportPanel button calls, never a test reimplementation.
// - Output assertions that a viewport import over the committed fixtures lands editable features
//   (elevation_point + ground_zone + building), the DGM re-triangulates (SC1), the SC5 attribution and the
//   SC3 impedance overlay render, a per-layer partial failure (Overpass 429) errors only buildings and
//   recovers on retry (D-07) — and that NOTHING escaped to the live network (the unmocked collector is []).

import { expect, test } from "@playwright/test";

import { bootOffline, installGisMocks, installTriangulateMock, importViewport } from "./_mocks";

// A stable project UUID (the OPFS cache is keyed per project; safeSeg sanitizes it defensively).
const PROJECT_ID = "11111111-1111-4111-8111-111111111111";
const { viewport } = importViewport();

test("viewport import lands editable features + DGM + attribution, fully offline", async ({ page }) => {
  const unmocked = await bootOffline(page);
  // GIS + triangulate mocks are registered AFTER boot so they win the most-recent-route match (a proxy
  // tile must return TIFF bytes, not the generic `{}` fallback).
  await installGisMocks(page);
  await installTriangulateMock(page);
  await expect(page.getByTestId("object-palette")).toBeVisible();

  // Open the target project and drive a viewport import through the real orchestrator.
  await page.evaluate(
    ({ id, bbox }) => {
      window.__enviTest.openProject(id, "E2E Import");
      window.__enviTest.runImport(bbox);
    },
    { id: PROJECT_ID, bbox: viewport },
  );

  // All three layers commit (first import also inits the WASM module — allow generous time).
  await expect(page.getByTestId("import-status-terrain")).toHaveText("done", { timeout: 20_000 });
  await expect(page.getByTestId("import-status-landcover")).toHaveText("done", { timeout: 20_000 });
  await expect(page.getByTestId("import-status-buildings")).toHaveText("done", { timeout: 20_000 });

  // Editable features of all three imported kinds land in the canonical scene store (DATA-01..03).
  const kinds = await page.evaluate(() => Object.values(window.__enviTest.state().kinds));
  expect(kinds).toContain("elevation_point");
  expect(kinds).toContain("ground_zone");
  expect(kinds).toContain("building");

  // SC1 — the imported elevation set re-triangulates through the debounced DGM producer.
  await expect(page.getByTestId("dgm-triangle-count")).toHaveText("1", { timeout: 15_000 });

  // SC5 — the map attribution credits each contributing source.
  const attribution = page.getByTestId("import-attribution");
  await expect(attribution).toContainText("AHN");
  await expect(attribution).toContainText("ESA WorldCover");
  await expect(attribution).toContainText("OpenStreetMap");

  // SC3 — the impedance debug overlay toggles on over the imported ground_zones.
  await page.getByTestId("import-debug-toggle").check();
  await expect(page.getByTestId("import-debug-toggle")).toBeChecked();
  const overlayOn = await page.evaluate(() => window.__enviTest.importState().debugOverlay);
  expect(overlayOn).toBe(true);

  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});

test("Overpass 429 errors only buildings + retry recovers; terrain/landcover still land (D-07)", async ({
  page,
}) => {
  const unmocked = await bootOffline(page);
  const gis = await installGisMocks(page);
  await installTriangulateMock(page);
  await expect(page.getByTestId("object-palette")).toBeVisible();

  // Overpass is rate-limited for this run; terrain (PDOK) + land cover (proxy) are healthy.
  gis.setOverpassMode("429");
  await page.evaluate(
    ({ id, bbox }) => {
      window.__enviTest.openProject(id, "E2E Import 429");
      window.__enviTest.runImport(bbox);
    },
    { id: PROJECT_ID, bbox: viewport },
  );

  // Independent layers (D-07): terrain + land cover commit while buildings errors after its 429 backoff.
  await expect(page.getByTestId("import-status-terrain")).toHaveText("done", { timeout: 20_000 });
  await expect(page.getByTestId("import-status-landcover")).toHaveText("done", { timeout: 20_000 });
  await expect(page.getByTestId("import-status-buildings")).toHaveText("error", { timeout: 20_000 });
  await expect(page.getByTestId("import-error-buildings")).toBeVisible();

  // Flip Overpass healthy and retry ONLY buildings — siblings are untouched (D-07 retry).
  gis.setOverpassMode("ok");
  await page.getByTestId("import-retry-buildings").click();
  await expect(page.getByTestId("import-status-buildings")).toHaveText("done", { timeout: 20_000 });
  await expect(page.getByTestId("import-status-terrain")).toHaveText("done");
  await expect(page.getByTestId("import-status-landcover")).toHaveText("done");

  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});
