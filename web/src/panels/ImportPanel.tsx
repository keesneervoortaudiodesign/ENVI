// ImportPanel.tsx — the GIS import panel (SC1/SC3/SC5, Surface A). An "Import (viewport)" action, three
// independently-toggleable layers (terrain / land cover / buildings) each with a live status + progress +
// retry-on-error, the max-area guardrail warning, the GLO-30 "surface model" badge (D-05), the impedance
// debug-overlay toggle (SC3), and the source attribution strings (SC5). Mirrors ValidationPanel's structure.
//
// # Module I/O
// - Input  the import store (per-layer status/toggle/progress/error, guardrail, viewport, debug toggle,
//   attributed sources) + the scene store's open project id (the import target). No props.
// - Output the panel JSX: every actionable control carries a `data-testid` (the 08-08 E2E drives these),
//   and every string — including imported error details + attribution — reaches the DOM as a React text
//   child (NEVER innerHTML; threat T-08-07-05). Clicking "Import" runs the enabled layers for the current
//   viewport; a failed layer shows a Retry that re-runs only that layer (D-07).
// - Valid input range: derives entirely from store state; import is disabled without an open project or a
//   known viewport, or when the guardrail blocks the current viewport.

import { useMemo, type ReactElement } from "react";

import { useImportStore, LAYER_KEYS, type LayerKey, type LayerState } from "../store/import";
import { useSceneStore } from "../store/sceneStore";
import { runImport, retryLayer } from "../import/importJob";
import { attributionStrings } from "../import/attribution";
import { InfoButton } from "../help/InfoButton";
import type { ControlId } from "../help/controlIds";

const LAYER_LABELS: Record<LayerKey, string> = {
  terrain: "Terrain",
  landcover: "Land cover",
  buildings: "Buildings",
};

// The chip severity for a layer status (reusing the shared `.chip.warn` / `.chip.crit` styles).
function statusSeverity(status: LayerState["status"]): "" | "warn" | "crit" {
  if (status === "error") {
    return "crit";
  }
  if (status === "running") {
    return "warn";
  }
  return "";
}

function LayerRow({ layer }: { readonly layer: LayerKey }): ReactElement {
  const state = useImportStore((s) => s.layers[layer]);
  const setEnabled = useImportStore((s) => s.setLayerEnabled);
  const severity = statusSeverity(state.status);
  const pct = Math.round(state.progress * 100);

  return (
    <li className="issue-row" data-testid={`import-layer-${layer}`}>
      <label className="btn-row">
        <input
          type="checkbox"
          checked={state.enabled}
          data-testid={`import-toggle-${layer}`}
          onChange={(e) => setEnabled(layer, e.target.checked)}
        />
        <span className="issue-text">{LAYER_LABELS[layer]}</span>
      </label>
      <InfoButton controlId={`import.layer_${layer}` as ControlId} />

      <span className={`chip ${severity}`} data-testid={`import-status-${layer}`}>
        {state.status === "error" ? <span className="dot crit" aria-hidden="true" /> : null}
        {state.status}
      </span>

      {state.status === "running" ? (
        <span className="mono" data-testid={`import-progress-${layer}`}>
          {pct}% {state.message}
        </span>
      ) : null}

      {state.status === "done" ? (
        <span className="mono" data-testid={`import-count-${layer}`}>
          {state.featureCount} features
        </span>
      ) : null}

      {layer === "terrain" && state.surfaceModel ? (
        <span className="chip warn" data-testid="import-glo30-badge">
          surface model — may include buildings/vegetation
        </span>
      ) : null}

      {state.status === "error" && state.error ? (
        <>
          <span className="issue-text" data-testid={`import-error-${layer}`}>
            {state.error.detail}
          </span>
          <button
            type="button"
            className="btn"
            data-testid={`import-retry-${layer}`}
            onClick={() => retryLayer(layer)}
          >
            Retry
          </button>
        </>
      ) : null}
    </li>
  );
}

export function ImportPanel(): ReactElement {
  const viewport = useImportStore((s) => s.viewport);
  const guardrail = useImportStore((s) => s.guardrail);
  const debugOverlay = useImportStore((s) => s.debugOverlay);
  const toggleDebugOverlay = useImportStore((s) => s.toggleDebugOverlay);
  const attributedSources = useImportStore((s) => s.attributedSources);
  const projectId = useSceneStore((s) => s.projectId);

  const attributions = useMemo(() => attributionStrings(attributedSources), [attributedSources]);
  const canImport = !!projectId && !!viewport && !guardrail?.blocked;

  return (
    <section className="panel" data-testid="import-panel">
      <div className="panel-header">Import</div>

      {projectId ? null : (
        <div className="empty-state" data-testid="import-no-project">
          Open a project to import GIS data.
        </div>
      )}

      <div className="btn-row">
        <button
          type="button"
          className="btn"
          data-testid="import-run"
          disabled={!canImport}
          onClick={() => {
            if (projectId && viewport) {
              runImport(projectId, viewport);
            }
          }}
        >
          Import (viewport)
        </button>
        <InfoButton controlId="import.run" />
        <label className="btn-row">
          <input
            type="checkbox"
            checked={debugOverlay}
            data-testid="import-debug-toggle"
            onChange={() => toggleDebugOverlay()}
          />
          <span className="issue-text">Impedance overlay</span>
        </label>
        <InfoButton controlId="import.debug_overlay" />
      </div>

      {guardrail ? (
        <div
          className={`chip ${guardrail.blocked ? "crit" : "warn"}`}
          data-testid="import-guardrail"
        >
          {guardrail.detail}
        </div>
      ) : null}

      <ul className="issue-list" data-testid="import-layers">
        {LAYER_KEYS.map((layer) => (
          <LayerRow key={layer} layer={layer} />
        ))}
      </ul>

      {attributions.length > 0 ? (
        <ul className="issue-list" data-testid="import-attribution">
          {attributions.map((text) => (
            <li key={text} className="issue-text">
              {text}
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  );
}
