/**
 * The one piece of arithmetic that stands between the Engine's truth and what is drawn.
 *
 * Every case here is a rule from ADR-0018 rather than a property of the formula: the UI derives
 * instead of guessing, it never runs past the Engine, and it never concludes an arrival.
 */

import { describe, expect, it } from "vitest";

import { progressAt, type Base } from "./useTravel";

/** What the Engine last said, heard at `t = 1000ms`. */
function heard(overrides: Partial<Base> = {}): Base {
  return { progress: 0, at: 1000, etaSeconds: 10, toward: "library", ...overrides };
}

describe("drawing between two authoritative states", () => {
  it("is where the Engine would say it is, not where an easing curve would", () => {
    // Halfway through the reported ETA is halfway along the remaining journey. Linear because
    // the Engine's own model is linear — a walk at a constant speed — and any curve here would
    // be the UI inventing acceleration nobody is doing.
    expect(progressAt(heard(), 6000)).toBeCloseTo(0.5, 5);
    expect(progressAt(heard(), 1000)).toBeCloseTo(0, 5);
  });

  it("spreads what is left over the seconds that are left", () => {
    // A correction arriving mid-walk says "you are 40% along with 6 seconds to go". Three
    // seconds later is 40% + half of the remaining 60%.
    const correction = heard({ progress: 0.4, etaSeconds: 6 });
    expect(progressAt(correction, 4000)).toBeCloseTo(0.7, 5);
  });

  it("stops at the door rather than walking past it", () => {
    // The heartbeat is coarser than a frame, so the ETA can elapse before the Engine has said
    // `Arrived`. Standing at the destination is a position; having arrived is a fact, and only
    // the Engine states facts.
    expect(progressAt(heard(), 30_000)).toBe(1);
  });

  it("treats no time left as being there", () => {
    // Not a division by zero, and not a jump back to the start. The Engine said there is
    // nothing left of this walk.
    expect(progressAt(heard({ etaSeconds: 0 }), 1000)).toBe(1);
  });
});
