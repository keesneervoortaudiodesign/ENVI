// opfs.ts — the per-project Origin Private File System cache (DATA-04, D-03). Whole source tiles are
// fetched once at ingestion and persisted here; after import the compute path reads terrain / land cover
// tiles ONLY from OPFS, so the network is touched only at ingestion time (the DATA-04 network-off
// guarantee, proven by the 08-08 Playwright replay).
//
// # Module I/O
// - Input  a project UUID + a registry source id + a tile name + tile bytes (`putTile`), or the same key
//   minus the bytes (`getTile`). Path segments derive ONLY from the project UUID and the FIXED
//   `cache/<source>/<tile>` layout — no user-controlled path segment ever reaches `getDirectoryHandle`
//   (threat T-08-07-02, V12). A hostile source/tile string is neutralised by `safeSeg` before it is used.
// - Output `putTile` persists the bytes; `getTile` resolves the cached bytes or `null` on a miss (the
//   fetch path). `estimateQuota` wraps `navigator.storage.estimate()` for honest quota-exhaustion UI
//   (threat T-08-07-01). Uses ONLY the async main-thread API (`getDirectory()` → `getFileHandle` →
//   `createWritable` / `getFile().arrayBuffer()`) — `FileSystemSyncAccessHandle` is worker-only and is
//   never used here (Pitfall 10).
// - Valid input range: `projectId` a UUID; `source`/`tile` from the registry / tile planner (already
//   fixed vocabularies) — `safeSeg` is defence-in-depth, not the primary trust boundary.

// A quota snapshot for the import UI: used vs granted bytes (either may be undefined if the browser does
// not report it — the UI degrades to "unknown" rather than fabricating a number).
export interface QuotaEstimate {
  readonly usageBytes: number | undefined;
  readonly quotaBytes: number | undefined;
}

// Whether OPFS is available in this environment (older/embedded browsers may lack it). Callers can warn
// honestly rather than throwing an opaque error deep in a fetch loop.
export function opfsAvailable(): boolean {
  return typeof navigator !== "undefined" && !!navigator.storage?.getDirectory;
}

// Sanitize a single path segment: keep only a conservative filename charset so no `/`, `\`, `..`, or NUL
// can escape the fixed `projects/<uuid>/cache/<source>/<tile>` layout (V12). This is belt-and-suspenders —
// the source ids and tile names are fixed registry/tile-planner vocabularies — but the cache never trusts
// them structurally.
function safeSeg(seg: string): string {
  const cleaned = seg.replace(/[^A-Za-z0-9._-]/g, "_");
  // A segment that reduces to empty or a dot-run would be ambiguous; pin it to a stable placeholder.
  return cleaned.length === 0 || /^\.+$/.test(cleaned) ? "_" : cleaned;
}

// Walk (creating) `projects/<projectId>/cache/<source>/` and return its directory handle. The project
// UUID and the two fixed segments are the ONLY inputs to the path — no user string participates.
async function cacheDir(projectId: string, source: string): Promise<FileSystemDirectoryHandle> {
  let dir = await navigator.storage.getDirectory();
  for (const seg of ["projects", safeSeg(projectId), "cache", safeSeg(source)]) {
    dir = await dir.getDirectoryHandle(seg, { create: true });
  }
  return dir;
}

// Persist a whole source tile under `projects/<projectId>/cache/<source>/<tile>` (async writable — main-
// thread safe). Overwrites an existing tile (a re-fetch of the same key is idempotent).
export async function putTile(
  projectId: string,
  source: string,
  tile: string,
  bytes: ArrayBuffer,
): Promise<void> {
  const dir = await cacheDir(projectId, source);
  const handle = await dir.getFileHandle(safeSeg(tile), { create: true });
  const writable = await handle.createWritable();
  try {
    await writable.write(bytes);
  } finally {
    await writable.close();
  }
}

// Read a cached tile's bytes, or `null` on a miss (the fetch-then-`putTile` path). A miss is the ordinary
// cold-cache state, never an error — any lookup failure (absent dir/file) resolves to `null`.
export async function getTile(
  projectId: string,
  source: string,
  tile: string,
): Promise<ArrayBuffer | null> {
  try {
    const dir = await cacheDir(projectId, source);
    const handle = await dir.getFileHandle(safeSeg(tile));
    const file = await handle.getFile();
    return await file.arrayBuffer();
  } catch {
    return null;
  }
}

// Remove a cached tile (best-effort). Used to evict a cached source tile so a subsequent compute read
// misses OPFS and must re-fetch — the DATA-04 network-off replay's negative guard depends on this to
// prove a render came from OPFS rather than a lingering in-memory copy. A miss is not an error.
export async function removeTile(projectId: string, source: string, tile: string): Promise<void> {
  try {
    const dir = await cacheDir(projectId, source);
    await dir.removeEntry(safeSeg(tile));
  } catch {
    /* already absent — nothing to evict */
  }
}

// An honest storage-quota snapshot (threat T-08-07-01): callers block/warn before a write that would
// exhaust the origin's quota, rather than letting a `write` fail opaquely. Both fields may be undefined.
export async function estimateQuota(): Promise<QuotaEstimate> {
  if (typeof navigator === "undefined" || !navigator.storage?.estimate) {
    return { usageBytes: undefined, quotaBytes: undefined };
  }
  const est = await navigator.storage.estimate();
  return { usageBytes: est.usage, quotaBytes: est.quota };
}

// Whether a write of `bytes` would fit the remaining quota (honest headroom check). Unknown quota → allow
// (the browser did not report a limit); the write itself still surfaces a real failure if one occurs.
export function fitsQuota(estimate: QuotaEstimate, bytes: number): boolean {
  if (estimate.quotaBytes === undefined || estimate.usageBytes === undefined) {
    return true;
  }
  return estimate.usageBytes + bytes <= estimate.quotaBytes;
}
