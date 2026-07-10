// env.d.ts — ambient typing for the DEV-only E2E bridge that the offline specs drive (07-07).
// The production bundle never defines `window.__enviTest`; it exists only under `import.meta.env.DEV`.

import type { EnviTestBridge } from "../../src/testBridge";

declare global {
  interface Window {
    __enviTest: EnviTestBridge;
  }
}

export {};
