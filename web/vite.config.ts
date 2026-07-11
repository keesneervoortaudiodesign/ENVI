// Vite build config for the ENVI map-authoring SPA (D-09; Phase 7 frontend shell).
//
// # Module I/O
// - Input  `src/main.tsx` (+ its CSS/TS imports) — the SPA entry; `index.html` is the build root.
// - Output `dist/` — a self-contained, RELATIVE-path bundle (`base: './'`) served from disk by
//   `envi-service` via `ServeDir(web/dist)` (SVC-03). No external/CDN references in index.html
//   (D-11 fully offline); build tooling (vite/tsc/@vitejs/plugin-react) is a devDependency and
//   is NEVER shipped in the bundle — only the runtime deps (react/maplibre/…) reach the browser.
//
// `base: './'` makes every asset reference relative so the bundle works regardless of the mount
// path envi-service serves it from. CSS is inlined into one stylesheet (`cssCodeSplit: false`);
// a single module chunk keeps the served asset surface minimal. `@vitejs/plugin-react` is the
// ENVI divergence from the metrao3 ancestor (which is vanilla TS with no React plugin).
//
// # WASM ingestion boundary (DATA-01..03, plan 08-06)
// `npm run build:wasm` compiles the `envi-gis-wasm` cdylib for `wasm32-unknown-unknown` and runs
// the version-locked `wasm-bindgen` (target `web`) to emit ESM glue + `.wasm` into the
// `assetsInclude`d `src/generated/wasm/` directory below (git-ignored — a build artifact
// regenerated from the Rust crate; see web/README.md for the mandatory CLI↔crate version
// lockstep). The future ingestion UI imports the glue directly from that path
// (`import init, { plan_import, ... } from "./generated/wasm/envi_gis_wasm"`); wasm-bindgen's
// `--target web` output is a standard ESM that fetches its `.wasm` via
// `new URL(..., import.meta.url)`, which Vite bundles/serves natively. The generated TS *types*
// for the boundary DTOs are the committed, no-drift-tested `src/generated/wire.ts`.
//
// The config avoids `node:*` imports on purpose (tsconfig `types: []`, no `@types/node`), so the
// wasm reference here is `assetsInclude` (a plain glob) rather than a path-alias built from
// `fileURLToPath`.

import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  base: "./",
  plugins: [react()],
  // The wasm-bindgen `.wasm` in src/generated/wasm/ is an explicit asset the Vite build consumes.
  assetsInclude: ["**/src/generated/wasm/*.wasm"],
  build: {
    target: "es2022",
    outDir: "dist",
    emptyOutDir: true,
    assetsInlineLimit: 4096,
    cssCodeSplit: false,
    modulePreload: { polyfill: false },
    rollupOptions: {
      output: {
        // Single, stable-ish chunking — keep the served asset surface minimal.
        manualChunks: undefined,
      },
    },
  },
});
