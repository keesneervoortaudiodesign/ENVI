// color.ts — the shared display-colour helper for the map overlays + the colour scale. ONE
// hex parser, so the isophone/legend palette (`store/colorScale`), the weather σ ramp
// (`weatherOverlay`), and the hatch/marker glyph generators (`hatchPatterns`) never keep
// parallel copies. Display-colour arithmetic only (no acoustics, D-01): every input is a
// program-controlled palette literal, never a user string (no XSS / injection surface).

// Parse a `#rrggbb` hex string (a leading `#` is optional) to `[r, g, b]`, each 0..255.
export function hexToRgb(hex: string): [number, number, number] {
  const h = hex.startsWith("#") ? hex.slice(1) : hex;
  return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
}
