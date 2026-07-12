// ProjectBar.tsx — the fixed project bar (Region 1): project identity, Open/New, the dirty/saved autosave
// indicator (the `.conn` pattern), the explicit Save, and the `⋯` overflow that opens the typed-name
// Delete-project dialog.
//
// # Module I/O
// - Input  the canonical store (project identity) + the autosave store (`status`/`savedAt`) that drives
//   the indicator. The indicator is spatially separated from the validation panel and always carries a
//   TEXT label with no `.dot` pulse (the dirty/warn-overload mitigation, UI-SPEC State Matrix).
// - Output the `.topbar` JSX. The save indicator states: Dirty (`--color-warn` "Unsaved") · Saving
//   (`--color-primary`) · Saved (`--color-ok` "Saved · hh:mm" mono clock) · Save failed (`--color-crit`).
//   Save (44px `.btn.primary`, D-12) flushes immediately; `⋯` → "Delete project…" opens the confirmation
//   modal (44px destructive target).
// - Valid input range: `projectName` may be null (no project open) — the identity shows "No project" and
//   the delete dialog falls back to the project id / placeholder key.

import { useEffect, useRef, useState, type ReactElement } from "react";

import { useSceneStore } from "../store/sceneStore";
import { useAutosaveStore, saveNow, type SaveStatus } from "../store/autosave";
import { DeleteProjectDialog } from "./DeleteProjectDialog";
import { ProjectPicker } from "./ProjectPicker";
import { InfoButton } from "../help/InfoButton";
import type { OriginDto } from "../generated/wire";

// A new project's WGS84 origin (the map's initial centre); the server pins its UTM CRS from it (D-03).
const DEFAULT_ORIGIN: OriginDto = { lon_deg: 4.9041, lat_deg: 52.3676 };

// The indicator presentation for each save status: the severity class + its text label. No `.dot` pulse
// (dirty-overload mitigation) — meaning comes from location + label, not hue alone.
function indicatorFor(status: SaveStatus, savedAt: number | null): { cls: string; label: string } {
  switch (status) {
    case "saving":
      return { cls: "primary", label: "Saving…" };
    case "saved":
      return { cls: "ok", label: savedAt ? `Saved · ${hhmm(savedAt)}` : "Saved" };
    case "error":
      return { cls: "crit", label: "Save failed — retry" };
    case "dirty":
      return { cls: "warn", label: "Unsaved" };
    case "idle":
    default:
      return { cls: "off", label: "No changes" };
  }
}

// A zero-padded hh:mm clock (local time) for the "Saved · hh:mm" indicator.
function hhmm(epochMs: number): string {
  const d = new Date(epochMs);
  const h = String(d.getHours()).padStart(2, "0");
  const m = String(d.getMinutes()).padStart(2, "0");
  return `${h}:${m}`;
}

export function ProjectBar(): ReactElement {
  const projectName = useSceneStore((s) => s.projectName);
  const projectId = useSceneStore((s) => s.projectId);
  const status = useAutosaveStore((s) => s.status);
  const savedAt = useAutosaveStore((s) => s.savedAt);
  // Whether the scene carries any authored isolation/L_W spectra. These are NOT serialized by the
  // whole-scene PUT this phase (a documented Phase-9/10 deferral — see sceneStore.sceneFeatureCollection),
  // so when present the "Saved" indicator would over-claim. A distinct affordance keeps it honest (ME-04).
  const hasUnpersistedSpectra = useSceneStore((s) => Object.keys(s.spectra).length > 0);

  const [menuOpen, setMenuOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  // Close the overflow menu on an outside click / Esc (torn down on unmount).
  useEffect(() => {
    if (!menuOpen) {
      return;
    }
    const onDown = (e: MouseEvent): void => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(false);
      }
    };
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === "Escape") {
        setMenuOpen(false);
      }
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [menuOpen]);

  const indicator = indicatorFor(status, savedAt);
  const deleteName = projectName ?? projectId ?? "current";

  return (
    <header className="topbar" data-testid="project-bar">
      <div className="topbar-left">
        <span className="identity">{projectName ?? "No project"}</span>
        <button type="button" className="btn" data-testid="project-open" onClick={() => setPickerOpen(true)}>
          Open
        </button>
        <InfoButton controlId="project.open" />
        <button type="button" className="btn" data-testid="project-new" onClick={() => setPickerOpen(true)}>
          New
        </button>
        <InfoButton controlId="project.new" />
      </div>
      <div className="topbar-right">
        <span className={`save-conn ${indicator.cls}`} data-testid="save-indicator" data-status={status}>
          <span className="save-conn-label">{indicator.label}</span>
        </span>
        {/* Honesty affordance (ME-04): authored isolation/L_W spectra are session-only this phase (not
            serialized by PUT /scene — Phase-9/10 deferral). Surface it so "Saved" never implies the
            acoustic authoring is persisted. */}
        {hasUnpersistedSpectra ? (
          <span
            className="chip warn"
            data-testid="spectra-unpersisted"
            title="Authored isolation / sound-power spectra are session-only this phase — they are not yet written to the project and will be lost on reload (Phase 9/10)."
          >
            Spectra session-only
          </span>
        ) : null}
        {/* 44px primary action (D-12). Explicit whole-scene PUT (debounced autosave also runs, D-04). */}
        <button
          type="button"
          className="btn primary"
          data-testid="save-scene"
          onClick={() => saveNow()}
        >
          Save
        </button>
        <InfoButton controlId="project.save" />
        <div className="menu-wrap" ref={menuRef}>
          <button
            type="button"
            className="btn"
            data-testid="project-menu"
            aria-label="More project actions"
            aria-haspopup="menu"
            aria-expanded={menuOpen}
            onClick={() => setMenuOpen((o) => !o)}
          >
            &#x22EF;
          </button>
          <InfoButton controlId="project.menu" />
          {menuOpen ? (
            <div className="menu" role="menu" data-testid="project-menu-list">
              <button
                type="button"
                className="menu-item"
                role="menuitem"
                data-testid="menu-delete-project"
                onClick={() => {
                  setMenuOpen(false);
                  setDeleteOpen(true);
                }}
              >
                Delete project…
              </button>
            </div>
          ) : null}
        </div>
      </div>

      {deleteOpen ? (
        <DeleteProjectDialog projectName={deleteName} onClose={() => setDeleteOpen(false)} />
      ) : null}

      {pickerOpen ? (
        <ProjectPicker onClose={() => setPickerOpen(false)} defaultOrigin={DEFAULT_ORIGIN} />
      ) : null}
    </header>
  );
}
