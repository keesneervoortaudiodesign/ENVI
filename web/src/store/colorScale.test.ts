// colorScale.test.ts — the WEB-06/D-04 colour-scale contract. Asserts the single
// breaks[]/colors[] source of truth: the EU-END default edges + hex (A4), the
// viridis/turbo sampling at the class count, V5 validation (reject
// non-monotonic/<2/non-finite), and that a preset switch / break edit keep
// `colors.length === breaks.length + 1` (legend ≡ contour ≡ class colours). Pure
// Node — no wasm, no map.

import { beforeEach, describe, expect, it } from "vitest";

import {
  END_BREAKS,
  END_COLORS,
  colorsForPreset,
  contourBreaks,
  generateBreaks,
  samplePalette,
  useColorScaleStore,
  validateBreaks,
} from "./colorScale";

const HEX = /^#[0-9a-f]{6}$/;

beforeEach(() => {
  useColorScaleStore.getState().reset();
});

describe("EU-END default (A4)", () => {
  it("ships the canonical [55,60,65,70,75] edges + the 6 green→violet hex", () => {
    const s = useColorScaleStore.getState();
    expect(s.preset).toBe("end");
    expect(s.breaks).toEqual([55, 60, 65, 70, 75]);
    expect(s.colors).toEqual([
      "#7fbf7f",
      "#bfdc6b",
      "#f6e04b",
      "#f4a637",
      "#e34948",
      "#8a2be2",
    ]);
    // 5 editable edges → 6 classes (below-lowest + 4 intervals + above-highest).
    expect(s.colors.length).toBe(s.breaks.length + 1);
    expect(END_BREAKS).toHaveLength(5);
    expect(END_COLORS).toHaveLength(6);
  });

  it("cap-extends the editable edges into N+1 closed bands for the tracer", () => {
    // < 55, 55–60, 60–65, 65–70, 70–75, ≥ 75 → 7 finite edges, 6 bands = 6 colours.
    const cb = contourBreaks([55, 60, 65, 70, 75]);
    expect(cb).toHaveLength(7);
    expect(cb[0]).toBeLessThan(55);
    expect(cb[cb.length - 1]).toBeGreaterThan(75);
    expect(cb.slice(1, 6)).toEqual([55, 60, 65, 70, 75]);
    expect(cb.length - 1).toBe(END_COLORS.length);
  });
});

describe("perceptually-uniform presets (D-04)", () => {
  it("samples viridis + turbo at the class count, all valid distinct hex", () => {
    for (const preset of ["viridis", "turbo"] as const) {
      const colors = colorsForPreset(preset, 6);
      expect(colors).toHaveLength(6);
      for (const c of colors) {
        expect(c).toMatch(HEX);
      }
      // A monotonic ramp — the endpoints differ (not a constant fill).
      expect(colors[0]).not.toEqual(colors[colors.length - 1]);
    }
  });

  it("switching preset keeps the single-source invariant colors.length === breaks.length + 1", () => {
    const st = useColorScaleStore.getState();
    st.setPreset("viridis");
    let s = useColorScaleStore.getState();
    expect(s.preset).toBe("viridis");
    expect(s.colors.length).toBe(s.breaks.length + 1);
    expect(s.colors[0]).toMatch(HEX);

    st.setPreset("turbo");
    s = useColorScaleStore.getState();
    expect(s.colors.length).toBe(s.breaks.length + 1);
  });

  it("samplePalette spans the ramp end-to-end and handles n=1", () => {
    const one = samplePalette(["#000000", "#ffffff"], 1);
    expect(one).toEqual(["#000000"]);
    const three = samplePalette(["#000000", "#ffffff"], 3);
    expect(three[0]).toBe("#000000");
    expect(three[2]).toBe("#ffffff");
    expect(three[1]).toBe("#808080"); // exact midpoint of the RGB lerp
  });
});

describe("validateBreaks (V5)", () => {
  it("accepts a strictly-increasing finite scale of ≥2", () => {
    expect(validateBreaks([55, 60, 65])).toBeNull();
    expect(validateBreaks([0, 1])).toBeNull();
  });

  it("rejects fewer than 2 breaks", () => {
    expect(validateBreaks([55])).toMatch(/at least 2/);
    expect(validateBreaks([])).toMatch(/at least 2/);
  });

  it("rejects a non-monotonic sequence", () => {
    expect(validateBreaks([55, 55, 60])).toMatch(/strictly increase/);
    expect(validateBreaks([60, 55])).toMatch(/strictly increase/);
  });

  it("rejects a non-finite value", () => {
    expect(validateBreaks([55, Number.NaN, 65])).toMatch(/finite/);
    expect(validateBreaks([55, Number.POSITIVE_INFINITY])).toMatch(/finite/);
  });
});

describe("store edits keep the single source of truth", () => {
  it("setBreaks applies a valid scale and re-derives colours to N+1", () => {
    const st = useColorScaleStore.getState();
    st.setPreset("viridis");
    st.setBreaks([50, 55, 60, 65]);
    const s = useColorScaleStore.getState();
    expect(s.breaks).toEqual([50, 55, 60, 65]);
    expect(s.error).toBeNull();
    expect(s.colors.length).toBe(5);
  });

  it("setBreaks on an invalid scale sets the inline error and keeps the last valid scale", () => {
    const st = useColorScaleStore.getState();
    const before = useColorScaleStore.getState().breaks;
    st.setBreaks([60, 55]); // non-monotonic
    const s = useColorScaleStore.getState();
    expect(s.error).toMatch(/strictly increase/);
    expect(s.breaks).toEqual(before); // unchanged — map never renders a broken contour
  });

  it("applyGenerator builds uniform edges from the NoizCalc §4.6.5 controls", () => {
    const st = useColorScaleStore.getState();
    st.applyGenerator(45, 5, 6);
    const s = useColorScaleStore.getState();
    expect(s.breaks).toEqual([45, 50, 55, 60, 65, 70]);
    expect(s.colors.length).toBe(7);
    expect(generateBreaks(45, 5, 6)).toEqual([45, 50, 55, 60, 65, 70]);
  });

  it("caches the isophone contour input (grid + crs + weighting) for re-contour", () => {
    const st = useColorScaleStore.getState();
    st.setIsophoneInput(
      { rows: 2, cols: 2, origin: [0, 0], spacing_m: 10, values: [50, 60, 70, 80] },
      { utm_zone: 31, south: false },
      "dB(C)",
    );
    const s = useColorScaleStore.getState();
    expect(s.grid?.cols).toBe(2);
    expect(s.crs?.utm_zone).toBe(31);
    expect(s.weightingLabel).toBe("dB(C)");
  });
});
