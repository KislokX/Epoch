/**
 * The door an agent knocks on, as a surface sees it (step 5.2).
 *
 * ## Event first, and a read at each edge
 *
 * A question appears at a moment only the endpoint's thread knows about — an agent decided to
 * write a file. The Engine says so with an event, and the only way that event can go unheard is
 * if nothing is listening: a WebView reload, a remount, StrictMode's double mount. So the
 * snapshot is re-read at exactly the moments a listener may have been absent — after subscribing,
 * when a turn starts, and when one ends — and the revision guard stops any of those reads from
 * overwriting something newer.
 *
 * **Not a poll.** There was an interval here, every 350 ms for the life of every turn, added
 * while the prompt was still failing to appear. Both causes of that turned out to be elsewhere:
 * the Engine published a question before its own slot could be recovered (`Asking::ask_about_with`),
 * and Codex was being handed a second, non-interactive permission route it took instead of the
 * native one. With those fixed the timer was a third source of truth for a question that already
 * had two, running three IPC round trips a second through the permission path.
 *
 * What is given up, honestly: an event dropped mid-turn while this hook is mounted and listening
 * would now wait for the turn to end. Nothing observed does that — Tauri delivers to registered
 * listeners — and a timer kept against an unobserved failure is the same invention as a gauge
 * kept against an unmeasured reading.
 *
 * ## The question is answered in one place, wherever the user is
 *
 * The prompt is drawn in the World rather than inside a conversation, because an agent is not
 * *in* a conversation — it is working. But the buttons and the words are the ones the model's
 * approvals already use, deliberately: a second prompt that looked different would teach the
 * user there are two permission systems, which is the exact thing 5.2 exists to prevent.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** What an agent is waiting on. */
export interface AgentQuestion {
  /** Whose hands these are. An agent acts as a character. */
  readonly character: string;
  readonly capability: string;
  /** What will happen, in the capability's own words. */
  readonly what: string;
  /** The change itself, when the capability could show it without making it (ADR-0009). */
  readonly preview: string | null;
  /**
   * Whether "and stop asking here" is a real option for this question.
   *
   * **False for a permission granted for one turn.** A coarse grant that quietly became permanent
   * would be exactly the escalation the arrangement exists to prevent — so the button is not
   * drawn, rather than drawn and ignored. A control that does not do what it says is worse than
   * a missing one.
   */
  readonly standing: boolean;
}

/** Whether Epoch is listening, for whom, and what is being asked. */
export interface AgentBridge {
  readonly url: string | null;
  readonly token: string | null;
  readonly character: string | null;
  readonly question: AgentQuestion | null;
}

const CLOSED: AgentBridge = { url: null, token: null, character: null, question: null };

export interface AgentDoor {
  readonly bridge: AgentBridge;
  readonly question: AgentQuestion | null;
  readonly open: (characterId: string) => Promise<string | null>;
  readonly close: () => Promise<void>;
  readonly answer: (approve: boolean, always: boolean) => Promise<void>;
  /** The configuration to paste into an agent, or `null` when the door is shut. */
  readonly configuration: () => Promise<string | null>;
  /** Write it into this World's project instead, so nobody has to. */
  readonly adopt: () => Promise<Adopted | string>;
}

/** What `adopt` changed, so the panel can say so. */
export interface Adopted {
  readonly wrote: readonly string[];
  /** Servers that were already there and were left alone. */
  readonly kept: readonly string[];
}

export function useAgentDoor(): AgentDoor {
  const [bridge, setBridge] = useState<AgentBridge>(CLOSED);
  const [question, setQuestion] = useState<AgentQuestion | null>(null);
  /*
    `agent_bridge` is a snapshot while `agent:asking` is a newer fact. A delayed empty snapshot
    must never erase a question emitted after it started, or Manual leaves the agent waiting with
    no visible answer button. This revision is updated synchronously by the event handlers.
  */
  const questionRevision = useRef(0);

  const reread = useCallback(async () => {
    const snapshotAt = questionRevision.current;
    const next = await invoke<AgentBridge>("agent_bridge");
    setBridge(next);
    // A question that was already open when this mounted. Without it, reloading the window
    // while an agent waits would leave a thread parked on a prompt nobody can see.
    if (questionRevision.current === snapshotAt) setQuestion(next.question);
  }, []);

  useEffect(() => {
    /*
      StrictMode mounts effects twice, and `listen` is async — so an unsubscribe scheduled
      before the handle arrived would leave the first listener attached and every question
      would be handled twice. The flag is what makes the late arrival tear itself down.
    */
    let live = true;
    const undo: Array<() => void> = [];
    const track = (pending: Promise<() => void>) => {
      void pending.then((off) => (live ? undo.push(off) : off()));
    };

    const opened = listen<AgentQuestion>("agent:asking", (event) => {
      questionRevision.current += 1;
      setQuestion(event.payload);
    });
    // No payload: the question is gone, however it went — answered, refused, or timed out. A
    // prompt left on screen after its own deadline is a button that does nothing.
    const settled = listen("agent:settled", () => {
      questionRevision.current += 1;
      setQuestion(null);
    });
    /*
      The two edges of a turn. A question can only be waiting unheard if this hook was not
      listening when it was asked, and a turn beginning is when a remount is most likely to have
      just happened — so one read there, and one when the turn ends, because an answer can settle
      just before `agent:settled` reaches this WebView and a stale prompt is a button that does
      nothing.
    */
    const started = listen("turn:started", () => void reread());
    const ended = listen("turn:ended", () => void reread());
    track(opened);
    track(settled);
    track(started);
    track(ended);

    // Subscribe before taking the recovery snapshot. Either event/snapshot ordering now renders
    // one answerable prompt rather than timing the agent out behind an empty screen.
    void Promise.all([opened, settled, started, ended]).then(() => {
      if (live) void reread();
    });

    return () => {
      live = false;
      for (const off of undo) off();
    };
  }, [reread]);

  return {
    bridge,
    question,
    open: async (characterId) => {
      try {
        setBridge(await invoke<AgentBridge>("open_agent_door", { characterId }));
        return null;
      } catch (error) {
        return String(error).replace(/^Error: /, "");
      }
    },
    close: async () => {
      await invoke("close_agent_door");
      setBridge(CLOSED);
      setQuestion(null);
    },
    answer: async (approve, always) => {
      // The Engine answers `false` when the question is already gone — the deadline can pass
      // between the prompt being drawn and the button being pressed. Either way it is gone
      // here, so the prompt comes down rather than sitting there refusing to be dismissed.
      await invoke("answer_agent", { approve, always }).catch(() => undefined);
      setQuestion(null);
    },
    configuration: () => invoke<string | null>("agent_configuration"),
    adopt: () =>
      invoke<Adopted>("adopt_project").catch(
        (error) => String(error).replace(/^Error: /, "") as string,
      ),
  };
}
