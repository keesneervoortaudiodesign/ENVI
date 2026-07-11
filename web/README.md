# ENVI web — map-authoring frontend

React 19 + Vite + MapLibre GL + Terra Draw single-page app for authoring a
Nord2000 scene on an OpenStreetMap basemap. The built bundle (`web/dist/`) is
**git-tracked** and served from disk by `envi-service` (`ServeDir(web/dist)`,
SVC-03) — fully offline, zero external/CDN assets (D-11).

Build/test tooling is `devDependencies` only; nothing tooling ships in the
bundle. Only the runtime deps (react / maplibre / react-map-gl / terra-draw /
zustand / turf) reach the browser.

## Commands

| Command | What it does |
|---------|--------------|
| `npm run dev` | Vite dev server (hot reload). |
| `npm run build:wasm` | Compile the `envi-gis-wasm` cdylib for `wasm32-unknown-unknown` (stable) and run `wasm-bindgen` to emit the ESM glue + `.wasm` into `src/generated/wasm/`. |
| `npm run build:wasm:compute` | Compile the **threaded** `envi-compute-wasm` cdylib (nightly + `-Zbuild-std` + atomics, SharedArrayBuffer pool) and emit its glue + `.wasm` + rayon worker snippets into `src/generated/wasm-compute/`. See the threaded-build section below. |
| `npm run build` | Full build: `build:wasm` → `build:wasm:compute` → `tsc --noEmit` → `vite build` (emits `dist/`). |
| `npm run build:web` | Frontend-only build (skips the wasm step) — use when the wasm glue is already generated. |
| `npm run preview` | Preview the built `dist/`. |
| `npm run test:unit` | Vitest unit tests. |
| `npm run test:e2e` | Playwright end-to-end tests against the built bundle (offline; `/api/*` mocked per-test). |

## GIS-ingestion WASM boundary (DATA-01..03)

The client-side GIS-ingestion core (`envi-gis`, pure Rust, sans-I/O) is exposed
to the browser through a thin `wasm-bindgen` cdylib, **`envi-gis-wasm`**
(`crates/envi-gis-wasm/`). TypeScript owns all I/O (`fetch`, OPFS, ids); the wasm
module is pure marshalling over the pure core:

`plan_import` · `decode_window` · `terrain_features` · `sample_base_elevation` ·
`map_landcover` · `parse_buildings` · `merge_features`.

### The build step

```bash
# From web/ (npm wires both steps):
npm run build:wasm
# …which runs, under the hood:
cargo build -p envi-gis-wasm --release --target wasm32-unknown-unknown
wasm-bindgen --target web \
  --out-dir src/generated/wasm --out-name envi_gis_wasm \
  ../target/wasm32-unknown-unknown/release/envi_gis_wasm.wasm
```

Output (`web/src/generated/wasm/`) is a **build artifact** — git-ignored,
regenerated from the committed Rust crate. Vite consumes it through the `@wasm`
alias (`vite.config.ts`); `wasm-bindgen --target web` emits a standard ESM that
fetches its `.wasm` via `new URL(…, import.meta.url)`, which Vite bundles and
serves natively.

### ⚠️ Version lockstep — `wasm-bindgen` crate ↔ `wasm-bindgen-cli` (MUST match)

The `wasm-bindgen` **crate** and the `wasm-bindgen` **CLI** must be the **exact
same version** — a mismatch produces broken glue that fails at runtime in the
browser, not at build time (08-RESEARCH Pitfall 8). This repo therefore pins both:

- Crate: `wasm-bindgen = "=0.2.126"` (exact `=`), in
  `crates/envi-gis-wasm/Cargo.toml`.
- CLI: install the matching version, locked:

  ```bash
  cargo install wasm-bindgen-cli --locked --version 0.2.126
  ```

**Pinned version: `0.2.126`.** When bumping `wasm-bindgen`, change *both* the
crate `=` pin and the installed CLI version in the same commit, and re-run
`npm run build:wasm`.

### Prerequisites for a full `npm run build`

- Rust toolchain with the `wasm32-unknown-unknown` target:
  `rustup target add wasm32-unknown-unknown`.
- `wasm-bindgen-cli` at the pinned version (command above).
- For the threaded compute module (`build:wasm:compute`): the pinned nightly
  toolchain **with the `rust-src` component** (see the next section).

Missing either? Use `npm run build:web` to build the frontend without
regenerating the wasm glue.

## Threaded compute WASM boundary (SVC-02 / GRID-02, plan 10-03)

The client-side Nord2000 grid solve runs as **threaded WebAssembly**: the
`envi-compute-wasm` cdylib wraps the pure `envi-compute` core + the OPFS tensor
sink and drives a `wasm-bindgen-rayon` thread pool over a `SharedArrayBuffer`
(sized to `navigator.hardwareConcurrency`). This needs a *different* toolchain
from the stable `build:wasm` — nightly Rust + `-Zbuild-std=std,panic_abort` +
`-C target-feature=+atomics,+bulk-memory,+mutable-globals`.

### The build step

```bash
# From web/:
npm run build:wasm:compute
# …which runs, under the hood:
cargo +nightly-2026-07-11 build -p envi-compute-wasm --release \
  --target wasm32-unknown-unknown --features threads \
  -Z build-std=std,panic_abort \
  --config "target.wasm32-unknown-unknown.rustflags=['-C', 'target-feature=+atomics,+bulk-memory,+mutable-globals']"
wasm-bindgen --target web \
  --out-dir src/generated/wasm-compute --out-name envi_compute_wasm \
  ../target/wasm32-unknown-unknown/release/envi_compute_wasm.wasm
```

Output (`web/src/generated/wasm-compute/` — glue + `_bg.wasm` + `snippets/` for
the rayon worker) is a **build artifact**, git-ignored, regenerated from the
committed Rust crate. Vite consumes the `.wasm` via `assetsInclude`.

### ⚠️ Scoping — the atomics/nightly toolchain MUST stay isolated (Pitfall 1)

The nightly toolchain and the atomics `RUSTFLAGS` are scoped to **this one
command only**:

- Nightly is selected per-invocation with `cargo +nightly-2026-07-11` — there is
  **no repo-root `rust-toolchain.toml`** (which would force nightly on every
  build).
- The atomics rustflags are passed with an **inline `--config`** (per-invocation,
  cross-platform — works in both `cmd.exe` and POSIX shells) — there is **no
  `.cargo/config.toml`** with a `[build]`/`[target] rustflags` (which would force
  atomics onto the stable `build:wasm` and native `cargo build`/`cargo test`,
  breaking them without `-Zbuild-std`).

The stable `build:wasm` (gis) is untouched and still builds on stable Rust.

### Prerequisites (threaded module only)

```bash
rustup toolchain install nightly-2026-07-11 --component rust-src
```

**Pinned nightly: `nightly-2026-07-11`** (verified to support `-Zbuild-std` for
`wasm32-unknown-unknown`). The `wasm-bindgen`-crate ↔ `wasm-bindgen-cli`
`=0.2.126` lockstep above applies to this module too — the same pinned CLI
generates both bundles; the atomics come from `-Zbuild-std` + the rustflags, not
the CLI. When bumping the nightly, change the date in `package.json` and here in
the same commit.

## Generated wire types (no hand-written TS mirror — D-10)

`src/generated/wire.ts` is **generated** from the Rust serde DTOs via `ts-rs`
(both the HTTP wire — `envi-store` + `envi-service` — and the WASM boundary —
`envi-gis-wasm`). It is committed, and a Rust no-drift test asserts that
regenerating it produces no diff, so a renamed/added Rust field fails
`cargo test`, not the browser. **Never hand-edit `wire.ts` or hand-author a TS
mirror of a Rust DTO.**

Regenerate after intentionally changing a wire/boundary DTO:

```bash
cargo test -p envi-service --test wire_no_drift -- --ignored regenerate_committed_wire_ts
```
