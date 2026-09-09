import { describe, expect, it, vi } from "vitest";

import { createSfx } from "./sfx";

function fakeAudio({ decodes = true }: { decodes?: boolean } = {}) {
  const oscillators: Array<{ frequency: number; type: OscillatorType; start: number; stop: number }> = [];
  const gainCalls: Array<{ value: number; when: number }> = [];
  // Sources a supplied sound was played through. Empty means the synthesised voice was used.
  const sources: Array<{ started: number }> = [];
  const resume = vi.fn().mockResolvedValue(undefined);
  const context = {
    decodeAudioData: (_data: ArrayBuffer) =>
      decodes
        ? Promise.resolve({ duration: 0.25 })
        : Promise.reject(new Error("not audio this browser knows")),
    createBufferSource: () => {
      const row = { started: 0 };
      return {
        buffer: null,
        connect: () => {},
        start: (when: number) => {
          row.started = when;
          sources.push(row);
        },
      };
    },
    currentTime: 3,
    destination: {} as AudioNode,
    createGain: () => ({
      gain: {
        setValueAtTime: (value: number, when: number) => gainCalls.push({ value, when }),
        exponentialRampToValueAtTime: (value: number, when: number) => gainCalls.push({ value, when }),
      },
      connect: () => {},
    }),
    createOscillator: () => {
      const row = { frequency: 0, type: "square" as OscillatorType, start: 0, stop: 0 };
      oscillators.push(row);
      return {
        get type() { return row.type; },
        set type(next: OscillatorType) { row.type = next; },
        frequency: { setValueAtTime: (value: number) => { row.frequency = value; } },
        connect: () => {},
        start: (when: number) => { row.start = when; },
        stop: (when: number) => { row.stop = when; },
      };
    },
    resume,
  };
  return { context, oscillators, gainCalls, sources, resume };
}

describe("offline World sound", () => {
  it("cannot schedule sound before a user gesture unlocks audio", () => {
    const audio = fakeAudio();
    const createContext = vi.fn(() => audio.context);
    const sfx = createSfx({ createContext });

    sfx.play("open");

    expect(createContext).not.toHaveBeenCalled();
    expect(audio.oscillators).toHaveLength(0);
  });

  it("schedules every click note with its authored pitch and envelope after unlock", async () => {
    const audio = fakeAudio();
    const sfx = createSfx({ createContext: () => audio.context });

    await sfx.unlock();
    sfx.play("click");

    expect(audio.resume).toHaveBeenCalledOnce();
    expect(audio.oscillators.map((note) => note.frequency)).toEqual([660, 990]);
    expect(audio.oscillators.map((note) => note.start)).toEqual([3, 3.05]);
    expect(audio.oscillators[0]?.stop).toBeCloseTo(3.08);
    expect(audio.oscillators[1]?.stop).toBeCloseTo(3.16);
    expect(audio.gainCalls).toContainEqual({ value: 0.06, when: 3.008 });
  });

  it("persists mute and schedules nothing while muted", async () => {
    const audio = fakeAudio();
    const items = new Map<string, string>();
    const sfx = createSfx({
      createContext: () => audio.context,
      storage: { getItem: (key) => items.get(key) ?? null, setItem: (key, value) => items.set(key, value) },
    });

    await sfx.unlock();
    sfx.setMuted(true);
    sfx.play("error");

    expect(items.get("epoch.sfx.muted")).toBe("true");
    expect(audio.oscillators).toHaveLength(0);
  });

  it("persists volume and applies it to each scheduled voice", async () => {
    const audio = fakeAudio();
    const items = new Map<string, string>();
    const sfx = createSfx({
      createContext: () => audio.context,
      storage: { getItem: (key) => items.get(key) ?? null, setItem: (key, value) => items.set(key, value) },
    });

    await sfx.unlock();
    sfx.setVolume(0.5);
    sfx.play("hover");

    expect(items.get("epoch.sfx.volume")).toBe("0.5");
    expect(sfx.volume()).toBe(0.5);
    expect(audio.gainCalls).toContainEqual({ value: 0.0125, when: 3.008 });
  });
});

describe("a World that has not been touched", () => {
  it("is not silent, which it was", async () => {
    /*
      **The defect, and it hid behind its own fix.**

      `Number(storage.getItem(key))` on a key that was never written is `Number(null)` \u2014 which is
      `0`, and `Number.isFinite(0)` is true, so the guard passed and the volume came out zero.
      `schedule` returns early at zero, so Epoch made no sound at all until somebody moved the
      slider once \u2014 after which it worked forever and nobody could see the problem again.

      Found in the window while checking that a World's own sounds play: not one oscillator was
      created for any click, on a World whose HUD said SOUND ON.
    */
    const audio = fakeAudio();
    const empty = {
      getItem: () => null,
      setItem: () => {},
    };
    const sfx = createSfx({ createContext: () => audio.context, storage: empty });

    expect(sfx.volume()).toBe(1);

    await sfx.unlock();
    sfx.play("click");
    expect(audio.oscillators.length).toBeGreaterThan(0);
  });

  it("still honours a volume somebody actually chose, including zero", async () => {
    // The fix must not read *stored silence* as *nothing stored*: turning it all the way down
    // is a decision, and one this file already has a test about.
    const audio = fakeAudio();
    const quiet = { getItem: () => "0", setItem: () => {} };
    const sfx = createSfx({ createContext: () => audio.context, storage: quiet });

    expect(sfx.volume()).toBe(0);
    await sfx.unlock();
    sfx.play("click");
    expect(audio.oscillators).toHaveLength(0);
  });
});

describe("a World's own voice", () => {
  it("replaces the synthesised one rather than playing beside it", async () => {
    // `sfx.ts` ships no files, so a supplied sound is never filling an empty slot \u2014 it is
    // taking over from something that already worked. Both playing would be two clicks.
    const audio = fakeAudio();
    const sfx = createSfx({ createContext: () => audio.context });
    await sfx.unlock();

    sfx.supply({ "sfx.click": "data:audio/wav;base64,AAAA" });

    // Decoded on arrival rather than on the first click, so the very first press is already in
    // this World's voice. Decoding when a sound is first *wanted* means one press per session in
    // the wrong voice, which is a World that sounds like Epoch for exactly one click.
    await Promise.resolve();
    sfx.play("click");

    expect(audio.sources.length).toBeGreaterThan(0);
    expect(audio.oscillators).toHaveLength(0);
  });

  it("falls back to the synthesised voice when the bytes will not decode", async () => {
    // The fallback chain, not an error path: a World with an unreadable sound is a World with
    // Epoch's sound, which is what every World has today.
    const audio = fakeAudio({ decodes: false });
    const sfx = createSfx({ createContext: () => audio.context });
    await sfx.unlock();

    sfx.supply({ "sfx.click": "data:audio/wav;base64,AAAA" });
    sfx.play("click");
    await Promise.resolve();
    await Promise.resolve();
    sfx.play("click");

    expect(audio.sources).toHaveLength(0);
    expect(audio.oscillators.length).toBeGreaterThan(0);
  });
});

describe("sound never takes an interaction with it", () => {
  it("schedules nothing at all when the volume is at the bottom", async () => {
    // Silence is not a quiet sound. Scheduling one anyway is what produced the failure below.
    const audio = fakeAudio();
    const sfx = createSfx({ createContext: () => audio.context });
    await sfx.unlock();
    sfx.setVolume(0);
    sfx.play("click");

    expect(audio.oscillators).toHaveLength(0);
    expect(audio.gainCalls).toHaveLength(0);
  });

  it("never ramps a gain to zero, whatever the volume", async () => {
    // An exponential ramp cannot reach zero, and Web Audio throws when asked. Reported as
    // "clicking an NPC does nothing and MISSIONS breaks the view — when the volume is low":
    // the exception left `play`, left the click handler that called it, and reached the error
    // boundary, so the click never did its work.
    const audio = fakeAudio();
    const sfx = createSfx({ createContext: () => audio.context });
    await sfx.unlock();

    for (const volume of [0.001, 0.01, 0.05, 1]) {
      sfx.setVolume(volume);
      sfx.play("click");
    }
    expect(audio.gainCalls.length).toBeGreaterThan(0);
    for (const call of audio.gainCalls) expect(call.value).toBeGreaterThan(0);
  });

  it("a sound that cannot be scheduled does not stop the World", async () => {
    // The general guard. Audio is decoration; the click is the product.
    const audio = fakeAudio();
    const angry = {
      ...audio.context,
      createGain: () => {
        throw new Error("this browser is having a day");
      },
    };
    const sfx = createSfx({ createContext: () => angry });
    await sfx.unlock();

    expect(() => sfx.play("click")).not.toThrow();
  });
});
