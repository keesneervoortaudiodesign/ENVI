// ConditioningPanel.tsx — the FLAGSHIP interactive conditioning surface (WEB-05 /
// SVC-06, SC2). Per-source Gain (dB) + Delay (ms) numeric inputs + a Mute toggle + a
// Filter control that REUSES the Phase-7 `SpectrumEditor` verbatim (D-11, opened via the
// canonical scene store's spectrum-editor overlay). Every edit drives the DEBOUNCED
// (~150 ms, D-10) recondition MAC over the cached tensor — spectra + the isophone map
// update LIVE with no button and no re-propagation. Two honest-state guarantees are
// surfaced here: a `.chip.warn` "Out of date" badge the moment the re-minted identity
// diverges (D-12; the badge NEVER blocks edits), and the SVC-06 409 reject banner when a
// MAC is refused against a mismatched tensor hash (never silently served).
//
// # Module I/O
// - Input  the results manifest (the sub-source set + the seeded conditioning drive), the
//   conditioning store (per-source drive + refuse/pending), the stale store (`isStale`),
//   and — for the filter — the reused SpectrumEditor's authored spectrum in the scene
//   store, materialised to a dense `[105]` by the SERVER (D-11, no TS acoustic math).
// - Output the per-source control rows + the stale badge + the reject banner. Fills the
//   11-05 ResultsPanel slot; renders nothing until a result exists.

import { useEffect, useRef, type ReactElement } from "react";

import {
  createWasmConditioningClient,
  defaultConditioning,
  useConditioningStore,
} from "../store/conditioning";
import { createWasmIdentityClient, useStaleStore, useStaleWatch } from "../store/stale";
import { useResultsStore } from "../store/results";
import { useSceneStore } from "../store/sceneStore";
import { useSpectrumPreview, N_BANDS } from "../spectrum/interpolateClient";
import { InfoButton } from "../help/InfoButton";

// The scene-store spectrum key a source's reused-SpectrumEditor filter authors under. A
// distinct namespace so it never collides with a feature/edge isolation spectrum (D-11).
export function filterSpectrumKey(sourceId: string): string {
  return `conditioning-filter:${sourceId}`;
}

// The UI-SPEC copy (§Copywriting Contract) — verbatim, English-only.
const STALE_DETAIL =
  "The scene changed since this result was computed. Recompute to refresh the noise map and spectra.";
const REJECT_COPY =
  "Can't recondition — this result no longer matches the current scene. Recompute first.";

export function ConditioningPanel(): ReactElement | null {
  const manifest = useResultsStore((s) => s.manifest);
  const order = useConditioningStore((s) => s.order);
  const perSource = useConditioningStore((s) => s.perSource);
  const refuse = useConditioningStore((s) => s.refuse);
  const pending = useConditioningStore((s) => s.pending);
  const seedFromManifest = useConditioningStore((s) => s.seedFromManifest);
  const attachConditioningClient = useConditioningStore((s) => s.attachConditioningClient);
  const hasConditioningClient = useConditioningStore((s) => s.client !== null);
  const setGain = useConditioningStore((s) => s.setGain);
  const setDelay = useConditioningStore((s) => s.setDelay);
  const setMuted = useConditioningStore((s) => s.setMuted);

  const isStale = useStaleStore((s) => s.isStale);
  const attachIdentityClient = useStaleStore((s) => s.attachIdentityClient);
  const hasIdentityClient = useStaleStore((s) => s.client !== null);

  const openSpectrumEditor = useSceneStore((s) => s.openSpectrumEditor);

  // Wire the stale-badge watcher (re-mint on a scene/grid edit; conditioning never stales).
  useStaleWatch();

  // Attach the real wasm clients once (kept out of module load so Node unit tests never
  // pull the wasm graph). Guarded against an HMR remount spinning a second client.
  useEffect(() => {
    if (!hasConditioningClient) {
      attachConditioningClient(createWasmConditioningClient());
    }
    if (!hasIdentityClient) {
      attachIdentityClient(createWasmIdentityClient());
    }
  }, [hasConditioningClient, hasIdentityClient, attachConditioningClient, attachIdentityClient]);

  // Seed the drive from the manifest whenever a fresh result lands (keyed by identity so a
  // conditioning-only manifest swap does not re-seed over the user's live edits).
  const tensorHash = manifest?.tensorHash ?? null;
  useEffect(() => {
    if (manifest) {
      seedFromManifest(manifest.perSourceConditioning);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tensorHash]);

  if (!manifest) {
    return null;
  }

  return (
    <div className="panel-section conditioning-panel" data-testid="conditioning-panel">
      <div className="panel-header conditioning-header">
        <span className="section-title">Conditioning</span>
        {isStale ? (
          <span className="chip warn" data-testid="conditioning-stale-badge">
            Out of date
          </span>
        ) : null}
        {pending ? (
          <span className="chip off" data-testid="conditioning-pending" aria-hidden="true">
            recalc…
          </span>
        ) : null}
      </div>

      {isStale ? (
        <p className="issue-text" data-testid="conditioning-stale-detail">
          {STALE_DETAIL}
        </p>
      ) : null}

      {refuse ? (
        <div className="reject-banner conditioning-reject" data-testid="conditioning-reject" role="alert">
          <span className="dot crit" aria-hidden="true" />
          <span className="reject-text">{REJECT_COPY}</span>
        </div>
      ) : null}

      {order.map((id, i) => {
        const c = perSource[id] ?? defaultConditioning();
        const hasFilter = c.filter_band_db !== null;
        return (
          <div className="conditioning-source" data-testid={`conditioning-source-${id}`} key={id}>
            <FilterMaterializer sourceId={id} />
            <div className="conditioning-source-head mono">Source {i + 1}</div>

            <label className="field-row">
              <span className="field-label">
                Gain
                <InfoButton controlId="conditioning.gain" />
              </span>
              <input
                type="number"
                className="field-input input dense mono"
                step="0.5"
                value={c.gain_db}
                data-testid={`conditioning-gain-${id}`}
                aria-label={`Source ${i + 1} gain, dB`}
                onChange={(e) => setGain(id, Number(e.target.value))}
              />
              <span className="field-unit">dB</span>
            </label>

            <label className="field-row">
              <span className="field-label">
                Delay
                <InfoButton controlId="conditioning.delay" />
              </span>
              <input
                type="number"
                className="field-input input dense mono"
                step="0.1"
                min={0}
                value={c.delay_ms}
                data-testid={`conditioning-delay-${id}`}
                aria-label={`Source ${i + 1} delay, ms`}
                onChange={(e) => setDelay(id, Number(e.target.value))}
              />
              <span className="field-unit">ms</span>
            </label>

            <div className="conditioning-controls">
              <button
                type="button"
                className={`btn dense${hasFilter ? " active" : ""}`}
                data-testid={`conditioning-filter-${id}`}
                onClick={() => openSpectrumEditor(filterSpectrumKey(id), `Source ${i + 1} filter`)}
              >
                {hasFilter ? "Edit filter" : "Add filter"}
              </button>
              <InfoButton controlId="conditioning.filter" />
              <label className="conditioning-mute">
                <input
                  type="checkbox"
                  data-testid={`conditioning-mute-${id}`}
                  checked={c.muted}
                  onChange={(e) => setMuted(id, e.target.checked)}
                />
                <span>Mute</span>
              </label>
              <InfoButton controlId="conditioning.mute" />
            </div>
          </div>
        );
      })}
    </div>
  );
}

// A render-nothing child that materialises one source's reused-SpectrumEditor filter into
// the conditioning drive. The authored coarse spectrum lives in the scene store (written by
// the reused editor, D-11); the dense `[105]` is DERIVED by the SERVER (`useSpectrumPreview`
// — the same debounced interpolation the editor's own curve uses), so the frontend authors
// NO dB itself. Setting the dense filter drives the debounced recondition MAC.
function FilterMaterializer({ sourceId }: { readonly sourceId: string }): null {
  const authored = useSceneStore((s) => s.spectra[filterSpectrumKey(sourceId)] ?? null);
  const setFilter = useConditioningStore((s) => s.setFilter);
  const preview = useSpectrumPreview(authored);
  const dense = preview.dense;
  // Track whether a filter was materialised so clearing the authored spectrum removes it
  // from the drive (rather than the mount no-op scheduling a needless recalc).
  const hadFilter = useRef(false);

  useEffect(() => {
    if (authored === null) {
      if (hadFilter.current) {
        setFilter(sourceId, null);
        hadFilter.current = false;
      }
      return;
    }
    if (dense && dense.length === N_BANDS) {
      setFilter(sourceId, dense);
      hadFilter.current = true;
    }
  }, [authored, dense, sourceId, setFilter]);

  return null;
}
