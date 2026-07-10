// App.tsx — the ENVI application shell: four persistent regions on the token grid (UI-SPEC).
//
// # Module I/O
// - Input  none yet (no store/props). The palette tool list and the region scaffolding are static;
//   the scene store, MapLibre canvas, inspector fields, and live validation land in later 07-plans.
// - Output the four-region shell JSX — project bar (.topbar), object palette (.panel), map-canvas
//   slot (empty until 07-06), and the right rail (.panel stack: inspector + validation empty-states).
//   Control heights come from --row-h-lg / --row-h; 44px is retained ONLY on the primary Save and
//   destructive actions (D-12). Every colour/space/radius is an existing theme token — no new token.

import { useEffect, useRef, useState, type ReactElement } from "react";

import { svgIcon, type IconName } from "./icons";

// One palette entry: the Terra Draw tool identity, its label, and the kind-hue token used for the
// leading `.dot` (UI-SPEC Object Palette — hue is an EXISTING token, never invented).
interface Tool {
  readonly name: IconName;
  readonly label: string;
  readonly dotToken: string;
}

// The pointer tool + the 9 frozen scene kinds (geojson.rs KINDS), in palette order.
const TOOLS: readonly Tool[] = [
  { name: "select", label: "Select / pan", dotToken: "--color-text-muted" },
  { name: "source", label: "Source", dotToken: "--color-primary" },
  { name: "receiver", label: "Receiver", dotToken: "--color-ok" },
  { name: "wall", label: "Wall / screen", dotToken: "--color-text" },
  { name: "building", label: "Building", dotToken: "--color-text-muted" },
  { name: "forest", label: "Forest", dotToken: "--color-ok" },
  { name: "ground_zone", label: "Ground zone", dotToken: "--color-off" },
  { name: "elevation_point", label: "Elevation point", dotToken: "--color-info" },
  { name: "elevation_line", label: "Elevation line", dotToken: "--color-info" },
  { name: "calc_area", label: "Calc area", dotToken: "--color-primary" },
];

// Render a named glyph by appending the DOM-constructed <svg> from icons.ts (never innerHTML /
// dangerouslySetInnerHTML). The useEffect cleanup removes the node so no listener/element leaks
// across re-renders (07-PATTERNS lifecycle discipline).
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

export function App(): ReactElement {
  const [activeTool, setActiveTool] = useState<IconName>("select");

  return (
    <div className="app-shell">
      {/* Region 1 — project bar (fixed, sticky top). */}
      <header className="topbar" data-testid="project-bar">
        <div className="topbar-left">
          <span className="identity">No project</span>
          <button type="button" className="btn">
            Open
          </button>
          <button type="button" className="btn">
            New
          </button>
        </div>
        <div className="topbar-right">
          <span className="save-indicator" data-testid="save-indicator">
            Not saved
          </span>
          {/* 44px primary action (D-12). */}
          <button type="button" className="btn primary">
            Save
          </button>
          <button type="button" className="btn" aria-label="More project actions">
            &#x22EF;
          </button>
        </div>
      </header>

      <div className="shell-body">
        {/* Region 2 — object palette rail. */}
        <nav className="panel palette" data-testid="object-palette" aria-label="Object palette">
          <div className="panel-header">Objects</div>
          <ul className="tool-list">
            {TOOLS.map((tool) => (
              <li key={tool.name}>
                <button
                  type="button"
                  className={`tool-row${activeTool === tool.name ? " active" : ""}`}
                  aria-pressed={activeTool === tool.name}
                  onClick={() => setActiveTool(tool.name)}
                >
                  <Icon name={tool.name} />
                  <span className="tool-label">{tool.label}</span>
                  <span className="dot" style={{ background: `var(${tool.dotToken})` }} />
                </button>
              </li>
            ))}
          </ul>
        </nav>

        {/* Region 3 — map canvas slot (MapLibre lands in 07-06). */}
        <main className="map-slot" data-testid="map-canvas" aria-label="Map canvas">
          <p className="map-placeholder">Map canvas — basemap and drawing arrive in 07-06.</p>
        </main>

        {/* Region 4 — right rail: property inspector + validation panel. */}
        <aside className="right-rail" data-testid="right-rail" aria-label="Inspector and validation">
          <section className="panel" data-testid="inspector">
            <div className="panel-header">Properties</div>
            <div className="empty-state">Select an object to edit its properties.</div>
          </section>
          <section className="panel" data-testid="validation">
            <div className="panel-header">Validation</div>
            <div className="empty-state">No issues — the scene is valid.</div>
          </section>
        </aside>
      </div>
    </div>
  );
}
