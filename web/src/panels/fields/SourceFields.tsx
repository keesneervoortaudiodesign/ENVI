// SourceFields.tsx — the source inspector fields (WEB-02): sub-source position [x, y, z] as three
// `.mono` inputs + an "Edit spectrum" trigger slot (the curve+table L_W editor itself lands in 07-08).
//
// # Module I/O
// - Input  the selected feature id, its `properties`, and an `update` callback into the canonical store.
//   The sub-source position is Z-up meters (`SubSourceDto.position`); the L_W[105] spectrum is scene
//   data in the store's `spectra` channel (D-03), authored by the 07-08 editor.
// - Output three numeric position rows + a disabled "Edit spectrum" button (07-08 opens the editor).
//   Directivity-balloon import is deferred (Phase 10/11). Every value reaches the DOM as text.
// - Valid input range: finite position components; the server validates on conversion.

import { type ReactElement } from "react";

import type { FieldsProps } from "./GroundZoneFields";

function positionOf(properties: Readonly<Record<string, unknown>>): [number, number, number] {
  const raw = properties["position"];
  if (Array.isArray(raw) && raw.length === 3 && raw.every((n) => typeof n === "number" && Number.isFinite(n))) {
    return [raw[0] as number, raw[1] as number, raw[2] as number];
  }
  return [0, 0, 0];
}

export function SourceFields({ properties, update }: FieldsProps): ReactElement {
  const position = positionOf(properties);
  const axes: readonly ["x" | "y" | "z", number][] = [
    ["x", position[0]],
    ["y", position[1]],
    ["z", position[2]],
  ];

  const setAxis = (index: number, value: number): void => {
    const next: [number, number, number] = [...position];
    next[index] = value;
    update({ position: next });
  };

  return (
    <div className="field-group">
      <div className="field-row">
        <span className="field-label">Sub-source position</span>
        <span className="field-input field-triple">
          {axes.map(([axis, value], i) => (
            <input
              key={axis}
              className="input dense mono"
              type="number"
              step="0.1"
              aria-label={`Position ${axis}`}
              data-testid={`source-pos-${axis}`}
              value={value}
              onChange={(e) => setAxis(i, Number(e.target.value))}
            />
          ))}
          <span className="field-unit">m</span>
        </span>
      </div>

      <div className="field-row">
        <span className="field-label">Sound-power spectrum</span>
        <button type="button" className="btn dense" data-testid="source-edit-spectrum" disabled>
          Edit spectrum
        </button>
      </div>
    </div>
  );
}
