// BuildingFields.tsx — the building inspector (WEB-04, DATA-03, WEB-09): the eaves HEIGHT (with its
// provenance) followed by the per-façade isolation panel.
//
// # Module I/O
// - Input  the selected building feature's id + `properties` (its own `eaves_height_m` and, when the
//   building was imported, the `height_provenance` tier `envi-gis` stamped on it), its still-inherited
//   field list, and an `update` callback into the canonical store.
// - Output the editable eaves-height row + the DATA-03 provenance chip (HeightField), then the existing
//   per-edge FacadePanel. The height is the feature's OWN value — importing 4718 buildings with 244
//   distinct heights previously showed NOTHING here, which is why they all appeared identical.
// - Valid input range: a finite eaves height in [0, 500] m; anything else is refused (never committed).

import { type ReactElement } from "react";

import { KIND_DEFAULTS } from "../../store/inheritance";
import { EAVES_HEIGHT_KEY } from "../../store/heights";
import { FacadePanel } from "../FacadePanel";
import { HeightField } from "./HeightField";
import type { FieldsProps } from "./types";

const DEFAULT_EAVES_M = KIND_DEFAULTS.building[EAVES_HEIGHT_KEY] as number;

export function BuildingFields(props: FieldsProps): ReactElement {
  const { id, properties, inherited, update } = props;
  return (
    <div className="field-group">
      <HeightField
        key={id}
        field={EAVES_HEIGHT_KEY}
        label="Eaves height"
        controlId="building.eaves_height"
        testId="building-eaves-height"
        kind="building"
        properties={properties}
        inherited={inherited}
        defaultValue={DEFAULT_EAVES_M}
        trackProvenance
        update={update}
      />
      <FacadePanel {...props} />
    </div>
  );
}
