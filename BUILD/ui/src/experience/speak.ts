/**
 * The crew, out loud (Phase 15, step 3).
 *
 * ## Four rules, and they are the whole design
 *
 * **It speaks the answer, not the turn.** `kind === "answered"` and nothing else. A tool result
 * is not speech and a `run_command` call is certainly not — the Chronicle also holds what was
 * approved and what was produced (ADR-0025), and reading a file listing aloud is not somebody
 * talking to you.
 *
 * **Speech failing must never cost an answer.** With no engine installed, no voice chosen or a
 * refusal from the shelf, the turn still arrives as text exactly as it does today. The same
 * discipline as the 3D job's preview: *a job's optional last step must not be able to take the
 * job down* — the thing that was actually asked for had already succeeded when it failed.
 *
 * **One voice at a time.** Phase 12½ made two characters answer at once, and two voices over
 * each other is not a conversation. A queue, and the single slot is a shape the Engine already
 * has.
 *
 * **Nothing is ever spoken twice, and what already happened is not spoken at all.** Every line
 * is remembered by what it is, so a re-render, a re-read of the Chronicle or a remount cannot
 * make Mage repeat himself — and a conversation you walk into does not read its backlog aloud.
 *
 * ## Why the whole answer and not sentence by sentence
 *
 * The roadmap asks for per-sentence playback as tokens arrive, and the reason is real: a model
 * at 40 tok/s otherwise leaves you waiting for half an answer before the first word.
 *
 * Measured here first, which changes how urgent that is: Piper speaks **44.7 seconds of Spanish
 * in 2.1 s** on this machine — about 21x faster than real time. So synthesising a finished
 * answer costs well under a second for anything of ordinary length, and the wait is the
 * *model's*, not the voice's. Streaming remains worth building and is no longer what stands
 * between the crew and being heard.
 */

import { tryVoice } from "../ipc/launcher";
import type { Timbre } from "../ipc/contracts";
import { spokenSrc } from "../ipc/world";
import { chosenOutput, voiceVolume, voicesOn } from "./audio";

/** One thing waiting to be said. */
interface Line {
  readonly voice: string;
  readonly text: string;
  /**
   * Captured when the line was queued, not read when it is spoken.
   *
   * A queue can outlive an edit, and a sentence should be said in the voice the character had
   * when they said it — the same reasoning that aims a model release at what was recorded when
   * the turn ran rather than at whatever the character has now.
   */
  readonly soundsLike: Timbre | null;
}

const waiting: Line[] = [];
let speaking = false;

/**
 * What has already been said, by content.
 *
 * A `Set` of what was spoken rather than a count of entries: the Chronicle is re-read on every
 * turn and a component may remount, and both would replay an answer somebody already heard.
 */
const spoken = new Set<string>();

/** Bounded, so a long session does not grow a set forever. */
const REMEMBER = 400;

/**
 * Everything a voice should not read out loud.
 *
 * A model writes for a screen: fenced code, Markdown emphasis, link syntax, bullets. Handing
 * those to a phonemiser produces *"asterisk asterisk important asterisk asterisk"*, and an
 * emoji produces nothing useful at all. **What a voice is given should be what a person hears** —
 * the same reasoning that keeps Markdown out of a spoken line, arriving one layer up.
 */
export function readable(text: string): string {
  return (
    text
      // Fenced code, then inline code. Read aloud, a block of Rust is noise.
      .replace(/```[\s\S]*?```/g, " ")
      .replace(/`([^`]*)`/g, "$1")
      // A link says its words, never its address.
      .replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1")
      .replace(/^\s{0,3}#{1,6}\s+/gm, "")
      .replace(/^\s{0,3}[-*+]\s+/gm, "")
      .replace(/\*\*([^*]+)\*\*/g, "$1")
      .replace(/[*_~>|]/g, " ")
      // Anything that is not a word in a language a phonemiser knows.
      .replace(/[\p{Extended_Pictographic}\p{Emoji_Presentation}]/gu, " ")
      .replace(/\s+/g, " ")
      .trim()
  );
}

/**
 * What `say` and `alreadyHeard` both key on, so the two cannot disagree.
 *
 * Its own function for a reason that is not tidiness: this key was written twice for one commit
 * and one of the copies carried a **NUL byte** where a space belonged — typed into a template
 * literal, compiled without complaint, passed every test, and turned the file binary to git and
 * to every grep. Nothing asserts on the bytes of a string that is only ever compared with
 * itself, so consistency is the only thing that saves it. One function is that consistency.
 */
function heardAs(voice: string, words: string): string {
  return voice + " :: " + words;
}

/** Playing is testable only if the player can be replaced. */
export interface Player {
  play(sound: string, volume: number, output: string): Promise<void>;
  /**
   * Stop whatever is sounding right now.
   *
   * **Added because there was no way to.** `stopSpeaking` emptied the queue and left the element
   * playing, so turning NPC VOICES off mid-sentence did nothing you could hear — and the
   * barge-in this file claims to do never actually cut anybody off. A queue is not a voice.
   */
  stop?(): void;
}

/**
 * The real one: an `<audio>` element, on the chosen output.
 *
 * **Handed an address the platform actually serves.** This was a `data:` URI first and made no
 * sound at all: `media-src` here is `epoch: http://epoch.localhost`, and a blocked source raises
 * the same `NotSupportedError` a broken file does. Then it was `epoch://spoken-<name>`, which is
 * also refused — on Windows the scheme is served under `http://epoch.localhost`, which is what
 * `convertFileSrc` knows.
 *
 * The lesson is the one already in `CLAUDE.md`, arriving from a third direction: **check the
 * governor the thing actually answers to**, and verify the side effect rather than the call. A
 * spy on `new Audio(...)` proves an object was created; `currentTime` moving is the only thing
 * that says anybody heard it.
 *
 * `setSinkId` is measured as present in this WebView2. It is still guarded — a browser without
 * it plays on the system default, which is right, and refusing to speak because a device could
 * not be chosen would be losing the answer over the preference.
 */
let sounding: HTMLAudioElement | null = null;

const speaker: Player = {
  stop() {
    // Paused rather than dropped: the element is what is making the noise, and letting go of a
    // reference does not stop a sound.
    sounding?.pause();
    sounding = null;
  },
  async play(sound, volume, output) {
    const audio = new Audio(sound);
    sounding = audio;
    audio.volume = volume;
    const withSink = audio as HTMLAudioElement & {
      setSinkId?: (id: string) => Promise<void>;
    };
    if (output && output !== "default" && typeof withSink.setSinkId === "function") {
      try {
        await withSink.setSinkId(output);
      } catch {
        // The device went away, or this build will not switch. Play anyway.
      }
    }
    await new Promise<void>((done) => {
      audio.onended = () => done();
      audio.onerror = () => done();
      // `pause()` fires neither, so `hush` has to release the wait itself or `drain` would sit
      // on a line nobody is listening to.
      audio.onpause = () => done();
      /*
        **`play()` does not always return a promise.** It does in every browser Epoch ships on
        and it does not in jsdom, which is where this threw — `Cannot read properties of
        undefined (reading 'catch')`, unhandled, from inside the function that plays a
        character's voice. Wrapping it costs nothing and the alternative is an unhandled
        rejection in the speaking path, which is exactly the shape that left `HEARING…` stuck
        with nothing on screen.
      */
      void Promise.resolve(audio.play()).catch(() => done());
    });
    if (sounding === audio) sounding = null;
  },
};

let player: Player = speaker;

/** For tests. Returns the way to put the real one back. */
export function playThrough(replacement: Player): () => void {
  player = replacement;
  return () => {
    player = speaker;
  };
}

function remember(heard: string): void {
  spoken.add(heard);
  if (spoken.size > REMEMBER) {
    spoken.delete(spoken.values().next().value as string);
  }
}

async function drain(): Promise<void> {
  if (speaking) return;
  speaking = true;
  try {
    while (waiting.length > 0) {
      const line = waiting.shift()!;
      const sound = await tryVoice(line.voice, line.text, line.soundsLike);
      // A string is the refusal. Nothing is said about it here: the answer is already on
      // screen, and a toast about a voice would be Epoch complaining about its own optional
      // last step.
      if (typeof sound === "string") continue;
      await player.play(spokenSrc(sound.sound), voiceVolume(), chosenOutput());
    }
  } finally {
    speaking = false;
  }
}

/**
 * Remember an answer as already heard, without saying it.
 *
 * **What is on screen when you walk in has already happened.** Measured in the window: opening
 * a conversation with a long Chronicle read the entire backlog aloud — seven answers, one after
 * another, from a transcript nobody had asked to hear again.
 *
 * Expressed as *marking* rather than as an index into the list, because a Chronicle is re-read
 * on every turn and a count is only right while nothing is ever inserted, compacted or removed.
 */
export function alreadyHeard(voice: string | null | undefined, text: string): void {
  if (!voice) return;
  const words = readable(text);
  if (words === "") return;
  remember(heardAs(voice, words));
}

/**
 * Say one answer, if there is a voice and something worth reading.
 *
 * Silent by construction in every case that should be silent: no voice chosen, nothing left
 * after the Markdown is taken out, or these exact words already said.
 */
export function say(
  voice: string | null | undefined,
  text: string,
  soundsLike?: Timbre | null,
): void {
  if (!voice) return;
  /*
    **Checked here, not at the call site.** The switch answers *do I want to hear anyone right
    now*, and the only honest place to ask it is the moment before somebody would speak — a
    caller that read it earlier would be acting on a decision the user has since changed.

    And it returns before remembering: switching voices back on should let the *next* answer be
    heard, not replay the ones that arrived while they were off. Those already happened, which
    is the same rule as walking into a conversation.
  */
  if (!voicesOn()) return;
  const words = readable(text);
  if (words === "") return;

  const heard = heardAs(voice, words);
  if (spoken.has(heard)) return;
  remember(heard);

  waiting.push({ voice, text: words, soundsLike: soundsLike ?? null });
  void drain();
}

/**
 * Stop talking, and **remember what was already said**.
 *
 * ## The loop this closes
 *
 * `stopSpeaking` did both jobs, and hands-free called it on every barge-in. Clearing `spoken`
 * is what made the World repeat itself: the Chronicle is re-read on every render, so with the
 * memory wiped every answer on screen became something that had *not* been said yet, and was
 * said again — and again. The owner watched one paragraph loop forever, and the Chronicle on
 * disk had five entries in it, which is what says the repetition was never in the turns.
 *
 * **Two questions, and one function was answering both.** *Be quiet now* keeps the memory;
 * *this conversation is over* throws it away. Reusing the second for the first is the same
 * shape as a setting whose name names one thing and decides two.
 */
export function hush(): void {
  waiting.length = 0;
  player.stop?.();
}

/**
 * Forget what has been said and drop anything queued.
 *
 * Called when a conversation is left: the queue holds *this* conversation's lines, and a
 * character finishing a sentence into a closed window is the World talking to nobody.
 */
/**
 * Whether the World is talking right now — either playing a line or with one queued.
 *
 * ## Why anything needs to ask
 *
 * Hands free opened the microphone while a character was speaking through the speakers, and the
 * World heard **itself**: Mage's answer came back in as a new question, was answered, and came
 * back again. The owner watched the same paragraph repeat forever.
 *
 * A room with speakers in it is a room where the microphone hears the speakers. Echo
 * cancellation is asked for now and is the right long-term answer, but its quality is a
 * property of somebody's hardware and cannot be measured from here — so the loop is closed by
 * construction rather than by trusting a filter nobody has measured.
 */
export function isSpeaking(): boolean {
  return speaking || waiting.length > 0;
}

export function stopSpeaking(): void {
  hush();
  spoken.clear();
}
