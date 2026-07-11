// weather.ts — the client-side weather-import state slice (METX-01, D-01/D-02/D-03). Mirrors the `import.ts`
// shape: an in-app state machine (idle → fetching → cached → derived / error) for a single representative
// hour, the derived per-azimuth A/B/C, the visible call-cost weight, and the three debug-overlay toggles +
// their computed geometry. The acoustic A/B/C fit runs in WASM (`deriveAbc`); this store never does acoustic
// arithmetic.
//
// # Module I/O
// - Input  the WeatherPanel's date+hour+z₀ picker, its Import + Compute-debug actions, and the site `(lat,
//   lon)` the panel reads from the map viewport. `runWeatherImport` drives `fetchWeather` (OPFS-cached, D-03)
//   then `deriveAbc` (WASM); `runDebugGeometry` drives the geometry shims over the scene.
// - Output the `status` state machine, `components` + per-azimuth `abc`, `fromCache` + `callCostWeight` (the
//   SC4 visibility), the `error` (rendered as TEXT, never innerHTML), and the debug toggles + `debug`
//   geometry the overlays read. The single source of truth for the weather UI.
// - Valid input range: `date` is `YYYY-MM-DD`, `hour` ∈ [0, 23] (UTC), `z0` > 0. A missing project/viewport is
//   guarded by the panel (Import disabled).

import { create } from "zustand";

import type { SoundSpeedProfileDto, WeatherComponentsDto } from "../generated/wire";
import { errorText, toStatusError } from "../api/client";
import { deriveAbc, fetchWeather } from "../import/weather";
import { computeDebugGeometry, type DebugGeometry } from "../import/sceneDebug";

// The import state machine (D-08 in-app state, mirroring the dgm/import slices' honest success/failure).
export type WeatherStatus = "idle" | "fetching" | "cached" | "derived" | "error";

// One path azimuth's derived sound-speed profile (the per-azimuth A/B/C readout).
export interface AzimuthAbc {
  readonly azimuth: number;
  readonly profile: SoundSpeedProfileDto;
}

// A structured failure: HTTP-ish status + a detail (rendered as TEXT, never innerHTML — T-09-05-01).
export interface WeatherError {
  readonly status: number;
  readonly detail: string;
}

// The 8 compass path azimuths the panel derives A/B/C for (degrees clockwise from north). A fixed display
// set — the real per-path fan-out is Phase 10; here the 8 sectors make downwind-vs-upwind A visible.
export const DISPLAY_AZIMUTHS: readonly number[] = [0, 45, 90, 135, 180, 225, 270, 315];

function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

export interface WeatherState {
  // The single-hour selection (D-01) + the fit roughness length.
  readonly date: string;
  readonly hour: number;
  readonly z0: number;

  readonly status: WeatherStatus;
  // Whether the last import was served from OPFS (zero network — SC4) and the logged call-cost weight.
  readonly fromCache: boolean;
  readonly callCostWeight: number | undefined;
  // The bearing-independent decomposition + one A/B/C profile per display azimuth.
  readonly components: WeatherComponentsDto | null;
  readonly abc: readonly AzimuthAbc[];
  readonly error: WeatherError | null;

  // Debug overlays (SC — receiver grid / impedance segmentation / screen vertices).
  readonly showGrid: boolean;
  readonly showImpedance: boolean;
  readonly showScreens: boolean;
  readonly debug: DebugGeometry | null;
  readonly debugStatus: string;
  readonly debugBusy: boolean;

  setDate(date: string): void;
  setHour(hour: number): void;
  setZ0(z0: number): void;
  toggleGrid(): void;
  toggleImpedance(): void;
  toggleScreens(): void;

  // Import the multi-level weather for a site + the selected hour, then derive per-azimuth A/B/C. A cache hit
  // issues zero network calls (SC4). Errors land in `error` (state → "error"), never thrown to the caller.
  runWeatherImport(projectId: string, lat: number, lon: number): Promise<void>;
  // Compute the debug geometry (receiver grid / impedance segmentation / screen vertices) over the scene.
  runDebugGeometry(): Promise<void>;
}

function toError(err: unknown): WeatherError {
  return toStatusError(err, "Weather import failed.");
}

export const useWeatherStore = create<WeatherState>((set, get) => ({
  date: todayIso(),
  hour: 12,
  z0: 0.05,

  status: "idle",
  fromCache: false,
  callCostWeight: undefined,
  components: null,
  abc: [],
  error: null,

  showGrid: false,
  showImpedance: false,
  showScreens: false,
  debug: null,
  debugStatus: "",
  debugBusy: false,

  setDate: (date) => set({ date }),
  setHour: (hour) => set({ hour: Math.max(0, Math.min(23, Math.trunc(hour))) }),
  setZ0: (z0) => set({ z0: z0 > 0 ? z0 : 0.001 }),
  toggleGrid: () => set((s) => ({ showGrid: !s.showGrid })),
  toggleImpedance: () => set((s) => ({ showImpedance: !s.showImpedance })),
  toggleScreens: () => set((s) => ({ showScreens: !s.showScreens })),

  runWeatherImport: async (projectId, lat, lon) => {
    const { date, hour, z0 } = get();
    set({ status: "fetching", error: null });
    try {
      const fetched = await fetchWeather(projectId, lat, lon, date, hour);
      set({
        status: "cached",
        fromCache: fetched.fromCache,
        callCostWeight: fetched.callCostWeight,
      });
      // The acoustic A/B/C fit runs entirely in WASM (deriveAbc → derive_weather).
      const result = await deriveAbc(fetched.json, hour, DISPLAY_AZIMUTHS, z0);
      const abc: AzimuthAbc[] = DISPLAY_AZIMUTHS.map((azimuth, i) => ({
        azimuth,
        profile: result.profiles[i],
      }));
      set({ status: "derived", components: result.components, abc });
    } catch (err) {
      set({ status: "error", error: toError(err) });
    }
  },

  runDebugGeometry: async () => {
    set({ debugBusy: true });
    try {
      const { geometry, notes } = await computeDebugGeometry();
      set({
        debug: geometry,
        debugStatus: notes.length > 0 ? notes.join(" ") : "Debug geometry computed.",
        debugBusy: false,
      });
    } catch (err) {
      set({
        debugStatus: errorText(err, "Debug geometry failed."),
        debugBusy: false,
      });
    }
  },
}));
