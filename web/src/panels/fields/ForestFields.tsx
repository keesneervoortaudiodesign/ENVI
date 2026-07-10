// ForestFields.tsx — the forest inspector fields (WEB-04, SCN-04): mean tree density, mean stem radius,
// and mean height as continuous `.mono` `.dense` NUMERIC inputs (not enumerations).
//
// # Module I/O
// - Input  the selected feature id, its `properties`, the fields still inherited from the last forest,
//   and an `update` callback into the canonical store. Values are stored RAW (no client acoustic math —
//   the server `TryFrom` converts + range-checks; D-01).
// - Output three numeric rows with unit suffixes (trees/m², m, m). A seeded field shows a
//   `.chip.info "inherited from last forest"` until edited; a first-of-kind default shows `.chip.off`.
//   Zero density is a `warn` state surfaced in the 07-09 validation panel — here it is a valid entry.
// - Valid input range: finite numbers; the store persists whatever is typed and the server validates.

import { type ReactElement } from "react";

import { KIND_DEFAULTS } from "../../store/inheritance";
import { SeedChip } from "./SeedChip";
import type { FieldsProps } from "./types";

function numeric(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

export function ForestFields({ properties, inherited, update }: FieldsProps): ReactElement {
  const defaults = KIND_DEFAULTS.forest;
  const density = numeric(properties["density_per_m2"], defaults["density_per_m2"] as number);
  const stemRadius = numeric(properties["stem_radius_m"], defaults["stem_radius_m"] as number);
  const height = numeric(properties["height_m"], defaults["height_m"] as number);

  return (
    <div className="field-group">
      <label className="field-row">
        <span className="field-label">Mean tree density</span>
        <span className="field-input">
          <input
            className="input dense mono"
            type="number"
            step="0.01"
            data-testid="forest-density"
            value={density}
            onChange={(e) => update({ density_per_m2: Number(e.target.value) })}
          />
          <span className="field-unit">trees/m²</span>
        </span>
        <SeedChip
          field="density_per_m2"
          inherited={inherited}
          isDefault={density === defaults["density_per_m2"]}
          kind="forest"
        />
      </label>

      <label className="field-row">
        <span className="field-label">Mean stem radius</span>
        <span className="field-input">
          <input
            className="input dense mono"
            type="number"
            step="0.01"
            data-testid="forest-stem-radius"
            value={stemRadius}
            onChange={(e) => update({ stem_radius_m: Number(e.target.value) })}
          />
          <span className="field-unit">m</span>
        </span>
        <SeedChip
          field="stem_radius_m"
          inherited={inherited}
          isDefault={stemRadius === defaults["stem_radius_m"]}
          kind="forest"
        />
      </label>

      <label className="field-row">
        <span className="field-label">Mean height</span>
        <span className="field-input">
          <input
            className="input dense mono"
            type="number"
            step="0.1"
            data-testid="forest-height"
            value={height}
            onChange={(e) => update({ height_m: Number(e.target.value) })}
          />
          <span className="field-unit">m</span>
        </span>
        <SeedChip field="height_m" inherited={inherited} isDefault={height === defaults["height_m"]} kind="forest" />
      </label>
    </div>
  );
}
