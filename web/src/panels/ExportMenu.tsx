// ExportMenu.tsx — the results EXPORT menu (GRID-05 / D-20/D-21/D-22). The "Export…"
// primary CTA opens a `.menu` of the three self-describing formats — GeoTIFF (raster
// level grid), GeoJSON (isophone fill polygons), CSV (spectra: band index + exact Hz).
// Each item invokes `downloadExport(format)`, which generates the bytes in WASM and
// triggers a Blob download — nothing leaves the device (D-20), and every file carries
// the full attribution footer (D-22). Fills the 11-05 ResultsPanel `ExportMenu` stub.
//
// # Module I/O
// - Input  the results manifest (result exists?), the colour-scale store (a cached grid
//   + CRS — the raster/vector export source), and the stale badge (D-12). No props.
// - Output the "Export…" menu JSX. Honest states (UI-SPEC §State Matrix): disabled until
//   a result exists; disabled while stale; a "Generating…" busy label; an inline
//   encode-error message on failure. NO acoustic math here — the bytes are WASM-produced.

import { useState, type ReactElement } from "react";

import { useResultsStore } from "../store/results";
import { useColorScaleStore } from "../store/colorScale";
import { useStaleStore } from "../store/stale";
import { downloadExport, type UiExportFormat } from "../store/exportUi";
import { InfoButton } from "../help/InfoButton";
import type { ControlId } from "../help/controlIds";

// The three export formats + their menu copy (UI-SPEC §Export menu inventory; sentence
// case, English-only, units/identity shown). Each carries an explicit `controlId` so the
// InfoButton coverage is a `tsc` guarantee — no `as ControlId` cast that could hide a
// missing catalog entry (WR-01 / D-25).
const FORMATS: { id: UiExportFormat; controlId: ControlId; label: string; hint: string }[] = [
  { id: "geotiff", controlId: "export.geotiff", label: "GeoTIFF", hint: "raster level grid" },
  { id: "geojson", controlId: "export.geojson", label: "GeoJSON", hint: "isophone polygons" },
  { id: "csv", controlId: "export.csv", label: "CSV", hint: "spectra: band index + exact Hz" },
];

export function ExportMenu(): ReactElement {
  const hasManifest = useResultsStore((s) => s.manifest !== null);
  // A raster/vector export needs the cached level grid + its CRS (fed by a finished
  // readout); this also pins the CRS the CSV footer stamps.
  const hasGrid = useColorScaleStore((s) => s.grid !== null && s.crs !== null);
  const isStale = useStaleStore((s) => s.isStale);

  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const hasResult = hasManifest && hasGrid;
  // Disabled until a result exists, while stale, or while a download is generating
  // (UI-SPEC §State Matrix: export row).
  const disabled = !hasResult || isStale || busy;

  const onExport = async (format: UiExportFormat): Promise<void> => {
    setOpen(false);
    setError(null);
    setBusy(true);
    try {
      await downloadExport(format);
    } catch (err) {
      // Honest encode-error state (T-11-09-03): surface the failure, no silent partial file.
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="panel-section export-menu" data-testid="export-menu">
      <div className="panel-header">
        <span className="section-title">Export</span>
      </div>

      <div className="menu-wrap">
        <button
          type="button"
          className="btn primary"
          data-testid="export-open"
          aria-haspopup="menu"
          aria-expanded={open}
          disabled={disabled}
          onClick={() => setOpen((o) => !o)}
        >
          {busy ? "Generating…" : "Export…"}
        </button>
        <InfoButton controlId="export.open" />

        {open && !disabled ? (
          <div className="menu export-menu-list" role="menu" data-testid="export-menu-list">
            {FORMATS.map((f) => (
              <div
                key={f.id}
                className="export-menu-item-row"
                style={{ display: "flex", alignItems: "center" }}
              >
                <button
                  type="button"
                  className="menu-item export-menu-item"
                  style={{ flex: 1 }}
                  role="menuitem"
                  data-testid={`export-${f.id}`}
                  onClick={() => void onExport(f.id)}
                >
                  <span className="export-menu-label">{f.label}</span>
                  <span className="export-menu-hint mono">{f.hint}</span>
                </button>
                <InfoButton controlId={f.controlId} />
              </div>
            ))}
          </div>
        ) : null}
      </div>

      {error ? (
        <p className="form-error" data-testid="export-error" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}
