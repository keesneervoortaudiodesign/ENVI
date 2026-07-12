// exportUi.test.ts — the GRID-05 export-dispatch contract (D-20/D-22). Drives
// `buildExportReq` + `downloadExport` with an INJECTED fake export client + download
// sink (no wasm, no DOM, no OPFS) and asserts: every format stamps the full attribution
// footer (CRS + weighting label + engine/scene identity + open-data attribution);
// `downloadExport` dispatches the correct `ExportFormat`; the download rides the sink
// (a Blob/objectURL path in the real sink) with a WASM-derived filename; and no acoustic
// arithmetic runs in TS (the bytes come from the injected encoder).

import { describe, expect, it, vi } from "vitest";

import type { ExportReq, ReceiverReadoutDto } from "../generated/wire";
import {
  DATA_ATTRIBUTION,
  ENGINE_VERSION,
  buildExportReq,
  downloadExport,
  exportBase,
  type ExportClient,
  type ExportContext,
  type DownloadSink,
} from "./exportUi";

const N_BANDS = 105;

function readoutFixture(seed: number): ReceiverReadoutDto {
  const band = Array.from({ length: N_BANDS }, (_, i) => 40 + ((i + seed) % 20));
  return {
    band_levels_db: band,
    coherent_db: band.map((v) => v - 1),
    incoherent_db: band.map((v) => v - 10),
    total_dba: 60 + seed,
    total_dbc: 63 + seed,
    total_coherent_db: 59 + seed,
    total_incoherent_db: 50 + seed,
  };
}

function contextFixture(): ExportContext {
  return {
    crs: { utm_zone: 31, south: false },
    weightingLabel: "dB(A)",
    tensorHash: "deadbeefcafef00d",
    grid: { rows: 2, cols: 2, origin: [500_000, 5_800_000], spacing_m: 10, values: [50, 55, 60, 65] },
    breaks: [-1e6, 55, 60, 65, 1e6],
    bandFills: ["#111111", "#222222", "#333333", "#444444"],
    receiverLabels: ["rcv-A", "rcv-B"],
    receivers: [readoutFixture(1), readoutFixture(2)],
  };
}

// A recording fake client + sink so the test asserts the dispatched request + download
// without touching wasm or the DOM.
function fakes(): {
  client: ExportClient;
  sink: DownloadSink;
  encoded: ExportReq[];
  saved: { filename: string; mime: string; bytes: Uint8Array }[];
} {
  const encoded: ExportReq[] = [];
  const saved: { filename: string; mime: string; bytes: Uint8Array }[] = [];
  const client: ExportClient = {
    encode: vi.fn(async (req: ExportReq) => {
      encoded.push(req);
      return new Uint8Array([1, 2, 3, 4]);
    }),
    filename: vi.fn(async (base: string, format) => `${base}.${format}`),
  };
  const sink: DownloadSink = {
    save: vi.fn((bytes: Uint8Array, filename: string, mime: string) => {
      saved.push({ filename, mime, bytes });
    }),
  };
  return { client, sink, encoded, saved };
}

describe("buildExportReq — the D-22 attribution footer on every format", () => {
  it("stamps CRS + weighting + engine/scene identity + attribution on all three formats", () => {
    const ctx = contextFixture();
    for (const format of ["geotiff", "geojson", "csv"] as const) {
      const req = buildExportReq(format, ctx);
      expect(req.crs).toEqual({ utm_zone: 31, south: false });
      expect(req.weighting_label).toBe("dB(A)");
      expect(req.engine_version).toBe(ENGINE_VERSION);
      expect(req.tensor_hash).toBe("deadbeefcafef00d");
      expect(req.attribution).toBe(DATA_ATTRIBUTION);
      expect(req.attribution).toContain("OpenStreetMap");
    }
  });

  it("selects the payload per format: grid for raster, breaks+fills for vector, receivers for CSV", () => {
    const ctx = contextFixture();

    const tif = buildExportReq("geotiff", ctx);
    expect(tif.format).toBe("geo_tiff");
    expect(tif.grid).not.toBeNull();
    expect(tif.breaks).toBeNull();
    expect(tif.receivers).toBeNull();

    const gj = buildExportReq("geojson", ctx);
    expect(gj.format).toBe("geo_json");
    expect(gj.grid).not.toBeNull();
    expect(gj.breaks).toEqual([-1e6, 55, 60, 65, 1e6]);
    expect(gj.band_fills).toEqual(["#111111", "#222222", "#333333", "#444444"]);
    expect(gj.receivers).toBeNull();

    const csv = buildExportReq("csv", ctx);
    expect(csv.format).toBe("csv");
    expect(csv.grid).toBeNull();
    expect(csv.receiver_labels).toEqual(["rcv-A", "rcv-B"]);
    expect(csv.receivers).toHaveLength(2);
  });
});

describe("downloadExport — client dispatch + Blob download", () => {
  it("dispatches each format's ExportFormat to the encoder and saves via the sink", async () => {
    for (const [format, tag] of [
      ["geotiff", "geo_tiff"],
      ["geojson", "geo_json"],
      ["csv", "csv"],
    ] as const) {
      const { client, sink, encoded, saved } = fakes();
      await downloadExport(format, { client, sink, context: contextFixture() });
      expect(encoded).toHaveLength(1);
      expect(encoded[0].format).toBe(tag);
      // The footer rode along on the dispatched request.
      expect(encoded[0].attribution).toBe(DATA_ATTRIBUTION);
      expect(encoded[0].weighting_label).toBe("dB(A)");
      // The download saved the encoder's bytes with a WASM-derived filename.
      expect(saved).toHaveLength(1);
      expect(saved[0].bytes).toEqual(new Uint8Array([1, 2, 3, 4]));
      expect(saved[0].filename).toContain(exportBase("deadbeefcafef00d"));
      expect(saved[0].filename).toContain(tag);
    }
  });

  it("derives the filename base from the scene identity (V12 program-derived name)", () => {
    expect(exportBase("deadbeefcafef00d")).toBe("envi-results-deadbeef");
    // An empty identity still yields a safe stem.
    expect(exportBase("")).toBe("envi-results-export");
  });
});
