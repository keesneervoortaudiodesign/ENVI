// demo.spec.ts — a VISUAL walkthrough of the Phase-11 results workflow driven by a REAL
// threaded solve, offline. Captures an annotated screenshot at each stage into
// `test-results/demo/` so the whole flow (draw → real solve → spectrum → dB toggle →
// isophone map → info help) can be reviewed as images. Run:
//   npx playwright test demo.spec.ts            (headless; screenshots in test-results/demo)
//   npx playwright test demo.spec.ts --headed   (watch it live in a browser window)

import { expect, test, type Page, type Route } from "@playwright/test";

import { bootOffline, installMetaMocks } from "./_mocks";

const OUT = "test-results/demo";
const PROJECT_ID = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
const SITE = { lng: 4.9, lat: 52.37 };

async function shot(page: Page, name: string): Promise<void> {
  await page.screenshot({ path: `${OUT}/${name}.png` });
}

test("DEMO: real solve → spectrum + isophone map + help, offline", async ({ page }) => {
  test.setTimeout(120_000);
  await bootOffline(page);
  await installMetaMocks(page);

  // 1. Open a project and draw a calc area + a source (the authoring path).
  await expect(page.getByTestId("calc-panel")).toBeVisible();
  await page.evaluate(({ id }) => window.__enviTest.openProject(id, "ENVI Demo"), { id: PROJECT_ID });
  await page.evaluate((s) => {
    window.__enviTest.commit("calc_area", s.lng, s.lat);
    window.__enviTest.commit("source", s.lng, s.lat);
  }, SITE);
  await shot(page, "01-scene-drawn");

  // 2. Run a GENUINE client-side threaded solve (offline, zero egress).
  await page.route("**/*", (route: Route) => {
    const u = route.request().url();
    return u.startsWith("http://localhost") || u.startsWith("data:") || u.startsWith("blob:")
      ? route.continue()
      : route.abort();
  });
  await page.getByTestId("calc-spacing").fill("20");
  await expect(page.getByTestId("calc-estimate")).toBeVisible({ timeout: 30_000 });
  await shot(page, "02-cost-estimate");
  await expect(page.getByTestId("calc-run")).toBeEnabled({ timeout: 30_000 });
  await page.getByTestId("calc-run").click();
  await expect(page.getByTestId("calc-status")).toHaveText("done", { timeout: 90_000 });
  await shot(page, "03-solve-done");

  // 3. Spectrum panel fed by the real solve: select a receiver, read its spectrum.
  const receiver = page.locator('[data-testid^="spectrum-receiver-"]').first();
  await expect(receiver).toBeVisible({ timeout: 30_000 });
  await receiver.click();
  await expect(page.getByTestId("spectrum-chart")).toBeVisible();
  await shot(page, "04-spectrum-dbA");

  // 4. Instant dB(A) → dB(C) toggle (no recompute).
  await page.getByTestId("spectrum-weighting-C").click();
  await expect(page.getByTestId("spectrum-total")).toHaveAttribute("data-weighting", "C");
  await shot(page, "05-spectrum-dbC");

  // 5. The isophone noise map, reconstructed from the SAME real solve (fill layer).
  await expect
    .poll(async () => (await page.evaluate(() => window.__enviTest.isophoneTelemetry())).layerType, {
      timeout: 30_000,
    })
    .toBe("fill");
  await expect
    .poll(async () => (await page.evaluate(() => window.__enviTest.isophoneTelemetry())).traceCount)
    .toBeGreaterThan(0);
  await shot(page, "06-isophone-map");

  // 6. Universal info-button help (standards-cited).
  await page.getByTestId("info-spectrum.split").click();
  await expect(page.getByTestId("info-popover-spectrum.split")).toBeVisible();
  await shot(page, "07-info-help");

  // A final full-page shot of the whole results view.
  await page.screenshot({ path: `${OUT}/08-overview.png`, fullPage: true });
});
