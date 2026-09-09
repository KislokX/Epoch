/**
 * What a speculation sweep says on the screen.
 *
 * **Two speed-up columns, and the test exists because collapsing them was tempting.** Measured on
 * this machine: `ngram-mod` on `gemma4:12b` ran at 41.3 t/s on a question it had not seen and
 * 182.5 t/s repeating a paragraph it had just written, against a 47.0 baseline. One number would
 * have to be an average of those, and an average of those describes neither workload.
 */

import { describe, expect, it } from "vitest";

import { sweepLines } from "./ModelsHere";
import type { BenchRun, Sweep, SpecTried } from "../ipc/launcher";

const runs = (rate: number, drafted?: number, accepted?: number): { runs: BenchRun[] } => ({
  runs: [1, 2, 3].map(
    () =>
      ({
        generation: rate,
        drafted: drafted ?? null,
        accepted: accepted ?? null,
        failed: null,
      }) as unknown as BenchRun,
  ),
});

const tried = (over: Partial<SpecTried> & { label: string }): SpecTried =>
  ({
    measured: runs(47),
    repeated: runs(47),
    load: {
      gpuPercent: null,
      vramUsed: null,
      ramUsed: null,
      cpuPercent: null,
      vramBefore: null,
      ramBefore: null,
      seconds: 1,
    },
    speedup: null,
    speedupRepeating: null,
    sameAnswers: null,
    stable: true,
    failures: [],
    ...over,
  }) as SpecTried;

describe("a speculation sweep on the screen", () => {
  it("keeps the two workloads apart, because they disagree", () => {
    const sweep: Sweep = {
      model: "gemma4:12b",
      best: null,
      tried: [
        tried({ label: "off" }),
        tried({
          label: "ngram-mod n=3",
          measured: runs(41.3, 0, 0),
          repeated: runs(182.5, 384, 384),
          speedup: 41.3 / 47,
          speedupRepeating: 182.5 / 52,
        }),
      ],
    };
    const said = sweepLines(sweep).join("\n");
    expect(said).toContain("fresh 0.88x");
    expect(said).toContain("repeat 3.51x");
  });

  it("says a speed-up with nothing drafted is not a speed-up", () => {
    // Drift looks exactly like a small win. The counts are what tell them apart.
    const sweep: Sweep = {
      model: "m",
      best: null,
      tried: [tried({ label: "off" }), tried({ label: "ngram-simple n=5", speedup: 1.01 })],
    };
    expect(sweepLines(sweep).join("\n")).toContain("nothing drafted");
  });

  it("reports nothing winning as an answer rather than as a missing winner", () => {
    // Off costs no memory and has nothing to go wrong. A blank where the winner goes would read
    // as a failure of the sweep.
    const said = sweepLines({ model: "m", best: null, tried: [tried({ label: "off" })] });
    expect(said[1]).toContain("nothing beat the baseline");
  });

  it("names the winner when there is one", () => {
    const said = sweepLines({
      model: "m",
      best: "ngram-mod n=3",
      tried: [tried({ label: "off" })],
    });
    expect(said[1]).toContain("fastest stable: ngram-mod n=3");
  });

  it("marks a configuration that wrote different text, however fast it was", () => {
    // Speculative decoding is supposed to change the speed and nothing else. A `false` here means
    // this configuration is a different model, which is worse than being slow.
    const said = sweepLines({
      model: "m",
      best: null,
      tried: [
        tried({ label: "off" }),
        tried({ label: "ngram-cache n=8", measured: runs(80, 100, 90), sameAnswers: false }),
      ],
    }).join("\n");
    expect(said).toContain("DIFFERENT TEXT");
  });

  it("shows a dash rather than a number where nothing was measured", () => {
    const said = sweepLines({
      model: "m",
      best: null,
      tried: [tried({ label: "off", measured: { runs: [] } })],
    }).join("\n");
    expect(said).toContain("\u2014");
    expect(said).not.toContain("0.0 t/s");
  });
});
