// conditioning.spec.ts — the WEB-05 / SVC-06 interactive CONDITIONING fast-recalc offline
// UAT (SC2). Drives the REAL vite-served bundle (COOP/COEP → `crossOriginIsolated`, so the
// compute wasm instantiates) in headless Chromium, fully offline: `/api/*` is route-mocked
// and a fixture tensor is seeded into OPFS keyed by the REAL wasm-minted identity, then the
// REAL `readout_receivers` MAC reconditions it. NOTHING is reimplemented — the panel renders
// WASM-produced values (D-01).
//
// Asserts the SC2 observables: adjusting a source's GAIN then FILTER (the reused Phase-7
// SpectrumEditor, D-11) live-updates the receiver spectrum AND the isophone map via the MAC
// with NO propagation re-run (the calc job never leaves idle, the isophone RE-CONTOURS the
// cached grid — SC3, no re-solve); a conditioning edit shows NO stale badge; a simulated
// SCENE edit (a diverged tensor identity) flips the "Out of date" badge (D-12); and a MAC
// against the now-mismatched hash surfaces the honest 409 reject banner and updates nothing.

import { expect, test } from "@playwright/test";

import { bootOffline, installMetaMocks } from "./_mocks";

test("conditioning fast-recalc: live MAC on spectra + map, never-stale, and the honest 409", async ({
  page,
}) => {
  const unmocked = await bootOffline(page);
  await installMetaMocks(page);

  await expect(page.getByTestId("results-panel")).toBeVisible();

  // Seed a 2-source · 9-receiver result keyed by the REAL minted tensor identity + a 3×3
  // isophone grid (cell count == receivers, so a recalc re-feeds the map 1:1).
  const seed = await page.evaluate(() => window.__enviTest.seedConditioning(2, 9));
  const [sourceA, sourceB] = seed.sourceIds;
  const receiverIds = seed.receiverIds;
  expect(seed.sourceIds).toEqual(["0", "1"]);
  expect(receiverIds).toHaveLength(9);

  // The conditioning panel renders one row per source; a fresh result is NOT stale.
  await expect(page.getByTestId("conditioning-panel")).toBeVisible();
  await expect(page.getByTestId(`conditioning-source-${sourceA}`)).toBeVisible();
  await expect(page.getByTestId(`conditioning-source-${sourceB}`)).toBeVisible();
  await expect(page.getByTestId("conditioning-stale-badge")).toHaveCount(0);

  // Select a receiver → its spectrum renders (the readout MAC over the seeded tensor).
  await page.getByTestId(`spectrum-receiver-${receiverIds[0]}`).click();
  const total = page.getByTestId("spectrum-total");
  await expect(total).toBeVisible();
  const totalBefore = (await total.textContent())?.trim();

  const traceBefore = await page.evaluate(() => window.__enviTest.isophoneTelemetry().traceCount);

  // --- GAIN edit → live recalc: spectra + map update via the MAC, no propagation ---
  await page.getByTestId(`conditioning-gain-${sourceA}`).fill("12");
  // The debounced MAC applies — the recalc epoch bumps (reconditioned spectra pushed) ...
  await expect
    .poll(() => page.evaluate(() => window.__enviTest.conditioningState().recalcEpoch))
    .toBeGreaterThan(0);
  // ... the isophone map RE-CONTOURS the reconditioned grid (SC3, no re-solve) ...
  await expect
    .poll(() => page.evaluate(() => window.__enviTest.isophoneTelemetry().traceCount))
    .toBeGreaterThan(traceBefore);
  // ... the selected receiver's weighted total genuinely moved ...
  await expect.poll(async () => (await total.textContent())?.trim()).not.toBe(totalBefore);
  // ... and NO solve worker ran (the calc job never left idle — this is a tensor MAC, not a re-solve).
  expect(await page.evaluate(() => window.__enviTest.calcJobState())).toBe("idle");
  // A conditioning edit NEVER stales (D-07).
  expect(await page.evaluate(() => window.__enviTest.staleState().isStale)).toBe(false);
  await expect(page.getByTestId("conditioning-stale-badge")).toHaveCount(0);

  // --- FILTER edit via the REUSED SpectrumEditor (D-11) → another live recalc ---
  const epochAfterGain = await page.evaluate(
    () => window.__enviTest.conditioningState().recalcEpoch,
  );
  await page.getByTestId(`conditioning-filter-${sourceA}`).click();
  await expect(page.getByTestId("spectrum-editor")).toBeVisible();
  // Author a filter: pick a resolution (seeds anchors) then edit one anchor → the SERVER
  // materialises the dense [105] (no TS acoustic math) → drives the recondition MAC.
  await page.getByTestId("spectrum-res-third").click();
  await page.getByTestId("spectrum-cell-0").fill("-6");
  await page.getByTestId("spectrum-close").click();
  await expect
    .poll(() => page.evaluate(() => window.__enviTest.conditioningState().recalcEpoch))
    .toBeGreaterThan(epochAfterGain);
  // The filter is now part of the source's drive (the panel shows "Edit filter").
  await expect(page.getByTestId(`conditioning-filter-${sourceA}`)).toHaveText("Edit filter");

  // --- SCENE edit → the stale badge appears (re-minted identity diverges, D-12) ---
  await page.evaluate(() => window.__enviTest.divergeScene());
  await expect(page.getByTestId("conditioning-stale-badge")).toBeVisible();
  expect(await page.evaluate(() => window.__enviTest.staleState().isStale)).toBe(true);

  // --- A MAC against the now-mismatched hash is REFUSED (SVC-06 409), never served ---
  const epochBeforeRefuse = await page.evaluate(
    () => window.__enviTest.conditioningState().recalcEpoch,
  );
  const totalBeforeRefuse = (await total.textContent())?.trim();
  await page.getByTestId(`conditioning-gain-${sourceA}`).fill("3");
  await expect(page.getByTestId("conditioning-reject")).toBeVisible();
  // No spectra were served: the recalc epoch did NOT advance and the total is unchanged.
  await expect
    .poll(() => page.evaluate(() => window.__enviTest.conditioningState().refuse))
    .toBe(true);
  expect(await page.evaluate(() => window.__enviTest.conditioningState().recalcEpoch)).toBe(
    epochBeforeRefuse,
  );
  expect((await total.textContent())?.trim()).toBe(totalBeforeRefuse);

  // Nothing touched the network the whole run.
  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});
