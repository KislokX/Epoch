/** The World’s authored Places as real keyboard-reachable navigation. */

import type { PlaceView } from "../../ipc/contracts";
import { ALL_LAYERS, type Layers } from "../../screens/editor/stage";
import { Frame } from "./Frame";
import { PixelIcon } from "./Pixel";
import type { Glyph } from "./Pixel";

interface WorldDockProps {
  readonly places: readonly PlaceView[];
  readonly visitedId: string | null;
  readonly peopleAt: (place: PlaceView) => number;
  readonly glyphFor: (place: PlaceView) => Glyph;
  readonly onVisit: (place: PlaceView) => void;
  /** Visibility is a local layout preference; it never changes the World being shown. */
  readonly layers?: Layers;
  readonly onLayers?: (layers: Layers) => void;
}

/** A dock is navigation, so each authored Place is a button rather than a decorative shortcut. */
const LAYERS: readonly (keyof Layers)[] = [
  "terrain",
  "roads",
  "buildings",
  "characters",
  "labels",
  "grid",
];

export function WorldDock({
  places,
  visitedId,
  peopleAt,
  glyphFor,
  onVisit,
  layers = ALL_LAYERS,
  onLayers = () => {},
}: WorldDockProps) {
  return (
    <Frame className="hud__dock" corner={10} fill="var(--ep-window)">
      <div className="hud__dock-body">
        <div className="hud__dock-travel">
          <span className="hud__dock-title">Travel</span>
          {places.length === 0 ? (
            <p className="hud__empty">This World has nowhere to go yet.</p>
          ) : (
            places.map((place) => {
              const people = peopleAt(place);
              const active = visitedId === place.id;
              return (
                <button
                  key={place.id}
                  type="button"
                  className={`epbtn${active ? " epbtn--on" : ""}`}
                  onClick={() => onVisit(place)}
                  title={place.subtitle ?? undefined}
                >
                  <PixelIcon glyph={glyphFor(place)} size={14} tone={active ? "gold" : "parchment"} />
                  <span>{place.title}</span>
                  {people > 0 && <span className="hud__dock-count">{people}</span>}
                </button>
              );
            })
          )}
        </div>
        <div className="hud__dock-layout" aria-label="Layout visibility">
          <span className="hud__dock-title">Layout</span>
          <div className="hud__dock-layers">
            {LAYERS.map((layer) => (
              <button
                key={layer}
                type="button"
                className={`epbtn${layers[layer] ? " epbtn--on" : ""}`}
                aria-pressed={layers[layer]}
                onClick={() => onLayers({ ...layers, [layer]: !layers[layer] })}
              >
                {layer}
              </button>
            ))}
          </div>
        </div>
      </div>
    </Frame>
  );
}
