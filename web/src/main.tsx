// main.tsx — the ENVI SPA entry point (D-09).
//
// # Module I/O
// - Input  the `#root` mount node from index.html; the theme tokens (theme.css). No external assets.
// - Output the mounted React tree (`<App/>`, the four-region shell) rendered into `#root`.
//   StrictMode is on (React 19) so effect double-invocation surfaces lifecycle leaks early.
//   theme.css supplies the adopted tokens; app.css layers the shell/component CSS on top.

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "./theme.css";
import "./app.css";
import { App } from "./App";

const rootEl = document.getElementById("root");
if (!rootEl) {
  throw new Error("ENVI: #root mount node missing from index.html");
}

// DEV-only: expose the offline E2E's programmatic-commit bridge. Vite statically replaces
// `import.meta.env.DEV` with `false` in the production `vite build`, so rollup drops this whole branch
// (and the `./testBridge` chunk) — it never ships in web/dist. Present only under the dev server.
if (import.meta.env.DEV) {
  void import("./testBridge").then((m) => m.installTestBridge());
}

createRoot(rootEl).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
