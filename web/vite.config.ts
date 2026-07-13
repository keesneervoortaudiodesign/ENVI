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

// wasm-bindgen-rayon (the threaded compute glue's pool) emits a worker helper that
// re-imports the wasm package by BARE DIRECTORY: `await import('../../..')` from
// `src/generated/wasm-compute/snippets/wasm-bindgen-rayon-*/src/workerHelpers.js`,
// which resolves to the `src/generated/wasm-compute/` directory. Neither Vite's dev
// import-analysis nor the rolldown build (incl. the worker sub-build) can resolve a
// directory without an index/extension, so the threaded glue fails to load in the
// browser the moment anything imports it (surfaced by the 10-05 CalcPanel — the
// first app code to pull the glue in; 10-03/10-04 only unit-tested the worker with a
// MOCK wasm + a guarded real-worker path, so this browser-load seam was never
// exercised). This transform rewrites ONLY that exact `import('../../..')` in ONLY
// that snippet file to the real glue ESM entry (`../../../envi_compute_wasm.js`), a
// resolvable relative path — leaving every other import untouched. Applied as a
// `transform` (not `resolveId`) so it also reaches the worker sub-build.
function resolveRayonWorkerHelperImport() {
  return {
    name: "envi-rayon-worker-helper-resolve",
    enforce: "pre" as const,
    transform(code: string, id: string): { code: string; map: null } | null {
      if (!/wasm-bindgen-rayon.*workerHelpers\.js/.test(id)) {
        return null;
      }
      const rewritten = code.replace(
        /import\((['"])\.\.\/\.\.\/\.\.\1\)/,
        'import("../../../envi_compute_wasm.js")',
      );
      return rewritten === code ? null : { code: rewritten, map: null };
    },
  };
}

export default defineConfig({
  base: "./",
  plugins: [react(), resolveRayonWorkerHelperImport()],
  // The compute Web Worker (`new Worker(new URL("./worker.ts", …), { type: "module" })`) is bundled by Vite
  // in a SEPARATE sub-build that does NOT inherit the top-level `plugins`; it uses `worker.plugins`. The
  // rayon worker helper's bare-directory import lives in that graph, so the rewrite plugin must be here too.
  worker: {
    format: "es",
    plugins: () => [resolveRayonWorkerHelperImport()],
  },
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
    // DEV-ONLY: forward the same-origin `/api/v1` surface to a locally-running `envi-service`
    // (`cargo run -p envi-service`, 127.0.0.1:8080). Without this the dev server has no backend, so
    // `/api/v1/proxy/**` — the byte relay for the two CORS-blocked S3 sources (GLO-30, WorldCover) —
    // 404s, and a GIS import silently loses its land-cover/terrain layers in dev while working in the
    // built bundle. If envi-service is not running the proxy just errors per-request (Vite logs it) and
    // the dev server still boots — the API is simply unavailable, which is the honest state.
    // Affects the dev server ONLY: `vite build` emits the same bundle, served by envi-service itself.
    proxy: {
      "/api": { target: "http://127.0.0.1:8080", changeOrigin: false },
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
