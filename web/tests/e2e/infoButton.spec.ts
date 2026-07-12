// infoButton.spec.ts — the app-wide Info-Button offline UAT (D-23/24/25). Drives the REAL vite-served
// bundle in headless Chromium, fully offline (`/api/*` route-mocked, the offline guard asserts zero
// external egress). It proves the retrofit shipped: an InfoButton is present on a sampled interactive
// control in EVERY panel (scene editor/palette/inspector, import, weather, calc, spectrum, colour-scale,
// conditioning, scenario, export), that clicking one opens its glance popover, and that "More" opens the
// docked help panel with the catalog's multi-paragraph body + a standards citation. Nothing is
// reimplemented — the popover/panel render the structured `catalog` content (D-25).
//
// The coverage GUARANTEE itself (every ControlId has extensive, cited help, or the build fails) is the
// Vitest `src/help/coverage.test.ts`; this spec is the DOM-behaviour tier for the same feature.

import { expect, test } from "@playwright/test";

import { bootOffline, installMetaMocks } from "./_mocks";

test("an InfoButton is present on a sampled control in every panel, offline", async ({ page }) => {
  const unmocked = await bootOffline(page);
  await installMetaMocks(page);

  // Seed a 2-source · 9-receiver result (opens a test project + a 3×3 isophone grid), so the
  // results-dependent panels (calc/spectrum/colour-scale/conditioning/scenario/export) all render.
  await expect(page.getByTestId("results-panel")).toBeVisible();
  await page.evaluate(() => window.__enviTest.seedConditioning(2, 9));
  await expect(page.getByTestId("conditioning-panel")).toBeVisible();

  // A committed + selected source surfaces the inspector's source fields (WEB-02).
  await page.evaluate(() => window.__enviTest.commit("source", 4.91, 52.37));
  await expect(page.getByTestId("inspector-source")).toBeVisible();

  // One representative InfoButton per panel — its presence proves the retrofit reached that panel.
  const sampled = [
    "info-palette.select", // object palette
    "info-project.save", // project bar
    "info-inspector.panel", // property inspector
    "info-source.position", // inspector source fields
    "info-import.run", // GIS import
    "info-weather.date", // weather
    "info-calc.spacing", // calculate (needs an open project)
    "info-spectrum.display_mode", // receiver spectrum
    "info-colorscale.preset", // colour scale
    "info-conditioning.gain", // conditioning
    "info-scenario.new", // scenario manager
    "info-export.open", // export menu
  ];
  for (const testid of sampled) {
    await expect(page.getByTestId(testid).first(), `${testid} missing`).toBeVisible();
  }

  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});

test("clicking an InfoButton opens the glance popover; More opens the docked help panel with cited content", async ({
  page,
}) => {
  const unmocked = await bootOffline(page);
  await installMetaMocks(page);

  // The spectrum display/weighting/split controls render without any result, so target the always-visible
  // Split toggle's InfoButton.
  const info = page.getByTestId("info-spectrum.split");
  await expect(info).toBeVisible();
  await expect(info).toHaveAttribute("aria-expanded", "false");

  // Click → the glance popover opens with the control's title + a standards citation on the glance line.
  await info.click();
  await expect(info).toHaveAttribute("aria-expanded", "true");
  const popover = page.getByTestId("info-popover-spectrum.split");
  await expect(popover).toBeVisible();
  await expect(popover).toContainText("Coherent / incoherent split");
  await expect(popover).toContainText("AV 1106/07"); // Nord2000 report number, cited (never pasted)

  // "More" → the docked right-rail help panel with the full multi-paragraph body + the Sources block.
  await page.getByTestId("info-more-spectrum.split").click();
  const dock = page.getByTestId("help-dock-spectrum.split");
  await expect(dock).toBeVisible();
  await expect(dock).toContainText("Coherent / incoherent split");
  await expect(dock).toContainText("turbulence"); // body prose (our own words)
  await expect(dock.getByTestId("help-citations")).toContainText("AV 1106/07");

  // The dock closes cleanly.
  await page.getByTestId("help-dock-close-spectrum.split").click();
  await expect(dock).toHaveCount(0);

  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});
