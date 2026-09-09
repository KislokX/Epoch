/**
 * Choosing a configuration on how steady it was, not on how fast it was once.
 *
 * **Measured 2026-08-31**: the same configuration answered at 48.3, 44.2 and 25.0 tok/s across
 * three searches of one model on one machine, because a model instance can be evicted from the
 * card partway through and never recover. A peak is the best moment of whichever instance
 * survived. These hold the rule the owner set from that:
 *
 * | | peak | median | range | collapses |
 * |---|---|---|---|---|
 * | A | 48.1 | 44.0 | 25–48 | 2 |
 * | B | 45.9 | 45.5 | 45.1–45.9 | 0 |
 *
 * B, without argument.
 */

import { describe, expect, it } from "vitest";

import { steadinessOf } from "./ModelsHere";
import type { Configuration, Verdict } from "../ipc/launcher";

const verdict = (over: Partial<Verdict>): Verdict => ({
  stateOf: "stable",
  median: null,
  fastest: null,
  slowest: null,
  spread: null,
  kept: 3,
  discarded: 0,
  collapses: 0,
  ...over,
});

const config = (
  generation: number,
  over: Partial<Configuration> = {},
): Configuration =>
  ({
    loadout: { context: 32_768, cache: "f16" },
    tuning: { flashAttn: null, speculation: { kind: "", draft: null } },
    offload: null,
    gpuLayers: null,
    generation,
    prompt: null,
    firstTokenMs: null,
    vramUsed: null,
    ramUsed: null,
    stable: true,
    verdict: verdict({ median: generation, fastest: generation, slowest: generation }),
    around: {
      gpuMean: null,
      sharedBefore: null,
      sharedAfter: null,
      vramBefore: null,
      vramPeak: null,
    },
    sameAnswers: true,
    // Closed on both sides: the ordinary case a fixture should represent.
    bracket: {
      before: "O-001",
      beforeHeld: "reproduced",
      after: "O-002",
      afterHeld: "reproduced",
    },
    at: 1,
    ...over,
  }) as unknown as Configuration;

/*
  **Which number a profile is chosen on moved to the Engine**, along with the copy of `resolve`
  this file was testing:
  `the_number_a_profile_is_chosen_on::the_median_decides_rather_than_the_single_reading` and
  `::a_row_with_no_verdict_is_not_offered_however_fast_it_read`.

  The second of those changed its answer on the way. This file asserted that a row with no verdict
  falls back to its reading and can win — true of the deck’s copy, whose filter asked
  `stateOf !== "unstable"` and let `undefined` through, and never true of the Engine, which calls
  an unrecorded verdict `Invalid` and excludes it. So the panel could mark a configuration
  `use_profile` would refuse to write, and a passing test said it was fine.
*/

describe("saying why", () => {
  it("puts the range and the collapses where somebody can argue with the choice", () => {
    const said = steadinessOf(
      config(48.1, {
        verdict: verdict({ median: 44.0, slowest: 25.0, fastest: 48.1, collapses: 2 }),
      }),
    );
    expect(said).toContain("25.0\u201348.1 over 3 runs");
    expect(said).toContain("2 collapses");
  });

  it("says nothing about a row with nothing to say", () => {
    // `0 collapses` on every row would make the rows that matter invisible.
    const quiet = steadinessOf(config(45.0, { verdict: verdict({ kept: 1, median: 45.0 }) }));
    expect(quiet).toBeNull();
  });

  it("distinguishes a discarded run from a collapse", () => {
    // Both are runs that were thrown away; only one of them is the instance going bad.
    const retried = steadinessOf(
      config(45.0, {
        verdict: verdict({ median: 45.0, slowest: 44.9, fastest: 45.2, discarded: 1 }),
      }),
    );
    expect(retried).toContain("1 discarded");
    expect(retried).not.toContain("collapse");
  });
});
