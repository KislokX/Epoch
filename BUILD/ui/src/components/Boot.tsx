/**
 * Cold start — the moment Epoch wakes up.
 *
 * Two beats, from the authored reference in `UINEW/`: a POST that reports the machine to itself,
 * and the wordmark it finishes into.
 *
 * ## It does the work rather than covering for it
 *
 * This is the one thing that changed from the reference, and it is the whole point. Its POST was
 * a script — `CORE MEMORY 16384K OK`, `REACTOR NOMINAL`, `HULL INTEGRITY SEALED` — text that
 * reads like a computer starting and reports nothing. Ours types a line **because a real step is
 * running**, and fills the value in with what that step measured. `startup.ts` owns all of it;
 * this file knows how to show a list, not how to read a vault.
 *
 * So the seconds here are not added to Epoch. They are the seconds the bridge used to spend
 * assembling itself in public — its survey landing late, its provider and agent probes held back
 * 350 ms so they would not fight the first paint — moved somewhere they are legible. The Launcher
 * then opens complete, with those answers already in hand.
 *
 * ## Nothing on this screen is invented
 *
 * Every value is measured, including the bad news: a provider that did not answer says where
 * Epoch looked, an agent that is installed and signed out says exactly that, and a step that
 * throws says it failed rather than going green. The percentage is steps finished over steps
 * there are — a count, never a clock pretending to be one. The one name shown is the
 * orchestrator's own, and a bridge nobody has introduced themselves to says so.
 *
 * The wordmark, the starfield and the sweep invent nothing either: they describe *that*
 * something is starting, which is the licence a door animation has (ADR-0022, `Departure`).
 *
 * ## Three things it must never do
 *
 * **Trap anybody.** SKIP, any key and any click end it at once, and a ceiling ends it whatever
 * the Engine is doing. Skipping early hands over nothing and the bridge simply asks for itself,
 * exactly as it did before this existed — so the sequence is never load-bearing.
 *
 * **Play a sound.** Web Audio may only begin from a user gesture and a cold start has none (see
 * `SfxGate`). A boot chime is therefore impossible rather than missing; arming the context early
 * to fake one would break the gate that keeps a programmatic render silent.
 *
 * **Repeat.** It belongs to starting the application, not to entering a World — `Departure`
 * already owns that boundary. `App` mounts once, so the flag lives there.
 */

import { useEffect, useRef, useState } from "react";

import {
  LIVE_SOURCES,
  runStartup,
  STARTUP_STEPS,
  type StartupLine,
  type StartupResult,
  type StartupSources,
} from "../experience/startup";

/** How the POST types: characters per tick, the tick, and the pause between lines. */
const TYPE = { CHARS: 4, TICK: 11, GAP: 90 } as const;
/** Where the value column starts, in characters. Dots fill the gap, as a BIOS does. */
const COLUMN = 40;
/** How long the last line holds before the wordmark takes over. */
const SETTLE = 420;
/** The shortest the wordmark may be on screen. */
const LOGO_HOLD = 1100;
/**
 * The longest a *presentation* may hold the screen.
 *
 * Not a timeout on the startup — those calls keep going and the bridge shows what it has when it
 * has it. This exists because an Engine that never answers must not be able to lock somebody out
 * of their own application.
 */
const CEILING = 12_000;

interface BootProps {
  /** Hand over everything that was read, so the bridge does not ask again. `null` when skipped. */
  readonly onDone: (startup: StartupResult | null) => void;
  /** Test seam. */
  readonly sources?: StartupSources;
}

/** `LABEL ....... ` — the label, padded with dots to the value column. */
export function dotted(label: string, indent = false): string {
  const text = indent ? `  ${label}` : label;
  return `${text} ${".".repeat(Math.max(3, COLUMN - text.length))}`;
}

export function Boot({ onDone, sources = LIVE_SOURCES }: BootProps) {
  const [lines, setLines] = useState<readonly StartupLine[]>([]);
  /** How many lines are fully typed. The one being typed is `lines[shown]`. */
  const [shown, setShown] = useState(0);
  const [typing, setTyping] = useState(0);
  const [steps, setSteps] = useState(0);
  const [read, setRead] = useState(false);
  const [phase, setPhase] = useState<"post" | "logo">("post");
  const [going, setGoing] = useState(false);

  /** What the startup measured. A ref so leaving early still hands over whatever landed. */
  const result = useRef<StartupResult | null>(null);

  // The real work. Started on mount and never restarted — this is the application opening.
  useEffect(() => {
    let live = true;
    void runStartup(sources, (next, finished) => {
      if (!live) return;
      setLines((all) => [...all, ...next]);
      setSteps(finished);
    }).then((startup) => {
      if (!live) return;
      result.current = startup;
      setRead(true);
    });
    return () => {
      live = false;
    };
  }, [sources]);

  // Typing. One tick at a time so the list reveals at the pace the work actually arrives: a
  // step that is still running leaves nothing to type, and the caret simply waits.
  useEffect(() => {
    if (phase !== "post" || shown >= lines.length) return;
    const line = lines[shown];
    if (!line) return;
    const label = dotted(line.label, line.indent);
    if (typing < label.length) {
      const id = window.setTimeout(
        () => setTyping((t) => Math.min(label.length, t + TYPE.CHARS)),
        TYPE.TICK,
      );
      return () => window.clearTimeout(id);
    }
    const id = window.setTimeout(() => {
      setShown((s) => s + 1);
      setTyping(0);
    }, TYPE.GAP);
    return () => window.clearTimeout(id);
  }, [phase, lines, shown, typing]);

  // The POST is over when the work is done *and* the last line has been read out.
  useEffect(() => {
    if (phase !== "post" || !read || shown < lines.length) return;
    const id = window.setTimeout(() => setPhase("logo"), SETTLE);
    return () => window.clearTimeout(id);
  }, [phase, read, shown, lines.length]);

  useEffect(() => {
    if (phase !== "logo") return;
    const id = window.setTimeout(() => setGoing(true), LOGO_HOLD);
    return () => window.clearTimeout(id);
  }, [phase]);

  useEffect(() => {
    const id = window.setTimeout(() => setGoing(true), CEILING);
    return () => window.clearTimeout(id);
  }, []);

  // Any key, any click. Not only the SKIP button: a control somebody has to find and aim at is
  // slower than the sequence it skips.
  useEffect(() => {
    const leave = () => setGoing(true);
    window.addEventListener("keydown", leave, true);
    window.addEventListener("pointerdown", leave, true);
    return () => {
      window.removeEventListener("keydown", leave, true);
      window.removeEventListener("pointerdown", leave, true);
    };
  }, []);

  // The dissolve is presentation, so a timer hands over rather than `transitionend`: a
  // reduced-motion machine fires no transition and would be left staring at a black rectangle.
  useEffect(() => {
    if (!going) return;
    const id = window.setTimeout(() => onDone(result.current), 420);
    return () => window.clearTimeout(id);
  }, [going, onDone]);

  const percent = Math.round((steps / STARTUP_STEPS) * 100);
  const orchestrator = result.current?.view.orchestrator;

  return (
    <div className={`boot${going ? " boot--going" : ""}`} data-phase={phase}>
      <div className="boot__stars" />
      <div className="boot__glow" />
      <div className="boot__vignette" />

      {phase === "post" && (
        <div className="boot__post">
          {lines.slice(0, shown + 1).map((line, i) => {
            const label = dotted(line.label, line.indent);
            const complete = i < shown;
            return (
              <div className="boot__row" key={`${line.label}-${String(i)}`}>
                <span className={`boot__label${line.tone === "head" ? " boot__label--head" : ""}`}>
                  {complete ? label : label.slice(0, typing)}
                </span>
                {complete && <span className={`boot__value boot__value--${line.tone}`}>{line.value}</span>}
              </div>
            );
          })}
          <div className="boot__status">
            <span>
              {read ? "WAKING INTELLIGENCE" : "CHECKING SYSTEMS"} · {String(percent)}%
            </span>
            <span className="boot__caret" />
          </div>
          <div className="boot__bar">
            {/* Steps finished over steps there are. A count, so it can never overrun or stall. */}
            <div className="boot__fill" style={{ width: `${String(percent)}%` }} />
          </div>
        </div>
      )}

      {phase === "logo" && (
        <div className="boot__logo">
          <div className="boot__rule">
            <span />◆<span />
          </div>
          <div className="boot__mark">
            {"EPOCH".split("").map((letter, i) => (
              <span
                key={letter + String(i)}
                className="boot__letter"
                style={{ animationDelay: `${String(i * 90)}ms` }}
              >
                {letter}
              </span>
            ))}
            <span className="boot__sweep" />
          </div>
          <div className="boot__sub">AI OPERATING SYSTEM</div>
          <div className="boot__welcome">
            {/*
              The one name on the screen, and it is theirs. A bridge nobody has introduced
              themselves to says that instead of inventing a title to greet.
            */}
            {orchestrator && !orchestrator.isUnnamed
              ? `WELCOME ABOARD, ${orchestrator.name.toUpperCase()}`
              : "THE BRIDGE IS YOURS"}
          </div>
        </div>
      )}

      <div className="boot__scan" />

      <button type="button" className="boot__skip" onClick={() => setGoing(true)}>
        SKIP
      </button>
    </div>
  );
}
