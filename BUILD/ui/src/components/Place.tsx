/**
 * A Place, composed and visitable (ADR-0021, ADR-0022).
 *
 * The final visual unit of a World is not an Asset and not a Renderable. It is a Place: a
 * composition of everything drawn, plus the named points that say where its inhabitants
 * stand, where its name sits and what responds to a pointer.
 *
 * This file composes. It does not decide. It does not own interaction state either — it is
 * told whether it is being visited and how much may be revealed, and draws that.
 *
 * Adding a new Place should require no change to this file.
 */

import type { PointerEvent as ReactPointerEvent } from "react";

import type { CharacterView, PlaceView } from "../ipc/contracts";
import { Figure, PERSON_UNIT } from "./Figure";
import { Mark } from "./Mark";

interface PlaceProps {
  readonly place: PlaceView;
  /** Everyone currently here. Real engine state, never decoration. */
  readonly occupants: readonly CharacterView[];
  readonly isVisited: boolean;
  readonly isHovered: boolean;
  /** How much of this Place may be shown: `min(proximity, available depth)` (ADR-0022). */
  readonly reveal: number;
  readonly onVisit: () => void;
  readonly onHover: (hovering: boolean) => void;
  /**
   * Take hold of this building, while the World is stopped for editing.
   *
   * `undefined` outside the editor, and that absence is the whole guard: there is no mode flag
   * to read wrongly and no handler quietly attached to a running World. A building that cannot
   * be picked up simply has nothing to pick it up with.
   */
  readonly onGrab?: (event: ReactPointerEvent) => void;
  /**
   * Take hold of somebody standing here, to give them a home somewhere else.
   *
   * `undefined` outside CREW mode, so a figure is only grabbable when grabbing means something.
   */
  readonly onGrabOccupant?: (characterId: string, event: ReactPointerEvent) => void;
  /**
   * Speak to somebody standing here.
   *
   * Separate from {@link onVisit} because they are different intents: visiting is going to a
   * Place, and this is addressing a person who happens to be standing outside it. The building
   * would otherwise swallow the click, and the crew panel would be the only way to reach anybody
   * — a list, to talk to somebody you can see.
   *
   * `undefined` while the World is stopped for editing, where a click on a figure means moving
   * them instead.
   */
  readonly onTalkTo?: (characterId: string) => void;
  /** Who is currently in hand, and therefore drawn faintly here rather than solidly. */
  readonly carrying?: string | null;
  /** Whether this Place's authored building name is visible. */
  readonly showLabel?: boolean;
  /**
   * What the editor wants said about this Place. `undefined` in a running World.
   *
   * Drawn *around* the artwork — a ring on the ground, a count above it — because a World's art
   * may be a PNG, an SVG or a format nobody has invented, and none of them can be assumed to
   * contain a highlightable door (ADR-0022).
   */
  readonly marker?: {
    readonly selected: boolean;
    readonly roadFrom: boolean;
    /** How many people call this home. `null` when residences are not being shown. */
    readonly residents: number | null;
  };
}

/**
 * Used only when a World declares no anchor for a role.
 *
 * Not a default appearance — a Place with one `visual` and nothing else must still render
 * correctly, or authoring a World becomes an exercise in filling in boilerplate. Any World
 * that cares overrides these by declaring the anchor.
 */
const FALLBACK = {
  spawn: { x: 0.66, y: 0 },
  label: { x: 0, y: 0.42 },
  interaction_bounds: { x: 0, y: -0.45, w: 0.62, h: 0.62 },
} as const;

function anchorOf(place: PlaceView, role: keyof typeof FALLBACK) {
  const declared = place.anchors.find((a) => a.role === role);
  if (!declared) return FALLBACK[role];
  return { x: declared.x, y: declared.y, w: declared.w, h: declared.h };
}

/** The footprint a Place has when its World does not say — `DEFAULT_FOOTPRINT` in the Engine. */
const DEFAULT_FOOTPRINT = 90;

export function Place({
  place,
  occupants,
  isVisited,
  isHovered,
  reveal,
  onVisit,
  onHover,
  onGrab,
  onGrabOccupant,
  onTalkTo,
  carrying,
  showLabel = true,
  marker,
}: PlaceProps) {
  const placement = place.placement;
  if (!placement) return null;

  const w = placement.footprint;
  const spawn = anchorOf(place, "spawn");
  const label = anchorOf(place, "label");
  const bounds = anchorOf(place, "interaction_bounds");
  const halfW = ("w" in bounds && bounds.w ? bounds.w : FALLBACK.interaction_bounds.w) * w;
  const halfH = ("h" in bounds && bounds.h ? bounds.h : FALLBACK.interaction_bounds.h) * w;

  const classes = [
    "place",
    occupants.length > 0 ? "place--inhabited" : null,
    place.isPlaceholder ? "place--unresolved" : null,
    isHovered ? "place--hovered" : null,
    isVisited ? "place--visited" : null,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <g className={classes} transform={`translate(${placement.x} ${placement.y})`}>
      {marker && (marker.selected || marker.roadFrom) && (
        /*
          On the ground, under the building. A highlight over the artwork would cover the thing
          it is highlighting, and a filter over it would depend on what the artwork is.
        */
        <ellipse
          className={marker.selected || marker.roadFrom ? "wed-ring wed-ring--on" : "wed-ring"}
          cx={0}
          cy={4}
          rx={w * 0.55}
          ry={w * 0.24}
        />
      )}

      {place.marks.length > 0 ? (
        place.marks.map((mark, i) => (
          <Mark key={`${mark.role}-${i}`} mark={mark} footprint={w} />
        ))
      ) : (
        // Positioned, but no World said what it looks like. Say so rather than invent one.
        <g className="mark mark--undeclared">
          <rect x={-w / 2} y={-w * 0.6} width={w} height={w * 0.6} />
        </g>
      )}

      {/*
        Inhabitants stand where the Place says they stand — and only *where* comes from the
        Place. Their size does not (see `PERSON_UNIT`): the spawn anchor is declared in
        footprint units, so a big building spaces people further from its door, but nobody grows
        because of the building they are next to.
      */}
      {occupants.map((character, i) => {
        // The anchor comes from the Place; the gap between people comes from the people. A
        // crowd should stay a crowd whatever building it is standing outside.
        const spread = (i - (occupants.length - 1) / 2) * 0.34;
        return (
          <Figure
            key={character.id}
            character={character}
            at={{ x: spawn.x * w + spread * PERSON_UNIT, y: spawn.y * w }}
            faded={carrying === character.id}
            onGrab={onGrabOccupant && ((event) => onGrabOccupant(character.id, event))}
            onTalkTo={onTalkTo && (() => onTalkTo(character.id))}
          />
        );
      })}

      {/*
        The name, at the building's own size.

        It was a constant 30 while footprints in the shipped pack run 90, 110, 125 and 150 — so
        the same label read as oversized on a small building and undersized on a large one, and
        nothing about it belonged to the thing it names. Derived from the footprint it reduces to
        30 at the default, so nothing that was already right moves.

        The visited bump keeps its ratio rather than a second constant; two numbers that have to
        stay in proportion should not be written down twice.
      */}
      {showLabel && (
        <text
          className="place__label"
          x={label.x * w}
          y={label.y * w}
          fontSize={(w / DEFAULT_FOOTPRINT) * (isVisited ? 34 : 30)}
        >
          {place.title}
        </text>
      )}

      {/*
        Reveal level 1. Only reached once the camera has actually arrived, and only carrying
        what is true: what this Place is for, and who is here. Nothing is invented, and there
        is no interior — this is the same Place, showing more of itself.
      */}
      {reveal >= 1 && (
        <g className="place__reveal" transform={`translate(0 ${(label.y + 0.22) * w})`}>
          {place.subtitle && (
            <text className="place__subtitle" x={0} y={0}>
              {place.subtitle}
            </text>
          )}
          <text className="place__occupancy" x={0} y={w * 0.2}>
            {occupants.length === 0
              ? "Nobody is here"
              : occupants.map((o) => `${o.name} - ${o.activity}`).join(" . ")}
          </text>
        </g>
      )}

      {marker?.residents !== null && marker?.residents !== undefined && (
        /* Positioned by the building, sized like a person: an annotation that grew with its
           subject would be unreadable over a small building and overwhelming over a large one. */
        <text className="wed-count" x={0} y={-w * 0.72} fontSize={PERSON_UNIT * 0.2}>
          ⌂ {marker.residents}
        </text>
      )}

      {/*
        What responds to the pointer. The World declares it; the footprint is only a fallback,
        because a Place whose art is taller than its footprint should still be reachable where
        it looks reachable.
      */}
      <rect
        className={onGrab ? "place__target place__target--movable" : "place__target"}
        onPointerDown={onGrab}
        x={bounds.x * w - halfW}
        y={bounds.y * w - halfH}
        width={halfW * 2}
        height={halfH * 2}
        tabIndex={0}
        role="button"
        aria-label={place.subtitle ? `${place.title} - ${place.subtitle}` : place.title}
        onClick={(event) => {
          // Otherwise the click also reaches the World, which reads it as "leave".
          event.stopPropagation();
          onVisit();
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            onVisit();
          }
        }}
        onPointerEnter={() => onHover(true)}
        onPointerLeave={() => onHover(false)}
        onFocus={() => onHover(true)}
        onBlur={() => onHover(false)}
      />
    </g>
  );
}
