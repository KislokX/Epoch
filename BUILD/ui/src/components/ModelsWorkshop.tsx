import { useEffect, useState } from "react";

import { listen } from "@tauri-apps/api/event";

import { Pager, slice } from "./Pager";
import { ModelVariants } from "./ModelVariants";

import type { Facet, MachineView, ModelOffer } from "../ipc/contracts";
import {
  fetchFeaturedModels,
  fetchMachine,
  searchModels,
  searchOllama,
  pullModel,
  suitedModels,
  timeModel,
  type SuitedModel,
  huggingFaceCli,
  planDownload,
  downloadFile,
  type HuggingFaceCli,
  weighModel,
  whoCanTime,
  type TimeableOn,
} from "../ipc/launcher";

/**
 * The facets, and **which source can answer each**.
 *
 * This is the honest half. Ollama declares `vision`, `tools`, `thinking` and `audio` for models
 * that are installed; Hugging Face describes vision, image and audio through `pipeline_tag` and
 * says nothing at all about tools or reasoning; Ollama's featured list annotates nothing.
 *
 * So a chip that Hugging Face cannot answer says so instead of returning an empty shelf and
 * letting somebody conclude no model does it.
 */
const FACETS: readonly { readonly id: Facet; readonly askable: boolean }[] = [
  { id: "vision", askable: true },
  { id: "image", askable: true },
  { id: "audio", askable: true },
  { id: "tools", askable: false },
  { id: "thinking", askable: false },
];

/**
 * The Models Workshop — what this machine is, what it has, and what will fit.
 *
 * ## Why the first row is the machine
 *
 * A list of models with no idea whether they run is a shop with no prices. The number that
 * decides is **free video memory**: a model that does not fit is not slower, it is a different
 * experience — it spills into system memory and answers at a fraction of the speed.
 *
 * A machine with no NVIDIA card has **no VRAM reading**, and this says so rather than showing a
 * zero. Zero would mean *nothing fits*, which is a claim, and nobody measured it.
 *
 * ## "Featured" is not "the library"
 *
 * `ollama.com/api/tags` returned nineteen entries when this was measured, of mixed shape, and
 * did not include a model this machine has installed. Labelling it a catalogue would make
 * somebody believe those nineteen are their options.
 *
 * So the useful half is the field: **any name can be weighed**, because the manifest endpoint
 * takes any name. Somebody who knows they want `qwen3:14b` types it and gets a real size and a
 * real answer.
 *
 * ## Recommending is not choosing
 *
 * It says what fits. It does not stop anybody assigning a model their card cannot hold — Epoch
 * measured and told them, which is the whole of its job here.
 */
/**
 * How many models one page of a list holds.
 *
 * Chosen so the panel keeps the height it was designed at rather than to a round number: the
 * card is the thing being protected, and a list that decides the card's size is a list that has
 * stopped being part of a layout.
 */
/**
 * A pull string as a repository and a quantisation.
 *
 * `hf.co/unsloth/x:Q4_K_M` is Ollama's spelling; `hf` takes the two halves separately. The colon
 * is the split, and a repository path has none — so the last one is the one that matters.
 */
function split(pull: string): [string, string | null] {
  const rest = pull.replace(/^hf\.co\//, "");
  const at = rest.lastIndexOf(":");
  return at === -1 ? [rest, null] : [rest.slice(0, at), rest.slice(at + 1)];
}

const PER_PAGE = 24;

/**
 * A repository name small enough for a box, without inventing one.
 *
 * The half after the slash, which is the model — `unsloth/` is who published it, and twenty
 * cards that all begin with an organisation are twenty cards you cannot tell apart at a glance.
 * The whole name is still on the card's `title`, so nothing is lost, only folded.
 */
function shortly(repo: string): string {
  const cut = repo.lastIndexOf("/");
  return cut < 0 ? repo : repo.slice(cut + 1);
}

interface ModelsWorkshopProps {
  /**
   * Take this model to MODELS and map its curve there.
   *
   * A prop rather than a call, because the measuring belongs to the deck that already has the
   * row, the curve table and the progress bar. Absent when this Workshop is rendered somewhere
   * with nowhere to send it, and the offer simply does not appear.
   */
  readonly onMeasure?: (model: string) => void;
}

export function ModelsWorkshop({ onMeasure }: ModelsWorkshopProps = {}) {
  const [machine, setMachine] = useState<MachineView | null>(null);
  const [shelf, setShelf] = useState<readonly ModelOffer[]>([]);
  const [asking, setAsking] = useState("");
  const [weighed, setWeighed] = useState<ModelOffer | string | null>(null);
  const [busy, setBusy] = useState(false);
  const [query, setQuery] = useState("");
  const [chosen, setChosen] = useState<readonly Facet[]>([]);
  const [found, setFound] = useState<readonly ModelOffer[] | string | null>(
    null,
  );
  const [searching, setSearching] = useState(false);
  /**
   * Which page of each list is shown.
   *
   * Hugging Face returns forty results and Ollama's shelf is a short mixed list, and both were
   * drawn in full: the panel grew past the bottom of the window and the last rows could not be
   * reached at all. Cutting the lists would be worse — it would mean the Workshop quietly stops
   * mentioning models that exist.
   *
   * Pages keep the panel's proportions and keep every result reachable, and a page is
   * addressable in a way a clipped list is not.
   */
  const [foundPage, setFoundPage] = useState(0);
  /**
   * The cursor for the next page of Hugging Face results, when there is one.
   *
   * `null` means this is the end of the list — which is a different fact from "we asked for
   * forty and stopped", and the difference is what the MORE button exists to show.
   */
  const [moreCursor, setMoreCursor] = useState<string | null>(null);
  /**
   * Which result is open, if any.
   *
   * One at a time: a repository's quantisation table is twenty-odd chips, and two of them open
   * at once is a panel nobody can read. Clicking the open one closes it.
   */
  const [opened, setOpened] = useState<string | null>(null);
  const [fetchingMore, setFetchingMore] = useState(false);
  /** What the local runtime is saying about a download, in its own words. */
  const [pulling, setPulling] = useState<{
    model: string;
    status: string;
    total: number | null;
    completed: number | null;
  } | null>(null);
  const [pulled, setPulled] = useState<string | null>(null);
  /**
   * What just landed, if anything did, so the offer names it.
   *
   * Separate from the sentence above because a failure is also a sentence and must not come
   * with a button offering to measure something that is not there.
   */
  const [arrived, setArrived] = useState<string | null>(null);
  /**
   * Whether this machine has the Hugging Face CLI.
   *
   * It is the only way to put a GGUF **on disk**, which is what llama.cpp and LM Studio take.
   * Ollama's endpoint files a model where Ollama will find it and nowhere else, so without this
   * those two runtimes could only ever be handed a URL.
   */
  const [hf, setHf] = useState<HuggingFaceCli | null>(null);
  const [fileSaid, setFileSaid] = useState<string | null>(null);
  const [fetching, setFetching] = useState<string | null>(null);
  const [shelfPage, setShelfPage] = useState(0);

  /**
   * One search box, two registries.
   *
   * `ollama pull` takes names from both and they are not the same service: `qwen3-coder`
   * resolves against Ollama's own registry, `unsloth/Qwen3-GGUF` against Hugging Face. Making
   * somebody choose a registry before searching would be making them answer a question about
   * Epoch's plumbing in order to ask one about models.
   *
   * **Ollama's first**, because its names are curated and short and are what somebody who has
   * used it before types. Each row says where it came from, so the two never blur.
   *
   * **A failure on one side is not a failed search.** Ollama has no search API — the names come
   * off its page — so it is the half that can rot, and Hugging Face still answering is a real
   * result rather than a consolation.
   */
  const hunt = async (text: string, facets: readonly Facet[]) => {
    setSearching(true);
    const [shelf, answer] = await Promise.all([
      searchOllama(text),
      searchModels(text, facets),
    ]);
    setSearching(false);
    const curated = Array.isArray(shelf) ? shelf : [];
    if (typeof answer === "string") {
      // Ollama's half may still have answered, and showing it beats showing an error over
      // results that exist.
      if (curated.length > 0) {
        setFound(curated);
        setMoreCursor(null);
      } else {
        setFound(answer);
        setMoreCursor(null);
      }
    } else {
      setFound([...curated, ...answer.offers]);
      setMoreCursor(answer.more);
    }
    // A new search is a new list, and staying on page three of the old one would show an empty
    // panel for a search that found plenty.
    setFoundPage(0);
  };

  /**
   * Ask Hugging Face for the next page, and **add** it to what is already shown.
   *
   * Appending rather than replacing, because the pages here are not addressable — a cursor says
   * "after this one" and there is no way back to page two. Replacing would make MORE a one-way
   * door that loses what the user was looking at.
   */
  const more = async () => {
    if (!moreCursor || !Array.isArray(found)) return;
    setFetchingMore(true);
    const answer = await searchModels(query, chosen, moreCursor);
    setFetchingMore(false);
    if (typeof answer === "string") {
      setMoreCursor(null);
      return;
    }
    setFound([...found, ...answer.offers]);
    setMoreCursor(answer.more);
  };

  /**
   * The runtime's own account of a download, as it arrives.
   *
   * Its words rather than ours: "pulling manifest", "verifying sha256 digest", "writing
   * manifest". A sentence Epoch wrote in advance would be a worse description of what is
   * happening, and it would keep saying it after the truth had moved on.
   */
  useEffect(() => {
    let alive = true;
    const stops: Array<() => void> = [];
    void listen<{
      model: string;
      progress: {
        status: string;
        total: number | null;
        completed: number | null;
      };
    }>("models:pulling", (event) => {
      if (!alive) return;
      setPulling({ model: event.payload.model, ...event.payload.progress });
    }).then((off) => (alive ? stops.push(off) : off()));
    void listen<{ quant: string; at: string | null; failure: string | null }>(
      "models:downloaded",
      (event) => {
        if (!alive) return;
        setFetching(null);
        setFileSaid(
          event.payload.failure ??
            `Saved to ${event.payload.at ?? "this machine"}. Point llama.cpp or LM Studio at it.`,
        );
      },
    ).then((off) => (alive ? stops.push(off) : off()));
    void listen<{ model: string; failure: string | null }>(
      "models:pulled",
      (event) => {
        if (!alive) return;
        setPulling(null);
        setPulled(
          event.payload.failure ??
            `${event.payload.model} is on this machine now.`,
        );
        setArrived(event.payload.failure ? null : event.payload.model);
        // What is installed changed, so the shelf's "already here" marks are stale.
        void fetchFeaturedModels().then((next) =>
          typeof next === "string" ? undefined : setShelf(next),
        );
      },
    ).then((off) => (alive ? stops.push(off) : off()));
    return () => {
      alive = false;
      stops.forEach((off) => off());
    };
  }, []);

  /**
   * Put the GGUF on disk, for a runtime that takes a file.
   *
   * **The plan first.** A quantisation filter is a glob, and one that matches nothing downloads
   * nothing while looking like success — so what it would fetch is read back and shown before
   * seventeen gigabytes are committed to it.
   */
  const saveFile = async (pull: string) => {
    const [repo, quant] = split(pull);
    if (!quant) {
      setFileSaid("Pick a quantisation first — a repository is not one file.");
      return;
    }
    setFileSaid(null);
    setFetching(quant);

    const planned = await planDownload(repo, quant);
    if (typeof planned === "string") {
      setFetching(null);
      setFileSaid(planned);
      return;
    }
    if (planned.length === 0) {
      setFetching(null);
      // Said plainly rather than starting a download that fetches nothing.
      setFileSaid(`Nothing in that repository matches '${quant}'.`);
      return;
    }

    setFileSaid(
      `Fetching ${planned.map((p) => `${p.file} (${p.size})`).join(", ")}…`,
    );
    const failure = await downloadFile(repo, quant);
    if (failure) {
      setFetching(null);
      setFileSaid(failure);
    }
  };

  /**
   * What fits here, asked of the sources rather than kept in a list.
   *
   * `null` is unasked and `[]` is *nothing fits*, which are different sentences: a card with no
   * room says so, and a card nobody has asked about says nothing at all.
   */
  const [suited, setSuited] = useState<readonly SuitedModel[] | null>(null);
  const [looking, setLooking] = useState(false);

  /** Which model is being timed right now, if any. */
  const [timing, setTiming] = useState<string | null>(null);

  /**
   * Which runtimes are answering, and what each can time.
   *
   * **This list times too, so it had the same defect.** `timeModel` took whichever provider
   * answered first, and the row underneath said *measured on Ollama* about a choice nobody made.
   * There is no picker here — this is a list of models that *fit*, and the timing is a
   * convenience — so it names the runtime it will use on the button instead. Naming it is the
   * part that matters; a reading nobody can attribute is the one that misleads.
   */
  const [timeable, setTimeable] = useState<readonly TimeableOn[]>([]);
  useEffect(() => {
    void whoCanTime().then(setTimeable);
  }, []);

  const runtimeFor = (model: string) =>
    timeable.find((it) => it.models.includes(model));

  const time = (model: string) => {
    const on = runtimeFor(model)?.id;
    if (on === undefined) return;
    setTiming(model);
    setPulled(null);
    void timeModel(model, on).then((said) => {
      setTiming(null);
      setPulled(said);
      // The measured answer changes every estimate under it, so the list is re-read rather than
      // left showing numbers derived from a throughput that has just been improved on.
      lookForSuited();
    });
  };

  const lookForSuited = () => {
    setLooking(true);
    setSuited(null);
    void suitedModels().then((found) => {
      setLooking(false);
      if (typeof found === "string") {
        setPulled(found);
        return;
      }
      setSuited(found);
    });
  };

  const download = (model: string) => {
    setPulled(null);
    setPulling({ model, status: "starting", total: null, completed: null });
    void pullModel(model).then((failure) => {
      if (failure) {
        setPulling(null);
        setPulled(failure);
      }
    });
  };

  const toggle = (facet: Facet) => {
    const next = chosen.includes(facet)
      ? chosen.filter((f) => f !== facet)
      : [...chosen, facet];
    setChosen(next);
    void hunt(query, next);
  };

  useEffect(() => {
    void huggingFaceCli().then(setHf);
    void fetchMachine().then(setMachine);
    void fetchFeaturedModels().then((found) =>
      typeof found === "string" ? undefined : setShelf(found),
    );
  }, []);

  const weigh = async () => {
    await weighNamed(asking);
  };

  /**
   * Weigh a specific name.
   *
   * Named rather than reading `asking`, because a chip sets the field and weighs in the same
   * gesture — and `setAsking` has not landed by the time the next line runs. Passing the value
   * is the fix that does not depend on when React chooses to re-render.
   */
  const weighNamed = async (name: string) => {
    const wanted = name.trim();
    if (!wanted) return;
    setBusy(true);
    setWeighed(await weighModel(wanted));
    setBusy(false);
  };

  return (
    <div className="mws">
      <div className="mws__machine">
        <span className="rm__label">This machine</span>
        <p className="cc__hint">
          {machine?.gpu ?? "No graphics Epoch can ask about here."}
          {machine?.vramFree != null && machine.vramTotal != null && (
            <>
              {" · "}
              {gb(machine.vramFree)} of {gb(machine.vramTotal)}{" "}
              {machine.unified ? "unified memory " : ""}free
            </>
          )}
          {/* Not repeated on a unified machine: the total above is the same pool. */}
          {machine?.ramTotal != null &&
            !(machine.unified && machine.vramTotal != null) && (
              <> · {gb(machine.ramTotal)} system memory</>
            )}
        </p>
      </div>

      <label className="cedit__field cedit__field--wide">
        <span>Will it run here?</span>
        <div className="mws__ask">
          <input
            type="text"
            value={asking}
            placeholder="qwen3:14b"
            onChange={(e) => setAsking(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                void weigh();
              }
            }}
          />
          <button
            type="button"
            className="btn btn--mini"
            disabled={busy || !asking.trim()}
            onClick={() => void weigh()}
          >
            {busy ? "ASKING…" : "WEIGH"}
          </button>
        </div>
        <i className="cedit__hint">
          Any model name. The size comes from its manifest — what a pull
          actually downloads.
        </i>
      </label>

      {weighed !== null &&
        (typeof weighed === "string" ? (
          <p className="notice notice--warn">{weighed}</p>
        ) : (
          <p className="mws__verdict">
            {/*
              The verdict first, as a word rather than a sentence. "Will it run here?" is the
              question the field asks, and the answer was previously buried at the end of a line
              that began with a filename.
            */}
            <b
              className={`mws__runs mws__runs--${
                weighed.fits === true
                  ? "yes"
                  : weighed.fits === false
                    ? "no"
                    : "unknown"
              }`}
            >
              {weighed.fits === true
                ? "RUNS HERE"
                : weighed.fits === false
                  ? "TOO BIG"
                  : "CANNOT SAY"}
            </b>{" "}
            <b>{weighed.name}</b> ·{" "}
            {weighed.bytes != null ? gb(weighed.bytes) : "size unknown"}
            {" · "}
            {/*
              The words follow the architecture. On a unified machine there is no second pool to
              spill into, so "it will spill into system memory and run slowly" describes a machine
              this is not — and gives the opposite advice, because what does not fit does not get
              slower, it does not load.
            */}
            {weighed.fits === true
              ? `fits in free ${machine?.unified ? "unified" : "video"} memory`
              : weighed.fits === false
                ? machine?.unified
                  ? "larger than this machine's free unified memory, and it shares one pool with the system"
                  : "it will spill into system memory and run slowly"
                : "no memory reading, so nobody can say"}
            {/*
              A repository has no size — it has one per quantisation. When nobody named one,
              saying which was chosen is what stops the number from looking like the whole
              repository's.
            */}
            {weighed.source === "huggingface" && !asking.includes(":") && (
              <i>
                {" "}
                — the largest quantisation that runs here, of the ones published
              </i>
            )}
          </p>
        ))}

      {/*
        **Two buttons, because there are two prices.**

        There were three — `DOWNLOAD TO OLLAMA`, `SAVE THE FILE`, `SERVE WITH LLAMA.CPP` — for
        one intent, and the owner said so plainly on 2026-09-02: *"se complica mucho"*. Somebody
        who wants a model was being asked to decide about plumbing first, and one of the three
        did something different from the other two (it started a separate Service that then had
        to be added by hand under Connections). That one is gone.

        What is left is the honest split, and it is a split about **cost** rather than about
        runtimes:

        | destination | what it takes |
        |---|---|
        | llama.cpp | nothing — the vault holds it and the router serves the vault |
        | LM Studio | nothing — a hard link, no second copy |
        | Ollama | a full copy and a hash, minutes per model, the bytes on disk twice |

        Ollama has no search path and insists on its own blob store, so it cannot share what the
        other two share. Folding it into the first button would spend gigabytes in silence, which
        ADR-0032 refuses; giving it its own with the cost written beside it lets somebody choose.
      */}
      {weighed !== null && typeof weighed !== "string" && (
        <div className="mws__get">
          {hf?.installed && weighed.pull.startsWith("hf.co/") && (
            <button
              type="button"
              className="btn btn--mini"
              disabled={fetching !== null}
              onClick={() => void saveFile(weighed.pull)}
            >
              {fetching ? "DOWNLOADING…" : "DOWNLOAD FOR LLAMA.CPP AND LM STUDIO"}
            </button>
          )}
          <button
            type="button"
            className="btn btn--mini"
            /*
              A control that cannot do what it says is worse than a missing one. Ollama refuses
              a sharded tag by name, so the button that would ask it is off and the sentence
              beside it says whose refusal it is.
            */
            disabled={pulling !== null || weighed.pullable === false}
            onClick={() => download(weighed.pull)}
          >
            {pulling ? "DOWNLOADING…" : "DOWNLOAD FOR OLLAMA"}
          </button>
          <span className="cc__hint">
            {weighed.pull.startsWith("hf.co/")
              ? hf?.installed
                ? `One download, and llama.cpp and LM Studio both have it${hf.user ? ` (hf, signed in as ${hf.user})` : ""}. Ollama keeps its own copy — the same weights again on disk, and a few minutes to hash them — so it is a separate press.`
                : "For llama.cpp or LM Studio, copy the file instead — they take a GGUF from disk and have no way to be told to fetch one. The Hugging Face CLI would do it here; it is not on this machine."
              : "Into the local runtime configured under Connections."}
          </span>
        </div>
      )}

      {/*
        The runtime's own sentence, as it arrives. "pulling manifest", "verifying sha256 digest",
        "writing manifest" — a truer account than anything written in advance, and it stops being
        true the moment the download moves on, which a fixed sentence would not.
      */}
      {pulling && (
        <p className="mws__verdict">
          <b>{pulling.model}</b> · {pulling.status}
          {pulling.total != null &&
            pulling.completed != null &&
            pulling.total > 0 && (
              <>
                {" · "}
                {Math.round((pulling.completed / pulling.total) * 100)}% of{" "}
                {gb(pulling.total)}
              </>
            )}
        </p>
      )}
      {pulled && (
        <p className="notice">
          {pulled}
          {/*
            **The offer, only where there is something to offer.**

            A model that has just landed is loaded the conservative way — the safe half of every
            trade — until somebody maps its curve. Measured on this card, that is 20.3 tok/s
            against 35.2 at the same context, and nothing said so.

            It is a plain offer, not a prompt: no dialog, nothing modal, and leaving it alone is
            the other answer. The time is on the button because four minutes is a decision, and
            a button that hides what it costs is one people press once.
          */}
          {arrived && onMeasure && (
            <>
              {" "}
              <button
                type="button"
                className="btn btn--mini"
                onClick={() => onMeasure(arrived)}
              >
                MEASURE IT (~4 MIN)
              </button>
            </>
          )}
        </p>
      )}
      {fileSaid && <p className="notice">{fileSaid}</p>}

      {/*
        **What fits here**, which is a different question from what exists.

        Epoch measures the fitting: the card's free memory against the real byte count of one
        quantisation, summed across shards. The *order* is Hugging Face's own `sort=downloads` and
        is named as borrowed — there is no measurement of whether a model is good, and a list
        ordered by something Epoch invented, with a download button under it, would be exactly the
        gauge nobody can explain.

        Behind a button because it costs a moment: a repository is not a size, so answering *will
        it run here* means asking about each one.
      */}
      <div className="mws__shelf">
        <div className="rdy__head">
          <span className="rm__label">What fits on this machine</span>
          <button
            type="button"
            className="btn btn--mini"
            disabled={looking}
            onClick={lookForSuited}
          >
            {looking ? "ASKING…" : "SHOW ME"}
          </button>
        </div>
        <p className="cc__hint">
          What you already have first, then the largest quantisation of each
          candidate that fits your card, leaving room for the conversation.
        </p>
        <p className="cc__hint">
          The order is <b>not</b> a judgement of quality — Epoch has no way to
          measure that and does not pretend to. What it measures is what fits
          and, once you have timed one model, how fast the others would answer.
          New candidates are in Hugging Face&rsquo;s own download order.
        </p>

        {suited !== null && suited.length === 0 && (
          <p className="cc__hint">
            Nothing published on that list fits this card with room for a
            conversation. The search below takes a name, and WEIGH answers for
            any model you can name.
          </p>
        )}

        {/*
          **A grid, because twenty of these read as a shape and not as a list.** Ranked left to
          right, five across, so the best of them are one glance rather than one scroll. The full
          repository name is on the card's `title`: a box this narrow cannot hold
          `unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF`, and truncating without somewhere to read
          the whole thing would be hiding it.
        */}
        <div className="fits">
          {suited?.map((one) => (
            <div
              key={`${one.repo}${one.here ?? ""}`}
              className={`fits__box${one.here ? " fits__box--here" : ""}`}
              title={one.repo}
            >
              <b className="fits__rank">{one.rank}</b>
              <p className="fits__name">{shortly(one.repo)}</p>
              <p className="fits__where">
                {one.here ? `here · ${one.here}` : "Hugging Face"}
              </p>
              <p className="fits__size">
                {gb(one.bytes)}
                {one.quant ? ` · ${one.quant}` : ""}
              </p>
              {/*
                **Measured and estimated never blur.** A measurement names the runtime it was
                taken on and carries no tilde; an estimate carries one and names nothing, because
                it is this machine's own throughput divided by a size rather than an answer
                anybody timed.
              */}
              <p
                className={`fits__rate${one.measuredOn ? " fits__rate--measured" : ""}`}
              >
                {one.tokensPerSecond === null
                  ? "speed unknown"
                  : one.measuredOn
                    ? `${one.tokensPerSecond.toFixed(1)} tok/s`
                    : `~${one.tokensPerSecond.toFixed(0)} tok/s`}
              </p>
              <p className="fits__how">
                {one.tokensPerSecond === null
                  ? "time one below"
                  : one.measuredOn
                    ? `measured on ${one.measuredOn}`
                    : "estimated"}
              </p>
              {one.here ? (
                <button
                  type="button"
                  className="btn btn--mini"
                  disabled={timing !== null || runtimeFor(one.repo) === undefined}
                  title={
                    runtimeFor(one.repo) === undefined
                      ? "Nothing here is serving it right now. Start a runtime and it can be timed."
                      : "One short answer, timed on the runtime that gives it."
                  }
                  onClick={() => time(one.repo)}
                >
                  {timing === one.repo
                    ? "TIMING…"
                    : runtimeFor(one.repo)
                      ? `TIME IT ON ${runtimeFor(one.repo)!.name.toUpperCase()}`
                      : "TIME IT"}
                </button>
              ) : (
                <button
                  type="button"
                  className="btn btn--mini"
                  onClick={() => download(one.pull)}
                >
                  DOWNLOAD
                </button>
              )}
            </div>
          ))}
        </div>
      </div>

      <div className="mws__shelf">
        <span className="rm__label">Search Hugging Face</span>
        <div className="mws__ask">
          <input
            type="text"
            value={query}
            placeholder="qwen, llama, gemma…"
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                void hunt(query, chosen);
              }
            }}
          />
          <button
            type="button"
            className="btn btn--mini"
            disabled={searching}
            onClick={() => void hunt(query, chosen)}
          >
            {searching ? "…" : "SEARCH"}
          </button>
        </div>

        <div className="wk__chips">
          {FACETS.map((facet) => (
            <button
              key={facet.id}
              type="button"
              className={`chip${chosen.includes(facet.id) ? " chip--on" : ""}${
                facet.askable ? "" : " chip--cold"
              }`}
              disabled={!facet.askable}
              title={
                facet.askable
                  ? undefined
                  : "Hugging Face does not describe this. Ollama does, for models already installed."
              }
              onClick={() => toggle(facet.id)}
            >
              {facet.id}
            </button>
          ))}
        </div>
        <p className="cc__hint">
          GGUF only — what Ollama can actually pull. Two chips are cold because
          Hugging Face has no field for them; Ollama declares those, for models
          already on this machine.
        </p>

        {typeof found === "string" && (
          <p className="notice notice--warn">{found}</p>
        )}
        {/*
          **A search that found nothing says why, because the constraint is invisible.**

          The Workshop asks Hugging Face with `filter=gguf`, and a model that has no GGUF simply
          does not appear — so somebody searching for one they know exists gets an empty panel
          and no reason. That is the same silence this product has spent a week removing.

          Measured, 2026-08-21, and it is also why nothing here converts: `Qwen/Qwen3-8B` is
          five safetensors shards, and searching `Qwen3-8B` *with* the filter returns
          `Qwen/Qwen3-8B-GGUF` — the official conversion of the same model. For nearly anything
          somebody wants, the converted version already exists and this finds it.

          Where it does not, converting means fetching the full-precision weights (~16 GB for an
          8B in bf16 against ~5 GB as Q4), running `ollama create --experimental`, and holding
          both — three times the disk and an hour of CPU to produce something usually already
          published. That is a real feature with a real cost, and naming the constraint is the
          honest thing to do until somebody has a model that needs it.
        */}
        {Array.isArray(found) && found.length === 0 && (
          <p className="cc__hint">
            Nothing matched. Models are listed here only when somebody has
            published a GGUF of them — try the same name with <b>-GGUF</b> after
            it, which is what the official conversions are called.
          </p>
        )}

        {Array.isArray(found) && (
          <>
            <ul className="mws__list">
              {slice(found, foundPage, PER_PAGE).map((offer) => (
                <li
                  key={offer.pull}
                  className={opened === offer.name ? "mws__open" : undefined}
                >
                  {/*
                  **Opening a repository rather than resolving it.**

                  Pressing a name used to put it in the weighing field, where Epoch chose a
                  quantisation — the largest that fits. A reasonable answer to a question nobody
                  had asked precisely: somebody choosing between `UD-Q4_K_M` and `UD-Q4_K_XL` is
                  making a real decision. So the repository opens, and picking a chip is what
                  chooses.
                */}
                  <button
                    type="button"
                    className="mws__pick"
                    onClick={() =>
                      setOpened(opened === offer.name ? null : offer.name)
                    }
                  >
                    {opened === offer.name ? "▾ " : "▸ "}
                    {offer.name}
                  </button>
                  {offer.facets.length > 0 && (
                    <em>{offer.facets.join(" · ")}</em>
                  )}
                  {offer.installed && <em>already here</em>}
                  {opened === offer.name && (
                    <ModelVariants
                      repo={offer.name}
                      machine={machine}
                      onChoose={(pull) => {
                        // Straight into the field the Workshop already weighs with, so one
                        // verdict and one DOWNLOAD serve both ways of arriving at a model.
                        setAsking(pull);
                        setWeighed(null);
                        void weighNamed(pull);
                      }}
                    />
                  )}
                </li>
              ))}
            </ul>
            <Pager
              count={found.length}
              perPage={PER_PAGE}
              at={foundPage}
              onGo={setFoundPage}
              label="Hugging Face results"
            />
            {/*
            **There is always more.** Hugging Face hosts tens of thousands of GGUF repositories
            and answers a page at a time; showing one page and stopping implied that was the
            whole library. The count says how much has been *fetched*, never how much exists —
            nobody is told that number, so nobody may state it.
          */}
            <div className="mws__more">
              {moreCursor ? (
                <button
                  type="button"
                  className="btn btn--mini"
                  disabled={fetchingMore}
                  onClick={() => void more()}
                >
                  {fetchingMore ? "FETCHING…" : "MORE RESULTS"}
                </button>
              ) : (
                <span className="cc__hint">
                  That is the end of the results.
                </span>
              )}
              <span className="cc__hint">{found.length} fetched so far</span>
            </div>
          </>
        )}
      </div>

      <div className="mws__shelf">
        <span className="rm__label">Featured by Ollama</span>
        {/*
          Said plainly. This endpoint returns a short, mixed list — it is not everything a
          person can run, and a heading calling it a catalogue would say otherwise.
        */}
        <p className="cc__hint">
          A short list Ollama is promoting, not the whole library. Sizes are not
          shown here: the catalogue reports a family, and a family is not what a
          pull downloads.
        </p>
        <ul className="mws__list">
          {slice(shelf, shelfPage, PER_PAGE).map((offer) => (
            <li key={offer.name}>
              <button
                type="button"
                className="mws__pick"
                onClick={() => {
                  setAsking(offer.pull);
                  setWeighed(null);
                }}
              >
                {offer.name}
              </button>
              {offer.installed && <em>already here</em>}
            </li>
          ))}
        </ul>
        <Pager
          count={shelf.length}
          perPage={PER_PAGE}
          at={shelfPage}
          onGo={setShelfPage}
          label="Featured models"
        />
      </div>
    </div>
  );
}

/** Bytes as gigabytes, one decimal. A measurement, so it is written as one. */
function gb(bytes: number): string {
  return `${(bytes / 1_000_000_000).toFixed(1)} GB`;
}
