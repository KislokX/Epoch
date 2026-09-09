/**
 * Drawing the space between two authoritative states.
 *
 * ## The rule this obeys
 *
 * The Engine owns reality; the UI owns animation (ADR-0018). The Engine publishes a departure,
 * roughly ten corrections and an arrival — coarse on purpose — and the World draws sixty frames
 * a second in between. Something has to fill that gap, and there are only two ways to do it:
 * guess, or *derive*.
 *
 * This derives. Every authoritative `journey` carries the speed and the ETA the Engine used, so
 * the position drawn between messages is the position the Engine would give if asked. The next
 * authoritative state always wins, immediately and without smoothing — a correction is the truth
 * arriving, not a competing opinion to be blended with.
 *
 * ## What it may never do
 *
 * Decide that somebody has arrived. Progress reaching 1.0 means *standing at the door*, and the
 * figure stays a traveller until the Engine says `Arrived` — because arrival changes who a Place
 * holds, and that is not a conclusion a renderer is allowed to reach (ADR-0018).
 *
 * ## Why it belongs in `experience/`
 *
 * This is Experience *Playback*: per-frame, and it may never leave presentation (ADR-0022). The
 * Experience *State* it interpolates — who is walking, from where, to where — is the Engine's,
 * and is testable with no window at all.
 */

import { useEffect, useRef, useState } from "react";

import type { CharacterView } from "../ipc/contracts";

/** The last thing the Engine said about one walk, and when we heard it. */
export interface Base {
  readonly progress: number;
  /** `performance.now()` when this arrived. */
  readonly at: number;
  readonly etaSeconds: number;
  /** Which walk this is, so a new destination resets rather than continues. */
  readonly toward: string;
}

/**
 * How far along each traveller is *right now*, keyed by character.
 *
 * Characters standing still are absent — the map is empty in the overwhelmingly common case,
 * and an empty map is also what stops the animation frame loop.
 */
export function useTravel(characters: readonly CharacterView[]): ReadonlyMap<string, number> {
  const bases = useRef(new Map<string, Base>());
  // A counter rather than the positions themselves: the positions are derived on read, and
  // storing them would be a second copy that can disagree with the clock that produced it.
  const [, redraw] = useState(0);

  const walking = characters.filter((who) => who.journey);

  // Take in whatever the Engine last said. Done during render rather than in an effect so the
  // very first frame after a departure is already drawn from the authoritative state.
  const seen = new Set<string>();
  for (const who of walking) {
    const journey = who.journey;
    if (!journey) continue;
    seen.add(who.id);
    const known = bases.current.get(who.id);
    // A correction for the same walk replaces the base; a different destination is a different
    // walk and starts over.
    if (
      !known ||
      known.toward !== journey.to ||
      known.progress !== journey.progress ||
      known.etaSeconds !== journey.etaSeconds
    ) {
      bases.current.set(who.id, {
        progress: journey.progress,
        at: performance.now(),
        etaSeconds: journey.etaSeconds,
        toward: journey.to,
      });
    }
  }
  // Somebody who stopped walking stops being interpolated. Left behind, their base would be
  // waiting to be mistaken for the next walk they make.
  for (const id of [...bases.current.keys()]) {
    if (!seen.has(id)) bases.current.delete(id);
  }

  // One frame loop, and only while somebody is actually walking. A World where nobody is going
  // anywhere costs nothing, which is nearly always.
  const anybody = walking.length > 0;
  useEffect(() => {
    if (!anybody) return;
    let running = true;
    let frame = 0;
    const step = () => {
      if (!running) return;
      redraw((n) => n + 1);
      frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    return () => {
      running = false;
      cancelAnimationFrame(frame);
    };
  }, [anybody]);

  const now = performance.now();
  const progress = new Map<string, number>();
  for (const who of walking) {
    const base = bases.current.get(who.id);
    if (base) progress.set(who.id, progressAt(base, now));
  }
  return progress;
}

/**
 * Where the Engine would say this walk is, at `now`.
 *
 * Pure, and separate from the hook, because this is the only arithmetic in the file and it is
 * the part that can be wrong. It is *derivation*, not easing: the remaining fraction of the
 * journey spread across the remaining seconds the Engine itself reported.
 *
 * Never past the end. Standing at the door is a position; having arrived is a fact, and only
 * the Engine states facts (ADR-0018).
 */
export function progressAt(base: Base, now: number): number {
  if (base.etaSeconds <= 0) return 1;
  const elapsed = (now - base.at) / 1000;
  const left = 1 - base.progress;
  return Math.min(1, base.progress + left * (elapsed / base.etaSeconds));
}
