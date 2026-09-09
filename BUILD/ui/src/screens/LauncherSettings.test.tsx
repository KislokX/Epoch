/**
 * The Launcher's own settings must say what is stored, not what was true at boot.
 *
 * ## The defect this holds shut
 *
 * Measured 2026-08-25 by using it: `Run several crew members at once` was switched **on**,
 * a World was entered, and coming back the Launcher showed it **Off** — while `settings.toml`
 * held `concurrentCrew = true`. The screen seeded its state from `startup.settings`, a snapshot
 * taken once when Epoch opened, and the line that re-read the file sat *below* an early return
 * meant for the expensive provider probes. A small TOML is not a probe; it was on the wrong side
 * of a line the file itself draws.
 *
 * That is the worst shape a wrong instrument can have. A blank gauge tells you nothing; this one
 * showed Off, and the user clicking to switch it on switched it **off** believing the opposite —
 * the correction did the damage.
 *
 * So the test asserts the disagreement is resolved the right way round: given a stale snapshot
 * *and* a stored value, the stored value wins.
 */

import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import { LauncherScreen } from "./LauncherScreen";
import type { LauncherView, ShipsLogView } from "../ipc/contracts";
import type { StartupResult } from "../experience/startup";

/*
  The Launcher mounts several decks' worth of panels, and each asks the Engine for its own
  shape. One blanket answer is not enough: a panel that maps over a list and is handed an object
  throws during render, which Vitest reports as an unhandled error beside a passing test — a
  green run with a red line in it, which is worse than either.

  So: the log gets the log's shape and everything else gets an empty list, which is what every
  other panel here is willing to render.
*/
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string) =>
    command === "ships_log" ? { quests: [], runs: [] } : [],
  ),
}));

const fetchSettings = vi.fn(async () => ({ concurrentCrew: true }));

vi.mock("../ipc/launcher", async () => {
  const real = await vi.importActual<Record<string, unknown>>("../ipc/launcher");
  return {
    ...real,
    fetchSettings: () => fetchSettings(),
    saveSettings: vi.fn(async () => null),
    fetchWorlds: vi.fn(async () => VIEW),
    fetchProviders: vi.fn(async () => []),
    fetchAgents: vi.fn(async () => []),
  };
});

const VIEW = {
  orchestrator: { name: "Kislok", isUnnamed: false, portrait: null, problems: [] },
  worlds: [],
  characters: [],
  vocabulary: {
    archetypes: [],
    places: [],
    reasoning: [],
    capabilities: [],
    built: [],
    groups: [],
  },
  skills: [],
  sessionSeconds: 0,
  problems: [],
  definitionProblems: [],
} as unknown as LauncherView;

/** A cold start that finished, carrying the value the file has since disagreed with. */
const STARTUP: StartupResult = {
  view: VIEW,
  settings: { concurrentCrew: false },
  log: { quests: [], runs: [] } as unknown as ShipsLogView,
  providers: [],
  agents: [],
} as unknown as StartupResult;

describe("the Launcher's settings", () => {
  beforeEach(() => {
    fetchSettings.mockClear();
  });

  it("reads the file even when a boot snapshot already answered", async () => {
    render(<LauncherScreen onEntered={() => undefined} startup={STARTUP} />);
    // The whole defect in one assertion: with `startup` present the fetch used to be skipped.
    await waitFor(() => expect(fetchSettings).toHaveBeenCalled());
  });

  it("shows what is stored, not what was true when Epoch opened", async () => {
    render(<LauncherScreen onEntered={() => undefined} startup={STARTUP} />);

    const settings = await screen.findByText(/Preferences & system/);
    settings.click();

    const toggle = await screen.findByRole("checkbox", {
      name: /Run several crew members at once/,
    });
    await waitFor(() => expect((toggle as HTMLInputElement).checked).toBe(true));
  });
});
