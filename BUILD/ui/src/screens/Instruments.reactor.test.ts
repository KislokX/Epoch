import { describe, expect, it } from "vitest";

import { reactorLoad } from "./Instruments";

/**
 * The gauge that was dark beside a live reading of the same fact.
 *
 * It read `REACTOR TEMP —` with a note promising GPU telemetry "once local models run on this
 * machine". They had been running since Phase 1, and `Machine::measure` had been reading VRAM
 * through `nvidia-smi` the whole time — the Workshop weighs models against it. The note was not
 * describing a missing subsystem; it was describing a wire nobody had run.
 *
 * A dark gauge beside a live reading of the same number is the cold-instrument rule failing in
 * the other direction: silence about something Epoch knows.
 */
describe("what the card is holding", () => {
  it("reads what is used, not what is free", () => {
    // The question somebody has is *how much of my card is gone*, and every other gauge on the
    // panel fills as its subject fills.
    const said = reactorLoad({
      gpu: "NVIDIA GeForce RTX 4070 SUPER",
      vramTotal: 12_900_000_000,
      vramFree: 9_800_000_000,
      ramTotal: 33_900_000_000, unified: false,
    });
    expect(said.reading).toBe("3.1 GB / 12.9 GB");
    expect(said.hot).toBe(false);
  });

  it("warns at four fifths, which is where the next model stops fitting", () => {
    // Not where anything is wrong — a full card is a card doing its job. It is the moment
    // loading another model becomes a decision.
    const tight = reactorLoad({
      gpu: "NVIDIA GeForce RTX 4070 SUPER",
      vramTotal: 12_900_000_000,
      vramFree: 1_000_000_000,
      ramTotal: 33_900_000_000, unified: false,
    });
    expect(tight.hot).toBe(true);
    expect(tight.reading).toBe("11.9 GB / 12.9 GB");
  });

  it("says nothing rather than zero when nothing measured it", () => {
    // `nvidia-smi` is NVIDIA's. An AMD or Intel card leaves this unknown, and an Apple machine
    // has no VRAM to have — its memory is unified, and splitting it into an invented "video"
    // share would describe a division the hardware does not have.
    const apple = reactorLoad({
      gpu: null,
      vramTotal: null,
      vramFree: null,
      ramTotal: 17_179_869_184, unified: false,
    });
    expect(apple.reading).toBe("—");
    expect(apple.hot).toBe(false);
    expect(apple.why).toMatch(/Unknown is a reading/);
  });

  it("names the card when the card is known but silent", () => {
    // Two different absences with two different sentences: nothing to ask, and asked and no
    // answer. The second one is something a person can go and check.
    const quiet = reactorLoad({
      gpu: "AMD Radeon RX 7900 XTX",
      vramTotal: null,
      vramFree: null,
      ramTotal: 33_900_000_000, unified: false,
    });
    expect(quiet.why).toContain("AMD Radeon RX 7900 XTX");
  });

  it("reports nothing at all before anything has been asked", () => {
    // A third state, and not the same as *no card*: a gauge reading `—` before anything looked
    // would be reporting an absence nobody measured.
    expect(reactorLoad(null).reading).toBe("—");
  });

  it("says the reading was measured rather than estimated", () => {
    // The whole panel's rule, in the tooltip of the one row that used to be dark.
    const said = reactorLoad({
      gpu: "NVIDIA GeForce RTX 4070 SUPER",
      vramTotal: 12_900_000_000,
      vramFree: 9_800_000_000,
      ramTotal: 33_900_000_000, unified: false,
    });
    expect(said.why).toMatch(/nvidia-smi/);
    expect(said.why).toMatch(/not estimated/);
  });
});
