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
  CreateProjectRequest,
  DgmReq,
  DgmResp,
  FreqAxisDto,
  InterpolateReq,
  InterpolateResp,
  ProjectMetaDto,
  SplToLwReq,
  SplToLwResp,
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

// Extract a user-facing message from an unknown thrown value: an `ApiError`'s server `detail`, a plain
// `Error`'s message, else the caller's `fallback`. The single shared shape for the panels/dialogs/editors
// that render a failed request as TEXT (never innerHTML) — `client.ts` owns `ApiError`, so it owns this.
export function errorText(err: unknown, fallback = "Request failed."): string {
  if (err instanceof ApiError) {
    return err.detail;
  }
  return err instanceof Error ? err.message : fallback;
}

// A structured `{ status, detail }` view of a thrown value — an `ApiError`'s
// (status, detail), else `{ status: 0, detail }` with `detail` from `errorText`.
// The single shared shape the layer/weather import stores record as a failure (both
// carry the same HTTP-ish status + text pair). `client.ts` owns `ApiError`, so it
// owns this normalizer too.
export function toStatusError(
  err: unknown,
  fallback = "Request failed.",
): { status: number; detail: string } {
  if (err instanceof ApiError) {
    return { status: err.status, detail: err.detail };
  }
  return { status: 0, detail: errorText(err, fallback) };
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

// DELETE a resource, tolerating an empty 2xx body. A non-2xx throws `ApiError` (status + server detail).
async function deleteResource(path: string, signal?: AbortSignal): Promise<void> {
  const res = await fetch(`${BASE}${path}`, { method: "DELETE", headers: { Accept: "application/json" }, signal });
  if (!res.ok) {
    throw new ApiError(res.status, await readDetail(res));
  }
}

async function sendJson<T>(
  method: "POST" | "PUT",
  path: string,
  body: unknown,
  signal?: AbortSignal,
  keepalive?: boolean,
): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    method,
    headers: { "Content-Type": "application/json", Accept: "application/json" },
    body: JSON.stringify(body),
    signal,
    keepalive,
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

// POST /api/v1/meta/spl-to-lw — back-calculate sound power L_W from a free-field SPL-at-reference (WEB-02).
// The acoustic free-field correction is SERVER-side (SVC-07) — the client never does Hz/log arithmetic.
export function splToLw(req: SplToLwReq, signal?: AbortSignal): Promise<SplToLwResp> {
  return sendJson<SplToLwResp>("POST", "/meta/spl-to-lw", req, signal);
}

// POST /api/v1/dgm/triangulate — server-side constrained-Delaunay TIN from elevation points/breaklines
// (D-08, SC1). A 4xx (interior-crossing / degenerate) throws `ApiError` for the caller to store.
export function triangulateDgm(req: DgmReq, signal?: AbortSignal): Promise<DgmResp> {
  return sendJson<DgmResp>("POST", "/dgm/triangulate", req, signal);
}

// GET /api/v1/projects — metadata of every stored project (the Open picker's list, D-06).
export function listProjects(signal?: AbortSignal): Promise<ProjectMetaDto[]> {
  return getJson<ProjectMetaDto[]>("/projects", signal);
}

// POST /api/v1/projects — create a project (the server pins its UTM CRS from the WGS84 origin, D-03) and
// return the new metadata. Backs the "New project" flow.
export function createProject(req: CreateProjectRequest, signal?: AbortSignal): Promise<ProjectMetaDto> {
  return sendJson<ProjectMetaDto>("POST", "/projects", req, signal);
}

// GET /api/v1/projects/{id} — open a project (records reopen-last server-side, D-06) and return metadata.
export function getProject(projectId: string, signal?: AbortSignal): Promise<ProjectMetaDto> {
  return getJson<ProjectMetaDto>(`/projects/${encodeURIComponent(projectId)}`, signal);
}

// GET /api/v1/projects/last — the last-opened project's metadata for reopen-last (D-06), or `null` when
// there is no record (a 404) OR the store returns an id-less body (the offline test stub). A missing
// last-project is a normal empty state, never an error — so this resolves to `null`, it does not throw.
export async function getLastProject(signal?: AbortSignal): Promise<ProjectMetaDto | null> {
  try {
    const meta = await getJson<ProjectMetaDto>("/projects/last", signal);
    return meta && typeof meta.id === "string" && meta.id.length > 0 ? meta : null;
  } catch (err) {
    if (err instanceof ApiError && err.status === 404) {
      return null;
    }
    throw err;
  }
}

// GET /api/v1/projects/{id}/scene — the persisted WGS84 scene FeatureCollection.
export function getScene(projectId: string, signal?: AbortSignal): Promise<SceneCollection> {
  return getJson<SceneCollection>(`/projects/${encodeURIComponent(projectId)}/scene`, signal);
}

// PUT /api/v1/projects/{id}/scene — whole-scene save (no per-feature PATCH; D-04 coalesces to this).
// `keepalive` routes the D-04 flush-on-unload PUT so the browser completes it as the document tears down
// (a plain fetch would be cancelled); the wire path/verb/base stay owned here, never rebuilt by callers.
export function putScene(
  projectId: string,
  scene: SceneCollection,
  opts?: { readonly signal?: AbortSignal; readonly keepalive?: boolean },
): Promise<void> {
  return sendJson<void>(
    "PUT",
    `/projects/${encodeURIComponent(projectId)}/scene`,
    scene,
    opts?.signal,
    opts?.keepalive,
  );
}

// DELETE /api/v1/projects/{id} — irreversibly removes the project folder (scene + settings + calc
// records). Guarded by the typed-name confirmation dialog client-side; the server is the backstop.
export function deleteProject(projectId: string, signal?: AbortSignal): Promise<void> {
  return deleteResource(`/projects/${encodeURIComponent(projectId)}`, signal);
}
