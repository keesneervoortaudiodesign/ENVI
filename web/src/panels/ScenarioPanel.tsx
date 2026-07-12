// ScenarioPanel.tsx — the weather WHAT-IF Scenario Manager (WEB-12 / METX-03/04,
// D-13/14/15/16). Fills the 11-05 ResultsPanel slot. Lists the named scenarios (base +
// user), offers "New scenario" (clone-then-edit, D-13), friendly met inputs (T/RH/p, a
// Beaufort wind class + direction, a temperature gradient, a downwind-worst-case toggle)
// with an ADVANCED raw per-azimuth A/B/C disclosure (D-14), a per-scenario Compute
// (each computes its OWN hash-keyed cached tensor — a met change is a recompute), an
// instant Switch, "Compare scenarios" (pick A and B → the diverging difference map,
// D-16), and a destructive delete-scenario confirm.
//
// # Module I/O
// - Input  the scenario store (list + active + compare + compute state) and the
//   difference store (the A − B delta the map renders). The friendly/advanced knobs
//   edit the ACTIVE scenario's met; Compute derives the per-azimuth A/B/C in WASM +
//   solves its cached tensor; Compare feeds the two scenarios' cached dB(A) totals to
//   the WASM difference boundary.
// - Output the scenario list + met editor + compare picker + delete confirm. Renders
//   the "One scenario" empty state when only the base exists. No acoustic math here.

import { useEffect, useState, type ReactElement } from "react";

import {
  BEAUFORT_MS,
  createWasmScenarioComputeClient,
  useScenarioStore,
  type MetOverrides,
} from "../store/scenarios";
import { createWasmDifferenceClient, useDifferenceStore } from "../store/difference";

// UI-SPEC §Copywriting Contract copy — verbatim, English-only.
const EMPTY_HEADING = "One scenario";
const EMPTY_BODY =
  "You have the base scenario only. Create a weather scenario to compare conditions.";

export function ScenarioPanel(): ReactElement {
  const scenarios = useScenarioStore((s) => s.scenarios);
  const activeId = useScenarioStore((s) => s.activeId);
  const compareA = useScenarioStore((s) => s.compareA);
  const compareB = useScenarioStore((s) => s.compareB);
  const computingId = useScenarioStore((s) => s.computingId);
  const error = useScenarioStore((s) => s.error);
  const hasClient = useScenarioStore((s) => s.client !== null);

  const attachClient = useScenarioStore((s) => s.attachClient);
  const newScenario = useScenarioStore((s) => s.newScenario);
  const renameScenario = useScenarioStore((s) => s.renameScenario);
  const setMet = useScenarioStore((s) => s.setMet);
  const computeScenario = useScenarioStore((s) => s.computeScenario);
  const switchScenario = useScenarioStore((s) => s.switchScenario);
  const setCompare = useScenarioStore((s) => s.setCompare);

  const attachDifferenceClient = useDifferenceStore((s) => s.attachClient);
  const hasDifferenceClient = useDifferenceStore((s) => s.client !== null);
  const computeDifference = useDifferenceStore((s) => s.compute);
  const clearDifference = useDifferenceStore((s) => s.clear);

  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);

  // Attach the real wasm clients once (kept out of module load so Node unit tests never
  // pull the wasm graph). Guarded against an HMR remount spinning a second client.
  useEffect(() => {
    if (!hasClient) {
      attachClient(createWasmScenarioComputeClient());
    }
    if (!hasDifferenceClient) {
      attachDifferenceClient(createWasmDifferenceClient());
    }
  }, [hasClient, hasDifferenceClient, attachClient, attachDifferenceClient]);

  const active = scenarios.find((s) => s.id === activeId) ?? scenarios[0];
  const met = active.met;

  // Read out the two compared scenarios' cached totals and drive the WASM difference.
  function runCompare(): void {
    const a = scenarios.find((s) => s.id === compareA);
    const b = scenarios.find((s) => s.id === compareB);
    if (!a?.solve || !b?.solve) {
      return;
    }
    void computeDifference({
      aDba: a.solve.totalsDba,
      bDba: b.solve.totalsDba,
      grid: a.solve.grid,
      crs: a.solve.crs,
      labelA: a.name,
      labelB: b.name,
    });
  }

  return (
    <div className="panel-section scenario-panel" data-testid="scenario-panel">
      <div className="panel-header scenario-header">
        <span className="section-title">Weather scenarios</span>
        <button
          type="button"
          className="btn dense"
          data-testid="scenario-new"
          onClick={() => newScenario()}
        >
          New scenario
        </button>
      </div>

      {scenarios.length === 1 ? (
        <div className="scenario-empty" data-testid="scenario-empty">
          <div className="section-title">{EMPTY_HEADING}</div>
          <p className="issue-text">{EMPTY_BODY}</p>
        </div>
      ) : null}

      {/* The named scenario list (base + user). */}
      <ul className="scenario-list" data-testid="scenario-list">
        {scenarios.map((s) => (
          <li
            key={s.id}
            className={`scenario-row${s.id === activeId ? " active" : ""}`}
            data-testid={`scenario-row-${s.id}`}
            data-active={s.id === activeId}
            data-computed={s.computed}
          >
            <button
              type="button"
              className="scenario-name"
              data-testid={`scenario-switch-${s.id}`}
              onClick={() => switchScenario(s.id)}
            >
              {s.name}
            </button>
            {s.computed ? (
              <span className="chip ok" data-testid={`scenario-cached-${s.id}`}>
                cached
              </span>
            ) : (
              <span className="chip warn" data-testid={`scenario-stale-${s.id}`}>
                not computed
              </span>
            )}
            <button
              type="button"
              className="btn dense"
              data-testid={`scenario-compute-${s.id}`}
              disabled={computingId === s.id}
              onClick={() => void computeScenario(s.id)}
            >
              {computingId === s.id ? "computing…" : "Compute"}
            </button>
            {s.id !== "base" ? (
              <button
                type="button"
                className="btn dense danger"
                data-testid={`scenario-delete-${s.id}`}
                onClick={() => setDeleteTarget(s.id)}
              >
                Delete
              </button>
            ) : null}
          </li>
        ))}
      </ul>

      {error ? (
        <p className="form-error" data-testid="scenario-error">
          {error}
        </p>
      ) : null}

      {/* The met editor for the ACTIVE scenario (friendly + advanced). */}
      <div className="scenario-met" data-testid="scenario-met">
        <div className="scenario-met-head">
          <input
            className="input dense"
            data-testid="scenario-name-input"
            aria-label="Scenario name"
            value={active.name}
            onChange={(e) => renameScenario(active.id, e.target.value)}
          />
          <button
            type="button"
            className="btn dense"
            data-testid="scenario-save"
            onClick={() => void computeScenario(active.id)}
          >
            Save scenario
          </button>
        </div>

        <div className="scenario-mode">
          <label className="scenario-mode-opt">
            <input
              type="radio"
              name="scenario-met-mode"
              data-testid="scenario-mode-friendly"
              checked={met.mode === "friendly"}
              onChange={() => setMet(active.id, { mode: "friendly" })}
            />
            <span>Friendly</span>
          </label>
          <label className="scenario-mode-opt">
            <input
              type="radio"
              name="scenario-met-mode"
              data-testid="scenario-mode-advanced"
              checked={met.mode === "advanced"}
              onChange={() => setMet(active.id, { mode: "advanced" })}
            />
            <span>Advanced (raw A/B/C)</span>
          </label>
        </div>

        {met.mode === "friendly" ? (
          <FriendlyInputs met={met} onChange={(patch) => setMet(active.id, patch)} />
        ) : (
          <AdvancedInputs met={met} onChange={(patch) => setMet(active.id, patch)} />
        )}
      </div>

      {/* Compare scenarios → the diverging difference map (D-16). */}
      <div className="scenario-compare" data-testid="scenario-compare">
        <div className="section-title">Compare scenarios</div>
        <div className="scenario-compare-picker">
          <label className="field-row">
            <span className="field-label">A</span>
            <select
              className="input dense"
              data-testid="scenario-compare-a"
              value={compareA ?? ""}
              onChange={(e) => setCompare(e.target.value || null, compareB)}
            >
              <option value="">—</option>
              {scenarios.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </select>
          </label>
          <label className="field-row">
            <span className="field-label">B</span>
            <select
              className="input dense"
              data-testid="scenario-compare-b"
              value={compareB ?? ""}
              onChange={(e) => setCompare(compareA, e.target.value || null)}
            >
              <option value="">—</option>
              {scenarios.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </select>
          </label>
        </div>
        <div className="btn-row">
          <button
            type="button"
            className="btn dense primary"
            data-testid="scenario-compare-run"
            disabled={!compareA || !compareB || compareA === compareB}
            onClick={runCompare}
          >
            Compare scenarios
          </button>
          <button
            type="button"
            className="btn dense"
            data-testid="scenario-compare-clear"
            onClick={() => clearDifference()}
          >
            Clear map
          </button>
        </div>
      </div>

      {deleteTarget ? (
        <DeleteScenarioDialog
          name={scenarios.find((s) => s.id === deleteTarget)?.name ?? ""}
          scenarioId={deleteTarget}
          onClose={() => setDeleteTarget(null)}
        />
      ) : null}
    </div>
  );
}

// The friendly met inputs (T/RH/p, Beaufort wind + direction, temp gradient, downwind
// worst-case) — the knobs the WASM derivation turns into per-azimuth A/B/C (D-14).
function FriendlyInputs({
  met,
  onChange,
}: {
  readonly met: MetOverrides;
  readonly onChange: (patch: Partial<MetOverrides>) => void;
}): ReactElement {
  return (
    <div className="scenario-friendly" data-testid="scenario-friendly">
      <NumberField
        label="Temp"
        unit="°C"
        step={0.5}
        value={met.temperature_c}
        testid="scenario-temp"
        onChange={(v) => onChange({ temperature_c: v })}
      />
      <NumberField
        label="Humidity"
        unit="%"
        step={1}
        value={met.humidity_pct}
        testid="scenario-humidity"
        onChange={(v) => onChange({ humidity_pct: v })}
      />
      <NumberField
        label="Pressure"
        unit="kPa"
        step={0.1}
        value={met.pressure_kpa}
        testid="scenario-pressure"
        onChange={(v) => onChange({ pressure_kpa: v })}
      />
      <label className="field-row">
        <span className="field-label">Wind (Beaufort)</span>
        <select
          className="input dense"
          data-testid="scenario-beaufort"
          value={met.beaufort}
          onChange={(e) => onChange({ beaufort: Number(e.target.value) })}
        >
          {BEAUFORT_MS.map((ms, b) => (
            <option key={b} value={b}>
              {`${b} (${ms} m/s)`}
            </option>
          ))}
        </select>
      </label>
      <NumberField
        label="Wind from"
        unit="°"
        step={5}
        value={met.windFromDeg}
        testid="scenario-wind-dir"
        onChange={(v) => onChange({ windFromDeg: v })}
      />
      <NumberField
        label="Temp gradient"
        unit="°C/m"
        step={0.005}
        value={met.tempGradientCPerM}
        testid="scenario-gradient"
        onChange={(v) => onChange({ tempGradientCPerM: v })}
      />
      <label className="conditioning-mute">
        <input
          type="checkbox"
          data-testid="scenario-worst-case"
          checked={met.downwindWorstCase}
          onChange={(e) => onChange({ downwindWorstCase: e.target.checked })}
        />
        <span>Downwind worst-case (favourable per bearing)</span>
      </label>
    </div>
  );
}

// The advanced raw per-azimuth A/B/C disclosure (D-14 expert bypass).
function AdvancedInputs({
  met,
  onChange,
}: {
  readonly met: MetOverrides;
  readonly onChange: (patch: Partial<MetOverrides>) => void;
}): ReactElement {
  const raw = met.raw ?? { a: 0, b: 0, c: 340.348, z0: met.z0 };
  const set = (patch: Partial<typeof raw>): void => onChange({ raw: { ...raw, ...patch } });
  return (
    <div className="scenario-advanced" data-testid="scenario-advanced">
      <p className="issue-text">Raw sound-speed profile applied to every path azimuth.</p>
      <NumberField
        label="A (log)"
        unit="m/s"
        step={0.1}
        value={raw.a}
        testid="scenario-raw-a"
        onChange={(v) => set({ a: v })}
      />
      <NumberField
        label="B (linear)"
        unit="1/s"
        step={0.005}
        value={raw.b}
        testid="scenario-raw-b"
        onChange={(v) => set({ b: v })}
      />
      <NumberField
        label="C (ground)"
        unit="m/s"
        step={0.1}
        value={raw.c}
        testid="scenario-raw-c"
        onChange={(v) => set({ c: v })}
      />
      <NumberField
        label="z₀"
        unit="m"
        step={0.001}
        value={raw.z0}
        testid="scenario-raw-z0"
        onChange={(v) => set({ z0: v })}
      />
    </div>
  );
}

function NumberField({
  label,
  unit,
  step,
  value,
  testid,
  onChange,
}: {
  readonly label: string;
  readonly unit: string;
  readonly step: number;
  readonly value: number;
  readonly testid: string;
  readonly onChange: (v: number) => void;
}): ReactElement {
  return (
    <label className="field-row">
      <span className="field-label">{label}</span>
      <input
        type="number"
        className="field-input input dense mono"
        step={step}
        value={value}
        data-testid={testid}
        aria-label={label}
        onChange={(e) => onChange(Number(e.target.value))}
      />
      <span className="field-unit">{unit}</span>
    </label>
  );
}

// The destructive delete-scenario confirm (UI-SPEC §Destructive Action copy). A
// scenario delete only removes its cached result, so the confirm is a single explicit
// button (not the typed-name gate the irreversible project delete uses), reusing the
// same scrim/danger-button idiom.
function DeleteScenarioDialog({
  name,
  scenarioId,
  onClose,
}: {
  readonly name: string;
  readonly scenarioId: string;
  readonly onClose: () => void;
}): ReactElement {
  const deleteScenario = useScenarioStore((s) => s.deleteScenario);
  return (
    <div
      className="overlay-scrim delete-scrim"
      data-testid="scenario-delete-dialog"
      role="dialog"
      aria-modal="true"
      aria-label="Delete scenario"
    >
      <div className="panel delete-panel">
        <div className="delete-head">
          <h2 className="section-title">Delete scenario &ldquo;{name}&rdquo;?</h2>
        </div>
        <p className="delete-body">
          This removes its cached result. The base scenario and other scenarios are
          unaffected.
        </p>
        <div className="btn-row delete-actions">
          <button
            type="button"
            className="btn"
            data-testid="scenario-delete-cancel"
            onClick={onClose}
          >
            Cancel
          </button>
          <button
            type="button"
            className="btn danger"
            data-testid="scenario-delete-confirm"
            onClick={() => {
              deleteScenario(scenarioId);
              onClose();
            }}
          >
            Delete scenario
          </button>
        </div>
      </div>
    </div>
  );
}
