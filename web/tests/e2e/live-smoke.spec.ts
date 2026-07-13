// live-smoke.spec.ts — the ONE spec that is allowed to touch the live network. OPT-IN ONLY.
//
//     ENVI_E2E_LIVE=1 npx playwright test live-smoke
//
// Everything else in this suite is hermetic: `full-journey.spec.ts` and the focused specs serve the
// committed fixtures through `page.route` and assert ZERO egress (the CLAUDE.md rule — offline, no
// credentials, deterministic). That hermetic discipline has one blind spot, and this spec is aimed
// squarely at it: if ESA/PDOK/Overpass/Open-Meteo/OpenFreeMap changed a URL, a response shape, or a
// required parameter, every mocked spec would stay green while the real app was broken in the browser.
// Fixtures prove we parse what we recorded; only a live call proves we still ASK the right question.
//
// So this spec deliberately trades determinism for truth. It is:
//   - skipped by default (never runs in a normal `npm run test:e2e`), so the default suite stays
//     offline, fast and reliable;
//   - deliberately SHALLOW — it checks the integration seams (does the request shape still work, does
//     the response still derive), NOT the physics or the UI logic, which the offline specs own;
//   - tolerant of nothing. If a live source answers, it must answer usefully. A 200 carrying an
//     unparseable body is a failure, not a pass.
//
// Requires: a working internet connection. No credentials — every source used here is keyless
// (Open-Meteo free tier, OpenFreeMap MIT basemap). If it fails, read the failure before assuming
// flakiness: a changed upstream contract looks exactly like a flake, and is the thing we are hunting.
//
// # Module I/O
// - Input  the REAL Vite-served bundle with ONLY the same-origin `/api/v1` surface stubbed (the Rust
//   backend does not run under the Playwright webServer — see playwright.config.ts). Every
//   cross-origin request goes to the real internet.
// - Output (1) the live OpenFreeMap basemap style + tiles load and the map reaches `ready`;
//          (2) a live Open-Meteo pressure-level fetch derives a real per-azimuth A/B/C table.
//   Both record the hosts actually contacted, so a silent switch to a cached/mocked path fails.

import { expect, test } from "@playwright/test";

import { installApiMocks, installMetaMocks } from "./_mocks";

const LIVE = process.env.ENVI_E2E_LIVE === "1";
const PROJECT_ID = "11ee0000-0000-4000-8000-00000000live";

// Skip the whole file unless explicitly opted in. `describe.skip` (not a per-test guard) keeps the
// default `npm run test:e2e` run completely free of live network calls.
test.describe(LIVE ? "live smoke (real network)" : "live smoke (SKIPPED — set ENVI_E2E_LIVE=1)", () => {
  test.skip(!LIVE, "live-network smoke is opt-in: run with ENVI_E2E_LIVE=1");

  // Live calls cross the public internet; be patient before calling it a failure.
  test.setTimeout(120_000);

  test("the REAL OpenFreeMap basemap loads and renders", async ({ page }) => {
    const hosts = new Set<string>();
    page.on("request", (req) => {
      const { hostname } = new URL(req.url());
      if (hostname !== "localhost" && hostname !== "127.0.0.1") hosts.add(hostname);
    });

    // Stub ONLY the same-origin backend (it does not run here). The basemap goes to the real CDN.
    await installApiMocks(page);
    await page.goto("/");

    await expect(page.getByTestId("map-ready")).toHaveText("yes", { timeout: 60_000 });
    await expect(page.getByTestId("map-canvas")).toBeVisible();

    // The style really came from OpenFreeMap — if this is empty, something silently served a stub and
    // this test would otherwise be a meaningless green.
    expect(
      [...hosts].some((h) => h.includes("openfreemap.org")),
      `the live basemap host was never contacted; hosts seen: ${[...hosts].join(", ") || "none"}`,
    ).toBe(true);
  });

  test("a REAL Open-Meteo fetch derives a per-azimuth A/B/C table", async ({ page }) => {
    const hosts = new Set<string>();
    const failures: string[] = [];
    page.on("request", (req) => {
      const { hostname } = new URL(req.url());
      if (hostname !== "localhost" && hostname !== "127.0.0.1") hosts.add(hostname);
    });
    page.on("requestfailed", (req) => {
      if (req.url().includes("open-meteo")) {
        failures.push(`${req.url()} — ${req.failure()?.errorText ?? "unknown"}`);
      }
    });

    await installApiMocks(page);
    // The spectrum/freq-axis meta endpoints are same-origin backend routes; keep them stubbed so the
    // panel renders. The weather fetch itself is NOT stubbed — it goes to the real Open-Meteo API.
    await installMetaMocks(page);
    await page.goto("/");
    await page.waitForFunction(() => typeof window.__enviTest !== "undefined");

    await expect(page.getByTestId("weather-panel")).toBeVisible();
    await page.evaluate(
      ({ id }) => window.__enviTest.openProject(id, "E2E Live Smoke"),
      { id: PROJECT_ID },
    );
    await expect(page.getByTestId("weather-import")).toBeEnabled({ timeout: 30_000 });

    // A recent past date: safely inside the Forecast product's window and always populated (unlike a
    // future date near the model's edge). This is the request the real app builds — host, params and all.
    const date = new Date(Date.now() - 2 * 24 * 3600 * 1000).toISOString().slice(0, 10);
    await page.getByTestId("weather-date").fill(date);
    await page.getByTestId("weather-hour").fill("12");
    await page.getByTestId("weather-import").click();

    // The LIVE response must parse and derive. A changed Open-Meteo schema lands here as a hard failure
    // — which is the entire point of this spec.
    await expect(
      page.getByTestId("weather-status"),
      `Open-Meteo did not derive. Request failures: ${failures.join("; ") || "none"} — if the body ` +
        `arrived but did not parse, the upstream schema likely changed (update the committed fixtures).`,
    ).toHaveText("derived", { timeout: 60_000 });

    // A real per-azimuth A/B/C table came out of a real atmosphere.
    await expect(page.getByTestId("weather-abc-table")).toBeVisible();
    for (const az of [0, 90, 180, 270]) {
      await expect(page.getByTestId(`weather-abc-${az}`)).toBeVisible();
    }
    await expect(page.getByTestId("weather-cache")).toHaveText("network fetch");

    expect(
      [...hosts].some((h) => h.includes("open-meteo.com")),
      `the live Open-Meteo host was never contacted; hosts seen: ${[...hosts].join(", ") || "none"}`,
    ).toBe(true);
  });
});
