/**
 * The Connections deck — where the thinking comes from.
 *
 * A backend belongs to the **machine**, not to a World and not to a character (ADR-0026): an
 * endpoint is a property of a computer the way a temperature is a property of a person. So one
 * list serves every World, and moving between Worlds never changes what can think.
 *
 * ## Two facts at once, and they are different facts
 *
 * *Configured* is what the user wrote down. *Online* is what answered when Epoch asked. A row
 * shows both, because collapsing them makes OFFLINE look like MISSING — and the fix for those
 * two is not the same fix. The status half is measured every probe and never assumed; a backend
 * that has not been probed reads UNKNOWN rather than borrowing the last answer.
 *
 * ## Nothing is validated here
 *
 * The Engine owns what a valid backend is and is the thing that has to live with the file
 * afterwards (ADR-0023). This form collects what was typed and shows the Engine's answer, so
 * the rules can only ever be stated in one place.
 *
 * ## Why an id cannot be edited
 *
 * A character's brain names a backend by id. Changing it here would leave every character
 * pointing at something that no longer exists, silently — so an id is chosen once, and a
 * different one is a different backend. Add, move the crew across, forget the old one: three
 * visible steps instead of one invisible breakage.
 */

import { useEffect, useState } from "react";
import { agentLink } from "../experience/agentLink";

import { openLink } from "../ipc/world";
import { Readiness } from "../components/Readiness";
import { ThisMachine } from "../components/ThisMachine";
import { LocalRuntimes } from "../components/LocalRuntimes";
import {
  addAgentAccount,
  fetchAgents,
  fetchBackends,
  forgetBackend,
  removeAgentAccount,
  renameAgentAccount,
  saveBackend,
  saveBackendKey,
  signInAgent,
} from "../ipc/launcher";
import type { AgentStatus } from "../ipc/launcher";
import type { Backend, BackendKind, ProviderStatus } from "../ipc/contracts";

/**
 * What each kind is, in the user's terms.
 *
 * The set mirrors the Engine's closed one. A kind listed here that the Engine does not build is
 * a configuration that can only fail later, which is why the Engine refuses rather than trusts.
 */
interface KindLook {
  readonly id: BackendKind;
  readonly name: string;
  readonly blurb: string;
  readonly usual: string;
  /**
   * Whether this kind cannot be asked anything without a credential.
   *
   * Mirrors the Engine's `Kind::needs_credential`. It is the difference between a backend that
   * is *offline* and one that is *waiting for you* — showing both the same way sends somebody
   * looking for a server that is running perfectly well.
   */
  readonly needsKey: boolean;
  /**
   * Where a credential for this kind comes from, when there is such a place.
   *
   * Optional because most kinds have none: a local runtime has nothing to sign up for, and a
   * link to nowhere would be worse than no link.
   */
  readonly where?: { readonly label: string; readonly url: string };
}

const OLLAMA = {
  id: "ollama",
  name: "Ollama",
  blurb: "Models on a machine you control. No key, no account, no bill.",
  usual: "http://localhost:11434",
  needsKey: false,
} as const satisfies KindLook;

const ANTHROPIC = {
  id: "anthropic",
  name: "Anthropic",
  blurb:
    "Claude, over the network. Needs an API key — a different thing from a Claude subscription, and billed separately.",
  usual: "https://api.anthropic.com",
  needsKey: true,
  /*
    Where the key comes from, as somewhere you can actually go.

    It was in the sentence as text, which told somebody the answer and then made them retype it
    into a browser. Opened through the Engine (`open_link`), which checks the scheme — the same
    door search results go through, and the reason a surface check here would only be a
    courtesy.
  */
  where: {
    label: "console.anthropic.com",
    url: "https://console.anthropic.com/settings/keys",
  },
} as const satisfies KindLook;

/*
  The one entry that is not a vendor.

  llama.cpp, LM Studio, vLLM, Deepseek, GLM, Groq and most of what ships next all speak the same
  API, so they are not a dozen backends — they are one shape at a dozen addresses. Adding the
  next one is typing an address, not shipping a release.

  **No key required, and no key forbidden**, which is why `needsKey` is false rather than true. A
  hosted vendor demands one and a llama.cpp on the desk has none, and this single entry is both.
  Marking it as needing a credential would make the local case look misconfigured; the backend
  itself says which it is, and the row reports what it said.

  No `where` link for the same reason: there is no one place these keys come from, and a link to
  the wrong vendor's console is worse than none.
*/
const OPENAI = {
  id: "openai",
  name: "OpenAI-compatible",
  blurb:
    "Anything speaking the OpenAI API — llama.cpp, LM Studio, vLLM on your own machine or another, Deepseek, GLM. Give it an address, and a key if it wants one.",
  usual: "http://localhost:8080",
  needsKey: false,
} as const satisfies KindLook;

const KINDS: readonly KindLook[] = [OLLAMA, ANTHROPIC, OPENAI];

/**
 * A name for a new backend of this kind, avoiding the ones already taken.
 *
 * Suggested rather than imposed: the id is what a character's brain names, so it has to be the
 * user's to choose — but the overwhelmingly common case is one backend per kind, and making
 * somebody invent a word for it was a blank field standing between them and a working setup.
 *
 * The second Ollama becomes `ollama_2` rather than colliding, which is the case that made id and
 * kind separate questions in the first place.
 */
function suggest(kind: BackendKind, taken: readonly string[]): string {
  if (!taken.includes(kind)) return kind;
  for (let n = 2; ; n += 1) {
    const tried = `${kind}_${n}`;
    if (!taken.includes(tried)) return tried;
  }
}

const NEW: Backend = {
  id: "",
  kind: OLLAMA.id,
  endpoint: OLLAMA.usual,
  enabled: true,
};

interface ConnectionsPanelProps {
  /** What answered, from the last probe. Measured — never a stored status. */
  readonly providers: readonly ProviderStatus[];
  /** True while a probe is in flight, so a row can say so instead of guessing. */
  readonly probing: boolean;
  /** Ask the Launcher to probe again: a corrected endpoint is worth nothing unproven. */
  readonly onProbe: () => void;
  /**
   * The runtimes block finished its own reading.
   *
   * Narrower than {@link onProbe} on purpose: what changed is which local servers answer, and
   * that cannot change whether an agent is signed in.
   */
  readonly onRuntimesRead?: () => void;
}

export function ConnectionsPanel({
  providers,
  probing,
  onProbe,
  onRuntimesRead,
}: ConnectionsPanelProps) {
  /**
   * Which local runtimes the block at the top of this panel is reporting.
   *
   * Told by that block rather than worked out here: two places deciding which ids are local
   * runtimes is how they come to disagree the first time a fourth one exists. It stays inside
   * this component because both halves now live in it — it was a prop through the Launcher when
   * they were two panels, which is a wire whose only job was to connect a thing to itself.
   */
  /**
   * Agents, which connect by **account** rather than by endpoint.
   *
   * Here because this is the deck where somebody comes to make a connection work, and a
   * signed-out agent is a connection that does not. Its own list rather than a fake backend
   * row: an agent has no endpoint to type, no credential Epoch may hold and no model list to
   * pull, so squeezing it into that shape would mean three empty columns explaining nothing.
   */
  const [agents, setAgents] = useState<readonly AgentStatus[]>([]);
  /**
   * Which program a second sign-in is being named for, and what the user has typed so far.
   *
   * The label is asked for rather than measured. Claude Code will say which email is signed in
   * and Codex cannot be asked at all, so a title Epoch invented would be exactly the gauge
   * nobody can explain — the user's own word for it is a fact about them instead.
   */
  const [naming, setNaming] = useState<{
    /** The row the input belongs under — an account id, unique either way. */
    row: string;
    kind: string;
    label: string;
    mode: "add" | "rename";
  } | null>(null);
  /** What the Engine said about the last add or forget. Its words, never a guess. */
  const [accountSaid, setAccountSaid] = useState<string | null>(null);

  const remeasure = () => {
    // **Fresh, or this button stops meaning anything.** The Engine keeps what it measured
    // because asking costs half a second of process spawning; the press is the way to say
    // *ask again*, and a REMEASURE that returned the same kept answer would be a lie with a
    // label on it.
    void fetchAgents(true).then(setAgents);
  };

  useEffect(() => {
    let alive = true;
    void fetchAgents().then((found) => alive && setAgents(found));
    return () => {
      alive = false;
    };
  }, []);
  const [backends, setBackends] = useState<readonly Backend[]>([]);
  /** Which backends have a credential. Ids only — the values never cross this boundary. */
  const [keyed, setKeyed] = useState<readonly string[]>([]);
  const [problem, setProblem] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);

  /** The Engine's reason for refusing the last thing attempted. Never invented here. */
  const [notice, setNotice] = useState<string | null>(null);
  /** Which row is open for editing, and the draft in it. `NEW.id` is the add form. */
  const [draft, setDraft] = useState<Backend | null>(null);
  const [adding, setAdding] = useState(false);
  /** Forgetting asks first: characters keep the assignment and simply stop being able to think. */
  const [forgetting, setForgetting] = useState<string | null>(null);

  async function reload() {
    const view = await fetchBackends();
    setBackends(view.backends);
    setKeyed(view.keyed ?? []);
    setProblem(view.problem ?? null);
    setLoaded(true);
  }

  useEffect(() => {
    void reload();
  }, []);

  /** Save, then re-probe: an endpoint the user just corrected is worth nothing unverified. */
  async function commit(backend: Backend) {
    const failure = await saveBackend(backend);
    setNotice(failure);
    if (failure) return;
    setDraft(null);
    setAdding(false);
    await reload();
    onProbe();
  }

  async function drop(id: string) {
    const failure = await forgetBackend(id);
    setNotice(failure);
    setForgetting(null);
    if (failure) return;
    await reload();
    onProbe();
  }

  return (
    <section className="pnl bay">
      <div className="bay__head">
        <div>
          <div className="bay__heading">
            <span className="lx__gem" aria-hidden />
            <h2>CONNECTIONS</h2>
          </div>
          <p className="bay__sub">
            Where the thinking comes from — this machine first, then everything
            it can reach. These belong to this machine, so every World sees the
            same list.
          </p>
        </div>
        <button
          type="button"
          className="btn btn--ghost"
          onClick={onProbe}
          disabled={probing}
        >
          {probing ? "PROBING…" : "PROBE AGAIN"}
        </button>
      </div>
      <div className="bay__rule" />

      {/*
        **One THIS MACHINE, because there were two** (owner, 2026-08-21).

        This deck opened with a panel headed THIS MACHINE — the hardware and the programs on it
        — and then, a few centimetres below, a list *also* headed "This machine". Two headings
        with one name is a reader deciding which of them is the real one, and the second had
        stopped being true anyway: once the local runtimes moved up here it was mostly agents
        and other people's computers.

        So the deck is one panel and the question it answers is one question — where the
        thinking comes from — read outwards: this machine's hardware, the programs on it, then
        everything it can reach.
      */}
      <ThisMachine />

      <div className="bay__rule" />

      {/*
        **The answer before the configuration** (owner, 2026-09-06).

        This sat *below* `LocalRuntimes`, and to find out what was connected you had to read
        through installing, starting and per-runtime parameters first. That is the one rule this
        project states about every screen — *what is happening, why, what should I do next* —
        failing on the deck whose entire subject is a status.

        **And `omit` went with the move.** It existed for a good reason in the old order: the
        panel directly above already reported the three local runtimes *and could act on them*,
        so repeating them a few centimetres later was two lists saying one thing. Reversed, the
        reasoning reverses with it — **a summary that leaves out the three programs you most
        want to know about is not a summary**, and what follows it is configuration rather than
        a second opinion.

        It also removes a hazard the move would otherwise have introduced. `omit` was filled by
        `LocalRuntimes` as it rendered; above it, the first paint would have listed all three and
        then dropped them a moment later. A row that vanishes is not a tidy list, it is a wrong
        instrument — and this codebase has already paid for one.
      */}
      <Readiness />

      <div className="bay__rule" />

      {/*
        A runtime that starts becomes a Provider without anybody writing anything, so asking
        again here changes the list above it.
      */}
      <LocalRuntimes onChanged={onProbe} onRead={onRuntimesRead} />

      {/*
        The machines themselves moved to their own deck.

        Connections answers *who does the thinking* — providers, agents, credentials. Which
        computers exist and what each may be is a different question with its own vocabulary
        (pairing codes, grants, where a turn is allowed to travel), and it had grown to three
        sections stacked under an unrelated heading.
      */}

      <div className="bay__rule" />

      {problem && (
        <p className="notice notice--warn">
          {problem} What you see below is the default, not your configuration —
          fix the file, or save from here to write a fresh one.
        </p>
      )}
      {notice && <p className="notice notice--warn">{notice}</p>}

      {loaded && backends.length === 0 && !problem && (
        <p className="notice">
          Nothing is configured, so nobody can think. That is a decision Epoch
          will not undo for you — add a backend below and the crew links light
          up.
        </p>
      )}

      {agents.length > 0 && (
        <ul className="conn conn--accounts">
          {agents.map((a) => {
            const link = agentLink(a);
            return (
              <li
                key={a.id}
                className={`conn__row${link.lit ? "" : " conn__row--off"}`}
              >
                <div className="conn__line">
                  <span className="conn__name">{a.name}</span>
                  <span className="conn__kind">agent</span>
                  <span className="conn__where">
                    {link.state === "online" || link.state === "ready"
                      ? link.detail
                      : a.installed
                        ? "no account"
                        : "not installed"}
                  </span>
                  <span
                    className={`conn__state${link.lit ? " conn__state--on" : ""}`}
                  >
                    {link.label}
                  </span>
                </div>

                <div className="conn__models">
                  {!a.installed
                    ? (a.note ?? "Not installed on this machine.")
                    : a.signedIn === false
                      ? "Installed and signed out. Epoch opens its own sign-in — it never sees the credential."
                      : a.signedIn === true
                        ? `Signed in${a.version ? ` · ${a.version}` : ""}. It brings its own tools; Epoch adds the crew, the Quest and this World's knowledge.`
                        : a.method
                          ? (a.note ??
                            `Signed in with ${a.method}. Epoch could not verify it.`)
                          : // **The agent's own reason, when it gave one.** Gemini's probe says
                            // exactly why the question cannot be asked and what to do if a turn
                            // fails for it; the generic sentence below is what to say when nobody
                            // said anything better. A measured reason always beats a written one.
                            (a.note ??
                            "Epoch could not ask whether this is signed in, so it is not saying either way.")}
                </div>

                {a.installed && (
                  <div className="conn__acts">
                    {a.signedIn !== true && (
                      <button
                        type="button"
                        className="btn btn--ghost"
                        onClick={() => void signInAgent(a.id)}
                      >
                        {/*
                        Somebody who already chose a method is not signed out, and telling them
                        to SIGN IN reads as a fault report for a thing that works.
                      */}
                        {link.state === "ready" ? "CHANGE SIGN-IN" : "SIGN IN"}
                      </button>
                    )}
                    <button
                      type="button"
                      className="btn btn--ghost"
                      onClick={remeasure}
                    >
                      CHECK AGAIN
                    </button>
                    {/*
                    **A second sign-in of the same program.** `CLAUDE_CONFIG_DIR` and `CODEX_HOME`
                    isolate one completely — measured, not remembered — so two accounts of one
                    agent are two genuinely separate sessions rather than one session shown twice.
                    Offered only where that is true: Gemini CLI signs in with an API key nobody can
                    be asked about, and the Engine refuses it in its own words.
                  */}
                    {a.id === a.kind && a.kind !== "gemini" && (
                      <button
                        type="button"
                        className="btn btn--ghost"
                        onClick={() => {
                          setAccountSaid(null);
                          setNaming({
                            row: a.id,
                            kind: a.kind,
                            label: "",
                            mode: "add",
                          });
                        }}
                      >
                        ADD ACCOUNT
                      </button>
                    )}
                    {a.id !== a.kind && (
                      <button
                        type="button"
                        className="btn btn--ghost"
                        onClick={() => {
                          setAccountSaid(null);
                          setNaming({
                            row: a.id,
                            kind: a.kind,
                            label: a.name,
                            mode: "rename",
                          });
                        }}
                      >
                        RENAME
                      </button>
                    )}
                    {a.id !== a.kind && (
                      <button
                        type="button"
                        className="btn btn--ghost"
                        onClick={() =>
                          void (async () => {
                            try {
                              // The Engine says what became of the sign-in; it leaves the folder
                              // where it is, and repeating its own sentence is the only honest
                              // account of that.
                              setAccountSaid(await removeAgentAccount(a.id));
                              remeasure();
                            } catch (why) {
                              setAccountSaid(String(why));
                            }
                          })()
                        }
                      >
                        FORGET
                      </button>
                    )}
                  </div>
                )}

                {naming?.row === a.id && (
                  <div className="conn__acts conn__naming">
                    <input
                      className="field"
                      autoFocus
                      placeholder="work, personal, whatever you will recognise"
                      value={naming.label}
                      onChange={(e) =>
                        setNaming({ ...naming, label: e.target.value })
                      }
                    />
                    <div className="conn__naming-answers">
                      <button
                        type="button"
                        className="btn btn--ghost"
                        disabled={naming.label.trim() === ""}
                        onClick={() =>
                          void (async () => {
                            try {
                              if (naming.mode === "rename") {
                                // The id never changes, because a character's brain names the id.
                                // Renaming an account must not silently move somebody's work to a
                                // different sign-in.
                                await renameAgentAccount(a.id, naming.label);
                                setAccountSaid("Renamed.");
                              } else {
                                // Straight into the agent's own login: Epoch makes the folder, hands
                                // the program its own environment and steps back. No password field
                                // here, nothing read back, no token kept.
                                await addAgentAccount(a.kind, naming.label);
                                /*
                                  **What actually happens, because it looks like a failure.**

                                  Measured 2026-08-25: the browser finishes the login through
                                  its own callback and the terminal prints `Login successful.`
                                  The pasteable code is the fallback for a browser that could
                                  not open, and trying to paste it into a console that does not
                                  take Ctrl+V reads as the window being broken; the terminal
                                  ending afterwards reads as a crash. Neither is, and neither is
                                  Epoch's to change — this is the agent's own sign-in and Epoch
                                  never sees what goes through it. What Epoch can do is say so.
                                */
                                setAccountSaid(
                                  "Added. Its own sign-in has opened in its own window — finish it in the browser. There is usually nothing to paste, and the terminal ending afterwards means it worked. This list re-reads itself when you come back to Epoch.",
                                );
                              }
                              setNaming(null);
                              remeasure();
                            } catch (why) {
                              setAccountSaid(String(why));
                            }
                          })()
                        }
                      >
                        {naming.mode === "rename"
                          ? "SAVE NAME"
                          : "ADD AND SIGN IN"}
                      </button>
                      <button
                        type="button"
                        className="btn btn--ghost"
                        onClick={() => setNaming(null)}
                      >
                        CANCEL
                      </button>
                    </div>
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      )}
      {accountSaid && <p className="notice">{accountSaid}</p>}

      <ul className="conn">
        {backends.map((backend) => {
          const kind = KINDS.find((k) => k.id === backend.kind);
          const probed = providers.find((p) => p.id === backend.id);
          const editing = draft !== null && !adding && draft.id === backend.id;

          return (
            <li
              key={backend.id}
              className={`conn__row${backend.enabled ? "" : " conn__row--off"}`}
            >
              <div className="conn__line">
                <span className="conn__name">{backend.id}</span>
                <span className="conn__kind">{kind?.name ?? backend.kind}</span>
                <span className="conn__where" title={backend.endpoint}>
                  {backend.endpoint}
                </span>
                <Status backend={backend} probed={probed} probing={probing} />
              </div>

              {keyed.includes(backend.id) ? (
                <div className="conn__models">
                  A credential is stored for this backend, and travels with
                  every request.
                </div>
              ) : (
                kind?.needsKey && (
                  <div className="conn__models">
                    No credential stored, so this one cannot be asked anything
                    yet. Open EDIT and add a key.
                  </div>
                )
              )}

              {backend.enabled && probed && (
                <div className="conn__models">
                  {probed.online
                    ? probed.models.length > 0
                      ? `${probed.models.length} model${probed.models.length === 1 ? "" : "s"}: ${probed.models.join(", ")}`
                      : "Answered, and reports no models pulled."
                    : (probed.note ?? "Did not answer.")}
                </div>
              )}

              <div className="conn__acts">
                <button
                  type="button"
                  className="btn btn--mini"
                  onClick={() => {
                    setNotice(null);
                    setAdding(false);
                    setDraft(editing ? null : backend);
                  }}
                >
                  {editing ? "CLOSE" : "EDIT"}
                </button>
                <button
                  type="button"
                  className="btn btn--mini"
                  onClick={() =>
                    void commit({ ...backend, enabled: !backend.enabled })
                  }
                >
                  {backend.enabled ? "SWITCH OFF" : "SWITCH ON"}
                </button>
                {forgetting === backend.id ? (
                  <>
                    <button
                      type="button"
                      className="btn btn--mini"
                      onClick={() => void drop(backend.id)}
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
                      Anyone thinking with it keeps the assignment and stops
                      being able to think.
                    </span>
                  </>
                ) : (
                  <button
                    type="button"
                    className="btn btn--mini"
                    onClick={() => {
                      setNotice(null);
                      setForgetting(backend.id);
                    }}
                  >
                    FORGET
                  </button>
                )}
              </div>

              {editing && draft && (
                <>
                  <Editor
                    draft={draft}
                    naming={false}
                    taken={backends.map((b) => b.id)}
                    onChange={setDraft}
                    onSave={() => void commit(draft)}
                  />
                  <Key
                    needed={kind?.needsKey ?? false}
                    stored={keyed.includes(backend.id)}
                    onSave={async (key) => {
                      const failure = await saveBackendKey(backend.id, key);
                      setNotice(failure);
                      if (!failure) {
                        await reload();
                        onProbe();
                      }
                    }}
                  />
                </>
              )}
            </li>
          );
        })}
      </ul>

      {adding && draft ? (
        <div className="conn__add">
          <Editor
            draft={draft}
            naming
            taken={backends.map((b) => b.id)}
            onChange={setDraft}
            onSave={() => void commit(draft)}
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
            setDraft({
              ...NEW,
              id: suggest(
                NEW.kind,
                backends.map((b) => b.id),
              ),
            });
          }}
        >
          ADD A BACKEND
        </button>
      )}

      <p className="notice" style={{ marginTop: 12 }}>
        OpenAI is not here yet. Adding one is an Engine change rather than an
        interface one — a Provider declares its own controls, so nothing on this
        screen needs editing to show them (ADR-0026).
      </p>
    </section>
  );
}

/**
 * The credential control.
 *
 * Deliberately its own block rather than a field in the form above: saving a key is a different
 * command from saving a backend, and it goes somewhere else entirely (the operating system's
 * encrypted store, never `providers.toml`). One button that did both would eventually write a
 * key into a file meant to be shared.
 *
 * The field is always empty on open. It is not that the value is hidden — **it is not here.**
 * There is no command that reads a key back, so there is nothing to prefill with, and a masked
 * field showing dots for a value the interface does not have would be a lie about where the key
 * lives.
 */
function Key({
  needed,
  stored,
  onSave,
}: {
  readonly needed: boolean;
  readonly stored: boolean;
  readonly onSave: (key: string) => Promise<void>;
}) {
  const [typed, setTyped] = useState("");

  return (
    <div className="cedit conn__edit">
      <label className="cedit__field cedit__field--wide">
        <span>{stored ? "REPLACE THE CREDENTIAL" : "CREDENTIAL"}</span>
        <input
          type="password"
          value={typed}
          autoComplete="off"
          spellCheck={false}
          placeholder={
            stored ? "stored — type a new one to replace it" : "none"
          }
          onChange={(e) => setTyped(e.target.value)}
        />
      </label>

      <div className="cedit__field cedit__field--wide">
        <p className="conn__blurb">
          {needed
            ? "This backend cannot be asked anything without one."
            : "Not needed on a local machine — this is for one reached across a network, behind something that asks."}{" "}
          It is kept in Windows&rsquo; own encrypted store, tied to your account
          — not in any file Epoch would ever share, and not in a character.
        </p>
        <div className="conn__acts" style={{ marginTop: 0 }}>
          <button
            type="button"
            className="btn"
            disabled={typed.trim().length === 0}
            onClick={() => void onSave(typed).then(() => setTyped(""))}
          >
            {stored ? "REPLACE" : "STORE"}
          </button>
          {stored && (
            <button
              type="button"
              className="btn btn--mini"
              onClick={() => void onSave("").then(() => setTyped(""))}
            >
              REMOVE IT
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

/**
 * The status half of a row. Three states, and they are genuinely three.
 *
 * OFF is the user's own choice and is not a fault. UNKNOWN is what a backend reads before
 * anything asked it — borrowing the previous answer would be the invented gauge this bridge
 * exists not to have.
 */
function Status({
  backend,
  probed,
  probing,
}: {
  readonly backend: Backend;
  readonly probed: ProviderStatus | undefined;
  readonly probing: boolean;
}) {
  if (!backend.enabled) return <span className="conn__status">OFF</span>;
  if (probing) return <span className="conn__status">PROBING…</span>;
  if (!probed) return <span className="conn__status">UNKNOWN</span>;
  return (
    <span className={`conn__status${probed.online ? " conn__status--on" : ""}`}>
      {probed.online ? "ONLINE" : "OFFLINE"}
    </span>
  );
}

/**
 * The form. The same one for adding and editing, minus the name.
 *
 * `naming` is off when editing because a character's brain names a backend by id: changing it
 * would point every one of them at something that no longer exists, without saying so.
 */
function Editor({
  draft,
  naming,
  taken,
  onChange,
  onSave,
}: {
  readonly draft: Backend;
  readonly naming: boolean;
  /** Names already in use, so a suggestion never collides. */
  readonly taken: readonly string[];
  readonly onChange: (next: Backend) => void;
  readonly onSave: () => void;
}) {
  const kind = KINDS.find((k) => k.id === draft.kind);

  return (
    <div className="cedit conn__edit">
      {naming && (
        <label className="cedit__field">
          <span>NAME</span>
          <input
            value={draft.id}
            placeholder="laptop"
            onChange={(e) => onChange({ ...draft, id: e.target.value })}
          />
        </label>
      )}

      <label className="cedit__field">
        <span>KIND</span>
        <select
          value={draft.kind}
          onChange={(e) => {
            const next = e.target.value as BackendKind;
            const usual =
              KINDS.find((k) => k.id === next)?.usual ?? draft.endpoint;
            // Only offer the usual address when nothing has been typed over it, or a kind
            // change would quietly discard somebody's own endpoint.
            const kept = KINDS.some((k) => k.usual === draft.endpoint)
              ? usual
              : draft.endpoint;
            // Same rule for the name: re-suggest only while it is still a suggestion. A name
            // the user typed survives changing the kind, because it was their answer.
            const named = KINDS.some((k) => suggest(k.id, taken) === draft.id)
              ? suggest(next, taken)
              : draft.id;
            onChange({
              ...draft,
              kind: next,
              endpoint: kept,
              id: naming ? named : draft.id,
            });
          }}
        >
          {KINDS.map((k) => (
            <option key={k.id} value={k.id}>
              {k.name}
            </option>
          ))}
        </select>
      </label>

      <label className="cedit__field cedit__field--wide">
        <span>ADDRESS</span>
        <input
          value={draft.endpoint}
          placeholder={kind?.usual}
          onChange={(e) => onChange({ ...draft, endpoint: e.target.value })}
        />
      </label>

      <div className="cedit__field cedit__field--wide">
        <p className="conn__blurb">
          {kind?.blurb}{" "}
          {kind?.where
            ? (() => {
                const where = kind.where;
                return (
                  <button
                    type="button"
                    className="conn__link"
                    onClick={() => void openLink(where.url)}
                    title={where.url}
                  >
                    {where.label}
                  </button>
                );
              })()
            : null}{" "}
          {naming &&
            "The name is how your crew will refer to it, and it cannot be changed later."}
        </p>
        <button type="button" className="btn" onClick={onSave}>
          SAVE
        </button>
      </div>
    </div>
  );
}
