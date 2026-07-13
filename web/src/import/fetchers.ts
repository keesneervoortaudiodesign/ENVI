// fetchers.ts — the direct-vs-proxy tile/query fetchers (D-02 CORS capability map). Routes each source to
// its verified reachability: AHN + Overpass are fetched cross-origin DIRECTLY; GLO-30 + WorldCover have no
// CORS headers and route through the same-origin allowlisted byte proxy (`/api/v1/proxy/{source}/{path}`).
//
// # Module I/O
// - Input  a covering `TileRefDto` (absolute upstream URL + source id) plus its `CorsDto` reachability
//   (from `plan_import`'s descriptor), or an Overpass endpoint + query body. Wire DTOs are imported from
//   `../generated/wire` (never hand-authored).
// - Output `fetchTileRange` resolves ONE half-open byte range of a tile (`Range: bytes=start-end`), the
//   COG range-read primitive `cogWindow.ts` drives; `fetchOverpass` resolves the raw JSON text with a
//   429-aware backoff retry (Overpass per-IP slots). A non-2xx response throws `ApiError` (status +
//   detail), reusing `../api/client`'s shared error type (it owns `ApiError`/`errorText` — never redeclared
//   here). An `AbortSignal` cancels a superseded request.
// - Valid input range: `tile.url` is the absolute upstream URL the tile planner built from the registry
//   `endpoint_template`; for a `proxy` source its pathname must lie under the proxy's allowlisted prefix
//   (enforced server-side — the fetcher only rewrites the URL, never fabricates a host).
//
// # Why ONE range per request (load-bearing — do not "optimise" into a multi-range GET)
//
// `Range` is a CORS-safelisted REQUEST header only while its value is a *simple range* (`bytes=N-M` /
// `bytes=N-`). A multi-range value (`bytes=0-9,20-29`) leaves the safelist and triggers a CORS preflight —
// and PDOK answers `OPTIONS` with `Access-Control-Allow-Headers: Content-Type` only, so the preflight FAILS
// and the whole AHN import dies. Verified live: PDOK returns `206 Partial Content` + `Access-Control-Allow-
// Origin: *` for a simple range, and the planner already coalesces adjacent COG tiles so the request count
// stays small. (`Content-Range` is NOT a safelisted RESPONSE header and PDOK exposes nothing, so the total
// file size is unreadable cross-origin — the design never needs it.)

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

// One fetched piece of a tile: the bytes found at file offset `offset`. A server that IGNORES `Range` and
// answers `200` with the whole body is honest about it — the bytes then start at offset 0 and the caller
// (and the WASM planner) treat them as such, so a non-range-capable mirror still works, just without the
// saving.
export interface FetchedRange {
  readonly offset: number;
  readonly bytes: Uint8Array;
}

// Fetch ONE half-open byte range `[start, end)` of a source tile, routing Direct vs the byte proxy per
// `cors`. Issues a single simple-range `Range: bytes=start-(end-1)` GET (see the module note on why it must
// stay single-range). A `206` yields exactly that slice; a `200` means the server served the whole file and
// the slice is the file itself, from offset 0. Non-2xx → `ApiError`.
export async function fetchTileRange(
  tile: TileRefDto,
  cors: CorsDto,
  start: number,
  end: number,
  signal?: AbortSignal,
): Promise<FetchedRange> {
  const url = cors === "proxy" ? proxyUrl(tile) : tile.url;
  const res = await fetch(url, {
    method: "GET",
    headers: { Range: `bytes=${start}-${end - 1}` },
    signal,
  });
  if (!res.ok) {
    throw new ApiError(res.status, await detailOf(res));
  }
  const bytes = new Uint8Array(await res.arrayBuffer());
  // 206 → the requested slice starts at `start`. 200 → Range was ignored; the body is the whole file.
  return { offset: res.status === 206 ? start : 0, bytes };
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
