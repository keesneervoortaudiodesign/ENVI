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
import { EAVES_HEIGHT_KEY, WALL_HEIGHT_KEY } from "./heights";

// A non-geometric property bag (the editable inspector fields for a kind). Geometry never lives here.
export type KindProps = Record<string, unknown>;

// First-of-kind documented defaults (UI-SPEC). Only kinds with editable non-geometric fields carry
// entries; the others start empty. Ground default class 'D' + roughness 'N'; forest gets sane positive
// means (zero density would itself be the `warn` state, so it is not a default).
//
// The two HEIGHTS are load-bearing, not cosmetic: the server scene contract REQUIRES a wall's `height_m`
// and a building's `eaves_height_m` (`crates/envi-store/src/geojson.rs`), and the screening marshaller drops
// any height-less object from the screen set — so a drawn wall with no height could never screen anything.
// A first-of-kind object therefore starts from a plausible, clearly-visible height (3 m garden-wall screen,
// 10 m eaves ≈ a 3-storey building) which the user edits in the inspector; the value then INHERITS to the
// next object of that kind like every other property.
export const KIND_DEFAULTS: Record<Kind, KindProps> = {
  source: {},
  receiver: {},
  wall: { semi_transparent: false, [WALL_HEIGHT_KEY]: 3 },
  building: { [EAVES_HEIGHT_KEY]: 10 },
  forest: { density_per_m2: 0.1, stem_radius_m: 0.15, height_m: 15 },
  ground_zone: { impedance_class: "D", roughness_class: "N" },
  elevation_point: { z_m: 0 },
  elevation_line: {},
  calc_area: {},
};

// The session-scoped last-committed non-geometric properties, per kind.
const lastByKind = new Map<Kind, KindProps>();

// Internal / geometry-structural keys that are NOT user-editable inspector fields and must never become
// "inherited" chips (LO-02): the semantic tag (`kind`), the feature `id`, and the building geometry
// metadata (`edge_ids`, the Terra Draw `mode`). Seeding these onto the next object of a kind would show
// spurious "inherited" chips and briefly seed a stale `edge_ids` array (harmless, since `tagFeature`
// overwrites it, but it leaks non-editable metadata into the WEB-04 UI). Excluded at the inheritance
// boundary so every caller of `recordLast` is covered.
//
// The D-11 PROVENANCE keys (stamped by `envi-gis` on import — see `crates/envi-gis/src/provenance.rs`) are
// excluded for a stronger reason: provenance describes where THIS feature's data came from, so it is never
// inheritable. Editing an imported building (which flows through `recordLast`) must not seed the NEXT,
// hand-drawn building with `imported: true`, an OSM `source_ref`, or a `height_provenance` of "height_tag" —
// that would be a fabricated pedigree, and it would make the DATA-03 provenance chip lie.
const NON_INHERITABLE_KEYS = new Set([
  "kind",
  "id",
  "edge_ids",
  "mode",
  // D-11 provenance (PROVENANCE_KEYS in envi-gis) — per-feature pedigree, never inherited.
  "source",
  "source_ref",
  "license",
  "retrieved_at",
  "imported",
  "user_modified",
  "height_provenance",
  "vertical_datum",
]);

export function lastOf(kind: Kind): KindProps | undefined {
  const prev = lastByKind.get(kind);
  return prev ? { ...prev } : undefined;
}

// Remember the just-committed (or just-edited) properties as the inheritance source for the NEXT object
// of this kind. Stored as a shallow copy (so later mutation of the caller's object cannot leak in) with the
// non-user-facing internal/geometry keys stripped (LO-02).
export function recordLast(kind: Kind, props: KindProps): void {
  const clean: KindProps = {};
  for (const [key, value] of Object.entries(props)) {
    if (!NON_INHERITABLE_KEYS.has(key)) {
      clean[key] = value;
    }
  }
  lastByKind.set(kind, clean);
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
