// sc1-author-every-kind.spec.ts — the SC1 + SC3 INTEGRATED authoring journey, fully offline (D-13a). This
// is the phase's goal-backward proof that the whole authoring surface holds together as ONE user flow:
// every scene kind is placeable + editable, a valid elevation set re-triangulates the DGM into an
// observable TIN overlay, interior-crossing breaklines surface a single crit row, last-object inheritance
// fires on a repeated kind, an isolation spectrum authored at 1/1-octave lands its anchors on the exact
// band indices with the server-derived preview, and the whole authored scene reaches the coalesced PUT.
//
// # Module I/O
// - Input  the REAL Vite-served bundle with the basemap + /api/v1 surface route-intercepted
//   (`installOffline`), plus the deterministic `/meta` fixtures (`installMetaMocks`), the
//   breaklines→4xx / points→TIN triangulate oracle (`installTriangulateMock`), and a PUT-scene capture.
//   Scene mutations run through the DEV `window.__enviTest` bridge (the same store paths a finished Terra
//   Draw edit uses) — headless WebGL drawing is unreliable and the plan permits programmatic commits.
// - Output assertions that a BROKEN implementation would fail: a missing kind is absent from the store/PUT;
//   a no-op DGM producer leaves the TIN triangle count at 0 and never raises the crit row; a dropped
//   inheritance seed shows no "inherited from last …" chip; a client-side (non-server) spectrum would not
//   place anchors on 4,16,…,100 with the mocked 105-grid preview. Nothing reaches the live network.

import { expect, test, type Page } from "@playwright/test";

import { installMetaMocks, installOffline, installTriangulateMock } from "./_mocks";

// The 9 locked scene kinds (SC1). `calc_area` is the calculation domain; the rest are authored objects.
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

async function boot(page: Page): Promise<string[]> {
  const unmocked = await installOffline(page);
  await installMetaMocks(page);
  await installTriangulateMock(page);
  return unmocked;
}

test("SC1+SC3: authors every kind (+inheritance +TIN +crit) and edits an isolation spectrum, offline", async ({
  page,
}) => {
  const unmocked = await boot(page);

  // Capture the outbound whole-scene PUT (registered AFTER the offline stack so it wins the route match).
  let putBody: { features: { properties?: { kind?: string } }[] } | null = null;
  await page.route(/\/api\/v1\/projects\/[^/]+\/scene$/, async (route) => {
    if (route.request().method() === "PUT") {
      putBody = route.request().postDataJSON();
    }
    await route.fulfill({ status: 200, contentType: "application/json", body: "{}" });
  });

  await page.goto("/");
  await expect(page.getByTestId("object-palette")).toBeVisible();
  await page.waitForFunction(() => typeof window.__enviTest !== "undefined");

  // 1) Place all 9 kinds through the palette + DEV commit bridge (proves palette tool → active kind tag).
  for (let i = 0; i < KINDS.length; i++) {
    const kind = KINDS[i];
    await page.getByTestId(`tool-${kind}`).click();
    await expect(page.getByTestId(`tool-${kind}`)).toHaveAttribute("aria-pressed", "true");
    await page.evaluate(
      ([lng, lat]) => window.__enviTest.commitActive(lng as number, lat as number),
      [4.9 + i * 0.001, 52.36 + i * 0.001],
    );
  }
  const placed = await page.evaluate(() => Object.values(window.__enviTest.state().kinds));
  for (const kind of KINDS) {
    expect(placed, `store is missing kind ${kind}`).toContain(kind);
  }
  expect(placed.every((k) => k !== null)).toBe(true);

  // 2) Ground_zone is authored via the closed-enum selects (A–H / N-S-M-L), and last-object inheritance
  //    seeds the NEXT ground_zone from the previous one. Edit the first's impedance to 'C'…
  await page.evaluate(() => window.__enviTest.commit("ground_zone", 4.95, 52.4));
  await expect(page.getByTestId("inspector-ground_zone")).toBeVisible();
  await page.getByTestId("ground-impedance").selectOption("C");
  await page.getByTestId("ground-roughness").selectOption("M");
  // …then a SECOND ground_zone inherits 'C' + 'M' and shows the inherited chip until the field is edited.
  await page.evaluate(() => window.__enviTest.commit("ground_zone", 4.96, 52.41));
  await expect(page.getByTestId("ground-impedance")).toHaveValue("C");
  await expect(page.getByTestId("ground-roughness")).toHaveValue("M");
  await expect(page.getByTestId("inspector")).toContainText("inherited from last ground_zone");
  await page.getByTestId("ground-impedance").selectOption("E");
  const inheritedAfterEdit = await page.evaluate(() => {
    const st = window.__enviTest.state();
    return st.selection ? st.inherited[st.selection] ?? [] : [];
  });
  expect(inheritedAfterEdit).not.toContain("impedance_class");

  // 3) SC1 re-triangulation: a valid ≥3 non-collinear elevation set fires the debounced triangulate and
  //    renders an OBSERVABLE TIN overlay (a no-op producer leaves the triangle count at 0).
  await page.evaluate(() => {
    window.__enviTest.commit("elevation_point", 4.90, 52.30);
    window.__enviTest.commit("elevation_point", 4.92, 52.30);
    window.__enviTest.commit("elevation_point", 4.91, 52.32);
  });
  await expect(page.getByTestId("dgm-triangle-count")).toHaveText("1", { timeout: 5000 });

  // …and two interior-crossing elevation_lines drive the server 4xx into EXACTLY ONE crit row.
  await page.evaluate(() => {
    window.__enviTest.commit("elevation_line", 4.90, 52.30);
    window.__enviTest.commit("elevation_line", 4.905, 52.305);
  });
  await expect(page.getByTestId("issue-crit")).toHaveCount(1, { timeout: 5000 });
  await expect(page.getByTestId("issue-crit")).toContainText("interior-cross");

  // 4) SC3 spectrum authoring: open the wall's isolation-spectrum editor, author at 1/1-octave, and assert
  //    the 9 anchors land on the EXACT octave band indices (4,16,…,100 — never a non-octave index) with the
  //    server-derived 105-grid preview line rendered (the interpolation is server-owned, SVC-07/D-05).
  const wallId = await page.evaluate(() => window.__enviTest.commit("wall", 4.98, 52.42));
  await expect(page.getByTestId("inspector-wall")).toBeVisible();
  await page.getByTestId("wall-edit-spectrum").click();
  await expect(page.getByTestId("spectrum-editor")).toBeVisible();
  await page.getByTestId("spectrum-res-octave").click();
  await expect(page.getByTestId("spectrum-editor")).toHaveAttribute("data-resolution", "octave");
  for (const idx of [4, 16, 28, 40, 52, 64, 76, 88, 100]) {
    await expect(page.getByTestId(`spectrum-anchor-${idx}`)).toHaveCount(1);
  }
  await expect(page.getByTestId("spectrum-anchor-5")).toHaveCount(0); // 5 is NOT an octave centre
  await expect(page.locator('[data-testid="spectrum-curve"] polyline')).toHaveCount(1);
  // Edit an octave anchor value, then close — an authored spectrum now exists for the wall.
  await page.getByTestId("spectrum-cell-64").fill("42");
  await page.getByTestId("spectrum-close").click();
  expect(await page.evaluate((id) => window.__enviTest.spectrum(id), wallId)).not.toBeNull();

  // 5) Save → the coalesced whole-scene PUT carries every authored kind with a valid `properties.kind`.
  await page.getByTestId("save-scene").click();
  await expect.poll(() => putBody, { timeout: 5000 }).not.toBeNull();
  const body = putBody as unknown as { features: { properties?: { kind?: string } }[] };
  const payloadKinds = new Set(body.features.map((f) => f.properties?.kind));
  for (const kind of KINDS) {
    expect(payloadKinds, `PUT payload missing kind ${kind}`).toContain(kind);
  }
  expect(body.features.every((f) => typeof f.properties?.kind === "string")).toBe(true);

  // Offline invariant: nothing escaped to the live network.
  expect(unmocked, `unmocked network requests: ${unmocked.join(", ")}`).toEqual([]);
});
