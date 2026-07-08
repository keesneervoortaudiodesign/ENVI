# NoiseModelling CNOSSOS cross-validation oracle (VAL-03)

This directory holds the **operator-run recipe** and the **minimal scenes** used to
cross-validate ENVI's *shared* sub-effects against an independent CNOSSOS-EU
implementation, [NoiseModelling](https://noisemodelling.readthedocs.io/) v6.0.0
(Université Gustave Eiffel, GPLv3, implements Directive (EU) 2015/996).

It mirrors the project's **committed-fixture oracle** pattern (see
`tools/nord2000_oracle/`): NoiseModelling is run **once, by a human operator**, and
its numeric outputs are committed as
`crates/envi-harness/tests/fixtures/oracle/noisemodelling.toml`. **The Rust test
suite reads only that committed TOML — no Java, no network, no credentials at test
time.**

---

## ⛔ CRITICAL ENVIRONMENT RULE — read this first

**NEVER force-close, kill, or restart the JVM, Gradle, Maven, any language-server
process, or VS Code while generating these fixtures.** NoiseModelling runs on the
JVM; force-killing a Java/Gradle process has crashed this operator's editor **twice**.
Let every Java process **finish on its own**. If a run appears to hang, wait — do not
`taskkill`, `kill`, `Stop-Process`, or close the editor. There is no fixture worth a
crashed session.

---

## Why an external oracle (and why re-implementation is rejected)

VAL-03 explicitly names *NoiseModelling*. Re-implementing the CNOSSOS formulas in
Python/Rust would cross-check nothing (it would just re-transcribe the same spec ENVI
reads). The value is an **independent binary** producing the numbers. So we run the
GPL binary and commit its **outputs only** — we never port or translate its source
(CLAUDE.md licensing rule; ideas, not code).

## The three scenes (`scenes/`)

All scenes: single point source, receiver at 100 m, flat ground, standard atmosphere
**15 °C / 70 % RH / 101.325 kPa** (FORCE parameters). Coordinates are in a local
projected CRS (EPSG:2154, metres); the origin is arbitrary. Source height 0.5 m,
receiver height 1.5 m ⇒ direct distance `d ≈ 100.005 m`.

| Scene | File | Isolates | Gate |
|-------|------|----------|------|
| (a) free field, HARD ground (G=0), no barrier | `scenes/free_field.geojson` | `Adiv` (divergence) + `Aatm` (air absorption) | **equality** (≤0.1 / ≤0.2 dB) |
| (b) + thin 4 m barrier at x=50 m | `scenes/thin_barrier.geojson` | `Abar` / CNOSSOS `Adif` (insertion loss) | **report-only** (different model) |
| (c) SOFT ground (G=1), no barrier | `scenes/soft_ground.geojson` | `Aground` (excess attenuation) | **report-only** (different model) |

**Comparable-quantity posture** (04-RESEARCH "Comparable quantities and the delta
posture"): only physics that is *genuinely identical* is equality-gated —
geometrical divergence (`Nord2000 −10·lg(4πR²) ≡ CNOSSOS 20·lg d + 11`, since
`10·lg 4π = 10.99`) and ISO 9613-1 air absorption. Barrier insertion loss
(Hadden-Pierce wedge vs empirical `Adif`) and flat-ground excess attenuation
(complex spherical-wave Q̂ interference vs empirical G-model) are **documented
expected-delta reports** — forcing equality there would indicate a comparison bug,
not success.

Octave bands: CNOSSOS is octave-band **63 Hz – 8 kHz**. All 8 octave centres fall
exactly on ENVI's 1/12-octave grid at **band indices 16, 28, 40, 52, 64, 76, 88,
100** (every 12th point from 63.096 Hz). Compare **by band index, never by nominal
frequency**.

---

## One-time operator recipe (requires Java — NOT installed on this machine)

`java -version` currently fails on this machine, so the committed fixture ships with
**clearly-marked placeholder rows** and `placeholder = true` in `[meta]`; the Rust
test therefore **SKIPs** (honest fail-soft — never a false Pass) with a pointer back
here. Regenerate the fixture when Java is available:

### 1. Install a JRE (one-time), or generate on another machine

```powershell
# Windows, one-time. Temurin 17 JRE is sufficient for NoiseModelling v6.
winget install EclipseAdoptium.Temurin.17.JRE
```

Verify: `java -version` prints `17.x`. (You may instead generate the fixture on any
machine that already has Java and copy the resulting `.toml` back — the tests never
need Java, only the committed TOML.)

### 2. Get NoiseModelling v6.0.0 (offline)

Download the **v6.0.0** standalone distribution (offline zip) from the NoiseModelling
releases and unzip it locally. No network is needed after download; no account/API
key is required.

### 3. Load the scenes and run

Load each `scenes/*.geojson` as the SOURCE / RECEIVER / BUILDINGS(barrier) / GROUND
layers via the WPS/Groovy scripts (or the standalone GUI), set the atmosphere to
15 °C / 70 % RH, and run the CNOSSOS point-to-point calculation for the single
receiver.

### 4. Export the separately-reported CNOSSOS attenuation terms

CNOSSOS implementations report the attenuation terms separately. Export, **per octave
band (63 Hz – 8 kHz)**:

- `Adiv` — geometric divergence (dB)
- `Aatm` — atmospheric absorption (dB)
- `Abar` / `Adif` — barrier insertion loss (dB, scene b only)
- `Aground` — ground attenuation (dB, scene c only; scene a is G=0 ⇒ ~0)

### 5. Write the fixture with provenance

Edit `crates/envi-harness/tests/fixtures/oracle/noisemodelling.toml`:

- Set `[meta] placeholder = false`.
- Set `generated_date`, keep `nm_version = "v6.0.0"`.
- The `scene_*_sha256` values pin the scene files that produced the numbers
  (recompute with `sha256sum scenes/<file>.geojson | cut -c1-16` if you edit a
  scene — a changed scene must invalidate stale numbers).
- Fill each `[[divergence]]`, `[[air_absorption]]`, `[[barrier]]`, `[[ground]]`
  row's value field (`adiv_db` / `aatm_db` / `abar_db` / `aground_db`) with the
  exported number at its `band_index`. Keep all 8 octave indices
  (16, 28, 40, 52, 64, 76, 88, 100) — the Rust coverage assertions reject a
  truncated fixture.

Record the NoiseModelling version, the scene hashes, and the generation date in
`[meta]`. Then `cargo test -p envi-harness --test oracle_noisemodelling` runs the
equality gates (divergence ≤0.1 dB, air absorption ≤0.2 dB/octave; a larger 8 kHz
air-absorption delta is documented, not forced) and prints the barrier/ground
expected-delta report.

---

## License / data hygiene

NoiseModelling is GPLv3. We **run the binary and commit its numeric outputs**; we do
**not** copy, port, or translate its source into this repository. Equations are cited
by report/standard number only. The scenes here are our own minimal inputs, not
NoiseModelling data.
