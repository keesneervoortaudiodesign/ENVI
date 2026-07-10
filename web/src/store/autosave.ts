// autosave.ts — debounced committed-edit autosave (D-04). Committed mutations ONLY (a finished shape, a
// released vertex drag, a property/spectrum change, an accepted ground_zone) schedule ONE coalesced
// whole-scene `PUT /projects/{id}/scene`; drag frames never do. A flush-on-unload catches an edit made
// just before the tab closes, and a small state machine drives the project-bar dirty/saving/saved indicator.
//
// # Module I/O
// - Input  the canonical store's `commitEpoch` — the counter bumped ONLY on committed edits, never on the
//   raw `applyTerraDrawChange` (change/drag) path. `useAutosave()` subscribes to it; each bump schedules a
//   debounced save. `beforeunload`/`pagehide` flush any pending save; `saveNow()` backs an explicit Save.
// - Output the coalesced `PUT` (via `sceneStore.saveScene`) and the observable `useAutosaveStore`
//   (`status` ∈ idle/dirty/saving/saved/error, `savedAt`, `error`) the project-bar indicator renders. The
//   server `detail` on a failed save is carried as text (rendered by the indicator, never innerHTML).
// - Valid input range: mounted once (in `App`); the effect tears down its subscription + unload listeners
//   + pending timer on unmount. `scheduleAutosave` is NEVER called from a Terra Draw `change` handler
//   (grep gate): it is invoked solely from the commit-epoch subscription here.

import { useEffect } from "react";
import { create } from "zustand";

import { useSceneStore } from "./sceneStore";
import { ApiError } from "../api/client";

const DEBOUNCE_MS = 750;

export type SaveStatus = "idle" | "dirty" | "saving" | "saved" | "error";

interface AutosaveState {
  readonly status: SaveStatus;
  // Epoch-ms of the last successful save (drives the "Saved · hh:mm" mono clock), or null.
  readonly savedAt: number | null;
  // The server `detail` (text) from the last failed save, or null.
  readonly error: string | null;
  setDirty(): void;
  setSaving(): void;
  setSaved(at: number): void;
  setError(detail: string): void;
}

export const useAutosaveStore = create<AutosaveState>((set) => ({
  status: "idle",
  savedAt: null,
  error: null,
  setDirty: () => set({ status: "dirty", error: null }),
  setSaving: () => set({ status: "saving" }),
  setSaved: (at) => set({ status: "saved", savedAt: at, error: null }),
  setError: (detail) => set({ status: "error", error: detail }),
}));

// Module-level debounce state — one timer, coalescing every committed edit within the window into a
// single PUT. `pending` guards the unload flush so it only fires when there is unsaved work.
let timer: ReturnType<typeof setTimeout> | undefined;
let pending = false;

function errorText(err: unknown): string {
  if (err instanceof ApiError) {
    return err.detail;
  }
  return err instanceof Error ? err.message : "Save failed.";
}

async function runSave(): Promise<void> {
  if (timer) {
    clearTimeout(timer);
    timer = undefined;
  }
  pending = false;
  useAutosaveStore.getState().setSaving();
  try {
    await useSceneStore.getState().saveScene();
    useAutosaveStore.getState().setSaved(Date.now());
  } catch (err) {
    useAutosaveStore.getState().setError(errorText(err));
  }
}

// Schedule a debounced coalesced save. Called ONLY from the committed-edit subscription (never a `change`
// handler) — this is the single scheduling entry point (D-04).
export function scheduleAutosave(): void {
  pending = true;
  useAutosaveStore.getState().setDirty();
  if (timer) {
    clearTimeout(timer);
  }
  timer = setTimeout(() => {
    void runSave();
  }, DEBOUNCE_MS);
}

// Flush any pending save immediately (tab close / navigate-away). No-op when nothing is pending.
export function flushAutosave(): void {
  if (!pending) {
    return;
  }
  void runSave();
}

// Save immediately regardless of the debounce (the explicit Save button).
export function saveNow(): void {
  void runSave();
}

// Wire autosave to committed edits + the unload flush. Mount once at the app root. Every subscription and
// listener + the pending timer is torn down on unmount (imperative-subscription discipline).
export function useAutosave(): void {
  useEffect(() => {
    let prevEpoch = useSceneStore.getState().commitEpoch;
    const unsubscribe = useSceneStore.subscribe((state) => {
      if (state.commitEpoch !== prevEpoch) {
        prevEpoch = state.commitEpoch;
        scheduleAutosave();
      }
    });

    const onUnload = (): void => flushAutosave();
    window.addEventListener("beforeunload", onUnload);
    window.addEventListener("pagehide", onUnload);

    return () => {
      unsubscribe();
      window.removeEventListener("beforeunload", onUnload);
      window.removeEventListener("pagehide", onUnload);
      if (timer) {
        clearTimeout(timer);
        timer = undefined;
      }
    };
  }, []);
}
