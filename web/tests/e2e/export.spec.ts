// export.spec.ts — the GRID-05 results EXPORT offline UAT (SC5).
//
// Drives the REAL vite-served bundle (the project's e2e definition of the real app; the
// dev server sends COOP/COEP so `crossOriginIsolated` holds and the compute wasm — incl.
// the 11-04 `export`/`export_filename` encoders — instantiates) in headless Chromium,
// fully offline: `/api/*` is route-mocked and a fixture tensor + level grid are seeded via
// the DEV bridge. NOTHING is reimplemented — the download bytes are produced by the REAL
// WASM export encoder (D-01/D-20).
//
// Asserts SC5: each of GeoTIFF / GeoJSON / CSV triggers a Blob download (a blob: URL — no
// network request) of NON-EMPTY bytes carrying the full attribution/metadata footer (CRS
// EPSG + open-data attribution); the GeoJSON parses as a valid FeatureCollection; the CSV
// carries band-index + exact-Hz columns; and the menu is disabled with no result and when
// stale.

import { readFile } from "node:fs/promises";

import { expect, test, type Download, type Page } from "@playwright/test";

import { bootOffline, installMetaMocks } from "./_mocks";

// Open the export menu, trigger one format's download, and return the saved bytes + the
// download handle. Waits for the CTA to be enabled first (a prior export cleared its busy
// state), so consecutive exports never race the disabled button.
async function grabExport(
  page: Page,
  format: "geotiff" | "geojson" | "csv",
): Promise<{ download: Download; bytes: Buffer }> {
  await expect(page.getByTestId("export-open")).toBeEnabled();
  await page.getByTestId("export-open").click();
  await expect(page.getByTestId("export-menu-list")).toBeVisible();
  const downloadPromise = page.waitForEvent("download");
  await page.getByTestId(`export-${format}`).click();
  const download = await downloadPromise;
  const path = await download.path();
  const bytes = await readFile(path);
  return { download, bytes };
}

test("all three formats download offline with attribution, and the menu gates on result + stale", async ({
  page,
}) => {
  const unmocked = await bootOffline(page);
  await installMetaMocks(page);

  // The export menu is mounted; with no result the CTA is disabled (UI-SPEC state matrix).
  await expect(page.getByTestId("export-menu")).toBeVisible();
  await expect(page.getByTestId("export-open")).toBeDisabled();

  // Seed a fixture tensor (3 receivers, one chunk) keyed by the REAL minted tensor
  // identity AND a cached level grid + CRS + weighting — the SAME store paths a finished
  // solve/readout feeds. With both present, a result exists and the CTA enables.
  const ids = await page.evaluate(() => window.__enviTest.seedResults(3));
  expect(ids).toHaveLength(3);
  await page.evaluate(() => window.__enviTest.seedIsophone());
  await expect(page.getByTestId("export-open")).toBeEnabled();

  // The menu offers exactly the three formats.
  await page.getByTestId("export-open").click();
  await expect(page.getByTestId("export-menu-list")).toBeVisible();
  await expect(page.getByTestId("export-geotiff")).toBeVisible();
  await expect(page.getByTestId("export-geojson")).toBeVisible();
  await expect(page.getByTestId("export-csv")).toBeVisible();
  // Close it again (Escape via a second CTA click) so the helper reopens cleanly.
  await page.getByTestId("export-open").click();

  // --- GeoTIFF: a non-empty little-endian TIFF carrying the metadata footer ---
  {
    const { download, bytes } = await grabExport(page, "geotiff");
    expect(download.suggestedFilename()).toMatch(/\.tif$/);
    expect(bytes.length).toBeGreaterThan(0);
    // Little-endian TIFF magic (II, 42).
    expect(bytes[0]).toBe(0x49);
    expect(bytes[1]).toBe(0x49);
    // The ImageDescription footer carries the CRS EPSG + open-data attribution (D-22).
    const ascii = bytes.toString("latin1");
    expect(ascii).toContain("EPSG:32631");
    expect(ascii).toContain("OpenStreetMap");
  }

  // --- GeoJSON: a valid RFC-7946 FeatureCollection with the attribution footer ---
  {
    const { download, bytes } = await grabExport(page, "geojson");
    expect(download.suggestedFilename()).toMatch(/\.geojson$/);
    const text = bytes.toString("utf-8");
    const fc = JSON.parse(text) as { type: string; features: unknown[] };
    expect(fc.type).toBe("FeatureCollection");
    expect(Array.isArray(fc.features)).toBe(true);
    expect(fc.features.length).toBeGreaterThan(0);
    // The metadata/attribution footer rides as FeatureCollection foreign members (D-22).
    expect(text).toContain("EPSG:32631");
    expect(text).toContain("OpenStreetMap");
  }

  // --- CSV: band-index + exact-Hz spectra columns + the attribution footer ---
  {
    const { download, bytes } = await grabExport(page, "csv");
    expect(download.suggestedFilename()).toMatch(/\.csv$/);
    const text = bytes.toString("utf-8");
    // The band identity is BAND INDEX + exact Hz (never nominal Hz alone).
    expect(text).toContain("band_index,exact_hz");
    // A receiver column per seeded receiver (the TS-minted UUIDs).
    for (const id of ids) {
      expect(text).toContain(id);
    }
    // The dB(A)/dB(C) totals + the `#`-comment attribution footer (D-22).
    expect(text).toContain("dBA_total");
    expect(text).toContain("# Attribution:");
    expect(text).toContain("OpenStreetMap");
  }

  // --- Stale gating: a scene edit since the solve disables the export (UI-SPEC) ---
  await page.evaluate(() => window.__enviTest.divergeScene());
  await expect(page.getByTestId("export-open")).toBeDisabled();

  // Nothing touched the network the whole run — every byte was generated in WASM and
  // downloaded via a blob: URL (D-20, nothing leaves the device).
  expect(unmocked, `unmocked: ${unmocked.join(", ")}`).toEqual([]);
});
