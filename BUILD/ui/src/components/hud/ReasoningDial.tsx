/**
 * How hard somebody thinks, one click away.
 *
 * ## Why this is its own component
 *
 * It is the first piece extracted from `Dialogue.tsx`, and the reason is a test rather than
 * tidiness: the rule *"a brain with no measured ladder shows no control"* could not be asserted
 * without rendering a 1,200-line component and every prop it needs. A rule nothing can check is a
 * rule that will be broken by the next person who has not read this comment.
 *
 * ## What it knows, and what it refuses to know
 *
 * It knows **nothing about brains**. The rungs, their labels, their order and whether there are
 * any at all arrive from the Engine (`deliberation::scale`), because granularity is a fact about
 * a backend: a local model has five, Claude Code has five *different* ones including `xhigh`, and
 * Haiku has none at all. A component that held that list would be wrong for every backend except
 * the one it was written against — and wrong silently.
 *
 * ## Absent, not cold
 *
 * A cold instrument says *this exists and currently reads nothing*, which is the right answer for
 * a gauge with no data. Here it would be a lie: there is no dial to read, because the model has
 * no effort levels. So the control is not drawn at all.
 */

import { useState } from "react";

import type { Dial } from "../../ipc/world";

interface ReasoningDialProps {
  /** The ladder and where they sit on it. `null` while it is still being asked for. */
  readonly dial: Dial | null;
  /** Whose deliberation this is, for the button's title. */
  readonly who: string;
  /**
   * Move them. `null` means the brain's own default.
   *
   * Called with the value the user just chose; the parent owns writing it and putting the real
   * one back if the Engine refuses.
   */
  readonly onChoose: (rung: string | null) => void;
}

export function ReasoningDial({ dial, who, onChoose }: ReasoningDialProps) {
  const [open, setOpen] = useState(false);

  // No ladder, no control. Not a disabled button, not a cold reading — nothing.
  if (!dial || dial.rungs.length === 0) return null;

  const at = dial.chosen ? dial.rungs.findIndex((r) => r.id === dial.chosen) + 1 : 0;

  return (
    <>
      <button
        type="button"
        className={`dlg__ctxbtn dlg__effbtn${dial.chosen ? "" : " dlg__ctxbtn--cold"}`}
        onClick={() => setOpen((was) => !was)}
        aria-expanded={open}
        /*
          Named, rather than left to its own text.

          Without this the accessible name is the concatenation of its two spans — "REASONINGAUTO"
          — which is what a screen reader would say and what a test would have to match. Both are
          reasons to say it properly once.
        */
        aria-label={`How hard ${who} deliberates`}
        title={`How hard ${who} deliberates`}
      >
        <span>REASONING</span>
        <b>
          {dial.chosen
            ? (dial.rungs.find((r) => r.id === dial.chosen)?.label ?? dial.chosen)
            : "AUTO"}
        </b>
      </button>

      {open && (
        /*
          A slider, because the thing being chosen is *an amount*. Radio buttons would draw five
          unrelated options; a ladder is one axis with a direction, and the direction is the
          information: left is faster, right thinks longer.

          Position 0 is the brain's own default and it is a real position, not an absence —
          "nothing was asked for" is not "the least", and a scale starting at the lowest rung
          would quietly turn every character into one that barely deliberates.
        */
        <div className="dlg__effpop" role="dialog" aria-label="Reasoning">
          <div className="dlg__ctxhead">
            <span>REASONING</span>
            <b>{dial.brain}</b>
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

          <input
            className="dlg__effslide"
            type="range"
            min={0}
            max={dial.rungs.length}
            step={1}
            value={at}
            onChange={(e) => {
              const to = Number(e.target.value);
              onChoose(to === 0 ? null : (dial.rungs[to - 1]?.id ?? null));
            }}
            aria-label="How hard they deliberate"
          />

          <div className="dlg__effmarks" aria-hidden>
            <i className={dial.chosen ? undefined : "on"}>Auto</i>
            {dial.rungs.map((r) => (
              <i key={r.id} className={dial.chosen === r.id ? "on" : undefined}>
                {r.label}
              </i>
            ))}
          </div>

          <p className="dlg__effnote">
            {dial.chosen
              ? "Faster on the left, longer thinking on the right. Kept with the character."
              : `Whatever ${dial.brain} does by default — nothing is being asked for.`}
          </p>
        </div>
      )}
    </>
  );
}
