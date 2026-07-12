// ResultsPanel.tsx — the right-rail RESULTS shell (WEB-11 + the wave-3 slot host).
// Mounts the five result surfaces in order; only `SpectrumPanel` is real in 11-05,
// the other four are render-nothing stubs OWNED by later plans (11-06 colour scale,
// 11-07 conditioning, 11-08 scenarios, 11-09 export). Each later plan fills its OWN
// file, so this shell is written ONCE and never re-edited — the isophone map,
// conditioning, scenarios, and export slot in without touching App.tsx (avoids a
// same-wave file conflict on the shell).
//
// # Module I/O
// - Input  the results store (via the child panels) + a mount-time effect that
//   attaches the real wasm readout client so a receiver selection can read the
//   OPFS tensor. No props.
// - Output the shell JSX: a titled results section hosting the five slots. The
//   shell holds NO acoustic logic — it is pure composition.

import { useEffect, type ReactElement } from "react";

import { useResultsStore, createWasmReadoutClient } from "../store/results";
import { SpectrumPanel } from "./SpectrumPanel";
import { ColorScaleEditor } from "./ColorScaleEditor";
import { ConditioningPanel } from "./ConditioningPanel";
import { ScenarioPanel } from "./ScenarioPanel";
import { ExportMenu } from "./ExportMenu";

export function ResultsPanel(): ReactElement {
  const attachReadoutClient = useResultsStore((s) => s.attachReadoutClient);
  const hasClient = useResultsStore((s) => s.client !== null);

  // Attach the real wasm readout client once (the store keeps it out of module
  // load so Node unit tests never pull the wasm graph). Guarded so an HMR remount
  // does not spin a second client.
  useEffect(() => {
    if (!hasClient) {
      attachReadoutClient(createWasmReadoutClient());
    }
  }, [hasClient, attachReadoutClient]);

  return (
    <section className="panel results-panel" data-testid="results-panel" aria-label="Results">
      <SpectrumPanel />
      <ColorScaleEditor />
      <ConditioningPanel />
      <ScenarioPanel />
      <ExportMenu />
    </section>
  );
}
