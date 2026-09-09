/**
 * Hands free: the World listens, and you talk to it (Phase 15, step 6).
 *
 * ## What this composes rather than builds
 *
 * The roadmap named this step's real cost as **cancelling a turn in flight**, since the World
 * runs one turn at a time by design. Re-measured before writing a line: `stop_turn` already
 * exists and the Chronicle already draws a halted turn. So the expensive half was built, and
 * what was actually missing is the small half — **knowing when somebody has finished a
 * sentence.**
 *
 * Everything else is already here: `record()` opens the microphone, `listen()` transcribes,
 * `say()` speaks, `stopSpeaking()` stops. This is the piece that decides *when*.
 *
 * ## The noise floor is measured, never declared
 *
 * A fixed threshold is a number that works in the room it was written in. This listens to the
 * room first — half a second of whatever is already there — and takes speech to be a multiple of
 * that. A quiet room gets a sensitive ear and a loud one does not fire on the fan.
 *
 * `AnalyserNode` rather than a model: Silero would be another download, another format and
 * another thing to keep in step, to answer a question that is *is this louder than the room*.
 * If it turns out not to be enough, the measurement that says so is somebody's own recording —
 * and whisper.cpp ships `--vad`, which is where this would go next.
 *
 * ## The World does not listen to itself, and that cost barge-in
 *
 * The first version listened while a character was speaking, on the reasoning that hearing you
 * start is what stops them mid-sentence. **Measured by using it**: the microphone heard the
 * speakers, Mage's own answer came back in as a new question, was answered, and came back
 * again — the owner watched one paragraph repeat forever.
 *
 * So detection holds off while a character is talking. Echo cancellation is requested and is the
 * right long-term answer, but how well it works is a property of somebody's speakers and this
 * cannot measure it — and **a loop that ships is worse than a feature that waits.** What is
 * given up is written down rather than hidden: with echo cancellation measured in a real room,
 * the hold becomes a raised threshold and interrupting mid-sentence works again.
 *
 * Interrupting still works today. It is the button, which stops them at once.
 */

import { chosenInput } from "./audio";
import { wavFromSamples } from "./hearing";
import { isSpeaking } from "./speak";

/** How long the room is listened to before deciding what counts as speech. */
const ROOM_MS = 500;

/**
 * How much louder than the room a sound has to be to count as somebody talking.
 *
 * A ratio rather than a level, because the level is the thing that differs between machines.
 * Deliberately generous: a missed word costs a repeat, and a fan taken for speech costs a turn
 * the user never asked for — which is the more expensive mistake by far.
 */
const OVER_THE_ROOM = 3.5;

/** Silence this long ends a turn. Long enough to think mid-sentence, short enough to feel live. */
const ENDS_AFTER_MS = 900;

/** Shorter than this is a cough, a chair or a door. Never sent. */
const TOO_SHORT_MS = 350;

/**
 * The longest one utterance may run before it is cut and sent.
 *
 * **An unbounded ring is a real hazard, not a hypothetical one.** Speech ends when the level
 * drops below the room's, and a room that gets louder after it was measured — a fan starting, a
 * conversation next door — never drops. Without this the buffer grows for as long as that lasts,
 * and what finally reaches whisper is minutes of audio that nobody said.
 *
 * Thirty seconds because that is whisper's own window: past it the cost stops being free, and
 * nobody speaks one sentence for longer.
 */
const AT_MOST_MS = 30_000;

/**
 * How much of what came *before* somebody was detected is kept.
 *
 * Detection needs a syllable to fire on, so the syllable it fired on is already in the past. Half
 * a second is generous — the measurement that set it is an owner saying *"Hola mage, cómo
 * estás"* and reading back **"Palomage."** — and generous is the safe direction: extra silence
 * in front of a sentence costs whisper nothing, and a missing first word costs the sentence.
 */
const PRE_ROLL_MS = 500;

/** What a hands-free session reports while it runs. */
export interface Conversation {
  /** Stop listening and release the microphone. */
  stop(): void;
}

export interface Listener {
  /**
   * Somebody started talking.
   *
   * A turn in flight is halted here — the model is answering something the user has just
   * changed their mind about, and that is still true when the *voice* has not started yet.
   */
  onStart(): void;
  /** A turn ended. The WAV is base64, exactly what `listen()` takes. */
  onEnd(wav: string): void;
  /** The room, as measured, so a surface can say what it is listening against. */
  onRoom(level: number): void;
  /**
   * Whether the ear is holding off because a character is talking.
   *
   * Said out loud rather than left as a silent pause: a control whose only feedback is a label
   * must not have a state in which the label does not move — and *it is not listening because
   * somebody else is talking* is information, not a gap.
   */
  onTheirTurn(holding: boolean): void;
}

/**
 * Listen until told to stop, and hand over each finished sentence.
 *
 * Throws when the microphone cannot be opened — the caller says so rather than this pretending
 * it is listening to a room it never reached.
 */
export async function converse(to: Listener): Promise<Conversation> {
  const device = chosenInput();
  const stream = await navigator.mediaDevices.getUserMedia({
    audio: {
      /*
        **Asked for out loud, never left to a default.** A room with speakers in it is a room
        where the microphone hears the speakers, and the first hands-free session proved it:
        Mage's answer came back in as a new question, was answered, and came back again — the
        same paragraph forever.

        These three are the browser's own filters and they cost nothing to request. They are
        also not enough on their own, which is why `isSpeaking()` gates the detector below:
        how well echo cancellation works is a property of somebody's hardware, and this cannot
        measure it.
      */
      echoCancellation: true,
      noiseSuppression: true,
      autoGainControl: true,
      ...(device && device !== "default" ? { deviceId: { exact: device } } : {}),
    },
  });

  const context = new AudioContext();
  const source = context.createMediaStreamSource(stream);
  const analyser = context.createAnalyser();
  analyser.fftSize = 1024;
  source.connect(analyser);
  const samples = new Float32Array(analyser.fftSize);

  /*
    **The audio is captured, not recorded, and the reason is the beginning of the sentence.**

    The first version started a `MediaRecorder` the moment speech was detected, so everything
    before that moment was gone — and detection needs a syllable to fire on. The owner said
    *"Hola mage, cómo estás"* and read back **"Palomage."**: the front of the sentence had never
    been recorded at all, and whisper made what it could of the rest.

    A rolling window of `MediaRecorder` chunks does not fix it. Its chunks are not independently
    decodable — only the first carries the header — so a window that drops old ones decodes to
    nothing. Raw samples have no such shape, so this keeps its own ring and hands over the half
    second *before* somebody started.

    `ScriptProcessorNode` is deprecated and is what this uses: an `AudioWorklet` needs a module
    fetched over a URL, which is a build-time arrangement and a CSP question, to do the one thing
    this needs — see every sample. It is measured working in this WebView2, and the day it stops
    the replacement is an `AudioWorklet` with the same ring inside it.
  */
  const capture = context.createScriptProcessor(4096, 1, 1);
  const ring: Float32Array[] = [];
  let ringSamples = 0;
  const keepAtMost = Math.round((PRE_ROLL_MS / 1000) * context.sampleRate);
  let keeping: Float32Array[] | null = null;

  capture.onaudioprocess = (event) => {
    const block = new Float32Array(event.inputBuffer.getChannelData(0));
    if (keeping !== null) {
      keeping.push(block);
      return;
    }
    ring.push(block);
    ringSamples += block.length;
    while (ringSamples - (ring[0]?.length ?? 0) >= keepAtMost && ring.length > 1) {
      ringSamples -= ring.shift()!.length;
    }
  };
  source.connect(capture);
  /*
    **Connected to silence, on purpose.** A `ScriptProcessorNode` does not run unless it reaches
    the destination, and reaching it at full gain plays the microphone back through the speakers
    — which is the feedback this whole file is about, arriving by a different door.
  */
  const mute = context.createGain();
  mute.gain.value = 0;
  capture.connect(mute);
  mute.connect(context.destination);

  let room = 0;
  let heard = 0;
  let talking = false;
  let quietSince = 0;
  let startedAt = 0;
  let stopped = false;

  const loudness = (): number => {
    analyser.getFloatTimeDomainData(samples);
    let sum = 0;
    for (const sample of samples) sum += sample * sample;
    return Math.sqrt(sum / samples.length);
  };

  /*
    **One timer, not two.** A separate loop for the room and another for speech would be two
    clocks that drift apart, and the second would be reading a floor the first had not finished
    measuring yet. The room is measured by the same tick that later listens against it.
  */
  const tick = window.setInterval(() => {
    if (stopped) return;
    const now = performance.now();
    const level = loudness();

    if (heard < ROOM_MS) {
      // Still listening to the room. The loudest moment wins rather than the average: a floor
      // taken from the quiet gaps would call the fan speech.
      room = Math.max(room, level);
      heard += TICK_MS;
      if (heard >= ROOM_MS) to.onRoom(room);
      return;
    }

    /*
      **The World does not listen to itself.**

      While a character is talking, nothing the microphone hears is treated as speech. That is a
      real cost — it is exactly the moment barge-in is for — and it is paid deliberately, because
      the alternative shipped an infinite loop: what came back was not somebody interrupting, it
      was the answer being read to the room and heard again.

      What is given up and what would give it back is written down rather than hidden: with echo
      cancellation measured against real speakers in a real room, this gate becomes a raised
      threshold instead of a stop, and interrupting mid-sentence works. That measurement needs a
      voice and a room, so it is the owner's and not this file's.

      Interrupting still works — it is the button, which stops them at once — and the moment they
      finish, the ear is open again.
    */
    if (isSpeaking()) {
      if (talking) {
        // Somebody was mid-sentence when a character started answering. Their words are kept:
        // dropping them would lose a question because somebody else began talking over it.
        talking = false;
        quietSince = 0;
        void finish(now - startedAt >= TOO_SHORT_MS);
      }
      to.onTheirTurn(true);
      return;
    }
    to.onTheirTurn(false);

    const speech = level > room * OVER_THE_ROOM;

    if (speech && !talking) {
      talking = true;
      startedAt = now;
      // Everything already in the ring comes with it: that is the half second in which the
      // sentence actually began.
      keeping = [...ring];
      ring.length = 0;
      ringSamples = 0;
      to.onStart();
      return;
    }

    if (!talking) return;

    if (speech) {
      quietSince = 0;
      // **Cut, rather than kept.** A room that got louder than the floor it was measured against
      // never goes quiet again, and the alternative to cutting is a buffer that grows until
      // somebody notices — which is what a stuck `LISTENING` looks like from outside.
      if (now - startedAt >= AT_MOST_MS) {
        talking = false;
        void finish(true);
      }
      return;
    }
    if (quietSince === 0) quietSince = now;
    if (now - quietSince < ENDS_AFTER_MS) return;

    // A turn ended.
    talking = false;
    const spoke = now - startedAt - ENDS_AFTER_MS;
    quietSince = 0;
    const wanted = spoke >= TOO_SHORT_MS;
    void finish(wanted);
  }, TICK_MS);

  const finish = async (wanted: boolean) => {
    const held = keeping;
    keeping = null;
    if (!wanted || held === null || held.length === 0) return;

    let total = 0;
    for (const block of held) total += block.length;
    const whole = new Float32Array(total);
    let at = 0;
    for (const block of held) {
      whole.set(block, at);
      at += block.length;
    }

    const wav = await wavFromSamples(whole, context.sampleRate);
    if (wav !== null && !stopped) to.onEnd(wav);
  };

  return {
    stop() {
      stopped = true;
      window.clearInterval(tick);
      keeping = null;
      capture.onaudioprocess = null;
      capture.disconnect();
      void context.close();
      stream.getTracks().forEach((track) => track.stop());
    },
  };
}

/** How often the room is looked at. Fine enough to catch a first syllable, cheap enough to run. */
const TICK_MS = 50;
