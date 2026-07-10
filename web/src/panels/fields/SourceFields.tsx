// SourceFields.tsx — the source inspector fields (WEB-02): sub-source position [x, y, z], the sound-power
// L_W[105] "Edit spectrum" (reuses the curve+table editor), and the optional SPL-at-reference calibration
// whose L_W back-calculation is performed SERVER-side (SVC-07 — no client acoustic Hz/log math).
//
// # Module I/O
// - Input  the selected feature id + `properties` (the sub-source position, Z-up meters) and the canonical
//   store. The sound-power L_W lives in the store's `spectra` channel keyed by the source id (D-03); the
//   directivity-balloon phase seam is DEFERRED (Phase 10/11), not authored here.
// - Output three position rows, an "Edit spectrum" button that opens the L_W editor, and an SPL-at-reference
//   section (reference-distance input + a "Derive L_W" action). Derivation materializes the currently
//   authored spectrum via the server, then calls `POST /meta/spl-to-lw` to back-calculate L_W (free-field
//   spherical spreading, server-side) and stores the result as `authored@twelfth`. NO client Hz/log
//   arithmetic — every acoustic step is a server call. Every value reaches the DOM as text.
// - Valid input range: finite position components; a positive reference distance (the server rejects `<= 0`).

import { useState, type ReactElement } from "react";

import type { FieldsProps } from "./GroundZoneFields";
import { useSceneStore } from "../../store/sceneStore";
import { splToLw, ApiError } from "../../api/client";
import { materializeDense } from "../../spectrum/interpolateClient";

function positionOf(properties: Readonly<Record<string, unknown>>): [number, number, number] {
  const raw = properties["position"];
  if (Array.isArray(raw) && raw.length === 3 && raw.every((n) => typeof n === "number" && Number.isFinite(n))) {
    return [raw[0] as number, raw[1] as number, raw[2] as number];
  }
  return [0, 0, 0];
}

export function SourceFields({ id, properties }: FieldsProps): ReactElement {
  const position = positionOf(properties);
  const updateProperties = useSceneStore((s) => s.updateProperties);
  const authored = useSceneStore((s) => s.spectra[id] ?? null);
  const setSpectrum = useSceneStore((s) => s.setSpectrum);
  const openSpectrumEditor = useSceneStore((s) => s.openSpectrumEditor);

  const [refDistance, setRefDistance] = useState(1);
  const [deriving, setDeriving] = useState(false);
  const [deriveError, setDeriveError] = useState<string | null>(null);

  const axes: readonly ["x" | "y" | "z", number][] = [
    ["x", position[0]],
    ["y", position[1]],
    ["z", position[2]],
  ];

  const setAxis = (index: number, value: number): void => {
    const next: [number, number, number] = [...position];
    next[index] = value;
    updateProperties(id, { position: next });
  };

  // Back-calculate L_W from the currently-authored SPL spectrum + reference distance, entirely server-side
  // (materialize dense via the interpolation endpoint, then POST /meta/spl-to-lw). Stores the result as
  // authored@twelfth. Disabled until an SPL spectrum has been authored (via the editor).
  const deriveLw = async (): Promise<void> => {
    setDeriveError(null);
    if (!authored) {
      setDeriveError("Author an SPL spectrum first (Edit spectrum), then derive L_W from it.");
      return;
    }
    setDeriving(true);
    try {
      const splDense = await materializeDense(authored); // server interpolation, not client math
      const resp = await splToLw({ spl_db: splDense, reference_distance_m: refDistance }); // server back-calc
      setSpectrum(id, { resolution: "twelfth", values: resp.l_w_db });
    } catch (err) {
      setDeriveError(err instanceof ApiError ? err.detail : "The L_W derivation request failed.");
    } finally {
      setDeriving(false);
    }
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
        <button
          type="button"
          className="btn dense"
          data-testid="source-edit-spectrum"
          onClick={() => openSpectrumEditor(id, "Source sound power L_W")}
        >
          Edit spectrum
        </button>
      </div>

      {/* Optional SPL-at-reference calibration → server-side L_W back-calc (WEB-02, SVC-07). */}
      <div className="field-row">
        <span className="field-label">SPL @ reference</span>
        <span className="field-input">
          <input
            className="input dense mono"
            type="number"
            step="0.5"
            min="0.1"
            aria-label="Reference distance"
            data-testid="source-spl-distance"
            value={refDistance}
            onChange={(e) => setRefDistance(Number(e.target.value))}
          />
          <span className="field-unit">m</span>
        </span>
      </div>
      <div className="field-row">
        <span className="field-label" />
        <button
          type="button"
          className="btn dense"
          data-testid="source-derive-lw"
          disabled={deriving}
          onClick={() => void deriveLw()}
        >
          {deriving ? "Deriving…" : "Derive L_W from SPL"}
        </button>
      </div>
      {deriveError ? (
        <p className="form-error" data-testid="source-derive-error">
          {deriveError}
        </p>
      ) : null}
    </div>
  );
}
