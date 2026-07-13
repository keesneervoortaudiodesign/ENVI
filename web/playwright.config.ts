// Playwright E2E config for the ENVI map-authoring SPA (Phase 7 — WEB-01..).
//
// Drives the REAL Vite-served bundle in headless Chromium. No prod credentials: the same-origin
// /api/* surface is mocked per-test via route interception, and (D-13a) the basemap tile/style/glyph
// XHRs are intercepted too, so the suite runs fully offline against the shipped UI. @playwright/test
// is a devDependency only — it is NEVER bundled by `vite build`, so nothing tooling ships in web/dist.
// The e2e specs themselves land in a later plan (07-08+); this config is the toolchain seam.

import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/e2e",
  // The live-network smoke NEVER runs in the default suite — it is opt-in via its own config
  // (`npm run test:e2e:live` → playwright.live.config.ts). Ignoring it here is structural, not a
  // convention: the default run cannot touch the internet even if someone forgets the env gate.
  testIgnore: /live-smoke\.spec\.ts$/,
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: 0,
  // Cap parallelism: every spec drives a MapLibre GL map through headless Chromium's SOFTWARE WebGL
  // (swiftshader — see launchOptions), which is CPU-bound, and the WASM GIS decode in the import specs
  // adds more. Playwright's default (~half the core count) over-subscribes on many-core machines and
  // starves the software-rendered contexts into 30 s timeouts; four workers keeps the whole suite green
  // and fast regardless of host core count.
  workers: 4,
  reporter: [["list"]],
  use: {
    baseURL: "http://localhost:5174",
    headless: true,
    trace: "on-first-retry",
    // MapLibre GL needs WebGL2; headless Chromium (137+) blocks the software (SwiftShader) fallback
    // unless explicitly allowed. These flags give a deterministic offscreen WebGL context so the map
    // renders in CI-less local runs without a GPU. (No effect on the shipped bundle — test-only.)
    launchOptions: {
      args: [
        "--enable-unsafe-swiftshader",
        "--use-gl=angle",
        "--use-angle=swiftshader",
        "--ignore-gpu-blocklist",
      ],
    },
  },
  // Two projects, deliberately SEQUENCED (not two ways to run the same thing).
  //
  // `full-journey` drives a genuine threaded-WASM solve — a `wasm-bindgen-rayon` pool that saturates every
  // core. Run concurrently with the focused specs it starves their software-WebGL (SwiftShader) map
  // contexts into spurious timeouts: `objectStyling` fails with featureCount 0 while passing standalone.
  // That is the same over-subscription the `workers: 4` cap above guards against, except the journey brings
  // its own thread pool, so no worker count makes them safe neighbours.
  //
  // `dependencies` makes Playwright finish `specs` completely before `journey` starts, so the solve gets the
  // machine to itself. Cost: a failure in `specs` skips `journey` (fix the unit-level break first — which is
  // the right order anyway). To run the journey alone, bypass the dependency: `npm run test:e2e:journey`.
  projects: [
    {
      name: "specs",
      testIgnore: /full-journey\.spec\.ts$/,
      use: { ...devices["Desktop Chrome"] },
    },
    {
      name: "journey",
      testMatch: /full-journey\.spec\.ts$/,
      dependencies: ["specs"],
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  webServer: {
    command: "npm run dev -- --port 5174 --strictPort",
    url: "http://localhost:5174",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
