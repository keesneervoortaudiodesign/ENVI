// coverage.test.ts — the D-25 acceptance backbone: a control without help FAILS the build.
//
// The `catalog` is a `Record<ControlId, HelpEntry>`, so a MISSING id is already a `tsc` error. This
// test adds the runtime guarantees the type cannot express: that every enumerated control has a
// SUBSTANTIVE, multi-paragraph, standards-cited help entry (D-24), that the catalog and the
// `CONTROL_IDS` enumeration are in exact bijection (no orphan entries, no duplicate ids), and a
// structural guard against pasting copyrighted AV 1106/07 text (T-11-11-02). The id set is derived
// from the `ControlId` enumeration itself, so a NEW control cannot ship without being enumerated
// here — which in turn forces a catalog entry.

import { describe, expect, it } from "vitest";

import { CONTROL_IDS } from "./controlIds";
import { catalog, type Citation } from "./catalog";

const VALID_SOURCES: ReadonlySet<Citation["source"]> = new Set(["AV1106/07", "TI386", "ENVI"]);

describe("help catalog coverage (D-25)", () => {
  it("enumerates a non-trivial control set with no duplicate ids", () => {
    expect(CONTROL_IDS.length).toBeGreaterThanOrEqual(50);
    const unique = new Set(CONTROL_IDS);
    expect(unique.size).toBe(CONTROL_IDS.length);
  });

  it("has EXACTLY one catalog entry per control id (bijection — no gaps, no orphans)", () => {
    const catalogKeys = Object.keys(catalog).sort();
    const idKeys = [...CONTROL_IDS].sort();
    expect(catalogKeys).toEqual(idKeys);
  });

  // The core guarantee: EVERY control has extensive, cited help. A control without help fails here.
  it.each(CONTROL_IDS)("control '%s' has extensive, standards-cited help", (id) => {
    const entry = catalog[id];
    expect(entry, `missing help entry for ${id}`).toBeTruthy();

    // Title: a real, trimmed string.
    expect(typeof entry.title).toBe("string");
    expect(entry.title.trim().length).toBeGreaterThan(0);

    // Body: multi-paragraph (D-24 "extensive"), every paragraph substantive and non-empty.
    expect(Array.isArray(entry.body)).toBe(true);
    expect(entry.body.length, `${id} body must be multi-paragraph`).toBeGreaterThanOrEqual(2);
    for (const para of entry.body) {
      expect(typeof para).toBe("string");
      expect(para.trim().length, `${id} has a too-short paragraph`).toBeGreaterThanOrEqual(40);
      // Structural anti-paste guard (T-11-11-02): our own-words prose is never a long verbatim block.
      expect(para.length, `${id} paragraph suspiciously long — did AV 1106/07 text get pasted?`).toBeLessThan(1200);
    }
    // Extensive overall.
    const totalChars = entry.body.reduce((n, p) => n + p.length, 0);
    expect(totalChars, `${id} help is not extensive enough`).toBeGreaterThanOrEqual(160);

    // Citations: at least one, each with a valid source + a non-empty report ref + a topic note.
    expect(entry.citations.length, `${id} must cite at least one source`).toBeGreaterThanOrEqual(1);
    for (const c of entry.citations) {
      expect(VALID_SOURCES.has(c.source), `${id} has an invalid citation source '${c.source}'`).toBe(true);
      expect(c.ref.trim().length).toBeGreaterThan(0);
      expect(c.note.trim().length).toBeGreaterThan(0);
    }
  });

  it("cites the acoustic standards by report number somewhere (Nord2000 AV 1106/07 + NoizCalc TI 386, D-24)", () => {
    const sources = new Set<Citation["source"]>();
    for (const id of CONTROL_IDS) {
      for (const c of catalog[id].citations) {
        sources.add(c.source);
      }
    }
    expect(sources.has("AV1106/07")).toBe(true);
    expect(sources.has("TI386")).toBe(true);
    // The AV citations must carry the report number (never just "Nord2000").
    const avRefs = CONTROL_IDS.flatMap((id) => catalog[id].citations)
      .filter((c) => c.source === "AV1106/07")
      .map((c) => c.ref);
    expect(avRefs.length).toBeGreaterThan(0);
    for (const ref of avRefs) {
      expect(ref, "AV citation must cite the report number 1106/07").toContain("1106/07");
    }
  });
});
