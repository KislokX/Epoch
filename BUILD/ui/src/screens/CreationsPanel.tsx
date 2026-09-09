import { useEffect, useRef, useState } from "react";

import {
  attachWorkflow,
  buildWorkflow,
  drawUsually,
  drawOn,
  forgetWorkflow,
  imageShelf,
  importWorkflow,
  testStudio,
  type ImageShelf,
  type StudioCheck,
  type StyleRow,
} from "../ipc/launcher";
import { ImageStudios } from "../components/ImageStudios";
import { VoiceEngines } from "../components/VoiceEngines";
import { weighMeasured } from "../lib/weigh";

/**
 * What this World can make.
 *
 * ## Its own deck, and not called Images
 *
 * It lived in Settings, which answers *choices about your own machine* — and a whole making
 * subsystem under that heading is the drift this file has already fixed twice: several answers
 * stacked under a title that names none of them.
 *
 * *Images* was the obvious name and would have been wrong within two phases. Measured on this
 * machine's ComfyUI: **165 video nodes, 47 audio, 36 for 3D**. The same workflows, the same
 * Styles and the same import will carry all of it (Phase 12), so the deck is named for what it
 * does rather than for the one medium that works first.
 *
 * ## Three things, in the order somebody meets them
 *
 * **The engine** — is ComfyUI here, is it answering. Nothing below matters until it is.
 * **The Styles** — the words a character can ask for, and whether anything serves them.
 * **The workflows** — what this machine actually has, what each can do, and what it needs.
 *
 * ## A Style with nothing behind it keeps its frame
 *
 * The Launcher's rule. It says what would light it up and it never quietly borrows another
 * Style's workflow — answering *pixel art* with a realistic picture is a different answer to a
 * different question, delivered confidently.
 */
/**
 * What each medium is called, once.
 *
 * The same words appear on a button, on a row and in the sentence a shelf shows when nothing on
 * it makes the thing being asked for. Three spellings of one fact agree by luck.
 */
const MEDIUM_LABEL: Record<string, string> = {
  all: "EVERYTHING",
  picture: "IMAGE",
  video: "VIDEO",
  sound: "AUDIO",
  model: "3D",
};

const MEDIUM_WORDS: Record<string, string> = {
  all: "anything",
  picture: "a picture",
  video: "a video",
  sound: "a sound",
  model: "a model",
};

export function CreationsPanel() {
  const [shelf, setShelf] = useState<ImageShelf | null>(null);
  /**
   * Which medium the library list is showing.
   *
   * User intent, held here and nowhere else — what each file makes is measured by the Engine and
   * arrives on the row. `all` is the state somebody opens this in, because a deck that starts
   * filtered hides things from a reader who never chose to filter.
   */
  const [medium, setMedium] = useState<
    "all" | "picture" | "video" | "sound" | "model"
  >("all");
  const [asking, setAsking] = useState(true);
  const [said, setSaid] = useState<string | null>(null);
  /** Which Style is open for attaching, by name. User intent only. */
  const [attaching, setAttaching] = useState<string | null>(null);
  /** Which workflow is being told what it draws, by id, and the name being typed. */
  const [naming, setNaming] = useState<string | null>(null);
  const [styleName, setStyleName] = useState("");
  /** The last walk of the picture chain, and whether one is running. */
  const [walked, setWalked] = useState<readonly StudioCheck[] | null>(null);
  const [walking, setWalking] = useState(false);
  const file = useRef<HTMLInputElement>(null);

  const read = () => {
    setAsking(true);
    void imageShelf().then((found) => {
      setShelf(found);
      setAsking(false);
    });
  };

  useEffect(read, []);

  /**
   * The window reads the bytes; the Engine writes the file.
   *
   * ADR-0024, unchanged: this has no path, no dialog plugin and no filesystem access. What
   * crosses is base64 and a name to show it under.
   */
  const take = async (chosen: File) => {
    setSaid(null);
    const data = await chosen.arrayBuffer().then((buffer) => {
      let binary = "";
      const bytes = new Uint8Array(buffer);
      for (const byte of bytes) binary += String.fromCharCode(byte);
      return btoa(binary);
    });
    const name = chosen.name.replace(/\.json$/i, "");
    const failure = await importWorkflow(name, data);
    setSaid(failure ?? `Took in ${name}. Attach it to a Style to use it.`);
    read();
  };

  /**
   * Walk it, and say so while it is walking.
   *
   * A real picture is drawn, so this is seconds of GPU work. The button says what it is doing
   * rather than spinning: a person who knows a model is loading waits, and a person watching a
   * spinner wonders whether it is stuck.
   */
  const walk = async () => {
    setWalking(true);
    setWalked(null);
    try {
      setWalked(await testStudio());
    } catch (why) {
      setWalked([
        { step: "Testing this studio", ok: false, said: String(why) },
      ]);
    }
    setWalking(false);
  };

  /**
   * Let Epoch write one.
   *
   * The whole reason this exists: importing is seven steps across two applications, and
   * somebody who has never opened ComfyUI could not draw at all until they had learned it.
   */
  const build = async () => {
    setSaid(null);
    try {
      const name = await buildWorkflow();
      setSaid(`Built ${name} and attached it to General. Ask a character for a picture.`);
    } catch (why) {
      setSaid(String(why));
    }
    read();
  };

  const attach = async (style: string, id: string, on: boolean) => {
    setSaid(await attachWorkflow(style, id, on));
    setAttaching(null);
    read();
  };

  /**
   * Say what a workflow draws, which is the act that creates the Style.
   *
   * The same call as attaching: from the Engine's side there is no difference between pointing
   * an existing Style at another workflow and naming a new one, and there should not be. One
   * decision, one place.
   */
  const name = async (id: string) => {
    const wanted = styleName.trim();
    if (wanted.length === 0) return;
    setSaid(await attachWorkflow(wanted, id, true));
    setNaming(null);
    setStyleName("");
    read();
  };

  return (
    <section className="pnl bay">
      <div className="bay__head">
        <div>
          <div className="bay__heading">
            <span className="lx__gem" aria-hidden />
            <h2>CREATIONS</h2>
          </div>
          <p className="bay__sub">
            What this World can make, and what it makes it with.
          </p>
        </div>
        <div className="bay__acts">
          <button
            type="button"
            className="btn btn--ghost"
            disabled={walking}
            title="Draw one picture, and report every step it took to get there"
            onClick={() => void walk()}
          >
            {walking ? "DRAWING…" : "TEST THIS STUDIO"}
          </button>
          <button
            type="button"
            className="btn btn--ghost"
            disabled={asking}
            onClick={read}
          >
            {asking ? "ASKING…" : "ASK AGAIN"}
          </button>
        </div>
      </div>
      <div className="bay__rule" />

      {/*
        ## Two columns, and the deck’s own sentence says which two

        At 2,560 this deck was **2,120 pixels tall** in one column with about 1,200 pixels empty
        beside it — six blocks stacked down a strip, so the last of them had to be scrolled past
        everything above it to reach.

        **Balancing across columns is still refused**, for the reason it was refused across whole
        decks and again in the docking bay: it decides where the eye starts, and this deck has a
        reading order that was argued over with the owner — the picture chain to its end, then the
        machines that serve it, then speaking. A masonry layout shuffles exactly that.

        So the split is by **meaning**, and the subtitle had already named it: *what this World can
        make, and what it makes it with.* Left is what can run — the studio, the machines that lend
        a card, the three programs that speak. Right is what it works with — the library, the
        Styles, the workflows. Order survives inside each column, because each column is the same
        list it was.

        Measured rather than eyeballed, and the balance is a consequence rather than the aim:
        201 + 134 + 520 against 563 + 313 + 104 — **855 and 980**. Had the honest split been
        lopsided it would have shipped lopsided.

        **`VoiceEngines` leaves the shelf’s guard on the way**, which is a fix rather than a side
        effect: it takes no props and asks its own questions, and it was inside that guard only by
        position — so speaking was hidden while a *picture* shelf was being read.
      */}

      <div className="cr__floor">
        <div className="cr__runs">
          {/*
            The engine first: nothing below it means anything until this answers.

            **And Speaking is not here any more** (owner, 2026-09-06). It sat between the studio and
            the machines that run it, so the deck read *pictures → speech → back to pictures* — his
            words: *"Speaking aparece antes de terminar de explicar el ecosistema creativo y
            posteriormente vuelve a imágenes."*

            His own mock-up kept the old order, and the sentence beside it did not. The sentence is
            the measurement: it says what reading the screen felt like. So the picture chain runs to
            its end — studio, then the machines that serve it — and Speaking follows, before the
            library, which is the first thing on this deck that genuinely belongs to both.
          */}
          <ImageStudios />

          {/*
            The walk, when there has been one. Every failure mode of this chain is silent, so the
            list is the instrument — and it stops at the first break because everything after a
            break is noise.
          */}
          {walked && (
            <ul className="rdy__list">
              {walked.map((check) => (
                <li
                  key={check.step}
                  className={`rdy__row rdy__row--${check.ok ? "ready" : "needsYou"}`}
                >
                  <span className="rdy__dot" aria-hidden />
                  <span className="rdy__name">{check.step}</span>
                  <span className="rdy__note">{check.said}</span>
                </li>
              ))}
            </ul>
          )}

          <div className="bay__rule" />

          {shelf !== null && shelf.problem && (
            <p className="notice notice--warn">{shelf.problem}</p>
          )}

          {shelf !== null && shelf.benches.length > 1 && (
                <div className="rm">
                  <span className="rm__label">Where pictures are made</span>
                  <p className="cc__hint">
                    A machine that lends its card can draw. Epoch says which ones can;
                    it never moves the work on its own.
                  </p>
                  <ul className="rdy__list">
                    {shelf.benches.map((bench) => (
                      <li
                        key={bench.id || "here"}
                        className={`rdy__row rdy__row--${bench.serving ? "ready" : "needsYou"}`}
                      >
                        <span className="rdy__dot" aria-hidden />
                        <span className="rdy__name">{bench.name}</span>
                        <span className="rdy__note">
                          {bench.serving
                            ? bench.models.length > 0
                              ? bench.models.join(", ")
                              : // Running perfectly and unable to make a picture. The same
                                // sentence wherever the machine is: it is the distinction that
                                // cost an afternoon, and it does not get cheaper at a distance.
                                "serving with nothing to load"
                            : bench.here
                              ? "not running here"
                              : "not serving ComfyUI when it was last asked"}
                        </span>
                        {shelf.drawOn === bench.id ? (
                          <span className="cc__hint">draws here</span>
                        ) : (
                          // An offer only where it can be taken. A disabled button looks
                          // like a live one at a glance, and the row already says why this
                          // machine cannot draw — a dead control adds nothing but a thing
                          // to press twice.
                          bench.serving && (
                            <button
                              type="button"
                              className="btn btn--mini"
                              title="Make pictures on this machine"
                              onClick={() =>
                                void drawOn(bench.id).then((failure) => {
                                  setSaid(failure);
                                  read();
                                })
                              }
                            >
                              DRAW HERE
                            </button>
                          )
                        )}
                      </li>
                    ))}
                  </ul>
                </div>
          )}

              <div className="bay__rule" />

              {/*
                Speaking, after the picture chain has finished rather than in the middle of it. Its
                own subject and its own three programs — a mouth, an ear, and the forge that turns a
                downloaded voice into one Epoch can use.
              */}
              <VoiceEngines />
        </div>

        {shelf === null ? (
          // Unasked is not empty.
          <p className="cc__hint">Asking this machine…</p>
        ) : (
          <div className="cr__stock">
                <div className="rm">
                  <span className="rm__label">The library</span>
                  <p className="cc__hint">
                    Where Epoch keeps what it downloads. It tells ComfyUI where this is;
                    it never moves, copies or renames a file you already had.
                  </p>
                  <p className="cc__hint">
                    <code>{shelf.library.root}</code>
                  </p>
                  <p className={shelf.library.restart ? "notice notice--warn" : "cc__hint"}>
                    {shelf.library.note}
                  </p>

                  {/*
                    **What each shelf holds, filtered by what it makes.**

                    The list was one joined sentence per shelf — every file, its kind, its family and
                    its size, run together with middots — which is readable with four files and
                    unreadable with forty. And there was no way to ask the question somebody actually
                    has, which is *what on this machine makes a video*.

                    The medium comes from `Understood::makes`: a family measured out of a model's own
                    tensors first, and a part's own convolutions where there is no family to have. The
                    panel groups its VAEs from the same reading.
                  */}
                  {/*
                    **A dropdown, like the one over the search.** One filter with two shapes on one
                    deck is two things to learn about one idea, and this is the shape the owner chose.
                  */}
                  <div className="rdy__filter">
                    <label className="cedit__field cedit__field--inline">
                      <span>SHOWING</span>
                      <select
                        value={medium}
                        onChange={(e) =>
                          setMedium(e.target.value as typeof medium)
                        }
                      >
                        {(["all", "picture", "video", "sound", "model"] as const).map(
                          (which) => (
                            <option key={which} value={which}>
                              {MEDIUM_LABEL[which]}
                            </option>
                          ),
                        )}
                      </select>
                    </label>
                  </div>
                  {/*
                    **Unplaced files are shown under every medium, and marked.**

                    A file Epoch could not read is the user's, and there is nothing they can do about a
                    failure of Epoch's. A filter that dropped it would take somebody's own model off
                    the screen with nothing on screen to explain it — the worst thing this control has
                    available.
                  */}
                  {/*
                    **Its own class, because this is the one list where balancing is right.**

                    Two columns were tried across whole decks and taken out again: multicol balances by
                    content height, and a deck is one big block and several small ones, so it put a
                    heading alone in the first column and everything else in the second.

                    Eleven shelves of the same shape are the opposite case — similar blocks, which is
                    precisely what balancing is for. `.rdy__list` is also the walked chain above and
                    the Readiness list one deck over, and neither of those is eleven of anything, so
                    the rule needs a name rather than a selector that happens to match.
                  */}
                  <ul className="rdy__list rdy__list--racks">
                    {shelf.library.shelves.map((rack) => {
                      const held = rack.held.filter(
                        (it) =>
                          medium === "all" ||
                          // Unplaced, and a part that belongs to every medium. Both stay.
                          it.medium === null ||
                          it.medium === "any" ||
                          it.medium === medium,
                      );
                      return (
                        <li
                          key={rack.id}
                          className={`rdy__row rdy__row--${held.length > 0 ? "ready" : "needsYou"}`}
                        >
                          <span className="rdy__dot" aria-hidden />
                          <span className="rdy__name">{rack.name}</span>
                          <span className="rdy__note">
                            {rack.held.length === 0 ? (
                              // An empty shelf is not a broken one. It says there is nowhere
                              // else these could be hiding.
                              "nothing here yet"
                            ) : held.length === 0 ? (
                              `nothing here makes ${MEDIUM_WORDS[medium]}`
                            ) : (
                              <ul className="rdy__files">
                                {held.map((it) => (
                                  <li key={it.file}>
                                    {it.file}
                                    <span className="cc__hint">
                                      {" — "}
                                      {it.kind}, {it.base}, {weighMeasured(it.bytes)}
                                      {it.medium === null
                                        ? " · Epoch could not tell what it makes"
                                        : it.medium === "any"
                                          ? " · used with any medium"
                                          : ` · makes ${MEDIUM_WORDS[it.medium]}`}
                                    </span>
                                  </li>
                                ))}
                              </ul>
                            )}
                          </span>
                        </li>
                      );
                    })}
                  </ul>
                </div>

                <div className="bay__rule" />

                <div className="rm">
                  <span className="rm__label">Advanced · Styles</span>
                  <p className="cc__hint">
                    What a character asks for &mdash; <em>pixel art</em>, never a filename. A Style
                    exists because a workflow below draws it, so this list is what this World can
                    actually make. Take the last workflow away and the Style goes with it.
                  </p>
                  <p className="cc__hint">
                    These two are how pictures were made before the library existed, and they are
                    still the only way to use a graph Epoch cannot build itself. Once the panel can
                    compile one from what you have, nothing here is required.
                  </p>

                  <ul className="rdy__list">
                    {shelf.styles.map((style) => (
                      <li
                        key={style.name}
                        className={`rdy__row rdy__row--${style.lit ? "ready" : "needsYou"}`}
                      >
                        <span className="rdy__dot" aria-hidden />
                        <span className="rdy__name">{style.name}</span>
                        <span className="rdy__note">{describe(style, shelf)}</span>
                        {style.name === shelf.usually && (
                          <span className="cc__hint">usually</span>
                        )}
                        <button
                          type="button"
                          className="btn btn--mini"
                          onClick={() =>
                            setAttaching(attaching === style.name ? null : style.name)
                          }
                        >
                          {attaching === style.name ? "CLOSE" : "ATTACH…"}
                        </button>
                        {style.lit && style.name !== shelf.usually && (
                          <button
                            type="button"
                            className="btn btn--mini"
                            title="Draw with this one when nobody says"
                            onClick={() =>
                              void drawUsually(style.name).then((failure) => {
                                setSaid(failure);
                                read();
                              })
                            }
                          >
                            USE USUALLY
                          </button>
                        )}
                      </li>
                    ))}
                  </ul>

                  {attaching && (
                    <div className="rm__list rm__list--changes">
                      <span className="rm__heading">
                        What can draw {attaching}
                      </span>
                      {shelf.workflows.length === 0 ? (
                        <span>
                          Nothing imported yet. A workflow is a file you export from
                          ComfyUI — its menu, <em>File → Export</em>.
                        </span>
                      ) : (
                        shelf.workflows.map((workflow) => {
                          const on = shelf.styles
                            .find((style) => style.name === attaching)
                            ?.using.includes(workflow.name);
                          return (
                            <button
                              key={workflow.id}
                              type="button"
                              className="btn btn--mini"
                              disabled={workflow.problem !== null}
                              title={workflow.problem ?? undefined}
                              onClick={() =>
                                void attach(attaching, workflow.id, !on)
                              }
                            >
                              {on ? "✓ " : ""}
                              {workflow.name}
                            </button>
                          );
                        })
                      )}
                    </div>
                  )}
                </div>

                <div className="bay__rule" />

                <div className="rm">
                  <div className="rdy__head">
                    <span className="rm__label">Advanced · Workflows on this machine</span>
                    <span className="rdy__acts">
                      <button
                        type="button"
                        className="btn btn--mini"
                        title="Write a plain one from what this ComfyUI has"
                        onClick={() => void build()}
                      >
                        BUILD ME ONE
                      </button>
                      <button
                        type="button"
                        className="btn btn--mini"
                        onClick={() => file.current?.click()}
                      >
                        IMPORT A WORKFLOW…
                      </button>
                    </span>
                  </div>
                  {/*
                    The window reads bytes out of this and hands over base64. It never learns a path,
                    and only the Engine writes (ADR-0024).
                  */}
                  <input
                    ref={file}
                    type="file"
                    accept="application/json,.json"
                    hidden
                    onChange={(event) => {
                      const chosen = event.target.files?.[0];
                      event.target.value = "";
                      if (chosen) void take(chosen);
                    }}
                  />

                  {shelf.workflows.length === 0 ? (
                    <p className="cc__hint">
                      None yet. Epoch ships no workflow on purpose — one that came in
                      the box would quietly become the house style, and nobody chose
                      that. <b>BUILD ME ONE</b> writes a plain one against whatever
                      checkpoint this ComfyUI reports, so nothing arrives from
                      elsewhere and nothing has to be exported by hand.
                    </p>
                  ) : (
                    <ul className="rdy__list">
                      {shelf.workflows.map((workflow) => (
                        <li
                          key={workflow.id}
                          className={`rdy__row rdy__row--${workflow.problem ? "needsYou" : "ready"}`}
                        >
                          <span className="rdy__dot" aria-hidden />
                          <span className="rdy__name">{workflow.name}</span>
                          <span className="rdy__note">{tell(workflow)}</span>
                          {/*
                            **Naming is what brings a Style into existence**, so the control lives on
                            the thing that causes it: a workflow arrived, and somebody says what it
                            draws. Epoch never reads the name off the file — a graph called
                            *SNES Pixel Art* is probably pixel art, and *probably* is not a thing to
                            act on when the consequence is deciding what somebody's `pixel art` means.
                          */}
                          <button
                            type="button"
                            className="btn btn--mini"
                            disabled={workflow.problem !== null}
                            title={
                              workflow.problem ??
                              "Say what this draws, in the words a character would ask for"
                            }
                            onClick={() => {
                              setNaming(naming === workflow.id ? null : workflow.id);
                              setStyleName("");
                            }}
                          >
                            {naming === workflow.id ? "CLOSE" : "IT DRAWS…"}
                          </button>
                          <button
                            type="button"
                            className="btn btn--mini"
                            onClick={() =>
                              void forgetWorkflow(workflow.id).then((failure) => {
                                setSaid(failure);
                                read();
                              })
                            }
                          >
                            FORGET
                          </button>
                          {naming === workflow.id && (
                            <span className="rdy__acts">
                              {/*
                                No class: every input inside a panel is themed by default here, and
                                opting in is the leak that rule was written to close.
                              */}
                              <input
                                value={styleName}
                                autoFocus
                                placeholder="watercolour"
                                onChange={(event) => setStyleName(event.target.value)}
                                onKeyDown={(event) => {
                                  if (event.key === "Enter") void name(workflow.id);
                                  if (event.key === "Escape") setNaming(null);
                                }}
                              />
                              <button
                                type="button"
                                className="btn btn--mini"
                                disabled={styleName.trim().length === 0}
                                onClick={() => void name(workflow.id)}
                              >
                                NAME IT
                              </button>
                            </span>
                          )}
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
          </div>
        )}
      </div>

      {said && <p className="notice">{said}</p>}
    </section>
  );
}

/** What a Style's row says, and an unlit one says what would light it. */

/**
 * What a Style's row says.
 *
 * **Only `General` can be unlit now.** Every other Style exists because something draws it, so a
 * row that cannot draw cannot be on this list at all (ADR-0030's second amendment). The honest
 * report of an absence did not disappear — it moved to the sentence a character says at the
 * moment somebody asks for a style nobody installed, which is where it is worth more.
 */
function describe(style: StyleRow, shelf: ImageShelf): string {
  if (style.lit) return style.using.join(" · ");
  if (shelf.workflows.length === 0) {
    return "no workflow yet — BUILD ME ONE, or import one below";
  }
  return "no workflow attached — press ATTACH";
}

/**
 * What a workflow is, in one line.
 *
 * Everything here was derived from the graph at import: what it can do by which nodes are in
 * it, what it needs by which files it names. Nothing was typed into a manifest, because a
 * capability somebody wrote down is one that will eventually lie.
 */
function tell(workflow: {
  readonly can: readonly string[];
  readonly needs: readonly string[];
  readonly problem: string | null;
}): string {
  if (workflow.problem) return workflow.problem;
  const can =
    workflow.can.length > 0 ? `draws from ${workflow.can.join(", ")}` : "draws";
  return workflow.needs.length > 0
    ? `${can} · uses ${workflow.needs.join(", ")}`
    : can;
}
