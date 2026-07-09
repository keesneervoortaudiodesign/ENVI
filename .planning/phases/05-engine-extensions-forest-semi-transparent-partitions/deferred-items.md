# Deferred items — Phase 05

Out-of-scope discoveries and pre-authorized deferrals logged during plan
execution (not implemented by the owning plan). Mirrors the Phase-04
deferred-items discipline.

## 1. Forest `Fs` coherence factor (Eq. 288) — decorrelation seam, deferred

- **What it is.** Nord2000 Sub-Model 10 also *decorrelates* the through-forest
  field via `Fs = 1 − k_f·T` (AV 1106/07 Eq. 288). `Fs` enters the overall
  coherence product `F = Ff·FΔν·Fc·Fr·Fs` (Eq. 110) and the screen coherence
  factors `F₂/F₃/F₄` (also Eqs. 118/177) — i.e. it belongs to the *coherence*
  machinery, not the excess-attenuation channel.
- **What Phase 5 shipped instead.** ENG-09 implements the SM10 **excess
  attenuation** `ΔL_s` only (Eqs. 289–291, Tables 8/9), applied solver-side as a
  real two-channel magnitude factor. `Fs` is **not** applied.
- **Why deferred (pre-authorized, D-03).** D-03 locked the Phase-5 forest scope to
  "excess attenuation, not decorrelation". There are **no FORCE forest cases** that
  gate on `Fs` (cases 121–124 stay capability-gated on the road-emission blocker),
  and touching the `propagation/` coherence paths was never in the locked design.
- **The seam (lands without structural change).** `CoherenceInputs` already carries
  the caller-multiplied `f_delta_nu` factor; an `Fs`-shaped **multiplicative
  coherence input** drops into the same seam. The screen `F₂/F₃/F₄` factors in
  `run_four_path`/`run_eight_path` (`terrain_effect/screen.rs`) need the same
  multiply. No new operator, no new conj.
- **Impact until wired.** Through-forest interference dips are modeled **too
  coherent** (the field keeps more phase correlation than Nord2000 intends). Benign
  for the excess-attenuation magnitude; matters only for coherent dip depth behind
  forests.
- **Revisit point.** Phase 9/10, when real forest path-crossings exist
  (`ForestCrossing` geometry extraction, Fig. 29) — **with a user check-in** before
  touching the coherence product.

## 2. D-01 `ForestCrossing` interface amendment — record

The `ForestCrossing` input interface was amended during 05-01 (research-mandated,
pre-authorized in the plan):

- **Ships:** `{ d_m, density_per_m2, stem_radius_m, absorption, height_m }`.
- **`kp` DROPPED** — it is Table 8's *computed* `k_f`, not a caller input (D-02:
  "the engine owns the formula").
- **`height_m` ADDED** — `h′ = nQ·h` (Eq. 290) is un-evaluable without the average
  tree height (Pitfall 5).

Downstream consumers (Phase-7 SCN-04 forest object, Phase-9 geometry) must carry
`height_m` and must NOT pass `kp`.

## 3. Q3 — Nord2000 default forest parameters (no verified source)

Sensible defaults for mean tree density / mean stem radius / height / absorption
have **no verified Nord2000 source** yet. Carry to **Phase 7 (SCN-04)** research —
the forest scene object needs documented defaults (or an explicit "user must
supply" contract). Do not invent unsourced constants in the engine.

## 4. Q4 / D-11 — one isolation spectrum per job

The ENG-10 seam carries **a single `IsolationSpectrum` per `SolveJob`** = the total
crossed-partition stack. Deferred upstream concerns:

- **Multi-partition composition** (`T₁·T₂·…` when a path crosses several
  partitions) — a Phase-9 **GEOX** path concern.
- **Per-façade `R(f)` selection** (SCN-02: which building façade's spectrum applies
  to a given crossing) — Phase-7/9 scene+geometry logic. The engine already applies
  *whichever* spectrum the job carries (D-11), so no engine change is needed when
  this lands; only the caller-side selection.

## 5. Directional-phase harness wiring (Phase-4 carry-forward, still open)

Unchanged from `04/deferred-items.md`: `DirectivityBalloon` carries an optional
per-band phase grid and the solver applies it via `SolveJob::directivity_phase_rad`
(coherent channel only), but **no harness `SolveJob` construction site populates it
yet**. Wire it when the coherent directional-source composition path lands
(Milestone 2, calc/results). See the Phase-4 note for the exact seam and the
end-to-end test to add.
