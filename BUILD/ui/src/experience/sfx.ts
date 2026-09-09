/**
 * Offline World sounds.
 *
 * There are deliberately no sound files: the small voice bank is synthesised through Web Audio
 * so it ships with the desktop application and does not make a network request on first paint.
 * `unlock()` is the hard gate. `play()` never creates an AudioContext, which makes a programmatic
 * render silent until the user has made a gesture.
 */

import { useSyncExternalStore } from "react";

export type SfxName = "hover" | "click" | "open" | "close" | "select" | "quest" | "error" | "type";

interface Note {
  readonly frequency: number;
  readonly duration: number;
  readonly delay?: number;
  readonly type?: OscillatorType;
  readonly volume?: number;
}

export const SFX_VOICES: Readonly<Record<SfxName, readonly Note[]>> = {
  hover: [{ frequency: 880, duration: 0.05, volume: 0.025 }],
  click: [
    { frequency: 660, duration: 0.06 },
    { frequency: 990, duration: 0.09, delay: 0.05 },
  ],
  open: [
    { frequency: 523, duration: 0.07 },
    { frequency: 784, duration: 0.07, delay: 0.06 },
    { frequency: 1046, duration: 0.12, delay: 0.12 },
  ],
  close: [
    { frequency: 784, duration: 0.06 },
    { frequency: 440, duration: 0.1, delay: 0.05 },
  ],
  select: [{ frequency: 1174, duration: 0.05, type: "triangle", volume: 0.08 }],
  quest: [
    { frequency: 659, duration: 0.09, type: "triangle", volume: 0.09 },
    { frequency: 784, duration: 0.09, delay: 0.09, type: "triangle", volume: 0.09 },
    { frequency: 988, duration: 0.09, delay: 0.18, type: "triangle", volume: 0.09 },
    { frequency: 1318, duration: 0.24, delay: 0.27, type: "triangle", volume: 0.09 },
  ],
  error: [
    { frequency: 196, duration: 0.18, type: "sawtooth", volume: 0.05 },
    { frequency: 147, duration: 0.22, delay: 0.1, type: "sawtooth", volume: 0.05 },
  ],
  // The actual note is chosen at schedule time, so typing never lands on a mechanical loop.
  type: [{ frequency: 1400, duration: 0.02, volume: 0.012 }],
};

interface GainLike {
  readonly gain: {
    setValueAtTime(value: number, when: number): void;
    exponentialRampToValueAtTime(value: number, when: number): void;
  };
  connect(destination: AudioNode): void;
}

interface OscillatorLike {
  type: OscillatorType;
  readonly frequency: { setValueAtTime(value: number, when: number): void };
  connect(destination: AudioNode | GainLike): void;
  start(when: number): void;
  stop(when: number): void;
}

export interface AudioContextLike {
  readonly currentTime: number;
  readonly destination: AudioNode;
  createGain(): GainLike;
  createOscillator(): OscillatorLike;
  resume(): Promise<void>;
  /**
   * Playing a sound a World supplied, through the same context as the synthesised ones.
   *
   * **Optional, and that is not laziness.** A test supplies a context that can make an
   * oscillator; requiring it to also decode audio would make every existing test carry a
   * dependency on a feature it does not exercise. A context without them simply cannot play
   * samples, which is the same answer a browser with no Web Audio gives: silence, never a
   * broken World.
   */
  decodeAudioData?(data: ArrayBuffer): Promise<AudioBufferLike>;
  createBufferSource?(): BufferSourceLike;
}

/** Only what is actually touched, like every other shape in this file. */
export interface AudioBufferLike {
  readonly duration: number;
}

export interface BufferSourceLike {
  buffer: AudioBufferLike | null;
  connect(destination: AudioNode | GainLike): void;
  start(when: number): void;
}

interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

interface SfxOptions {
  readonly createContext: () => AudioContextLike | null;
  readonly storage?: StorageLike | null;
  readonly random?: () => number;
}

const MUTE_KEY = "epoch.sfx.muted";
const VOLUME_KEY = "epoch.sfx.volume";
const FLOOR = 1e-4;
const ATTACK = 0.008;

function mutedFrom(storage: StorageLike | null | undefined): boolean {
  try {
    return storage?.getItem(MUTE_KEY) === "true";
  } catch {
    // An unavailable browser storage is not a reason for controls to stop working.
    return false;
  }
}

function volumeFrom(storage: StorageLike | null | undefined): number {
  try {
    /*
      **Nothing remembered means full, and this used to mean silent.**

      `Number(storage?.getItem(VOLUME_KEY))` on a key that was never written is `Number(null)`,
      which is **0** — and `Number.isFinite(0)` is true, so the guard passed and the clamp
      returned zero. `schedule` returns early on `volume <= 0`, so **Epoch made no sound at all
      until somebody moved the slider once**, after which it worked forever and the defect
      became invisible.

      Found while verifying that a World's own sounds play: no oscillator was created for any
      click, on a World whose HUD said SOUND ON. Measured in the window rather than reasoned
      about — `Number(null)` is 0, `localStorage` held no volume key.

      The `catch` below already returned 1, and the comment on it says what was intended. The
      hole is that a *missing* key is not an *unreadable* one, and only the second was handled.
    */
    const remembered = storage?.getItem(VOLUME_KEY);
    if (remembered === null || remembered === undefined) return 1;
    const level = Number(remembered);
    return Number.isFinite(level) ? Math.max(0, Math.min(1, level)) : 1;
  } catch {
    // An unreadable storage is optional persistence, not a reason to be silent.
    return 1;
  }
}

/** A schedulable, testable sound bank. Browser state is kept outside the voice definitions. */
export function createSfx({ createContext, storage, random = Math.random }: SfxOptions) {
  let context: AudioContextLike | null = null;
  let muted = mutedFrom(storage);
  let volume = volumeFrom(storage);
  const subscribers = new Set<() => void>();

  /*
    What this World supplies, and what it has decoded so far.

    **Decoded once and kept, because a click cannot wait.** `decodeAudioData` is asynchronous, so
    a sound decoded at play time would arrive after the interaction it belongs to. `warm` below
    therefore decodes on arrival rather than on demand — the first version decoded lazily and the
    first press of every session came out in Epoch's voice on a World that had supplied its own.

    A supplied sound whose bytes will not decode is dropped and the synthesised voice takes over
    again — the fallback chain, not an error path.
  */
  let supplied: Readonly<Record<string, string>> = {};
  const decoded = new Map<string, AudioBufferLike>();
  const decoding = new Set<string>();

  const notify = () => subscribers.forEach((listener) => listener());

  /** `data:audio/wav;base64,…` to the bytes, without a fetch. */
  const bytesOf = (uri: string): ArrayBuffer | null => {
    const at = uri.indexOf("base64,");
    if (at < 0) return null;
    try {
      const raw = atob(uri.slice(at + 7));
      const out = new Uint8Array(raw.length);
      for (let i = 0; i < raw.length; i += 1) out[i] = raw.charCodeAt(i);
      return out.buffer;
    } catch {
      return null;
    }
  };

  /**
   * Decode everything this World supplies, now.
   *
   * **On arrival, not on the first click.** Decoding when a sound is first *wanted* means the
   * press that wanted it gets Epoch's synthesised voice instead — one click in the wrong voice,
   * every session, on a World that supplied its own. Sounds are handed over when the World loads
   * and audio unlocks on the first gesture, so both are long before anybody presses anything.
   *
   * Silent about failure by design: bytes that will not decode are a sound this World does not
   * have, and the synthesised voice is still there. The fallback chain, not an error path.
   */
  const warm = () => {
    if (!context?.decodeAudioData) return;
    for (const [concept, uri] of Object.entries(supplied)) {
      const voice = concept.startsWith("sfx.") ? (concept.slice(4) as SfxName) : null;
      if (!voice || decoded.has(voice) || decoding.has(voice)) continue;
      const bytes = bytesOf(uri);
      if (!bytes) continue;
      decoding.add(voice);
      void context
        .decodeAudioData(bytes)
        .then((ready) => decoded.set(voice, ready))
        .catch(() => {})
        .finally(() => decoding.delete(voice));
    }
  };

  /**
   * Play what the World supplied for this voice, and say whether it did.
   *
   * `false` means *carry on and synthesise*: no sound supplied, no context that can play one, or
   * one that is still decoding — which `warm` makes rare rather than routine.
   */
  const sample = (voice: SfxName): boolean => {
    const uri = supplied[`sfx.${voice}`];
    if (!uri || !context) return false;
    if (!context.createBufferSource) return false;

    const buffer = decoded.get(voice);
    if (!buffer) {
      // Not decoded yet, and nothing is decoding it: the context arrived after the sounds did.
      warm();
      return false;
    }

    try {
      const source = context.createBufferSource();
      const gain = context.createGain();
      source.buffer = buffer;
      // The same volume the synthesised voices answer to. A supplied sound that ignored the
      // slider would be a second audio system wearing the first one's controls.
      gain.gain.setValueAtTime(Math.max(FLOOR, volume), context.currentTime);
      source.connect(gain);
      gain.connect(context.destination);
      source.start(context.currentTime);
      return true;
    } catch {
      return false;
    }
  };

  const schedule = (voice: SfxName) => {
    if (muted || !context) return;
    // Turned all the way down is silence, and silence is not a quiet sound — it is no sound.
    // Scheduling one anyway is what produced the failure below.
    if (volume <= 0) return;

    // A World's own voice wins over the synthesised one. It never fills an empty slot: there was
    // never an empty slot, which is why `sfx.ts` ships no files.
    if (sample(voice)) return;

    /*
      **A sound must never be able to break the World.**

      Reported as "clicking an NPC does nothing, MISSIONS breaks the view — when the volume is
      low", and those were one defect. An exponential ramp cannot reach zero: with the volume at
      the bottom the target became `0` and Web Audio threw. The exception left `playSfx`, left the
      click handler that called it, and reached the error boundary — so the click never did its
      work and the screen said THE VIEW BROKE.

      Two guards, because either alone would have been enough and neither is sufficient reasoning
      on its own. The floor is the specific fix; this is the general one: audio is decoration, the
      click is the product, and nothing decorative may take an interaction with it.
    */
    try {
      const now = context.currentTime;
      for (const note of SFX_VOICES[voice]) {
        const starts = now + (note.delay ?? 0);
        const oscillator = context.createOscillator();
        const gain = context.createGain();
        oscillator.type = note.type ?? "square";
        oscillator.frequency.setValueAtTime(
          voice === "type" ? note.frequency + random() * 400 : note.frequency,
          starts,
        );
        // Never zero, and never *below* the floor it ramps back down to.
        const peak = Math.max(FLOOR * 2, (note.volume ?? 0.06) * volume);
        gain.gain.setValueAtTime(FLOOR, starts);
        gain.gain.exponentialRampToValueAtTime(peak, starts + ATTACK);
        gain.gain.exponentialRampToValueAtTime(FLOOR, starts + note.duration);
        oscillator.connect(gain);
        gain.connect(context.destination);
        oscillator.start(starts);
        oscillator.stop(starts + note.duration + 0.02);
      }
    } catch {
      // A sound that could not be scheduled is a sound nobody hears. It is not a reason for the
      // World to stop working.
    }
  };

  return {
    /** Called by an actual pointer/key event. This is the only path that creates Web Audio. */
    unlock: async () => {
      context ??= createContext();
      if (!context) return;
      try {
        await context.resume();
      } catch {
        // A browser may still reject an edge-case gesture. Keep the World usable and let the
        // next real gesture resume the same context instead of surfacing an audio error.
      }
      // The sounds were supplied before there was anything to decode them with.
      warm();
    },
    /** Safe from effects and subscriptions: it stays silent until `unlock()` has succeeded. */
    play: schedule,
    /**
     * Hand over what this World supplies. Anything already decoded for a different World goes.
     *
     * Pushed in rather than read through a hook: a click handler runs outside React, and the
     * player has to answer it without one.
     */
    supply: (sounds: Readonly<Record<string, string>>) => {
      supplied = sounds;
      decoded.clear();
      decoding.clear();
      warm();
    },
    isMuted: () => muted,
    volume: () => volume,
    setMuted: (next: boolean) => {
      muted = next;
      try {
        storage?.setItem(MUTE_KEY, String(next));
      } catch {
        // Privacy-mode storage is optional persistence, not a fatal dependency.
      }
      notify();
      if (!next) schedule("select");
    },
    setVolume: (next: number) => {
      volume = Math.max(0, Math.min(1, Number.isFinite(next) ? next : 1));
      try {
        storage?.setItem(VOLUME_KEY, String(volume));
      } catch {
        // A denied storage write cannot break the World.
      }
      notify();
    },
    subscribe: (listener: () => void) => {
      subscribers.add(listener);
      return () => subscribers.delete(listener);
    },
  };
}

function browserContext(): AudioContextLike | null {
  if (typeof window === "undefined") return null;
  const Context = window.AudioContext;
  return Context ? new Context() : null;
}

function browserStorage(): StorageLike | null {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage;
  } catch {
    // Access to localStorage itself may be denied before getItem/setItem can be guarded.
    return null;
  }
}

const sfx = createSfx({ createContext: browserContext, storage: browserStorage() });

export const playSfx = sfx.play;
export const unlockSfx = sfx.unlock;
export const supplySfx = sfx.supply;

/** React reads the same persisted mute state every HUD surface uses. */
export function useSfx() {
  const muted = useSyncExternalStore(sfx.subscribe, sfx.isMuted, () => true);
  const volume = useSyncExternalStore(sfx.subscribe, sfx.volume, () => 1);
  return { muted, volume, setMuted: sfx.setMuted, setVolume: sfx.setVolume, play: sfx.play, unlock: sfx.unlock };
}
