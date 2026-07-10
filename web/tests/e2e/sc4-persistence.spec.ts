// sc4-persistence.spec.ts — the SC4 persistence journey, fully offline (D-13a): the drawn scene survives a
// basemap switch (style.load rehydrate), a page reload (GET returns the same scene), AND a project
// close/reopen (reopen-last restores) — all via the store-canonical scene + the real open/reopen seams.
//
// # Module I/O
// - Input  the REAL Vite-served bundle with the basemap + /api/v1 surface route-intercepted
//   (`installOffline`), plus a captured-round-trip project fixture: `GET /projects/last` returns a fixed
//   meta, `PUT /projects/{id}/scene` CAPTURES the outbound FeatureCollection, and the subsequent
//   `GET /projects/{id}/scene` returns EXACTLY that captured payload (round-trip fidelity — "what was
//   persisted is what comes back"). Scene mutations run through the DEV `window.__enviTest` bridge.
// - Output assertions a broken implementation fails: after a basemap switch the store scene is intact AND
//   Terra Draw is re-hydrated from it (a lost-on-switch bug drops the TD count); after a reload the store
//   re-hydrates the captured scene by id (a no-GET app stays empty); after close/reopen the reopen-last
//   path restores the same feature ids. Nothing reaches the live network.

import { expect, test, type Page } from "@playwright/test";

import { installOffline } from "./_mocks";

const PROJECT_ID = "sc4-project-0001";
const PROJECT_NAME = "SC4 Persistence Project";

// A representative scene (a subset of kinds is enough to prove persistence — SC1 covers "every kind").
const SCENE_KINDS = ["source", "receiver", "wall", "ground_zone"] as const;

// Read a numeric DOM readout (the map-canvas store/TD/rehydration counters are rendered as text).
async function readCount(page: Page, testId: string): Promise<number> {
  const text = await page.getByTestId(testId).textContent();
  return Number((text ?? "").trim());
}

// Wait until a numeric readout is > 0 (the Terra Draw re-hydration re-populates its view asynchronously).
async function expectRepopulated(page: Page, testId: string): Promise<void> {
  await expect.poll(() => readCount(page, testId), { timeout: 5000 }).toBeGreaterThan(0);
}

// Install the offline stack + a captured-round-trip project fixture. Returns the unmocked collector plus a
// getter for how many PUTs have landed (so the test can wait for autosave to persist before a reload).
async function installProjectFixture(
  page: Page,
): Promise<{ unmocked: string[]; putCount: () => number }> {
  const unmocked = await installOffline(page);

  // The persisted scene starts empty and becomes whatever the last PUT captured (round-trip fidelity).
  let captured: unknown = { type: "FeatureCollection", features: [] };
  let puts = 0;

  // reopen-last → a fixed project meta (id + name is all the client needs; timestamps are display-only).
  await page.route(/\/api\/v1\/projects\/last$/, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        id: PROJECT_ID,
        name: PROJECT_NAME,
        description: "",
        created_at_unix: 1,
        modified_at_unix: 1,
      }),
    }),
  );

  // The scene endpoint: PUT captures the outbound FeatureCollection; GET returns exactly that capture.
  await page.route(new RegExp(`/api/v1/projects/${PROJECT_ID}/scene$`), (route) => {
    const method = route.request().method();
    if (method === "PUT") {
      captured = route.request().postDataJSON();
      puts += 1;
      return route.fulfill({ status: 200, contentType: "application/json", body: "{}" });
    }
    return route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(captured) });
  });

  return { unmocked, putCount: () => puts };
}

// Boot the app and wait for reopen-last to hydrate the fixture project (the project bar shows its name).
async function bootAndOpen(page: Page): Promise<void> {
  await page.goto("/");
  await expect(page.getByTestId("object-palette")).toBeVisible();
  await page.waitForFunction(() => typeof window.__enviTest !== "undefined");
  await expect(page.getByTestId("project-bar").getByText(PROJECT_NAME)).toBeVisible();
}

test("SC4: scene survives a basemap switch, a page reload, and a project close/reopen (offline)", async ({
  page,
}) => {
  const { unmocked, putCount } = await installProjectFixture(page);
  await bootAndOpen(page);

  // Draw a representative scene through the canonical store.
  const drawnIds = await page.evaluate((kinds) => {
    return kinds.map((kind, i) => window.__enviTest.commit(kind, 4.9 + i * 0.001, 52.36 + i * 0.001));
  }, SCENE_KINDS as unknown as string[]);
  const n = drawnIds.length;
  await expect(page.getByTestId("store-feature-count")).toHaveText(String(n));

  // (1) Basemap switch → style.load re-hydrates the scene FROM the canonical store (D-03: the store is the
  //     truth, Terra Draw a controlled view). The store scene is intact across the switch and the rehydrate
  //     hook re-populates Terra Draw from it (a lost-on-switch bug would clear the store; a missing
  //     style.load rebuild would leave the rehydration counter at 0 and Terra Draw empty). Two switches
  //     prove no echo-loop drift.
  await page.getByTestId("switch-basemap").click();
  await expect(page.getByTestId("rehydration-count")).toHaveText("1", { timeout: 5000 });
  await expect(page.getByTestId("store-feature-count")).toHaveText(String(n));
  await expectRepopulated(page, "td-feature-count"); // repopulated from the store
  await page.getByTestId("switch-basemap").click();
  await expect(page.getByTestId("rehydration-count")).toHaveText("2", { timeout: 5000 });
  await expect(page.getByTestId("store-feature-count")).toHaveText(String(n));
  await expectRepopulated(page, "td-feature-count");

  // Autosave persists the drawn scene (the PUT captures it) — wait for the indicator + a captured PUT.
  await expect(page.getByTestId("save-indicator")).toHaveAttribute("data-status", "saved", { timeout: 5000 });
  expect(putCount()).toBeGreaterThanOrEqual(1);

  // (2) Page reload → boot reopen-last GETs the SAME scene (captured payload) and the store re-hydrates
  //     identically. A no-GET implementation would come back to an empty store.
  await page.reload();
  await expect(page.getByTestId("object-palette")).toBeVisible();
  await page.waitForFunction(() => typeof window.__enviTest !== "undefined");
  await expect(page.getByTestId("project-bar").getByText(PROJECT_NAME)).toBeVisible();
  await expect(page.getByTestId("store-feature-count")).toHaveText(String(n), { timeout: 5000 });
  const afterReload = await page.evaluate(() => window.__enviTest.featureIds().slice().sort());
  expect(afterReload).toEqual(drawnIds.slice().sort());
  // Terra Draw re-hydrated from the reloaded store too (loadScene → TD re-add repopulates the view).
  await expectRepopulated(page, "td-feature-count");

  // (3) Close the project then reopen-last → the same scene is restored (reopen-last GET round-trips).
  await page.evaluate(() => window.__enviTest.closeProject());
  await expect(page.getByTestId("store-feature-count")).toHaveText("0");
  await page.evaluate(() => window.__enviTest.reopenLast());
  await expect(page.getByTestId("project-bar").getByText(PROJECT_NAME)).toBeVisible();
  await expect(page.getByTestId("store-feature-count")).toHaveText(String(n), { timeout: 5000 });
  const afterReopen = await page.evaluate(() => window.__enviTest.featureIds().slice().sort());
  expect(afterReopen).toEqual(drawnIds.slice().sort());

  // Offline invariant: nothing escaped to the live network.
  expect(unmocked, `unmocked network requests: ${unmocked.join(", ")}`).toEqual([]);
});

test("SC4 seams: Open lists + opens a project, and New creates + opens one (offline, no placeholders)", async ({
  page,
}) => {
  const unmocked = await installOffline(page);

  // The project list (Open picker), an openable project + its scene, and project creation (New).
  await page.route(/\/api\/v1\/projects$/, (route) => {
    if (route.request().method() === "POST") {
      return route.fulfill({
        status: 201,
        contentType: "application/json",
        body: JSON.stringify({ id: "created-1", name: "Fresh Project", description: "", created_at_unix: 9, modified_at_unix: 9 }),
      });
    }
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify([
        { id: "existing-1", name: "Existing Project", description: "", created_at_unix: 5, modified_at_unix: 5 },
      ]),
    });
  });
  await page.route(/\/api\/v1\/projects\/existing-1$/, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ id: "existing-1", name: "Existing Project", description: "", created_at_unix: 5, modified_at_unix: 5 }),
    }),
  );
  await page.route(/\/api\/v1\/projects\/(existing-1|created-1)\/scene$/, (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ type: "FeatureCollection", features: [] }) }),
  );

  await page.goto("/");
  await expect(page.getByTestId("object-palette")).toBeVisible();

  // Open → the picker lists the stored project; clicking it opens the project (real, not a placeholder).
  await page.getByTestId("project-open").click();
  await expect(page.getByTestId("project-picker")).toBeVisible();
  await expect(page.getByTestId("picker-project")).toHaveCount(1);
  await page.getByTestId("picker-project").click();
  await expect(page.getByTestId("project-picker")).toHaveCount(0);
  await expect(page.getByTestId("project-bar").getByText("Existing Project")).toBeVisible();

  // New → typing a name + Create creates a project and opens it (the created project's name shows).
  await page.getByTestId("project-new").click();
  await expect(page.getByTestId("project-picker")).toBeVisible();
  await expect(page.getByTestId("picker-create")).toBeDisabled(); // blank name gates Create
  await page.getByTestId("picker-new-name").fill("Fresh Project");
  await expect(page.getByTestId("picker-create")).toBeEnabled();
  await page.getByTestId("picker-create").click();
  await expect(page.getByTestId("project-picker")).toHaveCount(0);
  await expect(page.getByTestId("project-bar").getByText("Fresh Project")).toBeVisible();

  expect(unmocked, `unmocked network requests: ${unmocked.join(", ")}`).toEqual([]);
});
