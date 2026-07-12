// WeatherPanel.tsx — the weather-import panel (METX-01, SC4). A date+hour picker, an Import action that
// date-switches Open-Meteo → OPFS cache → WASM A/B/C derivation, a per-azimuth A/B/C readout, the visible
// call-cost weight, and the three debug-overlay toggles (receiver grid / impedance segmentation / screen
// vertices). Mirrors ImportPanel's structure + conventions.
//
// # Module I/O
// - Input  the weather store (date/hour/z₀, status, per-azimuth A/B/C, call-cost, error, debug toggles +
//   status), the import store's viewport (the site lat/lon = viewport centre), and the scene store's open
//   project id (the OPFS cache target). No props.
// - Output the panel JSX: every actionable control carries a `data-testid` (the 09-06 E2E drives these), and
//   every fetched/derived string reaches the DOM as a React text child (NEVER innerHTML; T-09-05-01). Import
//   is disabled without an open project or a known viewport.
// - Valid input range: derives entirely from store state; the acoustic math never runs here (the fit is in
//   the WASM shim behind the store's `runWeatherImport`).

import { type ReactElement } from "react";

import { useWeatherStore, type WeatherStatus } from "../store/weather";
import { useImportStore } from "../store/import";
import { useSceneStore } from "../store/sceneStore";
import { InfoButton } from "../help/InfoButton";

// A short compass label for a display azimuth (readability of the per-azimuth readout).
const COMPASS: Readonly<Record<number, string>> = {
  0: "N",
  45: "NE",
  90: "E",
  135: "SE",
  180: "S",
  225: "SW",
  270: "W",
  315: "NW",
};

// The chip severity for the import status (reusing the shared `.chip.warn` / `.chip.crit` / `.chip.ok`).
function statusSeverity(status: WeatherStatus): "" | "ok" | "warn" | "crit" {
  if (status === "error") {
    return "crit";
  }
  if (status === "derived") {
    return "ok";
  }
  if (status === "fetching" || status === "cached") {
    return "warn";
  }
  return "";
}

// The site (lat, lon) = the current map-viewport centre (the import store's WGS84 viewport). Null with no map.
function useSiteLatLon(): { lat: number; lon: number } | null {
  const viewport = useImportStore((s) => s.viewport);
  if (!viewport) {
    return null;
  }
  return {
    lat: (viewport.min_lat + viewport.max_lat) / 2,
    lon: (viewport.min_lon + viewport.max_lon) / 2,
  };
}

function AbcTable(): ReactElement | null {
  const abc = useWeatherStore((s) => s.abc);
  if (abc.length === 0) {
    return null;
  }
  return (
    <table className="mono" data-testid="weather-abc-table">
      <thead>
        <tr>
          <th>Azimuth</th>
          <th>A</th>
          <th>B</th>
          <th>C</th>
        </tr>
      </thead>
      <tbody>
        {abc.map(({ azimuth, profile }) => (
          <tr key={azimuth} data-testid={`weather-abc-${azimuth}`}>
            <td>
              {azimuth}° {COMPASS[azimuth] ?? ""}
            </td>
            <td>{profile.a.toFixed(3)}</td>
            <td>{profile.b.toExponential(2)}</td>
            <td>{profile.c.toFixed(2)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

export function WeatherPanel(): ReactElement {
  const date = useWeatherStore((s) => s.date);
  const hour = useWeatherStore((s) => s.hour);
  const z0 = useWeatherStore((s) => s.z0);
  const status = useWeatherStore((s) => s.status);
  const fromCache = useWeatherStore((s) => s.fromCache);
  const callCostWeight = useWeatherStore((s) => s.callCostWeight);
  const error = useWeatherStore((s) => s.error);
  const setDate = useWeatherStore((s) => s.setDate);
  const setHour = useWeatherStore((s) => s.setHour);
  const setZ0 = useWeatherStore((s) => s.setZ0);
  const runWeatherImport = useWeatherStore((s) => s.runWeatherImport);

  const showGrid = useWeatherStore((s) => s.showGrid);
  const showImpedance = useWeatherStore((s) => s.showImpedance);
  const showScreens = useWeatherStore((s) => s.showScreens);
  const toggleGrid = useWeatherStore((s) => s.toggleGrid);
  const toggleImpedance = useWeatherStore((s) => s.toggleImpedance);
  const toggleScreens = useWeatherStore((s) => s.toggleScreens);
  const debugStatus = useWeatherStore((s) => s.debugStatus);
  const debugBusy = useWeatherStore((s) => s.debugBusy);
  const runDebugGeometry = useWeatherStore((s) => s.runDebugGeometry);

  const projectId = useSceneStore((s) => s.projectId);
  const site = useSiteLatLon();
  const canImport = !!projectId && !!site && status !== "fetching";
  const severity = statusSeverity(status);

  return (
    <section className="panel" data-testid="weather-panel">
      <div className="panel-header">Weather</div>

      {projectId ? null : (
        <div className="empty-state" data-testid="weather-no-project">
          Open a project to import weather.
        </div>
      )}

      <div className="btn-row">
        <label className="field-label">
          Date
          <InfoButton controlId="weather.date" />
          <input
            type="date"
            className="field-input"
            value={date}
            data-testid="weather-date"
            onChange={(e) => setDate(e.target.value)}
          />
        </label>
        <label className="field-label">
          Hour (UTC)
          <InfoButton controlId="weather.hour" />
          <input
            type="number"
            min={0}
            max={23}
            className="field-input"
            value={hour}
            data-testid="weather-hour"
            onChange={(e) => setHour(Number(e.target.value))}
          />
        </label>
        <label className="field-label">
          z₀ (m)
          <InfoButton controlId="weather.z0" />
          <input
            type="number"
            min={0.001}
            step={0.01}
            className="field-input"
            value={z0}
            data-testid="weather-z0"
            onChange={(e) => setZ0(Number(e.target.value))}
          />
        </label>
      </div>

      <div className="btn-row">
        <button
          type="button"
          className="btn"
          data-testid="weather-import"
          disabled={!canImport}
          onClick={() => {
            if (projectId && site) {
              void runWeatherImport(projectId, site.lat, site.lon);
            }
          }}
        >
          Import weather
        </button>
        <InfoButton controlId="weather.import" />
        <span className={`chip ${severity}`} data-testid="weather-status">
          {status}
        </span>
        {status === "derived" || status === "cached" ? (
          <span className="chip" data-testid="weather-cache">
            {fromCache ? "OPFS cache (no call)" : "network fetch"}
          </span>
        ) : null}
      </div>

      {callCostWeight !== undefined ? (
        <div className="mono" data-testid="weather-callcost">
          call-cost weight ≈ {callCostWeight.toFixed(2)}
        </div>
      ) : null}

      {status === "error" && error ? (
        <div className="chip crit" data-testid="weather-error">
          {error.detail}
        </div>
      ) : null}

      <AbcTable />

      <div className="panel-header">Debug overlays</div>
      <div className="btn-row">
        <label className="btn-row">
          <input
            type="checkbox"
            checked={showGrid}
            data-testid="weather-toggle-grid"
            onChange={() => toggleGrid()}
          />
          <span className="issue-text">Receiver grid</span>
        </label>
        <InfoButton controlId="weather.debug_grid" />
        <label className="btn-row">
          <input
            type="checkbox"
            checked={showImpedance}
            data-testid="weather-toggle-impedance"
            onChange={() => toggleImpedance()}
          />
          <span className="issue-text">Impedance segmentation</span>
        </label>
        <InfoButton controlId="weather.debug_impedance" />
        <label className="btn-row">
          <input
            type="checkbox"
            checked={showScreens}
            data-testid="weather-toggle-screens"
            onChange={() => toggleScreens()}
          />
          <span className="issue-text">Screen vertices</span>
        </label>
        <InfoButton controlId="weather.debug_screens" />
      </div>
      <div className="btn-row">
        <button
          type="button"
          className="btn"
          data-testid="weather-compute-debug"
          disabled={debugBusy}
          onClick={() => void runDebugGeometry()}
        >
          Compute debug geometry
        </button>
        <InfoButton controlId="weather.compute_debug" />
      </div>
      {debugStatus ? (
        <div className="issue-text" data-testid="weather-debug-status">
          {debugStatus}
        </div>
      ) : null}
    </section>
  );
}
