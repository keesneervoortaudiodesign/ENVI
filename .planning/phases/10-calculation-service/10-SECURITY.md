# Phase 10 — Calculation Service: Security Verification

**Verified:** 2026-07-12
**Method:** Retroactive threat-mitigation verification against implemented code (not documentation/intent).
**ASVS Level:** 1
**Block-on:** high
**Scope:** the `<threat_model>` blocks declared in `10-0{1..6}-PLAN.md` — verified per-threat by disposition.
**Read-only:** implementation files were not modified; only this SECURITY.md was written.

**Deferral note (D-12):** real authentication / login / sessions are OUT of Phase-10 scope — only
COOP/COEP headers + bundle serving land here. Missing auth is an accepted, documented deferral and is
NOT counted as a Phase-10 gap.

---

## Verdict: SECURED

**Mitigation coverage: 31 / 31 threats CLOSED** (27 `mitigate` + 4 `accept`). No BLOCKER, no PARTIAL,
no MISSING. No unregistered attack-surface flags.

---

## Threat Verification

### Plan 10-01 — envi-compute core (identity / cost / tiers / job-assembly)

| Threat ID | Category | Disposition | Verdict | Evidence |
|-----------|----------|-------------|---------|----------|
| T-10-01-01 | Tampering | mitigate | COVERED | `crates/envi-compute/src/identity.rs:400-413` — `tensor_hash_matches_frozen_pre_refactor_digest` pins a hard-coded blake3 digest (`dcb2485e…ee2c`) over a fixed input; a silent re-encoding after the move flips the test. Frozen byte encoding (`envi-tensor-hash-v1`, u64-LE lengths, `to_bits` floats) at `identity.rs:169-263`. |
| T-10-01-02 | DoS | mitigate | COVERED | `crates/envi-compute/src/cost.rs:149-186` — `guardrail` returns `Block` on `tensor_bytes > budget_bytes`; estimate is pure arithmetic, no tensor allocation (`cost.rs:81-120`). Test `large_grid_byte_math_does_not_overflow_u32_and_still_blocks` (`cost.rs:233-256`) proves the `u64` saturating math (WR-02) blocks an overflowing grid instead of wrapping under budget. |
| T-10-01-03 | Tampering | mitigate | COVERED | `crates/envi-compute/src/job_assembly.rs` — `grep conj` = **0 matches**; phase supplied raw as `directivity_phase_rad = has_phase().then(|| eval_phase(dir_local))` (`job_assembly.rs:128-151`); magnitude-only balloon → `None` (bit-identical), asserted `job_assembly.rs:378-379`. The single `.conj()` boundary stays in the engine. |
| T-10-01-04 | Info disclosure | accept | COVERED (accepted) | See Accepted Risks A1. Verified: `crates/envi-compute/Cargo.toml` deps are `envi-engine, serde, serde_json, geojson, uuid, blake3, thiserror, ts-rs` — no network, no `std::fs`, no secrets, no C-linked crate. WASM-safe pure crate; no runtime exfil surface. |
| T-10-01-SC | Tampering (supply-chain) | mitigate | COVERED | `crates/envi-compute/Cargo.toml` — no `tempfile`, no C-linked crate; only path + pre-audited registry crates. `crates/envi-engine/Cargo.toml` quarantine intact: exactly `ndarray + num-complex + thiserror`. |

### Plan 10-02 — Cross-origin isolation headers (envi-service + Vite)

| Threat ID | Category | Disposition | Verdict | Evidence |
|-----------|----------|-------------|---------|----------|
| T-10-02-01 | Info disclosure (Spectre/SAB) | mitigate | COVERED | `crates/envi-service/src/api/mod.rs:132-144` — `SetResponseHeaderLayer::overriding` emits `cross-origin-opener-policy: same-origin` + `cross-origin-embedder-policy: credentialless`, wrapping the whole router. This is the platform-required isolation mitigation. |
| T-10-02-02 | Info disclosure (COEP misconfig) | mitigate | COVERED | `api/mod.rs:136-139` — COEP value is the literal `credentialless` (`HeaderValue::from_static`), never `unsafe-none`; a oneshot contract test pins the exact header value (plan gate). |
| T-10-02-03 | DoS (require-corp breaks fetches) | mitigate | COVERED | `api/mod.rs:139` — `credentialless` chosen, `require-corp` absent (grep clean in both `api/mod.rs` and `web/vite.config.ts`); rationale documented at `api/mod.rs:117-129`, preserving Phase-8 direct basemap/AHN/Overpass fetches. |
| T-10-02-04 | Elevation of priv (premature auth) | accept | COVERED (accepted) | See Accepted Risks A2. Verified: Phase-10 server change is headers only — `git log` confirms the `/projects/{id}/calculations` route is Phase-6 (`feat(06-04)`), not added here; no new endpoint, no auth code. |

### Plan 10-03 — envi-compute-wasm cdylib + OPFS sink + threaded build

| Threat ID | Category | Disposition | Verdict | Evidence |
|-----------|----------|-------------|---------|----------|
| T-10-03-01 | Tampering (OPFS path keys) | mitigate | COVERED | `web/src/compute/opfs.ts:58-104` — `safeSeg` strips `/ \ .. NUL`, `assertHex` throws on any non-`[0-9a-f]` hash, `chunkFileName` rejects non-integer indices; `channelDir` composes the path from only `{projects, uuid, calc, hex-hash, fixed channel}`. Key source is `envi_compute_wasm::identity::marshalled_tensor_hash` (64-char lowercase hex), not free text. |
| T-10-03-02 | DoS (malformed put_chunk) | mitigate | COVERED | `crates/envi-compute-wasm/src/opfs_sink.rs:162-198` — replicates InMemorySink gates returning typed `SinkError::{ChannelShapeMismatch, BandCountMismatch, SubCountMismatch, ChunkOutOfBounds, NonFinite}`; no `unwrap/expect/panic!` in the `put_chunk` data path (the `unwrap`s at `:400-457` are in `#[cfg(test)]` read-back/assert code). Negative tests `opfs_sink.rs:505-560`. |
| T-10-03-03 | Tampering (byte-format integrity) | mitigate | COVERED | `opfs_sink.rs` round-trip tests (`:450-483`) assert index-for-index equality vs `InMemorySink` for full + sliced chunk views (frozen `[s][r_local][f]` interleaved-LE). |
| T-10-03-04 | DoS (atomics RUSTFLAGS leak) | mitigate | COVERED | No repo-root `rust-toolchain.toml`; no `.cargo/config.toml` (so no `[build]` atomics leak). Threaded toolchain scoped behind the `threads` cargo feature (`crates/envi-compute-wasm/Cargo.toml:84`) + the single `build:wasm:compute` npm script. |
| T-10-03-05 | Tampering (wasm-bindgen drift) | mitigate | COVERED | `crates/envi-compute-wasm/Cargo.toml:36` — `wasm-bindgen = "=0.2.126"` exact pin; `wasm-bindgen-rayon 1.3.0` caret dep satisfied by the pin (documented `:58-59`). |
| T-10-03-SC | Tampering (new installs) | mitigate | COVERED | Only `wasm-bindgen-rayon` + `rayon` added, both OK-verdict in the RESEARCH Package Legitimacy Audit; no `[ASSUMED]`/`[SUS]` package. Engine tree untouched. |

### Plan 10-04 — rayon pool + worker + OPFS glue + calc store

| Threat ID | Category | Disposition | Verdict | Evidence |
|-----------|----------|-------------|---------|----------|
| T-10-04-01 | DoS (runaway grid) | mitigate | COVERED | RSS bounded by `workers × chunk`, proven natively by `pool.rs:547-611` `sc3_peak_resident_chunks_never_exceed_n_workers`; the `cost::guardrail` Block + OPFS quota (`opfs.ts` `fitsQuota`) gate before submit; cooperative abort (`pool.rs:130-132`) frees a stuck run. |
| T-10-04-02 | Tampering (OPFS path injection) | mitigate | COVERED | Same `safeSeg`/`assertHex`/`chunkFileName` hex-only guard as T-10-03-01 (`web/src/compute/opfs.ts:58-104`). |
| T-10-04-03 | Tampering (SAB/atomics misuse) | mitigate | COVERED | `crates/envi-compute-wasm/src/lib.rs:93` — single `static CANCEL: AtomicBool` (SeqCst), flipped only by `request_cancel` (`:97-100`); no other cross-thread mutable state (disjoint files, `pool.rs` doc `:8-10`). Worker asserts `crossOriginIsolated` before `initThreadPool` (`worker.ts:132,295-300`). |
| T-10-04-04 | Spoofing (worker msg origin) | mitigate | COVERED | `web/src/compute/worker.ts:61-63,278-288` — inbound is a typed `WorkerInbound` union; the handler acts only on `"submit"`/`"cancel"` and silently ignores anything else (no arbitrary code path). |
| T-10-04-05 | DoS (unbounded job growth) | mitigate | COVERED | `worker.ts:106-114,130-139` — single in-flight job; a new `submit` calls `reset_cancel()` and supersedes; `prepare_solve` replaces the prior `PREPARED` scene. |
| T-10-04-06 | Repudiation (false green) | mitigate | COVERED | `worker.ts:99-104,132-136` — honest `CAPABILITY_FAILURE` state (not a generic `failed`); cancel keeps completed tiers (`:175-198`); `failed` carries the real reason (`:238-240`). |

### Plan 10-05 — CalcPanel UI + offline Playwright UAT

| Threat ID | Category | Disposition | Verdict | Evidence |
|-----------|----------|-------------|---------|----------|
| T-10-05-01 | Tampering (DOM injection) | mitigate | COVERED | `web/src/panels/CalcPanel.tsx` — `grep dangerouslySetInnerHTML|innerHTML` = **0**; all dynamic strings (disabled reason, guardrail detail, counts) render as React text children (`:487-509`). |
| T-10-05-02 | DoS (runaway grid from UI) | mitigate | COVERED | `CalcPanel.tsx:412-429` — `canRun` requires `!guardrail?.blocked && crossOriginIsolated`; Run is `disabled={!canRun}` (`:493`) with an always-shown `calc-disabled-reason` (`:428-429,507-509`) — never a silent block. |
| T-10-05-03 | Repudiation (false green/stall) | mitigate | COVERED | `CalcPanel.tsx:453-454` — distinct `calc-capability-error` banner (`role="alert"`) forces `canRun=false` when not isolated; honest terminal states via the frozen `JobStatus` vocabulary. |
| T-10-05-04 | Info disclosure (test egress) | mitigate | COVERED | `web/tests/e2e/calc.spec.ts:43-54` — `page.route("**/*", route => … route.abort())` intercepts ALL network; asserts `self.crossOriginIsolated === true` (`:69,114`), drives `calc-run`/`calc-abort`, asserts `calc-cancelled` + `calc-estimate` — offline zero-egress proof. |

### Plan 10-06 — real solve_chunk_range + scene marshalling + OPFS runtime

| Threat ID | Category | Disposition | Verdict | Evidence |
|-----------|----------|-------------|---------|----------|
| T-10-06-01 | Tampering/Info (OPFS chunk key) | mitigate | COVERED | `web/src/compute/opfs.ts:58-104,199-205` — `assertHex` on the tensor-hash segment, fixed literal channel dirs, integer `chunkFileName`; synchronous `openChunk(path)` only returns a handle the worker pre-opened through the guarded path. No raw scene string reaches `getDirectoryHandle`. |
| T-10-06-02 | DoS (scene/receiver size) | mitigate | COVERED | Pre-run `cost::guardrail` Block gates Run (T-10-05-02); WR-01 coverage gate `scene.rs:399-411` returns a typed `Range` error before any slice/alloc; `build_balloon` bounds grid size via `saturating_mul` + shape check (`scene.rs:121-154`). |
| T-10-06-03 | Tampering (DTO deserialization) | mitigate | COVERED | All request DTOs `#[serde(deny_unknown_fields)]` (`crates/envi-compute-wasm/src/dto.rs:43,102,226,250,287,301,322,335,347,362,379`), tests `:517-548`. Finiteness checks `scene.rs:81-94,98-117,157-171,191-259`; validating engine constructors (`TerrainProfile::try_from`, `Atmosphere::new`, `IsolationSpectrum::try_from`, `ForestCrossing::new`, `DirectivityBalloon::new/new_with_phase`) surface `ComputeError::Prepare`, never panic — proven by `build_rejects_degenerate_terrain_without_panicking` (`scene.rs:945-986`). |
| T-10-06-04 | DoS/Tampering (prepared-scene static + cancel) | mitigate | COVERED | `lib.rs:78` — `static PREPARED: RwLock<Option<PreparedScene>>`; write-locked once per submit (`prepare_solve` `:284-292`), read-locked per solve; poisoned lock → typed error, not panic; `tensor_hash` mismatch → `ComputeError::HashMismatch` (`:328-330`, test `:459-505`). `CANCEL` independent; abort short-circuits before file open (`:322-324`). |
| T-10-06-05 | Spoofing (worker msg origin) | accept | COVERED (accepted) | See Accepted Risks A3. Verified: dedicated worker handles only `postMessage` from its spawning client; typed `WorkerInbound` schema (`worker.ts:61-63,278-288`); no cross-origin message handling. Real auth deferred (D-12). |
| T-10-06-SC | Tampering (new installs) | accept | COVERED (accepted) | See Accepted Risks A4. Verified: no new package-manager install in this plan — reuses `envi-compute`/`envi-engine`/`rayon`/`wasm-bindgen(-rayon)`. |

---

## Accepted Risks Log

| ID | Threat | Rationale | Verifiable evidence |
|----|--------|-----------|---------------------|
| A1 | T-10-01-04 — envi-compute has no runtime trust surface | Pure-Rust WASM-safe crate; no network, no `std::fs`, no secrets, cannot exfiltrate. | `crates/envi-compute/Cargo.toml` dependency set (no fs/network/C crate); engine 3-dep quarantine intact. |
| A2 | T-10-02-04 — no auth added in Phase 10 | Real auth/login/sessions are a deferred phase (D-12) with its own threat model; Phase 10 lands COOP/COEP headers + bundle serving only. | `crates/envi-service/src/api/mod.rs` — headers-only change; `git log` shows `/calculations` route is Phase-6, no new endpoint or auth in Phase 10. |
| A3 | T-10-06-05 — worker message origin | The dedicated worker only receives `postMessage` from its spawning client; a typed schema is enforced and unknown messages ignored; no cross-origin handler exists. | `web/src/compute/worker.ts:61-63,278-288`. |
| A4 | T-10-06-SC — no new supply-chain surface | This plan installs no new package-manager dependency; all crates were pre-audited in 10-RESEARCH. | `git diff` of plan 10-06 adds no new registry crate; `cargo tree -p envi-engine` unchanged. |

---

## Unregistered Flags

**None.** SUMMARY `## Threat Flags` (present only in `10-02-SUMMARY.md`) reports "None — no new
security-relevant surface beyond the plan's `<threat_model>`." No new attack surface appeared during
implementation that lacks a threat mapping.

---

## Cross-cutting invariants verified

- **Engine byte-identical (D-02):** `crates/envi-engine/Cargo.toml` = `ndarray + num-complex + thiserror`
  only; no engine source in any Phase-10 `files_modified` — underpins the "unchanged FORCE-validated
  solve" trust assumption across T-10-01-03, T-10-03-02, T-10-06-03/04.
- **Single `.conj()` boundary:** `grep conj crates/envi-compute/src/job_assembly.rs` = 0; the ENVI
  `e^{+jωt}` convention boundary stays in `transfer.rs` (the two-channel `H_coh`/`P_incoh` contract
  cannot be violated at the assembly seam).
- **No server solve endpoint:** the only Phase-10 server surface is the COOP/COEP header layer; the
  solve is fully client-side.
- **Toolchain isolation:** no root `rust-toolchain.toml`, no `.cargo/config.toml` atomics leak;
  nightly/atomics scoped to the `threads` feature + one npm script.

---

*Phase 10 — Calculation Service. Every declared threat resolves to CLOSED (verified in code) or a
documented accepted risk. Status: **SECURED**.*
