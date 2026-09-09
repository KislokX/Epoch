/**
 * What a row says a benchmark found.
 *
 * **The scoring lives in the Engine and there is no second copy here.** There was one, mirroring
 * `trials::Outcome::score`, and the two disagreed the first time a real card reached the screen:
 * the row read `follow 0%` beside the Engine's own `following 8%`. What is left to hold is the
 * rendering — and the rule the owner added on 2026-08-31, which is that the three rates must
 * never be collapsed back into one on the way to the screen.
 */

import { describe, expect, it } from "vitest";

import { summarise } from "./ModelsHere";
import type { BenchCard, BenchColumn } from "../ipc/launcher";

const column = (
  kind: BenchColumn["kind"],
  correct: number,
  answered: number,
  asked: number,
): BenchColumn => ({
  kind,
  correct,
  answered,
  asked,
  correctness: answered > 0 ? correct / answered : null,
  completion: asked > 0 ? answered / asked : null,
  effective: asked > 0 ? correct / asked : null,
});

const card = (columns: BenchColumn[]): BenchCard =>
  ({
    model: "m",
    conditions: { context: 32_768 },
    speed: { runs: [{ generation: 48.5, failed: null }] },
    answers: [],
    columns,
  }) as unknown as BenchCard;

describe("a row's summary of a card", () => {
  it("leads with what was asked and how much of it came back right", () => {
    /*
        The owner's arithmetic: four of five reasoning trials answered, all four right. Accurate
        on everything it said, and it finished four fifths of what it was given.

        Printing all three as percentages with their own fractions made a row nobody could read
        — nine numbers, six of them repeats, three words a person has to be taught. The fraction
        that answers the question leads, and the shortfall is named as the one reason it is not
        five out of five.
    */
    const [, reasoning] = summarise(card([column("reasoning", 4, 4, 5)]));
    expect(reasoning).toContain("4 of 5 right");
    expect(reasoning).toContain("1 unanswered");
    // Nothing was answered wrongly, so nothing says it was.
    expect(reasoning).not.toContain("wrong");
  });

  it("names both shortfalls where there are two", () => {
    // Three asked, two answered, one of those right. The row has to say each thing once.
    const [, coding] = summarise(card([column("coding", 1, 2, 3)]));
    expect(coding).toContain("1 of 3 right");
    expect(coding).toContain("1 wrong");
    expect(coding).toContain("1 unanswered");
  });

  it("says nothing more about a column that was perfect", () => {
    /*
        The second half exists to explain a shortfall. A column with none has nothing to explain,
        and printing `0 wrong . 0 unanswered` would be a gauge that never moves.
    */
    const [, reasoning] = summarise(card([column("reasoning", 5, 5, 5)]));
    expect(reasoning).toContain("5 of 5 right");
    expect(reasoning).not.toContain("wrong");
    expect(reasoning).not.toContain("unanswered");
  });

  it("never reads as a failure where nothing ran", () => {
    /*
        A model whose coding trials could not run is not a model that failed them. `0 of 3 right`
        with `3 unanswered` beside it says exactly that: none was answered, so none was wrong.
    */
    const [, coding] = summarise(card([column("coding", 0, 0, 3)]));
    expect(coding).toContain("0 of 3 right");
    expect(coding).toContain("3 unanswered");
    expect(coding).not.toContain("wrong");
  });

  it("counts a fractional score as fractional, because a tool call is judged in parts", () => {
    // 0.2 of one call: it called something, and nothing else was right. Rounding that to 0 would
    // hide the one thing it did do.
    const [, tools] = summarise(card([column("tools", 1.2, 2, 2)]));
    expect(tools).toContain("1.2 of 2 right");
    expect(tools).toContain("0.8 wrong");
  });

  it("says nothing rather than a rate for a kind that was never asked", () => {
    const [, missing] = summarise(card([column("tools", 0, 0, 0)]));
    expect(missing).toContain("not asked");
  });

  it("leads with the conditions, because a rate without them is a fact about nothing", () => {
    expect(summarise(card([column("reasoning", 5, 5, 5)]))[0]).toBe(
      "32K \u00b7 48.5 tok/s",
    );
  });
});
