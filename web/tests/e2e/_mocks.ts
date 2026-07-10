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

// Fulfill same-origin backend requests. Scoped to the real `/api/v1/` prefix — NOT a broad `**/api/**`,
// which would also swallow the dev server's own module requests for `src/api/*.ts` (the fetch client),
// serving them as `application/json` and breaking the ES module graph. Unknown `/api/v1` routes return
// an empty JSON object rather than hitting a backend; specs override specific routes for real payloads.
export async function installApiMocks(page: Page): Promise<void> {
  await page.route("**/api/v1/**", (route: Route) =>
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

// A valid 105-band 1/12-octave freq axis (band-index keyed; nominal Hz display-only). Built from the x/12
// grid so the spectrum editor's Hz tick labels resolve; assertions are always by band INDEX, never Hz. The
// 27 third-octave centres land on indices 0, 4, 8, …, 104 (the octave centres are the subset 4, 16, … 100).
export function freqAxisFixture(): unknown {
  const G = Math.pow(10, 3 / 10);
  const centres = Array.from({ length: 105 }, (_, i) => 1000 * Math.pow(G, (i - 64) / 12));
  const thirdIdx = Array.from({ length: 27 }, (_, k) => 4 * k);
  return {
    n_bands: 105,
    centres_hz: centres,
    third_octave_indices: thirdIdx,
    nominal_third_octave_hz: thirdIdx.map((i) => Math.round(centres[i])),
  };
}

// Install the two `/meta` endpoints the isolation/sound-power spectrum editor needs (D-05): the freq axis
// and the SERVER-owned interpolation preview. The interpolate mock echoes a deterministic dense [105] ramp
// so anchor/preview assertions are exact. Registered AFTER `installOffline` so these specific routes win.
export async function installMetaMocks(page: Page): Promise<void> {
  await page.route(/\/api\/v1\/meta\/freq-axis$/, (route: Route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(freqAxisFixture()) }),
  );
  await page.route(/\/api\/v1\/meta\/interpolate-spectrum$/, (route: Route) => {
    const r_db = Array.from({ length: 105 }, (_, i) => 20 + (i % 12));
    return route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ r_db }) });
  });
}

// A deterministic single-triangle TIN for a valid elevation set (three non-collinear points → one face).
const STUB_TIN = {
  vertices: [
    [4.9, 52.36, 0],
    [4.91, 52.36, 1],
    [4.905, 52.37, 2],
  ],
  triangles: [[0, 1, 2]],
};

// Install `POST /dgm/triangulate` as a DETERMINISTIC oracle of the SC1 re-triangulation contract: a valid
// point-only elevation set returns the single-triangle TIN (200), while a set carrying ≥2 breaklines models
// the server's interior-crossing rejection and returns a typed 4xx (the crit source the ValidationPanel
// surfaces). A no-op producer would never fire either, so both SC1 branches genuinely exercise the wire.
export async function installTriangulateMock(page: Page): Promise<void> {
  await page.route(/\/api\/v1\/dgm\/triangulate$/, (route: Route) => {
    const body = route.request().postDataJSON() as { breaklines?: unknown[] } | null;
    const breaklines = Array.isArray(body?.breaklines) ? body.breaklines : [];
    if (breaklines.length >= 2) {
      return route.fulfill({
        status: 400,
        contentType: "application/json",
        body: JSON.stringify({ detail: "breaklines intersect in their interior" }),
      });
    }
    return route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(STUB_TIN) });
  });
}
