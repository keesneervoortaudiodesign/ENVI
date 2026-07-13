// HeightField.tsx — the shared, VALIDATING height row used by every height-bearing kind (DATA-03, WEB-04).
//
// # Module I/O
// - Input  the property key the kind stores its height under (`eaves_height_m` for a building,
//   `height_m` for a wall/screen — the server contract, `crates/envi-store/src/geojson.rs`), the selected
//   feature's `properties`, its still-inherited field list, the kind's first-of-kind default (for the
//   `default` chip only), and an `update` callback into the canonical store.
// - Output one `.field-row` with a `.mono .dense` numeric input in metres, the WEB-04 seed chip, an
//   English rejection note when the current entry is not committable, and — for the kinds that track it —
//   the DATA-03 `height_provenance` chip saying WHERE the height came from (measured / OSM height tag /
//   OSM levels / import fallback / user-edited / authored). Every value reaches the DOM as a React text
//   child (no raw-HTML sink).
// - Valid input range: a finite height in [HEIGHT_MIN_M, HEIGHT_MAX_M] m. A non-finite, negative, or
//   absurd entry is REFUSED — it is never written to the store; the field shows why and the previously
//   committed value stands. A committed edit stamps `height_provenance: "user"` (for provenance-tracking
//   kinds), so an imported guess the operator corrected no longer claims to be an OSM tag or a default.
//
// The displayed value is ALWAYS the feature's OWN stored height — never the kind default substituted
// behind the user's back. That substitution is precisely what makes 4718 imported buildings with 244
// distinct heights all look identical.

import { useState, type ReactElement } from "react";

import {
  HEIGHT_MAX_M,
  HEIGHT_MIN_M,
  HEIGHT_PROVENANCE_KEY,
  USER_HEIGHT_PROVENANCE,
  heightOf,
  heightProvenanceOf,
  heightRejectionReason,
  parseHeightM,
} from "../../store/heights";
import { SeedChip } from "./SeedChip";
import { InfoButton } from "../../help/InfoButton";
import type { ControlId } from "../../help/controlIds";

export interface HeightFieldProps {
  // The scene property key this kind stores its height under.
  readonly field: string;
  readonly label: string;
  readonly controlId: ControlId;
  // `data-testid` of the input; the rejection note is `${testId}-error`, the provenance chip
  // `${testId}-provenance`.
  readonly testId: string;
  readonly kind: string;
  readonly properties: Readonly<Record<string, unknown>>;
  readonly inherited: readonly string[];
  // The kind's first-of-kind default — used ONLY to decide whether the `default` chip shows.
  readonly defaultValue: number;
  // Whether this kind carries the DATA-03 `height_provenance` (buildings do; it is the datum that makes
  // "all the same height" diagnosable by the user).
  readonly trackProvenance: boolean;
  readonly update: (patch: Record<string, unknown>) => void;
}

export function HeightField({
  field,
  label,
  controlId,
  testId,
  kind,
  properties,
  inherited,
  defaultValue,
  trackProvenance,
  update,
}: HeightFieldProps): ReactElement {
  // The feature's OWN committed height (null when it carries none/an unusable one — shown honestly as an
  // empty, flagged field rather than as a fabricated default).
  const stored = heightOf(properties, field);

  // The in-progress entry. Held locally so a half-typed / rejected value is visible while the STORE keeps
  // the last committed one. Re-synced from the store whenever the committed value changes underneath (a
  // selection switch remounts via the caller's `key`, so this only catches genuine external updates).
  const [draft, setDraft] = useState<string>(stored === null ? "" : String(stored));
  const [seen, setSeen] = useState<number | null>(stored);
  if (seen !== stored) {
    setSeen(stored);
    setDraft(stored === null ? "" : String(stored));
  }

  const rejection = heightRejectionReason(draft);

  const onChange = (raw: string): void => {
    setDraft(raw);
    const value = parseHeightM(raw);
    if (value === null) {
      return; // refuse: nothing reaches the store, the last committed height stands
    }
    setSeen(value);
    update(
      trackProvenance
        ? { [field]: value, [HEIGHT_PROVENANCE_KEY]: USER_HEIGHT_PROVENANCE }
        : { [field]: value },
    );
  };

  const provenance = heightProvenanceOf(properties);

  return (
    <>
      <label className="field-row">
        <span className="field-label">
          {label}
          <InfoButton controlId={controlId} />
        </span>
        <span className="field-input">
          <input
            className="input dense mono"
            type="number"
            step="0.1"
            min={HEIGHT_MIN_M}
            max={HEIGHT_MAX_M}
            data-testid={testId}
            value={draft}
            aria-invalid={rejection !== null || undefined}
            onChange={(e) => onChange(e.target.value)}
          />
          <span className="field-unit">m</span>
        </span>
        <SeedChip field={field} inherited={inherited} isDefault={stored === defaultValue} kind={kind} />
      </label>

      {rejection !== null ? (
        <div className="field-row">
          <span className="field-label">Not committed</span>
          <span className="chip crit" data-testid={`${testId}-error`}>
            {rejection}
          </span>
        </div>
      ) : null}

      {trackProvenance ? (
        <div className="field-row">
          <span className="field-label">Height source</span>
          <span
            className={`chip ${provenance.tone}`}
            data-testid={`${testId}-provenance`}
            data-provenance={provenance.key}
          >
            {provenance.label}
          </span>
        </div>
      ) : null}
    </>
  );
}
