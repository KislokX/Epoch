import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { weigh } from "../lib/weigh";

import {
  assetBases,
  findAssets,
  importAsset,
  installAsset,
  type AssetRow,
  type MoreRow,
} from "../ipc/launcher";
import { openLink, previewSrc } from "../ipc/world";

/**
 * Finding something that draws, and bringing it in (ADR-0031, Phase 11.3–11.4).
 *
 * ## One search, several sites
 *
 * Civitai and Hugging Face are asked together and ranked as one list, so the better of two sites
 * is not buried under the worse of the first. Nothing here knows which answered — that is the
 * whole content of ADR-0031.
 *
 * ## A refusal is not an empty result
 *
 * Measured: Civitai's free-text search answered *temporarily overloaded* six times over several
 * minutes and then worked perfectly. So a site that did not answer is named beside the results
 * rather than folded into them — telling somebody their style does not exist because a server
 * was busy is the same failure as a cold instrument reading zero.
 *
 * ## Paged per source, because they page differently
 *
 * Civitai hands out a next-page URL; Hugging Face takes an offset into a fixed sort. One running
 * out is not the end of the other, so each carries its own place and only the ones with more are
 * asked again. NEXT is absent — not disabled-looking — when there is nowhere to go.
 *
 * ## The same shape as the other two shelves
 *
 * `wk__search`, `wk__grid`, `wk__pages`. A third visual language for the third shelf would be
 * three things to learn about one Workshop.
 */
/** What each medium is called on the filter, once. */
const MAKES_LABEL: Record<string, string> = {
  all: "EVERYTHING",
  picture: "IMAGE",
  video: "VIDEO",
  sound: "AUDIO",
  model: "3D",
};

export function FindAssets() {
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState("lora");
  const [adult, setAdult] = useState(false);
  const [order, setOrder] = useState("most_downloaded");
  const [base, setBase] = useState("");
  const [bases, setBases] = useState<readonly string[]>([]);

  // The list is the Engine's, measured. Typing it here would be a second answer that drifts.
  useEffect(() => {
    void assetBases().then(setBases);
  }, []);
  const [busy, setBusy] = useState(false);
  const [found, setFound] = useState<readonly AssetRow[] | null>(null);
  const [refused, setRefused] = useState<readonly string[]>([]);
  const [more, setMore] = useState<readonly MoreRow[]>([]);
  const [card, setCard] = useState("");
  const [trail, setTrail] = useState<readonly (readonly MoreRow[])[]>([]);
  const [said, setSaid] = useState<string | null>(null);
  const [fetching, setFetching] = useState<string | null>(null);
  /**
   * Which version of each asset is chosen, when it has more than one.
   *
   * Keyed by asset, not global: two cards on screen are two independent choices, and one
   * dropdown that moved both would be a control that lies about what it governs.
   */
  const [version, setVersion] = useState<Record<string, string>>({});

  /**
   * Which medium the results are narrowed to. `all` is what somebody opens this in.
   *
   * **Over the results and not into the query**, deliberately. Civitai has no medium parameter
   * at all and Hugging Face has a pipeline filter — asking only the one that can answer would
   * quietly turn a search of two sites into a search of one, which the `base` control already
   * does and already says so on the page. Narrowing what came back narrows nothing about where
   * it came from.
   */
  const [makes, setMakes] = useState<
    "all" | "picture" | "video" | "sound" | "model"
  >("all");

  /**
   * **Unplaced results stay.** A source that did not say what a thing makes has not said it is
   * not this one, and dropping it would hide somebody's answer for a failure of the catalogue's.
   * The same rule the shelves follow, one measurement weaker.
   */
  const showing = (found ?? []).filter(
    (row) => makes === "all" || row.makes === null || row.makes === makes,
  );

  const look = (from: readonly MoreRow[]) => {
    setBusy(true);
    setSaid(null);
    void findAssets(query, kind, adult, order, base, from).then((answer) => {
      setFound(answer.assets);
      setRefused(answer.refused);
      setMore(answer.more);
      setCard(answer.card);
      setBusy(false);
    });
  };

  const start = () => {
    setTrail([]);
    look([]);
  };

  const install = (row: AssetRow) => {
    // The chosen version, or the newest — which is what the card shows when nobody has touched
    // the dropdown.
    const which = version[row.id] ?? row.versions[0]?.id ?? row.id;
    setFetching(row.id);
    setArrived(null);
    /*
      **It said *this takes as long as it takes*, which is true and is not a reading.**

      Six gigabytes with nothing on screen but a word: nobody learns whether it is half done or
      barely started, and a wait with no number is what somebody stares at before deciding the
      program has hung. The bar comes from `workshop:fetching`, which the Engine emits from the
      thread doing the reading.
    */
    setSaid(`Fetching ${row.name}. ${weigh(row.bytes)}.`);
    void installAsset(row.catalogue, which).then((outcome) => {
      setFetching(null);
      setArrived(null);
      setSaid(outcome.said);
    });
  };

  /**
   * How far the download has got: what has arrived, and the total the source stated.
   *
   * `total: null` is **no total**, never zero — a source that sent no `Content-Length` and
   * published no size. The bar is then absent and the bytes are shown, because a fraction of an
   * unknown denominator is the invented reading this codebase keeps deleting.
   */
  const [arrived, setArrived] = useState<{
    readonly id: string;
    readonly done: number;
    readonly total: number | null;
  } | null>(null);

  useEffect(() => {
    const stop = listen<{ id: string; done: number; total: number | null }>(
      "workshop:fetching",
      (event) => setArrived(event.payload),
    );
    return () => {
      void stop.then((off) => off());
    };
  }, []);

  return (
    <>
      <form
        className="wk__search"
        onSubmit={(e) => {
          e.preventDefault();
          start();
        }}
      >
        <input
          type="search"
          value={query}
          placeholder="Search Civitai and Hugging Face — watercolour, pixel art, a name…"
          onChange={(e) => setQuery(e.target.value)}
        />
        <select value={kind} onChange={(e) => setKind(e.target.value)}>
          <option value="lora">LoRAs</option>
          <option value="checkpoint">Models</option>
          <option value="vae">VAEs</option>
          <option value="controlnet">ControlNets</option>
          {/*
            **Upscalers, which the Engine could always search for and this list never offered.**
            `Kind::Upscaler` maps to Civitai's own `types=Upscaler` and has since the registry was
            built; the shelf, the identification and the install path were all there. The only
            thing missing was the word in this menu — which is the same shape as the shelf that
            filled up and fed nothing, one screen earlier in the journey.
          */}
          <option value="upscaler">Upscalers</option>
          <option value="embedding">Embeddings</option>
          <option value="">Everything</option>
        </select>
        {/*
          Only the two orders both sites genuinely support. Alphabetical is offered by neither,
          and a control that could reorder only the page on screen would be a sort that lies
          about its scope.
        */}
        <select value={order} onChange={(e) => setOrder(e.target.value)}>
          <option value="most_downloaded">Most downloaded</option>
          <option value="newest">Newest</option>
        </select>
        {/*
          The sites' own words, not Epoch's five families. Thirty-eight labels appeared across
          400 models; Epoch can build graphs for five architectures, and those are two different
          questions. What it can *draw with* is settled later, from the bytes.
        */}
        <select
          value={base}
          title={
            base === ""
              ? undefined
              : "Only Civitai publishes a base model, so only Civitai is asked while this is set."
          }
          onChange={(e) => setBase(e.target.value)}
        >
          <option value="">Any base model</option>
          {bases.map((it) => (
            <option key={it} value={it}>
              {it}
            </option>
          ))}
        </select>
        {/*
          **Beside the other three, and there before a search rather than after one.**

          It was under the results, drawn only once something had come back — so somebody
          looking at an empty Workshop saw three dropdowns and no way to say *video*, which is
          the thing they came to say. Reported as *no está en EPOCH*, and it was not: it was
          below the fold of a page with nothing on it.

          It narrows what came back rather than what is asked for, which is deliberate and
          explained under the row: Civitai has no medium parameter and asking only Hugging Face
          would turn a search of two sites into a search of one.
        */}
        <select
          value={makes}
          title="Narrows the results by what each site said it makes. Anything a site did not place is still shown."
          onChange={(e) => setMakes(e.target.value as typeof makes)}
        >
          {(["all", "picture", "video", "sound", "model"] as const).map(
            (which) => (
              <option key={which} value={which}>
                {MAKES_LABEL[which]}
              </option>
            ),
          )}
        </select>
        <button type="submit" className="btn btn--mini" disabled={busy}>
          {busy ? "…" : "SEARCH"}
        </button>
        {/*
          Beside the search because it answers the same question from the other side: *I already
          have one*. Nothing is uploaded — the shell opens a dialog and the Engine reads the file
          where it is.
        */}
        <button
          type="button"
          className="btn btn--mini"
          disabled={busy || fetching !== null}
          title="Take a file you already have into the library"
          onClick={() =>
            void importAsset().then((outcome) => {
              // A dismissed dialog is an answer, not news.
              if (outcome) setSaid(outcome.said);
            })
          }
        >
          IMPORT…
        </button>
      </form>

      <div className="wk__chips">
        <label className="setting">
          <input
            type="checkbox"
            checked={adult}
            onChange={(e) => setAdult(e.target.checked)}
          />
          <span>Include adult</span>
        </label>
      </div>

      <p className="cc__hint wk__note">
        Both sites, asked together and ranked as one list. A character may say
        what exists and may never install it — fetching gigabytes is not a
        decision a tool call makes.
        {base !== "" && (
          <>
            {" "}
            <b>Only Civitai publishes a base model</b>, so while one is chosen
            it is the only site asked — showing the other unfiltered beside it
            would be a filter that half-works.
          </>
        )}
        {/*
          **Said beside the control while it is doing something**, not hidden in a hover: this
          is a hint somebody needs to *use* the filter, not one they read afterwards, and the
          difference between those two has its own rule.
        */}
        {makes !== "all" && (
          <>
            {" "}
            <b>Narrowed to what each site said it makes</b> — nothing has been
            downloaded, so a result neither of them placed is still shown, and
            says so on its own row.
          </>
        )}
      </p>

      {/*
        Beside the results, never instead of them. Four refusals with four different fixes:
        busy, unreachable, unreadable, needs a key.
      */}
      {refused.map((why) => (
        <p key={why} className="wk__failed">
          {why}
        </p>
      ))}

      {/*
        Measured, and only what was measured. A machine with no card readable says nothing here
        rather than a reassuring sentence about one nobody found.
      */}
      {card !== "" && <p className="cc__hint wk__note">This machine: {card}</p>}

      {said && <p className="wk__added">{said}</p>}
      {/*
        **How far it has got, drawn only while something is arriving.**

        The track is the same one the loadout search uses on the Models deck — one bar in this
        product, not two that drift apart. With no total there is no fraction to draw, so the
        track stays empty and the bytes carry the reading: an invented denominator is worse than
        a number without one.
      */}
      {/*
        **Not keyed by the row's id, because the Engine is not asked for a row.**

        `install_asset` is given the *version* that was chosen — `civitai:185743#636318` — and it
        keys its progress by what it was asked for, which is right. The row is
        `civitai:185743`, and comparing the two found nothing: measured in the window, nine
        events arrived carrying `done` climbing to the exact total and the bar never drew.

        One download runs at a time here, and `fetching` is what says so. That is the guard; the
        id on the event is what will identify it the day several can.
      */}
      {fetching !== null && arrived !== null && (
        <p className="mdls__step" aria-live="polite">
          <span className="mdls__step-track">
            {arrived.total !== null && arrived.total > 0 && (
              <i
                style={{
                  width: `${Math.min(100, (arrived.done / arrived.total) * 100)}%`,
                }}
              />
            )}
          </span>
          <span>
            {/*
              **`weigh(0)` says *size not stated*, and that is right where it lives and wrong
              here.** A catalogue row with no size has none to state; nought bytes arrived is a
              measurement, and the first frame of every download read
              *size not stated of 38 MB — 0%*. Seen in the window on the first run.
            */}
            {arrived.total !== null && arrived.total > 0
              ? `${arrived.done === 0 ? "starting" : weigh(arrived.done)} of ${weigh(arrived.total)} — ${Math.floor((arrived.done / arrived.total) * 100)}%`
              : arrived.done === 0
                ? "starting — this source did not say how big it is"
                : `${weigh(arrived.done)} so far — this source did not say how big it is`}
          </span>
        </p>
      )}

      <div className="wk__grid">
        {showing.map((row) => (
          <div key={row.id} className="wk__card">
            {/*
              **What it makes, which a name cannot say.** Choosing a style LoRA out of forty
              names is choosing blind, and both sources publish exactly the picture that settles
              it.

              `previewSrc` — the Engine fetches and serves it, so this `<img>` is a
              request to Epoch and never to Civitai. `loading="lazy"` because a page of results
              is twenty of these and only the ones on screen are worth a round trip.

              Not drawn for an asset the source marked adult: the filter above already decides
              whether those are listed at all, and a picture is a stronger thing to put on screen
              than a name.
            */}
            {row.preview !== null && !row.adult && (
              <img
                className="wk__shot"
                src={previewSrc(row.preview)}
                alt=""
                loading="lazy"
                // A source that answered with something that is not a picture leaves a broken
                // frame; there is nothing to say about it, so the frame goes.
                onError={(e) => {
                  e.currentTarget.style.display = "none";
                }}
              />
            )}
            <div className="wk__cardHead">
              <b>{row.name}</b>
              <span className="wk__from">{row.source}</span>
            </div>
            <p className="wk__cardWhat">
              {row.kind} · {row.saidBase || "base not stated"}
              {row.family !== "unknown" && ` (${row.family.toUpperCase()})`}
            </p>
            {/*
              **Why an unplaced result is in a filtered list**, said on the row that raises the
              question. Measured on a search for *video*: 48 results, 11 of which neither site
              placed — so they appear under VIDEO, under AUDIO and under IMAGE alike, which is
              the rule working and looks like the rule being broken.

              Only while a medium is chosen: with EVERYTHING pressed nothing needs explaining,
              and a mark on every row marks nothing.
            */}
            {makes !== "all" && row.makes === null && (
              <p className="cc__hint">
                {row.source} did not say what this makes — shown under every medium rather than
                hidden from the right one
              </p>
            )}
            <p className="cc__hint">
              {weigh(row.bytes)}
              {/*
                `null` is *nobody measured*, never *no*. And one that does not fit is still
                offered — Epoch measured and said; the decision is theirs.
              */}
              {row.fits === true && <> · fits</>}
              {row.fits === false && <> · larger than the free memory</>}
              {" · "}
              {row.downloads.toLocaleString()} downloads
              {row.by && <> · by {row.by}</>}
              {row.adult && <> · adult</>}
            </p>
            {row.triggers.length > 0 && (
              <p className="cc__hint">
                Needs in a prompt: {row.triggers.join(", ")}
              </p>
            )}
            {/*
              Only where there is something to choose. A dropdown with one entry is a control
              that changes nothing, and a LoRA published for two engines is two different files.
            */}
            {row.versions.length > 1 && (
              <select
                value={version[row.id] ?? row.versions[0]!.id}
                onChange={(e) =>
                  setVersion((held) => ({ ...held, [row.id]: e.target.value }))
                }
              >
                {row.versions.map((it) => (
                  <option key={it.id} value={it.id}>
                    {it.name} · {it.saidBase || "base not stated"} ·{" "}
                    {weigh(it.bytes)}
                  </option>
                ))}
              </select>
            )}

            <div className="wk__cardActs">
              {/*
                The page it came from. Epoch never fetches it — it is offered, because the
                thing a person most wants before downloading six gigabytes is to look at it.
              */}
              <button
                type="button"
                className="btn btn--mini"
                onClick={() => void openLink(row.page)}
              >
                SOURCE
              </button>
              <button
                type="button"
                className="btn btn--mini"
                disabled={fetching !== null}
                title={
                  row.needsKey
                    ? "This site wants your own API key — Settings → Catalogue keys"
                    : undefined
                }
                onClick={() => install(row)}
              >
                {fetching === row.id
                  ? "FETCHING…"
                  : row.needsKey
                    ? "NEEDS A KEY"
                    : "INSTALL"}
              </button>
            </div>
          </div>
        ))}
      </div>

      {found !== null && (trail.length > 0 || more.length > 0) && (
        <div className="wk__pages">
          <button
            type="button"
            className="btn btn--mini"
            disabled={busy || trail.length === 0}
            onClick={() => {
              const back = trail.slice(0, -1);
              setTrail(back);
              look(back[back.length - 1] ?? []);
            }}
          >
            ← PREVIOUS
          </button>
          <span>PAGE {trail.length + 1}</span>
          <button
            type="button"
            className="btn btn--mini"
            disabled={busy || more.length === 0}
            onClick={() => {
              const forward = [...trail, more];
              setTrail(forward);
              look(more);
            }}
          >
            NEXT →
          </button>
        </div>
      )}

      {found !== null &&
        found.length === 0 &&
        !busy &&
        refused.length === 0 && (
          <p className="cc__hint">Nothing matched. Both sites answered.</p>
        )}
    </>
  );
}
