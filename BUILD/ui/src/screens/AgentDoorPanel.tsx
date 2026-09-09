/**
 * Epoch, as a server (step 5.2).
 *
 * ## The mirror of the panel above it
 *
 * The MCP deck is about tools coming *in*: programs on this machine that offer things Epoch's
 * crew can use. This is the same protocol pointing the other way — Epoch offering *its* tools
 * to an agent that runs outside it.
 *
 * One deck, because it is one protocol and the user should not have to learn that it has a
 * direction. Two sections, because opening a door is not the same act as configuring a program.
 *
 * ## What it is actually for
 *
 * An agent — Claude Code, Codex — owns its own loop and its own permission prompt, which would
 * make Epoch's dropdown a decoration: a character set to `Manual` would be writing files while
 * the screen said it asks first. So the agent is configured with **no** file access and one
 * server: this one. Then every read, write and command it makes is judged by Epoch.
 *
 * That is why this panel says who the agent is acting as before it will open at all. A call has
 * to belong to somebody, and a default here would be Epoch deciding whose hands an outside
 * program is holding.
 */

import { useEffect, useState } from "react";

import type { Adopted, AgentDoor } from "../experience/useAgentDoor";

export function AgentDoorPanel({
  door,
  crew,
  bare,
}: {
  readonly door: AgentDoor;
  /**
   * Everybody who could be acted as — narrowed to what this actually reads.
   *
   * Not `CharacterSummary`: the bridge and the World describe a character differently, and a
   * panel that demanded one shape could only live on one surface. It needs an identity and a
   * name, so that is what it asks for.
   */
  readonly crew: readonly { readonly id: string; readonly name: string }[];
  /**
   * Drop the deck's own frame, for a surface that supplies one.
   *
   * One component on two surfaces rather than two that drift: the door is the same door from
   * the bridge and from inside a World, and only one of them can actually open it.
   */
  readonly bare?: boolean;
}) {
  const [who, setWho] = useState("");
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState<string | null>(null);
  const [configuration, setConfiguration] = useState<string | null>(null);
  /** What the last setup wrote. `null` until somebody presses it. */
  const [done, setDone] = useState<Adopted | null>(null);

  const open = Boolean(door.bridge.url);

  /*
    The configuration follows the door, not the button.

    It was fetched only inside `knock`, so a door that was already open when this mounted showed
    `…` forever — which is exactly what somebody sees after a reload, and it looks like the
    feature is broken rather than like a value was never asked for.
  */
  useEffect(() => {
    if (!open) {
      setConfiguration(null);
      return;
    }
    void door.configuration().then(setConfiguration);
    // `door` is rebuilt every render by its hook; depending on it would refetch forever.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, door.bridge.token]);

  const knock = async () => {
    if (!who || busy) return;
    setBusy(true);
    setFailed(await door.open(who));
    setBusy(false);
  };

  const setUp = async () => {
    setBusy(true);
    const result = await door.adopt();
    if (typeof result === "string") {
      setFailed(result);
      setDone(null);
    } else {
      setFailed(null);
      setDone(result);
    }
    setBusy(false);
  };


  /*
    Two shapes, not one shape minus a wrapper.

    `bare` was implemented as the deck's markup without its `<section>`, so a 208px sidebar got
    a heading it already had, a diamond, three paragraphs written for a wide column, and a
    Windows path with nowhere to go. Narrow is not "the same, smaller" — it is a different
    amount of words.

    The deck explains, because somebody reading the MCP deck is deciding whether they want this.
    The sidebar acts, because somebody in a World has already decided.
  */
  if (bare) {
    return (
      <div className="door door--tight">
        {failed && <p className="door__bad">{failed}</p>}

        {open ? (
          <>
            <p className="door__on">
              <span className="door__lamp" /> LISTENING as <b>{door.bridge.character}</b>
            </p>

            {done ? (
              <div className="door__done">
                <p className="door__on">
                  <span className="door__lamp" /> READY
                </p>
                {done.wrote.map((file) => (
                  <code key={file}>{file}</code>
                ))}
                <p className="door__note">Open your agent in that folder.</p>
              </div>
            ) : (
              <>
                <button
                  type="button"
                  className="epbtn"
                  disabled={busy}
                  onClick={() => void setUp()}
                >
                  SET UP PROJECT
                </button>
                <p className="door__note">
                  Adds Epoch to this project&rsquo;s <code>.mcp.json</code>. Your agent keeps
                  its own tools.
                </p>
              </>
            )}

            {/*
              Kept, folded, for an agent Epoch cannot set up: one on another machine, or one
              whose configuration lives somewhere Epoch has no business writing. Copying is a
              button rather than a selection, because the token has to be exact.
            */}
            <details className="door__manual">
              <summary>Do it by hand</summary>
              <pre className="door__config">{configuration ?? "…"}</pre>
              <button
                type="button"
                className="epbtn"
                onClick={() => {
                  if (configuration) void navigator.clipboard.writeText(configuration);
                }}
              >
                COPY
              </button>
            </details>

            <button type="button" className="epbtn" onClick={() => void door.close()}>
              CLOSE
            </button>
          </>
        ) : (
          <>
            <select
              className="door__who"
              value={who}
              onChange={(event) => setWho(event.target.value)}
              aria-label="The agent acts as"
            >
              <option value="">— acts as —</option>
              {crew.map((person) => (
                <option key={person.id} value={person.id}>
                  {person.name}
                </option>
              ))}
            </select>
            <button
              type="button"
              className="epbtn"
              disabled={!who || busy}
              onClick={() => void knock()}
            >
              {busy ? "OPENING…" : "OPEN THE DOOR"}
            </button>
            <p className="door__note">
              An agent in its own terminal gains your crew, the Quest and this World.
            </p>
          </>
        )}
      </div>
    );
  }

  return (
    <section className="pnl bay">
      <div className="bay__head">
        <div>
          <div className="bay__heading">
            <span className="lx__gem" aria-hidden />
            <h2>AGENTS</h2>
          </div>
          <p className="bay__sub">
            The same protocol, pointing outward. An agent running in its own terminal keeps its
            own tools and gains Epoch&rsquo;s &mdash; your crew, the Quest, this World. Those are
            judged by the same permission model as a model&rsquo;s, in front of you.
          </p>
        </div>
      </div>
      <div className="bay__rule" />
      <p className="door__note">
        The door is opened from inside a World, because everything it needs belongs to one: the
        project folder, the tools, and who the agent acts as. Board a World and look for
        <b> AGENTS</b> in the side column.
      </p>
    </section>
  );
}
