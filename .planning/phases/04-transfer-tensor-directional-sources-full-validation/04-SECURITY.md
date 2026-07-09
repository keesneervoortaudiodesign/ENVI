---
phase: 4
slug: transfer-tensor-directional-sources-full-validation
status: verified
threats_open: 0
asvs_level: 1
created: 2026-07-09
---

# Phase 4 — Security

> Per-phase security contract: threat register, accepted risks, and audit trail.
> Register authored at plan time (04-01…04-05 `<threat_model>` blocks); this audit
> verifies each plan-time mitigation exists in the implementation.

---

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| FORCE `.xls` → loaders (`cases/xls.rs`) | operator-controlled workbooks (terrain rows, Coordinates sheets, traffic, reference spectra) | untrusted numeric/text tables |
| refs/ artifacts (workbooks, AV/report PDFs) | fetched copyrighted reference data | sha256-pinned, git-ignored |
| coefficient tables (`emission/coefficients.rs`) | method-defined facts transcribed from the report | verified vs PDF page images |
| case geometry → engine (`SolveJob`, terrain interpretation, balloons) | case-derived source/receiver/profile/directivity | validated at the boundary |
| external oracle tools (Python/scipy, NoiseModelling) | one-time operator-run generators | committed outputs only; not build/test deps |

---

## Threat Register

| Threat ID | Category | Component | Disposition | Mitigation (verified in code) | Status |
|-----------|----------|-----------|-------------|-------------------------------|--------|
| T-04-01-01 | DoS | `solve()` chunk allocation | mitigate | `chunk_receivers` bounds the working set (256 MiB budget, `tensor_budget` high-water test); `chunk_receivers=0` → `DegenerateChunkSize` typed error, no panic | closed |
| T-04-01-02 | Tampering | `put_chunk` dim mismatch | mitigate | `InMemorySink::put_chunk` validates dims + finiteness → `SinkError`, never `unwrap`/panic (tensor.rs:250-268) | closed |
| T-04-01-03 | Tampering | conditioning vector length mismatch | mitigate | `compose_gain`/`readout_*` validate lengths vs `N_BANDS`/`n_sub` → typed `SinkError` | closed |
| T-04-01-04 | Info Disclosure | MAC serving a stale/mismatched tensor | **accept** | hash-keyed tensor identity is a Phase-11/SVC-06 concern; MAC-identity test proves numerical equivalence only — documented forward (see Accepted Risks) | closed |
| T-04-02-01 | Tampering | wrong-lineage coefficients | mitigate | provenance comment on every constant; now `PROVENANCE=Cited` (Table A.1, verified vs page image); Danish lineage → no Table A.2; `emission_force_delta` gates the pipeline | closed |
| T-04-02-02 | Spoofing | corrupt reference dropped into refs/ | mitigate | sha256 pins in `refs/refs.sha256`; the source-modelling report is committed to `docs/references/` with its sha256 recorded in `coefficients.rs` | closed |
| T-04-02-03 | Tampering | malformed balloon grid causing panic | mitigate | validating ctor → typed `DirectivityError` (finiteness / monotonic angles / band length); `eval*` never panic on data | closed |
| T-04-02-04 | DoS | degenerate 1/cosθ at θ→±90° | mitigate | `PASSBY_HALF_SPAN_DEG = 89.0` (never ±90°); balloon size bounded by fixed resolution | closed |
| T-04-02-05 | Repudiation | an [ASSUMED] value silently presented as verified | mitigate | provenance discipline; FORCE cases stay honest `Skipped` (measured-gap reason); `emission_force_delta` documents the gap, never forces a Pass | closed |
| T-04-03-01 | Tampering | SM3 transcription error | mitigate | §5.12 verified vs page image; `oracle_submodel3` + flat-bit-identical regression | closed |
| T-04-03-02 | Tampering | wind screen silently unrefracted | mitigate | `PropagationError::WeatherScreenNotImplemented` typed error (Pitfall 9); test asserts never-silent | closed |
| T-04-03-03 | DoS | degenerate segment geometry / ξ singularity | mitigate | ξ clamps + finiteness sweep across bands × profiles; typed errors, never panic | closed |
| T-04-03-04 | Repudiation | false FORCE Pass masking a gap | mitigate | honest-green: capability/coefficient gaps map to `Skipped`; no value on a Pass path | closed |
| T-04-03-05 | Tampering | stale reference-version drift | mitigate | pinned `.xls` (refs.sha256); 2018 amendments intentionally not implemented | closed |
| T-04-04-01 | DoS | oversized/malformed Coordinates sheet | mitigate | `MAX_SHEETS=200` / `MAX_PROFILE_ROWS=10_000` / `MAX_COORD_ROWS=20_000` caps + `validate_profile` + `confine()` path guard → typed `CaseLoadError`, never panic | closed |
| T-04-04-02 | Tampering | SM11/SM10 transcription error | mitigate | §5.20 verified vs page image; `oracle_submodel11` + coverage assertions (SM10 not pulled forward — Open-Q3(b) accepted gap) | closed |
| T-04-04-03 | Repudiation | quarantined class→(A,B) presented as verified | mitigate | `[ASSUMED]` tags + structure/direction tests only; L_den never a fixed-value oracle | closed |
| T-04-04-04 | Tampering | degenerate image-source path | mitigate | `reflect_over_segment` `valid` flag checked; own-façade reflection excluded; typed errors on degenerate geometry | closed |
| T-04-04-05 | Repudiation | false Pass hiding the forest gap | mitigate | Open-Q3(b) recorded; forest cases honest `Skipped(requires: forest-scattering)` | closed |
| T-04-05-01 | Tampering | truncated/corrupt fixture silently passing | mitigate | coverage assertions (all octave indices); provenance sha256 + scene hash; placeholder rows force a Skip | closed |
| T-04-05-02 | DoS | killing the JVM crashes the editor | mitigate | recipe states NEVER force-close JVM/Gradle/LSP/VS Code; verify grep asserts the warning present | closed |
| T-04-05-03 | Repudiation | equality forced on incomparable models | mitigate | barrier/ground report-only; only divergence + ISO 9613-1 gated | closed |
| T-04-05-04 | Tampering | GPL source translated into the tree | mitigate | outputs-only; cite by report/number; no NoiseModelling code in-repo | closed |
| T-04-SC | Tampering | npm/pip/cargo installs | mitigate | ZERO new packages; `cargo tree -p envi-engine` unchanged (ndarray + num-complex + thiserror); Python/NoiseModelling operator-run only, not build/test deps | closed |

Convention-integrity gate (cross-cutting): **zero actual `.conj()` calls** under
`crates/envi-engine/src/propagation/` (verified — the only matches are
doc-comment statements of the rule); the single convention boundary stays in
`transfer.rs`.

*Status: open · closed* — **all closed.**

---

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-04-01 | T-04-01-04 | Content-hash tensor identity (reject a MAC against a stale/mismatched tensor) is an SVC-06 / Phase-11 service concern. Milestone-1 is engine-only (no service, no cached-tensor reuse across requests); the MAC-identity test proves numerical equivalence. No exploit surface until the Phase-10/11 calculation service caches tensors. | kees | 2026-07-09 |

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-07-09 | 24 | 24 | 0 | Claude (Opus 4.8) — mitigations verified in code |

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter
