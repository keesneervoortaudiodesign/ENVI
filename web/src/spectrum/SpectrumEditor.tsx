// SpectrumEditor.tsx — the isolation / sound-power spectrum editor (WEB-10, SCN-03): a centered overlay
// with a curve view (dB vs band INDEX) + an authored anchor table, a 3-way resolution selector, a
// SERVER-owned interpolation preview (D-05), and explicit promote-to-authored@twelfth (D-06).
//
// # Module I/O
// - Input  a `spectrumKey` (a feature or edge UUID into the store's `spectra` channel) + a `title`. The
//   AUTHORED coarse spectrum is read from / written to the canonical store (`setSpectrum`) — only
//   `{ resolution, values }` is ever persisted (D-06); the dense `r_db[105]` preview is DERIVED by the
//   server (`POST /meta/interpolate-spectrum`), never stored. The freq axis (Hz labels, 1/3-oct tick
//   indices) comes from `GET /meta/freq-axis` (never hardcoded).
// - Output the overlay: a fixed-height inline-SVG curve (105 preview points as dB vs band index 0..104,
//   X ticks at the 1/3-oct centre indices showing nominal Hz display-only, preview line --color-primary,
//   authored anchors --color-text-strong) + a scrolling `.mono .dense` table (one row per authored anchor:
//   band index · nominal Hz · editable dB). Switching resolution re-projects non-destructively; switching
//   an octave/third spectrum to 1/12 PROMOTES with a visible `.chip.info` notice. Empty / loading / error
//   states per the UI-SPEC; every value reaches the DOM as a React text child (no innerHTML).
// - Valid input range: `spectrumKey` is any store key; edits keep `values.length` at the resolution's
//   anchor count. An interpolate `4xx` renders the server `detail` as text and keeps the last-good curve.

import { useState, type ReactElement } from "react";

import type { AuthoredSpectrumDto, Resolution } from "../generated/wire";
import { useSceneStore } from "../store/sceneStore";
import {
  anchorIndices,
  hzLabelForIndex,
  N_BANDS,
  reprojectAuthored,
  useFreqAxis,
  useSpectrumPreview,
} from "./interpolateClient";

const RESOLUTIONS: readonly { id: Resolution; label: string; count: number }[] = [
  { id: "octave", label: "1/1 (9)", count: 9 },
  { id: "third", label: "1/3 (27)", count: 27 },
  { id: "twelfth", label: "1/12 (105)", count: N_BANDS },
];

// Curve viewport (fixed height, UI-SPEC). Pure layout numbers (an SVG coordinate system), not tokens.
const CURVE_W = 460;
const CURVE_H = 180;
const PAD_L = 34;
const PAD_R = 10;
const PAD_T = 10;
const PAD_B = 22;

export interface SpectrumEditorProps {
  readonly spectrumKey: string;
  readonly title: string;
  readonly onClose: () => void;
}

export function SpectrumEditor({ spectrumKey, title, onClose }: SpectrumEditorProps): ReactElement {
  const authored = useSceneStore((s) => s.spectra[spectrumKey] ?? null);
  const setSpectrum = useSceneStore((s) => s.setSpectrum);

  const axis = useFreqAxis();
  const preview = useSpectrumPreview(authored);

  const [busy, setBusy] = useState(false);
  const [switchError, setSwitchError] = useState<string | null>(null);
  const [justPromoted, setJustPromoted] = useState(false);

  const resolution = authored?.resolution ?? null;

  // Switch the authoring resolution. Re-projects the authored values non-destructively via the server
  // (D-06); coarse → twelfth is an explicit PROMOTION (all 105 bands become individually editable).
  const switchResolution = async (next: Resolution): Promise<void> => {
    setSwitchError(null);
    if (!authored) {
      // Empty start: seed zeroed anchors at the chosen resolution (not a promotion — nothing to promote).
      setSpectrum(spectrumKey, { resolution: next, values: new Array(anchorIndices(next).length).fill(0) });
      setJustPromoted(false);
      return;
    }
    if (authored.resolution === next) {
      return;
    }
    const promoting = next === "twelfth" && authored.resolution !== "twelfth";
    setBusy(true);
    try {
      const reprojected = await reprojectAuthored(authored, next);
      setSpectrum(spectrumKey, reprojected);
      setJustPromoted(promoting);
    } catch {
      setSwitchError("Could not re-project the spectrum — the interpolation request failed.");
    } finally {
      setBusy(false);
    }
  };

  // Edit one authored anchor at the current resolution (a plain edit — NOT a promotion; promotion is the
  // explicit switch-to-1/12 above).
  const setAnchor = (anchorPos: number, value: number): void => {
    if (!authored) {
      return;
    }
    const values = authored.values.slice();
    values[anchorPos] = value;
    setSpectrum(spectrumKey, { resolution: authored.resolution, values });
  };

  return (
    <div className="overlay-scrim" data-testid="spectrum-editor-scrim" role="presentation" onClick={onClose}>
      <section
        className="panel spectrum-editor"
        data-testid="spectrum-editor"
        data-resolution={resolution ?? ""}
        role="dialog"
        aria-label={`Isolation spectrum editor — ${title}`}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="panel-header spectrum-header">
          <span className="section-title">{title}</span>
          <button type="button" className="btn dense" data-testid="spectrum-close" onClick={onClose}>
            Close
          </button>
        </div>

        <div className="spectrum-body">
          {/* Resolution selector (segmented). */}
          <div className="switch" role="group" aria-label="Authoring resolution">
            {RESOLUTIONS.map((r) => (
              <button
                key={r.id}
                type="button"
                className={`seg${resolution === r.id ? " active" : ""}`}
                data-testid={`spectrum-res-${r.id}`}
                aria-pressed={resolution === r.id}
                disabled={busy}
                onClick={() => void switchResolution(r.id)}
              >
                {r.label}
              </button>
            ))}
          </div>

          {justPromoted && resolution === "twelfth" ? (
            <span className="chip info" data-testid="spectrum-promote-notice">
              Switched to 1/12-octave — all 105 bands are now individually editable.
            </span>
          ) : null}

          {switchError ? (
            <p className="form-error" data-testid="spectrum-switch-error">
              {switchError}
            </p>
          ) : null}

          {!authored ? (
            <div className="empty-state" data-testid="spectrum-empty">
              No isolation spectrum. Pick a resolution to begin, then enter values below.
            </div>
          ) : (
            <div className="spectrum-views">
              <SpectrumCurve
                dense={preview.dense}
                loading={preview.loading}
                authored={authored}
                axis={axis}
              />
              <SpectrumTable authored={authored} axis={axis} onEdit={setAnchor} />
            </div>
          )}

          {preview.loading ? (
            <span className="chip off" data-testid="spectrum-loading">
              interpolating…
            </span>
          ) : null}
          {preview.error ? (
            <p className="form-error" data-testid="spectrum-error">
              {preview.error}
            </p>
          ) : null}
        </div>
      </section>
    </div>
  );
}

// The curve view: 105 preview points as dB vs band INDEX 0..104. Inline SVG built as React children (never
// innerHTML). X ticks at the 1/3-octave centre indices with nominal Hz display-only; authored anchors as
// markers on top of the preview line.
function SpectrumCurve({
  dense,
  loading,
  authored,
  axis,
}: {
  dense: number[] | null;
  loading: boolean;
  authored: AuthoredSpectrumDto;
  axis: ReturnType<typeof useFreqAxis>;
}): ReactElement {
  const anchors = anchorIndices(authored.resolution);
  const anchorVals = new Map(anchors.map((idx, k) => [idx, authored.values[k] ?? 0]));

  // Y domain: 0 .. a rounded-up dB ceiling covering the preview + authored values (autoscale, min 10 dB).
  const sample = [...(dense ?? []), ...authored.values];
  const dataMax = sample.length > 0 ? Math.max(...sample) : 10;
  const yTop = Math.max(10, Math.ceil(dataMax / 10) * 10);
  const yBottom = 0;

  const xOf = (index: number): number => PAD_L + (index / (N_BANDS - 1)) * (CURVE_W - PAD_L - PAD_R);
  const yOf = (db: number): number => {
    const t = (db - yBottom) / (yTop - yBottom || 1);
    return CURVE_H - PAD_B - t * (CURVE_H - PAD_T - PAD_B);
  };

  const linePoints =
    dense && dense.length === N_BANDS ? dense.map((db, i) => `${xOf(i).toFixed(1)},${yOf(db).toFixed(1)}`).join(" ") : "";

  const tickIndices = axis?.third_octave_indices ?? [];
  const yGrid = [0, 0.25, 0.5, 0.75, 1].map((f) => yBottom + f * (yTop - yBottom));

  return (
    <svg
      className={`spectrum-curve${loading ? " loading" : ""}`}
      data-testid="spectrum-curve"
      viewBox={`0 0 ${CURVE_W} ${CURVE_H}`}
      role="img"
      aria-label="Isolation spectrum, dB versus 1/12-octave band index"
    >
      {/* Horizontal gridlines + dB labels. */}
      {yGrid.map((db) => (
        <g key={`y${db}`}>
          <line
            x1={PAD_L}
            y1={yOf(db)}
            x2={CURVE_W - PAD_R}
            y2={yOf(db)}
            stroke="var(--color-border)"
            strokeWidth={1}
          />
          <text x={PAD_L - 4} y={yOf(db) + 3} textAnchor="end" className="curve-axis-label">
            {Math.round(db)}
          </text>
        </g>
      ))}
      {/* Vertical ticks at 1/3-octave centre indices, labelled with nominal Hz (display-only). */}
      {tickIndices.map((idx) => (
        <g key={`x${idx}`}>
          <line
            x1={xOf(idx)}
            y1={CURVE_H - PAD_B}
            x2={xOf(idx)}
            y2={CURVE_H - PAD_B + 3}
            stroke="var(--color-border)"
            strokeWidth={1}
          />
          {idx % 12 === 4 ? (
            <text x={xOf(idx)} y={CURVE_H - PAD_B + 13} textAnchor="middle" className="curve-axis-label">
              {hzLabelForIndex(axis, idx)}
            </text>
          ) : null}
        </g>
      ))}
      {/* Server-derived preview line (D-05). */}
      {linePoints ? (
        <polyline points={linePoints} fill="none" stroke="var(--color-primary)" strokeWidth={1.6} />
      ) : null}
      {/* Authored anchor markers on top. */}
      {anchors.map((idx) => (
        <circle
          key={`a${idx}`}
          data-testid={`spectrum-anchor-${idx}`}
          data-band-index={idx}
          cx={xOf(idx)}
          cy={yOf(anchorVals.get(idx) ?? 0)}
          r={2.4}
          fill="var(--color-text-strong)"
        />
      ))}
    </svg>
  );
}

// The authored-anchor table: one dense `.mono` row per anchor — band index · nominal Hz (display-only) ·
// editable dB. Editing a cell updates the store (and re-requests the debounced preview).
function SpectrumTable({
  authored,
  axis,
  onEdit,
}: {
  authored: AuthoredSpectrumDto;
  axis: ReturnType<typeof useFreqAxis>;
  onEdit: (anchorPos: number, value: number) => void;
}): ReactElement {
  const anchors = anchorIndices(authored.resolution);
  return (
    <div className="spectrum-table" data-testid="spectrum-table">
      <div className="spectrum-table-head mono">
        <span>Band</span>
        <span>Hz</span>
        <span>R dB</span>
      </div>
      <div className="spectrum-table-body">
        {anchors.map((bandIndex, pos) => (
          <div className="spectrum-table-row mono" key={bandIndex}>
            <span className="spectrum-band">{bandIndex}</span>
            <span className="spectrum-hz">{hzLabelForIndex(axis, bandIndex)}</span>
            <input
              className="input dense mono"
              type="number"
              step="0.5"
              aria-label={`R dB at band index ${bandIndex}`}
              data-testid={`spectrum-cell-${bandIndex}`}
              value={authored.values[pos] ?? 0}
              onChange={(e) => onEdit(pos, Number(e.target.value))}
            />
          </div>
        ))}
      </div>
    </div>
  );
}
