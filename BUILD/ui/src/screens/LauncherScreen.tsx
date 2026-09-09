/**
 * The Launcher — the bridge of a docked ship, and what you see before entering a World.
 *
 * It prepares; the World immerses (`PRODUCT_ARCHITECTURE.md`). That separation is unchanged.
 * What changed is the *answer* to what preparing should feel like: not a settings screen with
 * a map on it, but the airlock of a ship you are about to take somewhere.
 *
 * ## Every number here is measured
 *
 * The reference design is full of things Epoch does not have: XP levels, reactor temperature,
 * providers marked ONLINE, a log of everything the crew did today. The composition is worth
 * keeping and the data is not — an earlier Launcher invented a crew count and it was spotted
 * in seconds, because a number nobody can explain is worse than no number.
 *
 * So the panels all stay, at their true readings. Placeholders read zero, OFFLINE or `—`, and
 * each carries a note naming the subsystem that will light it up (see `Instruments.tsx`). A
 * cold instrument still tells you the ship is real.
 *
 * ## Selection is user intent; everything else is derived
 *
 * The only state here is what the user has done: which berth is selected, which deck is open,
 * whether a departure is under way. The rest is computed on read from the Engine's survey —
 * the same discipline the World's camera follows (ADR-0022).
 */

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { CharacterPanel } from "./CharacterPanel";
import { ConnectionsPanel } from "./ConnectionsPanel";
import { CreationsPanel } from "./CreationsPanel";
import { AudioSettings, SpokenLanguage } from "../components/AudioSettings";
import { CatalogueKeys } from "../components/CatalogueKeys";
import { HuggingFaceDeck } from "../components/HuggingFaceDeck";
import { useThisMachine } from "../components/useThisMachine";
import { ExportConfirm } from "../components/ExportConfirm";
import { PairedMachines } from "../components/PairedMachines";
import { McpPanel } from "./McpPanel";
import { ModelsHere } from "../components/ModelsHere";
import { WorkshopPanel } from "./WorkshopPanel";
import { Departure } from "./Departure";
import { NewWorld } from "./NewWorld";
import { RemoveConfirm } from "../components/RemoveConfirm";
import { EraseData } from "../components/EraseData";
import { AgentDoorPanel } from "./AgentDoorPanel";
import { useAgentDoor } from "../experience/useAgentDoor";
import type { StartupResult } from "../experience/startup";
import { useSfx } from "../experience/sfx";
import {
  BridgeConsole,
  CrewChannel,
  CrewLinks,
  Diagnostics,
  ShipsLog,
  TipOfTheDay,
} from "./Instruments";
import { ImageDrop } from "../components/ImageDrop";
import { WorldPreview } from "../components/WorldPreview";
import {
  enterWorld,
  fetchProviders,
  fetchSettings,
  fetchWorlds,
  renameOrchestrator,
  removeWorld,
  renameWorld,
  worldExport,
  exportWorld,
  worldRemoval,
  regenerateDoor,
  setConcurrentCrew,
  setToolRounds,
  setOrchestratorPortrait,
  setWorldArt,
  chooseFolder,
  setProjectRoot,
  chooseLibrary,
  setLibrary,
  openLibrary,
  scanLibrary,
  fetchAgents,
} from "../ipc/launcher";
import type { AgentStatus, Settings } from "../ipc/launcher";
import type {
  LauncherView,
  ProviderStatus,
  ExportView,
  RemovalView,
  ShipsLogView,
  WorldSummary,
} from "../ipc/contracts";

type Deck =
  | "worlds"
  | "creations"
  | "connections"
  | "machines"
  | "characters"
  | "mcp"
  | "workshop"
  | "models"
  | "settings";

const EMPTY: LauncherView = {
  orchestrator: {
    name: "ORCHESTRATOR",
    isUnnamed: true,
    portrait: null,
    problems: [],
  },
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

/**
 * A deck that exists on the ship but is not built yet.
 *
 * Deliberately not a "coming soon" toast. The copy says *why* it is shut in the world's own
 * terms, which is both honest and more interesting than an apology.
 */
const SHUT: Partial<Record<Deck, { name: string; copy: string }>> = {};

/**
 * The last survey that actually came back, kept across a mount.
 *
 * ## Why it is not React state
 *
 * The Launcher unmounts every time somebody enters a World and mounts again on the way back,
 * and its backend survey does not come back instantly — it reaches every paired machine over
 * the network. So for the first seconds after each return, `providers` was empty.
 *
 * An empty list is not merely an ugly gauge here. **The machine dropdown in the crew editor is
 * derived from it**, so a paired MacBook was not shown as offline — it was *absent*, with no
 * way to select it. Seen while driving the editor: `["", "This PC"]`, on a machine that was
 * paired, granted Compute, and answering.
 *
 * That is the same shape as the World's allowances and a worse consequence: a reading nobody
 * can act on yet, against an option nobody can choose at all. The rule is the one the Launcher
 * already produced once — a snapshot is a fine **first paint** and never a source of truth. The
 * probe below still runs and whatever it finds replaces this.
 *
 * Not persisted: a survey from an hour ago would offer machines that have since been unpaired.
 * It lives exactly as long as the process.
 */
const lastMeasured: {
  providers: readonly ProviderStatus[];
  agents: readonly AgentStatus[];
} = { providers: [], agents: [] };

/** One writer, so the kept copy cannot drift from what was rendered. */
function remember(what: {
  providers?: readonly ProviderStatus[];
  agents?: readonly AgentStatus[];
}) {
  if (what.providers) lastMeasured.providers = what.providers;
  if (what.agents) lastMeasured.agents = what.agents;
}

function berthLabel(n: number): string {
  return `BERTH ${String(n).padStart(2, "0")}`;
}

interface LauncherScreenProps {
  /** Called once a World has actually been entered. Identity, plus its name for the title. */
  readonly onEntered: (world: {
    id: string;
    name: string;
    /** Arrive in the World Editor. Set for a World that was just made — it is empty. */
    authoring?: boolean;
  }) => void;
  /**
   * What the cold start already read, or `null` if it was skipped before it finished.
   *
   * The bridge used to assemble itself in public: it painted empty, its survey landed a moment
   * later, and its provider and agent probes were held back 350 ms so they would not fight the
   * first paint. That work now happens in front of the bridge, where it is shown (`Boot`), and
   * arrives here already done.
   *
   * **It is a seed, not a source.** Every one of these is ordinary state with its existing
   * refresh path, so nothing downstream knows where the first value came from; and `null` puts
   * the screen back to asking for itself, exactly as it did before any of this existed.
   */
  readonly startup?: StartupResult | null;
}

/**
 * A project root, shortened to the last two folders.
 *
 * `C:\Users\someone\Downloads\PROYECTOTEST` becomes `Downloads\PROYECTOTEST`.
 *
 * The tail is the part that identifies the folder; the head is the part the user already
 * knows. Truncating in the middle of a deep path would eventually hide the folder's own name,
 * which is the one word that answers "is this the right one?". The whole path stays on hover.
 *
 * Presentation only — the Engine keeps and validates the path exactly as it was chosen.
 */
function shortRoot(path: string): string {
  const parts = path.split(/[\\/]+/).filter((p) => p.length > 0);
  // A drive root has one part and no folder to name. Show it as it is rather than as nothing.
  if (parts.length <= 2) return path;
  return parts.slice(-2).join("\\");
}

export function LauncherScreen({
  onEntered,
  startup = null,
}: LauncherScreenProps) {
  const { muted, volume, setMuted, setVolume } = useSfx();
  /** Whether the make-a-World dialog is open. User intent, like everything else held here. */
  const [makingWorld, setMakingWorld] = useState(false);
  /**
   * What has actually happened, across every World.
   *
   * The vault's history rather than this session's, so it is read once: nothing on this screen
   * can change it, and a World is only ever added to it from inside one.
   */
  const [log, setLog] = useState<ShipsLogView | null>(startup?.log ?? null);
  /** The door an agent knocks on. Configured here; the question it raises appears in the World. */
  const agentDoor = useAgentDoor();
  const [view, setView] = useState<LauncherView>(startup?.view ?? EMPTY);
  const [loaded, setLoaded] = useState(startup !== null);
  /**
   * Who can think, and whether we are currently asking.
   *
   * Held separately from the survey because probing reaches the network: the docking bay must
   * appear immediately, and the conduits fill in a moment later.
   */
  /**
   * What this computer is, for the one gauge that reads it.
   *
   * The same measurement `ThisMachine` shows in Connections — asked through the shared hook, so
   * two surfaces cannot come to disagree about how much of the card is gone.
   */
  const { machine } = useThisMachine();
  const [providers, setProviders] = useState<readonly ProviderStatus[]>(
    startup?.providers ?? lastMeasured.providers,
  );
  /**
   * Agents this machine has, measured beside the backends.
   *
   * Probed on the same button, because the panel answers one question — *who can the crew think
   * with* — and an agent is one of the answers.
   */
  const [agents, setAgents] = useState<readonly AgentStatus[]>(
    startup?.agents ?? lastMeasured.agents,
  );
  const [probing, setProbing] = useState(startup === null);
  /** The user's choices about their own machine. */
  const [settings, setSettings] = useState<Settings>(
    startup?.settings ?? { concurrentCrew: false, microphone: null, hearingLanguage: null, toolRounds: null },
  );

  /* ----------------------------------------------------------- user intent */
  const [deck, setDeck] = useState<Deck>("worlds");
  /**
   * A model another deck asked to open, without measuring it.
   *
   * Beside `toMeasure` and deliberately not folded into it: one of the two spends twenty minutes
   * of graphics card and the other opens a panel.
   */
  const [toReveal, setToReveal] = useState<string | null>(null);
  /**
   * A model the Workshop asked MODELS to measure, by name.
   *
   * Held here because it crosses two decks: the offer appears where a download lands, and the
   * measuring belongs where the row, the curve and the progress bar already are. Cleared by
   * MODELS the moment it takes it, so switching decks later does not start it again.
   */
  const [toMeasure, setToMeasure] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  /** The World being boarded, and whether the sequence was skipped. */
  const [departing, setDeparting] = useState<{
    id: string;
    instant: boolean;
  } | null>(null);
  const [renaming, setRenaming] = useState<{ id: string; name: string } | null>(
    null,
  );
  const [notice, setNotice] = useState<string | null>(null);
  /**
   * What the Settings deck has to say, said **inside the Settings deck**.
   *
   * `notice` is painted next to the Worlds — it was written for the bay and it belongs there.
   * Settings' two actions were reporting through it, four screens away, so REGENERATE looked
   * like a button that did nothing: it worked, said so, and said it somewhere nobody was
   * looking. A confirmation the user cannot see is the same as no confirmation.
   */
  const [settingsSaid, setSettingsSaid] = useState<string | null>(null);
  /** Editing who you are. Null while not editing — user intent only, like everything here. */
  const [me, setMe] = useState<string | null>(null);
  /**
   * Whether the selected World's chart is being shown in place of its key art.
   *
   * A momentary preference, not a setting: artwork is what the author wants you to see, and
   * the chart is what is actually there. Being able to check one against the other is the
   * point of keeping both — but it resets, because the author's framing is the default.
   */
  const [chart, setChart] = useState(false);
  /** True while a write is in flight, so a folder cannot be chosen twice. */
  const [busy, setBusy] = useState(false);
  /** Rotates the tip and the crew channel, so the bridge is never completely still. */
  const [beat, setBeat] = useState(0);

  const refresh = async () => setView(await fetchWorlds());

  /** Point a World at a folder, or clear it. The Engine validates; this only reports. */
  const setRoot = async (worldId: string, path: string | null) => {
    setBusy(true);
    const failure = await setProjectRoot(worldId, path);
    setBusy(false);
    setNotice(failure);
    if (!failure) await refresh();
  };

  /**
   * Ask the shell to open a native folder picker.
   *
   * Dismissing it is an answer, not an error — nothing happens and nothing is said.
   */
  const chooseRoot = async (worldId: string, start: string | null) => {
    setBusy(true);
    const chosen = await chooseFolder(start);
    setBusy(false);
    if (chosen) await setRoot(worldId, chosen);
  };

  const setNotes = async (worldId: string, path: string | null) => {
    setBusy(true);
    const failure = await setLibrary(worldId, path);
    setBusy(false);
    setNotice(failure);
    if (!failure) await refresh();
  };

  const chooseNotes = async (worldId: string, start: string | null) => {
    setBusy(true);
    const chosen = await chooseLibrary(start);
    if (!chosen) {
      setBusy(false);
      return;
    }
    // **Look before accepting it.** A folder picker opened inside a vault makes `.obsidian`
    // one double-click away, and choosing it produces a library with no notes in it that
    // *looks* configured — the crew then reports finding nothing, which reads as the feature
    // being broken rather than as the wrong folder.
    //
    // Still set, either way. Epoch says what is there; it does not decide whether a folder is
    // a good vault (the same rule the Project Root's scan follows).
    const looked = await scanLibrary(chosen);
    setBusy(false);
    await setNotes(worldId, chosen);
    if (!looked) return;
    if (/[\/]\.obsidian\/?$/.test(chosen)) {
      setNotice(
        "That is a vault's settings folder, not the vault. Choose the folder that contains .obsidian instead.",
      );
    } else if (looked.notes === 0) {
      setNotice(
        "No markdown notes in that folder. It is set — but there is nothing in it for the crew to read.",
      );
    }
  };

  /**
   * Open the library where the user reads it.
   *
   * The path itself is the control. It was inert text beside two buttons that both *change*
   * the choice — so the one thing somebody looking at their vault's name most wants to do had
   * nowhere to be clicked.
   */
  const openNotes = async (worldId: string) => {
    setBusy(true);
    const opened = await openLibrary(worldId);
    setBusy(false);
    if (typeof opened === "string") {
      setNotice(opened);
      return;
    }
    // Said, not silent. Getting a file manager when you expected Obsidian is confusing exactly
    // once, and the reason is something the user can act on.
    // Two reasons now, and neither is knowable from here: Obsidian is not installed, or this
    // folder has no `.obsidian/` and so is not a vault Obsidian knows. The sentence names both
    // rather than picking one, because a wrong reason sends somebody to fix the wrong thing.
    setNotice(
      opened
        ? null
        : "The folder was opened instead of Obsidian — either it is not installed, or this folder is not a vault it knows.",
    );
  };

  /**
   * Re-read what answers, and nothing else.
   *
   * Separate from {@link probe} because a runtime coming up cannot change whether Claude Code
   * is signed in, and `probe` runs every agent's own program to find that out. Asking the
   * expensive question because a cheap one changed is how an automatic refresh becomes a cost
   * nobody can explain.
   */
  const readBackends = async () => {
    const found = await fetchProviders();
    remember({ providers: found });
    setProviders(found);
  };

  const probe = async () => {
    setProbing(true);
    // CHECK AGAIN means *again*: the Engine keeps what it measured about the agents, and this
    // is the press that runs their programs a second time.
    const [found, hosted] = await Promise.all([
      fetchProviders(),
      fetchAgents(true),
    ]);
    remember({ providers: found, agents: hosted });
    setProviders(found);
    setAgents(hosted);
    setProbing(false);
  };

  useEffect(() => {
    let active = true;

    /*
      **The Worlds are re-read every time this mounts, including on the way back from a World.**

      They used to be skipped whenever the cold start had already read them — and that was
      right exactly once. `App` keeps its `startup` for the life of the process, so every
      return from a World remounted this bridge with the photograph taken at boot: a World
      created minutes ago was not on it, and neither was one just removed. Restarting Epoch
      was the only way to see your own fleet.

      A local read is cheap and it is the *whole* point of the screen. What the start sequence
      exists to move is the cost below — the probes — and that distinction is kept rather than
      the guard that flattened it.
    */
    void fetchWorlds().then((next) => {
      if (!active) return;
      setView(next);
      setLoaded(true);
    });
    // The log is the same kind of reading: local, cheap, and stale the moment anything happened
    // inside a World.
    void invoke<ShipsLogView>("ships_log").then(
      (next) => active && setLog(next),
    );
    /*
      **And so are the settings — which is why this line moved above the guard.**

      It used to sit below, among the probes, so a mount that already had a `startup` snapshot
      returned before ever reading the file. `startup.settings` is a snapshot taken once, when
      Epoch opened; it is a fine *first paint* and it is not a source of truth.

      Measured 2026-08-25: turning `Run several crew members at once` on, entering a World and
      coming back showed it **Off** while `settings.toml` held `concurrentCrew = true`. That is
      the worst shape a wrong instrument can have — the user sees Off, clicks to switch it on,
      and switches it off believing the opposite. A blank gauge tells you nothing; this one made
      the correction do the damage.

      Reading a small TOML is not a probe. It is exactly the cheap local read the comment above
      argues for, and it belongs on this side of the line.
    */
    void fetchSettings().then((next) => active && setSettings(next));
    /*
      **And the agents, for the same reason and with the same evidence.**

      `fetchAgents()` without `fresh` reads what the Engine already measured — the cheap half of
      the pair, exactly like the settings above. It sat below the guard, so a mount holding a
      `startup` snapshot never asked, and the boot photograph was the whole answer forever.

      Measured 2026-08-25: adding a second Claude Code account and returning to the Bridge showed
      CREW LINKS **without it**, and pressing CHECK AGAIN — a full re-probe of every agent — was
      the only way to see an account that was already in `settings.toml`. Worse the second time:
      entering a World and coming back made an account that *had* appeared **disappear again**.

      A vanishing row is not a cold instrument, it is a wrong one. And the reading it was hiding
      had already been taken: the Engine drops its kept survey the moment an account is added, so
      the first ask after that measures and every ask afterwards is free.
    */
    void fetchAgents().then((next) => active && setAgents(next));
    /*
      **And again whenever the user comes back from somewhere else.**

      A sign-in runs in the agent's own window and its own browser tab and finishes without
      telling anybody, so CREW LINKS went on reporting the state from before it. Focus is a real
      cause rather than a poll: they were elsewhere, and now they are not. `fresh` is not passed —
      the Engine dropped its kept survey when the sign-in started.
    */
    const cameBack = () => {
      void fetchAgents().then((next) => active && setAgents(next));
      /*
        **And the backends, for a cause the agents' own comment already argued.**

        A runtime is started in its own console window — `llama-server`, `ollama serve` — and
        finishes without telling anybody, exactly like a sign-in. So `providers` stayed at the
        photograph taken when Epoch opened, and CREW LINKS went on reporting it.

        Measured 2026-09-08 on a freshly installed machine: `ollama serve` answering
        `/api/tags` with one model, and the panel still reading *not running — start it with
        `ollama serve`* forty-five seconds later. The Connections deck was right at the same
        moment, because it takes its own reading — two lists disagreeing about one fact, which
        is the failure `LocalRuntimes` already has a comment about.

        Bounded, and that is why this is affordable: `openai.rs` gives every probe a 700 ms
        `timeout_connect`, so a configured backend that is switched off costs that and not the
        twenty-one seconds a bare connect to a closed port takes on this machine.
      */
      void readBackends();
    };
    window.addEventListener("focus", cameBack);

    // Probes are the expensive half: each can start a local CLI or wait on a local endpoint.
    // Skipped when the cold start already ran them, which is the cost the start sequence was
    // built to move — and they stay reachable at any time through CHECK AGAIN.
    if (startup) {
      return () => {
        active = false;
        window.removeEventListener("focus", cameBack);
      };
    }

    // Provider and agent probes can start a local CLI or wait on a local endpoint. They are
    // useful readings, but never a reason for the Bridge itself to compete for the first paint.
    // Manual `CHECK AGAIN` remains immediate; this delay applies only to automatic startup.
    const survey = window.setTimeout(() => {
      void fetchProviders().then((next) => {
        if (!active) return;
        remember({ providers: next });
        setProviders(next);
        setProbing(false);
      });
    }, 350);
    return () => {
      active = false;
      window.removeEventListener("focus", cameBack);
      window.clearTimeout(survey);
    };
    // Startup is the value it was mounted with; a later change would mean a different bridge.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const id = setInterval(() => setBeat((b) => b + 1), 12_000);
    return () => clearInterval(id);
  }, []);

  // Selection is derived until the user makes one: the first berth is selected because it is
  // first, not because anything stored it.
  const selected: WorldSummary | null =
    view.worlds.find((w) => w.id === selectedId) ?? view.worlds[0] ?? null;

  const boarding = departing
    ? (view.worlds.find((w) => w.id === departing.id) ?? null)
    : null;

  // Enter boards the selected World from the bridge — the design's own shortcut, and the
  // reason the hint under BOARD SHIP is not decoration.
  useEffect(() => {
    if (deck !== "worlds" || departing || renaming) return;
    const onKey = (e: KeyboardEvent) => {
      const typing =
        e.target instanceof HTMLElement &&
        ["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName);
      if (e.key === "Enter" && !typing && selected) {
        setDeparting({ id: selected.id, instant: e.shiftKey });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [deck, departing, renaming, selected]);

  /**
   * Actually cross into the World.
   *
   * The Engine is asked only at this point. Everything before it — doors, warp, the arrival
   * card — is presentation over a decision the user has already made, and abandoning the
   * sequence must leave nothing loaded behind it.
   */
  const enter = async () => {
    if (!boarding) return;
    const ok = await enterWorld(boarding.id);
    setDeparting(null);
    if (ok) onEntered({ id: boarding.id, name: boarding.name });
    else
      setNotice(
        `Berth ${boarding.berth} is empty — that World is no longer installed.`,
      );
  };

  /**
   * Which World's removal is being read, and what it would do.
   *
   * Two pieces, like the crew's: the id opens the block, the plan arrives from the Engine.
   * Nothing is guessed here — a World's id lives in eight places and this surface knows none
   * of them.
   */
  const [removingWorld, setRemovingWorld] = useState<string | null>(null);
  const [worldPlan, setWorldPlan] = useState<RemovalView | null>(null);
  /**
   * The World being sent out, and what that would carry.
   *
   * Two pieces of state rather than one, exactly as removal has: *which* World is being asked
   * about is the user's intent and arrives instantly; *what it would carry* is the Engine's
   * answer and arrives after a walk of the folder. Collapsing them would mean the panel could
   * not open until the disk had been read.
   */
  const [exportingWorld, setExportingWorld] = useState<string | null>(null);
  const [exportPlan, setExportPlan] = useState<ExportView | null>(null);
  const [exporting, setExporting] = useState(false);

  /** Ask what sending this World would carry, and open the panel while it answers. */
  const askWorldExport = async (id: string) => {
    setNotice(null);
    setRemovingWorld(null);
    setExportingWorld(id);
    setExportPlan(null);
    const answer = await worldExport(id);
    if (typeof answer === "string") {
      setNotice(answer);
      setExportingWorld(null);
      return;
    }
    setExportPlan(answer);
  };

  /**
   * Send it, and say where it landed.
   *
   * The Engine rebuilds the plan rather than being handed the one on screen: a list of paths
   * that came out of a surface is a list somebody could have changed, and this one copies
   * files.
   */
  const commitWorldExport = async (id: string) => {
    setExporting(true);
    const answer = await exportWorld(id);
    setExporting(false);
    setExportingWorld(null);
    setExportPlan(null);
    if (typeof answer === "string") {
      setNotice(answer);
      return;
    }
    /*
      **Cancelling is an answer.** The dialog is how the user says *where*, and closing it is how
      they say *not now* — a red line for that would be Epoch reporting a fault somebody caused
      on purpose. Nothing is said, because nothing happened.
    */
    if (answer.at === null) return;
    setNotice(
      `Sent to ${answer.at}. Unzip it into another Epoch's Worlds folder to install it.`,
    );
  };

  const askWorldRemoval = async (id: string) => {
    setNotice(null);
    setRemovingWorld(id);
    setWorldPlan(null);
    const answer = await worldRemoval(id);
    if (typeof answer === "string") {
      setNotice(answer);
      setRemovingWorld(null);
      return;
    }
    setWorldPlan(answer);
  };

  const commitWorldRemoval = async (id: string) => {
    const failure = await removeWorld(id);
    setNotice(failure);
    setRemovingWorld(null);
    setWorldPlan(null);
    await refresh();
  };

  const commitRename = async () => {
    if (!renaming) return;
    const failure = await renameWorld(renaming.id, renaming.name);
    if (failure) {
      setNotice(failure);
      return;
    }
    setNotice(null);
    setRenaming(null);
    await refresh();
  };

  return (
    <div className="lx">
      <div className="lx__grille" aria-hidden />
      <div className="lx__vignette" aria-hidden />

      {/* ------------------------------------------------------------ viewport */}
      <header className="lx__view">
        <div className="lx__stars" aria-hidden />
        <div className="lx__stars--bright" aria-hidden />
        <div className="lx__nebula" aria-hidden />
        <div className="lx__planet" aria-hidden />
        <div className="lx__planet-halo" aria-hidden />
        <div className="lx__comet" aria-hidden>
          <b />
          <i />
        </div>

        <div className="lx__wordmark-wrap">
          <div className="lx__crest" aria-hidden>
            <i />
            <span className="lx__gem" />
            <i />
          </div>
          <h1 className="lx__wordmark">EPOCH</h1>
          <p className="lx__tagline">AI OPERATING SYSTEM</p>
          <p className="lx__motto">
            &ldquo;Orchestrate intelligence. Build worlds.&rdquo;
          </p>
        </div>

        <div className="lx__strut lx__strut--l" aria-hidden />
        <div className="lx__strut lx__strut--r" aria-hidden />
        <div className="lx__rail" aria-hidden />
        <div className="lx__rivets" aria-hidden>
          {Array.from({ length: 8 }, (_, i) => (
            <span key={i} />
          ))}
        </div>
        <div className="lx__fade" aria-hidden />

        {/*
          The orchestrator's plate — whose bridge this is.

          The portrait is yours to supply, through the same Asset path a World's artwork takes.
          No artwork is an honest state and the first-run one: a visible stand-in, never a
          generated avatar of a person who has not chosen a face.

          There is no progression bar. It was drawn cold for a subsystem nobody scheduled, and
          a frame reading NOT YET COUNTED for long enough stops being a promise and becomes
          furniture (owner, 2026-08-21).
        */}
        <div className="lx__plate">
          <div className="lx__portrait">
            {view.orchestrator.portrait ? (
              <img src={view.orchestrator.portrait} alt="" draggable={false} />
            ) : (
              <span className="lx__portrait-none" aria-hidden>
                <i />
                <b />
              </span>
            )}
          </div>

          <div className="lx__plate-body">
            {me !== null ? (
              <form
                className="lx__plate-form"
                onSubmit={(e) => {
                  e.preventDefault();
                  void (async () => {
                    const failure = await renameOrchestrator(me);
                    if (failure) setNotice(failure);
                    setMe(null);
                    await refresh();
                  })();
                }}
              >
                <input
                  value={me}
                  autoFocus
                  aria-label="Your name"
                  placeholder="Your name"
                  onChange={(e) => setMe(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Escape") setMe(null);
                  }}
                />
                <button type="submit" className="btn btn--mini">
                  OK
                </button>
              </form>
            ) : (
              <button
                type="button"
                className="lx__plate-name"
                title="Change what the bridge calls you"
                onClick={() =>
                  setMe(
                    view.orchestrator.isUnnamed ? "" : view.orchestrator.name,
                  )
                }
              >
                {view.orchestrator.name.toUpperCase()}
              </button>
            )}

            <div className="lx__plate-role">
              {view.worlds.length}{" "}
              {view.worlds.length === 1 ? "WORLD" : "WORLDS"} DOCKED ·{" "}
              {view.characters.length} CREW
            </div>
            {/*
              **LEVEL came out (owner, 2026-08-21): measure it or remove it.**

              It was drawn cold and honestly — `— / —`, no light, `NOT YET COUNTED` — which was
              the right treatment for an instrument waiting on a subsystem being built. The rule
              it rested on is that *a dormant panel earns its outline by saying what it is
              waiting for*, and the thing it was waiting for was never scheduled: there is no XP
              design, no ADR, and nothing anywhere in the Engine counts a completed Quest toward
              a score.

              A frame that says NOT YET COUNTED for long enough stops being a promise and starts
              being furniture. And the honest reading of the same discipline, applied to a wait
              with no end named, is that the gauge should not be there.

              What is not lost: Quests already persist with their evidence and their outcomes
              (ADR-0025), so if progression is ever designed, it will be scored from something
              real rather than from a counter that was ticking in the meantime. Removing the
              frame removes nothing but the frame.
            */}

            <ImageDrop
              label={view.orchestrator.portrait ? "CHANGE FACE" : "SET FACE"}
              className="lx__plate-drop"
              onChoose={async (data) => {
                const failure = await setOrchestratorPortrait(data);
                if (!failure) await refresh();
                return failure;
              }}
              onClear={
                view.orchestrator.portrait
                  ? async () => {
                      const failure = await setOrchestratorPortrait(null);
                      if (!failure) await refresh();
                      return failure;
                    }
                  : undefined
              }
            />
          </div>
        </div>
      </header>

      {/* ---------------------------------------------------------------- deck */}
      <div className="lx__deck">
        <nav className="pnl" aria-label="Main menu">
          <h2 className="pnl__title">MAIN MENU</h2>
          <div className="pnl__rule" />

          <div className="lxnav__list">
            {/*
              **Three groups, and they are named.**

              The order was `Connections → Machines → MCP → Workshop → Models`, which left
              MODELS stranded between the Workshop and Settings — the owner met it that way and
              said the flow broke there. It did: *what have I got · where can it run · on which
              machines · what tools can it use* is a progression, and the list was not in it.
              That order is unchanged.

              **The titles were refused once and the refusal has been reversed** (owner,
              2026-09-06). The argument for spacing alone — proximity is what the eye reads
              first, and a bridge is not a settings tree — was true and was not enough:
              proximity says *these belong together* and cannot say **what** they have in
              common. An independently built design of this same Launcher landed on the same
              nine decks and named its groups, which is the one thing in it that survived being
              measured. So the names are taken and the grouping is not: that design puts
              CONNECTIONS with the crew, and our deck holds runtimes, providers and reachable
              machines — machine-side, not crew-side.
            */}
            <h3 className="lxnav__zone">WORLDS &amp; CREW</h3>
            <NavItem
              on={deck === "worlds"}
              label="WORLDS"
              hint="Your universes & projects"
              onClick={() => setDeck("worlds")}
              glyph={<span className="g-orb" />}
            />
            <NavItem
              on={deck === "characters"}
              label="CHARACTERS"
              hint="Crew & agents"
              onClick={() => setDeck("characters")}
              glyph={<span className="g-crew" />}
            />
            {/*
              **Its own deck, not a corner of Settings** (owner, 2026-08-22).

              Settings answers *choices about your own machine*, and a whole making subsystem
              under that heading is the drift already fixed twice in this file: four answers
              under a title that names none of them.

              And it is not called *Images*. Measured on this machine's ComfyUI: 165 video
              nodes, 47 audio, 36 for 3D. A deck named for one medium would be renamed inside
              two phases.
            */}
            <NavItem
              on={deck === "creations"}
              label="CREATIONS"
              hint="What this World can make"
              onClick={() => setDeck("creations")}
              glyph={<span className="g-canvas" />}
            />

            {/*
              **How it works**, in the order somebody actually asks it.

              MODELS first because it is the noun the other three are about: *what have I got*,
              then *where can it run*, then *on which machines*, then *what can it reach*. It
              used to sit last, after the Workshop, which put the inventory after the shop that
              fills it and left the three questions above it answering about nothing named yet.
            */}
            <h3 className="lxnav__zone">MACHINE</h3>
            {/*
              Kept apart from the Workshop rather than under it, because the questions are
              opposite — one is a catalogue of what *could* be here, the other an inventory of
              what *is*, and the only irreversible button in either belongs to the inventory.
            */}
            <NavItem
              on={deck === "models"}
              label="MODELS"
              hint="What is on this machine"
              onClick={() => setDeck("models")}
              glyph={<span className="g-tools" />}
            />
            <NavItem
              on={deck === "connections"}
              label="CONNECTIONS"
              hint="Who thinks, and what runs here"
              onClick={() => setDeck("connections")}
              glyph={<span className="g-link" />}
            />
            {/*
              After CONNECTIONS because it is the next question, not a subordinate one:
              Connections answers *who does the thinking* and what this computer can run;
              this answers *where else a turn may go*.
            */}
            <NavItem
              on={deck === "machines"}
              label="MACHINES"
              hint="Other computers a turn may go to"
              onClick={() => setDeck("machines")}
              glyph={<span className="g-link" />}
            />
            <NavItem
              on={deck === "mcp"}
              label="MCP"
              hint="Tools from outside Epoch"
              onClick={() => setDeck("mcp")}
              glyph={<span className="g-tools" />}
            />

            {/* Fitting the ship out, and the preferences that govern it. */}
            <h3 className="lxnav__zone">SYSTEM</h3>
            <NavItem
              on={deck === "workshop"}
              label="WORKSHOP"
              hint="Tools, MCPs & automations"
              onClick={() => setDeck("workshop")}
              glyph={<span className="g-tools" />}
            />
            <NavItem
              on={deck === "settings"}
              label="SETTINGS"
              hint="Preferences &amp; system"
              onClick={() => setDeck("settings")}
              glyph={<span className="g-gear" />}
            />
          </div>

          <div className="lxnav__sep" />

          <button
            type="button"
            className="lxnav lxnav--exit"
            onClick={() =>
              void import("@tauri-apps/api/window").then((w) =>
                w.getCurrentWindow().close(),
              )
            }
          >
            <span className="lxnav__glyph">
              <span className="g-exit" />
            </span>
            <span>
              <span className="lxnav__label">EXIT EPOCH</span>
              <span className="lxnav__hint">Safe travels, orchestrator.</span>
            </span>
          </button>

          <div className="lxdock">
            <div>
              <span>DOCK</span>
              {/*
                Measured, and it used to be `SEALED & STABLE` — a phrase taken from the reference
                design's invented POST, where it sat next to `HULL INTEGRITY`. It read as a
                status and nothing produced it: the dock said it was stable in exactly the case
                where a World had failed to load.

                `problems` is what the Engine actually found while looking for Worlds, so that is
                what this now says. `CLEAR` is a real all-clear.
              */}
              <b
                className={
                  view.problems.length > 0 ? "lxdock--warn" : undefined
                }
              >
                {view.problems.length === 0
                  ? "CLEAR"
                  : `${view.problems.length} PROBLEM${view.problems.length === 1 ? "" : "S"}`}
              </b>
            </div>
            <div>
              <span>CREW</span>
              <span>{view.characters.length} aboard</span>
            </div>
          </div>
        </nav>

        <main className="lx__main">
          {deck === "worlds" && (
            <section className="pnl bay">
              <div className="bay__head">
                <div>
                  <div className="bay__heading">
                    <span className="lx__gem" aria-hidden />
                    <h2>WORLDS</h2>
                  </div>
                  <p className="bay__sub">
                    {view.worlds.length === 0
                      ? "The docking bay is empty."
                      : `${view.worlds.length} ${
                          view.worlds.length === 1 ? "vessel" : "vessels"
                        } docked. Choose your destination, then board.`}
                  </p>
                </div>
                {/*
                  It never needed a filesystem plugin. The Engine scaffolds the folder — the
                  frontend asks, and the frontend never touches disk (ADR-0024). Same shape as
                  every import in Epoch.
                */}
                <button
                  type="button"
                  className="btn"
                  onClick={() => setMakingWorld(true)}
                >
                  <span className="btn__plus">+</span> NEW WORLD
                </button>
              </div>

              <div className="bay__rule" />

              {notice && <p className="notice notice--warn">{notice}</p>}
              {view.problems.map((problem, i) => (
                <p key={i} className="notice notice--warn">
                  {problem}
                </p>
              ))}

              {loaded && !selected ? (
                <p className="notice">
                  No Worlds installed. A World is a folder containing a{" "}
                  <code>pack.toml</code>.
                </p>
              ) : selected ? (
                <>
                  {/*
                    **The floor of the docking bay**, so the bay and the ship in it can sit side
                    by side on a wide screen. A wrapper rather than a grid on the section: the
                    heading, the rule and the faults above are page furniture and must keep
                    running the full width.
                  */}
                  <div className="bay__floor">
                  <div className="bay__hero">
                    {/* The left column: the World as seen, and where it works. Both are
                        properties of this World rather than of the application. */}
                    <div className="bay__left">
                      <div className="vp">
                        {/*
                        Two ways of seeing the same World, and both are real.

                        Key art is what the author wants you to feel about the place — how much
                        immersion it earns, how detailed it is allowed to look. That is theirs
                        to decide, which is why we do not draw it for them.

                        The chart is derived from the World's actual terrain, roads and Place
                        positions, so it cannot show something the World does not contain. It
                        is the floor: every World with a map has one, free, and it is what you
                        get when nobody supplied artwork.

                        Artwork wins by default, and the chart is one click away. Neither is a
                        placeholder for the other.
                      */}
                        {selected.art && !chart ? (
                          <img
                            className="vp__art"
                            src={selected.art}
                            alt={`Key art for ${selected.name}`}
                            draggable={false}
                          />
                        ) : (
                          <WorldPreview
                            preview={selected.preview}
                            name={selected.name}
                          />
                        )}

                        <div className="vp__scan" aria-hidden />
                        <div className="vp__vig" aria-hidden />
                        <div className="vp__berth">
                          {berthLabel(selected.berth)} ·{" "}
                          {selected.art && !chart ? "VIEWPORT" : "CHART"}
                        </div>

                        <div className="vp__tools">
                          {selected.art && selected.preview && (
                            <button
                              type="button"
                              className="btn btn--mini"
                              onClick={() => setChart(!chart)}
                            >
                              {chart ? "ART" : "CHART"}
                            </button>
                          )}
                          <ImageDrop
                            label={selected.art ? "REPLACE ART" : "ADD ART"}
                            onChoose={async (data) => {
                              const failure = await setWorldArt(
                                selected.id,
                                data,
                              );
                              if (!failure) {
                                setChart(false);
                                await refresh();
                              }
                              return failure;
                            }}
                            onClear={
                              selected.art
                                ? async () => {
                                    const failure = await setWorldArt(
                                      selected.id,
                                      null,
                                    );
                                    if (!failure) await refresh();
                                    return failure;
                                  }
                                : undefined
                            }
                          />
                        </div>

                        <div
                          className={`vp__link ${
                            selected.problems.length
                              ? "vp__link--warn"
                              : "vp__link--ok"
                          }`}
                        >
                          <span className="lamp" />
                          {selected.problems.length
                            ? "LINK DEGRADED"
                            : "LINK STABLE"}
                        </div>
                      </div>

                      {/*
                      Where this World actually works (ADR-0025). Under the viewport because
                      that is what it is a property of — a World, not the application. A single
                      global root would be a lie the moment a second World exists.

                      Setting it is ungated: it says *where* the work happens, never what the
                      work is allowed to do there. That second question is the Trust Engine's,
                      and it is asked separately every time something with an effect runs.
                    */}
                      <div
                        className={`proot${selected.projectMissing ? " proot--gone" : ""}`}
                      >
                        <span className="proot__label">PROJECT ROOT</span>
                        <div className="proot__row">
                          <span
                            className="proot__path"
                            title={selected.projectRoot ?? undefined}
                          >
                            {selected.projectRoot
                              ? shortRoot(selected.projectRoot)
                              : "none — nothing to read yet"}
                          </span>
                          <button
                            type="button"
                            className="btn btn--mini"
                            disabled={busy}
                            onClick={() =>
                              void chooseRoot(selected.id, selected.projectRoot)
                            }
                          >
                            {selected.projectRoot ? "CHANGE" : "CHOOSE"}
                          </button>
                          {selected.projectRoot && (
                            <button
                              type="button"
                              className="btn btn--mini"
                              disabled={busy}
                              onClick={() => void setRoot(selected.id, null)}
                            >
                              CLEAR
                            </button>
                          )}
                        </div>
                        {selected.projectMissing && (
                          <p className="proot__note">
                            That folder is not there any more. Your choice is
                            kept — pick it again or clear it.
                          </p>
                        )}
                      </div>

                      {/*
                        What this World knows, as opposed to where it works.

                        Beside the Project Root and deliberately not merged with it. One folder
                        setting would mean pointing a World at your notes in order to read them,
                        and thereby handing `write_file` and `run_command` the same folder — a
                        permission decision made silently by a convenience.

                        A library is read and never written: no writing capability is ever
                        constructed over one. Somebody's vault is years of their thinking.
                      */}
                      <div
                        className={`proot${selected.libraryMissing ? " proot--gone" : ""}`}
                      >
                        <span className="proot__label">LIBRARY</span>
                        <div className="proot__row">
                          {selected.library ? (
                            <button
                              type="button"
                              className="proot__path proot__path--open"
                              title={`${selected.library} — open in Obsidian`}
                              disabled={busy || selected.libraryMissing}
                              onClick={() => void openNotes(selected.id)}
                            >
                              {shortRoot(selected.library)}
                            </button>
                          ) : (
                            <span className="proot__path">
                              none — the crew knows nothing yet
                            </span>
                          )}
                          <button
                            type="button"
                            className="btn btn--mini"
                            disabled={busy}
                            onClick={() =>
                              void chooseNotes(selected.id, selected.library)
                            }
                          >
                            {selected.library ? "CHANGE" : "CHOOSE"}
                          </button>
                          {selected.library && (
                            <button
                              type="button"
                              className="btn btn--mini"
                              disabled={busy}
                              onClick={() => void setNotes(selected.id, null)}
                            >
                              CLEAR
                            </button>
                          )}
                        </div>
                        <p className="proot__note">
                          {selected.libraryMissing
                            ? "That folder is not there any more. Your choice is kept — pick it again or clear it."
                            : "An Obsidian vault, or any folder of markdown. Read, never written."}
                        </p>
                      </div>
                    </div>

                    <div className="brief">
                      {renaming?.id === selected.id ? (
                        <form
                          className="cedit__field"
                          onSubmit={(e) => {
                            e.preventDefault();
                            void commitRename();
                          }}
                        >
                          <span>RENAME WORLD</span>
                          <input
                            value={renaming.name}
                            autoFocus
                            aria-label="World name"
                            onChange={(e) =>
                              setRenaming({
                                id: selected.id,
                                name: e.target.value,
                              })
                            }
                            onKeyDown={(e) => {
                              if (e.key === "Escape") setRenaming(null);
                            }}
                          />
                          <div className="cedit__actions">
                            <button type="submit" className="btn btn--mini">
                              SAVE
                            </button>
                            <button
                              type="button"
                              className="btn btn--mini"
                              onClick={() => setRenaming(null)}
                            >
                              CANCEL
                            </button>
                          </div>
                        </form>
                      ) : (
                        <h3 className="brief__name">
                          {selected.name.toUpperCase()}
                        </h3>
                      )}

                      <div className="brief__kind">
                        {selected.kind ?? "UNDECLARED"} · v{selected.version}
                      </div>

                      <p
                        className={`brief__desc${
                          selected.description ? "" : " brief__desc--none"
                        }`}
                      >
                        {selected.description ??
                          "This World does not describe itself. Add a description to its pack.toml and it will read here."}
                      </p>

                      <div className="brief__stats">
                        <div className="stat">
                          <div className="stat__label">CREW</div>
                          <div className="stat__value">
                            {selected.characters}
                          </div>
                        </div>
                        <div className="stat">
                          <div className="stat__label">PLACES</div>
                          <div className="stat__value">{selected.places}</div>
                        </div>
                        <div className="stat stat--wide">
                          <div className="stat__label">KNOWLEDGE</div>
                          {/* PENDING: the Knowledge Engine (ADR-0010). Nothing is derived yet. */}
                          <div className="stat__value stat__value--quiet">
                            not yet kept
                          </div>
                        </div>
                      </div>

                      <div className="brief__note">
                        <span className="lamp" />
                        {selected.coverage >= 1
                          ? "Charted end to end — every concept the Engine knows is described here."
                          : `Covers ${Math.round(
                              selected.coverage * 100,
                            )}% of what the Engine knows; the rest appears as visible placeholders.`}
                      </div>

                      {selected.problems.map((problem, i) => (
                        <p
                          key={i}
                          className="notice notice--warn"
                          style={{ marginTop: 9 }}
                        >
                          {problem}
                        </p>
                      ))}

                      <div className="brief__go">
                        <button
                          type="button"
                          className="board"
                          onClick={(e) =>
                            setDeparting({
                              id: selected.id,
                              instant: e.shiftKey,
                            })
                          }
                        >
                          BOARD SHIP
                          <span className="board__sheen" />
                        </button>
                        <div className="brief__ready">
                          AIRLOCK {String(selected.berth).padStart(2, "0")}{" "}
                          READY
                          <br />
                          ENTER to depart
                        </div>
                        <button
                          type="button"
                          className="btn btn--mini"
                          onClick={() => {
                            setNotice(null);
                            setRenaming({
                              id: selected.id,
                              name: selected.name,
                            });
                          }}
                        >
                          RENAME
                        </button>
                        {/*
                          **Send before remove**, and not only because it is friendlier. The two
                          buttons sit together and one of them is irreversible; putting the
                          reversible one first is the same reasoning that makes a removal state
                          what survives.
                        */}
                        <button
                          type="button"
                          className="btn btn--mini"
                          onClick={() => void askWorldExport(selected.id)}
                        >
                          EXPORT
                        </button>
                        <button
                          type="button"
                          className="btn btn--mini"
                          onClick={() => void askWorldRemoval(selected.id)}
                        >
                          REMOVE
                        </button>
                      </div>
                    </div>
                  </div>

                  {exportingWorld === selected.id && (
                    <div className="bay__removal">
                      <ExportConfirm
                        plan={exportPlan}
                        busy={exporting}
                        onCancel={() => {
                          setExportingWorld(null);
                          setExportPlan(null);
                        }}
                        onConfirm={() => void commitWorldExport(selected.id)}
                      />
                    </div>
                  )}

                  {removingWorld === selected.id && (
                    <div className="bay__removal">
                      <RemoveConfirm
                        plan={worldPlan}
                        busy={false}
                        onCancel={() => {
                          setRemovingWorld(null);
                          setWorldPlan(null);
                        }}
                        onConfirm={() => void commitWorldRemoval(selected.id)}
                      />
                    </div>
                  )}

                  <div className="bay__spacer" aria-hidden />

                  <div className="bay__strip">
                    <span>
                      DOCKING BAY — {view.worlds.length}{" "}
                      {view.worlds.length === 1 ? "BERTH" : "BERTHS"}
                    </span>
                    <span>SORTED BY IDENTITY</span>
                  </div>

                  <ul className="berths">
                    {view.worlds.map((world) => (
                      <li key={world.id}>
                        <button
                          type="button"
                          className={`berth${world.id === selected.id ? " berth--on" : ""}`}
                          onClick={() => {
                            setSelectedId(world.id);
                            // The author's framing is the default for every World, so peeking
                            // at one World's chart does not follow you to the next.
                            setChart(false);
                          }}
                          aria-pressed={world.id === selected.id}
                        >
                          <div className="berth__tile">
                            {world.art ? (
                              <img src={world.art} alt="" draggable={false} />
                            ) : (
                              <WorldPreview
                                preview={world.preview}
                                name={world.name}
                              />
                            )}
                            <div className="berth__grid" aria-hidden />
                            <div className="berth__shade" aria-hidden />
                            <div className="berth__no">
                              {berthLabel(world.berth)}
                            </div>
                            {world.problems.length > 0 && (
                              <span
                                className="berth__flag"
                                title="This World has faults"
                              />
                            )}
                          </div>
                          <div className="berth__body">
                            <div className="berth__name">
                              {world.name.toUpperCase()}
                            </div>
                            <div className="berth__facts">
                              <span>{world.characters} CREW</span>
                              <span>{Math.round(world.coverage * 100)}%</span>
                            </div>
                          </div>
                        </button>
                      </li>
                    ))}
                  </ul>
                  </div>
                </>
              ) : null}
            </section>
          )}

          {deck === "connections" && (
            <>
              {/*
                **One panel, one THIS MACHINE** (owner, 2026-08-21). The hardware and the local
                runtimes were a section of their own directly above this one, under a heading
                with the same name as a list inside it. They are the first answer to the
                question this panel already asks, so they moved into it.
              */}
              <ConnectionsPanel
                providers={providers}
                probing={probing}
                onProbe={() => void probe()}
                /*
                  The runtimes block takes its own reading when the deck opens. That reading is
                  the answer to *what can think*, so the summary above it and the panel beside
                  it have to come from the same measurement or they will disagree — which is
                  what a user reported: llama.cpp SERVING on this deck and absent from CREW
                  LINKS. Backends only; the agents are a different question.
                */
                onRuntimesRead={() => void readBackends()}
              />
            </>
          )}

          {deck === "machines" && (
            <section className="pnl bay">
              <div className="bay__head">
                <div>
                  <div className="bay__heading">
                    <span className="lx__gem" aria-hidden />
                    <h2>MACHINES</h2>
                  </div>
                  {/*
                    **One question, said out loud** (owner, 2026-08-20). This deck held the
                    local machine, the paired ones, a Hugging Face reading and an installer,
                    which is four answers under one heading — and the heading named none of
                    them. What is left is the one thing only this deck can say.
                  */}
                  <p className="bay__sub">
                    Where a turn may go, besides this machine.
                  </p>
                </div>
              </div>
              <div className="bay__rule" />
              {/*
                Paired machines, the pairing door, and where a turn may travel (ADR-0029).

                Probed on every change to the roster: pairing a machine, unpairing one, or
                taking `compute` away all change what can think, and none of them should wait
                for somebody to find the button on another deck.
              */}
              <PairedMachines onChanged={() => void probe()} />
            </section>
          )}

          {deck === "mcp" && (
            <>
              {/*
                The crew's capability list is built from these servers, so changing one changes
                what a character can be given. Forgetting a server left it on screen as a group
                with a tool count beside it — a box that granted a connection nobody had.

                The Engine was right the whole time; the Launcher had simply read the vocabulary
                once, on open, and nothing could tell it otherwise. Same wire the crew editor
                already uses.
              */}
              <McpPanel onChanged={() => void refresh()} />
              {/*
                One deck, because it is one protocol and the user should not have to learn that
                it has a direction: tools coming in, and Epoch's tools going out.
              */}
              <AgentDoorPanel door={agentDoor} crew={view.characters} />
            </>
          )}

          {deck === "creations" && <CreationsPanel />}

          {deck === "workshop" && (
            <WorkshopPanel
              onInstalled={refresh}
              onMeasure={(model) => {
                setToMeasure(model);
                setDeck("models");
              }}
            />
          )}
          {deck === "models" && (
            <ModelsHere
              measure={toMeasure}
              onMeasureTaken={() => setToMeasure(null)}
              reveal={toReveal}
              onRevealed={() => setToReveal(null)}
            />
          )}

          {deck === "characters" && (
            <CharacterPanel
              view={view}
              providers={providers}
              onChanged={refresh}
              /*
                A character shows the window MODELS applied and cannot change it, so the panel
                has to be able to say **where** it can be changed. Reveals the row; it never
                starts a search — that errand is `measure`, and it costs a graphics card.
              */
              onConfigureInModels={(model) => {
                setToReveal(model);
                setDeck("models");
              }}
            />
          )}

          {deck === "settings" && (
            <section className="pnl bay">
              <div className="bay__head">
                <div>
                  <div className="bay__heading">
                    <span className="lx__gem" aria-hidden />
                    <h2>SETTINGS</h2>
                  </div>
                  <p className="bay__sub">
                    Choices about your own machine, grouped by what they affect.
                    Nothing here changes a World.
                  </p>
                </div>
              </div>
              <div className="bay__rule" />

              {/*
                **Six groups, each named for what it changes** (owner, 2026-09-06).

                The deck had three headings, two blocks under none at all, and an order that came from
                the order things were built in. So `EraseData` — what Epoch keeps on your disk — sat
                under *Crew behaviour*, and the two panels about downloading a model opened the deck
                with no heading over them.

                Named blocks were added here earlier today and they were the right half of the fix: a
                heading tells you a subject has started and cannot tell you the control under it is in
                the wrong subject. **Nothing was added this time except a sentence per group and a new
                order** — every control is the one it was.

                The groups are ours rather than the imported design’s five: it has no equivalent of
                *which machines may put a model on disk*, and its PERFORMANCE is only about memory,
                while ours also bounds how many model calls one turn may make. Same rule as the menu
                zones — the idea is taken, the list is measured against what is actually here.
              */}

              <div className="rm rm--group">
                <span className="rm__label">Audio</span>
                <span className="rm__about">Sound Epoch makes, and the devices it comes through.</span>
              </div>

              <label className="setting">
                <input
                  type="checkbox"
                  checked={!muted}
                  onChange={(e) => setMuted(!e.target.checked)}
                />
                <span>
                  <b>Interface sound</b>
                  <i>
                    Synthesised UI feedback only: no remote audio, no autoplay
                    and no sound until you first interact with Epoch. This
                    choice stays on this machine.
                  </i>
                </span>
              </label>

              <label className="setting setting--range">
                <span>
                  <b>Interface volume</b>
                  <i>
                    {Math.round(volume * 100)}% - hover, buttons, opening a
                    Quest and errors.
                  </i>
                </span>
                <input
                  type="range"
                  min="0"
                  max="100"
                  value={Math.round(volume * 100)}
                  aria-label="Interface volume"
                  onChange={(e) => setVolume(Number(e.target.value) / 100)}
                />
              </label>

              <AudioSettings />

              {/*
                Ninety-nine characters at 2,560, measured — the one sentence on this deck that a
                `.notice` was drawing at whatever width it was given. Same cap as everything
                else that is read rather than scanned.
              */}
              <p className="notice notice--read">
                World Pack selection and the visual vocabulary are World-owned.
                Audio is local. The controls above are live and local to this
                device.
              </p>

              <div className="bay__rule" />

              <div className="rm rm--group">
                <span className="rm__label">Language &amp; voice</span>
                <span className="rm__about">What you speak, and what the crew is told about it.</span>
              </div>

              <SpokenLanguage />

              <div className="bay__rule" />

              <div className="rm rm--group">
                <span className="rm__label">Performance</span>
                <span className="rm__about">How much of this machine Epoch may hold, and how much one turn may do.</span>
              </div>

              <label className="setting">
                <input
                  type="checkbox"
                  checked={settings.concurrentCrew}
                  onChange={(e) =>
                    void (async () => {
                      // **Only the field this control owns crosses the wire.** Sending a whole
                      // `Settings` from a form that manages one checkbox is what deleted an
                      // agent sign-in and a GPU backend on every press.
                      const next = { ...settings, concurrentCrew: e.target.checked };
                      setSettings(next);
                      const failure = await setConcurrentCrew(e.target.checked);
                      if (failure) {
                        setSettingsSaid(failure);
                        setSettings(await fetchSettings());
                      }
                    })()
                  }
                />
                <span>
                  <b>Run several crew members at once</b>
                  <i>
                    Off: the last speaker&rsquo;s model stays loaded, and a
                    different character speaking lets go of it first — at most
                    one at a time. On: every crew member&rsquo;s model stays
                    warm, so replies are instant across the whole crew and each
                    one holds its memory. Either way a model is released when a
                    picture or a video needs the card, and whenever you turn KEEP
                    off above a conversation. Applies to every machine this World
                    runs on, including paired ones.
                  </i>
                  {/*
                    **Measured, and it runs the other way on a card that also draws.** On a
                    12 GB card holding an 11.9 GB Flux checkpoint, turning this on made
                    everything slower — renders worst of all, 117 s against 34 s — because three
                    runtimes each keeping a copy of a 7.5 GB model is 12 GB asked to hold about
                    22. Somebody using one runtime and not drawing may still see the plain win it
                    promises, and the honest thing is to say which case was measured rather than
                    to recommend.

                    Said here because this is where the choice is made. Two characters working at
                    once does not need it — that is a turn each, and it works either way; what
                    this buys is not having to load the model again.
                  */}
                  <i>
                    Measured on a 12 GB card that also draws: <b>on</b> was
                    slower, because two models and a checkpoint do not fit at
                    once. It costs nothing to leave off — two crew members can
                    still work at the same time, they just each pay the load.
                  </i>
                </span>
              </label>

              {/*
                Measured, not guessed. On the machine this was built for, one 14B model held
                4 GB of VRAM and stayed resident for five minutes after the conversation ended
                — with the whole crew idle. Two at once is most of a consumer graphics card.

                **And it no longer decides whether a model is held at all** (2026-08-28). It
                did, which meant the ordinary machine paid ~19 s reloading on every single
                message to protect against a cost that only *several* models have. One is not
                several: the World runs one turn at a time, so the last speaker's model is what
                the card had loaded a second ago anyway.
              */}
              {/*
                **How many rounds of tools one turn may take.**

                The default's reasoning is *look → read → read → answer*, and it was written
                before a character could have an MCP server's thirty-two tools attached. The
                owner watched one spend the budget on Spotify searches, announce *"Te lo
                reproduzco ahora:"* and stop — correctly, and with nothing left to play it with.

                So the number is the machine's now, **including none at all**. It is bounded by
                default because each round is a full model call and a model that has
                misunderstood loops happily forever; `0` is the user's to choose on their own
                machine, and STOP is checked before every call either way.
              */}
              <label className="setting setting--wide">
                <span>
                  Tool rounds in one turn
                  <i>
                    Each round is a whole model call. 0 is no limit — STOP still ends it.
                  </i>
                </span>
                <input
                  type="number"
                  min={0}
                  max={99}
                  value={settings.toolRounds ?? 8}
                  onChange={(e) =>
                    void (async () => {
                      const rounds = Math.max(
                        0,
                        Math.min(99, Number(e.target.value) || 0),
                      );
                      setSettings({ ...settings, toolRounds: rounds });
                      const failure = await setToolRounds(rounds);
                      if (failure) {
                        setSettingsSaid(failure);
                        setSettings(await fetchSettings());
                      }
                    })()
                  }
                />
              </label>

              <div className="bay__rule" />

              <div className="rm rm--group">
                <span className="rm__label">Downloads</span>
                <span className="rm__about">Where a model file may land, and the one key that is needed to fetch it.</span>
              </div>

              {/*
                **Which machines can put a model file on disk**, asked of each of them.

                Here rather than under MACHINES because it is not a machine — it is a reading
                about all of them, and the deck it was in had four answers under one heading.
                It stays one panel because `hf` on this computer says nothing about the machine
                lending a graphics card, and it is that machine a model would land on.
              */}
              <HuggingFaceDeck />

              {/*
                **Where a key belongs, and only where one is needed.** Measured per source: one
                of the two catalogues gates downloads behind an account and the other gates
                nothing, so exactly one row appears here. Searching never needs a key at all,
                which is why this sits in Settings rather than in front of the shelf.
              */}
              <CatalogueKeys />

              <div className="bay__rule" />

              <div className="rm rm--group">
                <span className="rm__label">Stored data</span>
                <span className="rm__about">What Epoch keeps on this disk, and how to clear it.</span>
              </div>

              <EraseData />

              <div className="bay__rule" />

              <div className="rm rm--group">
                <span className="rm__label">Security</span>
                <span className="rm__about">The credential agents use to reach Epoch’s own tools.</span>
              </div>

              {/*
                The one control on this deck that is not a preference.
                **Not a checkbox**, because it is not a state — it is an act, and it takes
                effect the moment it is pressed.
              */}
              <div className="setting setting--act">
                <span>
                  <b>Regenerate the agent door credential</b>
                  <i>
                    Agents reach Epoch's tools through a local door with a
                    bearer token, kept encrypted for this Windows account.
                    Regenerating it <b>closes every door that is open</b> —
                    press it if the old one may have escaped into a commit, a
                    screenshot or a paste. A project&apos;s{" "}
                    <code>.mcp.json</code> still holds the old one until you
                    connect that agent again.
                  </i>
                </span>
                <button
                  type="button"
                  className="btn btn--mini"
                  onClick={() =>
                    void (async () => {
                      setSettingsSaid("Regenerating…");
                      const failure = await regenerateDoor();
                      setSettingsSaid(
                        failure ??
                          "New door credential. Any agent you had connected needs connecting again.",
                      );
                    })()
                  }
                >
                  REGENERATE
                </button>
              </div>

              {/*
                Beside the thing that caused it. Nothing here is dismissed on a timer: a
                credential that just changed is worth leaving on screen until the next action
                replaces it.
              */}
              {settingsSaid && (
                <p className="notice notice--warn">{settingsSaid}</p>
              )}
            </section>
          )}

          {SHUT[deck] && (
            <section className="pnl deck">
              <div className="deck__grille" aria-hidden />
              <div className="deck__body">
                <div className="deck__seal" aria-hidden>
                  <i />
                </div>
                <h2 className="deck__name">{SHUT[deck]!.name}</h2>
                <div className="deck__state">HATCH SEALED — NOT YET BUILT</div>
                <p className="deck__copy">{SHUT[deck]!.copy}</p>
                <button
                  type="button"
                  className="btn deck__back"
                  onClick={() => setDeck("worlds")}
                >
                  RETURN TO BRIDGE
                </button>
              </div>
            </section>
          )}
        </main>

        <aside className="lx__aside">
          <CrewLinks
            providers={providers}
            agents={agents}
            busy={probing}
            onRefresh={() => void probe()}
            onManage={() => setDeck("connections")}
          />
          <Diagnostics view={view} machine={machine} />
        </aside>
      </div>

      <div className="lx__strip">
        {/*
          **WHAT JUST HAPPENED was removed** (owner, 2026-08-21).

          It was a window on the Activity Stream — in memory, bounded, forgetful — sitting above
          the Ship's Log, which is the *record*: persisted Quest digests that outlive the
          process. The idea was that "what has been going on" has a *now* and a *since*.

          In use it did not read that way. What it actually showed was probe results — the same
          six runtimes starting and stopping answering, over and over — which is the Connections
          deck's job, said worse and without the buttons that fix it. The Ship's Log beside it
          was the panel with something to say.

          The stream itself stays (ADR-0015): the emitters, the metadata and the bounded Tail
          are the contract, and Observability consumes it. What is gone is a surface that turned
          a nervous system into a scrolling list of the same fact.
        */}
        <ShipsLog view={view} log={log} />
        {/*
          NEW WORLD opens the dialog from here too. It is the same act wherever it is pressed,
          so it must not be two different things depending on which panel you were looking at.
        */}
        <BridgeConsole log={log} onNewWorld={() => setMakingWorld(true)} />
        <TipOfTheDay index={beat} />
        <CrewChannel
          crew={view.characters}
          index={beat}
          onRoster={() => setDeck("characters")}
        />
      </div>

      {makingWorld && (
        /*
          A new World is empty, so it opens in the World Editor. Dropping somebody into nothing
          and letting them find their own way out would be the worst first five minutes Epoch
          could offer.
        */
        <NewWorld
          crew={view.characters}
          onClose={() => setMakingWorld(false)}
          onMade={(world) => onEntered({ ...world, authoring: true })}
        />
      )}

      {boarding && departing && (
        <Departure
          name={boarding.name}
          id={boarding.id}
          berth={boarding.berth}
          crew={boarding.characters}
          instant={departing.instant}
          onEnter={() => void enter()}
          onAbort={() => setDeparting(null)}
        />
      )}
    </div>
  );
}

function NavItem({
  on,
  label,
  hint,
  glyph,
  onClick,
}: {
  readonly on: boolean;
  readonly label: string;
  readonly hint: string;
  readonly glyph: React.ReactNode;
  readonly onClick: () => void;
}) {
  return (
    <button
      type="button"
      className={`lxnav${on ? " lxnav--on" : ""}`}
      onClick={onClick}
      aria-current={on ? "page" : undefined}
    >
      <span className="lxnav__glyph" aria-hidden>
        {glyph}
      </span>
      <span>
        <span className="lxnav__label">{label}</span>
        <span className="lxnav__hint">{hint}</span>
      </span>
      <span className="lxnav__chev" aria-hidden>
        &gt;
      </span>
    </button>
  );
}
