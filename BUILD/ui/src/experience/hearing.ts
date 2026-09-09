/**
 * The microphone (Phase 15, step 5).
 *
 * ## Why the page makes the WAV
 *
 * `MediaRecorder` produces WebM/Opus at whatever the device runs at; whisper wants **16 kHz mono
 * PCM**. Something has to convert, and the choice is the page or the Engine.
 *
 * The Engine would need `ffmpeg` — a program Epoch does not ship, has no package to ask for on
 * two platforms, and would have to install to hear one sentence. The page already has the two
 * pieces: `AudioContext.decodeAudioData` reads whatever was recorded, and `OfflineAudioContext`
 * resamples it. The WAV header after that is forty-four bytes of arithmetic.
 *
 * So this is not the frontend doing work that belongs in the Engine. It is the frontend using
 * what it already has rather than making the Engine acquire a dependency for it.
 *
 * ## What crosses, and what does not
 *
 * Bytes, as base64, exactly as an imported picture does (ADR-0024): the window has no filesystem
 * and never gets one. The recording is written to a temporary file by the Engine, read once, and
 * **thrown away** — a recording is a means, not a record, and keeping it would make evidence of
 * somebody clearing their throat.
 */

import { invoke } from "@tauri-apps/api/core";

import { chosenInput } from "./audio";

/** What whisper wants. Not negotiable, and not a preference. */
const WANTED_RATE = 16000;

export interface Heard {
  readonly text: string;
  /** How long the Engine took. A measurement of this machine, never a promise about another. */
  readonly millis: number;
  /**
   * Which language whisper **decided** it was hearing, when nobody told it.
   *
   * Absent when a language was named: there is no decision to report. Present, it is the reading
   * that turns *"why did it write Greek"* into an answer — the owner met
   * `Αυτοί, πρέπει να τα βλακουμε τα τάτια.` with nothing on screen saying what had happened.
   */
  /**
   * **`null`, not absent.** `Option<String>` without `skip_serializing_if` serialises as `null`,
   * and this said `heardAs?: string` — so the note fired on every transcription and read
   * `Heard as null (0% sure)`, on words that had been heard perfectly. A type that describes a
   * wire it has not been checked against is a type that will be wrong about exactly one value.
   */
  readonly heardAs?: string | null;
  /** How sure it was, 0–1, from whisper's own line. `null` for the same reason. */
  readonly sure?: number | null;
}

/** A recording in progress. */
export interface Recording {
  /** Stop, and hand back the WAV as base64. `null` when nothing usable was captured. */
  stop(): Promise<string | null>;
  /** Give up without transcribing. */
  cancel(): void;
}

/**
 * Start recording from the chosen input.
 *
 * Throws when the microphone is refused or absent — the caller says so rather than this
 * pretending a silent recording happened.
 */
export async function record(): Promise<Recording> {
  const device = chosenInput();
  const stream = await navigator.mediaDevices.getUserMedia({
    // `deviceId` only when somebody chose one: an `exact` constraint for a device that has been
    // unplugged fails the whole request, and *follow the system* is the default for a reason.
    audio: device && device !== "default" ? { deviceId: { exact: device } } : true,
  });

  const recorder = new MediaRecorder(stream);
  const chunks: Blob[] = [];
  recorder.ondataavailable = (event) => {
    if (event.data.size > 0) chunks.push(event.data);
  };
  recorder.start();

  const release = () => stream.getTracks().forEach((track) => track.stop());

  return {
    async stop() {
      const done = new Promise<void>((settled) => {
        recorder.onstop = () => settled();
      });
      recorder.stop();
      await done;
      release();
      const first = chunks[0];
      if (first === undefined) return null;
      return wavFrom(new Blob(chunks, { type: first.type }));
    },
    cancel() {
      try {
        recorder.stop();
      } catch {
        // Already stopped. Releasing the microphone is the part that matters.
      }
      release();
    },
  };
}

/**
 * Decode whatever was recorded, resample it to 16 kHz mono, and write a WAV.
 *
 * **Exported, because there are two callers and they must not diverge.** Hands free had its own
 * copy for a day — same arithmetic, written twice, with the target rate as a literal in one of
 * them. That is the `weigh` defect this codebase has already paid for: two functions with the
 * same doc comment and different behaviour at one value, neither wrong on its own.
 */
export async function wavFrom(blob: Blob): Promise<string | null> {
  const context = new AudioContext();
  let decoded: AudioBuffer;
  try {
    decoded = await context.decodeAudioData(await blob.arrayBuffer());
  } catch {
    return null;
  } finally {
    void context.close();
  }
  return wavFromBuffer(decoded);
}

/**
 * The same conversion, from samples somebody already has.
 *
 * Hands free captures raw audio rather than a recording — it needs the half-second *before*
 * somebody started talking, and a `MediaRecorder` cannot give that: its chunks are not
 * independently decodable, so a rolling window of them has no header and decodes to nothing.
 */
export async function wavFromSamples(
  samples: Float32Array,
  rate: number,
): Promise<string | null> {
  if (samples.length === 0 || rate <= 0) return null;
  const holding = new AudioContext({ sampleRate: rate });
  const buffer = holding.createBuffer(1, samples.length, rate);
  // Copied through the channel's own array rather than `copyToChannel`, whose types insist on a
  // `Float32Array<ArrayBuffer>` and reject the perfectly ordinary one a caller has.
  buffer.getChannelData(0).set(samples);
  void holding.close();
  return wavFromBuffer(buffer);
}

async function wavFromBuffer(decoded: AudioBuffer): Promise<string | null> {
  // **Resampled by the platform's own resampler.** Writing one here would be a filter nobody
  // measured, and `OfflineAudioContext` takes the target rate as a constructor argument.
  const frames = Math.max(1, Math.round((decoded.duration * WANTED_RATE) | 0));
  const offline = new OfflineAudioContext(1, frames, WANTED_RATE);
  const source = offline.createBufferSource();
  source.buffer = decoded;
  source.connect(offline.destination);
  source.start();
  const mono = await offline.startRendering();

  return base64(wavBytes(mono.getChannelData(0), WANTED_RATE));
}

/** A 16-bit PCM WAV. Forty-four bytes of header and the samples. */
function wavBytes(samples: Float32Array, rate: number): Uint8Array {
  const bytes = new Uint8Array(44 + samples.length * 2);
  const view = new DataView(bytes.buffer);
  const text = (at: number, what: string) => {
    for (let i = 0; i < what.length; i += 1) view.setUint8(at + i, what.charCodeAt(i));
  };

  text(0, "RIFF");
  view.setUint32(4, 36 + samples.length * 2, true);
  text(8, "WAVEfmt ");
  view.setUint32(16, 16, true); // PCM header length
  view.setUint16(20, 1, true); // PCM
  view.setUint16(22, 1, true); // mono
  view.setUint32(24, rate, true);
  view.setUint32(28, rate * 2, true); // bytes per second
  view.setUint16(32, 2, true); // bytes per frame
  view.setUint16(34, 16, true); // bits per sample
  text(36, "data");
  view.setUint32(40, samples.length * 2, true);

  for (let i = 0; i < samples.length; i += 1) {
    // Clamped before scaling: a sample outside [-1, 1] wraps rather than clips, and a wrap is
    // a loud click in the middle of somebody's sentence.
    const level = Math.max(-1, Math.min(1, samples[i] ?? 0));
    view.setInt16(44 + i * 2, level < 0 ? level * 0x8000 : level * 0x7fff, true);
  }
  return bytes;
}

function base64(bytes: Uint8Array): string {
  let binary = "";
  // In blocks, because `String.fromCharCode(...bytes)` on a whole recording is an argument list
  // long enough to overflow the stack.
  for (let i = 0; i < bytes.length; i += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(binary);
}

/**
 * Ask the Engine what was said.
 *
 * The crew's names are added by the Engine, which knows who lives in the World — measured, that
 * is the difference between hearing `Mage` and hearing *imagen*.
 */
export async function listen(
  wav: string,
  language?: string,
): Promise<Heard | string> {
  try {
    return await invoke<Heard>("listen", { wav, language });
  } catch (why) {
    return String(why);
  }
}
