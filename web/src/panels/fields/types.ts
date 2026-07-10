// types.ts — the shared prop contract for the per-kind inspector field groups (WEB-04).
//
// # Module I/O
// - Input  none (type-only module).
// - Output `FieldsProps` — the selected feature's id + `properties`, the list of field names still
//   inherited from the last object of the kind (WEB-04), and an `update` callback into the canonical store.
//   Consumed by every `fields/*` group (GroundZone/Forest/Source) and by Inspector/FacadePanel; kept in a
//   NEUTRAL module so no field group has to import a sibling group just for its prop type.
export interface FieldsProps {
  readonly id: string;
  readonly properties: Readonly<Record<string, unknown>>;
  readonly inherited: readonly string[];
  readonly update: (patch: Record<string, unknown>) => void;
}
