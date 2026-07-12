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

import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

const SCANNED_DIRS = ["src/store", "src/panels", "src/spectrum"];
// The dB-derivation operators. `Math.log` (natural log) is not used for dB and is not a
// reliable signal, so we target the base-10/base-2/pow/exp forms that a level computation
// would actually need.
const FORBIDDEN = /Math\.(log10|log2|pow|exp)\b/;

function tsFilesUnder(dir: string): string[] {
  let entries: string[];
  try {
    entries = readdirSync(dir);
  } catch {
    return [];
  }
  const out: string[] = [];
  for (const name of entries) {
    const full = join(dir, name);
    if (statSync(full).isDirectory()) {
      out.push(...tsFilesUnder(full));
    } else if (/\.tsx?$/.test(name) && !/\.test\.tsx?$/.test(name)) {
      out.push(full);
    }
  }
  return out;
}

describe("D-01: no acoustic math in the TS results layer", () => {
  it("has no dB-derivation Math calls (log10/log2/pow/exp) under the results surfaces", () => {
    const offenders: string[] = [];
    for (const dir of SCANNED_DIRS) {
      for (const file of tsFilesUnder(dir)) {
        const src = readFileSync(file, "utf-8");
        src.split("\n").forEach((line, i) => {
          if (FORBIDDEN.test(line)) {
            offenders.push(`${file}:${i + 1}  ${line.trim()}`);
          }
        });
      }
    }
    expect(
      offenders,
      `D-01 violation: acoustic dB-derivation math found in the TS layer.\n` +
        `All levels/weightings must come from the WASM readout boundary.\n` +
        offenders.join("\n"),
    ).toEqual([]);
  });
});
