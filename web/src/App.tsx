// App.tsx — the ENVI application shell: four persistent regions on the token grid (UI-SPEC).
//
// # Module I/O
// - Input  the canonical store (D-03): the dirty flag drives the save indicator; Save flushes the whole
//   scene (07-07). The palette + property inspector are their own components (panels/).
// - Output the four-region shell JSX — project bar (.topbar), object palette (<Palette>), map-canvas
//   slot (<MapCanvas>, 07-06), and the right rail (<Inspector> + validation empty-state). Control
//   heights come from --row-h-lg / --row-h; 44px is retained ONLY on the primary Save and destructive
//   actions (D-12). Every colour/space/radius is an existing theme token — no new token.

import { type ReactElement } from "react";

import { MapCanvas } from "./map/MapCanvas";
import { Palette } from "./panels/Palette";
import { useSceneStore } from "./store/sceneStore";

export function App(): ReactElement {
  const dirty = useSceneStore((s) => s.dirty);
  const saveScene = useSceneStore((s) => s.saveScene);

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
            {dirty ? "Unsaved" : "No changes"}
          </span>
          {/* 44px primary action (D-12). Explicit whole-scene PUT; debounced autosave lands in 07-09. */}
          <button
            type="button"
            className="btn primary"
            data-testid="save-scene"
            onClick={() => {
              void saveScene();
            }}
          >
            Save
          </button>
          <button type="button" className="btn" aria-label="More project actions">
            &#x22EF;
          </button>
        </div>
      </header>

      <div className="shell-body">
        {/* Region 2 — object palette rail. */}
        <Palette />

        {/* Region 3 — map canvas: dark-vector basemap + Terra Draw (07-06). */}
        <main className="map-slot" data-testid="map-canvas" aria-label="Map canvas">
          <MapCanvas />
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
