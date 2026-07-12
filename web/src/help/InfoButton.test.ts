// InfoButton.test.ts — unit coverage for the reusable info affordance (D-23). The project's Vitest
// runs in the `node` environment (no jsdom — a deliberate project choice), so this renders the
// component and its popover with `react-dom/server` (`renderToStaticMarkup`, no DOM needed) and asserts
// the static output. Full click→popover→"More"→docked-panel INTERACTION is exercised on the real bundle
// by `tests/e2e/infoButton.spec.ts` (the project's DOM-behaviour tier is Playwright, not jsdom).

import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { InfoButton, HelpPopover } from "./InfoButton";
import { catalog } from "./catalog";

describe("InfoButton", () => {
  it("renders the glyph button with an accessible help label + testid for its controlId", () => {
    const html = renderToStaticMarkup(createElement(InfoButton, { controlId: "spectrum.split" }));
    expect(html).toContain('data-testid="info-spectrum.split"');
    expect(html).toContain(`aria-label="Help: ${catalog["spectrum.split"].title}"`);
    // Closed by default — no popover in the static (closed) render.
    expect(html).toContain('aria-expanded="false"');
    expect(html).not.toContain("info-popover");
  });

  it("popover shows the title, the glance paragraph, a citation and a More affordance", () => {
    const entry = catalog["conditioning.delay"];
    const html = renderToStaticMarkup(
      createElement(HelpPopover, { controlId: "conditioning.delay", entry, onMore: () => {} }),
    );
    expect(html).toContain(entry.title);
    // The glance = the first body paragraph (a substring of it, robust to HTML entity encoding).
    expect(html).toContain(entry.body[0].slice(0, 24));
    // The "More" affordance opens the docked depth panel.
    expect(html).toContain('data-testid="info-more-conditioning.delay"');
    expect(html).toContain(">More<");
    // A standards citation is surfaced on the glance line.
    expect(html).toContain(entry.citations[0].ref);
  });
});
