// object-heights.spec.ts — the inspector HEIGHT surface, fully offline (D-13a): a building's own eaves
// height + its DATA-03 provenance, and a wall's screen height.
//
// # Module I/O
// - Input  the REAL Vite-served bundle with the basemap + `/api/v1` surface route-intercepted
//   (`bootOffline`). Scene mutations run through the DEV `window.__enviTest` bridge (the same store paths a
//   finished Terra Draw edit takes) — headless WebGL drawing is unreliable.
// - Output assertions a BROKEN implementation fails:
//     * two buildings with DIFFERENT heights each show their OWN value in the inspector — an inspector that
//       seeded the field from `KIND_DEFAULTS` (the bug that made 4718 imported buildings "all the same
//       height") would show one identical value for both and fail here;
//     * an imported building's `height_provenance` tier is DISPLAYED (the guessed `default` tier reads as a
//       warn chip, so the user can see the height was assumed, not known);
//     * typing a height in the inspector COMMITS to the store (and re-tags provenance as user-edited),
//       while a negative / absurd entry is REFUSED and never reaches the store;
//     * a wall carries an editable `height_m` — without one it can never act as a noise screen.
//   Nothing reaches the live network.

import { expect, test, type Page } from "@playwright/test";

import { bootOffline } from "./_mocks";

// Commit a building of `kind` at [lng, lat] and give it an explicit height + provenance, bypassing the
// inspector — this models an IMPORTED feature (the store's `loadImportedScene` preserves such properties
// verbatim) so the inspector is proven to read the FEATURE, not a kind default.
async function seedBuilding(
  page: Page,
  lng: number,
  lat: number,
  eaves: number,
  provenance: string,
): Promise<string> {
  return page.evaluate(
    ([lngV, latV, eavesV, prov]) => {
      const id = window.__enviTest.commit("building", lngV as number, latV as number);
      window.__enviTest.update(id, {
        eaves_height_m: eavesV as number,
        height_provenance: prov as string,
        imported: true,
      });
      return id;
    },
    [lng, lat, eaves, provenance] as const,
  );
}

test("building inspector shows each building's OWN eaves height + its provenance, and commits an edit", async ({
  page,
}) => {
  const unmocked = await bootOffline(page);

  // Two buildings with genuinely DIFFERENT heights and DIFFERENT provenance tiers (the two ends of the
  // import fallback ladder: a real OSM height tag vs a guessed default).
  const tall = await seedBuilding(page, 4.9, 52.36, 27.4, "height_tag");
  const short = await seedBuilding(page, 4.902, 52.362, 6.2, "default");

  // The second building is the current selection → the inspector must show ITS height, not the first's and
  // not the kind default.
  await expect(page.getByTestId("inspector-building")).toBeVisible();
  await expect(page.getByTestId("building-eaves-height")).toHaveValue("6.2");
  // …and WHERE that height came from: a guessed fallback, shown as a warn chip (DATA-03).
  const shortProv = page.getByTestId("building-eaves-height-provenance");
  await expect(shortProv).toHaveAttribute("data-provenance", "default");
  await expect(shortProv).toHaveClass(/warn/);
  await expect(shortProv).toContainText(/fallback|default/i);

  // Select the FIRST building: the inspector must switch to ITS value. An inspector seeded from
  // KIND_DEFAULTS would show the same number for both buildings and fail exactly here.
  await page.evaluate((id) => window.__enviTest.select(id), tall);
  await expect(page.getByTestId("building-eaves-height")).toHaveValue("27.4");
  await expect(page.getByTestId("building-eaves-height-provenance")).toHaveAttribute(
    "data-provenance",
    "height_tag",
  );

  // Editing the height commits through the normal store path (dirty + autosave) and re-tags provenance as
  // user-edited — an imported guess the operator corrected no longer claims to be an OSM tag.
  await page.getByTestId("building-eaves-height").fill("31.5");
  await expect
    .poll(() => page.evaluate((id) => window.__enviTest.properties(id)["eaves_height_m"], tall))
    .toBe(31.5);
  const props = await page.evaluate((id) => window.__enviTest.properties(id), tall);
  expect(props["height_provenance"]).toBe("user");
  expect(props["user_modified"]).toBe(true); // D-09: a re-import must not clobber the corrected height
  await expect(page.getByTestId("building-eaves-height-provenance")).toHaveAttribute(
    "data-provenance",
    "user",
  );

  // The OTHER building is untouched by that edit (per-feature values, not a shared constant).
  expect(await page.evaluate((id) => window.__enviTest.properties(id)["eaves_height_m"], short)).toBe(6.2);

  // A negative / absurd entry is REFUSED: it is flagged and never reaches the store (the last committed
  // height stands).
  for (const bad of ["-4", "9000"]) {
    await page.getByTestId("building-eaves-height").fill(bad);
    await expect(page.getByTestId("building-eaves-height-error")).toBeVisible();
    expect(await page.evaluate((id) => window.__enviTest.properties(id)["eaves_height_m"], tall)).toBe(31.5);
  }

  expect(unmocked, `unmocked network requests: ${unmocked.join(", ")}`).toEqual([]);
});

test("a wall carries an editable screen height that inherits to the next wall", async ({ page }) => {
  const unmocked = await bootOffline(page);

  // A first-of-kind wall starts from the documented default height (a height-less wall screens NOTHING —
  // the screening marshaller drops any wall without a finite `height_m`).
  const first = await page.evaluate(() => window.__enviTest.commit("wall", 4.91, 52.37));
  await expect(page.getByTestId("inspector-wall")).toBeVisible();
  const seeded = await page.evaluate((id) => window.__enviTest.properties(id)["height_m"], first);
  expect(typeof seeded).toBe("number");
  expect(seeded as number).toBeGreaterThan(0);
  await expect(page.getByTestId("wall-height")).toHaveValue(String(seeded));

  // Editing it commits to the store.
  await page.getByTestId("wall-height").fill("4.5");
  await expect
    .poll(() => page.evaluate((id) => window.__enviTest.properties(id)["height_m"], first))
    .toBe(4.5);

  // …and the NEXT wall inherits that height (WEB-04 last-object inheritance), shown with the inherited chip.
  const second = await page.evaluate(() => window.__enviTest.commit("wall", 4.912, 52.372));
  await expect(page.getByTestId("wall-height")).toHaveValue("4.5");
  await expect(page.getByTestId("inspector")).toContainText("inherited from last wall");
  expect(await page.evaluate((id) => window.__enviTest.properties(id)["height_m"], second)).toBe(4.5);

  // A rejected entry never reaches the store.
  await page.getByTestId("wall-height").fill("-1");
  await expect(page.getByTestId("wall-height-error")).toBeVisible();
  expect(await page.evaluate((id) => window.__enviTest.properties(id)["height_m"], second)).toBe(4.5);

  expect(unmocked, `unmocked network requests: ${unmocked.join(", ")}`).toEqual([]);
});
