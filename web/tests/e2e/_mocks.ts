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

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

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

// Boot the app fully offline: install the offline route stack (guard + basemap + /api mocks), navigate to
// the SPA, and wait for the DEV `window.__enviTest` bridge (installed via an async dynamic import) before a
// spec drives store commits. Returns the unmocked-request collector — it MUST stay empty for an offline run.
// Specs that need extra route mocks (meta/interpolate, triangulate, PUT capture) register them AFTER this
// call: those endpoints are only hit on post-boot user actions, and a later `page.route` wins the match.
export async function bootOffline(page: Page): Promise<string[]> {
  const unmocked = await installOffline(page);
  await page.goto("/");
  await page.waitForFunction(() => typeof window.__enviTest !== "undefined");
  return unmocked;
}

// --- GIS import mocks (08-08 offline import journey + DATA-04 replay) --------------------------------
//
// Serve the committed E2E fixtures (`fixtures/*.tif` + `overpass_buildings.json`) for EVERY GIS source the
// import path fetches: AHN via PDOK (cross-origin Direct), GLO-30 + WorldCover via the same-origin byte
// proxy (`/api/v1/proxy/**`), and OSM buildings via Overpass (Direct). Registered AFTER `installOffline`/
// `bootOffline` so each specific GIS route wins over the offline guard AND the generic `**/api/v1/**` mock
// (a proxy tile must return TIFF bytes, not the `{}` fallback) — the load-bearing most-recent-route ordering.
//
// Every route bumps a per-source counter so the DATA-04 replay can assert ZERO GIS egress after import
// (the empty unmocked collector proves nothing escaped; the counters prove the compute read hit OPFS, not
// the network). The Overpass route has a switchable 429 mode for the D-07 partial-failure/retry branch.

const FIXTURE = (name: string): Buffer =>
  readFileSync(fileURLToPath(new URL(`./fixtures/${name}`, import.meta.url)));

// Per-GIS-source request counters + a switchable Overpass mode, returned to the spec by `installGisMocks`.
export interface GisMockControl {
  readonly counts: {
    pdok: number;
    proxyGlo30: number;
    proxyWorldcover: number;
    overpass: number;
  };
  // Flip Overpass between a 200 (fixture JSON) and a 429 (rate-limited) response for the retry branch.
  setOverpassMode(mode: "ok" | "429"): void;
}

// Fulfill a whole-tile TIFF request, honouring a `Range` header with a 206 slice (defensive — the ENVI
// fetcher issues a plain whole-tile GET with no Range, but a COG reader could range-request, 08-RESEARCH).
function serveTiff(route: Route, bytes: Buffer): Promise<void> {
  const range = route.request().headers()["range"];
  const match = range ? /bytes=(\d+)-(\d*)/.exec(range) : null;
  if (match) {
    const start = Number(match[1]);
    const end = match[2] ? Number(match[2]) : bytes.length - 1;
    const slice = bytes.subarray(start, end + 1);
    return route.fulfill({
      status: 206,
      contentType: "image/tiff",
      headers: {
        "content-range": `bytes ${start}-${end}/${bytes.length}`,
        "accept-ranges": "bytes",
      },
      body: slice,
    });
  }
  return route.fulfill({ status: 200, contentType: "image/tiff", body: bytes });
}

// Install the fixture-serving GIS routes. Call AFTER `bootOffline(page)`. The fixtures are the committed
// bytes the real WASM decode path processes (never a reimplementation of the app).
export async function installGisMocks(page: Page): Promise<GisMockControl> {
  const ahn = FIXTURE("ahn_dtm_fixture.tif");
  const worldcover = FIXTURE("worldcover_fixture.tif");
  const overpass = FIXTURE("overpass_buildings.json").toString("utf-8");
  const control: GisMockControl = {
    counts: { pdok: 0, proxyGlo30: 0, proxyWorldcover: 0, overpass: 0 },
    setOverpassMode(mode) {
      overpassMode = mode;
    },
  };
  let overpassMode: "ok" | "429" = "ok";

  // AHN terrain — PDOK, cross-origin Direct (whole `dtm_05m/*.tif`).
  await page.route(/service\.pdok\.nl\/.*\.tif/, (route: Route) => {
    control.counts.pdok += 1;
    return serveTiff(route, ahn);
  });
  // GLO-30 terrain — same-origin byte proxy (defensive: the AHN viewport never hits it, but Pitfall 11
  // says mock every proxied origin; a stray GLO-30 fetch is thus caught by the counter, not the network).
  await page.route(/\/api\/v1\/proxy\/glo30\//, (route: Route) => {
    control.counts.proxyGlo30 += 1;
    return serveTiff(route, ahn);
  });
  // WorldCover land cover — same-origin byte proxy.
  await page.route(/\/api\/v1\/proxy\/worldcover\//, (route: Route) => {
    control.counts.proxyWorldcover += 1;
    return serveTiff(route, worldcover);
  });
  // OSM buildings — Overpass, cross-origin Direct (POST). Switchable 429 for the D-07 retry branch.
  await page.route(/overpass-api\.de\/api\/interpreter/, (route: Route) => {
    control.counts.overpass += 1;
    if (overpassMode === "429") {
      return route.fulfill({
        status: 429,
        contentType: "text/plain",
        body: "rate_limited",
      });
    }
    return route.fulfill({ status: 200, contentType: "application/json", body: overpass });
  });

  return control;
}

// The committed WGS84 import viewport + resolved tile names (generated with the fixtures by
// tools/gis_oracle/gen_e2e_fixtures.py). Imported by the import specs so coordinates have one source of truth.
export interface E2eViewport {
  readonly viewport: { min_lon: number; min_lat: number; max_lon: number; max_lat: number };
  readonly kaartblad: string;
  readonly worldcover_tile: string;
}
export function importViewport(): E2eViewport {
  const raw = FIXTURE("viewport.json").toString("utf-8");
  return JSON.parse(raw) as E2eViewport;
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

// --- Open-Meteo weather mocks (09-06 METX-01 offline weather-import journey + SC4) ------------------
//
// Serve the committed synthetic Open-Meteo pressure-level fixtures for BOTH date-switched hosts the real
// `fetchWeather` targets: the Forecast product (`https://api.open-meteo.com/v1/forecast`, recent dates) and
// the Archive/ERA5 product (`https://archive-api.open-meteo.com/v1/archive`, historical dates). The two
// hosts are HARD-CODED in `weather.ts` (SSRF T-09-05-03); we mock them by host so the real bundle fetches
// offline. `//api.open-meteo.com/` is matched with the leading `//` so it does NOT also swallow the archive
// host (whose `api` is preceded by `-`). Registered AFTER `bootOffline` so each specific route wins the
// most-recent-route match over the offline guard (an Open-Meteo body must be the fixture JSON, not aborted).
//
// Every route bumps a per-product counter so the journey can assert the correct product was hit for a given
// date (the D-02 date-switch branch coverage). The fixtures are synthetic (no copyrighted weather data) and
// carry a full 24-hour hourly block so any requested `hour_index` resolves.

export interface WeatherMockControl {
  readonly counts: {
    forecast: number;
    archive: number;
  };
}

// Install the fixture-serving Open-Meteo routes. Call AFTER `bootOffline(page)`. The served bytes are the
// exact JSON the real WASM `derive_weather` parse+fit path consumes (never a reimplementation of the app).
export async function installWeatherMocks(page: Page): Promise<WeatherMockControl> {
  const forecast = FIXTURE("openmeteo_forecast.json").toString("utf-8");
  const archive = FIXTURE("openmeteo_archive.json").toString("utf-8");
  const control: WeatherMockControl = { counts: { forecast: 0, archive: 0 } };

  // Forecast product — recent / near-future dates. The leading `//` anchors the host so the archive host
  // (`archive-api.open-meteo.com`) is NOT matched by this route.
  await page.route(/\/\/api\.open-meteo\.com\//, (route: Route) => {
    control.counts.forecast += 1;
    return route.fulfill({ status: 200, contentType: "application/json", body: forecast });
  });
  // Archive (ERA5) product — historical dates.
  await page.route(/archive-api\.open-meteo\.com\//, (route: Route) => {
    control.counts.archive += 1;
    return route.fulfill({ status: 200, contentType: "application/json", body: archive });
  });

  return control;
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
