import type { MarkView } from "../ipc/contracts";
import { cell, useSheetFrame } from "./useSheetFrame";
import { cellAspect, useNaturalSize } from "./useNaturalSize";

/**
 * A sprite sheet, playing.
 *
 * ## Why this exists at all
 *
 * A cut is four numbers, and four numbers cannot be checked by reading them. A sheet declared
 * `4×4` that is really `4×3` looks perfectly reasonable in a form and produces a character who
 * walks with somebody else's legs. **A mis-cut sheet is only ever discovered by watching it**,
 * which is why the editor's preview plays rather than showing the first frame.
 *
 * ## The Engine cut it; this only plays it
 *
 * Everything here is per-frame — which cell is on screen at this millisecond, and when to move
 * to the next. The Engine said how many cells there are, where they are and how long each is
 * shown, and none of that is decided here (ADR-0018: the Engine owns reality, the UI owns
 * animation).
 *
 * ## Reading order, every cell
 *
 * Cells play top-left to bottom-right, which for a directional sheet means the preview walks
 * through all four directions in turn. That is the honest thing to show: the question the user
 * is answering is *did I cut this right*, and one row would only answer it for one row.
 */
export function SheetPlayer({
  mark,
  size = 96,
  label,
}: {
  readonly mark: MarkView;
  /** Side of the square the sheet is drawn into, in pixels. */
  readonly size?: number;
  readonly label: string;
}) {
  const frames = mark.frames;
  // The same cadence the World plays at — shared, so a walk that looked right here does not
  // run at a different speed outside the editor.
  const at = useSheetFrame(frames);
  /*
    **How big the file really is, so a cell is not squashed into a square.**

    Measured on a real sheet: 163×32 cut into 6 columns gives cells of 27.17×32 — taller than
    they are wide. Drawn into a 96×96 box every frame was stretched 18% sideways, and the
    character was visibly fatter here than in the file. This preview's only job is answering
    *did I cut this right*, and one that distorts what it shows cannot answer it.

    Presentation arithmetic, not a cut: the Engine still says how many cells there are and how
    long each is shown. This decides the shape of the box they are drawn in.
  */
  const natural = useNaturalSize(mark.asset);

  if (!mark.asset) {
    return (
      <div className="sheet sheet--none" role="img" aria-label={`${label}: nothing to draw`}>
        <span>no sheet</span>
      </div>
    );
  }

  // A still image fills the box. Nothing to cut, so nothing to compute.
  if (!frames) {
    return (
      <div
        className="sheet"
        role="img"
        aria-label={label}
        style={{
          width: size,
          height: size,
          backgroundImage: `url(${mark.asset})`,
          backgroundSize: "contain",
          backgroundPosition: "center",
        }}
      />
    );
  }

  // No facing: every cell in reading order, which for a directional sheet walks through all
  // four directions in turn. That is this preview's question — *did I cut this right* — and one
  // row would only answer it for one row.
  const [column, row] = cell(frames, at);
  // A cell wider than it is tall keeps the height and loses width, and the other way round —
  // so the drawn box never exceeds the square it was given a side of. Unknown falls back to
  // the square: an image that has not decoded yet is not a reason to draw nothing.
  const aspect = cellAspect(natural, frames.columns, frames.rows);
  const width = aspect === null ? size : aspect >= 1 ? size : size * aspect;
  const height = aspect === null ? size : aspect >= 1 ? size / aspect : size;
  return (
    <div
      className="sheet"
      role="img"
      aria-label={label}
      data-frame={at}
      style={{
        width,
        height,
        backgroundImage: `url(${mark.asset})`,
        // The whole sheet scaled so that **one cell** covers the box.
        backgroundSize: `${frames.columns * 100}% ${frames.rows * 100}%`,
        // Percentage background positions are a ratio of the leftover space, not of the image
        // — so the last cell is 100% and a single column or row is 0, never a division by zero.
        backgroundPosition: `${percent(column, frames.columns)}% ${percent(row, frames.rows)}%`,
      }}
    />
  );
}

function percent(index: number, of: number): number {
  return of > 1 ? (index / (of - 1)) * 100 : 0;
}
