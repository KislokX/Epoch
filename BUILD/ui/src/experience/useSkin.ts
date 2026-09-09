/**
 * What this World's windows look like.
 *
 * ## The point of this file
 *
 * Not "themes". A World Pack already supplies the buildings, the land, the names and the
 * vocabulary; its windows are part of the same World and arrive through the same pipeline — one
 * concept vocabulary, one resolver, one fallback chain (ADR-0016). A sci-fi World should not
 * replace its characters and keep somebody else's dialog boxes.
 *
 * ## What a pack owns, and what it never owns
 *
 * The **skin**. Not layout, not behaviour, not what can be reached by keyboard, not whether text
 * is legible. Those stay code, for the same reason CONTENT_PHILOSOPHY already says a pack cannot
 * change behaviour: a World is content, and content must not be able to make the product
 * unusable.
 *
 * ## Absent is the ordinary answer
 *
 * Every World today declares nothing, and that is complete rather than unfinished — Epoch draws
 * its own windows and always will. A concept nobody supplies falls through to the drawn one,
 * which is the fallback chain doing its job rather than an error being handled.
 */

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/** One resolved piece of interface. */
export interface Skin {
  /** The image, as a `data:` URI — never a path (ADR-0020). */
  readonly image: string;
  /**
   * Corner inset per side, `[top, right, bottom, left]`, in the image's own pixels.
   *
   * Four rather than one because the first real frame needed four: a bevel lit from above is not
   * symmetric, and one number would have stretched its highlight or eaten into its face.
   */
  readonly corner: readonly [number, number, number, number];
  /** How the edges and the middle fill a window larger than the image. */
  readonly repeat: "stretch" | "repeat" | "round" | "space";
  /** Whole-number scale. Epoch is pixel art; a fractional one destroys it. */
  readonly scale: number;
  /**
   * The smallest this window may be drawn, `[width, height]` in screen pixels.
   *
   * Derived by the Engine from the artwork: below it the corners meet and the frame stops being
   * a frame. Applied as `min-width`/`min-height` rather than handled as a special case — a size
   * that cannot look right is one the window simply does not reach.
   */
  readonly minSize: readonly [number, number];
}

/** Concepts to images. Empty until a World supplies some. */
export type SkinSet = Readonly<Record<string, Skin>>;

const NONE: SkinSet = {};

/**
 * Asked for once per session, not once per window.
 *
 * `Frame` is used in dozens of places, so a hook that fetched on mount would put dozens of IPC
 * calls on the first frame — for an answer that is the same every time and does not change while
 * a World is open. The promise is shared; whoever asks first starts it and everybody waits on
 * the same one.
 */
let asked: Promise<SkinSet> | null = null;
/** What came back, so a window mounting later renders skinned on its first paint. */
let known: SkinSet = NONE;

function ask(): Promise<SkinSet> {
  asked ??= invoke<SkinSet>("ui_skin")
    .then((supplied) => {
      known = supplied ?? NONE;
      return known;
    })
    // An engine that will not answer is not a reason to fail to draw a window: Epoch has its
    // own, and they are what every World has been using so far.
    .catch(() => NONE);
  return asked;
}

/**
 * Fetched once, on mount.
 *
 * Its own command rather than part of `WorldView`, for the reason the backdrop already
 * established: images are large, and the projection is re-sent whenever anybody's presence
 * changes. A skin does not change while a World is open.
 */
export function useSkin(): SkinSet {
  // Starts at whatever is already known, so only the very first window in a session waits.
  const [skin, setSkin] = useState<SkinSet>(known);

  useEffect(() => {
    let looking = true;
    void ask().then((supplied) => {
      if (looking) setSkin(supplied);
    });
    return () => {
      looking = false;
    };
  }, []);

  return skin;
}

/**
 * Forget what was asked, so the next window asks again.
 *
 * For entering a different World: the skin belongs to the World, and a cache that outlived one
 * would draw the old World's windows around the new one's contents.
 */
export function forgetSkin(): void {
  asked = null;
  known = NONE;
}
