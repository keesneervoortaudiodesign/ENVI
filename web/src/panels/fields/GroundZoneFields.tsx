// GroundZoneFields.tsx — the ground_zone inspector fields (WEB-04): impedance class A–H + roughness
// class N/S/M/L as CLOSED-ENUM <select> controls (never free numeric entry), making an out-of-vocabulary
// value structurally impossible client-side (threat T-07-07-02).
//
// # Module I/O
// - Input  the selected feature id, its `properties`, the list of fields still inherited from the last
//   ground_zone (WEB-04), and an `update` callback into the canonical store.
// - Output two `<select>` rows: impedance (A–H, each option showing the class letter + its σ as a
//   display-only `.mono` suffix; class B = 31.5, corrected per CLAUDE.md) and roughness (N/S/M/L). Each
//   seeded field carries a `.chip.info "inherited from last ground_zone"` marker until edited; a
//   first-of-kind default field shows a `.chip.off "default"`. Every value reaches the DOM as text.
// - Valid input range: impedance ∈ A–H, roughness ∈ N/S/M/L — the only selectable options.

import { type ReactElement } from "react";

import { KIND_DEFAULTS } from "../../store/inheritance";
import { SeedChip } from "./SeedChip";
import { InfoButton } from "../../help/InfoButton";
import type { FieldsProps } from "./types";

// Nord2000 flow-resistivity σ per impedance class (kPa·s/m²). Mirrors
// `envi_engine::scene::impedance_class`; the σ is display-only (the class letter is the stored value).
const IMPEDANCE_SIGMA: Record<string, number> = {
  A: 12.5,
  B: 31.5,
  C: 80,
  D: 200,
  E: 500,
  F: 2000,
  G: 20000,
  H: 200000,
};
const IMPEDANCE_CLASSES = Object.keys(IMPEDANCE_SIGMA);
const ROUGHNESS_CLASSES = ["N", "S", "M", "L"] as const;

export function GroundZoneFields({ properties, inherited, update }: FieldsProps): ReactElement {
  const impedance = typeof properties["impedance_class"] === "string" ? (properties["impedance_class"] as string) : "D";
  const roughness = typeof properties["roughness_class"] === "string" ? (properties["roughness_class"] as string) : "N";
  const defaults = KIND_DEFAULTS.ground_zone;

  return (
    <div className="field-group">
      <label className="field-row">
        <span className="field-label">
          Impedance class
          <InfoButton controlId="ground_zone.impedance_class" />
        </span>
        <select
          className="input dense"
          data-testid="ground-impedance"
          value={impedance}
          onChange={(e) => update({ impedance_class: e.target.value })}
        >
          {IMPEDANCE_CLASSES.map((c) => (
            <option key={c} value={c}>
              {`${c} · σ ${IMPEDANCE_SIGMA[c]}`}
            </option>
          ))}
        </select>
        <SeedChip
          field="impedance_class"
          inherited={inherited}
          isDefault={impedance === defaults["impedance_class"]}
          kind="ground_zone"
        />
      </label>

      <label className="field-row">
        <span className="field-label">
          Roughness class
          <InfoButton controlId="ground_zone.roughness_class" />
        </span>
        <select
          className="input dense"
          data-testid="ground-roughness"
          value={roughness}
          onChange={(e) => update({ roughness_class: e.target.value })}
        >
          {ROUGHNESS_CLASSES.map((c) => (
            <option key={c} value={c}>
              {c}
            </option>
          ))}
        </select>
        <SeedChip
          field="roughness_class"
          inherited={inherited}
          isDefault={roughness === defaults["roughness_class"]}
          kind="ground_zone"
        />
      </label>
    </div>
  );
}
