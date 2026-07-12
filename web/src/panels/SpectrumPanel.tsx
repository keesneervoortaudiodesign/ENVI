// SpectrumPanel.tsx — the receiver SPECTRUM panel (WEB-11 / SC1): a chart-primary
// + expandable exact-numbers table view of one receiver's per-band levels, with a
// 1/3-oct default ⇄ 1/12-oct expert toggle (D-06), an instant dB(A)⇄dB(C) toggle
// (D-09, no recompute), the coherent/incoherent split (D-08), and receiver
// selection via a mini-map click AND a synced list (D-07).
//
// # Module I/O
// - Input  the results store (selected receiver + its cached `ReceiverReadoutDto`,
//   the display/weighting/split toggles, the manifest's receiver list) and the
//   server freq axis (`useFreqAxis`, for the display-only Hz labels).
// - Output the panel JSX: a dual receiver picker (mini-map + list), an inline-SVG
//   bar chart with an optional coherent/incoherent overlay, the weighted totals +
//   the always-shown split totals, and an expandable band-index · Hz · dB table.
//
// # D-01 — renders WASM-produced values, does ZERO acoustic math
// EVERY dB here (band levels, the two weighted totals, the split) is read verbatim
// from the WASM readout. This file contains NO log/pow/exp dB arithmetic (the D-01
// grep gate — no such Math calls — asserts it). Autoscale uses only max/min/round
// (layout arithmetic, not acoustics); the display resolution AGGREGATES BY BAND
// INDEX (the 27 third-octave indices), never by nominal Hz, and Hz labels come from
// the server axis — never computed here.

import { useState, type ReactElement } from "react";

import type { ReceiverReadoutDto } from "../generated/wire";
import {
  useResultsStore,
  type DisplayMode,
  type ReceiverRef,
  type Weighting,
} from "../store/results";
import { anchorIndices, hzLabelForIndex, N_BANDS, useFreqAxis } from "../spectrum/interpolateClient";

// UI-SPEC §Data Visualization Palettes #4 — the three result series colours.
const COLOR_TOTAL = "#3987e5";
const COLOR_COHERENT = "#199e70";
const COLOR_INCOHERENT = "#d95926";

// Chart viewport (fixed height). Pure SVG-layout numbers, not tokens.
const CHART_W = 460;
const CHART_H = 190;
const PAD_L = 34;
const PAD_R = 10;
const PAD_T = 10;
const PAD_B = 22;

// The band indices shown for a display mode: 1/3-oct = the 27 third-octave centre
// indices (0, 4, …, 104); 1/12-oct = every band. Pure band-index selection.
function displayIndices(mode: DisplayMode, thirdIndices: number[]): number[] {
  if (mode === "twelfth") {
    return Array.from({ length: N_BANDS }, (_, i) => i);
  }
  return thirdIndices.length > 0 ? thirdIndices : anchorIndices("third");
}

export function SpectrumPanel(): ReactElement {
  const manifest = useResultsStore((s) => s.manifest);
  const selectedReceiverId = useResultsStore((s) => s.selectedReceiverId);
  const displayMode = useResultsStore((s) => s.displayMode);
  const weighting = useResultsStore((s) => s.weighting);
  const showSplit = useResultsStore((s) => s.showSplit);
  const readouts = useResultsStore((s) => s.readouts);
  const loadingReceiverId = useResultsStore((s) => s.loadingReceiverId);
  const readoutError = useResultsStore((s) => s.readoutError);
  const selectReceiver = useResultsStore((s) => s.selectReceiver);
  const setDisplayMode = useResultsStore((s) => s.setDisplayMode);
  const setWeighting = useResultsStore((s) => s.setWeighting);
  const toggleSplit = useResultsStore((s) => s.toggleSplit);

  const [tableOpen, setTableOpen] = useState(false);
  const axis = useFreqAxis();

  const receivers = manifest?.receivers ?? [];
  const readout = selectedReceiverId ? (readouts[selectedReceiverId] ?? null) : null;
  const loading = selectedReceiverId !== null && loadingReceiverId === selectedReceiverId;
  const indices = displayIndices(displayMode, axis?.third_octave_indices ?? []);

  return (
    <div className="panel-section spectrum-panel" data-testid="spectrum-panel">
      <div className="panel-header">
        <span className="section-title">Receiver spectrum</span>
      </div>

      {/* Dual receiver selection: a mini-map of receiver markers AND a synced list. */}
      <ReceiverPicker
        receivers={receivers}
        selectedId={selectedReceiverId}
        onSelect={selectReceiver}
      />

      {/* Display / weighting / split toggles (segmented). */}
      <div className="spectrum-controls">
        <div className="switch" role="group" aria-label="Display resolution">
          <ToggleButton
            testid="spectrum-display-third"
            active={displayMode === "third"}
            onClick={() => setDisplayMode("third")}
          >
            1/3-oct
          </ToggleButton>
          <ToggleButton
            testid="spectrum-display-twelfth"
            active={displayMode === "twelfth"}
            onClick={() => setDisplayMode("twelfth")}
          >
            1/12-oct
          </ToggleButton>
        </div>
        <div className="switch" role="group" aria-label="Frequency weighting">
          <ToggleButton
            testid="spectrum-weighting-A"
            active={weighting === "A"}
            onClick={() => setWeighting("A")}
          >
            dB(A)
          </ToggleButton>
          <ToggleButton
            testid="spectrum-weighting-C"
            active={weighting === "C"}
            onClick={() => setWeighting("C")}
          >
            dB(C)
          </ToggleButton>
        </div>
        <button
          type="button"
          className={`seg${showSplit ? " active" : ""}`}
          data-testid="spectrum-split-toggle"
          aria-pressed={showSplit}
          onClick={toggleSplit}
        >
          Split
        </button>
      </div>

      {selectedReceiverId === null ? (
        <div className="empty-state" data-testid="spectrum-empty">
          Select a receiver — click a marker on the map or a row in the list — to see its spectrum.
        </div>
      ) : loading ? (
        <div className="empty-state loading" data-testid="spectrum-loading">
          Reading the tensor…
        </div>
      ) : readoutError ? (
        <p className="form-error" data-testid="spectrum-error">
          {readoutError}
        </p>
      ) : readout ? (
        <SpectrumView
          readout={readout}
          indices={indices}
          displayMode={displayMode}
          weighting={weighting}
          showSplit={showSplit}
          axis={axis}
          tableOpen={tableOpen}
          onToggleTable={() => setTableOpen((o) => !o)}
        />
      ) : (
        <div className="empty-state" data-testid="spectrum-empty">
          No readout yet for this receiver.
        </div>
      )}
    </div>
  );
}

function ToggleButton({
  testid,
  active,
  onClick,
  children,
}: {
  testid: string;
  active: boolean;
  onClick: () => void;
  children: string;
}): ReactElement {
  return (
    <button
      type="button"
      className={`seg${active ? " active" : ""}`}
      data-testid={testid}
      aria-pressed={active}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

// The dual receiver picker: an inline-SVG mini-map (clickable markers = the "map
// click" path) AND a synced list. Both call the SAME `onSelect` (store
// `selectReceiver`), and the selected marker/row carries the accent ring (D-07).
function ReceiverPicker({
  receivers,
  selectedId,
  onSelect,
}: {
  receivers: readonly ReceiverRef[];
  selectedId: string | null;
  onSelect: (id: string) => void;
}): ReactElement {
  if (receivers.length === 0) {
    return (
      <div className="empty-state" data-testid="spectrum-no-receivers">
        No results yet — run a calculation to place receivers.
      </div>
    );
  }
  const xs = receivers.map((r) => r.position[0]);
  const ys = receivers.map((r) => r.position[1]);
  const minX = Math.min(...xs);
  const maxX = Math.max(...xs);
  const minY = Math.min(...ys);
  const maxY = Math.max(...ys);
  const spanX = maxX - minX || 1;
  const spanY = maxY - minY || 1;
  const MAP_W = 220;
  const MAP_H = 90;
  const PAD = 10;
  const px = (x: number): number => PAD + ((x - minX) / spanX) * (MAP_W - 2 * PAD);
  const py = (y: number): number => MAP_H - PAD - ((y - minY) / spanY) * (MAP_H - 2 * PAD);

  return (
    <div className="spectrum-picker">
      <svg
        className="spectrum-map"
        data-testid="spectrum-map"
        viewBox={`0 0 ${MAP_W} ${MAP_H}`}
        role="group"
        aria-label="Receiver map"
      >
        {receivers.map((r) => (
          <circle
            key={r.id}
            data-testid={`spectrum-marker-${r.id}`}
            data-selected={selectedId === r.id ? "true" : "false"}
            cx={px(r.position[0])}
            cy={py(r.position[1])}
            r={selectedId === r.id ? 5 : 3.2}
            fill={selectedId === r.id ? "var(--color-primary)" : "var(--color-text-muted)"}
            stroke={selectedId === r.id ? "var(--color-primary)" : "none"}
            strokeWidth={selectedId === r.id ? 3 : 0}
            strokeOpacity={0.35}
            style={{ cursor: "pointer" }}
            onClick={() => onSelect(r.id)}
          />
        ))}
      </svg>
      <ul className="spectrum-receiver-list" data-testid="spectrum-receiver-list">
        {receivers.map((r) => (
          <li key={r.id}>
            <button
              type="button"
              className={`spectrum-receiver-row${selectedId === r.id ? " selected" : ""}`}
              data-testid={`spectrum-receiver-${r.id}`}
              data-selected={selectedId === r.id ? "true" : "false"}
              aria-pressed={selectedId === r.id}
              onClick={() => onSelect(r.id)}
            >
              <span className="mono">R{r.globalIndex}</span>
              <span className="mono spectrum-receiver-pos">
                {r.position[0].toFixed(0)}, {r.position[1].toFixed(0)}
              </span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

// The chart + totals + table for one receiver's readout.
function SpectrumView({
  readout,
  indices,
  displayMode,
  weighting,
  showSplit,
  axis,
  tableOpen,
  onToggleTable,
}: {
  readout: ReceiverReadoutDto;
  indices: number[];
  displayMode: DisplayMode;
  weighting: Weighting;
  showSplit: boolean;
  axis: ReturnType<typeof useFreqAxis>;
  tableOpen: boolean;
  onToggleTable: () => void;
}): ReactElement {
  const total = weighting === "A" ? readout.total_dba : readout.total_dbc;

  // Autoscale over the displayed values (+ the split channels when shown). These
  // are layout bounds via Math.max/min — NOT acoustic math.
  const shown = indices.map((i) => readout.band_levels_db[i] ?? 0);
  const splitVals = showSplit
    ? indices.flatMap((i) => [readout.coherent_db[i] ?? 0, readout.incoherent_db[i] ?? 0])
    : [];
  const allVals = [...shown, ...splitVals];
  const dataMax = allVals.length > 0 ? Math.max(...allVals) : 10;
  const dataMin = allVals.length > 0 ? Math.min(...allVals) : 0;
  const yTop = Math.ceil(dataMax / 10) * 10;
  const yBottom = Math.min(0, Math.floor(dataMin / 10) * 10);

  const n = indices.length;
  const plotW = CHART_W - PAD_L - PAD_R;
  const xOf = (k: number): number => PAD_L + (n <= 1 ? plotW / 2 : (k / (n - 1)) * plotW);
  const yOf = (db: number): number => {
    const t = (db - yBottom) / (yTop - yBottom || 1);
    return CHART_H - PAD_B - t * (CHART_H - PAD_T - PAD_B);
  };
  const barW = Math.max(1, (plotW / Math.max(1, n)) * 0.7);
  const yGrid = [0, 0.25, 0.5, 0.75, 1].map((f) => yBottom + f * (yTop - yBottom));

  const linePoints = (pick: (i: number) => number): string =>
    indices.map((i, k) => `${xOf(k).toFixed(1)},${yOf(pick(i)).toFixed(1)}`).join(" ");

  return (
    <div className="spectrum-view">
      {/* Totals: the selected weighted total + the ALWAYS-shown coherent/incoherent split. */}
      <div className="spectrum-totals">
        <span
          className="spectrum-total mono"
          data-testid="spectrum-total"
          data-weighting={weighting}
        >
          {total.toFixed(1)} dB({weighting})
        </span>
        <span className="spectrum-split-totals mono" data-testid="spectrum-split-totals">
          <span data-testid="spectrum-total-coherent" style={{ color: COLOR_COHERENT }}>
            coh {readout.total_coherent_db.toFixed(1)}
          </span>
          <span data-testid="spectrum-total-incoherent" style={{ color: COLOR_INCOHERENT }}>
            incoh {readout.total_incoherent_db.toFixed(1)}
          </span>
        </span>
      </div>

      <svg
        className="spectrum-chart"
        data-testid="spectrum-chart"
        data-band-count={n}
        data-display={displayMode}
        viewBox={`0 0 ${CHART_W} ${CHART_H}`}
        role="img"
        aria-label={`Receiver spectrum, dB versus band index (${displayMode})`}
      >
        {yGrid.map((db) => (
          <g key={`y${db}`}>
            <line
              x1={PAD_L}
              y1={yOf(db)}
              x2={CHART_W - PAD_R}
              y2={yOf(db)}
              stroke="var(--color-border)"
              strokeWidth={1}
            />
            <text x={PAD_L - 4} y={yOf(db) + 3} textAnchor="end" className="curve-axis-label">
              {Math.round(db)}
            </text>
          </g>
        ))}
        {/* Total per-band bars. */}
        {indices.map((i, k) => {
          const v = readout.band_levels_db[i] ?? 0;
          const yb = yOf(v);
          const y0 = yOf(yBottom);
          return (
            <rect
              key={`bar${i}`}
              data-testid="spectrum-band-bar"
              data-band-index={i}
              x={xOf(k) - barW / 2}
              y={Math.min(yb, y0)}
              width={barW}
              height={Math.abs(y0 - yb)}
              fill={COLOR_TOTAL}
              fillOpacity={showSplit ? 0.35 : 0.8}
            />
          );
        })}
        {/* Coherent / incoherent per-band overlay (on demand, D-08). */}
        {showSplit ? (
          <g data-testid="spectrum-split-overlay">
            <polyline
              points={linePoints((i) => readout.coherent_db[i] ?? 0)}
              fill="none"
              stroke={COLOR_COHERENT}
              strokeWidth={1.6}
            />
            <polyline
              points={linePoints((i) => readout.incoherent_db[i] ?? 0)}
              fill="none"
              stroke={COLOR_INCOHERENT}
              strokeWidth={1.6}
              strokeDasharray="3 2"
            />
          </g>
        ) : null}
      </svg>

      {/* Expandable exact-numbers table (band index · Hz · dB). */}
      <button
        type="button"
        className="btn dense spectrum-table-toggle"
        data-testid="spectrum-table-toggle"
        aria-expanded={tableOpen}
        onClick={onToggleTable}
      >
        {tableOpen ? "Hide exact numbers" : "Show exact numbers"}
      </button>
      {tableOpen ? (
        <div className="spectrum-table" data-testid="spectrum-results-table">
          <div className="spectrum-table-head mono">
            <span>Band</span>
            <span>Hz</span>
            <span>dB</span>
          </div>
          <div className="spectrum-table-body">
            {indices.map((i) => (
              <div className="spectrum-table-row mono" key={`row${i}`} data-band-index={i}>
                <span className="spectrum-band">{i}</span>
                <span className="spectrum-hz">{hzLabelForIndex(axis, i)}</span>
                <span className="spectrum-db">{(readout.band_levels_db[i] ?? 0).toFixed(1)}</span>
              </div>
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}
