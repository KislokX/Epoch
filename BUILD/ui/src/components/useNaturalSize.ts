import { useEffect, useState } from "react";

/**
 * How big a picture really is, asked of the picture.
 *
 * ## Why this is not a setting
 *
 * A sheet's pixel size is a fact about the file. Nobody types it, nothing stores it, and a
 * number somebody typed could disagree with the image — which is the whole failure mode the
 * animation editor exists to catch. So it is read from the decoded image and from nowhere else.
 *
 * ## And why the UI is allowed to know it
 *
 * This is presentation arithmetic: how many pixels wide the drawn box should be so a cell is
 * not stretched. The Engine still owns the cut — how many cells, where, how long each is shown
 * (ADR-0018). Nothing here changes what is drawn, only its shape.
 *
 * `null` until the image has decoded, and `null` forever if it cannot. A caller that gets
 * `null` must still draw something: an unknown size is not a reason to show nothing.
 */
export function useNaturalSize(source: string | null | undefined): Size | null {
  const [size, setSize] = useState<Size | null>(null);

  useEffect(() => {
    if (!source) {
      setSize(null);
      return;
    }
    let alive = true;
    const image = new Image();
    image.onload = () => {
      if (!alive) return;
      // Zero would be a decoded image with no pixels, which is not a thing — but it would also
      // divide by zero downstream, and a guess is never worth that.
      if (image.naturalWidth > 0 && image.naturalHeight > 0) {
        setSize({ width: image.naturalWidth, height: image.naturalHeight });
      }
    };
    image.onerror = () => {
      if (alive) setSize(null);
    };
    image.src = source;
    return () => {
      alive = false;
    };
  }, [source]);

  return size;
}

export interface Size {
  readonly width: number;
  readonly height: number;
}

/**
 * The shape of one cell, as a width-over-height ratio.
 *
 * **The defect this exists for.** A 163×32 sheet cut into 6 columns has cells of 27.17×32 —
 * taller than they are wide. Drawn into a square box, every frame was stretched 18% sideways
 * and the character was visibly fatter in the editor than on the file. The preview's whole job
 * is answering *did I cut this right*, and a preview that distorts what it shows cannot answer
 * it.
 */
export function cellAspect(
  size: Size | null,
  columns: number,
  rows: number,
): number | null {
  if (!size || columns <= 0 || rows <= 0) return null;
  return size.width / columns / (size.height / rows);
}

/**
 * Whether the grid divides the sheet into whole pixels.
 *
 * Returns the reason it does not, or `null` when it does. Measured against the image rather
 * than checked against the numbers, because the numbers agreeing with each other is exactly
 * what a wrong cut looks like: `163 ÷ 6` is 27.17, every frame after the first sits a fraction
 * of a pixel further off, and the last one shows part of its neighbour.
 *
 * Reported, never refused. A sheet may genuinely have an odd margin, and the person who drew it
 * is the one who knows — this says what was measured and lets them decide.
 */
export function unevenly(
  size: Size | null,
  columns: number,
  rows: number,
): string | null {
  if (!size || columns <= 0 || rows <= 0) return null;
  const across = size.width % columns === 0;
  const down = size.height % rows === 0;
  if (across && down) return null;
  if (!across && !down) {
    return `The sheet is ${size.width}×${size.height}, which ${columns}×${rows} does not divide evenly.`;
  }
  return across
    ? `The sheet is ${size.height} pixels tall, which ${rows} rows do not divide evenly.`
    : `The sheet is ${size.width} pixels wide, which ${columns} columns do not divide evenly.`;
}
