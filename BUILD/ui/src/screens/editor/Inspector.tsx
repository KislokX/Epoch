/**
 * What is selected, and what can be done to it.
 *
 * ## One inspector, three shapes
 *
 * The reference design's best structural idea: not a screen per thing, but one panel that
 * shows exactly what matters to the current selection — and an honest empty state when nothing
 * is selected. That is why the editor needs no navigation.
 *
 * ## Where it and Epoch disagree, Epoch is right
 *
 * The design offered `NAME` / `IDENTITY` / `SUBTITLE` as three text fields. In Epoch identity
 * is not a word you type — it is the thing that never changes when you rename, which is the
 * whole reason ADR-0028 exists. So it is shown and never editable, right under the name, so
 * renaming visibly does not move it.
 *
 * The design also had `SCALE` × `RADIUS` × `GROUND OFFSET`. A Place has exactly one size here
 * (`footprint`) on purpose — "there is deliberately no second multiplier, so *how big is this
 * Place* has one answer". Interaction radius is an anchor the World declares (ADR-0021), which
 * is what lets a Place whose art is taller than its footprint still be reachable where it looks
 * reachable; it is reported, not overridden.
 *
 * And its `SPRITE` picker offered seven hardcoded building types. The engine references
 * concepts and a World Pack resolves them (ADR-0016) — a frontend that knows what a "keep"
 * looks like is the same mistake as a frontend that knows Ollama has `num_ctx`. What replaces
 * it is the honest version of the same gesture: import your own artwork.
 */

import { useState } from "react";

import { ImageDrop } from "../../components/ImageDrop";
import type { CharacterView, PlaceView, Road, WorldMap } from "../../ipc/contracts";
import { LookPanel, type LookPanelProps } from "./LookPanel";
import { Chip, Cold, Field, Panel, Row, Slider, TextInput } from "./Panel";
import type { Layers } from "./stage";
import type { EditorApi } from "./useEditor";

const LAYERS: readonly (keyof Layers)[] = [
  "terrain",
  "roads",
  "buildings",
  "characters",
  "labels",
  "grid",
];

export interface InspectorProps {
  readonly editor: EditorApi;
  readonly places: readonly PlaceView[];
  readonly crew: readonly CharacterView[];
  readonly map: WorldMap | null;
  readonly onRename: (id: string, name: string) => void;
  readonly onResize: (id: string, footprint: number) => void;
  readonly onDelete: (id: string) => void;
  readonly onDisconnect: (from: string, to: string) => void;
  /** Start re-tracing the exact route in the centre stage; it is not a second map editor. */
  readonly onTraceRoad: (road: Road) => void;
  readonly onPut: (id: string) => void;
  readonly onScale: (characterId: string, scale: number | null) => void;
  /**
   * Say where one of somebody's routine activities happens here. `null` is at home.
   *
   * The *only* way a character ever walks on their own: the Simulation carries out declared
   * routine rules and invents none, so if nobody makes this choice, nobody wanders (ADR-0018).
   */
  readonly onRoutinePlace: (characterId: string, activity: string, placeId: string | null) => void;
  /** Give a building artwork, or take it away. Resolves to an error message, or null. */
  readonly onArt: (placeId: string, dataUri: string | null) => Promise<string | null>;
  /** Where a new building lands. `null` when the World has no land yet. */
  readonly centre: { readonly x: number; readonly y: number } | null;
  readonly layers: Layers;
  readonly onLayers: (layers: Layers) => void;
  /** Paint the land, or strip it back. Resolves to an error message, or null. */
  readonly onLand: (dataUri: string | null) => Promise<string | null>;
  /** Whether the user has painted any. */
  readonly hasLand: boolean;
  /** Everything the LOOK mode needs, passed through rather than rebuilt here. */
  readonly look: LookPanelProps;
}

export function Inspector(props: InspectorProps) {
  const { editor } = props;
  return (
    <>
      {/*
        The tool you picked is the first thing you see.

        These sat under the Inspector, which takes the leftover height — so switching to TERRAIN
        put its one control below the fold of a full-height panel and the mode looked like it did
        nothing. A mode panel is the answer to the click that opened it, so it goes first.
      */}
      {editor.mode === "terrain" && (
        <Panel title="The Land">
          {/*
            Painted land replaces the World's declared terrain rather than sitting under it —
            the same rule a building's artwork follows, and for the same reason: two terrains in
            one place is two terrains. The derived chart is one CLEAR behind and never goes away
            (ADR-0024), so a World with no painting is complete rather than unfinished.

            Stretched to the World's extent, so what lines up in your image lines up with where
            the buildings actually stand.
          */}
          <div className="pxgrid pxgrid--2" style={{ margin: 0 }}>
            <ImageDrop
              label={props.hasLand ? "Replace the land" : "Paint the land"}
              className="pxbtn"
              onChoose={props.onLand}
              onClear={props.hasLand ? () => props.onLand(null) : undefined}
            />
          </div>
          <Cold>
            {props.hasLand
              ? "CLEAR brings back the terrain this World declared. It was never removed."
              : "With none, the World draws its own geography — which is a complete answer, not a gap."}
          </Cold>
          <Cold>
            Windows are yours now — see LOOK. Sky, weather and music still come from the World
            Pack (ADR-0016) and are still authored by hand.
          </Cold>
        </Panel>
      )}

      {editor.mode === "look" && <LookPanel {...props.look} />}

      {editor.mode === "quest" && (
        <Panel title="Quest Templates">
          <Cold>
            A Quest is the unit of work (ADR-0025): intent, participants, approvals, evidence.
            Authoring lifecycles is not built yet.
          </Cold>
        </Panel>
      )}

      <Roads {...props} />

      {/*
        An authoring filter, and never a claim about the World: turning buildings off does not
        remove them and nothing downstream is told they are gone. It exists because arranging
        roads under five buildings is easier with the buildings out of the way.
      */}

      <Panel title="Inspector" right={<span className="pxlabel">{editor.selection.kind}</span>} grow>
        <Body {...props} />
      </Panel>
      <Panel title="Layers">
        <div className="pxgrid pxgrid--2" style={{ margin: 0 }}>
          {LAYERS.map((layer) => (
            <button
              key={layer}
              type="button"
              className={`pxbtn${props.layers[layer] ? " pxbtn--on" : ""}`}
              onClick={() => props.onLayers({ ...props.layers, [layer]: !props.layers[layer] })}
            >
              {props.layers[layer] ? "◉" : "○"} {layer}
            </button>
          ))}
        </div>
      </Panel>
    </>
  );
}

/**
 * The map's authored roads, named in one place.
 *
 * A line on the map can be difficult to select when roads cross; the list makes ownership and
 * destructive action explicit. It deliberately contains only vault roads: pack geography is a
 * World Pack authoring concern and cannot be silently removed from a running World.
 */
function Roads({ map, places, onDisconnect, onTraceRoad }: InspectorProps) {
  const roads = Object.entries(map?.roads ?? {});
  const title = <span className="pxlabel">{roads.length} route{roads.length === 1 ? "" : "s"}</span>;
  const nameOf = (id: string) => places.find((place) => place.id === id)?.title ?? id;

  return (
    <Panel title="Roads" right={title}>
      {roads.length === 0 ? (
        <Cold>None traced yet. Choose ROAD, click a building, trace any corners, then click its destination.</Cold>
      ) : (
        <div className="wed__roads">
          {roads.map(([id, road]) => {
            // `via` was added after straight roads already existed. An old file's absent field
            // is the straight route it always meant, not a malformed editable record.
            const corners = road.via ?? [];
            return (
              <div key={id} className="wed__road">
                <button
                  type="button"
                  className="wed__road-trace"
                  title="Re-trace this route on the map"
                  onClick={() => onTraceRoad(road)}
                >
                  <span>{nameOf(road.from)} <b>→</b> {nameOf(road.to)}</span>
                  <small>{corners.length === 0 ? "straight" : `${corners.length} corner${corners.length === 1 ? "" : "s"}`}</small>
                </button>
                <button
                  type="button"
                  className="pxbtn pxbtn--danger wed__road-remove"
                  aria-label={`Delete road from ${nameOf(road.from)} to ${nameOf(road.to)}`}
                  title="Delete this road"
                  onClick={() => onDisconnect(road.from, road.to)}
                >
                  ×
                </button>
              </div>
            );
          })}
        </div>
      )}
    </Panel>
  );
}

function Body({
  editor,
  places,
  crew,
  map,
  onRename,
  onResize,
  onDelete,
  onDisconnect,
  onTraceRoad,
  onPut,
  onScale,
  onRoutinePlace,
  onArt,
  centre,
}: InspectorProps) {
  const selection = editor.selection;
  /**
   * Where a slider is, before it is let go.
   *
   * One slot rather than one per control: only one slider can be under a pointer at a time, and
   * two slots would be two things that could disagree about which drag is in progress.
   */
  const [pending, setPending] = useState<{
    what: "footprint" | "height";
    value: number;
  } | null>(null);

  if (selection.kind === "place") {
    const place = places.find((p) => p.id === selection.id);
    if (!place) return <p className="pxnote">That building is gone.</p>;

    const residents = crew.filter((c) => c.home === place.id);
    const here = crew.filter((c) => c.place === place.id);
    const roads = Object.values(map?.roads ?? {}).filter(
      (r) => r.from === place.id || r.to === place.id,
    );
    /* Read from the vault rather than from the projection: what matters here is whether the
       *user* supplied a drawing, not whether the building has one from any source. */
    const hasArt = Boolean(map?.places[place.id]?.mark);

    return (
      <div>
        <Field label="Name">
          <TextInput value={place.title} onCommit={(v) => onRename(place.id, v)} />
        </Field>

        {/*
          Shown, never editable. Seeing that this does not change when the name does is what
          makes the rest of the editor comprehensible — a road points here, not at the sign on
          the door, and so does every Quest in History.
        */}
        <Field label="Identity — never changes">
          <TextInput value={place.id} readOnly />
        </Field>

        <Row k="Kind" v={place.concept ?? <span style={{ color: "var(--ed-quiet)" }}>none</span>} />
        {!place.concept && (
          <Cold>
            A building with no kind exists, is visited and holds residents. It simply lights up
            for no subsystem — which is an honest thing for a building you invented to be.
          </Cold>
        )}
        {place.subtitle && <Row k="Subtitle" v={place.subtitle} />}

        {place.placement ? (
          <>
            <Row k="Standing at" v={`${Math.round(place.placement.x)} , ${Math.round(place.placement.y)}`} />
            <Field label="Building size">
              {/*
                The pending value lives here, and only the release reaches the Engine. Dragging
                a slider that wrote on every movement would be one vault write per frame, and
                each of them re-reads and re-projects the whole World.
              */}
              <Slider
                value={pending?.what === "footprint" ? pending.value : place.placement.footprint}
                min={40}
                max={260}
                step={5}
                suffix="u"
                onChange={(v) => setPending({ what: "footprint", value: v })}
                onCommit={() => {
                  if (pending?.what === "footprint") onResize(place.id, pending.value);
                  setPending(null);
                }}
              />
            </Field>
            <Cold>
              Moves the building and its name. Not the people standing there — how tall somebody
              is has nothing to do with which building they are next to.
            </Cold>
          </>
        ) : (
          <>
            <Row k="Standing at" v={<span style={{ color: "var(--ed-warn)" }}>nowhere</span>} />
            <button
              type="button"
              className="pxbtn"
              style={{ width: "100%", marginTop: 6 }}
              disabled={!centre}
              title={centre ? "Bring it into the World" : "This World has no land to put it on yet"}
              onClick={() => onPut(place.id)}
            >
              ⌖ Put it in the World
            </button>
          </>
        )}

        <Field label={`Interaction radius — declared by the World`}>
          <TextInput
            readOnly
            value={
              place.anchors.find((a) => a.role === "interaction_bounds")
                ? `${Math.round(
                    (place.anchors.find((a) => a.role === "interaction_bounds")?.w ?? 0) *
                      (place.placement?.footprint ?? 0) *
                      2,
                  )} u`
                : "from the footprint"
            }
          />
        </Field>

        <div style={{ paddingTop: 6 }}>
          <span className="pxlabel">Residents ({residents.length})</span>
          <div className="pxchips">
            {residents.length ? (
              residents.map((c) => (
                <Chip key={c.id} tone="leaf">
                  {c.name}
                </Chip>
              ))
            ) : (
              <span className="pxnote">empty halls</span>
            )}
          </div>
          {here.length > 0 && here.some((c) => c.home !== place.id) && (
            <Cold>
              {here
                .filter((c) => c.home !== place.id)
                .map((c) => c.name)
                .join(", ")}{" "}
              {here.filter((c) => c.home !== place.id).length === 1 ? "is" : "are"} here but
              lives elsewhere.
            </Cold>
          )}
        </div>

        <div style={{ paddingTop: 6 }}>
          <span className="pxlabel">Connections ({roads.length})</span>
          <div className="pxchips">
            {roads.length ? (
              roads.map((road) => {
                const other = road.from === place.id ? road.to : road.from;
                return (
                  <button
                    key={`${road.from}-${road.to}`}
                    type="button"
                    className="pxchip"
                    style={{ cursor: "pointer" }}
                    title="Take this road down"
                    onClick={() => onDisconnect(road.from, road.to)}
                  >
                    {places.find((p) => p.id === other)?.title ?? other} ✕
                  </button>
                );
              })
            ) : (
              <span className="pxnote">no roads lead here</span>
            )}
          </div>
        </div>

        <div style={{ paddingTop: 6 }}>
          <span className="pxlabel">Artwork</span>
          {/*
            The reference design offered seven building types to pick from. The Engine knows no
            building types — a World Pack resolves what a Place looks like (ADR-0016) — so the
            honest version of that gesture is: bring your own.

            Authored artwork wins the building and the pack's drawing stays one CLEAR away
            (ADR-0024). The frontend never touches disk: bytes go base64 across IPC and the
            Engine names the file from the bytes.
          */}
          <div className="pxgrid pxgrid--2" style={{ marginTop: 4 }}>
            <ImageDrop
              label={hasArt ? "Replace" : "Import a drawing"}
              className="pxbtn"
              onChoose={(data) => onArt(place.id, data)}
              onClear={hasArt ? () => onArt(place.id, null) : undefined}
            />
          </div>
          {!hasArt && (
            <Cold>
              With none, the building is drawn the way this World's pack drew it — which is a
              complete answer, not a gap.
            </Cold>
          )}
        </div>

        <div className="pxgrid pxgrid--2" style={{ marginTop: 10 }}>
          <button
            type="button"
            className="pxbtn"
            title="Switch to SELECT and drag it on the map"
            onClick={() => editor.setMode("select")}
            style={{ gridColumn: "1 / -1" }}
          >
            ✥ Move it on the map
          </button>
          <button
            type="button"
            className="pxbtn pxbtn--danger pxgrid__wide"
            title="Take it down, and every road that led to it"
            onClick={() => onDelete(place.id)}
          >
            ✕ Delete place
          </button>
        </div>
      </div>
    );
  }

  if (selection.kind === "character") {
    const person = crew.find((c) => c.id === selection.id);
    if (!person) return <p className="pxnote">They are not in this World.</p>;
    const nameOf = (id: string) => places.find((p) => p.id === id)?.title ?? id;

    return (
      <div>
        <Row k="Name" v={person.name} />
        <Row k="Archetype" v={person.archetype} />
        <Row k="Home" v={nameOf(person.home)} />
        <Row k="Standing in" v={nameOf(person.place)} />
        <Row k="Doing" v={person.activity} />
        <Row k="Class" v={person.class} />

        {person.mark && (
          <Field label="Height">
            <Slider
              value={pending?.what === "height" ? pending.value : person.mark.scale}
              min={0.05}
              max={1.5}
              suffix="×"
              onChange={(v) => setPending({ what: "height", value: v })}
              onCommit={() => {
                if (pending?.what === "height") onScale(person.id, pending.value);
                setPending(null);
              }}
            />
          </Field>
        )}
        <Cold>
          A scale, never a resize — the file they imported keeps every pixel it arrived with.
        </Cold>

        {/*
          WHERE THEIR ROUTINE HAPPENS.

          The routine itself belongs to them and travels into every World; where each step
          happens is a fact about *this* World and is decided here (ADR-0028) — the same split
          `Home` above already makes.

          It is also the whole of authoring idle movement. The Simulation carries out declared
          routine rules and invents nothing, so a World where nobody chose anything is a World
          where nobody wanders — which is a complete World, not an unfinished one.
        */}
        {person.routine.length > 0 && (
          <div style={{ paddingTop: 10 }}>
            <span className="pxlabel">Routine</span>
            <ul className="wed__routine">
              {person.routine.map((step) => (
                <li key={step.activity}>
                  <span className="wed__routine-what" title={`${step.seconds}s`}>
                    {step.activity}
                  </span>
                  <select
                    aria-label={`Where ${person.name} does "${step.activity}"`}
                    value={step.place ?? ""}
                    onChange={(e) =>
                      onRoutinePlace(person.id, step.activity, e.target.value || null)
                    }
                  >
                    {/* Home is not a Place chosen from the list — it is the absence of a
                        choice, and it has to read that way or clearing one becomes impossible. */}
                    <option value="">at home</option>
                    {places
                      .filter((p) => p.placement)
                      .map((p) => (
                        <option key={p.id} value={p.id}>
                          {p.title}
                        </option>
                      ))}
                  </select>
                </li>
              ))}
            </ul>
            <Cold>
              They walk there when that step comes round, and home again after — never on their
              own, and only if the walk fits inside the step.
            </Cold>
          </div>
        )}

        <button
          type="button"
          className="pxbtn"
          style={{ width: "100%", marginTop: 8 }}
          onClick={() => onScale(person.id, null)}
          title="Back to the height the rest of the crew stands at"
        >
          Reset height
        </button>
      </div>
    );
  }

  if (selection.kind === "road") {
    const road = Object.values(map?.roads ?? {}).find(
      (r) => `${r.from}-${r.to}` === selection.id,
    );
    if (!road) return <p className="pxnote">That road is gone.</p>;
    const from = places.find((p) => p.id === road.from);
    const to = places.find((p) => p.id === road.to);
    const length =
      from?.placement && to?.placement
        ? Math.round(Math.hypot(from.placement.x - to.placement.x, from.placement.y - to.placement.y))
        : null;

    return (
      <div>
        <Row k="Length" v={length === null ? "—" : `${length} u`} />
        <div style={{ paddingTop: 6 }}>
          <span className="pxlabel">Joins</span>
          <div className="pxchips">
            <Chip>{from?.title ?? road.from}</Chip>
            <Chip>{to?.title ?? road.to}</Chip>
          </div>
        </div>
        <Cold>
          A road is how the World shows a handoff happening. It never decides whether one may
          happen — scenery must not be able to veto work.
        </Cold>
        <button
          type="button"
          className="pxbtn"
          style={{ width: "100%", marginTop: 10 }}
          onClick={() => onTraceRoad(road)}
        >
          Trace route
        </button>
        <button
          type="button"
          className="pxbtn pxbtn--danger"
          style={{ width: "100%", marginTop: 10 }}
          onClick={() => onDisconnect(road.from, road.to)}
        >
          ✕ Delete road
        </button>
      </div>
    );
  }

  return (
    <p className="pxnote">
      Nothing selected. Click a building, a road or somebody on the map — this panel only ever
      shows what matters to what you picked.
    </p>
  );
}
