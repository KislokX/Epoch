/**
 * What this World sounds like.
 *
 * ## The same pipeline as its windows, and the same fallback
 *
 * A World Pack supplies its buildings, its land, its names and its windows through one concept
 * vocabulary (ADR-0016). Its voice arrives the same way: `sfx.click` resolved by the active pack,
 * delivered as a `data:` URI so an installed World cannot name a file the browser then opens.
 *
 * ## Absent is the ordinary answer, and it is not an empty slot
 *
 * `sfx.ts` synthesises its eight voices through Web Audio and **deliberately ships no files**, so
 * that Epoch makes a noise on a machine that has downloaded nothing. A World supplying a sound is
 * therefore *replacing* something that already worked — which is the opposite of the usual asset,
 * where absent means a placeholder. Every World today supplies none, and that is complete.
 */

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/** Concept to audio, as `data:` URIs. */
export type SoundSet = Readonly<Record<string, string>>;

const NONE: SoundSet = {};

/**
 * Asked for once per session, like the skin.
 *
 * Sounds are wanted by a click handler rather than by a component, so the answer is also pushed
 * into `sfx` as soon as it arrives — a hook cannot help something that runs outside React.
 */
let asked: Promise<SoundSet> | null = null;
let known: SoundSet = NONE;

/** Called with whatever came back, so the player can reach it without a hook. */
let deliver: (sounds: SoundSet) => void = () => {};

/** Where `sfx` registers itself. Kept out of the module graph's way to avoid a cycle. */
export function onSounds(receive: (sounds: SoundSet) => void): void {
  deliver = receive;
  receive(known);
}

function ask(): Promise<SoundSet> {
  asked ??= invoke<SoundSet>("world_sounds")
    .then((supplied) => {
      known = supplied ?? NONE;
      deliver(known);
      return known;
    })
    // An engine that will not answer is not a reason for the World to fall silent: Epoch has its
    // own voice, and it is what every World has been using so far.
    .catch(() => NONE);
  return asked;
}

export function useSounds(): SoundSet {
  const [sounds, setSounds] = useState<SoundSet>(known);

  useEffect(() => {
    let looking = true;
    void ask().then((supplied) => {
      if (looking) setSounds(supplied);
    });
    return () => {
      looking = false;
    };
  }, []);

  return sounds;
}

/**
 * Ask again, and hand the answer to whoever is listening.
 *
 * ## Why this re-asks rather than only forgetting
 *
 * The skin's equivalent forgets, and that is right for it: a `Frame` mounts *inside* a World, so
 * the next one to mount asks and gets the new answer.
 *
 * Sound has no such reader. `SfxGate` mounts once, at the application root, and it does so **on
 * the Launcher — before any World is open**. Measured in the window: the Engine answered
 * `world_sounds` with the World's own click while every press still produced an oscillator,
 * because the empty answer from the bridge had been cached for the session and nothing ever
 * mounted to ask again.
 *
 * So forgetting alone would leave the World permanently silent of its own voice, which is worse
 * than the state it was meant to fix.
 *
 * Two callers, and they are the two moments the answer can change: entering a World, and saving
 * one in the editor. A sound the author just chose and cannot hear until Epoch restarts is an
 * editor whose changes are invisible — the cold instrument of editors.
 */
export function refreshSounds(): void {
  asked = null;
  known = NONE;
  void ask();
}
