# OPEN ITEMS — the single, exhaustive ledger

**This file is the ONE place an open end may live.** If work is not finished, it is listed here. If it is
not listed here, it must not exist. Anything parked in a code comment, a phase `deferred-items.md`, a
commit message, or someone's head is **invisible** — and this project has already lost real bugs that way
(see OPEN-08 below: a WASM crash deferred in Phase 08 and then forgotten for two months).

**Rules**
1. Closing an item = the code does the thing, a test pins it, and the gates are green. Not "it's designed",
   not "the seam exists", not "it's documented".
2. Deleting an item without closing it requires the user's explicit decision, recorded here with a date.
3. Every item states **how you will know it is done** — a command or an observable outcome, not a feeling.
4. Items marked **EXTERNAL** cannot be closed by effort alone. Do not pretend otherwise.

*Last verified against the tree: 2026-07-13 (by grep + a live in-browser run, not from memory).*

---

## A. Blocked on something outside the repo (cannot be finished by working harder)

### OPEN-01 — FORCE numeric Pass (VAL-02) — **EXTERNAL**
The full road chain is wired and the emission coefficients are CITED (Jonasson Table A.1, verified against
the page image). But that public set is the **intermediate DK Nord 2005** lineage: it over-predicts the
FORCE free-field emission by a **measured ~2.3 dBA** (`emission_force_delta`), outside the Ch.6 1 dB
tolerance. The FORCE road cases therefore stay honest `Skipped`, never a false Pass.
**Blocker:** the definitive **Dec-2006** coefficient set. We do not have that document.
**Done when:** the Dec-2006 coefficients are obtained and integrated, and the FORCE road suite passes
within 1 dB. **Until then this cannot be closed, and closing it any other way would be a false green.**
**Do NOT** "fix" this by loosening the tolerance or flipping the capability flag.

---

## B. Not started — Milestone 3 (the drawn scene does not reach the solver)

This is the big one, and it is the thing a user actually notices: **you can draw a wall and the numbers do
not change.** `web/src/compute/marshalScene.ts` fabricates a flat homogeneous corridor
(`weather: null`, `forest: null`, `isolation: null`, no screens, a 2-point profile with a hardcoded
σ=200). The wire (`PrepareSolveReq`) and the Phase-9 extractors already exist — they are **dangling**.

Full plan: `.planning/ROADMAP.md` → Milestone 3, Phases 12–17. Requirements: SOLV-01..07, ENG-11, ENG-12.

| # | Item | Phase | Done when |
|---|------|-------|-----------|
| OPEN-02 | Scene → solver foundation (real coords, scene on the wire, **per-path** derivation, receiver plan reconciled, identity hashing complete) | 12 | A flat empty scene solves `f64::to_bits`-identically to today (refactor proven inert), and a field-sensitivity test proves every scene field is in `marshalled_tensor_hash` |
| OPEN-03 | Screens / ground zones / partitions reach the solve | 13 | A 4 m wall drawn between source and receiver **lowers the level**; the same wall behind the receiver does not |
| OPEN-04 | DGM terrain → real cut profile, **+ ENG-11 convex wedge** (§5.12, Eqs. 141–151) | 14 | A real imported Amsterdam terrain solves without a typed refusal (today any hillcrest kills it) |
| OPEN-05 | Forest per-path through-length → SM10 | 15 | A path crossing more forest attenuates more; a path missing the forest is unchanged |
| OPEN-06 | **ENG-12 refracted screen diffraction** (SM4 Eqs. 184–186 + the same pattern in SM5/SM6 + curved-ray threading) | 16 | `WeatherScreenNotImplemented` is **deleted**, and `ξ → 0` recovers the homogeneous screen bit-for-bit |
| OPEN-07 | Weather drives the solve (per-path, per-azimuth) | 17 | A downwind receiver differs from an upwind one in the expected direction; the scenario difference map is driven by two real solves, not fixtures |

**The trap in OPEN-06/07 (read before touching them).** With weather set, any screened path currently
**hard-errors by design**. It is tempting to "just let it through". A silently unrefracted screen — or a
mixed tensor with weather on ground paths but not screen paths — is a **FALSE RESULT** and is forbidden by
the honest-green rule. There is no cheap path: implement ENG-12, or refuse the solve loudly.

---

## C. Known bugs, still open

### OPEN-08 — Re-import into a populated scene traps the WASM (`unreachable`)
Deferred in Phase 08 (`.planning/phases/08-gis-ingestion-dgm/deferred-items.md`) and never closed.
Running a viewport import a **second time** into an already-populated scene panics the WASM module.
**Done when:** a re-import over a populated scene merges cleanly (D-09 idempotent merge) with a test that
fails on the panic. **This is exactly the class of item this ledger exists to stop losing.**

### OPEN-09 — `switch-basemap` switches to a blank dark style
`ALT_BASEMAP_STYLE` (`web/src/map/basemap.ts`) is a source-less inline style built as an *offline test*
mechanism for exercising `map.setStyle()` + `style.load` re-hydration. Now that the default basemap is the
colour Liberty style, a user clicking "Switch basemap" gets a **blank dark map**, which reads as a bug.
**Done when:** the switch offers a real second style (e.g. Liberty ⇄ Positron/dark), with the inline
source-less style kept for tests only.

---

## D. Deliberate approximations that are currently INVISIBLE to the user

These are not bugs, but they are lies of omission if never surfaced. Each must either be fixed or shown in
the UI/result metadata.

### OPEN-10 — Results are propagation-TRANSFER levels, not absolute L_den
A solve produces transfer levels (the isophone breaks auto-fit to a **negative** dB range). Absolute
EU-END `L_den` bands need a source **emission/SWL** model, which does not exist for drawn sources.
**Done when:** either a source emission/SWL model lands, or the UI states plainly that the map shows
relative transfer, not absolute levels. Right now it says neither.

### OPEN-11 — Forest `Fs` coherence factor (Eq. 288) not implemented
SM10's *excess attenuation* is implemented (ENG-09); its **decorrelation** term is not. The seam exists
(`CoherenceInputs`). See `.planning/phases/05-.../deferred-items.md`.
**Done when:** `Fs` lands, or the forest result documents that it omits decorrelation.

### OPEN-12 — Forest FORCE cases (121–124) still capability-gated
They stay `Skipped(requires: forest-scattering)` because the road-case `ForestCrossing` geometry
extraction does not exist. **This is unblocked by OPEN-05 (Phase 15)** — close them together.

---

## D2. From the COG range-read fix (2026-07-13) — real, and one of them is systemic

### OPEN-17 — `envi-gis-wasm` marshals `Option::None` as `undefined`, not `null` — **SYSTEMIC**
`serde_wasm_bindgen`'s `to_js` serializes `Option::None` as **`undefined`**, while the generated
`wire.ts` types the same field as `T | null`. So `x !== null` is **always true** on a `None` — a silent
type lie the TypeScript compiler cannot catch (the types say `null`, the runtime says `undefined`).
**This bug class has now bitten twice in one day**: it broke the COG header loop for *every* file, and it
left a latent bug in `importJob` where `base_elevation_m: None` arrived as `undefined`, so the "no terrain
covers this footprint" branch never fired — meaning a building with no terrain under it would have taken
a wrong base elevation instead of being reported.
**The fix** is one line — `.serialize_missing_as_null(true)` on the `to_js` serializer — but it changes
serialization for **every** GIS DTO consumed by `panels/`, `store/` and `map/`, so it was (correctly) not
done while other work was in flight. Current state: every TS consumer of an Option-typed WASM result needs
a defensive `?? null`, which is exactly the kind of rule that gets forgotten.
**Done when:** `serialize_missing_as_null(true)` is set, the `?? null` workarounds are removed, and a test
pins that a `None` field arrives as `null` across the WASM boundary.

### OPEN-18 — COG overview levels are not used
The range planner reads the **base image only**. A 1.5 km² AHN window decodes 2739×2244 px down to 1558
points — reading an overview (pyramid) level instead would cut the 10.9 MB fetch substantially again.
**Done when:** the planner selects the coarsest overview that still satisfies the requested resolution.

### OPEN-19 — Range fetches are sequential
The ≤6 planned range requests are issued one after another; parallelising them would cut the terrain
wall-clock (currently ~8.6 s for the whole import).

## E. Housekeeping (small, but they are open ends)

- **OPEN-13** — Delete the dead `SegmentedRefractionNotImplemented` variant. Verified 2026-07-13: declared
  at `crates/envi-engine/src/propagation/mod.rs:162`, **constructed at 0 sites** (segmented-ground
  refraction was wired in Phase 4). Its doc comment still claims the feature "is not yet wired", which is
  false. Also re-check `NonFlatTerrainNotImplemented` (1 construction site) — is it still reachable now
  that Sub-Model 3 exists?
- **OPEN-14** — Phase 11's **5 completion gates** were never run (code-review / simplify / secure / verify
  / doc-consistency). `.planning/STATE.md` still says "completion gates pending".
- ~~**OPEN-15** — Temporary diagnostic specs from the 2026-07-13 debug session~~ **CLOSED 2026-07-13**:
  `_diag-import.spec.ts` / `_diag-basemap.spec.ts` deleted once their bugs were fixed. Their permanent
  replacement is `npm run test:e2e:live` (the opt-in live-network smoke).
- **OPEN-16** — `height_provenance: "user"` is a value the TS side emits that the Rust side never does
  (introduced 2026-07-13). Nothing consumes provenance for logic, so it is safe — but the two sides should
  agree on the vocabulary.

---

## F. Explicitly OUT of scope — not open ends, do not "finish" these

Listed so nobody mistakes them for forgotten work:
- **GRID-03** — L_den weather-class statistics (deferred beyond Milestone 2 by design).
- **FUT-01..05** — DXF import, SketchUp import, per-segment correction interface, 2.5D BEM barrier
  corrections, SOFA directivity.
- Variable wall height along a base line; multi-height/façade receivers; road/rail/berm emission objects;
  report/print-sheet generation.
