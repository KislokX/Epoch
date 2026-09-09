import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { MarkView } from "../ipc/contracts";
import { SheetPlayer } from "./SheetPlayer";

function sheet(overrides: Partial<NonNullable<MarkView["frames"]>> = {}): MarkView {
  return {
    role: "visual",
    renderer: "animated_sprite",
    supported: false,
    asset: "data:image/png;base64,iVBORw0KGgo=",
    scale: 1,
    anchor: [0.5, 1],
    shape: [],
    frames: {
      columns: 4,
      rows: 4,
      count: 16,
      milliseconds: 100,
      directions: [],
      ...overrides,
    },
  };
}

describe("the sheet player", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("shows one cell of the sheet, and walks it in reading order", () => {
    render(<SheetPlayer mark={sheet()} label="Walking" />);
    const box = screen.getByRole("img", { name: "Walking" });

    // The sheet scaled so that exactly one cell covers the box — never the whole strip, which
    // is the wrong image rather than a smaller one.
    expect(box).toHaveStyle({ backgroundSize: "400% 400%" });
    // Percentage positions are a ratio of the leftover space, so the first cell is 0/0.
    expect(box).toHaveStyle({ backgroundPosition: "0% 0%" });

    act(() => void vi.advanceTimersByTime(100));
    expect(box).toHaveStyle({ backgroundPosition: "33.33333333333333% 0%" });

    // The fifth cell is the start of the second row: reading order, not one row forever.
    act(() => void vi.advanceTimersByTime(300));
    expect(box.dataset.frame).toBe("4");
    expect(box).toHaveStyle({ backgroundPosition: "0% 33.33333333333333%" });
  });

  it("stops at the number of frames that are real, not at the number of cells", () => {
    // The three empty cells of a 4x4 sheet holding thirteen frames. Played to the end of the
    // grid, a character would stand still for a quarter of every loop.
    render(<SheetPlayer mark={sheet({ count: 13 })} label="Walking" />);
    const box = screen.getByRole("img", { name: "Walking" });

    act(() => void vi.advanceTimersByTime(1300));
    expect(box.dataset.frame).toBe("0");
  });

  it("draws a single-cell sheet without dividing by zero", () => {
    render(
      <SheetPlayer mark={sheet({ columns: 1, rows: 1, count: 1 })} label="Idle" />,
    );
    expect(screen.getByRole("img", { name: "Idle" })).toHaveStyle({
      backgroundPosition: "0% 0%",
    });
  });

  it("says so rather than drawing nothing when the sheet could not be read", () => {
    const unreadable: MarkView = { ...sheet(), asset: null };
    render(<SheetPlayer mark={unreadable} label="Walking" />);
    expect(
      screen.getByRole("img", { name: "Walking: nothing to draw" }),
    ).toBeInTheDocument();
  });
});
