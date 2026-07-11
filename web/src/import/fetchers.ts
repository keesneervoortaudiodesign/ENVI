// fetchers.ts — the direct-vs-proxy tile/query fetchers (D-02 CORS capability map). Routes each source to
// its verified reachability: AHN + Overpass are fetched cross-origin DIRECTLY; GLO-30 + WorldCover have no
// CORS headers and route through the same-origin allowlisted byte proxy (`/api/v1/proxy/{source}/{path}`).
//
// # Module I/O
// - Input  a covering `TileRefDto` (absolute upstream URL + source id) plus its `CorsDto` reachability
//   (from `plan_import`'s descriptor), or an Overpass endpoint + query body. Wire DTOs are imported from
//   `../generated/wire` (never hand-authored).
// - Output `fetchTile` resolves the tile's raw bytes (`ArrayBuffer`) via a whole-tile plain GET — NO
//   `Range` header, so no CORS preflight anywhere (Pitfall 2); `fetchOverpass` resolves the raw JSON text
//   with a 429-aware backoff retry (Overpass per-IP slots). A non-2xx response throws `ApiError` (status +
//   detail), reusing `../api/client`'s shared error type (it owns `ApiError`/`errorText` — never redeclared
//   here). An `AbortSignal` cancels a superseded request.
// - Valid input range: `tile.url` is the absolute upstream URL the tile planner built from the registry
//   `endpoint_template`; for a `proxy` source its pathname must lie under the proxy's allowlisted prefix
//   (enforced server-side — the fetcher only rewrites the URL, never fabricates a host).

import { ApiError } from "../api/client";
import type { CorsDto, TileRefDto } from "../generated/wire";

// The same-origin proxy mount (mirrors `envi-service`'s `GET /api/v1/proxy/{source}/{*path}` route).
// Exported as the ONE proxy-mount constant every fetcher (tiles + weather) builds its relay path on.
export const PROXY_BASE = "/api/v1/proxy";

// Overpass 429 backoff: a handful of retries with growing delay, matching the single-user tool's modest
// slot budget (08-RESEARCH §Buildings). Kept small — the proxy fallback is the escalation if limits bite.
const OVERPASS_MAX_ATTEMPTS = 3;
const OVERPASS_BACKOFF_MS = 1500;

// Rewrite a `proxy` source's absolute upstream URL into the same-origin relay path. The upstream pathname
// (which already begins with the source's allowlisted prefix) becomes the proxy `{*path}` wildcard; the
// server re-validates the prefix, so this only moves the fetch same-origin, it never widens reach.
function proxyUrl(tile: TileRefDto): string {
  const upstreamPath = new URL(tile.url).pathname; // leading "/", e.g. "/Copernicus_DSM_COG_.../*.tif"
  return `${PROXY_BASE}/${tile.source_id}${upstreamPath}`;
}

// Extract a short error detail from a (possibly binary/empty) non-2xx response without throwing.
// Shared by every raw-`Response` fetcher (tiles, Overpass, weather) so the truncation rule lives once.
export async function detailOf(res: Response): Promise<string> {
  try {
    const text = await res.text();
    return text.slice(0, 200) || res.statusText || `status ${res.status}`;
  } catch {
    return res.statusText || `status ${res.status}`;
  }
}

// Fetch a whole source tile's bytes, routing Direct vs the byte proxy per `cors`. Whole-tile plain GET
// (no `Range` header → no preflight). Non-2xx → `ApiError`.
export async function fetchTile(
  tile: TileRefDto,
  cors: CorsDto,
  signal?: AbortSignal,
): Promise<ArrayBuffer> {
  const url = cors === "proxy" ? proxyUrl(tile) : tile.url;
  const res = await fetch(url, { method: "GET", signal });
  if (!res.ok) {
    throw new ApiError(res.status, await detailOf(res));
  }
  return res.arrayBuffer();
}

// Build the Overpass `out geom` query for a viewport (single request per import; way + multipolygon
// relations). Coordinates are `(south, west, north, east)` per the Overpass bbox convention.
export function overpassQuery(
  minLon: number,
  minLat: number,
  maxLon: number,
  maxLat: number,
): string {
  const bbox = `(${minLat},${minLon},${maxLat},${maxLon})`;
  return (
    "[out:json][timeout:25];" +
    `(way["building"]${bbox};` +
    `relation["building"]["type"="multipolygon"]${bbox};);` +
    "out body geom;"
  );
}

// Fetch the Overpass JSON text for a query (direct, CORS-open), retrying on HTTP 429 with backoff. A
// non-2xx (other than a retried-then-exhausted 429) throws `ApiError`. The caller passes the raw text
// straight to `parse_buildings` (WASM owns the JSON→features transform).
export async function fetchOverpass(
  endpoint: string,
  query: string,
  signal?: AbortSignal,
): Promise<string> {
  let lastErr: ApiError | null = null;
  for (let attempt = 0; attempt < OVERPASS_MAX_ATTEMPTS; attempt++) {
    const res = await fetch(endpoint, {
      method: "POST",
      headers: { "Content-Type": "text/plain" },
      body: query,
      signal,
    });
    if (res.ok) {
      return res.text();
    }
    if (res.status === 429 && attempt < OVERPASS_MAX_ATTEMPTS - 1) {
      lastErr = new ApiError(429, await detailOf(res));
      await delay(OVERPASS_BACKOFF_MS * (attempt + 1), signal);
      continue;
    }
    throw new ApiError(res.status, await detailOf(res));
  }
  // Exhausted the 429 retries.
  throw lastErr ?? new ApiError(429, "Overpass rate limit — try again shortly.");
}

// A cancellable delay (rejects if the signal aborts mid-wait, so a superseded import stops backing off).
function delay(ms: number, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) {
      reject(new DOMException("Aborted", "AbortError"));
      return;
    }
    const timer = setTimeout(resolve, ms);
    signal?.addEventListener(
      "abort",
      () => {
        clearTimeout(timer);
        reject(new DOMException("Aborted", "AbortError"));
      },
      { once: true },
    );
  });
}
