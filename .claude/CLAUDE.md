# ENVI — Project Instructions

Web-based, self-hosted **Nord2000** environmental sound-propagation engine, implemented from scratch. Rust backend (heavy acoustics + GIS math), JSX/React frontend, OpenStreetMap-based graphical input. See `.planning/PROJECT.md` for full context, `.planning/REQUIREMENTS.md` for scope, `.planning/ROADMAP.md` for phases, and `.planning/research/` + `docs/` for research.

> Toolchain / workflow / GitHub conventions below are adopted from an established sibling repo's engineering practices. They are general engineering rules — none of that project's domain specifics apply here.

## Session Start (do this at the beginning of EVERY new session)
1. **Pull from GitHub first** — `git pull --ff-only origin main` so you start from the latest. Do this before planning or editing.
2. **Check GSD state** — read `.planning/STATE.md` (and `ROADMAP.md`) to see the current phase/position before doing GSD work. Run `/gsd-progress` if unsure where things stand.

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

## Quality gates (run before considering any code change done)
- `cargo clippy --all-targets -- -D warnings` — zero lint warnings.
- `cargo fmt --check` — enforced formatting.
- `cargo test` — all tests pass (this includes the FORCE/analytic validation harness).
- `#![deny(unsafe_code)]` on the pure-math engine crate. `unsafe` is allowed **only** at genuine FFI boundaries (`gdal`, `proj` C bindings), never in the acoustics/geometry logic.

## GSD Workflow — Phase Completion gates
This repo is driven by GSD (`/gsd-*`). At the end of EVERY GSD phase, before it is marked complete, **run all applicable gates** — mandatory, not optional. A phase is not "done" until each finding is fixed or explicitly recorded as an accepted risk:
- **`/gsd-code-review <phase>`** — review the phase's changed files for bugs, security, and quality issues.
- **`/gsd-secure <phase>`** — verify the phase's threat-model mitigations exist in the implemented code (produces SECURITY.md).
- **`/gsd-verify <phase>`** — goal-backward verification that the shipped code delivers the phase's success criteria (produces VERIFICATION.md); the phase is not complete until its status is `passed` (or gaps are explicitly accepted).
- **Documentation consistency scan** — after the gates, grep the phase number across `.planning/` and reconcile every hit: `STATE.md` (frontmatter counts/percent/position + status table), `ROADMAP.md` (`**Plans:** N/N`, per-plan `[x]`, Progress table row), `REQUIREMENTS.md` traceability, and each phase artifact's `status:` frontmatter must all agree — no stale `In Progress`, no mismatched plan count, no verdict/status contradiction. (The GSD `phase complete` tool can mangle STATE.md frontmatter — re-verify by hand.)
- **README** — any new feature or changed behaviour must be reflected in the repo `README.md` before the phase is "done".

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
