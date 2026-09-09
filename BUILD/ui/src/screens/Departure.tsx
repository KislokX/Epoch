/**
 * Departure — the airlock between the bridge and a World.
 *
 * Crossing into a World should feel like crossing a boundary (EXPERIENCE_CONSTITUTION XXI).
 * Before this, entering was a React state flip: the Launcher vanished and the World appeared,
 * in one frame, with nothing between them. Correct, instant, and it made a World feel like a
 * tab rather than a place.
 *
 * ## This sequence invents no information
 *
 * It is pure presentation over a real transition, which is exactly what the Experience Layer
 * is allowed to be (ADR-0022): the stages describe work the Engine is genuinely doing —
 * loading the pack, resolving its assets, populating the cast. The progress bar advances on
 * a timer rather than on engine progress, and that is honest for the same reason a door
 * animation is: it describes *that* something is opening, never how far along it is.
 *
 * The only fact shown is the crew count, which is real (ADR-0023).
 *
 * ## Skippable, always
 *
 * Immersion never costs productivity (LIVING_WORLD_DESIGN_GUIDE). Shift-clicking BOARD skips
 * straight to arrival, Escape aborts back to the bridge, and reduced-motion users get the
 * whole thing without animation.
 */

import { useEffect, useState } from "react";

/** What the ship is doing, in order. Index 0 is "not departing". */
const STAGES = [
  "",
  "AIRLOCK PRESSURIZING",
  "HANGAR DOORS OPENING",
  "APPROACH VECTOR LOCKED",
  "ARRIVAL CONFIRMED",
] as const;

const PROGRESS = ["0%", "22%", "58%", "88%", "100%"] as const;

/** When each stage begins, in milliseconds from the moment BOARD is pressed. */
const BEATS: readonly [number, number, number] = [800, 2100, 3900];

interface DepartureProps {
  readonly name: string;
  readonly berth: number;
  /** The World's identity. Its colour is derived from this, so it never changes. */
  readonly id: string;
  /** How many crew members live in this World. Real, from their own rosters. */
  readonly crew: number;
  /** Skip the sequence and land on arrival immediately. */
  readonly instant: boolean;
  /** Step through the hatch. */
  readonly onEnter: () => void;
  /** Return to the bridge without entering. */
  readonly onAbort: () => void;
}

/**
 * A World's colour, from its identity.
 *
 * A small stable hash over the id, spread across the wheel. Deterministic on purpose: the same
 * World is the same planet on every machine and in every session, and two Worlds are almost
 * never the same colour without anybody having to choose one.
 *
 * **Above the component, deliberately.** It was written below it, which is valid JavaScript —
 * declarations hoist — and still cost a boarding: a hot reload swapped the component in while
 * this binding was not yet in the module, and the airlock threw instead of opening. A module is
 * re-executed top to bottom, so what a component needs is defined before it. The rule is cheap
 * and the failure it prevents was not: nothing decorative may stand between somebody and their
 * World.
 */
function hueOf(id: string): number {
  let hash = 0;
  for (let i = 0; i < id.length; i += 1) {
    hash = (hash * 31 + id.charCodeAt(i)) % 360000;
  }
  return hash % 360;
}

export function Departure({
  name,
  berth,
  id,
  crew,
  instant,
  onEnter,
  onAbort,
}: DepartureProps) {
  const [stage, setStage] = useState(instant ? 4 : 1);

  useEffect(() => {
    if (instant) return;
    const timers = BEATS.map((ms, i) => setTimeout(() => setStage(i + 2), ms));
    return () => timers.forEach(clearTimeout);
  }, [instant]);

  // Escape aborts at any point, including mid-flight. A sequence you cannot leave is a
  // sequence that will be resented by the third time somebody sees it.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onAbort();
      if (e.key === "Enter" && stage >= 4) onEnter();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [stage, onAbort, onEnter]);

  const open = stage >= 2;
  const arrived = stage >= 4;

  return (
    <div className="dep" role="dialog" aria-label={`Departing for ${name}`}>
      <div className="dep__stars" />
      {/*
        Every World is a different world.

        The hue is **derived from the World's identity**, not stored: a colour somebody would
        have to choose is a colour most Worlds would never get, and a random one would make the
        same World a different planet every time you flew to it. Identity never changes
        (ADR-0028), so neither does this — your World is that colour, permanently, without
        anybody having authored anything.

        Purely presentational. Nothing downstream reads it, and a World Pack that later wants to
        say what colour it is simply overrides a variable.
      */}
      <div
        className="dep__planet"
        style={
          {
            transform: `scale(${stage >= 3 ? 1.9 : 0.55})`,
            "--hue": hueOf(id),
          } as React.CSSProperties
        }
      />
      <div className="dep__warp" style={{ opacity: stage === 3 ? 0.55 : 0 }} />

      {/* The hatch. Two halves that part — the boundary made literal. */}
      <div
        className="dep__door dep__door--l"
        style={{ transform: `translateX(${open ? "-100%" : "0%"})` }}
      >
        <div className="dep__seam" />
        <div className="dep__panel" />
      </div>
      <div
        className="dep__door dep__door--r"
        style={{ transform: `translateX(${open ? "100%" : "0%"})` }}
      >
        <div className="dep__seam" />
        <div className="dep__panel" />
      </div>

      <div className="dep__hud">
        <div className="dep__hud-inner">
          <div className="dep__stage" aria-live="polite">
            {STAGES[stage]}
          </div>
          <div
            className="dep__bar"
            role="progressbar"
            aria-valuenow={stage}
            aria-valuemin={0}
            aria-valuemax={4}
          >
            <div className="dep__fill" style={{ width: PROGRESS[stage] }} />
          </div>
          <div className="dep__dest">
            DESTINATION {name.toUpperCase()} &middot; AIRLOCK{" "}
            {String(berth).padStart(2, "0")}
          </div>
        </div>
      </div>

      {arrived && (
        <div className="dep__arrival">
          <div className="dep__arrival-body">
            <div className="dep__welcome">WELCOME BACK</div>
            <h2 className="dep__name">{name.toUpperCase()}</h2>
            <p className="dep__crew">
              {/*
                The one fact on this screen, and it is measured: who actually lives here,
                read from their own rosters. A World nobody has been assigned to says so
                rather than claiming a crew it does not have.
              */}
              {crew === 0
                ? "Nobody lives here yet. The world is quiet."
                : `${crew} crew ${crew === 1 ? "member is" : "members are"} already aboard.`}
              <br />
              The world is warm. Your chair is where you left it.
            </p>
            <div className="dep__actions">
              <button type="button" className="board dep__enter" onClick={onEnter}>
                ENTER WORLD
                <span className="board__sheen" />
              </button>
              <button type="button" className="btn btn--quiet" onClick={onAbort}>
                BACK TO BRIDGE
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
