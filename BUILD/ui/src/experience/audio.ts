/**
 * Which microphone Epoch listens through, and which speakers it speaks through.
 *
 * ## Nothing here is knowable until somebody is asked
 *
 * Measured in Epoch's own window, 2026-09-04: before any permission,
 * `navigator.mediaDevices.enumerateDevices()` answers with **three devices whose labels and ids
 * are empty strings**. A select built on that renders three blank rows, which reads as *you have
 * no speakers* rather than as *nobody has been asked yet* — the inversion this codebase keeps
 * paying for.
 *
 * After the browser's own prompt is allowed, the same call answers **nine devices, eight
 * labelled**, with real ids. So `asked` is a state of its own, and the panel says which of the
 * two it is instead of drawing an empty list either way.
 *
 * ## Windows publishes each device up to three times
 *
 * `Default - Speakers (Razer…)`, `Communications - Speakers (Razer…)` and `Speakers (Razer…)` are
 * one pair of headphones listed three times. Showing all of them reads as three sets of speakers.
 *
 * The two aliases are identified **exactly** — their `deviceId` is the literal reserved string
 * `default` or `communications`, not a guess from the label — so collapsing them is not hiding a
 * device Epoch failed to understand. `default` is kept and renamed *Follow the system*, because
 * following the system is a real choice and the one most people want; `communications` is
 * dropped, because it is Windows' notion of call audio and Epoch is not a phone.
 *
 * ## Why `localStorage` and not `settings.toml`
 *
 * A device id belongs to **this machine** — it is meaningless on another one, in exactly the way
 * `num_gpu` is (ADR-0026). The interface volume already lives here for the same reason, and a
 * second home for one of the two would be two answers to *where do audio preferences live*.
 */

/** One thing that can play or record. */
export interface Device {
  readonly id: string;
  readonly label: string;
}

export interface Devices {
  /** Whether the browser has been allowed to say what these are. */
  readonly asked: boolean;
  readonly inputs: readonly Device[];
  readonly outputs: readonly Device[];
}

export const NOTHING_ASKED: Devices = { asked: false, inputs: [], outputs: [] };

/** Windows' own aliases. Reserved ids, so this is a comparison and never a guess at a name. */
const ALIASES = new Set(["default", "communications"]);

const FOLLOW_THE_SYSTEM = "Follow the system";

const INPUT_KEY = "epoch.audio.input";
const OUTPUT_KEY = "epoch.audio.output";
const VOICE_VOLUME_KEY = "epoch.voices.volume";

/**
 * Turn what the browser said into what a person should be offered.
 *
 * Exported so it can be tested without a browser: this is the whole of the reasoning, and it is
 * the part that would silently start showing three sets of headphones.
 */
export function offer(all: readonly MediaDeviceInfo[]): Devices {
  // A single unlabelled entry is what an unasked browser answers with. Distinguishing that from
  // *there are none* is the entire point of this function's return type.
  const asked = all.some((device) => device.label !== "");

  const shape = (kind: MediaDeviceKind): Device[] => {
    const mine = all.filter((device) => device.kind === kind);
    const real = mine
      .filter((device) => !ALIASES.has(device.deviceId))
      // **An entry with no name and no id is not a choice.** This test caught the exact defect
      // this file was written to prevent: an unasked browser answers with one `audioinput` whose
      // label and id are both empty, which sailed past the alias filter and became a blank row.
      // Nothing can select it and nobody can read it.
      .filter((device) => device.deviceId !== "" && device.label !== "")
      .map((device) => ({ id: device.deviceId, label: device.label }));
    // `Follow the system` first and always, when the platform offers it — it is what somebody
    // wants until they have a reason not to, and it keeps working when the hardware changes.
    const followsSystem = mine.some((device) => device.deviceId === "default");
    return followsSystem
      ? [{ id: "default", label: FOLLOW_THE_SYSTEM }, ...real]
      : real;
  };

  return { asked, inputs: shape("audioinput"), outputs: shape("audiooutput") };
}

/** Ask the browser. Never prompts by itself — see {@link letEpochListen}. */
export async function look(): Promise<Devices> {
  if (typeof navigator === "undefined" || !navigator.mediaDevices) {
    return NOTHING_ASKED;
  }
  try {
    return offer(await navigator.mediaDevices.enumerateDevices());
  } catch {
    // Unreadable is not empty, and there is nothing useful to say about it here: the panel
    // shows the unasked wording, which is true — nobody got an answer.
    return NOTHING_ASKED;
  }
}

/**
 * Ask for the microphone, which is what makes every label appear.
 *
 * **A button somebody presses, never something Epoch does on its own.** The browser's own prompt
 * appears over the World, and a decision about a microphone belongs to the person whose
 * microphone it is. The stream is stopped immediately: this asks for *permission*, not for audio.
 */
export async function letEpochListen(): Promise<Devices> {
  if (typeof navigator === "undefined" || !navigator.mediaDevices) {
    return NOTHING_ASKED;
  }
  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    stream.getTracks().forEach((track) => track.stop());
  } catch {
    // Refused, or no microphone. Either way the labels stay hidden, and `look` will say so.
  }
  return look();
}

/** Watch for a device being plugged in or pulled out. Returns the way to stop watching. */
export function onDeviceChange(receive: () => void): () => void {
  if (typeof navigator === "undefined" || !navigator.mediaDevices) {
    return () => {};
  }
  navigator.mediaDevices.addEventListener("devicechange", receive);
  return () =>
    navigator.mediaDevices.removeEventListener("devicechange", receive);
}

function remembered(key: string): string {
  try {
    return globalThis.localStorage?.getItem(key) ?? "";
  } catch {
    return "";
  }
}

function remember(key: string, value: string): void {
  try {
    globalThis.localStorage?.setItem(key, value);
  } catch {
    // A machine that will not store a preference still plays sound. Nothing here is worth
    // failing a click over.
  }
}

export const chosenInput = () => remembered(INPUT_KEY);
export const chooseInput = (id: string) => remember(INPUT_KEY, id);
export const chosenOutput = () => remembered(OUTPUT_KEY);
export const chooseOutput = (id: string) => remember(OUTPUT_KEY, id);

/**
 * How loud the crew is, separately from how loud the interface is.
 *
 * **Two questions, so two controls** — the owner's own correction when this phase was designed.
 * `SOUND` is hover, clicks and windows opening; this is people talking. One control deciding
 * both would mean muting your own clicks to silence Mage, and the first person to mute a
 * character at 3am mutes them forever without remembering why.
 *
 * The default is 1 and **`null` is not zero**: `Number(null)` is `0` and `Number.isFinite(0)` is
 * `true`, which is exactly how Epoch shipped a day of complete silence.
 */
/**
 * Whether anybody speaks at all, right now.
 *
 * **A different question from the volume, and a different one again from `SOUND`.** The owner's
 * own correction when this phase was designed:
 *
 * | question | where it lives |
 * |---|---|
 * | how does this person sound? | the Character |
 * | how loud is the crew? | Settings |
 * | do I want to hear anyone *right now*? | here, in the HUD |
 *
 * It is in the World rather than in Settings because it is a decision about **this moment** —
 * somebody else walks into the room, a call starts — and a decision about the moment must be
 * reachable in the moment. Sending somebody to a settings screen to shut a character up is how
 * they end up muting everything and never turning it back on.
 *
 * Separate from `SOUND` for the reason that control is separate: `SOUND` is the interface's own
 * voice, and one switch deciding both would mean silencing your own clicks to silence Mage.
 */
const VOICES_ON_KEY = "epoch.voices.on";

export function voicesOn(): boolean {
  // **Absent is on.** A crew that arrives silent on a fresh machine looks broken, and the
  // person who has never touched this switch has not asked for silence.
  return remembered(VOICES_ON_KEY) !== "off";
}

export function setVoicesOn(on: boolean): void {
  remember(VOICES_ON_KEY, on ? "on" : "off");
}

export function voiceVolume(): number {
  const said = remembered(VOICE_VOLUME_KEY);
  if (said === "") return 1;
  const level = Number(said);
  return Number.isFinite(level) ? Math.max(0, Math.min(1, level)) : 1;
}

export function setVoiceVolume(level: number): void {
  remember(VOICE_VOLUME_KEY, String(Math.max(0, Math.min(1, level))));
}
