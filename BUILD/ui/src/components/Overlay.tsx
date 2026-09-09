/**
 * A window over everything else, and the four things every one of them has to get right.
 *
 * ## Why this exists
 *
 * Epoch has several of these — Missions, New World, the crew roster, the World Editor's dialogs
 * — and each one was arriving at the same four rules on its own:
 *
 * - **Pointer events.** The HUD's band is `pointer-events: none` so the World stays reachable
 *   through the gaps, and every panel turns it back on *for itself* (`CLAUDE.md`). Missions
 *   forgot: its ✕ rendered, was aimed at, clicked — and the click went through to the World.
 * - **Escape.** Some closed on it, some did not. A key that works in one window and not the next
 *   is worse than one that never works, because the user stops trusting it.
 * - **Focus.** None of them moved focus in, so a keyboard user tabbed through the World behind
 *   the dialog; and none put it back, so closing one left the focus ring nowhere.
 * - **The scrim.** Clicking outside closed New World and did nothing in Missions.
 *
 * Four rules, five implementations, and every new window a chance to be the one that forgets.
 * Here they are once, and a window that wants different behaviour has to *say so*.
 *
 * ## What it deliberately does not do
 *
 * It does not draw. There is no frame, no header, no ✕ — those belong to the window, which knows
 * what it is. This owns the *modality*: what is beneath, what closes it, and where the focus is.
 */

import { useEffect, useRef, type ReactNode } from "react";

import { playSfx } from "../experience/sfx";

interface OverlayProps {
  readonly children: ReactNode;
  /** Named for a screen reader, because a dialog with no name is "dialog". */
  readonly label: string;
  readonly onClose: () => void;
  /**
   * Extra classes for the scrim — a different tone, a different stacking order.
   *
   * The layout (fill the screen, centre the child, take pointer events) belongs to `.overlay`
   * and is not a caller's decision: those are the parts that were forgotten.
   */
  readonly className?: string;
  /**
   * Whether clicking the scrim closes it. Default yes.
   *
   * `false` for a window in the middle of something the user would not want dismissed by a
   * stray click — a form half filled in, a sequence part-way through.
   */
  readonly dismissOnScrim?: boolean;
}

export function Overlay({
  children,
  label,
  onClose,
  className,
  dismissOnScrim = true,
}: OverlayProps) {
  const scrim = useRef<HTMLDivElement>(null);
  /**
   * Where the focus was before this opened.
   *
   * Captured in a ref during the first render rather than in the effect, so it is the element
   * that was focused when the user acted — by the time an effect runs, focus may already have
   * moved into the new subtree.
   */
  const cameFrom = useRef<Element | null>(
    typeof document === "undefined" ? null : document.activeElement,
  );

  useEffect(() => {
    playSfx("open");
    // Into the window, so the next Tab is inside it rather than in the World behind it.
    const first = scrim.current?.querySelector<HTMLElement>(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
    );
    first?.focus();

    const previous = cameFrom.current;
    return () => {
      playSfx("close");
      // And back where it was. A focus ring left nowhere is a keyboard user losing their place.
      if (previous instanceof HTMLElement && document.contains(previous)) previous.focus();
    };
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      // Stopped here, so one Escape means one thing: closing the window, never also leaving the
      // Place behind it.
      event.stopPropagation();
      onClose();
    };
    // Capture, so this wins over anything listening on the way up — including the World's own
    // key handling, which is exactly what a modal window is for.
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);

  return (
    <div
      ref={scrim}
      className={`overlay${className ? ` ${className}` : ""}`}
      role="dialog"
      aria-modal="true"
      aria-label={label}
      onPointerDown={(event) => {
        // The scrim itself, not something inside it. Without the check, a drag that began in
        // the window and ended on the scrim would close it.
        if (dismissOnScrim && event.target === event.currentTarget) onClose();
      }}
    >
      {children}
    </div>
  );
}
