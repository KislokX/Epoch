/**
 * The World — the home screen and the permanent navigation hub.
 *
 * There is no Home. There is only the World (EXPERIENCE_CONSTITUTION). It renders on the first
 * frame, before any data arrives, and fills in as the engine answers. It never shows a loading
 * screen in front of itself (Build From Life, rule 1).
 *
 * ## The HUD is over the World, never instead of it
 *
 * Four layers, and the order is the whole design:
 *
 * ```
 * backdrop   authored art: sky, horizon, distance          (World Pack, optional)
 * scrim      so the instruments stay readable
 * stage      the World itself — terrain, roads, Places, inhabitants
 * hud        the instruments, laid over the top
 * ```
 *
 * The stage keeps its camera, its drag, its `Visit` and its reveal. The HUD is `pointer-events:
 * none` except on its own panels, so the middle of the screen is still the World — dragging
 * between the sidebars pans, exactly as it did before there was a HUD.
 *
 * ## Every reading is measured or visibly zero
 *
 * Same rule as the bridge. The reference HUD ships a working ship — ENERGY 85/100, five models
 * online, four workflows, a dock of features. Those panels are all here, at their true
 * readings, each with a note naming the subsystem that will light it up.
 *
 * Two mappings are deliberately *better* than the reference rather than dimmer, because real
 * data existed for them:
 *
 * - **Crew** lists the real inhabitants with what they are really doing, and clicking one
 *   travels to them. The reference popped a toast.
 * - **The dock** is the World's Places. A dock is for navigation, and `Visit` *is* navigation
 *   here — so instead of four dead buttons it is the real thing.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { Dialogue } from "../components/hud/Dialogue";
import { ConnectedModels } from "../components/hud/ConnectedModels";
import { WorldDock } from "../components/hud/WorldDock";
import { Frame } from "../components/hud/Frame";
import { refreshSounds } from "../experience/useSounds";
import { setVoicesOn, voicesOn } from "../experience/audio";
import { hush } from "../experience/speak";
import { PixelIcon, Portrait, Toast } from "../components/hud/Pixel";
import type { Glyph, ToastTone } from "../components/hud/Pixel";
import { CrewCard } from "../components/hud/CrewCard";
import type { CrewAllowanceReading } from "../components/hud/CrewCard";
import { SidebarCard as Card } from "../components/hud/SidebarCard";
import { Minimap } from "../components/Minimap";
import { WorldEditor } from "./editor/WorldEditor";
import { ALL_LAYERS, type Layers } from "./editor/stage";
import { WorldMap } from "../components/WorldMap";
import { useCamera } from "../experience/useCamera";
import { useAgentDoor } from "../experience/useAgentDoor";
import { AgentDoorPanel } from "./AgentDoorPanel";
import { Files } from "./Files";
import { Missions } from "./Missions";
import { Terminals } from "../components/Terminals";
import { useTerminal } from "../experience/useTerminal";
import { useTravel } from "../experience/useTravel";
import { useTurn } from "../experience/useTurn";
import { useVisit } from "../experience/useVisit";
import { playSfx, useSfx } from "../experience/sfx";
import { useWorld } from "../hooks/useWorld";
import {
  fetchAgentPlanUsage,
  fetchAgents,
  fetchBackdrop,
  fetchProviders,
  fetchWorlds,
  openLibrary,
} from "../ipc/launcher";
import type {
  AgentCredits,
  AgentPlanUsage,
  AgentStatus,
} from "../ipc/launcher";
import {
  closeQuest,
  fetchConversations,
  fetchRememberedContexts,
  fetchWorkspace,
  selectQuest,
} from "../ipc/world";
import type { ContextReading, QuestSummary } from "../ipc/world";
import type {
  CharacterView,
  Orchestrator,
  PlaceView,
  ProviderStatus,
} from "../ipc/contracts";

/** Place concepts, given a glyph. Classification only — never a name (ADR-0017). */
const CONCEPT_GLYPH: Record<string, Glyph> = {
  knowledge_center: "scroll",
  research_lab: "potion",
  automation_hub: "core",
  command_center: "star",
  guild: "shield",
};

/** Archetypes, given a glyph. Same rule. */
const ARCHETYPE_GLYPH: Record<string, Glyph> = {
  researcher: "potion",
  coordinator: "star",
  guardian: "shield",
  historian: "scroll",
};

/**
 * How wide the side columns are allowed to be.
 *
 * `SIDE_NATURAL` is not a taste — it is the width below which the cards inside stop fitting:
 * dragged narrower, the CONNECTED MODELS plate clipped to "CONNECTED MOD" and the minimap
 * squashed. A panel that can be dragged into illegibility is a panel that lies about its own
 * contents, so that is the floor rather than a smaller number that merely looked possible.
 *
 * The ceiling is derived, not chosen: whatever leaves the World a strip worth looking at, and
 * whatever the conversation is not already occupying. The HUD is over the World, never instead
 * of it — and never on top of another panel.
 */
const SIDE_NATURAL = 208;
const WORLD_MINIMUM = 420;

/**
 * How wide this column may be, right now.
 *
 * ## The collision, and who yields
 *
 * **Whoever is being dragged is the one that stops.** An earlier version had the column push
 * the conversation narrower as it grew, which is a panel resizing another panel — a box the
 * user sized should stay the size they chose. So this measures where the dialogue actually is
 * and refuses to pass it, in the same way the dialogue measures the HUD bar and refuses to
 * pass that.
 *
 * Measured rather than calculated from the dialogue's stored width, because the dialogue is
 * centred on the window and knowing *that* here would be this file learning another panel's
 * layout. A rectangle is a fact; a formula is a duplicate.
 */
/**
 * How tall the Terminal may be, right now.
 *
 * Bounded by the dock, measured — the same rule as everything else here. A constant would be
 * right for one window size and wrong for every other, and 640 was right for mine.
 */
function clampTerm(height: number): number {
  const wanted = Math.max(90, Math.round(height));
  const log = document.querySelector(".hud__term")?.getBoundingClientRect();
  const floor =
    document.querySelector(".hud__dock")?.getBoundingClientRect().top ??
    window.innerHeight;
  if (!log) return Math.min(wanted, 640);
  // From where the log starts to where the dock begins, less the grip's own row.
  return Math.min(wanted, Math.max(90, floor - log.top - 22));
}
function clampSide(width: number): number {
  // Two columns, so the World loses twice whatever one column gains.
  const room = (window.innerWidth - WORLD_MINIMUM) / 2;
  let most = Math.max(SIDE_NATURAL, Math.min(560, room));

  const conversation = document.querySelector(".dlg")?.getBoundingClientRect();
  if (conversation && conversation.width > 0) {
    const free = window.innerWidth - conversation.right - 12;
    most = Math.max(SIDE_NATURAL, Math.min(most, free));
  }

  return Math.max(SIDE_NATURAL, Math.min(most, Math.round(width)));
}
interface WorldScreenProps {
  /** Return to the Launcher. The frame owns this, never the World itself. */
  readonly onLeave: () => void;
  /**
   * Open in the World Editor rather than in the World.
   *
   * Set for a World that was just created. The Engine already froze it on the way in, so this
   * is the surface agreeing with a state that is already true rather than a second decision
   * that could disagree with it.
   */
  readonly authoring?: boolean;
}

export function WorldScreen({ onLeave, authoring = false }: WorldScreenProps) {
  const { status, view, note, reread } = useWorld();
  const { muted, setMuted } = useSfx();
  /**
   * Whether the crew speaks out loud right now.
   *
   * Read once and held here, because the switch is drawn here: `voicesOn()` is the source and
   * this is what the button shows. `speak.ts` asks the source again at the moment somebody would
   * speak, so a stale copy in a component can never be what decides.
   */
  const [speaks, setSpeaks] = useState(voicesOn);
  /**
   * How far along everybody who is walking is, this frame.
   *
   * Held here rather than inside the map, because the map is not the only thing that draws a
   * person: the minimap does too, and two interpolations would be two answers to where somebody
   * is. One frame loop, one answer, read by both.
   */
  const travelled = useTravel(view.characters);
  /** Whether the World is stopped and the editor is open. User intent, held here. */
  const [editing, setEditing] = useState(authoring);
  /** Local layout only: hiding a layer never changes the World or its vault. */
  const [layers, setLayers] = useState<Layers>(ALL_LAYERS);

  /** Authored backdrop, fetched once. Never part of the per-tick projection — it is megabytes. */
  const [backdrop, setBackdrop] = useState<string | null>(null);
  /**
   * The land its owner painted. Same rule as the backdrop: megabytes, so its own command.
   *
   * Fetched here as well as in the editor, and that matters — land visible while authoring and
   * gone the moment you press PLAY would mean the editor was showing a World that does not
   * exist. It is the same World either way.
   */
  const [land, setLand] = useState<string | null>(null);
  /** Who is at the console. Read once: it does not change while a World is open. */
  const [me, setMe] = useState<Orchestrator | null>(null);
  /** Who can think. Probed on mount — reaches the network, so the World never waits on it. */
  const [providers, setProviders] = useState<readonly ProviderStatus[]>([]);
  const [agentStatuses, setAgentStatuses] = useState<readonly AgentStatus[]>(
    [],
  );
  /**
   * Each signed-in **account's** plan windows, by account id.
   *
   * Per account, because an allowance belongs to a sign-in and not to a program. Two Claude Code
   * accounts have two separate windows, and drawing one of them twice would be a real reading of
   * the wrong quantity — the most convincing way a gauge can lie, because something is genuinely
   * being measured.
   *
   * A missing key is *unasked*; a `null` value is *asked and it could not say*. Neither is zero.
   */
  const [planUsage, setPlanUsage] = useState<
    Record<string, readonly AgentPlanUsage[] | null>
  >(measuredAllowances);
  /** Who the orchestrator is speaking to. User intent, like everything else held here. */
  const [talkingTo, setTalkingTo] = useState<string | null>(null);
  /** Which model each character was assigned. Read once with the crew; it is theirs, not the World's. */
  const [minds, setMinds] = useState<Record<string, string | null>>({});
  /**
   * Which model each character's agent actually thought with, last turn.
   *
   * Separate from `minds`, which is what they were *assigned*. A routing agent makes the two
   * differ, and only the agent can say which one it used — so this fills in when it reports it
   * and stays empty when it does not.
   */
  const [thoughtWith, setThoughtWith] = useState<Record<string, string>>({});
  /** An agent id exists only when this character's brain is an externally signed-in agent. */
  const [agents, setAgents] = useState<Record<string, string | null>>({});
  /** Last measured context by character, all scoped by the Engine to the active Quest. */
  const [crewContexts, setCrewContexts] = useState<
    Record<string, ContextReading>
  >({});
  const [toast, setToast] = useState<{
    title: string;
    message?: string;
    /** Which kind of thing happened. Knowledge kept is a gain, not an alert. */
    tone?: ToastTone;
  } | null>(null);
  const toastTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  /** Everything the crew has run this session. Listened to for everybody, not just whoever is being spoken to. */
  const terminal = useTerminal();
  /**
   * The door an agent knocks on. Listening always, opened never by this screen — a question can
   * arrive whether or not anybody is talking to anybody.
   */
  const agent = useAgentDoor();
  const terminalLog = useRef<HTMLDivElement>(null);
  /**
   * How tall the Terminal is, in pixels. User intent, remembered — a build's output and a single
   * `git status` want very different amounts of room, and only the person watching knows which
   * they are doing.
   */
  const [termHeight, setTermHeight] = useState(() => {
    const raw = Number(window.localStorage.getItem("epoch.terminal.height"));
    return Number.isFinite(raw) && raw >= 90 ? raw : 190;
  });
  /**
   * How wide the right column is.
   *
   * Widening lives on the *column*, not on the Terminal, because a card cannot be wider than
   * the column holding it — and a card that escaped its column would be a panel overlapping the
   * World. So the System Map and Connected Models widen with it, which is honest: they were
   * cramped at 208 too.
   */
  const [sideWidth, setSideWidth] = useState(() => {
    const raw = Number(window.localStorage.getItem("epoch.side.width"));
    return Number.isFinite(raw) ? clampSide(raw) : SIDE_NATURAL;
  });

  /*
    **This World's own voice, asked for now that there is a World.**

    `SfxGate` mounts once at the application root — on the Launcher, before anybody has entered
    anywhere — so the answer it cached was the empty one a bridge with no World open gives. It
    never asked again, because nothing else mounts to make it: unlike a `Frame`, there is exactly
    one player and it lives above every World.

    Measured in the window: the Engine answered `world_sounds` with the World's own click while
    every press still produced Epoch's synthesised oscillator.

    Here rather than in the gate, because *a World was entered* is this screen's fact.
  */
  useEffect(() => {
    refreshSounds();
  }, []);

  // The window can get smaller than the column somebody dragged in a bigger one.
  useEffect(() => {
    const onResize = () => {
      setSideWidth((w) => clampSide(w));
      setTermHeight((h) => clampTerm(h));
    };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  /*
    A plan allowance belongs to the signed-in **account**, not to a Quest and not to a program.

    One measurement per account, keyed by account id, because two Claude Code sign-ins have two
    separate windows. Asking once for "claude-code" and drawing it twice would be a real reading
    of the wrong quantity — the most convincing way a gauge can lie.

    It starts after the World has painted: an account probe runs a program, and useful probes
    never belong on the first-render critical path. It re-runs when the account list changes,
    because an account added while the World is open should light its own gauge.
  */
  const measurable = agentStatuses
    .filter((it) => it.installed && it.signedIn !== false)
    .map((it) => it.id)
    .join(",");
  useEffect(() => {
    if (measurable === "") return;
    let alive = true;
    const ids = measurable.split(",");
    const measure = () => {
      void Promise.all(ids.map((id) => fetchAgentPlanUsage(id))).then(
        (readings) => {
          if (!alive) return;
          setPlanUsage((held) => {
            const next = { ...held };
            ids.forEach((id, at) => {
              next[id] = readings[at] ?? null;
            });
            // Kept where the next mount can find it. See `measuredAllowances`.
            measuredAllowances = next;
            return next;
          });
        },
      );
    };
    const first = window.setTimeout(measure, 900);
    const timer = window.setInterval(measure, 60_000);
    return () => {
      alive = false;
      window.clearTimeout(first);
      window.clearInterval(timer);
    };
  }, [measurable]);
  useEffect(() => {
    window.localStorage.setItem("epoch.terminal.height", String(termHeight));
    window.localStorage.setItem("epoch.side.width", String(sideWidth));
  }, [termHeight, sideWidth]);

  /**
   * Drag the Terminal's corner. Down is taller, **left** is wider.
   *
   * Left rather than right because the column is anchored to the right edge of the window: it
   * grows towards the middle, and a grip that grew it the other way would push it off screen.
   */
  const grabTerminal = (e: React.PointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    const handle = e.currentTarget;
    handle.setPointerCapture(e.pointerId);
    const from = { x: e.clientX, y: e.clientY };
    const start = { w: sideWidth, h: termHeight };
    const move = (m: PointerEvent) => {
      setTermHeight(clampTerm(start.h + (m.clientY - from.y)));
      setSideWidth(clampSide(start.w - (m.clientX - from.x)));
    };
    const drop = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", drop);
      handle.removeEventListener("pointercancel", drop);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", drop);
    handle.addEventListener("pointercancel", drop);
  };

  // The newest line is the one worth seeing, so the panel follows the work rather than making
  // somebody scroll to find out what is happening now.
  useEffect(() => {
    const el = terminalLog.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [terminal]);

  useEffect(() => {
    let alive = true;
    void fetchBackdrop().then((data) => alive && setBackdrop(data));
    // Asked once on arrival. A vault can be changed from the Launcher, which means leaving this
    // World and coming back — so there is no state here that can go stale without a remount.
    void fetchWorkspace().then(
      (where) => alive && setLibrary(where?.library ?? null),
    );
    void invoke<string | null>("world_land").then(
      (data) => alive && setLand(data),
    );
    void fetchWorlds().then((v) => {
      if (!alive) return;
      setMe(v.orchestrator);
      // `brain`, not `model`. Reading `model` made a character with an agent — and no model
      // named, which is the common case — look like somebody who could not think at all.
      setMinds(Object.fromEntries(v.characters.map((c) => [c.id, c.brain])));
      setAgents(Object.fromEntries(v.characters.map((c) => [c.id, c.agent])));
    });
    // These probes may start a local agent or wait for an offline local endpoint. Let the World
    // establish its first visual frame before asking; status fills in independently afterwards.
    const background = window.setTimeout(() => {
      void fetchProviders().then((p) => alive && setProviders(p));
      void fetchAgents().then((found) => alive && setAgentStatuses(found));
    }, 250);
    return () => {
      alive = false;
      window.clearTimeout(background);
    };
  }, []);

  const ping = (title: string, message?: string, tone?: ToastTone) => {
    setToast({ title, message, tone });
    if (toastTimer.current) clearTimeout(toastTimer.current);
    toastTimer.current = setTimeout(() => setToast(null), 2600);
  };

  useEffect(
    () => () => {
      if (toastTimer.current) clearTimeout(toastTimer.current);
    },
    [],
  );

  const extent = useMemo(
    () =>
      view.map ? { width: view.map.width, height: view.map.height } : null,
    [view.map],
  );

  /**
   * Where the World opens.
   *
   * The largest Place, which in an authored World is the one everything else is arranged
   * around. Derived from the World rather than named in code, so a different World opens
   * wherever *its* centre of gravity is — and a World with no Places still opens.
   */
  const arrival = useMemo(() => {
    let widest: { x: number; y: number; footprint: number } | null = null;
    for (const place of view.places) {
      const at = place.placement;
      if (!at) continue;
      if (!widest || at.footprint > widest.footprint) {
        widest = { x: at.x, y: at.y, footprint: at.footprint };
      }
    }
    return widest;
  }, [view.places]);

  const camera = useCamera(extent, arrival);
  const visit = useVisit(camera.lookAtWorld, camera.camera);

  /** Travel to a Place. Shared by the dock, the crew list and the world itself. */
  const goTo = (place: PlaceView, why?: string) => {
    if (!place.placement) return;
    visit.visit({
      id: place.id,
      x: place.placement.x,
      y: place.placement.y,
      footprint: place.placement.footprint,
    });
    ping(why ?? "Travelling", place.title);
  };

  /**
   * Who is standing where.
   *
   * By **identity**, not by concept (ADR-0028). Presence used to carry a `PlaceConcept`, so
   * matching on `concept` was right — and it kept working after the Engine changed only
   * because the pack happens to key its Places by their concept name. The first building the
   * user creates has an id of its own and nobody would ever be found in it.
   */
  const occupants = (place: PlaceView): CharacterView[] =>
    view.characters.filter((c) => c.place === place.id);

  /**
   * A character's name, from their id.
   *
   * Falls back to the id rather than to a placeholder: an id is true and a little ugly, and
   * "Unknown" would be a face invented for somebody who has one.
   */
  const named = (id: string) =>
    view.characters.find((c) => c.id === id)?.name ?? id;

  const placed = view.places.filter((p) => p.placement);
  const visited = view.places.find((p) => p.id === visit.visiting) ?? null;
  const speaking = view.characters.find((c) => c.id === talkingTo) ?? null;
  const turn = useTurn(speaking?.id ?? null);

  /**
   * Which model actually answered, kept per character.
   *
   * Not scoped to a Quest, deliberately: it describes the *character's brain* on its last turn,
   * not the conversation — the card that shows it is in the crew list, which is the same card
   * whichever chat is open. A late event from another Quest is therefore not stale, it is the
   * most recent true reading.
   */
  useEffect(() => {
    let alive = true;
    let stop: (() => void) | null = null;
    void listen<{ characterId: string; model: string }>(
      "turn:thought",
      (event) => {
        if (!alive) return;
        setThoughtWith((known) => ({
          ...known,
          [event.payload.characterId]: event.payload.model,
        }));
      },
    ).then((off) => {
      if (!alive) {
        off();
        return;
      }
      stop = off;
    });
    return () => {
      alive = false;
      stop?.();
    };
  }, []);

  /**
   * Context is a Quest reading, so refresh it as one Engine projection. `turn:knew` deliberately
   * carries no Quest id: an external agent can finish after the user has opened another chat.
   * Re-reading here makes that late event harmless instead of copying its old number into this
   * Quest's cards.
   */
  useEffect(() => {
    let alive = true;
    const rereadContexts = () => {
      void fetchRememberedContexts().then((readings) => {
        if (!alive) return;
        setCrewContexts(
          Object.fromEntries(
            readings.map((reading) => [reading.characterId, reading]),
          ),
        );
      });
    };
    rereadContexts();
    let stopKnew: (() => void) | null = null;
    let stopCleared: (() => void) | null = null;
    void Promise.all([
      listen("turn:knew", rereadContexts),
      listen("turn:context-cleared", rereadContexts),
    ]).then(([knew, cleared]) => {
      if (!alive) {
        knew();
        cleared();
        return;
      }
      stopKnew = knew;
      stopCleared = cleared;
    });
    return () => {
      alive = false;
      stopKnew?.();
      stopCleared?.();
    };
  }, [turn.quest?.id]);

  /**
   * Every conversation in this World, open and ended.
   *
   * Re-read whenever the current one changes: a new one appears when NEW is pressed, a title
   * settles once the first thing is said, and closing one moves it from the dock to History. All
   * three are the same event as far as this list is concerned.
   */
  const [chats, setChats] = useState<readonly QuestSummary[]>([]);
  const [showMissions, setShowMissions] = useState(false);
  /**
   * FILES — what this World has made, as opposed to what it has worked on.
   *
   * Its own window rather than a tab of MISSIONS: those are two questions, and a Quest answers
   * both. State here because the top bar opens it and the World owns what is over it.
   */
  const [showFiles, setShowFiles] = useState(false);
  /**
   * This World's library, so the control that opens it knows whether there is one.
   *
   * Read from the Engine rather than remembered from the Launcher: a vault can be chosen,
   * changed or unplugged while a World is open, and a World holding its own copy would
   * eventually offer to open a folder that is no longer the library.
   */
  const [library, setLibrary] = useState<string | null>(null);
  /** The ones you can still continue. Everything else is History, and lives in MISSIONS. */
  const open = chats.filter((c) => c.open);

  const rereadChats = useCallback(() => {
    void fetchConversations().then(setChats);
  }, []);

  useEffect(rereadChats, [rereadChats, turn.quest?.id, turn.quest?.saidCount]);

  /**
   * Go to where somebody is, and speak to them. Conversations happen in places.
   *
   * **By identity, never by kind.** This matched `p.concept === who.place` — a leftover from
   * before Places belonged to the user (ADR-0028), and it worked only because the shipped pack
   * happens to name its buildings after their concepts. Somebody standing outside a building the
   * user built has no concept at all, so the camera simply did not move, and the conversation
   * opened over the wrong part of the World.
   *
   * Somebody on the road is visited at the **end** of it. They are on their way there, the
   * conversation will happen there, and following them down a road would be a camera chasing a
   * figure instead of showing a place.
   */
  const talkTo = (who: CharacterView) => {
    const wherever = who.journey?.to ?? who.place;
    const there = view.places.find((p) => p.id === wherever);
    if (there) goTo(there, `Visiting ${who.name}`);
    setTalkingTo(who.id);
  };

  /**
   * Open a conversation — the Chronicle, not just the selection.
   *
   * Picking one from Workflows or Missions made it *current* and left the screen unchanged, so
   * the act had no visible result and the way to actually read it was to click the character
   * afterwards. Choosing a conversation is asking to read it.
   *
   * It travels, because it goes through {@link talkTo}: conversations happen in places
   * (LIVING_WORLD_DESIGN_GUIDE), and a Chronicle opening over the wrong part of the World would
   * be the one gesture in here that teleports.
   *
   * If nobody in this World took part — a character the user has since moved elsewhere — the
   * Quest is still selected and no Chronicle opens. There is nobody to open it *with*, and
   * inventing a speaker would be worse than the empty result.
   */
  const openQuest = (chat: QuestSummary) => {
    void selectQuest(chat.id).then(() => {
      rereadChats();
      void turn.reread(chat.id);
      const who = view.characters.find((c) => chat.participants.includes(c.id));
      if (who) talkTo(who);
    });
  };

  /*
    THE EDITOR IS A SURFACE, NOT A PANEL.

    It was a panel inside the HUD, which made authoring a World feel like a settings dialog
    over it. Editing a World is a different activity from living in one — the World is stopped,
    nobody is working, and every gesture means something else — so it gets the whole screen and
    its own camera, and returns you here by starting the World again.

    Rendered instead of the World rather than over it: two live cameras over one projection
    would both be listening for the same pointer, and which one won would depend on z-order.
  */
  if (editing && view) {
    return (
      <WorldEditor
        worldName={view.packName ?? "this World"}
        places={view.places}
        characters={view.characters}
        map={view.map}
        minds={minds}
        onPlay={() => {
          void invoke("end_editing").then(reread);
          // Land may have been painted or cleared in there. Re-read it on the way out rather
          // than trusting a copy taken before the editor opened.
          void invoke<string | null>("world_land").then(setLand);
          setEditing(false);
        }}
        onBridge={() => {
          // Leaving for the bridge still starts the World again: a Workspace left frozen would
          // be frozen the next time somebody entered it, with nothing on screen saying why.
          void invoke("end_editing");
          setEditing(false);
          onLeave();
        }}
        onChanged={reread}
      />
    );
  }

  return (
    <main className="world">
      <div className="world__frame">
        {/* --------------------------------------------------------- backdrop */}
        {backdrop && (
          <img
            className="world__backdrop"
            src={backdrop}
            alt=""
            draggable={false}
            aria-hidden
          />
        )}
        {backdrop && <div className="world__scrim" aria-hidden />}

        {/* ------------------------------------------------------------ stage */}
        <div
          className="world__stage"
          ref={camera.containerRef}
          // The browser's context menu is a document affordance. There is no document here,
          // and "Save as / Print / Inspect" over a living world breaks presence instantly.
          onContextMenu={(event) => event.preventDefault()}
        >
          {view.map && camera.viewBox ? (
            <WorldMap
              map={view.map}
              places={view.places}
              characters={view.characters}
              travelled={travelled}
              viewBox={camera.viewBox}
              camera={camera.camera}
              surfaceRef={camera.surfaceRef}
              isDragging={camera.isDragging}
              isSettled={camera.isSettled}
              onPointerDown={camera.onPointerDown}
              onPointerMove={camera.onPointerMove}
              onPointerUp={camera.onPointerUp}
              land={land}
              layers={layers}
              visiting={visit.visiting}
              hovering={visit.hovering}
              onHover={visit.hover}
              // A pan that ends over a building is still a pan. Without this, moving the
              // camera past a Place would take the user somewhere they never asked to go.
              onVisit={(place) => {
                if (camera.wasDragged()) return;
                goTo(place, "Arriving at");
              }}
              // Click the person, speak to the person. The crew panel was the only way to reach
              // anybody, which is a list standing in for somebody you can already see.
              onTalkTo={(id) => {
                if (camera.wasDragged()) return;
                const person = view.characters.find((c) => c.id === id);
                if (person) talkTo(person);
              }}
              /*
                No `editing` prop here, ever. The running World is handed no way to be edited —
                not a set of handlers with a flag saying not to use them (ADR-0028). Editing
                happens in the World Editor, which is its own surface over its own camera.
              */
              onLeave={() => {
                if (!camera.wasDragged()) visit.leave();
              }}
            />
          ) : (
            // A world with no geography yet. Still the World — never a loading screen.
            <div className="world__unmapped" />
          )}
        </div>

        {/* -------------------------------------------------------------- hud */}
        <div className="hud">
          {/* TOP: who is at the console, what the ship reads, what you can do */}
          <Frame className="hud__bar" corner={10} fill="var(--ep-window)">
            <div className="hud__bar-body">
              <div className="hud__who">
                <Portrait
                  src={me?.portrait ?? null}
                  kind="player"
                  size={52}
                  alt=""
                />
                <div>
                  <p className="hud__who-name ep-shadow-text">
                    {me?.name ?? "ORCHESTRATOR"}
                  </p>
                  <p className="hud__who-role">ORCHESTRATOR</p>
                </div>
              </div>

              {/*
                One gauge per **account**, in the order the Engine lists them.

                It used to be exactly two, hardcoded — which was right while a program could only
                be signed into once. Now that `CLAUDE_CONFIG_DIR` and `CODEX_HOME` let one machine
                hold several sign-ins, drawing one reading twice would be a real measurement of the
                wrong quantity, and there would be no way to tell which account was running out.

                The title is the account's **name**: the program's own name for the sign-in that
                was already there, and the user's own label for one they added. Never invented —
                Claude Code will say which email is signed in and that is shown under it, while
                Codex has no way to be asked at all, so for that one the label is the whole name.
              */}
              <div className="hud__allowances">
                {agentStatuses
                  .filter((it) => it.installed && it.signedIn !== false)
                  .map((account) => {
                    const held = planUsage[account.id] ?? null;
                    if (account.kind === "codex") {
                      const one = held?.[0] ?? null;
                      return (
                        <PlanUsage
                          key={account.id}
                          title={account.name}
                          who={account.account}
                          tone="codex"
                          credits={one?.credits ?? null}
                          readings={[
                            {
                              label: planWindow(one?.windowMinutes ?? null),
                              // Codex has only one plan window. Its established gauge is a charge
                              // meter: the fill and reading mean what remains, unlike Claude's two
                              // `usage` windows which report what has been used.
                              percent: one?.remainingPercent ?? null,
                              hint: planHint(account.name, one),
                              direction: "remaining" as const,
                            },
                          ]}
                        />
                      );
                    }
                    if (account.kind !== "claude-code") return null;
                    return (
                      <PlanUsage
                        key={account.id}
                        title={account.name}
                        who={account.account}
                        tone="claude"
                        readings={[
                          planReading(held, 5 * 60, "5 hours", account.name, "used"),
                          planReading(
                            held,
                            7 * 24 * 60,
                            "weekly",
                            account.name,
                            "used",
                          ),
                        ]}
                      />
                    );
                  })}
              </div>

              <div className="hud__world-facts" aria-label="World readings">
                {/* Measured: what this World actually contains and whether its clock is running. */}
                <span>
                  <PixelIcon glyph="shield" size={14} tone="parchment" />{" "}
                  {view.characters.length} CREW
                </span>
                <span>
                  <PixelIcon glyph="core" size={14} tone="energy" />{" "}
                  {placed.length} PLACES
                </span>
                <span
                  className={
                    editing ? "hud__clock hud__clock--stopped" : "hud__clock"
                  }
                >
                  <i />
                  {editing ? "WORLD STOPPED" : "WORLD RUNNING"}
                </span>
              </div>

              <div className="hud__menu">
                {/* PENDING: each of these is its own subsystem, and none exists yet. */}
                {/*
                  MISSIONS — every conversation this World has had.

                  It was cold with "not built yet", which was true of a Missions *subsystem* and
                  had stopped being true of the thing people actually wanted behind that word:
                  the list of what you have been working on. Quests persist, ended ones included
                  (ADR-0025 §7), so the panel has something real behind it.
                */}
                {/*
                  A terminal is the user's, never a capability — nothing a model can reach opens
                  one. It sits here because the thing it exists for is somebody else's sign-in,
                  and having to leave the World to run one wizard is the moment the World stops
                  being where the work happens.
                */}
                <Terminals />
                {/*
                  FILES sits before MISSIONS because it answers the shorter question. *What did
                  we make* is looked up far more often than *what did we work on*, and the
                  answer to the first is a thing you can open.
                */}
                <button
                  type="button"
                  className="epbtn"
                  onClick={() => setShowFiles((open) => !open)}
                  aria-expanded={showFiles}
                  title="Everything this World has made"
                >
                  FILES
                </button>
                <button
                  type="button"
                  className="epbtn"
                  onClick={() => {
                    rereadChats();
                    setShowMissions((open) => !open);
                  }}
                  aria-expanded={showMissions}
                  title="Everything this World has worked on"
                >
                  MISSIONS
                </button>
                {/*
                  It said "Not built yet" and was a cold instrument for a knowledge panel that
                  does not exist. Something does exist now — this World's vault — and the
                  sentence had stopped being true, which is worse than a dark gauge.

                  It opens the notes where the user actually reads them rather than showing them
                  here. A markdown reader inside the World would be a worse Obsidian, and the
                  crew can already read the vault; this is for the person, not the crew.
                */}
                <button
                  type="button"
                  className="epbtn"
                  disabled={!library}
                  title={
                    library
                      ? `Open ${library} in Obsidian`
                      : "This World has no library — choose one on the bridge"
                  }
                  onClick={() => {
                    void openLibrary().then((opened) => {
                      if (typeof opened === "string") ping("LIBRARY", opened);
                      else if (!opened)
                        ping(
                          "OPENED THE FOLDER",
                          "Obsidian is not installed, or this is not a vault it knows.",
                        );
                    });
                  }}
                >
                  LIBRARY
                </button>
                <button
                  type="button"
                  className="epbtn"
                  disabled
                  title="Not built yet"
                >
                  HELP
                </button>
                <button
                  type="button"
                  className="epbtn"
                  onClick={() => setMuted(!muted)}
                  title={muted ? "Enable World sound" : "Mute World sound"}
                  aria-pressed={muted}
                >
                  {muted ? "SOUND OFF" : "SOUND ON"}
                </button>
                {/*
                  **A second switch, because there is a second question** (owner, 2026-09-04).

                  | question | where it is answered |
                  |---|---|
                  | how does this person sound? | the Character — it travels with them |
                  | how loud is the crew? | Settings |
                  | do I want to hear anyone *right now*? | here |

                  Here rather than in Settings because it is a decision about **this moment**:
                  somebody walks into the room, a call starts. Sending them to a settings screen
                  to shut a character up is how people mute everything and never turn it back on.

                  And separate from SOUND for the reason SOUND is separate from everything else —
                  that is the interface's own voice, and one switch deciding both would mean
                  silencing your own clicks to silence Mage.
                */}
                <button
                  type="button"
                  className="epbtn"
                  onClick={() => {
                    /*
                      **Turning it off stops the sentence in progress.** It did not: `voicesOn`
                      is read before a line is queued, so a character already talking talked on
                      to the end — the owner pressed it mid-answer and heard no difference.

                      *This moment* is what this control is for. A switch that takes effect
                      after the thing you pressed it about has finished is a switch about the
                      next thing, and it teaches people to press it and then reach for the
                      volume knob anyway.
                    */
                    if (speaks) hush();
                    setVoicesOn(!speaks);
                    setSpeaks(!speaks);
                  }}
                  title={
                    speaks
                      ? "Stop the crew speaking out loud"
                      : "Let the crew speak out loud again"
                  }
                  aria-pressed={!speaks}
                >
                  {speaks ? "NPC VOICES ON" : "NPC VOICES OFF"}
                </button>
                {/*
                  Entering stops the World, and stopping is refused while anybody is working —
                  with the refusal naming who and what (ADR-0028). The editor never interrupts
                  work; it waits for it, and says what it is waiting for.
                */}
                <button
                  type="button"
                  className="epbtn"
                  onClick={() => {
                    void invoke("begin_editing")
                      .then(() => setEditing(true))
                      .catch((error) =>
                        ping("Not yet", String(error).replace(/^Error: /, "")),
                      );
                  }}
                >
                  EDIT WORLD
                </button>
                {/*
                  The way back sits in the frame, not in the World. Leaving is administration,
                  and administration is the bridge's business.
                */}
                <button
                  type="button"
                  className="epbtn epbtn--primary"
                  onClick={onLeave}
                >
                  BRIDGE
                </button>
              </div>
            </div>
          </Frame>

          {/* MIDDLE: sidebars flank the World; the centre stays the World */}
          <div className="hud__middle">
            <aside
              className="hud__side hud__side--left ep-scroll"
              aria-label="Crew and work"
            >
              <Card title="Crew" scrolls>
                {view.characters.length === 0 ? (
                  <p className="hud__empty">
                    Nobody lives in this World. Assign somebody from the bridge.
                  </p>
                ) : (
                  view.characters.map((c) => (
                    <CrewCard
                      key={c.id}
                      glyph={ARCHETYPE_GLYPH[c.archetype] ?? "core"}
                      name={c.name}
                      // From the roster, not from `view.characters`: the World's projection is
                      // re-sent on every presence change and a brain is not presence. This is
                      // already in hand — it is what the dialogue header reads.
                      brain={minds[c.id] ?? null}
                      thoughtWith={thoughtWith[c.id] ?? null}
                      account={crewAccount(agents[c.id] ?? null, agentStatuses)}
                      // Measured: what she is genuinely doing. WORKING is only ever true when
                      // something is actually running (ADR-0018 causality).
                      // **Three states, because there are three.** Waiting is not idle — a job
                      // of theirs is in flight — and it is not working either: the picture is
                      // being made by ComfyUI, and a card claiming otherwise credits this
                      // character with somebody else's work.
                      activity={
                        c.class === "work"
                          ? "WORKING"
                          : c.class === "waiting"
                            ? "WAITING"
                            : "IDLE"
                      }
                      active={talkingTo === c.id}
                      allowance={crewAllowance(
                        agents[c.id] ?? null,
                        agentStatuses,
                        planUsage,
                      )}
                      context={crewContexts[c.id] ?? null}
                      onClick={() => talkTo(c)}
                    />
                  ))
                )}
              </Card>

              {/*
                THE AGENT DOOR — here, because the door is per World.

                It lived only on the bridge, where no World is entered: the project root, the
                capability registry, the Trust mode and the Quest all belong to a World, so a
                door opened there answered "no World is open" to everything while looking like
                it had worked. The Engine refuses that now; this is where the answer is yes.
              */}
              <Card title="Agents">
                <AgentDoorPanel door={agent} crew={view.characters} bare />
              </Card>

              {/*
                THE WORK THAT IS OPEN, and the way to put it down.

                This was cold, with a note saying Quests would arrive "once a Capability can
                actually run". They run. A panel still describing an unbuilt future while the
                thing it describes is happening two panels away is not a cold instrument, it is
                a stale one — and the two are told apart by whether anything is behind them.

                The ✕ is `set aside`, the same act as NEW in the conversation. Deliberately not
                called "close": the Quest is not deleted, it stops being the current one and
                stays in History (ADR-0025). Whatever is said next starts new work — and, for an
                agent, a new session on their side too.
              */}
              {/*
                THE CONVERSATIONS THAT ARE OPEN.

                Several at once, because that is how the work actually goes: one thing running
                while another waits. Clicking one looks at it — its Chronicle comes back, and so
                does the agent session that belongs to it, because a session is keyed to the
                Quest rather than to the person.

                The ✕ **ends** it. That is a different act from NEW, which only starts another
                one beside this. An ended conversation is not deleted — it moves to MISSIONS as
                what it was (ADR-0025 §7).
              */}
              <Card title="Workflows" dormant={open.length === 0} scrolls>
                {open.length === 0 ? (
                  <p className="hud__empty">
                    Nothing open. Say something to somebody and the work starts
                    here.
                  </p>
                ) : (
                  <ul className="hud__chats">
                    {open.map((chat) => (
                      <li key={chat.id}>
                        <button
                          type="button"
                          className={`hud__chat${chat.current ? " hud__chat--on" : ""}`}
                          onClick={() => openQuest(chat)}
                          title={chat.title}
                        >
                          <b>{chat.title}</b>
                          <i>
                            {chat.saidCount} said ·{" "}
                            {chat.producedEvidence ? "evidence" : "no evidence"}
                          </i>
                        </button>
                        <button
                          type="button"
                          className="hud__quest-shut"
                          onClick={() => {
                            void closeQuest(chat.id).then(({ note }) => {
                              // Ending a conversation is a real terminal state, unlike an
                              // ordinary turn that simply leaves the Quest open for more work.
                              playSfx("quest");
                              rereadChats();
                              void turn.reread();
                              // Said only when there is something to say. A Quest that produced
                              // nothing produced nothing, and announcing that would be noise
                              // (ADR-0025) — the silence is the honest half of this feature.
                              //
                              // `ping`, not `setToast`: the helper is what clears the notice
                              // again, and setting the state directly left it on screen for the
                              // rest of the session. And the note's *name*, not its path — the
                              // name is the Quest's title now, which is the part that tells
                              // somebody what was kept.
                              if (note) {
                                ping(
                                  "Kept in the library",
                                  note.split(/[\/]/).pop() ?? note,
                                  // A gain, not an alert: something the World now knows.
                                  "reward",
                                );
                              }
                            });
                          }}
                          title="End this conversation. It moves to MISSIONS."
                          aria-label={`Close ${chat.title}`}
                        >
                          ✕
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
              </Card>
            </aside>

            {/*
              History, when it is asked for.

              Over the World rather than beside it: you come to it looking for one thing among
              many, you read, and you leave. An instrument is something you glance at while
              working; this is not one.
            */}
            {showFiles && <Files onClose={() => setShowFiles(false)} />}

            {showMissions && (
              <Missions
                all={chats}
                onClose={() => setShowMissions(false)}
                onOpen={(quest) => {
                  const chat = chats.find((c) => c.id === quest);
                  if (!chat) return;
                  openQuest(chat);
                  // The window has done its job. It is a place you come to, read, and leave.
                  setShowMissions(false);
                }}
              />
            )}

            {/*
              The centre is the World. The only thing that ever sits over it is a conversation,
              because a conversation is the one thing that happens *here* rather than about here.
            */}
            <div className="hud__centre">
              {toast && !speaking && (
                <div className="hud__toast-slot">
                  <Toast
                    title={toast.title}
                    message={toast.message}
                    tone={toast.tone ?? "info"}
                  />
                </div>
              )}
            </div>

            <aside
              className="hud__side hud__side--right ep-scroll"
              style={{ width: `${sideWidth}px` }}
              aria-label="World instruments"
            >
              <Card title="System Map">
                {/*
                  The real minimap, derived from the World's geography — not the reference's
                  decorative 6×4 grid. It already existed and already worked; a fake grid beside
                  it would have been the only lie on this screen.
                */}
                {view.map && (
                  <div className="hud__map inset">
                    <Minimap
                      map={view.map}
                      places={view.places}
                      view={camera.view}
                      onLookAt={camera.lookAtWorld}
                      land={land}
                      characters={view.characters}
                      travelled={travelled}
                      inline
                    />
                  </div>
                )}
              </Card>

              <ConnectedModels providers={providers} agents={agentStatuses} />

              {/*
                TERMINAL — what the crew is running, as it runs.

                Every line is a real `turn:step`: a capability that was judged, allowed and
                started, then how it ended. Nothing is synthesised and nothing is predicted, so
                an empty Terminal means nothing is running — information, not a gap. That is
                also why it is not dormant-styled when quiet: the instrument is live, the World
                is simply idle, and those are different states.

                It answers a question the dialogue box cannot: the box shows one conversation,
                and work belongs to the World.
              */}
              <Card title="Terminal" grows>
                {/*
                  The frame is always here, including the grip.

                  It used to render a bare sentence until something ran, which meant the way to
                  make the panel resizable was to give a character work — a control that has to
                  be *earned* is a control nobody finds. An empty Terminal is a live instrument
                  reading nothing, and a live instrument keeps its dimensions.
                */}
                <>
                  <div
                    className="hud__term inset ep-scroll"
                    ref={terminalLog}
                    style={{ height: `${termHeight}px` }}
                  >
                    {terminal.length === 0 && (
                      <p className="hud__empty">
                        Nothing has run yet. Work appears here as it happens.
                      </p>
                    )}
                    {terminal.map((line) => (
                      <div
                        key={line.id}
                        className={`hud__term-line${line.ok === false ? " hud__term-line--bad" : ""}`}
                      >
                        <span className="hud__term-who">{named(line.who)}</span>
                        <span className="hud__term-what">{line.what}</span>
                        {line.ok === null && (
                          <span className="hud__term-run">running</span>
                        )}
                        {/*
                            Said, not tinted.

                            Failure was a colour on one word in a 10px font, and it did not
                            carry: four near-identical lines went past — two of them refused by
                            the sandbox, two of them the retry that worked — and they read as
                            four attempts at the same thing. The colour stays; the word is what
                            makes the pair legible as *blocked, then allowed*.

                            `failed` because that is the whole of what is known here: the run
                            ended badly. Why it did belongs to the line's own output, which is
                            right underneath and is the program's own account.
                          */}
                        {line.ok === false && (
                          <span className="hud__term-bad">failed</span>
                        )}
                        {/*
                            What the program actually printed, as it printed it. Shown for a run
                            that is still going *and* for one that has finished — hiding it on
                            completion would delete the only account of how something failed.
                          */}
                        {line.output.map((printed, at) => (
                          <span key={at} className="hud__term-out">
                            {printed}
                          </span>
                        ))}
                        {line.ok !== null && line.detail && (
                          <span className="hud__term-detail">
                            {line.detail}
                          </span>
                        )}
                      </div>
                    ))}
                  </div>
                  {/* The corner, draggable both ways. Output is unpredictable in size and in
                        line length; the panel should not be the thing deciding how much of it
                        you may see. */}
                  <div
                    className="hud__term-grip"
                    onPointerDown={grabTerminal}
                    role="separator"
                    aria-label="Resize the terminal"
                    aria-orientation="horizontal"
                  />
                </>
              </Card>
            </aside>

            {/*
              THE CONVERSATION — anchored to the window, not to the middle column.
  
              It lived inside `.hud__centre`, which is `flex: 1` — so widening the Terminal
              narrowed the centre, and the conversation narrowed with it. That is a panel
              resizing another panel by way of the layout, which no clamp could fix because
              nothing had been clamped: the box the user dragged was simply being squeezed.
  
              Here it sits over the whole band. Its size is the user's, its bounds are
              measured, and the Terminal stops at its edge rather than pushing it.
            */}
            {/*
              AN AGENT IS WAITING (step 5.2).

              Drawn here rather than inside a conversation, because an agent is not *in* one —
              it is working, in its own terminal, and the call it just made is held open on a
              socket until this is answered.

              The words and the buttons are the model's, deliberately. A second prompt that
              looked different would teach the user there are two permission systems, which is
              the exact thing the whole of 5.2 exists to prevent.
            */}
            {agent.question && (
              <div className="hud__agent-slot">
                <div className="dlg__await">
                  <p>
                    <b>
                      {agent.question.character.toUpperCase()} NEEDS YOUR WORD
                    </b>
                    {agent.question.what}
                  </p>
                  {agent.question.preview && (
                    <pre className="dlg__diff">{agent.question.preview}</pre>
                  )}
                  <p className="dlg__await-note">
                    {agent.question.standing
                      ? "An agent is holding, waiting for this. Nothing has been done."
                      : "For this turn only. Epoch will ask again next time."}
                  </p>
                  <div className="dlg__await-acts">
                    <button
                      type="button"
                      className="epbtn epbtn--primary"
                      onClick={() => void agent.answer(true, false)}
                    >
                      ALLOW ONCE
                    </button>
                    {/*
                      Offered only when it is real. A permission granted for one turn cannot
                      become permanent, so the button that would promise that is absent rather
                      than present and ignored.
                    */}
                    {agent.question.standing && (
                      <button
                        type="button"
                        className="epbtn"
                        title="And stop asking about this in this World"
                        onClick={() => void agent.answer(true, true)}
                      >
                        ALWAYS HERE
                      </button>
                    )}
                    <button
                      type="button"
                      className="epbtn"
                      onClick={() => void agent.answer(false, false)}
                    >
                      NO
                    </button>
                  </div>
                </div>
              </div>
            )}

            {speaking && (
              <div className="hud__dialogue-slot">
                <Dialogue
                  who={speaking}
                  model={minds[speaking.id] ?? null}
                  agent={agents[speaking.id] ?? null}
                  crew={view.characters}
                  turn={turn}
                  // Travel to where they are, then give them the turn. Both matter:
                  // conversations happen in places, and a colleague who is only *switched to*
                  // has not actually received anything.
                  onSpeakTo={(id) => {
                    const person = view.characters.find((c) => c.id === id);
                    if (!person) return;
                    talkTo(person);
                    void turn.handOver(id);
                  }}
                  onClose={() => setTalkingTo(null)}
                />
              </div>
            )}
          </div>

          <WorldDock
            places={placed}
            visitedId={visited?.id ?? null}
            peopleAt={(place) => occupants(place).length}
            glyphFor={(place) =>
              (place.concept && CONCEPT_GLYPH[place.concept]) || "core"
            }
            onVisit={(place) => goTo(place, "Travelling to")}
            layers={layers}
            onLayers={setLayers}
          />
        </div>

        {/* Honest about the connection, in the world's own corner. */}
        {status !== "live" && (
          <p className="world__quiet">
            {status === "unavailable"
              ? "The world is quiet — the engine is not answering."
              : "This world is not moving — changes cannot reach here."}
            {note ? ` (${note})` : ""}
          </p>
        )}
      </div>
    </main>
  );
}

/**
 * A plan allowance is not the same thing as the tokens an individual turn used.
 *
 * Both agents return allowances from their locally signed-in CLI, but by different contracts:
 * Claude's zero-turn `/usage` JSON result has two named windows; Codex's local experimental
 * app-server reports one. `PlanUsage` receives already-measured percentages and never turns a
 * turn's context tokens into a subscription percentage.
 */
function PlanUsage({
  title,
  who,
  tone,
  readings,
  credits,
}: {
  readonly title: string;
  /**
   * Which account this is, **as the agent itself reported it**.
   *
   * Claude Code answers `auth status --json` with the signed-in email. Codex has no way to be
   * asked at all — `login status` says only *"Logged in using ChatGPT"* — so this is `null` there
   * and the title, which is the user's own label, is the whole name. An address invented for the
   * one that cannot be asked would be the invented gauge the Launcher forbids.
   */
  readonly who?: string | null;
  readonly tone: "claude" | "codex";
  readonly readings: readonly {
    readonly label: string;
    readonly percent: number | null;
    readonly hint: string;
    readonly direction?: "used" | "remaining";
  }[];
  /**
   * What this agent says is left once its window is spent.
   *
   * Absent for an agent that keeps no balance, which is most of them: Claude Code reports
   * subscription percentages and nothing else. It is shown **only** when there is one, because
   * an empty purse and no purse are different sentences.
   */
  readonly credits?: AgentCredits | null;
}) {
  return (
    <section
      className={`hud__plan-usage hud__plan-usage--${tone}`}
      aria-label={title}
    >
      <p className="hud__plan-title">{title}</p>
      {who && <p className="hud__plan-who">{who}</p>}
      {readings.map((reading) => (
        <div className="hud__plan-row" key={reading.label} title={reading.hint}>
          <span>{reading.label}</span>
          <span
            className="hud__plan-track"
            aria-label={
              reading.percent === null
                ? "Unavailable"
                : `${reading.percent}% ${reading.direction ?? "remaining"}`
            }
          >
            {reading.percent !== null && (
              <i style={{ width: `${reading.percent}%` }} />
            )}
          </span>
          <b>{reading.percent === null ? "—" : `${reading.percent}%`}</b>
        </div>
      ))}
      {/*
        The balance, when there is one.

        A window at 0% was true and read as "this character is finished", while the agent went on
        working out of a balance the screen never mentioned. A reading that is accurate and leads
        to the wrong conclusion is still the wrong instrument.

        No bar: a balance has no ceiling to be a fraction of. It is a number, so it is shown as
        one — and the agent names no currency, so neither does Epoch.
      */}
      {credits && (
        <p
          className="hud__plan-credits"
          title={
            credits.unlimited
              ? `${title.replace(" usage", "")} reports credit with no limit.`
              : `${title.replace(" usage", "")} reports ${credits.balance} credits left. Work continues on these once the window above is spent.`
          }
        >
          <span>CREDITS</span>
          <b>{credits.unlimited ? "UNLIMITED" : credits.balance.toFixed(2)}</b>
        </p>
      )}
    </section>
  );
}

/**
 * The last allowances actually measured, kept across a mount.
 *
 * ## Why this is not React state
 *
 * `WorldScreen` unmounts every time somebody goes back to the bridge, so its state went with it
 * — and the probe that refills it runs a program, is deliberately kept off the first-render path
 * by 900 ms, and then takes as long as a CLI takes. Every trip back therefore painted a deck of
 * empty tracks reading `—`, for seconds, on gauges that had been measured a minute earlier and
 * were still true.
 *
 * Reported as *"sometimes the usages do not appear"*, with three screenshots of the same
 * instrument caught at three different moments: one with Claude Code blank, one with Codex
 * labelled `PLAN` and reading `—`, one complete. Nothing was wrong with any of them
 * individually — a blank gauge is the honest way to say *unasked* — and together they are a deck
 * nobody can trust, because the same row means two different things a second apart.
 *
 * **This is the same defect the Launcher already had**, in the other screen: a remount reseeding
 * from nothing, so a reading that had appeared disappeared on the way back from a World. The
 * rule it produced is that a snapshot is a fine **first paint** and never a source of truth —
 * which is exactly the job here. The interval below still re-measures, and whatever it finds
 * replaces this.
 *
 * Module scope rather than a store because it is one map with one writer, it must outlive a
 * component and nothing else reads it. It is deliberately **not** persisted to disk: an
 * allowance an hour old is worse than no allowance, and a process that has been restarted has
 * measured nothing yet.
 */
let measuredAllowances: Record<string, readonly AgentPlanUsage[] | null> = {};

/** What the Codex CLI called the rolling window, reduced to a compact bridge label. */
function planWindow(minutes: number | null): string {
  if (minutes === 5 * 60) return "5 hours";
  if (minutes === 7 * 24 * 60) return "weekly";
  if (minutes && minutes % 60 === 0) return `${minutes / 60}h`;
  return "plan";
}

/** The tooltip is evidence too: it says exactly where the live reading came from. */
function planHint(agent: string, usage: AgentPlanUsage | null): string {
  if (!usage) {
    return `${agent} plan usage could not be measured from its local CLI.`;
  }
  const reset = usage.resetsAt
    ? ` Resets ${new Date(usage.resetsAt * 1000).toLocaleString()}.`
    : "";
  return `${agent} reports ${usage.usedPercent}% used in its ${planWindow(usage.windowMinutes)} window.${reset}`;
}

/** Find one named Claude allowance; an incomplete answer must leave that gauge cold. */
function planReading(
  readings: readonly AgentPlanUsage[] | null,
  windowMinutes: number,
  label: string,
  agent: string,
  kind: "used" | "remaining" = "remaining",
) {
  const usage =
    readings?.find((reading) => reading.windowMinutes === windowMinutes) ??
    null;
  return {
    label,
    percent: usage
      ? kind === "used"
        ? usage.usedPercent
        : usage.remainingPercent
      : null,
    hint: planHint(agent, usage),
    direction: kind,
  };
}

/**
 * Which sign-in a character speaks with, in the words the row above uses — and only when this
 * machine has more than one of that program.
 *
 * ## Why it is here at all
 *
 * Two characters on two different Claude Code accounts drew identical cards: `haiku`, `haiku`.
 * The allowance meters were already keyed by account and correct, so the difference *was* on
 * screen — as two numbers with nothing naming whose they were. The only way to find out was to
 * ask the character, and a character's answer about its own sign-in is a self-report rather
 * than a measurement. It happened to be right, because an agent can read its own configuration;
 * a local model would have invented one.
 *
 * ## Why it disappears with one account
 *
 * Naming the only sign-in on the machine is an instrument that never moves. This exists for the
 * case where the cards were indistinguishable, and it should be absent everywhere else.
 *
 * Resolved through `agentStatuses` and compared on `kind`, never by taking the id apart: an id
 * identifies and a kind classifies, and a surface that reads `claude-code-2` as *the second
 * Claude Code* is a second place deciding what a program is.
 */
function crewAccount(
  account: string | null,
  accounts: readonly AgentStatus[],
): string | null {
  if (!account) return null;
  const it = accounts.find((one) => one.id === account);
  if (!it) return null;
  const siblings = accounts.filter((one) => one.kind === it.kind);
  return siblings.length > 1 ? it.name : null;
}

/**
 * The allowance card follows the agent assigned to that character, never a model-name guess.
 *
 * Keyed by **account**, not by program: a character assigned to a second Claude Code sign-in
 * reads that sign-in's window. Drawing the first account's reading for both would be a real
 * measurement of the wrong quantity — the most convincing way a gauge can lie.
 *
 * The account is resolved through `agentStatuses` because only the Engine knows which program
 * an account id belongs to; the id alone is `claude-code-2`, which this surface must never
 * take apart itself.
 */
function crewAllowance(
  account: string | null,
  accounts: readonly AgentStatus[],
  usage: Record<string, readonly AgentPlanUsage[] | null>,
): CrewAllowanceReading | null {
  if (!account) return null;
  const it = accounts.find((one) => one.id === account) ?? null;
  const held = usage[account] ?? null;
  if (it?.kind === "claude-code") {
    const reading = planReading(held, 5 * 60, "5 hours", it.name);
    return { remainingPercent: reading.percent, hint: reading.hint };
  }
  if (it?.kind === "codex") {
    const one = held?.[0] ?? null;
    return {
      remainingPercent: one?.remainingPercent ?? null,
      hint: planHint(it.name, one),
    };
  }
  // A Provider model has no subscription plan reported through this interface. No bar is more
  // honest than a full one: local work does not spend a plan allowance.
  return null;
}
