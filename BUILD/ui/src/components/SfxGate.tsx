/**
 * The one gesture gate for World sound.
 *
 * Web Audio may only begin from a user gesture. Listening at the document keeps that rule true
 * for every current and future HUD control without inserting a wrapper around the World or
 * changing the pointer-event contract that lets the camera work through HUD gaps.
 */

import { useEffect } from "react";

import { playSfx, supplySfx, unlockSfx } from "../experience/sfx";
import { onSounds, useSounds } from "../experience/useSounds";

export function SfxGate() {
  /*
    **What this World supplies, handed to the player.**

    Here rather than anywhere else because this component is already the one place that knows
    about sound and mounts once for the whole application. A hook cannot help the click handler
    below — it runs outside React — so the answer is pushed in instead of read.

    `onSounds` also registers the player for later answers, which is what makes a sound saved in
    the editor audible without restarting Epoch.
  */
  const sounds = useSounds();
  useEffect(() => {
    onSounds(supplySfx);
  }, []);
  useEffect(() => {
    supplySfx(sounds);
  }, [sounds]);

  useEffect(() => {
    const unlock = () => { void unlockSfx(); };
    const pixelButton = (target: EventTarget | null) =>
      target instanceof Element ? target.closest("button.epbtn") : null;

    const hover = (event: PointerEvent) => {
      const button = pixelButton(event.target);
      // Moving across an icon *inside* one button is not a second hover of that button.
      if (!button || button === pixelButton(event.relatedTarget)) return;
      playSfx("hover");
    };
    const click = (event: MouseEvent) => {
      if (!pixelButton(event.target)) return;
      playSfx("click");
    };

    document.addEventListener("pointerdown", unlock, true);
    document.addEventListener("keydown", unlock, true);
    document.addEventListener("pointerover", hover, true);
    document.addEventListener("click", click, true);
    return () => {
      document.removeEventListener("pointerdown", unlock, true);
      document.removeEventListener("keydown", unlock, true);
      document.removeEventListener("pointerover", hover, true);
      document.removeEventListener("click", click, true);
    };
  }, []);

  return null;
}
