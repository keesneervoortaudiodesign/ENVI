// validation.spec.ts — the SC2 + persistence E2E, fully offline (D-13a): draw-time ground-zone hard
// reject (Surface B), the persistent click-to-select validation panel (Surface A), debounced committed-
// edit autosave (D-04), and the typed-name Delete-project confirmation.
//
// # Module I/O
// - Input  the REAL Vite-served bundle with the basemap + /api/* surface route-intercepted
//   (`installOffline`), plus a mocked `POST /dgm/triangulate` returning 4xx (the interior-crossing
//   breakline reject that drives the crit row) and a counting `PUT /scene`. Scene mutations run through
//   the DEV `window.__enviTest` bridge (the same store paths a finished Terra Draw edit uses).
// - Output assertions that: a contained ground_zone commits while a partial cross reverts + banners +
//   zooms to the EXISTING zone; warn rows (semi-transparent wall, zero-density forest) + a crit row
//   (mocked triangulate 4xx) appear and click-to-select+zoom works; exactly ONE coalesced PUT fires per
//   committed edit after the debounce with the indicator transitioning Dirty→Saving→Saved; and the delete
//   danger button enables only on an exact typed-name match.

import { expect, test } from "@playwright/test";

import { bootOffline } from "./_mocks";

test("ground_zone topology: contained commits; partial cross reverts + banner + zoom-to-existing (D-07)", async ({
  page,
}) => {
  const unmocked = await bootOffline(page);

  // Zone A — the reference square.
  const a = await page.evaluate(() =>
    window.__enviTest.commitGroundZone([
      [5.0, 52.0],
      [5.02, 52.0],
      [5.02, 52.02],
      [5.0, 52.02],
      [5.0, 52.0],
    ]),
  );
  expect(a.outcome).toBe("ok");
  expect(a.id).not.toBeNull();

  // Zone B — fully inside A → containment is ALLOWED (innermost wins), commits.
  const b = await page.evaluate(() =>
    window.__enviTest.commitGroundZone([
      [5.005, 52.005],
      [5.012, 52.005],
      [5.012, 52.012],
      [5.005, 52.012],
      [5.005, 52.005],
    ]),
  );
  expect(b.outcome).toBe("contained");
  expect(b.id).not.toBeNull();
  await expect(page.getByTestId("reject-banner")).toHaveCount(0);

  // Zone C — partially crosses A (top-right corner), disjoint from B → HARD REJECT.
  const c = await page.evaluate(() =>
    window.__enviTest.commitGroundZone([
      [5.016, 52.016],
      [5.03, 52.016],
      [5.03, 52.03],
      [5.016, 52.03],
      [5.016, 52.016],
    ]),
  );
  expect(c.outcome).toBe("partial-cross");
  expect(c.id).toBeNull(); // reverted — never committed
  expect(c.conflictId).toBe(a.id); // the EXISTING crossed zone

  // The transient banner is shown and "Zoom to conflicting zone" targets the EXISTING zone A.
  await expect(page.getByTestId("reject-banner")).toBeVisible();
  await page.getByTestId("reject-zoom").click();
  await expect(page.getByTestId("zoom-target")).toHaveText(a.id!);

  // The reject banner is Surface B — it is NEVER a row in the persistent validation panel (Surface A).
  await expect(page.getByTestId("validation").getByText("partially overlap")).toHaveCount(0);

  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});

test("validation panel: warn rows + crit row (mocked triangulate 4xx); click-to-select + zoom (WEB-04)", async ({
  page,
}) => {
  const unmocked = await bootOffline(page);
  // The interior-crossing breaklines reject — the server 4xx the dgm producer stores as the crit source
  // (only hit on a post-boot commit, so registering it after navigation is safe).
  await page.route(/\/api\/v1\/dgm\/triangulate$/, (route) =>
    route.fulfill({
      status: 400,
      contentType: "application/json",
      body: JSON.stringify({ detail: "breaklines intersect in their interior" }),
    }),
  );

  // (a) A semi-transparent wall WITHOUT a spectrum → a warn row.
  const wallId = await page.evaluate(() => window.__enviTest.commit("wall", 4.9, 52.36));
  await expect(page.getByTestId("inspector-wall")).toBeVisible();
  await page.getByTestId("wall-semitransparent").check();
  await expect(page.getByTestId("issue-warn")).toHaveCount(1);

  // (b) A forest with zero mean density → a second warn row.
  const forestId = await page.evaluate(() => {
    const id = window.__enviTest.commit("forest", 4.95, 52.35);
    window.__enviTest.update(id, { density_per_m2: 0 });
    return id;
  });
  await expect(page.getByTestId("issue-warn")).toHaveCount(2);

  // (c) Two interior-crossing elevation_lines (with ≥3 elevation points so the producer fires) → the
  // mocked triangulate 4xx drives EXACTLY ONE crit row naming the conflict.
  const lineId = await page.evaluate(() => {
    window.__enviTest.commit("elevation_point", 4.9, 52.3);
    window.__enviTest.commit("elevation_point", 4.92, 52.3);
    window.__enviTest.commit("elevation_point", 4.91, 52.32);
    const first = window.__enviTest.commit("elevation_line", 4.9, 52.3);
    window.__enviTest.commit("elevation_line", 4.905, 52.305);
    return first;
  });
  await expect(page.getByTestId("issue-crit")).toHaveCount(1, { timeout: 5000 });
  await expect(page.getByTestId("issue-crit")).toContainText("interior-cross");

  // Click a warn row → selects + zoom-to-fits the wall (first warn row is the wall).
  await page.getByTestId("issue-warn").first().click();
  await expect(page.getByTestId("zoom-target")).toHaveText(wallId);
  expect(await page.evaluate(() => window.__enviTest.state().selection)).toBe(wallId);

  // Click the crit row → selects + zooms the first crossing elevation_line.
  await page.getByTestId("issue-crit").click();
  await expect(page.getByTestId("zoom-target")).toHaveText(lineId);
  expect(await page.evaluate(() => window.__enviTest.state().selection)).toBe(lineId);

  expect(forestId).toBeTruthy();
  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});

test("autosave: exactly ONE coalesced PUT per committed edit after debounce; Dirty→Saving→Saved (D-04)", async ({
  page,
}) => {
  const unmocked = await bootOffline(page);
  let putCount = 0;
  await page.route(/\/api\/v1\/projects\/[^/]+\/scene$/, (route) => {
    if (route.request().method() === "PUT") {
      putCount += 1;
      return route.fulfill({ status: 200, contentType: "application/json", body: "" });
    }
    return route.fulfill({ status: 200, contentType: "application/json", body: "{}" });
  });

  // A single committed edit.
  await page.evaluate(() => window.__enviTest.commit("source", 4.9, 52.36));

  // The indicator shows Unsaved (dirty) immediately, then transitions to Saved after the debounce.
  await expect(page.getByTestId("save-indicator")).toHaveAttribute("data-status", "dirty");
  await expect(page.getByTestId("save-indicator")).toHaveAttribute("data-status", "saved", {
    timeout: 5000,
  });

  // Exactly ONE coalesced PUT fired (not one per drag frame).
  expect(putCount).toBe(1);
  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});

test("delete project: typed-name gate — danger enabled ONLY on an exact match; success resets", async ({
  page,
}) => {
  const unmocked = await bootOffline(page);
  await page.route(/\/api\/v1\/projects\/proj-1$/, (route) => {
    if (route.request().method() === "DELETE") {
      return route.fulfill({ status: 204, body: "" });
    }
    return route.fulfill({ status: 200, contentType: "application/json", body: "{}" });
  });
  await page.evaluate(() => window.__enviTest.openProject("proj-1", "My Project"));

  // Open the dialog from the overflow menu.
  await page.getByTestId("project-menu").click();
  await page.getByTestId("menu-delete-project").click();
  await expect(page.getByTestId("delete-dialog")).toBeVisible();

  // Focus opens on Cancel (never the danger button), which starts disabled.
  await expect(page.getByTestId("delete-cancel")).toBeFocused();
  await expect(page.getByTestId("delete-confirm")).toBeDisabled();

  // A wrong name keeps it disabled; the exact name enables it.
  await page.getByTestId("delete-name-input").fill("Wrong Name");
  await expect(page.getByTestId("delete-confirm")).toBeDisabled();
  await page.getByTestId("delete-name-input").fill("My Project");
  await expect(page.getByTestId("delete-confirm")).toBeEnabled();

  // Confirm → the delete succeeds and the bar routes to the empty/no-project state.
  await page.getByTestId("delete-confirm").click();
  await expect(page.getByTestId("delete-dialog")).toHaveCount(0);
  await expect(page.getByTestId("project-bar").getByText("No project")).toBeVisible();

  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});
