/**
 * What the Composer actually gave a character for the current turn.
 *
 * This is measured context, not an estimate taken after an answer. A `Knew` reading may be
 * absent because the Composer has not run, or it may have a zero budget because an agent reports
 * used tokens but not the size of its window. Those are different facts and must not both become
 * zero or a percentage.
 *
 * The trigger is rendered through a slot because it belongs among the composer's controls while
 * the report must remain a sibling of the whole composer: CSS anchors that report to `.dlg__ask`.
 * Keeping that physical relationship stops the popover moving into the text field.
 */

import { useState } from "react";
import type { ReactNode } from "react";

import type { Knew } from "../../experience/useTurn";

interface ContextGaugeProps {
  readonly knew: Knew | null;
  /** Whose next turn will make an absent reading measurable. */
  readonly who: string;
  /** Places the fixed control beside reasoning without changing where the report is anchored. */
  readonly children: (trigger: ReactNode) => ReactNode;
}

const percent = (knew: Knew) => Math.round((knew.used / knew.budget) * 100);

export function ContextGauge({ knew, who, children }: ContextGaugeProps) {
  const [open, setOpen] = useState(false);
  const reading = !knew ? "—" : knew.budget > 0 ? `${percent(knew)}%` : knew.used.toLocaleString();
  const total = !knew
    ? "— / —"
    : knew.budget > 0
      ? `${knew.used.toLocaleString()} / ${knew.budget.toLocaleString()} (${percent(knew)}%)`
      : `${knew.used.toLocaleString()} used`;

  const trigger = (
    <button
      type="button"
      className={`dlg__ctxbtn${knew && knew.dropped > 0 ? " dlg__ctxbtn--over" : ""}${knew ? "" : " dlg__ctxbtn--cold"}`}
      onClick={() => setOpen((was) => !was)}
      aria-expanded={open}
      title="What this character was told this turn"
    >
      <span>CONTEXT</span>
      <b>{reading}</b>
    </button>
  );

  return (
    <>
      {children(trigger)}
      {open && (
        <div className="dlg__ctxpop" role="dialog" aria-label="Context">
          <div className="dlg__ctxhead">
            <span>CONTEXT</span>
            <b>{total}</b>
            <button
              type="button"
              className="dlg__dismiss"
              onClick={() => setOpen(false)}
              aria-label="Close"
              title="Put this away"
            >
              ✕
            </button>
          </div>
          <span className="dlg__ctxbar" aria-hidden>
            <i
              className={knew && knew.dropped > 0 ? "over" : undefined}
              style={{ width: `${knew ? Math.min(100, knew.budget > 0 ? (knew.used / knew.budget) * 100 : 0) : 0}%` }}
            />
          </span>
          {knew ? (
            <>
              <p className="dlg__knew">{knew.line}</p>
              {knew.budget === 0 && (
                <p className="dlg__knew">
                  This agent reports what it used and not how much it can hold, so there is no
                  percentage to show.
                </p>
              )}
              <p className="dlg__knew">
                {knew.dropped > 0
                  ? `${knew.dropped} older ${knew.dropped === 1 ? "block" : "blocks"} did not fit, so this turn was told less than the whole story.`
                  : "Nothing was left out."}
              </p>
              {knew.reserved > 0 && (
                <p className="dlg__knew">
                  Window {(knew.budget + knew.reserved).toLocaleString()} ·{" "}
                  {knew.reserved.toLocaleString()} kept for the reply.
                </p>
              )}
            </>
          ) : (
            <p className="dlg__knew">
              Nothing measured yet — this fills in when {who} takes a turn.
            </p>
          )}
        </div>
      )}
    </>
  );
}
