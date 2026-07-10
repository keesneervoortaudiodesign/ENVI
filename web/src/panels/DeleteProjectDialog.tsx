// DeleteProjectDialog.tsx — the typed-name confirmation modal for the irreversible "Delete project"
// action (UI-SPEC "Destructive Action"). Deleting removes the project folder (scene + settings + calc
// records) from disk with no undo, so the danger button stays disabled until the user types the exact
// project name.
//
// # Module I/O
// - Input  the current project name (compared client-side against the typed text — no server call, no
//   acoustic math) and an `onClose` callback. The project id comes from the store (the delete target).
// - Output a centered modal on a scrim: a `.section-title` + `.chip.crit "IRREVERSIBLE"`, body copy naming
//   the on-disk deletion (project name via a React text child — never innerHTML), a name-match `.input`
//   gating the `.btn.danger` (44px, disabled until exact match), and Cancel (44px, default focus, Esc
//   cancels). States idle/deleting/error(server `detail` as TEXT)/success — on success the store routes to
//   the empty/no-project state and the dialog closes.
// - Valid input range: focus opens on Cancel, NEVER the danger button; a double-submit is blocked while
//   deleting.

import { useEffect, useRef, useState, type ReactElement } from "react";

import { useSceneStore } from "../store/sceneStore";
import { deleteProject, errorText } from "../api/client";

type DeleteState = "idle" | "deleting" | "error";

export function DeleteProjectDialog({
  projectName,
  onClose,
}: {
  readonly projectName: string;
  readonly onClose: () => void;
}): ReactElement {
  const projectId = useSceneStore((s) => s.projectId);
  const resetProject = useSceneStore((s) => s.resetProject);
  const [typed, setTyped] = useState("");
  const [state, setState] = useState<DeleteState>("idle");
  const [error, setError] = useState<string | null>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);

  const matches = typed === projectName;

  // Focus opens on Cancel (never the danger button) — a misclick must not delete.
  useEffect(() => {
    cancelRef.current?.focus();
  }, []);

  // Esc cancels (torn down on unmount).
  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === "Escape" && state !== "deleting") {
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, state]);

  async function confirmDelete(): Promise<void> {
    if (!matches || state === "deleting") {
      return;
    }
    setState("deleting");
    setError(null);
    try {
      await deleteProject(projectId ?? "current");
      resetProject(); // route to the empty/no-project state (success)
      onClose();
    } catch (err) {
      setError(errorText(err, "Delete failed."));
      setState("error"); // dialog stays open, name field retained for a retry
    }
  }

  return (
    <div
      className="overlay-scrim delete-scrim"
      data-testid="delete-dialog"
      role="dialog"
      aria-modal="true"
      aria-label="Delete project"
    >
      <div className="panel delete-panel">
        <div className="delete-head">
          <h2 className="section-title">Delete project</h2>
          <span className="chip crit">IRREVERSIBLE</span>
        </div>
        <p className="delete-body">
          This permanently deletes &ldquo;{projectName}&rdquo; from disk &mdash; its scene, settings, and all
          calculation records. This cannot be undone. Type the project name to confirm.
        </p>
        <input
          className="input"
          data-testid="delete-name-input"
          type="text"
          autoComplete="off"
          spellCheck={false}
          aria-label="Type the project name to confirm"
          value={typed}
          disabled={state === "deleting"}
          onChange={(e) => setTyped(e.target.value)}
        />
        {state === "error" && error ? (
          <p className="form-error" data-testid="delete-error">
            {error}
          </p>
        ) : (
          <p className="delete-hint">Names must match exactly.</p>
        )}
        <div className="btn-row delete-actions">
          <button
            ref={cancelRef}
            type="button"
            className="btn"
            data-testid="delete-cancel"
            disabled={state === "deleting"}
            onClick={onClose}
          >
            Cancel
          </button>
          <button
            type="button"
            className="btn danger"
            data-testid="delete-confirm"
            disabled={!matches || state === "deleting"}
            onClick={() => void confirmDelete()}
          >
            {state === "deleting" ? "deleting…" : "Delete project"}
          </button>
        </div>
      </div>
    </div>
  );
}
