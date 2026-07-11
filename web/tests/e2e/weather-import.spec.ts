// weather-import.spec.ts — the METX-01 / SC4 offline weather-import journey + zero-egress what-if proof.
//
// # Module I/O
// - Input  the REAL Vite-served bundle with the whole network intercepted: the offline guard + basemap +
//   generic `/api/*` mocks (`bootOffline`), then `installWeatherMocks` serving the committed synthetic
//   Open-Meteo pressure-level fixtures for BOTH date-switched hosts (Forecast `api.open-meteo.com` +
//   Archive `archive-api.open-meteo.com`). The real `WeatherPanel` controls are driven by `data-testid`
//   (date / hour / z₀ / Import) — never a reimplementation of the app; only the two third-party hosts +
//   `/api` are mocked via `page.route`.
// - Output the load-bearing METX-01 / SC4 assertions:
//   (1) an online import fetches the correct date-switched product, derives the per-azimuth A/B/C (shown in
//       the panel), and logs + shows the call-cost weight;
//   (2) the SC4 zero-egress property — after the cached import, the network is INVERTED (both Open-Meteo
//       hosts re-registered to RECORD + ABORT) and a what-if edit (change z₀ → re-Import) re-derives A/B/C
//       from OPFS with ZERO Open-Meteo calls (the egress collector stays [] and the "OPFS cache" chip shows);
//   (3) both date-switch branches (Forecast for a recent date, Archive for a historical date) are exercised.
//   Every test ends with the offline-clean `expect(unmocked, ...).toEqual([])` — nothing escaped to the live
//   network (no credentials, no live Open-Meteo/CDS connection).

import { expect, test, type Route } from "@playwright/test";

import { bootOffline, installWeatherMocks } from "./_mocks";

const PROJECT_ID = "33333333-3333-4333-8333-333333333333";

// A date clearly inside the Forecast window (a couple of days ahead ⇒ requested > today − cutoff ⇒ Forecast).
function forecastDate(): string {
  return new Date(Date.now() + 2 * 24 * 3600 * 1000).toISOString().slice(0, 10);
}
// A date clearly inside the Archive window (years ago ⇒ requested ≤ today − cutoff ⇒ Archive/ERA5).
const ARCHIVE_DATE = "2024-01-15";

// Open a project through the DEV bridge and wait for the WeatherPanel Import to arm (the site lat/lon comes
// from the map viewport, written on map load — so an armed button proves both project + viewport are ready).
async function openProjectAndArm(page: import("@playwright/test").Page, name: string): Promise<void> {
  await expect(page.getByTestId("weather-panel")).toBeVisible();
  await page.evaluate(
    ({ id, projectName }) => window.__enviTest.openProject(id, projectName),
    { id: PROJECT_ID, projectName: name },
  );
  await expect(page.getByTestId("weather-import")).toBeEnabled({ timeout: 20_000 });
}

// Drive the panel's date/hour picker + Import, then wait for the derived state.
async function importWeather(
  page: import("@playwright/test").Page,
  date: string,
  hour: number,
): Promise<void> {
  await page.getByTestId("weather-date").fill(date);
  await page.getByTestId("weather-hour").fill(String(hour));
  await page.getByTestId("weather-import").click();
  await expect(page.getByTestId("weather-status")).toHaveText("derived", { timeout: 20_000 });
}

// The A coefficient rendered for a display azimuth (the 2nd cell of that azimuth's row).
async function readA(page: import("@playwright/test").Page, azimuth: number): Promise<string> {
  return (await page.getByTestId(`weather-abc-${azimuth}`).locator("td").nth(1).textContent()) ?? "";
}

test("METX-01/SC4: offline Forecast import derives per-azimuth A/B/C; a what-if edit issues zero Open-Meteo calls", async ({
  page,
}) => {
  const unmocked = await bootOffline(page);
  const weather = await installWeatherMocks(page);

  // Collect the browser console so the visible call-cost log line can be asserted (SC4 budget visibility).
  const logs: string[] = [];
  page.on("console", (msg) => logs.push(msg.text()));

  await openProjectAndArm(page, "E2E Weather");

  // --- Phase 1: an online import fetches the Forecast product and derives per-azimuth A/B/C. ---
  await importWeather(page, forecastDate(), 12);

  // The date-switch picked the Forecast host (recent date) exactly once; the Archive host was never hit.
  expect(weather.counts.forecast).toBe(1);
  expect(weather.counts.archive).toBe(0);

  // Per-azimuth A/B/C renders for the 8 compass sectors (the WASM `derive_weather` output, not a stub).
  await expect(page.getByTestId("weather-abc-table")).toBeVisible();
  await expect(page.getByTestId("weather-abc-0")).toBeVisible();
  await expect(page.getByTestId("weather-abc-90")).toBeVisible();
  await expect(page.getByTestId("weather-abc-270")).toBeVisible();

  // This first import was a network fetch (not a cache hit) — the cache chip + call-cost line say so.
  await expect(page.getByTestId("weather-cache")).toHaveText("network fetch");
  await expect(page.getByTestId("weather-callcost")).toBeVisible();
  await expect(page.getByTestId("weather-callcost")).toContainText("call-cost weight");
  // The call-cost weight was also LOGGED once (the visible Open-Meteo budget spend).
  const costLogs = logs.filter((l) => /Open-Meteo fetch call-cost weight/.test(l));
  expect(costLogs.length).toBe(1);

  // Capture the downwind (az 90) A before the what-if so we can prove the re-derivation actually ran.
  const aBefore = await readA(page, 90);
  const logsBeforeWhatIf = logs.length;

  // --- Phase 2 (SC4): invert the network. Re-register BOTH Open-Meteo hosts (and the proxy fallback) to
  // RECORD + ABORT — these win the most-recent-route match, so any Open-Meteo fetch now both fails AND is
  // recorded. A what-if edit MUST stay entirely on the OPFS cache (zero egress). ---
  const omEgress: string[] = [];
  const offRoute = (route: Route): Promise<void> => {
    omEgress.push(route.request().url());
    return route.abort();
  };
  await page.route(/\/\/api\.open-meteo\.com\//, offRoute);
  await page.route(/archive-api\.open-meteo\.com\//, offRoute);
  await page.route(/\/api\/v1\/proxy\/openmeteo-/, offRoute);

  // The what-if edit: change z₀ (a roughness what-if) and re-Import for the SAME (site, date, hour). The
  // weather body is served from OPFS (cache hit ⇒ no network) and A/B/C is re-derived in WASM at the new z₀.
  await page.getByTestId("weather-z0").fill("0.5");
  await page.getByTestId("weather-import").click();
  await expect(page.getByTestId("weather-status")).toHaveText("derived", { timeout: 20_000 });

  // The re-import was served from OPFS — the cache chip flips to the zero-call state (SC4).
  await expect(page.getByTestId("weather-cache")).toHaveText("OPFS cache (no call)");

  // The PRIMARY SC4 property: the what-if edit made ZERO Open-Meteo calls (nothing escaped, nothing aborted).
  expect(omEgress, `Open-Meteo egress on the what-if path: ${omEgress.join(", ")}`).toEqual([]);
  // The product counters are unchanged — no second fetch of either host.
  expect(weather.counts.forecast).toBe(1);
  expect(weather.counts.archive).toBe(0);
  // No new call-cost line was logged on the cache-hit what-if (a cache hit spends zero budget).
  const costLogsAfter = logs.filter((l) => /Open-Meteo fetch call-cost weight/.test(l));
  expect(costLogsAfter.length).toBe(1);
  expect(logs.length).toBeGreaterThanOrEqual(logsBeforeWhatIf); // sanity: the console listener stayed live

  // The re-derivation genuinely ran on the cached body: the downwind A changed with the new z₀ (the log
  // basis `ln(z/z₀+1)` shifts a_wind) — proving the what-if re-computed A/B/C, it did not just replay a chip.
  const aAfter = await readA(page, 90);
  expect(aAfter).not.toBe(aBefore);

  // The offline guard never fired: no basemap/GIS/Open-Meteo request ever escaped to the live network.
  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});

test("METX-01: a historical date hits the Archive (ERA5) product, fully offline (date-switch branch)", async ({
  page,
}) => {
  const unmocked = await bootOffline(page);
  const weather = await installWeatherMocks(page);

  await openProjectAndArm(page, "E2E Weather Archive");

  // A historical date date-switches to the Archive host (the other D-02 branch).
  await importWeather(page, ARCHIVE_DATE, 6);

  expect(weather.counts.archive).toBe(1);
  expect(weather.counts.forecast).toBe(0);

  // Per-azimuth A/B/C renders + the call-cost line is visible for the Archive product too.
  await expect(page.getByTestId("weather-abc-table")).toBeVisible();
  await expect(page.getByTestId("weather-abc-180")).toBeVisible();
  await expect(page.getByTestId("weather-cache")).toHaveText("network fetch");
  await expect(page.getByTestId("weather-callcost")).toContainText("call-cost weight");

  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});
