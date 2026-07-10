---
phase: 07
name: frontend-shell-scene-editing
asvs_level: 1
block_on: high
threats_total: 39
threats_verified: 39
threats_open: 0
accepted_risks: 3
status: secured
verified_at: 2026-07-10
verifier: gsd-secure-phase (retroactive, code-verified against current tree post code-review + simplify)
---

# Phase 7 — Security Verification (SECURITY.md)

**Phase:** 07 — frontend-shell-scene-editing
**ASVS enforcement level:** 1 · **Block-on:** high
**Threats closed:** 39/39 · **Open (blocker):** 0

Every declared threat in the ten plan `<threat_model>` blocks (T-07-01-xx … T-07-10-xx) was
verified by reading the CURRENT code — not plan text, not SUMMARY, not the code-review narrative.
The tree moved twice since the plans (a code-review fix pass and a simplify pass); each mitigation
below was confirmed present at a concrete `file:line` in the tree as it stands. Grep-negative checks
(XSS, secrets, external assets) were run against `web/src` / `web/index.html`, not the built bundle.

---

## 1. Threat Verification (by declared disposition)

### `mitigate` threats — mitigation located in code

| Threat ID | Category | Evidence (file:line) | Verdict |
|-----------|----------|----------------------|---------|
| T-07-01-01 | Tampering / DoS | `crates/envi-store/src/interpolate.rs:114-124` — length gate (`BadBandCount`) + finiteness gate (`NonFinite`) BEFORE the `[0;105]` alloc/loop | CLOSED |
| T-07-01-02 | Tampering | `dto.rs:300-309` `TryFrom<&IsolationSpectrumDto>` → `interpolate()` → `IsolationSpectrum::new(dense)` mapped to `StoreError::Engine`; `interpolate.rs:44-51` + test `does_not_clamp_out_of_range:268` prove the core does NOT clamp, so an R>1000 reaches the constructor and is rejected | CLOSED |
| T-07-01-03 | Tampering | `dto.rs:338-353` `TryFrom<&ForestParamsDto>` → `ForestCrossing::new(...)` mapped to `StoreError::Engine` (negative density / non-positive radius+height rejected) | CLOSED |
| T-07-01-04 | Elevation of Privilege | `crates/envi-engine/Cargo.toml` `[dependencies]` = exactly `ndarray, num-complex, thiserror` — no serde/ts-rs/spade reached the engine | CLOSED |
| T-07-02-01 | DoS (panic) | `crates/envi-dgm/src/tin.rs:235-238` — `can_add_constraint(from,to)` pre-check gates the sole `add_constraint`; interior crossing returns `IntersectingConstraint`, never panics; tests `interior_crossing_breaklines…:344`, `self_intersecting_breakline…:372` | CLOSED |
| T-07-02-02 | DoS | `tin.rs:159-174` — `MAX_POINTS`/`MAX_BREAKLINE_VERTICES` = 500_000 caps checked BEFORE any O(n log n) work; test `oversized_point_set_is_rejected:383` | CLOSED |
| T-07-02-03 | Tampering | `tin.rs:176-192` finiteness rejection before insert; `tin.rs:206-211` `TooFewPoints` for <3 non-collinear; tests `non_finite…:325`, `all_collinear…:310` | CLOSED |
| T-07-02-04 | Elevation of Privilege | `crates/envi-dgm/Cargo.toml` `[dependencies]` = `spade, thiserror` only — no `envi-engine` edge, so spade cannot reach the engine quarantine; no C crates (`gdal`/`proj`) | CLOSED |
| T-07-03-01 | DoS | `interpolate.rs:114-117` length gate before alloc; body cap documented `api/mod.rs:9-14` (~2 MB axum default) | CLOSED |
| T-07-03-02 | Tampering | `api/meta.rs:120-132` handler delegates to `interpolate()` then `IsolationSpectrum::new`; both faults surface as `BadRequest` via `From<StoreError>` — never a clamped-wrong 200 | CLOSED |
| T-07-03-03 | DoS | `api/dgm.rs:73-78` delegates to `build_tin` (caps in-crate); `From<DgmError>` → 400 | CLOSED |
| T-07-03-04 | DoS (panic) | `error.rs:106-127` — EVERY `DgmError` variant maps to `BadRequest`(400); thread never aborts | CLOSED |
| T-07-03-05 | Information Disclosure | `error.rs:80-101` — `Io`/`Json`/`PathEscape` logged server-side, return generic `"internal error"`; validation faults carry only safe text; `DgmError` `Display` carries coords/counts only (no paths) | CLOSED |
| T-07-04-01 | Tampering | `crates/envi-service/tests/wire_no_drift.rs` present — generated `wire.ts` no-drift test fails cargo test on drift; handlers `#[derive(TS)] #[ts(export_to="wire.ts")]` (`meta.rs`, `dgm.rs`) | CLOSED |
| T-07-04-02 | Elevation of Privilege | `envi-engine/Cargo.toml` 3-dep (no ts-rs); ts-rs only in store/service | CLOSED |
| T-07-05-01 | Tampering | `web/index.html:1-21` — only a local `/src/main.tsx` module; zero CDN/font/external stylesheet or script | CLOSED |
| T-07-06-02 | Tampering (XSS) | grep `web/src` for `dangerouslySetInnerHTML`/`innerHTML`/`document.write`/`eval` = ZERO (matches are comments only); `icons.ts:38-49` builds SVG via `createElementNS`, not string-parse | CLOSED |
| T-07-06-03 | DoS (leak) | `store/autosave.ts:157-166` effect cleanup removes `beforeunload`/`pagehide` listeners + clears the timer + unsubscribes; review-verified for `useTerraDraw`/`useDgmTrigger`/map subscriptions | CLOSED |
| T-07-06-04 | Spoofing | `web/tests/e2e/_mocks.ts:52-66` `installOfflineGuard` aborts + records any non-localhost/data/blob request; basemap + `/api/v1` intercepted | CLOSED |
| T-07-07-01 | Tampering (XSS) | zero `innerHTML` in `web/src`; inspector values render as React text children | CLOSED |
| T-07-07-02 | Tampering | `web/src/panels/fields/GroundZoneFields.tsx:44-84` — closed-enum `<select>` for impedance A–H (class B = 31.5, corrected) and roughness N/S/M/L; out-of-vocab value structurally impossible client-side | CLOSED |
| T-07-07-03 | Tampering | fetch client imports generated `web/src/generated/wire.ts` (no hand-declared DTO to drift) | CLOSED |
| T-07-08-01 | Tampering | `web/src/store/edges.ts` `ringDiff` — `hasDuplicateCoords` guard `:122-124` (ME-03), same-count `changed>1 → rebuildResult` `:137-145` (ME-02), fresh-UUID `rebuild` fallback `:239-242`; regression tests referenced in `edges.test.ts` | CLOSED |
| T-07-08-02 | Tampering (XSS) | interpolate `detail` rendered text-only (no innerHTML); server path-redaction intact (`error.rs`) | CLOSED |
| T-07-08-03 | Tampering | interpolation (`store/interpolate.rs`) + SPL→L_W (`store/calibrate.rs`) are SERVER-side; client sends values / plots by band index only (review focus #5 verified) | CLOSED |
| T-07-08-04 | Tampering | spectra live in the store keyed by feature/edge UUID (`edges.ts` I/O header; `reconcileFacade` operates on an edge-UUID→spectrum map), never in Terra Draw feature properties | CLOSED |
| T-07-09-01 | Tampering | `web/src/validate/groundZone.ts:53-79` `classifyGroundZone` returns `partial-cross` (rejected at draw time, D-07); server `PUT /scene` validation is the backstop | CLOSED |
| T-07-09-02 | DoS | `web/src/store/autosave.ts:93-102` debounce fires only from the `commitEpoch` subscription (`:145-151`), never the drag/change path; one coalesced whole-scene PUT | CLOSED |
| T-07-09-03 | Tampering (XSS) | zero `innerHTML`; project name compared client-side for the delete gate; save/delete `detail` rendered as text | CLOSED |
| T-07-09-04 | Denial (data loss) | `web/src/panels/DeleteProjectDialog.tsx:38` `matches = typed === projectName`; danger button `disabled={!matches || deleting}` `:122`; Cancel is default focus | CLOSED |
| T-07-10-01 | Spoofing | `_mocks.ts` offline guard aborts unmocked requests (shared by all E2E specs) | CLOSED |
| T-07-10-02 | Tampering | source `index.html` zero external assets; `api/mod.rs` static-bundle contract test asserts `GET /` has no external references | CLOSED |

### `accept` threats — verified documented as accepted risk

| Threat ID | Category | Disposition basis | Verdict |
|-----------|----------|-------------------|---------|
| T-07-02-SC | Tampering (supply chain) | `spade 2.15` pinned in `envi-dgm/Cargo.toml`; pure Rust, no C deps; RESEARCH legitimacy audit = Approved | CLOSED (accepted) |
| T-07-04-SC | Tampering (supply chain) | `ts-rs 12` — dev/build only, absent from `envi-engine`; RESEARCH audit = Approved | CLOSED (accepted) |
| T-07-04-03 | Tampering | `[f64;105]` TS length erased is documented; runtime `BadBandCount` server-side is the real gate (`interpolate.rs`) | CLOSED (accepted) |
| T-07-05-SC | Tampering (supply chain) | `web/package.json` — build tooling (vite/typescript/vitest/@playwright/@types/@vitejs) is `devDependencies` ONLY; runtime deps (react, react-dom, maplibre-gl, react-map-gl, terra-draw, terra-draw-maplibre-gl-adapter, zustand, @turf/turf) are the only shipped code; NO `postinstall` script | CLOSED (accepted) |
| T-07-05-02 | Information Disclosure | `web/dist` is the intended public artifact; secrets scan of `web/src` = no API key/token/credential (basemap is keyless) | CLOSED (accepted) |
| T-07-06-01 | Information Disclosure | OpenFreeMap XHR accepted (D-13a); `web/src/map/basemap.ts:18` `DARK_BASEMAP_STYLE` is a CONSTANT, consumed at `MapCanvas.tsx:181` — not scene/attacker-derived (no SSRF surface) | CLOSED (accepted) |
| T-07-10-03 | Information Disclosure | shipped bundle is the intended artifact; no secrets (light/no-auth localhost) | CLOSED (accepted) |

---

## 2. Independent ASVS-L1 checks (beyond the declared register)

| Area | Finding |
|------|---------|
| Input validation — 3 new endpoints | `interpolate-spectrum`, `spl-to-lw`, `dgm/triangulate` all reject bad input as typed `400`; no `5xx`/panic path exists. `spade` never panics — every insert is `map_err`'d and the sole `add_constraint` is `can_add_constraint`-gated (`tin.rs:195-239`). Length/finiteness gates run before allocation. |
| Information disclosure (Phase-6 MED-1) | `error.rs:80-101` redaction intact for both new mappings; `From<DgmError>` never emits paths (coords/counts only). |
| XSS (whole new frontend) | ZERO `dangerouslySetInnerHTML`/`innerHTML`/`document.write`/`eval` in `web/src`; server strings render as React text children; SVG icons built via `createElementNS`. |
| Path traversal | New endpoints are body-only (`Json<...>`) — no raw string path segment introduced; project/scene routes use `Path<Uuid>` (`scene.rs:24,39`); frontend adds `encodeURIComponent` defense-in-depth (`client.ts:149-192`). Server is the real gate. |
| SSRF / third-party fetch | Basemap style URL is a module constant, not scene-derived — no attacker-controllable fetch URL. |
| Supply chain | Build tooling devDependencies-only; no `postinstall`; `ts-rs` (Rust build dep) absent from `envi-engine`; no C-linked crates in `envi-dgm`. |
| Autosave integrity | In-flight guard + trailing-save latch (`autosave.ts:58-89`) prevents a stale scene overwriting a newer one; unload flush uses `fetch(..., {keepalive:true})` via `putScene(...,{keepalive})` to the same `/scene` seam (`autosave.ts:113-134`, `client.ts:173-190`). |
| Secrets | No hardcoded credentials/tokens/API keys anywhere in `web/src`; basemap is deliberately keyless. |
| Engine hygiene | `#![deny(unsafe_code)]` on `envi-dgm` (`lib.rs:31`); zero C-linked crates; English-only; `envi-engine` byte-identical (3-dep). |

---

## 3. Accepted Risks

1. **Auth / access control — none (light/no-auth localhost).**
   ENVI is a self-hosted single-user internal tool served on localhost (PROJECT.md "Deployment: light/no auth"; "Out of Scope: multi-user SaaS / accounts / tenant isolation"). Every plan's threat block declares "Auth/access-control: N/A — localhost single-user, accepted." This is an explicit, documented project constraint — verified present, NOT a silent omission. No session/crypto surface exists this phase.

2. **LO-01 — DGM TIN preview triangulates WGS84 degrees in Euclidean space (Phase-8 projection deferral).**
   `web/src/dgm/dgmTrigger.ts` feeds raw `[lng, lat, z]` degrees into the server TIN, geometrically skewing the overlay away from the equator (~1.6:1 at 52°N). It is an explicitly labeled stub (module header documents the Phase-8 SceneXY-meters projection), never presented as a real elevation result. Not a security defect (no injection/DoS/disclosure surface — the endpoint still validates finiteness/caps). Accepted pending Phase-8 terrain import. Consistent with `07-REVIEW.md ## Accepted Risks`.

3. **Supply-chain / bundle-disclosure `accept` dispositions** (T-07-02-SC, T-07-04-SC, T-07-04-03, T-07-05-SC, T-07-05-02, T-07-06-01, T-07-10-03) — each verified above; recorded as accepted per the RESEARCH package-legitimacy audit and the localhost/no-secrets deployment posture.

---

## 4. Unregistered attack surface (new during implementation, no threat mapping)

None. The new attack surface introduced this phase — three POST endpoints, the `spade` TIN crate, the
first `npm` dependency tree, and the entire React frontend — is each covered by a declared threat
(T-07-02-xx, T-07-03-xx, T-07-05-SC, T-07-06/07/08/09-xx). No SUMMARY `## Threat Flags` entry lacked a
mapping.

---

## Verdict

**SECURED — 39/39 threats closed (32 mitigate-verified in code, 7 accept-documented), 0 open.**
No HIGH mitigation is missing or insufficient. The phase does not block on high.

_Verified 2026-07-10 by reading the current tree (post code-review + simplify). Implementation files
were not modified — this is a verification pass; only this SECURITY.md was written._
