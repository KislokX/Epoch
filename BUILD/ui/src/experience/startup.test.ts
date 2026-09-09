/**
 * The cold start reports what it measured, including the parts that are bad news.
 *
 * These are the tests that make the sequence allowed to exist. A start screen that reads like a
 * computer starting and reports nothing is the reference design's `CORE MEMORY 16384K OK`; the
 * only thing separating ours from that is that every line here comes back from a real call, and
 * these hold that.
 */

import { describe, expect, it, vi } from "vitest";

import { agentReading, runStartup, STARTUP_STEPS, type StartupSources } from "./startup";
import type { AgentStatus } from "../ipc/launcher";
import type { LauncherView, ProviderStatus, ShipsLogView } from "../ipc/contracts";

const VIEW = {
  orchestrator: { name: "Kislok", isUnnamed: false, portrait: null, problems: [] },
  worlds: [
    {
      id: "default",
      name: "Default World",
      berth: 1,
      places: 5,
      characters: 2,
      coverage: 0.6,
      problems: [],
    },
  ],
  characters: [{ id: "mage" }, { id: "paladin" }],
  sessionSeconds: 0,
  problems: [],
  definitionProblems: [],
} as unknown as LauncherView;

function sources(over: Partial<StartupSources> = {}): StartupSources {
  return {
    worlds: () => Promise.resolve(VIEW),
    settings: () => Promise.resolve({ concurrentCrew: false, microphone: null, hearingLanguage: null, toolRounds: null }),
    log: () => Promise.resolve({ quests: [], runs: [] } as unknown as ShipsLogView),
    providers: () => Promise.resolve([]),
    agents: () => Promise.resolve([]),
    ...over,
  };
}

async function linesFrom(over: Partial<StartupSources> = {}) {
  const seen: { label: string; value: string; tone: string }[] = [];
  const result = await runStartup(sources(over), (batch) => seen.push(...batch));
  return { seen, result };
}

describe("runStartup", () => {
  it("reports what the vault actually holds", async () => {
    const { seen } = await linesFrom();

    expect(seen[0]).toMatchObject({ label: "READING THE VAULT", value: "1 WORLD · 2 CREW" });
    expect(seen[1]).toMatchObject({
      label: "BERTH 01 DEFAULT WORLD",
      // Every one of these is a reading the Launcher already shows. `coverage` is the fraction
      // of the Engine's concepts this World supplies — not a loading percentage in disguise.
      value: "5 PLACES · 2 CREW · 60% CHARTED",
    });
  });

  it("says where Epoch looked when a provider did not answer", async () => {
    // The reference design marked every link ONLINE. An instrument that is green whatever
    // happened is one the user learns to stop reading.
    const { seen } = await linesFrom({
      providers: () =>
        Promise.resolve([
          {
            id: "ollama",
            name: "Ollama",
            machine: null,
            endpoint: "http://localhost:11434",
            online: false,
            local: true,
            models: [],
            note: "nothing answered",
          },
        ] as readonly ProviderStatus[]),
    });

    const line = seen.find((l) => l.label === "OLLAMA");
    expect(line).toMatchObject({ value: "OFFLINE · nothing answered", tone: "warn" });
  });

  it("keeps a failing step visibly failed rather than green", async () => {
    const { seen } = await linesFrom({ log: () => Promise.reject(new Error("no")) });

    expect(seen.find((l) => l.label === "HISTORY")).toMatchObject({
      value: "COULD NOT BE READ",
      tone: "warn",
    });
  });

  it("still finishes, and still hands the bridge what it did read", async () => {
    // One broken step may not cost the user the other four. The Launcher renders honestly with
    // whatever arrived, so the sequence hands over rather than refusing.
    const { result } = await linesFrom({ providers: () => Promise.reject(new Error("no")) });

    expect(result.view.worlds).toHaveLength(1);
    expect(result.providers).toEqual([]);
  });

  it("counts every step, so a percentage can never overrun or stall", async () => {
    const report = vi.fn();
    await runStartup(sources(), report);

    const finals = report.mock.calls.map((call) => call[1] as number);
    expect(finals).toEqual([1, 2, 3, 4, 5]);
    expect(STARTUP_STEPS).toBe(5);
  });
});

describe("agentReading", () => {
  const base: AgentStatus = {
    id: "claude-code",
    kind: "claude-code",
    name: "Claude Code",
    installed: true,
    version: "2.1",
    lookedIn: null,
    note: null,
    signedIn: true,
    account: "someonex@gmail.com",
  };

  it("names a sign-in it could read but not verify, rather than shrugging", () => {
    // Gemini CLI has no command that reports this, and asking anyway is a billed prompt — but
    // the method the user chose is readable. Configured is more than nothing, and less than
    // the confirmed case below.
    expect(
      agentReading({ ...base, signedIn: null, account: null, method: "Gemini API Key" }),
    ).toMatchObject({ value: "READY · Gemini API Key", tone: "good" });
  });

  it("still shrugs when there is nothing at all to read", () => {
    // No reading beats an invented one. An agent that reports neither must not borrow the
    // wording of one that reports something.
    expect(
      agentReading({ ...base, signedIn: null, account: null, method: null }),
    ).toMatchObject({ value: "COULD NOT ASK", tone: "warn" });
  });

  it("keeps installed and signed-in apart, because the fixes are different", () => {
    expect(agentReading({ ...base, installed: false })).toMatchObject({ value: "NOT INSTALLED" });
    expect(agentReading({ ...base, signedIn: false })).toMatchObject({ value: "SIGNED OUT" });
  });

  it("treats an unreadable answer as unasked, never as signed out", () => {
    // A machine with the desktop app open and the CLI signed out taught this once already: no
    // reading beats an invented one.
    expect(agentReading({ ...base, signedIn: null })).toMatchObject({
      value: "COULD NOT ASK",
      tone: "warn",
    });
  });

  it("names the account the agent reported, never a credential", () => {
    expect(agentReading(base)).toMatchObject({ value: "READY · someonex@gmail.com" });
  });
});
