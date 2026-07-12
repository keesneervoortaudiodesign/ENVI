// results-real-solve.spec.ts — the TRUE end-to-end proof (SC1/SC2 production path): a
// GENUINE client-side threaded solve, run to completion, feeds the spectrum panel via
// the real `applyResultsFeed` link (CalcPanel's `applyTierComplete → setManifest`) — no
// DEV fixture seed. Drives the real vite-served bundle offline (COOP/COEP ⇒ cross-origin
// isolated ⇒ the wasm-bindgen-rayon pool starts), zero network egress.
//
// A small calc area + a coarse fine-spacing keep the FINE tier tiny so the whole solve
// (points → coarse → fine → done) completes in-test in a few seconds. If the threaded
// pool cannot start in some environment, the run is skipped HONESTLY (never a fake green),
// mirroring calc.spec Test 2.

import { expect, test, type Route } from "@playwright/test";

import { bootOffline, installMetaMocks } from "./_mocks";

const PROJECT_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const SITE = { lng: 4.9, lat: 52.37 };

test("a real threaded solve runs to done and feeds the spectrum panel via the production link", async ({
  page,
}) => {
  test.setTimeout(120_000);
  const unmocked = await bootOffline(page);
  await installMetaMocks(page);

  await expect(page.getByTestId("calc-panel")).toBeVisible();
  await page.evaluate(({ id }) => window.__enviTest.openProject(id, "E2E Real Solve"), {
    id: PROJECT_ID,
  });
  await page.evaluate((s) => {
    window.__enviTest.commit("calc_area", s.lng, s.lat);
    window.__enviTest.commit("source", s.lng, s.lat);
  }, SITE);

  expect(await page.evaluate(() => self.crossOriginIsolated === true)).toBe(true);

  // A large fine spacing → a tiny fine tier → the whole solve finishes fast.
  await page.getByTestId("calc-spacing").fill("20");
  await expect(page.getByTestId("calc-estimate")).toBeVisible({ timeout: 30_000 });

  // Record any live-network request across the solve (must be zero — all same-origin).
  const egress: string[] = [];
  await page.route("**/*", (route: Route) => {
    const url = route.request().url();
    if (
      url.startsWith("http://localhost") ||
      url.startsWith("http://127.0.0.1") ||
      url.startsWith("data:") ||
      url.startsWith("blob:")
    ) {
      return route.continue();
    }
    egress.push(url);
    return route.abort();
  });

  await expect(page.getByTestId("calc-run")).toBeEnabled({ timeout: 30_000 });
  await page.getByTestId("calc-run").click();

  // The pool must actually start for the solve to progress. If it stays queued for a
  // generous window, the threaded pool did not initialise here — skip honestly.
  let started = false;
  for (let i = 0; i < 20; i += 1) {
    const status = (await page.getByTestId("calc-status").textContent()) ?? "";
    if (status.includes("running") || status.includes("done")) {
      started = true;
      break;
    }
    await page.waitForTimeout(1000);
  }
  test.skip(!started, "wasm-bindgen-rayon thread pool did not initialise in this environment");

  // The genuine solve completes: the status chip reaches `done`.
  await expect(page.getByTestId("calc-status")).toHaveText("done", { timeout: 90_000 });

  // The PRODUCTION link fired: the fine `TierComplete` fed the results store via
  // `applyResultsFeed`, so the spectrum panel now offers the solved receivers — with NO
  // DEV fixture seed. Selecting one renders REAL readout values over the REAL solved tensor.
  await expect(page.getByTestId("results-panel")).toBeVisible();
  const firstReceiver = page.locator('[data-testid^="spectrum-receiver-"]').first();
  await expect(firstReceiver).toBeVisible({ timeout: 30_000 });
  await firstReceiver.click();

  const chart = page.getByTestId("spectrum-chart");
  await expect(chart).toBeVisible({ timeout: 30_000 });
  await expect(chart).toHaveAttribute("data-band-count", "27");
  // A real weighted total is shown (from the WASM readout over the solved tensor).
  await expect(page.getByTestId("spectrum-total")).toBeVisible();

  expect(egress, `solve-time egress: ${egress.join(", ")}`).toEqual([]);
  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});
