// import-offline-replay.spec.ts — the DATA-04 network-off replay proof, fully offline (D-13a).
//
// # Module I/O
// - Input  the REAL bundle. Phase 1: a full viewport import (installGisMocks fixtures) POPULATES the
//   per-project OPFS tile cache. Phase 2: the network is INVERTED — every GIS route (PDOK / the
//   `/api/v1/proxy/**` byte relay / Overpass) is re-registered to RECORD + ABORT (network off) — and the
//   compute-read path is re-driven through the DEV `window.__enviTest` bridge.
// - Output the load-bearing DATA-04 assertions:
//   (a) Primary/honest property — ZERO GIS egress: re-running terrain + land cover with the network off
//       reads tile bytes back from OPFS (cache hit ⇒ no fetch), so the dedicated GIS-egress collector stays
//       empty and the boot offline-guard collector stays [] (localhost scene/wasm reads are NOT GIS and do
//       not weaken DATA-04).
//   (b) Real OPFS read + negative guard: evicting the cached terrain tile makes the SAME compute path miss
//       OPFS, attempt a (now-blocked) GIS fetch, and fail — proving the successful read came from OPFS, not
//       a lingering in-memory copy.
//   Ends with the offline-clean `expect(unmocked, ...).toEqual([])` assertion.

import { expect, test, type Route } from "@playwright/test";

import { bootOffline, installGisMocks, installTriangulateMock, importViewport } from "./_mocks";

const PROJECT_ID = "22222222-2222-4222-8222-222222222222";
const { viewport, kaartblad, worldcover_tile } = importViewport();

test("DATA-04: post-import compute reads terrain/land cover from OPFS with the network off", async ({
  page,
}) => {
  // --- Phase 1: an online import populates the OPFS tile cache. ---
  const unmocked = await bootOffline(page);
  await installGisMocks(page);
  await installTriangulateMock(page);
  await expect(page.getByTestId("object-palette")).toBeVisible();

  await page.evaluate(
    ({ id, bbox }) => {
      window.__enviTest.openProject(id, "E2E Replay");
      window.__enviTest.runImport(bbox);
    },
    { id: PROJECT_ID, bbox: viewport },
  );
  await expect(page.getByTestId("import-status-terrain")).toHaveText("done", { timeout: 20_000 });
  await expect(page.getByTestId("import-status-landcover")).toHaveText("done", { timeout: 20_000 });

  // The terrain + land-cover source tiles are now cached in OPFS (the read-back-from-cache seam, DATA-04).
  expect(await page.evaluate((t) => window.__enviTest.cachedTile("ahn4-dtm", t), kaartblad)).toBe(true);
  expect(
    await page.evaluate((t) => window.__enviTest.cachedTile("worldcover", t), worldcover_tile),
  ).toBe(true);

  // --- Phase 2: invert the network. Re-register every GIS route to RECORD + ABORT (network off). These
  // win over the fixture mocks (most-recent route), so any GIS fetch now both fails AND is recorded. ---
  const gisEgress: string[] = [];
  const offRoute = (route: Route): Promise<void> => {
    gisEgress.push(route.request().url());
    return route.abort();
  };
  await page.route(/service\.pdok\.nl\//, offRoute);
  await page.route(/\/api\/v1\/proxy\//, offRoute);
  await page.route(/overpass-api\.de\//, offRoute);

  // (a) Reset the in-memory scene (same project id ⇒ the OPFS tile cache persists) so the re-run merges
  // into a CLEAN scene — this isolates the DATA-04 read property from an unrelated re-merge path. Then
  // re-drive the compute read for the OPFS-cached layers only (Overpass is not an OPFS-cached raster, so
  // buildings is disabled — its network fetch would be an expected, unrelated miss). A cache hit reads
  // tile bytes back from OPFS and commits, bumping the scene load epoch (the honest "compute path ran").
  await page.evaluate((id) => {
    window.__enviTest.closeProject();
    window.__enviTest.openProject(id, "E2E Replay");
  }, PROJECT_ID);
  // The cache survived the scene reset (still an OPFS hit for the same project).
  expect(await page.evaluate((t) => window.__enviTest.cachedTile("ahn4-dtm", t), kaartblad)).toBe(true);

  const beforeEpoch = await page.evaluate(() => window.__enviTest.sceneEpoch());
  await page.evaluate((bbox) => {
    window.__enviTest.setImportLayerEnabled("buildings", false);
    window.__enviTest.runImport(bbox);
  }, viewport);
  await expect
    .poll(() => page.evaluate(() => window.__enviTest.sceneEpoch()), { timeout: 20_000 })
    .toBeGreaterThan(beforeEpoch);
  await expect(page.getByTestId("import-status-terrain")).toHaveText("done");
  await expect(page.getByTestId("import-status-landcover")).toHaveText("done");
  // The compute read actually landed features from OPFS (network off) — a real OPFS read, not vacuous.
  const terrainCount = await page.evaluate(() => window.__enviTest.importState().layers.terrain.featureCount);
  expect(terrainCount).toBeGreaterThan(0);
  // Primary DATA-04 property: the OPFS read path made ZERO requests to any GIS source or the byte proxy.
  expect(gisEgress, `GIS egress on the OPFS read path: ${gisEgress.join(", ")}`).toEqual([]);

  // (b) Negative guard: evict the cached terrain tile, then re-run terrain ONLY. With the cache gone and
  // the network off, the SAME compute path must miss OPFS, attempt a (blocked) GIS fetch, and fail —
  // proving the phase-2(a) success came from OPFS, not an in-memory copy.
  await page.evaluate((t) => window.__enviTest.evictTile("ahn4-dtm", t), kaartblad);
  expect(await page.evaluate((t) => window.__enviTest.cachedTile("ahn4-dtm", t), kaartblad)).toBe(false);
  await page.evaluate((bbox) => {
    window.__enviTest.setImportLayerEnabled("landcover", false);
    window.__enviTest.setImportLayerEnabled("buildings", false);
    window.__enviTest.runImport(bbox);
  }, viewport);
  await expect(page.getByTestId("import-status-terrain")).toHaveText("error", { timeout: 20_000 });
  expect(gisEgress.some((u) => u.includes("service.pdok.nl"))).toBe(true);

  // The offline guard never fired: no GIS/basemap request ever escaped to the live network.
  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});
