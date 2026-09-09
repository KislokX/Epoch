"""Find every sprite on a packed rip sheet.

## Bands, and the ones that are two bands

A rip sheet separates its rows with fully transparent lines, so a run of non-empty rows is a row
of sprites. Usually. On Mage's sheet one band came out 80 pixels tall against a median of 38 —
two rows whose sprites touch vertically, with no transparent line between them. Detected as one,
it produced a 195-pixel-wide "sprite" that was really six frames from two animations.

## Why a straight cut was the wrong fix

The first repair split such a band at its thinnest interior row. It found the seam correctly and
still produced visible rubbish: the thinnest row had 76 pixels crossing it, and those pixels are
**the feet of the row above and the hats of the row below**, occupying the same scanline. Any
horizontal cut leaves a slice of each in the other's frames — which is exactly what showed up as
crescents above and below the walking sprites.

So the band is not cut. Its **connected components** are found and sorted into rows by where
each one's centre of mass lies. A hat belongs to the sprite it is attached to, whatever scanline
it happens to overlap, and nothing is ever sliced through.

## What it will not do

Decide what an animation is. It finds sprites and puts them in reading order; which of them
belong together is in the author's head and nowhere in the pixels.
"""

from PIL import Image

Box = tuple[int, int, int, int]


class Sprite:
    """One frame: where it sits, and which pixels there are actually its own.

    **The box is not enough where two rows share scanlines.** A crop is a rectangle, and in a
    double band the rectangle over one sprite still contains the feet or the hat of its
    neighbour above or below. Carrying the shape means the neighbour can be removed rather than
    trimmed around — which is what the crescents above and below the walking frames were.

    `pixels` is `None` for the ordinary case, where the row has a transparent line of its own
    and the rectangle contains nothing but the sprite.
    """

    __slots__ = ("box", "pixels")

    def __init__(self, box: Box, pixels: set[tuple[int, int]] | None = None):
        self.box = box
        self.pixels = pixels

    def __iter__(self):
        # So a caller can still unpack it as the four numbers it used to be.
        return iter(self.box)

    def __getitem__(self, i):
        return self.box[i]

#: A band this much taller than the sheet's median is two rows that touch.
DOUBLE = 1.5

#: A run this narrow is a stray pixel column, not a frame.
NARROWEST = 6

#: A component this much smaller than the biggest in its band is dust — a stray pixel or two
#: left by the rip. Kept out of the row assignment so it cannot drag a centroid.
DUST = 0.02


def sprites_of(image: Image.Image) -> list[Box]:
    width, height = image.size
    alpha = image.split()[3].load()

    def ink(y: int) -> int:
        return sum(1 for x in range(width) if alpha[x, y])

    bands: list[tuple[int, int]] = []
    y = 0
    while y < height:
        if ink(y) == 0:
            y += 1
            continue
        start = y
        while y < height and ink(y) > 0:
            y += 1
        bands.append((start, y))

    if not bands:
        return []
    heights = sorted(b - a for a, b in bands)
    median = heights[len(heights) // 2]

    found: list[Sprite] = []
    for top, bottom in bands:
        if bottom - top <= median * DOUBLE:
            found.extend(_runs(alpha, width, top, bottom, None))
            continue
        # Two rows sharing scanlines. Split the *shapes*, never the pixels.
        upper, lower = _two_rows(alpha, width, top, bottom)
        for members in (upper, lower):
            if not members:
                continue
            edge = _bounds(members)
            found.extend(_runs(alpha, width, edge[0], edge[1], members))
    return found


def _components(alpha, width: int, top: int, bottom: int) -> list[set[tuple[int, int]]]:
    """Every connected blob of ink in the band, four-connected.

    Iterative rather than recursive: a 195-pixel-wide blob is thousands of pixels deep and
    Python's stack is not.
    """
    seen = set()
    blobs = []
    for y0 in range(top, bottom):
        for x0 in range(width):
            if alpha[x0, y0] == 0 or (x0, y0) in seen:
                continue
            blob = set()
            stack = [(x0, y0)]
            seen.add((x0, y0))
            while stack:
                x, y = stack.pop()
                blob.add((x, y))
                for nx, ny in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
                    if not (0 <= nx < width and top <= ny < bottom):
                        continue
                    if (nx, ny) in seen or alpha[nx, ny] == 0:
                        continue
                    seen.add((nx, ny))
                    stack.append((nx, ny))
            blobs.append(blob)
    return blobs


def _two_rows(alpha, width: int, top: int, bottom: int):
    """Sort a double band's shapes into an upper row and a lower one.

    By centre of mass against the band's middle. A sprite whose hat crosses the midpoint still
    belongs to the row its body is in, which is the whole reason this is not a cut.
    """
    blobs = _components(alpha, width, top, bottom)
    if not blobs:
        return [], []
    biggest = max(len(b) for b in blobs)
    middle = (top + bottom) / 2

    upper, lower = [], []
    for blob in blobs:
        if len(blob) < biggest * DUST:
            continue
        centre = sum(y for _, y in blob) / len(blob)
        (upper if centre < middle else lower).append(blob)
    return upper, lower


def _bounds(blobs) -> tuple[int, int]:
    """The top and bottom of a row, from the shapes actually in it."""
    top = min(y for blob in blobs for _, y in blob)
    bottom = max(y for blob in blobs for _, y in blob) + 1
    return top, bottom


def _runs(alpha, width: int, top: int, bottom: int, members) -> list[Box]:
    """One box per sprite across a row, by columns that have ink in it.

    `members` restricts the ink to one row's own shapes, so a neighbour sharing a scanline
    cannot widen a box or invent one.
    """
    if members is None:

        def has_ink(x: int) -> bool:
            return any(alpha[x, yy] for yy in range(top, bottom))

    else:
        columns = {x for blob in members for x, _ in blob}

        def has_ink(x: int) -> bool:
            return x in columns

    own = None
    if members is not None:
        own = {pixel for blob in members for pixel in blob}

    boxes: list[Sprite] = []
    x = 0
    while x < width:
        if not has_ink(x):
            x += 1
            continue
        left = x
        while x < width and has_ink(x):
            x += 1
        if x - left >= NARROWEST:
            boxes.append(Sprite((left, top, x, bottom), own))
    return boxes


def tighten(image: Image.Image, sprite) -> Image.Image:
    """One frame, with the neighbours removed and then trimmed to its own ink.

    Two steps, and the first is the one that was missing. In a double band the crop still
    contains whatever of the row above or below happens to overlap those scanlines, so every
    pixel not belonging to this row's own shapes is cleared before anything is measured. Then
    the trim, because a row's top and bottom belong to the whole row and a short frame would
    otherwise carry the tallest frame's empty space — a walk cycle bobbing for a reason nobody
    drew.
    """
    box = sprite.box if isinstance(sprite, Sprite) else sprite
    pixels = sprite.pixels if isinstance(sprite, Sprite) else None
    cut = image.crop(box).convert("RGBA")

    if pixels is not None:
        x0, y0, _, _ = box
        clean = Image.new("RGBA", cut.size, (0, 0, 0, 0))
        source = cut.load()
        target = clean.load()
        for y in range(cut.height):
            for x in range(cut.width):
                if (x0 + x, y0 + y) in pixels:
                    target[x, y] = source[x, y]
        cut = clean

    bounds = cut.getbbox()
    return cut.crop(bounds) if bounds else cut
