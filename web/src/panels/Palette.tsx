// Palette.tsx — the object palette rail: the pointer/select tool + all 9 locked scene kinds (WEB-01).
//
// # Module I/O
// - Input  the canonical store's `activeTool` / `setActiveTool` (D-03) and the per-kind metadata from
//   `draw/kinds.ts` (label, icon, kind-hue token). Selecting a row sets the active tool; Terra Draw's
//   mode tracks it in `useTerraDraw` (07-06/07-07), and a finished shape is tagged with that kind.
// - Output the vertical tool list — each row `--row-h-lg`, a DOM-constructed SVG glyph (icons.ts, never
//   innerHTML), the label, and a `.dot` in the kind hue (an EXISTING theme token, D-11). The active tool
//   gets the primary left-border + tint (app.css `.tool-row.active`).
// - Valid input range: the 9 KINDS + `"select"`. Every value reaches the DOM as a React text child.

import { useEffect, useRef, type ReactElement } from "react";

import { svgIcon, type IconName } from "../icons";
import { KIND_META, KINDS, type DrawTool } from "../draw/kinds";
import { useSceneStore } from "../store/sceneStore";

// One palette entry: its tool identity, label, icon, and kind-hue token (select has no hue).
interface ToolRow {
  readonly tool: DrawTool;
  readonly label: string;
  readonly icon: IconName;
  readonly hueToken: string | null;
}

const ROWS: readonly ToolRow[] = [
  { tool: "select", label: "Select / pan", icon: "select", hueToken: null },
  ...KINDS.map((kind): ToolRow => {
    const meta = KIND_META[kind];
    return { tool: kind, label: meta.label, icon: meta.icon, hueToken: meta.hueToken };
  }),
];

// Render a named glyph by appending the DOM-constructed <svg> from icons.ts (built via DOM APIs, never
// parsed from an HTML string). The cleanup removes the node so nothing leaks across re-renders.
function Icon({ name }: { name: IconName }): ReactElement {
  const hostRef = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    const host = hostRef.current;
    if (!host) {
      return;
    }
    const svg = svgIcon(name);
    host.appendChild(svg);
    return () => {
      host.removeChild(svg);
    };
  }, [name]);
  return <span className="tool-icon" ref={hostRef} aria-hidden="true" />;
}

export function Palette(): ReactElement {
  const activeTool = useSceneStore((s) => s.activeTool);
  const setActiveTool = useSceneStore((s) => s.setActiveTool);

  return (
    <nav className="panel palette" data-testid="object-palette" aria-label="Object palette">
      <div className="panel-header">Objects</div>
      <ul className="tool-list">
        {ROWS.map((row) => (
          <li key={row.tool}>
            <button
              type="button"
              className={`tool-row${activeTool === row.tool ? " active" : ""}`}
              data-testid={`tool-${row.tool}`}
              aria-pressed={activeTool === row.tool}
              onClick={() => setActiveTool(row.tool)}
            >
              <Icon name={row.icon} />
              <span className="tool-label">{row.label}</span>
              {row.hueToken ? (
                <span className="dot" style={{ background: `var(${row.hueToken})` }} />
              ) : null}
            </button>
          </li>
        ))}
      </ul>
    </nav>
  );
}
