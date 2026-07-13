// playwright.live.config.ts — the OPT-IN live-network config (`npm run test:e2e:live`).
//
// The default config (playwright.config.ts) `testIgnore`s live-smoke.spec.ts, so a normal
// `npm run test:e2e` is structurally incapable of touching the internet. This config is the only way
// to run it: it targets that one spec and sets the env gate the spec also honours, so a direct
// `ENVI_E2E_LIVE=1 npx playwright test live-smoke` works too (setting it here keeps the npm script
// cross-platform — bare `VAR=1 cmd` does not work in npm scripts on Windows, and this avoids adding a
// cross-env devDependency for one line).
//
// What it is for: the hermetic suite proves we correctly parse the RECORDED fixtures. Only a live call
// proves we still ASK the right question — that ESA/PDOK/Overpass/Open-Meteo/OpenFreeMap have not
// changed a URL, a parameter or a response shape underneath us. Run it when a data source misbehaves in
// the real app but the offline suite is green, and periodically to catch upstream drift.
//
// Needs an internet connection. No credentials (every source used is keyless).

import { defineConfig, devices } from "@playwright/test";

// The spec's own guard reads this; setting it here means the npm script needs no shell env syntax.
process.env.ENVI_E2E_LIVE = "1";

export default defineConfig({
  testDir: "./tests/e2e",
  testMatch: /live-smoke\.spec\.ts$/,
  fullyParallel: false,
  // Serial: live endpoints are shared, rate-limited and slow. Parallelism buys nothing and risks 429s.
  workers: 1,
  // A live source can genuinely blip. ONE retry distinguishes a transient network hiccup from a real
  // upstream contract change (which fails deterministically, twice).
  retries: 1,
  reporter: [["list"]],
  use: {
    baseURL: "http://localhost:5174",
    headless: true,
    trace: "on-first-retry",
    // MapLibre needs WebGL2; headless Chromium blocks the SwiftShader fallback unless allowed.
    launchOptions: {
      args: [
        "--enable-unsafe-swiftshader",
        "--use-gl=angle",
        "--use-angle=swiftshader",
        "--ignore-gpu-blocklist",
      ],
    },
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "npm run dev -- --port 5174 --strictPort",
    url: "http://localhost:5174",
    reuseExistingServer: true,
    timeout: 120_000,
  },
});
