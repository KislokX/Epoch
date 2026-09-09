import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

/**
 * Everything the crew has actually run, newest last.
 *
 * ## Why this is not part of `useTurn`
 *
 * `useTurn` follows *one* conversation: it filters steps to the character being spoken to and
 * forgets them when the turn ends, which is right for a dialogue box. The Terminal answers a
 * different question — **what is happening in this World right now** — so it listens to every
 * character and keeps what it heard.
 *
 * ## It reports; it never narrates
 *
 * Every line here is a real `turn:step` from the Engine: a capability that was judged, allowed
 * and started, and then how it ended. Nothing is synthesised, nothing is predicted, and a quiet
 * Terminal means nothing is running — which is information, not a gap to fill
 * (LIVING_WORLD_DESIGN_GUIDE: silence and idle are honest).
 */
export interface Line {
  /** Monotonic within a session, so React has a key that two identical commands cannot share. */
  readonly id: number;
  readonly at: number;
  readonly who: string;
  readonly capability: string;
  /** The sentence the user would have approved — what it is doing, in words. */
  readonly what: string;
  /** `null` while it is still running. */
  readonly ok: boolean | null;
  readonly detail: string;
  /**
   * What it has printed so far, oldest first.
   *
   * Capped per line rather than globally: one runaway build must not push every other entry out
   * of a panel somebody is using to understand what happened.
   */
  readonly output: readonly string[];
}

/** How many printed lines one run keeps on screen. */
const TAIL = 60;

/**
 * How much is kept.
 *
 * A panel, not a log: the Activity Recorder is the durable answer and it is DESIGN NOW
 * (ADR-0015). Holding a session's worth of output in the renderer would make a long day cost
 * memory for something nobody scrolls back to.
 */
const REMEMBERED = 200;

export function useTerminal(): Line[] {
  const [lines, setLines] = useState<Line[]>([]);

  useEffect(() => {
    let next = 0;
    let stop: (() => void) | null = null;
    let cancelled = false;

    void listen<{
      characterId: string;
      phase: "using" | "said" | "used";
      capability: string;
      what?: string;
      line?: string;
      ok?: boolean;
      detail?: string;
    }>("turn:step", (event) => {
      const step = event.payload;
      next += 1;
      const id = next;
      setLines((current) => {
        if (step.phase === "using") {
          const line: Line = {
            id,
            at: Date.now(),
            who: step.characterId,
            capability: step.capability,
            what: step.what ?? step.capability,
            ok: null,
            detail: "",
            output: [],
          };
          return [...current, line].slice(-REMEMBERED);
        }

        if (step.phase === "said") {
          const printed = step.line ?? "";
          for (let i = current.length - 1; i >= 0; i -= 1) {
            const candidate = current[i];
            if (
              candidate &&
              candidate.ok === null &&
              candidate.capability === step.capability &&
              candidate.who === step.characterId
            ) {
              const grown: Line = {
                ...candidate,
                output: [...candidate.output, printed].slice(-TAIL),
              };
              return current.map((l, at) => (at === i ? grown : l));
            }
          }
          // Nothing is running that this could belong to. Dropped rather than invented: a line
          // with no run to attach it to would be output attributed to nothing.
          return current;
        }

        // Close the newest line still running for this character and capability. Matched rather
        // than indexed, so two reads finishing out of order cannot swap their results — the
        // same reasoning `useTurn` uses, for the same reason.
        for (let i = current.length - 1; i >= 0; i -= 1) {
          const candidate = current[i];
          if (
            candidate &&
            candidate.ok === null &&
            candidate.capability === step.capability &&
            candidate.who === step.characterId
          ) {
            const closed: Line = {
              ...candidate,
              ok: step.ok ?? false,
              detail: step.detail ?? "",
            };
            return current.map((l, at) => (at === i ? closed : l));
          }
        }
        // A finish with no start: kept rather than dropped. Losing evidence of something that
        // ran would make this panel a nicer-looking lie than no panel at all.
        const orphan: Line = {
          id,
          at: Date.now(),
          who: step.characterId,
          capability: step.capability,
          what: step.capability,
          ok: step.ok ?? false,
          detail: step.detail ?? "",
          output: [],
        };
        return [...current, orphan].slice(-REMEMBERED);
      });
    }).then((off) => {
      if (cancelled) off();
      else stop = off;
    });

    return () => {
      cancelled = true;
      stop?.();
    };
  }, []);

  return lines;
}
