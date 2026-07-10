// RejectBanner.tsx — the transient ground-zone hard-reject banner (D-07, Surface B). Map-anchored,
// top-center, crit-styled. It appears ONLY while the store holds a `groundReject` (a partial-cross that
// reverted its geometry) and is NEVER a row in the persistent validation panel (Surface A).
//
// # Module I/O
// - Input  the canonical store's `groundReject` (the id of the EXISTING zone the rejected candidate
//   crossed + a nonce) and the `dismissGroundReject` / `zoomToFeature` actions.
// - Output a top-center `.chip.crit`-toned banner explaining the conflict with a "Zoom to conflicting
//   zone" action (targets the EXISTING zone — the rejected polygon has no object to select) and a manual
//   dismiss. Auto-dismisses after a short timeout; the timer resets on each new reject (nonce) and is torn
//   down on unmount / dismiss. All text is a React child (never innerHTML).
// - Valid input range: renders nothing when `groundReject` is null.

import { useEffect, type ReactElement } from "react";

import { useSceneStore } from "../store/sceneStore";

// Auto-dismiss delay — long enough to read + click "Zoom to conflicting zone", short enough to be transient.
const AUTO_DISMISS_MS = 6000;

export function RejectBanner(): ReactElement | null {
  const groundReject = useSceneStore((s) => s.groundReject);
  const dismissGroundReject = useSceneStore((s) => s.dismissGroundReject);
  const zoomToFeature = useSceneStore((s) => s.zoomToFeature);

  // Reset the auto-dismiss timer whenever a NEW reject arrives (nonce changes). Torn down on unmount.
  const nonce = groundReject?.nonce ?? null;
  useEffect(() => {
    if (nonce === null) {
      return;
    }
    const timer = setTimeout(() => dismissGroundReject(), AUTO_DISMISS_MS);
    return () => clearTimeout(timer);
  }, [nonce, dismissGroundReject]);

  if (!groundReject) {
    return null;
  }

  return (
    <div className="reject-banner" data-testid="reject-banner" role="alert">
      <span className="dot crit" aria-hidden="true" />
      <span className="reject-text">
        Ground zone can&rsquo;t partially overlap another. Containment is allowed.
      </span>
      <button
        type="button"
        className="btn dense"
        data-testid="reject-zoom"
        onClick={() => zoomToFeature(groundReject.conflictId)}
      >
        Zoom to conflicting zone
      </button>
      <button
        type="button"
        className="btn dense"
        data-testid="reject-dismiss"
        aria-label="Dismiss"
        onClick={() => dismissGroundReject()}
      >
        Dismiss
      </button>
    </div>
  );
}
