/**
 * The four rules, each as a test, because each of them is a way this could go wrong quietly.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { waitFor } from "@testing-library/react";

const spoke = vi.fn();

vi.mock("../ipc/launcher", () => ({
  tryVoice: (voice: string, say: string) => spoke(voice, say),
}));

// The address a platform serves a custom scheme on is Tauri's to know, not this file's. Stubbed
// so the test asserts on *which file* was played, which is the part this module decides.
vi.mock("../ipc/world", () => ({
  spokenSrc: (name: string) => `epoch-test://${name}`,
}));

import { setVoicesOn } from "./audio";
import {
  alreadyHeard,
  hush,
  isSpeaking,
  playThrough,
  readable,
  say,
  stopSpeaking,
} from "./speak";

const played: { sound: string; volume: number; output: string }[] = [];
let restore = () => {};

const settle = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  spoke.mockReset();
  spoke.mockResolvedValue({ sound: "AAA", millis: 400, bytes: 100 });
  played.length = 0;
  restore = playThrough({
    async play(sound, volume, output) {
      played.push({ sound, volume, output });
    },
  });
  stopSpeaking();
});

afterEach(() => {
  restore();
});

describe("what a voice is handed", () => {
  it("reads the words and not the punctuation a screen needed", () => {
    expect(readable("**Important**: see `main.rs`")).toBe("Important: see main.rs");
    expect(readable("- one\n- two")).toBe("one two");
    expect(readable("# Heading\nbody")).toBe("Heading body");
    // A link says its words, never its address.
    expect(readable("read [the docs](https://example.com/x)")).toBe("read the docs");
    // A whole code block is noise out loud.
    expect(readable("here:\n```rust\nfn main() {}\n```\ndone")).toBe("here: done");
  });

  it("drops what a phonemiser cannot say", () => {
    // Measured on a real answer: `gemma-4-12b` ended one with a waving hand.
    expect(readable("¡Hola! 👋 Estoy listo.")).toBe("¡Hola! Estoy listo.");
  });

  it("says nothing at all rather than saying nothing useful", () => {
    expect(readable("```rust\nfn main() {}\n```")).toBe("");
  });
});

describe("who speaks and when", () => {
  it("is silent when nobody chose a voice", async () => {
    say(null, "Hola");
    say(undefined, "Hola");
    await settle();
    expect(spoke).not.toHaveBeenCalled();
  });

  it("does not ask for a sound there are no words for", async () => {
    say("es_ES-davefx-medium", "```\ncode\n```");
    await settle();
    // Nothing asked of the engine costs nothing and can misread nothing.
    expect(spoke).not.toHaveBeenCalled();
  });

  it("never says the same thing twice", async () => {
    say("es_ES-davefx-medium", "Hola, soy Mage.");
    say("es_ES-davefx-medium", "Hola, soy Mage.");
    await settle();
    await settle();
    // The Chronicle is re-read every turn and a window may remount. Either would replay an
    // answer somebody already heard.
    expect(spoke).toHaveBeenCalledTimes(1);
  });

  it("does not read a backlog aloud", async () => {
    // Walking into a conversation must not replay it. Measured in the window before this
    // existed: seven answers, one after another, from a transcript nobody asked to hear again.
    alreadyHeard("v", "Ya dicho.");
    say("v", "Ya dicho.");
    await settle();
    expect(spoke).not.toHaveBeenCalled();

    // And marking one does not deafen the next.
    say("v", "Esto es nuevo.");
    await settle();
    expect(spoke).toHaveBeenCalledTimes(1);
  });

  it("says nothing while the crew is switched off", async () => {
    setVoicesOn(false);
    say("v", "No deberia oirse.");
    await settle();
    expect(spoke).not.toHaveBeenCalled();

    // And switching back on lets the *next* answer be heard rather than replaying the ones
    // that arrived while nobody was listening. Those already happened.
    setVoicesOn(true);
    say("v", "No deberia oirse.");
    await settle();
    expect(spoke).toHaveBeenCalledWith("v", "No deberia oirse.");
    globalThis.localStorage.clear();
  });

  it("lets two characters say the same words", async () => {
    say("es_ES-davefx-medium", "Listo.");
    say("es_MX-ald-medium", "Listo.");
    await settle();
    await settle();
    expect(spoke).toHaveBeenCalledTimes(2);
  });

  it("plays one line at a time", async () => {
    let release: () => void = () => {};
    const first = new Promise<void>((r) => (release = r));
    let started = 0;
    restore();
    restore = playThrough({
      async play() {
        started += 1;
        if (started === 1) await first;
      },
    });

    say("v", "Uno.");
    say("v", "Dos.");
    await settle();
    await settle();
    // Two voices over each other is not a conversation.
    expect(started).toBe(1);
    release();
    await settle();
    await settle();
    expect(started).toBe(2);
  });

  it("carries on when the engine refuses", async () => {
    spoke.mockResolvedValueOnce("Piper is not installed here yet");
    spoke.mockResolvedValueOnce({ sound: "BBB", millis: 1, bytes: 1 });

    say("v", "Uno.");
    say("v", "Dos.");
    await settle();
    await settle();
    await settle();

    // A refusal is not announced and does not stop the queue: the answer is already on screen,
    // and an optional last step must never take the thing that succeeded down with it.
    expect(played.map((p) => p.sound)).toEqual(["epoch-test://BBB"]);
  });

  it("plays at the crew's own volume, on the chosen output", async () => {
    globalThis.localStorage.setItem("epoch.voices.volume", "0.25");
    globalThis.localStorage.setItem("epoch.audio.output", "8fad908e7a");
    say("v", "Hola.");
    await settle();
    await settle();
    expect(played[0]?.volume).toBeCloseTo(0.25);
    expect(played[0]?.output).toBe("8fad908e7a");
    globalThis.localStorage.clear();
  });
});

/**
 * Whether the World is talking, asked by the thing that must not listen while it is.
 *
 * The first hands-free session opened the microphone next to the speakers, heard Mage's own
 * answer, sent it back as a question, and repeated one paragraph forever. This is the predicate
 * that closes that loop by construction rather than by trusting a filter nobody measured.
 */
describe("whether the World is talking", () => {
  it("is quiet with nothing said and nothing queued", () => {
    stopSpeaking();
    expect(isSpeaking()).toBe(false);
  });

  it("counts a line that is queued, not only one that is playing", async () => {
    // The gap between *queued* and *playing* is milliseconds, and it is exactly where an open
    // microphone would catch the first syllable of the World's own voice.
    stopSpeaking();
    const played: string[] = [];
    const restore = playThrough({
      async play(sound) {
        played.push(sound);
        await new Promise((done) => setTimeout(done, 5));
      },
    });
    try {
      say("es_ES-davefx-medium", "Hola.");
      expect(isSpeaking()).toBe(true);
      await waitFor(() => expect(played.length).toBe(1));
    } finally {
      restore();
      stopSpeaking();
    }
  });
});

/**
 * Being quiet, and forgetting, are two questions.
 *
 * One function answered both, and hands free called it on every barge-in. Clearing the memory is
 * what made the World repeat itself: the Chronicle is re-read on every render, so with the
 * memory wiped every answer on screen became something that had *not* been said yet — and was
 * said again. The Chronicle on disk had five entries, which is what says the repetition was
 * never in the turns.
 */
describe("stopping a voice against forgetting a conversation", () => {
  function player() {
    const played: string[] = [];
    const stopped: number[] = [];
    const restore = playThrough({
      async play(sound) {
        played.push(sound);
        await new Promise((done) => setTimeout(done, 1));
      },
      stop() {
        stopped.push(1);
      },
    });
    return { played, stopped, restore };
  }

  it("hush keeps what was already said, so nothing is repeated", async () => {
    stopSpeaking();
    const { played, restore } = player();
    try {
      say("es_ES-davefx-medium", "Hola.");
      await waitFor(() => expect(played.length).toBe(1));

      hush();
      // The same answer, offered again exactly as the Chronicle offers it on every render.
      say("es_ES-davefx-medium", "Hola.");
      await new Promise((done) => setTimeout(done, 20));
      expect(played.length).toBe(1);
    } finally {
      restore();
      stopSpeaking();
    }
  });

  it("leaving a conversation forgets it, so walking back in can be heard again", async () => {
    stopSpeaking();
    const { played, restore } = player();
    try {
      say("es_ES-davefx-medium", "Hola.");
      await waitFor(() => expect(played.length).toBe(1));

      stopSpeaking();
      say("es_ES-davefx-medium", "Hola.");
      await waitFor(() => expect(played.length).toBe(2));
    } finally {
      restore();
      stopSpeaking();
    }
  });

  it("stops the sentence that is sounding, not only the queue behind it", async () => {
    // Turning NPC VOICES off mid-answer did nothing anybody could hear: `voicesOn` is read
    // before a line is queued, and emptying a queue does not silence an `<audio>` element.
    stopSpeaking();
    const { stopped, restore } = player();
    try {
      say("es_ES-davefx-medium", "Hola.");
      hush();
      expect(stopped.length).toBe(1);
    } finally {
      restore();
      stopSpeaking();
    }
  });
});
