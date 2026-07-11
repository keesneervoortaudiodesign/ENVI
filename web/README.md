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
| `npm run build:wasm` | Compile the `envi-gis-wasm` cdylib for `wasm32-unknown-unknown` and run `wasm-bindgen` to emit the ESM glue + `.wasm` into `src/generated/wasm/`. |
| `npm run build` | Full build: `build:wasm` → `tsc --noEmit` → `vite build` (emits `dist/`). |
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

Missing either? Use `npm run build:web` to build the frontend without
regenerating the wasm glue.

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
