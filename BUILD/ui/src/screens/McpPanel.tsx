/**
 * The MCP deck — tools that live outside Epoch.
 *
 * Its own deck rather than a section of Connections, because the two answer different
 * questions: a connection is **who thinks**, a server here is **what the crew can do**. Putting
 * them behind one word would make "which model" and "which tools" the same choice.
 *
 * ## Why every one of these asks the first time
 *
 * Risk in Epoch is derived from declared effects, never declared directly — so nobody can label
 * a delete as low-risk (ADR-0008). MCP declares no effects at all, and the hints it does offer
 * (`readOnlyHint`, `destructiveHint`) are a third party grading its own risk, arriving from the
 * least trustworthy place it could: a program installed from somewhere else.
 *
 * So they are refused, and an outside tool is assumed to read, write, delete, execute and reach
 * the network. Every one asks the first time — and the Trust store remembers a standing answer,
 * so it asks *once* rather than every time.
 *
 * ## Nothing here runs until somebody asks
 *
 * Opening this screen starts no processes. A server's tools are whatever the server says they
 * are, so finding out means running it — which is a button, not a side effect of navigation.
 */

import { useEffect, useState } from "react";

import {
  fetchMcp,
  forgetMcp,
  probeMcp,
  refreshMcp,
  saveMcp,
  saveMcpSecret,
} from "../ipc/launcher";
import type { McpServer } from "../ipc/contracts";

const NEW_SERVER: McpServer = {
  id: "",
  command: "",
  args: [],
  env: {},
  secrets: [],
  enabled: true,
};

export function McpPanel({
  onChanged,
}: {
  /**
   * Told when the configuration changed, because it changes more than this deck.
   *
   * The crew's capability list is built from these servers: a connected one becomes a single
   * box granting everything it offers. Forgetting a server used to leave that box on the
   * character editor with a tool count beside it — a connection nobody had, offered as though
   * they did. Nothing was wrong in the Engine; the Launcher had read the vocabulary once, on
   * open, and had no way to hear that it was now false.
   */
  readonly onChanged?: () => void;
}) {
  const [servers, setServers] = useState<readonly McpServer[]>([]);
  const [problem, setProblem] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);

  /** The Engine's reason for refusing the last thing attempted. Never invented here. */
  const [notice, setNotice] = useState<string | null>(null);
  const [draft, setDraft] = useState<McpServer | null>(null);
  /** Why the last credential was not stored, shown beside the button that tried. */
  const [refusal, setRefusal] = useState<string | null>(null);
  /**
   * Whether the open form is the *add* form.
   *
   * Kept as its own flag rather than inferred from an empty name. An earlier version gated the
   * add form on `draft.id === ""`, so typing the first character of a name made the form
   * unmount mid-keystroke — the name could never be typed at all. State that means "which form
   * is open" must not be derived from a field the user is editing.
   */
  const [adding, setAdding] = useState(false);
  const [forgetting, setForgetting] = useState<string | null>(null);

  /** What the servers actually offered, once somebody asked. `null` means nobody has. */
  const [offered, setOffered] = useState<readonly string[] | null>(null);
  const [refused, setRefused] = useState<readonly string[]>([]);
  const [asking, setAsking] = useState(false);

  async function reload() {
    const view = await fetchMcp();
    setServers(view.servers);
    setProblem(view.problem ?? null);
    setLoaded(true);
  }

  useEffect(() => {
    void reload();
    // Remembered from last time, so a restart does not lose the tool list. Costs nothing and
    // starts nothing — the whole reason the answer is written down.
    void ask();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /**
   * What the servers offer.
   *
   * Answers from what they said last time, so this is free and the tools are on screen without
   * anybody pressing anything. `refresh` is the one that starts the processes.
   */
  async function ask(fresh = false) {
    setAsking(true);
    const [tools, problems] = await (fresh ? refreshMcp() : probeMcp());
    setOffered(tools);
    setRefused(problems);
    setAsking(false);
  }

  /**
   * The configuration changed. Read it back, and read back what the servers said.
   *
   * One function because it was three, and two of them were wrong in the same way: they threw
   * the remembered tool list away and never re-read it, so changing anything turned *every*
   * server's status to UNASKED — the deck reporting that it had never asked anybody, after an
   * action that asked nothing. Discarding what was on screen is right, because it is now stale;
   * leaving nothing in its place is not, because the answer is still on disk.
   *
   * `ask()` reads that memory. It costs nothing and starts no processes — `refresh` is the one
   * that spawns anything, and it stays a button.
   */
  async function settled() {
    setOffered(null);
    setRefused([]);
    await reload();
    await ask();
    onChanged?.();
  }

  async function commit(server: McpServer) {
    const failure = await saveMcp(server);
    setNotice(failure);
    if (failure) return;
    setDraft(null);
    setAdding(false);
    await settled();
  }

  /**
   * Store a credential for a server that already exists.
   *
   * Saved on its own rather than with the rest of the form, because it travels a different way:
   * everything else in the draft round-trips, and this only ever goes one direction. Reloading
   * afterwards brings back the *name*, which is all there is to bring back.
   */
  async function keep(server: string, name: string, value: string) {
    const failure = await saveMcpSecret(server, name, value);
    // Next to the button that failed, not at the top of the deck.
    //
    // It went to the panel's notice first, which is the right place for *the panel* and the
    // wrong place for this: the form sits far below the head, so pressing ADD and having
    // nothing happen was the whole visible result. An error nobody scrolls to is an error
    // nobody has. Reported as "the ADD button does nothing", which is exactly what it looked
    // like from where the user was.
    setRefusal(failure);
    if (failure) return;
    // The draft is holding the version from before the credential existed; saving it now would
    // write that back over the name we just added.
    setDraft(null);
    await settled();
  }

  async function drop(id: string) {
    const failure = await forgetMcp(id);
    setNotice(failure);
    setForgetting(null);
    if (failure) return;
    await settled();
  }

  return (
    <section className="pnl bay">
      <div className="bay__head">
        <div>
          <div className="bay__heading">
            <span className="lx__gem" aria-hidden />
            <h2>MCP</h2>
          </div>
          <p className="bay__sub">
            Programs on this machine that offer tools over the Model Context
            Protocol. Whatever they offer becomes an ordinary capability &mdash;
            same permission gate, same record.
          </p>
        </div>
        {/*
          What they offer is already on screen, remembered from last time. This asks *again* —
          the way to pick up a server whose tools changed, since this build cannot notice that
          on its own.
        */}
        <button
          type="button"
          className="btn btn--ghost"
          onClick={() => void ask(true)}
          disabled={asking}
          title="Start the servers and ask again. Their answer is remembered."
        >
          {asking ? "ASKING…" : "ASK AGAIN"}
        </button>
      </div>
      <div className="bay__rule" />

      {problem && <p className="notice notice--warn">{problem}</p>}
      {notice && <p className="notice notice--warn">{notice}</p>}

      {loaded && servers.length === 0 && !problem && (
        <p className="notice">
          Nothing configured, and nothing running. An MCP server is a program
          Epoch would start, so none ships by default.
        </p>
      )}

      <ul className="conn">
        {servers.map((server) => {
          const editing = draft !== null && !adding && draft.id === server.id;
          const mine = (offered ?? []).filter((id) =>
            id.startsWith(`${server.id}_`),
          );
          const line = [server.command, ...server.args].join(" ");

          return (
            <li
              key={server.id}
              className={`conn__row${server.enabled ? "" : " conn__row--off"}`}
            >
              <div className="conn__line">
                <span className="conn__name">{server.id}</span>
                <span className="conn__kind">MCP</span>
                <span className="conn__where" title={line}>
                  {line}
                </span>
                <span
                  className={`conn__status${mine.length > 0 ? " conn__status--on" : ""}`}
                >
                  {!server.enabled
                    ? "OFF"
                    : asking
                      ? "ASKING…"
                      : offered === null
                        ? "UNASKED"
                        : `${mine.length} TOOLS`}
                </span>
              </div>

              {mine.length > 0 && (
                <div className="conn__models">{mine.join(", ")}</div>
              )}

              <div className="conn__acts">
                <button
                  type="button"
                  className="btn btn--mini"
                  onClick={() => {
                    setNotice(null);
                    setAdding(false);
                    setDraft(editing ? null : server);
                  }}
                >
                  {editing ? "CLOSE" : "EDIT"}
                </button>
                <button
                  type="button"
                  className="btn btn--mini"
                  onClick={() =>
                    void commit({ ...server, enabled: !server.enabled })
                  }
                >
                  {server.enabled ? "SWITCH OFF" : "SWITCH ON"}
                </button>
                {forgetting === server.id ? (
                  <>
                    <button
                      type="button"
                      className="btn btn--mini"
                      onClick={() => void drop(server.id)}
                    >
                      FORGET IT
                    </button>
                    <button
                      type="button"
                      className="btn btn--mini"
                      onClick={() => setForgetting(null)}
                    >
                      KEEP
                    </button>
                    <span className="conn__warn">
                      Its process stops, and any character reaching for its
                      tools loses them.
                    </span>
                  </>
                ) : (
                  <button
                    type="button"
                    className="btn btn--mini"
                    onClick={() => {
                      setNotice(null);
                      setForgetting(server.id);
                    }}
                  >
                    FORGET
                  </button>
                )}
              </div>

              {editing && draft && (
                <ServerForm
                  draft={draft}
                  naming={false}
                  onChange={setDraft}
                  onSave={() => void commit(draft)}
                  onSecret={(name, value) => keep(server.id, name, value)}
                  refusal={refusal}
                />
              )}
            </li>
          );
        })}
      </ul>

      {refused.length > 0 && (
        <div className="notice notice--warn">
          {refused.map((why, i) => (
            <div key={i}>{why}</div>
          ))}
        </div>
      )}

      {adding && draft ? (
        <div className="conn__add">
          <ServerForm
            draft={draft}
            naming
            onChange={setDraft}
            onSave={() => void commit(draft)}
            onSecret={null}
            refusal={null}
          />
          <button
            type="button"
            className="btn btn--mini"
            onClick={() => {
              setAdding(false);
              setDraft(null);
              setNotice(null);
            }}
          >
            CANCEL
          </button>
        </div>
      ) : (
        <button
          type="button"
          className="btn btn--violet"
          onClick={() => {
            setNotice(null);
            setForgetting(null);
            setAdding(true);
            setDraft(NEW_SERVER);
          }}
        >
          ADD A SERVER
        </button>
      )}

      <p className="notice" style={{ marginTop: 12 }}>
        Every tool here asks the first time it is used, in every mode but Auto.
        MCP declares no effects, and the hints it offers are a program grading
        its own risk &mdash; so Epoch assumes the worst of it and lets you
        decide once, by name.
      </p>
    </section>
  );
}

function ServerForm({
  draft,
  naming,
  onChange,
  onSave,
  onSecret,
  refusal,
}: {
  readonly draft: McpServer;
  readonly naming: boolean;
  readonly onChange: (next: McpServer) => void;
  readonly onSave: () => void;
  /**
   * Where a credential goes, or `null` while the server does not exist yet.
   *
   * There is nothing to encrypt a value *against* before the server has a name — the store keys
   * on it. So the add form says so instead of offering a field that could not work.
   */
  readonly onSecret: ((name: string, value: string) => void) | null;
  /** Why the Engine refused the last credential, or `null`. */
  readonly refusal: string | null;
}) {
  const [variable, setVariable] = useState("");
  const [value, setValue] = useState("");
  const [private_, setPrivate] = useState(true);
  /** What this form refused on its own, before asking anybody. */
  const [wrong, setWrong] = useState<string | null>(null);

  function add() {
    const name = variable.trim();
    // Both halves, and both said out loud. Returning silently on an empty name is what made
    // this button look broken: nothing happened and nothing explained why.
    if (name.length === 0) {
      setWrong("A variable needs a name — the left box.");
      return;
    }
    if (value.trim().length === 0) {
      setWrong(`${name} needs a value — the right box.`);
      return;
    }
    setWrong(null);
    if (private_ && onSecret) {
      onSecret(name, value);
    } else {
      onChange({ ...draft, env: { ...draft.env, [name]: value } });
    }
    setVariable("");
    setValue("");
  }

  return (
    <div className="cedit conn__edit">
      {naming && (
        <label className="cedit__field">
          <span>NAME</span>
          <input
            value={draft.id}
            placeholder="playwright"
            onChange={(e) => onChange({ ...draft, id: e.target.value })}
          />
        </label>
      )}
      <label className="cedit__field">
        <span>COMMAND</span>
        <input
          value={draft.command}
          placeholder="npx.cmd"
          onChange={(e) => onChange({ ...draft, command: e.target.value })}
        />
      </label>
      <label className="cedit__field cedit__field--wide">
        <span>ARGUMENTS</span>
        <input
          value={draft.args.join(" ")}
          placeholder="-y @playwright/mcp@latest"
          onChange={(e) =>
            onChange({
              ...draft,
              args: e.target.value.split(" ").filter((a) => a.length > 0),
            })
          }
        />
      </label>
      {/*
        What was actually parsed, one chip per argument.

        Found by using it: a server was configured from a JSON snippet — `["-y", "@pkg"]` — and
        typed back in with the comma. Split on spaces, that is the single argument `-y,`, which
        the runner ignores and the server never starts from. Nothing on the screen disagreed
        with what had been typed, and the failure surfaced minutes later somewhere else as a
        capability that had gone missing.

        The same rule the Workshop card follows: show the literal thing that will run.
      */}
      {draft.args.length > 0 && (
        <div className="cedit__field cedit__field--wide">
          <span>WILL RUN AS</span>
          <div className="caps">
            <span className="chip chip--on">{draft.command || "—"}</span>
            {draft.args.map((argument, at) => (
              <span
                key={`${argument}-${at}`}
                className={`chip${/[,"']/.test(argument) ? " chip--pending" : ""}`}
                title={
                  /[,"']/.test(argument)
                    ? "This argument contains a comma or quote. Arguments are split on spaces only — there is no shell to strip it."
                    : undefined
                }
              >
                {argument}
              </span>
            ))}
          </div>
        </div>
      )}
      {/*
        What the server is given when it starts.

        Two lists rather than one, because they are two different things and the difference is
        the point: a root path is configuration and belongs in a file somebody can read, and a
        token is a credential and belongs in the encrypted store. A credential shows its **name**
        and never its value — there is no command that reads one back.
      */}
      <div className="cedit__field cedit__field--wide">
        <span>ENVIRONMENT</span>
        <div className="conn__vars">
          {draft.secrets.map((name) => (
            <div key={`s-${name}`} className="conn__var">
              <code>{name}</code>
              <span className="wk__secret">kept encrypted</span>
              <button
                type="button"
                className="btn btn--mini"
                title="Removing it here and saving deletes the stored value too."
                onClick={() =>
                  onChange({
                    ...draft,
                    secrets: draft.secrets.filter((held) => held !== name),
                  })
                }
              >
                REMOVE
              </button>
            </div>
          ))}
          {Object.entries(draft.env).map(([name, held]) => (
            <div key={`e-${name}`} className="conn__var">
              <code>{name}</code>
              <input
                value={held}
                onChange={(e) =>
                  onChange({
                    ...draft,
                    env: { ...draft.env, [name]: e.target.value },
                  })
                }
              />
              <button
                type="button"
                className="btn btn--mini"
                onClick={() => {
                  const rest = { ...draft.env };
                  delete rest[name];
                  onChange({ ...draft, env: rest });
                }}
              >
                REMOVE
              </button>
            </div>
          ))}
          {draft.secrets.length === 0 &&
            Object.keys(draft.env).length === 0 && (
              <p className="conn__blurb">
                Nothing set. A server that needs a key or an id says so by
                refusing to start, and the reason appears above.
              </p>
            )}
        </div>
        {/*
          Labelled, because the placeholders taught the wrong thing.

          Two bare boxes, the first placeheld `SPOTIFY_CLIENT_ID`, and it read as *put your
          client id here* rather than as an example of a variable's name. Watched somebody paste
          the id into the name box — correctly, by the only reading the screen offered.

          An example is not a label. A field whose meaning is carried entirely by an example of
          what goes in it has to be read backwards to be understood, and the first person to get
          it wrong was the person the example was written for.
        */}
        <div className="conn__var conn__var--add">
          <label className="conn__cell">
            <span>VARIABLE</span>
            <input
              value={variable}
              placeholder="SPOTIFY_CLIENT_ID"
              onChange={(e) => setVariable(e.target.value)}
            />
          </label>
          <label className="conn__cell">
            <span>ITS VALUE</span>
            <input
              type={private_ ? "password" : "text"}
              value={value}
              placeholder="the id or key from the service"
              onChange={(e) => setValue(e.target.value)}
            />
          </label>
          <label
            className="conn__keep"
            title="Store the value encrypted instead of in mcp.toml."
          >
            <input
              type="checkbox"
              checked={private_}
              disabled={onSecret === null}
              onChange={(e) => setPrivate(e.target.checked)}
            />
            <span>SECRET</span>
          </label>
          <button type="button" className="btn btn--mini" onClick={add}>
            ADD
          </button>
        </div>
        {(wrong ?? refusal) && (
          <p className="notice notice--warn">{wrong ?? refusal}</p>
        )}
        <p className="conn__blurb">
          {onSecret === null ? (
            <>
              Save the server first and a credential can be added to it &mdash;
              the encrypted store keys on the server&rsquo;s name, so there is
              nowhere to put one until it has one. Anything added here now goes
              in <code>mcp.toml</code> in the clear.
            </>
          ) : (
            <>
              A <b>secret</b> is stored encrypted the moment you add it and is
              never shown again, here or anywhere. Anything else is written
              plainly to <code>mcp.toml</code>, which is what configuration
              should be.
            </>
          )}
        </p>
      </div>
      <div className="cedit__field cedit__field--wide">
        <p className="conn__blurb">
          Split on spaces, and there is <b>no shell</b> &mdash; the same rule
          the crew&rsquo;s own command capability follows. Paste from a JSON
          snippet and the commas come with it, so what will actually run is
          shown above. On Windows a Node launcher is <code>npx.cmd</code>, not{" "}
          <code>npx</code>.
        </p>
        <button type="button" className="btn" onClick={onSave}>
          SAVE
        </button>
      </div>
    </div>
  );
}
