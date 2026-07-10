// main.tsx — the ENVI SPA entry point (D-09).
//
// # Module I/O
// - Input  the `#root` mount node from index.html; the theme tokens (theme.css). No external assets.
// - Output the mounted React tree rendered into `#root`. StrictMode is on (React 19) so effect
//   double-invocation surfaces lifecycle leaks early. The four-region app shell (`<App/>`) is wired
//   in here once it lands (07-05 Task 2 — App.tsx + app.css); this bootstrap owns only the mount.

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "./theme.css";

const rootEl = document.getElementById("root");
if (!rootEl) {
  throw new Error("ENVI: #root mount node missing from index.html");
}

createRoot(rootEl).render(<StrictMode />);
