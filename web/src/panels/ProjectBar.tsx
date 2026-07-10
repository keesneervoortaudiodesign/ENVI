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

  const [menuOpen, setMenuOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
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
        <button type="button" className="btn">
          Open
        </button>
        <button type="button" className="btn">
          New
        </button>
      </div>
      <div className="topbar-right">
        <span className={`save-conn ${indicator.cls}`} data-testid="save-indicator" data-status={status}>
          <span className="save-conn-label">{indicator.label}</span>
        </span>
        {/* 44px primary action (D-12). Explicit whole-scene PUT (debounced autosave also runs, D-04). */}
        <button
          type="button"
          className="btn primary"
          data-testid="save-scene"
          onClick={() => saveNow()}
        >
          Save
        </button>
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
    </header>
  );
}
