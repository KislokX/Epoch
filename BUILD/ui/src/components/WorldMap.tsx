/**
 * The world, drawn in world units.
 *
 * One SVG in world coordinates: terrain, roads, Places and inhabitants all share the same
 * space. That is the whole point — a single coordinate system means the camera later moves
 * *through a world* rather than over a canvas, and everything follows one transform.
 *
 * This file lays out the land and hands each Place to the Place renderer. It knows nothing
 * about what a Place contains, how big its shadow is or where its name sits — including the
 * order they are drawn in, which the engine already decided.
 *
 * It also does not decide what is on screen. The `viewBox` comes from the camera, because the
 * World has no canonical size and the visible area is only a window into it. This file used
 * to weld the two together — `viewBox="0 0 map.width map.height"` — which quietly assumed the
 * whole World was always in view.
 */

import { useState } from "react";
import type { MouseEvent, PointerEvent, RefObject } from "react";

import type { Camera } from "../experience/camera";
import { revealLevel } from "../experience/visit";
import type { CharacterView, MapView, PlaceView } from "../ipc/contracts";
import type { Layers, StageEditing } from "../screens/editor/stage";
import { Figure } from "./Figure";
import { Place } from "./Place";

interface WorldMapProps {
  readonly map: MapView;
  /** Already in draw order: the engine sorts, so every renderer agrees. */
  readonly places: readonly PlaceView[];
  readonly characters: readonly CharacterView[];
  /** What the camera is looking at, in world units. */
  readonly viewBox: string;
  readonly camera: Camera | null;
  readonly surfaceRef: RefObject<SVGSVGElement | null>;
  readonly isDragging: boolean;
  /** False while the camera is travelling. A Place only opens once the World is still. */
  readonly isSettled: boolean;
  readonly onPointerDown: (event: PointerEvent) => void;
  readonly onPointerMove: (event: PointerEvent) => void;
  readonly onPointerUp: (event: PointerEvent) => void;
  /** Which Place the user is at, and which is under the pointer. */
  readonly visiting: string | null;
  readonly hovering: string | null;
  readonly onVisit: (place: PlaceView) => void;
  /**
   * Speak to somebody, by clicking the person rather than the building they stand outside.
   *
   * Passed through to each `Place`, which owns the figures. Only offered to a running World —
   * the editor gives a click on a figure a different meaning.
   */
  readonly onTalkTo?: (characterId: string) => void;
  /**
   * How far along each traveller is, this frame.
   *
   * Passed in rather than derived here, so the World, the minimap and anything else drawing a
   * person all read *one* interpolation. Two would be two answers to where somebody is, and the
   * frame they disagreed on is the frame the user notices.
   *
   * Absent falls back to the Engine's own last word, which is coarse and correct — that is what
   * the editor sees, and a frozen World has nobody walking anyway.
   */
  readonly travelled?: ReadonlyMap<string, number>;
  readonly onHover: (placeId: string | null) => void;
  readonly onLeave: () => void;
  /**
   * The land its owner painted, as a `data:` URI, or `null` for the World's own geography.
   *
   * A prop rather than a field on `MapView` because it is megabytes and the projection is
   * re-sent whenever anybody's presence changes. Fetched once, on mount and after a change.
   */
  readonly land?: string | null;
  /**
   * The editor's intentions about this World. `undefined` unless it is stopped for editing
   * (ADR-0028).
   *
   * Its absence is the guard: a running World is handed no way to be edited at all, rather
   * than being handed one and told not to use it. There is no mode flag for this file to read
   * wrongly, and no ordering between two rules deciding whether the camera still works.
   */
  readonly editing?: StageEditing;
  /** Running World's local visibility controls. They hide drawing only; no World data changes. */
  readonly layers?: Layers;
}

/** Stroke width in world units, by how travelled a road is. */
const ROAD_WIDTH = { major: 26, minor: 14 } as const;

/**
 * Which building a point is over, if any.
 *
 * Nearest within its own footprint, so a big building is a big target and a small one is a
 * small one — the same rule the eye uses. Returns `null` freely: dropping somebody on empty
 * land is a real answer, and it means the gesture was abandoned.
 */
function placeUnder(places: readonly PlaceView[], x: number, y: number): string | null {
  let best: { id: string; d: number } | null = null;
  for (const place of places) {
    const at = place.placement;
    if (!at) continue;
    const d = Math.hypot(at.x - x, at.y - y);
    if (d <= at.footprint * 0.8 && (!best || d < best.d)) best = { id: place.id, d };
  }
  return best?.id ?? null;
}

function polygonPoints(points: readonly (readonly [number, number])[]): string {
  return points.map(([x, y]) => `${x},${y}`).join(" ");
}

/** A journey's reported progress is time, not an instruction to cut across an authored road. */
function pointAlong(
  points: readonly (readonly [number, number])[],
  progress: number,
): { x: number; y: number } | null {
  const first = points[0];
  if (!first || points.length < 2) return null;
  const segments: { from: readonly [number, number]; to: readonly [number, number]; length: number }[] = [];
  for (let index = 0; index + 1 < points.length; index += 1) {
    const from = points[index];
    const to = points[index + 1];
    if (!from || !to) continue;
    segments.push({ from, to, length: Math.hypot(to[0] - from[0], to[1] - from[1]) });
  }
  const total = segments.reduce((sum, segment) => sum + segment.length, 0);
  if (total === 0) return { x: first[0], y: first[1] };
  let left = Math.min(1, Math.max(0, progress)) * total;
  for (const segment of segments) {
    const { from, to, length } = segment;
    if (left > length) {
      left -= length;
      continue;
    }
    const fraction = length === 0 ? 0 : left / length;
    return {
      x: from[0] + (to[0] - from[0]) * fraction,
      y: from[1] + (to[1] - from[1]) * fraction,
    };
  }
  const last = points.at(-1)!;
  return { x: last[0], y: last[1] };
}

function routeBetween(
  routes: MapView["routes"],
  from: { x: number; y: number },
  to: { x: number; y: number },
): readonly (readonly [number, number])[] | null {
  const same = (point: readonly [number, number], at: { x: number; y: number }) =>
    point[0] === at.x && point[1] === at.y;
  for (const route of routes) {
    const start = route.points[0];
    const end = route.points.at(-1);
    if (!start || !end) continue;
    if (same(start, from) && same(end, to)) return route.points;
    if (same(start, to) && same(end, from)) return [...route.points].reverse();
  }
  return null;
}

export function WorldMap({
  map,
  places,
  characters,
  viewBox,
  camera,
  surfaceRef,
  isDragging,
  isSettled,
  onPointerDown,
  onPointerMove,
  onPointerUp,
  visiting,
  hovering,
  onVisit,
  onTalkTo,
  travelled,
  onHover,
  onLeave,
  land,
  editing,
  layers: visibleLayers,
}: WorldMapProps) {
  const layers = editing?.layers ?? visibleLayers;
  /**
   * The building currently in hand, and where it is being held.
   *
   * Held here rather than written to the vault on every pointer move: dragging would otherwise
   * be one file write per frame, and every one of them would re-read and re-project the World.
   * The Engine is told once, on release — which is also the only moment the user has actually
   * decided anything.
   */
  const [held, setHeld] = useState<{ id: string; x: number; y: number } | null>(null);
  /**
   * Somebody picked up to be given a home, and where they are being held.
   *
   * The World is stopped, so nobody actually walks: dropping them changes *where they live*,
   * not where they are standing. Their figure returns to the building they were in and the
   * residence line moves — which is the truthful animation of what happened, and the reason
   * this is not modelled as movement.
   */
  const [carried, setCarried] = useState<{ id: string; x: number; y: number } | null>(null);

  /**
   * Screen pixels to world units, through the SVG's own matrix.
   *
   * Not derived from the viewBox by hand. The camera zooms and `preserveAspectRatio` letterboxes,
   * so the scale factor and the offset both change; asking the element what transform it is
   * actually using cannot drift from what the user is looking at.
   */
  const worldPoint = (event: { clientX: number; clientY: number }) => {
    const svg = surfaceRef.current;
    const screen = svg?.getScreenCTM();
    if (!svg || !screen) return null;
    const point = svg.createSVGPoint();
    point.x = event.clientX;
    point.y = event.clientY;
    const world = point.matrixTransform(screen.inverse());
    return { x: Math.round(world.x), y: Math.round(world.y) };
  };
  // Everyone standing at each Place, so a building can show it is occupied.
  const occupants = new Map<string, CharacterView[]>();
  for (const character of characters) {
    // **Somebody walking is nobody's occupant.** They have left and not arrived, so the Place
    // they set out from would otherwise keep drawing them standing outside it while a second
    // copy of them walks away down the road.
    if (character.journey) continue;
    const list = occupants.get(character.place);
    if (list) list.push(character);
    else occupants.set(character.place, [character]);
  }

  return (
    <svg
      ref={surfaceRef}
      className={`worldmap${isDragging ? " worldmap--dragging" : ""}${
        visiting ? " worldmap--visiting" : ""
      }`}
      viewBox={viewBox}
      preserveAspectRatio="xMidYMid meet"
      role="presentation"
      onClick={(event: MouseEvent) => {
        // A click on the land itself means "I am looking at the World again". A Place stops
        // its own click from getting here.
        if (event.defaultPrevented) return;
        // In the editor it means the opposite of a selection: nothing is selected now.
        if (editing) {
          if (editing.mode === "road" && editing.roadFrom) {
            const at = worldPoint(event);
            if (at) editing.onRoadCorner(at);
            return;
          }
          editing.onSelect({ kind: "none" });
          return;
        }
        onLeave();
      }}
      onPointerDown={onPointerDown}
      onPointerMove={(event) => {
        const at = worldPoint(event);
        editing?.onCursor(at);
        // Something in hand moves instead of the camera. Panning while dragging would move
        // both, and whatever is held would appear to stick to the land.
        if (held) {
          if (at) setHeld({ id: held.id, ...at });
          return;
        }
        if (carried) {
          if (at) setCarried({ id: carried.id, ...at });
          return;
        }
        onPointerMove(event);
      }}
      onPointerUp={(event) => {
        if (held) {
          editing?.onMovePlace(held.id, held.x, held.y);
          setHeld(null);
          return;
        }
        if (carried) {
          // Dropped on a building, or dropped on nothing. Nothing is a real answer — it means
          // the gesture was abandoned, not that they now live in a field.
          //
          // Dropping somebody back on the building they already live in is also nothing: a
          // stray click in CREW mode would otherwise register as an edit, take an undo slot,
          // and appear in History as something the user did.
          const onto = placeUnder(places, carried.x, carried.y);
          const already = characters.find((c) => c.id === carried.id)?.home;
          if (onto && onto !== already) editing?.onAssignHome(carried.id, onto);
          setCarried(null);
          return;
        }
        // Building happens on release, and only when the gesture was not a pan — otherwise
        // moving the camera would leave a trail of buildings behind it.
        //
        // And never on top of a building that is already there. A Place's own click is stopped
        // from reaching the World, but a pointer event is not, so without this a click meant to
        // select something in PLACE mode would put a second building through it.
        if (editing?.mode === "place" && !isDragging) {
          const at = worldPoint(event);
          if (at && !placeUnder(places, at.x, at.y)) {
            editing.onBuildAt(at.x, at.y);
            onPointerUp(event);
            return;
          }
        }
        onPointerUp(event);
      }}
      onPointerCancel={(event) => {
        // Cancelled, so nothing was decided: whatever was held goes back, because the vault
        // was never told it had moved.
        setHeld(null);
        setCarried(null);
        editing?.onCursor(null);
        onPointerUp(event);
      }}
    >
      {/*
        Terrain.

        Painted land **replaces** the declared areas rather than sitting under them — the same
        rule imported artwork follows for a building (ADR-0024), and for the same reason: two
        terrains in one place is two terrains. The derived chart never goes away; it is one CLEAR
        behind, and it was what was being drawn a moment ago.

        Confined to the World's own extent, and drawn with `<image>` rather than inlined, so
        imported artwork cannot execute script (ADR-0020).
      */}
      <g className="worldmap__terrain">
        {(layers?.terrain ?? true) && land && (
          <image
            href={land}
            x={0}
            y={0}
            width={map.width}
            height={map.height}
            preserveAspectRatio="none"
          />
        )}
        {(layers?.terrain ?? true) &&
          !land &&
          map.terrain.map((area, i) => (
          <polygon
            key={`${area.kind}-${i}`}
            className={`terrain terrain--${area.kind}`}
            points={polygonPoints(area.points)}
          />
          ))}
      </g>

      {/* Roads. A worn road says people travel here often. */}
      <g className="worldmap__routes">
        {(layers?.roads ?? true) &&
          map.routes.map((route, i) => (
          <polyline
            key={i}
            className={`route route--${route.prominence}`}
            points={polygonPoints(route.points)}
            strokeWidth={ROAD_WIDTH[route.prominence]}
          />
          ))}
      </g>

      {editing?.mode === "road" && editing.roadFrom && (
        /* A draft is visual feedback only. It is not written until a destination is chosen. */
        <polyline
          className="wed-road-draft"
          points={polygonPoints([
            (() => {
              const from = places.find((place) => place.id === editing.roadFrom)?.placement;
              return [from?.x ?? 0, from?.y ?? 0] as const;
            })(),
            ...editing.roadVia,
            ...(() => {
              const to = editing.roadTo
                ? places.find((place) => place.id === editing.roadTo)?.placement
                : null;
              return to ? ([[to.x, to.y] as const]) : [];
            })(),
          ])}
        />
      )}

      {layers?.grid && (
        /*
          A grid in world units, for lining things up. Derived from the World's own extent
          rather than from a tile size — Epoch has no tiles, and inventing one here would put a
          measurement on screen that nothing else in the system agrees with.
        */
        <g className="wed-grid">
          {Array.from({ length: Math.ceil(map.width / 100) + 1 }, (_, i) => (
            <line key={`v${i}`} x1={i * 100} y1={0} x2={i * 100} y2={map.height} />
          ))}
          {Array.from({ length: Math.ceil(map.height / 100) + 1 }, (_, i) => (
            <line key={`h${i}`} x1={0} y1={i * 100} x2={map.width} y2={i * 100} />
          ))}
        </g>
      )}

      <g className="worldmap__places">
        {(layers?.buildings ?? true) &&
          places.map((place) => {
          const at = place.placement;
          return (
          <Place
            key={place.id}
            /*
              While it is in hand, it is drawn where the hand is. One shallow copy per frame,
              and the projection underneath is untouched — so letting go without a drop, or a
              refusal from the Engine, leaves the World showing the truth it already had.
            */
            place={
              held?.id === place.id && at
                ? { ...place, placement: { ...at, x: held.x, y: held.y } }
                : place
            }
            occupants={
              (layers?.characters ?? true) ? (occupants.get(place.id) ?? []) : []
            }
            isVisited={visiting === place.id}
            isHovered={hovering === place.id}
            reveal={revealLevel(
              visiting,
              place.id,
              camera,
              at?.footprint ?? 0,
              isSettled,
            )}
            onVisit={() => {
              if (!editing) return onVisit(place);
              // A click on a building means different things per tool, and exactly one of
              // them. This is the whole reason the rail exists.
              if (editing.mode === "road") editing.onRoadPick(place.id);
              else editing.onSelect({ kind: "place", id: place.id });
            }}
            onHover={(hovering) => onHover(hovering ? place.id : null)}
            onGrab={
              // Only SELECT picks a building up. In ROAD or CREW a drag on a building would
              // silently move it while the user believed they were doing something else.
              editing?.mode === "select" && at
                ? (event) => {
                    // Primary button only, the same rule the camera follows. Without it a
                    // right-drag over a building moved it while the user was reaching for a
                    // context menu — an edit nobody asked for, recorded in History as one.
                    if (event.button !== 0) return;
                    // The World must not also read this as a pan.
                    event.stopPropagation();
                    const grabbed = worldPoint(event);
                    setHeld({ id: place.id, x: grabbed?.x ?? at.x, y: grabbed?.y ?? at.y });
                  }
                : undefined
            }
            onGrabOccupant={
              editing?.mode === "crew"
                ? (characterId, event) => {
                    if (event.button !== 0) return;
                    event.stopPropagation();
                    const grabbed = worldPoint(event);
                    setCarried({
                      id: characterId,
                      x: grabbed?.x ?? at?.x ?? 0,
                      y: grabbed?.y ?? at?.y ?? 0,
                    });
                  }
                : undefined
            }
            onTalkTo={editing ? undefined : onTalkTo}
            carrying={carried?.id ?? null}
            /* Layout's Labels layer is precisely the authored name of each building. */
            showLabel={layers?.labels ?? true}
            marker={
              editing
                ? {
                    selected:
                      editing.selection.kind === "place" && editing.selection.id === place.id,
                    roadFrom: editing.roadFrom === place.id || editing.roadTo === place.id,
                    residents: editing.showResidences
                      ? characters.filter((c) => c.home === place.id).length
                      : null,
                  }
                : undefined
            }
          />
          );
          })}
      </g>

      {/*
        THE ROAD.

        Drawn after the buildings, so somebody walking in front of one is in front of it. Their
        position is the Engine's — a fraction along a line between two Places it chose — smoothed
        between authoritative states by `useTravel`, which derives rather than guesses.

        A walk whose ends are not both placed is not drawn. That is not a failure: an unplaced
        Place is a World that has not been finished, and the Engine will not have sent anybody
        there in the first place.
      */}
      <g className="worldmap__travellers">
        {(layers?.characters ?? true) &&
          characters.map((person) => {
            const journey = person.journey;
            if (!journey) return null;
            const from = places.find((p) => p.id === journey.from)?.placement;
            const to = places.find((p) => p.id === journey.to)?.placement;
            if (!from || !to) return null;
            const along = travelled?.get(person.id) ?? journey.progress;
            const path = routeBetween(map.routes, from, to);
            const at = path ? pointAlong(path, along) : null;
            return (
              <Figure
                key={person.id}
                character={person}
                at={at ?? {
                  x: from.x + (to.x - from.x) * along,
                  y: from.y + (to.y - from.y) * along,
                }}
                onTalkTo={onTalkTo && !editing ? () => onTalkTo(person.id) : undefined}
              />
            );
          })}
      </g>

      {editing && (
        /*
          What the editor adds *around* the World, never inside it (ADR-0022). A World's
          artwork may be a PNG, an SVG or a format that does not exist yet, and none of them can
          be assumed to contain a highlightable door — so everything the editor says is drawn on
          the ground beside the art.
        */
        <g className="worldmap__editing">
          {editing.showResidences &&
            characters.map((person) => {
              // Only worth a line when the two differ. Drawing one from a building to itself
              // would be a mark that carries no information, on every character, always.
              if (person.home === person.place) return null;
              const from = places.find((p) => p.id === person.place)?.placement;
              const to = places.find((p) => p.id === person.home)?.placement;
              if (!from || !to) return null;
              return (
                <line
                  key={person.id}
                  className="wed-link"
                  x1={from.x}
                  y1={from.y}
                  x2={to.x}
                  y2={to.y}
                />
              );
            })}

          {carried && (
            /*
              Who is in hand. A ring rather than their sprite: they have not moved — the World
              is stopped — and dragging their body across the map would say they had.
            */
            <g className="wed-ghost" transform={`translate(${carried.x} ${carried.y})`}>
              <circle className="wed-ring wed-ring--on" r={38} />
              <text className="wed-count" y={-48}>
                {characters.find((c) => c.id === carried.id)?.name ?? ""}
              </text>
            </g>
          )}
        </g>
      )}
    </svg>
  );
}
