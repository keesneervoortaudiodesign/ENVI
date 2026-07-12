// opfs.ts — the WORKER-side Origin Private File System glue for the chunked tensor
// store (D-08/D-09). This is the `envi-compute-opfs` module the Rust OPFS sink's
// `#[wasm_bindgen(module = "envi-compute-opfs")]` extern binds to (resolved by the
// Vite `resolve.alias` added in plan 10-04), PLUS the directory-layout + quota
// helpers the compute worker uses directly.
//
// # Module I/O
// - Input  a project UUID + the hex tensor-identity hash + an integer chunk index
//   (the ONLY inputs to a path — never a raw user string), and chunk bytes to
//   write. Names derive solely from the hex manifest hash + integer chunk index;
//   `safeSeg`/`assertHex` reject anything else so no `/`, `\`, `..`, or NUL can
//   escape the fixed `projects/<id>/calc/<hash>/{tensor,pincoh}/chunk_<idx>.bin`
//   layout (V12 path-traversal defence, threat T-10-04-02).
// - Output an exclusive-lock `FileSystemSyncAccessHandle` per chunk file (worker-
//   only — `createSyncAccessHandle` throws on the main thread, Pitfall 4), the
//   synchronous `write/flush/close` over it, and an honest OPFS quota snapshot.
// - Valid input range: `projectId` a UUID, `tensorHash` lowercase hex, `chunkIndex`
//   a non-negative integer. A malformed segment is a thrown `Error`, never a
//   silently-mangled path.
//
// # Worker-only (Pitfall 4/5)
// `createSyncAccessHandle()` is available ONLY inside a dedicated Web Worker and
// takes an EXCLUSIVE lock until `close()`. The disjoint-chunk-range design
// (pool.rs) guarantees one file per rayon task, so no two tasks contend the same
// handle. All I/O here runs inside the compute worker.

import { estimateQuota, fitsQuota, opfsAvailable, type QuotaEstimate } from "../import/opfs";

// Re-export the Phase-8 quota helpers so the worker/cost guardrail has a single
// import surface for the hard OPFS budget check (SC1 / D-09).
export { estimateQuota, fitsQuota, opfsAvailable };
export type { QuotaEstimate };

// The two parallel channel sub-directories under a tensor's calc directory (D-08):
// `tensor/` holds interleaved (re,im) f64-LE H_coh chunks; `pincoh/` holds f64-LE
// P_incoh chunks. Fixed literal segments — never a user string.
export type ChunkChannel = "tensor" | "pincoh";

// A minimal structural type for the OPFS sync access handle (the DOM lib may not
// ship it yet in every TS version). Despite the name, `createSyncAccessHandle()`
// itself is async; the returned handle's read/write/flush/close are synchronous.
export interface SyncAccessHandle {
  write(buffer: ArrayBufferView | ArrayBuffer, opts?: { at?: number }): number;
  flush(): void;
  close(): void;
  getSize(): number;
  truncate(newSize: number): void;
}

interface SyncCapableFileHandle extends FileSystemFileHandle {
  createSyncAccessHandle(): Promise<SyncAccessHandle>;
}

// Sanitize a single path segment: keep only a conservative filename charset so no
// `/`, `\`, `..`, or NUL escapes the fixed layout (V12). Belt-and-suspenders — the
// project UUID + fixed literals are trusted vocabularies — but the store never
// trusts a segment structurally.
function safeSeg(seg: string): string {
  const cleaned = seg.replace(/[^A-Za-z0-9._-]/g, "_");
  return cleaned.length === 0 || /^\.+$/.test(cleaned) ? "_" : cleaned;
}

// Assert a string is lowercase hex (the blake3 tensor-identity hash, D-09). The
// hash-keyed directory name is the primary path key, so it is validated strictly —
// a non-hex hash is a programming error (never a user string) and throws loudly
// rather than being silently `safeSeg`-mangled into a colliding directory.
function assertHex(hash: string): string {
  if (!/^[0-9a-f]+$/.test(hash)) {
    throw new Error(`invalid tensor hash (expected lowercase hex): ${hash}`);
  }
  return hash;
}

// The zero-padded chunk file name `chunk_<idx>.bin` (idx an integer). Padding keeps
// the OPFS directory listing lexicographically ordered by chunk index.
export function chunkFileName(chunkIndex: number): string {
  if (!Number.isInteger(chunkIndex) || chunkIndex < 0) {
    throw new Error(`invalid chunk index (expected a non-negative integer): ${chunkIndex}`);
  }
  return `chunk_${String(chunkIndex).padStart(5, "0")}.bin`;
}

// The relative file path of one chunk within a tensor's calc directory, e.g.
// `tensor/chunk_00042.bin` — exactly the `ChunkSpanDto.tensor_file`/`pincoh_file`
// the tier-complete event records (D-07). Relative to `projects/<id>/calc/<hash>/`.
export function chunkRelativePath(channel: ChunkChannel, chunkIndex: number): string {
  return `${channel}/${chunkFileName(chunkIndex)}`;
}

// Walk (creating) `projects/<projectId>/calc/<tensorHash>/<channel>/` and return
// its directory handle. Only the UUID, the hex hash, and the two fixed literals
// participate in the path (V12) — no arbitrary user string reaches
// `getDirectoryHandle`.
async function channelDir(
  projectId: string,
  tensorHash: string,
  channel: ChunkChannel,
): Promise<FileSystemDirectoryHandle> {
  let dir = await navigator.storage.getDirectory();
  for (const seg of ["projects", safeSeg(projectId), "calc", assertHex(tensorHash), channel]) {
    dir = await dir.getDirectoryHandle(seg, { create: true });
  }
  return dir;
}

// Open (exclusive-lock) one chunk file's sync access handle in the worker (D-08).
// The disjoint-chunk-range design gives each rayon task its own file, so no two
// opens contend (Pitfall 5). Caller MUST `close()` (via `closeChunk`) to release
// the lock so a re-run / cache reopen (D-09) does not collide.
export async function openChunkHandle(
  projectId: string,
  tensorHash: string,
  channel: ChunkChannel,
  chunkIndex: number,
): Promise<SyncAccessHandle> {
  const dir = await channelDir(projectId, tensorHash, channel);
  const file = (await dir.getFileHandle(chunkFileName(chunkIndex), {
    create: true,
  })) as SyncCapableFileHandle;
  const handle = await file.createSyncAccessHandle();
  // Truncate to 0 BEFORE any write (WR-03). The sink writes each chunk contiguously
  // from offset 0, but on the D-09 reuse path the file is reopened `create:true`, so
  // a shorter re-run (fewer bytes than a prior run) would otherwise leave stale
  // trailing bytes from the previous write. Truncating on open guarantees the file
  // holds exactly this run's bytes.
  handle.truncate(0);
  return handle;
}

// --- Hoisted async-open registry (D-08/D-09) ----------------------------------
//
// `createSyncAccessHandle()` is ASYNC while the Rust OPFS sink's `openChunk` extern
// is SYNCHRONOUS (the engine solve runs synchronously inside a rayon task). So the
// worker HOISTS the open: it `await`s `preopenChunk(...)` for a chunk's two channels
// BEFORE calling the synchronous `solve_chunk_range`, stashing each handle under its
// relative chunk path; the synchronous `openChunk(path)` then just returns the
// pre-opened handle. The active project id is set once per submit (`setActiveProject`)
// so a path key is only ever the hex hash + integer chunk index (never a user string).

let activeProjectId: string | null = null;

// Pre-opened `FileSystemSyncAccessHandle`s keyed by relative chunk path
// (`<channel>/chunk_<idx>.bin`) — exactly the path the Rust extern `openChunk`
// receives (see `chunk_relative_path` in envi-compute-wasm/src/lib.rs). Chunks are
// processed sequentially by the worker (one `await solve_chunk_range` at a time),
// so at most one chunk's tensor+pincoh handles are resident here.
const preopenedHandles = new Map<string, SyncAccessHandle>();

// Set the OPFS project the current submit's chunks belong to (D-09). Called once by
// the worker before the tier loop; `preopenChunk` walks
// `projects/<projectId>/calc/<hash>/…` for this id.
export function setActiveProject(projectId: string): void {
  activeProjectId = projectId;
}

// Asynchronously open one chunk file's sync-access handle AHEAD of the synchronous
// solve and register it under its relative chunk path. The worker calls this for the
// `tensor` and `pincoh` channels before each `solve_chunk_range`.
export async function preopenChunk(
  tensorHash: string,
  channel: ChunkChannel,
  chunkIndex: number,
): Promise<void> {
  if (activeProjectId === null) {
    throw new Error("preopenChunk called before setActiveProject");
  }
  const handle = await openChunkHandle(activeProjectId, tensorHash, channel, chunkIndex);
  preopenedHandles.set(chunkRelativePath(channel, chunkIndex), handle);
}

// Release a pre-opened handle by (channel, chunk index) if it is still registered —
// the leak guard for early-error paths where the Rust sink never opened/closed it
// (on the success path Rust's `closeChunk` already closed + evicted it, so this is a
// no-op). Idempotent.
export function releasePreopenedChunk(channel: ChunkChannel, chunkIndex: number): void {
  const key = chunkRelativePath(channel, chunkIndex);
  const handle = preopenedHandles.get(key);
  if (handle !== undefined) {
    try {
      handle.close();
    } catch {
      // already closed — nothing to release.
    }
    preopenedHandles.delete(key);
  }
}

// --- The `envi-compute-opfs` extern glue (the Rust OPFS sink binds these names) ---
//
// The Rust sink's extern block declares `openChunk`/`writeChunk`/`flushChunk`/
// `closeChunk` (opfs_sink.rs). These are the JS side of that seam. `openChunk` is
// SYNCHRONOUS and returns the handle the worker pre-opened under `path`
// (`<channel>/chunk_<idx>.bin`); the hex-hash path guard already ran inside
// `preopenChunk` → `openChunkHandle` → `channelDir` (`assertHex`/`safeSeg`, V12).
// The synchronous write/flush/close mirror the `FileSystemSyncAccessHandle` methods.

// Return the pre-opened sync handle registered for `path` (throws if the worker did
// not hoist the open first — the synchronous solve must never block on an async open).
export function openChunk(path: string): SyncAccessHandle {
  const handle = preopenedHandles.get(path);
  if (handle === undefined) {
    throw new Error(`no pre-opened OPFS handle for chunk path: ${path}`);
  }
  return handle;
}

// Synchronously write `bytes` at byte offset `at`; returns bytes written.
export function writeChunk(handle: SyncAccessHandle, bytes: Uint8Array, at: number): void {
  handle.write(bytes, { at });
}

// Commit buffered writes to storage (call before `closeChunk` for durability).
export function flushChunk(handle: SyncAccessHandle): void {
  handle.flush();
}

// Release the exclusive lock (final) AND evict the handle from the pre-open registry
// (found by identity) so a re-run / cache reopen (D-09) does not collide.
export function closeChunk(handle: SyncAccessHandle): void {
  handle.close();
  for (const [key, value] of preopenedHandles) {
    if (value === handle) {
      preopenedHandles.delete(key);
      break;
    }
  }
}

// --- Chunk READ glue (11-05 — the spectrum-panel readout path) ----------------
//
// Phase 10 only WROTE chunk files (the worker-only sync-handle write path above).
// The results readout READS them back and hands the raw bytes straight to the
// `readout_receivers` WASM export (`crates/envi-compute-wasm/src/opfs_reader.rs`);
// JS moves bytes, it never decodes or does acoustic math (D-01). Reads use the
// async File API, so — unlike the write path's worker-only `createSyncAccessHandle`
// (Pitfall 4) — they run on the main thread too (the compute wasm is already used
// main-thread for cost/plan, so the readout composes there). The SAME
// `safeSeg`/`assertHex` V12 path guards apply: only the project UUID + hex tensor
// hash + the two fixed channel literals + an integer chunk index ever reach the
// OPFS walk, so no `/`, `\`, `..`, or NUL can escape the fixed layout.

// Walk to `projects/<id>/calc/<hash>/<channel>/` WITHOUT creating anything — a
// missing directory throws (surfaced as the honest "result data unavailable"
// state, T-11-05-03) rather than fabricating an empty tree.
async function channelDirReadonly(
  projectId: string,
  tensorHash: string,
  channel: ChunkChannel,
): Promise<FileSystemDirectoryHandle> {
  let dir = await navigator.storage.getDirectory();
  for (const seg of ["projects", safeSeg(projectId), "calc", assertHex(tensorHash), channel]) {
    dir = await dir.getDirectoryHandle(seg, { create: false });
  }
  return dir;
}

// Read one channel's chunk-file bytes via the async File API.
async function readChannelBytes(
  projectId: string,
  tensorHash: string,
  channel: ChunkChannel,
  chunkIndex: number,
): Promise<Uint8Array> {
  const dir = await channelDirReadonly(projectId, tensorHash, channel);
  const file = await dir.getFileHandle(chunkFileName(chunkIndex), { create: false });
  const blob = await file.getFile();
  return new Uint8Array(await blob.arrayBuffer());
}

// Read a chunk's `H_coh` (tensor/) + `P_incoh_abs` (pincoh/) byte pair — exactly
// the two `&[u8]` inputs the `readout_receivers` WASM reader decodes back into the
// `[s][r_local][f]` arrays. The caller passes these straight to the export.
export async function readChunk(
  projectId: string,
  tensorHash: string,
  chunkIndex: number,
): Promise<{ tensor: Uint8Array; pincoh: Uint8Array }> {
  const [tensor, pincoh] = await Promise.all([
    readChannelBytes(projectId, tensorHash, "tensor", chunkIndex),
    readChannelBytes(projectId, tensorHash, "pincoh", chunkIndex),
  ]);
  return { tensor, pincoh };
}

// Write a chunk file's raw bytes via the async File API (main-thread capable) —
// the read path's inverse, used to seed a fixture tensor into OPFS for the offline
// results UAT (and any future non-worker cache-write path). Same V12 path guards.
export async function writeChunkFile(
  projectId: string,
  tensorHash: string,
  channel: ChunkChannel,
  chunkIndex: number,
  bytes: Uint8Array,
): Promise<void> {
  let dir = await navigator.storage.getDirectory();
  for (const seg of ["projects", safeSeg(projectId), "calc", assertHex(tensorHash), channel]) {
    dir = await dir.getDirectoryHandle(seg, { create: true });
  }
  const file = await dir.getFileHandle(chunkFileName(chunkIndex), { create: true });
  const writable = await file.createWritable();
  await writable.write(bytes as unknown as BufferSource);
  await writable.close();
}
