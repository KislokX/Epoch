/**
 * A person, drawn wherever they are.
 *
 * ## Why this left `Place`
 *
 * Everybody used to be standing outside a building, so the only code that could draw a person
 * was the code that drew buildings. Travel makes that false: somebody halfway between the
 * Laboratory and the Library belongs to neither, and the two ways of drawing a person would have
 * started life identical and drifted — one of them getting the label size fix, the other not.
 *
 * So a figure is a figure. Its caller says *where*; everything about how somebody is drawn is
 * here, once.
 *
 * ## What it will not do
 *
 * It does not decide where anybody is, and it never animates on its own. The Engine owns
 * reality; the UI owns animation (ADR-0018) — and animation here means interpolating between
 * two states the Engine has already committed to, which happens in `useTravel`, above this.
 */

import type { PointerEvent as ReactPointerEvent } from "react";

import type { CharacterView } from "../ipc/contracts";
import { Mark } from "./Mark";

/**
 * The height a character stands at when nobody has chosen one — `CHARACTER_SCALE` in the Kernel.
 *
 * Here only to keep the label's appearance exactly: every derived size below is written so that
 * at this height it produces the number it produced before this was a component.
 */
const DEFAULT_HEIGHT = 0.34;

/**
 * What a person's height is a fraction *of*.
 *
 * A fixed reference rather than the building they happen to be beside: a character's `scale` is
 * a fraction, and when the thing it multiplied was the footprint next to them, somebody standing
 * by a large building was drawn large and walking to a small one shrank them. Their height
 * changed because of where they were, which is not a fact about them.
 *
 * It matters twice as much now that people walk: between two buildings there is no footprint to
 * be a fraction of at all.
 */
export const PERSON_UNIT = 90;

interface FigureProps {
  readonly character: CharacterView;
  /** Where they stand, in world units. The caller owns this; the figure owns everything else. */
  readonly at: { readonly x: number; readonly y: number };
  /** Drawn faintly, because they are currently in somebody's hand in the editor. */
  readonly faded?: boolean;
  /**
   * Pick them up, while the World is stopped for editing.
   *
   * `undefined` outside CREW mode, and that absence is the guard: a figure that cannot be moved
   * has nothing attached to move it.
   */
  readonly onGrab?: (event: ReactPointerEvent) => void;
  /**
   * Speak to them. Never offered at the same time as {@link onGrab} — a click on a person is
   * never ambiguous about which of the two it meant.
   */
  readonly onTalkTo?: () => void;
}

export function Figure({ character, at, faded, onGrab, onTalkTo }: FigureProps) {
  /*
    What to draw them as, right now.

    **The action first, the still picture second, the stand-in last** — ADR-0016's chain, with
    one more link than it had. The Engine already said which action this is; picking the sheet
    is a lookup rather than a judgement, and a character who has been drawn walking but not
    working keeps their still sprite for work rather than losing their face.
  */
  const body = character.actions[character.action] ?? character.mark;

  /*
    How tall this person stands, as a fraction of the reference height — which is exactly what
    `Mark` multiplies by to draw them. Reading it from the mark rather than storing it again
    means the label can never disagree with the sprite it belongs to.
  */
  const stands = body?.scale ?? DEFAULT_HEIGHT;
  const unit = PERSON_UNIT;

  return (
    <g
      /*
        `figure--played` is the switch that stops the World laying a gait over somebody else's
        drawing. Without a sheet the CSS bob **is** the walk, and it stays — that is the whole
        fallback. With one, the artwork already contains the gait, and adding ours would make
        somebody bob to a rhythm their own animation is not keeping.
      */
      className={`figure figure--${character.class}${character.journey ? " figure--walking" : ""}${body?.frames ? " figure--played" : ""}`}
      transform={`translate(${at.x} ${at.y})`}
      opacity={faded ? 0.35 : undefined}
    >
      <ellipse className="figure__shadow" cx={0} cy={0} rx={unit * 0.1} ry={unit * 0.035} />
      {/*
        The target sits above the sprite so the whole person is reachable regardless of what
        their artwork happens to be — the same reason a Place is hit-tested by a declared region
        rather than by its pixels (ADR-0022).
      */}
      {onGrab && (
        <rect
          className="place__target place__target--movable"
          x={-unit * 0.09}
          y={-unit * stands}
          width={unit * 0.18}
          height={unit * stands}
          onPointerDown={onGrab}
          onClick={(event) => event.stopPropagation()}
        />
      )}
      {!onGrab && onTalkTo && (
        <rect
          className="place__target place__target--person"
          x={-unit * 0.09}
          y={-unit * stands}
          width={unit * 0.18}
          height={unit * stands}
          tabIndex={0}
          role="button"
          aria-label={`Speak to ${character.name}`}
          onClick={(event) => {
            // The building underneath would otherwise take this as a Visit, and standing next
            // to a Place is not the same as being it.
            event.stopPropagation();
            onTalkTo();
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter" || event.key === " ") {
              event.preventDefault();
              onTalkTo();
            }
          }}
        >
          <title>{`Speak to ${character.name}`}</title>
        </rect>
      )}
      {/*
        Keyed on the activity so React remounts these when it changes, replaying their
        animations. The change is a real transition — the Engine only publishes when presence
        actually changes.
      */}
      {body ? (
        // They brought their own face (ADR-0023). Same Mark, same renderer, same fallbacks a
        // Place gets — and a sheet plays through the same one, so animated artwork is not a
        // second pipeline either.
        //
        // Keyed on the action rather than the sentence: swapping *which drawing* is on screen
        // is a remount, and re-keying every time the words changed would restart a walk cycle
        // for a character whose activity text moved on.
        <g key={`body-${character.action}`} className="figure__body">
          <Mark
            mark={body}
            footprint={unit}
            // Measured by the Engine from the leg being walked. Absent when they are standing,
            // because standing has never measured a facing.
            facing={character.journey?.facing}
          />
        </g>
      ) : (
        // Nobody said what they look like. A visible stand-in, never an invention.
        <rect
          key={`body-${character.action}`}
          className="figure__body figure__body--undeclared"
          x={-unit * 0.055}
          y={-unit * 0.23}
          width={unit * 0.11}
          height={unit * 0.23}
          rx={unit * 0.052}
        />
      )}
      {/*
        Above **their** head, at **their** size — both derived from how tall this person actually
        stands, so a character shrunk to half size does not wear a caption meant for somebody
        twice as tall.

        This does not undo the deliberate choice in `world.css` that the note is not scaled with
        the camera: from far away you should perceive that somebody is there, not read what they
        are doing. That is about the camera; this is about the person.
      */}
      <text
        key={`note-${character.activity}`}
        className="figure__activity"
        x={0}
        y={-unit * (stands + 0.12)}
        fontSize={unit * stands * 0.5}
      >
        {character.activity}
      </text>
    </g>
  );
}
