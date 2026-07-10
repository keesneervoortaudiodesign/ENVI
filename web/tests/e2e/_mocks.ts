// _mocks.ts — Playwright route interception so the ENVI E2E runs FULLY OFFLINE (D-13a, CLAUDE.md).
//
// # Module I/O
// - Input  a Playwright `Page`.
// - Output three installers: `installApiMocks` (fulfills every same-origin `/api/*` request — the
//   house metrao3 pattern), `installBasemapMocks` (intercepts the OpenFreeMap style/tiles/glyphs so no
//   request reaches `tiles.openfreemap.org`), and `installOfflineGuard` (aborts + records ANY other
//   external request so a silent network hit fails the test). Register the guard FIRST and the specific
//   mocks AFTER — Playwright matches the most-recently-registered route first, so the specific mocks win.
// - Valid input range: localhost (the Vite-served bundle), data: and blob: URLs are always allowed.

import type { Page, Route } from "@playwright/test";

// A minimal dark vector style — background only, NO external sources — so intercepting the style URL is
// sufficient: the map never requests tiles, glyphs, or sprites. Firing `load`/`style.load` is all the
// Gate-1 lifecycle test needs.
const STUB_DARK_STYLE = {
  version: 8,
  name: "envi-e2e-stub-dark",
  sources: {},
  layers: [{ id: "background", type: "background", paint: { "background-color": "#0b0d10" } }],
};

// Intercept the OpenFreeMap basemap surface. The style JSON is replaced with the source-less stub; the
// tile/glyph/sprite patterns are covered defensively in case a style variant references them.
export async function installBasemapMocks(page: Page): Promise<void> {
  await page.route(/openfreemap\.org\/styles\//, (route: Route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(STUB_DARK_STYLE),
    }),
  );
  // Defensive: any tile/glyph/sprite fetch (should never happen with the source-less stub) → empty 200.
  await page.route(/openfreemap\.org\/.*\.(pbf|png|json)/, (route: Route) =>
    route.fulfill({ status: 200, contentType: "application/octet-stream", body: "" }),
  );
}

// Fulfill same-origin /api/* requests. This plan's map surface makes no API calls yet, but the house
// pattern installs the API layer so later specs (scene GET/PUT, freq-axis) inherit it. Unknown /api
// routes return an empty JSON object rather than hitting a backend.
export async function installApiMocks(page: Page): Promise<void> {
  await page.route("**/api/**", (route: Route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: "{}" }),
  );
}

// The offline invariant: allow localhost/data/blob, abort everything else and record it so the test can
// assert nothing unexpected touched the network. Register this BEFORE the specific mocks above.
export function installOfflineGuard(page: Page, onUnmocked: (url: string) => void): Promise<void> {
  return page.route("**/*", (route: Route) => {
    const url = route.request().url();
    if (
      url.startsWith("http://localhost") ||
      url.startsWith("http://127.0.0.1") ||
      url.startsWith("data:") ||
      url.startsWith("blob:")
    ) {
      return route.continue();
    }
    onUnmocked(url);
    return route.abort();
  });
}

// Install the full offline stack in the correct order (guard first, then specific mocks) and return the
// collector of any unmocked external URLs (must stay empty for an offline-clean run).
export async function installOffline(page: Page): Promise<string[]> {
  const unmocked: string[] = [];
  await installOfflineGuard(page, (url) => unmocked.push(url));
  await installBasemapMocks(page);
  await installApiMocks(page);
  return unmocked;
}
