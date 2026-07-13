// cogWindow.ts — the WINDOWED COG range read: the two-pass "fetch the header, then only the tiles the
// viewport overlaps" driver, and the OPFS range-pack that caches what it fetched.
//
// # The defect this replaces
//
// Import used to issue a plain whole-tile GET. For an Amsterdam viewport that meant
// `.../dtm_05m/M_25GN1.tif` — `Content-Length: 346,218,632`, **330 MB** — to serve a ~1 km² window (and
// 54 MB of ESA WorldCover for the same reason). The import hung. A Cloud-Optimised GeoTIFF exists precisely
// so a client can read the header, work out which internal tiles overlap its window, and fetch ONLY those
// tiles' byte ranges; both upstreams answer `206 Partial Content` (verified live). Nothing but the missing
// planner stood in the way.
//
// # Module I/O
// - Input  a project id, a covering `TileRefDto` + its `CorsDto` reachability, the WGS84 viewport, and the
//   tile's source CRS.
// - Output `loadCogWindow` resolves `{ bytes, parts, window }` — the concatenated fetched byte parts, the
//   manifest describing where each part sits in the file, and the pixel window to decode — or `null` when
//   the viewport does not overlap the tile. That triple is exactly what every byte-taking WASM entry point
//   (`terrain_features` / `map_landcover` / `sample_base_elevation`) now consumes.
// - Valid input range: whatever `plan_tiles` emitted. ALL the GIS reasoning (header-fit, viewport → pixel
//   window, window → chunk byte ranges, cache subtraction, budgets) lives in WASM (`plan_cog_reads`); this
//   module owns only the side effects — `fetch` and OPFS — per the sans-I/O boundary.
//
// # Invariants (load-bearing)
// 1. **Two passes, no fallback.** Pass 1 fetches a header prefix; if the IFD reaches past it the planner
//    says exactly how many bytes it needs and we re-fetch (bounded — it converges, and `MAX_HEADER_BYTES`
//    on the Rust side refuses to creep into a whole-file download). Pass 2 fetches only the planned ranges.
//    There is NO "give up and GET the file" path anywhere: that was the bug.
// 2. **One single-range request per planned range** (`fetchTileRange`) — a multi-range GET is not
//    CORS-safelisted and PDOK's preflight would reject it. The planner coalesces adjacent COG tiles, so
//    the request count stays small.
// 3. **DATA-04 still holds.** The fetched ranges are cached per project in OPFS under the SAME
//    `cache/<source>/<tile>` key as before (as a self-describing *range pack*), and the planner subtracts
//    the cached ranges — so a re-import of the same viewport plans ZERO fetches and the compute path reads
//    from OPFS with the network off. A legacy whole-tile cache entry (a raw TIFF) is still readable: it is
//    just the degenerate one-part-at-offset-0 pack.

import type { BboxDto, BytePartDto, CorsDto, PixelWindowDto, TerrainSourceCrsDto, TileRefDto } from "../generated/wire";
import { ApiError } from "../api/client";
import { fetchTileRange, type FetchedRange } from "./fetchers";
import { estimateQuota, fitsQuota, getTile, putTile } from "./opfs";
import { planCogReads } from "./wasm";

// The header prefix fetched on pass 1. Mirrors `envi_gis::cog::plan::DEFAULT_HEADER_PREFIX_BYTES` — a
// GDAL-written COG keeps its whole IFD chain (including the TileOffsets/TileByteCounts arrays and the geo
// tags) at the front of the file; AHN's 330 MB BigTIFF needs well under 100 KB of it. When it is not enough
// the planner returns `need_header` and we re-fetch exactly that many bytes.
const HEADER_PREFIX_BYTES = 256 * 1024;

// Bound on the header re-ask loop. The Rust planner's ask grows strictly and is capped, so this only guards
// against a pathological upstream; two rounds is already more than any real COG needs.
const MAX_HEADER_ROUNDS = 4;

// Cap on a cached range pack. The pack grows as a project imports different viewports of the same tile; past
// this, the older ranges are dropped and the pack is rebuilt from the header + the newest fetch (the cache
// is an optimisation, never a correctness input — a dropped range is simply re-fetched later).
const MAX_PACK_BYTES = 96 * 1024 * 1024;

// --- the range pack (the on-OPFS format) ------------------------------------

// A partially-fetched tile: the concatenated bytes of `parts`, in `parts` order. This is precisely the
// `(bytes, parts)` pair the WASM boundary takes — one typed-array copy, no per-part marshalling.
export interface CogPack {
  readonly bytes: Uint8Array;
  readonly parts: BytePartDto[];
}

// The window of a viewport inside a tile, plus the bytes needed to decode it.
export interface CogWindow extends CogPack {
  readonly window: PixelWindowDto;
}

// `ENVICOG1` — the pack magic. A cached file that does NOT start with it is a legacy whole-tile TIFF, read
// back as the degenerate single-part pack (so an existing project's cache keeps working).
const MAGIC = new Uint8Array([0x45, 0x4e, 0x56, 0x49, 0x43, 0x4f, 0x47, 0x31]);
const HEADER_FIXED = MAGIC.length + 4; // magic + u32 part count
const PART_ENTRY = 16; // f64 offset + f64 len

function hasMagic(view: Uint8Array): boolean {
  return view.length >= HEADER_FIXED && MAGIC.every((b, i) => view[i] === b);
}

// Serialise a pack: magic | u32 partCount | partCount × (f64 offset, f64 len) | payload.
function encodePack(pack: CogPack): ArrayBuffer {
  const size = HEADER_FIXED + pack.parts.length * PART_ENTRY + pack.bytes.length;
  const buf = new ArrayBuffer(size);
  const view = new DataView(buf);
  const bytes = new Uint8Array(buf);
  bytes.set(MAGIC, 0);
  view.setUint32(MAGIC.length, pack.parts.length, true);
  let at = HEADER_FIXED;
  for (const p of pack.parts) {
    view.setFloat64(at, p.offset, true);
    view.setFloat64(at + 8, p.len, true);
    at += PART_ENTRY;
  }
  bytes.set(pack.bytes, at);
  return buf;
}

// Parse a cached entry back into a pack. A legacy raw-TIFF entry becomes one part at offset 0. A corrupt
// pack resolves to `null` (a cache miss, never a throw — the caller just re-fetches).
function decodePack(buf: ArrayBuffer): CogPack | null {
  const all = new Uint8Array(buf);
  if (!hasMagic(all)) {
    // Legacy whole-tile cache entry (or any raw TIFF): one part covering the file.
    return all.length > 0 ? { bytes: all, parts: [{ offset: 0, len: all.length }] } : null;
  }
  const view = new DataView(buf);
  const count = view.getUint32(MAGIC.length, true);
  const payloadAt = HEADER_FIXED + count * PART_ENTRY;
  if (payloadAt > all.length) {
    return null; // truncated pack — treat as a miss
  }
  const parts: BytePartDto[] = [];
  let declared = 0;
  for (let i = 0; i < count; i++) {
    const at = HEADER_FIXED + i * PART_ENTRY;
    const offset = view.getFloat64(at, true);
    const len = view.getFloat64(at + 8, true);
    if (!Number.isSafeInteger(offset) || !Number.isSafeInteger(len) || len <= 0) {
      return null;
    }
    parts.push({ offset, len });
    declared += len;
  }
  const bytes = all.subarray(payloadAt);
  if (declared !== bytes.length) {
    return null; // manifest and payload disagree — treat as a miss
  }
  return { bytes, parts };
}

// Sort the fetched pieces by file offset and fuse overlapping / exactly-adjacent ones, so the resulting
// parts are disjoint and each COG chunk lies wholly inside one part (what the Rust sparse view requires).
// Later bytes win on an overlap (a re-fetch is the fresher read).
function fusePieces(pieces: FetchedRange[]): CogPack {
  const sorted = [...pieces].filter((p) => p.bytes.length > 0).sort((a, b) => a.offset - b.offset);
  const merged: FetchedRange[] = [];
  for (const piece of sorted) {
    const prev = merged[merged.length - 1];
    const prevEnd = prev ? prev.offset + prev.bytes.length : -1;
    if (!prev || piece.offset > prevEnd) {
      merged.push({ offset: piece.offset, bytes: piece.bytes });
      continue;
    }
    // Overlapping or adjacent: extend `prev` with whatever of `piece` reaches past it.
    const pieceEnd = piece.offset + piece.bytes.length;
    if (pieceEnd <= prevEnd) {
      continue; // wholly contained
    }
    const tail = piece.bytes.subarray(prevEnd - piece.offset);
    const fused = new Uint8Array(prev.bytes.length + tail.length);
    fused.set(prev.bytes, 0);
    fused.set(tail, prev.bytes.length);
    merged[merged.length - 1] = { offset: prev.offset, bytes: fused };
  }

  const total = merged.reduce((n, p) => n + p.bytes.length, 0);
  const bytes = new Uint8Array(total);
  const parts: BytePartDto[] = [];
  let at = 0;
  for (const p of merged) {
    bytes.set(p.bytes, at);
    at += p.bytes.length;
    parts.push({ offset: p.offset, len: p.bytes.length });
  }
  return { bytes, parts };
}

// The pack's parts as the byte ranges the planner subtracts (`have`).
function haveRanges(pack: CogPack | null): { start: number; end: number }[] {
  return (pack?.parts ?? []).map((p) => ({ start: p.offset, end: p.offset + p.len }));
}

// The pack's pieces, ready to be fused with freshly fetched ones.
function piecesOf(pack: CogPack): FetchedRange[] {
  const out: FetchedRange[] = [];
  let at = 0;
  for (const p of pack.parts) {
    out.push({ offset: p.offset, bytes: pack.bytes.subarray(at, at + p.len) });
    at += p.len;
  }
  return out;
}

// --- OPFS ------------------------------------------------------------------

async function readPack(projectId: string, tile: TileRefDto): Promise<CogPack | null> {
  const buf = await getTile(projectId, tile.source_id, tile.tile);
  return buf === null ? null : decodePack(buf);
}

// Persist the pack (best-effort — an exhausted quota degrades to "no cache", never a failed import; the
// bytes we just fetched are already in hand).
async function writePack(projectId: string, tile: TileRefDto, pack: CogPack): Promise<void> {
  const encoded = encodePack(pack);
  if (!fitsQuota(await estimateQuota(), encoded.byteLength)) {
    return;
  }
  try {
    await putTile(projectId, tile.source_id, tile.tile, encoded);
  } catch {
    /* quota / no-OPFS: the cache is an optimisation, not a correctness input */
  }
}

// --- the two-pass driver ----------------------------------------------------

// Load exactly the bytes needed to decode `bbox` out of `tile`, cache-first.
//
// Pass 1 gets the header (from OPFS if the cache already holds it), pass 2 gets only the COG tiles the
// viewport overlaps — minus whatever the cache already covers, so a warm cache issues NO request at all
// (DATA-04). Returns `null` when the viewport does not overlap this tile.
export async function loadCogWindow(
  projectId: string,
  tile: TileRefDto,
  cors: CorsDto,
  bbox: BboxDto,
  sourceCrs: TerrainSourceCrsDto,
  signal: AbortSignal,
): Promise<CogWindow | null> {
  let pack = await readPack(projectId, tile);
  let fetched = false;

  for (let round = 0; ; round++) {
    // --- Pass 1: the header. Fetched ONLY when we hold none — a cached pack already carries its header
    // prefix, however short, and the planner (not a length guess here) decides whether it suffices. A
    // `header.length < HEADER_PREFIX_BYTES` test would re-fetch every small tile forever and break the
    // DATA-04 network-off replay.
    let header = headerPrefixOf(pack);
    if (header === null) {
      pack = fusePieces([
        ...(pack ? piecesOf(pack) : []),
        await fetchTileRange(tile, cors, 0, HEADER_PREFIX_BYTES, signal),
      ]);
      if (signal.aborted) return null;
      fetched = true;
      header = headerPrefixOf(pack);
    }
    if (header === null || pack === null) {
      throw new ApiError(0, "This source tile returned no header bytes.");
    }

    const plan = await planCogReads(header, {
      bbox,
      source_crs: sourceCrs,
      have: haveRanges(pack),
      max_decoded_px: null,
      max_fetch_bytes: null,
    });

    // `serde_wasm_bindgen` marshals a Rust `Option::None` as `undefined`, NOT `null` (the generated
    // `wire.ts` still types it `T | null`). Normalise BOTH to null here — a bare `!== null` test would read
    // `undefined` as "a value", and this loop would then re-ask for the header until it hit its cap.
    const needHeader = plan.need_header ?? null;
    const window = plan.window ?? null;

    if (needHeader !== null) {
      // The IFD (or one of its out-of-line tag arrays) reaches past what we hold — the planner says
      // exactly how far. Fetch precisely that prefix and re-plan (the ask grows strictly, so this ends).
      if (round + 1 >= MAX_HEADER_ROUNDS) {
        throw new ApiError(0, "This source tile's header could not be read in a bounded number of reads.");
      }
      pack = fusePieces([
        ...piecesOf(pack),
        await fetchTileRange(tile, cors, 0, needHeader, signal),
      ]);
      if (signal.aborted) return null;
      fetched = true;
      continue;
    }

    if (window === null) {
      // The viewport does not overlap this tile. Keep the header we fetched (it is cheap and makes a
      // neighbouring viewport's import a pure cache hit), then report "nothing here".
      if (fetched) {
        await writePack(projectId, tile, pack);
      }
      return null;
    }

    // --- Pass 2: fetch ONLY the planned ranges (one simple-range GET each). An empty plan means the
    // cache already covers the window: zero requests, which is what DATA-04's network-off replay needs.
    const pieces: FetchedRange[] = piecesOf(pack);
    for (const range of plan.fetch) {
      pieces.push(await fetchTileRange(tile, cors, range.start, range.end, signal));
      if (signal.aborted) return null;
      fetched = true;
    }
    let next = fusePieces(pieces);

    // Keep the cached pack bounded: past the cap, drop the history and keep the header + this window.
    if (next.bytes.length > MAX_PACK_BYTES) {
      const head = headerPrefixOf(next);
      const windowPieces = piecesOf(next).filter((p) => p.offset !== 0);
      next = fusePieces([
        ...(head ? [{ offset: 0, bytes: head }] : []),
        ...windowPieces.slice(-Math.max(plan.fetch.length, 1)),
      ]);
    }

    if (fetched) {
      await writePack(projectId, tile, next);
      // Read back from OPFS so the decode path consumes the CACHE, not the in-flight bytes (DATA-04's
      // read-back seam). A cache miss here (quota/no-OPFS) falls back to what we hold.
      const readBack = await readPack(projectId, tile);
      if (readBack) {
        next = readBack;
      }
    }
    return { ...next, window };
  }
}

// The bytes of the part anchored at file offset 0 (the TIFF header), or `null` if the pack has none.
function headerPrefixOf(pack: CogPack | null): Uint8Array | null {
  if (!pack || pack.parts.length === 0 || pack.parts[0].offset !== 0) {
    return null;
  }
  return pack.bytes.subarray(0, pack.parts[0].len);
}
