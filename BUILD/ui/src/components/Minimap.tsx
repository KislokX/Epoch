/**
 * Where you are inside the World.
 *
 * Not decoration, and not a control panel. The camera shows a slice of a World that has no
 * canonical size — a village today, a city once someone's ecosystem has grown — so something
 * has to answer "how much more is out there?". Without this, moving the camera *loses* the
 * geography the World authored; with it, the geography is communicated better than seeing
 * everything at once ever did.
 *
 * Entirely derived. Same terrain, same Places, same world units, drawn small. No new assets,
 * nothing authored twice, and nothing here can disagree with the World because there is no
 * second copy of anything.
 */

import type { Frame } from "../experience/camera";
import type { CharacterView, MapView, PlaceView } from "../ipc/contracts";
import { Mark } from "./Mark";

interface MinimapProps {
  readonly map: MapView;
  readonly places: readonly PlaceView[];
  /** What the camera can currently see, in world units. */
  readonly view: Frame | null;
  /** Looking somewhere on the minimap takes the camera there. */
  readonly onLookAt: (at: { x: number; y: number }) => void;
  /**
   * Drawn inside a HUD panel rather than floating in the World's corner.
   *
   * The minimap was its own overlay before the HUD existed. It has a home now, and the
   * positioning that made it float would fight the panel it sits in.
   */
  readonly inline?: boolean;
  /**
   * The land its owner painted, if any.
   *
   * Without it the minimap drew only the declared terrain polygons — and a World whose land is
   * a picture has none, so the panel went black while the World it was a map *of* was full of
   * detail. A minimap showing something the World does not is a bug; a minimap showing nothing
   * the World does is the same bug with the sign flipped.
   */
  readonly land?: string | null;
  /**
   * Everyone in the World, so the small map shows who is where.
   *
   * Optional: a minimap of an empty World is a map of an empty World, not a broken one.
   */
  readonly characters?: readonly CharacterView[];
  /**
   * How far along each traveller is, this frame — the same map the World is drawn from.
   *
   * Without it somebody walking sat at the building they left until they arrived, so the World
   * and the minimap disagreed about where a person was for thirty seconds at a time.
   */
  readonly travelled?: ReadonlyMap<string, number>;
}

/**
 * What a person's height is a fraction of — the same reference the World uses.
 *
 * Duplicated here rather than imported from `Place`, which owns rendering a *Place*. Both read
 * the Engine's `DEFAULT_FOOTPRINT`; a shared constant module for one number would be more
 * indirection than the number is worth, and the comment is the link.
 */
const PERSON_UNIT = 90;

function polygonPoints(points: readonly (readonly [number, number])[]): string {
  return points.map(([x, y]) => `${x},${y}`).join(" ");
}

/**
 * Where somebody on the road is, in world units — or `null` if they are not on one.
 *
 * The same straight line between two Places the World draws, read from the same interpolation,
 * so the two pictures cannot disagree about where a person is.
 */
function walkingTo(
  person: CharacterView,
  places: readonly PlaceView[],
  travelled: ReadonlyMap<string, number> | undefined,
): { x: number; y: number; footprint: number } | null {
  const journey = person.journey;
  if (!journey) return null;
  const from = places.find((p) => p.id === journey.from)?.placement;
  const to = places.find((p) => p.id === journey.to)?.placement;
  if (!from || !to) return null;
  const along = travelled?.get(person.id) ?? journey.progress;
  return {
    x: from.x + (to.x - from.x) * along,
    y: from.y + (to.y - from.y) * along,
    footprint: from.footprint,
  };
}

export function Minimap({
  map,
  places,
  view,
  onLookAt,
  inline,
  land,
  characters = [],
  travelled,
}: MinimapProps) {
  // A Place's dot is sized from its footprint, so the minimap says the same thing the World
  // says: Places are not uniform, and scale is information.
  const dot = (place: PlaceView) => Math.max(map.width * 0.008, place.placement!.footprint * 0.6);

  return (
    <svg
      className={`minimap${inline ? " minimap--inline" : ""}`}
      viewBox={`0 0 ${map.width} ${map.height}`}
      preserveAspectRatio="xMidYMid meet"
      aria-label="Where you are in the World"
      onPointerDown={(event) => {
        const box = event.currentTarget.getBoundingClientRect();
        // The svg is letterboxed by `meet`, so convert through the rendered scale rather than
        // assuming the element and the World share proportions.
        const scale = Math.min(box.width / map.width, box.height / map.height);
        const inset = {
          x: (box.width - map.width * scale) / 2,
          y: (box.height - map.height * scale) / 2,
        };
        onLookAt({
          x: (event.clientX - box.left - inset.x) / scale,
          y: (event.clientY - box.top - inset.y) / scale,
        });
      }}
    >
      <rect className="minimap__ground" x={0} y={0} width={map.width} height={map.height} />

      {/*
        Painted land replaces the declared areas here exactly as it does in the World — same
        rule, so the small map and the big one cannot end up disagreeing about what the ground
        is. The image is already a `data:` URI the World fetched once; this draws the same
        string rather than asking for it again.
      */}
      {land && (
        <image
          href={land}
          x={0}
          y={0}
          width={map.width}
          height={map.height}
          preserveAspectRatio="none"
        />
      )}

      {!land &&
        map.terrain.map((area, i) => (
          <polygon
            key={`${area.kind}-${i}`}
            className={`minimap__terrain terrain--${area.kind}`}
            points={polygonPoints(area.points)}
          />
        ))}

      {/*
        Buildings, as they actually look.

        The same `Mark` the World draws, at the same footprint, so the small map cannot end up
        disagreeing with the big one about what a building is — which was the whole problem
        with dots: a World whose Places are drawings was represented by circles that shared
        nothing with it.

        A dot remains the answer for a Place nothing has drawn yet. It is not a fallback for a
        drawing that failed — `Mark` handles that itself — it is what "nobody has said what this
        looks like" looks like from a distance.
      */}
      {places
        .filter((place) => place.placement !== null)
        .map((place) =>
          place.marks.length > 0 ? (
            <g
              key={place.id}
              className="minimap__mark"
              transform={`translate(${place.placement!.x} ${place.placement!.y})`}
            >
              {place.marks.map((mark, i) => (
                <Mark key={`${mark.role}-${i}`} mark={mark} footprint={place.placement!.footprint} />
              ))}
            </g>
          ) : (
            <circle
              key={place.id}
              className="minimap__place"
              cx={place.placement!.x}
              cy={place.placement!.y}
              r={dot(place)}
            />
          ),
        )}

      {/*
        And the crew, standing where they are.

        Drawn at the same reference height the World uses, so somebody is the same size here
        relative to the land as they are there. Position comes from their Place — the minimap
        does not know where a spawn anchor is, and it does not need to: being *at* the building
        is the fact worth showing at this size.
      */}
      {characters.map((person, i) => {
        const at = walkingTo(person, places, travelled)
          ?? places.find((p) => p.id === person.place)?.placement;
        if (!at) return null;
        // Two people at one building would draw on top of each other. Spread them by the same
        // amount the World does, so a crowd reads as a crowd. Somebody on the road is not in a
        // crowd, so they are not spread.
        const beside = person.journey
          ? [person]
          : characters.filter((c) => !c.journey && c.place === person.place);
        const spread = (beside.indexOf(person) - (beside.length - 1) / 2) * 30;
        return person.mark ? (
          <g
            key={person.id}
            className="minimap__soul"
            transform={`translate(${at.x + spread} ${at.y})`}
          >
            <Mark mark={person.mark} footprint={PERSON_UNIT} />
          </g>
        ) : (
          <circle
            key={person.id}
            className="minimap__soul-dot"
            cx={at.x + spread}
            cy={at.y - 12}
            r={Math.max(map.width * 0.004, 8)}
          />
        );
        void i;
      })}

      {/* You are here. The World continues past every edge of this rectangle. */}
      {view && (
        <rect
          className="minimap__view"
          x={view.x}
          y={view.y}
          width={view.width}
          height={view.height}
        />
      )}
    </svg>
  );
}
