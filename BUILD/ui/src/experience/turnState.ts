/**
 * The one mutable surface-state record for a running turn.
 *
 * The Engine owns the Quest and decides every outcome. This reducer owns only what one
 * Experience Surface needs while listening: streamed text, an in-flight tool, notices, and the
 * last measured context. Named transitions make those short-lived facts reviewable without
 * turning the hook into thirteen unrelated state cells.
 */

import type { QuestSummary } from "../ipc/world";
import type { Awaiting, Knew, Source, Wanted, Working } from "./useTurn";

export interface TurnState {
  readonly quest: QuestSummary | null;
  readonly planned: string | null;
  readonly writing: string;
  /**
   * **Who is actually speaking**, when a turn is running in the shown Quest.
   *
   * `null` when nobody is. It exists because the live block used to be titled with whoever was
   * *selected*, which was the same person for as long as the World ran one turn at a time — and
   * stopped being, the moment two characters could work. Watched: a message sent to Mage
   * streamed under PALADIN's name because the crew card had been clicked in between, and the
   * finished answer then landed under MAGE, correctly. A caret under the wrong name is a
   * character appearing to say something they never said.
   */
  readonly speaking: string | null;
  readonly thinking: boolean;
  /** Earlier Chronicle records being reduced into a continuity brief, if any. */
  readonly compacting: number | null;
  readonly failed: string | null;
  readonly invited: readonly string[];
  readonly working: readonly Working[];
  readonly awaiting: Awaiting | null;
  readonly wanted: Wanted | null;
  readonly unfinished: boolean;
  /**
   * How many rounds the last turn took.
   *
   * **A limit nobody can see the size of is one people assume is arbitrary.** The owner watched
   * a character say *"Te lo reproduzco ahora:"* and stop, and read it as the character being
   * limited — which it was, and the notice said only *"the round limit"*. The Engine had counted
   * them the whole time.
   */
  readonly rounds: number;
  readonly halted: boolean;
  readonly knew: Knew | null;
  readonly sources: readonly Source[];
}

export const INITIAL_TURN_STATE: TurnState = {
  quest: null,
  planned: null,
  writing: "",
  speaking: null,
  thinking: false,
  compacting: null,
  failed: null,
  invited: [],
  working: [],
  awaiting: null,
  wanted: null,
  unfinished: false,
  rounds: 0,
  halted: false,
  knew: null,
  sources: [],
};

export interface TurnStep {
  readonly phase: "using" | "said" | "used";
  readonly capability: string;
  readonly what?: string;
  readonly ok?: boolean;
  readonly detail?: string;
}

export interface TurnEnded {
  readonly error: string | null;
  readonly invited: readonly string[];
  readonly sources?: readonly Source[];
  readonly stop?: string;
  /** How many rounds of tools the turn took. Counted by the Engine, printed by the notice. */
  readonly rounds?: number;
  readonly awaiting?: Awaiting | null;
  readonly wanted?: Wanted | null;
}

export type TurnAction =
  | { readonly type: "quest/reloaded"; readonly quest: QuestSummary | null }
  | { readonly type: "context/cleared" }
  | { readonly type: "context/remembered"; readonly knew: Knew }
  | { readonly type: "context/measured"; readonly knew: Knew }
  | { readonly type: "turn/token"; readonly token: string }
  | { readonly type: "turn/step"; readonly step: TurnStep }
  | { readonly type: "turn/started"; readonly speaker?: string }
  | { readonly type: "turn/compacting"; readonly through: number }
  | { readonly type: "turn/handoverStarted" }
  | { readonly type: "turn/ended"; readonly ended: TurnEnded }
  | { readonly type: "turn/failedToStart"; readonly failure: string }
  | { readonly type: "turn/answering" }
  | { readonly type: "turn/wantedAnswered" }
  | { readonly type: "quest/planned"; readonly title: string | null }
  | { readonly type: "quest/setAside" }
  | { readonly type: "notice/invitationCleared" }
  | { readonly type: "notice/endingCleared" }
  | { readonly type: "notice/failureCleared" }
  | { readonly type: "turn/reread" };

function applyStep(
  current: readonly Working[],
  step: TurnStep,
): readonly Working[] {
  // A spoken line is Chronicle material, not Terminal work. Ignoring it explicitly makes a new
  // Engine phase unable to silently become a completed tool invocation.
  if (step.phase === "said") return current;

  if (step.phase === "using") {
    return [
      ...current,
      {
        capability: step.capability,
        what: step.what ?? step.capability,
        running: true,
        ok: null,
        detail: "",
      },
    ];
  }

  // A capability may run twice at once. Finish the last still-running instance, rather than an
  // index supplied by a transport event, so concurrent reads cannot exchange their outcomes.
  let at = -1;
  for (let i = current.length - 1; i >= 0; i -= 1) {
    const candidate = current[i];
    if (
      candidate &&
      candidate.running &&
      candidate.capability === step.capability
    ) {
      at = i;
      break;
    }
  }
  const finished: Working = {
    capability: step.capability,
    what: at >= 0 ? (current[at]?.what ?? step.capability) : step.capability,
    running: false,
    ok: step.ok ?? false,
    detail: step.detail ?? "",
  };
  if (at < 0) return [...current, finished];
  return current.map((work, index) => (index === at ? finished : work));
}

export function turnReducer(state: TurnState, action: TurnAction): TurnState {
  switch (action.type) {
    case "quest/reloaded":
      return { ...state, quest: action.quest };
    case "context/cleared":
      return { ...state, knew: null };
    case "context/remembered":
      // A current measurement wins over an older agent-reported window.
      return state.knew ? state : { ...state, knew: action.knew };
    case "context/measured":
      return { ...state, knew: action.knew };
    case "turn/token":
      return { ...state, writing: state.writing + action.token };
    case "turn/step":
      return { ...state, working: applyStep(state.working, action.step) };
    case "turn/started":
      return {
        ...state,
        failed: null,
        speaking: action.speaker ?? null,
        writing: "",
        invited: [],
        sources: [],
        thinking: true,
        compacting: null,
      };
    case "turn/compacting":
      return { ...state, compacting: action.through };
    case "turn/handoverStarted":
      return {
        ...state,
        failed: null,
        writing: "",
        invited: [],
        thinking: true,
      };
    case "turn/ended":
      return {
        ...state,
        thinking: false,
        speaking: null,
        compacting: null,
        writing: "",
        invited: action.ended.invited,
        sources: action.ended.sources ?? [],
        working: [],
        awaiting: action.ended.awaiting ?? null,
        wanted: action.ended.wanted ?? null,
        unfinished: action.ended.stop === "rounds_exhausted",
        rounds: action.ended.rounds ?? 0,
        halted: action.ended.stop === "stopped",
        failed: action.ended.error ?? state.failed,
      };
    case "turn/failedToStart":
      return {
        ...state,
        thinking: false,
        compacting: null,
        failed: action.failure,
      };
    case "turn/answering":
      return { ...state, awaiting: null, thinking: true, failed: null };
    case "turn/wantedAnswered":
      return { ...state, wanted: null, thinking: true, failed: null };
    case "quest/planned":
      return { ...state, planned: action.title };
    case "quest/setAside":
      return {
        ...state,
        planned: null,
        writing: "",
        thinking: false,
        compacting: null,
        knew: null,
        failed: null,
        unfinished: false,
        halted: false,
        sources: [],
        working: [],
        awaiting: null,
        wanted: null,
      };
    case "notice/invitationCleared":
      return { ...state, invited: [] };
    case "notice/endingCleared":
      return { ...state, unfinished: false, halted: false };
    case "notice/failureCleared":
      return { ...state, failed: null };
    case "turn/reread":
      // A Quest switch is not a continuation of whatever was painted in the old window. The
      // listener will immediately restore the selected Quest's own live or completed state if
      // it has one. Clearing first prevents a one-frame lie where Quest A looks like it is
      // still answering inside Quest B.
      return {
        ...state,
        writing: "",
        thinking: false,
        compacting: null,
        knew: null,
        failed: null,
        invited: [],
        sources: [],
        working: [],
        awaiting: null,
        wanted: null,
        unfinished: false,
        halted: false,
      };
  }
}
