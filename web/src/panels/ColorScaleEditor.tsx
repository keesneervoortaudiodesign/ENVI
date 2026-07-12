// ColorScaleEditor.tsx — the isophone COLOUR-SCALE editor (WEB-06 / D-04, NoizCalc
// TI 386 §4.6.5 controls). A preset picker (EU-END default + viridis/turbo) plus
// editable break rows and the §4.6.5 generator (smallest interval, interval
// magnitude, number of intervals, ascending, keep-color-sequence). Editing the
// scale RE-CONTOURS the cached level grid (SC3) with NO re-solve — every control
// mutates the single `breaks[]`/`colors[]` source of truth, and the isophone layer
// + legend re-render from it (legend ≡ contour ≡ class colour).
//
// # Module I/O
// - Input  the colour-scale store (the single source of truth + the cached grid).
// - Output the editor JSX in the ResultsPanel slot: preset segmented control, the
//   §4.6.5 generator, editable break rows with class swatches, and the inline V5
//   validation error (`.form-error`). NO acoustic math (D-01) — this panel only
//   manages break numbers + colours; the re-contour is a WASM tracer call.

import { useEffect, useState, type ReactElement } from "react";

import {
  useColorScaleStore,
  type Preset,
} from "../store/colorScale";
import { InfoButton } from "../help/InfoButton";

const PRESETS: { id: Preset; label: string }[] = [
  { id: "end", label: "EU-END" },
  { id: "viridis", label: "Viridis" },
  { id: "turbo", label: "Turbo" },
];

export function ColorScaleEditor(): ReactElement {
  const preset = useColorScaleStore((s) => s.preset);
  const breaks = useColorScaleStore((s) => s.breaks);
  const colors = useColorScaleStore((s) => s.colors);
  const ascending = useColorScaleStore((s) => s.ascending);
  const keepColorSequence = useColorScaleStore((s) => s.keepColorSequence);
  const error = useColorScaleStore((s) => s.error);
  const setPreset = useColorScaleStore((s) => s.setPreset);
  const setBreaks = useColorScaleStore((s) => s.setBreaks);
  const applyGenerator = useColorScaleStore((s) => s.applyGenerator);
  const setAscending = useColorScaleStore((s) => s.setAscending);
  const setKeepColorSequence = useColorScaleStore((s) => s.setKeepColorSequence);

  // Editable-break draft buffer: the inputs edit freely (intermediate invalid
  // states show the error without reverting); a valid edit commits to the store.
  const [drafts, setDrafts] = useState<string[]>(() => breaks.map(String));
  useEffect(() => {
    setDrafts(breaks.map(String));
  }, [breaks]);

  // The §4.6.5 generator inputs (uniform-scale shortcut), seeded from the scale.
  const [smallest, setSmallest] = useState<string>(() => String(breaks[0] ?? 55));
  const [magnitude, setMagnitude] = useState<string>(() =>
    String((breaks[1] ?? 60) - (breaks[0] ?? 55) || 5),
  );
  const [count, setCount] = useState<string>(() => String(breaks.length));

  const commitDraft = (index: number, text: string): void => {
    const next = [...drafts];
    next[index] = text;
    setDrafts(next);
    const parsed = next.map((t) => Number.parseFloat(t));
    setBreaks(parsed);
  };

  const applyUniform = (): void => {
    applyGenerator(
      Number.parseFloat(smallest),
      Number.parseFloat(magnitude),
      Number.parseInt(count, 10),
    );
  };

  return (
    <div className="panel-section colorscale-editor" data-testid="colorscale-editor">
      <div className="panel-header">
        <span className="section-title">Colour scale</span>
      </div>

      {/* Preset picker (EU-END default + perceptually-uniform viridis/turbo). */}
      <div className="switch" role="group" aria-label="Colour-scale preset">
        {PRESETS.map((p) => (
          <button
            key={p.id}
            type="button"
            className={`seg${preset === p.id ? " active" : ""}`}
            data-testid={`colorscale-preset-${p.id}`}
            aria-pressed={preset === p.id}
            onClick={() => setPreset(p.id)}
          >
            {p.label}
          </button>
        ))}
        <InfoButton controlId="colorscale.preset" />
      </div>

      {/* Editable break rows: value + class swatch (the single source of truth). */}
      <div className="field-row colorscale-breaks-caption">
        <span className="field-label mono">Isophone breaks</span>
        <InfoButton controlId="colorscale.breaks" />
      </div>
      <ul className="colorscale-breaks" data-testid="colorscale-breaks">
        {drafts.map((d, i) => (
          <li className="field-row colorscale-break-row" key={`break-${i}`}>
            <span
              className="colorscale-swatch"
              data-testid={`colorscale-swatch-${i}`}
              style={{ background: colors[i + 1] ?? colors[colors.length - 1] }}
              aria-hidden
            />
            <label className="field-label mono" htmlFor={`colorscale-break-${i}`}>
              Edge {i + 1}
            </label>
            <input
              id={`colorscale-break-${i}`}
              type="number"
              className="field-input mono"
              data-testid={`colorscale-break-${i}`}
              value={d}
              step={1}
              onChange={(e) => commitDraft(i, e.target.value)}
            />
            <span className="field-unit mono">dB</span>
          </li>
        ))}
      </ul>

      {/* NoizCalc §4.6.5 uniform generator. */}
      <div className="colorscale-generator" data-testid="colorscale-generator">
        <div className="field-row">
          <label className="field-label" htmlFor="colorscale-smallest">
            Smallest interval
          </label>
          <InfoButton controlId="colorscale.smallest" />
          <input
            id="colorscale-smallest"
            type="number"
            className="field-input mono"
            data-testid="colorscale-smallest"
            value={smallest}
            step={1}
            onChange={(e) => setSmallest(e.target.value)}
          />
          <span className="field-unit mono">dB</span>
        </div>
        <div className="field-row">
          <label className="field-label" htmlFor="colorscale-magnitude">
            Interval magnitude
          </label>
          <InfoButton controlId="colorscale.magnitude" />
          <input
            id="colorscale-magnitude"
            type="number"
            className="field-input mono"
            data-testid="colorscale-magnitude"
            value={magnitude}
            step={1}
            min={1}
            onChange={(e) => setMagnitude(e.target.value)}
          />
          <span className="field-unit mono">dB</span>
        </div>
        <div className="field-row">
          <label className="field-label" htmlFor="colorscale-count">
            Number of intervals
          </label>
          <InfoButton controlId="colorscale.count" />
          <input
            id="colorscale-count"
            type="number"
            className="field-input mono"
            data-testid="colorscale-count"
            value={count}
            step={1}
            min={2}
            onChange={(e) => setCount(e.target.value)}
          />
        </div>
        <button
          type="button"
          className="btn dense"
          data-testid="colorscale-apply"
          onClick={applyUniform}
        >
          Apply uniform scale
        </button>
        <InfoButton controlId="colorscale.apply" />
      </div>

      {/* Ascending + keep-color-sequence toggles (§4.6.5). */}
      <div className="colorscale-toggles">
        <label className="field-check">
          <input
            type="checkbox"
            data-testid="colorscale-ascending"
            checked={ascending}
            onChange={(e) => setAscending(e.target.checked)}
          />
          Ascending
        </label>
        <InfoButton controlId="colorscale.ascending" />
        <label className="field-check">
          <input
            type="checkbox"
            data-testid="colorscale-keep-sequence"
            checked={keepColorSequence}
            onChange={(e) => setKeepColorSequence(e.target.checked)}
          />
          Keep colour sequence
        </label>
        <InfoButton controlId="colorscale.keep_sequence" />
      </div>

      {error ? (
        <p className="form-error" data-testid="colorscale-error">
          {error}
        </p>
      ) : null}
    </div>
  );
}
