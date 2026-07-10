// draw-kinds.spec.ts — the SC1 draw-each-kind + last-object-inheritance proof, fully offline (D-13a).
//
// # Module I/O
// - Input  the REAL Vite-served bundle with the basemap + /api/* surface route-intercepted
//   (`installOffline`) plus a PUT-scene interceptor that captures the outbound payload. Features are
//   placed through the DEV `window.__enviTest` bridge (the same store commit path a finished Terra Draw
//   shape uses) — headless WebGL drawing is unreliable, and the plan permits programmatic commits.
// - Output assertions that: all 9 locked kinds land in the canonical store with a valid `properties.kind`
//   and appear in the mocked whole-scene PUT payload; a second ground_zone inherits the first's edited
//   impedance/roughness and shows the "inherited from last ground_zone" chip until the field is edited;
//   and NO request escapes to the live network.

import { expect, test, type Page } from "@playwright/test";

import { installOffline } from "./_mocks";

const KINDS = [
  "source",
  "receiver",
  "wall",
  "building",
  "forest",
  "ground_zone",
  "elevation_point",
  "elevation_line",
  "calc_area",
] as const;

// Place a feature of `kind` by selecting its palette tool (proving palette → active kind) and committing
// through the DEV bridge at a distinct lng/lat so geometries never coincide.
async function placeKind(page: Page, kind: string, i: number): Promise<void> {
  await page.getByTestId(`tool-${kind}`).click();
  await expect(page.getByTestId(`tool-${kind}`)).toHaveAttribute("aria-pressed", "true");
  await page.evaluate(
    ([lng, lat]) => window.__enviTest.commitActive(lng as number, lat as number),
    [4.9 + i * 0.001, 52.36 + i * 0.001],
  );
}

test("places all 9 kinds into the store + PUT payload, with last-object inheritance", async ({ page }) => {
  const unmocked = await installOffline(page);

  // Capture the outbound whole-scene PUT (registered AFTER installOffline so it wins the route match).
  let putBody: { features: { properties?: { kind?: string } }[] } | null = null;
  await page.route(/\/api\/v1\/projects\/[^/]+\/scene$/, async (route) => {
    if (route.request().method() === "PUT") {
      putBody = route.request().postDataJSON();
      await route.fulfill({ status: 200, contentType: "application/json", body: "{}" });
    } else {
      await route.fulfill({ status: 200, contentType: "application/json", body: "{}" });
    }
  });

  await page.goto("/");
  await expect(page.getByTestId("object-palette")).toBeVisible();
  // The DEV commit bridge installs via an async dynamic import — wait for it before driving commits.
  await page.waitForFunction(() => typeof window.__enviTest !== "undefined");

  // 1) Place each of the 9 kinds.
  for (let i = 0; i < KINDS.length; i++) {
    await placeKind(page, KINDS[i], i);
  }

  // Every kind is in the store with a valid properties.kind.
  const placed = await page.evaluate(() => window.__enviTest.state().kinds);
  const kindsInStore = Object.values(placed);
  for (const kind of KINDS) {
    expect(kindsInStore, `store is missing kind ${kind}`).toContain(kind);
  }
  expect(kindsInStore.every((k) => k !== null)).toBe(true);

  // 2) Last-object inheritance: edit the first ground_zone's impedance, then place a second ground_zone.
  await page.getByTestId("tool-ground_zone").click();
  // Select the first ground_zone (it is the currently-selected object after placement above? No — the
  // 9-kind loop left calc_area selected. Re-place a first ground_zone to make it the selection.)
  await page.evaluate(() => window.__enviTest.commit("ground_zone", 4.95, 52.4));
  // The inspector now shows this ground_zone. Change its impedance from the default 'D' to 'C'.
  await page.getByTestId("ground-impedance").selectOption("C");
  await expect(page.getByTestId("ground-impedance")).toHaveValue("C");

  // Place a SECOND ground_zone → it inherits impedance 'C' + roughness, and shows the inherited chip.
  await page.evaluate(() => window.__enviTest.commit("ground_zone", 4.96, 52.41));
  await expect(page.getByTestId("ground-impedance")).toHaveValue("C");
  await expect(page.getByTestId("inspector")).toContainText("inherited from last ground_zone");

  // Editing the inherited field clears its chip (WEB-04). Change impedance → 'E'.
  await page.getByTestId("ground-impedance").selectOption("E");
  await expect(page.getByTestId("ground-impedance")).toHaveValue("E");
  // The impedance chip is gone; only the still-inherited roughness chip may remain — assert the
  // impedance row no longer carries an inherited marker by checking the chip count dropped.
  const inheritedAfterEdit = await page.evaluate(() => {
    const st = window.__enviTest.state();
    return st.selection ? st.inherited[st.selection] ?? [] : [];
  });
  expect(inheritedAfterEdit).not.toContain("impedance_class");

  // 3) Save → the whole-scene PUT payload carries every placed feature with a valid kind.
  await page.getByTestId("save-scene").click();
  await expect.poll(() => putBody).not.toBeNull();
  const body = putBody as unknown as { features: { properties?: { kind?: string } }[] };
  const payloadKinds = new Set(body.features.map((f) => f.properties?.kind));
  for (const kind of KINDS) {
    expect(payloadKinds, `PUT payload missing kind ${kind}`).toContain(kind);
  }
  expect(body.features.every((f) => typeof f.properties?.kind === "string")).toBe(true);

  // Offline invariant: nothing reached the live network.
  expect(unmocked, `unmocked network requests: ${unmocked.join(", ")}`).toEqual([]);
});
