// spectrum.spec.ts — the SC3 acoustic-authoring E2E, fully offline (D-13a): the isolation-spectrum editor
// (server preview + anchors on exact band indices + promote-to-twelfth), the semi-transparent screen
// treatment (WEB-08), and the D-02 no-silent-repoint ring-diff integration.
//
// # Module I/O
// - Input  the REAL Vite-served bundle with the basemap + /api/* surface route-intercepted
//   (`installOffline`), plus specific mocks for GET /meta/freq-axis and POST /meta/interpolate-spectrum so
//   the editor's server preview resolves without a backend. Scene mutations are driven through the DEV
//   `window.__enviTest` bridge (the same store paths a finished Terra Draw edit uses).
// - Output assertions that: a semi-transparent screen renders the `warn` state without a spectrum and the
//   `info` (acoustic-screen) state once one is assigned; the editor plots authored anchors on the exact
//   octave band indices (4, 16, … 100) with a server preview line; switching an octave spectrum to 1/12
//   PROMOTES (visible notice + resolution twelfth); and a façade override survives a vertex insert
//   ELSEWHERE in the ring (the override edge keeps its UUID, spectrum, and geometric segment).

import { expect, test } from "@playwright/test";

import { bootOffline, installMetaMocks } from "./_mocks";

test("semi-transparent screen: warn without a spectrum, info (acoustic screen) once assigned", async ({ page }) => {
  const unmocked = await bootOffline(page);
  await installMetaMocks(page);

  // Place a wall/screen (selected on commit) and mark it semi-transparent.
  const wallId = await page.evaluate(() => window.__enviTest.commit("wall", 4.9, 52.36));
  await expect(page.getByTestId("inspector-wall")).toBeVisible();
  await page.getByTestId("wall-semitransparent").check();

  // Without a spectrum → the warn state (severity, not a kind hue).
  await expect(page.getByTestId("wall-treatment")).toHaveAttribute("data-treatment", "warn");

  // Assign a spectrum via the editor (pick a resolution → an authored spectrum exists for the wall).
  await page.getByTestId("wall-edit-spectrum").click();
  await expect(page.getByTestId("spectrum-editor")).toBeVisible();
  await page.getByTestId("spectrum-res-octave").click();
  await page.getByTestId("spectrum-close").click();

  // With a spectrum → the info (acoustic-screen / double-stroke) state.
  await expect(page.getByTestId("wall-treatment")).toHaveAttribute("data-treatment", "info");
  expect(await page.evaluate((id) => window.__enviTest.spectrum(id), wallId)).not.toBeNull();
  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});

test("editor: octave anchors land on exact band indices, server preview renders, promote-to-twelfth is explicit", async ({
  page,
}) => {
  const unmocked = await bootOffline(page);
  await installMetaMocks(page);

  // A source, selected → open its sound-power editor.
  await page.evaluate(() => window.__enviTest.commit("source", 4.91, 52.37));
  await page.getByTestId("source-edit-spectrum").click();
  await expect(page.getByTestId("spectrum-editor")).toBeVisible();

  // Author at 1/1-octave: the 9 anchors land on band indices 4, 16, …, 100 — never a non-octave index.
  await page.getByTestId("spectrum-res-octave").click();
  await expect(page.getByTestId("spectrum-editor")).toHaveAttribute("data-resolution", "octave");
  for (const idx of [4, 16, 28, 40, 52, 64, 76, 88, 100]) {
    await expect(page.getByTestId(`spectrum-anchor-${idx}`)).toHaveCount(1);
  }
  await expect(page.getByTestId("spectrum-anchor-5")).toHaveCount(0); // 5 is NOT an octave centre
  // The server preview line rendered (interpolate mock resolved).
  await expect(page.locator('[data-testid="spectrum-curve"] polyline')).toHaveCount(1);

  // Edit an octave anchor (a plain edit — no promotion yet).
  await page.getByTestId("spectrum-cell-64").fill("33");
  await expect(page.getByTestId("spectrum-promote-notice")).toHaveCount(0);

  // Switch to 1/12 → PROMOTES explicitly: the notice appears and the authored resolution is twelfth.
  await page.getByTestId("spectrum-res-twelfth").click();
  await expect(page.getByTestId("spectrum-promote-notice")).toBeVisible();
  await expect(page.getByTestId("spectrum-editor")).toHaveAttribute("data-resolution", "twelfth");
  // Now every 1/12 band is individually editable — edit one.
  await page.getByTestId("spectrum-cell-5").fill("41");
  await expect(page.getByTestId("spectrum-cell-5")).toHaveValue("41");

  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});

test("façade override survives a vertex insert ELSEWHERE in the ring (D-02, no silent re-point)", async ({
  page,
}) => {
  const unmocked = await bootOffline(page);
  await installMetaMocks(page);

  // A building gets per-edge UUIDs at draw time.
  const buildingId = await page.evaluate(() => window.__enviTest.commit("building", 4.92, 52.38));
  const edges = await page.evaluate((id) => window.__enviTest.buildingEdges(id), buildingId);
  expect(edges.length).toBeGreaterThanOrEqual(3);

  // Override the LAST edge (the wrap edge v2→v0) and record its geometric segment.
  const overrideEdge = edges[edges.length - 1];
  const before = await page.evaluate(
    ([id, e]) => {
      window.__enviTest.setSpectrum(e, { resolution: "octave", values: [30, 30, 30, 30, 30, 30, 30, 30, 30] });
      return window.__enviTest.edgeSegment(id, e);
    },
    [buildingId, overrideEdge] as const,
  );
  expect(before).not.toBeNull();

  // Insert a vertex on a DIFFERENT edge (between v0 and v1) and reconcile via the store ring-diff.
  const after = await page.evaluate(
    ([id, e]) => {
      const seg0 = window.__enviTest.edgeSegment(id, window.__enviTest.buildingEdges(id)[0]); // v0 → v1
      const seg1 = window.__enviTest.edgeSegment(id, window.__enviTest.buildingEdges(id)[1]); // v1 → v2
      const v0 = seg0!.from;
      const v1 = seg0!.to;
      const v2 = seg1!.to;
      const w: [number, number] = [(v0[0] + v1[0]) / 2, (v0[1] + v1[1]) / 2]; // midpoint of the v0→v1 edge
      // A closed ring with w inserted between v0 and v1 (elsewhere than the overridden v2→v0 edge).
      window.__enviTest.applyBuildingRing(id, [v0, w, v1, v2, v0]);
      return {
        edges: window.__enviTest.buildingEdges(id),
        spectrum: window.__enviTest.spectrum(e),
        segment: window.__enviTest.edgeSegment(id, e),
      };
    },
    [buildingId, overrideEdge] as const,
  );

  // The override edge kept its UUID, its spectrum, and its geometric segment — it did NOT re-point.
  expect(after.edges).toContain(overrideEdge);
  expect(after.edges.length).toBe(edges.length + 1); // one edge split into two
  expect(after.spectrum).toEqual({ resolution: "octave", values: [30, 30, 30, 30, 30, 30, 30, 30, 30] });
  expect(after.segment).toEqual(before);

  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});
