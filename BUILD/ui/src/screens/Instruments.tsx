/**
 * The bridge instruments — the panels down the right side and along the bottom.
 *
 * ## The rule these obey
 *
 * Every value here is either **measured** or **visibly zero**. Nothing is dressed.
 *
 * The reference design filled these with a working ship: LEVEL 42, REACTOR TEMP 41.2°C,
 * ACTIVE AGENTS 12/25, five providers ONLINE, four log entries. That is the right
 * *composition* and the wrong *data* — an earlier Launcher shipped an invented crew count and
 * it was spotted immediately, because a number nobody can explain is worse than no number.
 *
 * So the instruments stay, at their true readings. A reactor at 0 and a provider marked
 * OFFLINE still tell you the ship is real; they just tell you it is docked and cold. When the
 * subsystems below arrive, these panels light up without moving.
 *
 * ## What lights each one up
 *
 * Each placeholder carries a `PENDING:` note naming the subsystem that will supply it. They
 * are deliberately in one file so that list stays readable — this is the Launcher's ledger of
 * what Epoch has not built yet.
 */

import { useState } from "react";
import { agentLink } from "../experience/agentLink";

import { Mark } from "../components/Mark";
import { Pager, pageOf, slice } from "../components/Pager";
import type {
  CharacterSummary,
  LauncherView,
  MachineView,
  ProviderStatus,
  ShipsLogView,
} from "../ipc/contracts";
import type { AgentStatus } from "../ipc/launcher";

/* -------------------------------------------------------------------------- */
/* Providers                                                                   */
/* -------------------------------------------------------------------------- */

/** How each provider is drawn. Presentation only — identity comes from the Engine. */
const FALLBACK_LOOK = { hue: "#c79bf5", shape: "polygon(25% 0,100% 0,75% 100%,0 100%)" };

/**
 * A mark for each program, drawn rather than shipped.
 *
 * ## Why these are not the real logos
 *
 * Ollama, Anthropic, OpenAI, Google, llama.cpp and LM Studio all have their own artwork, and it
 * is **their trademark**. `CONTENT_PHILOSOPHY.md`'s hard rule governs what Epoch distributes —
 * the official build ships original, commissioned, CC0 or explicitly redistributable assets and
 * nothing else — and a brand mark is the clearest case of something that is none of those.
 *
 * So these are original marks: a shape and a hue per program, distinct enough to tell nine rows
 * apart at a glance, which is the job the row actually has. They are CSS rather than an asset
 * set for the reason the Launcher's icons are: a World Pack should be able to restyle this
 * surface without a baked-in icon set being the one thing it cannot replace.
 *
 * ## Keyed on the program, not on the configured id
 *
 * It was keyed on `ProviderStatus.id`, so every backend somebody added by hand — and every
 * Bridge — fell through to one purple placeholder. Nine rows, four of them identical. The id is
 * whatever the configuration called it; the **program** is the thing worth recognising, and it
 * has had its own field since a machine could offer three.
 */
const LOOK: Record<string, { hue: string; shape: string }> = {
  // A block, because it is the one that sits closest to the metal.
  ollama: { hue: "#cbbfa6", shape: "polygon(0 0,100% 0,100% 100%,0 100%)" },
  // A caret, for the thing everything else is built on top of.
  llama_cpp: { hue: "#e05f7a", shape: "polygon(50% 0,100% 100%,50% 68%,0 100%)" },
  // A lens: the one with a workbench around the model.
  lm_studio: { hue: "#8b7cf6", shape: "polygon(0 22%,100% 0,100% 78%,0 100%)" },
  anthropic: { hue: "#e0834a", shape: "polygon(50% 0,100% 50%,50% 100%,0 50%)" },
  openai: { hue: "#5fd6a4", shape: "circle(50%)" },
  google: {
    hue: "#6fb8ef",
    shape: "polygon(50% 0,62% 38%,100% 50%,62% 62%,50% 100%,38% 62%,0 50%,38% 38%)",
  },
};

/**
 * Which mark one row gets.
 *
 * The name is tried first because it is now the program (`Ollama`, `LM Studio`, `llama.cpp`),
 * then the id, so a hand-configured backend called `the studio box` still gets a mark if its id
 * says what it is. Falls through to the placeholder rather than inventing one — a row nobody can
 * account for is worse than a row that admits it.
 */
export function lookFor(one: { readonly id: string; readonly name?: string }) {
  const key = `${one.name ?? ""} ${one.id}`.toLowerCase();
  if (key.includes("lm studio") || key.includes("lm_studio")) return LOOK.lm_studio!;
  if (key.includes("llama.cpp") || key.includes("llama_cpp")) return LOOK.llama_cpp!;
  if (key.includes("ollama")) return LOOK.ollama!;
  /*
    **`llama` on its own, and it has to come after `ollama`.**

    A backend Epoch found or somebody added is named `llama`, not `llama.cpp` — so it matched
    nothing above and fell through to the fallback mark, which is the shape used for a program
    Epoch cannot place. Reported as "el logo de llama".

    Ordering is the whole of it: `ollama` contains `llama`, so a bare check placed one line
    earlier would give Ollama llama.cpp's mark and nobody would notice which of the two was wrong.
  */
  if (key.includes("llama")) return LOOK.llama_cpp!;
  if (key.includes("claude") || key.includes("anthropic")) return LOOK.anthropic!;
  if (key.includes("codex") || key.includes("openai") || key.includes("gpt")) return LOOK.openai!;
  if (key.includes("gemini") || key.includes("google")) return LOOK.google!;
  return FALLBACK_LOOK;
}


/*
  How many entries a card can hold before it stops being a card.

  Measured against the bridge rather than chosen: at more than this the Ship's Log ran to four
  screens and pushed the whole layout out of shape. Different numbers because the two cards are
  different heights — the console spends its top on four buttons.
*/
const PER_LOG_PAGE = 6;
const PER_CONSOLE_PAGE = 7;

/**
 * Crew links — who can currently do the thinking.
 *
 * Every row here is **measured**. The Engine asks each Provider whether it is reachable, and
 * reports what it found along with where it looked (ADR-0007) — so OFFLINE is a fact the user
 * can go and check, and ONLINE is never a claim nobody verified.
 *
 * The reference design shipped five providers all reading ONLINE with model names. This shows
 * the ones Epoch can actually speak to, at their real status, with the models they really have.
 */
export function CrewLinks({
  providers,
  agents,
  onManage,
  onRefresh,
  busy,
}: {
  readonly providers: readonly ProviderStatus[];
  /**
   * Agents this machine has, beside the backends.
   *
   * In the same panel because the question the panel answers is *who can the crew think with*,
   * and an agent is one of the answers. Its light means something different underneath — a
   * backend is reachable or is not, an agent is signed in or is not — but the two facts belong
   * to one instrument: a Launcher that lit up for Ollama and stayed silent about Claude Code
   * would be describing half the machine.
   */
  readonly agents: readonly AgentStatus[];
  readonly onManage: () => void;
  readonly onRefresh: () => void;
  readonly busy?: boolean;
}) {
  const online =
    providers.filter((p) => p.online).length + agents.filter((a) => a.signedIn === true).length;

  return (
    <section className={`pnl${online === 0 ? " pnl--dormant" : ""}`}>
      <div className="pnl__head">
        <h2 className="pnl__title">CREW LINKS</h2>
        {online > 0 ? (
          <span className="lamp" style={{ color: "var(--lx-green)" }} />
        ) : (
          <span className="dormant__tag" style={{ margin: 0 }}>
            ALL COLD
          </span>
        )}
      </div>
      <div className="pnl__rule" />

      {providers.length === 0 && agents.length === 0 ? (
        <p className="dormant">
          The Engine is not answering, so Epoch cannot say what is reachable. That is different
          from nothing being reachable, and it will not pretend otherwise.
        </p>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {providers.map((p) => {
            const look = lookFor(p);
            return (
              <div key={p.id} className="link" title={p.note ?? p.endpoint}>
                <span className="link__glyph">
                  <i
                    style={{ background: look.hue, clipPath: look.shape }}
                    /* Unlit unless it actually answered. */
                    className={p.online ? "link__lit" : undefined}
                  />
                </span>
                <span className="link__id">
                  <b>
                    {p.name}
                    {/*
                      **Which computer, when it is not this one.** Nine rows read `Ollama`,
                      `llama.cpp`, `LM Studio`, `Ollama`, `llama.cpp`, `LM Studio` — six entries,
                      two machines, and nothing on screen said which was which.

                      Only for remote ones: writing "on This PC" against everything local would
                      be four words of noise on the common case.
                    */}
                    {p.machine && <em className="link__where"> — {p.machine}</em>}
                  </b>
                  <i>
                    {p.online
                      ? p.models.length > 0
                        ? `${p.models.length} model${p.models.length === 1 ? "" : "s"} · ${p.models[0]}`
                        : "no models pulled"
                      : (p.note ?? p.endpoint)}
                  </i>
                </span>
                <span className={`link__status${p.online ? " link__status--on" : ""}`}>
                  {p.online ? "ONLINE" : "OFFLINE"}
                </span>
              </div>
            );
          })}

          {/*
            Signed in, not merely installed.

            Two facts with two fixes: a missing program is an install, a signed-out one is a
            sign-in. Collapsing them into OFFLINE would send somebody to do the wrong thing —
            which is exactly what happened when nothing here asked the question at all.
          */}
          {agents.map((a) => {
            // One decision, four surfaces. This panel is the one that kept showing the old
            // answer next to three that showed the new one.
            const link = agentLink(a);
            return (
              <div key={a.id} className="link" title={a.note ?? a.lookedIn ?? a.name}>
                <span className="link__glyph">
                  <i
                    style={{
                      background: lookFor(a).hue,
                      clipPath: lookFor(a).shape,
                    }}
                    className={link.lit ? "link__lit" : undefined}
                  />
                </span>
                <span className="link__id">
                  <b>{a.name}</b>
                  <i>{link.detail}</i>
                </span>
                <span className={`link__status${link.lit ? " link__status--on" : ""}`}>
                  {link.label}
                </span>
              </div>
            );
          })}
        </div>
      )}

      <div style={{ display: "flex", gap: 6, marginTop: 11 }}>
        <button type="button" className="btn btn--ghost" onClick={onRefresh} disabled={busy}>
          {busy ? "PROBING…" : "PROBE AGAIN"}
        </button>
        <button type="button" className="btn btn--ghost" onClick={onManage}>
          MANAGE
        </button>
      </div>
    </section>
  );
}

/* -------------------------------------------------------------------------- */
/* Diagnostics                                                                 */
/* -------------------------------------------------------------------------- */

function duration(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${d}d ${h}h ${m}m`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m ${seconds % 60}s`;
}

interface DiagnosticsProps {
  readonly view: LauncherView;
  /**
   * What this computer is, when it has been asked.
   *
   * `null` while the measurement is in flight, which is a third state and not the same as *no
   * card*: a gauge that read `—` before anything had been asked would be reporting an absence
   * nobody had looked for.
   */
  readonly machine?: MachineView | null;
}

/**
 * Ship diagnostics.
 *
 * The three gauges are **measured**: how much of the Engine's vocabulary the installed Worlds
 * actually cover, how many Places they describe, and how much of the crew is currently posted
 * somewhere. Those were always computed and, until this screen existed, read by nobody.
 */
export function Diagnostics({ view, machine = null }: DiagnosticsProps) {
  const problems = view.problems.length + view.definitionProblems.length
    + view.worlds.reduce((n, w) => n + w.problems.length, 0);

  const coverage = view.worlds.length
    ? view.worlds.reduce((n, w) => n + w.coverage, 0) / view.worlds.length
    : 0;

  const reactor = reactorLoad(machine);
  const places = view.worlds.reduce((n, w) => n + w.places, 0);
  const posted = view.characters.filter((c) => c.worlds.length > 0).length;
  const crew = view.characters.length;

  return (
    <section className="pnl">
      <h2 className="pnl__title">SHIP DIAGNOSTICS</h2>
      <div className="pnl__rule" />

      <div className="hull">
        <span>HULL</span>
        {problems === 0 ? (
          <b className="ok">ALL SYSTEMS NOMINAL</b>
        ) : (
          <b className="warn">
            {problems} FAULT{problems === 1 ? "" : "S"} LOGGED
          </b>
        )}
      </div>

      <Gauge
        label="WORLD COVERAGE"
        reading={`${Math.round(coverage * 100)}%`}
        fraction={coverage}
        tone="green"
      />
      <Gauge
        label="PLACES CHARTED"
        reading={`${places} across ${view.worlds.length}`}
        /* Five concepts per World is full charting — the Engine's own vocabulary size. */
        fraction={view.worlds.length ? places / (view.worlds.length * 5) : 0}
        tone="blue"
      />
      <Gauge
        label="CREW POSTED"
        reading={`${posted} / ${crew}`}
        fraction={crew ? posted / crew : 0}
        tone="violet"
      />

      <div className="readout">
        {/*
          **REACTOR LOAD — measured now, and it always could have been.**

          It read `REACTOR TEMP —` with a note promising GPU telemetry "once local models run on
          this machine". Local models have run on this machine since Phase 1, and the number was
          already being measured one panel away: `Machine::measure` reads VRAM through
          `nvidia-smi`, and the Workshop has weighed models against it for weeks.

          So the note was not describing a missing subsystem. It was describing a wire nobody
          had run, and a dark gauge beside a live reading of the same fact is the cold-instrument
          rule failing in the other direction — silence about something Epoch knows.

          **Load, not temperature.** Nothing here reads a thermometer, and a label naming a
          measurement that is not taken is the failure this panel exists to avoid. What the card
          is *holding* is the number that matters when a 9 GB model is about to be loaded onto it.

          Still `—` on a machine with no NVIDIA card: unknown is a reading, and an Apple machine's
          memory is unified rather than split.
        */}
        <div>
          <span>REACTOR LOAD</span>
          <b className={reactor.hot ? "warn" : undefined} title={reactor.why}>
            {reactor.reading}
          </b>
        </div>
        {/*
          **CONTEXT DRIFT is gone from this panel, and it is not a regression.**

          It waited on the Context Composer's report (ADR-0012), which now exists and is
          measured: `Knew` carries what was used, what the budget was, what was reserved for the
          reply and how much was dropped, per turn.

          Per *turn* is the problem. The reading belongs to a conversation, arrives as an event
          while one is running, and the World's HUD already shows it beside the character it
          describes. The Launcher is outside every World, so this row could never fill — a dark
          gauge waiting for a number that will never reach it is a gauge nobody can explain,
          which is the rule that put the `—` there in the first place, applied one step further.

          Where it lives instead: `components/hud/ContextGauge.tsx`.
        */}
        <div>
          <span>SESSION</span>
          <b>{duration(view.sessionSeconds)}</b>
        </div>
      </div>
    </section>
  );
}

/**
 * What the graphics card is holding, in the words a person choosing a model needs.
 *
 * **Used, not free** — the same direction every other gauge on this panel reads, and the
 * question somebody actually has is *how much of my card is gone*. `hot` at four fifths,
 * because that is where the next model stops fitting rather than where anything is wrong.
 *
 * `—` when nothing measured it. `nvidia-smi` is NVIDIA's, so an AMD or Intel card leaves this
 * unknown, and an Apple machine has no VRAM to have: its memory is unified, and splitting it
 * into an invented "video" share would describe a division the hardware does not have.
 */
export function reactorLoad(machine: MachineView | null): {
  reading: string;
  hot: boolean;
  why: string;
} {
  if (!machine?.vramTotal || machine.vramFree === null) {
    return {
      reading: "—",
      hot: false,
      why: machine?.gpu
        ? `${machine.gpu} did not report its memory.`
        : "No card on this machine reports its memory. Unknown is a reading.",
    };
  }
  const used = machine.vramTotal - machine.vramFree;
  const share = used / machine.vramTotal;
  return {
    reading: `${gb(used)} / ${gb(machine.vramTotal)}`,
    // Four fifths is where the next model stops fitting, not where anything is wrong.
    hot: share >= 0.8,
    why: `${machine.gpu ?? "This machine"} is holding ${gb(used)} of ${gb(
      machine.vramTotal,
    )}. Measured with nvidia-smi, not estimated.`,
  };
}

/** Bytes as a person reads them. */
function gb(bytes: number): string {
  return `${(bytes / 1e9).toFixed(1)} GB`;
}

function Gauge({
  label,
  reading,
  fraction,
  tone,
}: {
  readonly label: string;
  readonly reading: string;
  readonly fraction: number;
  readonly tone: "green" | "blue" | "violet";
}) {
  const pct = Math.max(0, Math.min(1, fraction)) * 100;
  return (
    <div className={`gauge gauge--${tone}`}>
      <div className="gauge__row">
        <span>{label}</span>
        <b>{reading}</b>
      </div>
      <div className="gauge__bar">
        <div className="gauge__fill" style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}

/* -------------------------------------------------------------------------- */
/* Ship's log                                                                  */
/* -------------------------------------------------------------------------- */

/**
 * The ship's log — the work itself, across every World.
 *
 * ## It was waiting for the wrong subsystem
 *
 * This panel used to say it was pending the Activity Recorder (ADR-0015), the durable
 * append-only log that is still deliberately unbuilt. That was true about *Activities* and
 * wrong about this: **Quests already persist to the vault**, with their whole chronicle. A
 * history that survives sessions has existed for a while; nothing was reading it.
 *
 * So a cold instrument turned out to be a connected one nobody had plugged in — which is the
 * standing rule about cold instruments read the other way round: they are for what does not
 * exist, never for what we were too incurious to connect.
 *
 * ## Failures are entries
 *
 * Blocked, Rejected, Abandoned and Failed appear exactly as they were. A log that kept only
 * successes would be propaganda (ADR-0025 §7), and the one moment somebody most needs this
 * panel is after something went wrong.
 *
 * Faults still show, because a World that will not load is also something that happened.
 */
export function ShipsLog({
  view,
  log,
}: {
  readonly view: LauncherView;
  readonly log: ShipsLogView | null;
}) {
  const [page, setPage] = useState(0);
  /**
   * Everything up to here has been read.
   *
   * **A marker, never a deletion.** These are real Quests, kept as what they were — a History
   * that could be emptied by a button would be one nobody could trust (ADR-0025 §7). Clearing
   * hides what has already been read; the Quests are untouched, and anything new appears above
   * it. Held for this session, because it is a fact about *reading*, not about the work.
   */
  const [readUpTo, setReadUpTo] = useState(0);
  const faults = [
    ...view.problems,
    ...view.definitionProblems,
    ...view.worlds.flatMap((w) => w.problems.map((p) => `${w.name}: ${p}`)),
  ];
  const quests = (log?.quests ?? []).filter((quest) => quest.at > readUpTo);
  const empty = quests.length === 0 && faults.length === 0;
  const at = pageOf(page, quests.length, PER_LOG_PAGE);

  return (
    <section className={`pnl${empty ? " pnl--dormant" : ""}`}>
      <div className="pnl__head">
        <h2 className="pnl__title">SHIP&rsquo;S LOG</h2>
        {quests.length > 0 && (
          <button
            type="button"
            className="pnl__clear"
            title="Put what you have read away. The Quests themselves are untouched."
            onClick={() => {
              setReadUpTo(Math.max(...quests.map((quest) => quest.at)));
              setPage(0);
            }}
          >
            CLEAR
          </button>
        )}
      </div>
      <div className="pnl__rule" />

      {empty ? (
        <>
          <div className="dormant__tag">NO ENTRIES</div>
          <p className="dormant">
            Nothing has happened yet. Board a World and start a Quest; what comes of it is
            written down and will still be here next time.
          </p>
        </>
      ) : (
        /*
          The faults sit above the page and are the same on every page, so they are counted into
          the reservation. Reserving six while drawing eight is how a full page came out taller
          than the page after it.
        */
        <ul
          className="log"
          style={
            {
              "--rows": PER_LOG_PAGE + Math.min(faults.length, 2),
            } as React.CSSProperties
          }
        >
          {faults.slice(0, 2).map((fault, i) => (
            <li key={`fault-${i}`}>
              <span className="log__dot log__dot--bad" />
              <span className="log__text">{fault}</span>
            </li>
          ))}
          {slice(quests, at, PER_LOG_PAGE).map((quest, i) => (
            <li key={`quest-${i}`}>
              <span className={`log__dot log__dot--${tone(quest.state)}`} />
              <span className="log__text" title={quest.title}>
                {quest.title}
                <em>
                  {" "}
                  · {quest.worldName} · {quest.state.replace(/_/g, " ")}
                  {/* Measured. A Quest that produced no evidence produced nothing, and the log
                      has to be able to say so. */}
                  {quest.evidence > 0 ? ` · ${quest.evidence} artifact${quest.evidence === 1 ? "" : "s"}` : ""}
                </em>
              </span>
            </li>
          ))}
        </ul>
      )}

      <Pager
        count={quests.length}
        perPage={PER_LOG_PAGE}
        at={at}
        onGo={setPage}
        label="Ship's log pages"
      />
    </section>
  );
}

/** How an outcome reads at a glance. Failure is an outcome, not an error. */
function tone(state: string): "good" | "bad" | "wait" {
  if (state === "completed") return "good";
  if (state === "blocked" || state === "rejected" || state === "abandoned" || state === "failed") {
    return "bad";
  }
  return "wait";
}

/* -------------------------------------------------------------------------- */
/* Bridge console                                                              */
/* -------------------------------------------------------------------------- */

interface ConsoleProps {
  readonly onNewWorld: () => void;
  /** What the crew has actually run. Read from the vault, never from this session. */
  readonly log: ShipsLogView | null;
}

/**
 * Quick actions.
 *
 * Only the ones that exist are live. A button that looks pressable and does nothing is worse
 * than one that is visibly not ready — it teaches the user that the bridge is a picture.
 */
export function BridgeConsole({ onNewWorld, log }: ConsoleProps) {
  const [page, setPage] = useState(0);
  /** Read up to here. A marker, not a deletion — the runs are evidence in a Quest (ADR-0025). */
  const [readUpTo, setReadUpTo] = useState(0);
  const runs = (log?.runs ?? []).filter((run) => run.at > readUpTo);
  const at = pageOf(page, runs.length, PER_CONSOLE_PAGE);
  const actions = [
    { label: "NEW\nWORLD", hue: "#6fb8ef", shape: "circle(50%)", ready: true, run: onNewWorld },
    // PENDING: needs the dialog + fs plugins to pick a folder and copy it into the vault's
    // `worlds/` — never `packs/`, which is the installer's folder and is replaced on every update.
    // A World written there once was left behind by an uninstall (epoch_engine::relocate).
    { label: "IMPORT\nWORLD", hue: "#5fd6a4", shape: "polygon(50% 100%,0 45%,32% 45%,32% 0,68% 0,68% 45%,100% 45%)", ready: false },
    // PENDING: needs a zip crate, and a decision on whether a backup carries the crew.
    { label: "BACKUP\nSHIP", hue: "#cbbfa6", shape: "polygon(0 12%,100% 12%,100% 88%,0 88%)", ready: false },
    // PENDING: the Observability surface over the Activity Stream (ADR-0015).
    { label: "SYSTEM\nREPORT", hue: "#c79bf5", shape: "polygon(0 0,72% 0,100% 22%,100% 100%,0 100%)", ready: false },
  ] as const;

  return (
    <section className="pnl">
      <h2 className="pnl__title">BRIDGE CONSOLE</h2>
      <div className="pnl__rule" />
      <div className="console">
        {actions.map((a) => (
          <button
            key={a.label}
            type="button"
            disabled={!a.ready}
            title={a.ready ? undefined : "Not built yet"}
            onClick={"run" in a ? a.run : undefined}
          >
            <span
              className="console__glyph"
              style={{ "--c": a.hue, "--g": `${a.hue}55`, clipPath: a.shape } as React.CSSProperties}
            />
            <span className="console__label">{a.label}</span>
          </button>
        ))}
      </div>
      {/*
        What the crew has actually run — the console's readout, under its controls.

        Every line is a capability a character executed, recorded as evidence in a Quest's
        chronicle (ADR-0025). Not a transcript and not a narration: if nothing ran, nothing is
        listed, and the panel says that rather than filling itself.
      */}
      <div className="console__feed">
        <div className="pnl__head">
          <div className="console__feed-head">RECENT ACTIVITY</div>
          {runs.length > 0 && (
            <button
              type="button"
              className="pnl__clear"
              title="Put what you have read away. What ran stays recorded as evidence."
              onClick={() => {
                setReadUpTo(Math.max(...runs.map((run) => run.at)));
                setPage(0);
              }}
            >
              CLEAR
            </button>
          )}
        </div>
        {runs.length > 0 ? (
          <ul
            className="console__runs"
            style={{ "--rows": PER_CONSOLE_PAGE } as React.CSSProperties}
          >
            {slice(runs, at, PER_CONSOLE_PAGE).map((run, i) => (
              <li key={i}>
                <span className="console__who">{run.character}</span>
                <span className="console__ran" title={run.summary}>
                  {run.summary}
                </span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="console__idle">
            {log && log.runs.length > 0
              ? "Nothing new. What ran before is still recorded as evidence."
              : "Nobody has run anything yet. This fills as the crew works."}
          </p>
        )}

        <Pager
          count={runs.length}
          perPage={PER_CONSOLE_PAGE}
          at={at}
          onGo={setPage}
          label="Recent activity pages"
        />
      </div>

      <div className="hintbar">
        HOLD <b>SHIFT</b> WHILE BOARDING TO SKIP THE DEPARTURE SEQUENCE.
      </div>
    </section>
  );
}

/* -------------------------------------------------------------------------- */
/* Tip of the day                                                              */
/* -------------------------------------------------------------------------- */

/**
 * Tips.
 *
 * Static authored copy, and honest as such — every line describes something Epoch actually
 * does today. A tip promising a feature that does not exist would be the same lie as a gauge
 * reading a temperature nothing measured.
 */
const TIPS = [
  "A character belongs to you, not to a World. Move them between Worlds from the Characters deck and their name, face and routine travel with them.",
  "Worlds degrade rather than break. One covering three of five concepts still opens — the rest appear as visible placeholders instead of nothing.",
  "Everything a World shows is authored in its pack.toml. Edit it while Epoch is running and the World reloads itself.",
  "Hold Shift while boarding to skip the departure sequence. Immersion should never cost you time.",
] as const;

/* -------------------------------------------------------------------------- */
/* Crew channel                                                                */
/* -------------------------------------------------------------------------- */


interface CrewChannelProps {
  readonly crew: readonly CharacterSummary[];
  /** Rotates with the tips, so the bridge is never completely still. */
  readonly index: number;
  readonly onRoster: () => void;
}

/**
 * The crew channel.
 *
 * The design had crew members reporting finished work — "Lucca completed research on Context
 * Compression". Nothing is running, so any such line would be fabricated, and fabricating
 * *work* is the one thing Build From Life rule 3 names outright.
 *
 * What is true is who they are and what their routine has them doing. So the channel reports
 * that, in the future tense it has earned: where you will find them, and doing what. When
 * execution exists this panel starts carrying real reports without changing shape.
 */
export function CrewChannel({ crew, index, onRoster }: CrewChannelProps) {
  const someone = crew.length ? crew[index % crew.length] : null;
  const doing = someone?.routine[0]?.activity;

  return (
    <section className={`pnl pnl--crew${someone ? "" : " pnl--dormant"}`}>
      <h2 className="pnl__title">CREW CHANNEL</h2>
      <div className="pnl__rule" />

      {someone ? (
        <div className="chan">
          <div className="chan__face">
            {someone.portrait ? (
              <svg viewBox="-50 -100 100 100" role="img" aria-label={someone.name}>
                <Mark
                  mark={someone.portrait}
                  footprint={
                    someone.portrait.asset && someone.portrait.scale > 0
                      ? 100 / someone.portrait.scale
                      : 100
                  }
                />
              </svg>
            ) : (
              <span className="chan__gem" aria-hidden />
            )}
          </div>
          <p className="chan__text">
            <b className="chan__who">{someone.name.toUpperCase()}</b>
            {someone.worlds.length === 0
              ? "is not posted to any World yet. Assign them a berth and they will be there when you board."
              : doing
                ? `is aboard. Board and you will find them ${doing}.`
                : "is aboard."}
          </p>
        </div>
      ) : (
        <>
          <div className="dormant__tag">NO CREW</div>
          <p className="dormant">
            Nobody is in the vault yet. A crew member is a file in{" "}
            <code>vault/definitions/characters</code>.
          </p>
        </>
      )}

      <button type="button" className="btn btn--violet" onClick={onRoster}>
        OPEN CREW ROSTER
      </button>
    </section>
  );
}

export function TipOfTheDay({ index }: { readonly index: number }) {
  return (
    <section className="pnl">
      <h2 className="pnl__title">TIP OF THE DAY</h2>
      <div className="pnl__rule" />
      <div className="tip">
        <div className="tip__lamp" aria-hidden>
          <span className="stalk" />
          <span className="bulb" />
          <span className="face">
            <i />
            <i />
          </span>
          <span className="base" />
        </div>
        <p className="tip__text">{TIPS[index % TIPS.length]}</p>
      </div>
      <div className="tip__dots" aria-hidden>
        {TIPS.map((_, i) => (
          <i key={i} className={i === index % TIPS.length ? "on" : undefined} />
        ))}
      </div>
    </section>
  );
}
