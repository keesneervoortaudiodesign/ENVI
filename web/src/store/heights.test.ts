// heights.test.ts — the height contract: what the inspector will COMMIT, what it REFUSES, and what it says
// about where a height came from (DATA-03).
//
// # Why this file exists (no-false-green house rule)
// The reported "imported buildings are all the same height" bug was never a data bug — the scene store held
// 244 distinct eaves heights — it was that NOTHING showed the height, so the fallback tiers were invisible.
// These tests pin the two rules that keep it fixed: a feature's OWN height is what is read (a kind-default
// substitution is a lie), and the provenance tier is reported honestly (a guessed `default` reads as a WARN,
// never as a measured value). Run by Vitest: `npm run test:unit`.

import { describe, expect, it } from "vitest";

import {
  EAVES_HEIGHT_KEY,
  HEIGHT_MAX_M,
  USER_HEIGHT_PROVENANCE,
  WALL_HEIGHT_KEY,
  heightOf,
  heightProvenanceOf,
  heightRejectionReason,
  isValidHeightM,
  parseHeightM,
} from "./heights";

describe("heightOf — the feature's OWN height, never a substituted default", () => {
  it("reads each feature's own stored height (distinct features ⇒ distinct heights)", () => {
    const a = { [EAVES_HEIGHT_KEY]: 7.5 };
    const b = { [EAVES_HEIGHT_KEY]: 21.3 };
    expect(heightOf(a, EAVES_HEIGHT_KEY)).toBe(7.5);
    expect(heightOf(b, EAVES_HEIGHT_KEY)).toBe(21.3);
  });

  it("returns null (not a default) when the feature carries no usable height", () => {
    expect(heightOf({}, EAVES_HEIGHT_KEY)).toBeNull();
    expect(heightOf({ [EAVES_HEIGHT_KEY]: "12" }, EAVES_HEIGHT_KEY)).toBeNull(); // a string is not a height
    expect(heightOf({ [EAVES_HEIGHT_KEY]: Number.NaN }, EAVES_HEIGHT_KEY)).toBeNull();
    expect(heightOf({ [EAVES_HEIGHT_KEY]: -3 }, EAVES_HEIGHT_KEY)).toBeNull();
  });

  it("reads the wall's own key (`height_m`), which is what the screening marshaller requires", () => {
    expect(heightOf({ [WALL_HEIGHT_KEY]: 3 }, WALL_HEIGHT_KEY)).toBe(3);
    expect(heightOf({}, WALL_HEIGHT_KEY)).toBeNull();
  });
});

describe("parseHeightM / isValidHeightM — a bad entry is REFUSED, never silently stored", () => {
  it("accepts finite metres in range (including 0 and the cap)", () => {
    expect(parseHeightM("12.4")).toBe(12.4);
    expect(parseHeightM(" 0 ")).toBe(0);
    expect(parseHeightM(String(HEIGHT_MAX_M))).toBe(HEIGHT_MAX_M);
    expect(isValidHeightM(9.81)).toBe(true);
  });

  it("refuses empty, non-numeric, non-finite, negative, and absurd entries", () => {
    for (const raw of ["", "   ", "abc", "1e999", "-0.1", "-12", String(HEIGHT_MAX_M + 1), "99999"]) {
      expect(parseHeightM(raw), `"${raw}" must not commit`).toBeNull();
      expect(heightRejectionReason(raw), `"${raw}" must explain itself`).toBeTruthy();
    }
    expect(isValidHeightM(Number.POSITIVE_INFINITY)).toBe(false);
    expect(isValidHeightM(Number.NaN)).toBe(false);
    expect(isValidHeightM("12")).toBe(false);
  });

  it("gives no rejection reason for a valid entry", () => {
    expect(heightRejectionReason("18")).toBeNull();
  });
});

describe("heightProvenanceOf — DATA-03: WHERE the height came from", () => {
  it("maps each envi-gis import tier to its own honest label", () => {
    expect(heightProvenanceOf({ height_provenance: "height_tag" }).key).toBe("height_tag");
    expect(heightProvenanceOf({ height_provenance: "levels" }).key).toBe("levels");
    expect(heightProvenanceOf({ height_provenance: "measured" }).key).toBe("measured");
    const guessed = heightProvenanceOf({ height_provenance: "default" });
    expect(guessed.key).toBe("default");
    // A GUESSED height is the tier that makes buildings look identical — it must read as a warning.
    expect(guessed.tone).toBe("warn");
    expect(guessed.label).toMatch(/fallback|default/i);
  });

  it("reports a user edit as user-edited, and an ENVI-drawn feature as authored (never a fake tier)", () => {
    expect(heightProvenanceOf({ height_provenance: USER_HEIGHT_PROVENANCE }).key).toBe(USER_HEIGHT_PROVENANCE);
    expect(heightProvenanceOf({}).key).toBe("authored");
    expect(heightProvenanceOf({ height_provenance: "wat" }).key).toBe("authored");
  });
});
