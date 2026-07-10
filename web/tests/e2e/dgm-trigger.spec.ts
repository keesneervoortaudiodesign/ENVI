// dgm-trigger.spec.ts — the SC1 DGM re-triangulation PRODUCER proof, fully offline (D-13a).
//
// # Module I/O
// - Input  the REAL bundle with the basemap + /api/* surface intercepted, plus a captured
//   `POST /dgm/triangulate` route. Elevation objects are committed through the DEV bridge.
// - Output assertions that: committing ≥3 non-collinear elevation points fires EXACTLY ONE debounced
//   triangulate request (drag frames coalesced) whose success mesh renders (the TIN triangle-count
//   readout becomes non-zero); and a 4xx reject is written into the dgm slice (the reject readout shows
//   the status — the 07-09 crit source), never a silent swallow. No request escapes to the live network.

import { expect, test, type Page } from "@playwright/test";

import { installOffline } from "./_mocks";

const MESH = {
  vertices: [
    [4.90, 52.36, 0],
    [4.91, 52.36, 1],
    [4.905, 52.37, 2],
  ],
  triangles: [[0, 1, 2]],
};

async function ready(page: Page): Promise<void> {
  await page.goto("/");
  await expect(page.getByTestId("object-palette")).toBeVisible();
  await page.waitForFunction(() => typeof window.__enviTest !== "undefined");
}

test("≥3 non-collinear elevation points fire one debounced triangulate → TIN renders", async ({ page }) => {
  const unmocked = await installOffline(page);
  let calls = 0;
  await page.route(/\/api\/v1\/dgm\/triangulate$/, async (route) => {
    calls += 1;
    await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(MESH) });
  });

  await ready(page);

  // Commit three non-collinear elevation points in quick succession (simulates committed edits).
  await page.evaluate(() => {
    window.__enviTest.commit("elevation_point", 4.90, 52.36);
    window.__enviTest.commit("elevation_point", 4.91, 52.36);
    window.__enviTest.commit("elevation_point", 4.905, 52.37);
  });

  // The debounced producer fires once and the TIN renders (triangle-count readout > 0).
  await expect(page.getByTestId("dgm-triangle-count")).toHaveText("1");
  // Exactly one request despite three rapid commits (debounce coalesced them — not one per edit).
  expect(calls).toBe(1);
  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});

test("a 4xx triangulate reject is stored (crit source), not silently swallowed", async ({ page }) => {
  const unmocked = await installOffline(page);
  await page.route(/\/api\/v1\/dgm\/triangulate$/, async (route) => {
    await route.fulfill({
      status: 400,
      contentType: "application/json",
      body: JSON.stringify({ detail: "breaklines interior-cross" }),
    });
  });

  await ready(page);

  await page.evaluate(() => {
    window.__enviTest.commit("elevation_point", 4.90, 52.36);
    window.__enviTest.commit("elevation_point", 4.91, 52.36);
    window.__enviTest.commit("elevation_point", 4.905, 52.37);
  });

  // The reject status lands in the dgm slice (the 07-09 ValidationPanel reads it) — not swallowed.
  await expect(page.getByTestId("dgm-reject")).toHaveText("400");
  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});
