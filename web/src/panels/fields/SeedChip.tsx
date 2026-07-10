// SeedChip.tsx — the shared last-object-inheritance seed marker for the inspector field groups (WEB-04).
//
// # Module I/O
// - Input  a field name, the list of fields still inherited from the last object of `kind`, whether the
//   value equals the kind's first-of-kind default, and the `kind` label used in the inherited-chip copy.
// - Output a `.chip.info "inherited from last {kind}"` marker while the field is still seeded (until the
//   user edits it), a `.chip.off "default"` for a first-of-kind default field, or null once edited. Every
//   value reaches the DOM as a React text child (never innerHTML).
// - Valid input range: `kind` is a scene-kind label (e.g. "ground_zone", "forest").

import { type ReactElement } from "react";

export function SeedChip({
  field,
  inherited,
  isDefault,
  kind,
}: {
  readonly field: string;
  readonly inherited: readonly string[];
  readonly isDefault: boolean;
  readonly kind: string;
}): ReactElement | null {
  if (inherited.includes(field)) {
    return <span className="chip info">inherited from last {kind}</span>;
  }
  if (isDefault) {
    return <span className="chip off">default</span>;
  }
  return null;
}
