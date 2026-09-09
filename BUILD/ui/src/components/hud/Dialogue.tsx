/**
 * The dialogue box — talking to somebody who is standing in front of you.
 *
 * Deliberately in the World rather than in a panel on the bridge. You travel to where she is,
 * she is there, and you speak to her. That is the difference between a chat client with a
 * fantasy skin and a place where somebody works: the conversation happens *somewhere*
 * (LIVING_WORLD_DESIGN_GUIDE — "conversations happen in places").
 *
 * ## It shows her working before she says anything
 *
 * A cold local model takes minutes to load. During that time the honest thing to show is not a
 * spinner but the truth: she is working, and the World already renders work differently from
 * routine. The caret blinks because something is genuinely happening.
 *
 * ## What it refuses to hide
 *
 * A character with no model cannot think, and this says so with the fix rather than failing at
 * the moment you press enter. Nothing here fabricates an answer, a delay or a mood.
 *
 * ## How big it is, is the user's call
 *
 * A dialogue box sized for "hello" is the wrong box for a paragraph of instructions, and we
 * cannot know in advance which one this conversation is. So the box remembers a size the user
 * dragged, and the field they type into grows with what they are writing until it hits that
 * size. Both are *user intent* — the only thing the Experience Layer is allowed to store — and
 * everything else about the box stays derived.
 */

import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import type { Said } from "../../ipc/world";
import type { CSSProperties } from "react";

import { Frame, Plate } from "./Frame";
import { ApprovalPrompts } from "./ApprovalPrompts";
import { listen } from "@tauri-apps/api/event";

import { StudioPanel } from "../StudioPanel";
import { ChronicleList } from "./ChronicleList";
import { ContextGauge } from "./ContextGauge";
import { DialogueComposer } from "./DialogueComposer";
import { HandoverOffer } from "./HandoverOffer";
import { ModePicker } from "./ModePicker";
import { ReasoningDial } from "./ReasoningDial";
import { KeepWarm } from "./KeepWarm";
import { SetAsideButton } from "./SetAsideButton";
import { StandingDecisions } from "./StandingDecisions";
import { UnreachableBrain } from "./UnreachableBrain";
import { TurnFailureNotice } from "./TurnFailureNotice";
import { UndoNotice } from "./UndoNotice";
import { PixelIcon, Portrait } from "./Pixel";
import type { CharacterView } from "../../ipc/contracts";
import {
  chooseReasoning,
  fetchAutonomy,
  fetchReasoning,
  gateNote,
  handoverPreview,
  fetchWorkspace,
  fetchStanding,
  fetchUndoable,
  forgetStanding,
  openLink,
  setAutonomy as setAutonomyMode,
  undoLast,
} from "../../ipc/world";
import { alreadyHeard, say, stopSpeaking } from "../../experience/speak";
import type {
  AutonomyView,
  Dial,
  Standing,
  Undoable,
  Workspace,
} from "../../ipc/world";
import { fetchAgents, signInAgent } from "../../ipc/launcher";
import type { AgentStatus } from "../../ipc/launcher";
import type { Turn } from "../../experience/useTurn";

/**
 * Open an address through the Engine.
 *
 * At module scope rather than inline: passed as a prop it is compared by identity, and a new
 * function every render is enough to defeat memoisation downstream. It closes over nothing.
 */
const openThroughEngine = (url: string) => void openLink(url);

interface DialogueProps {
  /** The specialist being spoken to right now. */
  readonly who: CharacterView;
  /** What thinks for them — a model's name, or an agent's. `null` means they cannot think. */
  readonly model: string | null;
  /**
   * The signed-in agent thinking for them, when that is what thinks for them.
   *
   * Here for one reason today: an agent is handed an attached picture and a model is only told
   * one was shared, so the composer's note must be able to say either.
   */
  readonly agent: string | null;
  /** Everyone in the World, so a colleague's line can be shown under their own name. */
  readonly crew: readonly CharacterView[];
  readonly turn: Turn;
  /** Hand the conversation to somebody else. They speak for themselves; nobody speaks for them. */
  readonly onSpeakTo: (characterId: string) => void;
  readonly onClose: () => void;
}

/** How big the box is. Dragged by the user, remembered between sessions. */
interface Size {
  readonly w: number;
  /**
   * Height of the conversation log — the part that runs out of room first.
   *
   * **The box's total height is derived from this and then held constant**, which is the
   * point. It
   * used to be the log's height and nothing else, so every row that came and went below the
   * log — "Paladin was mentioned", an undo offer, a failure notice — changed the height of the
   * whole box. The user drags a shape they want to keep, and the box kept answering by
   * becoming a different shape whenever somebody spoke.
   *
   * Now the total is fixed and the log absorbs the difference. A notice appearing costs a
   * little scrollback, which is recoverable, instead of moving the window under the cursor,
   * which is not.
   */
  readonly h: number;
}

/**
 * Everything that is not the log: the header, the mode row, the composer.
 *
 * A single number rather than a measurement because it must not depend on what is currently
 * on screen — the whole bug was a height that answered to its own contents. The log is
 * `flex: 1` under it, so being a few pixels out costs a few pixels of scrollback and nothing
 * else; the box still does not move.
 */
const CHROME = 168;

const SIZE_KEY = "epoch.dialogue.size";
/**
 * The change the user has already put away.
 *
 * Kept outside the component for the same reason the window's size is: opening a different
 * conversation unmounts this box, and state that lives here dies with it. Dismissing the bar and
 * having it come back one chat later is not a notice being helpful, it is one that will not go
 * away — and it teaches people to stop dismissing things.
 *
 * The *summary*, never a flag: putting this change away must not hide the next one, and the next
 * change is a different sentence. `undo_last` is still the Engine's, untouched — this only says
 * what has been read.
 */
const UNDO_READ_KEY = "epoch.undo.read";

/** Which change the user has already put away, by id, or `null` if none. */
function alreadyRead(): string | null {
  try {
    return window.localStorage.getItem(UNDO_READ_KEY);
  } catch {
    // Storage refused. Showing the bar is the safe failure: the change is real either way.
    return null;
  }
}
const DEFAULT_SIZE: Size = { w: 680, h: 190 };
const MIN_SIZE: Size = { w: 420, h: 110 };

/**
 * How much room there actually is, measured.
 *
 * ## Why this is not a margin
 *
 * It was `innerWidth - 80`, a guess that was right until the side columns became resizable.
 * Then two panels were each sizing themselves against a constant, neither knew the other
 * existed, and they overlapped — the dialogue slid under the Terminal, and widening the
 * Terminal did nothing to stop it.
 *
 * A guessed margin is only ever correct for the layout it was written against. This measures
 * the columns that are really there, so the constraint stays true when they move, and when a
 * column is added that nobody has thought of yet.
 *
 * The box is anchored to the centre of the *window*, so its half-width is bounded by the
 * nearer column: symmetric, because moving the anchor to compensate would make the box drift
 * sideways as a panel is dragged, which is worse than being a little smaller.
 */
function bounds(): Size {
  const gap = 12;
  const middle = window.innerWidth / 2;

  let room = window.innerWidth / 2 - gap;
  for (const column of document.querySelectorAll(".hud__side")) {
    const box = column.getBoundingClientRect();
    if (box.width === 0) continue;
    // Distance from the centre of the window to the inner edge of this column.
    const reach = box.left > middle ? box.left - middle : middle - box.right;
    room = Math.min(room, reach - gap);
  }

  return {
    w: Math.max(MIN_SIZE.w, room * 2),
    // The *whole* box has to fit, so the chrome comes out of the room available to the log.
    h: Math.max(MIN_SIZE.h, window.innerHeight - CHROME - 132),
  };
}

const clampSize = (s: Size): Size => {
  const max = bounds();
  return {
    w: Math.min(Math.max(s.w, MIN_SIZE.w), max.w),
    h: Math.min(Math.max(s.h, MIN_SIZE.h), max.h),
  };
};

/**
 * The last size the user chose. A window smaller than the one they left behind is still a
 * legitimate answer — hence the clamp — but their intent survives it and comes back when the
 * window does.
 */
function rememberedSize(): Size {
  try {
    const raw = window.localStorage.getItem(SIZE_KEY);
    if (!raw) return DEFAULT_SIZE;
    const parsed = JSON.parse(raw) as Partial<Size>;
    if (typeof parsed?.w !== "number" || typeof parsed?.h !== "number")
      return DEFAULT_SIZE;
    return clampSize({ w: parsed.w, h: parsed.h });
  } catch {
    // A browser that will not give us storage is not a reason to fail to open the box.
    return DEFAULT_SIZE;
  }
}


/**
 * Whether every answered line in a Chronicle can be attributed to somebody.
 *
 * **Its own function so a test can hold it.** The whole defect was an inline `= true` that ran
 * before the crew had arrived, and nothing in a `useEffect` is visible to an assertion — the
 * same reason `spoken_language` was pulled out of a `Command` builder one crate over.
 *
 * `who === null` is a line with no speaker at all, and a character who has left the World is not
 * coming back before the next render. Neither is a reason to keep waiting.
 */
export function everyoneIsKnown(
  said: readonly Said[],
  crew: readonly CharacterView[],
): boolean {
  return said.every(
    (entry) =>
      entry.kind !== "answered" ||
      entry.who === null ||
      crew.some((who) => who.id === entry.who),
  );
}

export function Dialogue({
  who,
  model,
  agent,
  crew,
  turn,
  onSpeakTo,
  onClose,
}: DialogueProps) {
  // Closed by default. Opened on request, and never left holding a reading from work that has
  // since been put down — a panel describing a Quest that no longer exists is worse than none.
  const [size, setSize] = useState<Size>(rememberedSize);
  /** The title being typed. `null` while not renaming — user intent only. */
  const [renaming, setRenaming] = useState<string | null>(null);
  const log = useRef<HTMLDivElement>(null);
  /** The corner you resize by — measured, because it is the thing that must stay reachable. */
  const grip = useRef<HTMLDivElement>(null);
  /** The size at the instant a drag starts — a drag measures from where it began, not from where the last frame left off. */
  const sized = useRef(size);
  sized.current = size;

  // Follow the conversation as it grows, including while she is still writing.
  useEffect(() => {
    const el = log.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [turn.said, turn.writing]);

  useEffect(() => {
    try {
      window.localStorage.setItem(SIZE_KEY, JSON.stringify(size));
    } catch {
      // Remembering is a courtesy; not remembering must never break the conversation.
    }
  }, [size]);

  // A box the user sized for a maximised window must not hang off the edge of a small one.
  useEffect(() => {
    const onResize = () => setSize((s) => clampSize(s));
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  /*
    The grip must stay reachable, and that is measured rather than guessed.

    `bounds()` caps the *log* at `innerHeight - 300`, which assumes the rest of the box is 300px
    tall. It is not: an approval block, a capability request, a standing-decisions row and the
    mode row each add to it. Dragged tall with those on screen, the top-right corner climbed
    behind the HUD bar — and since the grip is the only way to shrink, the box could be grown
    into a state with no way out.

    So the invariant is stated as itself: whatever else the box does, the corner you resize it
    by stays below the bar. Measured from the two elements involved, after layout, so it holds
    for chrome that does not exist yet.
  */
  useLayoutEffect(() => {
    const handle = grip.current;
    if (!handle) return;
    const bar =
      document.querySelector(".hud__bar")?.getBoundingClientRect().bottom ?? 0;
    const limit = bar + 8;
    const over = limit - handle.getBoundingClientRect().top;

    // Height only. Width is *not* re-clamped here on purpose.
    //
    // The first version did clamp it, and widening the Terminal made the conversation shrink.
    // That is a panel resizing another panel, which is not what a boundary is: a box the user
    // sized should stay the size they chose. **Whoever is being dragged is the one that
    // stops** — so the Terminal measures this box and refuses to grow past it, exactly as this
    // box measures the HUD bar and refuses to grow past that.
    //
    // Only ever shrinks, and only past a whole pixel: a clamp that could grow would fight the
    // drag, and one without a threshold would re-render on rounding.
    if (over > 1)
      setSize((current) => clampSize({ w: current.w, h: current.h - over }));
  });

  /*
    Resizing. The box is anchored bottom-centre, so the grip pulls the top-right corner: up is
    taller, and out is wider on both sides at once because the centre does not move.
  */
  const grab = useCallback((e: React.PointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
    const handle = e.currentTarget;
    handle.setPointerCapture(e.pointerId);
    const from = { x: e.clientX, y: e.clientY };
    const start = sized.current;

    const move = (m: PointerEvent) => {
      setSize(
        clampSize({
          w: start.w + (m.clientX - from.x) * 2,
          h: start.h - (m.clientY - from.y),
        }),
      );
    };
    const drop = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", drop);
      handle.removeEventListener("pointercancel", drop);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", drop);
    handle.addEventListener("pointercancel", drop);
  }, []);

  // Escape closes the box. It must not also leave the Place — one key, one meaning.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);

  /*
    How much the crew may do on its own.

    Read from the Engine on open and after every change rather than held as local state: it is
    a stored decision about this World, and a surface that kept its own copy could show one
    mode while the gate enforced another. That disagreement would be invisible until something
    ran that the user thought could not.
  */
  const [autonomy, setAutonomy] = useState<AutonomyView | null>(null);
  /*
    A Quest can begin life without an id.  In that short interval the initial read and a newly
    chosen mode race across IPC.  Keep a monotonic revision so an older read can never repaint
    the selector after the user has made a newer choice.
  */
  const autonomyRevision = useRef(0);

  useEffect(() => {
    let alive = true;
    const revision = autonomyRevision.current;
    void fetchAutonomy().then((found) => {
      if (alive && revision === autonomyRevision.current) setAutonomy(found);
    });
    return () => {
      alive = false;
    };
  }, [turn.quest?.id]);

  const pickMode = async (mode: string) => {
    /*
      A fresh Quest has no id yet, but its selected mode is still real Engine state: it lives in
      the World’s prepared-session settings until the first turn inaugurates the Quest. Reflect
      that choice immediately, then re-read the Engine as the authority. Waiting for the IPC
      round trip made the select snap back in an empty conversation, which looked like Mode was
      unavailable precisely when a person needs to choose it.
    */
    const revision = ++autonomyRevision.current;
    const before = autonomy;
    setAutonomy((current) =>
      current ? { ...current, current: mode } : current,
    );

    const failed = await setAutonomyMode(mode);
    const confirmed = await fetchAutonomy();
    // A later pick won while this request was in flight.
    if (revision !== autonomyRevision.current) return;
    if (confirmed) {
      setAutonomy(confirmed);
    } else if (failed) {
      // An unavailable Engine must not leave an optimistic value pretending it was saved.
      setAutonomy(before);
    }
  };

  /*
    What could be put back.

    Re-read whenever the conversation changes rather than tracked here: the journal is a file
    the Engine writes, and a surface holding its own count would eventually offer an undo for
    something already undone.
  */
  const [undoable, setUndoable] = useState<Undoable | null>(null);
  const [undoFailed, setUndoFailed] = useState<string | null>(null);
  /**
   * A change the user has put away — read back from storage, so it survives this box closing.
   *
   * The summary rather than a boolean: dismissing *this* change must not hide the *next* one, and
   * a flag would have done exactly that — one ✕ and the bar never returns, including for work
   * somebody would want to undo.
   */
  const [undoHidden, setUndoHidden] = useState<string | null>(alreadyRead);

  /**
   * The agent that is installed but signed out, when the failure looks like that.
   *
   * Asked only when a failure mentions it, because a probe starts real processes: no
   * conversation should pay for that until something has actually gone wrong. And asked at all
   * because the alternative is repeating the agent's own sentence back as if Epoch had checked
   * — which is the shape of lie the World is not allowed to tell.
   */
  const [signedOut, setSignedOut] = useState<AgentStatus | null>(null);

  /**
   * How hard this character thinks, and how hard they *can* be asked to.
   *
   * Fetched per character rather than held once, because the ladder belongs to their brain: a
   * local model's rungs and Claude Code's are different lengths and do not hold the same levels.
   */
  const [dial, setDial] = useState<Dial | null>(null);
  /**
   * Whether the Studio Panel is open in this conversation (ADR-0033).
   *
   * **Opened by the character, never by a button.** Asked for a picture without being told how it
   * should look, a character calls `open_studio` and the panel is its reply — the way a person
   * would say *sure, tell me how you want it*. A button would be one more thing to know about,
   * and it would put the panel somewhere other than in the answer.
   *
   * Transient on purpose: this is what the window is showing, not what happened. What the panel
   * makes becomes evidence on the Quest, and that is the part that is written down.
   */
  const [studio, setStudio] = useState(false);
  /**
   * Where the last picture from the panel landed, when the World has a Project Root.
   *
   * Beside the conversation and never in it: the Chronicle is what a model reads, and a path in
   * there is the one thing a model needs to claim a picture nobody made. Cleared the moment
   * anything else happens, because it is about the picture that was just made and nothing else.
   */
  const [drewAt, setDrewAt] = useState<string | null>(null);

  useEffect(() => {
    const stop = listen("studio:open", () => setStudio(true));
    return () => {
      void stop.then((off) => off());
    };
  }, []);
  /**
   * What this agent's gate actually does under the mode that is selected.
   *
   * Asked of the Engine whenever either changes. Claude Code asks before each tool; Codex starts
   * read-only and Epoch asks only after the sandbox reports a blocked change. Neither description
   * may be shown for the other agent.
   */
  const [gate, setGate] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    void fetchReasoning(who.id).then((found) => alive && setDial(found));
    return () => {
      alive = false;
    };
  }, [who.id, turn.quest?.id]);

  useEffect(() => {
    const agent = dial?.agentId;
    const mode = autonomy?.current;
    if (!agent || !mode) {
      setGate(null);
      return;
    }
    let alive = true;
    void gateNote(agent, mode).then((note) => alive && setGate(note));
    return () => {
      alive = false;
    };
  }, [dial?.agentId, autonomy?.current]);

  useEffect(() => {
    const failed = turn.failed;
    if (!failed || !/not logged in|\/login|signed out/i.test(failed)) {
      setSignedOut(null);
      return;
    }
    let alive = true;
    void fetchAgents().then((found) => {
      if (alive) {
        setSignedOut(
          found.find((a) => a.installed && a.signedIn === false) ?? null,
        );
      }
    });
    return () => {
      alive = false;
    };
  }, [turn.failed]);

  /**
   * The crew, out loud (Phase 15).
   *
   * **`answered` and nothing else.** The Chronicle also holds what was approved and what was
   * produced (ADR-0025), and reading a file listing aloud is not somebody talking to you.
   *
   * Whose voice comes from `crew`, by the id on the entry, and not from `who` — the person
   * whose window this is may not be the one who last spoke: a Quest hands over, and a colleague
   * answering in somebody else's voice would be worse than silence.
   *
   * `say` is silent when nobody chose a voice, refuses to repeat itself, and keeps the answer on
   * screen whatever the engine does. Nothing here needs to check any of that, which is the point
   * of it living one layer down.
   */
  /**
   * Whether anything in this Chronicle has been considered yet.
   *
   * **What is already on screen when you walk in has already happened.** Without this, opening a
   * conversation read the whole backlog aloud — measured in the window: seven answers, one after
   * another, from a transcript nobody had asked to hear again.
   */
  const walkedIn = useRef(false);

  useEffect(() => {
    for (const entry of turn.said) {
      if (entry.kind !== "answered") continue;
      const speaker = entry.who === null ? null : crew.find((c) => c.id === entry.who);
      if (walkedIn.current) {
        say(speaker?.speaksWith, entry.content, speaker?.soundsLike);
      } else {
        // Remembered as heard rather than skipped by counting: a Chronicle is re-read every
        // turn, and an index is only right while nothing is ever inserted or compacted.
        alreadyHeard(speaker?.speaksWith, entry.content);
      }
    }
    /*
      **Not marked until it is known who said each line.**

      `alreadyHeard` returns at once when there is no voice, and there is no voice while `crew`
      is still empty — it arrives from the World a moment after the Chronicle does. So the first
      pass marked *nothing*, set this flag anyway, and the second pass — with the crew present —
      read the entire backlog aloud. The exact defect this ref exists to prevent, arriving
      through the one door it was not watching.

      An entry nobody can be found for is not a reason to keep waiting: `who === null` is a line
      with no speaker at all, and a character who has left the World is not coming back before
      the next render.
    */
    walkedIn.current = walkedIn.current || everyoneIsKnown(turn.said, crew);
  }, [turn.said, crew]);

  /* Leaving a conversation stops it: a character finishing a sentence into a closed window is
     the World talking to nobody. */
  useEffect(() => stopSpeaking, []);

  useEffect(() => {
    void fetchUndoable().then(setUndoable);
  }, [turn.said, turn.thinking]);

  /*
    Standing decisions, read back so they can be taken away.

    "Always allow" is the decision most worth being able to change your mind about, and until
    now it was the only one with no way back. Shown here rather than in a settings screen for
    the same reason the mode is: this is where it was granted.
  */
  const [standing, setStanding] = useState<readonly Standing[]>([]);
  /**
   * Where this World works. Read once — the Project Root is chosen in the Launcher, so it does
   * not change while somebody is standing in a conversation.
   */
  const [workspace, setWorkspace] = useState<Workspace | null>(null);

  useEffect(() => {
    void fetchWorkspace().then(setWorkspace);
  }, []);

  useEffect(() => {
    void fetchStanding().then(setStanding);
  }, [turn.said, turn.thinking]);

  const revoke = async (decision: Standing) => {
    await forgetStanding(decision);
    setStanding(await fetchStanding());
  };

  const takeItBack = async () => {
    const { failed } = await undoLast();
    setUndoFailed(failed);
    setUndoable(await fetchUndoable());
  };

  return (
    <Frame
      className="dlg"
      corner={10}
      studs
      fill="var(--ep-window)"
      // `--dlg-chrome` is passed rather than hardcoded in CSS so the number the layout uses and
      // the number `bounds()` reserves cannot drift apart into a box that will not fit.
      style={
        {
          "--dlg-w": `${size.w}px`,
          "--dlg-log": `${size.h}px`,
          "--dlg-chrome": `${CHROME}px`,
        } as CSSProperties
      }
    >
      {/*
        The grip. Deliberately outside the frame's own corner stud so a drag can never be
        mistaken for a click on the window, and it takes pointer events alone — the rest of the
        box keeps working exactly as it did.
      */}
      <div
        ref={grip}
        className="dlg__grip"
        onPointerDown={grab}
        role="separator"
        aria-label="Resize the conversation"
        aria-orientation="horizontal"
      />

      <div className="dlg__body">
        <div className="dlg__head">
          <Portrait size={44} kind="npc">
            {/*
              Their icon — the image whose job is to identify somebody, which is exactly this
              job. Falls back to the world sprite in the Engine, so one drawing still makes
              them recognisable; and it is still their own artwork through the same pipeline,
              never a second portrait renderer (ADR-0023).
            */}
            {who.icon?.asset ? (
              <img src={who.icon.asset} alt="" draggable={false} />
            ) : undefined}
          </Portrait>
          <div className="dlg__who">
            {/*
              The Quest is the heading, not the character. You are not in a conversation with
              Mage — you are working on something, and right now you are speaking to Mage about
              it (ADR-0025).
            */}
            {renaming !== null ? (
              <form
                className="dlg__rename"
                onSubmit={(e) => {
                  e.preventDefault();
                  void turn.rename(renaming);
                  setRenaming(null);
                }}
              >
                <input
                  value={renaming}
                  autoFocus
                  aria-label="Quest name"
                  onChange={(e) => setRenaming(e.target.value)}
                  onKeyDown={(e) => {
                    e.stopPropagation();
                    if (e.key === "Escape") setRenaming(null);
                  }}
                />
                <button type="submit" className="epbtn">
                  OK
                </button>
              </form>
            ) : (
              <button
                type="button"
                className="dlg__title"
                // Never disabled. A new conversation can be named before it starts — the name is
                // held and applied the moment the first thing said creates the Quest.
                title={
                  turn.quest
                    ? `${turn.quest.title} — rename`
                    : "Name this conversation before it starts"
                }
                onClick={() =>
                  setRenaming(turn.quest?.title ?? turn.planned ?? "")
                }
              >
                <Plate>
                  {turn.quest?.title ?? turn.planned ?? "New Quest"}
                </Plate>
              </button>
            )}
            <p className="dlg__state">
              {turn.thinking ? (
                <>
                  <PixelIcon glyph="core" size={12} tone="energy" /> {who.name}{" "}
                  · {who.activity}
                </>
              ) : turn.quest ? (
                <>
                  {turn.quest.state.replace(/_/g, " ").toUpperCase()} ·{" "}
                  {turn.quest.participants.length || 1}{" "}
                  {turn.quest.participants.length === 1
                    ? "CONTRIBUTOR"
                    : "CONTRIBUTORS"}
                  {/* Talking is not evidence. Saying so keeps History from being a story. */}
                  {turn.quest.producedEvidence
                    ? " · HAS EVIDENCE"
                    : " · NO EVIDENCE YET"}
                </>
              ) : model ? (
                <>
                  SPEAKING TO {who.name.toUpperCase()} · {model.toUpperCase()}
                </>
              ) : (
                <>NO BRAIN ASSIGNED</>
              )}
            </p>
          </div>

          {/*
            The lamp over the conversation: is her brain on the card right now? Only ever drawn
            for a brain there is something to hold — an agent's and a hosted model's are
            somebody else's memory, and the control does not appear.
          */}
          <KeepWarm characterId={who.id} settled={turn.thinking} />
          <SetAsideButton
            hasQuest={turn.quest !== null}
            onSetAside={() => {
              void turn.setAside();
            }}
          />
          <button
            type="button"
            className="epbtn dlg__close"
            onClick={onClose}
            aria-label="Close"
          >
            ✕
          </button>
        </div>

        {/*
          The World saying it is mute.

          Without a Project Root the crew has no tools at all, and a character with no tools
          answers "I cannot do that" perfectly truthfully. That reads like a broken product,
          and the only place that said otherwise was a screen the user had already left.
        */}
        {workspace && !workspace.projectRoot && (
          <p className="dlg__mute">
            This World has no project folder, so {who.name} has no tools here —
            nothing to read, write or run. Choose one in the Launcher and the
            crew can work.
          </p>
        )}

        <div className="dlg__log inset ep-scroll" ref={log}>
          <ChronicleList
            who={who}
            model={model}
            crew={crew}
            said={turn.said}
            writing={turn.writing}
            speaking={
              // Whoever is actually answering, which is not always who is selected.
              turn.speaking
                ? (crew.find((one) => one.id === turn.speaking)?.name ?? null)
                : null
            }
            thinking={turn.thinking}
            compacting={turn.compacting}
            working={turn.working}
            // **Stable on purpose.** A fresh arrow here changed a prop on every streamed
            // token, so the settled Chronicle re-reconciled 20,000 nodes for a callback none of
            // its lines had called — 138 ms per token with 5,000 lines standing. It captures
            // nothing, so there is nothing for it to close over stalely.
            onOpenLink={openThroughEngine}
          />

          <ApprovalPrompts
            who={who}
            wanted={turn.wanted}
            awaiting={turn.awaiting}
            onAnswerWanted={turn.answerWanted}
            onAnswer={turn.answer}
          />

          {/*
            SOURCES — where the answer came from, folded away until asked for.

            Folded because reading is the point and a wall of links between the answer and the
            next question is a tax on every turn that did not need checking. Present because an
            answer assembled from pages nobody can name is indistinguishable from one that was
            made up.

            These arrive as data from the Engine (`turn:ended`), never recovered from the text.
            A surface that scraped URLs out of prose would sooner or later cite something the
            character merely mentioned — which is worse than no citation at all.
          */}
          {turn.sources.length > 0 && (
            <details className="dlg__sources">
              <summary>
                {turn.sources.length}{" "}
                {turn.sources.length === 1 ? "source" : "sources"}
              </summary>
              <ol>
                {turn.sources.map((source) => (
                  <li key={source.url}>
                    {/*
                      Opens in the user's own browser rather than inside the World: a page is
                      not a place in Epoch, and rendering somebody else's site inside the frame
                      would put untrusted markup where the World lives.

                      A button rather than an anchor. There is nowhere for an `href` to go in a
                      webview, and the Engine has to see the address before anything starts —
                      it refuses a scheme that is not http or https, because these addresses
                      were chosen by a search engine out of pages written by strangers.
                    */}
                    <button
                      type="button"
                      className="dlg__source"
                      title={`Open ${source.url}`}
                      onClick={() => void openLink(source.url)}
                    >
                      <span className="dlg__source-title">{source.title}</span>
                      <span className="dlg__source-url">{source.url}</span>
                    </button>
                  </li>
                ))}
              </ol>
            </details>
          )}

          {(turn.unfinished || turn.halted) && (
            /*
              Dismissable, because it is a note about an ending rather than a fault. Everything
              it refers to is already in the Chronicle above it — the notice only says *why* the
              turn stopped, and once that has been read it is in the way.
            */
            <p className="dlg__failed dlg__ending">
              {turn.halted
                ? "You stopped this. What was found is kept — say “continue” to carry on."
                : `Stopped after ${turn.rounds} rounds of tools — that is the limit for one turn. What was found is kept; say “continue” to carry on.`}
              <button
                type="button"
                className="dlg__dismiss"
                onClick={turn.clearEnding}
                title="Put this away"
                aria-label="Dismiss"
              >
                ✕
              </button>
            </p>
          )}

          {undoFailed && <p className="dlg__failed">{undoFailed}</p>}

          {/*
            A Service that did not answer is a question, not a notice: the assigned Brain is
            part of who somebody is (ADR-0026), so Epoch says which machine and lets the user
            decide. Above the plain notice because it is the actionable half of the same fault.
          */}
          <UnreachableBrain
            failure={turn.failed}
            who={who}
            onRetry={() => {
              turn.clearFailed();
              void turn.handOver(who.id);
            }}
            onChanged={() => {
              turn.clearFailed();
              void turn.handOver(who.id);
            }}
            onStop={turn.clearFailed}
          />

          <TurnFailureNotice
            failure={turn.failed}
            signedOut={signedOut}
            onSignIn={(agentId) => {
              void signInAgent(agentId);
            }}
            onDismiss={turn.clearFailed}
          />

          {/*
            **Where the last picture landed.** Beside the conversation and never inside it: the
            Chronicle is what a model reads, and a path in there is the one thing a model needs
            to claim a picture nobody made (ADR-0030's third amendment).

            Dismissable, because it is about one picture and nothing else.
          */}
          {drewAt && (
            <p className="notice">
              Saved in {drewAt}{" "}
              <button
                type="button"
                className="btn btn--mini"
                onClick={() => setDrewAt(null)}
              >
                OK
              </button>
            </p>
          )}

          {/*
            **The Studio Panel lives here, not in the Launcher** (ADR-0033). The Launcher prepares —
            what is installed, what draws where; the World is where somebody actually asks for a
            picture, and the panel belongs at the moment of asking.

            Inside the Chronicle rather than floating over the World: what it makes is evidence on
            the Quest, and a picture should appear where it was asked for.

            **And inside the conversation itself, not beneath it.** It used to sit below the
            standing decisions and the mode picker, between the Chronicle and the composer —
            which put a thing a character handed you outside the record of them handing it to
            you. It is the last thing in the log now, so it arrives where their sentence arrived
            and scrolls with everything else that happened.
          */}
          {studio && (
            /*
              **A window, not a slab.** It was a translucent block with one hairline pinned above
              the composer, which read as the conversation having grown a form. What a character
              opens is a *thing in the World*, so it is drawn with the same `Frame` every other
              window here uses — which also means a World Pack re-skins it, and a panel that drew
              its own chrome would be the one window a Pack could not touch.
            */
            <Frame
              corner={10}
              fill="var(--ep-window-2)"
              className="dlg__studio"
              role="dialog"
              aria-label="Picture panel"
            >
              <div className="dlg__studioBar">
                <Plate>{who.name} opened the creations panel</Plate>
                <button
                  type="button"
                  className="btn btn--mini"
                  onClick={() => setStudio(false)}
                  aria-label="Close the creations panel"
                >
                  CLOSE
                </button>
              </div>
              <div className="dlg__studioBody ep-scroll">
                {/*
                  **The panel draws, and nothing is said afterwards.**

                  It used to speak the prompt, so the picture would come back as the character's
                  reply and land on the Quest as evidence. Measured 2026-08-25, both halves went
                  wrong. The character did not draw — handed *"a lighthouse at night"* with the
                  panel already open, `gemma4:12b` called `open_studio` again. And once the Engine
                  drew directly, that same message made it draw the picture **a second time**: one
                  render from the button and one from a model reading the prompt as a fresh
                  request.

                  The evidence half is kept and moved rather than dropped: the Engine files the
                  picture on the active Quest in the same `Produced` entry a capability's run
                  makes, so History sees it (ADR-0025) and the Chronicle draws the real bytes from
                  it. Nothing is asked of a model, which costs nothing and cannot misread
                  anything. Whoever wants to talk about the picture can — it is right there in the
                  conversation.
                */}
                <StudioPanel
                  onDrew={(at) => {
                    // Where it landed, said beside the conversation and never inside it.
                    setDrewAt(at);
                  }}
                  onChosen={() => {
                    setStudio(false);
                    // The Chronicle refreshes on turn events, and this deliberately runs no
                    // turn — so the evidence the Engine just filed would sit there unseen until
                    // the next thing somebody said. One re-read, caused by the press.
                    void turn.reread();
                  }}
                />
              </div>
            </Frame>
          )}
        </div>

        {/*
          Somebody was named, and they have not answered.

          Offered rather than done, and read before it travels — both of which are the offer's
          own business (`HandoverOffer`). This box only says who was named and what to do about
          the answer.
        */}
        <HandoverOffer
          invited={turn.thinking ? [] : turn.invited}
          crew={crew}
          preview={handoverPreview}
          onHandOver={(id) => {
            turn.declineInvite();
            onSpeakTo(id);
          }}
          onDecline={turn.declineInvite}
        />

        {/*
          How much the crew may do on its own, at the bottom of the conversation because that
          is where the decision is made — not buried in a settings screen you would have to
          leave the World to reach.

          The list comes from the Engine (`get_autonomy`). A hardcoded copy would drift the day
          a mode is added, and drift in the permission system would look like a bug in the one
          place nobody should be guessing.
        */}
        {/*
          Put the last change back.

          Shown only when there is something to put back, and it is the reason the file
          capabilities may declare themselves undoable at all — an undo nobody can reach is not
          a reversal, it is a claim.

          **Said as the World's, because that is what it is.** The Journal records changes to the
          project, not to a conversation, and this bar sat inside the Chronicle reading as though
          this conversation had made them: `created pepepe.md` appeared under a conversation about
          somebody's name, because another character had written that file in another chat.

          The behaviour is right and only the sentence was wrong. Scoping the undo to this
          conversation would be worse — undoing "the last change here" while a newer change exists
          elsewhere is exactly the silent discard `Journal::undo_last` refuses to perform.
        */}
        <UndoNotice
          change={undoable}
          hidden={undoHidden}
          onUndo={takeItBack}
          onDismiss={(id) => {
            setUndoHidden(id);
            try {
              window.localStorage.setItem(UNDO_READ_KEY, id);
            } catch {
              // Remembering is a courtesy. Not remembering brings this notice back, which is safe.
            }
          }}
        />

        <StandingDecisions decisions={standing} onRevoke={revoke} />

        {autonomy && (
          <ModePicker
            autonomy={autonomy}
            brain={dial?.agent ? dial.brain : null}
            gate={gate}
            onPick={(mode) => void pickMode(mode)}
          />
        )}

        <div className="dlg__ask">
          <ContextGauge knew={turn.knew} who={who.name}>
            {(contextGauge) => (
              <DialogueComposer
                characterId={who.id}
                conversationId={turn.quest?.id ?? null}
                name={who.name}
                model={model}
                thinking={turn.thinking}
                logHeight={size.h}
                history={turn.said.flatMap((entry) =>
                  entry.who === null &&
                  entry.kind === "said" &&
                  entry.content.trim()
                    ? [entry.content]
                    : [],
                )}
                // The function itself, never a lambda re-listing its arguments. One that listed two
                // of three dropped every image on the floor: the composer held them, the Engine
                // never saw them, and an image with no text refused to send at all. TypeScript
                // cannot catch that — a shorter parameter list is a valid function here — so the
                // fix is to have no list to get wrong.
                picturesReachThem={Boolean(agent)}
                onSpeak={turn.speak}
                onHalt={turn.halt}
                mode={
                  autonomy?.modes.find(
                    (option) => option.id === autonomy.current,
                  )?.label ?? null
                }
                reasoning={
                  dial
                    ? (dial.rungs.find((rung) => rung.id === dial.chosen)
                        ?.label ?? (dial.chosen ? dial.chosen : "Auto"))
                    : null
                }
                onCompact={
                  dial && turn.quest ? () => void turn.compact() : undefined
                }
              >
                {/*
            One button, two jobs, because they are the same job: the thing you want while a
            character is thinking is to stop them, and there is nothing else to press. A separate
            STOP that only appears mid-turn would move the target under the cursor.
          */}
                {/*
              What the character was told this turn (ADR-0012's Context Report), behind a
              button rather than laid across the composer.

              A permanent bar spends a row of the conversation on a number that matters a few
              times a session. Asked-for, it costs nothing until it is wanted — and the button
              still carries the one bit worth glancing at, so a Quest running out of room stays
              visible without opening anything.

              Measured, not estimated: `used / budget` comes from the Composer that made the
              decision, not from a count taken afterwards.
            */}
                {/*
              Always here, cold until there is something to read.

              It used to appear after the first turn, which put a new control under the cursor
              at the exact moment the user was reading an answer — and taught them the button
              was something that came and went rather than something the box has. A cold
              instrument keeps its frame and reads `—`; that is the standing rule for a panel
              with nothing behind it yet, and it applies to a button just as well.
            */}
                {/*
              How hard they think, one click away.

              Beside CONTEXT because they are the same kind of thing: the two dials over a turn
              that the user actually changes mid-conversation. Its own component because the rule
              it carries — a brain with no measured ladder shows no control — could not be
              asserted while it lived inside this file.
            */}
                <ReasoningDial
                  dial={dial}
                  who={who.name}
                  onChoose={(rung) => {
                    // Shown at once, written behind it. The Engine is the record; this is the same
                    // value arriving on screen before the disk answers, and a failure puts the real
                    // one back rather than leaving the slider claiming something untrue.
                    setDial(dial ? { ...dial, chosen: rung } : dial);
                    void chooseReasoning(who.id, rung).then((failed) => {
                      if (failed) void fetchReasoning(who.id).then(setDial);
                    });
                  }}
                />
                {contextGauge}
              </DialogueComposer>
            )}
          </ContextGauge>
        </div>
      </div>
    </Frame>
  );
}
