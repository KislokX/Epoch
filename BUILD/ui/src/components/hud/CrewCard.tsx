/** One crew member's honest, compact instruments in the World sidebar. */

import type { CSSProperties } from "react";

import { Cursor, PixelIcon } from "./Pixel";
import type { Glyph } from "./Pixel";

export interface CrewContextReading {
  readonly used: number;
  readonly budget: number;
}

export interface CrewAllowanceReading {
  /** The remaining amount reported by this character's signed-in agent, never inferred. */
  readonly remainingPercent: number | null;
  readonly hint: string;
}

interface CrewCardProps {
  readonly glyph: Glyph;
  readonly name: string;
  /**
   * What thinks for them, in their own words — a model's name, or the agent's.
   *
   * `null` is a real state and reads as such: a character with no brain assigned cannot work,
   * and the card is where somebody notices that before asking them something.
   */
  readonly brain: string | null;
  /**
   * Which model actually thought on the last turn, **as the agent reported it afterwards**.
   *
   * Never the model that was requested. A routing agent answers with whichever model it chose —
   * a measured Gemini run opened with `"model":"auto"` and thought with `gemini-3-flash-preview`
   * — so this is shown only when the agent said so, and nothing is shown when it did not.
   */
  readonly thoughtWith?: string | null;
  /**
   * **Which sign-in this character speaks with**, when the machine has more than one of that
   * program.
   *
   * The card said `haiku` for two characters on two different accounts, so they read as
   * identical — and the only way to find out which was which was to ask the character, whose
   * answer about itself is a self-report and not a measurement. Everything was measured,
   * nothing was invented, and the reading still did not arrive.
   *
   * `null` with one account, deliberately: naming the only sign-in there is would be an
   * instrument that never moves.
   */
  readonly account?: string | null;
  /**
   * What this character is doing, in three states rather than two.
   *
   * `WAITING` is the one the owner named: a job of theirs is running and **they are not the one
   * running it** — ComfyUI is drawing the picture they asked for. `IDLE` would say nothing is
   * happening and `WORKING` would credit them with somebody else's work.
   */
  readonly activity: "WORKING" | "WAITING" | "IDLE";
  readonly active: boolean;
  readonly allowance: CrewAllowanceReading | null;
  readonly context: CrewContextReading | null;
  readonly onClick: () => void;
}

function Meter({ label, percent, detail, cold, hint, colour, tone = "neutral", indeterminate = false }: {
  readonly label: string;
  readonly percent: number | null;
  readonly detail: string;
  readonly cold?: boolean;
  readonly hint?: string;
  readonly colour: string;
  readonly tone?: "allowance" | "context" | "neutral";
  /** The agent reported usage but not the size of its context window. Never a fake percentage. */
  readonly indeterminate?: boolean;
}) {
  return (
    <span className={`hud__crew-meter${cold ? " hud__crew-meter--cold" : ""}`} title={hint}>
      <span className="hud__crew-meter-head"><i>{label}</i><b>{detail}</b></span>
      <span className={`epbar hud__crew-meter-track hud__crew-meter-track--${tone}`} aria-label={`${label}: ${detail}`}>
        {(percent !== null || indeterminate) && (
          <i
            className={`epbar__fill${indeterminate ? " epbar__fill--indeterminate" : ""}`}
            // A lit band says "there is measured activity". It is deliberately not proportional
            // when a CLI reports used tokens without exposing the context ceiling.
            style={{ width: indeterminate ? "38%" : `${Math.max(0, Math.min(100, percent ?? 0))}%`, "--bar": colour } as CSSProperties}
          />
        )}
      </span>
    </span>
  );
}

/**
 * A local model intentionally has no allowance meter: it has no subscription plan to exhaust.
 * A signed-in agent whose CLI cannot report a reading keeps the meter cold, which is distinct
 * from an exhausted plan. Context belongs to the active Quest only.
 */
export function CrewCard({ glyph, name, brain, thoughtWith, account, activity, active, allowance, context, onClick }: CrewCardProps) {
  /*
    How full the window is — the same direction, and the same number, as the gauge above the
    composer.

    It used to be the room *left*: `100 - used/budget`, so a conversation that had used 1% of its
    window showed 99% on the card and 1% in the chat, four centimetres apart, under the same word.
    The intent was a mana bar that compaction visibly refills, and that reading is still there —
    it is the bar draining rather than filling. What is not negotiable is that two instruments
    called CONTEXT agree about which way the number runs.
  */
  const contextPercent = context && context.budget > 0
    ? Math.round((context.used / context.budget) * 100)
    : null;
  const contextDetail = !context
    ? "—"
    : context.budget > 0
      ? `${Math.max(0, contextPercent ?? 0)}%`
      : `${context.used.toLocaleString()} used`;

  return (
    <button type="button" className={`hud__crew-card${active ? " hud__crew-card--on" : ""}`} onClick={onClick}>
      {active && <span className="hud__crew-cursor"><Cursor shape="arrow" /></span>}
      <span className="hud__crew-card-head">
        <PixelIcon glyph={glyph} size={16} tone={active ? "gold" : "parchment"} />
        <b>{name}</b>
        <em>{activity}</em>
      </span>
      {/*
        Under the name, because that is the question the card could not answer: two characters
        look identical here and may be thinking with entirely different things — and swapping a
        brain is a thing people do between turns. Not a meter: it is a fact, not a reading.
      */}
      <span
        className={`hud__crew-brain${brain ? "" : " hud__crew-brain--none"}`}
        title={brain ? `${name} thinks with ${brain}` : `${name} has no brain assigned`}
      >
        {brain ?? "no brain"}
        {/*
          Beside the brain, because it is the other half of the same fact: `haiku` says what
          thinks, and this says whose sign-in it thinks on. Only ever drawn when the machine has
          two of one program — the case where the two cards were indistinguishable.
        */}
        {account && (
          <i
            className="hud__crew-account"
            title={`${name} works on the ${account} sign-in`}
          >
            {` · ${account}`}
          </i>
        )}
      </span>
      {/*
        Under the brain rather than replacing it: they are two different facts. The brain is what
        the character was assigned; this is what actually answered, and only an agent that routes
        makes them differ. Hidden when it adds nothing — repeating the brain's own name would be
        an instrument that never moves.
      */}
      {thoughtWith && thoughtWith !== brain && (
        <span
          className="hud__crew-thought"
          title={`Reported by the agent after the turn — ${name} asked for a brain, not this model`}
        >
          thought with {thoughtWith}
        </span>
      )}
      {allowance && (
        <Meter
          label="ALLOWANCE"
          percent={allowance.remainingPercent}
          detail={allowance.remainingPercent === null ? "—" : `${allowance.remainingPercent}%`}
          cold={allowance.remainingPercent === null}
          hint={allowance.hint}
          colour="var(--ep-warn)"
          tone="allowance"
        />
      )}
      <Meter
        label="CONTEXT"
        percent={contextPercent}
        detail={contextDetail}
        cold={!context}
        hint={context && context.budget > 0
          ? `${context.used.toLocaleString()} / ${context.budget.toLocaleString()} used; compact restores available room.`
          : context
            ? `${context.used.toLocaleString()} tokens used. This agent does not report its context ceiling, so the band is activity rather than a percentage.`
            : undefined}
        colour="var(--ep-cyan)"
        tone="context"
        indeterminate={Boolean(context && context.budget === 0 && context.used > 0)}
      />
    </button>
  );
}
