// icons.ts — inline SVG glyphs for the ENVI object palette (offline; no icon font / no CDN — D-11).
//
// # Module I/O
// - Input  a named glyph (`IconName`): the select/pan tool + the 9 locked scene kinds (UI-SPEC
//   Object Palette table). The glyph set is a fixed, hand-authored constant.
// - Output an `<svg>` SVGElement, 16px stroke, `currentColor` — built via DOM APIs (never
//   innerHTML, never dangerouslySetInnerHTML) so the markup is constructed, not parsed from a string.
//   Valid input range: any `IconName`; unknown names cannot be constructed (compile-time exhaustive).

// The palette tool identities: the pointer tool + the 9 frozen kinds (geojson.rs KINDS).
export type IconName =
  | "select"
  | "source"
  | "receiver"
  | "wall"
  | "building"
  | "forest"
  | "ground_zone"
  | "elevation_point"
  | "elevation_line"
  | "calc_area";

const PATHS: Record<IconName, string[]> = {
  // Simple stroke paths in a 0..16 viewBox (UI-SPEC glyph column).
  select: ["M3 2 L3 13 L6 10 L8 14 L10 13 L8 9 L12 9 Z"],
  source: ["M8 8 h0.01", "M4.5 8.5 A 8 8 0 0 1 11.5 8.5", "M2 6 A 12 12 0 0 1 14 6"],
  receiver: ["M8 3 v2", "M8 11 v2", "M3 8 h2", "M11 8 h2", "M8 5.5 a2.5 2.5 0 1 0 0.01 0"],
  wall: ["M4 12 L12 4"],
  building: ["M3 6 L8 3 L13 6", "M4 6 v7 h8 V6"],
  forest: ["M8 2 a3 3 0 0 0 -2 5 a3 3 0 0 0 -1 4 h6 a3 3 0 0 0 -1 -4 a3 3 0 0 0 -2 -5 Z", "M8 11 v3"],
  ground_zone: ["M2 3 h12 v10 h-12 Z", "M4 12 L12 4", "M8 12 L12 8", "M4 8 L8 4"],
  elevation_point: ["M8 3 L13 8 L8 13 L3 8 Z", "M7 8 h2"],
  elevation_line: ["M2 8 q2 -4 4 0 q2 4 4 0 q2 -4 4 0"],
  calc_area: ["M3 3 h3", "M10 3 h3", "M13 3 v3", "M13 10 v3", "M13 13 h-3", "M6 13 h-3", "M3 13 v-3", "M3 6 v-3"],
};

export function svgIcon(name: IconName): SVGElement {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 16 16");
  svg.setAttribute("width", "16");
  svg.setAttribute("height", "16");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "1.4");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  svg.setAttribute("aria-hidden", "true");
  for (const d of PATHS[name]) {
    const p = document.createElementNS("http://www.w3.org/2000/svg", "path");
    p.setAttribute("d", d);
    svg.appendChild(p);
  }
  return svg;
}
