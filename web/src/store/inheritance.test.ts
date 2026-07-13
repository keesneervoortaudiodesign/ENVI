// inheritance.test.ts — the WEB-04 last-object inheritance contract, focused on the two HEIGHT-bearing
// kinds and the D-11 provenance keys.
//
// # Why this file exists (no-false-green house rule)
// Two silent-corruption modes live here. (1) A height that does not inherit forces the user to retype it on
// every wall of a run — and a wall that reaches the server WITHOUT `height_m` is dropped from the screening
// set, so the barrier silently stops screening. (2) Provenance keys that DO inherit would fabricate a
// pedigree: a hand-drawn building seeded from an edited imported one would claim `imported: true` and an OSM
// `height_provenance`, which is exactly the lie the DATA-03 provenance chip exists to prevent. Run by
// Vitest: `npm run test:unit`.

import { beforeEach, describe, expect, it } from "vitest";

import { EAVES_HEIGHT_KEY, WALL_HEIGHT_KEY } from "./heights";
import { KIND_DEFAULTS, lastOf, recordLast, resetInheritance, seedProps } from "./inheritance";

beforeEach(() => {
  resetInheritance();
});

describe("KIND_DEFAULTS — the height-bearing kinds start with a usable height", () => {
  it("gives a first-of-kind wall a finite screen height (the server scene contract requires `height_m`)", () => {
    const h = KIND_DEFAULTS.wall[WALL_HEIGHT_KEY];
    expect(typeof h).toBe("number");
    expect(h as number).toBeGreaterThan(0);
  });

  it("gives a first-of-kind building a finite eaves height (`eaves_height_m`)", () => {
    const h = KIND_DEFAULTS.building[EAVES_HEIGHT_KEY];
    expect(typeof h).toBe("number");
    expect(h as number).toBeGreaterThan(0);
  });

  it("seeds a first-of-kind object from the defaults, with NO inherited fields (a `default` chip)", () => {
    const wall = seedProps("wall");
    expect(wall.props[WALL_HEIGHT_KEY]).toBe(KIND_DEFAULTS.wall[WALL_HEIGHT_KEY]);
    expect(wall.inheritedFields).toEqual([]);
    const building = seedProps("building");
    expect(building.props[EAVES_HEIGHT_KEY]).toBe(KIND_DEFAULTS.building[EAVES_HEIGHT_KEY]);
    expect(building.inheritedFields).toEqual([]);
  });
});

describe("height inherits from the last object of the kind (WEB-04)", () => {
  it("seeds the NEXT wall with the previous wall's height, marked inherited", () => {
    recordLast("wall", { semi_transparent: false, [WALL_HEIGHT_KEY]: 4.5 });
    const next = seedProps("wall");
    expect(next.props[WALL_HEIGHT_KEY]).toBe(4.5);
    expect(next.inheritedFields).toContain(WALL_HEIGHT_KEY);
  });

  it("seeds the NEXT building with the previous building's eaves height, marked inherited", () => {
    recordLast("building", { [EAVES_HEIGHT_KEY]: 24 });
    const next = seedProps("building");
    expect(next.props[EAVES_HEIGHT_KEY]).toBe(24);
    expect(next.inheritedFields).toContain(EAVES_HEIGHT_KEY);
  });
});

describe("D-11 provenance is per-feature pedigree — it NEVER inherits", () => {
  it("strips every provenance key when an imported feature's edit becomes the inheritance source", () => {
    // The property bag of an IMPORTED building the user just edited in the inspector.
    recordLast("building", {
      kind: "building",
      id: "way/123",
      edge_ids: ["e1", "e2"],
      mode: "polygon",
      [EAVES_HEIGHT_KEY]: 17.2,
      height_provenance: "height_tag",
      imported: true,
      user_modified: true,
      source: "osm_buildings",
      source_ref: "way/123",
      license: "ODbL",
      retrieved_at: "2026-07-13T00:00:00Z",
      vertical_datum: "NAP",
    });

    const recorded = lastOf("building");
    expect(recorded).toEqual({ [EAVES_HEIGHT_KEY]: 17.2 });

    const next = seedProps("building");
    // The height (a real editable property) carries over…
    expect(next.props[EAVES_HEIGHT_KEY]).toBe(17.2);
    // …but nothing that would fabricate a pedigree for a hand-drawn building.
    for (const key of [
      "height_provenance",
      "imported",
      "user_modified",
      "source",
      "source_ref",
      "license",
      "retrieved_at",
      "vertical_datum",
      "kind",
      "id",
      "edge_ids",
      "mode",
    ]) {
      expect(next.props, `${key} must never inherit`).not.toHaveProperty(key);
      expect(next.inheritedFields, `${key} must never show an inherited chip`).not.toContain(key);
    }
  });
});
