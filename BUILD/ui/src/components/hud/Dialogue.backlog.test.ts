/**
 * What is on screen when you walk in has already happened — and who said it has to be known
 * before that can be marked.
 *
 * `alreadyHeard` returns at once when there is no voice, and there is no voice while the crew is
 * still empty: it arrives from the World a moment after the Chronicle does. So the first pass
 * marked *nothing*, the flag was set anyway, and the second pass read the whole backlog aloud —
 * the exact defect that flag exists to prevent, through the one door it was not watching.
 */

import { describe, expect, it } from "vitest";

import { everyoneIsKnown } from "./Dialogue";
import type { Said } from "../../ipc/world";
import type { CharacterView } from "../../ipc/contracts";

const mage = { id: "mage" } as CharacterView;

function answered(who: string | null): Said {
  return { kind: "answered", who, content: "Hola." } as unknown as Said;
}

describe("whether a backlog can be marked as already heard", () => {
  it("waits while the crew has not arrived", () => {
    // The whole defect in one line: a Chronicle with answers and nobody to attribute them to.
    expect(everyoneIsKnown([answered("mage")], [])).toBe(false);
  });

  it("is satisfied once everybody who spoke is known", () => {
    expect(everyoneIsKnown([answered("mage")], [mage])).toBe(true);
  });

  it("does not wait for somebody who cannot arrive", () => {
    // A line with no speaker at all, and a character who has left the World. Neither is coming
    // back before the next render, and waiting for them would be waiting forever — which reads
    // as a World that never speaks rather than as one that spoke twice.
    expect(everyoneIsKnown([answered(null)], [])).toBe(true);
    expect(everyoneIsKnown([answered("ghost")], [mage])).toBe(false);
  });

  it("an empty Chronicle is known, so walking into a new conversation is not held up", () => {
    expect(everyoneIsKnown([], [])).toBe(true);
  });
});
