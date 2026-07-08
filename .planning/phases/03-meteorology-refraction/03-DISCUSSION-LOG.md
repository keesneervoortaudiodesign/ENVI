# Phase 3: Meteorology & Refraction - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-08
**Phase:** 3-meteorology-refraction
**Areas discussed:** Validation gate strategy, Weather-route depth, Shadow-zone depth, F_τ turbulence coherence

---

## Validation gate strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Oracle+anchor ladder (as Phase 2) | Homogeneous-limit exactness anchor (\|ξ\|<clamp reproduces Phase 2 bit-for-bit) + committed scipy oracle for ξ/Δτ/equiv-linear collapse + property tests for direction. FORCE up/downwind stay capability-gated until the Phase-4 emission model. | ✓ |
| Direct FORCE refraction comparison now | Attempt numeric match against FORCE up/downwind reference spectra in Phase 3. | |

**User's choice:** Oracle+anchor ladder (as Phase 2)
**Notes:** Consistent with the frozen acceptance ladder; avoids a false pass since FORCE reference spectra need the Phase-4 road-emission model.

---

## Weather-route depth

| Option | Description | Selected |
|--------|-------------|----------|
| Route 2 + Route 1 now, defer Route 3 to v2 | Build the FORCE-driven u/dt-dz→A/B path + Route 1 weather-class energy-weighting now; defer Monin–Obukhov reconstruction (MET-06) to v2 METX. | |
| All three routes now (roadmap as-written) | Implement Route 1 + Route 2 + Route 3 incl. Monin–Obukhov reconstruction and LSQ A/B/C fit, per MET-06 in Phase 3. | ✓ |
| Route 2 only now | Just the FORCE u/dt-dz → A/B path; defer Route 1 and Route 3. | |

**User's choice:** All three routes now (roadmap as-written)
**Notes:** MET-06 stays in Phase 3. Because no weather API exists yet (v2 METX), Route 3's LSQ A/B/C fit is validated against synthetic/derived surface-met inputs + a committed oracle, with the fit boundary shaped so the v2 weather-import path plugs in without a rewrite.

---

## Shadow-zone depth

| Option | Description | Selected |
|--------|-------------|----------|
| Full AV 1106/07 shadow-zone model | Implement Nord2000's own upward-refraction / shadow-zone attenuation treatment as specified. | ✓ |
| Equivalent-linear natural behavior + documented limit | Rely on the circular-ray model's inherent behavior into shadow, with a stated accuracy limit. | |

**User's choice:** Full AV 1106/07 shadow-zone model
**Notes:** Success criterion 1 explicitly requires shadow-zone cases to match — it's in scope, not optional.

---

## F_τ turbulence coherence

| Option | Description | Selected |
|--------|-------------|----------|
| Extend F-chain via existing seam, property-test direction | F_τ enters through the FΔν/F-chain seam feeding the two-channel P_incoh readout; Cv²/Ct² from FORCE-case fields (Nord2000 defaults when absent); validate by property tests. | ✓ |
| Pin F_τ against a scipy oracle | Add a committed scipy oracle fixture for F_τ with fixed Cv²/Ct² and pin values numerically. | |

**User's choice:** Extend F-chain via existing seam, property-test direction
**Notes:** Consistent with the frozen two-channel contract and how Phase 2 handled Fc/Fr (exact turbulence constants are AV Assumptions — inert on zero-turbulence cases, untrustworthy to pin).

---

## Claude's Discretion

- Module layout under `propagation/` for the new refraction code, the exact ξ clamp threshold value, and the Route-3 LSQ solver choice — planner/researcher decide, subject to the frozen conventions.
- Exact synthetic surface-met fixtures and oracle-fixture granularity for Route 3.

## Deferred Ideas

- Live weather-API ingestion (Open-Meteo runtime, ERA5/CDS weather-class statistics) — v2 METX milestone.
- Full FORCE road-traffic numeric pass (VAL-02) — Phase 4, needs the road-emission model.
