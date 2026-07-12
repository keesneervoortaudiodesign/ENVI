// InfoButton.tsx — the reusable app-wide info affordance (D-23). A 16px "i"-in-a-ring glyph
// placed immediately after a control's label (or trailing a panel header). It has TWO depths:
// a POPOVER for a glance (title + the first paragraph + the citation + a "More" affordance) and,
// via "More", a docked right-rail HELP PANEL for the full multi-paragraph body (D-23 presentation).
//
// # Module I/O
// - Input  a `controlId` (must be in the `ControlId` union — a control cannot render an InfoButton
//   for an id the catalog does not cover; the `Record<ControlId, HelpEntry>` makes that a `tsc`
//   error). All help text is looked up from the structured `catalog` (D-25), never inlined here.
// - Output the glyph button + (when open) the popover + (via More) a portalled docked help panel.
//   Every string reaches the DOM as a React text child (no raw-HTML sink). Theme-aware via metrao3
//   tokens, with a `:focus-visible` accent ring and full keyboard/Esc handling (a11y).
//
// SSR/Node-safe: `document` is only touched inside effects and event handlers (never during render),
// so `renderToStaticMarkup` in the Node unit test renders the closed button without a DOM. The
// docked panel uses a portal, which is only mounted when opened (never during a closed SSR render).

import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type ReactElement, type RefObject } from "react";
import { createPortal } from "react-dom";

import { svgIcon } from "../icons";
import { catalog, type Citation, type HelpEntry } from "./catalog";
import type { ControlId } from "./controlIds";

const STYLE_ID = "envi-info-button-styles";

// Inject the InfoButton stylesheet once (metrao3 tokens only — no invented colours). Kept in-module
// so the affordance is fully self-contained (no app.css edit) and offline. No-op on the server.
function ensureHelpStyles(): void {
  if (typeof document === "undefined" || document.getElementById(STYLE_ID)) {
    return;
  }
  const style = document.createElement("style");
  style.id = STYLE_ID;
  style.textContent = `
.info-button-wrap { position: relative; display: inline-flex; vertical-align: middle; }
.info-button {
  display: inline-flex; align-items: center; justify-content: center;
  width: 20px; height: 20px; padding: 2px; margin-left: 4px; line-height: 0;
  border: none; background: transparent; color: var(--color-text-dim);
  cursor: pointer; border-radius: var(--radius-full);
}
.info-button:hover { color: var(--color-text-muted); }
.info-button[aria-expanded="true"] { color: var(--color-info); }
.info-button:focus-visible { outline: 2px solid var(--color-primary); outline-offset: 1px; color: var(--color-info); }
.info-popover {
  position: fixed; z-index: 1000;
  width: 288px; max-width: 80vw; text-align: left;
  background: var(--color-surface-2); color: var(--color-text);
  border: 1px solid var(--color-border-strong); border-radius: var(--radius-md);
  box-shadow: var(--shadow-lg); padding: var(--space-3);
  font-size: var(--text-xs); font-weight: 400;
}
.info-popover-title { margin: 0 0 var(--space-2); font-size: var(--text-sm); font-weight: 600; color: var(--color-text-strong); }
.info-popover-body { margin: 0 0 var(--space-3); line-height: 1.45; }
.info-popover-actions { display: flex; align-items: center; justify-content: space-between; gap: var(--space-2); }
.info-cite { font-size: var(--text-xxs); color: var(--color-text-muted); }
.info-more {
  border: 1px solid var(--color-border-strong); background: var(--color-surface-3);
  color: var(--color-primary); border-radius: var(--radius-sm);
  padding: 2px var(--space-2); font-size: var(--text-xxs); cursor: pointer;
}
.info-more:hover { color: var(--color-primary-hover); }
.info-more:focus-visible { outline: 2px solid var(--color-primary); outline-offset: 1px; }
.help-dock {
  position: fixed; top: 0; right: 0; bottom: 0; width: 380px; max-width: 92vw; z-index: 1001;
  background: var(--color-surface); color: var(--color-text);
  border-left: 1px solid var(--color-border-strong); box-shadow: var(--shadow-lg);
  padding: var(--space-5); overflow: auto; text-align: left;
}
.help-dock-head { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--space-3); margin-bottom: var(--space-3); }
.help-dock-title { margin: 0; font-size: var(--text-lg); font-weight: 600; color: var(--color-text-strong); }
.help-dock-close {
  flex: none; width: 28px; height: 28px; line-height: 0; cursor: pointer;
  border: 1px solid var(--color-border-strong); background: var(--color-surface-2);
  color: var(--color-text-muted); border-radius: var(--radius-sm); font-size: var(--text-base);
}
.help-dock-close:hover { color: var(--color-text); }
.help-dock-close:focus-visible { outline: 2px solid var(--color-primary); outline-offset: 1px; }
.help-dock-body p { margin: 0 0 var(--space-3); line-height: 1.55; font-size: var(--text-sm); }
.help-citations { margin-top: var(--space-4); border-top: 1px solid var(--color-border); padding-top: var(--space-3); }
.help-citations-head { margin: 0 0 var(--space-2); font-size: var(--text-xs); font-weight: 600; color: var(--color-text-muted); text-transform: uppercase; letter-spacing: 0.04em; }
.help-citation { margin: 0 0 var(--space-2); font-size: var(--text-xs); color: var(--color-text-muted); line-height: 1.4; }
.help-cite-tag { display: inline-block; margin-right: var(--space-2); padding: 1px var(--space-2); border-radius: var(--radius-sm); background: var(--color-surface-3); color: var(--color-text); font-family: var(--font-mono); font-size: var(--text-xxs); }
`;
  document.head.appendChild(style);
}

// A human label for a citation source family (report numbers stay load-bearing).
function citationTag(source: Citation["source"]): string {
  switch (source) {
    case "AV1106/07":
      return "Nord2000";
    case "TI386":
      return "NoizCalc";
    case "ENVI":
      return "ENVI";
    default:
      return source;
  }
}

// The DOM-constructed "i" glyph (icons.ts, never innerHTML). Appended in an effect so it renders in
// the browser; the closed-button Node render simply leaves the host empty (the test asserts the button).
function InfoGlyph(): ReactElement {
  const hostRef = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    const host = hostRef.current;
    if (!host) {
      return;
    }
    const svg = svgIcon("info");
    host.appendChild(svg);
    return () => {
      host.removeChild(svg);
    };
  }, []);
  return <span className="info-glyph" ref={hostRef} aria-hidden="true" />;
}

// The rendered citation list (shared by the popover glance line and the docked panel).
function Citations({ citations }: { readonly citations: readonly Citation[] }): ReactElement {
  return (
    <div className="help-citations" data-testid="help-citations">
      <p className="help-citations-head">Sources</p>
      {citations.map((c, i) => (
        <p className="help-citation" key={`${c.source}-${i}`}>
          <span className="help-cite-tag">{citationTag(c.source)}</span>
          {c.ref} — {c.note}
        </p>
      ))}
    </div>
  );
}

// The glance POPOVER: title + first paragraph + the primary citation + a "More" affordance. Pure and
// inline (no portal) so the Node unit test can render + assert it via renderToStaticMarkup.
export function HelpPopover({
  controlId,
  entry,
  onMore,
  style,
  popoverRef,
}: {
  readonly controlId: ControlId;
  readonly entry: HelpEntry;
  readonly onMore: () => void;
  readonly style?: CSSProperties;
  readonly popoverRef?: RefObject<HTMLDivElement | null>;
}): ReactElement {
  const primary = entry.citations[0];
  return (
    <div
      className="info-popover"
      role="dialog"
      aria-label={`Help: ${entry.title}`}
      data-testid={`info-popover-${controlId}`}
      ref={popoverRef}
      style={style}
    >
      <p className="info-popover-title">{entry.title}</p>
      <p className="info-popover-body">{entry.body[0]}</p>
      <div className="info-popover-actions">
        {primary ? (
          <span className="info-cite">
            {citationTag(primary.source)} · {primary.ref}
          </span>
        ) : (
          <span />
        )}
        <button
          type="button"
          className="info-more"
          data-testid={`info-more-${controlId}`}
          onClick={onMore}
        >
          More
        </button>
      </div>
    </div>
  );
}

// The docked right-rail HELP PANEL: the full multi-paragraph body + every citation.
function HelpDock({
  controlId,
  entry,
  onClose,
}: {
  readonly controlId: ControlId;
  readonly entry: HelpEntry;
  readonly onClose: () => void;
}): ReactElement {
  return (
    <aside
      className="help-dock"
      role="dialog"
      aria-label={`Help: ${entry.title}`}
      data-testid={`help-dock-${controlId}`}
    >
      <div className="help-dock-head">
        <h2 className="help-dock-title">{entry.title}</h2>
        <button
          type="button"
          className="help-dock-close"
          aria-label="Close help"
          data-testid={`help-dock-close-${controlId}`}
          onClick={onClose}
        >
          &times;
        </button>
      </div>
      <div className="help-dock-body">
        {entry.body.map((para, i) => (
          <p key={i}>{para}</p>
        ))}
      </div>
      <Citations citations={entry.citations} />
    </aside>
  );
}

// The public affordance. Renders the glyph button; clicking it toggles the glance popover; "More"
// opens the docked panel for depth. One component, two depths (D-23).
export function InfoButton({ controlId }: { readonly controlId: ControlId }): ReactElement {
  const entry = catalog[controlId];
  const [open, setOpen] = useState(false);
  const [docked, setDocked] = useState(false);
  const [popStyle, setPopStyle] = useState<CSSProperties | null>(null);
  const wrapRef = useRef<HTMLSpanElement>(null);
  const btnRef = useRef<HTMLButtonElement>(null);
  const popRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    ensureHelpStyles();
  }, []);

  // Anchor the (portalled, position:fixed) popover to the button — right-aligned, just below it — so it
  // renders above every panel/map stacking context instead of being clipped/intercepted by the shell.
  useLayoutEffect(() => {
    if (open && btnRef.current && typeof window !== "undefined") {
      const r = btnRef.current.getBoundingClientRect();
      setPopStyle({ top: Math.round(r.bottom + 6), right: Math.round(window.innerWidth - r.right) });
    }
  }, [open]);

  // Close the popover on an outside click / Escape (the dock has its own close). Torn down on unmount.
  useEffect(() => {
    if (!open) {
      return;
    }
    const onDown = (e: MouseEvent): void => {
      const t = e.target as Node;
      const inWrap = wrapRef.current?.contains(t) ?? false;
      const inPop = popRef.current?.contains(t) ?? false;
      if (!inWrap && !inPop) {
        setOpen(false);
      }
    };
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === "Escape") {
        setOpen(false);
      }
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  // Close the dock on Escape.
  useEffect(() => {
    if (!docked) {
      return;
    }
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === "Escape") {
        setDocked(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [docked]);

  return (
    <span className="info-button-wrap" ref={wrapRef}>
      <button
        ref={btnRef}
        type="button"
        className="info-button"
        data-testid={`info-${controlId}`}
        aria-label={`Help: ${entry.title}`}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        <InfoGlyph />
      </button>

      {open && popStyle && typeof document !== "undefined"
        ? createPortal(
            <HelpPopover
              controlId={controlId}
              entry={entry}
              popoverRef={popRef}
              style={{ position: "fixed", ...popStyle }}
              onMore={() => {
                setOpen(false);
                setDocked(true);
              }}
            />,
            document.body,
          )
        : null}

      {docked && typeof document !== "undefined"
        ? createPortal(
            <HelpDock controlId={controlId} entry={entry} onClose={() => setDocked(false)} />,
            document.body,
          )
        : null}
    </span>
  );
}
