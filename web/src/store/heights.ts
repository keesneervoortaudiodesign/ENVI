// heights.ts — the shared HEIGHT contract for the height-bearing scene kinds (DATA-03, WEB-04): parsing +
// validation of an inspector height entry, and the display mapping of the imported `height_provenance` tier.
//
// # Module I/O
// - Input  a feature's non-geometric `properties` bag and/or the raw string an `<input type="number">`
//   currently holds.
// - Output `heightOf(properties, key)` — the feature's OWN height in metres, or `null` when it carries no
//   usable one (NEVER a kind-default constant substituted behind the user's back: a substituted default is
//   exactly what makes every imported building look "the same height"); `parseHeightM(raw)` — the committed
//   value for a valid entry, or `null` for a non-finite / negative / absurd one (the inspector refuses to
//   commit those rather than silently writing garbage into the scene); and `heightProvenanceOf(properties)`
//   — the DATA-03 provenance tier of a height (measured / OSM height tag / OSM levels / import fallback /
//   user-edited / authored) with the chip tone the inspector paints it in.
// - Valid input range: heights are metres in [HEIGHT_MIN_M, HEIGHT_MAX_M]. `height_provenance` is the string
//   `envi-gis` stamps on import (`crates/envi-gis/src/provenance.rs`); any unknown/absent value reads as
//   "authored in ENVI" (drawn here, not imported).
//
// The property keys are the SERVER contract, not UI names: a building screens at `eaves_height_m` and a
// wall/barrier at `height_m` (`crates/envi-store/src/geojson.rs` requires both), so an object without its
// height is dropped from the screening set entirely — a wall with no height can never act as a noise screen.

// A height entry outside this range is refused (never committed). Zero is permitted (a degenerate,
// non-screening object is a legitimate authoring state); a negative height is nonsense, and no scene object
// in an environmental-noise model is 500 m tall — an entry above the cap is a typo, not a building.
export const HEIGHT_MIN_M = 0;
export const HEIGHT_MAX_M = 500;

// The scene property keys (mirroring the Rust GeoJSON contract) — one source of truth for the panels.
export const EAVES_HEIGHT_KEY = "eaves_height_m";
export const WALL_HEIGHT_KEY = "height_m";
export const HEIGHT_PROVENANCE_KEY = "height_provenance";

// The provenance value stamped when the USER types a height (the tier `envi-gis` never produces — an
// imported guess the operator has corrected by hand is no longer an OSM tag or a fallback default).
export const USER_HEIGHT_PROVENANCE = "user";

// The provenance tiers: the four `envi-gis` import tiers + the user-edited one.
export type HeightProvenance = "measured" | "height_tag" | "levels" | "default" | typeof USER_HEIGHT_PROVENANCE;

// How a provenance tier is presented: a stable machine key (asserted by the E2E), the English chip label,
// and its severity tone. `default` is a WARN — it is the tier that silently makes buildings look identical,
// so the user must be able to SEE that a height was guessed rather than known.
export interface ProvenanceDisplay {
  readonly key: string;
  readonly label: string;
  readonly tone: "info" | "warn" | "off";
}

const PROVENANCE_DISPLAY: Record<HeightProvenance, ProvenanceDisplay> = {
  measured: { key: "measured", label: "measured", tone: "info" },
  height_tag: { key: "height_tag", label: "OSM height tag", tone: "info" },
  levels: { key: "levels", label: "OSM building levels", tone: "info" },
  default: { key: "default", label: "import fallback default", tone: "warn" },
  [USER_HEIGHT_PROVENANCE]: { key: USER_HEIGHT_PROVENANCE, label: "user-edited", tone: "info" },
};

// A feature drawn in ENVI carries no import provenance at all — say so, rather than inventing a tier.
const AUTHORED: ProvenanceDisplay = { key: "authored", label: "authored in ENVI", tone: "off" };

// Is `value` a usable height in metres?
export function isValidHeightM(value: unknown): value is number {
  return (
    typeof value === "number" &&
    Number.isFinite(value) &&
    value >= HEIGHT_MIN_M &&
    value <= HEIGHT_MAX_M
  );
}

// The feature's OWN height under `key`, or null when it has none / an unusable one. Never falls back to a
// kind default — the inspector must show what the feature actually carries (the DATA-03 honesty rule).
export function heightOf(properties: Readonly<Record<string, unknown>>, key: string): number | null {
  const raw = properties[key];
  return isValidHeightM(raw) ? raw : null;
}

// Parse a raw numeric-input string into a committable height, or null when the entry must be REFUSED:
// empty, non-numeric, NaN/±Infinity, negative, or absurd (> HEIGHT_MAX_M).
export function parseHeightM(raw: string): number | null {
  const trimmed = raw.trim();
  if (trimmed === "") {
    return null;
  }
  const value = Number(trimmed);
  return isValidHeightM(value) ? value : null;
}

// The English reason a raw entry was refused (rendered next to the field, so a rejected value is visible).
export function heightRejectionReason(raw: string): string | null {
  const trimmed = raw.trim();
  if (trimmed === "") {
    return "Enter a height in metres.";
  }
  const value = Number(trimmed);
  if (!Number.isFinite(value)) {
    return "Height must be a finite number of metres.";
  }
  if (value < HEIGHT_MIN_M) {
    return "Height cannot be negative.";
  }
  if (value > HEIGHT_MAX_M) {
    return `Height above ${HEIGHT_MAX_M} m is implausible — check the value.`;
  }
  return null;
}

// The DATA-03 provenance of a feature's height: where the number CAME FROM.
export function heightProvenanceOf(properties: Readonly<Record<string, unknown>>): ProvenanceDisplay {
  const raw = properties[HEIGHT_PROVENANCE_KEY];
  if (typeof raw === "string" && Object.prototype.hasOwnProperty.call(PROVENANCE_DISPLAY, raw)) {
    return PROVENANCE_DISPLAY[raw as HeightProvenance];
  }
  return AUTHORED;
}
