// results-spectrum.spec.ts — the WEB-11 receiver SPECTRUM PANEL offline UAT (SC1).
//
// Drives the REAL vite-served bundle (the project's e2e definition of the real app;
// the dev server sends COOP/COEP so `crossOriginIsolated` holds and the compute
// wasm instantiates) in headless Chromium, fully offline: `/api/*` is route-mocked
// (`bootOffline` + `installMetaMocks`) and a fixture tensor is seeded into OPFS via
// the DEV bridge, then the REAL `readout_receivers` wasm export decodes it. NOTHING
// is reimplemented — the panel renders WASM-produced values (D-01).
//
// Asserts the WEB-11 observables: the empty "Select a receiver" state; selection
// via the synced LIST and via a MAP marker click; the 1/3⇄1/12 toggle changing the
// band count (aggregated by band index); the dB(A)⇄dB(C) toggle changing the total
// instantly with NO recompute (no loading state, no worker round-trip); and the
// on-demand coherent/incoherent split overlay.

import { expect, test } from "@playwright/test";

import { bootOffline, installMetaMocks } from "./_mocks";

test("empty state, dual selection, display + weighting toggles, and the split overlay", async ({
  page,
}) => {
  const unmocked = await bootOffline(page);
  await installMetaMocks(page);

  // The panel is mounted; with no results it shows the no-receivers prompt.
  await expect(page.getByTestId("results-panel")).toBeVisible();

  // Seed a fixture tensor (3 receivers, one chunk) keyed by the REAL minted tensor
  // identity, and set the results manifest — the same store path a finished solve
  // feeds. Returns the receiver UUIDs.
  const ids = await page.evaluate(() => window.__enviTest.seedResults(3));
  expect(ids).toHaveLength(3);

  // With results present but nothing selected → the honest "Select a receiver" empty state.
  await expect(page.getByTestId("spectrum-empty")).toBeVisible();
  await expect(page.getByTestId("spectrum-receiver-list")).toBeVisible();

  // --- Select via the synced LIST ---
  await page.getByTestId(`spectrum-receiver-${ids[0]}`).click();
  const chart = page.getByTestId("spectrum-chart");
  await expect(chart).toBeVisible();
  await expect(page.getByTestId(`spectrum-receiver-${ids[0]}`)).toHaveAttribute(
    "data-selected",
    "true",
  );

  // The 1/3-octave default aggregates by band index to the 27 third-octave centres.
  await expect(chart).toHaveAttribute("data-band-count", "27");
  await expect(chart).toHaveAttribute("data-display", "third");
  expect(await page.getByTestId("spectrum-band-bar").count()).toBe(27);

  // Toggle to the 1/12-octave expert view → all 105 bands (band-index aggregation).
  await page.getByTestId("spectrum-display-twelfth").click();
  await expect(chart).toHaveAttribute("data-band-count", "105");
  expect(await page.getByTestId("spectrum-band-bar").count()).toBe(105);
  await page.getByTestId("spectrum-display-third").click();
  await expect(chart).toHaveAttribute("data-band-count", "27");

  // --- Instant dB(A)⇄dB(C) toggle: the total changes with NO recompute ---
  const total = page.getByTestId("spectrum-total");
  await expect(total).toHaveAttribute("data-weighting", "A");
  const dbaText = (await total.textContent())?.trim();
  await page.getByTestId("spectrum-weighting-C").click();
  await expect(total).toHaveAttribute("data-weighting", "C");
  const dbcText = (await total.textContent())?.trim();
  expect(dbcText).not.toEqual(dbaText); // the weighted total genuinely changed
  // No recompute: the loading state never appears on a weighting toggle (the
  // readout is cached; both weightings are precomputed).
  await expect(page.getByTestId("spectrum-loading")).toHaveCount(0);

  // --- Coherent/incoherent split overlay on demand ---
  await expect(page.getByTestId("spectrum-split-overlay")).toHaveCount(0);
  await page.getByTestId("spectrum-split-toggle").click();
  await expect(page.getByTestId("spectrum-split-overlay")).toBeVisible();
  // The split totals are always present.
  await expect(page.getByTestId("spectrum-total-coherent")).toBeVisible();
  await expect(page.getByTestId("spectrum-total-incoherent")).toBeVisible();

  // --- Select a DIFFERENT receiver via a MAP marker click ---
  await page.getByTestId(`spectrum-marker-${ids[2]}`).click();
  await expect(page.getByTestId(`spectrum-marker-${ids[2]}`)).toHaveAttribute(
    "data-selected",
    "true",
  );
  await expect(chart).toBeVisible();

  // Nothing touched the network the whole run.
  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});
