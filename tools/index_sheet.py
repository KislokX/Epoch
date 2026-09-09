"""Number every sprite on a packed rip sheet, so a person can point at them.

Epoch's cut is a uniform grid; a rip sheet is not one. This does the half a machine can do —
find every sprite and give it a number — and leaves the half only the author can do: saying
which numbers are one animation.

Drawn at 3x on a dark ground, because the numbers sit over 20-pixel sprites on a transparent
sheet and a number over white would vanish on the pale ones.

    python index_sheet.py SOURCE OUT.png
"""

import sys

from PIL import Image, ImageDraw

from sheet_sprites import sprites_of

SCALE = 3


def main() -> int:
    if len(sys.argv) < 3:
        print(__doc__)
        return 2

    source, out = sys.argv[1], sys.argv[2]
    image = Image.open(source).convert("RGBA")
    found = sprites_of(image)

    big = image.resize((image.width * SCALE, image.height * SCALE), Image.NEAREST)
    canvas = Image.new("RGBA", big.size, (14, 11, 22, 255))
    canvas.alpha_composite(big)
    draw = ImageDraw.Draw(canvas)

    for i, (x0, y0, x1, y1) in enumerate(found):
        box = (x0 * SCALE, y0 * SCALE, x1 * SCALE - 1, y1 * SCALE - 1)
        draw.rectangle(box, outline=(232, 183, 74, 190))
        draw.text((box[0] + 2, box[1] + 1), str(i), fill=(255, 230, 160, 255))

    canvas.convert("RGB").save(out)
    print(f"{len(found)} sprites -> {out}")

    seen = None
    for i, (_, y0, _, y1) in enumerate(found):
        if y0 != seen:
            print(f"  row y{y0}..{y1} ({y1 - y0} tall) starts at {i}")
            seen = y0
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
