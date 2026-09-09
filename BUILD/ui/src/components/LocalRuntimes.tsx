import { useEffect, useState } from "react";

import {
  importWeights,
  installRuntime,
  lendAllToLmStudio,
  localRuntimes,
  sharedWeights,
  startRouter,
  startRuntime,
  runtimeEngines,
  chooseEngine,
  compressCache,
  proveRuntime,
  type RuntimeEngines,
  type LocalRuntime,
  type SharedWeights,
} from "../ipc/launcher";

/**
 * The other things on this machine that can run a model.
 *
 * ## Why they are not a new kind of Provider
 *
 * llama.cpp and LM Studio both serve an **OpenAI-compatible API**, and Epoch has spoken that
 * since Phase 8 — the whole reason `openai` is a `Kind` is that adding a backend should be a
 * form rather than a pull request. So this panel measures and offers; nothing here talks to a
 * model, and adding one as a Service is the ordinary Connections form with the address below.
 *
 * ## Three facts, three fixes
 *
 * **Installed** · **serving** · **where**. Install it, start it, or nothing at all — and a
 * machine with LM Studio installed and its server switched off is not a machine without LM
 * Studio. Telling somebody to install what they already have is exactly the failure keeping
 * these apart prevents (the same discipline ADR-0027 applies to agents).
 *
 * ## Serving is measured by asking
 *
 * A runtime is usable when its API answers, however it got there. Finding a binary answers the
 * other question — whether there is something to start.
 *
 * ## Nothing is adopted, because nothing needs to be
 *
 * A runtime that answers **is** a Service, on the same terms as one on a paired machine: the
 * registry builds a Provider for whatever is serving and stops offering it when it stops. There
 * was a button here that wrote a backend entry saying what had already been measured, and it
 * existed only because the local case had been built before the remote one.
 *
 * ## Installing is offered, never done
 *
 * Epoch opens the door and steps back: the command runs in the user's own terminal, where the
 * licence, the elevation prompt and the output belong to the person reading them. Nothing here
 * claims success either — a terminal opened is all it can honestly say, so ASK AGAIN is how you
 * find out.
 */
export function LocalRuntimes({
  onChanged,
  onShowing,
  onRead,
}: {
  readonly onChanged?: () => void;
  /**
   * This panel has taken its own reading of what is serving.
   *
   * Separate from {@link onChanged}, which is the ASK AGAIN press and deliberately asks a wider
   * question. This one fires on the automatic read when the deck opens, so the summary above
   * and the Launcher's own panel are not left describing an older moment than this one.
   */
  readonly onRead?: () => void;
  /**
   * Which runtimes this panel is reporting, so nothing else repeats them.
   *
   * Told rather than assumed: the readiness list below would otherwise have to know which ids
   * are local runtimes, and two components deciding that separately is how they come to
   * disagree the first time a fourth one is added.
   */
  readonly onShowing?: (ids: readonly string[]) => void;
} = {}) {
  const [runtimes, setRuntimes] = useState<readonly LocalRuntime[] | null>(null);
  const [asking, setAsking] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  /**
   * What Ollama already has on disk, as files another runtime can open.
   *
   * **Ollama stores unmodified GGUF**, measured: the blob a manifest marks as the model begins
   * `GGUF`, and `llama-server -m <blob>` loaded `qwen3:14b` and answered in 2.4 s on the card.
   * So llama.cpp costs no download and no second copy of nine gigabytes — which was the real
   * reason a second runtime sat installed and empty.
   */
  const [weights, setWeights] = useState<readonly SharedWeights[]>([]);
  /**
   * Which GPU backend each runtime has here.
   *
   * Its own read because it is its own probe — a directory listing and one CLI call — and
   * because the deck's main reading must not grow one (the same reason TIME IT's runtimes are
   * asked for separately).
   */
  const [backends, setBackends] = useState<readonly RuntimeEngines[]>([]);
  /** Which model is picked for which runtime, before it is started. User intent only. */
  const [picked, setPicked] = useState<Record<string, string>>({});

  const read = (tell = false) => {
    setAsking(true);
    void localRuntimes().then((all) => {
      setRuntimes(all);
      setAsking(false);
      onShowing?.(all.map((one) => one.id));
      // Only the automatic read tells anybody. `reask` reports through `onChanged`, which asks
      // a wider question on purpose; firing both would make one press measure twice.
      if (tell) onRead?.();
    });
    // Both shelves: what Ollama pulled, and what SAVE THE FILE wrote into the vault. The
    // second one used to reach nothing at all.
    void sharedWeights().then(setWeights);
    void runtimeEngines().then(setBackends);
  };

  // Told, because this reading is fresher than the one the Launcher took when Epoch opened,
  // and both are answers to the same question. A user starting llama.cpp saw it SERVING here
  // and missing from CREW LINKS, which is two measurements of one fact left to drift.
  useEffect(() => {
    read(true);
    // Mount only: `read` is redefined every render and this is the deck opening, not a change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const start = async (runtime: LocalRuntime) => {
    setSaid(null);
    const failed = await startRuntime(runtime.id);
    if (failed !== null) {
      setSaid(failed);
      return;
    }
    setSaid(
      `A terminal opened on ${runtime.name}. Once it says it is listening, press ASK AGAIN.`,
    );
    /*
      **And this is where a compressed cache earns the right to be kept.**

      It is proved against the server that was *just started with it* — never the one running a
      moment ago, which was started without it and would prove nothing (11.18: measuring a path
      Epoch does not take). The Engine answers `null` when there was nothing to prove, which is a
      different thing from a failure and must not print like one.
    */
    const proved = await proveRuntime(runtime.id);
    if (proved !== null) {
      setSaid(proved);
      void runtimeEngines().then(setBackends);
    }
  };

  /*
    **ASK AGAIN re-reads the Services too**, because a runtime that started is a Provider that
    now exists. The registry builds them from what is serving, so the answer to *what can think*
    changed the moment this reading did — and two lists disagreeing about that is exactly what
    made a paired machine's LM Studio invisible for a whole session.
  */
  const reask = () => {
    read();
    onChanged?.();
  };

  /**
   * Hand a runtime everything this machine has.
   *
   * One action for two runtimes because the answer is the same shape — *all of them* — and only
   * the mechanism differs: llama.cpp is started as a router over a shelf, LM Studio has the
   * shelf stocked and loads from it itself. The Engine knows which; this asks for the outcome.
   */
  const everything = async (runtime: LocalRuntime) => {
    setSaid(null);
    setSaid(
      runtime.id === "llama_cpp"
        ? await startRouter()
        : await lendAllToLmStudio(),
    );
  };

  const bring = async (path: string) => {
    setSaid(null);
    const failed = await importWeights(path);
    setSaid(
      failed ??
        "A terminal opened on Ollama. It hashes the whole file to see whether it already has it — about three and a half minutes for nine gigabytes — and costs no disk if it does. When it says `success`, press ASK AGAIN.",
    );
  };

  const install = async (runtime: LocalRuntime) => {
    setSaid(null);
    const failed = await installRuntime(runtime.id);
    setSaid(
      failed ??
        `A terminal opened on ${runtime.install}. When it finishes, press ASK AGAIN.`,
    );
  };

  return (
    <div className="rm">
      <div className="hfd__head">
        <span className="rm__label">Other runtimes here</span>
        <button
          type="button"
          className="btn btn--mini"
          disabled={asking}
          onClick={reask}
        >
          {asking ? "ASKING…" : "ASK AGAIN"}
        </button>
      </div>

      {runtimes === null ? (
        <p className="cc__hint">Asking this machine…</p>
      ) : (
        <ul className="hfd__list">
          {runtimes.map((runtime) => (
            <li key={runtime.id} className="hfd__row">
              <span className="hfd__where">{runtime.name}</span>
              <span className={`hfd__state hfd__state--${state(runtime)}`}>
                {said_of(runtime)}
              </span>

              {/* Where it was found, so "not installed" is a fact somebody can check. */}
              {runtime.foundAt && <em className="hfd__at">{runtime.foundAt}</em>}

              {/*
                **Measured, and it looked like a bug in Epoch.** `lms server start` starts
                `LM Studio.exe --run-as-service`, and the moment it loads a model it spawns
                `llama-server.exe` — LM Studio's inference runtime *is* llama.cpp. So a machine
                running both shows two processes with the same name, and the reasonable reading
                is that Epoch pressed the wrong button. It did not, and this is the cheapest
                possible place to say so: one sentence next to the row it explains.
              */}
              {/*
                **What it will actually run the model on, in the program's own words.**

                Only llama.cpp answers this (`--list-devices`), and the answer turned out to
                matter more than anything else on this deck: the same GGUF took 36.7 s through
                Epoch's llama.cpp and 11.3 s through LM Studio — same card, same engine, because
                one was Vulkan and the other CUDA. Somebody who installed llama.cpp from the
                button above is getting a third of their card with nothing on screen to say so.
              */}
              {runtime.devices.length > 0 && (
                <em className="hfd__at">{runtime.devices.join(" · ")}</em>
              )}
              {runtime.handicap && (
                <span className="rt__offer">
                  <code>{runtime.handicap}</code>
                </span>
              )}

              {/*
                **Which GPU backend it will use, read from what is installed.**

                A per-card CUDA · Vulkan · Auto chooser was refused once and was right to be
                (11.20): a menu assembled from what is *conceivable* eventually offers somebody
                `Intel Arc: CUDA`. Every row here is read from this machine — Ollama's own
                backend directory, `lms runtime ls` — so a machine with one engine gets one row
                and nobody is offered something that does not exist.

                Three programs, three answers, and the differences are the content: Ollama holds
                four backends and is told which at start; LM Studio holds several and keeps the
                choice itself; llama.cpp carries one per install, so its answer is a sentence.
              */}
              <RuntimeBackend
                found={backends.find((it) => it.id === runtime.id)}
                onChose={async (engine) => {
                  setSaid(await chooseEngine(runtime.id, engine));
                  void runtimeEngines().then(setBackends);
                }}
                onCompress={async (on) => {
                  setSaid(await compressCache(runtime.id, on));
                  void runtimeEngines().then(setBackends);
                }}
              />

              {/*
                **Why this row has no START, said on the row.**

                Asked by the owner, who looked for llama.cpp's start button and could not find
                it. Both reasons are good and neither was visible: it has no plain START because
                starting it bare gives a server with `Available models (0)` — measured — and its
                real start is hidden while it is already up, because a router that is running
                knows the shelf.

                So the row next to it offers START and this one offers nothing, with nothing
                saying why. That is the cold-instrument rule arriving at a *button*: a panel with
                nothing behind it keeps its frame and explains, and an absence with no
                explanation reads as something missing rather than something deliberate.
              */}
              {runtime.id === "llama_cpp" && runtime.serving && (
                <em className="hfd__at">
                  already serving {runtime.models.length}{" "}
                  {runtime.models.length === 1 ? "model" : "models"} — its start is START WITH
                  EVERYTHING, and it appears here when it stops
                </em>
              )}

              {runtime.id === "lm_studio" && runtime.serving && (
                <em className="hfd__at">
                  runs models through llama.cpp — its process is called llama-server.exe too
                </em>
              )}

              {/*
                **Start it from here.** A server Epoch spawned and owned would die when Epoch
                does and print where nobody can read it, so this opens the machine's own
                terminal on the command — the same door as installing.

                Offered only while it is *not* answering: a START beside something already
                serving is a button whose job is already done.
              */}
              {/*
                **llama.cpp has no plain START any more**, and that is a removal rather than an
                omission: starting it bare gives a server with `Available models (0)` — measured
                — which is a Service that answers and can never be used. Its START *is* the
                router, one line down.
              */}
              {runtime.start !== null && !runtime.serving && runtime.id !== "llama_cpp" && (
                <span className="rt__offer">
                  <button
                    type="button"
                    className="btn btn--mini"
                    onClick={() => void start(runtime)}
                  >
                    START
                  </button>
                  <code>{runtime.start}</code>
                </span>
              )}

              {/*
                **What each runtime can be lent, and what lending means to it.**

                Three programs, three verbs, one shape — because the differences are real and
                naming them is the whole content of this control:

                - **llama.cpp** is *started holding* a file. It takes a path on the command line,
                  so this is a server that does not exist yet.
                - **LM Studio** is *lent* one. It loads from its own shelf on its own terms, so
                  the model is put on that shelf and the application does the rest.
                - **Ollama** is *given* one. It stores by content hash, so a file it has never
                  seen costs exactly that file and one it already holds costs nothing at all.

                None of them is a download. Every model here is already on this disk — from
                Ollama's own store, or from SAVE THE FILE in the Workshop — which was the thing
                that made a second runtime look expensive when it is free.
              */}
              <Lend
                runtime={runtime}
                weights={weights}
                picked={picked[runtime.id] ?? ""}
                onPick={(path) => setPicked({ ...picked, [runtime.id]: path })}
                onImport={bring}
                onAll={everything}
              />

              {/*
                **Two readings, because they are two facts.**

                `models` is what the server *offers* — a shelf. It used to be printed here as
                "holding …", so a llama.cpp router listing five files on disk reported five
                models in memory, and somebody looking at a 7.5 GB process in Task Manager was
                being told something that had never been measured.

                `resident` is the card. Empty says IN MEMORY: nothing, out loud, rather than
                disappearing — a gauge that vanishes when it reads zero is one nobody can trust
                when it reads something.
              */}
              {runtime.serving && (
                <span className="rt__offer">
                  <code>
                    {runtime.endpoint} &middot; your crew can be assigned to it
                    {runtime.models.length > 0
                      ? ` · offers ${runtime.models.join(", ")}`
                      : " · nothing on its shelf"}
                    {" · in memory: "}
                    {runtime.resident.length > 0
                      ? runtime.resident.join(", ")
                      : "nothing"}
                  </code>
                </span>
              )}

              {!runtime.installed && (
                <span className="rt__offer">
                  <button
                    type="button"
                    className="btn btn--mini"
                    onClick={() => void install(runtime)}
                  >
                    INSTALL
                  </button>
                  <code>{runtime.install}</code>
                </span>
              )}
            </li>
          ))}
        </ul>
      )}

      {said && <p className="notice">{said}</p>}

      <p className="cc__hint">
        All three speak an API Epoch already speaks, so none of them needs
        anything added to this build — only to be running. One that is answering
        is already a Service your crew can be assigned to, exactly as a paired
        machine&rsquo;s is. Both can use the models Ollama already has — they
        are ordinary GGUF files, linked rather than copied, so nothing is
        downloaded twice and Ollama keeps its own.
      </p>
    </div>
  );
}

/**
 * What one runtime can be lent from this machine's own disk.
 *
 * ## One shape rather than three
 *
 * The three offers were converging on the same block with a different verb, which is the point
 * at which they should be one thing: a list of models this machine already has, and what each
 * program would do with the one that is chosen. Keeping them apart would have meant fixing the
 * same sentence three times, and forgetting once.
 *
 * ## What each of them is offered, and when
 *
 * **llama.cpp** takes a path on the command line, so it can be *started holding* anything on
 * the disk — but only while it is not already serving, because a server that is up cannot be
 * handed a different model without being restarted.
 *
 * **LM Studio** loads from its own shelf, so the offer is to *put* a model there. Offered
 * whether or not it is serving: stocking a shelf is not starting a server.
 *
 * **Ollama** is offered only what it does not already have. Importing one of its own blobs
 * would spend three and a half minutes hashing to produce a second name for one model —
 * measured, and the manifest is all it would add.
 */
/**
 * What one runtime can be given from this machine's own disk.
 *
 * ## Two of them stopped asking *which*
 *
 * llama.cpp and LM Studio were each handed **one model**, chosen from a dropdown — a question
 * nobody wants to answer before they know what they are going to ask, and the reason those two
 * felt like configuration while Ollama felt like a program.
 *
 * Measured 2026-08-21, and it turned out neither ever needed the question:
 *
 * - `llama-server --models-dir <shelf>` is a **router**: it reports every model in a directory,
 *   loads one only when a request names it, and leaves the rest available. Two models listed,
 *   `"Blue"` in 21.2 s from the one asked for, the other still merely on offer.
 * - **LM Studio** loads from its own shelf on its own terms, so what it needs is for everything
 *   to be *there*.
 *
 * Both are hard links, so a shelf of five models adds bytes for none of them and Ollama keeps
 * its own copy — the same file under two names.
 *
 * ## Ollama is the one that still picks
 *
 * And it should: importing costs three and a half minutes of hashing per model, so *all of them*
 * would be an hour somebody did not ask for. It is offered only what it does not already have.
 */
function Lend({
  runtime,
  weights,
  picked,
  onPick,
  onImport,
  onAll,
}: {
  readonly runtime: LocalRuntime;
  readonly weights: readonly SharedWeights[];
  readonly picked: string;
  readonly onPick: (path: string) => void;
  readonly onImport: (path: string) => void | Promise<void>;
  readonly onAll: (runtime: LocalRuntime) => void | Promise<void>;
}) {
  if (!runtime.installed) return null;

  /*
    **A machine with no models says so, rather than offering nothing.**

    Reported from the AMD machine: llama.cpp installed, its card listed on the row above, and no
    way to start it anywhere on the deck. The cause is here — with an empty shelf this returned
    `null` and the whole control vanished.

    The guard was right and silent. llama.cpp's start *is* the router, and a router over an empty
    shelf is a server with `Available models (0)` — which is exactly why the plain START was
    removed. But a person who has just installed llama.cpp through Epoch, on Epoch's own
    recommendation, is owed the reason: an absence with nothing saying why reads as something
    broken.
  */
  /*
    **A runtime's own cache is a model on this machine, and Epoch was not asking.**

    Reported from the AMD machine: a model downloaded with `llama download`, `llama serve` in a
    terminal reporting `Available models (1)`, and this row saying *there are none* — on a row
    whose own header read `SERVING · 1 model`. Two readings of one machine contradicting each
    other, side by side.

    `everything_here` looks at Ollama's store and Epoch's shelf. llama.cpp keeps its own cache
    (`--cache-list`), which is neither — so the shelf being empty was never the same question as
    the machine having nothing.

    Three states now, and each is a different sentence: it is already serving; it has something
    of its own to serve; it has nothing anywhere.
  */
  if (weights.length === 0) {
    const all = TAKES_EVERYTHING[runtime.id];
    if (!all) return null;
    // Already serving. Whatever it is offering, this row is not the place to be told there is
    // nothing — the header one line up says how many.
    if (runtime.serving) return null;
    if (runtime.cached.length > 0) {
      return (
        <span className="rt__offer">
          <button
            type="button"
            className="btn btn--mini"
            onClick={() => void onAll(runtime)}
          >
            {all.verb}
          </button>
          <code>
            {runtime.cached.length}{" "}
            {runtime.cached.length === 1 ? "model" : "models"} of its own:{" "}
            {runtime.cached.join(", ")}
          </code>
        </span>
      );
    }
    return (
      <span className="rt__offer">
        <em className="hfd__at">
          nothing to serve yet &mdash; {runtime.name} offers the models on this machine, and there
          are none. WORKSHOP is where one arrives; MODELS only lists what is already here.
        </em>
      </span>
    );
  }

  const all = TAKES_EVERYTHING[runtime.id];
  if (all) {
    // A running router already knows the shelf; restarting it to say so again would be a button
    // whose job is done. LM Studio's shelf can always be restocked — it is not a server.
    if (all.onlyWhenStopped && runtime.serving) return null;
    return (
      <span className="rt__offer">
        <button
          type="button"
          className="btn btn--mini"
          onClick={() => void onAll(runtime)}
        >
          {all.verb}
        </button>
        <code>
          {all.says} — {weights.length}{" "}
          {weights.length === 1 ? "model" : "models"}, linked rather than copied
        </code>
      </span>
    );
  }

  // Ollama: only what it does not already have, one at a time, because each one is minutes.
  const choices = weights.filter((held) => held.from !== "Ollama");
  if (runtime.id !== "ollama" || choices.length === 0) return null;
  const chosen = choices.find((held) => held.path === picked) ?? null;

  return (
    <span className="rt__offer">
      <select value={picked} onChange={(e) => onPick(e.target.value)}>
        <option value="">&mdash; give it a model saved by the Workshop &mdash;</option>
        {choices.map((held) => (
          <option key={held.path} value={held.path}>
            {held.name} · {(held.bytes / 1e9).toFixed(1)} GB · {held.from}
          </option>
        ))}
      </select>
      <button
        type="button"
        className="btn btn--mini"
        disabled={!picked}
        onClick={() => void onImport(picked)}
      >
        IMPORT IT
      </button>
      {/*
        **What it costs, before it is pressed.**

        Measured 2026-08-27, and it corrects what this code used to believe: Ollama does not
        link a GGUF it is given, it stores its own rewritten copy. The same model was on the
        disk twice, 21.2 GB, with nothing here saying that would happen.

        Epoch cannot avoid the copy — a blob is named by the hash of *Ollama's* version, which
        is not the file Epoch has, and writing the manifest by hand would mean guessing the
        chat template. So the cost is stated instead, from the size already read.
      */}
      {chosen && <code>{importCost(chosen.bytes)}</code>}
    </span>
  );
}

/**
 * What importing one model into Ollama costs.
 *
 * The file's own size, because that is exactly what Ollama's copy weighs. Kept in the surface
 * rather than fetched: it is arithmetic on a number already on the row, and a round trip to be
 * told the size of something already displayed would be a second answer to one question.
 */
function importCost(bytes: number): string {
  return (
    `Ollama keeps its own copy — this adds ${(bytes / 1e9).toFixed(1)} GB and takes a few ` +
    "minutes. The model already works with llama.cpp and LM Studio from where it is."
  );
}

/** The two that take the whole shelf, and what pressing it means to each. */
const TAKES_EVERYTHING: Record<
  string,
  { verb: string; says: string; onlyWhenStopped?: boolean }
> = {
  llama_cpp: {
    verb: "START WITH EVERYTHING",
    says: "Serves every model this machine has, loading one when a character asks",
    // A router that is already up knows the shelf. Restarting it says nothing new.
    onlyWhenStopped: true,
  },
  lm_studio: {
    verb: "LEND IT EVERYTHING",
    says: "Puts every model this machine has on LM Studio's shelf",
  },
};

/** Which of the three facts is the one to report, worst first. */
function state(runtime: LocalRuntime): "absent" | "out" | "ready" {
  if (!runtime.installed && !runtime.serving) return "absent";
  // Installed and not answering is a real, ordinary state: the server is started from the
  // application or from a terminal, and saying "offline" would send somebody to reinstall it.
  if (!runtime.serving) return "out";
  return "ready";
}

function said_of(runtime: LocalRuntime): string {
  switch (state(runtime)) {
    case "absent":
      return "NOT INSTALLED";
    case "out":
      return "INSTALLED · NOT SERVING";
    default:
      return `SERVING · ${runtime.models.length} model${runtime.models.length === 1 ? "" : "s"}`;
  }
}

/**
 * Which GPU backend one runtime will use, and whether it holds the cache compressed.
 *
 * **Nothing here is a list of backends that exist in the world.** Every row was read off this
 * machine, which is what separates it from the chooser that was refused in 11.20 — that one
 * would have offered every card every option and eventually recommended a configuration that
 * cannot exist.
 */
function RuntimeBackend({
  found,
  onChose,
  onCompress,
}: {
  readonly found: RuntimeEngines | undefined;
  readonly onChose: (engine: string | null) => void;
  readonly onCompress: (on: boolean) => void;
}) {
  // Not asked yet, rather than nothing to say. The read lands a moment after the deck does.
  if (!found) return null;

  const engines = found.engines;
  return (
    <>
      {engines.kind === "fixed" && (
        // One backend per install, so this is a reading and not a control. Changing it means
        // installing a different build, which is what the row above already offers.
        <em className="hfd__at">
          this build is {engines.of} — a different one means installing another build
        </em>
      )}

      {engines.kind === "choice" && (
        <span className="rt__offer">
          <label className="rt__pick">
            GRAPHICS
            <select
              value={found.chose ?? ""}
              onChange={(e) => onChose(e.target.value === "" ? null : e.target.value)}
            >
              {/*
                **The empty row is the program's own detection**, which is what it does with
                nothing set. It is an absence rather than a value, so it is not one of the
                engines above it.
              */}
              <option value="">Let it choose</option>
              {engines.of.map((one) => (
                <option key={one.id} value={one.id}>
                  {one.name}
                  {one.chosen ? " — in use" : ""}
                </option>
              ))}
            </select>
          </label>
        </span>
      )}

      {found.canCompress && (
        <span className="rt__offer">
          <label className="rt__pick">
            <input
              type="checkbox"
              checked={found.compressedCache}
              onChange={(e) => onCompress(e.target.checked)}
            />
            COMPRESS THE ATTENTION CACHE
          </label>
          {/*
            Measured, and both halves are said: what it buys, and that it can fail outright.
            A quantized cache needs flash attention, which belongs to the backend — so this is
            proved by one real load after the next START, and switched back off if that fails.
          */}
          <em className="hfd__at">
            a 12B at 65,536 tokens fits on a 12 GB card with this on and spills without it —
            read when the server starts, and proved by a load
          </em>
        </span>
      )}
    </>
  );
}
