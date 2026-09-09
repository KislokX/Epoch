/**
 * Talking to a character.
 *
 * The turn runs in the Engine, on its own thread, for as long as the model takes — which for a
 * cold local model is minutes. So nothing here waits: `speak` returns as soon as the turn has
 * *started*, and the answer arrives as events.
 *
 * ## Why the streamed text lives here and not in the Engine's projection
 *
 * A half-written sentence is not World state. It is one surface watching one turn, and it
 * belongs to the surface watching it — the same line ADR-0018 draws between Experience State
 * and Experience Playback. What *is* World state is that she is working, and that arrives the
 * ordinary way, in `world:changed`.
 */

import { useCallback, useEffect, useReducer, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

import { answerCapability } from "../ipc/world";
import type { ImageAttachmentInput, TextAttachmentInput } from "../ipc/world";
import { INITIAL_TURN_STATE, turnReducer } from "./turnState";

// `Said` lives beside the wire it comes off, in `ipc/world`, and is re-exported here so a
// component that renders a turn does not have to know that. Two definitions of one shape was how
// `kind` came to be a closed union in one file and a bare `string` in the other.
export type { Said } from "../ipc/world";
import type { Said } from "../ipc/world";

/**
 * Somewhere the last answer came from.
 *
 * Not evidence. A search leaves nothing behind, so a Quest that only searched still produced
 * nothing (ADR-0025) — these exist so the answer can be *checked*, which is a different job.
 * Carried as data from the Engine rather than recovered from the text: a surface that scraped
 * URLs out of prose would eventually cite something the character merely mentioned.
 */
export interface Source {
  readonly title: string;
  readonly url: string;
}

/** The work, as a surface sees it. */
// Re-exported so a component importing a turn's shape does not have to know which module the
// Quest itself lives in. One definition, in `ipc/world`, because it is what the Engine sends.
export type { QuestSummary } from "../ipc/world";
import type { QuestSummary } from "../ipc/world";

/** One thing a character is doing, or has just done, during a turn. */
export interface Working {
  readonly capability: string;
  /** What it is doing, concretely: "read `src/main.rs`". */
  readonly what: string;
  /** False once it has finished. */
  readonly running: boolean;
  /** How it went. Null while still running. */
  readonly ok: boolean | null;
  /** The first line of what came back, or why it failed. */
  readonly detail: string;
}

/**
 * A capability the character reached for and was never given.
 *
 * Not the same as "that tool does not exist": this one could be theirs if you said so, which
 * makes it a question rather than a dead end.
 */
export interface Wanted {
  readonly capability: string;
  readonly summary: string;
  readonly risk: string;
  readonly reversal: string;
  /** What it would let them do, in plain words. */
  readonly effects: readonly string[];
}

/**
 * What a character was told this turn, and what they were not (ADR-0012).
 *
 * Understandable Autonomy is not a slogan: an answer that seems to ignore something is
 * explainable only if you can see whether it was ever there.
 */
export interface Knew {
  /** One line, assembled by the Engine. */
  readonly line: string;
  readonly dropped: number;
  readonly used: number;
  /** What the Composer may actually spend. */
  readonly budget: number;
  /**
   * Held back for the reply, and therefore not part of `budget`.
   *
   * Carried so the panel can say why the number it shows is smaller than the one the user set.
   * Without it, a character configured for 6,144 shows a ceiling of 5,120 and looks broken.
   */
  readonly reserved: number;
}

/** Something waiting on the user's word. */
export interface Awaiting {
  readonly capability: string;
  readonly what: string;
  readonly risk: string;
  readonly reversal: string;
  /**
   * The change itself, when the capability could show it without making it.
   *
   * A description tells you what will happen; this lets you notice that the wrong thing is
   * about to. `null` when the capability genuinely cannot say.
   */
  readonly preview: string | null;
}

export interface Turn {
  /** The Quest being worked on. `null` when nothing has been asked yet. */
  readonly quest: QuestSummary | null;
  /** Everything said so far, oldest first. Never includes anyone's standing instructions. */
  readonly said: readonly Said[];
  /** What she is writing right now, as it arrives. Empty when she is not writing. */
  readonly writing: string;
  /**
   * Who is writing it — `null` when nobody is.
   *
   * Not always the character the sidebar has selected: clicking a crew card while a turn is in
   * flight changes who you are addressing, not who is answering.
   */
  readonly speaking: string | null;
  /** True from asking until the turn ends — including while the model is still loading. */
  readonly thinking: boolean;
  /** Earlier records being reduced into a continuity brief, if a compaction is live. */
  readonly compacting: number | null;
  /** Ask the running turn to stop at its next seam. Does not clear {@link thinking}. */
  readonly halt: () => void;
  /** Why the last turn failed, if it did. Cleared on the next attempt. */
  readonly failed: string | null;
  /**
   * Crew members the last message named, who have not answered.
   *
   * An offer, never an action. Naming a colleague invites them into the Quest — but bringing
   * them in loads a second model, and that memory is the user's to spend. A character never
   * answers on somebody else's behalf, so if the user wants Paladin, Paladin must actually
   * speak.
   */
  readonly invited: readonly string[];
  /** Dismiss the invitation without bringing anyone in. */
  readonly declineInvite: () => void;
  /**
   * Where the last answer came from, if anywhere.
   *
   * Belongs to the turn rather than to the Chronicle for now: the Chronicle keeps the tool
   * result, which contains the same addresses in text. Persisting them as structure is worth
   * doing the day History needs to cite them.
   */
  readonly sources: readonly Source[];
  /**
   * What they are *doing* right now, as opposed to saying.
   *
   * A separate channel from the tokens, because the World renders work differently from talk
   * (ADR-0018). Cleared when the turn ends: this is the present tense, not a log.
   */
  readonly working: readonly Working[];
  /**
   * Something that needs your word before it can run (ADR-0009).
   *
   * The turn stopped *before* it, with nothing attempted. `null` while nothing is waiting.
   */
  readonly awaiting: Awaiting | null;
  /**
   * A capability they asked for and do not have (ADR-0009).
   *
   * The turn stopped before it. Granting writes their Definition — which only a person may do —
   * and the call is judged afterwards like any other.
   */
  readonly wanted: Wanted | null;
  /** Grant it, or refuse. Either way the character carries on and explains itself. */
  readonly answerWanted: (grant: boolean) => Promise<void>;
  /** True when the turn stopped because it ran out of rounds rather than because it finished. */
  readonly unfinished: boolean;
  /** How many rounds of tools the last turn took, so the notice can name the number. */
  readonly rounds: number;
  /**
   * Which ending the notice is about: the round limit, or the user's own stop.
   *
   * Separate from {@link unfinished} because they are different sentences. "It ran out of
   * rounds" is news; "you stopped it" is a confirmation, and telling somebody what they just
   * did in the voice of a fault reads as an error they caused.
   */
  readonly halted: boolean;
  /** Put the ending notice away. It says nothing the Chronicle does not already hold. */
  readonly clearEnding: () => void;
  /** Put away the failure notice. Every notice can be dismissed; none of them are the record. */
  readonly clearFailed: () => void;
  /** What they were told this turn. `null` before the first turn of a session. */
  readonly knew: Knew | null;
  /**
   * Answer what is waiting.
   *
   * `always` records a standing decision for this World, so the next one does not stop either.
   * Denying is not a cancellation — the model is told and carries on, which is how it explains
   * itself instead of leaving half an answer on screen.
   */
  readonly answer: (approve: boolean, always: boolean) => Promise<void>;
  /**
   * Hand the work to somebody else and let them answer.
   *
   * No new message: the Chronicle already holds what was addressed to them. This is what makes
   * the crew a crew rather than one model playing several parts — Paladin *reads* what Mage
   * said to him and replies himself.
   */
  readonly handOver: (characterId: string) => Promise<void>;
  /** Whether the live desktop engine acknowledged every selected attachment. */
  readonly speak: (
    said: string,
    attachments?: readonly TextAttachmentInput[],
    images?: readonly ImageAttachmentInput[],
  ) => Promise<boolean>;
  /** Persist a continuity brief and restart this external agent's next session fresh. */
  readonly compact: () => Promise<void>;
  /** Put the current work down. The next thing said inaugurates a new Quest. */
  readonly setAside: () => Promise<void>;
  /**
   * Rename the work. Its identity never changes — only what it is called.
   *
   * Before the work exists this *plans* the name: it is kept and applied the moment the first
   * thing said inaugurates the Quest. Nothing is created early, so a name typed and abandoned
   * leaves nothing behind.
   */
  readonly rename: (title: string) => Promise<void>;
  /** A name chosen before there was anything to name, so the header can show it. */
  readonly planned: string | null;
  /**
   * Read the current conversation again.
   *
   * For when something *outside* this hook changed which one it is — switching chats, closing
   * one. The Engine is the record; this is the box catching up with it rather than a second
   * copy being kept in step.
   */
  readonly reread: (questId?: string) => Promise<void>;
}

export function useTurn(characterId: string | null): Turn {
  const [state, dispatch] = useReducer(turnReducer, INITIAL_TURN_STATE);
  const {
    quest,
    planned,
    writing,
    speaking,
    thinking,
    compacting,
    failed,
    invited,
    working,
    awaiting,
    wanted,
    unfinished,
    rounds,
    halted,
    knew,
    sources,
  } = state;
  /**
   * A name typed before the work exists.
   *
   * A ref for the applying and a state for the drawing: the apply happens inside a callback that
   * must not be re-created every keystroke, and the header has to show what was typed.
   */
  const plannedName = useRef<string | null>(null);

  /**
   * Who we are listening for, read inside the event handler rather than closed over.
   *
   * The listeners are registered once. Without this they would capture whichever character was
   * selected when they were created, and switching specialists mid-turn would route one
   * character's tokens into another's box.
   */
  const listeningFor = useRef<string | null>(characterId);
  listeningFor.current = characterId;

  /**
   * The conversation this surface is actually drawing.
   *
   * Character identity alone is not enough: one Mage can finish Quest A after the user has
   * opened Quest B. Updating this ref before React awaits the new projection closes the brief
   * interval in which Quest A's stream used to render into Quest B.
   */
  const showingQuest = useRef<string | null>(null);

  /**
   * A turn belongs to a Quest even while its window is not open.
   *
   * The Engine is authoritative about completed Chronicle records; this short-lived index only
   * prevents navigation from discarding the lifecycle event that clears a spinner. It is keyed
   * by both speaker and Quest because a handover can change speakers without changing work.
   */
  const runningTurns = useRef(new Set<string>());
  const endedTurns = useRef(new Map<string, TurnEndedEvent>());

  /**
   * What an agent last reported about its own window.
   *
   * Epoch recomposes a *model's* context to re-read the gauge; it cannot for an agent, whose
   * context is its own. So the last measurement is remembered and asked for on arrival —
   * otherwise walking to the bridge and back blanked a number measured minutes ago, which reads
   * as "nothing has happened" about a conversation that plainly has.
   *
   * **Its own effect, keyed on the character.** It lived in the listener effect, whose only
   * dependency is a callback with no dependencies of its own — so it ran exactly once, at mount,
   * while nobody was being spoken to yet, and asked about `null` forever after. The listeners
   * are allowed to be set up once; this is not.
   */
  useEffect(() => {
    // **Cleared first, every time.** The gauge belongs to the conversation, and a test caught
    // what reasoning had not: switching from Mage to Paladin kept Mage's 24,691 on screen. A
    // reading that describes a conversation nobody is looking at is worse than no reading,
    // because a wrong number is still believed.
    dispatch({ type: "context/cleared" });
    if (!characterId) return;
    let alive = true;
    void invoke<{ used: number; budget: number } | null>("remembered_context", {
      character: characterId,
    })
      .then((last) => {
        if (!alive || !last) return;
        dispatch({
          type: "context/remembered",
          knew: {
            line: "Measured on their last turn, in this session.",
            dropped: 0,
            used: last.used,
            budget: last.budget,
            reserved: 0,
          },
        });
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [characterId]);

  const reload = useCallback(
    async (expectedQuest?: string): Promise<QuestSummary | null> => {
      if (expectedQuest !== undefined) showingQuest.current = expectedQuest;
      try {
        const next = await invoke<QuestSummary | null>("active_quest");
        // An ordinary refresh follows the Engine's active Quest. An explicit chat selection wins
        // until it has been read back, so an old turn cannot make the listener point back at it.
        if (expectedQuest !== undefined && next?.id !== expectedQuest)
          return null;
        showingQuest.current = next?.id ?? null;
        dispatch({ type: "quest/reloaded", quest: next });
        return next;
      } catch {
        if (expectedQuest === undefined) showingQuest.current = null;
        dispatch({ type: "quest/reloaded", quest: null });
        return null;
      }
    },
    [],
  );

  /**
   * The Quest is read once, not per character.
   *
   * This is the correction ADR-0025 forced: opening the box on Robo after talking to Mage shows
   * the *same* work, because the Chronicle belongs to the Quest. Under the old model this
   * effect keyed on `characterId` and each specialist had their own thread.
   */
  useEffect(() => {
    void reload();
  }, [reload]);

  useEffect(() => {
    /*
      Every token arrived twice, so a streaming answer read "UnUn closureclosure" until the
      turn finished and the recorded text replaced it.

      `listen` is asynchronous and StrictMode mounts twice. The first effect called `listen`,
      was torn down while that promise was still pending — so `stops` was empty and the
      teardown unsubscribed nothing — and then both promises resolved into two live listeners
      appending to the same buffer. The doubling was the whole bug, not the model.

      `live` closes the race: a subscription that arrives after teardown cancels itself instead
      of being pushed onto a list nobody will read again.
    */
    let live = true;
    const stops: Array<() => void> = [];
    const track = (stop: () => void) => {
      if (live) stops.push(stop);
      else stop();
    };

    const turnKey = (character: string, quest: string) =>
      `${character}\u0000${quest}`;
    // Tokens and tools are attributable to the person speaking. A turn's start and finish,
    // however, belong to the Quest: someone may visit a colleague while the first person's
    // work is still in flight, and that shared conversation must still stop when it finishes.
    const belongsToShownConversation = (payload: {
      characterId: string;
      questId: string;
    }) =>
      payload.characterId === listeningFor.current &&
      payload.questId === showingQuest.current;
    const belongsToShownQuest = (payload: { questId: string }) =>
      payload.questId === showingQuest.current;

    void listen<{ characterId: string; questId: string }>(
      "turn:started",
      (event) => {
        const { characterId: speaker, questId } = event.payload;
        runningTurns.current.add(turnKey(speaker, questId));
        endedTurns.current.delete(turnKey(speaker, questId));
        if (belongsToShownQuest(event.payload))
          dispatch({ type: "turn/started", speaker });
      },
    ).then(track);

    void listen<{ characterId: string; questId: string; token: string }>(
      "turn:token",
      (event) => {
        // **By Quest, not by who is selected.** A Quest runs one turn at a time, so the tokens
        // arriving for the shown Quest are that turn's — and dropping them because the crew card
        // had been clicked in between left a caret blinking under an empty block, next to a name
        // that was not even speaking.
        if (!belongsToShownQuest(event.payload)) return;
        dispatch({ type: "turn/token", token: event.payload.token });
      },
    ).then(track);

    // What they were told, sent as the turn starts rather than when it ends: by the time the
    // answer arrives you already want to know what was behind it.
    void listen<Knew & { characterId: string; questId: string }>(
      "turn:knew",
      (event) => {
        if (!belongsToShownConversation(event.payload)) return;
        dispatch({
          type: "context/measured",
          knew: {
            line: event.payload.line,
            dropped: event.payload.dropped,
            used: event.payload.used,
            budget: event.payload.budget,
            reserved: event.payload.reserved ?? 0,
          },
        });
      },
    ).then(track);

    void listen<{ characterId: string; questId: string }>(
      "turn:context-cleared",
      (event) => {
        if (!belongsToShownConversation(event.payload)) return;
        // A compaction only clears this after the new Chronicle record was safely written and the
        // old external session was retired. Until the next turn reports its own window, unknown
        // is more truthful than carrying a number from a session that no longer exists.
        dispatch({ type: "context/cleared" });
      },
    ).then(track);

    void listen<{ characterId: string; questId: string; through: number }>(
      "turn:compacting",
      (event) => {
        if (!belongsToShownConversation(event.payload)) return;
        dispatch({ type: "turn/compacting", through: event.payload.through });
      },
    ).then(track);

    // What they are doing. Arrives interleaved with the tokens, on its own channel.
    void listen<{
      characterId: string;
      questId: string;
      phase: "using" | "said" | "used";
      capability: string;
      what?: string;
      risk?: string;
      reversal?: string;
      ok?: boolean;
      detail?: string;
    }>("turn:step", (event) => {
      const step = event.payload;
      if (!belongsToShownConversation(step)) return;
      dispatch({ type: "turn/step", step });
    }).then(track);

    void listen<TurnEndedEvent>("turn:ended", (event) => {
      const key = turnKey(event.payload.characterId, event.payload.questId);
      // Always consume the lifecycle event first. The answer can be safely ignored while the
      // user looks elsewhere because it is already in the Chronicle; the finished state must
      // not be ignored or the old Quest will spin forever when they come back.
      runningTurns.current.delete(key);
      endedTurns.current.set(key, event.payload);
      if (!belongsToShownQuest(event.payload)) return;
      dispatch({
        type: "turn/ended",
        ended: {
          error: event.payload.error,
          invited: event.payload.invited ?? [],
          sources: event.payload.sources,
          stop: event.payload.stop,
          awaiting: event.payload.awaiting,
          wanted: event.payload.wanted,
        },
      });
      if (event.payload.error) {
        return;
      }
      // Re-read rather than append. The Engine wrote the answer into the Chronicle, and the
      // Chronicle is the record — reconstructing it here would be a second copy free to
      // disagree with it.
      void reload(event.payload.questId);
    }).then(track);

    // **Work that finished after the turn it belonged to had ended** (ADR-0034). A picture takes
    // longer than the sentence that asked for it, so the Engine files it minutes later and there
    // is no `turn:ended` coming to notice.
    //
    // The same re-read, for the same reason: the Chronicle is the record, and the alternative is
    // this component inventing an entry that the Engine's own copy is free to disagree with.
    //
    // By Quest, like everything else here — the job knows which one asked, and the person may
    // well be reading somebody else by now. Filing it correctly and then refusing to redraw the
    // conversation it landed in would be the same defect one layer up.
    void listen<{ characterId: string; questId: string; what: string }>(
      "world:finished",
      (event) => {
        if (!belongsToShownQuest(event.payload)) return;
        void reload(event.payload.questId);
      },
    ).then(track);

    return () => {
      live = false;
      stops.forEach((stop) => stop());
    };
  }, [reload]);

  const speak = useCallback(
    async (
      text: string,
      attachments: readonly TextAttachmentInput[] = [],
      images: readonly ImageAttachmentInput[] = [],
    ) => {
      const message = text.trim();
      if (
        !characterId ||
        (!message && attachments.length === 0 && images.length === 0) ||
        thinking
      )
        return false;

      dispatch({ type: "turn/started" });

      try {
        const receipt = await invoke<{
          attachments: number;
          images?: number;
        } | null>("speak_to", {
          characterId,
          said: message,
          attachments,
          images,
        });
        // The name the user chose before there was anything to name. Applied now, because now
        // the Quest exists — and cleared either way, so it can never land on the one after.
        if (plannedName.current) {
          try {
            await invoke("rename_quest", { title: plannedName.current });
          } catch {
            // The Engine's title stands. Not worth a notice: they can rename it from the header.
          }
          plannedName.current = null;
          dispatch({ type: "quest/planned", title: null });
        }
        // The intent is now in the Quest — possibly having inaugurated it. Read it back so what
        // the user asked appears, without this guessing what the Engine recorded.
        await reload();
        // A running application may hot-reload this UI while still hosting the previous Rust
        // command. That command starts the text turn and ignores the unknown `attachments`
        // field, so retain the draft rather than lying that the file travelled with it.
        // Both kinds, counted separately. An older Rust process ignores the field it does not
        // know and answers without it, so a missing count is a picture that did not travel —
        // never a picture assumed to have arrived.
        const text =
          attachments.length === 0 ||
          receipt?.attachments === attachments.length;
        const pictures =
          images.length === 0 || receipt?.images === images.length;
        return text && pictures;
      } catch (error) {
        // The turn never started, so no `turn:ended` is coming to clear this.
        dispatch({ type: "turn/failedToStart", failure: String(error) });
        await reload();
        return false;
      }
    },
    [characterId, thinking, reload],
  );

  const compact = useCallback(async () => {
    if (!characterId || thinking) return;
    dispatch({ type: "turn/started" });
    try {
      await invoke("compact_context", { characterId });
    } catch (error) {
      dispatch({ type: "turn/failedToStart", failure: String(error) });
    }
  }, [characterId, thinking]);

  /** Put the current work down. The next thing said inaugurates a new Quest. */
  /**
   * Put the work down. The next thing said starts a new Quest.
   *
   * Everything measured about the *previous* Quest is cleared here rather than left to be
   * overwritten by the next turn. A new Quest is a new file with its own Chronicle, so its
   * context starts at nothing — and a bar still reading 27% from work that has been put down
   * is not a stale number, it is a wrong one, describing a conversation that no longer exists.
   */
  const setAside = useCallback(async () => {
    // Unknown, not zero, and not the last one's. The next thing said opens a new conversation
    // whose window nobody has measured yet.
    dispatch({ type: "context/cleared" });
    // This is intentionally before the IPC call. A late token from the work just put down
    // belongs to that Quest's Chronicle, never to the empty NEW window.
    showingQuest.current = null;
    dispatch({ type: "turn/reread" });
    plannedName.current = null;
    dispatch({ type: "quest/planned", title: null });
    try {
      await invoke("set_quest_aside");
    } catch {
      // Nothing to put down is not a failure.
    }
    dispatch({ type: "quest/setAside" });
    await reload();
  }, [reload]);

  /**
   * Hand the work to somebody else.
   *
   * `listeningFor` is set **before** the invoke, not left to the next render. The Engine starts
   * streaming as soon as the command lands, and a ref that still named the previous speaker
   * would drop the first tokens of the new one on the floor — the same class of bug the ref
   * exists to prevent, arriving through the one path that does not wait for React.
   */
  const handOver = useCallback(
    async (id: string) => {
      if (thinking) return;
      listeningFor.current = id;
      dispatch({ type: "turn/handoverStarted" });

      try {
        await invoke("hand_over", { characterId: id });
      } catch (error) {
        dispatch({ type: "turn/failedToStart", failure: String(error) });
      }
      await reload();
    },
    [thinking, reload],
  );

  /** Give the work a different name. Its identity never changes — only what it is called. */
  const rename = useCallback(
    async (title: string) => {
      // **Named before it exists.**
      //
      // A Quest is inaugurated by the first thing said, so between pressing NEW and speaking
      // there is nothing on the Engine to rename — and the field was simply disabled, which
      // reads as "you may not name this" rather than "there is nothing here yet". Naming the
      // work before starting it is exactly how people actually open a new chat.
      //
      // Held here rather than invented in the Engine: no Quest is created early, so a name
      // typed and then abandoned leaves nothing behind.
      if (!quest) {
        plannedName.current = title.trim() || null;
        dispatch({ type: "quest/planned", title: plannedName.current });
        return;
      }
      try {
        await invoke("rename_quest", { title });
      } catch {
        // A refused rename leaves the old name, which is a fine outcome to say nothing about.
      }
      await reload();
    },
    [quest, reload],
  );

  /** Answer a turn that stopped to ask. Same listeners as any other turn carry the rest. */
  const answer = useCallback(async (approve: boolean, always: boolean) => {
    const who = listeningFor.current;
    if (!who) return;
    dispatch({ type: "turn/answering" });
    try {
      await invoke("answer_pending", { characterId: who, approve, always });
    } catch (error) {
      dispatch({ type: "turn/failedToStart", failure: String(error) });
    }
  }, []);

  /** Grant the capability they reached for, or refuse. */
  const answerWanted = useCallback(async (grant: boolean) => {
    const who = listeningFor.current;
    if (!who) return;
    dispatch({ type: "turn/wantedAnswered" });
    const failure = await answerCapability(who, grant);
    if (failure) {
      dispatch({ type: "turn/failedToStart", failure });
    }
  }, []);

  /**
   * Ask the running turn to stop at its next seam.
   *
   * Deliberately does not clear `thinking`. Whatever is in flight — a model call, a tool
   * already running — is still in flight, and showing "done" before it is done would be the
   * interface telling a story the Engine has not agreed to yet. `turn:ended` clears it, as it
   * does for every other ending.
   */
  const halt = useCallback(() => {
    void invoke("stop_turn");
  }, []);

  return {
    quest,
    halt,
    said: quest?.chronicle ?? [],
    writing,
    speaking,
    thinking,
    compacting,
    failed,
    invited,
    declineInvite: () => dispatch({ type: "notice/invitationCleared" }),
    sources,
    working,
    awaiting,
    wanted,
    answerWanted,
    unfinished,
    rounds,
    halted,
    clearEnding: () => dispatch({ type: "notice/endingCleared" }),
    clearFailed: () => dispatch({ type: "notice/failureCleared" }),
    knew,
    answer,
    handOver,
    speak,
    compact,
    setAside,
    rename,
    planned,
    reread: async (questId?: string) => {
      // The gauge belongs to the conversation, so it goes back to unknown until the new one
      // reports its own. Carrying the old number across would be the instrument describing a
      // conversation nobody is looking at.
      dispatch({ type: "turn/reread" });
      const next = await reload(questId);
      if (!next) return;

      // A handover can change the speaker, but never the Quest that owns the turn. Rehydrate by
      // Quest id so opening that work remains correct even before the sidebar selects its new NPC.
      const questSuffix = `\u0000${next.id}`;
      const isRunning = [...runningTurns.current].some((key) =>
        key.endsWith(questSuffix),
      );
      if (isRunning) {
        dispatch({ type: "turn/started" });
        return;
      }

      const ended = [...endedTurns.current.entries()].find(([key]) =>
        key.endsWith(questSuffix),
      )?.[1];
      if (ended) {
        dispatch({
          type: "turn/ended",
          ended: {
            error: ended.error,
            invited: ended.invited ?? [],
            sources: ended.sources,
            stop: ended.stop,
            awaiting: ended.awaiting,
            wanted: ended.wanted,
          },
        });
      }
    },
  };
}

interface TurnEndedEvent {
  readonly characterId: string;
  readonly questId: string;
  readonly answer: string | null;
  readonly error: string | null;
  readonly invited: string[];
  readonly sources?: Source[];
  readonly stop?: string;
  readonly awaiting?: Awaiting | null;
  readonly wanted?: Wanted | null;
}
