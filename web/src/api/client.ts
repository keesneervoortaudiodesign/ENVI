// client.ts — the typed same-origin fetch client for the ENVI backend (Phase-6/07-03 REST surface).
//
// # Module I/O
// - Input  request bodies whose types are IMPORTED from the generated `../generated/wire` mirror of the
//   Rust serde DTOs (D-10) — this module NEVER hand-declares a wire type, so it cannot drift from the
//   server. The scene body is raw WGS84 GeoJSON (`terra-draw` `GeoJSONStoreFeatures`), which has no
//   ts-rs DTO because `scene.geojson` is a plain RFC-7946 FeatureCollection, not a serde struct.
// - Output typed promises: `getFreqAxis` (the 105-band axis — never hardcode Hz client-side),
//   `interpolateSpectrum` (D-05 preview), `triangulateDgm` (D-08 TIN, SC1), `getScene`/`putScene`
//   (whole-scene GET/PUT — no per-feature PATCH exists). A non-2xx response throws `ApiError` carrying
//   the HTTP status + the server's path-redacted `detail` string (surfaced as TEXT by callers).
// - Valid input range: relative `/api/v1/...` paths (same-origin; the SPA is served by envi-service).
//   `triangulateDgm` accepts an `AbortSignal` so a superseded debounced call can be cancelled.

import type {
  DgmReq,
  DgmResp,
  FreqAxisDto,
  InterpolateReq,
  InterpolateResp,
} from "../generated/wire";
import type { GeoJSONStoreFeatures } from "terra-draw";

const BASE = "/api/v1";

// A whole-scene GeoJSON FeatureCollection on the wire (WGS84, RFC 7946). Terra Draw's feature type is
// the store's canonical geometry carrier; this is NOT a mirror of a Rust DTO (the scene has none).
export interface SceneCollection {
  readonly type: "FeatureCollection";
  readonly features: readonly GeoJSONStoreFeatures[];
}

// A structured transport error: HTTP status + the server's `detail` (already path-redacted server-side;
// callers still render it as untrusted TEXT). NOT a wire type — it never crosses the network.
export class ApiError extends Error {
  readonly status: number;
  readonly detail: string;
  constructor(status: number, detail: string) {
    super(`HTTP ${status}: ${detail}`);
    this.name = "ApiError";
    this.status = status;
    this.detail = detail;
  }
}

// Extract a safe `detail` string from a (possibly non-JSON) error body without throwing.
async function readDetail(res: Response): Promise<string> {
  try {
    const body: unknown = await res.json();
    if (body && typeof body === "object" && "detail" in body) {
      const detail = (body as { detail: unknown }).detail;
      if (typeof detail === "string") {
        return detail;
      }
    }
  } catch {
    /* non-JSON / empty body — fall through to the status text */
  }
  return res.statusText || `status ${res.status}`;
}

async function getJson<T>(path: string, signal?: AbortSignal): Promise<T> {
  const res = await fetch(`${BASE}${path}`, { method: "GET", headers: { Accept: "application/json" }, signal });
  if (!res.ok) {
    throw new ApiError(res.status, await readDetail(res));
  }
  return (await res.json()) as T;
}

async function sendJson<T>(method: "POST" | "PUT", path: string, body: unknown, signal?: AbortSignal): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    method,
    headers: { "Content-Type": "application/json", Accept: "application/json" },
    body: JSON.stringify(body),
    signal,
  });
  if (!res.ok) {
    throw new ApiError(res.status, await readDetail(res));
  }
  // Some endpoints (PUT scene) return an empty 2xx body; tolerate a non-JSON/empty response.
  const text = await res.text();
  return (text ? JSON.parse(text) : undefined) as T;
}

// GET /api/v1/meta/freq-axis — the 105-point 1/12-octave axis, served once (never hardcode client-side).
export function getFreqAxis(signal?: AbortSignal): Promise<FreqAxisDto> {
  return getJson<FreqAxisDto>("/meta/freq-axis", signal);
}

// POST /api/v1/meta/interpolate-spectrum — expand authored anchors onto the dense grid (D-05 preview).
export function interpolateSpectrum(req: InterpolateReq, signal?: AbortSignal): Promise<InterpolateResp> {
  return sendJson<InterpolateResp>("POST", "/meta/interpolate-spectrum", req, signal);
}

// POST /api/v1/dgm/triangulate — server-side constrained-Delaunay TIN from elevation points/breaklines
// (D-08, SC1). A 4xx (interior-crossing / degenerate) throws `ApiError` for the caller to store.
export function triangulateDgm(req: DgmReq, signal?: AbortSignal): Promise<DgmResp> {
  return sendJson<DgmResp>("POST", "/dgm/triangulate", req, signal);
}

// GET /api/v1/projects/{id}/scene — the persisted WGS84 scene FeatureCollection.
export function getScene(projectId: string, signal?: AbortSignal): Promise<SceneCollection> {
  return getJson<SceneCollection>(`/projects/${encodeURIComponent(projectId)}/scene`, signal);
}

// PUT /api/v1/projects/{id}/scene — whole-scene save (no per-feature PATCH; D-04 coalesces to this).
export function putScene(projectId: string, scene: SceneCollection, signal?: AbortSignal): Promise<void> {
  return sendJson<void>("PUT", `/projects/${encodeURIComponent(projectId)}/scene`, scene, signal);
}
