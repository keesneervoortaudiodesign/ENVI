# ENVI — Project Instructions

Web-based, self-hosted **Nord2000** environmental sound-propagation engine, implemented from scratch. Rust backend (heavy acoustics + GIS math), JSX/React frontend, OpenStreetMap-based graphical input. See `.planning/PROJECT.md` for full context, `.planning/REQUIREMENTS.md` for scope, `.planning/ROADMAP.md` for phases, and `.planning/research/` + `docs/` for research.

> Toolchain / workflow / GitHub conventions below are adopted from an established sibling repo's engineering practices. They are general engineering rules — none of that project's domain specifics apply here.

## Session Start (do this at the beginning of EVERY new session)
1. **Pull from GitHub first** — `git pull --ff-only origin main` so you start from the latest. Do this before planning or editing.
2. **Check GSD state** — read `.planning/STATE.md` (and `ROADMAP.md`) to see the current phase/position before doing GSD work. Run `/gsd-progress` if unsure where things stand.

## Environment / process hygiene (HARD RULE)
- **NEVER close, kill, or restart VS Code** — do not run any command that terminates the editor or its host process (`code --*` shutdowns, `Stop-Process`/`taskkill`/`kill` targeting `Code.exe`, `code-server`, or its window). This has already crashed the user's editor **twice**, most likely because VS Code's **Java language-server / extension-host services were force-closed** and took the editor down with them. Do not force-close Java processes (`java.exe`, `jdk`, `jvm`, Gradle/Maven daemons, `*LanguageServer*`) either — killing them can cascade into VS Code.
- If a process genuinely needs stopping, **target only the specific child process you started** (by PID you own), never a broad name match, and prefer graceful termination. When in doubt, **ask the user to close it themselves** rather than issuing the kill.

## Working Language
- **Everything Claude produces is written in English** — code, comments, UI strings, README/docs, all `.md` files, commit messages, and planning artefacts. Hard rule, session to session, overriding any default. If you find a file in another language, it predates this rule — translate rather than extend.
- **Conversation language follows the user** — reply in whatever language they write to you in. Only the chat adapts; written/committed output stays English.

## Language & Toolchain
- **Rust, edition 2024** for the backend/engine. **Native Rust crates preferred over FFI** wherever a mature one exists; accept C-linked crates (`gdal`, `proj`) only where there is no viable pure-Rust option, and keep them behind a thin boundary.
- **Verified engine stack** (from `.planning/phases/01-*/01-RESEARCH.md` and `research/OPEN-GIS-LANDSCAPE.md`): a cargo **workspace** split into a pure-math engine crate and an I/O/harness crate; `ndarray` + `num-complex` for the complex transfer tensor; `geo`/`rstar`/`spade`/`gdal`/`proj` for geometry/GIS; `calamine` for reading FORCE `.xls` cases; `libtest-mimic` for case-driven dynamic tests.
- **Frontend:** JSX/React with **MapLibre GL JS 5 + react-map-gl 8 + Terra Draw** (OSM input); results rendered as server-side isophone fill polygons, not a heatmap layer. (Frontend arrives in a later milestone — Milestone 1 is engine-only, no UI.)
- **Numerics house rule (engine):** `f64` throughout; guard catastrophic cancellation (the Δτ travel-time difference especially); clamp the ξ / homogeneous-atmosphere singularities; clamp roughness length `z₀ ≥ 0.001 m`. These are correctness-critical, not style.

## Build
- `cargo build --release` (workspace root).
- Run the engine's validation harness with `cargo test` — case-driven comparison against analytic / ISO / FORCE reference values with per-band tolerance reporting.
- `cargo run -p envi-harness -- report` prints per-case Pass/Skip with per-band deviations.

## Codebase as built (learned in Phases 1–2 — treat as binding conventions)
- **Workspace layout:** two crates.
  - `envi-engine` — pure math, `#![deny(unsafe_code)]`, dependencies quarantined to **`ndarray` + `num-complex` + `thiserror` only** (no I/O crates; a `cargo tree -p envi-engine` check enforces this). Modules under `propagation/` (`special` w(ẑ), `ground`, `diffraction`, `fresnel`, `rays`, `coherence`, `terrain_effect/{submodel1,2,7,screen}`, `terrain_interpretation`), plus `scene`, `geometry`, `transfer`, `freq`.
  - `envi-harness` — all I/O: FORCE `.xls` (calamine) + TOML case loaders, the `Capability`-gated `run_case` dispatch, the `libtest-mimic` dynamic runner, oracle-comparison tests, and the `report` CLI.
- **Complex / phase contract (load-bearing — do not violate):** the canonical transfer is **`H_coh: Complex<f64>` with phase preserved through EVERY environmental operator** (divergence e^{−jkR}, air absorption, ground Q̂, wedge diffraction, and later refraction). Turbulence-decorrelated energy lives in a **separate real `P_incoh` channel** added only at level readout; total level = `|coherent Σ|² + P_incoh`. `F→1 ⇒ P_incoh→0` (bit-exact). Never collapse phase to energy mid-chain; never let `P_incoh` overwrite `H_coh`. `TransferTensor = Array3<Complex<f64>>` indexed `[sub_source, receiver, freq]` is the frozen forward contract for the Phase-4 fast-recalc tensor.
- **Time-convention quarantine:** ENVI's frozen convention is **e^{+jωt}**; Nord2000's source equations are e^{−jωt}. The Nord2000-native math (impedance +j, phase e^{+jωτ}) is implemented verbatim inside `propagation/` with **exactly ONE `.conj()` boundary — `transfer::nord_ratio_to_transfer`**. Never introduce `.conj()` anywhere in `propagation/` (a grep gate enforces zero); write conjugation as explicit `Complex::new(re, -im)` for Faddeeva symmetry, and keep the single real boundary in `transfer.rs`.
- **Frequency framework:** a **105-point 1/12-octave grid** (x/12 rule, G = 10^(3/10), ~25.12 Hz–10 kHz); every 4th point is an exact 1/3-octave centre. **Compare by BAND INDEX, never by nominal frequency** (a recurring pitfall).
- **FORCE test data + acceptance ladder:** the FORCE `.xls` workbooks and AV PDFs live in a **git-ignored `refs/`** (fetched by `refs/fetch.sh`, SHA-256-pinned in `refs/refs.sha256`); never commit them (copyright). Tests **fail-soft** — a case reports `Skipped(requires: …)` when refs or an unimplemented capability are absent, never a false Pass. Numeric green comes from the **oracle+anchor ladder**: exact analytic/ISO anchors + committed **scipy oracle fixtures** under `tools/nord2000_oracle/` (regenerated by the `gen_*.py` scripts; **no Python needed at test time**) + property tests. Note the oracle-independence caveat: the Python oracles reimplement the same equation transcriptions as the engine (except Faddeeva via `scipy.special.wofz`), so they cross-check *implementation*, not the *spec reading* — a shared misread of AV 1106/07 won't be caught by the oracle alone.
- **FORCE full-suite pass is Phase 4** (needs the road emission model; maps to VAL-02). Phases 2–3 gate on anchors/oracle, and FORCE road cases stay capability-gated (shrinking `requires:` list).
- **Nord2000 data pinned:** impedance classes A–H by flow resistivity σ (note **class B = 31.5**, corrected from research); roughness classes N/S/M/L. `refs/` AV 1106/07 rev.4 is the implement-from document — **verify transcribed equations/coefficients against the PDF page images** (multiple research transcription errors were caught this way; when in doubt, the authoritative `.xls`/PDF wins over a summary).

## Quality gates (run before considering any code change done)
- `cargo clippy --all-targets -- -D warnings` — zero lint warnings.
- `cargo fmt --check` — enforced formatting.
- `cargo test` — all tests pass (this includes the FORCE/analytic validation harness).
- `#![deny(unsafe_code)]` on the pure-math engine crate. `unsafe` is allowed **only** at genuine FFI boundaries (`gdal`, `proj` C bindings), never in the acoustics/geometry logic.

## GSD Workflow — Phase Completion gates (MANDATORY — always run, auto-fix)
This repo is driven by GSD (`/gsd-*`). At the end of EVERY GSD phase, before it is marked complete, **ALWAYS run the gates below AND automatically fix the issues they surface** — do not skip them, do not merely record findings, do not stop to ask permission first. A phase is not "done" until these have run, every finding is **fixed** (only where a fix is genuinely wrong or infeasible may it be explicitly recorded as an accepted risk with rationale), and all quality gates are green again.
Run these five gates in order; each finding is **fixed** (or explicitly recorded as an accepted risk with rationale), and `cargo test/clippy/fmt` are re-run green after every gate that touches code:

1. **`/gsd-code-review <phase>`** — finds bugs, security issues, and code-quality problems in the phase's changed files; every finding fixed or accepted as risk. (Run with `--fix`, looping `--fix --auto` until the review is clean or only accepted-risk findings remain.)
2. **`/simplify`** — quality-only cleanup (reuse, simplification, efficiency, altitude) of the changed code, applied to the tree; every cleanup applied or explicitly declined. It does not hunt for bugs — it complements `/gsd-code-review`, never replaces it.
3. **`/gsd-secure <phase>`** (`/gsd-secure-phase`) — retroactively verifies the phase's threat-model mitigations actually exist in the code (produces SECURITY.md); every gap closed or accepted. Fix any missing/insufficient mitigation, then re-verify until SECURITY.md is clean.
4. **`/gsd-verify <phase>`** — goal-backward verification that the shipped code delivers the phase's success criteria (produces VERIFICATION.md); status must be `passed` or gaps explicitly accepted. Run AFTER the code-review/secure fixes so it verifies the fixed tree.
5. **Documentation consistency scan** — full document scan confirming close-out introduced no doc inconsistencies across `.planning/STATE.md` (frontmatter counts/percent/position + status table), `ROADMAP.md` (`**Plans:** N/N`, per-plan `[x]`, Progress table row), `REQUIREMENTS.md` traceability, and each phase artifact's `status:` frontmatter — all must agree (no stale `In Progress`, no mismatched plan count, no verdict/status contradiction; the GSD `phase complete` tool can mangle STATE.md frontmatter — re-verify by hand). Also enforces the code/README documentation contract: Module I/O headers, `crates/README.md`, and root `README.md` reflect any new feature or changed behaviour.

**No phase is "done" until all five have run and every finding is fixed or explicitly recorded as an accepted risk.**

## GitHub use
- **Pull at session start** (see above); keep `main` current.
- **Commit / push only when the user asks.** When committing, use clear conventional-style messages (`docs:`, `feat:`, `fix:`, `test:`, `chore:`) and end the message with:
  `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`
- **Never work directly on a shared default branch for risky changes** without saying so; branch first when appropriate.
- **No GitHub Actions / CI workflows by default.** Do not create or scaffold anything under `.github/workflows/` unless the user explicitly asks for CI. Builds, tests, and any deploys are operator-driven from an authenticated local session (`cargo …`, the app's own run/deploy commands). If CI is ever wanted, it is out of scope until explicitly requested.
- **Credentials/tokens are entered by the user interactively** — never pre-fill, guess, or carry over account names, tokens, or secrets from any other context or repo.
- Use the `gh` CLI for GitHub operations when it is available. (`gh` is not currently installed on this machine — if a repo/PR operation needs it, ask the user to install + `gh auth login`, or provide a remote URL to push to.)

## Frontend testing (future — when the JSX frontend exists)
- Automated browser UAT for the React frontend uses **Playwright** (`@playwright/test`, a devDependency only — never bundled into the production build).
- Tests drive the **real** built bundle (not a reimplementation); mock the backend `/api/*` per-test via `page.route(...)` so runs are offline and need no credentials.
- Playwright artifacts (`test-results/`, `playwright-report/`) are git-ignored.

## Licensing / data hygiene
Personal/internal tool. **Do not redistribute** the copyrighted Nord2000 documents (AV 1106/07 etc.) — implement from the equations and cite by report number. Honor open-data attribution (Copernicus / ESA / OSM). Avoid non-commercial-licensed data (FABDEM, Meteostat). Port architectural *ideas* from GPL references (e.g. NoiseModelling), never translate GPL source into this codebase.
