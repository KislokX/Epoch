/**
 * What Epoch actually does while it is starting.
 *
 * ## Why this exists at all
 *
 * The bridge used to assemble itself in public: it painted immediately with nothing in it, its
 * survey landed a moment later, and its provider and agent probes were deliberately held back
 * 350 ms so they would not compete for the first paint. Every one of those probes starts a local
 * CLI or waits on a local endpoint, so the first seconds on the bridge were spent watching
 * panels fill in.
 *
 * Moving that work in front of the bridge is not a delay added to the application — it is the
 * same seconds, spent somewhere they can be *shown*. A start sequence that reports real work is
 * better than a bridge that arrives half-built.
 *
 * ## Everything here is measured
 *
 * There is no `CORE MEMORY 16384K OK`, no `REACTOR NOMINAL`, no `HULL INTEGRITY SEALED`. Those
 * read well and mean nothing, and a screen that opens the application with four invented
 * readings teaches the user that Epoch's instruments are decoration — which is the exact habit
 * the Launcher's cold-instrument rule exists to prevent.
 *
 * Every line below is the real answer to a real question, including the ones that are bad news.
 * A provider that did not answer says so and says where Epoch looked. An agent that is installed
 * but signed out says *that*, because the fix is different from an install. A step that throws
 * reports a failure rather than a reassuring green.
 *
 * ## Separate from the animation on purpose
 *
 * `Boot.tsx` renders this; it does not know how to do any of it. That is what makes the honest
 * part testable without a DOM, and it is why the sequence can be read end to end in one file.
 */

import { invoke } from "@tauri-apps/api/core";
import { agentLink } from "./agentLink";

import {
  fetchAgents,
  fetchProviders,
  fetchSettings,
  fetchWorlds,
  type AgentStatus,
  type Settings,
} from "../ipc/launcher";
import type { LauncherView, ProviderStatus, ShipsLogView } from "../ipc/contracts";

/**
 * How a reading should be read.
 *
 * `good` is a working thing, `warn` is a real problem the user can act on, `cold` is an honest
 * absence, `head` is the name of a step rather than a reading. Four rather than two because
 * "not signed in" and "failed to ask" are different facts and must not share a colour.
 */
export type Tone = "head" | "good" | "warn" | "cold";

export interface StartupLine {
  /** What was asked. */
  readonly label: string;
  /** What came back. Empty only while the label is still being typed. */
  readonly value: string;
  readonly tone: Tone;
  /** A detail belonging to the step above it. */
  readonly indent?: boolean;
}

export interface StartupResult {
  readonly view: LauncherView;
  readonly settings: Settings;
  readonly log: ShipsLogView | null;
  readonly providers: readonly ProviderStatus[];
  readonly agents: readonly AgentStatus[];
}

/** Everything the sequence needs, injected so the whole of it can be tested without Tauri. */
export interface StartupSources {
  worlds(): Promise<LauncherView>;
  settings(): Promise<Settings>;
  log(): Promise<ShipsLogView>;
  providers(): Promise<readonly ProviderStatus[]>;
  agents(): Promise<readonly AgentStatus[]>;
}

export const LIVE_SOURCES: StartupSources = {
  worlds: fetchWorlds,
  settings: fetchSettings,
  log: () => invoke<ShipsLogView>("ships_log"),
  providers: fetchProviders,
  agents: fetchAgents,
};

/** What the bridge falls back to when a step could not answer at all. */
const NO_WORLDS: LauncherView = {
  orchestrator: { name: "", isUnnamed: true, portrait: null, problems: [] },
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
};

/** How many steps there are, so a percentage is a count rather than a clock. */
export const STARTUP_STEPS = 5;

/** One step's worth of progress: its lines, and how many steps are now finished. */
export type StartupReport = (lines: readonly StartupLine[], done: number) => void;

/**
 * Run the startup, reporting each step as it lands.
 *
 * Sequential rather than `Promise.all`, deliberately. The point is to *show* the work, and four
 * lines arriving at once shows nothing; it also keeps the two probes that spawn processes from
 * competing with the vault read for the same disk.
 */
export async function runStartup(
  sources: StartupSources,
  report: StartupReport,
): Promise<StartupResult> {
  let done = 0;
  const step = (lines: readonly StartupLine[]) => {
    done += 1;
    report(lines, done);
  };

  // --- the vault ------------------------------------------------------------------------
  let view = NO_WORLDS;
  try {
    view = await sources.worlds();
    const lines: StartupLine[] = [
      {
        label: "READING THE VAULT",
        value: `${count(view.worlds.length, "WORLD")} · ${count(view.characters.length, "CREW", "CREW")}`,
        tone: "head",
      },
    ];
    for (const world of view.worlds) {
      lines.push({
        label: `BERTH ${pad(world.berth)} ${world.name.toUpperCase()}`,
        // `coverage` is the fraction of the Engine's concepts this World supplies — a real
        // reading the Launcher already shows, not a loading percentage dressed up as one.
        value: `${String(world.places)} PLACES · ${String(world.characters)} CREW · ${String(Math.round(world.coverage * 100))}% CHARTED`,
        tone: world.problems.length > 0 ? "warn" : "good",
        indent: true,
      });
    }
    if (view.worlds.length === 0) {
      lines.push({
        label: "NO WORLDS YET",
        value: "MAKE ONE FROM THE BRIDGE",
        tone: "cold",
        indent: true,
      });
    }
    step(lines);
  } catch {
    step([{ label: "READING THE VAULT", value: "COULD NOT BE READ", tone: "warn" }]);
  }

  // --- settings -------------------------------------------------------------------------
  let settings: Settings = { concurrentCrew: false, microphone: null, hearingLanguage: null, toolRounds: null };
  try {
    settings = await sources.settings();
    step([
      {
        label: "SETTINGS",
        /*
          **This setting decides memory, and it never decided turns.** It read
          `ONE TURN AT A TIME` when off — a true sentence about the World on the day it was
          written, and one that stopped being true on 2026-08-26, when `running` and `waiting`
          became maps keyed by character. Two crew members work at once whatever this says.

          A real reading of the wrong quantity is the most convincing way an instrument can lie,
          because something genuinely is being measured. It now names the quantity it is actually
          a reading of: how many brains stay on the card.
        */
        value: settings.concurrentCrew
          ? "CREW MODELS STAY WARM"
          : "ONE MODEL RESIDENT",
        tone: "good",
      },
    ]);
  } catch {
    step([{ label: "SETTINGS", value: "DEFAULTS", tone: "cold" }]);
  }

  // --- history --------------------------------------------------------------------------
  let log: ShipsLogView | null = null;
  try {
    log = await sources.log();
    step([
      {
        label: "HISTORY",
        value: `${count(log.quests.length, "QUEST")} · ${count(log.runs.length, "RUN")}`,
        tone: log.quests.length > 0 ? "good" : "cold",
      },
    ]);
  } catch {
    step([{ label: "HISTORY", value: "COULD NOT BE READ", tone: "warn" }]);
  }

  // --- providers ------------------------------------------------------------------------
  let providers: readonly ProviderStatus[] = [];
  try {
    providers = await sources.providers();
    const lines: StartupLine[] = [
      { label: "PROVIDERS", value: count(providers.length, "CONFIGURED", "CONFIGURED"), tone: "head" },
    ];
    for (const provider of providers) {
      lines.push({
        label: provider.name.toUpperCase(),
        value: provider.online
          ? `ONLINE · ${count(provider.models.length, "MODEL")}`
          : `OFFLINE · ${provider.note ?? provider.endpoint}`,
        tone: provider.online ? "good" : "warn",
        indent: true,
      });
    }
    step(lines);
  } catch {
    step([{ label: "PROVIDERS", value: "COULD NOT BE ASKED", tone: "warn" }]);
  }

  // --- agents ---------------------------------------------------------------------------
  let agents: readonly AgentStatus[] = [];
  try {
    agents = await sources.agents();
    const lines: StartupLine[] = [
      { label: "AGENTS", value: count(agents.length, "KNOWN", "KNOWN"), tone: "head" },
    ];
    for (const agent of agents) {
      lines.push({ label: agent.name.toUpperCase(), ...agentReading(agent), indent: true });
    }
    step(lines);
  } catch {
    step([{ label: "AGENTS", value: "COULD NOT BE ASKED", tone: "warn" }]);
  }

  return { view, settings, log, providers, agents };
}

/**
 * What an agent's two separate facts read as.
 *
 * Installed and signed-in have different fixes, so they never collapse into one status — that
 * distinction is already law in `CLAUDE.md`, and it was learned from a machine with the desktop
 * app open and the CLI signed out. An unreadable answer is `unknown`, never "signed out": no
 * reading beats an invented one.
 */
export function agentReading(agent: AgentStatus): { value: string; tone: Tone } {
  // One decision (`agentLink`); this readout only chooses its own vocabulary, which is the
  // boot log's rather than a panel's — "NOT INSTALLED" reads better here than "ABSENT".
  const link = agentLink(agent);
  switch (link.state) {
    case "absent":
      return { value: "NOT INSTALLED", tone: "cold" };
    case "signedOut":
      return { value: "SIGNED OUT", tone: "warn" };
    case "unknown":
      return { value: "COULD NOT ASK", tone: "warn" };
    // Both name what is behind the word: an account when the agent confirmed one, the chosen
    // method when that is all Epoch could read.
    default:
      return { value: `READY · ${link.detail}`, tone: "good" };
  }
}

function count(n: number, one: string, many = `${one}S`): string {
  return `${String(n)} ${n === 1 ? one : many}`;
}

function pad(n: number): string {
  return String(n).padStart(2, "0");
}
