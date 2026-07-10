// vitest.config.ts — the UNIT test runner scope (`npm run test:unit` → `vitest run`).
//
// # Module I/O
// - Input  the `src/**/*.test.ts` Vitest unit suites (currently the load-bearing D-02 ring-diff tests,
//   `src/store/edges.test.ts`).
// - Output a Node-environment unit run. Critically, the Playwright browser specs under `tests/e2e/*.spec.ts`
//   are EXCLUDED: they import `@playwright/test` (a different runner) and must never be executed by Vitest —
//   `npm run test:e2e` (`playwright test`) owns those. Without this scoping, Vitest's default `**/*.spec.ts`
//   glob would pull the Playwright specs in and fail the unit gate. Pure logic tests need no DOM, so the
//   default `node` environment is used (no jsdom dependency added).

import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // Unit tests live beside the code they cover, under src/.
    include: ["src/**/*.test.ts"],
    // tests/e2e/*.spec.ts are Playwright specs (own runner) — never run them under Vitest.
    exclude: ["node_modules/**", "dist/**", "tests/e2e/**"],
    environment: "node",
  },
});
