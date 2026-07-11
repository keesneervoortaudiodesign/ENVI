// App.tsx — the ENVI application shell: four persistent regions on the token grid (UI-SPEC).
//
// # Module I/O
// - Input  the canonical store (D-03) + the autosave store (D-04). `useAutosave()` wires committed edits
//   to the debounced whole-scene PUT + the flush-on-unload. The palette, inspector, validation panel,
//   project bar, and spectrum editor are their own components (panels/ + spectrum/).
// - Output the four-region shell JSX — project bar (<ProjectBar>), object palette (<Palette>), map-canvas
//   slot (<MapCanvas> + the transient <RejectBanner>), and the right rail (<Inspector> + <ValidationPanel>).
//   Control heights come from --row-h-lg / --row-h; 44px is retained ONLY on the primary Save and
//   destructive actions (D-12). Every colour/space/radius is an existing theme token — no new token.

import { useEffect, type ReactElement } from "react";

import { MapCanvas } from "./map/MapCanvas";
import { Palette } from "./panels/Palette";
import { Inspector } from "./panels/Inspector";
import { ProjectBar } from "./panels/ProjectBar";
import { RejectBanner } from "./panels/RejectBanner";
import { ValidationPanel } from "./panels/ValidationPanel";
import { ImportPanel } from "./panels/ImportPanel";
import { WeatherPanel } from "./panels/WeatherPanel";
import { SpectrumEditor } from "./spectrum/SpectrumEditor";
import { useSceneStore } from "./store/sceneStore";
import { useAutosave } from "./store/autosave";
import { reopenLast } from "./store/projectActions";
import { teardownImport } from "./import/importJob";

export function App(): ReactElement {
  const spectrumEditor = useSceneStore((s) => s.spectrumEditor);
  const closeSpectrumEditor = useSceneStore((s) => s.closeSpectrumEditor);
  // Wire committed-edit autosave + flush-on-unload (D-04). Mounted once at the app root.
  useAutosave();

  // Reopen-last on boot (D-06): restore the last-opened project + its scene. Best-effort — a missing
  // last-project resolves to the ordinary "No project" empty state, never an error (SC4 reopen path).
  useEffect(() => {
    void reopenLast();
    // Abort any in-flight imports + drop retained terrain on unmount (effect-cleanup teardown).
    return () => teardownImport();
  }, []);

  return (
    <div className="app-shell">
      {/* Region 1 — project bar (fixed, sticky top): identity, save indicator, Save, delete overflow. */}
      <ProjectBar />

      <div className="shell-body">
        {/* Region 2 — object palette rail. */}
        <Palette />

        {/* Region 3 — map canvas: dark-vector basemap + Terra Draw (07-06) + the transient ground-zone
            hard-reject banner (D-07, Surface B — map-anchored, never a validation-panel row). */}
        <main className="map-slot" data-testid="map-canvas" aria-label="Map canvas">
          <MapCanvas />
          <RejectBanner />
        </main>

        {/* Region 4 — right rail: property inspector + validation panel. */}
        <aside className="right-rail" data-testid="right-rail" aria-label="Inspector and validation">
          <Inspector />
          <ImportPanel />
          <WeatherPanel />
          <ValidationPanel />
        </aside>
      </div>

      {/* Isolation / sound-power spectrum editor overlay (WEB-10), opened from source / wall / façade. */}
      {spectrumEditor ? (
        <SpectrumEditor
          spectrumKey={spectrumEditor.key}
          title={spectrumEditor.title}
          onClose={closeSpectrumEditor}
        />
      ) : null}
    </div>
  );
}
