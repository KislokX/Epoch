"""Turn a packed rip sheet into the uniform sheets Epoch can cut.

## Why this exists

Epoch's cut is four numbers — columns, rows, count, milliseconds — describing a grid of **equal
cells** in reading order. That is the right model: a renderer can clip it without decoding
anything, and an author can check it by watching it play.

A rip sheet is not that. Mage's is 811x341: bands from 32 to 80 pixels tall, sprites from 15 to
36 wide, and one band where two rows touch with no transparent line between them. Nothing can
cut it correctly, because what says which frames belong together is not in the pixels.

So the split is: `sheet_sprites.py` finds and numbers the sprites, the author says which numbers
are one animation, and this pads every chosen frame into one cell size and lays them out.

## What it will not do

Guess. It never reorders frames, never drops one that will not fit, and never picks a cell
smaller than the widest sprite it was given — the cell grows to hold the art rather than the art
being cropped to fit the cell.

## Alignment

Frames are centred across and sat on the **bottom** of the cell, because a character stands on
the ground and Epoch's anchor is their feet. Centring vertically would make a crouching frame
hover.

## Flipping

`~` before a group mirrors it. West and east are usually the same drawing facing the other way,
and mirroring is exactly true rather than an approximation — but only the author knows whether
their character is symmetrical enough for it (a staff in one hand crosses over).

Usage:

    python repack_sheet.py SOURCE OUT.png 12,13,14,15
    python repack_sheet.py SOURCE OUT.png 12,13,14,15 ~12,13,14,15   # second row mirrored
"""

import sys

from PIL import Image, ImageOps

from sheet_sprites import sprites_of, tighten


def main() -> int:
    if len(sys.argv) < 4:
        print(__doc__)
        return 2

    source, out = sys.argv[1], sys.argv[2]
    image = Image.open(source).convert("RGBA")
    found = sprites_of(image)

    rows = []
    for raw in sys.argv[3:]:
        mirror = raw.startswith("~")
        indices = [int(n) for n in raw.lstrip("~").split(",") if n.strip()]
        for index in indices:
            if not 0 <= index < len(found):
                print(f"there is no sprite {index}; the sheet has {len(found)}")
                return 1
        frames = [tighten(image, found[i]) for i in indices]
        if mirror:
            frames = [ImageOps.mirror(frame) for frame in frames]
        rows.append(frames)

    # The cell grows to hold the art. Cropping to a smaller one would quietly take a staff or a
    # cape off, which is the single failure a preview cannot show you is *your* mistake.
    columns = max(len(row) for row in rows)
    cell_w = max(frame.width for row in rows for frame in row)
    cell_h = max(frame.height for row in rows for frame in row)

    sheet = Image.new("RGBA", (cell_w * columns, cell_h * len(rows)), (0, 0, 0, 0))
    for r, row in enumerate(rows):
        for c, frame in enumerate(row):
            x = c * cell_w + (cell_w - frame.width) // 2
            y = r * cell_h + (cell_h - frame.height)
            sheet.alpha_composite(frame, (x, y))

    sheet.save(out)
    longest = max(len(row) for row in rows)
    print(f"{out}  {sheet.width}x{sheet.height}")
    print(f"  Cols {columns} · Rows {len(rows)} · Frames {longest * len(rows)} · cells {cell_w}x{cell_h}")
    if any(len(row) != longest for row in rows):
        # A short row leaves empty cells that still play, so the count has to cover the grid and
        # the author has to know some of it is blank.
        print("  ! rows differ in length; the short ones end with empty cells")
    if len(rows) > 1:
        print("  Row directions: one word per row, top row first.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
