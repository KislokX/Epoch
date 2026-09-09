/**
 * One drawn part of a Place (ADR-0019, ADR-0021).
 *
 * This file draws whatever it is handed and **does not know what it is drawing**. There is
 * no place concept here, no switch on identity, no "if library". A Laboratory's wall and a
 * Library's column reach this function as identical data with different numbers.
 *
 * Geometry arrives in footprint units (1.0 = the Place's footprint, origin at base centre),
 * so a single scale puts it at whatever size the World authored.
 *
 * A role or a renderer this build cannot draw becomes a visible placeholder, never nothing.
 */

import { useId } from "react";

import type { Direction, MarkView, ShapeLayerView } from "../ipc/contracts";
import { cell, useSheetFrame } from "./useSheetFrame";
import { cellAspect, useNaturalSize } from "./useNaturalSize";

interface MarkProps {
  readonly mark: MarkView;
  /** Footprint in world units. */
  readonly footprint: number;
  /**
   * Which way the subject is facing, when anything knows.
   *
   * Only a journey has ever measured one (ADR-0018), so it is absent for everybody standing
   * still — and a directional sheet then plays in reading order rather than being pinned to a
   * facing nobody measured.
   */
  readonly facing?: Direction;
}

function layer(l: ShapeLayerView, i: number) {
  const className = `tone tone--${l.tone}`;

  switch (l.form) {
    case "rect":
      return (
        <rect
          key={i}
          className={className}
          x={l.x}
          y={l.y}
          width={l.w}
          height={l.h}
          rx={l.r || undefined}
        />
      );

    case "dome":
      // A half-circle sitting on (x, y), opening downward.
      return (
        <path
          key={i}
          className={className}
          d={`M ${l.x - l.r},${l.y} A ${l.r},${l.r} 0 0 1 ${l.x + l.r},${l.y} Z`}
        />
      );

    case "ellipse":
      // Centre at (x, y), radii w and h — centre-based like `dome`.
      return <ellipse key={i} className={className} cx={l.x} cy={l.y} rx={l.w} ry={l.h} />;

    case "polygon":
      return (
        <polygon
          key={i}
          className={className}
          points={l.points.map(([x, y]) => `${x},${y}`).join(" ")}
        />
      );

    default:
      return null;
  }
}

export function Mark({ mark, footprint, facing }: MarkProps) {
  const className = `mark mark--${mark.role}`;
  // Called unconditionally, above every branch: a hook may not sit behind an `if`, and the two
  // branches that do not animate pass `undefined` and get frame 0 with no timer running.
  const at = useSheetFrame(mark.frames);
  /*
    **How big the sheet really is**, so a cell keeps its shape out here too.

    The editor's preview had this wrong and so did the World: the clip was a square and the
    strip was stretched to fill a square grid, so a 163x32 sheet cut into 6 columns - cells of
    27.17x32 - drew everybody 18% wider than they were drawn. The same character was a different
    shape standing still and walking.

    Playback arithmetic, which is the UI's (ADR-0018). The Engine said how many cells and how
    long each is shown; this decides the shape of the box one is drawn in.
  */
  const natural = useNaturalSize(mark.asset);
  const clip = useId();

  // The World asked for something this build cannot draw. Degrade visibly (ADR-0016).
  if (!mark.supported) {
    return (
      <g className={`${className} mark--unsupported`} data-renderer={mark.renderer}>
        <rect x={-footprint / 2} y={-footprint * 0.6} width={footprint} height={footprint * 0.6} />
        <text className="mark__note" x={0} y={-footprint * 0.72}>
          {mark.role}: {mark.renderer}?
        </text>
      </g>
    );
  }

  // A sheet. One cell of it, drawn by scaling the whole strip up and clipping to the cell —
  // which is the SVG spelling of what the editor's preview does with a background position.
  //
  // **Nothing here decides anything about the World.** The cut came from the Engine, the frame
  // from a clock, and the facing from a journey the Engine measured.
  if (mark.asset && mark.frames) {
    const size = footprint * mark.scale;
    const [ax, ay] = mark.anchor;
    const [column, row] = cell(mark.frames, at, facing);

    // The cell fitted inside the square it was given, rather than filled into it. Unknown falls
    // back to the square: an image that has not decoded yet must still draw somebody.
    const aspect = cellAspect(natural, mark.frames.columns, mark.frames.rows);
    const cellW = aspect === null ? size : aspect >= 1 ? size : size * aspect;
    const cellH = aspect === null ? size : aspect >= 1 ? size / aspect : size;

    // The anchor still means what it meant: a fraction of the drawn cell, so feet at [0.5, 1]
    // stay on the ground whatever shape the cell turned out to be.
    const x = -cellW * ax;
    const y = -cellH * ay;
    return (
      <g className={`${className} mark--sheet`} data-frame={at}>
        <clipPath id={clip}>
          <rect x={x} y={y} width={cellW} height={cellH} />
        </clipPath>
        <image
          /*
            **The class belongs on the picture, not on the group around it.**

            `.mark--asset` has carried `image-rendering: pixelated` since the first sprite. The
            sheet path never got it: its class sits on the `<g>`, and there was no rule matching
            it at all — so every animated character was scaled with the browser's smoothing and
            came out soft, while the same character standing still was crisp.

            Named on the element that draws, rather than trusted to inherit. `image-rendering`
            does inherit, and relying on that is how this build already lost an evening once to a
            property applied to a group that did not.
          */
          className="mark--asset"
          href={mark.asset}
          clipPath={`url(#${clip})`}
          x={x - column * cellW}
          y={y - row * cellH}
          width={cellW * mark.frames.columns}
          height={cellH * mark.frames.rows}
          // The sheet is a grid of equal cells, so it is stretched to the grid rather than
          // fitted to it — the grid is now the sheet's own proportions, so nothing is
          // distorted and every cell still lands on its own rectangle.
          preserveAspectRatio="none"
        />
      </g>
    );
  }

  // An Asset. Drawn as an <image> from a data URI, never inlined as markup: a World is
  // third-party content and inlining its SVG would let it run script (ADR-0020).
  if (mark.asset) {
    const size = footprint * mark.scale;
    const [ax, ay] = mark.anchor;
    return (
      <image
        className={`${className} mark--asset`}
        href={mark.asset}
        x={-size * ax}
        y={-size * ay}
        width={size}
        height={size}
        preserveAspectRatio="xMidYMax meet"
      />
    );
  }

  return (
    <g className={className} transform={`scale(${footprint})`}>
      {mark.shape.map(layer)}
    </g>
  );
}
