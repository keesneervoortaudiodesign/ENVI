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
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: 0,
  reporter: [["list"]],
  use: {
    baseURL: "http://localhost:5174",
    headless: true,
    trace: "on-first-retry",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "npm run dev -- --port 5174 --strictPort",
    url: "http://localhost:5174",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
