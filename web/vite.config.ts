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
//
// # Cross-origin isolation (SVC-02, D-04, plan 10-02)
// `server.headers` emits `Cross-Origin-Opener-Policy: same-origin` +
// `Cross-Origin-Embedder-Policy: credentialless` so `npm run dev` AND the Playwright test server
// are cross-origin isolated (`self.crossOriginIsolated === true`) — the prerequisite for
// `SharedArrayBuffer` and the wasm-bindgen-rayon thread pool the client-side solve spawns. This
// mirrors the production headers the `envi-service` axum bundle sends. The COEP value is
// `credentialless`, NOT `require-corp`: credentialless strips credentials on no-cors sub-resource
// loads so the Phase-8 direct third-party fetches (basemap/AHN/Overpass) keep working without a
// CORP header on every source — `require-corp` would break them. Native Vite feature: no new npm
// dependency (no `vite-plugin-cross-origin-isolation`).

import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Resolve a path relative to this config WITHOUT a `node:*` import (the config
// stays node-types-free — see the header note). `URL.pathname` is "/D:/…/x" on
// Windows; strip the leading slash before a drive letter so the alias replacement
// is a real filesystem path the resolver accepts.
function localPath(rel: string): string {
  const p = new URL(rel, import.meta.url).pathname;
  return /^\/[A-Za-z]:\//.test(p) ? p.slice(1) : p;
}

export default defineConfig({
  base: "./",
  plugins: [react()],
  resolve: {
    alias: {
      // The Rust OPFS sink binds a bare `envi-compute-opfs` extern specifier
      // (`#[wasm_bindgen(module = "envi-compute-opfs")]`, opfs_sink.rs); map it to
      // the worker-side JS glue so the threaded wasm module resolves it at bundle
      // time (the wasm crate compiles before this TS exists). Only pulled in once
      // the sink is wired into `solve_chunk_range`; an inert, harmless alias until.
      "envi-compute-opfs": localPath("./src/compute/opfs.ts"),
    },
  },
  server: {
    headers: {
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cross-Origin-Embedder-Policy": "credentialless",
    },
  },
  // The wasm-bindgen `.wasm` artifacts are explicit assets the Vite build consumes:
  // `src/generated/wasm/` is the stable single-threaded GIS boundary (build:wasm);
  // `src/generated/wasm-compute/` is the THREADED (SharedArrayBuffer/atomics) compute
  // module (build:wasm:compute — nightly + -Zbuild-std, plan 10-03), git-ignored like
  // the gis one. Both are regenerated from their committed Rust crates.
  assetsInclude: [
    "**/src/generated/wasm/*.wasm",
    "**/src/generated/wasm-compute/*.wasm",
  ],
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
