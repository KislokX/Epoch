/**
 * What a cold start may and may not do.
 *
 * The rules, not the pixels: it may not trap anybody, it must hand the bridge what it read, and
 * skipping it early must cost nothing. The typing and the wordmark are presentation and are not
 * asserted — what `startup.ts` measured is, and that has its own tests.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";

import { Boot, dotted } from "./Boot";
import type { StartupSources } from "../experience/startup";
import type { LauncherView, ShipsLogView } from "../ipc/contracts";

const VIEW = {
  orchestrator: { name: "Kislok", isUnnamed: false, portrait: null, problems: [] },
  worlds: [],
  characters: [],
  sessionSeconds: 0,
  problems: [],
  definitionProblems: [],
} as unknown as LauncherView;

/** Sources that answer at once, so a test measures the sequence rather than the network. */
function quick(over: Partial<StartupSources> = {}): StartupSources {
  return {
    worlds: () => Promise.resolve(VIEW),
    settings: () => Promise.resolve({ concurrentCrew: false, microphone: null, hearingLanguage: null, toolRounds: null }),
    log: () => Promise.resolve({ quests: [], runs: [] } as unknown as ShipsLogView),
    providers: () => Promise.resolve([]),
    agents: () => Promise.resolve([]),
    ...over,
  };
}

/** A source that never answers — the case a ceiling exists for. */
function hung(): StartupSources {
  return quick({ providers: () => new Promise(() => undefined) });
}

/**
 * Advance until the sequence hands over, or give up.
 *
 * One small step at a time rather than one jump, because each beat schedules the next from an
 * effect that runs *after* its state commits — and under fake timers a commit lands as `act`
 * exits, so a single long advance only ever fires one timer. On a real clock the next timer is
 * created microseconds after the commit and the chain runs on its own.
 */
async function settle(until: () => boolean, steps = 500) {
  for (let i = 0; i < steps && !until(); i += 1) {
    await act(async () => {
      await vi.advanceTimersByTimeAsync(30);
    });
  }
}

describe("Boot", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => {
    vi.useRealTimers();
  });

  it("hands the bridge everything it read", async () => {
    // The whole reason the sequence is allowed to hold the screen: the work it does is work the
    // Launcher no longer repeats.
    const done = vi.fn();
    render(<Boot onDone={done} sources={quick()} />);

    await settle(() => done.mock.calls.length > 0);

    expect(done).toHaveBeenCalledTimes(1);
    expect(done.mock.calls[0]?.[0]).toMatchObject({ view: VIEW });
  });

  it("hands over nothing when skipped, so the bridge asks for itself", async () => {
    // The fallback is what keeps this presentation rather than a step the application depends
    // on. A skipped start must never leave the Launcher with half a survey.
    const done = vi.fn();
    render(<Boot onDone={done} sources={hung()} />);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(50);
    });
    act(() => void window.dispatchEvent(new KeyboardEvent("keydown", { key: "a" })));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(500);
    });

    expect(done).toHaveBeenCalledWith(null);
  });

  it("gets out of the way even when a step never answers", async () => {
    const done = vi.fn();
    render(<Boot onDone={done} sources={hung()} />);

    await settle(() => done.mock.calls.length > 0);

    expect(done).toHaveBeenCalled();
  });

  it("leaves on a click as well as a key", async () => {
    const done = vi.fn();
    render(<Boot onDone={done} sources={hung()} />);

    act(() => void window.dispatchEvent(new PointerEvent("pointerdown")));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(500);
    });

    expect(done).toHaveBeenCalled();
  });

  it("offers a skip control as a real button", async () => {
    // Reachable without a mouse, like everything else (Phase 4, Life 9).
    render(<Boot onDone={vi.fn()} sources={hung()} />);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(50);
    });

    expect(screen.getByRole("button", { name: "SKIP" })).toBeTruthy();
  });

  it("reports the work as it lands, not before", async () => {
    render(<Boot onDone={vi.fn()} sources={hung()} />);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500);
    });

    // Three steps of five finished before the hung one. A percentage that is a count cannot
    // claim more than has actually happened.
    expect(screen.getByText(/CHECKING SYSTEMS · 60%/)).toBeTruthy();
  });
});

describe("dotted", () => {
  it("pads a label out to the value column, and indents a detail", () => {
    expect(dotted("HISTORY")).toBe(`HISTORY ${".".repeat(33)}`);
    expect(dotted("OLLAMA", true).startsWith("  OLLAMA ")).toBe(true);
  });

  it("still separates a label too long for the column", () => {
    // Otherwise a long World name would run straight into its reading.
    expect(dotted("A".repeat(60))).toContain(" ...");
  });
});
