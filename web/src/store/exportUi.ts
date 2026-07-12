// exportUi.ts — the results EXPORT dispatch (GRID-05 / D-20/D-21/D-22). Assembles an
// `ExportReq` from the cached client state (the isophone level grid + colour scale +
// CRS from `colorScale`, the tensor identity + per-receiver readouts from `results`),
// invokes the 11-04 `envi-compute-wasm::export` encoder, wraps the returned bytes in a
// `Blob`, and triggers a browser download via an object URL — nothing leaves the device
// (D-20). Every export carries the full metadata/attribution footer (CRS + dB weighting
// label + engine/scene identity + open-data attribution, D-22).
//
// # Module I/O
// - Input  the colour-scale store (cached `grid`/`crs`/`breaks`/`colors`/`weightingLabel`)
//   and the results store (`manifest.tensorHash` + the per-receiver `ReceiverReadoutDto`
//   readouts, read through the SAME `ReadoutClient` seam the spectrum panel uses).
// - Output a browser download: the WASM-encoded bytes of the selected `ExportFormat`
//   (GeoTIFF raster / GeoJSON isophones / CSV spectra) saved via a `Blob` + object URL
//   with a program-derived, WASM-sanitized filename (V12, T-11-09-02).
// - Valid input range: a present results manifest + a cached grid/CRS (the ExportMenu
//   gates the affordance on this and on the stale badge).
//
// # D-01 / D-20 — ZERO acoustic math here
// Every byte is generated in WASM: this module marshals the request, calls the WASM
// `export` encoder (and the WASM `readout_receivers` core for the CSV spectra, via the
// results `ReadoutClient`), and triggers the download. There is NO log/pow/exp dB
// arithmetic in this file — TS assembles the request and downloads the returned bytes.

import type {
  ExportCrsDto,
  ExportFormat,
  ExportGridDto,
  ExportReq,
  ReceiverReadoutDto,
} from "../generated/wire";
import { contourBreaks, useColorScaleStore } from "./colorScale";
import { useResultsStore, type ResultsState } from "./results";

// The engine version string stamped into every export footer (scene identity, D-22).
// Half of the reproducibility key alongside the scene `tensor_hash`; matches the
// engine-side convention used by the export encoders (`ExportMeta.engine_version`).
export const ENGINE_VERSION = "envi 0.1.0";

// The open-data attribution stamped into every export footer (D-22). Honours each
// source's attribution requirement (OSM / Overture / ESA WorldCover / Copernicus);
// mirrors the UI-SPEC export-footer copy verbatim (English-only).
export const DATA_ATTRIBUTION = "© OpenStreetMap / Overture / ESA WorldCover / Copernicus";

// The UI-facing export format keys (the ExportMenu items), mapped to the generated
// `ExportFormat` wire tags (`geo_tiff`/`geo_json`/`csv`).
export type UiExportFormat = "geotiff" | "geojson" | "csv";

const WIRE_FORMAT: Record<UiExportFormat, ExportFormat> = {
  geotiff: "geo_tiff",
  geojson: "geo_json",
  csv: "csv",
};

// The MIME type of each downloaded artifact (the Blob content-type).
const MIME: Record<UiExportFormat, string> = {
  geotiff: "image/tiff",
  geojson: "application/geo+json",
  csv: "text/csv",
};

// The pieces the ExportReq footer + payload are assembled from — gathered from the
// client stores (or injected in tests). Kept as a plain value object so `buildExportReq`
// is a pure, unit-testable function.
export interface ExportContext {
  // The project's pinned CRS (EPSG source + the reprojection seam).
  readonly crs: ExportCrsDto;
  // The dB weighting label from result metadata (never the panel toggle).
  readonly weightingLabel: string;
  // The frozen tensor-identity hash (scene identity, D-09/D-22).
  readonly tensorHash: string;
  // The cached level grid (GeoTIFF raster + GeoJSON contour source).
  readonly grid: ExportGridDto | null;
  // The CAP-extended contour break edges (GeoJSON) — the SAME edges the live isophone
  // layer contours, so the export matches the on-screen map.
  readonly breaks: readonly number[];
  // The per-band class fill colours (GeoJSON), aligned to the traced bands.
  readonly bandFills: readonly string[];
  // The CSV receiver column labels (TS-minted UUIDs), aligned to `receivers`.
  readonly receiverLabels: readonly string[];
  // The per-receiver readout spectra for the CSV (every dB WASM-produced, D-01).
  readonly receivers: readonly ReceiverReadoutDto[];
}

// Assemble the `ExportReq` for a format from the gathered context (PURE). The footer
// fields (CRS + weighting + engine/scene identity + attribution) are stamped on EVERY
// format (D-22); the payload fields are format-selected (grid for raster/vector, breaks
// + fills for vector, receivers for CSV) — a payload the format does not need is left
// null/empty so the WASM encoder never reads a stale field.
export function buildExportReq(format: UiExportFormat, ctx: ExportContext): ExportReq {
  return {
    format: WIRE_FORMAT[format],
    crs: ctx.crs,
    weighting_label: ctx.weightingLabel,
    engine_version: ENGINE_VERSION,
    tensor_hash: ctx.tensorHash,
    attribution: DATA_ATTRIBUTION,
    grid: format === "csv" ? null : ctx.grid,
    breaks: format === "geojson" ? [...ctx.breaks] : null,
    band_fills: format === "geojson" ? [...ctx.bandFills] : [],
    receiver_labels: format === "csv" ? [...ctx.receiverLabels] : [],
    receivers: format === "csv" ? [...ctx.receivers] : null,
  };
}

// A program-derived download filename base (sanitized in WASM by `export_filename`,
// V12 / T-11-09-02). Ties the file to the scene identity without leaking a path.
export function exportBase(tensorHash: string): string {
  const short = tensorHash.slice(0, 8);
  return `envi-results-${short.length > 0 ? short : "export"}`;
}

// --- The WASM export client seam (browser-only; injectable for tests) ------------

// Invokes the 11-04 `envi-compute-wasm::export` boundary. `encode` returns the
// download bytes for a request; `filename` returns the WASM-sanitized download name.
export interface ExportClient {
  encode(req: ExportReq): Promise<Uint8Array>;
  filename(base: string, format: ExportFormat): Promise<string>;
}

// The real client: lazily instantiates the compute wasm module on the MAIN THREAD (the
// same module the readout / trace / identity seams use — the dev/prod server sends
// COOP/COEP so `crossOriginIsolated` holds), and calls the `export` / `export_filename`
// exports. `export` is exposed as `_export` by wasm-bindgen (`export` is a reserved JS
// word). A dynamic import inside the factory keeps the wasm graph out of the Node
// unit-test module load (mirrors the results `ReadoutClient`).
export function createWasmExportClient(): ExportClient {
  let glue: Promise<typeof import("../generated/wasm-compute/envi_compute_wasm")> | null = null;
  const ensureGlue = (): Promise<typeof import("../generated/wasm-compute/envi_compute_wasm")> => {
    glue ??= (async () => {
      const g = await import("../generated/wasm-compute/envi_compute_wasm");
      await g.default();
      return g;
    })();
    return glue;
  };
  return {
    async encode(req) {
      const g = await ensureGlue();
      return g._export(req) as Uint8Array;
    },
    async filename(base, format) {
      const g = await ensureGlue();
      return g.export_filename(base, format) as string;
    },
  };
}

// Single module-level client (browser-only), kept out of the stores so they stay
// Node-testable (mirrors the isophone-layer trace-client seam).
let exportClient: ExportClient | null = null;
function ensureClient(): ExportClient {
  exportClient ??= createWasmExportClient();
  return exportClient;
}

// --- The browser download sink (Blob + object URL; injectable for tests) ---------

// Triggers a browser download of `bytes` under `filename` — a `Blob` + `URL.createObjectURL`
// anchor click, then revokes the URL (D-20: nothing leaves the device; no server request).
export interface DownloadSink {
  save(bytes: Uint8Array, filename: string, mime: string): void;
}

// The real browser sink: wrap the WASM bytes in a `Blob`, mint an object URL, click a
// hidden `<a download>` to trigger the save, and revoke the URL. No network egress.
export function browserDownloadSink(): DownloadSink {
  return {
    save(bytes, filename, mime) {
      // Copy into a plain ArrayBuffer-backed view: the threaded compute wasm may return
      // a SharedArrayBuffer-backed Uint8Array, which is not a valid `BlobPart`.
      const copy = new Uint8Array(bytes);
      const blob = new Blob([copy], { type: mime });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = filename;
      anchor.rel = "noopener";
      document.body.appendChild(anchor);
      anchor.click();
      anchor.remove();
      // Defer the revoke: revoking synchronously right after click() can truncate the
      // download before the browser's download manager has read the Blob.
      setTimeout(() => URL.revokeObjectURL(url), 0);
    },
  };
}

// --- CSV readout collection (WASM-produced spectra, D-01) -------------------------

// Collect the per-receiver readout spectra for the CSV export, in manifest receiver
// order. A receiver whose readout is already cached (the user viewed it) is reused; an
// uncached one is fetched through the SAME `ReadoutClient` the spectrum panel uses (the
// WASM `readout_receivers` core over the OPFS chunk) — so the CSV covers EVERY receiver,
// every dB WASM-produced (D-01), with no acoustic math in TS.
async function collectReadouts(
  state: ResultsState,
): Promise<{ labels: string[]; readouts: ReceiverReadoutDto[] }> {
  const { manifest, readouts, client } = state;
  const labels: string[] = [];
  const out: ReceiverReadoutDto[] = [];
  if (!manifest) {
    return { labels, readouts: out };
  }
  for (const r of manifest.receivers) {
    let readout = readouts[r.id];
    if (!readout && client) {
      const span = manifest.spans.find((s) => s.chunkIndex === r.chunkIndex);
      if (span) {
        readout = await client.readout({
          projectId: manifest.projectId,
          tensorHash: manifest.tensorHash,
          scene: manifest.scene,
          perSourceConditioning: manifest.perSourceConditioning,
          chunkIndex: r.chunkIndex,
          rLocal: r.rLocal,
          chunkReceiverIds: span.receiverIds,
        });
      }
    }
    if (readout) {
      labels.push(r.id);
      out.push(readout);
    }
  }
  return { labels, readouts: out };
}

// Gather the export context from the client stores (reads state; for CSV, fetches the
// receiver spectra through the WASM readout seam). Throws an honest error when there is
// nothing to export (no result) or no CRS is pinned yet.
async function gatherExportContext(format: UiExportFormat): Promise<ExportContext> {
  const scale = useColorScaleStore.getState();
  const results = useResultsStore.getState();
  if (!results.manifest) {
    throw new Error("No result to export yet. Run a calculation first.");
  }
  if (!scale.crs) {
    throw new Error("No CRS available for export. Compute the noise map first.");
  }
  let receiverLabels: string[] = [];
  let receivers: ReceiverReadoutDto[] = [];
  if (format === "csv") {
    const collected = await collectReadouts(results);
    receiverLabels = collected.labels;
    receivers = collected.readouts;
  }
  return {
    crs: scale.crs,
    weightingLabel: scale.weightingLabel,
    tensorHash: results.manifest.tensorHash,
    grid: scale.grid,
    breaks: contourBreaks(scale.breaks),
    bandFills: scale.colors,
    receiverLabels,
    receivers,
  };
}

// --- The public dispatch ----------------------------------------------------------

// Optional injected collaborators (tests supply a fake client + sink + context; the app
// uses the real WASM client + browser sink + the store-gathered context).
export interface DownloadDeps {
  readonly client?: ExportClient;
  readonly sink?: DownloadSink;
  readonly context?: ExportContext;
}

// Export the current result as `format` and download it entirely client-side (GRID-05,
// D-20): assemble the `ExportReq` (with the full attribution footer, D-22), invoke the
// WASM encoder for the bytes, and trigger a `Blob` download with a WASM-sanitized
// filename. All bytes are produced in WASM — TS marshals the request and saves the file.
export async function downloadExport(format: UiExportFormat, deps: DownloadDeps = {}): Promise<void> {
  const client = deps.client ?? ensureClient();
  const sink = deps.sink ?? browserDownloadSink();
  const context = deps.context ?? (await gatherExportContext(format));
  const req = buildExportReq(format, context);
  const bytes = await client.encode(req);
  const filename = await client.filename(exportBase(context.tensorHash), req.format);
  sink.save(bytes, filename, MIME[format]);
}
