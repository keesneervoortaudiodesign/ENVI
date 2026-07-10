// lifecycle.spec.ts — the Gate-1 Terra Draw ⇄ react-map-gl lifecycle proof, fully offline (D-13a).
//
// # Module I/O
// - Input  the REAL Vite-served bundle (Playwright webServer) with the basemap + /api/* network
//   surface route-intercepted (`installOffline`).
// - Output assertions that: the dark basemap renders with OSM attribution; exactly ONE Terra Draw
//   instance exists under React StrictMode's dev double-mount; a store-canonical feature survives a
//   `map.setStyle()` basemap switch via the single `style.load` re-hydration (SC4); store-originated
//   `addFeatures` echoes never feed back into the store (no loop); and NO request escapes to the live
//   network (`tiles.openfreemap.org`).

import { expect, test } from "@playwright/test";

import { installOffline } from "./_mocks";

test("store-canonical Terra Draw lifecycle survives a basemap switch, fully offline", async ({
  page,
}) => {
  const unmocked = await installOffline(page);

  await page.goto("/");

  // The map loads (stub dark style → style.load → build) and the instance becomes ready.
  await expect(page.getByTestId("map-ready")).toHaveText("yes");

  // SINGLE Terra Draw instance under StrictMode's dev double-mount (instance-in-ref guard + stop()).
  await expect(page.getByTestId("td-instances-live")).toHaveText("1");

  // No re-hydration yet — the initial style load happens before the instance is built.
  await expect(page.getByTestId("rehydration-count")).toHaveText("0");

  // OSM attribution is displayed (data hygiene). The compact control keeps the text in the DOM.
  await expect(page.locator(".maplibregl-ctrl-attrib-inner")).toContainText("OpenStreetMap");

  // Seed one canonical feature (user-intent path): it lands in the store AND renders in Terra Draw.
  await page.getByTestId("add-test-point").click();
  await expect(page.getByTestId("store-feature-count")).toHaveText("1");
  await expect(page.getByTestId("td-feature-count")).toHaveText("1");

  // Switch basemap → map.setStyle() destroys sources/layers; the single style.load hook re-hydrates the
  // scene from the store (SC4). The store must NOT grow (origin:"api" echoes are ignored — no loop).
  await page.getByTestId("switch-basemap").click();
  await expect(page.getByTestId("rehydration-count")).toHaveText("1");
  await expect(page.getByTestId("store-feature-count")).toHaveText("1");
  await expect(page.getByTestId("td-feature-count")).toHaveText("1");

  // Switch back to the (intercepted) network dark style — a second full setStyle + re-hydration.
  await page.getByTestId("switch-basemap").click();
  await expect(page.getByTestId("rehydration-count")).toHaveText("2");
  await expect(page.getByTestId("store-feature-count")).toHaveText("1");
  await expect(page.getByTestId("td-feature-count")).toHaveText("1");

  // Still exactly one live instance after two style swaps.
  await expect(page.getByTestId("td-instances-live")).toHaveText("1");

  // Offline invariant: nothing reached the live network.
  expect(unmocked, `unmocked network requests: ${unmocked.join(", ")}`).toEqual([]);
});
