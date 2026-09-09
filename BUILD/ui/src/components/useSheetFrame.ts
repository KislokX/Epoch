import { useEffect, useState } from "react";

import type { Direction, FramesView } from "../ipc/contracts";

/**
 * Which cell of a sheet is on screen right now.
 *
 * ## The one place the cadence lives
 *
 * The editor's preview and the World draw a sheet in completely different ways — a CSS
 * background in a box, and an `<image>` clipped inside SVG — but they must agree about *when*
 * a frame changes, or a walk that looked right in the editor would run at a different speed
 * outside it. So the timing is here, once, and the two callers only differ in how they draw.
 *
 * ## This is playback, and playback is the UI's
 *
 * The Engine said how many cells there are, where they are and how long each is shown. Which
 * one is visible at this millisecond is per-frame, and per-frame never leaves presentation
 * (ADR-0018).
 */
export function useSheetFrame(frames: FramesView | undefined): number {
  const count = frames ? Math.max(1, frames.count) : 1;
  const milliseconds = frames?.milliseconds ?? 0;
  const [at, setAt] = useState(0);

  useEffect(() => {
    // A still picture, or a sheet of one cell: nothing to advance, and a timer firing forever
    // to redraw the same frame is work nobody asked for.
    if (count <= 1 || milliseconds <= 0) return;
    const timer = window.setInterval(
      () => setAt((previous) => (previous + 1) % count),
      milliseconds,
    );
    return () => window.clearInterval(timer);
  }, [count, milliseconds]);

  return at % count;
}

/**
 * Where one frame sits in the grid: `[column, row]`.
 *
 * **With a `facing`**, a directional sheet plays one row: somebody walking east must not cycle
 * through north on the way. A facing the sheet has no row for falls back to its first — the
 * sheet is the authority on which directions it was drawn for, and the row it does have beats
 * nothing at all.
 *
 * **Without one**, every cell plays in reading order. That is the editor's question rather than
 * the World's: *did I cut this right* is answered by watching all four directions go past, and
 * one row would only answer it for one row.
 *
 * A sheet that declares no directions is read the same way either way — its cells are one
 * animation rather than four.
 */
export function cell(
  frames: FramesView,
  at: number,
  facing?: Direction,
): readonly [number, number] {
  if (facing && frames.directions.length > 0) {
    const row = Math.max(0, frames.directions.indexOf(facing));
    return [at % frames.columns, Math.min(row, frames.rows - 1)];
  }
  return [at % frames.columns, Math.floor(at / frames.columns) % frames.rows];
}
