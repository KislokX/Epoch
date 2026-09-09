/**
 * The Voices Workshop — a shelf, not a sibling (Phase 15, ADR-0031).
 *
 * ## Why this is its own screen and not another word in the CREATIONS menu
 *
 * The Engine's half genuinely is one more shelf: one `Asset`, one `Catalogue`, one `fetch`, one
 * install path. The **screen** is not, and the difference is every control on the other one.
 * A base-model filter, an adult filter, a medium filter, a download count and a preview picture
 * are five things a voice has no answer to — and a row that answers nothing on five controls is
 * a screen that trains you to stop reading them.
 *
 * So: a search box, and rows that say what a voice is and how big it is. Nothing else, because
 * nothing else has been measured about one.
 *
 * ## What a row says before it is downloaded
 *
 * The name, what the voice **is**, and how big it is.
 *
 * The description was very nearly left off. Reasoned about, it lives in the sidecar — four
 * kilobytes on somebody else's server, and 176 round trips to paint a page, which is 14.2 s
 * measured against a shelf that paints in 1.5 s. So the first version showed a name and a size
 * and said the description would arrive when it was free, after the install.
 *
 * Then somebody searched `spanish` and was told **nothing here matched that**, on a shelf holding
 * nine Spanish voices. The only thing being matched was the path — `es/es_ES/davefx/medium` —
 * and the word a person actually uses appears nowhere in it.
 *
 * The repository publishes `voices.json`: the same fields, for every voice, in **one request,
 * 245 KB, 0.68 s**. So it was never a choice between an expensive description and none.
 *
 * > **A cost measured on one way of getting something is not a measurement of the thing.**
 */

import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { findAssets, installAsset, type AssetRow } from "../ipc/launcher";
import { sampleSrc } from "../ipc/world";
import { voiceVolume } from "../experience/audio";
import { weigh } from "../lib/weigh";

export function FindVoices() {
  const [query, setQuery] = useState("");
  const [rows, setRows] = useState<readonly AssetRow[]>([]);
  const [refused, setRefused] = useState<readonly string[]>([]);
  const [looking, setLooking] = useState(false);
  const [said, setSaid] = useState("");
  const [fetching, setFetching] = useState<string | null>(null);

  /**
   * How far the download has got.
   *
   * `total: null` is **no total**, never zero. A voice is tens of megabytes rather than the
   * gigabytes the other shelf deals in, so this is usually over in seconds — which is a reason
   * to keep the bar honest, not a reason to leave it out: the one that is slow is the one on a
   * bad connection, and that is exactly who needs it.
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

  const look = (words: string) => {
    setLooking(true);
    setSaid("");
    void findAssets(words, "voice", false, "most_downloaded", "", []).then(
      (answer) => {
        setLooking(false);
        setRows(answer.assets);
        setRefused(answer.refused);
      },
    );
  };

  // The shelf opens full rather than empty. There is one measured repository and its whole
  // contents are one request away, so an empty first paint would be a screen asking somebody to
  // guess a search term for a list nobody has seen.
  useEffect(() => {
    look("");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /**
   * Which row is playing, so pressing a second one stops the first.
   *
   * **One voice at a time here too.** Two samples over each other tells you nothing about
   * either, which is the same reason the crew speaks in a queue.
   */
  const [playing, setPlaying] = useState<string | null>(null);
  const sound = useRef<HTMLAudioElement | null>(null);

  useEffect(() => {
    // Leaving the shelf stops it. A voice still talking into a closed panel is the World
    // talking to nobody.
    return () => {
      sound.current?.pause();
      sound.current = null;
    };
  }, []);

  const hear = (row: AssetRow) => {
    sound.current?.pause();
    if (playing === row.id || row.sample === null) {
      setPlaying(null);
      return;
    }
    const audio = new Audio(sampleSrc(row.sample));
    // The crew's own volume, because this *is* the crew — the same slider that decides how loud
    // they are in a conversation.
    audio.volume = voiceVolume();
    audio.onended = () => setPlaying(null);
    audio.onerror = () => setPlaying(null);
    sound.current = audio;
    setPlaying(row.id);
    void audio.play().catch(() => setPlaying(null));
  };

  const install = (row: AssetRow) => {
    setFetching(row.id);
    setArrived(null);
    setSaid(`Fetching ${row.name}. ${weigh(row.bytes)}.`);
    void installAsset(row.catalogue, row.id).then((outcome) => {
      setFetching(null);
      setArrived(null);
      setSaid(outcome.said);
    });
  };

  return (
    <>
      <form
        className="wk__search"
        onSubmit={(e) => {
          e.preventDefault();
          look(query);
        }}
      >
        <input
          type="search"
          value={query}
          placeholder="A language, a country, a name — spanish, mexico, davefx…"
          onChange={(e) => setQuery(e.target.value)}
        />
        <button type="submit" className="btn btn--mini" disabled={looking}>
          {looking ? "LOOKING…" : "SEARCH"}
        </button>
      </form>

      <p className="cc__hint">
        Piper voices, from the repositories Epoch has measured. A voice is an{" "}
        <code>.onnx</code> with its sidecar beside it — that pairing is what makes it a voice
        here, not a tag and not its name.
      </p>

      {refused.map((why) => (
        <p key={why} className="cc__warn">
          {why}
        </p>
      ))}

      {said && <p className="cc__hint">{said}</p>}

      {/*
        **A refusal that says what this shelf can answer.**

        `Nothing here matched that` is a status report, and a status report about an empty screen
        teaches nothing — somebody searching `luffy voice` cannot tell whether the shelf is broken,
        whether Epoch is offline, or whether Piper simply has no character voices. It has none, and
        that is worth saying out loud along with the words that do work.

        The same discipline a tool's refusal already follows: say what is true, then say what
        exists, and never leave the reader to guess which of three things happened.
      */}
      {!looking && rows.length === 0 && refused.length === 0 && (
        <p className="cc__hint">
          Nothing here matched that. These are Piper voices, so what works is a language
          (<code>spanish</code>), a country (<code>mexico</code>), a locale
          (<code>es_MX</code>), a quality (<code>medium</code>) or a voice's own name
          (<code>davefx</code>) — not a character or a person.
        </p>
      )}

      <div className="wk__grid">
        {rows.map((row) => (
          <article key={row.id} className="wk__card">
            <div className="wk__cardHead">
              <b>{row.name}</b>
              <span className="wk__from">{row.source}</span>
            </div>
            {/*
              **What this voice is, before anybody downloads 63 MB to find out.** Read from the
              repository's own index — the same fields the sidecar carries, in one request rather
              than 176. Absent where no index described it, which reads as unmeasured rather than
              as a blank line.
            */}
            {row.saidBase !== "" && <p className="wk__cardWhat">{row.saidBase}</p>}
            <p className="cc__hint">
              {weigh(row.bytes)}
              {row.by !== null && ` · ${row.by}`}
            </p>
            {fetching === row.id && arrived?.id === row.id && (
              <p className="cc__hint">
                {arrived.total === null
                  ? weigh(arrived.done)
                  : `${weigh(arrived.done)} of ${weigh(arrived.total)}`}
              </p>
            )}
            {/*
              **A voice is the one asset whose description is worthless.** *es_ES-davefx-medium,
              medium, 22050 Hz* says nothing about how somebody sounds, and 63 MB is a lot to
              download to find out. So this shelf does not only describe: it plays.

              Absent rather than disabled where the repository published no recording — a button
              with nothing behind it is a control that does nothing.
            */}
            {row.sample !== null && (
              <button
                type="button"
                className="btn btn--mini"
                onClick={() => hear(row)}
              >
                {playing === row.id ? "STOP" : "LISTEN"}
              </button>
            )}
            <button
              type="button"
              className="btn btn--mini"
              disabled={fetching !== null}
              onClick={() => install(row)}
            >
              {fetching === row.id ? "FETCHING…" : "INSTALL"}
            </button>
          </article>
        ))}
      </div>
    </>
  );
}
