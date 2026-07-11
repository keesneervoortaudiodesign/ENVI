// weather.ts — the browser-side Open-Meteo weather import (METX-01, D-01/D-02/D-03). A date-switched
// multi-level fetch (direct CORS first, login-server byte-proxy fallback), an OPFS cache keyed strictly by
// `(lat, lon, timestamp)`, a per-fetch call-cost log, and a `deriveAbc` that delegates ALL A/B/C acoustic
// math to the `envi-gis` WASM shim (`derive_weather`) — TypeScript never does acoustic arithmetic.
//
// # Module I/O
// - Input  a site `(lat, lon)` (WGS84 degrees), a requested `date` (`YYYY-MM-DD`) + `hour` (0–23, UTC), the
//   open project id (the OPFS cache is per-project), and — for the derivation — the path azimuths to project
//   the per-bearing `A` onto. Wire DTOs are imported from `../generated/wire` (never hand-authored).
// - Output `fetchWeather` resolves the raw Open-Meteo JSON body (from OPFS on a hit — ZERO network — or from
//   the network on a miss, then cached) plus whether it came from cache + the logged call-cost weight.
//   `deriveAbc` resolves the WASM `WeatherDeriveResult` (bearing-independent components + one
//   `SoundSpeedProfile` per requested azimuth). A non-2xx network response throws `ApiError` (reused from
//   `../api/client`, never redeclared).
// - Valid input range: the two Open-Meteo hosts are HARD-CODED (`api.` / `archive-api.open-meteo.com`) — the
//   client never fetches a user-supplied host (SSRF, threat T-09-05-03). `hour` indexes the single requested
//   UTC day, so `hour_index === hour`.

import { ApiError } from "../api/client";
import type { WeatherDeriveReq, WeatherDeriveResult } from "../generated/wire";
import { PROXY_BASE, detailOf } from "./fetchers";
import { getWeather, putWeather } from "./opfs";
import { deriveWeather } from "./wasm";

// The two date-switched Open-Meteo products (D-02). Historical dates → the ERA5-backed Archive API;
// recent / near-future dates → the Forecast API. Same schema + units — the client picks the host by date.
// HARD-CODED hosts: the fetch target is never assembled from a user string (SSRF, T-09-05-03).
const FORECAST_URL = "https://api.open-meteo.com/v1/forecast";
const ARCHIVE_URL = "https://archive-api.open-meteo.com/v1/archive";

// The two Open-Meteo hosts map to two fixed proxy source ids; the same-origin byte-proxy mount itself is the
// shared `PROXY_BASE` (mirrors `envi-service`'s allowlisted `GET /api/v1/proxy/{source}/{*path}` relay — the
// Phase-8 CORS-restricted fallback). The server re-validates its own allowlist, so this only moves the fetch
// same-origin.
const PROXY_SOURCE: Readonly<Record<string, string>> = {
  [FORECAST_URL]: "openmeteo-forecast",
  [ARCHIVE_URL]: "openmeteo-archive",
};

// The Archive (ERA5 reanalysis) trails real-time by ~5 days; anything older than this cutoff is served by
// the Archive product, everything more recent (incl. near-future) by the Forecast product (D-02). A named
// const, not a magic number — the exact boundary is an engineering choice, not a spec value.
const ARCHIVE_CUTOFF_DAYS = 7;

// The pressure levels requested (hPa), surface → ~1.5 km — MUST match `envi_gis::weather::PRESSURE_LEVELS_HPA`
// (the WASM shim reads exactly this set by name). Kept here only to BUILD the request URL; no acoustic use.
const PRESSURE_LEVELS_HPA: readonly number[] = [1000, 975, 950, 925, 900, 850];

// The near-surface height-AGL anchors the shim also reads (conditions the log fit near the ground).
const NEAR_SURFACE_VARS: readonly string[] = [
  "temperature_2m",
  "wind_speed_10m",
  "wind_direction_10m",
];

// The full hourly variable list: 4 fields × each pressure level + the 3 near-surface anchors (27 total).
function hourlyVariables(): string[] {
  const vars: string[] = [];
  for (const hpa of PRESSURE_LEVELS_HPA) {
    vars.push(
      `temperature_${hpa}hPa`,
      `wind_speed_${hpa}hPa`,
      `wind_direction_${hpa}hPa`,
      `geopotential_height_${hpa}hPa`,
    );
  }
  vars.push(...NEAR_SURFACE_VARS);
  return vars;
}

// The result of a weather import: the raw Open-Meteo JSON body, whether it was served from OPFS (a cache HIT
// issues ZERO network calls — the SC4 zero-egress-on-what-if property), and the logged call-cost weight
// (undefined on a cache hit — no call was made).
export interface WeatherFetch {
  readonly json: string;
  readonly fromCache: boolean;
  readonly callCostWeight: number | undefined;
}

// The OPFS cache key: `(lat, lon, timestamp)` ONLY (D-01) — NEVER re-keyed on view/UI state (Pitfall 6). The
// lat/lon are rounded to a stable precision so trivially-different floats hit the same cache entry. `safeSeg`
// (inside `putWeather`/`getWeather`) neutralises any path-traversal attempt in the key (threat T-09-05-02).
export function weatherKey(lat: number, lon: number, timestamp: string): string {
  return `${lat.toFixed(4)}_${lon.toFixed(4)}_${timestamp}`;
}

// The canonical `(site, timestamp)` string for a requested date + UTC hour (D-01: a single representative
// hour). E.g. `("2024-01-15", 14) → "2024-01-15T14"`.
export function weatherTimestamp(date: string, hour: number): string {
  return `${date}T${String(hour).padStart(2, "0")}`;
}

// Pick the date-switched product URL (D-02). A requested date at/earlier than `today − ARCHIVE_CUTOFF_DAYS`
// uses the Archive (ERA5) product; anything more recent (incl. near-future) uses the Forecast product.
export function productUrl(date: string, now: Date = new Date()): string {
  const requested = Date.parse(`${date}T00:00:00Z`);
  const cutoff = now.getTime() - ARCHIVE_CUTOFF_DAYS * 24 * 3600 * 1000;
  return Number.isFinite(requested) && requested <= cutoff ? ARCHIVE_URL : FORECAST_URL;
}

// Build the full Open-Meteo request URL for a single UTC day (`start_date = end_date = date`). `wind_speed_unit=ms`
// is REQUIRED — the shim reads `wind_speed_ms`, and Open-Meteo defaults to km/h (a silent unit bug otherwise).
function requestUrl(base: string, lat: number, lon: number, date: string): string {
  const params = new URLSearchParams({
    latitude: String(lat),
    longitude: String(lon),
    hourly: hourlyVariables().join(","),
    start_date: date,
    end_date: date,
    wind_speed_unit: "ms",
    timezone: "UTC",
  });
  return `${base}?${params.toString()}`;
}

// Open-Meteo weights a call by variable-count × time-steps ("API call weight"); log an estimate per network
// fetch so the non-commercial daily budget stays visible (SC4). 27 variables × 24 hourly steps / 100 ≈ 6.5.
function callCostWeight(): number {
  return (hourlyVariables().length * 24) / 100;
}

// Rewrite a direct product URL into the same-origin byte-proxy path (CORS-restricted deploy fallback). The
// upstream path + query become the proxy `{*path}` wildcard under a FIXED source id; the server re-validates
// its allowlist (an un-allowlisted host simply 404s — an honest, host-pinned fallback, never a widened reach).
function proxyUrlFor(base: string, directUrl: string): string {
  const sourceId = PROXY_SOURCE[base] ?? "openmeteo-forecast";
  const u = new URL(directUrl);
  return `${PROXY_BASE}/${sourceId}${u.pathname}${u.search}`;
}

// Fetch a whole Open-Meteo response body, direct-CORS first, byte-proxy fallback on a network/CORS failure.
// A non-2xx (from either path) throws `ApiError` (status + short detail).
async function fetchBody(base: string, directUrl: string, signal?: AbortSignal): Promise<string> {
  try {
    const res = await fetch(directUrl, { method: "GET", signal });
    if (!res.ok) {
      throw new ApiError(res.status, await detailOf(res));
    }
    return await res.text();
  } catch (err) {
    // A thrown ApiError is a real non-2xx from the direct path — propagate it (do NOT proxy-retry a 4xx).
    if (err instanceof ApiError) {
      throw err;
    }
    // A caller abort is NOT a network/CORS failure — re-throw it so a cancelled fetch does not spawn a
    // second (proxy) request with the already-aborted signal (WR-03), which would only reject again and
    // momentarily reach the same-origin proxy. A clean cancel must stay a clean cancel.
    if (signal?.aborted || (err as { name?: string })?.name === "AbortError") {
      throw err;
    }
    // A network/CORS failure (TypeError) → try the same-origin allowlisted proxy once.
    const res = await fetch(proxyUrlFor(base, directUrl), { method: "GET", signal });
    if (!res.ok) {
      throw new ApiError(res.status, await detailOf(res));
    }
    return await res.text();
  }
}

// Import the multi-level weather for a site + requested UTC hour. Reads OPFS FIRST — a cache HIT returns the
// cached body with ZERO network calls (SC4). On a miss, date-switches the product, fetches (direct → proxy),
// caches under `(lat,lon,timestamp)`, logs the call-cost, and returns the body. `deriveAbc` (below) turns the
// returned JSON into per-azimuth A/B/C entirely in WASM.
export async function fetchWeather(
  projectId: string,
  lat: number,
  lon: number,
  date: string,
  hour: number,
  signal?: AbortSignal,
): Promise<WeatherFetch> {
  const timestamp = weatherTimestamp(date, hour);
  const key = weatherKey(lat, lon, timestamp);

  const cached = await getWeather(projectId, key);
  if (cached !== null) {
    // Cache hit — the SC4 zero-egress path. No network call, no call-cost.
    return { json: cached, fromCache: true, callCostWeight: undefined };
  }

  const base = productUrl(date);
  const json = await fetchBody(base, requestUrl(base, lat, lon, date), signal);
  await putWeather(projectId, key, json);

  const weight = callCostWeight();
  // The visible call-cost line (SC4): the weighted Open-Meteo budget spent by THIS fetch.
  console.info(`[weather] Open-Meteo fetch call-cost weight ≈ ${weight.toFixed(2)} (${base})`);
  return { json, fromCache: false, callCostWeight: weight };
}

// The reference downwind bearing φ_wind the WASM shim requires as an input (degrees clockwise from north).
// This reads the surface wind DIRECTION field out of the response (a data extraction + the +180° downwind
// convention) — NOT acoustic / A-B-C arithmetic, which stays wholly in WASM. Falls back to the 1000 hPa wind
// direction, then to 0° (a defined reference) if neither is present, so the derivation never depends on a
// silently-fabricated value.
function downwindBearingDeg(json: string, hourIndex: number): number {
  let hourly: Record<string, unknown> | undefined;
  try {
    const parsed = JSON.parse(json) as { hourly?: Record<string, unknown> };
    hourly = parsed.hourly;
  } catch {
    return 0;
  }
  const read = (name: string): number | undefined => {
    const arr = hourly?.[name];
    if (Array.isArray(arr)) {
      const v = arr[hourIndex];
      return typeof v === "number" && Number.isFinite(v) ? v : undefined;
    }
    return undefined;
  };
  const windFrom = read("wind_direction_10m") ?? read("wind_direction_1000hPa") ?? 0;
  return (windFrom + 180) % 360;
}

// Derive the per-azimuth A/B/C from an OPFS-cached Open-Meteo body — the WASM shim owns ALL of the acoustic
// math (temperature/wind fit, per-bearing projection); this only marshals the request. `hour` indexes the
// single requested UTC day. `z0` is the roughness length (clamped ≥ 0.001 m in the shim).
export function deriveAbc(
  json: string,
  hour: number,
  pathAzimuthsDeg: readonly number[],
  z0 = 0.05,
): Promise<WeatherDeriveResult> {
  const req: WeatherDeriveReq = {
    openmeteo_json: json,
    hour_index: hour,
    phi_wind_deg: downwindBearingDeg(json, hour),
    z0,
    path_azimuths_deg: [...pathAzimuthsDeg],
  };
  return deriveWeather(req);
}
