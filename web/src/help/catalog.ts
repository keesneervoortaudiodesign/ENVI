// catalog.ts — the STRUCTURED help catalog (D-24/D-25): a typed `Record<ControlId, HelpEntry>`
// keyed by the `controlIds.ts` enumeration. Help content lives HERE as data, never scattered as
// text in JSX — so it is maintainable, reviewable, and drift-checkable, and the `Record` type
// makes a missing entry a `tsc` error (D-25).
//
// # Module I/O
// - Input  none — hand-authored prose.
// - Output `catalog`, a total map from every `ControlId` to a `HelpEntry` (title + multi-paragraph
//   English body + standards citations), consumed by `<InfoButton>` (glance popover + docked panel)
//   and validated by `coverage.test.ts`.
//
// # Content rules (D-24) — enforced by review + the coverage test
// - English-only, sentence case, units always shown (dB, dB(A), dB(C), Hz, m, ms, m/s).
// - Band identity is a BAND INDEX on the 105-point 1/12-octave grid + the exact Hz from the server
//   frequency axis — never a nominal-Hz label used as the identity (a recurring engine pitfall).
// - Every acoustic rationale is written in OUR OWN WORDS and cites the source by report number:
//   the Nord2000 comprehensive model report AV 1106/07 (DELTA Acoustics) and/or the NoizCalc
//   Technical Information TI 386 (d&b audiotechnik). ⚠ AV 1106/07 is COPYRIGHTED — its text is
//   NEVER pasted here (threat T-11-11-02); we paraphrase and cite by number only.

import type { ControlId } from "./controlIds";

// A standards citation. `source` is the document family; `ref` is the human-readable report
// identifier shown in the help UI; `note` is the specific topic the control draws on.
export interface Citation {
  readonly source: "AV1106/07" | "TI386" | "ENVI";
  readonly ref: string;
  readonly note: string;
}

// One control's help: a short title, a multi-paragraph English body (glance = first paragraph;
// depth = the full body in the docked panel), and at least one standards citation.
export interface HelpEntry {
  readonly title: string;
  readonly body: readonly string[];
  readonly citations: readonly Citation[];
}

// Shorthand citation builders (report numbers are load-bearing — cite, never paste).
const av = (note: string): Citation => ({ source: "AV1106/07", ref: "AV 1106/07 rev. 4 (Nord2000)", note });
const ti = (note: string): Citation => ({ source: "TI386", ref: "TI 386 (1.6 EN), NoizCalc", note });
const envi = (note: string): Citation => ({ source: "ENVI", ref: "ENVI engine / project decision", note });

export const catalog: Record<ControlId, HelpEntry> = {
  // ============================ Object palette (Phase 7) ============================
  "palette.select": {
    title: "Select / pan tool",
    body: [
      "Switches the map into selection mode: click an object to select it and edit its properties in the inspector, or drag empty map to pan. This is the default, non-drawing tool — no new geometry is created while it is active.",
      "Use it to leave a drawing tool without placing a shape, and to pick existing sources, receivers, screens, buildings, forests, ground zones, terrain features, or the calculation area for editing.",
    ],
    citations: [envi("Map-authoring shell; tool palette is an ENVI/NoizCalc-style authoring convention")],
  },
  "palette.source": {
    title: "Sound source",
    body: [
      "Places a sound source. Each source carries a sub-source position [x, y, z] in metres (z is height above the local ground) and a sound-power spectrum L_W across the 105-point 1/12-octave band grid. The engine radiates each source with spherical spreading and the full Nord2000 propagation chain to every receiver.",
      "Sound power is authored per band (or calibrated from an SPL-at-reference measurement, which the server back-calculates to L_W). Multiple co-located sub-sources sum coherently in the engine's coherent channel — two identical sources add +3.01 dB (energy), not +6 dB, unless they are genuinely phase-locked.",
      "Nord2000 models outdoor propagation from point sources; road and area sources are decomposed into point sub-sources upstream. The complex transfer per sub-source is what makes fast recalculation and coherent loudspeaker-array summation possible.",
    ],
    citations: [
      av("Point-source radiation and the coherent transfer H[sub-source, receiver, band]"),
      ti("Sources as loudspeaker/stage setups radiated to grid points"),
    ],
  },
  "palette.receiver": {
    title: "Receiver (immission point)",
    body: [
      "Places a receiver — a point where the sound level is read out. Receivers have no acoustic properties of their own; they are immission points whose per-band level, dB(A) and dB(C) totals, and coherent/incoherent split are read from the cached transfer tensor.",
      "Discrete receivers are solved exactly (not interpolated from the grid). A typical receiver height is 4 m for façade assessment or 1.5 m for a standing listener; set the acoustic height when the receiver is assembled into a solve.",
    ],
    citations: [av("Immission-point readout of the per-band sound-pressure level")],
  },
  "palette.wall": {
    title: "Wall / screen",
    body: [
      "Draws a wall. A solid wall is an opaque barrier; marking it semi-transparent turns it into an acoustic screen that diffracts sound over its top edge and needs an isolation spectrum for the transmitted path.",
      "Nord2000 treats a screen with its diffraction sub-model, caping the path at two screen edges; the screen top is injected into the terrain profile as a vertex so the diffraction and ground effects stay consistent.",
    ],
    citations: [av("Screen diffraction sub-model; barrier top as a profile vertex")],
  },
  "palette.building": {
    title: "Building",
    body: [
      "Draws a building footprint. Buildings block and diffract sound like solid screens and expose one façade per footprint edge, each of which can carry its own façade isolation spectrum (or inherit the building default) for reflection/immission work.",
      "Building height comes from imported attributes (measured height, then height tags, then levels × 3 m + 1.5 m) or a user default. Per-edge façade identity is preserved through geometry edits by a stable per-edge UUID, so an override never silently re-points to a different wall.",
    ],
    citations: [
      av("Buildings as diffracting/reflecting obstacles"),
      ti("Façade assessment on building edges"),
    ],
  },
  "palette.forest": {
    title: "Forest",
    body: [
      "Draws a forest zone. Vegetation adds excess attenuation that grows with propagation distance through the stand, governed by mean tree density (trees/m²), mean stem radius (m), and mean tree height (m).",
      "ENVI implements the Nord2000 forest law (the scattering/absorption sub-model with a distance-saturating term and a floor), not a flat per-metre paraphrase. Density and stem radius set the scattering cross-section; mean height scales the effective in-canopy path length.",
    ],
    citations: [av("Forest excess-attenuation sub-model (density, stem radius, height)")],
  },
  "palette.ground_zone": {
    title: "Ground zone",
    body: [
      "Draws a ground-cover zone with an impedance class (A–H) and a roughness class (N/S/M/L). Ground type controls the strength and phase of the ground reflection, which is often the single largest term in outdoor propagation over soft ground.",
      "Zones partition the ground into non-overlapping regions; the terrain profile between a source and receiver is segmented by the zones it crosses, and each segment carries its own impedance. Class A is the softest (most absorptive, low flow resistivity) and class H the hardest (near-rigid).",
    ],
    citations: [
      av("Ground impedance model; Delany–Bazley classes A–H by flow resistivity σ"),
      envi("Class B flow resistivity σ = 31.5 kPa·s/m² (corrected against the AV table)"),
    ],
  },
  "palette.elevation_point": {
    title: "Elevation point",
    body: [
      "Places a terrain elevation spot height (z, in metres). Elevation points and lines feed the digital ground model (a triangulated surface) from which every source/receiver height and terrain profile is sampled.",
      "Import can thin dense spot-height clouds against a maximum-deviation tolerance to keep the ground model tractable; individual points placed here are kept exactly.",
    ],
    citations: [ti("Elevation spot heights feeding the digital ground model; import filtering by tolerance")],
  },
  "palette.elevation_line": {
    title: "Elevation line (breakline)",
    body: [
      "Draws a terrain breakline — a ridge, embankment, ditch, or road edge whose crest must be honoured by the ground model. Its z comes from its endpoints, and the triangulation is constrained to follow it rather than smoothing across it.",
      "Breaklines matter because a screen-like ground feature (a berm) changes both the terrain profile and the diffraction geometry; a smoothed surface would lose the crest that provides shielding.",
    ],
    citations: [ti("Breaklines in the digital ground model")],
  },
  "palette.calc_area": {
    title: "Calculation area",
    body: [
      "Draws the calculation domain — the polygon over which the grid noise map is computed. Receivers are laid out on a lattice inside this area (minus building footprints), at the fine spacing set in the Calculate panel, with coarser tiers for speed.",
      "A larger area or a finer spacing means more grid receivers, a larger transfer tensor, and a longer solve; the Calculate panel shows the receiver count, tensor size, and time estimate before you run.",
    ],
    citations: [ti("Grid noise map over a calculation area")],
  },

  // ============================ Project bar (Phase 7) ============================
  "project.open": {
    title: "Open project",
    body: [
      "Opens an existing project. A project holds the scene (all drawn objects), its coordinate reference system, and the authored acoustic data. The last-opened project is restored automatically on load.",
      "Each project pins a projected CRS (a UTM or RD zone) from its origin so that all geometry math is metric and consistent.",
    ],
    citations: [envi("Project = scene + CRS + authored acoustics; reopen-last on boot")],
  },
  "project.new": {
    title: "New project",
    body: [
      "Creates a new project from a WGS84 origin (the initial map centre). The server pins the project's projected CRS from that origin so subsequent geometry is stored and computed in metres.",
      "Start here for a fresh site; import GIS terrain, land cover, and buildings for the area afterwards from the Import panel.",
    ],
    citations: [envi("New project pins its projected CRS from the WGS84 origin")],
  },
  "project.save": {
    title: "Save",
    body: [
      "Writes the whole scene to the project immediately. Edits also autosave on a short debounce, so this is an explicit flush for peace of mind before closing.",
      "The save indicator shows Unsaved / Saving / Saved · hh:mm / Save failed so the persistence state is always visible and never implied by colour alone.",
    ],
    citations: [envi("Explicit whole-scene PUT plus debounced autosave (D-04/D-12)")],
  },
  "project.menu": {
    title: "More project actions",
    body: [
      "Opens the overflow menu for project-level actions such as deleting the project. It keeps rarely used, potentially destructive actions out of the main toolbar so they are not triggered by accident.",
      "Destructive actions there use an explicit typed-name confirmation, because deleting a project removes its scene and results permanently and cannot be undone.",
    ],
    citations: [envi("Overflow menu; irreversible actions gated by a typed-name confirm")],
  },

  // ============================ Inspector + fields (Phase 7/8) ============================
  "inspector.panel": {
    title: "Property inspector",
    body: [
      "Shows the editable properties of the selected object, dispatched by its kind: source position and sound power, forest density/radius/height, ground-zone impedance and roughness, wall screen treatment, building façades, and terrain z. Select an object on the map to edit it here.",
      "Every field writes straight to the canonical scene store; there is no separate apply step. Fields seeded by inheritance from the last object of the same kind are marked until you edit them.",
    ],
    citations: [envi("Kind-dispatched property inspector over the canonical scene store")],
  },
  "building.eaves_height": {
    title: "Eaves height (m)",
    body: [
      "The height of the building's eaves above local ground, in metres. It is the property that makes a building act as a screen: the footprint is extruded to this height and its top edge is injected into the terrain profile, so the diffracted path over the roof line — and the shadow behind it — follow directly from this number. A building without a usable eaves height carries no screening at all.",
      "Alongside the value the inspector shows its PROVENANCE — where the number came from. An imported building is tagged 'OSM height tag' when the source carried an explicit height, 'OSM building levels' when the height was derived from a storey count, and 'import fallback default' when neither was present and a generic value had to be assumed. 'Measured' marks a surveyed height, and editing the field yourself re-tags it 'user-edited'. A block of buildings that all read 'import fallback default' is telling you the source data had no heights, not that the model is broken.",
      "Editing an imported building here also flags it as user-modified, so a later re-import of the same area cannot overwrite the height you corrected. Accepted entries are finite metres between 0 and 500 m; a negative or implausibly large entry is refused rather than silently stored.",
    ],
    citations: [
      av("Screening object: footprint extruded to eaves height; roof edge as a diffracting profile vertex, AV 1106/07"),
      envi("DATA-03 height provenance (height tag / levels / fallback) stamped at GIS import; D-09 re-import merge guard"),
    ],
  },
  "wall.height": {
    title: "Wall height (m)",
    body: [
      "The height of the wall or screen above local ground, in metres. This is the only reason a wall exists acoustically: its top edge becomes the diffracting edge in the propagation path, and the screening effect at a receiver follows from how far that edge rises above the direct source-receiver line. A wall with no height diffracts nothing and is dropped from the screening set entirely.",
      "Typical values are around 2–3 m for a garden or site wall and 3–6 m for a purpose-built noise barrier; the acoustic benefit grows with how deep the receiver sits in the geometric shadow, not with height alone. Combine the height with the semi-transparent flag and an isolation spectrum when the screen also transmits sound through its body.",
      "Accepted entries are finite metres between 0 and 500 m; a negative or implausibly large entry is refused rather than silently stored. The value inherits to the next wall you draw, so a run of identical barriers only has to be typed once.",
    ],
    citations: [
      av("Screen diffraction sub-model; the screen top as a terrain-profile vertex, AV 1106/07"),
      ti("Barriers as height-bearing screening objects in the propagation scene"),
    ],
  },
  "wall.semi_transparent": {
    title: "Semi-transparent (acoustic screen)",
    body: [
      "When off, the wall is an opaque barrier. When on, it becomes an acoustic screen: sound diffracts over its top edge and a transmitted path through it is governed by an isolation spectrum. A semi-transparent screen without an isolation spectrum is flagged as an incomplete (warn) state.",
      "Nord2000 handles the shielding with its diffraction sub-model over the screen top; the transmitted contribution is attenuated by the isolation spectrum you assign below.",
    ],
    citations: [av("Screen diffraction; transmission governed by an isolation spectrum")],
  },
  "wall.isolation_spectrum": {
    title: "Isolation spectrum (screen)",
    body: [
      "Opens the spectrum editor to author the screen's sound-reduction index R across the band grid — how much the transmitted path is attenuated per band. Author at 1/1-octave or 1/3-octave anchors and promote to the full 1/12-octave grid; the server interpolates the dense curve.",
      "Higher R means a quieter transmitted path, so more of the residual level comes from diffraction over the top. R is in dB per band; band identity is the band index plus the exact centre frequency, not the nominal label.",
    ],
    citations: [
      av("Sound-reduction index applied to the transmitted screen path"),
      envi("Min-phase complex isolation kernel R(f) → T(f) (ENVI extension)"),
    ],
  },
  "elevation_point.z": {
    title: "Elevation Z",
    body: [
      "The spot height of this terrain point, in metres, in the project's vertical datum. It contributes a vertex to the digital ground model from which source/receiver heights and terrain profiles are sampled.",
      "Consistent, correct z values matter: ground effect and diffraction both depend on the source/receiver heights above the profile, which are read from this surface.",
    ],
    citations: [ti("Elevation spot height in the digital ground model")],
  },
  "source.position": {
    title: "Sub-source position [x, y, z]",
    body: [
      "The sub-source location in project metres: x and y in the projected plane, z as height above the local ground. Height above ground is acoustically important — it sets the ground-reflection geometry and the source/image path-length difference.",
      "For road and industrial sources the sub-source heights are prescribed by the source model (for example the Nord2000 road sub-source heights); for a bespoke source, place z at the true acoustic centre height.",
    ],
    citations: [av("Source height governs ground-reflection geometry and Δτ path difference")],
  },
  "source.sound_power": {
    title: "Sound-power spectrum L_W",
    body: [
      "Opens the editor to author the source sound power L_W per band, in dB re 1 pW. This is the emission that the propagation chain attenuates on the way to each receiver. Author at octave or third-octave anchors and promote to the 1/12-octave grid.",
      "Sound power (L_W), not sound pressure (L_p), is the source datum — pressure depends on distance and environment, power does not. If you only have a measured SPL at a known distance, use SPL @ reference below to back-calculate L_W.",
    ],
    citations: [
      av("Emission expressed as sound power; propagation yields the receiver pressure level"),
      ti("Source spectra drive the calculation"),
    ],
  },
  "source.spl_reference": {
    title: "SPL @ reference distance",
    body: [
      "The distance (in metres) at which a measured sound-pressure level was taken. With an authored SPL spectrum and this distance, Derive L_W back-calculates the sound power assuming free-field spherical spreading — entirely on the server, with no client-side acoustic arithmetic.",
      "Use a positive distance measured in the source's far field. The back-calculation removes the 20·log₁₀(distance) spreading and the free-field constant to recover L_W per band.",
    ],
    citations: [envi("Server-side SPL→L_W back-calculation (free-field spherical spreading, SVC-07)")],
  },
  "source.derive_lw": {
    title: "Derive L_W from SPL",
    body: [
      "Back-calculates the source sound power from the currently authored SPL spectrum and the reference distance, then stores the result as the source's L_W at 1/12-octave resolution. Author an SPL spectrum first, then derive.",
      "The derivation runs on the server (free-field spherical spreading); the frontend performs no logarithmic or per-band acoustic math itself.",
    ],
    citations: [envi("SPL→L_W derivation is a server call (D-01: zero client acoustic math)")],
  },
  "forest.density": {
    title: "Mean tree density",
    body: [
      "Mean number of tree stems per square metre (trees/m²) in the stand. Together with stem radius it sets the scattering-and-absorption cross-section per unit path length, so a denser stand attenuates more.",
      "This is a continuous physical parameter, not a category — enter the site value. Zero density is valid but means no forest attenuation and is flagged in validation as probably unintended.",
    ],
    citations: [av("Forest sub-model: stem density sets the per-metre scattering cross-section")],
  },
  "forest.stem_radius": {
    title: "Mean stem radius",
    body: [
      "Mean trunk radius, in metres. With density it determines how much sound each metre of in-canopy path scatters and absorbs; thicker trunks scatter more per stem.",
      "Enter the average over the stand (for example 0.1–0.2 m for mature trees). The value feeds the Nord2000 forest law's cross-section term, not a flat empirical per-metre figure.",
    ],
    citations: [av("Forest sub-model: stem radius in the scattering cross-section")],
  },
  "forest.height": {
    title: "Mean tree height",
    body: [
      "Mean tree height, in metres. It sets the effective in-canopy path length: how much of the source–receiver ray actually passes through vegetation rather than under the canopy or above it.",
      "Height matters most when the source or receiver is low and the canopy is tall, so the ray spends its length inside the stand. It scales the attenuation, which then saturates with distance and is bounded by a floor.",
    ],
    citations: [av("Forest sub-model: mean height scales the effective in-canopy path length")],
  },
  "ground_zone.impedance_class": {
    title: "Impedance class (A–H)",
    body: [
      "The Nord2000 ground impedance class, from A (softest, most absorptive) to H (hardest, near-rigid). It is stored as the class letter; the σ shown beside each option is the flow resistivity in kPa·s/m² that the class maps to (for example A = 12.5, B = 31.5, D = 200, G = 20000).",
      "Ground impedance controls the amplitude and phase of the ground reflection via the Delany–Bazley one-parameter model. Soft ground (low σ) can produce a strong destructive ground dip; hard ground reflects nearly perfectly. This is often the largest single environmental term over open ground.",
      "Classes are a closed vocabulary — you cannot enter an out-of-range value — which keeps the scene consistent with the engine's impedance table (the single source of truth for σ).",
    ],
    citations: [
      av("Ground impedance model; Delany–Bazley classes A–H by flow resistivity σ"),
      envi("Class B σ = 31.5 kPa·s/m² (corrected against the AV impedance table)"),
    ],
  },
  "ground_zone.roughness_class": {
    title: "Roughness class (N/S/M/L)",
    body: [
      "The surface-roughness class — N (none/smooth), S (small), M (medium), L (large). Roughness perturbs the effective ground reflection beyond the flat-impedance term, softening the coherent ground dip that perfectly smooth ground would produce.",
      "Choose it to match the site texture (ploughed fields, rough grass, rubble). Like impedance it is a closed vocabulary so an out-of-vocabulary value is impossible.",
    ],
    citations: [av("Surface-roughness classes perturbing the ground reflection")],
  },
  "facade.default": {
    title: "Building default façade isolation",
    body: [
      "The isolation spectrum every façade edge of this building uses unless it has its own override. Editing the default applies live to all inheriting edges. Use it to set a whole building once, then override only the special façades.",
      "Façade isolation is the sound-reduction index per band used for the transmitted/immission path at the building surface.",
    ],
    citations: [av("Façade sound-reduction index"), ti("Building façade assessment")],
  },
  "facade.edge": {
    title: "Per-façade override",
    body: [
      "Each footprint edge is one façade. A façade either inherits the building default or overrides it with its own isolation spectrum (click Edit on the edge row). Overrides are keyed by a stable per-edge UUID, so inserting or moving a vertex elsewhere on the footprint never silently re-points an override to a different wall.",
      "Override only the façades that differ (a glazed elevation, a louvred plant wall); leave the rest inheriting so a single default edit still reaches them.",
    ],
    citations: [envi("Per-edge UUID identity (D-02: no silent re-point across geometry edits)")],
  },

  // ============================ Import panel (Phase 8) ============================
  "import.run": {
    title: "Import (viewport)",
    body: [
      "Fetches the enabled GIS layers (terrain, land cover, buildings) for the current map viewport and merges them into the scene. Re-importing refreshes untouched features while keeping your manual edits (a user-modified guard). A maximum-area guardrail blocks viewports that are too large to import responsibly.",
      "All heavy GIS decoding runs locally in WebAssembly; only the raw source bytes are fetched, and re-import is keyed on source identity so nothing you edited by hand is clobbered.",
    ],
    citations: [envi("Client-side WASM GIS ingestion; re-import merge keyed on (source, ref)")],
  },
  "import.debug_overlay": {
    title: "Impedance overlay",
    body: [
      "Toggles a debug overlay that colours the imported land-cover ground zones by their Nord2000 impedance class, so you can verify the WorldCover→class mapping before computing.",
      "It is a display aid only and does not change the scene or the calculation.",
    ],
    citations: [envi("Land-cover → impedance-class debug overlay")],
  },
  "import.layer_terrain": {
    title: "Terrain layer",
    body: [
      "Imports elevation as the digital ground model source. Where available, a high-resolution national DTM (a true bare-earth model) is used; elsewhere a global 30 m surface model (GLO-30) is used and flagged, because a surface model includes buildings and vegetation rather than bare earth.",
      "Terrain drives every height and profile in the calculation, so prefer a bare-earth DTM where the coverage exists. Dense grids are decimated to a tractable point budget.",
    ],
    citations: [ti("Terrain feeds the digital ground model"), envi("DTM vs GLO-30 surface-model provenance (D-05)")],
  },
  "import.layer_landcover": {
    title: "Land-cover layer",
    body: [
      "Imports ESA WorldCover land cover and vectorises it into ground zones, each mapped to a Nord2000 impedance class (for example water, built-up, cropland, tree cover). It gives the ground-effect calculation a spatially varying impedance instead of one flat assumption.",
      "Review the mapped classes with the impedance overlay; you can edit any zone's class afterwards in the inspector.",
    ],
    citations: [
      av("Spatially varying ground impedance"),
      envi("ESA WorldCover → Nord2000 impedance-class table"),
    ],
  },
  "import.layer_buildings": {
    title: "Buildings layer",
    body: [
      "Imports OpenStreetMap/Overpass building footprints with heights (from measured/tagged height, then levels × 3 m + 1.5 m, then a default). Buildings act as diffracting and reflecting obstacles and expose per-edge façades.",
      "Invalid rings are skipped and reported rather than failing the whole import, so one bad polygon never blocks the layer.",
    ],
    citations: [
      av("Buildings as diffracting/reflecting obstacles"),
      envi("OSM footprints with the height provenance chain; skip-and-report bad rings"),
    ],
  },

  // ============================ Weather panel (Phase 9) ============================
  "weather.date": {
    title: "Weather date",
    body: [
      "The calendar date whose weather is imported from the Open-Meteo archive/forecast for the site (the map-viewport centre). Weather sets the sound-speed profile that bends rays and, with wind, makes propagation direction-dependent (upwind quieter, downwind louder).",
      "Combined with the hour below, it selects the atmospheric snapshot the A/B/C sound-speed profile is derived from.",
    ],
    citations: [av("Refraction from the atmospheric sound-speed profile"), ti("Meteorology as a calculation input")],
  },
  "weather.hour": {
    title: "Weather hour (UTC)",
    body: [
      "The hour of day, in UTC, for the imported weather snapshot (0–23). Time of day strongly affects the vertical temperature and wind gradients: night-time temperature inversions and daytime lapse conditions bend rays differently, changing levels especially at long range.",
      "Pick the hour relevant to the assessment (for example a quiet night-time inversion for a worst-case downwind condition).",
    ],
    citations: [av("Diurnal temperature/wind gradients drive refraction")],
  },
  "weather.z0": {
    title: "Roughness length z₀ (m)",
    body: [
      "The aerodynamic roughness length of the terrain, in metres — how quickly wind speed grows with height above ground. It shapes the logarithmic wind profile and therefore the wind contribution to the sound-speed gradient. It is clamped to at least 0.001 m to avoid a singular profile.",
      "Typical values: ~0.0002 m over water, ~0.03 m over grass, ~0.1–0.5 m over crops/scrub, ~1 m over suburbs. A larger z₀ means a stronger near-ground wind shear.",
    ],
    citations: [
      av("Logarithmic wind profile in the sound-speed gradient"),
      envi("z₀ clamped ≥ 0.001 m (numerics house rule)"),
    ],
  },
  "weather.import": {
    title: "Import weather",
    body: [
      "Fetches the weather for the selected date/hour and derives the per-azimuth sound-speed A/B/C profile in WebAssembly. Results are cached in OPFS, so re-importing the same date/hour costs no network call.",
      "The derivation uses a well-conditioned separable model (temperature gradient plus a neutral-log wind law) rather than an ill-posed raw fit of sparse pressure levels; the per-azimuth A/B/C readout appears below.",
    ],
    citations: [
      av("A/B/C sound-speed profile parameters"),
      envi("Separable Route-2 A/B/C derivation from Open-Meteo (METX-01)"),
    ],
  },
  "weather.debug_grid": {
    title: "Debug: receiver grid",
    body: [
      "Overlays the computed receiver lattice on the map so you can check the grid layout inside the calculation area before running a full solve.",
      "It is a display aid only: it does not change the scene or the calculation, and toggling it costs nothing. Use it to confirm the grid covers the area you expect and clears the building footprints.",
    ],
    citations: [envi("Receiver-grid debug overlay")],
  },
  "weather.debug_impedance": {
    title: "Debug: impedance segmentation",
    body: [
      "Overlays how a source–receiver terrain profile is split into ground-impedance segments where it crosses ground zones, so you can verify that the ground model matches the drawn land cover.",
      "It is a display aid only and does not change the calculation. Because ground effect is often the largest environmental term, confirming the segmentation is a useful sanity check before a solve.",
    ],
    citations: [av("Terrain profile segmented by ground impedance"), envi("Impedance-segmentation debug overlay")],
  },
  "weather.debug_screens": {
    title: "Debug: screen vertices",
    body: [
      "Overlays the screen/diffraction vertices injected into the terrain profile (wall tops, building edges, berm crests), so you can confirm which obstacles the diffraction sub-model will actually see.",
      "It is a display aid only. Nord2000 caps a path at two screen edges, so this overlay is a quick way to check that the intended shielding obstacles are the ones being picked up.",
    ],
    citations: [av("Screen tops as diffraction vertices in the profile"), envi("Screen-vertex debug overlay")],
  },
  "weather.compute_debug": {
    title: "Compute debug geometry",
    body: [
      "Runs the geometry pre-pass that produces the debug overlays (receiver grid, impedance segmentation, screen vertices) without launching a full acoustic solve.",
      "Use it to sanity-check the geometry cheaply before committing to a real calculation, which is far more expensive. It builds the terrain profiles and segmentation the solver would use, then draws them.",
    ],
    citations: [envi("Geometry pre-pass for the debug overlays")],
  },

  // ============================ Calculate panel (Phase 10) ============================
  "calc.spacing": {
    title: "Fine grid spacing (m)",
    body: [
      "The receiver spacing of the finest grid tier, in metres. Smaller spacing gives a smoother, higher-resolution noise map but multiplies the receiver count (roughly as 1/spacing²), the transfer-tensor size, and the solve time. The panel shows the resulting receiver count, tensor megabytes, and time estimate.",
      "Solving is tiered — discrete receiver points, then a coarse grid, then this fine grid — so no receiver is computed twice and early tiers give a quick preview. Choose spacing to match the map scale: a few metres for a detailed site, coarser for a wide overview.",
    ],
    citations: [ti("Grid resolution of the noise map trades detail against calculation size")],
  },
  "calc.run": {
    title: "Run calculation",
    body: [
      "Starts the multi-threaded solve over the drawn scene, filling the transfer tensor for every grid and discrete receiver. It is enabled only when a project is open, a calculation area and at least one source exist, the browser session is cross-origin isolated (required for the threaded WebAssembly), and the cost guardrail does not block. A disabled Run always shows the single most relevant reason.",
      "The whole calculation runs locally in WebAssembly worker threads; nothing acoustic is computed on a server. Progress is reported per tier and can be aborted.",
    ],
    citations: [
      av("Nord2000 forward solve producing per-band levels"),
      envi("Client-side threaded WASM solve; cross-origin-isolation gate (D-01)"),
    ],
  },
  "calc.abort": {
    title: "Abort calculation",
    body: [
      "Cooperatively cancels the running solve. Tiers that already finished are kept, so an aborted run still leaves you the coarser preview it had reached.",
      "Use it when a fine-spacing estimate turns out larger than intended; adjust the spacing and re-run.",
    ],
    citations: [envi("Cooperative cancel; completed tiers retained")],
  },

  // ============================ Receiver spectrum (Phase 11) ============================
  "spectrum.receiver": {
    title: "Receiver selection",
    body: [
      "Selects which receiver's spectrum is shown, either by clicking a marker on the mini-map or a row in the synced list. The chart and totals then read that receiver's per-band levels from the cached tensor.",
      "Selection is purely a readout — no recomputation happens — so switching receivers is instant.",
    ],
    citations: [envi("Receiver readout from the cached transfer tensor (D-01)")],
  },
  "spectrum.display_mode": {
    title: "Display resolution (1/3-oct ⇄ 1/12-oct)",
    body: [
      "Switches the chart and table between the 27 one-third-octave band centres (the default overview) and the full 105-point 1/12-octave grid (expert detail). The underlying data is always the 1/12-octave engine grid; the third-octave view aggregates by band INDEX (every fourth point is an exact one-third-octave centre), never by nominal frequency.",
      "Use 1/3-octave for a readable overview and comparison against standards, and 1/12-octave to inspect narrow ground-dip or comb features that a coarser view would hide.",
    ],
    citations: [
      envi("105-point 1/12-octave grid; every 4th index is a 1/3-octave centre; compare by band index"),
      ti("Third-octave presentation of results"),
    ],
  },
  "spectrum.weighting": {
    title: "Frequency weighting dB(A) / dB(C)",
    body: [
      "Chooses the frequency weighting of the total shown: A-weighting approximates human loudness sensitivity and is the standard for environmental noise; C-weighting is nearly flat and emphasises low-frequency content (useful for bass/rumble complaints).",
      "This only re-weights and re-sums the already-computed per-band levels — it never triggers a recompute, so it is instant. Per-band bars stay unweighted; the total switches.",
    ],
    citations: [ti("A- and C-weighted totals of the spectrum"), av("Per-band levels summed to a weighted total")],
  },
  "spectrum.split": {
    title: "Coherent / incoherent split",
    body: [
      "Overlays the two physical channels that make up each band level: the coherent channel (phase-preserving sum of complex transfers — interference, ground dips, array beamforming) and the incoherent channel (turbulence-decorrelated energy). The total is |coherent sum|² + incoherent energy.",
      "The split explains features in the total: a deep coherent notch that the incoherent floor partly fills in, for example. When turbulence decorrelation is off, the incoherent channel is zero and the total is purely coherent.",
    ],
    citations: [
      av("Coherent transfer plus a separate turbulence-decorrelated (incoherent) energy term"),
      envi("Total = |coherent Σ|² + P_incoh; F→1 ⇒ P_incoh→0 (complex-tensor pillar)"),
    ],
  },
  "spectrum.table": {
    title: "Show exact numbers",
    body: [
      "Expands a table of the exact per-band values: band index, the exact centre frequency in Hz (from the server frequency axis), and the level in dB. Band identity is the index plus exact Hz — nominal labels are for reading only.",
      "Use it to read or copy precise values rather than eyeballing the chart.",
    ],
    citations: [envi("Band identity = band index + exact Hz (never the nominal label)")],
  },

  // ============================ Colour-scale editor (Phase 11) ============================
  "colorscale.preset": {
    title: "Colour-scale preset",
    body: [
      "Picks the isophone colour scheme: EU-END (the environmental-noise-directive banded default), or the perceptually uniform Viridis / Turbo ramps. The preset sets the class colours; editing it re-contours the cached level grid with no re-solve.",
      "Perceptually uniform ramps avoid the misleading bright bands of a rainbow scale; EU-END matches the familiar regulatory noise-band colours.",
    ],
    citations: [ti("Grid noise-map colouring, §4.6.5"), envi("Re-contour on colour change, no re-solve (SC3)")],
  },
  "colorscale.breaks": {
    title: "Isophone break values",
    body: [
      "The dB level boundaries between colour classes (the isophone edges). Each row is one edge value in dB; the swatch shows the class colour above that edge. Editing a break re-contours the map instantly from the cached grid — the single breaks/colours source of truth drives the contour tracer, the fill, and the legend together, so legend, contour, and class always agree.",
      "Set breaks to the levels that matter for the assessment (for example regulatory limit values), or use the uniform generator below for an evenly spaced scale.",
    ],
    citations: [ti("Editable isophone class boundaries, §4.6.5"), envi("Single breaks[]/colors[] source of truth (D-02/D-04)")],
  },
  "colorscale.smallest": {
    title: "Smallest interval (generator)",
    body: [
      "The lowest break value, in dB, for the uniform-scale generator (NoizCalc §4.6.5). It is where the evenly spaced isophone ladder starts.",
      "Combined with the interval magnitude and the number of intervals, it defines a regular set of break values with one click.",
    ],
    citations: [ti("Uniform grid-scale generator, §4.6.5")],
  },
  "colorscale.magnitude": {
    title: "Interval magnitude (generator)",
    body: [
      "The spacing between consecutive breaks, in dB, for the uniform generator — for example 5 dB gives a 45/50/55/60 ladder. Must be positive.",
      "A smaller magnitude gives finer colour banding (more classes over the same range); a larger one gives broader bands.",
    ],
    citations: [ti("Uniform grid-scale interval, §4.6.5")],
  },
  "colorscale.count": {
    title: "Number of intervals (generator)",
    body: [
      "How many evenly spaced classes the uniform generator produces (at least 2). With the smallest value and the magnitude, it fixes the whole ladder: breaks run from the smallest value upward in equal steps.",
      "More intervals give finer colour resolution over the level range; fewer give broader bands. The generator only seeds the break list — you can still hand-edit individual breaks afterwards.",
    ],
    citations: [ti("Number of grid-scale intervals, §4.6.5")],
  },
  "colorscale.apply": {
    title: "Apply uniform scale",
    body: [
      "Replaces the current break list with an evenly spaced ladder built from the smallest value, interval magnitude, and interval count, then re-contours the map from the cached grid with no re-solve.",
      "Use it as a quick reset to a regular scale before hand-tuning individual breaks. It overwrites the existing breaks, so save any bespoke ladder you want to keep first.",
    ],
    citations: [ti("Apply the generated uniform scale, §4.6.5")],
  },
  "colorscale.ascending": {
    title: "Ascending order",
    body: [
      "Controls whether the colour sequence runs from low to high level (ascending) or is reversed. It changes which end of the ramp maps to the loudest class, without changing the break values themselves.",
      "Ascending (dark/cool = quiet, bright/warm = loud) is the usual convention for noise maps; reverse it only if a report template expects the opposite orientation.",
    ],
    citations: [ti("Ascending/descending colour order, §4.6.5")],
  },
  "colorscale.keep_sequence": {
    title: "Keep colour sequence",
    body: [
      "When on, the class colours keep their sequence as you change the number of intervals, so the palette identity is preserved and only the break values move. When off, colours are re-sampled across the ramp for the new class count.",
      "Keeping the sequence is useful when a fixed set of band colours has a regulatory meaning; re-sampling is useful when you just want an even spread of the ramp over whatever class count you pick.",
    ],
    citations: [ti("Keep-colour-sequence option, §4.6.5")],
  },

  // ============================ Conditioning panel (Phase 11) ============================
  "conditioning.gain": {
    title: "Source gain (dB)",
    body: [
      "Adjusts this source's level by a per-band-flat gain, in dB, and re-reads the map and spectra live from the cached tensor with no re-propagation. +6 dB doubles the pressure contribution of this source; −∞ effectively mutes it.",
      "Conditioning is a post-processing re-mix of the already-solved complex transfers (a multiply-and-accumulate over the tensor), which is why it updates in real time. Because it does not change propagation, a conditioning edit never marks the result as out of date.",
    ],
    citations: [
      envi("Fast-recalc MAC over the cached tensor; conditioning excluded from tensor identity (D-07/SVC-06)"),
      ti("Per-source level adjustment in the mix"),
    ],
  },
  "conditioning.delay": {
    title: "Source delay (ms)",
    body: [
      "Applies a time delay to this source, in milliseconds, before the coherent sum. Delay changes the phase each source contributes and therefore the interference pattern between sources — the basis of loudspeaker-array steering and alignment.",
      "It only affects the coherent channel; the incoherent energy is magnitude-only. The recalculation is a live MAC over the cached tensor, so the interference map updates instantly.",
    ],
    citations: [
      envi("Delay enters the coherent channel as phase; live MAC recalc (D-01)"),
      ti("Loudspeaker delay/alignment in complex setups"),
    ],
  },
  "conditioning.filter": {
    title: "Source filter",
    body: [
      "Opens the spectrum editor to shape this source's level per band (an equaliser curve in dB across the band grid), reusing the same editor as source/screen spectra. The filter is applied in the live recalc, so the map and spectra follow your edits.",
      "The dense per-band filter is materialised by the server's interpolation from your anchors; the frontend authors no dB arithmetic itself. Use it to model band-limiting, roll-off, or a measured system response.",
    ],
    citations: [
      envi("Per-band filter into the recondition MAC; dense curve materialised server-side (D-11)"),
      ti("Electronic filters on sources"),
    ],
  },
  "conditioning.mute": {
    title: "Mute source",
    body: [
      "Removes this source from the mix without deleting it, so you can audition the map with and without a given source. Muting and unmuting recomputes the readout live from the cached tensor with no re-propagation.",
      "It is the quickest way to see a single source's contribution to the total: mute everything else, or mute just the source in question and watch the map change.",
    ],
    citations: [envi("Per-source mute in the live recalc")],
  },

  // ============================ Scenario manager (Phase 11) ============================
  "scenario.new": {
    title: "New scenario",
    body: [
      "Creates a new weather scenario by cloning the active one, so you can vary the meteorology (temperature, humidity, pressure, wind, gradient) and compare conditions. Each scenario computes and caches its own tensor, because a meteorology change alters propagation and requires a real recompute.",
      "Use scenarios for what-if comparisons: a calm night versus a downwind breeze, for instance.",
    ],
    citations: [av("Meteorology changes the propagation and requires recomputation"), ti("Weather scenarios (e.g. two wind directions)")],
  },
  "scenario.list": {
    title: "Scenario list",
    body: [
      "Lists the base scenario and any you create. Each row switches to a scenario, shows whether its result is cached or needs computing, computes it, and (for non-base scenarios) deletes it.",
      "Switching between already-computed scenarios is instant because each caches its own result; a meteorology change marks that scenario as needing recomputation, since weather alters propagation.",
    ],
    citations: [envi("Per-scenario cached tensor keyed by its met/scene identity")],
  },
  "scenario.name": {
    title: "Scenario name",
    body: [
      "A human-readable label for the active scenario (for example \"Downwind SW 5 m/s\"). Naming keeps comparisons legible in the scenario list and on the difference-map legend.",
      "Give scenarios descriptive names that capture the condition they represent, so an A−B comparison reads clearly in a report.",
    ],
    citations: [envi("Named scenario references")],
  },
  "scenario.save": {
    title: "Save scenario",
    body: [
      "Computes (or recomputes) the active scenario with its current meteorology and caches the result. Because a met change alters propagation, this runs a full solve for the scenario rather than a live re-mix.",
      "Save/compute after editing the met inputs so the scenario's cached map reflects them; the list marks a scenario that has pending edits as not yet computed.",
    ],
    citations: [av("Meteorology recompute yields a new propagation result")],
  },
  "scenario.mode": {
    title: "Friendly / Advanced met input",
    body: [
      "Chooses how you specify the atmosphere. Friendly takes intuitive quantities (temperature, humidity, pressure, a Beaufort wind class and direction, a temperature gradient) that WebAssembly turns into the per-azimuth sound-speed A/B/C profile. Advanced lets an expert enter the raw A/B/C profile directly, bypassing the derivation.",
      "Use Friendly for everyday what-ifs and Advanced when you already have a measured or prescribed sound-speed profile.",
    ],
    citations: [
      av("Sound-speed A/B/C profile parameters"),
      envi("Friendly knobs → WASM A/B/C derivation; advanced raw bypass (D-14)"),
    ],
  },
  "scenario.temperature": {
    title: "Air temperature (°C)",
    body: [
      "The air temperature, in degrees Celsius. It sets the speed of sound (about 340 m/s near 15 °C) and, with humidity and pressure, the atmospheric absorption that attenuates high frequencies over distance.",
      "Temperature is one input to the friendly→A/B/C derivation; the vertical temperature gradient (below) is what actually bends rays.",
    ],
    citations: [av("Temperature sets sound speed and air absorption")],
  },
  "scenario.humidity": {
    title: "Relative humidity (%)",
    body: [
      "Relative humidity, in percent. With temperature and pressure it governs atmospheric absorption, which grows with frequency and distance — mid/high bands are attenuated far more than low bands, and humidity strongly modulates that.",
      "Dry air can, counter-intuitively, absorb more at some mid frequencies than moist air, so humidity is not a minor input at long range.",
    ],
    citations: [av("Humidity in the air-absorption model")],
  },
  "scenario.pressure": {
    title: "Atmospheric pressure (kPa)",
    body: [
      "Ambient atmospheric pressure, in kilopascals (about 101.3 kPa at sea level). It enters the atmospheric-absorption calculation together with temperature and humidity.",
      "Its effect is smaller than temperature and humidity but is included for completeness and for elevated sites.",
    ],
    citations: [av("Pressure in the air-absorption model")],
  },
  "scenario.beaufort": {
    title: "Wind speed (Beaufort)",
    body: [
      "Wind strength as a Beaufort class (0 calm … up), shown with its equivalent m/s. Wind makes propagation direction-dependent: downwind rays bend toward the ground (higher levels, less shielding), upwind rays bend upward into a shadow zone (lower levels).",
      "The Beaufort class and direction are converted, with the roughness length, into the per-azimuth sound-speed profile the engine uses for refraction.",
    ],
    citations: [av("Wind gradient bends rays; up/down-wind asymmetry"), ti("Wind in the meteorology")],
  },
  "scenario.wind_direction": {
    title: "Wind direction (° from)",
    body: [
      "The direction the wind blows FROM, in degrees (0/360 = north, 90 = east). It decides which source–receiver bearings are downwind (louder) and which are upwind (quieter), so it rotates the asymmetry of the noise map.",
      "Enter the meteorological convention (direction the wind comes from), matching how weather reports state it.",
    ],
    citations: [av("Bearing-dependent refraction from wind direction")],
  },
  "scenario.temp_gradient": {
    title: "Temperature gradient (°C/m)",
    body: [
      "The vertical temperature gradient, in °C per metre. A positive gradient (temperature rising with height — an inversion, typical at night) bends rays downward and raises levels at range, much like downwind; a negative lapse gradient bends rays upward.",
      "Together with wind it forms the sound-speed profile that governs refraction. Inversions are the classic cause of distant noise being unexpectedly audible at night.",
    ],
    citations: [av("Temperature gradient in the sound-speed profile and refraction")],
  },
  "scenario.worst_case": {
    title: "Downwind worst-case",
    body: [
      "Applies a favourable (downwind) sound-speed profile on EVERY source–receiver bearing at once — a non-physical, standardised worst case widely used for regulatory traffic/industry assessments, where you want the highest plausible level in all directions rather than one real wind.",
      "It is deliberately conservative: no single real wind is downwind everywhere. For event or short-duration assessments, prefer specific realistic scenarios instead.",
    ],
    citations: [
      ti("Downwind option = a hypothetical worst case, downwind in every direction"),
      av("Favourable-condition (downwind) propagation"),
    ],
  },
  "scenario.raw_a": {
    title: "Raw profile A (log term, m/s)",
    body: [
      "The logarithmic-term coefficient of the raw sound-speed profile, applied to every path azimuth (advanced mode). It scales the ln(height) part of the effective sound-speed-with-height, which is dominated by wind shear.",
      "Set A/B/C directly only when you have a prescribed or measured profile; otherwise use the friendly inputs, which derive these for you.",
    ],
    citations: [av("Sound-speed profile: logarithmic (wind) term")],
  },
  "scenario.raw_b": {
    title: "Raw profile B (linear term, 1/s)",
    body: [
      "The linear-term coefficient of the raw sound-speed profile (advanced mode), in 1/s. It captures the part of the sound-speed-with-height that grows linearly, dominated by the temperature gradient.",
      "A positive B corresponds to a downward-refracting (inversion/downwind-like) atmosphere.",
    ],
    citations: [av("Sound-speed profile: linear (temperature-gradient) term")],
  },
  "scenario.raw_c": {
    title: "Raw profile C (ground sound speed, m/s)",
    body: [
      "The sound speed at the ground, in m/s (advanced mode) — the profile's offset, about 340 m/s at 15 °C. A and B describe how sound speed changes with height above this base value.",
      "It follows from the ground-level air temperature; leave it near 340 m/s unless you are modelling a markedly hotter or colder surface.",
    ],
    citations: [av("Sound-speed profile: ground-level sound speed")],
  },
  "scenario.raw_z0": {
    title: "Raw profile roughness z₀ (m)",
    body: [
      "The roughness length used by the raw profile's logarithmic term, in metres (advanced mode), clamped to at least 0.001 m. It sets how sharply the wind term grows near the ground.",
      "See the weather panel's z₀ help for typical surface values.",
    ],
    citations: [av("Roughness length in the logarithmic wind profile"), envi("z₀ clamped ≥ 0.001 m")],
  },
  "scenario.compare_a": {
    title: "Compare — scenario A",
    body: [
      "Picks the first scenario for the difference map. Comparing two computed scenarios shows A − B in dB(A) per grid point on a diverging colour scale, so you can see where and by how much one condition is louder than another.",
      "Both scenarios must be computed first; the difference is calculated in WebAssembly from their cached dB(A) totals.",
    ],
    citations: [envi("Scenario A−B difference map computed in WASM (D-16)")],
  },
  "scenario.compare_b": {
    title: "Compare — scenario B",
    body: [
      "Picks the second scenario for the difference map (the subtrahend in A − B). Choose a scenario different from A; identical picks produce a flat zero difference.",
      "Both A and B must be computed before the difference can be drawn, since it works from their cached dB(A) grids.",
    ],
    citations: [envi("Scenario A−B difference map (D-16)")],
  },
  "scenario.compare_run": {
    title: "Compare scenarios",
    body: [
      "Computes the A − B dB(A) difference across the grid and renders it as a diverging map with a legend, highlighting where meteorology changes the exposure most and in which direction.",
      "The difference is computed in WebAssembly from the two scenarios' cached dB(A) totals, so both must be computed first. Positive and negative differences use opposite ends of the diverging scale.",
    ],
    citations: [envi("Diverging difference map from two cached scenario totals")],
  },
  "scenario.compare_clear": {
    title: "Clear difference map",
    body: [
      "Removes the difference overlay and returns the map to the single active scenario's noise map.",
      "Use it to step back from a comparison to the ordinary level view without changing either scenario's cached result.",
    ],
    citations: [envi("Clear the scenario difference overlay")],
  },

  // ============================ Export menu (Phase 11) ============================
  "export.open": {
    title: "Export…",
    body: [
      "Opens the export menu. All export bytes are generated locally in WebAssembly and saved via an in-browser download — nothing leaves the device. Export is disabled until a result exists and while the result is out of date, so you never export a stale map. Every file carries a full metadata/attribution footer.",
      "The footer records the coordinate reference system, the dB weighting, the engine version and scene/tensor identity, and the open-data attributions (OpenStreetMap/Overture, ESA WorldCover, Copernicus).",
    ],
    citations: [
      envi("WASM-generated bytes, local download, full metadata + attribution (D-20/D-21/D-22)"),
      ti("Exporting the grid noise map"),
    ],
  },
  "export.geotiff": {
    title: "Export GeoTIFF (raster grid)",
    body: [
      "Exports the level grid as a georeferenced Float32 GeoTIFF — one level per grid cell, north-up, with the CRS geo-keys and a NODATA value for cells outside the domain. Open it in any GIS to overlay the noise map on other layers or post-process it.",
      "The raster is the exact per-cell dB values of the cached grid (in the selected weighting), not a re-interpolation, and includes the attribution/metadata footer.",
    ],
    citations: [envi("Float32 GeoTIFF with geo-keys + NODATA + metadata footer (GRID-05/D-22)")],
  },
  "export.geojson": {
    title: "Export GeoJSON (isophone polygons)",
    body: [
      "Exports the isophone fill as GeoJSON MultiPolygons — one feature per colour class, with its dB break range in the properties. Use it to bring the banded contours into a GIS or web map as vector data.",
      "Polygons are derived from the same contour tracer that draws the on-screen isophones, so the exported bands match the map exactly (RFC-7946 GeoJSON with the attribution footer as foreign members).",
    ],
    citations: [envi("RFC-7946 isophone MultiPolygons per class + attribution (GRID-05/D-22)")],
  },
  "export.csv": {
    title: "Export CSV (receiver spectra)",
    body: [
      "Exports the receiver spectra as CSV: one row per band with the band index, the exact centre frequency in Hz, and the level in dB, plus the dB(A) and dB(C) totals. Band identity is the index and exact Hz — never a rounded nominal label — so the data stays unambiguous.",
      "Use it for reporting or further analysis in a spreadsheet; the header carries the CRS, weighting, engine/scene identity, and attribution.",
    ],
    citations: [envi("CSV: band index + exact Hz + dB, with dB(A)/dB(C) totals and footer (GRID-05/D-22)")],
  },
};
