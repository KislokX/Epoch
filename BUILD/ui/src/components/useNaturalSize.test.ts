import { describe, expect, it } from "vitest";

import { cellAspect, unevenly } from "./useNaturalSize";

/**
 * The numbers come from a real sheet, not from an example.
 *
 * Mage's idle: 163×32, cut into 6 columns and 1 row. It is the sheet that showed the defect,
 * so it is the sheet the test is written against.
 */
const MAGE_IDLE = { width: 163, height: 32 };

describe("a cell keeps the shape it was drawn in", () => {
  it("is taller than it is wide, and a square box was making it fatter", () => {
    // 163 ÷ 6 = 27.17 wide against 32 tall. Forced into a 96×96 box, every frame was stretched
    // 18% sideways — in the editor's preview and in the World alike, so the same character was
    // one shape standing still and another shape walking.
    const aspect = cellAspect(MAGE_IDLE, 6, 1);
    expect(aspect).not.toBeNull();
    expect(aspect!).toBeCloseTo(0.849, 3);

    // Fitted rather than filled: the drawn box never exceeds the square it was given.
    const side = 96;
    const width = aspect! >= 1 ? side : side * aspect!;
    const height = aspect! >= 1 ? side / aspect! : side;
    expect(width).toBeLessThan(side);
    expect(height).toBe(side);
    expect(width / height).toBeCloseTo(aspect!, 5);
  });

  it("leaves a square sheet square", () => {
    // The common case must not move. A 4×4 sheet of square cells is exactly what it was.
    expect(cellAspect({ width: 256, height: 256 }, 4, 4)).toBe(1);
  });

  it("says nothing when the image has not decoded", () => {
    // Unknown is not a shape. A caller that gets null draws the square it already drew —
    // an image still loading is not a reason to show nobody.
    expect(cellAspect(null, 6, 1)).toBeNull();
    expect(cellAspect(MAGE_IDLE, 0, 1)).toBeNull();
  });
});

describe("a grid that does not divide the sheet", () => {
  it("says so, with what it measured", () => {
    // Four numbers agreeing with each other is exactly what a wrong cut looks like. Only the
    // image can catch this, and 163 ÷ 6 is where it was caught.
    expect(unevenly(MAGE_IDLE, 6, 1)).toBe(
      "The sheet is 163 pixels wide, which 6 columns do not divide evenly.",
    );
  });

  it("names the rows when it is the rows", () => {
    expect(unevenly({ width: 128, height: 50 }, 4, 3)).toBe(
      "The sheet is 50 pixels tall, which 3 rows do not divide evenly.",
    );
  });

  it("says both when both are wrong, rather than only the first", () => {
    expect(unevenly({ width: 163, height: 50 }, 6, 3)).toBe(
      "The sheet is 163×50, which 6×3 does not divide evenly.",
    );
  });

  it("is quiet when the grid fits", () => {
    // Reported, never refused — and never nagged about when there is nothing to report.
    expect(unevenly({ width: 162, height: 32 }, 6, 1)).toBeNull();
    expect(unevenly(null, 6, 1)).toBeNull();
  });
});
