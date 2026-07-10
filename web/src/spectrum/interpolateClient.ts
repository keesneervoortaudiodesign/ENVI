// interpolateClient.ts — the SERVER-owned isolation-spectrum interpolation seam (D-05) for the editor.
//
// # Module I/O
// - Input  an authored coarse spectrum (`AuthoredSpectrumDto`: `{ resolution, values }`) and, for axis
//   labels, nothing (the axis is fetched from the server). NO Hz-based acoustic math happens here — the
//   dense `[105]` preview is produced by `POST /api/v1/meta/interpolate-spectrum` (SVC-07), and the axis
//   by `GET /meta/freq-axis`; this module only debounces, aborts superseded calls, and does band-INDEX
//   arithmetic (which grid indices the octave/third/twelfth anchors land on — structural, not acoustic).
// - Output `useFreqAxis()` (the 105-band axis, fetched once), `useSpectrumPreview(authored)` (a debounced
//   dense `[105]` preview + loading/error state), and the pure `anchorIndices` /
//   `hzLabelForIndex` band-index helpers the curve + table render against.
// - Valid input range: `values.length` must equal the resolution's anchor count (9 / 27 / 105); a wrong
//   length or out-of-range `R` surfaces as the server's `4xx` `detail` string in the preview error state.

import { useEffect, useRef, useState } from "react";

import type { AuthoredSpectrumDto, FreqAxisDto, Resolution } from "../generated/wire";
import { errorText, getFreqAxis, interpolateSpectrum } from "../api/client";

// The total 1/12-octave band count (mirrors envi_engine::freq::N_BANDS; the axis served at /meta/freq-axis
// carries the authoritative value, this is only the array length the editor renders against).
export const N_BANDS = 105;

// The exact band INDICES the anchors of a resolution land on — the "octave/third centres fall exactly on
// 1/12-octave band indices" guarantee made structural (RESEARCH Pattern 6):
//   octave  → 4 + 12·k  (k 0..8)   → 4, 16, 28, …, 100
//   third   → 4·k       (k 0..26)  → 0, 4, 8, …, 104
//   twelfth → 0..104 (every band)
export function anchorIndices(resolution: Resolution): number[] {
  switch (resolution) {
    case "octave":
      return Array.from({ length: 9 }, (_, k) => 4 + 12 * k);
    case "third":
      return Array.from({ length: 27 }, (_, k) => 4 * k);
    case "twelfth":
      return Array.from({ length: N_BANDS }, (_, i) => i);
  }
}

// A display-ONLY Hz label for a band index, sourced from the freq axis (never hardcoded, never a key). A
// 1/3-octave centre uses its nominal label (25, 31.5, …); any other 1/12 band uses its exact centre Hz.
export function hzLabelForIndex(axis: FreqAxisDto | null, index: number): string {
  if (!axis) {
    return "";
  }
  const thirdPos = axis.third_octave_indices.indexOf(index);
  if (thirdPos >= 0 && thirdPos < axis.nominal_third_octave_hz.length) {
    const hz = axis.nominal_third_octave_hz[thirdPos];
    return hz >= 1000 ? `${hz / 1000} kHz` : `${hz} Hz`;
  }
  const centre = axis.centres_hz[index];
  if (centre === undefined) {
    return "";
  }
  return centre >= 1000 ? `${(centre / 1000).toFixed(2)} kHz` : `${centre.toFixed(1)} Hz`;
}

// Fetch the 105-band frequency axis once (the axis labels + third-octave tick indices). Torn down cleanly:
// a superseded/aborted fetch never writes state after unmount (subscription discipline).
export function useFreqAxis(): FreqAxisDto | null {
  const [axis, setAxis] = useState<FreqAxisDto | null>(null);
  useEffect(() => {
    const controller = new AbortController();
    getFreqAxis(controller.signal)
      .then((a) => setAxis(a))
      .catch(() => {
        /* axis unavailable — the curve renders without Hz labels; band index still drives the plot */
      });
    return () => controller.abort();
  }, []);
  return axis;
}

// The debounced server preview state.
export interface SpectrumPreview {
  readonly dense: number[] | null; // the dense [105] r_db grid, or null before the first response
  readonly loading: boolean;
  readonly error: string | null; // the server's path-redacted `detail`, rendered as TEXT by the caller
}

// Debounced `POST /meta/interpolate-spectrum` (D-05): coalesces rapid edits into one request (~250 ms) and
// aborts the superseded call. Returns the dense `[105]` preview; the interpolation math lives SERVER-side.
export function useSpectrumPreview(authored: AuthoredSpectrumDto | null, debounceMs = 250): SpectrumPreview {
  const [preview, setPreview] = useState<SpectrumPreview>({ dense: null, loading: false, error: null });
  // Serialize the request so effect re-runs only on a real content change (not identity churn).
  const key = authored ? `${authored.resolution}:${authored.values.join(",")}` : null;
  const controllerRef = useRef<AbortController | null>(null);

  useEffect(() => {
    if (!authored || key === null) {
      setPreview({ dense: null, loading: false, error: null });
      return;
    }
    setPreview((p) => ({ dense: p.dense, loading: true, error: null }));
    const timer = setTimeout(() => {
      controllerRef.current?.abort();
      const controller = new AbortController();
      controllerRef.current = controller;
      interpolateSpectrum(
        { resolution: authored.resolution, values: authored.values },
        controller.signal,
      )
        .then((resp) => {
          if (!controller.signal.aborted) {
            setPreview({ dense: resp.r_db, loading: false, error: null });
          }
        })
        .catch((err: unknown) => {
          if (controller.signal.aborted) {
            return;
          }
          const detail = errorText(err, "Interpolation request failed.");
          setPreview((p) => ({ dense: p.dense, loading: false, error: detail })); // keep last-good curve
        });
    }, debounceMs);
    return () => clearTimeout(timer);
    // `key` captures the authored content; re-run only when it changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, debounceMs]);

  return preview;
}

// Materialize the dense [105] grid for `authored` via the server (one-shot, un-debounced) — used by the
// resolution re-projection + promote-to-twelfth paths. The band-index math is the server's, not ours.
export async function materializeDense(authored: AuthoredSpectrumDto): Promise<number[]> {
  const resp = await interpolateSpectrum({ resolution: authored.resolution, values: authored.values });
  return resp.r_db;
}

// Re-project an authored spectrum to a NEW resolution non-destructively (D-06): materialize the dense grid
// server-side, then SAMPLE it at the new resolution's anchor indices (pure indexing, not acoustic math).
export async function reprojectAuthored(
  authored: AuthoredSpectrumDto,
  newResolution: Resolution,
): Promise<AuthoredSpectrumDto> {
  if (authored.resolution === newResolution) {
    return authored;
  }
  const dense = await materializeDense(authored);
  const values = anchorIndices(newResolution).map((i) => dense[i] ?? 0);
  return { resolution: newResolution, values };
}
