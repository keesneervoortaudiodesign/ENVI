// FacadePanel.tsx — the per-façade isolation assignment for a selected building (WEB-09, SCN-02): one row
// per footprint edge, keyed by its stable per-edge UUID (D-02), showing Inherits-default vs Override.
//
// # Module I/O
// - Input  the selected building feature (its `properties.edge_ids` — the D-02 per-edge UUIDs reconciled by
//   the store's ring-diff, never a vertex index) and the store's `spectra` channel (an edge UUID present ⇒
//   that façade OVERRIDES the building default; absent ⇒ it INHERITS). The "Building default" is stored
//   under the building's own feature id.
// - Output a "Building default" row + one row per edge (short UUID `.mono`, a `.chip.off "DEFAULT"` /
//   `.chip.info "OVERRIDE"`, an "Edit" button). Clicking a row's Edit opens the SpectrumEditor for that
//   edge UUID (or the building id for the default); editing the default applies live to every inheriting
//   edge (they read the default key on the map). Every value reaches the DOM as a React text child.
// - Valid input range: a building feature with an `edge_ids: string[]` property; an empty/absent list
//   renders the default row only (edges get UUIDs at draw time, so this is only the degenerate case).

import { type ReactElement } from "react";

import { useSceneStore } from "../store/sceneStore";
import type { FieldsProps } from "./fields/types";

// A short, legible form of an edge UUID for the row label (the full UUID stays the store key).
function shortId(id: string): string {
  return id.length > 8 ? `${id.slice(0, 8)}…` : id;
}

export function FacadePanel({ id, properties }: FieldsProps): ReactElement {
  const spectra = useSceneStore((s) => s.spectra);
  const openSpectrumEditor = useSceneStore((s) => s.openSpectrumEditor);

  const rawEdgeIds = properties["edge_ids"];
  const edgeIds = Array.isArray(rawEdgeIds) ? (rawEdgeIds.filter((x) => typeof x === "string") as string[]) : [];

  const defaultKey = id; // the building default is stored under the building's own feature id
  const hasDefault = Object.prototype.hasOwnProperty.call(spectra, defaultKey);

  return (
    <div className="field-group facade-panel" data-testid="facade-panel">
      {/* Building default — applied to every edge that has no override. */}
      <div className="facade-row facade-default" data-testid="facade-default-row">
        <span className="facade-edge mono">Building default</span>
        <span className={`chip ${hasDefault ? "info" : "off"}`}>{hasDefault ? "SET" : "NONE"}</span>
        <button
          type="button"
          className="btn dense"
          data-testid="facade-edit-default"
          onClick={() => openSpectrumEditor(defaultKey, "Building default isolation")}
        >
          Edit
        </button>
      </div>

      {edgeIds.length === 0 ? (
        <p className="field-note">No façade edges yet — draw the footprint to generate per-edge assignments.</p>
      ) : (
        edgeIds.map((edgeId) => {
          const override = Object.prototype.hasOwnProperty.call(spectra, edgeId);
          return (
            <div className="facade-row" data-testid={`facade-edge-${edgeId}`} key={edgeId}>
              <span className="facade-edge mono" data-edge-id={edgeId}>
                {shortId(edgeId)}
              </span>
              <span className={`chip ${override ? "info" : "off"}`} data-testid={`facade-chip-${edgeId}`}>
                {override ? "OVERRIDE" : "DEFAULT"}
              </span>
              <button
                type="button"
                className="btn dense"
                data-testid={`facade-edit-${edgeId}`}
                onClick={() => openSpectrumEditor(edgeId, `Façade ${shortId(edgeId)} isolation`)}
              >
                Edit
              </button>
            </div>
          );
        })
      )}
    </div>
  );
}
