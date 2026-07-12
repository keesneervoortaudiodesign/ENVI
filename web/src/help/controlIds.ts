// controlIds.ts — the SINGLE enumeration of every interactive control that carries an
// app-wide Info-Button (D-23/D-25). This array is the coverage source of truth: the
// `ControlId` type is derived from it, `catalog.ts` is a `Record<ControlId, HelpEntry>`
// (so a missing help entry fails `tsc`), and `coverage.test.ts` walks this array to prove
// every control has extensive, standards-cited help (a control without help FAILS).
//
// # Module I/O
// - Input  none — a hand-authored constant, grouped by the panel that renders it.
// - Output `CONTROL_IDS` (the frozen runtime enumeration) and the `ControlId` string-literal
//   union derived from it. A control cannot render an `<InfoButton controlId=…/>` for an id
//   not in this union, and a new control cannot ship without being enumerated here (which in
//   turn forces a `catalog.ts` entry — the D-25 guarantee).
//
// Ids are dot-namespaced by panel so the set stays legible and collision-free. Repeated /
// list controls (palette tools are per-kind; conditioning/scenario/facade/break rows are
// per-instance) get ONE id for the control TYPE — the InfoButton is rendered once at the
// group or panel-header level per the UI-SPEC placement contract, not per dynamic instance.

export const CONTROL_IDS = [
  // --- Object palette (Phase 7 — WEB-01): the pointer tool + the 9 locked scene kinds ---
  "palette.select",
  "palette.source",
  "palette.receiver",
  "palette.wall",
  "palette.building",
  "palette.forest",
  "palette.ground_zone",
  "palette.elevation_point",
  "palette.elevation_line",
  "palette.calc_area",

  // --- Project bar (Phase 7) ---
  "project.open",
  "project.new",
  "project.save",
  "project.menu",

  // --- Property inspector + per-kind fields (Phase 7/8 — WEB-02/04/08/09) ---
  "inspector.panel",
  "wall.semi_transparent",
  "wall.isolation_spectrum",
  "elevation_point.z",
  "source.position",
  "source.sound_power",
  "source.spl_reference",
  "source.derive_lw",
  "forest.density",
  "forest.stem_radius",
  "forest.height",
  "ground_zone.impedance_class",
  "ground_zone.roughness_class",
  "facade.default",
  "facade.edge",

  // --- GIS import panel (Phase 8) ---
  "import.run",
  "import.debug_overlay",
  "import.layer_terrain",
  "import.layer_landcover",
  "import.layer_buildings",

  // --- Weather panel (Phase 9 — METX-01) ---
  "weather.date",
  "weather.hour",
  "weather.z0",
  "weather.import",
  "weather.debug_grid",
  "weather.debug_impedance",
  "weather.debug_screens",
  "weather.compute_debug",

  // --- Calculate panel (Phase 10 — WEB-07) ---
  "calc.spacing",
  "calc.run",
  "calc.abort",

  // --- Receiver spectrum panel (Phase 11 — WEB-11) ---
  "spectrum.receiver",
  "spectrum.display_mode",
  "spectrum.weighting",
  "spectrum.split",
  "spectrum.table",

  // --- Colour-scale editor (Phase 11 — WEB-06 / D-04, NoizCalc §4.6.5) ---
  "colorscale.preset",
  "colorscale.breaks",
  "colorscale.smallest",
  "colorscale.magnitude",
  "colorscale.count",
  "colorscale.apply",
  "colorscale.ascending",
  "colorscale.keep_sequence",

  // --- Conditioning panel (Phase 11 — WEB-05 / SVC-06) ---
  "conditioning.gain",
  "conditioning.delay",
  "conditioning.filter",
  "conditioning.mute",

  // --- Weather scenario manager (Phase 11 — WEB-12 / METX-03/04) ---
  "scenario.new",
  "scenario.list",
  "scenario.name",
  "scenario.save",
  "scenario.mode",
  "scenario.temperature",
  "scenario.humidity",
  "scenario.pressure",
  "scenario.beaufort",
  "scenario.wind_direction",
  "scenario.temp_gradient",
  "scenario.worst_case",
  "scenario.raw_a",
  "scenario.raw_b",
  "scenario.raw_c",
  "scenario.raw_z0",
  "scenario.compare_a",
  "scenario.compare_b",
  "scenario.compare_run",
  "scenario.compare_clear",

  // --- Export menu (Phase 11 — GRID-05 / D-20/21/22) ---
  "export.open",
  "export.geotiff",
  "export.geojson",
  "export.csv",
] as const;

// The string-literal union derived from the enumeration — the compile-time coverage key.
export type ControlId = (typeof CONTROL_IDS)[number];
