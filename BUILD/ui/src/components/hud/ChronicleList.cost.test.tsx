/**
 * What a long Chronicle costs to draw.
 *
 * ## Why measure before virtualising
 *
 * The roadmap's claim is 5,000 entries at 60 fps, and the plan is a virtualised list. Windowing
 * a list is not free: it takes ownership of scroll position, breaks find-in-page and browser
 * search, and makes "scroll to the bottom when a new line arrives" — which this dialogue does on
 * every token — something to implement rather than something the browser does. So the number
 * comes first, like the vault sweep and the history read.
 *
 * ## What this measures, and what it cannot
 *
 * jsdom lays nothing out and paints nothing, so this is **not** a frame rate. What it measures
 * is what a real browser would then have to work with: how many DOM nodes exist, and how long
 * React spends building them. Both are honest, and the node count is the one that decides
 * whether a browser can scroll a list smoothly — a paint budget is spent per node, not per
 * message.
 */

import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";

vi.mock("../../ipc/world", () => ({
  sharedImage: () => Promise.resolve(null),
}));

import { ChronicleList } from "./ChronicleList";
import type { CharacterView } from "../../ipc/contracts";
import type { Said } from "../../ipc/world";

function person(id: string, name: string): CharacterView {
  return {
    id,
    name,
    archetype: "guardian",
    home: "tower",
    place: "tower",
    activity: "reading",
    action: "idle",
    actions: {},
    class: "idle",
    mark: null,
    icon: null,
    speaksWith: null,
    soundsLike: null,
    routine: [],
  };
}

/**
 * A conversation of `count` lines, alternating the way a real one does.
 *
 * The text is the length a person actually types, because a measurement against one-word
 * messages would flatter whatever it is measuring.
 */
function conversation(count: number): Said[] {
  return Array.from({ length: count }, (_, i) => ({
    who: i % 2 === 0 ? null : "mage",
    content:
      i % 2 === 0
        ? `and what about ${i}? here is the sort of sentence somebody types into a chat box when they are explaining what they want.`
        : `answer ${i}: about as long as an answer tends to be when somebody is explaining what they just did and why they did it that way.`,
    attachments: [],
    images: [],
    kind: i % 2 === 0 ? ("said" as const) : ("answered" as const),
  }));
}

const SIZES = [50, 500, 5000];

/**
 * Stable, because that is what the real caller passes.
 *
 * A fresh arrow per render is a changed prop per render, and measuring against one would be
 * measuring a caller nobody has rather than the list.
 */
const onOpenLink = () => {};

describe("the cost of a long Chronicle", () => {
  it("draws every line, and this is what that costs", () => {
    const mage = person("mage", "Mage");
    const readings: string[] = [];

    for (const size of SIZES) {
      const began = performance.now();
      const { container, unmount } = render(
        <ChronicleList
          who={mage}
          model="gemma4:12b"
          crew={[mage]}
          said={conversation(size)}
          writing=""
          thinking={false}
          compacting={null}
          working={[]}
          onOpenLink={onOpenLink}
        />,
      );
      const took = performance.now() - began;
      const nodes = container.querySelectorAll("*").length;

      readings.push(
        `${String(size).padStart(5)} lines  ${nodes
          .toString()
          .padStart(7)} nodes  ${took.toFixed(0).padStart(6)} ms  ${(
          nodes / size
        ).toFixed(1)} nodes/line`,
      );
      // Every line is drawn — which is the thing being measured, and the thing virtualising
      // would change.
      expect(nodes).toBeGreaterThan(size);
      unmount();
    }

    console.log(`\n${readings.join("\n")}\n`);
  }, 120_000);

  it("pays for the whole list again on every streamed token", () => {
    // **The reading that decides what the fix is.** A dialogue re-renders while a character
    // writes — `writing` changes on every token — so the question is not what the list costs to
    // build once, it is what it costs to leave standing while somebody talks.
    const mage = person("mage", "Mage");
    const said = conversation(5000);
    const crew = [mage];

    const draw = (writing: string) => (
      <ChronicleList
        who={mage}
        model="gemma4:12b"
        crew={crew}
        said={said}
        writing={writing}
        thinking={false}
        compacting={null}
        working={[]}
        onOpenLink={onOpenLink}
      />
    );

    const { rerender, unmount } = render(draw(""));

    // Ten tokens: a short sentence, and about a second of a local model talking.
    const began = performance.now();
    for (let token = 1; token <= 10; token += 1) {
      rerender(draw("a".repeat(token)));
    }
    const each = (performance.now() - began) / 10;

    console.log(`\n  one token, 5000 lines standing: ${each.toFixed(1)} ms\n`);
    unmount();
    // **The regression this file exists to catch.** It was 138 ms, which is a second of work
    // for every second a local model talks — the window fell behind and never caught up. The
    // settled lines are memoised now, so a token costs the live part and nothing else.
    //
    // Generous on purpose: a threshold near the real number would fail on a loaded machine and
    // teach somebody to ignore it. What must not come back is the old cost, and the two are two
    // orders of magnitude apart.
    expect(each).toBeLessThan(20);
  }, 120_000);
});
