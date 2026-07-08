# Nord2000 Road Source Modelling — Coefficient Reference

Transcription of the coefficient tables from the road source-modelling report,
verified against the PDF page images (house rule). See the companion PDF
`nord2000-source-modelling-jonasson-rev061017.pdf` in this directory.

## Provenance

- **Document:** "Source modelling report" (Nord2000 road source model / "DK Nord 2005"), H. Jonasson (SP), final **rev. 2006-10-17** (studded-tyre coefficients corrected 2006-09-05; Table A.1 errors corrected 2006-10-17).
- **Source:** publicly hosted at `https://kunskapscentrumbuller.se/documents/Source%20modelling%20report%20final%20rev%20061017.pdf`
- **sha256:** `d087db6416e62d04e48c8759bfc362fd18ab02989c030be593d1e5c482de24b5`
- **Table A.1 input version:** `051229rev` — "Base coefficients to be used in DK Nord 2005".

### Caveat — these are the source coefficients (empirical FORCE check required)

The report states (§2.3.2 / §2.4) that the tabulated `aR/bR/aP/bP` are **intermediate**
values, with a "definite set of coefficients … expected around December 2006".
These Table A.1 values are the **DK Nord 2005** base set. Whether they reproduce
the FORCE `.xls` overall levels **exactly** is an empirical question — plug them
in and compare band-by-band before claiming a verified FORCE numeric Pass. They
supersede the previously-`[ASSUMED]` emission fits either way (real, cited data).

## Equations (Annex A)

- **(A.1) Rolling:** `L_WR(f) = aR(f) + bR(f)·lg(v / v_ref)`, with `v_ref = 70 km/h`.
- **(A.2) Category-3 from Category-2 rolling:** `(aR)_cat3 = (aR)_cat2 + 10·lg(n_axles / 2)`. Tabulated Cat-3 values below refer to **4 axles**.
- **(A.3) Propulsion:** `L_WP(f) = aP(f) + bP(f)·[(v − v_ref) / v_ref]`, `v_ref = 70 km/h`.
- Total sub-source power combines rolling + propulsion energetically (incoherent).

Source heights (equal-power split conventions from the body): normally **0.01 m,
0.30 m, 0.75 m**; propulsion 80% at the highest source, 20% at the lowest (heavy
vehicles with high exhaust assign all propulsion ≤ 315 Hz to the high source).

## Table A.1 — Basic sound power coefficients (DK Nord 2005, input 051229rev)

Categories: **Cat 1** = light/passenger, **Cat 2** = medium-heavy (2 axles),
**Cat 3** = heavy (tabulated for 4 axles; use A.2 for other axle counts).
Frequencies are 1/3-octave nominal centres, 25 Hz–10 kHz (27 bands).

| f (Hz) | R1 aR | R1 bR | R2 aR | R2 bR | R3 aR | R3 bR | P1 aP | P1 bP | P2 aP | P2 bP | P3 aP | P3 bP |
|-------:|------:|------:|------:|------:|------:|------:|------:|------:|------:|------:|------:|------:|
| 25    | 69.9 | 33.0 | 76.5 | 33.0 | 79.5 | 33.0 | 89.8 | 2.0 | 97.0  | 0.0 | 97.7  | 0.0 |
| 31.5  | 69.9 | 33.0 | 76.5 | 33.0 | 79.5 | 33.0 | 91.6 | 2.0 | 97.7  | 0.0 | 97.3  | 0.0 |
| 40    | 69.9 | 33.0 | 76.5 | 33.0 | 79.5 | 33.0 | 91.5 | 0.0 | 98.5  | 0.0 | 98.2  | 0.0 |
| 50    | 74.9 | 30.0 | 78.5 | 30.0 | 81.5 | 30.0 | 92.5 | 0.0 | 98.5  | 0.0 | 103.3 | 0.0 |
| 63    | 74.9 | 30.0 | 79.5 | 30.0 | 82.5 | 30.0 | 96.6 | 2.0 | 101.5 | 0.0 | 107.9 | 0.0 |
| 80    | 74.9 | 30.0 | 79.5 | 30.0 | 82.5 | 30.0 | 94.2 | 2.0 | 101.4 | 0.0 | 105.4 | 0.0 |
| 100   | 79.3 | 41.0 | 82.5 | 41.0 | 85.5 | 41.0 | 92.0 | 4.0 | 97.0  | 0.0 | 101.0 | 0.0 |
| 125   | 82.5 | 41.2 | 84.3 | 41.2 | 87.3 | 41.2 | 87.4 | 2.0 | 96.5  | 0.0 | 101.0 | 0.0 |
| 160   | 81.3 | 42.3 | 84.3 | 42.3 | 87.3 | 42.3 | 86.1 | 2.0 | 95.2  | 0.0 | 101.3 | 0.0 |
| 200   | 80.9 | 41.8 | 84.3 | 41.8 | 87.3 | 41.8 | 86.1 | 6.0 | 99.6  | 0.0 | 101.3 | 0.0 |
| 250   | 78.9 | 38.6 | 87.4 | 38.6 | 90.4 | 38.6 | 87.2 | 8.2 | 100.7 | 8.5 | 102.5 | 8.5 |
| 315   | 78.8 | 35.5 | 88.2 | 35.5 | 91.2 | 35.5 | 86.5 | 8.2 | 101.0 | 8.5 | 103.0 | 8.5 |
| 400   | 80.5 | 31.7 | 92.0 | 31.7 | 95.0 | 31.7 | 85.6 | 8.2 | 98.3  | 8.5 | 102.0 | 8.5 |
| 500   | 87.0 | 25.9 | 94.1 | 25.9 | 97.1 | 25.9 | 80.6 | 8.2 | 94.2  | 8.5 | 101.4 | 8.5 |
| 630   | 88.7 | 26.5 | 96.5 | 26.5 | 99.5 | 26.5 | 80.7 | 8.2 | 92.4  | 8.5 | 99.4  | 8.5 |
| 800   | 90.8 | 32.5 | 96.8 | 32.5 | 99.8 | 32.5 | 78.8 | 8.2 | 93.4  | 12.5 | 95.1 | 8.5 |
| 1000  | 93.3 | 37.7 | 95.6 | 37.7 | 98.6 | 37.7 | 79.3 | 8.2 | 95.5  | 12.5 | 95.8 | 8.5 |
| 1250  | 92.5 | 41.4 | 93.0 | 41.4 | 96.0 | 41.4 | 82.4 | 8.2 | 96.0  | 12.5 | 95.3 | 8.5 |
| 1600  | 92.8 | 41.6 | 93.9 | 41.6 | 96.9 | 41.6 | 83.7 | 8.2 | 93.8  | 12.5 | 92.2 | 8.5 |
| 2000  | 90.4 | 42.3 | 91.5 | 42.3 | 94.5 | 42.3 | 83.4 | 9.5 | 93.4  | 12.5 | 93.2 | 8.5 |
| 2500  | 88.4 | 38.9 | 88.1 | 38.9 | 91.1 | 38.9 | 81.3 | 9.5 | 92.1  | 12.5 | 90.7 | 8.5 |
| 3150  | 85.6 | 39.5 | 86.1 | 39.5 | 89.1 | 39.5 | 81.8 | 9.5 | 90.1  | 12.5 | 88.8 | 8.5 |
| 4000  | 82.7 | 39.6 | 84.2 | 39.6 | 87.2 | 39.6 | 79.9 | 9.5 | 87.9  | 12.5 | 87.5 | 8.5 |
| 5000  | 79.7 | 39.8 | 80.3 | 39.8 | 83.3 | 39.8 | 77.9 | 9.5 | 85.6  | 12.5 | 85.9 | 8.5 |
| 6300  | 75.6 | 40.2 | 77.3 | 40.2 | 80.3 | 40.2 | 75.1 | 9.5 | 85.7  | 8.5 | 86.9 | 8.5 |
| 8000  | 72.0 | 40.8 | 77.3 | 40.8 | 80.3 | 40.8 | 73.1 | 9.5 | 82.6  | 8.5 | 83.8 | 8.5 |
| 10000 | 67.5 | 41.0 | 77.3 | 41.0 | 80.3 | 41.0 | 69.5 | 9.5 | 79.5  | 8.5 | 80.3 | 8.5 |

## Table A.2 — Nordic tyre/road correction to `aR` (Sweden, Norway, Finland ONLY)

**Not applied to Danish roads.** The FORCE cases are Danish (DELTA), so they use
Table A.1 directly without this correction.

| f (Hz) | 250 | 315 | 400 | 500 | 630 | 800 | 1000 | 1250 | 1600 | 2000 | 2500 | 3150 | 4000 | 5000 | 6300 | 8000 | 10000 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| ΔaR | 1.0 | 1.0 | 1.0 | 1.0 | 2.0 | 2.0 | 2.0 | 2.0 | −1.0 | −2.0 | −3.0 | −4.0 | −4.0 | −3.0 | −1.0 | 0.0 | 2.0 |

## Table A.3 — Studded-tyre correction `ΔL_studs(v) = a + b·lg(v/70)`, 50 ≤ v ≤ 90 km/h

Clamped: `ΔL_studs(v<50)=ΔL_studs(50)`, `ΔL_studs(v>90)=ΔL_studs(90)`. Not used
by the (non-studded) FORCE road cases.

| f (Hz) | a | b | | f (Hz) | a | b |
|---:|---:|---:|---|---:|---:|---:|
| 25   | 0.0 | 0    | | 630  | 1.8 | −3.9  |
| 31.5 | 0.0 | 0    | | 800  | 0.8 | −2.7  |
| 40   | 0.0 | 0    | | 1000 | 0.5 | −4.2  |
| 50   | 0.0 | 0    | | 1250 | 0.2 | −11.7 |
| 63   | 0.0 | 0    | | 1600 | −0.2 | −11.7 |
| 80   | 0.0 | 0    | | 2000 | −0.4 | −14.9 |
| 100  | 0.0 | 0    | | 2500 | 0.5 | −17.6 |
| 125  | 0.3 | −4.1 | | 3150 | 0.8 | −21.8 |
| 160  | 1.4 | −6.0 | | 4000 | 0.9 | −21.6 |
| 200  | 1.5 | −8.5 | | 5000 | 2.1 | −19.2 |
| 250  | 0.9 | −4.1 | | 6300 | 5.0 | −14.6 |
| 315  | 1.2 | 1.7  | | 8000 | 7.3 | −9.9  |
| 400  | 1.5 | 0.6  | | 10000 | 10.0 | −10.2 |
| 500  | 1.9 | −4.6 | | | | |

## Integration note (ENVI)

This dataset replaces the previously-`[ASSUMED]` emission fits in
`crates/envi-harness/src/emission/`. On integration, flip
`emission::coefficients::PROVENANCE` from `Assumed` to a `Verified`/cited variant
carrying this document's report id + sha256, which un-gates the FORCE overall-level
comparison in the `run_case` arms (04-03/04-04). Compare by **band index** at the
27 third-octave centres; the honest-green rule still holds — if the numbers do not
land within Ch.6 tolerance (these are the "intermediate" DK Nord 2005 values), keep
the case honest (Fail with the delta report, or a documented accepted gap), never a
forced Pass. Danish cases: no Table A.2 correction.
