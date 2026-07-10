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

createRoot(rootEl).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
