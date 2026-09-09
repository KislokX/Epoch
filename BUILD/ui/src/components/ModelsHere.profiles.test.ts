/**
 * What a simplified model card says.
 *
 * **These mirror `profiles::Optimized::resolve` in the Engine, and the duplication is deliberate
 * and bounded.** The Engine is the authority — `use_profile` resolves there before it writes
 * anything — and this copy exists so the CONFIGURE panel can show four options without four
 * round trips per row per render. The examples below are the same examples the Rust tests use,
 * so the two are held against one set of facts rather than against each other.
 */

import { describe, expect, it } from "vitest";

import {
  chosenProfile,
  costOf,
  mayChoose,
  summariseTuning,
  whyAside,
  headline,
  improvement,
  oneAnswerOnly,
  outcomeOf,
  profilesOf,
  quickLine,
} from "./ModelsHere";
import type { Shown } from "./ModelsHere";
import type { Aside, Intent, Profile, WasAside } from "../ipc/launcher";
import type {
  Configuration,
  ModelHere,
  Optimized,
  Quick,
} from "../ipc/launcher";

const config = (
  context: number,
  cache: string,
  generation: number,
  over: Partial<Configuration> = {},
): Configuration =>
  ({
    loadout: { context, cache },
    tuning: { flashAttn: null, speculation: { kind: "", draft: null } },
    offload: null,
    gpuLayers: null,
    generation,
    prompt: null,
    firstTokenMs: null,
    vramUsed: null,
    ramUsed: null,
    stable: true,
    sameAnswers: true,
    // Closed on both sides, because that is the ordinary case a fixture should represent. The
    // tests that care take it away deliberately.
    bracket: {
      before: "O-001",
      beforeHeld: "reproduced",
      after: "O-002",
      afterHeld: "reproduced",
    },
    at: 1,
    ...over,
  }) as unknown as Configuration;

/**
 * What the Engine resolves, spelled out rather than recomputed.
 *
 * **A fixture that derived this would be the deleted copy wearing a test hat.** The rule lives in
 * `profiles::Optimized::resolve` and is proved by its own tests there; these say what the panel
 * does with an answer, which is a different question and the only one left on this side.
 *
 * Rows are named in intent order and each takes the configuration the Engine would have chosen.
 */
const asProfiles = (
  picks: Partial<Record<Intent, Configuration>>,
): readonly Profile[] => {
  const seen: Profile[] = [];
  for (const intent of ["auto", "balanced", "fast", "maxQuality", "longContext"] as const) {
    const one = picks[intent];
    if (one === undefined) continue;
    seen.push({
      intent,
      name: intent,
      about: "",
      sameAs: seen.find((other) => other.configuration === one)?.intent ?? null,
      configuration: one,
    });
  }
  return seen;
};

/** One configuration, offered under every goal — the shape of a model with one answer. */
const oneAnswer = (it: Configuration): readonly Profile[] =>
  asProfiles({ auto: it, balanced: it, fast: it, maxQuality: it, longContext: it });

const optimized = (tried: Configuration[], over: Partial<Optimized> = {}): Optimized =>
  ({
    model: "m",
    gpu: "a card",
    build: "b1",
    runtime: "llama_cpp",
    baseline: config(32_768, "f16", 41.0),
    tried,
    custom: null,
    chosen: null,
    usable: true,
    ...over,
  }) as unknown as Optimized;

const model = (over: Partial<ModelHere> = {}): ModelHere =>
  ({
    name: "m",
    tokensPerSecond: null,
    loadout: { context: 32_768, cache: "f16" },
    ...over,
  }) as unknown as ModelHere;

/*
  **The rule moved and its tests went with it.**

  This block held nine tests over a TypeScript copy of `profiles::Optimized::resolve`, and their
  job was to keep the copy honest. The copy is deleted — an optimisation travels as a view now, so
  the Engine can resolve once and be read — and every rule they covered is asserted in Rust,
  against the same examples:

  | what it asserted | where it lives now |
  |---|---|
  | BALANCED holds the standard context | `balanced_is_the_fastest_that_still_holds_the_standard_context` |
  | MAX QUALITY refuses an approximation | `max_quality_refuses_what_approximates_even_when_it_is_faster` |
  | harmless speculation is not charged against quality | `speculation_that_changed_nothing_does_not_count_against_quality` |
  | an unfinished configuration is never offered | `a_configuration_that_did_not_finish_is_never_offered` |
  | two intents on one row say so | `two_intents_landing_on_one_row_say_so_rather_than_hiding_one` |
  | LONG CONTEXT needs a context that varied | `long_context_is_not_offered_where_no_context_was_varied` |
  | a practical tie goes to the simpler row | `a_practical_tie_goes_to_the_simpler_configuration` |
  | LONG CONTEXT is the fastest of the longest | `long_context_is_the_most_that_was_stable_and_then_the_fastest_of_those` |
  | a range entirely above another wins | `what_is_offered_and_what_is_only_named` |

  Keeping them here would have meant a fixture that derives the answer, which is the copy again.
*/

describe("what the headline says", () => {
  it("shows nothing at all where nothing was measured", () => {
    // A plausible number is worse than a blank one.
    expect(headline(model(), undefined, undefined)).toBeNull();
  });

  it("prefers the most recent measurement over the oldest", () => {
    const it = config(32_768, "f16", 52.8);
    const found = optimized([it], { chosen: "balanced", profiles: oneAnswer(it) });
    expect(headline(model({ tokensPerSecond: 41.0 }), found, undefined)).toContain(
      "52.8 tok/s",
    );
    const recent = { generation: 55.1, context: 32_768, vramUsed: null } as Quick;
    expect(headline(model({ tokensPerSecond: 41.0 }), found, recent)).toContain(
      "55.1 tok/s",
    );
  });

  it("names the profile in force only once one has been chosen", () => {
    // Measuring something is not choosing it (ADR-0033).
    const measured = optimized([config(32_768, "f16", 52.8)]);
    expect(chosenProfile(measured)).toBeNull();
    expect(chosenProfile({ ...measured, chosen: "balanced" })).toBe("BALANCED");
  });

  it("quotes an improvement only when both halves were measured", () => {
    const it = config(32_768, "q8_0", 52.8);
    const chosen = optimized([it], { chosen: "balanced", profiles: oneAnswer(it) });
    expect(improvement(chosen)).toBe("+28.8%");
    expect(improvement({ ...chosen, baseline: null })).toBeNull();
    expect(improvement({ ...chosen, chosen: null })).toBeNull();
  });
});

describe("a quick reading", () => {
  it("says unstable rather than a rate where a run did not finish", () => {
    const broken = {
      generation: 50.0,
      context: 32_768,
      vramUsed: 11_700_000_000,
      healthy: false,
    } as Quick;
    expect(quickLine(broken)).toContain("unstable");
    expect(quickLine({ ...broken, healthy: true })).toContain("healthy");
    expect(quickLine({ ...broken, healthy: true, generation: null })).toContain(
      "no answer",
    );
  });
});

describe("what a finished search is allowed to say", () => {
  const winner = config(32_768, "f16", 53.4);
  const measured = optimized([winner], { profiles: oneAnswer(winner) });

  it("offers the recommendation a run actually produced", () => {
    const got = outcomeOf(null, measured);
    expect(got.kind).toBe("recommended");
    expect(got.kind === "recommended" && got.shown.generation).toBe(53.4);
  });

  it("never dresses a refusal in the previous run's record", () => {
    /*
      The golden test's own failure, 2026-09-01. A search refused at the gate, wrote nothing and
      paused — and the panel drew `OPTIMIZATION COMPLETE · ★ BALANCED · 53.4 tok/s · APPLY` over
      the optimization stored by the run before it. A recommendation offered on evidence this run
      did not produce is the one thing that test exists to catch.

      What the model was measured at before is still true and still on the row above. It is
      simply not what this run found.
    */
    const refused =
      "Nothing was compared. The control read 49.8 tok/s, and no reference with enough " +
      "provenance to compare against exists for this model on this card and build.";
    const got = outcomeOf(refused, measured);
    expect(got.kind).toBe("nothing");
    expect(got.kind === "nothing" && got.why).toBe(refused);
  });

  it("says why in the session's own words when a run finished with nothing usable", () => {
    const degraded = optimized([config(32_768, "f16", 53.4)], {
      usable: false,
      session: { note: "The control stopped reproducing after Flash attention · on." },
    } as unknown as Partial<Optimized>);
    const got = outcomeOf(null, degraded);
    expect(got.kind).toBe("nothing");
    expect(got.kind === "nothing" && got.why).toContain("stopped reproducing");
  });

  it("has something to say even about a model with no record at all", () => {
    const got = outcomeOf(null, undefined);
    expect(got.kind).toBe("nothing");
    expect(got.kind === "nothing" && got.why.length).toBeGreaterThan(0);
  });
});

describe("when every goal lands on one configuration", () => {
  it("says so once instead of four times", () => {
    /*
      Measured on gemma4:12b, 2026-09-01: eight candidates spanning 3.3%, seven inside 0.5%, so
      the tie-break took the plainest and every intent resolved to it. AUTO, BALANCED, FAST and
      MAX QUALITY all read `51.7 tok/s · 32,768 · 11.9 GB`, three captioned "same as AUTO".
      Nothing false, and the shape was: a list of four implies four things were found.
    */
    const it = config(32_768, "f16", 51.7);
    const found = optimized([it], { profiles: oneAnswer(it) });
    expect(oneAnswerOnly(found)).toBe(true);
    expect(profilesOf(found).slice(1).every((it) => it.sameAs !== null)).toBe(true);
  });

  it("keeps the list when the goals really differ", () => {
    // FAST took the 16K row and BALANCED held the standard window, so there are two answers to
    // show rather than one described four times.
    const wide = config(32_768, "f16", 48.0);
    const quick = config(16_384, "f16", 53.0);
    const found = optimized([wide, quick], {
      profiles: asProfiles({
        auto: wide,
        balanced: wide,
        fast: quick,
        maxQuality: wide,
      }),
    });
    expect(oneAnswerOnly(found)).toBe(false);
  });

  it("is not true of a model with nothing measured", () => {
    expect(oneAnswerOnly(undefined)).toBe(false);
    expect(oneAnswerOnly(optimized([]))).toBe(false);
  });
});

/*
  **Which workload a profile is about is a rule too**, and it moved with the rest:
  `a_short_prompt_and_a_full_window_are_never_ranked_against_each_other`,
  `a_model_with_no_ladder_still_answers_from_the_short_rows` and
  `a_ladder_is_what_makes_fast_and_long_context_differ`.
*/

describe("what a profile costs", () => {
  const at = (generation: number): Shown =>
    ({ generation }) as unknown as Shown;

  it("says nothing about the fastest row", () => {
    // A row that is the reference has no price against itself, and printing `0% slower` on it
    // would be an instrument that never moves.
    expect(costOf(at(40.7), 40.7)).toBeNull();
  });

  it("says nothing where the difference is inside the noise", () => {
    // 40.7 against 39.7 is 2.5% and worth saying; 40.7 against 40.4 is under one, and a benchmark
    // that reports a 1% gap as a cost is asking somebody to choose on a number it cannot defend.
    expect(costOf(at(40.4), 40.7)).toBeNull();
    expect(costOf(at(39.7), 40.7)).toBe("3% slower");
  });

  it("switches to a multiple where a percentage stops being readable", () => {
    /*
      The whole reason this exists. LONG CONTEXT resolved to 256K at 2.6 tok/s — stable,
      bracketed and honest, and about a minute for a short reply. As a percentage it reads
      `1,465% slower`, which is a number nobody converts. As `15x slower` it is unmissable.
    */
    expect(costOf(at(2.65), 40.69)).toBe("15× slower");
    expect(costOf(at(18.8), 40.69)).toBe("2.2× slower");
  });

  it("answers nothing rather than dividing by a reading it does not have", () => {
    expect(costOf(at(40.0), 0)).toBeNull();
    expect(costOf(at(0), 40.0)).toBeNull();
  });
});

/** What the search bought, and what may be divided by what to say so. */
describe("what the search bought", () => {
  const at = (generation: number, fill: number | null) =>
    config(32_768, "f16", generation, { filled: fill });

  const after = (chosen: Configuration, base: Configuration): Optimized =>
    optimized([chosen], {
      chosen: "balanced",
      baseline: base,
      profiles: oneAnswer(chosen),
    });

  it("compares two readings of the same workload", () => {
    expect(improvement(after(at(48.8, null), at(46.0, null)))).toBe("+6.1%");
  });

  it("says nothing when the two were not measured the same way", () => {
    /*
      Measured 2026-09-02 on `gemma4:12b`. The deck read `-18.5%` over a configuration the search
      had just chosen: a filled 32K row at 39.7 divided by a baseline that answered 21 tokens in
      the same window at 48.8. Both correct, and the quotient is the price of a long conversation
      wearing the costume of a regression.
    */
    expect(improvement(after(at(39.7, 30_835), at(48.8, null)))).toBeNull();
    expect(improvement(after(at(48.8, null), at(39.7, 30_835)))).toBeNull();
  });

  it("compares two filled readings", () => {
    // Both filled is like for like, whatever the windows were.
    expect(improvement(after(at(40.7, 15_075), at(39.7, 30_835)))).toBe("+2.5%");
  });
});

/** What Epoch says about a configuration it measured and will not recommend. */
describe("what is said about a row held back", () => {
  const held = (why: Aside, slowest: number, fastest: number): WasAside => ({
    why,
    configuration: config(32_768, "f16", (slowest + fastest) / 2, {
      verdict: {
        stateOf: "unstable",
        median: (slowest + fastest) / 2,
        fastest,
        slowest,
        spread: fastest - slowest,
        kept: 3,
        discarded: 0,
        collapses: 0,
      },
    } as Partial<Configuration>),
  });

  it("names the range that was actually measured", () => {
    // gemma4:12b's `ngram-mod`, 2026-09-02: it might be half as fast again and it might be
    // slightly slower, and the two numbers are the only way to see that.
    const said = whyAside(held("varied", 48.8, 72.2), 48.96);
    expect(said).toContain("48.8");
    expect(said).toContain("72.2");
    // One decimal, the same as every other rate on the deck: two spellings of one
    // quantity on one screen is how a reader stops trusting either.
    expect(said).toContain("49.0");
  });

  it("says a collapse is not a configuration at all", () => {
    /*
      The one exclusion that is not a judgement about risk. Its surviving runs came from an
      instance that broke, so there is nothing behind the number to put into effect -- and the
      sentence has to say that rather than implying the user is being protected from a gamble.
    */
    const said = whyAside(held("collapsed", 59, 61), 48.0);
    expect(said).toContain("broke partway through");
    expect(mayChoose(held("collapsed", 59, 61))).toBe(false);
  });

  it("lets somebody choose every risk that is theirs to take", () => {
    // Epoch measured and said; the card, the model and the gamble are the owner's.
    for (const why of ["varied", "changedAnswers", "unwitnessed", "sessionDegraded"] as const) {
      expect(mayChoose(held(why, 40, 60))).toBe(true);
    }
  });

  it("does not invent a comparison it was not given", () => {
    // With nothing to compare against, the range is still the honest half and is still said.
    const said = whyAside(held("varied", 48.8, 72.2), null);
    expect(said).toContain("48.8");
    expect(said).not.toContain("NaN");
  });
});

/** What a configuration has switched on, in a few words. */
describe("summarising a configuration", () => {
  it("names only what is on", () => {
    expect(summariseTuning(config(32_768, "f16", 40))).toBe("");
    expect(summariseTuning(config(32_768, "q8_0", 40))).toBe("compressed cache");
  });

  it("names the speculation kind rather than the word speculation", () => {
    // `draft-mtp` is the model's own prediction head and `ngram-mod` is not; calling both of them
    // "speculation" hides the only part somebody is choosing between.
    const it = config(32_768, "f16", 52.4, {
      tuning: { flashAttn: null, speculation: { kind: "draft-mtp", draft: null } },
    } as Partial<Configuration>);
    expect(summariseTuning(it)).toBe("draft-mtp");
  });
});

/**
 * A reading has to say what it is a reading *of*.
 *
 * `32,768` is `STANDARD_CONTEXT` — the window every model is benchmarked at, so two models can
 * be compared. It is not what llama.cpp is started with: an unmeasured model gets 16,384. The
 * owner read the first here and met the second in a server log, with nothing between the two
 * numbers to say they answered different questions.
 */
describe("what a quick reading's window is a window of", () => {
  it("names the bench, so it cannot be read as what the model loads with", () => {
    const one = {
      generation: 45.4,
      vramUsed: null,
      context: 32_768,
      healthy: true,
    } as Parameters<typeof quickLine>[0];
    const said = quickLine(one);
    expect(said).toContain("benched at 32,768");
    // The bare noun is what made it ambiguous.
    expect(said).not.toMatch(/32,768 context/);
  });
});
