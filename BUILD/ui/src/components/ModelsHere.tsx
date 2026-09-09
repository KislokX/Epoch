/**
 * What is actually on this machine, how fast it answers, and how it will be loaded.
 *
 * ## Why this is not part of the Workshop
 *
 * The Workshop answers *what could be here* — a catalogue, a search, a download. This answers
 * *what is here*, and the two want opposite things on screen. A catalogue is ranked and
 * speculative; an inventory is a list of facts with a size beside each one, and it holds the
 * only irreversible button either deck has.
 *
 * ## Every reading is measured or absent
 *
 * A speed is shown only when something timed it, and it names the runtime it was timed on.
 * Nothing here is estimated — an estimate belongs beside a model you do not have yet, where
 * there is nothing better; a model sitting on the disk can simply be asked.
 *
 * A loadout says which it is: measured, or the safe default nobody has improved on. The default
 * is not a reading and does not dress as one.
 *
 * ## And the choice is the user's
 *
 * MEASURE maps the curve — context against speed, both caches at the base — and Epoch marks the
 * balanced row. Every other row stays selectable, because context against speed is a preference
 * and not a fact: the 27B's 24k row costs a third of its speed and is exactly what somebody
 * writing long documents wants.
 *
 * Rows too small to hold a real turn are shown and marked, never hidden. They are genuinely the
 * fastest and they genuinely stop mid-sentence (ADR-0033's rule for an incompatible part).
 */
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import {
  chooseLoadout,
  modelsAndLoadouts,
  removeModel,
  searchLoadout,
  timeModel,
  whoCanTime,
  whoCanMeasure,
  tuneModel,
  tuningWrites,
  benchmark,
  benchmarks,
  type ModelHere as Model,
  type TimeableOn,
  type MeasurableOn,
  type Tuning,
  type BenchCard,
  type BenchRun,
  type Configuration,
  type Intent,
  type Optimized,
  type WasAside,
  type Preflight,
  type Optimizing,
  type Quick,
  type PausedBenchmark,
  type Sweep,
  cleanStateCheck,
  forgetOptimization,
  pausedBenchmark,
  calibrate,
  STAGES,
  optimization,
  beforeBenchmarking,
  dismissPause,
  optimizeModel,
  quickTest,
  speculationTypes,
  stopOptimizing,
  sweepSpeculation,
  useMeasured,
  useProfile,
} from "../ipc/launcher";

/**
 * A runtime, named so two of them can be told apart.
 *
 * **A name on its own identifies nobody once a second machine lends a card.** Measured with a
 * router here and another on the MacBook: the picker read `llama.cpp`, `Ollama`, `llama.cpp` —
 * three correct entries, and nothing saying which computer any of them was on.
 *
 * The machine is shown only when there is one, which is only ever a lent machine: qualifying
 * *this* one would be an instrument that never moves.
 */
function whichOne(it: TimeableOn): string {
  return it.machine ? `${it.name} · ${it.machine}` : it.name;
}

interface ModelsHereProps {
  /**
   * A model the Workshop just downloaded and offered to measure.
   *
   * By name, because that is what the download event carries and what a person read on the
   * notice. The row is found once the deck's own read comes back — a model that landed a second
   * ago is not in a list fetched before it existed.
   */
  readonly measure?: string | null;
  /** Told when the errand has been taken, so it is not run twice. */
  readonly onMeasureTaken?: () => void;
  /**
   * A model to open the panel on, without measuring anything.
   *
   * Separate from `measure` because they are two errands and only one of them costs a graphics
   * card. A character panel's `Configured in MODELS →` is somebody asking to *see* where the
   * window comes from; starting a twenty-minute search on their behalf would be the worst
   * possible reading of that click.
   */
  readonly reveal?: string | null;
  /** Told when the row has been opened, so it is not re-opened over the user's own clicks. */
  readonly onRevealed?: () => void;
}

export function ModelsHere({
  measure,
  onMeasureTaken,
  reveal,
  onRevealed,
}: ModelsHereProps = {}) {
  const [held, setHeld] = useState<readonly Model[] | null>(null);
  const [timing, setTiming] = useState<string | null>(null);
  const [searching, setSearching] = useState<string | null>(null);
  /**
   * Exactly what each model's tuning writes into llama.cpp's preset.
   *
   * **Asked of the Engine rather than assembled here.** The owner asked what the parameters
   * behind `SPECULATIVE DECODING` actually are — the right question about a control whose values
   * are named after a technique rather than after what they do. A panel that built its own
   * version of those lines would eventually show one thing while the file said another, which is
   * how a benchmark came to report a compressed cache it never ran.
   */
  const [writes, setWrites] = useState<Record<string, string>>({});

  /**
   * A benchmark somebody asked for, held back because the machine is busy.
   *
   * **Advice, never a gate.** The search waits for a quiet machine on its own and refuses to
   * record a run taken through a busy one; what it cannot do is say so before somebody walks
   * away. `null` whenever the reading found nothing to say, so a quiet machine gets no panel and
   * no extra click.
   */
  const [waiting, setWaiting] = useState<{
    readonly model: string;
    readonly at: Preflight;
    readonly then: () => void;
  } | null>(null);

  /**
   * Take a reading first, and only interrupt where it found something.
   *
   * A machine that could not be asked is not a busy one: the work starts, and the search checks
   * again anyway.
   */
  const unlessBusy = (one: Model, then: () => void) => {
    void beforeBenchmarking().then((at) => {
      if (at === null || at.quiet) then();
      else setWaiting({ model: one.name, at, then });
    });
  };

  /**
   * What the search that just ended produced, so the answer is shown where the waiting happened.
   *
   * **`refused` is the whole reason this is a record and not a name.** The first version held the
   * model's name and the panel looked its optimization up — which is the *stored* one, from
   * whichever search last succeeded. So a run that refused at the gate, wrote nothing and paused
   * still drew `OPTIMIZATION COMPLETE · ★ BALANCED · 53.4 tok/s · APPLY` over the previous run's
   * record. Measured 2026-09-01, and it is the one thing a golden test exists to catch: a
   * recommendation offered on evidence this run did not produce.
   *
   * The outcome of a run is a property of that run. It is carried, never looked up.
   */
  const [finished, setFinished] = useState<{
    readonly model: string;
    readonly refused: string | null;
  } | null>(null);
  /** Which model is one click from being deleted. One at a time, never on the first click. */
  const [asking, setAsking] = useState<string | null>(null);
  /** Which model's curve is open. One at a time: two tables at once is a panel nobody reads. */
  const [open, setOpen] = useState<string | null>(null);
  const [said, setSaid] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  /**
   * How far the running search has got, in settings tried.
   *
   * **Reported by the Engine, never guessed here.** `Probe::run` is called once per setting, so
   * this is a count of things that happened; the total is the bound `loadout::how_many` derives
   * from the same ladder the search walks. A search that stops early fills the bar faster,
   * which is still the truth.
   */
  const [step, setStep] = useState<{ done: number; total: number } | null>(
    null,
  );

  useEffect(() => {
    const stop = listen<{ path: string; done: number; total: number }>(
      "models:measuring",
      (event) => setStep({ done: event.payload.done, total: event.payload.total }),
    );
    return () => {
      void stop.then((off) => off());
    };
  }, []);

  const look = () => {
    // **Asked once, used three times.** This read walks every GGUF header on the shelf, which
    // is 20.9 s on the owner's 80 GB one — and it was called twice on mount, because the second
    // caller wanted the same rows for a different reason and reached for the same function
    // rather than the answer it had already asked for. Two identical questions, back to back,
    // with the window unable to paint between them.
    void modelsAndLoadouts().then((rows) => {
      setHeld(rows);
      readProfiles(rows);
      // What each model's current tuning writes, so the box is right before anybody touches a
      // control rather than only after.
      rows.forEach((row) => {
        void tuningWrites(row.tuning).then((lines) =>
          setWrites((held) => ({ ...held, [row.name]: lines })),
        );
      });
    });
    // **Its own call, because it probes.** The read above touches nothing but the disk and a
    // file Epoch wrote; this asks three servers whether they are answering. Refreshed with the
    // deck rather than per row, so a list of twenty models is still three questions.
    void whoCanTime().then(setTimeable);
    void whoCanMeasure().then(setMeasurable);
    void benchmarks().then(setCards);
    void speculationTypes().then(setSpecTypes);
    void pausedBenchmark().then(setPaused);
  };

  /**
   * Re-read what has been measured for every model on the deck.
   *
   * Its own pass rather than a field on the deck row: it is a different store, it is read after a
   * search finishes as well as on mount, and folding it into the deck would mean re-probing three
   * servers to refresh a number that came out of a file.
   */
  const readProfiles = (rows: readonly Model[]) => {
    rows.forEach((row) => {
      void optimization(row.name).then((found) => {
        if (found === null) return;
        setOptimized((held) => ({ ...held, [row.name]: found }));
      });
    });
  };

  useEffect(look, []);

  /**
   * Which runtime each row would be timed on.
   *
   * **User intent, and it had none.** TIME IT used to take whichever provider answered first,
   * which is Ollama for every model on this machine's shelf — so the other two could not be
   * measured at all, while the answer named Ollama as if that had been the question. 11.19
   * measured 8.5 s, 10.5 s and 19.8 s for one short answer across the three: which runtime *is*
   * the interesting half of the number.
   *
   * Keyed by path rather than by name, like everything else on this deck: two rows can be the
   * same weights stored twice.
   */
  const [timeOn, setTimeOn] = useState<Record<string, string>>({});

  /** Which runtimes are answering, and what each can time. Empty until the probe lands. */
  const [timeable, setTimeable] = useState<readonly TimeableOn[]>([]);
  const [measurable, setMeasurable] = useState<readonly MeasurableOn[]>([]);
  /** Every benchmark this machine has taken, and which model is being benched now. */
  const [cards, setCards] = useState<readonly BenchCard[]>([]);
  const [benching, setBenching] = useState<string | null>(null);
  const [doing, setDoing] = useState<string | null>(null);
  /** Which runtime each row will be measured on, once somebody has chosen one. */
  const [measureOn, setMeasureOn] = useState<Record<string, string>>({});
  /**
   * The speculative decodings the installed llama.cpp offers.
   *
   * Asked once for the deck rather than carried on every row: it is a property of the build, not
   * of a model. Empty until it answers, which draws a control with only `off` in it — the honest
   * shape of *nobody has said yet*, and it fills in a moment later.
   */
  const [specTypes, setSpecTypes] = useState<readonly string[]>([]);
  /**
   * What has been measured for each model here, keyed by name.
   *
   * **Only what was measured on this machine.** The Engine refuses a result taken on another
   * card, so an empty entry means *nobody has measured this here* rather than *there is nothing
   * to know* — which is why the card says `not measured yet` instead of showing four plausible
   * profiles.
   */
  const [optimized, setOptimized] = useState<Record<string, Optimized>>({});
  /** Which model a search is running for, and how far it has got. */
  const [optimizing, setOptimizing] = useState<string | null>(null);
  const [searchStep, setSearchStep] = useState<Optimizing | null>(null);
  /** Which model's CONFIGURE panel is open, and which row's `\u22ef` was pressed. */
  const [configuring, setConfiguring] = useState<string | null>(null);
  const [advanced, setAdvanced] = useState<string | null>(null);
  const [flags, setFlags] = useState(false);
  /** The last quick reading, per model. */
  const [quick, setQuick] = useState<Record<string, Quick>>({});
  const [testing, setTesting] = useState<string | null>(null);
  /**
   * A search that stopped because the machine stopped reproducing itself.
   *
   * **Read when the deck opens**, so somebody coming back is told rather than having to remember
   * there was one. Kept out of the row and given its own panel: it is not a fact about a model,
   * it is a fact about the machine, and every row on the deck is affected by it.
   */
  const [paused, setPaused] = useState<PausedBenchmark | null>(null);
  const [checking, setChecking] = useState(false);
  /** Which model a sweep is running for, and what it has found. */
  const [sweeping, setSweeping] = useState<string | null>(null);
  const [sweep, setSweep] = useState<Sweep | null>(null);

  /** The runtimes that could time this row, in the order they were configured. */
  const runtimesFor = (one: Model) =>
    timeable.filter((it) => it.models.includes(one.name));

  /**
   * Which runtimes can map this model's curve.
   *
   * **A shorter list than TIME IT's, and deliberately.** Timing needs a runtime that answers;
   * mapping needs one whose answer is *applied* somewhere. LM Studio takes a context only at
   * load through its own CLI and Epoch does not load through it, so a curve there would end in
   * a setting nothing reads — the dead `Manual` mode of ADR-0027. The Engine decides that; this
   * only draws what it was given.
   */
  const measurableFor = (name: string) =>
    measurable.filter((it) => it.models.includes(name));

  const time = (one: Model) => {
    const on = timeOn[one.path] ?? runtimesFor(one)[0]?.id;
    if (on === undefined) return;
    setTiming(one.path);
    setSaid(null);
    void timeModel(one.name, on).then((answer) => {
      setTiming(null);
      setSaid(answer);
      look();
    });
  };

  /*
    **Subscribed once, before anything is asked for.** A subscription opened inside the click
    handler races the Engine, and the step that goes missing is always the first one — which is
    the one that says the benchmark started.
  */
  useEffect(() => {
    const stop = listen<{ model: string; what: string; done: number; total: number }>(
      "models:benching",
      ({ payload }) => {
        setDoing(
          payload.what === "speed"
            ? `timing it, run ${String(payload.done)} of ${String(payload.total)}`
            : `${payload.what} — ${String(payload.done)} of ${String(payload.total)}`,
        );
      },
    );
    return () => {
      void stop.then((off) => {
        off();
      });
    };
  }, []);

  /*
    The search's own channel, subscribed before anything is asked for — a subscription opened
    inside a click handler races the Engine, and the step that goes missing is always the first.
  */
  useEffect(() => {
    const stop = listen<Optimizing>("models:optimizing", ({ payload }) => {
      setSearchStep(payload);
    });
    return () => {
      void stop.then((off) => {
        off();
      });
    };
  }, []);

  /** Put one model through the Standard benchmark. Minutes, and it is asked for. */
  const bench = (one: Model, on: string) => {
    setBenching(one.name);
    setSaid(null);
    setDoing(null);
    void benchmark(one.name, on)
      .then(setSaid)
      .finally(() => {
        setBenching(null);
        setDoing(null);
        void benchmarks().then(setCards);
      });
  };

  /**
   * Measure every speculative decoding for one model, and show what each did.
   *
   * **Much longer than a benchmark**, because each configuration restarts llama.cpp and reloads
   * the model — so it is asked for explicitly and reports every step. The result is a table and
   * not a setting: Epoch puts back whatever the model had, because a measurement is Epoch's and
   * the choice is the user's (ADR-0033).
   */
  const runSweep = (one: Model) => {
    setSweeping(one.name);
    setSweep(null);
    setSaid(null);
    setDoing(null);
    void sweepSpeculation(one.name)
      .then((answer) => {
        if (typeof answer === "string") setSaid(answer);
        else setSweep(answer);
      })
      .finally(() => {
        setSweeping(null);
        setDoing(null);
        look();
      });
  };

  /**
   * BENCHMARK & OPTIMIZE: try what this build offers and record what each one did.
   *
   * **The long one.** Twenty-odd model loads, every step reported, and stoppable — a job of this
   * length with no way out is one somebody kills by closing the window.
   */
  const optimise = (one: Model) => {
    setOptimizing(one.name);
    setSearchStep(null);
    setSaid(null);
    setFinished(null);
    void optimizeModel(one.name)
      .then((answer) => {
        // Whatever came of it — a recommendation or a refusal — the person who waited is told,
        // and told about *this* run.
        if (typeof answer === "string") {
          setSaid(answer);
          setFinished({ model: one.name, refused: answer });
        } else {
          setOptimized((held) => ({ ...held, [one.name]: answer }));
          setFinished({ model: one.name, refused: null });
        }
      })
      .finally(() => {
        setOptimizing(null);
        setSearchStep(null);
        look();
      });
  };

  /**
   * QUICK TEST: how the configuration this model has right now behaves.
   *
   * Deliberately not the search. Under a minute against twenty, and it measures what is actually
   * set rather than looking for something better.
   */
  const test = (one: Model) => {
    setTesting(one.name);
    setSaid(null);
    void quickTest(one.name)
      .then((answer) => {
        if (typeof answer === "string") setSaid(answer);
        else setQuick((held) => ({ ...held, [one.name]: answer }));
      })
      .finally(() => {
        setTesting(null);
      });
  };

  /**
   * What applying a profile is about to do, said before it does it.
   *
   * **The owner's rule, 2026-09-02: this has to be said.** Writing the preset was never enough —
   * llama.cpp's router reads it once, at startup, so a profile only reaches a character when the
   * router is restarted. That restart costs about a minute and interrupts a turn in flight, and
   * both are things somebody should meet before pressing rather than after waiting.
   *
   * It describes the action, never its outcome: the Engine's own sentence says what actually
   * happened, and this one would be a claim nothing measured.
   */
  const RESTARTING =
    "Applying… llama.cpp restarts so the model actually loads this way — " +
    "about a minute, and anyone mid-answer will need asking again.";

  /** Write one profile's whole configuration out. */
  const apply = (one: Model, intent: Intent) => {
    /*
      **Held while it runs, which it did not need to be until it took a minute.**

      Applying used to write a file and return. It now stops and starts llama.cpp, so a second
      press lands inside the first restart and stacks two routers fighting for one port — which
      is exactly the state that produced three servers and four consoles during a search.

      And a refusal is caught: the Engine has a real failure to report here now (the router did
      not come back), and an uncaught rejection would leave the notice on screen saying it was
      still applying.
    */
    setBusy(true);
    setSaid(RESTARTING);
    void useProfile(one.name, intent)
      .then((answer) => {
        setSaid(answer);
        setConfiguring(null);
        void optimization(one.name).then((found) => {
          if (found !== null) setOptimized((held) => ({ ...held, [one.name]: found }));
        });
      })
      .catch((why: unknown) => setSaid(String(why)))
      .finally(() => {
        setBusy(false);
        look();
      });
  };

  /**
   * Put one measured configuration into effect, chosen rather than offered.
   *
   * Same path as a profile, including the re-read: what somebody chose by hand and what Epoch
   * recommended are the same kind of thing once applied, and a second way of writing it would be
   * a second thing to keep working.
   */
  const applyMeasured = (one: Model, at: number) => {
    /*
      **Held while it runs, which it did not need to be until it took a minute.**

      Applying used to write a file and return. It now stops and starts llama.cpp, so a second
      press lands inside the first restart and stacks two routers fighting for one port — which
      is exactly the state that produced three servers and four consoles during a search.

      And a refusal is caught: the Engine has a real failure to report here now (the router did
      not come back), and an uncaught rejection would leave the notice on screen saying it was
      still applying.
    */
    setBusy(true);
    setSaid(RESTARTING);
    void useMeasured(one.name, at)
      .then((answer) => {
        setSaid(answer);
        setConfiguring(null);
        void optimization(one.name).then((found) => {
          if (found !== null) setOptimized((held) => ({ ...held, [one.name]: found }));
        });
      })
      .catch((why: unknown) => setSaid(String(why)))
      .finally(() => {
        setBusy(false);
        look();
      });
  };

  /** Say how a model should run, and re-read the deck so the row shows what was written. */
  const tell = async (one: Model, tuning: Tuning) => {
    setSaid(await tuneModel(one.name, tuning));
    // What that choice becomes in the preset, from the function that writes it.
    void tuningWrites(tuning).then((lines) =>
      setWrites((held) => ({ ...held, [one.name]: lines })),
    );
    look();
  };

  const mapCurve = (one: Model, on: string) => {
    setSearching(one.path);
    setSaid(null);
    // Nothing tried yet, and it says so rather than starting at a fraction nobody measured.
    setStep(null);
    void searchLoadout(one.path, on)
      .then(setSaid)
      .catch((why: unknown) => setSaid(String(why)))
      .finally(() => {
        setSearching(null);
        setStep(null);
        setOpen(one.path);
        look();
      });
  };

  /**
   * Run the errand the Workshop handed over, once, when the row it names exists.
   *
   * **Named by model, matched against what this deck actually holds.** A name that is not here
   * is not an error to shout about — the download may have been an Ollama pull that this list
   * has not caught up with — so the errand is simply dropped, and the deck is the same deck it
   * would have been.
   */
  useEffect(() => {
    if (!measure || held === null || searching !== null) return;
    const row = held.find((one) => one.name === measure);
    onMeasureTaken?.();
    // The errand names a model and not a runtime, so it takes the one with the most to map —
    // llama.cpp where it is here, because it is the only one that can vary the cache too.
    const on = measurableFor(row?.name ?? "")[0];
    if (row && on) mapCurve(row, on.id);
    // `held` is the trigger: the errand can only be run once the list it refers to has arrived.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [measure, held]);

  /**
   * Open the row somebody arrived here to look at.
   *
   * Waits for the list, exactly as the measuring errand does: a name is not a row until the
   * deck's own read comes back. A name that is not here is dropped rather than shouted about —
   * the model may be served by a backend this deck does not list.
   */
  useEffect(() => {
    if (!reveal || held === null) return;
    onRevealed?.();
    const row = held.find((one) => one.name === reveal);
    if (row) setOpen(row.path);
    // `held` is the trigger, for the reason above.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [reveal, held]);

  const choose = (one: Model, context: number) => {
    setBusy(true);
    void chooseLoadout(one.path, context)
      .then(setSaid)
      .catch((why: unknown) => setSaid(String(why)))
      .finally(() => {
        setBusy(false);
        look();
      });
  };

  const remove = (path: string) => {
    setBusy(true);
    setAsking(null);
    setSaid(null);
    void removeModel(path)
      .then(setSaid)
      .catch((why: unknown) => setSaid(String(why)))
      .finally(() => {
        setBusy(false);
        look();
      });
  };

  const total = (held ?? []).reduce((sum, one) => sum + one.bytes, 0);

  return (
    <section className="pnl bay">
      <div className="bay__head">
        <div>
          <div className="bay__heading">
            <span className="lx__gem" aria-hidden />
            <h2>MODELS</h2>
          </div>
          <p className="bay__sub">
            What is on this machine, and how it will be loaded.
          </p>
        </div>
      </div>
      <div className="bay__rule" />

      <div className="brg">
        <div className="brg__head">
          <span className="rm__label">On this machine</span>
          <span className="cc__hint">
            {held === null
              ? "asking…"
              : held.length === 0
                ? "nothing yet"
                : `${held.length} · ${gb(total)}`}
          </span>
        </div>

        {held !== null && held.length === 0 && (
          <p className="cc__hint">
            The Models Workshop is where models are found and fetched.
          </p>
        )}

        {said !== null && <p className="mws__verdict">{said}</p>}

      {/*
        **A paused benchmark is a fact about the machine, not about a model.**

        Measured 2026-08-31: a search opened its control at 28.0 tok/s where this machine produces
        about 46 when healthy, and no recovery Epoch is permitted to attempt restored it. So it
        stops, keeps everything, and says what it needs — once, above the deck, because every row
        below is affected by it.

        `RETRY` runs **only the control**. It resumes nothing: the rows measured before the break
        and the rows after it came from two different machines.
      */}
      {paused !== null && (
        <div className="mdls__paused">
          <p className="mdls__paused-head">BENCHMARK PAUSED</p>
          <p className="cc__hint">{paused.session.note ?? paused.session.trigger}</p>
          <p className="mdls__paused-num">
            Current control:{" "}
            <b>
              {paused.lastControl === null
                ? "no answer"
                : `${paused.lastControl.toFixed(1)} tok/s`}
            </b>
          </p>
          {/* Shown only where one exists. `known clean reference: —` invites a question it
              cannot answer. */}
          {paused.cleanReferenceSeen !== null && (
            <p className="mdls__paused-num">
              Known clean reference: <b>{paused.cleanReferenceSeen.toFixed(1)} tok/s</b>
            </p>
          )}
          {/*
            **Two different things to be waiting for, and they need two different sentences.**

            `recoveryRequired` means *I know what healthy looked like and I am not there* — a
            claim about the machine, and the one where a reboot is the likely fix.
            `referenceRequired` means *I have nothing comparable enough to tell* — a claim about
            the record. Telling somebody to restart their machine over the second would send
            them hunting a fault nobody has shown exists.
          */}
          {paused.session.stateOf === "referenceRequired" ? (
            <p className="cc__hint">
              Nothing here can be compared against yet. CALIBRATE measures this
              machine once and writes down what the number is a measurement of —
              model, build, card, context, cache, workload. Nothing is cleared
              and no search runs.
            </p>
          ) : (
            <p className="cc__hint">
              A clean GPU state is required. It may require restarting the
              machine — Epoch does not do that for you.
            </p>
          )}
          <span className="mdls__acts">
            <button
              type="button"
              className="mdls__do mdls__do--main"
              disabled={checking}
              onClick={() => {
                setChecking(true);
                void cleanStateCheck(paused.session.artifact)
                  .then((answer) => {
                    setSaid(answer);
                    void pausedBenchmark().then(setPaused);
                  })
                  .finally(() => {
                    setChecking(false);
                  });
              }}
            >
              {checking ? "CHECKING…" : "RETRY"}
            </button>
            {/*
              A separate press, deliberately. A check that quietly minted the reference it had
              just failed to find would be deciding for the user (ADR-0033) — and would make the
              very next check pass by construction.
            */}
            <button
              type="button"
              className="mdls__do"
              disabled={checking}
              onClick={() => {
                setChecking(true);
                void calibrate(paused.session.artifact)
                  .then((answer) => {
                    setSaid(answer);
                    void pausedBenchmark().then(setPaused);
                  })
                  .finally(() => {
                    setChecking(false);
                  });
              }}
            >
              CALIBRATE
            </button>
            <button
              type="button"
              className="mdls__do mdls__do--quiet"
              onClick={() => {
                /*
                  **CANCEL throws it away.** It used to dismiss the banner from view and leave the
                  record, on the reasoning that the pause was the Engine's and stood until the
                  control reproduced. That was right while the pause gated a search; it gates
                  nothing now, so what was left was a banner that came back on the next start —
                  a control that does not do what it says.

                  The evidence is untouched: every control it was built from is an Observation.
                */
                const artifact = paused?.session?.artifact;
                setPaused(null);
                if (artifact !== undefined) void dismissPause(artifact);
              }}
            >
              CANCEL
            </button>
          </span>
        </div>
      )}

        {held?.map((one) => (
          <div
            key={one.path}
            /*
              **Open to the right, not downward** (owner, 2026-09-06). A model's configuration is
              tall — thirteen measured rows, four offers and a curve — and expanding it inline
              pushed every other model off the screen while a thousand pixels sat empty beside
              it.

              A modifier rather than a new component: the row is already a grid, so widening it
              and giving the panel a column of its own is the whole change. Below the threshold
              it opens downward exactly as before, because there is nowhere for it to go.
            */
            className={`mdls__row${configuring === one.name ? " mdls__row--open" : ""}`}
          >
            <div className="mdls__what">
              {/*
                **The headline: what it is, how it runs here, and what Epoch found.**

                The owner's redesign, 2026-08-31. What used to be on this row — flash attention, a
                cache type, a speculation kind, a draft file — has not gone anywhere; it is behind
                `\u22ef`. What is here instead is the *answer*: the rate that was measured, the
                context it was measured at, the memory it took, and which profile is in force.

                Nothing on this line is estimated. A model nobody has measured says so and shows
                no numbers, because a plausible number is worse than a blank one.
              */}
              <p className="mdls__name">
                {one.name}
                <span className="mdls__state">
                  {runtimesFor(one).length > 0 ? "\u25cf READY" : "\u25cb NOT SERVING"}
                </span>
              </p>
              {/*
                The shelf it is on, because the two behave differently and the difference decides
                what removing it means: Ollama is asked to unfile a manifest, a saved GGUF is a
                file that gets deleted.
              */}
              <p className="mdls__from">
                {one.from} · {gb(one.bytes)}
                {one.sees ? " · can see" : ""}
                {one.trainedContext
                  ? ` · trained for ${thousands(one.trainedContext)}`
                  : ""}
              </p>

              {/*
                **The same weights, twice on the disk.**

                An `ollama create` of a GGUF the vault already holds copies it rather than
                linking, so 21.2 GB sat here as two rows with nothing to say either could go.
                Said on both rows and folded on neither: both files exist, and deleting one must
                not delete the other.

                Epoch compares the size it already read; where it has a hash beside both files
                from an earlier measurement it compares those too. It never hashes to answer
                this — the deck opens often, and hashing a shelf would cost a minute of disk.
              */}
              {one.sameWeightsAs.length > 0 && (
                <p className="mdls__twin">
                  the same weights as {one.sameWeightsAs.join(", ")} — one copy
                  can go
                </p>
              )}

              {/*
                **The three numbers somebody actually reads**, and each one is a measurement or is
                absent. `chosen` names the profile in force; the improvement beside it is against
                the search's own baseline, never against a remembered figure.
              */}
              {headline(one, optimized[one.name], quick[one.name]) !== null && (
                <p className="mdls__headline">
                  {headline(one, optimized[one.name], quick[one.name])}
                </p>
              )}

              {chosenProfile(optimized[one.name]) !== null && (
                <p className="mdls__profile">
                  <b>★ {chosenProfile(optimized[one.name])}</b>
                  {improvement(optimized[one.name]) !== null && (
                    <span className="mdls__better">
                      {` · ${improvement(optimized[one.name])}`}
                    </span>
                  )}
                  <span className="cc__hint"> best measured configuration</span>
                </p>
              )}

              {/*
                **A profile governs one runtime, and it says which.**

                A search runs on llama.cpp and writes a llama.cpp preset. The same weights are
                often also served by Ollama — that is how `gemma4:12b` gets benchmarked at all —
                and a character whose Brain is the Ollama one is answered by a program that never
                saw any of this. Saying `★ AUTO · best measured configuration` over that row
                claims something about a process that was not configured.

                It is not a lie to be fixed by moving the character: doing that silently would
                change the chat template, and whether a model emits native `tool_calls` is a
                property of the template — measured, on this machine, with this model. A 3%
                improvement is not a reason to change how somebody's crew calls tools.

                So the row says what governs what, and the user decides. Only where the model is
                actually served somewhere else, because on a llama.cpp-only model this sentence
                would be an instrument that never moves.
              */}
              {chosenProfile(optimized[one.name]) !== null &&
                runtimesFor(one).some((it) => it.id !== "llama_cpp") && (
                  <p className="cc__hint">
                    Governs llama.cpp. A character running this model on{" "}
                    {runtimesFor(one)
                      .filter((it) => it.id !== "llama_cpp")
                      .map((it) => it.name)
                      .join(" or ")}{" "}
                    is not using it.
                  </p>
                )}

              {optimized[one.name] === undefined &&
                runtimesFor(one).some((it) => it.id === "llama_cpp") && (
                  <p className="mdls__offer">
                    ⚠ Optimization available
                  </p>
                )}

              {/*
                **The speed, and it never blurs with an estimate.** It carries the runtime it was
                measured on and no tilde. Absent until somebody presses TIME IT — a model on the
                disk can be asked, so guessing at it here would be inventing a reading.

                Behind `\u22ef` now: it is the old headline, and two headlines is one too many.
              */}
              {advanced === one.name && (
              <>
              <p className="mdls__rate">
                {one.tokensPerSecond === null ? (
                  <span className="cc__hint">speed not measured</span>
                ) : (
                  <>
                    <b>{one.tokensPerSecond.toFixed(1)} tok/s</b>{" "}
                    <span className="cc__hint">
                      measured on {one.measuredOn}
                    </span>
                  </>
                )}
              </p>

              <p className="mdls__load">
                <span className="cc__hint">loads with </span>
                <b>{thousands(one.loadout.context)}</b>
                <span className="cc__hint">
                  {" "}
                  tokens · {one.loadout.cache === "q8_0" ? "compressed" : "full"}{" "}
                  cache ·{" "}
                </span>
                {one.measured ? (
                  <span className="mdls__measured">measured</span>
                ) : (
                  <span className="cc__hint">not measured yet</span>
                )}
              </p>

              <p className="mdls__path" title={one.path}>
                {one.path}
              </p>
              </>
              )}
            </div>

            <div className="mdls__doing">
              {/*
                **Two actions and a way in, and that is the whole row.**

                `BENCHMARK & OPTIMIZE` is the primary one until something has been measured; after
                that the primary one is `QUICK TEST`, because the expensive question has an answer
                and the cheap one is what somebody asks next. `CONFIGURE` opens the measured
                profiles. `\u22ef` opens everything that used to be on this row.

                Only where llama.cpp is serving it: Ollama exposes no speculation and takes a
                cache type only at start, LM Studio takes neither, and a search there would end in
                settings nothing reads (ADR-0027's dead `Manual` mode).
              */}
              {runtimesFor(one).some((it) => it.id === "llama_cpp") && (
                <span className="mdls__acts">
                  {/*
                    **A stored optimization that cannot be used must not take the button away.**

                    This keyed on `=== undefined`, so the moment a model had *any* record the row
                    offered QUICK TEST instead — including when the record is a degraded session
                    that produces no profiles. The one model on this machine in exactly that state
                    had no way to re-run the search at all, which is the situation the search
                    exists for.

                    `usable` is the Engine's own word for whether those rows may become profiles.
                  */}
                  {optimized[one.name]?.usable !== true ? (
                    <button
                      type="button"
                      className="mdls__do mdls__do--main"
                      disabled={busy || searching !== null || optimizing !== null}
                      onClick={() => {
                        unlessBusy(one, () => optimise(one));
                      }}
                    >
                      {optimizing === one.name
                        ? "OPTIMIZING\u2026"
                        : "BENCHMARK & OPTIMIZE"}
                    </button>
                  ) : (
                    <button
                      type="button"
                      className="mdls__do mdls__do--main"
                      disabled={busy || searching !== null || optimizing !== null}
                      onClick={() => {
                        test(one);
                      }}
                    >
                      {testing === one.name ? "TESTING\u2026" : "QUICK TEST"}
                    </button>
                  )}

                  <button
                    type="button"
                    className="mdls__do"
                    disabled={busy || searching !== null || optimizing !== null}
                    onClick={() => {
                      setConfiguring(configuring === one.name ? null : one.name);
                    }}
                  >
                    CONFIGURE
                  </button>

                  <button
                    type="button"
                    className="mdls__more"
                    title="Advanced configuration"
                    aria-label="Advanced configuration"
                    onClick={() => {
                      setAdvanced(advanced === one.name ? null : one.name);
                    }}
                  >
                    ⋯
                  </button>
                </span>
              )}

              {/*
                **A search says what it is doing, because it takes twenty minutes.** The card it
                is measuring on, how far it has got, the configuration in flight, and the best so
                far — which is a real reading of a finished run and never the one in progress.
              */}
              {optimizing === one.name && (
                <div className="mdls__search">
                  <p className="mdls__search-head">
                    BENCHMARKING {one.name.toUpperCase()}
                  </p>
                  {searchStep !== null && (
                    <>
                      <p className="cc__hint">{searchStep.machine}</p>
                      {/*
                        **The work, not the instrument.** Nobody pressing this needs to know what
                        a sentinel is, which reference governs, or that `O-004` exists. They need
                        to know the hardware was checked, a baseline is being established,
                        configurations are being tried, and roughly how much is left.

                        The sentence underneath still says exactly what is happening — the
                        checklist is a frame for it, not a replacement.
                      */}
                      <ol className="mdls__stages">
                        {STAGES.map((it, n) => {
                          const at = STAGES.findIndex(
                            (s) => s.id === searchStep.stage,
                          );
                          const state =
                            n < at ? "done" : n === at ? "now" : "next";
                          return (
                            <li
                              key={it.id}
                              className={`mdls__stage mdls__stage--${state}`}
                            >
                              <span className="mdls__stage-mark" aria-hidden>
                                {state === "done"
                                  ? "✓"
                                  : state === "now"
                                    ? "▸"
                                    : "·"}
                              </span>
                              {it.label}
                            </li>
                          );
                        })}
                      </ol>
                      <p className="mdls__bar" aria-live="polite">
                        <span
                          className="mdls__bar-fill"
                          style={{
                            width: `${String(
                              searchStep.total === 0
                                ? 0
                                : Math.round(
                                    (searchStep.done / searchStep.total) * 100,
                                  ),
                            )}%`,
                          }}
                        />
                      </p>
                      <p className="mdls__testing">
                        <span className="cc__hint">Testing: </span>
                        {searchStep.about}
                      </p>
                      {searchStep.best !== null && (
                        <p className="mdls__best">
                          <span className="cc__hint">Current best: </span>
                          <b>{searchStep.best.generation.toFixed(1)} tok/s</b>
                          {searchStep.best.vramUsed !== null &&
                            ` \u00b7 ${gb(searchStep.best.vramUsed)} VRAM`}
                          {` \u00b7 ${thousands(searchStep.best.loadout.context)} context`}
                        </p>
                      )}
                      <p className="cc__hint">
                        {searchStep.done} / {searchStep.total} configurations
                        tested
                      </p>
                      {/*
                        The flags are here for whoever wants them and are not the point of this
                        screen. Nobody watching a twenty-minute job needs to read a regex to know
                        it is working.
                      */}
                      <button
                        type="button"
                        className="mdls__link"
                        onClick={() => {
                          setFlags(!flags);
                        }}
                      >
                        {flags
                          ? "Hide technical details"
                          : "Show technical details \u25b8"}
                      </button>
                      {flags && (
                        <code className="mdls__flags">{searchStep.technical}</code>
                      )}
                    </>
                  )}
                  <button
                    type="button"
                    className="mdls__do"
                    onClick={() => {
                      void stopOptimizing();
                    }}
                  >
                    STOP
                  </button>
                </div>
              )}

              {/*
                **What the machine is doing, before somebody walks away for half an hour.**

                The search already waits for a quiet machine and refuses to record a run taken
                through a busy one. What it could not do is say so *beforehand* — and a benchmark
                that spends twenty minutes waiting for a compilation nobody knew about is twenty
                minutes of somebody's afternoon.

                **It appears only when there is something to say.** A quiet machine gets no panel
                and no extra click; the reading itself decides whether the advice exists, which is
                the same rule that keeps a profile absent until something was measured.

                And it is advice rather than a gate: START is always there. Epoch waits, it does
                not refuse, and a person who knows the 34% is a video finishing in the background
                knows more than this reading does.
              */}
              {waiting?.model === one.name && (
                <div className="mdls__done">
                  <p className="mdls__search-head">BEFORE IT STARTS</p>
                  <p className="cc__hint">
                    Something else is using this machine: {waiting.at.says}.
                  </p>
                  {waiting.at.neighbours.map((it: string) => (
                    <p className="cc__hint" key={it}>
                      {it}
                    </p>
                  ))}
                  {/*
                    The one sentence that is the point of the panel, and it says what closing
                    things buys rather than ordering anybody to close them.
                  */}
                  <p className="cc__hint">
                    A benchmark waits while the machine is busy and will not record a
                    run taken through it. Closing what you are not using makes it
                    finish sooner and its readings tighter — the background of an
                    ordinary desktop is already below what this checks.
                  </p>
                  <span className="mdls__acts">
                    <button
                      type="button"
                      className="mdls__do mdls__do--main"
                      onClick={() => {
                        const go = waiting.then;
                        setWaiting(null);
                        go();
                      }}
                    >
                      START ANYWAY
                    </button>
                    <button
                      type="button"
                      className="mdls__do mdls__do--quiet"
                      onClick={() => {
                        setWaiting(null);
                      }}
                    >
                      NOT NOW
                    </button>
                  </span>
                </div>
              )}

              {/*
                **The moment the twenty minutes were for.**

                A search used to end by putting its panel away, leaving the row a little different
                and the answer three clicks inside CONFIGURE. Somebody who pressed one button and
                waited should be told what came of it, in the place they were already looking, with
                the one thing they would do next — and nothing here is invented: the profile, the
                reading, the context and the steadiness are the ones that were measured.

                It stays until it is dismissed or another search starts, because a person who
                walked away for twenty minutes should not have to have been watching.
              */}
              {finished?.model === one.name && optimizing === null && (
                <div className="mdls__done">
                  {outcomeOf(finished.refused, optimized[one.name]).kind ===
                  "recommended" ? (
                    <>
                      <p className="mdls__search-head">OPTIMIZATION COMPLETE</p>
                      <p className="mdls__profile-name">
                        ★ {recommendedOf(optimized[one.name])?.name}
                      </p>
                      <p className="mdls__profile-num">
                        <b>
                          {recommendedOf(optimized[one.name])?.generation.toFixed(1)} tok/s
                        </b>
                        {` \u00b7 ${thousands(recommendedOf(optimized[one.name])!.context)} context`}
                        {improvement(optimized[one.name]) !== null && (
                          <span className="mdls__better">
                            {improvement(optimized[one.name])}
                          </span>
                        )}
                      </p>
                      {/*
                        **What state it was measured in, where that is worth saying.**

                        A search no longer refuses because the machine is slower than the day its
                        reference was minted — a desktop is never in one state, and a gate waiting
                        for that state waits for something that does not come back. What it does
                        instead is measure, compare candidates against each other in the state
                        they share, and say so.

                        It matters because a degraded state can change the *ranking* and not only
                        the numbers: a machine slow because it is spilling to host memory favours
                        whatever relieves the pressure. Blocking never fixed that. Saying it does.
                      */}
                      {optimized[one.name]?.session?.note != null && (
                        <p className="cc__hint">
                          {optimized[one.name]?.session?.note}
                        </p>
                      )}
                      {recommendedOf(optimized[one.name])?.steadiness !== null && (
                        <p className="cc__hint">
                          {recommendedOf(optimized[one.name])?.steadiness}
                        </p>
                      )}
                      <span className="mdls__acts">
                        <button
                          type="button"
                          className="mdls__do mdls__do--main"
                          disabled={busy}
                          onClick={() => {
                            const pick = recommendedOf(optimized[one.name]);
                            if (pick !== null) apply(one, pick.intent);
                            setFinished(null);
                          }}
                        >
                          APPLY
                        </button>
                        <button
                          type="button"
                          className="mdls__do mdls__do--quiet"
                          onClick={() => {
                            setConfiguring(one.name);
                            setFinished(null);
                          }}
                        >
                          SEE EVERYTHING MEASURED
                        </button>
                      </span>
                    </>
                  ) : (
                    <>
                      {/*
                        **A search that recommends nothing says so.** Stopped by drift, refused
                        for want of a reference, or every candidate contaminated — all real
                        outcomes, and none of them is a configuration to apply.

                        The words are the Engine's own refusal where it gave one, and the stored
                        session's note otherwise. Never a sentence written here, and never the
                        previous search's record: whatever this model was measured at before is
                        still true and is still on the row above — it is simply not what this run
                        found.
                      */}
                      <p className="mdls__search-head">NOTHING TO RECOMMEND</p>
                      <p className="cc__hint">
                        {
                          (
                            outcomeOf(finished.refused, optimized[one.name]) as {
                              why?: string;
                            }
                          ).why
                        }
                      </p>
                      <button
                        type="button"
                        className="mdls__do mdls__do--quiet"
                        onClick={() => {
                          setFinished(null);
                        }}
                      >
                        DISMISS
                      </button>
                    </>
                  )}
                </div>
              )}

              {/*
                **The profiles, and every one of them is a row that ran.** An intent with nothing
                measured behind it is simply absent — a greyed-out `LONG CONTEXT` would be a
                catalogue rather than a reading.
              */}

              {/* What the configuration in force is doing right now. */}
              {quick[one.name] !== undefined && testing !== one.name && (
                <p className="mdls__quick">{quickLine(quick[one.name])}</p>
              )}

              {/*
                **Everything below is Advanced, and none of it went away.**

                Flash attention, speculative decoding, the draft file, the loadout curve, the
                benchmark, the speculation sweep and REMOVE. They are the same controls they
                were; what changed is that a person no longer has to pass through them to use
                a model. `Simple arriba, poder absoluto abajo` — the owner, 2026-08-31.

                REMOVE is behind it too, deliberately. It is the one irreversible thing on the
                row, and a row whose primary actions are two measurements should not also
                offer deletion at the same reach.
              */}
            </div>
              {configuring === one.name && (
                <div className="mdls__configure">
                  {optimized[one.name] === undefined ? (
                    <p className="cc__hint">
                      Nothing has been measured for this model on this machine
                      yet. BENCHMARK &amp; OPTIMIZE finds out.
                    </p>
                  ) : optimized[one.name]?.usable === false ? (
                    /*
                      **A degraded search produces no profiles, and says what broke it.**
                      The rows are kept as the diagnosis; none of them may become something
                      somebody runs for hours, because everything after the control stopped
                      reproducing is a measurement of the damage.
                    */
                    <>
                      <p className="mdls__offer">
                        ⚠ The search did not finish on a stable machine.
                      </p>
                      <p className="cc__hint">
                        {optimized[one.name]?.session?.note ??
                          "The control stopped reproducing partway through."}
                      </p>
                      {optimized[one.name]?.session?.trigger != null && (
                        <p className="cc__hint">
                          It stopped reproducing after:{" "}
                          {optimized[one.name]?.session?.trigger}
                        </p>
                      )}
                    </>
                  ) : (
                    <>
                      <p className="cc__hint">
                        Epoch tested{" "}
                        {optimized[one.name]?.tried.length ?? 0} configurations
                        on this machine.
                      </p>
                      {/*
                        The evidence the profiles were derived from, before the profiles. A row
                        reading `2.6 tok/s` is a surprise on its own and an obvious consequence
                        underneath the shape it came from.
                      */}
                      <FilledCurve
                        found={optimized[one.name]}
                        busy={busy}
                        onChoose={(at) => applyMeasured(one, at)}
                      />
                      {oneAnswerOnly(optimized[one.name]) && (
                        <p className="cc__hint">
                          Every goal it could test resolved to the same
                          configuration, so there is one to offer rather than
                          four descriptions of it.
                        </p>
                      )}
                      {(oneAnswerOnly(optimized[one.name])
                        ? profilesOf(optimized[one.name]).slice(0, 1)
                        : profilesOf(optimized[one.name])
                      ).map((it) => (
                        <div className="mdls__profile-row" key={it.intent}>
                          <p className="mdls__profile-name">
                            {it.recommended ? "\u2605 " : ""}
                            {it.name}
                            {it.recommended ? " \u2014 Recommended" : ""}
                            {it.sameAs !== null && (
                              <span className="cc__hint">
                                {" "}
                                same as {it.sameAs}
                              </span>
                            )}
                          </p>
                          <p className="mdls__profile-num">
                            <b>{it.generation.toFixed(1)} tok/s</b>
                            {` \u00b7 ${thousands(it.context)} context`}
                            {it.vram !== null && ` \u00b7 ${gb(it.vram)} VRAM`}
                            {/*
                              **What it costs, beside what it is.** LONG CONTEXT on this machine
                              is 256K at 2.6 tok/s — honest, stable, bracketed, and about a
                              minute for a short reply. The window alone does not say that; the
                              comparison does.
                            */}
                            {(() => {
                              const rows = profilesOf(optimized[one.name]);
                              const fastest = Math.max(
                                ...rows.map((r) => r.generation),
                                0,
                              );
                              const cost = costOf(it, fastest);
                              return cost === null ? null : (
                                <span className="mdls__slower">{` \u00b7 ${cost}`}</span>
                              );
                            })()}
                          </p>
                          <p className="cc__hint">{it.about}</p>
                          {it.steadiness !== null && (
                            <p className="cc__hint">{it.steadiness}</p>
                          )}
                          <button
                            type="button"
                            className="mdls__do"
                            disabled={busy}
                            onClick={() => {
                              apply(one, it.intent);
                            }}
                          >
                            USE THIS CONFIGURATION
                          </button>
                        </div>
                      ))}
                      {/*
                        **A stage that did not run says so.** The search and the ladder are one
                        press, so a model with tuning profiles and no curve has either not been
                        asked or been refused — and those look identical unless the refusal is
                        written down.
                      */}
                      {optimized[one.name]?.ladder != null && (
                        <p className="cc__hint">
                          No context curve: {optimized[one.name]?.ladder}
                        </p>
                      )}
                      {/*
                        **What was measured and is not on offer**, under the offers rather than
                        instead of them. Epoch's list is what it is willing to recommend; this is
                        everything else it saw, and the two are not the same list.
                      */}
                      {(optimized[one.name]?.aside ?? []).length > 0 && (
                        <div className="mdls__aside">
                          <p className="rm__label">ALSO MEASURED, NOT RECOMMENDED</p>
                          {(optimized[one.name]?.aside ?? []).map((it) => {
                            const best =
                              profilesOf(optimized[one.name])[0]?.generation ?? null;
                            const rate =
                              it.configuration.verdict?.median ??
                              it.configuration.generation;
                            return (
                              <div
                                className="mdls__aside-row"
                                key={it.configuration.at}
                              >
                                <p className="mdls__profile-num">
                                  <b>{rate.toFixed(1)} tok/s</b>
                                  {` · ${thousands(it.configuration.loadout.context)} context`}
                                  {(() => {
                                    const lines = summariseTuning(
                                      it.configuration,
                                    );
                                    return lines === ""
                                      ? null
                                      : ` · ${lines}`;
                                  })()}
                                </p>
                                <p className="cc__hint">{whyAside(it, best)}</p>
                                {mayChoose(it) && (
                                  <button
                                    type="button"
                                    className="mdls__do mdls__do--quiet"
                                    disabled={busy}
                                    onClick={() => {
                                      applyMeasured(one, it.configuration.at);
                                    }}
                                  >
                                    USE IT ANYWAY
                                  </button>
                                )}
                              </div>
                            );
                          })}
                        </div>
                      )}
                      <button
                        type="button"
                        className="mdls__link"
                        onClick={() => {
                          void forgetOptimization(one.name).then((answer) => {
                            setSaid(answer);
                            setOptimized((held) => {
                              const rest = { ...held };
                              delete rest[one.name];
                              return rest;
                            });
                            setConfiguring(null);
                          });
                        }}
                      >
                        Forget these measurements
                      </button>
                    </>
                  )}
                </div>
              )}
              {advanced === one.name && (
                <div className="mdls__advanced">

              {/*
                **Which runtime, said and chosen.**

                One button that measured whichever provider answered first is a real reading of a
                quantity nobody picked — the most convincing way an instrument can lie, because
                something genuinely is being measured. With one runtime the button names it and
                there is nothing to choose; with two the choice appears, because that is when
                there is one.
              */}
              {runtimesFor(one).length > 1 && (
                <select
                  className="mdls__on"
                  value={timeOn[one.path] ?? runtimesFor(one)[0]!.id}
                  onChange={(e) =>
                    setTimeOn({ ...timeOn, [one.path]: e.target.value })
                  }
                >
                  {runtimesFor(one).map((it) => (
                    <option key={it.id} value={it.id}>
                      {whichOne(it)}
                    </option>
                  ))}
                </select>
              )}
              <button
                type="button"
                className="btn btn--mini"
                disabled={
                  timing !== null ||
                  searching !== null ||
                  busy ||
                  runtimesFor(one).length === 0
                }
                title={
                  runtimesFor(one).length === 0
                    ? // Not a fault and not a refusal: nothing is running that holds it. The
                      // fix is to start one, and saying so beats a grey button with no reason.
                      "Nothing here is serving it right now. Start a runtime and it can be timed."
                    : "One short answer, timed on the runtime that gives it."
                }
                onClick={() => time(one)}
              >
                {timing === one.path
                  ? "TIMING…"
                  : runtimesFor(one).length === 1
                    ? `TIME IT ON ${whichOne(runtimesFor(one)[0]!).toUpperCase()}`
                    : "TIME IT"}
              </button>

              {/*
                **A finished search must be re-runnable.**

                The row's primary becomes `QUICK TEST` once a search has produced usable
                profiles, and `BENCHMARK & OPTIMIZE` was nowhere else — so the one action that
                re-measures a machine was unreachable for exactly the models that had been
                measured. A driver, a build or a card changes and the search that would notice is
                gone.

                This is the same defect as the degraded record that took the button away, arriving
                from the other side: **a stored optimization must not remove the search, whatever
                it says.** It moves here rather than staying primary, because after a search the
                cheap question is the one somebody asks next.
              */}
              {runtimesFor(one).some((it) => it.id === "llama_cpp") &&
                optimized[one.name]?.usable === true && (
                  <button
                    type="button"
                    className="mdls__do"
                    disabled={busy || searching !== null || optimizing !== null}
                    title="Measure every configuration again from the beginning. Half an hour."
                    onClick={() => {
                      unlessBusy(one, () => optimise(one));
                    }}
                  >
                    {optimizing === one.name
                      ? "OPTIMIZING\u2026"
                      : "BENCHMARK & OPTIMIZE AGAIN"}
                  </button>
                )}

              {/*
                **The ladder is not a second button.**

                It was one, and the owner's objection was not the waiting: two buttons doing one
                job leave a path where the measurement never reaches the machine. Measured
                2026-09-02, both Qwen models had been searched at 32,768 and were both still
                serving at 16,384, because the second press was one nobody had made. The search
                carries its own winner up the windows now — longer, once, and complete when it
                ends.
              */}

              {/*
                **The whole benchmark, beside the two readings it is not.**

                TIME IT is one number. MEASURE is a curve of one number. This is the speed *and*
                whether the model is any good — reasoning, coding actually run, constraints
                checked, tool calls inspected in five parts — against a fixed, versioned set of
                questions, so a card taken today and one taken in March are rows of one table.

                It needs a runtime that is already answering, because a benchmark that started a
                server would be measuring the start.
              */}
              {runtimesFor(one).length > 0 && (
                <span className="rt__offer">
                  <button
                    type="button"
                    className="btn btn--mini"
                    disabled={busy || timing !== null || searching !== null || benching !== null}
                    title="A speed reading and every trial in the suite. Minutes."
                    onClick={() => {
                      const on = timeOn[one.path] ?? runtimesFor(one)[0]?.id;
                      if (on !== undefined) bench(one, on);
                    }}
                  >
                    {/*
                      **Named for what it measures.** It was `BENCHMARK`, one word away from
                      `BENCHMARK & OPTIMIZE` on the same card and measuring something completely
                      different — one asks how fast a configuration is, the other asks how capable
                      the model is. Two buttons sharing a word is a button somebody presses by
                      mistake, and somebody did.
                    */}
                    {benching === one.name ? "SCORING…" : "QUALITY SUITE"}
                  </button>
                  {benching === one.name && doing !== null && (
                    <em className="hfd__at">{doing}</em>
                  )}
                  {/*
                    **Every card, not the newest.** Two contexts of one model are two facts, and a
                    row that showed only the last would answer *how good is it* with *how good was
                    it at whatever somebody tried most recently*.
                  */}
                  {/*
                    **SWEEP measures; it does not choose.** It runs every speculative decoding
                    this build offers against a baseline it takes first, and puts back whatever
                    the model had when it finishes — the measurement is Epoch's and the setting
                    is the user's (ADR-0033).

                    Only where llama.cpp is serving it. Ollama exposes no speculation flags and
                    LM Studio takes none Epoch can set, so the button elsewhere would be the dead
                    `Manual` mode of ADR-0027.
                  */}
                  {runtimesFor(one).some((it) => it.id === "llama_cpp") && (
                    <button
                      type="button"
                      className="mdls__do"
                      disabled={busy || searching !== null || optimizing !== null}
                      onClick={() => {
                        runSweep(one);
                      }}
                    >
                      {sweeping === one.name ? "SWEEPING…" : "SWEEP SPECULATION"}
                    </button>
                  )}
                  {sweeping === one.name && doing !== null && (
                    <em className="hfd__at">{doing}</em>
                  )}
                  {sweeping === null && sweep !== null && sweep.model === one.name && (
                    <code className="mdls__card">
                      {sweepLines(sweep).map((line) => (
                        <span key={line}>{line}</span>
                      ))}
                    </code>
                  )}

                  {benching !== one.name &&
                    cards
                      .filter((card) => card.model === one.name)
                      .map((card) => (
                        <code
                          className="mdls__card"
                          key={`${card.conditions.at}-${card.conditions.context}`}
                        >
                          {summarise(card).map((line) => (
                            <span key={line}>{line}</span>
                          ))}
                        </code>
                      ))}
                </span>
              )}

              {/*
                **What this model is told, as opposed to what was measured about it.**

                Flash attention and speculative decoding are choices; a curve is a reading. They
                sit beside each other because a person configuring a model wants both in one
                place, and they are kept apart in the Engine because one of them is a thing that
                happened and the other is a thing somebody decided.

                **`AS IT DECIDES` is not a spelling of `OFF`.** llama.cpp's own default is `auto`,
                which is a real third state — and writing `off` because a dropdown had two entries
                would be Epoch choosing the thing it is trying not to choose.
              */}
              <span className="rt__offer">
                <label className="rt__pick">
                  FLASH ATTENTION
                  <select
                    value={
                      one.tuning.flashAttn === null
                        ? ""
                        : one.tuning.flashAttn
                          ? "on"
                          : "off"
                    }
                    disabled={busy || searching !== null || optimizing !== null}
                    onChange={(e) => {
                      void tell(one, {
                        ...one.tuning,
                        flashAttn:
                          e.target.value === "" ? null : e.target.value === "on",
                      });
                    }}
                  >
                    <option value="">as llama.cpp decides</option>
                    <option value="on">on</option>
                    <option value="off">off</option>
                  </select>
                </label>
              </span>

              {/*
                **SPECULATIVE DECODING, not MTP.** MTP is one value of `--spec-type` among the
                eleven this build offers, and four of the others need no second file at all — so
                a control named after one of them was naming a technique and offering a feature.
                The owner's correction, 2026-08-31.

                The list comes from the Engine, which reads it from `llama-server`. A copy here
                would be the second place to keep in step, and `--spec-type` went from three
                values to eleven between releases.

                **A `draft-*` kind needs a file and Epoch will not guess which.** Which draft
                goes with which model is not something a filename may answer (ADR-0024), and
                nothing has been measured about a draft file's header here — so every other GGUF
                is offered and the person picks.

                And there is deliberately no `AUTO`. Measured on this machine, the right answer
                depends on the *workload*: `ngram-mod` on `gemma4:12b` ran at 182.5 t/s repeating
                a paragraph it had just written and 41.3 t/s on a question it had not seen,
                against a 47.0 baseline. An `AUTO` would have to pick one of those to be wrong
                about. SWEEP measures both and says so; the choice stays the user's.
              */}
              <span className="rt__offer">
                <label className="rt__pick">
                  SPECULATIVE DECODING
                  <select
                    value={one.tuning.speculation.kind}
                    disabled={busy || searching !== null || optimizing !== null}
                    onChange={(e) => {
                      const kind = e.target.value;
                      void tell(one, {
                        ...one.tuning,
                        speculation: {
                          ...one.tuning.speculation,
                          kind,
                          // A draft file left behind after switching to a kind that ignores it
                          // is a setting nothing reads.
                          draft: kind.startsWith("draft-")
                            ? one.tuning.speculation.draft
                            : null,
                        },
                      });
                    }}
                  >
                    <option value="">off</option>
                    {specTypes
                      .filter((kind) => kind !== "none")
                      /* A kind that needs a file this machine does not have is a setting that
                         cannot load. Not hidden for tidiness — offering it would be a control
                         whose only outcome is a refusal. */
                      .filter((kind) => !kind.startsWith("draft-") || one.drafts.length > 0)
                      .map((kind) => (
                        <option key={kind} value={kind}>
                          {kind}
                        </option>
                      ))}
                  </select>
                </label>

                {one.tuning.speculation.kind.startsWith("draft-") && (
                  <label className="rt__pick">
                    DRAFT MODEL
                    <select
                      value={one.tuning.speculation.draft ?? ""}
                      disabled={busy || searching !== null || optimizing !== null}
                      onChange={(e) => {
                        void tell(one, {
                          ...one.tuning,
                          speculation: {
                            ...one.tuning.speculation,
                            draft: e.target.value || null,
                          },
                        });
                      }}
                    >
                      <option value="">pick one</option>
                      {one.drafts.map((at) => (
                        <option key={at} value={at}>
                          {/* The file, not the path it lives at: a dropdown of absolute paths is
                              a dropdown nobody can read. Both separators, because an Ollama blob
                              and a vault file are written differently on one machine. */}
                          {at.split(/[\/]/).pop()}
                        </option>
                      ))}
                    </select>
                  </label>
                )}

                {one.tuning.speculation.kind !== "" && (
                  <em className="hfd__at">
                    drafts several tokens and verifies them in one pass &mdash; the same answer,
                    faster or not depending on how many survive
                  </em>
                )}

                {/*
                  **What these controls actually write.**

                  `SPECULATIVE DECODING` is one parameter with eleven values, not a bundle of
                  switches — they are mutually exclusive, and the ones a model cannot do are
                  already absent because Epoch reads the file rather than the name. So there is
                  nothing to turn off individually. What was missing is simply saying what the
                  choice becomes, which is two lines in a file nobody can see.

                  Generated by the function that writes them. Absent until something is set,
                  because a box reading `nothing` is a box about nothing.
                */}
                {(writes[one.name] ?? "").trim() !== "" && (
                  <code className="mdls__flags">{writes[one.name]}</code>
                )}
              </span>

              {/*
                **MEASURE offers the runtimes that can answer, and says how much each maps.**

                It used to name llama.cpp and say it could never be anything else, on the
                reasoning that a curve is eight `llama-server` flags and that a loadout is only
                consumed by `presets.ini`. Both halves were re-measured on 2026-08-30 and the
                second one is what actually decided it.

                Ollama takes `options.num_ctx` per request, so a curve runs through the server
                that is already answering — four rows rather than eight, because its cache is
                read once at start and a per-request one is ignored *silently* (8.39 GB with it
                and 8.39 GB without, to the byte). And the answer does land somewhere now: the
                largest context that loaded becomes a ceiling on the window every turn sizes for
                itself.

                LM Studio is still absent, and for the original reason — nothing would read the
                result. That reason is now a sentence the Engine returns rather than a rule
                written into this button.

                **How much of the curve each maps is on the row**, because eight rows on one
                runtime and four on another is honest and silently different is not.
              */}
              {/*
                **Offered whether or not a curve exists**, which the first version got wrong: it
                appeared only on an unmeasured row, so a model measured on llama.cpp could never
                be measured on Ollama — MEASURE AGAIN would silently re-take the same one. A
                control keyed to how far somebody has got is wrong until they have finished.

                Where a curve exists it opens on the runtime that curve was taken on, so the
                default is *re-measure this*, and changing it is a deliberate act.
              */}
              {measurableFor(one.name).length > 1 && (
                <select
                  className="mdls__on"
                  aria-label="which runtime to measure on"
                  value={
                    measureOn[one.path] ??
                    one.curveOn ??
                    measurableFor(one.name)[0]?.id ??
                    ""
                  }
                  disabled={timing !== null || searching !== null || busy}
                  onChange={(e) =>
                    setMeasureOn({ ...measureOn, [one.path]: e.target.value })
                  }
                >
                  {measurableFor(one.name).map((it) => (
                    <option key={it.id} value={it.id}>
                      {it.name} · {it.maps}
                    </option>
                  ))}
                </select>
              )}
              <button
                type="button"
                className="btn btn--mini"
                disabled={
                  timing !== null ||
                  searching !== null ||
                  busy ||
                  (one.readings.length === 0 && measurableFor(one.name).length === 0)
                }
                title={
                  one.readings.length > 0
                    ? "The curve this model was measured at, and a way to measure it again."
                    : measurableFor(one.name).length === 0
                      ? "No runtime here can map a curve for this model whose answer anything would read."
                      : "A load at every size, and on llama.cpp both caches too. Minutes, and it is asked for."
                }
                onClick={() => {
                  if (one.readings.length > 0) {
                    setOpen(open === one.path ? null : one.path);
                    return;
                  }
                  const on =
                    measureOn[one.path] ?? measurableFor(one.name)[0]?.id;
                  if (on !== undefined) mapCurve(one, on);
                }}
              >
                {searching === one.path
                  ? "MEASURING…"
                  : one.readings.length > 0
                    ? open === one.path
                      ? "HIDE"
                      : "SETTINGS"
                    : measurableFor(one.name).length === 1
                      ? `MEASURE ON ${measurableFor(one.name)[0]!.name.toUpperCase()}`
                      : "MEASURE"}
              </button>

              {/*
                **Never on the first click.** Removing a model is minutes of downloading and tens
                of gigabytes, and there is no undo — so the button says what it is about to do, by
                name, and waits for a second press.
              */}
              {asking === one.path ? (
                <>
                  <button
                    type="button"
                    className="btn btn--mini btn--danger"
                    disabled={busy}
                    onClick={() => remove(one.path)}
                  >
                    DELETE {one.name}
                  </button>
                  <button
                    type="button"
                    className="btn btn--mini"
                    onClick={() => setAsking(null)}
                  >
                    KEEP IT
                  </button>
                </>
              ) : (
                <button
                  type="button"
                  className="btn btn--mini"
                  disabled={busy || searching !== null || optimizing !== null}
                  onClick={() => setAsking(one.path)}
                >
                  REMOVE
                </button>
              )}
                </div>
              )}

            {/*
              Where the job has got to, on the row it belongs to.

              **Four minutes is long enough to be mistaken for a hang**, and it was — reported
              as "I pressed TIME IT and nothing came out". The only signal was a button label
              reading MEASURING, which is eight characters on a control somebody has already
              looked away from.

              Every number here is measured: the count is settings actually tried, the total is
              the bound the search cannot exceed. TIME IT has no steps to report — it is one
              short answer — so its row says it is working and claims no fraction, which is the
              honest difference between the two jobs.
            */}
            {(searching === one.path || timing === one.path) && (
              <p className="mdls__step" aria-live="polite">
                <span className="mdls__step-track">
                  {searching === one.path && step && (
                    <i
                      style={{
                        width: `${Math.min(100, (step.done / step.total) * 100)}%`,
                      }}
                    />
                  )}
                </span>
                <span>
                  {timing === one.path
                    ? "timing one answer"
                    : step
                      ? `setting ${step.done} of at most ${step.total} — each one loads the model`
                      : "starting; the file is read once first"}
                </span>
              </p>
            )}

            {open === one.path && one.readings.length > 0 && (
              <>
                <Curve
                  one={one}
                  onChoose={choose}
                  busy={busy}
                  applied={appliedProfile(optimized[one.name])}
                />
                {/*
                  **And a way to measure it again**, which there was not.

                  MEASURE became SETTINGS the moment a curve existed, and nothing anywhere
                  offered another search — so a reading taken on a busy card, or before a driver
                  changed, or on a build of llama.cpp that has since been replaced, was the only
                  reading that model would ever have. Reported by the owner, who tried on a model
                  that already had one and found the button had turned into something else.

                  Here rather than beside SETTINGS because four minutes is not a thing to press
                  by accident, and because this is where somebody looking at a curve they doubt
                  is already standing.
                */}
                <button
                  type="button"
                  className="btn btn--mini"
                  disabled={timing !== null || searching !== null || busy}
                  title="Minutes, and it replaces the curve above with what it finds."
                  // **On the runtime it was measured on**, not on whichever is first: measuring
                  // again is re-taking *this* curve, and quietly moving it to another program
                  // would replace a reading with an answer about something else.
                  onClick={() => {
                    const on =
                      measureOn[one.path] ??
                      one.curveOn ??
                      measurableFor(one.name)[0]?.id;
                    if (on !== undefined) mapCurve(one, on);
                  }}
                >
                  MEASURE AGAIN
                </button>
              </>
            )}
          </div>
        ))}
      </div>

      {held !== null && held.length > 0 && (
        <>
          <p className="cc__hint">
            MEASURE loads a model eight ways and times each — about four minutes,
            once. It runs a bare <b>llama.cpp</b> of its own, holding this model
            and nothing else; TIME IT asks the runtime you picked, as the World
            uses it. <b>The two are not the same measurement and will not agree.</b>{" "}
            The server that answers turns also loads a model&rsquo;s vision
            projector beside it and keeps several slots open, and both cost speed
            — measured here, 14.0 tok/s against the bench&rsquo;s 20.6 on one
            model. TIME IT is what a character will actually get.
          </p>
          <p className="cc__hint">
            Removing takes the shelf&rsquo;s hard link with it. Without that the
            bytes would stay on the disk and llama.cpp would keep offering a model
            Ollama no longer has.
          </p>
        </>
      )}
    </section>
  );
}

/**
 * The curve, with the balanced row marked and every other row selectable.
 *
 * Ordered by context rather than by speed: it is read as *how much conversation do I want*, and
 * the speed is the price beside each answer.
 */
function Curve({
  one,
  onChoose,
  busy,
  applied,
}: {
  one: Model;
  onChoose: (one: Model, context: number) => void;
  busy: boolean;
  /** The profile in force, so its number can be told apart from these. `null` when none is. */
  applied: Shown | null;
}) {
  const rows = [...one.readings].sort(
    (a, b) => a.context - b.context || a.cache.localeCompare(b.cache),
  );
  const fastest = Math.max(
    ...rows.map((r) => r.tokensPerSecond ?? 0),
    0,
  );

  return (
    <div className="curve">
      <div className="curve__head">
        <span className="rm__label">What this model does on this card</span>
        {/*
          **What was measured, beside what it says.**

          Each row is one short prompt answered inside a window of that size — about twenty
          tokens in, a hundred and sixty out. So it is the cost of *holding* a window, which is a
          real cost and is not the cost of *filling* one. A reader who takes `65,536 · 48.5` as
          "this model runs at 48.5 with 64K of conversation" has been misled by a number that is
          perfectly correct about something else.
        */}
        <span className="cc__hint">
          one short prompt in each window — the cost of holding it, not of
          filling it
        </span>
      </div>
      {rows.map((row) => {
        const chosen =
          row.context === one.loadout.context && row.cache === one.loadout.cache;
        const marked =
          one.recommended !== null &&
          row.context === one.recommended.context &&
          row.cache === one.recommended.cache;
        const holds = row.context >= 15257;
        return (
          <button
            key={`${row.context}-${row.cache}`}
            type="button"
            className={`curve__row${chosen ? " curve__row--on" : ""}${
              holds ? "" : " curve__row--short"
            }`}
            disabled={busy || row.tokensPerSecond === null}
            onClick={() => onChoose(one, row.context)}
          >
            <span className="curve__ctx">{thousands(row.context)}</span>
            <span className="curve__cache">
              {row.cache === "q8_0" ? "compressed" : "full"}
            </span>
            <span className="curve__rate">
              {row.tokensPerSecond === null
                ? "did not load"
                : `${row.tokensPerSecond.toFixed(1)} tok/s`}
            </span>
            {/*
              A bar, because the whole point of the table is the shape: a flat curve says take
              the biggest, a cliff says stay where you are. Derived from the readings, never a
              constant.
            */}
            <span className="curve__bar" aria-hidden>
              <i
                style={{
                  width:
                    fastest > 0 && row.tokensPerSecond !== null
                      ? `${(row.tokensPerSecond / fastest) * 100}%`
                      : "0%",
                }}
              />
            </span>
            <span className="curve__note">
              {/*
                **One word, one meaning.** This said `balanced`, and so does the profile above it
                — two different questions wearing one word on one screen, disagreeing. The
                profile's BALANCED is *the fastest configuration at the standard window*; this is
                *the largest window that costs no measurable speed*, which is what
                `recommended_of` actually computes. Both are true and they are not the same
                answer, so neither may borrow the other's name.
              */}
              {chosen ? "in use" : marked ? "most context, same speed" : ""}
              {holds ? "" : " · too small for a turn"}
            </span>
          </button>
        );
      })}
      <p className="cc__hint">
        A turn was measured at 13,209 tokens, so anything below about 15,000
        stops mid-answer. It is measured and shown anyway — it is genuinely the
        fastest, and the card is yours.
      </p>
      {/*
        **The one line that stops a reader comparing two instruments.**

        A profile's number comes from the search — its own flags, its own control, five runs — and
        a curve row comes from one short prompt at whatever this model was set to that afternoon.
        Measured 2026-09-01 they read `51.7` and `48.6` for the same 32,768, side by side on one
        screen, with nothing saying they were about different configurations. Neither was wrong.
        Together they were, and the missing sentence is this one.
      */}
      {applied !== null && (
        <p className="cc__hint">
          Your profile answers at <b>{applied.generation.toFixed(1)} tok/s</b>{" "}
          in {thousands(applied.context)} — measured with the flags it chose, so
          it is not one of the rows above.
        </p>
      )}
    </div>
  );
}

function gb(bytes: number): string {
  return `${(bytes / 1e9).toFixed(1)} GB`;
}

function thousands(n: number): string {
  return n.toLocaleString("en-US");
}

/**
 * One card in a line: the speed, and how much of each kind of trial it satisfied.
 *
 * **A column with nothing in it reads `—`, never `0%`.** A model whose coding trials could not
 * run because the machine has no interpreter is not a model that failed them, and a zero there
 * would say it was — which is the cold-instrument rule arriving in a table cell.
 */
/**
 * A sweep, as a table somebody can read.
 *
 * **Two speed-up columns and neither is the headline on its own.** `fresh` is three different
 * questions — the honest number for a chat workload, and the one that stops a run priming the
 * next. `repeat` is the same question three times, which is where an ngram speculator actually
 * fires: measured on this machine, `ngram-mod` on `gemma4:12b` was 41.3 t/s on unseen text and
 * 182.5 t/s repeating a paragraph it had just written, against a 47.0 baseline. An average of
 * those two would describe neither.
 *
 * The counts come too — drafted and accepted — because a speed-up with nothing drafted is not a
 * speed-up at all, it is drift, and this table has to make that visible rather than plausible.
 */
export function sweepLines(sweep: Sweep): readonly string[] {
  const rate = (runs: readonly BenchRun[]): number | null => {
    const got = runs
      .filter((it) => it.failed === null)
      .map((it) => it.generation)
      .filter((it): it is number => it !== null)
      .sort((a, b) => a - b);
    return got.length === 0 ? null : (got[Math.floor(got.length / 2)] ?? null);
  };
  const show = (it: number | null, unit: string): string =>
    it === null ? "—" : `${it.toFixed(1)}${unit}`;
  const times = (it: number | null): string =>
    it === null ? "  —  " : `${it.toFixed(2)}x`;

  const head = [
    `${sweep.model}`,
    sweep.best === null
      ? "nothing beat the baseline by more than noise — off is the answer"
      : `fastest stable: ${sweep.best}`,
  ];

  const rows = sweep.tried.map((one) => {
    const drafted = one.measured.runs.reduce((sum, r) => sum + (r.drafted ?? 0), 0);
    const accepted = one.measured.runs.reduce((sum, r) => sum + (r.accepted ?? 0), 0);
    const draft =
      drafted === 0
        ? "nothing drafted"
        : `drafted ${String(drafted)} kept ${String(accepted)} (${((accepted / drafted) * 100).toFixed(0)}%)`;
    const warn = !one.stable
      ? "  UNSTABLE"
      : one.sameAnswers === false
        ? "  DIFFERENT TEXT"
        : "";
    return (
      `${one.label.padEnd(24, " ")} ${show(rate(one.measured.runs), " t/s").padStart(9, " ")}` +
      ` fresh ${times(one.speedup)}  repeat ${times(one.speedupRepeating)}  ${draft}${warn}`
    );
  });

  // What the machine was doing, from the baseline's own watch. One line, because it is the same
  // machine for every row and repeating it would be noise.
  const load = sweep.tried[0]?.load;
  const gb = (bytes: number | null | undefined): string =>
    bytes === null || bytes === undefined ? "—" : `${(bytes / 1e9).toFixed(1)} GB`;
  const machine =
    load === undefined
      ? []
      : [
          `machine while running:` +
            ` gpu ${show(load.gpuPercent?.peak ?? null, "%")} peak` +
            ` / ${show(load.gpuPercent?.mean ?? null, "%")} mean` +
            ` · cpu ${show(load.cpuPercent?.mean ?? null, "%")}` +
            ` · vram ${gb(load.vramUsed?.peak)} (was ${gb(load.vramBefore)})` +
            ` · ram ${gb(load.ramUsed?.peak)} (was ${gb(load.ramBefore)})`,
        ];

  return [...head, ...rows, ...machine];
}

/**
 * The three numbers on a card: how fast, how much context, how much memory.
 *
 * **Every one of them measured, or the line is absent.** The order they are looked for in is the
 * order of how recently they were measured — a quick reading beats the search's own, which beats
 * the deck's older TIME IT. A model nobody has measured returns `null` and the card shows no
 * numbers rather than plausible ones.
 */
export function headline(
  model: Model,
  found: Optimized | undefined,
  recent: Quick | undefined,
): string | null {
  const chosen = chosenConfiguration(found);
  const rate =
    recent?.generation ?? chosen?.generation ?? model.tokensPerSecond ?? null;
  if (rate === null) return null;

  const context = recent?.context ?? chosen?.loadout.context ?? model.loadout.context;
  const vram = recent?.vramUsed ?? chosen?.vramUsed ?? null;
  return (
    `${rate.toFixed(1)} tok/s \u00b7 ${thousands(context)} context` +
    (vram === null ? "" : ` \u00b7 ${gb(vram)} VRAM`)
  );
}

/**
 * Which measured configuration is actually in force, if the user chose one.
 *
 * **An index into what the Engine resolved, never a second resolution.** `custom` is the one the
 * user pinned by hand and does not appear among the offered profiles, so it is read from where it
 * is stored.
 */
function chosenConfiguration(found: Optimized | undefined): Configuration | null {
  if (found === undefined || found.chosen === null) return null;
  if (found.chosen === "custom") return found.custom;
  return (
    (found.profiles ?? []).find((it) => it.intent === found.chosen)?.configuration ??
    null
  );
}

/** The name of the profile in force. `null` until somebody has chosen one. */
export function chosenProfile(found: Optimized | undefined): string | null {
  if (found === undefined || found.chosen === null) return null;
  return nameOf(found.chosen);
}

/**
 * How much better the chosen configuration is than the search's own baseline.
 *
 * **Both halves measured, or nothing.** An improvement against a remembered number compares two
 * afternoons, and the baseline is the first thing the search runs for exactly this reason.
 */
export function improvement(found: Optimized | undefined): string | null {
  const chosen = chosenConfiguration(found);
  const base = found?.baseline ?? null;
  const before = base?.generation ?? null;
  if (chosen === null || base === null || before === null || before <= 0) {
    return null;
  }
  /*
    **Only against something measured the same way.**

    Measured on `gemma4:12b`, 2026-09-02: the deck read `★ BALANCED -18.5%` over a
    configuration the search had just chosen. Nothing was wrong with either number. The baseline
    answered a 21-token prompt inside a 32K window at 48.8; the chosen row answered the same
    window with 30,835 tokens in it, at 39.7. Holding a window and filling one are two different
    instruments, and dividing one by the other reports **the cost of a long conversation as a
    regression caused by the search** — the opposite of what the line exists to say, at the moment
    somebody is deciding whether the search was worth running.

    `resolve` has grouped by workload since the ladder existed. This is the same rule, and it had
    never been applied on the way out.
  */
  const filled = (it: { filled?: number | null }) =>
    it.filled !== null && it.filled !== undefined;
  if (filled(chosen) !== filled(base)) return null;
  const better = (chosen.generation / before - 1) * 100;
  return `${better >= 0 ? "+" : ""}${better.toFixed(1)}%`;
}

/**
 * How a quick reading reads.
 *
 * ## Why the window says what it is a window *of*
 *
 * It printed `32,768 context`, which is `STANDARD_CONTEXT` — the window **every** model is
 * benchmarked at, so that two models can be compared at all. It is not what llama.cpp is started
 * with: an unmeasured model gets `conservative()`, which is 16,384.
 *
 * The owner read the first and met the second in a server log, and the two numbers had nothing
 * between them to say they were about different questions. That is the cold-instrument rule's
 * most convincing face — **a real reading of the wrong quantity** — because something genuinely
 * is being measured, and it is simply not the number that decides how his character runs.
 *
 * The reading is kept and named. `loads with …` a few lines down is the other one, and it
 * already says *measured* or *not measured yet*.
 */
export function quickLine(one: Quick | undefined): string {
  if (one === undefined) return "";
  const rate =
    one.generation === null ? "no answer" : `${one.generation.toFixed(1)} tok/s`;
  const vram = one.vramUsed === null ? "" : ` \u00b7 ${gb(one.vramUsed)} VRAM`;
  const verdict = !one.healthy
    ? "unstable"
    : one.generation === null
      ? "no answer"
      : "healthy";
  return `${rate}${vram} \u00b7 benched at ${thousands(one.context)} \u00b7 ${verdict}`;
}

function nameOf(intent: Intent): string {
  return {
    auto: "AUTO",
    balanced: "BALANCED",
    fast: "FAST",
    maxQuality: "MAX QUALITY",
    longContext: "LONG CONTEXT",
    custom: "CUSTOM",
  }[intent];
}

function aboutOf(intent: Intent): string {
  return {
    auto: "Epoch picks. Today that is the balanced configuration.",
    balanced: "Best balance of quality, speed and context.",
    fast: "The fastest measured, at whatever context it needed to give up.",
    maxQuality: "Nothing approximated: full-precision cache, unaltered answers.",
    longContext: "The most context that stayed stable here.",
    custom: "Yours.",
  }[intent];
}

/**
 * How steady one configuration was, in the words somebody would use to argue with the choice.
 *
 * `null` where there is nothing to say — a single run, no spread worth reporting and nothing
 * thrown away. Saying *0 collapses* about every row would make the ones that matter invisible.
 */
export function steadinessOf(one: Configuration): string | null {
  const it = one.verdict;
  if (!it || it.kept === 0) return null;
  const parts: string[] = [];
  if (it.slowest !== null && it.fastest !== null && it.kept > 1) {
    parts.push(
      `${it.slowest.toFixed(1)}\u2013${it.fastest.toFixed(1)} over ${String(it.kept)} runs`,
    );
  }
  if (it.collapses > 0) {
    parts.push(
      `${String(it.collapses)} collapse${it.collapses === 1 ? "" : "s"}`,
    );
  } else if (it.discarded > 0) {
    parts.push(`${String(it.discarded)} discarded`);
  }
  return parts.length > 0 ? parts.join(" \u00b7 ") : null;
}

/** One profile, ready to render. */
export interface Shown {
  readonly intent: Intent;
  readonly name: string;
  readonly about: string;
  readonly generation: number;
  readonly context: number;
  readonly vram: number | null;
  readonly recommended: boolean;
  /** Where this resolved to the same configuration as an earlier intent. */
  readonly sameAs: string | null;
  /**
   * How steady it was: the spread across its runs, and how many had to be thrown away.
   *
   * **This is why Epoch can explain a choice.** A configuration that peaked higher and was
   * discarded is not a mystery on the screen — it says `25.0–48.1, 2 collapses` beside the one
   * that says `45.1–45.9`.
   */
  readonly steadiness: string | null;
}

/**
 * Every profile with a measurement behind it.
 *
 * **An intent that resolves to nothing is absent, not greyed out.** A greyed `LONG CONTEXT` would
 * be a catalogue rather than a reading, and this codebase has already decided that once for
 * Styles: the rule protects readings, not catalogues.
 *
 * Two intents landing on one configuration is ordinary and is said rather than hidden — on a
 * machine where the fastest run is also the longest, FAST and LONG CONTEXT are the same row.
 */
/**
 * What is switched on in a configuration, in a few words.
 *
 * Derived from the row rather than from a stored sentence: a description written when a row was
 * measured is one that goes stale the day a knob is added, and nothing would say so.
 */
export function summariseTuning(it: Configuration): string {
  const said: string[] = [];
  if (it.loadout.cache === "q8_0") said.push("compressed cache");
  if (it.tuning.flashAttn === true) said.push("flash attention");
  const spec = it.tuning.speculation.kind;
  if (spec !== "" && spec !== "none") said.push(spec);
  if (it.offload !== null) said.push("split across GPU and CPU");
  return said.join(" · ");
}

/**
 * What a set-aside row was, and why Epoch did not offer it.
 *
 * ## Why an exclusion has to be spoken
 *
 * Measured 2026-09-02: three searches each held a configuration back and no surface said so. On
 * `Qwen3.6-35B-A3B-UD-IQ4_XS` the held-back row was the model’s own prediction head at 52.4
 * tok/s — faster than the 48.6 that was recommended, and the entire reason that build was on
 * the machine. A person reading the deck saw 48.6 and could only conclude that MTP did nothing.
 *
 * The rule is not new; it is the cold-instrument rule from a direction it had not been
 * approached from. Not an invented reading, not a withheld one, not a reading of the wrong
 * quantity — **a measurement that was taken, was correct, decided the outcome, and is
 * invisible.**
 *
 * Every sentence names what was measured. Nothing here is a category: the range is the range that
 * ran, and it is what makes the difference between *this is a gamble* and *this is simply better*
 * legible without anybody having to trust the word `unstable`.
 */
export function whyAside(one: WasAside, best: number | null): string {
  const it = one.configuration;
  const low = it.verdict?.slowest ?? null;
  const high = it.verdict?.fastest ?? null;
  const range =
    low !== null && high !== null
      ? `It ran ${low.toFixed(1)} to ${high.toFixed(1)} tok/s`
      : "Its runs were not recorded";
  switch (one.why) {
    case "varied":
      return best === null
        ? `${range} — too far apart to call it one configuration.`
        : `${range}, so it could land below the ${best.toFixed(1)} on offer and it could land well above it. Epoch will not choose a gamble for you; you can.`;
    case "collapsed":
      // Named, never offered. A collapsed set is two configurations wearing one name.
      return "The model instance broke partway through, so this speed is not a reading of a configuration at all.";
    case "changedAnswers":
      return "It changed what the model answered, which is a different model rather than a faster one.";
    case "unwitnessed":
      return "No control witnessed the machine on both sides of it, so nothing says the reading is about this configuration.";
    case "sessionDegraded":
      return "The machine stopped reproducing during this search, so everything after that point describes the damage.";
    default:
      // An Engine newer than this build named a reason this one has not learned. Saying that
      // plainly beats picking the nearest sentence, which would be a description of a different
      // measurement.
      return `${range}. Epoch held it back for a reason this screen cannot name.`;
  }
}

/** Whether somebody may put a set-aside row into effect. */
export function mayChoose(one: WasAside): boolean {
  /*
    **One refusal, and it is not about risk.** Everything else held back is a judgement the owner
    may overrule on their own card. A collapsed run is not a risky configuration — its surviving
    runs came from an instance that broke, so there is no configuration behind the number to put
    into effect. The Engine refuses this too; the button simply does not pretend otherwise.
  */
  return one.why !== "collapsed";
}

/**
 * The ladder, drawn — what the model does with the conversation actually full.
 *
 * ## Why this is a picture rather than a sentence
 *
 * The five rows measured on `gemma4:12b` here are 40.7 · 39.7 · 37.8 · 18.8 · 2.6 tok/s, and the
 * one thing a person needs from them is **where it falls off**. Epoch could compute a threshold
 * and say "this model runs well to 64K" — and it would be Epoch deciding, from one machine, what
 * *well* means. The bars say it in the time it takes to look at them and decide nothing.
 *
 * It sits above the profiles because it is the evidence they were derived from: LONG CONTEXT
 * being 15× slower stops being a surprise once the cliff it sits at the bottom of is on screen.
 *
 * **Only rows whose window was actually filled.** The other curve on this deck is one short
 * prompt inside a large window — the cost of *holding* it — and the two answer different
 * questions. Mixing them would put 48.5 beside 2.6 for the same 256K.
 */
function FilledCurve({
  found,
  onChoose,
  busy,
}: {
  found: Optimized | undefined;
  /** Put one measured rung into effect. */
  onChoose: (at: number) => void;
  busy: boolean;
}) {
  const rows = (found?.tried ?? [])
    .filter((it) => it.filled !== null && it.filled !== undefined)
    .filter((it) => it.generation > 0)
    .sort((a, b) => a.loadout.context - b.loadout.context);
  if (rows.length < 2) return null;
  const fastest = Math.max(...rows.map((it) => it.generation));

  return (
    <div className="curve">
      <div className="curve__head">
        <span className="rm__label">As the conversation fills up</span>
        <span className="cc__hint">
          each window filled to about nine tenths, then answered
        </span>
      </div>
      {/*
        **Every rung is pressable, and the reason is a comment of mine that was wrong.**

        It read: *"Not pressable, because choosing a window is what the profiles above it are
        for."* Measured 2026-09-02 on `gemma-4-12B-it-qat`, which climbed all five rungs:

        | rung | tok/s | reachable? |
        |---|---|---|
        | 16,384 | 43.1 | FAST |
        | 32,768 | 42.1 | BALANCED |
        | 65,536 | 39.6 | **nothing** |
        | 131,072 | 36.1 | **nothing** |
        | 262,144 | 4.6 | LONG CONTEXT |

        The profiles cover the two ends and skip the middle — and the middle is where this
        model is interesting, because 128K costs it 14% where it costs `gemma4:12b` 53%. The
        owner asked for exactly that rung and there was no way to press it.

        A measurement that is correct, present and unusable has not arrived. It is the fifth face
        of the cold-instrument rule and the one this codebase keeps rediscovering.

        It goes through `use_measured`, the same door `USE IT ANYWAY` uses — a rung is a measured
        configuration like any other, and the Engine already knows how to put one into effect.
      */}
      {rows.map((it) => (
        <button
          type="button"
          className="curve__row curve__row--rung"
          key={it.loadout.context}
          disabled={busy}
          title="Load this model with this window"
          onClick={() => onChoose(it.at)}
        >
          <span className="curve__ctx">{thousands(it.loadout.context)}</span>
          <span className="curve__rate">
            {it.generation.toFixed(1)} tok/s
          </span>
          <span className="curve__bar" aria-hidden>
            <i style={{ width: `${(it.generation / fastest) * 100}%` }} />
          </span>
        </button>
      ))}
    </div>
  );
}

export function profilesOf(found: Optimized | undefined): readonly Shown[] {
  if (found === undefined) return [];
  /*
    **Read, not re-derived.** This function held a full copy of `profiles::Optimized::resolve`,
    defended on the grounds that the alternative was a command per intent per row on every render.
    That was true while the only way to ask was a round trip, and it stopped being true when an
    optimisation began travelling as a view — the answer now arrives on the same payload as the
    rows it was computed from.

    Then the copy did what a copy does. `Verdict::dominates` was added to the Engine on
    2026-09-02 so a configuration whose slowest run beat another's fastest could be recommended
    whatever its label; the deck kept showing 48.6 for a model the Engine had begun answering 52.4
    for, and neither side was wrong about its own rule.

    What stays here is presentation: the wording, the steadiness sentence, and which row is
    marked. None of it decides anything.
  */
  return (found.profiles ?? []).map((it) => ({
    intent: it.intent,
    name: nameOf(it.intent),
    about: aboutOf(it.intent),
    generation: it.configuration.verdict?.median ?? it.configuration.generation,
    context: it.configuration.loadout.context,
    vram: it.configuration.vramUsed,
    recommended: it.intent === "balanced",
    sameAs: it.sameAs === null ? null : nameOf(it.sameAs),
    steadiness: steadinessOf(it.configuration),
  }));
}

/**
 * The one Epoch stands behind, or `null` where a search produced nothing it can.
 *
 * **An index into `profilesOf`, never a second resolution.** Two places deciding which profile is
 * recommended is how the completion panel and the CONFIGURE list come to disagree the first time
 * the rule changes.
 */
export function recommendedOf(found: Optimized | undefined): Shown | null {
  return profilesOf(found).find((it) => it.recommended) ?? null;
}

/**
 * Whether every intent landed on one configuration.
 *
 * **Four cards saying the same thing is the UI pretending to four answers.** Measured
 * 2026-09-01 on `gemma4:12b`: eight candidates spanning 3.3%, seven of them inside 0.5%, so the
 * tie-break picked the plainest and every intent then resolved to it — AUTO, BALANCED, FAST and
 * MAX QUALITY, all `51.7 tok/s · 32,768 · 11.9 GB`, three of them captioned *same as AUTO*.
 *
 * Nothing there was false. What was false was the shape: a list of four implies four things were
 * found, and one thing was. The honest rendering is one card and a sentence saying so — and it
 * is the same rule that keeps LONG CONTEXT absent until a context has been varied.
 */
export function oneAnswerOnly(found: Optimized | undefined): boolean {
  const shown = profilesOf(found);
  return shown.length > 1 && shown.slice(1).every((it) => it.sameAs !== null);
}

/** What a finished search may say, and it is decided in one place. */
export type Outcome =
  | { readonly kind: "recommended"; readonly shown: Shown }
  | { readonly kind: "nothing"; readonly why: string };

/**
 * What to show when a search ends.
 *
 * **The outcome belongs to the run, and it is carried rather than looked up.** The first version
 * of the completion panel held the model's name and read its optimization back — which is the
 * *stored* one, from whichever search last succeeded. So a run that refused at the gate, wrote
 * nothing and paused still drew `OPTIMIZATION COMPLETE · ★ BALANCED · 53.4 tok/s · APPLY` over
 * the previous run's record: a recommendation offered on evidence this run did not produce, which
 * is the one thing the golden test exists to catch. Measured 2026-09-01, and caught by it.
 *
 * A refusal wins over anything stored, always. What the model was measured at before is still
 * true and still on the row above; it is simply not what this run found.
 */
export function outcomeOf(
  refused: string | null,
  found: Optimized | undefined,
): Outcome {
  if (refused !== null) return { kind: "nothing", why: refused };
  const shown = recommendedOf(found);
  if (shown !== null) return { kind: "recommended", shown };
  return {
    kind: "nothing",
    why:
      found?.session?.note ??
      "The search ended without a configuration it could stand behind.",
  };
}

/**
 * The profile actually in force for a model, or `null` where none is.
 *
 * **`chosen` is what somebody pressed, not what Epoch would pick.** The recommendation and the
 * applied profile are different facts and they are usually the same row, which is exactly why
 * reading one for the other survives until the day they differ.
 */
export function appliedProfile(found: Optimized | undefined): Shown | null {
  if (found?.chosen == null) return null;
  return profilesOf(found).find((it) => it.intent === found.chosen) ?? null;
}

/**
 * What a profile costs against the fastest thing measured for this model.
 *
 * ## Why a profile has to carry this
 *
 * Measured 2026-09-02 on `gemma4:12b`, five windows filled to about nine tenths:
 *
 * ```text
 *  16K   40.7 tok/s
 *  32K   39.7
 *  64K   37.8
 * 128K   18.8
 * 256K    2.6
 * ```
 *
 * `LONG CONTEXT` resolves to the largest window that stayed stable, and 256K did: three runs
 * spanning 2.6–2.7, closed by a control that reproduced. Honest by its own definition, and
 * **2.6 tok/s means about a minute for a short reply.** Somebody picks it because they want a
 * long conversation and gets something that feels broken — a reading that is correct, present
 * and unusable, which is the one failure of a gauge that nothing in the number itself reveals.
 *
 * So the number carries what it costs. Nothing is hidden and no threshold decides anything: the
 * ratio is derived from the rows themselves and the person reads it.
 *
 * **Two formats, because one would make the other unreadable.** `2% slower` and `15× slower` are
 * each natural in their own range, and rendering both as a percentage buries the cliff in
 * `1,435%`.
 */
export function costOf(shown: Shown, fastest: number): string | null {
  if (fastest <= 0 || shown.generation <= 0) return null;
  const times = fastest / shown.generation;
  if (times < 1.02) return null;
  return times >= 2
    ? `${times.toFixed(times >= 10 ? 0 : 1)}\u00d7 slower`
    : `${((times - 1) * 100).toFixed(0)}% slower`;
}

export function summarise(card: BenchCard): readonly string[] {
  const runs = card.speed.runs.filter((it) => it.failed === null);
  const rates = runs
    .map((it) => it.generation)
    .filter((it): it is number => it !== null)
    .sort((a, b) => a - b);
  const middle = rates[Math.floor(rates.length / 2)] ?? null;

  const context = `${String(Math.round(card.conditions.context / 1024))}K`;
  const speed = middle === null ? "no answer" : `${middle.toFixed(1)} tok/s`;

  /*
    **Three rates per column, and they are still never collapsed — but they are not all shouted.**

    A column carries correctness (of what it answered, how much was right), completion (of what it
    was asked, how much it answered) and effective (of what it was asked, how much came back
    right). One *figure* can say only one of those, and that reasoning has not changed: an
    unanswered trial either counts as wrong — it was never given — or costs nothing, which is
    worse, because a task nobody completed did not go well.

    What changed is that printing all three as percentages, each with its own fraction, made a
    row nobody could read:

    ```text
    reasoning  correct 100.0% (4/4) . completed 80.0% (4/5) . effective 80.0% (4/5)
    ```

    Nine numbers, six of them repeats, and three words a person has to be taught. The owner
    pointed at it: *eso confunde al usuario.*

    **So: the fraction that answers the question, and the shortfall named only where there is
    one.** `4 of 5 right` is effective — of what it was asked, how much came back right — and
    `1 unanswered` is the whole reason it is not five. Nothing is lost: answered is `asked` minus
    the unanswered, correct is the numerator, and the row reads in one pass.

    A column that was perfect says so and stops, which is the point: the second half exists to
    explain a shortfall, and there is no shortfall to explain.

    The Engine still computes everything from one set of counts. This renders them. There is no
    second implementation of the arithmetic here, and the reason is that there was one and it
    disagreed with the Engine in public on the first real run.
  */
  // A tool call is judged in parts, so a tally can be fractional. Shown whole when it is whole.
  const tally = (what: number): string =>
    Math.abs(what - Math.round(what)) < 0.05 ? what.toFixed(0) : what.toFixed(1);

  const lines = card.columns.map((column) => {
    const name = column.kind.padEnd(10, " ");
    if (column.asked === 0) return `${name} not asked`;
    const wrong = Math.max(0, column.answered - column.correct);
    const unanswered = Math.max(0, column.asked - column.answered);
    const why = [
      wrong >= 0.05 ? `${tally(wrong)} wrong` : null,
      unanswered > 0 ? `${String(unanswered)} unanswered` : null,
    ].filter((it): it is string => it !== null);
    const said = `${name} ${tally(column.correct)} of ${String(column.asked)} right`;
    return why.length === 0 ? said : `${said}  ·  ${why.join(" · ")}`;
  });
  return [`${context} · ${speed}`, ...lines];
}
