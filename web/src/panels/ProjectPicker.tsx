// ProjectPicker.tsx — the Open / New project overlay (WEB-01, D-06): the REAL project-lifecycle surface
// that finalizes the 07-09 placeholder Open/New buttons. Lists the stored projects (open on click) and
// creates a new one (name + the current map origin), then hydrates the canonical store via projectActions.
//
// # Module I/O
// - Input  an `onClose` callback + a default WGS84 origin (the map centre) for a newly-created project.
//   The project list is fetched once on mount (`listProjectMetas`, metadata only). Opening/creating routes
//   through `projectActions` (open → getProject + getScene + loadScene; create → createProject then open).
// - Output a centered modal on a scrim: a fetched list of projects (name + last-modified, click to open)
//   with loading / empty / error states, and a "New project" name field + Create action. On a successful
//   open/create the store holds the project + scene and the overlay closes. A server `detail` renders as a
//   React TEXT child (never innerHTML). Every value reaches the DOM as text.
// - Valid input range: `defaultOrigin` is WGS84 degrees; a blank/whitespace name disables Create.

import { useEffect, useRef, useState, type ReactElement } from "react";

import { errorText } from "../api/client";
import { createAndOpen, listProjectMetas, openProjectById } from "../store/projectActions";
import type { OriginDto, ProjectMetaDto } from "../generated/wire";

type ListState =
  | { readonly kind: "loading" }
  | { readonly kind: "error"; readonly detail: string }
  | { readonly kind: "loaded"; readonly projects: readonly ProjectMetaDto[] };

// A short local date-time for the "last modified" column (unix epoch SECONDS on the wire → ms).
function modifiedLabel(meta: ProjectMetaDto): string {
  const seconds = Number(meta.modified_at_unix);
  if (!Number.isFinite(seconds) || seconds <= 0) {
    return "—";
  }
  return new Date(seconds * 1000).toLocaleString();
}

export function ProjectPicker({
  onClose,
  defaultOrigin,
}: {
  readonly onClose: () => void;
  readonly defaultOrigin: OriginDto;
}): ReactElement {
  const [list, setList] = useState<ListState>({ kind: "loading" });
  const [newName, setNewName] = useState("");
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const nameRef = useRef<HTMLInputElement>(null);

  // Fetch the project list once on mount; ignore a late resolve after unmount.
  useEffect(() => {
    let live = true;
    listProjectMetas()
      .then((projects) => {
        if (live) {
          setList({ kind: "loaded", projects });
        }
      })
      .catch((err: unknown) => {
        if (live) {
          setList({ kind: "error", detail: errorText(err) });
        }
      });
    return () => {
      live = false;
    };
  }, []);

  // Esc closes (torn down on unmount).
  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === "Escape" && !busy) {
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, busy]);

  async function open(id: string): Promise<void> {
    if (busy) {
      return;
    }
    setBusy(true);
    setActionError(null);
    try {
      await openProjectById(id);
      onClose();
    } catch (err) {
      setActionError(errorText(err));
      setBusy(false);
    }
  }

  async function create(): Promise<void> {
    const name = newName.trim();
    if (!name || busy) {
      return;
    }
    setBusy(true);
    setActionError(null);
    try {
      await createAndOpen(name, defaultOrigin);
      onClose();
    } catch (err) {
      setActionError(errorText(err));
      setBusy(false);
    }
  }

  return (
    <div
      className="overlay-scrim"
      data-testid="project-picker-scrim"
      role="presentation"
      onClick={() => {
        if (!busy) {
          onClose();
        }
      }}
    >
      <section
        className="panel project-picker"
        data-testid="project-picker"
        role="dialog"
        aria-modal="true"
        aria-label="Open or create a project"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="panel-header spectrum-header">
          <span className="section-title">Projects</span>
          <button type="button" className="btn dense" data-testid="picker-close" onClick={onClose} disabled={busy}>
            Close
          </button>
        </div>

        <div className="picker-body">
          {list.kind === "loading" ? (
            <div className="empty-state" data-testid="picker-loading">
              Loading projects…
            </div>
          ) : list.kind === "error" ? (
            <p className="form-error" data-testid="picker-error">
              {list.detail}
            </p>
          ) : list.projects.length === 0 ? (
            <div className="empty-state" data-testid="picker-empty">
              No projects yet. Create one below.
            </div>
          ) : (
            <ul className="picker-list" data-testid="picker-list">
              {list.projects.map((meta) => (
                <li key={meta.id}>
                  <button
                    type="button"
                    className="picker-row"
                    data-testid="picker-project"
                    data-project-id={meta.id}
                    disabled={busy}
                    onClick={() => void open(meta.id)}
                  >
                    <span className="picker-name">{meta.name}</span>
                    <span className="picker-modified mono">{modifiedLabel(meta)}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}

          {/* New project — name + the current map origin (the server pins its UTM CRS from it, D-03). */}
          <div className="picker-create">
            <span className="section-title">New project</span>
            <div className="btn-row">
              <input
                ref={nameRef}
                className="input"
                type="text"
                autoComplete="off"
                spellCheck={false}
                placeholder="Project name"
                aria-label="New project name"
                data-testid="picker-new-name"
                value={newName}
                disabled={busy}
                onChange={(e) => setNewName(e.target.value)}
              />
              <button
                type="button"
                className="btn primary"
                data-testid="picker-create"
                disabled={busy || newName.trim().length === 0}
                onClick={() => void create()}
              >
                Create
              </button>
            </div>
          </div>

          {actionError ? (
            <p className="form-error" data-testid="picker-action-error">
              {actionError}
            </p>
          ) : null}
        </div>
      </section>
    </div>
  );
}
