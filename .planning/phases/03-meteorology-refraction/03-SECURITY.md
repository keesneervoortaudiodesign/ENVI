# Security Verification — Phase 03: Meteorology & Refraction

**Status:** SECURED
**ASVS Level:** 1
**Block-on:** high
**Threats Closed:** 19 / 19 (13 mitigate verified-in-code + 6 accepted-risk documented)
**Threats Open:** 0
**Verified:** 2026-07-08
**Method:** retroactive, grep + code-read + targeted test-run. Implementation files were **not modified** (no gaps found). Every `mitigate` threat is verified by locating the mitigation call in the shipped code and confirming its accompanying test is live (`cargo test -p envi-engine -p envi-harness --lib` = 70 harness + engine lib tests green).

---

## Scope

Threat registers extracted from the three plan `<threat_model>` blocks:
- `03-01-PLAN.md` — refraction core (T-03-01-01/02/03/04/SC + n/a)
- `03-02-PLAN.md` — CalcEqSSPGround, weather routes, reflection split (T-03-02-01/02/03/04/SC + n/a)
- `03-03-PLAN.md` — F_τ coherence, Routes 1/3, capability flip (T-03-03-01/02/03/04/05/SC + n/a)

Changed source surface audited: `crates/envi-engine/src/propagation/{coherence.rs, refraction/*.rs, terrain_effect/*.rs}` and `crates/envi-harness/src/{weather/*.rs, capability.rs, cases/*, lib.rs}`.

---

## Threat Verification (mitigate)

| Threat ID | Category | Disposition | Verdict | Evidence (file:line) |
|-----------|----------|-------------|---------|----------------------|
| T-03-01-01 | Denial of Service | mitigate | CLOSED | `refraction/eqssp.rs:31` (`XI_HOMOGENEOUS = 1e-6`) + `:182` homogeneous shortcut; `refraction/profile.rs:14,22` z₀ clamp ≥0.001; `refraction/shadow_zone.rs:57-67,73,96` finite/edge guards; finiteness asserted in `shadow_zone.rs:152-163` |
| T-03-01-02 | Tampering (degenerate profile / no-root cubic) | mitigate | CLOSED | `refraction/eqssp.rs:103-117,169-180` typed `DegenerateProfile` before any div/sqrt; `PropagationError::{NoReflectionRoot, DegenerateProfile, DegenerateShadowZone}` (03-01 SUMMARY provides); tests `eqssp.rs:339-353`, `shadow_zone.rs:189-201` |
| T-03-01-03 | Tampering (supply chain) | mitigate | CLOSED | `cargo tree -p envi-engine` = `ndarray + num-complex + thiserror` only (run 2026-07-08) |
| T-03-01-04 | Information Disclosure (AV/FORCE copyright) | mitigate | CLOSED | `.gitignore:4-6` `refs/*` ignored except `fetch.sh`/`refs.sha256`; `git ls-files refs/` = those two files only; oracle scripts carry equations + citations, not document text |
| T-03-02-01 | Denial of Service (degenerate d/hS/hR, NaN/Inf met) | mitigate | CLOSED | `refraction/eqssp.rs:249-251` (`d.min(400.0)`, `h_s.max(0.5)`, `h_r.max(0.5)` for Eqs. 24/25); `:224-236` non-finite/non-positive f/d/σ typed-rejected; z₀ clamp `weather/route2.rs:98`, `route3.rs:114`; test `eqssp.rs:449-464` |
| T-03-02-02 | Tampering (malformed FORCE met) | mitigate | CLOSED | `weather/route2.rs:45-54` `finite_or` typed `CaseLoadError::NonFinite`; `:73-78` `zu>0` rejected; `:85-96` negative su/sdtdz rejected; `:150-166` final finiteness guard; tests `:277-326` |
| T-03-02-03 | Spoofing / correctness ([ASSUMED] A/B constants) | mitigate | CLOSED | `weather/route2.rs:7-26` + `weather/mod.rs:11-18` `[ASSUMED]` doc-banners; `route2.rs:142` isotropic-temp part `[ASSUMED] 0`; validated by direction tests only (`:227-254`); no FORCE numeric Pass (see T-03-03-03) |
| T-03-02-04 | Tampering (new dependency / linalg) | mitigate | CLOSED | `cargo tree -p envi-engine` unchanged; `grep nalgebra\|ndarray-linalg` over all `Cargo.toml`/`Cargo.lock` = no matches |
| T-03-03-01 | Tampering / DoS (Route 1 malformed .xls / probabilities) | mitigate | CLOSED | `weather/route1.rs:45` `MAX_STAT_ROWS` row cap; `:109-147` probabilities non-finite/negative/zero-sum rejected + normalized; `:230-334` label-anchored read, typed `Workbook`/`MissingLabel`/`NonFinite`, fail-soft when refs absent; tests `:402-421,456-479` |
| T-03-03-02 | Denial of Service (Route 3 singular LSQ / bad met) | mitigate | CLOSED | `weather/route3.rs:157-182` `solve_3x3` determinant/scale singular guard → `None` → typed `Invalid`; `:100-118` non-finite met + `zu>0` rejected; `:114,214` z₀ clamp; tests `:286-312` |
| T-03-03-03 | Spoofing / correctness ([ASSUMED] constants / false FORCE Pass) | mitigate | CLOSED | Checkpoint resolved option (c) (03-03 SUMMARY); `lib.rs:71-75` refraction `run_case` arm returns `Skipped` (no committed reference); `capability.rs:230-262` wind/gradient FORCE `missing == {EmissionModel}` (stays Skipped, never Pass); `route1.rs:18-33`/`route3.rs:19-26` `[ASSUMED]` banners |
| T-03-03-04 | Tampering (linalg crate for Route 3) | mitigate | CLOSED | hand-rolled 3×3 Cramer `weather/route3.rs:157-182`; no `nalgebra`/`ndarray-linalg` anywhere (grep = 0); `cargo tree -p envi-engine` unchanged |
| T-03-03-05 | Information Disclosure (class-table / AV copyright) | mitigate | CLOSED | class-table values read from git-ignored SHA-pinned `refs/TestYearlyAverage.xls` (`.gitignore:4-6`); equations cited by report number; no PDF text committed |

**Cross-cutting invariant checks (all green):**
- `.conj()` grep gate over `crates/envi-engine/src/propagation/` = **0 real calls** (5 hits are doc-comments in `special.rs`, `refraction/{shadow_zone,circular_ray,mod}.rs`); `coherence_f_delta_nu` is real-valued (`coherence.rs:103-108`) and F_τ multiplies `f_delta_nu` without touching the `+j` phase.
- F_τ uses the full `2π` argument, not `0.23π` (`coherence.rs:66-75,103-108`; Pitfall 5) — pinned by test `coherence.rs:248-263`.
- Band-index comparison on the 105-point grid preserved in CalcEqSSPGround (`eqssp.rs:270-279`, test `:378-417`).

---

## Accepted Risks Log

| Threat ID | Category | Disposition | Rationale (verified) |
|-----------|----------|-------------|----------------------|
| T-03-01-SC | Tampering (npm/pip/cargo installs) | accept | No package installs in Phase 3. `tech-stack.added: []` in all three SUMMARYs; `cargo tree -p envi-engine` unchanged; no new harness crate (calamine pre-existed since Phase 1). |
| T-03-02-SC | Tampering (package installs) | accept | Same as above — zero packages added this phase. |
| T-03-03-SC | Tampering (package installs) | accept | Same as above — zero packages added this phase. |
| T-03-01 (n/a) | Spoofing/Repudiation/Elevation | accept | Offline pure-math library. No auth/session/network/privilege surface exists in `envi-engine`/`envi-harness`; no high-severity web/authz threat applies. |
| T-03-02 (n/a) | Repudiation/Info-Disclosure/Elevation | accept | Same — offline numeric library, no such surface. |
| T-03-03 (n/a) | Repudiation/Elevation | accept | Same — offline numeric library, no such surface. |

---

## Unregistered Flags

None. No SUMMARY file declares a `## Threat Flags` section. The documented plan deviations
(`CaseKind::Refraction` variant, `calc_eq_ssp_ground` taking `σ` instead of `Ẑ_G`,
`SoundSpeedProfile::{s_a,s_b}` fields, the `run_case` refraction-arm message) are internal
interface adjustments over already-modelled trust boundaries (case/oracle files, FORCE met,
weather-route coefficients). They introduce **no new untrusted-input surface** and each remains
covered by an existing threat's mitigation. No new attack surface appeared during implementation.

---

## Result

All 19 threats across the three Phase-3 plans resolve to CLOSED (13 mitigations verified present
and live in code; 6 accepted risks documented). Zero open threats. No implementation change was
required. Phase 3 clears the security gate (block-on: high — no high-severity open threat).
