import { describe, expect, it } from "vitest";

import { INITIAL_TURN_STATE, turnReducer } from "./turnState";

describe("the visible state of a turn", () => {
  it("finishes the last matching live capability when two identical tools overlap", () => {
    let state = turnReducer(INITIAL_TURN_STATE, {
      type: "turn/step",
      step: { phase: "using", capability: "read_file", what: "read first.md" },
    });
    state = turnReducer(state, {
      type: "turn/step",
      step: { phase: "using", capability: "read_file", what: "read second.md" },
    });

    state = turnReducer(state, {
      type: "turn/step",
      step: { phase: "used", capability: "read_file", ok: true, detail: "done" },
    });

    expect(state.working).toEqual([
      expect.objectContaining({ what: "read first.md", running: true }),
      expect.objectContaining({ what: "read second.md", running: false, ok: true }),
    ]);
  });

  it("keeps an older context measurement only until this turn receives its own", () => {
    const remembered = { line: "last turn", dropped: 0, used: 800, budget: 4_096, reserved: 0 };
    const measured = { line: "this turn", dropped: 2, used: 1_200, budget: 4_096, reserved: 64 };
    const withRemembered = turnReducer(INITIAL_TURN_STATE, {
      type: "context/remembered",
      knew: remembered,
    });
    const withMeasured = turnReducer(withRemembered, { type: "context/measured", knew: measured });

    expect(withMeasured.knew).toEqual(measured);
    expect(turnReducer(withMeasured, { type: "context/remembered", knew: remembered }).knew).toEqual(
      measured,
    );
  });

  it("ends present-tense work while preserving the Engine's failure as a dismissable notice", () => {
    const working = turnReducer(INITIAL_TURN_STATE, {
      type: "turn/step",
      step: { phase: "using", capability: "shell", what: "run check" },
    });
    const ended = turnReducer(working, {
      type: "turn/ended",
      ended: { error: "tool failed", invited: ["paladin"], sources: [], stop: "stopped" },
    });

    expect(ended).toMatchObject({
      thinking: false,
      writing: "",
      working: [],
      failed: "tool failed",
      halted: true,
      unfinished: false,
      invited: ["paladin"],
    });
  });

  it("shows compaction only while the Engine says that maintenance is running", () => {
    const compacting = turnReducer(INITIAL_TURN_STATE, {
      type: "turn/compacting",
      through: 4,
    });
    expect(compacting.compacting).toBe(4);

    const ended = turnReducer(compacting, {
      type: "turn/ended",
      ended: { error: null, invited: [] },
    });
    expect(ended.compacting).toBeNull();
  });

  it("does not carry citations or a measured context into the next Quest", () => {
    const state = {
      ...INITIAL_TURN_STATE,
      planned: "old plan",
      knew: { line: "old", dropped: 0, used: 50, budget: 100, reserved: 0 },
      sources: [{ title: "old source", url: "https://epoch.example/old" }],
      working: [{ capability: "read_file", what: "old.md", running: true, ok: null, detail: "" }],
      unfinished: true,
      halted: true,
    };

    expect(turnReducer(state, { type: "quest/setAside" })).toMatchObject({
      planned: null,
      knew: null,
      sources: [],
      working: [],
      unfinished: false,
      halted: false,
    });
  });

  it("clears the previous Quest's live state before another Quest is read", () => {
    const live = {
      ...INITIAL_TURN_STATE,
      thinking: true,
      compacting: 12,
      writing: "half an answer",
      working: [{ capability: "shell", what: "write report", running: true, ok: null, detail: "" }],
    };

    expect(turnReducer(live, { type: "turn/reread" })).toMatchObject({
      thinking: false,
      compacting: null,
      writing: "",
      working: [],
    });
  });
});

/**
 * A limit nobody can see the size of is one people assume is arbitrary.
 *
 * The owner watched a character say *"Te lo reproduzco ahora:"* and stop, and read it as the
 * character being limited. It was — eight rounds is the whole budget for one turn, and a
 * conversation with an MCP server attached spends them on searches. The Engine had counted them
 * the whole time and the notice said only *"the round limit"*.
 */
describe("how many rounds a turn took", () => {
  it("carries the count so the notice can name it", () => {
    const after = turnReducer(INITIAL_TURN_STATE, {
      type: "turn/ended",
      ended: { error: null, invited: [], stop: "rounds_exhausted", rounds: 8 },
    });
    expect(after.unfinished).toBe(true);
    expect(after.rounds).toBe(8);
  });

  it("is zero when the Engine did not say, which is not a claim that it took none", () => {
    // An older Engine, or a path that does not count. `0` is *unsaid* here and the notice reads
    // as a limit rather than as a measurement of it.
    const after = turnReducer(INITIAL_TURN_STATE, {
      type: "turn/ended",
      ended: { error: null, invited: [], stop: "rounds_exhausted" },
    });
    expect(after.rounds).toBe(0);
  });
});
