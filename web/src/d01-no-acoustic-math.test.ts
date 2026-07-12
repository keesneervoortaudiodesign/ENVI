// d01-no-acoustic-math.test.ts — enforces project invariant D-01: the JS/TS layer
// performs ZERO acoustic arithmetic; every dB / level / weighting value is produced by
// the Rust→WASM boundary and only rendered by TS. This converts the per-plan review
// "grep gate" (previously a manual discipline, flagged in 11-SECURITY.md) into an
// automated regression test so the invariant cannot silently regress.
//
// Scope: the results/acoustic surfaces (stores, results panels, spectrum). We forbid the
// derivation primitives log10/log2/pow/exp — the operators used to turn linear pressure
// energy into decibels. Geometry/layout math (sqrt, round, min, max) and the σ flow-
// resistivity colour scale in web/src/map/weatherOverlay.ts (a display transform, not an
// acoustic readout) are intentionally out of scope.
//
// Source files are read via Vite's raw glob (`?raw`, eager) rather than `node:fs`, so the
// test typechecks under the app tsconfig (`types: []`, no @types/node) and runs in the
// Vitest (Vite) transform unchanged.

import { describe, expect, it } from "vitest";

// Eagerly inline every results-surface source file as a raw string. Vite resolves these
// at transform time; the map is keyed by the file path relative to this module.
const SOURCES = import.meta.glob("./{store,panels,spectrum}/**/*.{ts,tsx}", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

// The dB-derivation operators. `Math.log` (natural log) is not used for dB and is not a
// reliable signal, so we target the base-10/base-2/pow/exp forms a level computation needs.
const FORBIDDEN = /Math\.(log10|log2|pow|exp)\b/;

describe("D-01: no acoustic math in the TS results layer", () => {
  it("has no dB-derivation Math calls (log10/log2/pow/exp) under the results surfaces", () => {
    const offenders: string[] = [];
    for (const [path, src] of Object.entries(SOURCES)) {
      if (/\.test\.tsx?$/.test(path)) {
        continue;
      }
      src.split("\n").forEach((line, i) => {
        if (FORBIDDEN.test(line)) {
          offenders.push(`${path}:${i + 1}  ${line.trim()}`);
        }
      });
    }
    expect(
      offenders,
      `D-01 violation: acoustic dB-derivation math found in the TS layer.\n` +
        `All levels/weightings must come from the WASM readout boundary.\n` +
        offenders.join("\n"),
    ).toEqual([]);
  });
});
