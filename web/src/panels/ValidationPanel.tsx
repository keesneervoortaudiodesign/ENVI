// ValidationPanel.tsx — the persistent validation panel (WEB-04, SC2, Surface A). Lists NON-geometric
// issues on objects that EXIST; each row is click-to-select + zoom-to-fit + open-inspector. It is
// deliberately distinct from the transient ground-zone reject banner (Surface B), which is NEVER a row
// here (D-07 resolves the SC2 ambiguity).
//
// # Module I/O
// - Input  the canonical scene store (features + spectra + selectAndZoom) and the `dgm` slice's
//   `rejectReason` (the crit source produced by 07-07's `dgmTrigger` when `POST /dgm/triangulate` returns
//   a 4xx for interior-crossing breaklines — this panel is only the SURFACE, not the producer).
// - Output a live-recomputed issue list (no spinner): (a) a semi-transparent wall WITHOUT an isolation
//   spectrum → `.chip.warn`; (b) a forest with mean density 0 → `.chip.warn`; (c) an interior-crossing
//   elevation breakline → `.chip.crit` (with the `.dot.crit` pulse) whose text is the stored reject
//   `detail`. Clicking a row selects + zoom-to-fits the offending object and opens its inspector. An empty
//   scene shows the valid empty-state. Every description reaches the DOM as a React text child (no innerHTML).
// - Valid input range: derives entirely from current store state; no props.

import { useMemo, type ReactElement } from "react";

import { useSceneStore } from "../store/sceneStore";
import { useDgmStore } from "../store/dgm";

type Severity = "warn" | "crit";

interface IssueRow {
  readonly key: string;
  readonly severity: Severity;
  readonly text: string;
  // The object the row selects + zooms to (a real feature id), or null when there is nothing to target.
  readonly targetId: string | null;
}

// A short, stable id label for the row (first 6 chars of the UUID), or a dash when there is no target.
function shortId(id: string | null): string {
  return id ? id.slice(0, 6) : "—";
}

export function ValidationPanel(): ReactElement {
  const features = useSceneStore((s) => s.features);
  const spectra = useSceneStore((s) => s.spectra);
  const selectAndZoom = useSceneStore((s) => s.selectAndZoom);
  const dgmReject = useDgmStore((s) => s.rejectReason);

  // Derive the issue list from the current store slices only (features + spectra + dgmReject). Memoized so a
  // render that changes none of them (e.g. an unrelated store notify) reuses the last list rather than
  // re-scanning every feature twice on each render.
  const rows = useMemo<IssueRow[]>(() => {
    const out: IssueRow[] = [];

    for (const [id, feature] of Object.entries(features)) {
      const props = (feature.properties ?? {}) as Record<string, unknown>;
      const kind = props["kind"];
      // (a) A semi-transparent wall/screen with no isolation spectrum is acoustically incomplete (WEB-04/08).
      if (kind === "wall" && props["semi_transparent"] === true && !Object.prototype.hasOwnProperty.call(spectra, id)) {
        out.push({
          key: `wall-nospectrum-${id}`,
          severity: "warn",
          text: "Semi-transparent wall has no isolation spectrum.",
          targetId: id,
        });
      }
      // (b) A forest with zero mean density contributes nothing at solve time (WEB-04/SCN-04).
      if (kind === "forest" && props["density_per_m2"] === 0) {
        out.push({
          key: `forest-zero-${id}`,
          severity: "warn",
          text: "Forest has zero mean tree density.",
          targetId: id,
        });
      }
    }

    // (c) The elevation-breakline interior-cross crit row, sourced from the dgm slice's `rejectReason`
    // (07-07's dgmTrigger producer). The "offending object" is the crossing breaklines — target the first
    // elevation_line so the row still selects + zooms.
    if (dgmReject) {
      const firstBreakline = Object.entries(features).find(
        ([, f]) => (f.properties ?? {})["kind"] === "elevation_line",
      );
      out.push({
        key: "dgm-reject",
        severity: "crit",
        text: `Elevation breaklines interior-cross — ${dgmReject.detail}`,
        targetId: firstBreakline ? firstBreakline[0] : null,
      });
    }

    return out;
  }, [features, spectra, dgmReject]);

  return (
    <section className="panel" data-testid="validation">
      <div className="panel-header">Validation</div>
      {rows.length === 0 ? (
        <div className="empty-state">No issues — the scene is valid.</div>
      ) : (
        <ul className="issue-list" data-testid="issue-list">
          {rows.map((row) => (
            <li key={row.key}>
              <button
                type="button"
                className="issue-row"
                data-testid={`issue-${row.severity}`}
                data-target={row.targetId ?? ""}
                onClick={() => {
                  if (row.targetId) {
                    selectAndZoom(row.targetId);
                  }
                }}
              >
                <span className={`chip ${row.severity}`}>
                  {row.severity === "crit" ? <span className="dot crit" aria-hidden="true" /> : null}
                  {row.severity}
                </span>
                <span className="issue-text">{row.text}</span>
                <span className="issue-id mono">{shortId(row.targetId)}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
