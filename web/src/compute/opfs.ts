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
  return file.createSyncAccessHandle();
}

// --- The `envi-compute-opfs` extern glue (the Rust OPFS sink binds these names) ---
//
// The Rust sink's extern block declares `openChunk`/`writeChunk`/`flushChunk`/
// `closeChunk` (opfs_sink.rs). These are the JS side of that seam. `openChunk`
// takes the full relative path `projects/<id>/calc/<hash>/<channel>/chunk_<idx>.bin`
// and walks it defensively (every segment `safeSeg`-guarded). The synchronous
// write/flush/close mirror the `FileSystemSyncAccessHandle` methods 1:1.
//
// NOTE (integration seam): `createSyncAccessHandle()` is async while the Rust
// extern `open_chunk` is synchronous. Until `solve_chunk_range` is wired to the
// sink (per-range scene marshalling, see pool.rs), handles are opened via the
// async `openChunkHandle` above; `openChunk` here is the path-parsing entry the
// wired sink will use once the open is hoisted ahead of the synchronous solve.

// Walk an already-safe relative chunk path and open its sync handle (worker-only).
export async function openChunk(path: string): Promise<SyncAccessHandle> {
  const segments = path.split("/").filter((s) => s.length > 0);
  if (segments.length === 0) {
    throw new Error("empty OPFS chunk path");
  }
  let dir = await navigator.storage.getDirectory();
  for (let i = 0; i < segments.length - 1; i += 1) {
    dir = await dir.getDirectoryHandle(safeSeg(segments[i]), { create: true });
  }
  const leaf = safeSeg(segments[segments.length - 1]);
  const file = (await dir.getFileHandle(leaf, { create: true })) as SyncCapableFileHandle;
  return file.createSyncAccessHandle();
}

// Synchronously write `bytes` at byte offset `at`; returns bytes written.
export function writeChunk(handle: SyncAccessHandle, bytes: Uint8Array, at: number): void {
  handle.write(bytes, { at });
}

// Commit buffered writes to storage (call before `closeChunk` for durability).
export function flushChunk(handle: SyncAccessHandle): void {
  handle.flush();
}

// Release the exclusive lock (final).
export function closeChunk(handle: SyncAccessHandle): void {
  handle.close();
}
