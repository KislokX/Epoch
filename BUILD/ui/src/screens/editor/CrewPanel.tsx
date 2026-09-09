/**
 * The crew, and who they are.
 *
 * ## Where the real instruments are, they are connected
 *
 * The reference design mocked this panel: `provider: "Local Bridge"`, `temperature: 0.7`,
 * `context: 32768`, capabilities as decorative chips. Every one of those is a real, measured
 * value in Epoch — the resolved backend, the canonical Character parameters (ADR-0026), the
 * measured context window. So they are read from the character's own definition instead of
 * invented, and where a value genuinely has no answer yet the panel says so.
 *
 * The mock is still doing useful work: it decided the *shape* — identity first, appearance
 * second, model third, and everything else behind **Advanced**, because the default is that the
 * user touches nothing (ADR-0026).
 *
 * ## Residence is the editor's business; the rest belongs to the character
 *
 * Where somebody lives is per-World and is the one thing on this panel that ADR-0028 put in the
 * editor's hands. Name, face, prompt and parameters travel with the character into every World
 * (ADR-0026), so they are edited in the Character panel that already exists and are shown here
 * read-only — one editor for a fact, and it is not this one.
 */

import type { CharacterView, PlaceView } from "../../ipc/contracts";
import { Chip, Cold, Field, Panel, Row } from "./Panel";
import type { EditorApi } from "./useEditor";

export function CrewPanel({
  editor,
  crew,
  places,
  minds,
  onAssign,
}: {
  readonly editor: EditorApi;
  readonly crew: readonly CharacterView[];
  readonly places: readonly PlaceView[];
  /** Which model each character resolved to, when one is known. Measured, never assumed. */
  readonly minds: Readonly<Record<string, string | null>>;
  readonly onAssign: (characterId: string, placeId: string | null) => void;
}) {
  const selectedId = editor.selection.kind === "character" ? editor.selection.id : null;
  const selected = crew.find((c) => c.id === selectedId) ?? null;
  const nameOf = (id: string) => places.find((p) => p.id === id)?.title ?? id;

  return (
    <>
      <Panel
        title="Crew"
        right={<span className="pxlabel">{crew.length} souls</span>}
        cap="42%"
      >
        {crew.length === 0 ? (
          <p className="pxnote">Nobody lives in this World yet.</p>
        ) : (
          <ul className="wed__crew">
            {crew.map((person) => (
              <li key={person.id}>
                <button
                  type="button"
                  className={`wed__soul${selectedId === person.id ? " wed__soul--on" : ""}`}
                  onClick={() => editor.select({ kind: "character", id: person.id })}
                >
                  <Face person={person} />
                  <span>
                    <span className="pxtitle">{person.name}</span>
                    <em>{nameOf(person.home)}</em>
                    {/*
                      Their real activity, from the simulation. The World is stopped, so this is
                      what they were doing when it stopped — not a loop of invented verbs.
                    */}
                    <i>{person.activity}</i>
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </Panel>

      <Panel title={selected ? `General · ${selected.name}` : "General"} grow>
        {!selected ? (
          <p className="pxnote">
            Select a crew member to see who they are and where they live.
          </p>
        ) : (
          <div>
            <Row k="Name" v={selected.name} />
            <Row k="Archetype" v={selected.archetype} />
            <Row k="Standing in" v={nameOf(selected.place)} />
            <Row
              k="Model"
              v={
                minds[selected.id] ? (
                  <span style={{ color: "var(--ed-cyan)" }}>{minds[selected.id]}</span>
                ) : (
                  <span style={{ color: "var(--ed-quiet)" }}>—</span>
                )
              }
            />

            {/*
              The one per-World fact on this panel, and therefore the only editable one here.
              Everything above travels with the character into every World (ADR-0026), so it is
              edited where it belongs — one editor per fact.
            */}
            <Field label="Residence">
              <select
                className="pxinput pxinput--mono"
                value={selected.home}
                onChange={(e) => onAssign(selected.id, e.target.value || null)}
                onKeyDown={(e) => e.stopPropagation()}
              >
                <option value="">— not settled —</option>
                {places.map((place) => (
                  <option key={place.id} value={place.id}>
                    {place.title}
                  </option>
                ))}
              </select>
            </Field>
            <p className="pxnote">
              Or switch to CREW and drag them onto a building.
            </p>

            <div style={{ paddingTop: 8 }}>
              <span className="pxlabel">Appearance</span>
              <div className="pxchips">
                {selected.mark ? <Chip tone="leaf">sprite</Chip> : <Chip>no sprite</Chip>}
                {selected.icon ? <Chip tone="leaf">portrait</Chip> : <Chip>no portrait</Chip>}
              </div>
              <Cold>
                Faces and sprites are imported from the character's own panel, so they travel
                into every World with them.
              </Cold>
            </div>

            <div style={{ paddingTop: 8 }}>
              <span className="pxlabel">Animations</span>
              <div className="pxgrid pxgrid--4">
                {["idle", "walk", "think", "work"].map((clip) => (
                  <button
                    key={clip}
                    type="button"
                    className="pxbtn"
                    disabled
                    title="Animated sprites are not built yet — a character is one still frame today."
                    style={{ fontSize: 6 }}
                  >
                    ▶ {clip}
                  </button>
                ))}
              </div>
            </div>
          </div>
        )}
      </Panel>
    </>
  );
}

/**
 * A face, or a visible stand-in — never a generated likeness (ADR-0024).
 *
 * Somebody who has not chosen a portrait gets their initial in a frame. It is unmistakably a
 * label rather than a face, which is the point: inventing a face for somebody is the same lie
 * as inventing a crew.
 */
function Face({ person }: { readonly person: CharacterView }) {
  const art = person.icon?.asset ?? person.mark?.asset ?? null;
  return (
    <div className="pxinset wed__face" aria-hidden>
      {art ? <img src={art} alt="" /> : person.name.slice(0, 1).toUpperCase()}
    </div>
  );
}
