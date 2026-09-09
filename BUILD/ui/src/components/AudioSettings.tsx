import { useEffect, useState } from "react";

import { fetchSettings, setHearingLanguage } from "../ipc/launcher";

import {
  chooseInput,
  chooseOutput,
  chosenInput,
  chosenOutput,
  letEpochListen,
  look,
  onDeviceChange,
  setVoiceVolume,
  voiceVolume,
  type Devices,
  NOTHING_ASKED,
} from "../experience/audio";

/**
 * How loud the crew is, and which hardware they arrive through.
 *
 * ## Two volumes, because there are two questions
 *
 * `Interface volume` is hover, clicks and windows opening — Epoch's own voice, synthesised. This
 * is people talking. **One control deciding both** would mean muting your own clicks to silence
 * Mage, and the first person to mute a character at 3am mutes them forever without remembering
 * why. The owner's own correction when this phase was designed, and it is the same rule that
 * split `Run several crew members at once` from what it had quietly also been deciding.
 *
 * ## The dropdowns keep their frame until somebody is asked
 *
 * Measured in Epoch's own window: an unasked browser answers `enumerateDevices()` with three
 * devices whose labels and ids are **empty strings**. So before permission there is nothing to
 * put in a list, and the panel says *that* rather than drawing an empty select — which would
 * read as *you have no speakers*.
 *
 * The button that asks is a button **somebody presses**. The browser's own prompt appears over
 * the World and a decision about a microphone belongs to the person whose microphone it is.
 */
/**
 * The languages offered, and why there are nine rather than ninety-nine.
 *
 * whisper knows 99. A list that long is a list nobody scrolls, and every one past the first few
 * is a row somebody reads and skips. These are the ones with the most Piper voices on the shelf
 * — the only measurement Epoch holds about who is likely to be talking to it — and *Detect each
 * time* is always the first option for everybody else.
 */
const SPOKEN: readonly (readonly [string, string])[] = [
  ["es", "Espanol"],
  ["en", "English"],
  ["pt", "Portugues"],
  ["fr", "Francais"],
  ["de", "Deutsch"],
  ["it", "Italiano"],
  ["ru", "Russkiy"],
  ["zh", "Zhongwen"],
  ["ja", "Nihongo"],
];

export function AudioSettings() {
  const [devices, setDevices] = useState<Devices>(NOTHING_ASKED);
  const [volume, setVolume] = useState(voiceVolume);
  const [input, setInput] = useState(chosenInput);
  const [output, setOutput] = useState(chosenOutput);
  const [asking, setAsking] = useState(false);

  useEffect(() => {
    void look().then(setDevices);
    // A pair of headphones plugged in while this panel is open should appear in it. The browser
    // says so; polling would be the invented reading of a real event.
    return onDeviceChange(() => {
      void look().then(setDevices);
    });
  }, []);

  const ask = async () => {
    setAsking(true);
    setDevices(await letEpochListen());
    setAsking(false);
  };

  const pick = (
    what: "input" | "output",
    id: string,
  ) => {
    if (what === "input") {
      chooseInput(id);
      setInput(id);
    } else {
      chooseOutput(id);
      setOutput(id);
    }
  };

  return (
    <>
      <label className="setting setting--range">
        <span>
          <b>NPC voices</b>
          <i>
            {Math.round(volume * 100)}% - how loud the crew is when they speak.
            Separate from the interface, so silencing your own clicks does not
            silence them.
          </i>
        </span>
        <input
          type="range"
          min="0"
          max="100"
          value={Math.round(volume * 100)}
          aria-label="NPC voices volume"
          onChange={(e) => {
            const level = Number(e.target.value) / 100;
            setVoiceVolume(level);
            setVolume(level);
          }}
        />
      </label>

      <p className="notice" style={{ marginTop: 12 }}>
        <b>Audio devices</b>
      </p>

      {!devices.asked ? (
        <>
          {/*
            The cold instrument, worded as what it is. Windows will not name a microphone or a
            speaker to a page that has never been allowed to listen — so this is *nobody has
            been asked*, and saying *no devices* would be an invented reading.
          */}
          <p className="cc__hint">
            Windows will not name your microphone or your speakers until Epoch has been allowed
            to listen once. Nothing is recorded and nothing leaves this machine — the permission
            is what makes the names appear.
          </p>
          <button
            type="button"
            className="btn btn--mini"
            disabled={asking}
            onClick={() => void ask()}
          >
            {asking ? "ASKING…" : "LET EPOCH LISTEN"}
          </button>
        </>
      ) : (
        <>
          <label className="setting setting--range">
            <span>
              <b>Input</b>
              <i>Which microphone Epoch listens through.</i>
            </span>
            <select
              value={input}
              aria-label="Input device"
              onChange={(e) => pick("input", e.target.value)}
            >
              {devices.inputs.map((device) => (
                <option key={device.id} value={device.id}>
                  {device.label}
                </option>
              ))}
            </select>
          </label>

          <label className="setting setting--range">
            <span>
              <b>Output</b>
              <i>Which speakers the crew comes out of.</i>
            </span>
            <select
              value={output}
              aria-label="Output device"
              onChange={(e) => pick("output", e.target.value)}
            >
              {devices.outputs.map((device) => (
                <option key={device.id} value={device.id}>
                  {device.label}
                </option>
              ))}
            </select>
          </label>

          {/*
            A machine with one microphone and one pair of speakers has nothing to choose between,
            and a select with one entry is a control that cannot do anything. Said rather than
            hidden, so nobody wonders whether the list failed to load.
          */}

          {devices.inputs.length <= 1 && devices.outputs.length <= 1 && (
            <p className="cc__hint">
              This machine has one of each, so there is nothing to choose between yet.
            </p>
          )}
        </>
      )}
    </>
  );
}

/**
 * Which language you speak.
 *
 * ## Why it is not in the audio panel any more (2026-09-06)
 *
 * It was, and it was nested inside that panel's *devices have been asked for* branch — so on a
 * machine where nobody had granted the microphone, the control that decides what whisper is told
 * **did not exist**. The two questions had been placed together because they are both about the
 * microphone, and they are not the same question: one is *which piece of hardware*, the other is
 * *what language is coming out of it*, and only the first needs a permission to answer.
 *
 * Somebody who types rather than talks still has a spoken language the moment they press SPEAK.
 *
 * ## The choices, and why there are nine rather than ninety-nine
 *
 * Detecting is the honest default and it is a guess on a short phrase: *"hazme una tabla con
 * esos datos"* came back as Greek and took three tries. The obvious default was this machine's
 * own locale, and it was measured before it was written — on the owner's machine
 * `navigator.language` answers `en-US` and he speaks Spanish. **A Windows install language is a
 * fact about the installer**, so nothing is assumed and the question is asked.
 *
 * whisper knows 99 languages. A list that long is a list nobody scrolls, and every entry past
 * the first few is a row somebody reads and skips. These are the ones Piper ships voices for on
 * this shelf — the only measurement Epoch holds about who is likely to be talking — and
 * *Detect each time* is always the first option for everybody else.
 */
export function SpokenLanguage() {
  const [speaks, setSpeaks] = useState<string>("");

  useEffect(() => {
    void fetchSettings().then((it) => setSpeaks(it.hearingLanguage ?? ""));
  }, []);

  return (
    <label className="setting setting--wide">
      <span>
        Language you speak
        <i>What the microphone is told. Detecting is a guess on a short phrase.</i>
      </span>
      <select
        value={speaks}
        aria-label="Language you speak"
        onChange={(e) => {
          const said = e.target.value;
          setSpeaks(said);
          void setHearingLanguage(said);
        }}
      >
        <option value="">Detect each time</option>
        {SPOKEN.map(([code, name]) => (
          <option key={code} value={code}>
            {name}
          </option>
        ))}
      </select>
    </label>
  );
}
