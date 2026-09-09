import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import {
  installEar,
  installedTimbres,
  installedVoices,
  installVoiceEngine,
  prepareVoiceForge,
  voiceEars,
  voiceEngines,
  tryVoice,
  voiceForge,
  type InstalledTimbre,
  type VoiceEar,
  type VoiceEngine,
  type VoiceForge,
} from "../ipc/launcher";
import { weigh } from "../lib/weigh";
import { spokenSrc } from "../ipc/world";

/**
 * The program on this machine that turns text into sound (Phase 15).
 *
 * ## Why it is a third list and not a row on either of the other two
 *
 * Connections answers *who does the thinking*, and everything there becomes a brain a character
 * can be assigned to. `ImageStudios` answers *what can draw*. A voice engine is neither, and
 * putting it on either list would put it in a dropdown it has no business being in — the same
 * argument that already keeps a diffusion server out of the Brain dropdown.
 *
 * ## Two facts, not three
 *
 * The other two decks draw **installed · serving · neither**. Piper has no server and no port:
 * it is an executable handed a sentence, which writes a `.wav` and exits. So there is no
 * `SERVING` lamp here, because a lamp that can never move is worse than no lamp — and the row
 * says *why* rather than leaving somebody looking for the one it is missing.
 *
 * ## And this is the one Epoch fetches itself
 *
 * Every other program on these decks has a package manager command Epoch prints and runs in the
 * user's own terminal. Piper has none — measured on 2026-09-04: `winget search piper` answers
 * `npiperelay` and `PhotoPiper`, and Homebrew answers 404 as formula and as cask. So the row
 * either downloads the release archive or it is a link and an instruction, which is a row that
 * does nothing.
 *
 * What it downloads is said before it is pressed, with its size and its licence, because a
 * 22.5 MB fetch under GPL-3 is not something somebody should discover afterwards.
 */
export function VoiceEngines() {
  const [engines, setEngines] = useState<readonly VoiceEngine[] | null>(null);
  const [forge, setForge] = useState<VoiceForge | null>(null);
  const [timbres, setTimbres] = useState<readonly InstalledTimbre[]>([]);
  const [forging, setForging] = useState<string | null>(null);
  const [trying, setTrying] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  /*
    **True before the first read, not after it.** `false` here meant one frame in which nothing
    had been asked and the deck said so as though the answer had come back empty -- the same
    inversion as reading silence as *no*, lasting a paint.
  */
  const [asking, setAsking] = useState(true);
  const [fetching, setFetching] = useState<string | null>(null);

  /** How far the download has got. `total: null` is **no total**, never zero. */
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
    // The forge reports **steps**, not bytes: `pip` cannot be measured, and a bar that guessed
    // would be an instrument nobody can explain. A sentence naming the step is the honest shape.
    const stopSteps = listen<{ id: string; step: string }>(
      "workshop:fetching",
      (event) => {
        if (event.payload.id === "rvc" && event.payload.step) {
          setForging(event.payload.step);
        }
      },
    );
    return () => {
      void stop.then((off) => off());
      void stopSteps.then((off) => off());
    };
  }, []);

  const [ears, setEars] = useState<readonly VoiceEar[] | null>(null);

  /**
   * Say one line through one timbre, and report what actually happened.
   *
   * **The refusal is the point.** `speak_as` falls back to the plain voice when a timbre cannot
   * be applied, which is right in a conversation and would be a lie on a deck that had just
   * printed READY — so `but` is read here and said out loud, and a run with nothing to report
   * says how long it took.
   */
  const tryTimbre = async (name: string): Promise<string> => {
    const voices = await installedVoices();
    const voice = voices[0];
    if (!voice) return "There is no voice installed to colour.";
    const said = await tryVoice(
      voice.name,
      "Hola. Así sueno con esta voz.",
      { voice: name, semitones: 12, speaker: 0 },
    );
    if (typeof said === "string") return said;
    if (said.but) return said.but;
    const sound = spokenSrc(said.sound);
    await new Audio(sound).play().catch(() => {});
    return `${name} spoke in ${(said.millis / 1000).toFixed(1)}s.`;
  };

  const read = () => {
    setAsking(true);
    void Promise.all([voiceEngines(), voiceEars(), voiceForge(), installedTimbres()]).then(
      ([mouths, listening, rvc, converted]) => {
        setEngines(mouths);
        setEars(listening);
        setForge(rvc);
        setTimbres(converted);
        setAsking(false);
      },
    );
  };

  useEffect(read, []);

  const fetchEar = async (what: string, label: string) => {
    setSaid(null);
    setFetching(what);
    setArrived(null);
    const outcome = await installEar(what);
    setFetching(null);
    setArrived(null);
    setSaid(outcome.failed ? `${label}: ${outcome.said}` : outcome.said);
    read();
  };

  const install = async (engine: VoiceEngine) => {
    setSaid(null);
    setFetching(engine.id);
    setArrived(null);
    const outcome = await installVoiceEngine(engine.id);
    setFetching(null);
    setArrived(null);
    setSaid(outcome.said);
    read();
  };

  return (
    <div className="rm">
      <div className="rdy__head">
        <span className="rm__label">Speaking</span>
        <button
          type="button"
          className="btn btn--mini"
          disabled={asking}
          onClick={read}
        >
          {asking ? "ASKING…" : "ASK AGAIN"}
        </button>
      </div>

      {engines === null ? (
        // Unasked is not empty. Walking a folder and the PATH takes a moment, and drawing
        // nothing meanwhile would read as *there is nothing*.
        <p className="cc__hint">Asking this machine…</p>
      ) : (
        engines.map((engine) => (
          <div key={engine.id} className="lrt">
            <p className="verdict">
              <b className={`runs runs--${engine.installed ? "yes" : "no"}`}>
                {engine.installed ? "INSTALLED" : "NOT HERE"}
              </b>{" "}
              {engine.name}
            </p>

            {engine.installed ? (
              <p className="cc__hint">
                {engine.at}
                {/*
                  Whose install answered. It matters because Epoch may delete what it fetched
                  and may not delete what somebody put there themselves.
                */}
                {engine.ours
                  ? " — fetched by Epoch"
                  : " — your own install, which Epoch will not touch"}
              </p>
            ) : (
              <p className="cc__hint">
                No voice engine here, so nobody can speak yet. Every Piper voice on the shelf
                needs this to be heard.
              </p>
            )}

            {/*
              **No SERVING lamp, and the row says why.** Somebody who reads the two decks beside
              this one will look for the third state; leaving it unexplained is how a missing
              instrument reads as a broken one.
            */}
            <p className="cc__hint">
              Nothing runs in the background: Piper is handed one sentence and writes a sound,
              so there is no server to start or stop here.
            </p>

            {!engine.installed && engine.archive !== null && (
              <>
                <button
                  type="button"
                  className="btn btn--mini"
                  disabled={fetching !== null}
                  onClick={() => void install(engine)}
                >
                  {fetching === engine.id
                    ? "FETCHING…"
                    : `INSTALL ${engine.name.toUpperCase()}`}
                </button>
                {fetching === engine.id && arrived?.id === engine.id && (
                  <p className="cc__hint">
                    {arrived.total === null
                      ? `${weigh(arrived.done)} so far`
                      : `${weigh(arrived.done)} of ${weigh(arrived.total)}`}
                  </p>
                )}
                <p className="cc__hint">
                  {engine.installing} About {weigh(engine.archive.bytes)}, under{" "}
                  {engine.licence}.
                </p>
              </>
            )}

            {/*
              Nothing measured for this platform. The row keeps its frame and names what is
              missing rather than offering a download that would fail.
            */}
            {!engine.installed && engine.archive === null && (
              <p className="cc__hint">
                Epoch has no measured download of {engine.name} for this platform. Installing it
                by hand and putting it on the PATH works — this panel looks there too.
              </p>
            )}
          </div>
        ))
      )}

      {/*
        **The ear, under the same heading and as its own rows** (Phase 15, step 5).

        Beside the mouth because they are one subject to a person — *can this machine talk with
        me* — and separate rows because they are two programs with two states, and a machine can
        have either without the other.

        The model is offered on the same row rather than as a second step. A whisper with no
        model hears nothing, exactly as a mouth with no voice says nothing, and two rows to press
        in the right order is a shape that teaches somebody the order by failing at them.
      */}
      {ears?.map((ear) => (
        <div key={ear.id} className="lrt">
          <p className="verdict">
            <b className={`runs runs--${ear.installed ? "yes" : "no"}`}>
              {ear.installed ? "INSTALLED" : "NOT HERE"}
            </b>{" "}
            {ear.name} — listening
          </p>

          {ear.installed ? (
            <p className="cc__hint">
              {ear.at}
              {ear.ours
                ? " — fetched by Epoch"
                : " — your own install, which Epoch will not touch"}
            </p>
          ) : (
            <p className="cc__hint">
              Nothing here can listen yet, so the microphone in a conversation has nowhere to
              send what you say.
            </p>
          )}

          <p className="cc__hint">
            Everything runs on the CPU: the graphics card is never asked for, so listening never
            competes with a character thinking or a picture drawing.
          </p>

          {!ear.installed && ear.archive !== null && (
            <>
              <button
                type="button"
                className="btn btn--mini"
                disabled={fetching !== null}
                onClick={() => void fetchEar(ear.id, ear.name.toUpperCase())}
              >
                {fetching === ear.id ? "FETCHING…" : `INSTALL ${ear.name.toUpperCase()}`}
              </button>
              <p className="cc__hint">
                {ear.installing} About {weigh(ear.archive.bytes)}, under {ear.licence}.
              </p>
            </>
          )}

          {!ear.installed && ear.archive === null && (
            <p className="cc__hint">
              Epoch has no measured download of {ear.name} for this platform yet.
            </p>
          )}

          {/*
            Two models, and they are a real choice rather than a quality ladder — measured, the
            larger one costs three times as much and was only better once the crew's names were
            passed along. Each says what it costs so the choice can be made rather than guessed.
          */}
          {ear.models.map((model) => (
            <p key={model.id} className="cc__hint">
              <b>{model.id}</b> · {weigh(model.bytes)} · {model.about}{" "}
              {model.here ? (
                "— installed."
              ) : (
                <button
                  type="button"
                  className="btn btn--mini"
                  disabled={fetching !== null}
                  onClick={() => void fetchEar(model.id, model.id.toUpperCase())}
                >
                  {fetching === model.id ? "FETCHING…" : "GET IT"}
                </button>
              )}
            </p>
          ))}

          {fetching !== null && arrived?.id === fetching && (
            <p className="cc__hint">
              {arrived.total === null
                ? `${weigh(arrived.done)} so far`
                : `${weigh(arrived.done)} of ${weigh(arrived.total)}`}
            </p>
          )}
        </div>
      ))}

      {/*
        **The RVC forge, as its own row** (Phase 15, the voice changer).

        Under the same heading because it is the same subject -- how a character sounds -- and
        separate because it is a third program with a third state: a machine can speak perfectly
        with Piper and have none of this, which is the ordinary case and not a fault.

        One next step, never a list of four unticked boxes. The order is fixed, so the row says
        the one thing that is actually in the way and what it costs before it is pressed.
      */}
      <div className="lrt">
        <p className="verdict">
          <b className={`runs runs--${forge?.canSpeak ? "yes" : "no"}`}>
            {forge === null ? "—" : forge.canSpeak ? "READY" : "NOT HERE"}
          </b>{" "}
          RVC — sounding like somebody else
        </p>
        {forge === null ? (
          /*
            **Two different silences.** While the deck is still asking, the sentence at the top
            covers it and repeating it here says the same thing twice; afterwards, `null` means
            the machine could not be asked -- which is not the same as a forge that is cold, and
            must not read like one.
          */
          asking ? null : (
            <p className="cc__hint">
              This machine could not be asked about RVC. Nothing is claimed either way.
            </p>
          )
        ) : forge.canSpeak ? (
          <>
            <p className="cc__hint">
              {timbres.length === 0
                ? "Nothing converted yet. Put an RVC `.pth` on the TIMBRES shelf and it can be converted here."
                : `${timbres.length} voice${timbres.length === 1 ? "" : "s"} converted: ${timbres
                    .map((it) => it.name)
                    .join(", ")}.`}
            </p>
            {/*
              **The deck's own check, and it ends at audio.** The row above says READY from four
              booleans; this is the only thing that says a converted voice actually speaks — and
              it is where a refusal has somewhere to be read, which is what makes `but` a
              reading rather than a field nothing shows.

              A timbre colours a voice, so it needs one under it. With no Piper voice installed
              there is nothing to try and the button is absent rather than dead.
            */}
            {timbres[0] && engines?.some((it) => it.installed) ? (
              <button
                type="button"
                className="btn btn--mini"
                disabled={trying}
                onClick={() => {
                  setTrying(true);
                  void tryTimbre(timbres[0]!.name)
                    .then(setSaid)
                    .finally(() => setTrying(false));
                }}
              >
                {trying ? "SPEAKING…" : `TRY ${timbres[0]!.name.toUpperCase()}`}
              </button>
            ) : null}
          </>
        ) : (
          <>
            <p className="cc__hint">{forge.nextStep}</p>
            {forge.python ? (
              <button
                type="button"
                className="btn btn--mini"
                disabled={forging !== null}
                onClick={() => {
                  setForging("Starting…");
                  void prepareVoiceForge()
                    .then((done) => {
                      setSaid(done);
                      return Promise.all([
                        voiceForge().then(setForge),
                        installedTimbres().then(setTimbres),
                      ]);
                    })
                    .catch((why: unknown) => setSaid(String(why)))
                    .finally(() => setForging(null));
                }}
              >
                {forging ?? "PREPARE IT"}
              </button>
            ) : (
              /* Shown, never run. Installing a language system-wide is the user's decision and
                 the user's password. */
              <p className="cc__hint">{forge.howToGetPython}</p>
            )}
          </>
        )}
      </div>

      {said !== null && <p className="cc__hint">{said}</p>}
    </div>
  );
}
