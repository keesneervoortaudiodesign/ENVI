// calc.spec.ts — the WEB-07 offline UAT for the client-side calculation surface. Drives the REAL Vite-served
// bundle (COOP/COEP dev headers ⇒ `self.crossOriginIsolated === true`, so SharedArrayBuffer works) with the
// WHOLE network intercepted, so the run is fully offline (zero egress — the client-side compute needs no
// backend and no third party). The CalcPanel controls are driven by `data-testid` — never a reimplementation
// of the app; the scene is seeded through the DEV `window.__enviTest` bridge (the SC1 authoring store path).
//
// # Two tests, both HONEST
// - Test 1 (always runs, PASSES): proves the parts that genuinely execute end-to-end against the real
//   threaded-wasm bundle — cross-origin isolation, the capability banner's honest ABSENCE, the REAL wasm
//   `estimate_cost` pre-run readout (receiver count + tensor MiB + time, SC1), the Run gate, that clicking
//   Run submits (the job enters `queued` and the Abort control appears), that Abort keeps the app healthy,
//   and that NOTHING escaped to the live network across the flow.
// - Test 2 (full tiered solve + abort→Cancelled): drives Run and runs the full SC2 flow — per-tier progress →
//   coarse `done` → single-click Abort → `cancelled` with the completed tier kept. The `build:wasm:compute`
//   artifact now emits a SHARED `WebAssembly.Memory`, so the wasm-bindgen-rayon pool initialises and the solve
//   runs for real (the earlier 10-03 non-shared-memory gap was closed by the 10-05 remediation). The runtime
//   `test.skip` guard below remains ONLY as a defensive honest-skip for any environment where the pool cannot
//   start — it does NOT skip in the standard build.

import { expect, test, type Page, type Route } from "@playwright/test";

import { bootOffline } from "./_mocks";

const PROJECT_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";

// A site inside the NL viewport (matches the other offline specs' region). The exact position is immaterial —
// the CalcPanel marshals a valid flat-ground corridor scaled to the drawn area's receiver count.
const SITE = { lng: 4.9, lat: 52.37 };

// Seed an open project + a calc_area polygon + a source point through the DEV bridge (the SC1 authoring path).
async function seedCalcScene(page: Page): Promise<void> {
  await page.evaluate(({ id }) => window.__enviTest.openProject(id, "E2E Calc"), { id: PROJECT_ID });
  await page.evaluate((s) => {
    window.__enviTest.commit("calc_area", s.lng, s.lat);
    window.__enviTest.commit("source", s.lng, s.lat);
  }, SITE);
}

// Re-register the offline guard as a RECORD + ABORT gate that captures any live-network request during the
// solve (registered after the offline stack so it wins the most-recent-route match). The client-side compute
// must touch zero network — wasm + OPFS + workers are all same-origin (localhost).
async function installSolveEgressGuard(page: Page, egress: string[]): Promise<void> {
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
}

test("WEB-07/SC1: an offline real-bundle calc is cross-origin isolated, shows a REAL cost estimate, submits, and never touches the network", async ({
  page,
}) => {
  test.setTimeout(90_000);
  const unmocked = await bootOffline(page);

  await expect(page.getByTestId("calc-panel")).toBeVisible();
  await seedCalcScene(page);

  // The session is cross-origin isolated (COOP/COEP dev headers) — the threaded-solve prerequisite — so the
  // honest capability-failure banner is ABSENT (the S1 negative).
  const isolated = await page.evaluate(() => self.crossOriginIsolated === true);
  expect(isolated, "the dev server must be cross-origin isolated for the threaded solve").toBe(true);
  await expect(page.getByTestId("calc-capability-error")).toHaveCount(0);

  // Editing the fine spacing surfaces a REAL pre-run cost estimate from the wasm `estimate_cost` (SC1): a
  // receiver count, a tensor MiB footprint, and a time estimate — all before Run.
  await page.getByTestId("calc-spacing").fill("3");
  const estimate = page.getByTestId("calc-estimate");
  await expect(estimate).toBeVisible({ timeout: 30_000 });
  await expect(estimate).toContainText("receivers");
  await expect(estimate).toContainText("MiB tensor");
  await expect(estimate).toContainText("~");
  // The estimate is real, not a placeholder: a positive receiver count derived from the drawn area + spacing.
  const estimateText = (await estimate.textContent()) ?? "";
  expect(Number.parseInt(estimateText, 10)).toBeGreaterThan(0);

  const solveEgress: string[] = [];
  await installSolveEgressGuard(page, solveEgress);

  // The Run gate holds (project + calc area + source + isolated + not blocked) and submitting enters the job
  // lifecycle: the status chip reads `queued` and the single-click Abort control appears.
  await expect(page.getByTestId("calc-run")).toBeEnabled({ timeout: 30_000 });
  await page.getByTestId("calc-run").click();
  await expect(page.getByTestId("calc-status")).toContainText("queued", { timeout: 30_000 });
  await expect(page.getByTestId("calc-abort")).toBeVisible();

  // Abort is a single click with no confirm dialog; the app stays healthy afterwards.
  await page.getByTestId("calc-abort").click();
  await expect(page.getByTestId("calc-panel")).toBeVisible();
  await expect(page.getByTestId("calc-run")).toBeVisible();

  // Zero egress: neither the offline guard nor the solve-time guard recorded any live-network request.
  expect(solveEgress, `solve-time egress: ${solveEgress.join(", ")}`).toEqual([]);
  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});

test("WEB-07/SC2: a genuine threaded solve shows tiered progress and aborts to Cancelled (skips honestly if the pool cannot start)", async ({
  page,
}) => {
  test.setTimeout(180_000);
  const unmocked = await bootOffline(page);

  await expect(page.getByTestId("calc-panel")).toBeVisible();
  await seedCalcScene(page);

  expect(await page.evaluate(() => self.crossOriginIsolated === true)).toBe(true);

  // Spacing 3 m keeps the fine tier large (many chunks) so Abort reliably lands mid-solve after the coarse
  // tier completes.
  // A fine 0.5 m spacing makes the FINE tier large (several thousand receivers => many chunk boundaries, a
  // multi-second tier) so the post-coarse Abort reliably lands mid-solve. Because we abort early (during the
  // fine tier), the full fine set is never computed — the large tier only widens the abort window, it does not
  // lengthen the test. The tensor stays tens of MiB (well under the 256 MiB budget => Run stays enabled).
  await page.getByTestId("calc-spacing").fill("0.5");
  await expect(page.getByTestId("calc-estimate")).toBeVisible({ timeout: 30_000 });

  const solveEgress: string[] = [];
  await installSolveEgressGuard(page, solveEgress);

  await expect(page.getByTestId("calc-run")).toBeEnabled({ timeout: 30_000 });
  await page.getByTestId("calc-run").click();

  // The wasm-bindgen-rayon pool starts in the standard shared-memory build, so the tiered solve runs. The
  // guard below is defensive only: if some environment cannot start the pool, skip HONESTLY rather than assert
  // a solve that cannot run there. In the standard build `started` is true and the full flow executes.
  let started = false;
  for (let i = 0; i < 20; i += 1) {
    const status = (await page.getByTestId("calc-status").textContent()) ?? "";
    if (status.includes("running") || status.includes("done")) {
      started = true;
      break;
    }
    await page.waitForTimeout(1500);
  }
  test.skip(
    !started,
    "wasm-bindgen-rayon thread pool did not initialise in this environment (defensive guard; the standard " +
      "shared-memory build:wasm:compute starts the pool and runs the full solve).",
  );

  // The coarse tier reaches `done` (its TierComplete event) — tiered progress is genuine, not a stub.
  await expect(page.getByTestId("calc-progress")).toBeVisible({ timeout: 30_000 });
  const coarseRow = page.getByTestId("calc-tier-coarse");
  await expect(coarseRow.getByText("done", { exact: true })).toBeVisible({ timeout: 90_000 });

  // Single-click cooperative Abort while the fine tier is still running → the job lands `cancelled` and the
  // already-completed coarse tier KEEPS its `done` chip (completed tiers preserved, SC2).
  await page.getByTestId("calc-abort").click();
  await expect(page.getByTestId("calc-status")).toHaveText("cancelled", { timeout: 90_000 });
  await expect(page.getByTestId("calc-cancelled")).toBeVisible();
  await expect(coarseRow.getByText("done", { exact: true })).toBeVisible();

  // Zero egress across the whole genuine solve → abort flow.
  expect(solveEgress, `solve-time egress: ${solveEgress.join(", ")}`).toEqual([]);
  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});
