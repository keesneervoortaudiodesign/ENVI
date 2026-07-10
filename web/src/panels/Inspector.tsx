// Inspector.tsx — the right-rail property inspector (WEB-04): dispatches on the selected feature's
// `kind` to a per-kind field component and edits the canonical store.
//
// # Module I/O
// - Input  the canonical store's `selection`, the selected feature, its `kind`, and the still-inherited
//   field list (WEB-04). Edits go back via `updateProperties` (which clears the inherited marker on the
//   edited field and keeps the kind's inheritance source current).
// - Output the property panel: an empty-state when nothing is selected; otherwise `.dense` rows for the
//   selected kind. Ground_zone / forest / source have dedicated field components (closed-enum selects,
//   numerics, position + spectrum slot); the remaining kinds render minimal inline rows. The `kind`
//   dispatch ends in `assertNeverKind`, so a dropped case fails `tsc` (D-09). Every value reaches the
//   DOM as a React text child — no raw-HTML injection.
// - Valid input range: `selection` is a feature id in the store or null.

import { type ReactElement } from "react";

import { assertNeverKind, type Kind } from "../draw/kinds";
import { useSceneStore } from "../store/sceneStore";
import { GroundZoneFields, type FieldsProps } from "./fields/GroundZoneFields";
import { ForestFields } from "./fields/ForestFields";
import { SourceFields } from "./fields/SourceFields";

// Render the field body for a kind. Exhaustive over the 9 kinds — `assertNeverKind` makes a missing
// case a compile error (D-09).
function KindFields({ kind, props }: { kind: Kind; props: FieldsProps }): ReactElement {
  switch (kind) {
    case "source":
      return <SourceFields {...props} />;
    case "forest":
      return <ForestFields {...props} />;
    case "ground_zone":
      return <GroundZoneFields {...props} />;
    case "elevation_point":
      return <ElevationPointFields {...props} />;
    case "receiver":
      return <BasicNote text="Receiver — an immission point. No non-geometric properties yet." />;
    case "wall":
      return <WallFields {...props} />;
    case "building":
      return <BasicNote text="Building — per-façade isolation is authored in the 07-08 editor." />;
    case "elevation_line":
      return <BasicNote text="Elevation line — a DGM breakline. Its Z comes from its endpoints." />;
    case "calc_area":
      return <BasicNote text="Calc area — the calculation domain boundary. No editable properties." />;
    default:
      return assertNeverKind(kind);
  }
}

function BasicNote({ text }: { text: string }): ReactElement {
  return <p className="field-note">{text}</p>;
}

function ElevationPointFields({ properties, update }: FieldsProps): ReactElement {
  const z = typeof properties["z_m"] === "number" ? (properties["z_m"] as number) : 0;
  return (
    <div className="field-group">
      <label className="field-row">
        <span className="field-label">Elevation Z</span>
        <span className="field-input">
          <input
            className="input dense mono"
            type="number"
            step="0.1"
            data-testid="elevation-z"
            value={z}
            onChange={(e) => update({ z_m: Number(e.target.value) })}
          />
          <span className="field-unit">m</span>
        </span>
      </label>
    </div>
  );
}

function WallFields({ properties, update }: FieldsProps): ReactElement {
  const semiTransparent = properties["semi_transparent"] === true;
  return (
    <div className="field-group">
      <label className="field-row">
        <span className="field-label">Semi-transparent</span>
        <input
          type="checkbox"
          data-testid="wall-semitransparent"
          checked={semiTransparent}
          onChange={(e) => update({ semi_transparent: e.target.checked })}
        />
      </label>
      <div className="field-row">
        <span className="field-label">Isolation spectrum</span>
        <button type="button" className="btn dense" data-testid="wall-edit-spectrum" disabled>
          Edit spectrum
        </button>
      </div>
    </div>
  );
}

export function Inspector(): ReactElement {
  const selection = useSceneStore((s) => s.selection);
  const feature = useSceneStore((s) => (s.selection ? s.features[s.selection] : undefined));
  const inheritedMap = useSceneStore((s) => s.inheritedFields);
  const kindOf = useSceneStore((s) => s.kindOf);
  const updateProperties = useSceneStore((s) => s.updateProperties);

  const kind = selection ? kindOf(selection) : null;

  return (
    <section className="panel" data-testid="inspector">
      <div className="panel-header">Properties</div>
      {!selection || !feature || !kind ? (
        <div className="empty-state">Select an object to edit its properties.</div>
      ) : (
        <div className="inspector-body" data-testid={`inspector-${kind}`}>
          <div className="field-row field-kind">
            <span className="field-label">Kind</span>
            <span className="chip off mono">{kind}</span>
          </div>
          <KindFields
            kind={kind}
            props={{
              id: selection,
              properties: (feature.properties ?? {}) as Record<string, unknown>,
              inherited: inheritedMap[selection] ?? [],
              update: (patch) => updateProperties(selection, patch),
            }}
          />
        </div>
      )}
    </section>
  );
}
