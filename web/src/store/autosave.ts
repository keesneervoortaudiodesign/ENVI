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
import { errorText, putScene } from "../api/client";

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
// single PUT. `pending` guards the unload flush so it only fires when there is unsaved work — it is cleared
// only once a save actually SUCCEEDS, so an in-flight or failed save still counts as unsaved work.
let timer: ReturnType<typeof setTimeout> | undefined;
let pending = false;
// In-flight sequencing (ME-01): the currently-running save (or null), plus a single trailing-save latch.
// Two concurrent whole-scene PUTs could complete out of order and persist the STALER snapshot, so saves are
// serialized: while one is in flight a new request only sets `queued`, and a single trailing save re-runs
// with the LATEST store snapshot once the in-flight one settles.
let inFlight: Promise<void> | null = null;
let queued = false;

async function runSave(): Promise<void> {
  if (timer) {
    clearTimeout(timer);
    timer = undefined;
  }
  // Serialize against any in-flight save: request a single trailing re-run instead of starting a second
  // concurrent PUT (ME-01). `pending` is deliberately NOT cleared here — the queued save (or the unload
  // flush) still owes the latest snapshot until a save succeeds.
  if (inFlight) {
    queued = true;
    return;
  }
  inFlight = (async () => {
    useAutosaveStore.getState().setSaving();
    try {
      await useSceneStore.getState().saveScene();
      pending = false; // cleared only on genuine success — the newest snapshot is now persisted
      useAutosaveStore.getState().setSaved(Date.now());
    } catch (err) {
      useAutosaveStore.getState().setError(errorText(err, "Save failed."));
    }
  })();
  await inFlight;
  inFlight = null;
  if (queued) {
    queued = false;
    void runSave(); // re-run once with the latest store snapshot (supersedes the older one)
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

// Flush any pending save on the UNLOAD path (tab close / navigate-away). No-op when nothing is pending.
//
// A normal `fetch` without `keepalive` is NOT guaranteed to complete as the document is torn down — the
// browser cancels in-flight requests — so routing the unload flush through `runSave()`/`saveScene()` would
// silently drop the last edit made inside the debounce window (HI-01, breaking the D-04 flush-on-close
// guarantee). This path instead issues the whole-scene PUT with `{ keepalive: true }`, which the browser
// completes during unload. `sendBeacon` is deliberately NOT used: a whole-scene FeatureCollection can exceed
// its ~64 KB cap. Best-effort only — the indicator is NOT set to "saved" (we cannot await the response and
// the page is going away), keeping the state honest.
export function flushAutosaveOnUnload(): void {
  if (!pending) {
    return;
  }
  if (timer) {
    clearTimeout(timer);
    timer = undefined;
  }
  const state = useSceneStore.getState();
  const id = state.projectId ?? "current";
  try {
    // Reuse the client's scene-PUT seam (path/verb/base live only there) with `keepalive` so the browser
    // completes it during unload. Fire-and-forget: not awaited, and any rejection is swallowed (the page is
    // going away — nothing more can be done, and the indicator is deliberately NOT set to "saved").
    void putScene(id, state.sceneFeatureCollection(), { keepalive: true }).catch(() => {
      /* best-effort on unload */
    });
    pending = false;
  } catch {
    /* best-effort on unload — nothing more can be done as the document tears down */
  }
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

    const onUnload = (): void => flushAutosaveOnUnload();
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
