// scenarios.spec.ts — the WEB-12 / METX-03/04 weather WHAT-IF offline UAT (SC4). Drives
// the REAL vite-served bundle (COOP/COEP → `crossOriginIsolated`, so the wasm
// instantiates) in headless Chromium, fully offline: `/api/*` is route-mocked and the
// scenario compute client is seeded with the REAL WASM friendly A/B/C derivation
// (`derive_weather_friendly`) + a fixture full-solve readout, while the REAL WASM
// `difference_dba` boundary computes the A − B delta. NOTHING acoustic is reimplemented
// in TS (D-01).
//
// Asserts the SC4 observables: "New scenario" clones the current met + edits it +
// computes its OWN hash-keyed cached tensor (a recompute is dispatched); switching
// between two named scenarios is INSTANT (loads a cached tensor, NO re-solve); "Compare
// scenarios" renders the DIVERGING difference map (blue↔gray↔red, gray at Δ≈0, a
// signed-dB legend, fill not raster); and deleting a scenario prompts the destructive
// confirm and removes it — with ZERO network egress.

import { expect, test } from "@playwright/test";

import { bootOffline } from "./_mocks";

test("weather scenarios: clone-then-edit recompute, instant switch, and the diverging A − B map", async ({
  page,
}) => {
  const unmocked = await bootOffline(page);

  await expect(page.getByTestId("results-panel")).toBeVisible();
  await expect(page.getByTestId("scenario-panel")).toBeVisible();

  // Seed the registry + attach the (real-derive / fixture-solve) compute client and the
  // REAL WASM difference client.
  await page.evaluate(() => window.__enviTest.seedScenarios());

  // The "One scenario" empty state shows with only the base scenario.
  await expect(page.getByTestId("scenario-empty")).toBeVisible();
  await expect(page.getByTestId("scenario-row-base")).toBeVisible();

  // --- Compute the base scenario (a recompute is dispatched) ---
  const epoch0 = await page.evaluate(() => window.__enviTest.scenarioState().computeEpoch);
  await page.getByTestId("scenario-compute-base").click();
  await expect(page.getByTestId("scenario-cached-base")).toBeVisible();
  await expect
    .poll(() => page.evaluate(() => window.__enviTest.scenarioState().computeEpoch))
    .toBeGreaterThan(epoch0);

  // --- New scenario = clone-then-edit (D-13) ---
  await page.getByTestId("scenario-new").click();
  // The empty state clears (a second scenario exists) and the clone is active.
  await expect(page.getByTestId("scenario-empty")).toHaveCount(0);
  const afterClone = await page.evaluate(() => window.__enviTest.scenarioState());
  expect(afterClone.scenarios).toHaveLength(2);
  const cloneId = afterClone.scenarios.find((s) => s.id !== "base")!.id;
  expect(afterClone.activeId).toBe(cloneId);
  // A clone is NOT yet computed — it must solve its OWN tensor identity.
  expect(afterClone.scenarios.find((s) => s.id === cloneId)!.computed).toBe(false);

  // Edit the friendly met (a warmer scenario) — the identity changes ⇒ a recompute is due.
  await page.getByTestId("scenario-temp").fill("40");

  // --- Compute the clone: its OWN cached tensor (a met change is a recompute) ---
  const epochBeforeClone = await page.evaluate(
    () => window.__enviTest.scenarioState().computeEpoch,
  );
  await page.getByTestId(`scenario-compute-${cloneId}`).click();
  await expect(page.getByTestId(`scenario-cached-${cloneId}`)).toBeVisible();
  await expect
    .poll(() => page.evaluate(() => window.__enviTest.scenarioState().computeEpoch))
    .toBeGreaterThan(epochBeforeClone);

  // --- Switch INSTANTLY between the two named scenarios — NO re-solve ---
  const beforeSwitch = await page.evaluate(() => window.__enviTest.scenarioState());
  await page.getByTestId("scenario-switch-base").click();
  await page.getByTestId(`scenario-switch-${cloneId}`).click();
  const afterSwitch = await page.evaluate(() => window.__enviTest.scenarioState());
  // The switch epoch advanced (two instant switches) but NO further compute ran.
  expect(afterSwitch.switchEpoch).toBe(beforeSwitch.switchEpoch + 2);
  expect(afterSwitch.computeEpoch).toBe(beforeSwitch.computeEpoch);
  expect(afterSwitch.activeId).toBe(cloneId);

  // --- Compare scenarios → the DIVERGING A − B difference map (D-16) ---
  await page.getByTestId("scenario-compare-a").selectOption(cloneId);
  await page.getByTestId("scenario-compare-b").selectOption("base");
  await page.getByTestId("scenario-compare-run").click();

  // The delta was computed in WASM and the diverging fill traced (fill, not raster).
  await expect
    .poll(() => page.evaluate(() => window.__enviTest.differenceState().hasDelta))
    .toBe(true);
  await expect
    .poll(() => page.evaluate(() => window.__enviTest.differenceTelemetry().featureCount))
    .toBeGreaterThan(0);
  const diff = await page.evaluate(() => window.__enviTest.differenceTelemetry());
  expect(diff.layerType).toBe("fill"); // fill polygons, never a heatmap raster (D-02)
  // The midpoint (Δ ≈ 0) is EXACTLY the neutral gray — never a hue, never the brand accent.
  expect(diff.midpointColor).toBe("#383835");

  // The signed-dB legend renders with the neutral "no change" band; no legend row uses
  // the brand accent as a data pole.
  await expect(page.getByTestId("difference-legend")).toBeVisible();
  const legendColors = await page
    .getByTestId("difference-legend")
    .locator("[data-color]")
    .evaluateAll((els) => els.map((e) => e.getAttribute("data-color")));
  expect(legendColors).toContain("#383835"); // the gray midpoint
  expect(legendColors).not.toContain("#4ea8ff"); // the brand accent is NOT a pole

  // --- Delete a scenario → the destructive confirm removes it ---
  await page.getByTestId(`scenario-delete-${cloneId}`).click();
  await expect(page.getByTestId("scenario-delete-dialog")).toBeVisible();
  await page.getByTestId("scenario-delete-confirm").click();
  await expect(page.getByTestId(`scenario-row-${cloneId}`)).toHaveCount(0);
  // Back to the base-only "One scenario" state.
  await expect(page.getByTestId("scenario-empty")).toBeVisible();

  // Nothing touched the network the whole run.
  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});
