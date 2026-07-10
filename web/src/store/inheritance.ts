// inheritance.ts — per-kind, session-scoped last-object property inheritance (WEB-04).
//
// # Module I/O
// - Input  a `Kind` and (on commit/edit) the non-geometric property bag just committed for an object of
//   that kind. `recordLast(kind, props)` remembers it; `lastOf(kind)` reads it back.
// - Output `seedProps(kind)` — the property bag a NEW object of `kind` starts from, plus the list of
//   field names that were inherited from the previous object of that kind (so the inspector can show the
//   `.chip.info "inherited from last {kind}"` marker until the user edits that field). First-of-kind
//   objects fall back to `KIND_DEFAULTS` and report NO inherited fields (they show a `default` chip).
// - Valid input range: `Kind` ∈ the 9 frozen kinds. State is module-level and session-scoped — it lives
//   for the page's lifetime and resets on reload (the documented WEB-04 default); `resetInheritance()`
//   clears it for deterministic tests.

import type { Kind } from "../draw/kinds";

// A non-geometric property bag (the editable inspector fields for a kind). Geometry never lives here.
export type KindProps = Record<string, unknown>;

// First-of-kind documented defaults (UI-SPEC). Only kinds with editable non-geometric fields carry
// entries; the others start empty. Ground default class 'D' + roughness 'N'; forest gets sane positive
// means (zero density would itself be the `warn` state, so it is not a default).
export const KIND_DEFAULTS: Record<Kind, KindProps> = {
  source: {},
  receiver: {},
  wall: { semi_transparent: false },
  building: {},
  forest: { density_per_m2: 0.1, stem_radius_m: 0.15, height_m: 15 },
  ground_zone: { impedance_class: "D", roughness_class: "N" },
  elevation_point: { z_m: 0 },
  elevation_line: {},
  calc_area: {},
};

// The session-scoped last-committed non-geometric properties, per kind.
const lastByKind = new Map<Kind, KindProps>();

export function lastOf(kind: Kind): KindProps | undefined {
  const prev = lastByKind.get(kind);
  return prev ? { ...prev } : undefined;
}

// Remember the just-committed (or just-edited) properties as the inheritance source for the NEXT object
// of this kind. Stored as a shallow copy so later mutation of the caller's object cannot leak in.
export function recordLast(kind: Kind, props: KindProps): void {
  lastByKind.set(kind, { ...props });
}

// The seed for a new object of `kind`: the previous object's properties (with every field marked
// "inherited"), or the documented defaults (no inherited fields) for the first object of the kind.
export function seedProps(kind: Kind): { props: KindProps; inheritedFields: string[] } {
  const prev = lastByKind.get(kind);
  if (prev && Object.keys(prev).length > 0) {
    return { props: { ...prev }, inheritedFields: Object.keys(prev) };
  }
  return { props: { ...KIND_DEFAULTS[kind] }, inheritedFields: [] };
}

// Clear all session inheritance — test determinism only (each spec starts from first-of-kind defaults).
export function resetInheritance(): void {
  lastByKind.clear();
}
