/**
 * What the World renderer needs to know while it is being edited (ADR-0028).
 *
 * ## One object, and its absence is the guard
 *
 * A running World is handed `undefined` here. It is not handed a set of handlers and asked not
 * to use them, and there is no `isEditing` flag for a component to read wrongly: a World that
 * cannot be edited has nothing to edit it with. That is the same shape that made dragging safe
 * and it generalises to every gesture the editor adds later.
 *
 * ## Why this lives beside the editor and not beside the renderer
 *
 * The renderer draws a World. These are the editor's intentions *about* a World. Keeping the
 * type here means the renderer imports one thing from the editor and the editor never has to
 * grow a second opinion about how a Place looks.
 */

import type { EditMode, Selection } from "./useEditor";

/**
 * What the editor is currently drawing.
 *
 * An authoring filter, not a claim about the World: turning buildings off does not remove them,
 * and nothing downstream is told they are gone. It exists because arranging roads under five
 * buildings is easier with the buildings out of the way — the same reason a drawing program has
 * layers, and for the same duration.
 */
export interface Layers {
  readonly terrain: boolean;
  readonly roads: boolean;
  readonly buildings: boolean;
  readonly characters: boolean;
  readonly labels: boolean;
  /** The world-unit grid, for lining things up. */
  readonly grid: boolean;
}

export const ALL_LAYERS: Layers = {
  terrain: true,
  roads: true,
  buildings: true,
  characters: true,
  labels: true,
  grid: false,
};

export interface StageEditing {
  readonly mode: EditMode;
  readonly layers: Layers;
  readonly selection: Selection;
  /** The building a road is being drawn from, if one is. */
  readonly roadFrom: string | null;
  /** The fixed destination while an existing road is being re-traced. */
  readonly roadTo: string | null;
  /** The intermediate world-unit corners of the road currently being traced. */
  readonly roadVia: readonly (readonly [number, number])[];
  /** Draw who lives where: a line from each person to their home, and a count per building. */
  readonly showResidences: boolean;
  /** Somebody clicked a thing. */
  readonly onSelect: (selection: Selection) => void;
  /** A building was put down somewhere, in world units. Written on release, never per frame. */
  readonly onMovePlace: (placeId: string, x: number, y: number) => void;
  /** Empty land was clicked in PLACE mode. */
  readonly onBuildAt: (x: number, y: number) => void;
  /** A building was clicked in ROAD mode — first end, or second. */
  readonly onRoadPick: (placeId: string) => void;
  /** Empty ground was clicked while tracing a road: keep that authored corner. */
  readonly onRoadCorner: (at: { x: number; y: number }) => void;
  /** Somebody was dropped on a building in CREW mode. */
  readonly onAssignHome: (characterId: string, placeId: string) => void;
  /** The pointer moved, in world units. Drives the status bar's readout. */
  readonly onCursor: (at: { x: number; y: number } | null) => void;
}
